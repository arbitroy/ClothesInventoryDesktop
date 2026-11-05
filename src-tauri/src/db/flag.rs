use crate::db::{DbError, DbResult};

use super::{Database, now_ms};
use rusqlite::params;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingFlag {
    pub id: String,
    pub reason: String,
    pub created_at: i64,
    pub transaction_id: String,
    pub product_name: String,
    pub transaction_quantity: i64,
    pub transaction_timestamp: i64,
    pub stock_now: i64,
}

impl Database {
    pub fn list_pending_flags(&self) -> DbResult<Vec<PendingFlag>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.reason, f.created_at,
            t.id, p.name, t.quantity, t.timestamp,
            (SELECT COALESCE(SUM(CASE type WHEN 'in' THEN quantity
                                       WHEN 'out' THEN -quantity END), 0)
            FROM transactions WHERE product_id = t.product_id) AS stock_now
            FROM review_flags f
            JOIN transactions t ON t.id = f.entity_id
            JOIN products p ON p.id = t.product_id
            WHERE f.entity_type = 'transaction' AND f.status = 'pending'
            ORDER BY f.created_at",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PendingFlag {
                id: row.get(0)?,
                reason: row.get(1)?,
                created_at: row.get(2)?,
                transaction_id: row.get(3)?,
                product_name: row.get(4)?,
                transaction_quantity: row.get(5)?,
                transaction_timestamp: row.get(6)?,
                stock_now: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn resolve_flag(&self, flag_id: &str, approve: bool) -> DbResult<()> {
        let status = if approve { "approved" } else { "rejected" };
        let changed = self.conn.execute(
            "UPDATE review_flags SET status = ?1, resolved_at = ?2
         WHERE id = ?3 AND status = 'pending'",
            params![status, now_ms(), flag_id],
        )?;
        if changed == 0 {
            return Err(DbError::FlagNotPending);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversell_flag_can_be_resolved() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 5, None).unwrap();
        db.finalize_receipt(&r).unwrap();

        db.record_out(&jeans, 8, "sales", None).unwrap(); // 5 in stock, sell 8

        let flags = db.list_pending_flags().unwrap();
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].transaction_quantity, 8);
        assert_eq!(flags[0].stock_now, -3);

        db.resolve_flag(&flags[0].id, true).unwrap();
        assert!(db.list_pending_flags().unwrap().is_empty());

        // resolving again is refused
        assert!(matches!(
            db.resolve_flag(&flags[0].id, true),
            Err(DbError::FlagNotPending)
        ));
    }
}