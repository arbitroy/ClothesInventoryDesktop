use super::{now_ms, Database, DbError, DbResult};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
    pub revoked: bool,
}

fn hash(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Database {
    /// Registers a phone. Returns (device id, token). The token is shown once and
    /// never stored; only its hash is.
    pub fn create_device(&self, name: &str) -> DbResult<(String, String)> {
        let id = uuid::Uuid::now_v7().to_string();
        let secret = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        self.conn.execute(
            "INSERT INTO devices (id, name, token_hash, paired_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, hash(&secret), now_ms()],
        )?;
        let token = format!("{id}.{secret}");
        Ok((id, token))
    }

    /// Checks a token and returns the device id it belongs to.
    pub fn authenticate(&self, token: &str) -> DbResult<String> {
        let (id, secret) = token.split_once('.').ok_or(DbError::Unauthorized)?;
        let stored: Option<(String, Option<i64>)> = self
            .conn
            .query_row(
                "SELECT token_hash, revoked_at FROM devices WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match stored {
            Some((stored_hash, None)) if stored_hash == hash(secret) => {
                self.conn.execute(
                    "UPDATE devices SET last_seen_at = ?1 WHERE id = ?2",
                    params![now_ms(), id],
                )?;
                Ok(id.to_string())
            }
            _ => Err(DbError::Unauthorized), // unknown, wrong secret, or revoked
        }
    }

    pub fn revoke_device(&self, id: &str) -> DbResult<usize> {
        Ok(self.conn.execute(
            "UPDATE devices SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![now_ms(), id],
        )?)
    }

    pub fn list_devices(&self) -> DbResult<Vec<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, paired_at, last_seen_at, revoked_at IS NOT NULL
             FROM devices ORDER BY paired_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Device {
                id: row.get(0)?,
                name: row.get(1)?,
                paired_at: row.get(2)?,
                last_seen_at: row.get(3)?,
                revoked: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_device_works_until_revoked() {
        let db = Database::open(":memory:").unwrap();
        let (id, token) = db.create_device("Amina's phone").unwrap();

        assert_eq!(db.authenticate(&token).unwrap(), id);

        // wrong secret, malformed token, unknown device: all refused
        for bad in [format!("{id}.wrong"), "garbage".to_string(), "nope.abc".to_string()] {
            let r = db.authenticate(&bad);
            assert!(matches!(r, Err(DbError::Unauthorized)), "got: {r:?}");
        }

        // the secret itself is never stored
        let secret = token.split_once('.').unwrap().1;
        let stored: String = db
            .conn
            .query_row("SELECT token_hash FROM devices", [], |r| r.get(0))
            .unwrap();
        assert!(!stored.contains(secret));

        // revoked: the same token stops working
        assert_eq!(db.revoke_device(&id).unwrap(), 1);
        let r = db.authenticate(&token);
        assert!(matches!(r, Err(DbError::Unauthorized)), "got: {r:?}");
        assert!(db.list_devices().unwrap()[0].revoked);
    }
}