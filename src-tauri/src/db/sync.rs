use std::collections::HashMap;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub device_id: String,
    pub created_at: i64,
    pub desktop: Value,
    pub incoming: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptData {
    container_number: String,
    received_date: i64,
    notes: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemData {
    receipt_id: String,
    product_id: String,
    quantity_received: i64,
    notes: Option<String>,
    created_at: i64,
}

#[derive(Deserialize)]
struct StatusData {
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingChange {
    pub entity: String,
    pub id: String,
    pub data: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    pub cursor: i64,
    pub has_more: bool,
    pub changes: Vec<OutgoingChange>,
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

    fn insert_receipt(conn: &Connection, c: &IncomingChange) -> DbResult<&'static str> {
        if Self::row_exists(conn, "container_receipts", &c.id)? {
            return Ok("duplicate");
        }
        let d: ReceiptData = serde_json::from_value(c.data.clone())?;
        // always starts as a draft; finalizing arrives as its own update
        conn.execute(
            "INSERT INTO container_receipts
           (id, container_number, received_date, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                c.id,
                d.container_number,
                d.received_date,
                d.notes,
                d.created_at,
                d.updated_at
            ],
        )?;
        Ok("applied")
    }

    fn insert_item(conn: &Connection, c: &IncomingChange) -> DbResult<&'static str> {
        if Self::row_exists(conn, "container_items", &c.id)? {
            return Ok("duplicate");
        }
        let d: ItemData = serde_json::from_value(c.data.clone())?;
        conn.execute(
            "INSERT INTO container_items
           (id, receipt_id, product_id, quantity_received, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                c.id,
                d.receipt_id,
                d.product_id,
                d.quantity_received,
                d.notes,
                d.created_at
            ],
        )?;
        Ok("applied")
    }

    fn apply_receipt_update(conn: &Connection, c: &IncomingChange) -> DbResult<&'static str> {
        let d: StatusData = serde_json::from_value(c.data.clone())?;
        if d.status != "finalized" {
            return Err(DbError::UnsupportedChange); // the only edit a phone can make
        }
        let already_finalized: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM container_receipts WHERE id = ?1 AND status = 'finalized')",
            [&c.id],
            |row| row.get(0),
        )?;
        if already_finalized {
            return Ok("duplicate"); // already finalized, same outcome, nothing to fight over
        }
        conn.execute(
            "UPDATE container_receipts
         SET status = 'finalized', version = version + 1, updated_at = ?1
         WHERE id = ?2",
            params![now_ms(), c.id],
        )?;
        Ok("applied")
    }

    fn apply_one(conn: &Connection, device_id: &str, c: &IncomingChange) -> DbResult<&'static str> {
        match (c.entity.as_str(), c.op.as_str()) {
            ("product", "insert") => Self::insert_product(conn, c),
            ("transaction", "insert") => Self::insert_transaction(conn, c),
            ("product", "update") => Self::apply_product_update(conn, device_id, c),
            ("receipt", "insert") => Self::insert_receipt(conn, c),
            ("receipt", "update") => Self::apply_receipt_update(conn, c),
            ("container_item", "insert") => Self::insert_item(conn, c),
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
        if d.kind == "in" && d.reason.is_none() {
            let item_id = d.receipt_item_id.as_deref().ok_or(DbError::BadStockInId)?;
            if c.id != format!("in:{item_id}") {
                return Err(DbError::BadStockInId);
            }
        }
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

    pub fn list_pending_conflicts(&self) -> DbResult<Vec<ConflictView>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.entity_type, c.entity_id, c.device_id, c.created_at, c.incoming,
                p.name, p.size, p.notes, p.deleted_at
                FROM conflicts c
                JOIN products p ON p.id = c.entity_id
                WHERE c.status = 'pending' AND c.entity_type = 'product'
                ORDER BY c.created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let incoming: String = row.get(5)?;
            Ok(ConflictView {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                device_id: row.get(3)?,
                created_at: row.get(4)?,
                incoming: serde_json::from_str(&incoming).unwrap_or(Value::Null),
                desktop: serde_json::json!({
                    "name": row.get::<_, String>(6)?,
                    "size": row.get::<_, Option<String>>(7)?,
                    "notes": row.get::<_, Option<String>>(8)?,
                    "deleted": row.get::<_, Option<i64>>(9)?.is_some(),
                }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn resolve_conflict(&mut self, conflict_id: &str, use_phone: bool) -> DbResult<()> {
        let tx = self.conn.transaction()?;

        let (entity_type, entity_id, incoming): (String, String, String) = tx
            .query_row(
                "SELECT entity_type, entity_id, incoming
                FROM conflicts WHERE id = ?1 AND status = 'pending'",
                [conflict_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(DbError::ConflictNotPending)?;
        if entity_type != "product" {
            return Err(DbError::UnsupportedChange);
        }

        let now = now_ms();
        if use_phone {
            let deleted: Option<i64> = tx.query_row(
                "SELECT deleted_at FROM products WHERE id = ?1",
                [&entity_id],
                |row| row.get(0),
            )?;
            if deleted.is_some() {
                return Err(DbError::ProductDeleted);
            }
            let d: ProductData = serde_json::from_str(&incoming)?;
            tx.execute(
                "UPDATE products
                    SET name = ?1, size = ?2, notes = ?3, version = version + 1, updated_at = ?4
                    WHERE id = ?5",
                params![d.name, d.size, d.notes, now, entity_id],
            )?;
        } else {
            // keep the desktop's data, but still move the version on
            tx.execute(
                "UPDATE products SET version = version + 1, updated_at = ?1 WHERE id = ?2",
                params![now, entity_id],
            )?;
        }

        tx.execute(
            "UPDATE conflicts SET status = 'resolved', resolution = ?1, resolved_at = ?2
            WHERE id = ?3",
            params![
                if use_phone {
                    "use_phone"
                } else {
                    "keep_desktop"
                },
                now,
                conflict_id
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn changes_since(&self, since: i64, limit: i64) -> DbResult<PullResult> {
        let limit = limit.clamp(1, 1000);

        // read one page of the log (one extra row tells us if there is more)
        let mut stmt = self.conn.prepare(
            "SELECT seq, entity_type, entity_id FROM change_log
         WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )?;
        let mut log: Vec<(i64, String, String)> = stmt
            .query_map(params![since, limit + 1], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;

        let has_more = log.len() as i64 > limit;
        log.truncate(limit as usize);
        let cursor = log.last().map(|(seq, _, _)| *seq).unwrap_or(since);

        // one entry per thing, parents before children
        let mut latest: HashMap<(String, String), i64> = HashMap::new();
        for (seq, entity, id) in log {
            latest.insert((entity, id), seq); // later entries overwrite earlier ones
        }
        let mut order: Vec<((String, String), i64)> = latest.into_iter().collect();
        order.sort_by_key(|(key, seq)| (Self::dependency_rank(&key.0), *seq));

        let mut changes = Vec::with_capacity(order.len());
        for ((entity, id), _) in order {
            if let Some(data) = Self::entity_json(&self.conn, &entity, &id)? {
                changes.push(OutgoingChange { entity, id, data });
            }
        }
        Ok(PullResult {
            cursor,
            has_more,
            changes,
        })
    }

    fn dependency_rank(entity: &str) -> u8 {
        match entity {
            "product" => 0,
            "receipt" => 1,
            "container_item" => 2,
            "transaction" => 3,
            _ => 4, // daily_balance
        }
    }

    // The current row as JSON, built by SQLite itself with json_object.
    fn entity_json(conn: &Connection, entity: &str, id: &str) -> DbResult<Option<Value>> {
        let text: Option<String> = if entity == "daily_balance" {
            // its id is "date|productId"
            let Some((date, product_id)) = id.split_once('|') else {
                return Ok(None);
            };
            conn.query_row(
                "SELECT json_object('date', date, 'productId', product_id,
                'openingBalance', opening_balance, 'totalIn', total_in, 'totalOut', total_out,
                'expectedBalance', expected_balance, 'closingBalance', closing_balance,
                'variance', variance, 'varianceNotes', variance_notes,
                'closedBy', closed_by, 'closedAt', closed_at)
             FROM daily_balances WHERE date = ?1 AND product_id = ?2",
                params![date, product_id],
                |row| row.get(0),
            )
            .optional()?
        } else {
            let sql = match entity {
                "product" => {
                    "SELECT json_object('id', id, 'name', name, 'size', size, 'notes', notes,
                'version', version, 'createdAt', created_at, 'updatedAt', updated_at,
                'deletedAt', deleted_at) FROM products WHERE id = ?1"
                }
                "receipt" => {
                    "SELECT json_object('id', id, 'containerNumber', container_number,
                'receivedDate', received_date, 'notes', notes, 'status', status,
                'version', version, 'createdAt', created_at, 'updatedAt', updated_at,
                'deletedAt', deleted_at) FROM container_receipts WHERE id = ?1"
                }
                "container_item" => {
                    "SELECT json_object('id', id, 'receiptId', receipt_id,
                'productId', product_id, 'quantityReceived', quantity_received,
                'notes', notes, 'createdAt', created_at) FROM container_items WHERE id = ?1"
                }
                "transaction" => {
                    "SELECT json_object('id', id, 'productId', product_id, 'type', type,
                'reason', reason, 'quantity', quantity, 'userId', user_id, 'notes', notes,
                'receiptItemId', receipt_item_id, 'pricePerUnit', price_per_unit,
                'totalPrice', total_price, 'timestamp', timestamp)
                FROM transactions WHERE id = ?1"
                }
                _ => return Ok(None),
            };
            conn.query_row(sql, [id], |row| row.get(0)).optional()?
        };
        Ok(match text {
            Some(t) => Some(serde_json::from_str(&t)?),
            None => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::now_ms;
    use serde_json::json;

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

    fn product_with_conflict(db: &mut Database) -> String {
        let id = db.create_product("Jeans", None, None).unwrap(); // version 1
        let edit = |change_id: &str, name: &str| IncomingChange {
            change_id: change_id.into(),
            entity: "product".into(),
            id: id.clone(),
            op: "update".into(),
            base_version: Some(1),
            data: serde_json::json!({
                "name": name, "size": null, "notes": null, "createdAt": 0, "updatedAt": 0
            }),
        };
        db.apply_changes("phone-1", &[edit("c1", "Jeans (blue)")])
            .unwrap(); // applied, now v2
        db.apply_changes("phone-2", &[edit("c2", "Denim")]).unwrap(); // conflict
        id
    }

    fn log_count(db: &Database, id: &str) -> i64 {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM change_log WHERE entity_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
    }

    #[test]
    fn resolving_for_the_phone_applies_its_version() {
        let mut db = Database::open(":memory:").unwrap();
        let id = product_with_conflict(&mut db);

        let conflicts = db.list_pending_conflicts().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].desktop["name"], "Jeans (blue)");
        assert_eq!(conflicts[0].incoming["name"], "Denim");

        let logged_before = log_count(&db, &id);
        db.resolve_conflict(&conflicts[0].id, true).unwrap();

        let p = db.get_product(&id).unwrap().unwrap();
        assert_eq!((p.name.as_str(), p.version), ("Denim", 3));
        assert!(db.list_pending_conflicts().unwrap().is_empty());
        assert!(log_count(&db, &id) > logged_before); // the phone will pull it

        // resolving again is refused
        let r = db.resolve_conflict(&conflicts[0].id, true);
        assert!(matches!(r, Err(DbError::ConflictNotPending)), "got: {r:?}");
    }

    #[test]
    fn resolving_for_the_desktop_keeps_data_but_moves_the_version_on() {
        let mut db = Database::open(":memory:").unwrap();
        let id = product_with_conflict(&mut db);

        let conflict_id = db.list_pending_conflicts().unwrap()[0].id.clone();
        let logged_before = log_count(&db, &id);
        db.resolve_conflict(&conflict_id, false).unwrap();

        let p = db.get_product(&id).unwrap().unwrap();
        assert_eq!((p.name.as_str(), p.version), ("Jeans (blue)", 3));
        assert!(log_count(&db, &id) > logged_before);
    }

    #[test]
    fn double_finalize_never_doubles_stock() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();

        let change =
            |cid: &str, entity: &str, id: &str, op: &str, data: serde_json::Value| IncomingChange {
                change_id: cid.into(),
                entity: entity.into(),
                id: id.into(),
                op: op.into(),
                base_version: None,
                data,
            };

        // the phone creates a receipt with one item and pushes both
        let res = db.apply_changes("phone-1", &[
        change("c1", "receipt", "r1", "insert",
            json!({"containerNumber": "BALE-009", "receivedDate": 0, "createdAt": 0, "updatedAt": 0})),
        change("c2", "container_item", "i1", "insert",
            json!({"receiptId": "r1", "productId": jeans, "quantityReceived": 10, "createdAt": 0})),
    ]).unwrap();
        assert!(res.iter().all(|r| r.status == "applied"), "got: {res:?}");

        // the desktop finalizes it first: stock +10
        db.finalize_receipt("r1").unwrap();
        assert_eq!(db.current_stock(&jeans).unwrap(), 10);

        // the phone finalized it too, and pushes its own version
        let res = db.apply_changes("phone-1", &[
        change("c3", "receipt", "r1", "update", json!({"status": "finalized"})),
        change("c4", "transaction", "in:i1", "insert",
            json!({"productId": jeans, "type": "in", "receiptItemId": "i1", "quantity": 10, "timestamp": 1})),
    ]).unwrap();
        assert_eq!(res[0].status, "duplicate");
        assert_eq!(res[1].status, "duplicate");
        assert_eq!(db.current_stock(&jeans).unwrap(), 10); // not 20

        // a stock-in with a random id is refused
        let res = db.apply_changes("phone-1", &[
        change("c5", "transaction", "random-id", "insert",
            json!({"productId": jeans, "type": "in", "receiptItemId": "i1", "quantity": 10, "timestamp": 1})),
    ]).unwrap();
        assert_eq!(res[0].status, "rejected", "got: {res:?}");
    }

    #[test]
    fn pull_sends_current_state_in_order_and_pages_correctly() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", None, None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 5, None).unwrap();
        db.finalize_receipt(&r).unwrap();
        let mut p = db.get_product(&jeans).unwrap().unwrap();
        p.name = "Jeans (blue)".into();
        db.update_product(&p).unwrap(); // the product has now changed twice

        // everything at once: each thing once, parents first, latest data
        let all = db.changes_since(0, 200).unwrap();
        assert!(!all.has_more);
        let kinds: Vec<&str> = all.changes.iter().map(|c| c.entity.as_str()).collect();
        assert_eq!(
            kinds,
            ["product", "receipt", "container_item", "transaction"]
        );
        assert_eq!(all.changes[0].data["name"], "Jeans (blue)");
        assert_eq!(all.changes[1].data["status"], "finalized");
        assert!(all.changes[3].id.starts_with("in:"));

        // nothing new after the cursor...
        let none = db.changes_since(all.cursor, 200).unwrap();
        assert!(none.changes.is_empty());
        assert_eq!(none.cursor, all.cursor);

        // ...until something changes
        db.record_out(&jeans, 1, "sales", None).unwrap();
        let next = db.changes_since(all.cursor, 200).unwrap();
        assert_eq!(next.changes.len(), 1);
        assert_eq!(next.changes[0].entity, "transaction");

        // paging, two log entries at a time, still reaches everything
        let mut seen = std::collections::HashSet::new();
        let mut cursor = 0;
        loop {
            let page = db.changes_since(cursor, 2).unwrap();
            for c in &page.changes {
                seen.insert((c.entity.clone(), c.id.clone()));
            }
            cursor = page.cursor;
            if !page.has_more {
                break;
            }
        }
        assert_eq!(seen.len(), 5); // the 4 above plus the new sale
    }
}
