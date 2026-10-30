use super::{Database, now_ms};
use rusqlite::params;

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ContainerItem {
    pub id: String,
    pub receipt_id: String,
    pub product_id: String,
    pub quantity_received: i64,
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug)]
pub struct ReceiptTotals {
    pub total_skus: i64,
    pub total_units: i64,
}

impl Database {
    pub fn create_receipt(
        &self,
        container_number: &str,
        received_date: i64,
        notes: Option<&str>,
    ) -> rusqlite::Result<String> {
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
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO container_items
                (id, receipt_id,  product_id, quantity_received, notes, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, receipt_id, product_id, quantity, notes, now],
        )?;
        Ok(id)
    }

    pub fn receipt_totals(&self, receipt_id: &str) -> rusqlite::Result<ReceiptTotals> {
        self.conn.query_row(
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
        )
    }

    pub fn list_items(&self, receipt_id: &str) -> rusqlite::Result<Vec<ContainerItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, receipt_id, product_id, quantity_received, notes, created_at
         FROM container_items
         WHERE receipt_id = ?1
         ORDER BY created_at",
        )?;
        let rows = stmt.query_map([receipt_id], |row| {
            Ok(ContainerItem {
                id: row.get(0)?,
                receipt_id: row.get(1)?,
                product_id: row.get(2)?,
                quantity_received: row.get(3)?,
                notes: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
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
}
