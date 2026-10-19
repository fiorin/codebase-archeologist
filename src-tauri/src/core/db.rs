use crate::core::error::Result;
use rusqlite::Connection;
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 1;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("migrations/001_init.sql"))?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}
