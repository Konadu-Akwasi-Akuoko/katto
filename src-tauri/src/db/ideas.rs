use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;

use crate::error::Result;

/// An idea row. Column parity with hyper-frames `tools/studio` (D7: no numeric
/// score or ranking fields). `status` moves `backlog -> promoted | discarded`;
/// discarded rows are kept as an audit trail rather than deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Idea {
    pub id: String,
    pub r#type: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub rationale: Option<String>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub source_title: Option<String>,
    pub evidence_json: Option<String>,
    pub raw_signal_id: Option<String>,
    pub first_seen: String,
    pub notes: Option<String>,
    pub promoted_slug: Option<String>,
    pub kind_source: Option<String>,
    pub kind_why: Option<String>,
}

const SELECT_COLUMNS: &str = "id, type, kind, status, title, rationale, source, source_url, source_title, evidence_json, raw_signal_id, first_seen, notes, promoted_slug, kind_source, kind_why";

fn from_row(row: &Row) -> rusqlite::Result<Idea> {
    Ok(Idea {
        id: row.get("id")?,
        r#type: row.get("type")?,
        kind: row.get("kind")?,
        status: row.get("status")?,
        title: row.get("title")?,
        rationale: row.get("rationale")?,
        source: row.get("source")?,
        source_url: row.get("source_url")?,
        source_title: row.get("source_title")?,
        evidence_json: row.get("evidence_json")?,
        raw_signal_id: row.get("raw_signal_id")?,
        first_seen: row.get("first_seen")?,
        notes: row.get("notes")?,
        promoted_slug: row.get("promoted_slug")?,
        kind_source: row.get("kind_source")?,
        kind_why: row.get("kind_why")?,
    })
}

/// Insert a new idea row.
pub fn create(conn: &Connection, idea: &Idea) -> Result<()> {
    conn.execute(
        "INSERT INTO ideas
           (id, type, kind, status, title, rationale, source, source_url, source_title,
            evidence_json, raw_signal_id, first_seen, notes, promoted_slug, kind_source, kind_why)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            idea.id,
            idea.r#type,
            idea.kind,
            idea.status,
            idea.title,
            idea.rationale,
            idea.source,
            idea.source_url,
            idea.source_title,
            idea.evidence_json,
            idea.raw_signal_id,
            idea.first_seen,
            idea.notes,
            idea.promoted_slug,
            idea.kind_source,
            idea.kind_why,
        ],
    )?;
    Ok(())
}

/// Ideas with the given status, newest-first by `first_seen` (index-backed by
/// `idx_ideas_status`).
pub fn list_by_status(conn: &Connection, status: &str) -> Result<Vec<Idea>> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM ideas WHERE status = ?1 ORDER BY first_seen DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([status], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Fetch an idea by id, or `None` if absent.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Idea>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM ideas WHERE id = ?1");
    Ok(conn.query_row(&sql, [id], from_row).optional()?)
}

/// Overwrite an idea's editable columns with the modal's saved state. Unlike a
/// partial patch, every editable column is set to the provided value; `None`
/// clears a nullable column. `title`/`kind` are NOT NULL and always carry a
/// value. `evidence_json` is passed pre-merged by the command layer.
#[allow(clippy::too_many_arguments)]
pub fn update(
    conn: &Connection,
    id: &str,
    title: &str,
    kind: &str,
    notes: Option<&str>,
    rationale: Option<&str>,
    source_url: Option<&str>,
    evidence_json: Option<&str>,
    kind_source: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE ideas SET
           title = ?2, kind = ?3, notes = ?4, rationale = ?5,
           source_url = ?6, evidence_json = ?7, kind_source = ?8
         WHERE id = ?1",
        params![
            id,
            title,
            kind,
            notes,
            rationale,
            source_url,
            evidence_json,
            kind_source
        ],
    )?;
    Ok(())
}

/// Ideas first seen at/after `iso_utc` (curation-run delta counting).
pub fn count_since(conn: &Connection, iso_utc: &str) -> Result<u32> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM ideas WHERE first_seen >= ?1",
        [iso_utc],
        |r| r.get::<_, u32>(0),
    )?;
    Ok(n)
}

/// Set an idea's status (e.g. `discarded`); the row is retained as an audit trail.
pub fn set_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE ideas SET status = ?2 WHERE id = ?1",
        params![id, status],
    )?;
    Ok(())
}

/// What an import upsert did to the row.
#[derive(Debug, PartialEq)]
pub enum UpsertOutcome {
    Inserted,
    Updated,
    Unchanged,
}

/// Idempotent import upsert by `id`: insert when absent, update every
/// non-id column when different, report `Unchanged` when identical.
pub fn upsert_imported(conn: &Connection, idea: &Idea) -> Result<UpsertOutcome> {
    match get(conn, &idea.id)? {
        None => {
            create(conn, idea)?;
            Ok(UpsertOutcome::Inserted)
        }
        Some(existing) if existing == *idea => Ok(UpsertOutcome::Unchanged),
        Some(_) => {
            conn.execute(
                "UPDATE ideas SET type = ?2, kind = ?3, status = ?4, title = ?5,
                   rationale = ?6, source = ?7, source_url = ?8, source_title = ?9,
                   evidence_json = ?10, raw_signal_id = ?11, first_seen = ?12,
                   notes = ?13, promoted_slug = ?14, kind_source = ?15, kind_why = ?16
                 WHERE id = ?1",
                params![
                    idea.id,
                    idea.r#type,
                    idea.kind,
                    idea.status,
                    idea.title,
                    idea.rationale,
                    idea.source,
                    idea.source_url,
                    idea.source_title,
                    idea.evidence_json,
                    idea.raw_signal_id,
                    idea.first_seen,
                    idea.notes,
                    idea.promoted_slug,
                    idea.kind_source,
                    idea.kind_why,
                ],
            )?;
            Ok(UpsertOutcome::Updated)
        }
    }
}

/// Mark an idea promoted: `status='promoted'` and record the resulting project slug.
pub fn mark_promoted(conn: &Connection, id: &str, slug: &str) -> Result<()> {
    conn.execute(
        "UPDATE ideas SET status = 'promoted', promoted_slug = ?2 WHERE id = ?1",
        params![id, slug],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn sample(id: &str, first_seen: &str) -> Idea {
        Idea {
            id: id.to_string(),
            r#type: "manual".to_string(),
            kind: "unset".to_string(),
            status: "backlog".to_string(),
            title: "NVMe Deep Dive".to_string(),
            rationale: None,
            source: None,
            source_url: None,
            source_title: None,
            evidence_json: None,
            raw_signal_id: None,
            first_seen: first_seen.to_string(),
            notes: None,
            promoted_slug: None,
            kind_source: None,
            kind_why: None,
        }
    }

    #[test]
    fn count_since_counts_first_seen_after_cutoff() {
        let conn = test_db();
        create(&conn, &sample("before", "2026-07-22 07:59:59")).unwrap();
        create(&conn, &sample("after", "2026-07-22 08:00:01")).unwrap();
        assert_eq!(count_since(&conn, "2026-07-22 08:00:00").unwrap(), 1);
    }

    #[test]
    fn create_then_get_round_trips() {
        let conn = test_db();
        let idea = sample("i1", "2026-07-09T00:00:00Z");
        create(&conn, &idea).unwrap();
        assert_eq!(get(&conn, "i1").unwrap(), Some(idea));
    }

    #[test]
    fn get_absent_is_none() {
        let conn = test_db();
        assert_eq!(get(&conn, "nope").unwrap(), None);
    }

    #[test]
    fn list_by_status_is_first_seen_desc() {
        let conn = test_db();
        create(&conn, &sample("a", "2026-07-01T00:00:00Z")).unwrap();
        create(&conn, &sample("b", "2026-07-03T00:00:00Z")).unwrap();
        create(&conn, &sample("c", "2026-07-02T00:00:00Z")).unwrap();
        let ids: Vec<String> = list_by_status(&conn, "backlog")
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn list_by_status_filters_status() {
        let conn = test_db();
        create(&conn, &sample("a", "2026-07-01T00:00:00Z")).unwrap();
        create(&conn, &sample("b", "2026-07-02T00:00:00Z")).unwrap();
        set_status(&conn, "b", "discarded").unwrap();
        let backlog: Vec<String> = list_by_status(&conn, "backlog")
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(backlog, vec!["a"]);
    }

    #[test]
    fn update_sets_and_clears_editable_columns() {
        let conn = test_db();
        let mut idea = sample("u1", "2026-07-09T10:00:00Z");
        idea.r#type = "curated".to_string();
        idea.rationale = Some("old why".to_string());
        idea.source = Some("hn".to_string());
        idea.source_url = Some("https://example.com".to_string());
        idea.evidence_json = Some(r#"{"lean":"hold","quotes":["q"]}"#.to_string());
        idea.notes = Some("old note".to_string());
        idea.kind_source = Some("ai".to_string());
        idea.kind_why = Some("why".to_string());
        create(&conn, &idea).unwrap();

        // set title/kind, clear notes + source_url, keep merged evidence_json
        update(
            &conn,
            "u1",
            "New",
            "long",
            None,
            Some("new why"),
            None,
            Some(r#"{"lean":"strong","quotes":["q"]}"#),
            Some("human"),
        )
        .unwrap();

        let got = get(&conn, "u1").unwrap().unwrap();
        assert_eq!(got.title, "New");
        assert_eq!(got.kind, "long");
        assert_eq!(got.notes, None);
        assert_eq!(got.rationale.as_deref(), Some("new why"));
        assert_eq!(got.source_url, None);
        assert_eq!(got.kind_source.as_deref(), Some("human"));
        assert_eq!(
            got.evidence_json.as_deref(),
            Some(r#"{"lean":"strong","quotes":["q"]}"#)
        );
    }

    #[test]
    fn discard_keeps_the_row_as_audit() {
        let conn = test_db();
        create(&conn, &sample("i1", "2026-07-09T00:00:00Z")).unwrap();
        set_status(&conn, "i1", "discarded").unwrap();
        let got = get(&conn, "i1").unwrap().unwrap();
        assert_eq!(got.status, "discarded");
    }

    #[test]
    fn mark_promoted_sets_status_and_slug() {
        let conn = test_db();
        create(&conn, &sample("i1", "2026-07-09T00:00:00Z")).unwrap();
        mark_promoted(&conn, "i1", "nvme-deep-dive-2026-07-09").unwrap();
        let got = get(&conn, "i1").unwrap().unwrap();
        assert_eq!(got.status, "promoted");
        assert_eq!(
            got.promoted_slug.as_deref(),
            Some("nvme-deep-dive-2026-07-09")
        );
    }

    #[test]
    fn upsert_imported_inserts_when_absent() {
        let conn = test_db();
        let idea = sample("s1", "2026-07-09T00:00:00Z");
        assert!(matches!(
            upsert_imported(&conn, &idea).unwrap(),
            UpsertOutcome::Inserted
        ));
        assert!(get(&conn, "s1").unwrap().is_some());
    }

    #[test]
    fn upsert_imported_updates_changed_row() {
        let conn = test_db();
        let mut idea = sample("s1", "2026-07-09T00:00:00Z");
        create(&conn, &idea).unwrap();
        idea.title = "New title".into();
        idea.status = "promoted".into();
        idea.promoted_slug = Some("new-title-2026-07-22".into());
        assert!(matches!(
            upsert_imported(&conn, &idea).unwrap(),
            UpsertOutcome::Updated
        ));
        let got = get(&conn, "s1").unwrap().unwrap();
        assert_eq!(got.title, "New title");
        assert_eq!(got.promoted_slug.as_deref(), Some("new-title-2026-07-22"));
    }

    #[test]
    fn upsert_imported_reports_unchanged_when_identical() {
        let conn = test_db();
        let idea = sample("s1", "2026-07-09T00:00:00Z");
        create(&conn, &idea).unwrap();
        assert!(matches!(
            upsert_imported(&conn, &idea).unwrap(),
            UpsertOutcome::Unchanged
        ));
    }
}
