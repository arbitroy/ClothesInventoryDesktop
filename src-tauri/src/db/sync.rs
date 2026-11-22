use crate::db::now_ms;

use super::dates::day_bounds_ms;
use super::{Database, DbError, DbResult};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingChange {
    pub entity: String, // "product", "transaction", ...
    pub change_id: String,
    pub id: String,
    pub op: String, // "insert" or "update"
    pub base_version: Option<i64>,
    pub data: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResult {
    pub change_id: String,
    pub status: &'static str, // applied | duplicate | conflict | rejected
    pub message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductData {
    name: String,
    size: Option<String>,
    notes: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionData {
    product_id: String,
    #[serde(rename = "type")]
    kind: String,
    reason: Option<String>,
    quantity: i64,
    user_id: Option<String>,
    notes: Option<String>,
    receipt_item_id: Option<String>,
    price_per_unit: Option<i64>,
    total_price: Option<i64>,
    timestamp: i64,
}

impl Database {
    fn previous_answer(conn: &Connection, change_id: &str) -> DbResult<Option<String>> {
        Ok(conn
            .query_row(
                "SELECT status FROM applied_changes WHERE change_id = ?1",
                [change_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn remember(conn: &Connection, device_id: &str, change_id: &str, status: &str) -> DbResult<()> {
        conn.execute(
            "INSERT INTO applied_changes (change_id, device_id, status, applied_at)
            VALUES (?1, ?2, ?3, ?4)",
            params![change_id, device_id, status, now_ms()],
        )?;
        Ok(())
    }

    fn apply_product_update(
        conn: &Connection,
        device_id: &str,
        c: &IncomingChange,
    ) -> DbResult<&'static str> {
        let base = c.base_version.ok_or(DbError::MissingBaseVersion)?;
        let d: ProductData = serde_json::from_value(c.data.clone())?;

        let (version, deleted_at): (i64, Option<i64>) = conn
            .query_row(
                "SELECT version, deleted_at FROM products WHERE id = ?1",
                [&c.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DbError::ProductNotFound)?;

        if version == base && deleted_at.is_none() {
            conn.execute(
                "UPDATE products
                SET name = ?1, size = ?2, notes = ?3, version = version + 1, updated_at = ?4
                WHERE id = ?5",
                params![d.name, d.size, d.notes, now_ms(), c.id],
            )?;
            Ok("applied")
        } else {
            // someone changed it first: keep both versions, overwrite nothing
            conn.execute(
                "INSERT INTO conflicts
                (id, entity_type, entity_id, device_id, base_version, incoming, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    uuid::Uuid::now_v7().to_string(),
                    c.entity,
                    c.id,
                    device_id,
                    base,
                    c.data.to_string(),
                    now_ms()
                ],
            )?;
            Ok("conflict")
        }
    }

    pub fn apply_changes(
        &mut self,
        device_id: &str,
        changes: &[IncomingChange],
    ) -> DbResult<Vec<ChangeResult>> {
        let mut tx = self.conn.transaction()?;
        let mut results = Vec::with_capacity(changes.len());

        for change in changes {
            let reply = |status: &'static str, message: Option<String>| ChangeResult {
                change_id: change.change_id.clone(),
                status,
                message,
            };

            // seen before? Answer the same thing again
            if let Some(previous) = Self::previous_answer(&tx, &change.change_id)? {
                let status = if previous == "conflict" {
                    "conflict"
                } else {
                    "duplicate"
                };
                results.push(reply(status, None));
                continue;
            }

            let sp = tx.savepoint()?;
            match Self::apply_one(&sp, device_id, change) {
                Ok(status) => {
                    if status != "duplicate" {
                        Self::remember(&sp, device_id, &change.change_id, status)?;
                    }
                    sp.commit()?;
                    results.push(reply(status, None));
                }
                // sp is dropped here, which undoes just this change
                Err(e) => results.push(reply("rejected", Some(e.to_string()))),
            }
        }
        tx.commit()?;
        Ok(results)
    }

    fn apply_one(conn: &Connection, device_id: &str, c: &IncomingChange) -> DbResult<&'static str> {
        match (c.entity.as_str(), c.op.as_str()) {
            ("product", "insert") => Self::insert_product(conn, c),
            ("transaction", "insert") => Self::insert_transaction(conn, c),
            ("product", "update") => Self::apply_product_update(conn, device_id, c),
            _ => Err(DbError::UnsupportedChange),
        }
    }

    fn insert_product(conn: &Connection, c: &IncomingChange) -> DbResult<&'static str> {
        if Self::row_exists(conn, "products", &c.id)? {
            return Ok("duplicate");
        }
        let d: ProductData = serde_json::from_value(c.data.clone())?;
        conn.execute(
            "INSERT INTO products (id, name, size, notes, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![c.id, d.name, d.size, d.notes, d.created_at, d.updated_at],
        )?;
        Ok("applied")
    }

    fn insert_transaction(conn: &Connection, c: &IncomingChange) -> DbResult<&'static str> {
        if Self::row_exists(conn, "transactions", &c.id)? {
            return Ok("duplicate");
        }
        let d: TransactionData = serde_json::from_value(c.data.clone())?;
        conn.execute(
            "INSERT INTO transactions
            (id, product_id, type, reason, quantity, user_id, notes,
                receipt_item_id, price_per_unit, total_price, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                c.id,
                d.product_id,
                d.kind,
                d.reason,
                d.quantity,
                d.user_id,
                d.notes,
                d.receipt_item_id,
                d.price_per_unit,
                d.total_price,
                d.timestamp
            ],
        )?;

        if d.kind == "out" {
            Self::flag_if_negative(conn, &d.product_id, &c.id)?;
        }
        Self::flag_if_late(conn, &c.id, d.timestamp)?;
        Ok("applied")
    }

    // A transaction dated inside a day that's already closed.
    fn flag_if_late(conn: &Connection, transaction_id: &str, timestamp: i64) -> DbResult<()> {
        let latest: Option<String> =
            conn.query_row("SELECT MAX(date) FROM daily_balances", [], |row| row.get(0))?;
        if let Some(date) = latest {
            let (_, end) = day_bounds_ms(&date)?;
            if timestamp <= end {
                Self::insert_flag(conn, transaction_id, "late_arrival")?;
            }
        }
        Ok(())
    }

    // `table` must always be a literal from our own code, never user input.
    fn row_exists(conn: &Connection, table: &str, id: &str) -> DbResult<bool> {
        Ok(conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [id],
            |row| row.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::now_ms;

    #[test]
    fn push_applies_once_flags_oversell_and_isolates_failures() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 5, None).unwrap();
        db.finalize_receipt(&r).unwrap(); // 5 in stock on the desktop

        let sale = |id: &str, qty: i64| IncomingChange {
            entity: "transaction".into(),
            change_id: format!("c-{id}"),
            id: id.into(),
            op: "insert".into(),
            base_version: None,
            data: serde_json::json!({
                "productId": jeans, "type": "out", "reason": "sales",
                "quantity": qty, "timestamp": now_ms()
            }),
        };

        let res = db.apply_changes("phone-1", &[sale("t1", 8)]).unwrap();
        assert_eq!(res[0].status, "applied");
        assert_eq!(db.current_stock(&jeans).unwrap(), -3);
        assert_eq!(db.list_pending_flags().unwrap().len(), 1); // oversell flagged

        // the same change again does nothing
        let res = db.apply_changes("phone-1", &[sale("t1", 8)]).unwrap();
        assert_eq!(res[0].status, "duplicate");
        assert_eq!(db.current_stock(&jeans).unwrap(), -3);

        // a bad change is rejected, the good one next to it still applies
        let res = db
            .apply_changes("phone-1", &[sale("t2", 0), sale("t3", 1)])
            .unwrap();
        assert_eq!(res[0].status, "rejected", "got: {res:?}");
        assert_eq!(res[1].status, "applied");
    }

    #[test]
    fn product_edit_applies_or_becomes_a_conflict() {
        let mut db = Database::open(":memory:").unwrap();
        let id = db.create_product("Jeans", None, None).unwrap(); // version 1

        let edit = |change_id: &str, name: &str, base: i64| IncomingChange {
            change_id: change_id.into(),
            entity: "product".into(),
            id: id.clone(),
            op: "update".into(),
            base_version: Some(base),
            data: serde_json::json!({
                "name": name, "size": null, "notes": null, "createdAt": 0, "updatedAt": 0
            }),
        };
        let pending = |db: &Database| -> i64 {
            db.conn
                .query_row(
                    "SELECT COUNT(*) FROM conflicts WHERE status = 'pending'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };

        // edit from version 1: applied, the product is now version 2
        let res = db
            .apply_changes("phone-1", &[edit("c1", "Jeans (blue)", 1)])
            .unwrap();
        assert_eq!(res[0].status, "applied");
        assert_eq!(db.get_product(&id).unwrap().unwrap().version, 2);

        // the same change retried: duplicate, not a false conflict
        let res = db
            .apply_changes("phone-1", &[edit("c1", "Jeans (blue)", 1)])
            .unwrap();
        assert_eq!(res[0].status, "duplicate");
        assert_eq!(pending(&db), 0);

        // a different edit that also started from version 1: conflict, nothing overwritten
        let res = db
            .apply_changes("phone-2", &[edit("c2", "Denim", 1)])
            .unwrap();
        assert_eq!(res[0].status, "conflict");
        assert_eq!(db.get_product(&id).unwrap().unwrap().name, "Jeans (blue)");
        assert_eq!(pending(&db), 1);

        // retrying the conflicting change doesn't add a second conflict
        let res = db
            .apply_changes("phone-2", &[edit("c2", "Denim", 1)])
            .unwrap();
        assert_eq!(res[0].status, "conflict");
        assert_eq!(pending(&db), 1);
    }
}
