//! One-time studio.db idea import: reads the old planner's `ideas` table
//! (column-for-column identical to katto's — hyper-frames `tools/studio`),
//! maps the status domain, normalizes `first_seen`, and upserts by `id` in a
//! single transaction. Idempotent: re-running reports everything unchanged.
//! `evidence_json` (which may embed the categorical `lean`) copies verbatim —
//! never interpreted, never scored (D7).

use rusqlite::Connection;

use crate::db::ideas::{Idea, UpsertOutcome, upsert_imported};
use crate::error::Result;

/// The dry-run/apply report the wizard renders.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, specta::Type)]
pub struct ImportReport {
    pub imported: u32,
    pub updated: u32,
    pub skipped: u32,
    pub warnings: Vec<String>,
}

/// Map a studio status onto katto's domain; `None` = unknown, skip the row.
pub fn map_status(studio: &str) -> Option<&'static str> {
    match studio {
        "new" | "keep" => Some("backlog"),
        "rejected" => Some("discarded"),
        "promoted" => Some("promoted"),
        _ => None,
    }
}

/// Normalize both observed `first_seen` shapes to `YYYY-MM-DDTHH:MM:SSZ`:
/// `2026-07-09 12:34:56` (skill `datetime('now')`) and
/// `2026-07-09T00:00:00.000Z` (server `toISOString()`). Anything else passes
/// through untouched (the caller records a warning).
pub fn normalize_first_seen(raw: &str) -> String {
    let bytes = raw.as_bytes();
    // space form: 19 chars, space at index 10
    if bytes.len() == 19 && bytes.get(10) == Some(&b' ') {
        let mut out = raw.to_string();
        out.replace_range(10..11, "T");
        out.push('Z');
        return out;
    }
    if bytes.get(10) == Some(&b'T') {
        // T-form: truncate subseconds, ensure a trailing Z
        let seconds = &raw[..raw.len().min(19)];
        if seconds.len() == 19 {
            return format!("{seconds}Z");
        }
    }
    raw.to_string()
}

/// Read and map every studio row. Rows with an unknown status are skipped
/// with a warning naming the id; a pre-`ensureColumns` database (missing
/// `kind_source`/`kind_why`) is noted once and reads those as NULL.
///
/// # Errors
/// Any SQLite error reading the source (schema-alien DBs surface here).
pub fn read_source(conn: &Connection) -> Result<(Vec<Idea>, Vec<String>)> {
    let mut columns = std::collections::HashSet::new();
    {
        let mut stmt = conn.prepare("PRAGMA table_info(ideas)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get("name")?;
            columns.insert(name);
        }
    }
    let mut warnings = Vec::new();
    let has_kind_columns = columns.contains("kind_source") && columns.contains("kind_why");
    if !has_kind_columns {
        warnings.push(
            "source predates the kind_source/kind_why columns — imported as empty".to_string(),
        );
    }
    let kind_cols = if has_kind_columns {
        "kind_source, kind_why"
    } else {
        "NULL AS kind_source, NULL AS kind_why"
    };
    let sql = format!(
        "SELECT id, type, kind, status, title, rationale, source, source_url, source_title,
                evidence_json, raw_signal_id, first_seen, notes, promoted_slug, {kind_cols}
         FROM ideas"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut ideas = Vec::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get("id")?;
        let studio_status: String = row.get("status")?;
        let Some(status) = map_status(&studio_status) else {
            warnings.push(format!("skipped {id}: unknown status \"{studio_status}\""));
            continue;
        };
        let raw_first_seen: String = row.get("first_seen")?;
        let first_seen = normalize_first_seen(&raw_first_seen);
        if first_seen == raw_first_seen && !raw_first_seen.ends_with('Z') {
            warnings.push(format!(
                "{id}: first_seen \"{raw_first_seen}\" kept verbatim (unrecognized shape)"
            ));
        }
        ideas.push(Idea {
            id,
            r#type: row.get("type")?,
            kind: row.get("kind")?,
            status: status.to_string(),
            title: row.get("title")?,
            rationale: row.get("rationale")?,
            source: row.get("source")?,
            source_url: row.get("source_url")?,
            source_title: row.get("source_title")?,
            evidence_json: row.get("evidence_json")?,
            raw_signal_id: row.get("raw_signal_id")?,
            first_seen,
            notes: row.get("notes")?,
            promoted_slug: row.get("promoted_slug")?,
            kind_source: row.get("kind_source")?,
            kind_why: row.get("kind_why")?,
        });
    }
    Ok((ideas, warnings))
}

/// Apply mapped rows in one transaction — commits whole or rolls back whole;
/// partial application is impossible.
///
/// # Errors
/// Any upsert failure aborts the transaction and surfaces here.
pub fn apply(conn: &mut Connection, ideas: &[Idea]) -> Result<ImportReport> {
    let tx = conn.transaction().map_err(crate::error::Error::from)?;
    let mut report = ImportReport::default();
    for idea in ideas {
        match upsert_imported(&tx, idea)? {
            UpsertOutcome::Inserted => report.imported += 1,
            UpsertOutcome::Updated => report.updated += 1,
            UpsertOutcome::Unchanged => report.skipped += 1,
        }
    }
    tx.commit().map_err(crate::error::Error::from)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUDIO_DDL_POST: &str = r#"
CREATE TABLE ideas (
  id TEXT PRIMARY KEY, type TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'unset',
  status TEXT NOT NULL DEFAULT 'new', title TEXT NOT NULL, rationale TEXT,
  source TEXT, source_url TEXT, source_title TEXT, evidence_json TEXT,
  raw_signal_id TEXT, first_seen TEXT NOT NULL, notes TEXT, promoted_slug TEXT,
  kind_source TEXT, kind_why TEXT
);"#;

    // pre-ensureColumns shape: no kind_source / kind_why
    const STUDIO_DDL_PRE: &str = r#"
CREATE TABLE ideas (
  id TEXT PRIMARY KEY, type TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'unset',
  status TEXT NOT NULL DEFAULT 'new', title TEXT NOT NULL, rationale TEXT,
  source TEXT, source_url TEXT, source_title TEXT, evidence_json TEXT,
  raw_signal_id TEXT, first_seen TEXT NOT NULL, notes TEXT, promoted_slug TEXT
);"#;

    fn source_db(ddl: &str) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(ddl).unwrap();
        conn
    }

    #[test]
    fn map_status_covers_all_studio_values() {
        assert_eq!(map_status("new"), Some("backlog"));
        assert_eq!(map_status("keep"), Some("backlog"));
        assert_eq!(map_status("rejected"), Some("discarded"));
        assert_eq!(map_status("promoted"), Some("promoted"));
        assert_eq!(map_status("weird"), None);
    }

    #[test]
    fn normalize_first_seen_handles_both_shapes() {
        assert_eq!(
            normalize_first_seen("2026-07-09 12:34:56"),
            "2026-07-09T12:34:56Z"
        );
        assert_eq!(
            normalize_first_seen("2026-07-09T00:00:00.000Z"),
            "2026-07-09T00:00:00Z"
        );
        assert_eq!(
            normalize_first_seen("2026-07-09T00:00:00Z"),
            "2026-07-09T00:00:00Z"
        );
    }

    #[test]
    fn read_source_maps_rows_and_preserves_promoted_slug_verbatim() {
        let src = source_db(STUDIO_DDL_POST);
        src.execute_batch(r#"
INSERT INTO ideas (id, type, kind, status, title, first_seen, promoted_slug, kind_source, kind_why, evidence_json)
VALUES ('a1', 'mirror', 'long', 'promoted', 'T1', '2026-07-01 10:00:00', 'video-slug-2026-07-01', 'ai', 'why', '{"lean":"strong"}'),
       ('b2', 'manual', 'unset', 'keep', 'T2', '2026-07-02T09:00:00.000Z', NULL, NULL, NULL, NULL),
       ('c3', 'trend', 'short', 'weird', 'T3', '2026-07-03 08:00:00', NULL, NULL, NULL, NULL);
"#).unwrap();
        let (ideas, warnings) = read_source(&src).unwrap();
        assert_eq!(ideas.len(), 2, "unknown status row skipped");
        let a = ideas.iter().find(|i| i.id == "a1").unwrap();
        assert_eq!(a.status, "promoted");
        assert_eq!(a.promoted_slug.as_deref(), Some("video-slug-2026-07-01"));
        assert_eq!(a.first_seen, "2026-07-01T10:00:00Z");
        assert_eq!(a.evidence_json.as_deref(), Some(r#"{"lean":"strong"}"#));
        let b = ideas.iter().find(|i| i.id == "b2").unwrap();
        assert_eq!(b.status, "backlog");
        assert!(
            warnings.iter().any(|w| w.contains("c3")),
            "skip warning names the id"
        );
    }

    #[test]
    fn read_source_tolerates_pre_ensure_columns_db() {
        let src = source_db(STUDIO_DDL_PRE);
        src.execute_batch(
            "INSERT INTO ideas (id, type, status, title, first_seen) VALUES ('a1','mirror','new','T','2026-07-01 10:00:00');"
        ).unwrap();
        let (ideas, warnings) = read_source(&src).unwrap();
        assert_eq!(ideas[0].kind_source, None);
        assert_eq!(ideas[0].kind_why, None);
        assert!(
            warnings.iter().any(|w| w.contains("kind_source")),
            "column absence noted"
        );
    }

    #[test]
    fn apply_is_transactional_and_idempotent() {
        let mut conn = crate::db::test_db();
        let idea = Idea {
            id: "x1".to_string(),
            r#type: "manual".to_string(),
            kind: "unset".to_string(),
            status: "backlog".to_string(),
            title: "T".to_string(),
            rationale: None,
            source: None,
            source_url: None,
            source_title: None,
            evidence_json: None,
            raw_signal_id: None,
            first_seen: "2026-07-01T10:00:00Z".to_string(),
            notes: None,
            promoted_slug: None,
            kind_source: None,
            kind_why: None,
        };
        let report = apply(&mut conn, std::slice::from_ref(&idea)).unwrap();
        assert_eq!((report.imported, report.updated, report.skipped), (1, 0, 0));
        let again = apply(&mut conn, &[idea]).unwrap();
        assert_eq!((again.imported, again.updated, again.skipped), (0, 0, 1));
    }
}
