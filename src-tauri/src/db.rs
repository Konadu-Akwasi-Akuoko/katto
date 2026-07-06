use std::path::Path;

use rusqlite::Connection;
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub mod events;
mod handle;
pub mod jobs;
pub mod migrations;
pub mod settings;

pub use handle::DbHandle;

/// A SQLite `INTEGER PRIMARY KEY` / rowid. Serializes transparently as its inner
/// integer (the wire value); exported to TypeScript as `number` because these
/// ids are AUTOINCREMENT rowids that never approach 2^53, and Tauri's JSON IPC
/// transmits `i64` as a JS number anyway.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(transparent)]
pub struct RowId(#[specta(type = i32)] pub i64);

impl FromSql for RowId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(RowId)
    }
}

impl ToSql for RowId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

/// Open the app database at `path`, apply production pragmas, and run every
/// pending migration. The returned connection is fully migrated and ready.
pub fn open(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    apply_pragmas(&conn, true)?;
    migrations::MIGRATIONS.to_latest(&mut conn)?;
    Ok(conn)
}

/// Production pragmas. `wal` is skipped for in-memory test databases, where a
/// rollback journal is required (WAL is a no-op there anyway).
fn apply_pragmas(conn: &Connection, wal: bool) -> Result<()> {
    if wal {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// Fresh in-memory database with the full migration ladder applied and the same
/// pragmas as production (minus WAL). Shared by every `db/` repository test.
#[cfg(test)]
pub fn test_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    apply_pragmas(&conn, false).unwrap();
    migrations::MIGRATIONS.to_latest(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real file-backed `open()` path (the only place the WAL
    /// branch runs): a fresh file comes back migrated with WAL enabled.
    #[test]
    fn open_creates_migrated_wal_db() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("katto.db")).unwrap();

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");

        let settings_rows: i64 = conn
            .query_row("SELECT count(*) FROM settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(settings_rows, 0);
    }
}
