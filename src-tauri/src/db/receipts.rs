use crate::db::error::{DbError, DbResult};

use super::{Database, now_ms};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub id: String,
    pub container_number: String,
    pub received_date: i64,
    pub notes: Option<String>,
    pub status: String,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerItem {
    pub id: String,
    pub receipt_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity_received: i64,
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptTotals {
    pub total_skus: i64,
    pub total_units: i64,
}

impl Database {
    fn receipt_status(conn: &rusqlite::Connection, receipt_id: &str) -> DbResult<String> {
        conn.query_row(
            "SELECT status FROM container_receipts WHERE id = ?1",
            [receipt_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(DbError::ReceiptNotFound)
    }

    pub fn create_receipt(
        &self,
        container_number: &str,
        received_date: i64,
        notes: Option<&str>,
    ) -> DbResult<String> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO container_receipts
                (id, container_number, received_date, notes, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, container_number, received_date, notes, now],
        )?;
        Ok(id)
    }

    pub fn add_item(
        &self,
        receipt_id: &str,
        product_id: &str,
        quantity: i64,
        notes: Option<&str>,
    ) -> DbResult<String> {
        if Self::receipt_status(&self.conn, receipt_id)? != "draft" {
            return Err(DbError::ReceiptFinalized);
        }

        if quantity < 1 {
            return Err(DbError::InvalidQuantity);
        }

        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1 AND deleted_at IS NULL)",
            [product_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DbError::ProductNotFound);
        }

        let id = uuid::Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO container_items
            (id, receipt_id, product_id, quantity_received, notes, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, receipt_id, product_id, quantity, notes, now_ms()],
        )?;
        Ok(id)
    }

    pub fn receipt_totals(&self, receipt_id: &str) -> DbResult<ReceiptTotals> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(quantity_received), 0)
            FROM container_items
            WHERE receipt_id = ?1",
            [receipt_id],
            |row| {
                Ok(ReceiptTotals {
                    total_skus: row.get(0)?,
                    total_units: row.get(1)?,
                })
            },
        )?)
    }

    pub fn list_items(&self, receipt_id: &str) -> DbResult<Vec<ContainerItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT ci.id, ci.receipt_id, ci.product_id, p.name,
            ci.quantity_received, ci.notes, ci.created_at
            FROM container_items ci
            JOIN products p ON p.id = ci.product_id
            WHERE ci.receipt_id = ?1
            ORDER BY ci.created_at",
        )?;
        let rows = stmt.query_map([receipt_id], |row| {
            Ok(ContainerItem {
                id: row.get(0)?,
                receipt_id: row.get(1)?,
                product_id: row.get(2)?,
                product_name: row.get(3)?,
                quantity_received: row.get(4)?,
                notes: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, rusqlite::Error>>()?)
    }

    pub fn list_receipts(&self) -> DbResult<Vec<Receipt>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, container_number, received_date, notes, status, version, created_at, updated_at, deleted_at 
            FROM container_receipts
            WHERE deleted_at IS NULL
            ORDER BY received_date DESC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Receipt {
                id: row.get(0)?,
                container_number: row.get(1)?,
                received_date: row.get(2)?,
                notes: row.get(3)?,
                status: row.get(4)?,
                version: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, rusqlite::Error>>()?)
    }

    pub fn finalize_receipt(&mut self, receipt_id: &str) -> DbResult<usize> {
        let tx = self.conn.transaction()?;

        if Self::receipt_status(&tx, receipt_id)? != "draft" {
            return Err(DbError::ReceiptFinalized);
        }
        let item_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM container_items WHERE receipt_id = ?1",
            [receipt_id],
            |row| row.get(0),
        )?;
        if item_count == 0 {
            return Err(DbError::EmptyReceipt);
        }
        let changed = tx.execute(
            "UPDATE container_receipts
            SET status = 'finalized', version = version + 1, updated_at = ?1
            WHERE id = ?2 AND status = 'draft'",
            params![now_ms(), receipt_id],
        )?;
        if changed == 0 {
            return Ok(0); // tx is dropped, nothing was written
        }

        // read first, into a Vec
        let items: Vec<(String, String, i64)> = {
            let mut stmt = tx.prepare(
                "SELECT id, product_id, quantity_received
                FROM container_items
                WHERE receipt_id = ?1
                ORDER BY created_at",
            )?;
            let rows = stmt.query_map([receipt_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };

        // then insert one 'in' transaction per item
        for (item_id, product_id, quantity) in &items {
            let id = uuid::Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO transactions
                (id, product_id, type, quantity, receipt_item_id, timestamp)
                VALUES (?1, ?2, 'in', ?3, ?4, ?5)",
                params![id, product_id, quantity, item_id, now_ms()],
            )?;
        }

        tx.commit()?;
        Ok(items.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_item_checks_foreign_keys() {
        let db = Database::open(":memory:").unwrap();
        let product = db.create_product("Jeans", Some("M"), None).unwrap();
        let receipt = db.create_receipt("BALE-001", 0, None).unwrap();

        // works
        assert!(db.add_item(&receipt, &product, 50, None).is_ok());

        // product doesn't exist: the database refuses
        assert!(db.add_item(&receipt, "nope", 50, None).is_err());

        // quantity must be positive: the database refuses
        assert!(db.add_item(&receipt, &product, 0, None).is_err());
    }

    #[test]
    fn totals_add_up() {
        let db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let shirts = db.create_product("Shirts", Some("L"), None).unwrap();
        let receipt = db.create_receipt("BALE-001", 0, None).unwrap();

        db.add_item(&receipt, &jeans, 50, None).unwrap();
        db.add_item(&receipt, &shirts, 30, None).unwrap();

        let t = db.receipt_totals(&receipt).unwrap();
        assert_eq!(t.total_skus, 2);
        assert_eq!(t.total_units, 80);
    }

    #[test]
    fn list_items_only_returns_that_receipt() {
        let db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let r1 = db.create_receipt("BALE-001", 0, None).unwrap();
        let r2 = db.create_receipt("BALE-002", 0, None).unwrap();

        db.add_item(&r1, &jeans, 50, None).unwrap();
        db.add_item(&r1, &jeans, 10, None).unwrap();
        db.add_item(&r2, &jeans, 5, None).unwrap();

        assert_eq!(db.list_items(&r1).unwrap().len(), 2);
        assert_eq!(db.list_items(&r2).unwrap().len(), 1);
    }

    #[test]
    fn finalize_creates_stock_in_once() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let shirts = db.create_product("Shirts", Some("L"), None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 50, None).unwrap();
        db.add_item(&r, &shirts, 30, None).unwrap();
        assert_eq!(db.finalize_receipt(&r).unwrap(), 2);
        // finalizing again does nothing
        assert!(matches!(
            db.finalize_receipt(&r),
            Err(DbError::ReceiptFinalized)
        ));
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2); // still 2, not 4
    }

    #[test]
    fn cannot_add_item_after_finalize() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 50, None).unwrap();
        db.finalize_receipt(&r).unwrap();

        assert!(db.add_item(&r, &jeans, 10, None).is_err());
    }

    #[test]
    fn cannot_finalize_empty_receipt() {
        let mut db = Database::open(":memory:").unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        assert!(matches!(
            db.finalize_receipt(&r),
            Err(DbError::EmptyReceipt)
        ));
    }

    #[test]
    fn item_name_survives_product_delete() {
        let mut db = Database::open(":memory:").unwrap();
        let jeans = db.create_product("Jeans", Some("M"), None).unwrap();
        let r = db.create_receipt("BALE-001", 0, None).unwrap();
        db.add_item(&r, &jeans, 5, None).unwrap();
        db.finalize_receipt(&r).unwrap();
        db.record_out(&jeans, 5, "sales", None).unwrap();
        db.delete_product(&jeans).unwrap();

        let items = db.list_items(&r).unwrap();
        assert_eq!(items[0].product_name, "Jeans");
    }
}
