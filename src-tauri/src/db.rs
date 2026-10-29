use rusqlite::{Connection, OptionalExtension, params};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub size: Option<String>,
    pub notes: Option<String>,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

pub struct Database {
    conn: Connection,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl Database {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS products (
                id         TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                size       TEXT,
                notes      TEXT,
                version    INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn create_product(
        &self,
        name: &str,
        size: Option<&str>,
        notes: Option<&str>,
    ) -> rusqlite::Result<String> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO products (id, name, size, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, name, size, notes, now],
        )?;
        Ok(id)
    }

    pub fn list_products(&self) -> rusqlite::Result<Vec<Product>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, size, notes, version, created_at, updated_at, deleted_at
             FROM products
             WHERE deleted_at IS NULL
             ORDER BY name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Product {
                id: row.get(0)?,
                name: row.get(1)?,
                size: row.get(2)?,
                notes: row.get(3)?,
                version: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                deleted_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_product(&self, id: &str) -> rusqlite::Result<Option<Product>> {
        self.conn
            .query_row(
                "SELECT id, name, size, notes, version, created_at, updated_at, deleted_at
             FROM products
             WHERE id = ?",
                [id],
                |row| {
                    Ok(Product {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        size: row.get(2)?,
                        notes: row.get(3)?,
                        version: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        deleted_at: row.get(7)?,
                    })
                },
            )
            .optional()
    }

    pub fn update_product(&self, p: &Product) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE products
                SET name = ?1, size = ?2, notes = ?3,
                version = version + 1, updated_at = ?4
                WHERE id = ?5 AND version = ?6",
            params![p.name, p.size, p.notes, now_ms(), p.id, p.version],
        )
    }

    pub fn delete_product(&self, id: &str) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE products
            SET deleted_at = ?1, version = version + 1, updated_at = ?1
            WHERE id = ?2 AND deleted_at IS NULL",
            params![now_ms(), id],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_list() {
        let db = Database::open(":memory:").unwrap();
        db.create_product("Men's T-Shirts", Some("L"), None)
            .unwrap();

        let products = db.list_products().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].name, "Men's T-Shirts");
    }

    #[test]
    fn get_product_works() {
        let db = Database::open(":memory:").unwrap();
        let id = db.create_product("Jeans", Some("M"), None).unwrap();

        let found = db.get_product(&id).unwrap();
        assert_eq!(found.unwrap().name, "Jeans");

        let missing = db.get_product("does-not-exist").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn stale_version_is_rejected() {
        let db = Database::open(":memory:").unwrap();
        let id = db.create_product("Jeans", Some("M"), None).unwrap();

        // two people load the same product (both see version 1)
        let mut a = db.get_product(&id).unwrap().unwrap();
        let mut b = db.get_product(&id).unwrap().unwrap();

        a.name = "Jeans (blue)".into();
        assert_eq!(db.update_product(&a).unwrap(), 1); // saved, now version 2

        b.name = "Jeans (black)".into();
        assert_eq!(db.update_product(&b).unwrap(), 0); // still thinks version 1, so rejected
    }

    #[test]
    fn delete_hides_product_but_keeps_row() {
        let db = Database::open(":memory:").unwrap();
        let id = db.create_product("Jeans", Some("M"), None).unwrap();

        assert_eq!(db.delete_product(&id).unwrap(), 1);

        // gone from the list...
        assert!(db.list_products().unwrap().is_empty());

        // ...but the row is still there, marked as deleted
        let p = db.get_product(&id).unwrap().unwrap();
        assert!(p.deleted_at.is_some());
        assert_eq!(p.version, 2);

        // deleting again does nothing
        assert_eq!(db.delete_product(&id).unwrap(), 0);
    }
}
