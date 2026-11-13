use crate::db::{
    dates::day_bounds_ms,
    error::{DbError, DbResult},
};

use super::Database;
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

        // prepare the query above, pass params![start, end],
        // build each row, computing `expected` from the other three
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
