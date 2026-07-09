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

/// Best-effort: a missed signal only delays a refetch until the next query
/// mount, so emit failures (e.g. no live WebView) are ignored.
pub fn events_appended(app: &AppHandle) {
    let _ = EventsAppended.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn jobs_changed(app: &AppHandle) {
    let _ = JobsChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn projects_changed(app: &AppHandle) {
    let _ = ProjectsChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn ideas_changed(app: &AppHandle) {
    let _ = IdeasChanged.emit(app);
}
