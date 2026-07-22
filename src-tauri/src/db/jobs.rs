use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;

use crate::error::{Error, Result};

/// A background job row. `status` moves `queued -> running -> (done | failed)`;
/// no other transition is legal. This repository owns the DB state only; writing
/// the terminal `events` row is the jobs runtime's job (a later slice).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub progress: f64,
    pub payload_json: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

fn from_row(row: &Row) -> rusqlite::Result<Job> {
    Ok(Job {
        id: row.get("id")?,
        kind: row.get("kind")?,
        label: row.get("label")?,
        status: row.get("status")?,
        progress: row.get("progress")?,
        payload_json: row.get("payload_json")?,
        error: row.get("error")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
    })
}

/// Insert a new job in the `queued` state with zero progress.
pub fn create(
    conn: &Connection,
    id: &str,
    kind: &str,
    label: &str,
    payload_json: Option<&str>,
) -> Result<Job> {
    conn.execute(
        "INSERT INTO jobs (id, kind, label, status, progress, payload_json)
         VALUES (?1, ?2, ?3, 'queued', 0, ?4)",
        params![id, kind, label, payload_json],
    )?;
    Ok(Job {
        id: id.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        status: "queued".to_string(),
        progress: 0.0,
        payload_json: payload_json.map(str::to_string),
        error: None,
        started_at: None,
        finished_at: None,
    })
}

/// Fetch a job by id, or `None` if absent.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Job>> {
    Ok(conn
        .query_row(
            "SELECT id, kind, label, status, progress, payload_json, error, started_at, finished_at
             FROM jobs WHERE id = ?1",
            [id],
            from_row,
        )
        .optional()?)
}

/// Fetch a job that is known to exist (after a successful transition), surfacing
/// a `QueryReturnedNoRows` DB error rather than panicking if it somehow doesn't.
fn fetch(conn: &Connection, id: &str) -> Result<Job> {
    Ok(conn.query_row(
        "SELECT id, kind, label, status, progress, payload_json, error, started_at, finished_at
         FROM jobs WHERE id = ?1",
        [id],
        from_row,
    )?)
}

/// Jobs newest-first. When `active_only`, restrict to `queued`/`running`.
pub fn list(conn: &Connection, active_only: bool) -> Result<Vec<Job>> {
    let sql =
        "SELECT id, kind, label, status, progress, payload_json, error, started_at, finished_at
               FROM jobs
               WHERE (?1 = 0 OR status IN ('queued','running'))
               ORDER BY rowid DESC";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![active_only as i64], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Whether an active (queued/running) job of `kind` references `needle` in
/// its payload. DB-backed on purpose: this guards one-run-per-bundle, and an
/// in-memory guard would die with a webview reload.
pub fn active_with_payload(conn: &Connection, kind: &str, needle: &str) -> Result<bool> {
    let exists: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs
          WHERE kind = ?1 AND status IN ('queued','running')
            AND instr(payload_json, ?2) > 0)",
        params![kind, needle],
        |row| row.get(0),
    )?;
    Ok(exists != 0)
}

/// `queued -> running`. Stamps `started_at`.
pub fn start(conn: &Connection, id: &str) -> Result<Job> {
    transition(conn, id, "running")?;
    conn.execute(
        "UPDATE jobs SET started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ?1",
        [id],
    )?;
    fetch(conn, id)
}

/// Update progress (clamped to `0.0..=1.0`) on a running job.
pub fn set_progress(conn: &Connection, id: &str, progress: f64) -> Result<()> {
    let clamped = progress.clamp(0.0, 1.0);
    conn.execute(
        "UPDATE jobs SET progress = ?2 WHERE id = ?1 AND status = 'running'",
        params![id, clamped],
    )?;
    Ok(())
}

/// `running -> done`. Sets progress to 1.0 and stamps `finished_at`.
pub fn finish(conn: &Connection, id: &str) -> Result<Job> {
    transition(conn, id, "done")?;
    conn.execute(
        "UPDATE jobs
         SET progress = 1.0, finished_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id = ?1",
        [id],
    )?;
    fetch(conn, id)
}

/// `running -> failed`, or `queued -> failed` for a job that could not start.
/// Records `error` and stamps `finished_at`.
pub fn fail(conn: &Connection, id: &str, error: &str) -> Result<Job> {
    transition(conn, id, "failed")?;
    conn.execute(
        "UPDATE jobs
         SET error = ?2, finished_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id = ?1",
        params![id, error],
    )?;
    fetch(conn, id)
}

/// Enforce the legal state machine, then write the new status. Legal moves:
/// `queued -> running`, `running -> done`, `running -> failed`, and
/// `queued -> failed` (a job that could not start has failed).
fn transition(conn: &Connection, id: &str, to: &str) -> Result<()> {
    let from: String =
        conn.query_row("SELECT status FROM jobs WHERE id = ?1", [id], |r| r.get(0))?;
    let legal = matches!(
        (from.as_str(), to),
        ("queued", "running") | ("queued", "failed") | ("running", "done") | ("running", "failed")
    );
    if !legal {
        return Err(Error::JobTransition(format!(
            "invalid job transition: {from} -> {to}"
        )));
    }
    conn.execute("UPDATE jobs SET status = ?2 WHERE id = ?1", params![id, to])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn seed(conn: &Connection) -> Job {
        create(conn, "j1", "transcribe", "Transcribe clip", None).unwrap()
    }

    #[test]
    fn active_with_payload_guards_only_live_jobs_of_the_kind() {
        let conn = test_db();
        create(
            &conn,
            "j1",
            "cut_pipeline",
            "Rough cut",
            Some("{\"bundle_path\":\"/studio/p/audio/c.kruproj\"}"),
        )
        .unwrap();
        assert!(active_with_payload(&conn, "cut_pipeline", "/studio/p/audio/c.kruproj").unwrap());
        assert!(
            !active_with_payload(&conn, "cut_pipeline", "/studio/p/audio/other.kruproj").unwrap()
        );
        assert!(!active_with_payload(&conn, "ingest", "/studio/p/audio/c.kruproj").unwrap());
        start(&conn, "j1").unwrap();
        assert!(active_with_payload(&conn, "cut_pipeline", "/studio/p/audio/c.kruproj").unwrap());
        fail(&conn, "j1", "boom").unwrap();
        assert!(!active_with_payload(&conn, "cut_pipeline", "/studio/p/audio/c.kruproj").unwrap());
    }

    #[test]
    fn create_starts_queued_at_zero() {
        let conn = test_db();
        let job = seed(&conn);
        assert_eq!(job.status, "queued");
        assert_eq!(job.progress, 0.0);
        assert!(job.started_at.is_none() && job.finished_at.is_none());
    }

    #[test]
    fn happy_path_queued_running_done() {
        let conn = test_db();
        seed(&conn);
        assert_eq!(start(&conn, "j1").unwrap().status, "running");
        set_progress(&conn, "j1", 0.5).unwrap();
        let done = finish(&conn, "j1").unwrap();
        assert_eq!(done.status, "done");
        assert_eq!(done.progress, 1.0);
        assert!(done.started_at.is_some() && done.finished_at.is_some());
    }

    #[test]
    fn running_can_fail_with_message() {
        let conn = test_db();
        seed(&conn);
        start(&conn, "j1").unwrap();
        let failed = fail(&conn, "j1", "ffmpeg exited 1").unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("ffmpeg exited 1"));
    }

    #[test]
    fn fail_from_queued_succeeds() {
        let conn = test_db();
        seed(&conn);
        let failed = fail(&conn, "j1", "failed to start: db locked").unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("failed to start: db locked"));
        assert!(failed.started_at.is_none() && failed.finished_at.is_some());
    }

    #[test]
    fn cannot_finish_a_queued_job() {
        let conn = test_db();
        seed(&conn);
        let err = finish(&conn, "j1").unwrap_err();
        assert!(matches!(err, Error::JobTransition(_)));
    }

    #[test]
    fn cannot_restart_a_done_job() {
        let conn = test_db();
        seed(&conn);
        start(&conn, "j1").unwrap();
        finish(&conn, "j1").unwrap();
        assert!(matches!(
            start(&conn, "j1").unwrap_err(),
            Error::JobTransition(_)
        ));
    }

    #[test]
    fn progress_is_ignored_before_running() {
        let conn = test_db();
        seed(&conn);
        set_progress(&conn, "j1", 0.9).unwrap();
        assert_eq!(get(&conn, "j1").unwrap().unwrap().progress, 0.0);
    }

    #[test]
    fn active_only_excludes_terminal_jobs() {
        let conn = test_db();
        create(&conn, "a", "k", "A", None).unwrap();
        create(&conn, "b", "k", "B", None).unwrap();
        start(&conn, "b").unwrap();
        finish(&conn, "b").unwrap();
        let active: Vec<String> = list(&conn, true)
            .unwrap()
            .into_iter()
            .map(|j| j.id)
            .collect();
        assert_eq!(active, vec!["a"]);
    }
}
