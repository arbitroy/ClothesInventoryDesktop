use std::collections::HashMap;

use crate::db::{
    dates::day_bounds_ms,
    error::{DbError, DbResult},
    now_ms,
};

use super::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPreviewRow {
    pub product_id: String,
    pub product_name: String,
    pub opening: i64,
    pub total_in: i64,
    pub total_out: i64,
    pub expected: i64, // opening + total_in - total_out
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    pub product_id: String,
    pub counted: i64,
    pub notes: Option<String>,
}

impl Database {
    fn preview_rows(
        conn: &rusqlite::Connection,
        start: i64,
        end: i64,
    ) -> DbResult<Vec<DayPreviewRow>> {
        let mut stmt = conn.prepare(
            "   SELECT p.id, p.name,
                COALESCE(SUM(CASE WHEN t.timestamp < ?1
                THEN (CASE t.type WHEN 'in' THEN t.quantity ELSE -t.quantity END) END), 0),
                COALESCE(SUM(CASE WHEN t.timestamp BETWEEN ?1 AND ?2 AND t.type = 'in'
                THEN t.quantity END), 0),
                COALESCE(SUM(CASE WHEN t.timestamp BETWEEN ?1 AND ?2 AND t.type = 'out'
                THEN t.quantity END), 0)
                FROM products p
                LEFT JOIN transactions t ON t.product_id = p.id
                WHERE p.deleted_at IS NULL
                GROUP BY p.id
                ORDER BY p.name",
        )?;

        let rows = stmt.query_map(params![start, end], |row| {
            let opening: i64 = row.get(2)?;
            let total_in: i64 = row.get(3)?;
            let total_out: i64 = row.get(4)?;
            Ok(DayPreviewRow {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                opening,
                total_in,
                total_out,
                expected: opening + total_in - total_out,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn day_preview(&self, date: &str) -> DbResult<Vec<DayPreviewRow>> {
        let (start, end) = day_bounds_ms(date)?;

        let closed: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM daily_balances WHERE date = ?1)",
            [date],
            |row| row.get(0),
        )?;
        if closed {
            return Err(DbError::DayAlreadyClosed);
        }
        Self::preview_rows(&self.conn, start, end)
    }

    pub fn close_day(
        &mut self,
        date: &str,
        counts: &[DayCount],
        closed_by: &str,
    ) -> DbResult<usize> {
        let (start, end) = day_bounds_ms(date)?;
        let tx = self.conn.transaction()?;

        let closed: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM daily_balances WHERE date = ?1)",
            [date],
            |row| row.get(0),
        )?;
        if closed {
            return Err(DbError::DayAlreadyClosed);
        }

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if date > today.as_str() {
            return Err(DbError::FutureDate);
        }

        let latest: Option<String> =
            tx.query_row("SELECT MAX(date) FROM daily_balances", [], |row| row.get(0))?;
        if let Some(latest) = latest {
            if date < latest.as_str() {
                return Err(DbError::DayOutOfOrder);
            }
        }

        let rows = Self::preview_rows(&tx, start, end)?; // the same numbers staff saw

        // quick lookup: product id -> its count
        let counts_by_id: HashMap<&str, &DayCount> =
            counts.iter().map(|c| (c.product_id.as_str(), c)).collect();

        let now = now_ms();
        for row in &rows {
            let count = counts_by_id
                .get(row.product_id.as_str())
                .ok_or_else(|| DbError::MissingCount(row.product_name.clone()))?;
            if count.counted < 0 {
                return Err(DbError::InvalidCount);
            }
            let variance = count.counted - row.expected;

            tx.execute(
                "INSERT INTO daily_balances
                (date, product_id, opening_balance, total_in, total_out, expected_balance,
                closing_balance, variance, variance_notes, closed_by, closed_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    date,
                    row.product_id,
                    row.opening,
                    row.total_in,
                    row.total_out,
                    row.expected,
                    count.counted,
                    variance,
                    count.notes,
                    closed_by,
                    now
                ],
            )?;

            if variance != 0 {
                let kind = if variance > 0 { "in" } else { "out" };
                let id = uuid::Uuid::now_v7().to_string();
                tx.execute(
                    "INSERT INTO transactions
                    (id, product_id, type, reason, quantity, user_id, notes, timestamp)
                    VALUES (?1, ?2, ?3, 'adjustment', ?4, ?5, ?6, ?7)",
                    params![
                        id,
                        row.product_id,
                        kind,
                        variance.abs(),
                        closed_by,
                        format!("Daily count {date}"),
                        end
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_shows_opening_in_out_expected() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();
        db.create_product("Shirts", None, None).unwrap(); // never moves

        // 5 jeans came in two days ago
        let two_days_ago = chrono::Local::now().timestamp_millis() - 2 * 24 * 60 * 60 * 1000;
        db.conn
            .execute(
                "INSERT INTO transactions (id, product_id, type, quantity, timestamp)
                VALUES ('old', ?1, 'in', 5, ?2)",
                rusqlite::params![jeans, two_days_ago],
            )
            .unwrap();

        // today: 10 in via a receipt, 3 sold
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 10, None).unwrap();
        db.finalize_receipt(&r).unwrap();
        db.record_out(&jeans, 3, "sales", None).unwrap();

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let rows = db.day_preview(&today).unwrap();

        // sorted by name: Jeans, Shirts
        assert_eq!(
            (
                rows[0].opening,
                rows[0].total_in,
                rows[0].total_out,
                rows[0].expected
            ),
            (5, 10, 3, 12)
        );
        assert_eq!(rows[1].expected, 0);
    }

    #[test]
    fn close_day_snaps_stock_to_the_count() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();
        let shirts = db.create_product("Shirts", None, None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 50, None).unwrap();
        db.add_item(&r, &shirts, 20, None).unwrap();
        db.finalize_receipt(&r).unwrap();

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        // a missing count is refused, and nothing is saved
        let only_jeans = [DayCount {
            product_id: jeans.clone(),
            counted: 47,
            notes: None,
        }];
        assert!(matches!(
            db.close_day(&today, &only_jeans, "amina"),
            Err(DbError::MissingCount(_))
        ));
        assert_eq!(db.current_stock(&jeans).unwrap(), 50); // jeans were NOT adjusted

        // jeans short by 3, shirts have 2 extra
        let counts = [
            DayCount {
                product_id: jeans.clone(),
                counted: 47,
                notes: Some("3 damaged".into()),
            },
            DayCount {
                product_id: shirts.clone(),
                counted: 22,
                notes: None,
            },
        ];
        assert_eq!(db.close_day(&today, &counts, "amina").unwrap(), 2);
        assert_eq!(db.current_stock(&jeans).unwrap(), 47);
        assert_eq!(db.current_stock(&shirts).unwrap(), 22);

        // the same day can't be closed twice
        assert!(matches!(
            db.close_day(&today, &counts, "amina"),
            Err(DbError::DayAlreadyClosed)
        ));
    }

    #[test]
    fn days_close_in_order_and_not_in_the_future() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();
        let counts = [DayCount {
            product_id: jeans,
            counted: 0,
            notes: None,
        }];

        let now = chrono::Local::now();
        let fmt = "%Y-%m-%d";
        let today = now.format(fmt).to_string();
        let yesterday = (now - chrono::Duration::days(1)).format(fmt).to_string();
        let tomorrow = (now + chrono::Duration::days(1)).format(fmt).to_string();

        let r = db.close_day(&tomorrow, &counts, "amina");
        assert!(matches!(r, Err(DbError::FutureDate)), "got: {r:?}");

        db.close_day(&today, &counts, "amina").unwrap();

        let r = db.close_day(&yesterday, &counts, "amina");
        assert!(matches!(r, Err(DbError::DayOutOfOrder)), "got: {r:?}");
    }
}
