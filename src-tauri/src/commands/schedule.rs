use tauri::{AppHandle, State};

use crate::db;
use crate::db::RowId;
use crate::db::schedule::ScheduleEntry;
use crate::error::Result;
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

/// Pin a project to a date. There is at most one entry per `(project_slug, kind)`
/// pair, so this inserts or updates in place. Broadcasts `schedule-changed`, which
/// also refreshes the tray's next-shoot line.
#[tauri::command]
#[specta::specta]
pub async fn upsert_schedule_entry(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    kind: String,
    date: String,
    note: Option<String>,
) -> Result<ScheduleEntry> {
    let entry = state
        .db
        .call(move |conn| db::schedule::upsert(conn, &project_slug, &kind, &date, note.as_deref()))
        .await?;
    crate::broadcast::schedule_changed(&app);
    Ok(entry)
}

/// Remove a schedule entry by id and broadcast `schedule-changed`.
#[tauri::command]
#[specta::specta]
pub async fn delete_schedule_entry(
    state: State<'_, AppState>,
    app: AppHandle,
    id: RowId,
) -> Result<()> {
    state
        .db
        .call(move |conn| db::schedule::delete(conn, id.0))
        .await?;
    crate::broadcast::schedule_changed(&app);
    Ok(())
}
