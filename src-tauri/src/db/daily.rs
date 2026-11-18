use crate::db::{
    dates::day_bounds_ms,
    error::{DbError, DbResult},
};

use super::Database;
use rusqlite::params;
use serde::Serialize;

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

impl Database {
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

        let mut stmt = self.conn.prepare(
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
}
