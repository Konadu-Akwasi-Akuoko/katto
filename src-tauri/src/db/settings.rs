use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Result;

/// Read a settings value by key, or `None` if unset.
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

/// Upsert a settings value.
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn unset_key_is_none() {
        let conn = test_db();
        assert_eq!(get(&conn, "studio_root").unwrap(), None);
    }

    #[test]
    fn set_then_get_round_trips() {
        let conn = test_db();
        set(&conn, "studio_root", "/Volumes/Studio").unwrap();
        assert_eq!(
            get(&conn, "studio_root").unwrap().as_deref(),
            Some("/Volumes/Studio")
        );
    }

    #[test]
    fn set_overwrites_existing_value() {
        let conn = test_db();
        set(&conn, "idle_reap_minutes", "30").unwrap();
        set(&conn, "idle_reap_minutes", "15").unwrap();
        assert_eq!(
            get(&conn, "idle_reap_minutes").unwrap().as_deref(),
            Some("15")
        );
    }
}
