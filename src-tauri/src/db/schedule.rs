use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// The two pinnable dates. Enum so an out-of-vocabulary kind is rejected at the
/// IPC boundary and can never reach the schedule or manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleKind {
    Shoot,
    Publish,
}

impl ScheduleKind {
    /// The stored column value.
    pub fn as_str(self) -> &'static str {
        match self {
            ScheduleKind::Shoot => "shoot",
            ScheduleKind::Publish => "publish",
        }
    }
}

/// A planner schedule entry pinning a project to a date. `kind` is `shoot` or
/// `publish`. There is at most one entry per `(project_slug, kind)` pair — see
/// [`upsert`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ScheduleEntry {
    // Exported as `number` (an AUTOINCREMENT rowid that never approaches 2^53),
    // matching `RowId`; specta forbids a bare `i64` crossing to TypeScript.
    #[specta(type = i32)]
    pub id: i64,
    pub project_slug: String,
    pub kind: String,
    pub date: String,
    pub note: Option<String>,
}

const SELECT_COLUMNS: &str = "id, project_slug, kind, date, note";

fn from_row(row: &Row) -> rusqlite::Result<ScheduleEntry> {
    Ok(ScheduleEntry {
        id: row.get("id")?,
        project_slug: row.get("project_slug")?,
        kind: row.get("kind")?,
        date: row.get("date")?,
        note: row.get("note")?,
    })
}

/// Insert or update the single entry for `(project_slug, kind)`, backed by the
/// `idx_schedule_project_kind` unique index (a real `ON CONFLICT` upsert). Reads
/// the row back for its id.
pub fn upsert(
    conn: &Connection,
    project_slug: &str,
    kind: &str,
    date: &str,
    note: Option<&str>,
) -> Result<ScheduleEntry> {
    conn.execute(
        "INSERT INTO schedule (project_slug, kind, date, note) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_slug, kind) DO UPDATE SET date = excluded.date, note = excluded.note",
        params![project_slug, kind, date, note],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM schedule WHERE project_slug = ?1 AND kind = ?2",
        params![project_slug, kind],
        |row| row.get(0),
    )?;
    Ok(ScheduleEntry {
        id,
        project_slug: project_slug.to_string(),
        kind: kind.to_string(),
        date: date.to_string(),
        note: note.map(str::to_string),
    })
}

/// All entries whose `date` falls within `[from, to]` (inclusive bounds),
/// ordered by date ascending. Dates are ISO `YYYY-MM-DD` strings that sort
/// lexicographically.
pub fn list_range(conn: &Connection, from: &str, to: &str) -> Result<Vec<ScheduleEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM schedule
         WHERE date >= ?1 AND date <= ?2
         ORDER BY date ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![from, to], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Every entry for one project, ordered by date ascending. Callers capture this
/// into the audit log before deleting a project row — `schedule.project_slug` is
/// ON DELETE CASCADE, so the rows are gone the instant the project is.
pub fn list_for_project(conn: &Connection, project_slug: &str) -> Result<Vec<ScheduleEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM schedule
         WHERE project_slug = ?1
         ORDER BY date ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![project_slug], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Delete a schedule entry by id.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM schedule WHERE id = ?1", [id])?;
    Ok(())
}

/// Remove the single pin for `(project_slug, kind)`, if any.
pub fn delete_for(conn: &Connection, project_slug: &str, kind: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM schedule WHERE project_slug = ?1 AND kind = ?2",
        params![project_slug, kind],
    )?;
    Ok(())
}

/// The earliest upcoming shoot (`kind='shoot'`, `date >= today`) with its
/// project title, for the tray's next-shoot line. `None` when none is scheduled.
pub fn next_shoot(conn: &Connection, today: &str) -> Result<Option<(ScheduleEntry, String)>> {
    let sql = format!(
        "SELECT {}, p.title FROM schedule s
         JOIN projects p ON p.slug = s.project_slug
         WHERE s.kind = 'shoot' AND s.date >= ?1
         ORDER BY s.date ASC, s.id ASC
         LIMIT 1",
        "s.id, s.project_slug, s.kind, s.date, s.note"
    );
    Ok(conn
        .query_row(&sql, [today], |row| {
            Ok((from_row(row)?, row.get::<_, String>("title")?))
        })
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects::{self, Project};
    use crate::db::test_db;

    fn seed_project(conn: &Connection, slug: &str) {
        projects::insert(
            conn,
            &Project {
                slug: slug.to_string(),
                title: "NVMe Deep Dive".to_string(),
                root_path: format!("/Volumes/Studio/Projects/{slug}"),
                status: "idea".to_string(),
                target_nle: "fcp".to_string(),
                priority: "none".to_string(),
                shoot_date: None,
                publish_date: None,
                created_at: "2026-07-09T00:00:00Z".to_string(),
                last_touched_at: None,
                kind: "unset".to_string(),
            },
        )
        .unwrap();
    }

    #[test]
    fn delete_for_clears_only_that_kind() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        upsert(&conn, "p-2026-07-09", "publish", "2026-08-20", None).unwrap();
        delete_for(&conn, "p-2026-07-09", "shoot").unwrap();
        let rows = list_range(&conn, "2026-08-01", "2026-08-31").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "publish");
    }

    #[test]
    fn upsert_inserts_then_updates_same_pair() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        let first = upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        let second = upsert(&conn, "p-2026-07-09", "shoot", "2026-08-05", Some("moved")).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.date, "2026-08-05");
        assert_eq!(second.note.as_deref(), Some("moved"));
        let all = list_range(&conn, "2026-01-01", "2026-12-31").unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn upsert_distinguishes_kind() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        let shoot = upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        let publish = upsert(&conn, "p-2026-07-09", "publish", "2026-08-20", None).unwrap();
        assert_ne!(shoot.id, publish.id);
        assert_eq!(
            list_range(&conn, "2026-01-01", "2026-12-31").unwrap().len(),
            2
        );
    }

    #[test]
    fn list_range_bounds_are_inclusive() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        upsert(&conn, "p-2026-07-09", "publish", "2026-08-31", None).unwrap();
        let got: Vec<String> = list_range(&conn, "2026-08-01", "2026-08-31")
            .unwrap()
            .into_iter()
            .map(|e| e.date)
            .collect();
        assert_eq!(got, vec!["2026-08-01", "2026-08-31"]);
        assert!(
            list_range(&conn, "2026-08-02", "2026-08-30")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn list_for_project_returns_only_that_projects_entries_with_notes() {
        let conn = test_db();
        seed_project(&conn, "a-2026-07-09");
        seed_project(&conn, "b-2026-07-09");
        upsert(
            &conn,
            "a-2026-07-09",
            "publish",
            "2026-08-20",
            Some("goes live"),
        )
        .unwrap();
        upsert(
            &conn,
            "a-2026-07-09",
            "shoot",
            "2026-08-01",
            Some("studio B"),
        )
        .unwrap();
        upsert(&conn, "b-2026-07-09", "shoot", "2026-08-02", None).unwrap();

        let got = list_for_project(&conn, "a-2026-07-09").unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].date, "2026-08-01");
        assert_eq!(got[0].note.as_deref(), Some("studio B"));
        assert_eq!(got[1].note.as_deref(), Some("goes live"));
        assert!(got.iter().all(|e| e.project_slug == "a-2026-07-09"));
    }

    #[test]
    fn list_for_project_is_empty_for_an_unscheduled_project() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        assert!(list_for_project(&conn, "p-2026-07-09").unwrap().is_empty());
    }

    #[test]
    fn delete_removes_the_entry() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        let e = upsert(&conn, "p-2026-07-09", "shoot", "2026-08-01", None).unwrap();
        delete(&conn, e.id).unwrap();
        assert!(
            list_range(&conn, "2026-01-01", "2026-12-31")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn next_shoot_picks_earliest_from_today_with_title() {
        let conn = test_db();
        seed_project(&conn, "a-2026-07-09");
        seed_project(&conn, "b-2026-07-09");
        // A past shoot must be ignored.
        upsert(&conn, "a-2026-07-09", "shoot", "2026-07-01", None).unwrap();
        upsert(&conn, "b-2026-07-09", "shoot", "2026-07-20", None).unwrap();
        upsert(&conn, "a-2026-07-09", "shoot", "2026-07-15", None).unwrap();
        let (entry, title) = next_shoot(&conn, "2026-07-09").unwrap().unwrap();
        assert_eq!(entry.date, "2026-07-15");
        assert_eq!(entry.project_slug, "a-2026-07-09");
        assert_eq!(title, "NVMe Deep Dive");
    }

    #[test]
    fn next_shoot_ignores_publish_kind() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        upsert(&conn, "p-2026-07-09", "publish", "2026-07-15", None).unwrap();
        assert_eq!(next_shoot(&conn, "2026-07-09").unwrap(), None);
    }

    #[test]
    fn next_shoot_none_when_all_past() {
        let conn = test_db();
        seed_project(&conn, "p-2026-07-09");
        upsert(&conn, "p-2026-07-09", "shoot", "2026-07-01", None).unwrap();
        assert_eq!(next_shoot(&conn, "2026-07-09").unwrap(), None);
    }
}
