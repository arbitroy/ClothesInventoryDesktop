use super::{DbResult, schema};
use rusqlite::Connection;
use tauri::scope;

// Script number N is MIGRATIONS[N - 1]. Add new ones to the END, never edit old ones.
const MIGRATIONS: &[&str] = &[
    schema::V1, // the schema we have today
    schema::V2,
    schema::V3,
];

pub fn run(conn: &mut Connection) -> DbResult<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = i as i64 + 1;
        if version <= current {
            continue; // already applied
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?; // the script and the new number are saved together
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_twice_changes_nothing() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        run(&mut conn).unwrap(); // the second run must do nothing, and not fail

        let v: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn v2_backfills_old_rows_and_logs_new_changes() {
        let mut conn = Connection::open_in_memory().unwrap();

        // pretend this is an old database: only V1, with a product already in it
        conn.execute_batch(schema::V1).unwrap();
        conn.execute(
            "INSERT INTO products (id, name, created_at, updated_at) VALUES ('p1', 'Jeans', 0, 0)",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();

        run(&mut conn).unwrap(); // applies V2 only

        let logged = |conn: &Connection| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM change_log WHERE entity_id = 'p1'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };

        // the old product was backfilled, and still exists
        assert_eq!(logged(&conn), 1);

        // from now on, changes are logged automatically
        conn.execute("UPDATE products SET name = 'Denim' WHERE id = 'p1'", [])
            .unwrap();
        assert_eq!(logged(&conn), 2);
    }
}
