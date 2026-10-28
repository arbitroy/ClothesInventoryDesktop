use rusqlite::{Connection, params};
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
}
