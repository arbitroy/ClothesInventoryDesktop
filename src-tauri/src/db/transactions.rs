use crate::db::{DbError, DbResult};

use super::{Database, now_ms};
use rusqlite::params;

impl Database {
    pub fn current_stock(&self, product_id: &str) -> DbResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN type = 'in' THEN quantity ELSE -quantity END), 0)
            FROM transactions
            WHERE product_id = ?1",
            [product_id],
            |row| row.get(0),
        )?)
    }

    pub fn record_out(
        &mut self,
        product_id: &str,
        quantity: i64,
        reason: &str,
        price_per_unit: Option<i64>,
    ) -> DbResult<String> {
        if quantity < 1 {
            return Err(DbError::InvalidQuantity);
        }
        if !["sales", "damage", "adjustment"].contains(&reason) {
            return Err(DbError::InvalidReason);
        }
        Self::ensure_product_active(&self.conn, product_id)?;
        let tx = self.conn.transaction()?;
        let id = uuid::Uuid::now_v7().to_string();

        tx.execute(
            "INSERT INTO transactions
                (id, product_id, type, quantity, price_per_unit, total_price, reason, timestamp)
                VALUES (?1, ?2, 'out', ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                product_id,
                quantity,
                price_per_unit,
                price_per_unit.map(|price| price * quantity),
                reason,
                now_ms()
            ],
        )?;
        Self::flag_if_negative(&tx, product_id, &id)?;
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

    #[test]
    fn in_adjustment_allowed_but_not_other_in_reasons() {
        let db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();

        let ok = db.conn.execute(
            "INSERT INTO transactions (id, product_id, type, reason, quantity, timestamp)
            VALUES ('t1', ?1, 'in', 'adjustment', 2, 0)",
            [&jeans],
        );
        assert!(ok.is_ok());

        let bad = db.conn.execute(
            "INSERT INTO transactions (id, product_id, type, reason, quantity, timestamp)
            VALUES ('t2', ?1, 'in', 'sales', 2, 0)",
            [&jeans],
        );
        assert!(bad.is_err());
    }

    #[test]
    fn record_out_refuses_bad_input() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();

        let r = db.record_out(&jeans, 0, "sales", None);
        assert!(matches!(r, Err(DbError::InvalidQuantity)), "got: {r:?}");

        let r = db.record_out(&jeans, 1, "gift", None);
        assert!(matches!(r, Err(DbError::InvalidReason)), "got: {r:?}");

        let r = db.record_out("nope", 1, "sales", None);
        assert!(matches!(r, Err(DbError::ProductNotFound)), "got: {r:?}");
    }
}
