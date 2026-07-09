use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_specta::Event;

/// Broadcast after any `events` row is written; the dashboard refetches its feed.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct EventsAppended;

/// Broadcast whenever any `jobs` row is created or changes state.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct JobsChanged;

/// Broadcast on studio-root mount transitions (and once at watcher start).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DriveStatusChanged {
    pub mounted: bool,
    pub path: String,
}

/// Broadcast after any project row is created or mutated (status, dates,
/// reconcile add/remove); planner and projects surfaces refetch their lists.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct ProjectsChanged;

/// Broadcast after any idea row is created or mutated (create, update, discard,
/// promote); the backlog surface refetches its list.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct IdeasChanged;

/// Broadcast after any schedule entry is upserted or deleted; the calendar
/// refetches and the tray's next-shoot line refreshes.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct ScheduleChanged;

/// Best-effort: a missed signal only delays a refetch until the next query
/// mount, so emit failures (e.g. no live WebView) are ignored.
pub fn events_appended(app: &AppHandle) {
    let _ = EventsAppended.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn jobs_changed(app: &AppHandle) {
    let _ = JobsChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`]. A project change can move
/// the tray's current-project line, so the planner lines refresh here too.
pub fn projects_changed(app: &AppHandle) {
    let _ = ProjectsChanged.emit(app);
    crate::tray::refresh_planner_lines(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn ideas_changed(app: &AppHandle) {
    let _ = IdeasChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`]. Refreshes the tray's
/// next-shoot line alongside the broadcast.
pub fn schedule_changed(app: &AppHandle) {
    let _ = ScheduleChanged.emit(app);
    crate::tray::refresh_planner_lines(app);
}
