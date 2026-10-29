mod products;
mod receipts;
mod schema;

pub use products::Product;
pub use receipts::{ContainerItem, Receipt};

use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

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
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Self { conn })
    }
}