use super::{Database, now_ms};
use rusqlite::{OptionalExtension, params};

impl Database {
    pub fn current_stock(&self, product_id: &str) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN type = 'in' THEN quantity ELSE -quantity END), 0)
            FROM transactions
            WHERE product_id = ?1",
            [product_id],
            |row| row.get(0),
        )
    }

    pub fn record_out(
        &mut self,
        product_id: &str,
        quantity: i64,
        out_reason: &str,
        price_per_unit: Option<i64>,
    ) -> rusqlite::Result<String> {
        let tx = self.conn.transaction()?;
        let id = uuid::Uuid::now_v7().to_string();

        tx.execute(
            "INSERT INTO transactions
                (id, product_id, type, quantity, price_per_unit, total_price, out_reason, timestamp)
                VALUES (?1, ?2, 'out', ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                product_id,
                quantity,
                price_per_unit,
                price_per_unit.map(|price| price * quantity),
                out_reason,
                now_ms()
            ],
        )?;
        let stock: i64 = tx.query_row(
            "SELECT COALESCE(SUM(CASE WHEN type = 'in' THEN quantity ELSE -quantity END), 0)
            FROM transactions
            WHERE product_id = ?1",
            [product_id],
            |row| row.get(0),
        )?;
        if stock.is_negative() {
            let review_id = uuid::Uuid::now_v7().to_string();
            tx.execute(
                "
            INSERT INTO review_flags
            (id , entity_type, entity_id, reason, status, created_at)
            VALUES
            (?1, 'transaction', ?2, 'negative_stock', 'pending', ?3)
            ",
                params![review_id, id, now_ms()],
            )?;
        }

        tx.commit()?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overselling_is_recorded_and_flagged() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 5, None).unwrap();
        db.finalize_receipt(&r).unwrap();

        // stock is 5, selling 3 is fine: no flag
        db.record_out(&jeans, 3, "sales", Some(1000)).unwrap();
        assert_eq!(db.current_stock(&jeans).unwrap(), 2);

        // selling 4 more goes to -2: recorded, but flagged
        db.record_out(&jeans, 4, "sales", Some(1000)).unwrap();
        assert_eq!(db.current_stock(&jeans).unwrap(), -2);

        let flags: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM review_flags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(flags, 1);
    }
}
