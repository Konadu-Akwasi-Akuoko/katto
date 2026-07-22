use rusqlite::Connection;
use serde::Serialize;

use crate::error::Result;

/// A named recurring job with anacron-style catch-up semantics (Phase 6).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ScheduledJob {
    pub name: String,
    pub spec: String,
    pub last_success_at: Option<String>,
    pub enabled: bool,
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledJob> {
    Ok(ScheduledJob {
        name: row.get(0)?,
        spec: row.get(1)?,
        last_success_at: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
    })
}

/// Every scheduled job, ordered by name.
pub fn list(conn: &Connection) -> Result<Vec<ScheduledJob>> {
    let mut stmt = conn
        .prepare("SELECT name, spec, last_success_at, enabled FROM scheduled_jobs ORDER BY name")?;
    let jobs = stmt
        .query_map([], row_to_job)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(jobs)
}

/// Fetch one scheduled job by name, or `None` if absent.
pub fn get(conn: &Connection, name: &str) -> Result<Option<ScheduledJob>> {
    let mut stmt = conn.prepare(
        "SELECT name, spec, last_success_at, enabled FROM scheduled_jobs WHERE name = ?1",
    )?;
    let mut rows = stmt.query_map([name], row_to_job)?;
    Ok(rows.next().transpose()?)
}

/// Record a successful run. `iso_utc` is the caller's timestamp string; the
/// scheduler stores naive-local (`YYYY-MM-DD HH:MM:SS`) to match its due-math.
pub fn set_last_success(conn: &Connection, name: &str, iso_utc: &str) -> Result<()> {
    conn.execute(
        "UPDATE scheduled_jobs SET last_success_at = ?2 WHERE name = ?1",
        (name, iso_utc),
    )?;
    Ok(())
}

/// Update a job's schedule spec and enabled flag.
pub fn update(conn: &Connection, name: &str, spec: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE scheduled_jobs SET spec = ?2, enabled = ?3 WHERE name = ?1",
        (name, spec, enabled as i64),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn seed_row_lists_after_migrations() {
        let conn = test_db();
        let jobs = list(&conn).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "nightly-curation");
        assert_eq!(jobs[0].spec, "daily@00:00;catchup=20h");
        assert!(jobs[0].enabled);
        assert!(jobs[0].last_success_at.is_none());
    }

    #[test]
    fn set_last_success_round_trips() {
        let conn = test_db();
        set_last_success(&conn, "nightly-curation", "2026-07-22T08:00:00Z").unwrap();
        let job = get(&conn, "nightly-curation").unwrap().unwrap();
        assert_eq!(job.last_success_at.as_deref(), Some("2026-07-22T08:00:00Z"));
    }

    #[test]
    fn update_changes_spec_and_enabled() {
        let conn = test_db();
        update(&conn, "nightly-curation", "daily@02:30;catchup=20h", false).unwrap();
        let job = get(&conn, "nightly-curation").unwrap().unwrap();
        assert_eq!(job.spec, "daily@02:30;catchup=20h");
        assert!(!job.enabled);
    }

    #[test]
    fn get_unknown_name_is_none() {
        let conn = test_db();
        assert!(get(&conn, "nope").unwrap().is_none());
    }
}
