use std::path::Path;

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::commands::projects::{now_rfc3339, require_mounted};
use crate::db;
use crate::db::schedule::{ScheduleEntry, ScheduleKind};
use crate::error::{Error, Result};
use crate::projects::manifest::{ProjectManifest, read_manifest, write_manifest};
use crate::state::AppState;

/// Schedule entries whose date falls within `[from, to]` (inclusive ISO bounds),
/// ordered by date. Drives the calendar's month/week views.
#[tauri::command]
#[specta::specta]
pub async fn list_schedule(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<Vec<ScheduleEntry>> {
    state
        .db
        .call(move |conn| db::schedule::list_range(conn, &from, &to))
        .await
}

/// Apply one pin to the three stores that must agree: the schedule index, the
/// in-memory manifest (the caller writes it to disk), and the project row.
/// `date == None` clears the pin. Returns the written entry (or `None` when
/// cleared). The caller owns the manifest write, touch, event, and broadcast so a
/// two-field update (`set_project_dates`) writes the manifest once.
pub(crate) fn write_pin(
    conn: &Connection,
    slug: &str,
    manifest: &mut ProjectManifest,
    kind: ScheduleKind,
    date: Option<&str>,
    note: Option<&str>,
) -> Result<Option<ScheduleEntry>> {
    let entry = match date {
        Some(d) => Some(db::schedule::upsert(conn, slug, kind.as_str(), d, note)?),
        None => {
            db::schedule::delete_for(conn, slug, kind.as_str())?;
            None
        }
    };
    let owned = date.map(str::to_string);
    match kind {
        ScheduleKind::Shoot => manifest.shoot_date = owned,
        ScheduleKind::Publish => manifest.publish_date = owned,
    }
    db::projects::set_dates(
        conn,
        slug,
        manifest.shoot_date.as_deref(),
        manifest.publish_date.as_deref(),
    )?;
    Ok(entry)
}

/// Pin a project to a date (shoot or publish). Writes the schedule index, mirrors
/// the date into `project.json` and the project row, touches, records an event,
/// and broadcasts. `ScheduleKind` is an enum, so a bad kind is rejected at the IPC
/// boundary. Guarded by the studio-root mount like every other folder write.
#[tauri::command]
#[specta::specta]
pub async fn upsert_schedule_entry(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    kind: ScheduleKind,
    date: String,
    note: Option<String>,
) -> Result<ScheduleEntry> {
    let now = now_rfc3339()?;
    let entry = state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            let project = db::projects::get(conn, &project_slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {project_slug}")))?;
            let dir = Path::new(&project.root_path);
            let mut manifest = read_manifest(dir)?;
            let entry = write_pin(
                conn,
                &project_slug,
                &mut manifest,
                kind,
                Some(&date),
                note.as_deref(),
            )?
            .ok_or_else(|| Error::Io("pin write returned no entry".to_string()))?;
            write_manifest(dir, &manifest)?;
            db::projects::touch(conn, &project_slug, &now)?;
            db::events::record(conn, "project-dates-changed", Some(&project_slug), None)?;
            Ok(entry)
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    crate::broadcast::schedule_changed(&app);
    Ok(entry)
}

/// Clear a project's shoot or publish pin. Removes the schedule row and the
/// mirrored date from the manifest and row, touches, records an event, and
/// broadcasts.
#[tauri::command]
#[specta::specta]
pub async fn delete_schedule_entry(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    kind: ScheduleKind,
) -> Result<()> {
    let now = now_rfc3339()?;
    state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            let project = db::projects::get(conn, &project_slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {project_slug}")))?;
            let dir = Path::new(&project.root_path);
            let mut manifest = read_manifest(dir)?;
            write_pin(conn, &project_slug, &mut manifest, kind, None, None)?;
            write_manifest(dir, &manifest)?;
            db::projects::touch(conn, &project_slug, &now)?;
            db::events::record(conn, "project-dates-changed", Some(&project_slug), None)?;
            Ok(())
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    crate::broadcast::schedule_changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::projects::create_project_inner;
    use crate::db::test_db;

    #[test]
    fn write_pin_updates_schedule_manifest_and_row() {
        let conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let project = create_project_inner(
            &conn,
            &root.path().join("Projects"),
            "NVMe",
            None,
            "resolve",
            "2026-07-09T10:00:00Z",
        )
        .unwrap();
        let dir = Path::new(&project.root_path);
        let mut manifest = read_manifest(dir).unwrap();

        write_pin(
            &conn,
            &project.slug,
            &mut manifest,
            ScheduleKind::Shoot,
            Some("2026-08-01"),
            None,
        )
        .unwrap();
        write_manifest(dir, &manifest).unwrap();

        assert_eq!(manifest.shoot_date.as_deref(), Some("2026-08-01"));
        assert_eq!(
            db::projects::get(&conn, &project.slug)
                .unwrap()
                .unwrap()
                .shoot_date
                .as_deref(),
            Some("2026-08-01")
        );
        let rows = db::schedule::list_range(&conn, "2026-08-01", "2026-08-01").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "shoot");

        write_pin(
            &conn,
            &project.slug,
            &mut manifest,
            ScheduleKind::Shoot,
            None,
            None,
        )
        .unwrap();
        assert_eq!(manifest.shoot_date, None);
        assert!(
            db::schedule::list_range(&conn, "2026-08-01", "2026-08-01")
                .unwrap()
                .is_empty()
        );
    }
}
