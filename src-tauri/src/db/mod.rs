mod error;
mod products;
mod receipts;
mod schema;
mod transactions;
mod flag;
mod dates;  
mod daily;

pub use error::{DbError, DbResult};
pub use products::{Product, ProductWithStock};
pub use receipts::{ContainerItem, Receipt, ReceiptTotals};
pub use flag::PendingFlag;

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
    pub fn open(path: impl AsRef<std::path::Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Self { conn })
    }
}
