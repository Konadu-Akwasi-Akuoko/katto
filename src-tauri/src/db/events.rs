use rusqlite::{Connection, Row, params};
use serde::Serialize;

use crate::error::Result;

/// A row in the append-only activity log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub id: i64,
    pub ts: String,
    pub kind: String,
    pub project_slug: Option<String>,
    pub payload_json: Option<String>,
}

fn from_row(row: &Row) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get("id")?,
        ts: row.get("ts")?,
        kind: row.get("kind")?,
        project_slug: row.get("project_slug")?,
        payload_json: row.get("payload_json")?,
    })
}

/// Append an event. The timestamp is stamped by SQLite (UTC, RFC3339) so callers
/// need no clock. Returns the new row id. This is the only write path — `events`
/// is append-only (no update/delete).
pub fn record(
    conn: &Connection,
    kind: &str,
    project_slug: Option<&str>,
    payload_json: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO events (ts, kind, project_slug, payload_json)
         VALUES (strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?1, ?2, ?3)",
        params![kind, project_slug, payload_json],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Most-recent-first page of events. `before_id`, when set, returns only events
/// older than that id (keyset pagination over the monotonic primary key).
pub fn list(conn: &Connection, limit: u32, before_id: Option<i64>) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT id, ts, kind, project_slug, payload_json
         FROM events
         WHERE (?1 IS NULL OR id < ?1)
         ORDER BY id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![before_id, limit], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn record_returns_increasing_ids() {
        let conn = test_db();
        let a = record(&conn, "app_started", None, None).unwrap();
        let b = record(&conn, "onboarding_complete", None, None).unwrap();
        assert!(b > a);
    }

    #[test]
    fn list_is_newest_first_and_limited() {
        let conn = test_db();
        for i in 0..5 {
            record(&conn, "tick", None, Some(&format!("{{\"n\":{i}}}"))).unwrap();
        }
        let page = list(&conn, 3, None).unwrap();
        assert_eq!(page.len(), 3);
        assert!(page[0].id > page[1].id && page[1].id > page[2].id);
    }

    #[test]
    fn before_id_pages_backward() {
        let conn = test_db();
        let ids: Vec<i64> = (0..4)
            .map(|_| record(&conn, "tick", None, None).unwrap())
            .collect();
        let page = list(&conn, 10, Some(ids[2])).unwrap();
        let got: Vec<i64> = page.iter().map(|e| e.id).collect();
        assert_eq!(got, vec![ids[1], ids[0]]);
    }

    #[test]
    fn stores_project_and_payload() {
        let conn = test_db();
        record(
            &conn,
            "ingest_done",
            Some("my-video"),
            Some("{\"count\":42}"),
        )
        .unwrap();
        let e = &list(&conn, 1, None).unwrap()[0];
        assert_eq!(e.project_slug.as_deref(), Some("my-video"));
        assert_eq!(e.payload_json.as_deref(), Some("{\"count\":42}"));
    }
}
