use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// The per-project priority axis. Columns encode *status*; cards encode
/// *priority* — the two axes never share a colour, so a card in a same-hued
/// column can never be misread. This is the write boundary: rows are read back
/// as a lenient `String` (a stale value badges rather than dropping the
/// project), but nothing can *persist* a value outside this set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLevel {
    None,
    Low,
    Medium,
    High,
}

impl PriorityLevel {
    /// The stored column value.
    pub fn as_str(self) -> &'static str {
        match self {
            PriorityLevel::None => "none",
            PriorityLevel::Low => "low",
            PriorityLevel::Medium => "medium",
            PriorityLevel::High => "high",
        }
    }
}

/// A project row. The folder on disk is the source of truth; this row is an index
/// reconciled on launch. `last_touched_at` records the most recent interaction and
/// drives the tray's current-project line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Project {
    pub slug: String,
    pub title: String,
    pub root_path: String,
    pub status: String,
    pub target_nle: String,
    /// `none | low | medium | high`. Read leniently: an unrecognised value is
    /// carried verbatim and renders no priority chrome.
    pub priority: String,
    pub shoot_date: Option<String>,
    pub publish_date: Option<String>,
    pub created_at: String,
    pub last_touched_at: Option<String>,
}

const SELECT_COLUMNS: &str = "slug, title, root_path, status, target_nle, priority, shoot_date, publish_date, created_at, last_touched_at";

fn from_row(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        slug: row.get("slug")?,
        title: row.get("title")?,
        root_path: row.get("root_path")?,
        status: row.get("status")?,
        target_nle: row.get("target_nle")?,
        priority: row.get("priority")?,
        shoot_date: row.get("shoot_date")?,
        publish_date: row.get("publish_date")?,
        created_at: row.get("created_at")?,
        last_touched_at: row.get("last_touched_at")?,
    })
}

/// Insert a new project row. Errors if the slug already exists.
pub fn insert(conn: &Connection, p: &Project) -> Result<()> {
    conn.execute(
        "INSERT INTO projects
           (slug, title, root_path, status, target_nle, priority, shoot_date, publish_date, created_at, last_touched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            p.slug,
            p.title,
            p.root_path,
            p.status,
            p.target_nle,
            p.priority,
            p.shoot_date,
            p.publish_date,
            p.created_at,
            p.last_touched_at,
        ],
    )?;
    Ok(())
}

/// Insert or update a project row keyed on `slug` (reconcile path).
pub fn upsert(conn: &Connection, p: &Project) -> Result<()> {
    conn.execute(
        "INSERT INTO projects
           (slug, title, root_path, status, target_nle, priority, shoot_date, publish_date, created_at, last_touched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(slug) DO UPDATE SET
           title = excluded.title,
           root_path = excluded.root_path,
           status = excluded.status,
           target_nle = excluded.target_nle,
           priority = excluded.priority,
           shoot_date = excluded.shoot_date,
           publish_date = excluded.publish_date,
           created_at = excluded.created_at,
           last_touched_at = excluded.last_touched_at",
        params![
            p.slug,
            p.title,
            p.root_path,
            p.status,
            p.target_nle,
            p.priority,
            p.shoot_date,
            p.publish_date,
            p.created_at,
            p.last_touched_at,
        ],
    )?;
    Ok(())
}

/// All projects, most-recently-touched first (untouched rows fall last: NULL
/// sorts smallest in SQLite, so `DESC` places them after every touched row).
pub fn list(conn: &Connection) -> Result<Vec<Project>> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM projects ORDER BY last_touched_at DESC, slug ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Fetch a project by slug, or `None` if absent.
pub fn get(conn: &Connection, slug: &str) -> Result<Option<Project>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM projects WHERE slug = ?1");
    Ok(conn.query_row(&sql, [slug], from_row).optional()?)
}

/// Delete a project row by slug (reconcile removal for a vanished folder).
pub fn delete(conn: &Connection, slug: &str) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE slug = ?1", [slug])?;
    Ok(())
}

/// Update a project's status.
pub fn set_status(conn: &Connection, slug: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET status = ?2 WHERE slug = ?1",
        params![slug, status],
    )?;
    Ok(())
}

/// Update a project's priority.
pub fn set_priority(conn: &Connection, slug: &str, priority: &PriorityLevel) -> Result<()> {
    conn.execute(
        "UPDATE projects SET priority = ?2 WHERE slug = ?1",
        params![slug, priority.as_str()],
    )?;
    Ok(())
}

/// Update a project's shoot and publish dates (either may be cleared with `None`).
pub fn set_dates(
    conn: &Connection,
    slug: &str,
    shoot: Option<&str>,
    publish: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE projects SET shoot_date = ?2, publish_date = ?3 WHERE slug = ?1",
        params![slug, shoot, publish],
    )?;
    Ok(())
}

/// Stamp `last_touched_at` on a project.
pub fn touch(conn: &Connection, slug: &str, ts: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET last_touched_at = ?2 WHERE slug = ?1",
        params![slug, ts],
    )?;
    Ok(())
}

/// Whether a project row with this slug exists (dedupe check during promote).
pub fn slug_exists(conn: &Connection, slug: &str) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM projects WHERE slug = ?1", [slug], |_| Ok(()))
        .optional()?
        .is_some())
}

/// The most recently touched project, or `None` when nothing has been touched.
pub fn most_recently_touched(conn: &Connection) -> Result<Option<Project>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM projects
         WHERE last_touched_at IS NOT NULL
         ORDER BY last_touched_at DESC LIMIT 1"
    );
    Ok(conn.query_row(&sql, [], from_row).optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn sample(slug: &str) -> Project {
        Project {
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
        }
    }

    #[test]
    fn insert_then_get_round_trips() {
        let conn = test_db();
        let p = sample("nvme-deep-dive-2026-07-09");
        insert(&conn, &p).unwrap();
        assert_eq!(get(&conn, &p.slug).unwrap(), Some(p));
    }

    #[test]
    fn get_absent_is_none() {
        let conn = test_db();
        assert_eq!(get(&conn, "nope").unwrap(), None);
    }

    #[test]
    fn insert_duplicate_slug_errors() {
        let conn = test_db();
        let p = sample("dup-2026-07-09");
        insert(&conn, &p).unwrap();
        assert!(insert(&conn, &p).is_err());
    }

    #[test]
    fn upsert_inserts_then_updates() {
        let conn = test_db();
        let mut p = sample("up-2026-07-09");
        upsert(&conn, &p).unwrap();
        p.title = "Renamed".to_string();
        p.status = "editing".to_string();
        upsert(&conn, &p).unwrap();
        let got = get(&conn, &p.slug).unwrap().unwrap();
        assert_eq!(got.title, "Renamed");
        assert_eq!(got.status, "editing");
    }

    #[test]
    fn delete_removes_the_row() {
        let conn = test_db();
        let p = sample("del-2026-07-09");
        insert(&conn, &p).unwrap();
        delete(&conn, &p.slug).unwrap();
        assert_eq!(get(&conn, &p.slug).unwrap(), None);
    }

    #[test]
    fn set_status_updates() {
        let conn = test_db();
        let p = sample("s-2026-07-09");
        insert(&conn, &p).unwrap();
        set_status(&conn, &p.slug, "shooting").unwrap();
        assert_eq!(get(&conn, &p.slug).unwrap().unwrap().status, "shooting");
    }

    #[test]
    fn set_dates_updates_both() {
        let conn = test_db();
        let p = sample("d-2026-07-09");
        insert(&conn, &p).unwrap();
        set_dates(&conn, &p.slug, Some("2026-08-01"), Some("2026-08-15")).unwrap();
        let got = get(&conn, &p.slug).unwrap().unwrap();
        assert_eq!(got.shoot_date.as_deref(), Some("2026-08-01"));
        assert_eq!(got.publish_date.as_deref(), Some("2026-08-15"));
    }

    #[test]
    fn set_dates_clears_with_none() {
        let conn = test_db();
        let mut p = sample("dn-2026-07-09");
        p.shoot_date = Some("2026-08-01".to_string());
        insert(&conn, &p).unwrap();
        set_dates(&conn, &p.slug, None, None).unwrap();
        let got = get(&conn, &p.slug).unwrap().unwrap();
        assert_eq!(got.shoot_date, None);
        assert_eq!(got.publish_date, None);
    }

    #[test]
    fn slug_exists_reflects_presence() {
        let conn = test_db();
        assert!(!slug_exists(&conn, "x-2026-07-09").unwrap());
        insert(&conn, &sample("x-2026-07-09")).unwrap();
        assert!(slug_exists(&conn, "x-2026-07-09").unwrap());
    }

    #[test]
    fn touch_stamps_last_touched() {
        let conn = test_db();
        let p = sample("t-2026-07-09");
        insert(&conn, &p).unwrap();
        touch(&conn, &p.slug, "2026-07-09T12:00:00Z").unwrap();
        assert_eq!(
            get(&conn, &p.slug)
                .unwrap()
                .unwrap()
                .last_touched_at
                .as_deref(),
            Some("2026-07-09T12:00:00Z")
        );
    }

    #[test]
    fn most_recently_touched_picks_latest() {
        let conn = test_db();
        insert(&conn, &sample("a-2026-07-09")).unwrap();
        insert(&conn, &sample("b-2026-07-09")).unwrap();
        insert(&conn, &sample("c-2026-07-09")).unwrap();
        touch(&conn, "a-2026-07-09", "2026-07-09T10:00:00Z").unwrap();
        touch(&conn, "b-2026-07-09", "2026-07-09T12:00:00Z").unwrap();
        assert_eq!(
            most_recently_touched(&conn).unwrap().map(|p| p.slug),
            Some("b-2026-07-09".to_string())
        );
    }

    #[test]
    fn most_recently_touched_none_when_untouched() {
        let conn = test_db();
        insert(&conn, &sample("a-2026-07-09")).unwrap();
        assert_eq!(most_recently_touched(&conn).unwrap(), None);
    }

    #[test]
    fn list_orders_touched_before_untouched() {
        let conn = test_db();
        insert(&conn, &sample("a-2026-07-09")).unwrap();
        insert(&conn, &sample("b-2026-07-09")).unwrap();
        touch(&conn, "b-2026-07-09", "2026-07-09T12:00:00Z").unwrap();
        let slugs: Vec<String> = list(&conn).unwrap().into_iter().map(|p| p.slug).collect();
        assert_eq!(slugs, vec!["b-2026-07-09", "a-2026-07-09"]);
    }

    #[test]
    fn priority_defaults_to_none_on_insert() {
        let conn = test_db();
        let p = sample("pri-2026-07-16");
        insert(&conn, &p).unwrap();
        assert_eq!(get(&conn, &p.slug).unwrap().unwrap().priority, "none");
    }

    #[test]
    fn set_priority_updates() {
        let conn = test_db();
        let p = sample("pri2-2026-07-16");
        insert(&conn, &p).unwrap();
        set_priority(&conn, &p.slug, &PriorityLevel::High).unwrap();
        assert_eq!(get(&conn, &p.slug).unwrap().unwrap().priority, "high");
    }

    #[test]
    fn from_row_keeps_an_unrecognised_priority_verbatim() {
        let conn = test_db();
        let p = sample("pri3-2026-07-16");
        insert(&conn, &p).unwrap();
        conn.execute(
            "UPDATE projects SET priority = 'archived' WHERE slug = ?1",
            [&p.slug],
        )
        .unwrap();
        // Folders are truth: a stale value badges (the TS priorityAppearance
        // returns null for it), it never fails the read.
        assert_eq!(get(&conn, &p.slug).unwrap().unwrap().priority, "archived");
    }

    #[test]
    fn priority_level_maps_to_its_stored_string() {
        assert_eq!(PriorityLevel::Medium.as_str(), "medium");
        assert_eq!(PriorityLevel::None.as_str(), "none");
    }
}
