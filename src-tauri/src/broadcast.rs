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

/// Broadcast when a `katto://` deep link is opened (notification click or OS
/// LaunchServices open); the frontend router navigates to `route` (`"ideas"` or
/// `"project/<slug>"`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DeepLinkOpened {
    pub route: String,
}

/// Broadcast when a camera card is detected and enumerated. Carries no payload:
/// the frontend refetches the `card_offer` query, so the offer crosses IPC once
/// (and the generated event bindings stay free of nested semantic mappers).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct CardDetected;

/// Broadcast when the detected card's volume is unmounted.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct CardRemoved;

/// Broadcast when the session set changes (spawn/close/reap); the dock
/// refetches its session list.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct SessionsChanged;

/// Broadcast on every session state transition; drives tab dots, the sidebar
/// icon states, and the transient done-check.
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct SessionStateChanged {
    pub id: String,
    pub state: crate::sessions::state::SessionState,
}

/// Broadcast when a render lands in a project's `assets/vfx/<effect>/`; the
/// project detail's effects card refetches.
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct VfxRenderLanded {
    pub slug: String,
    pub effect: String,
    pub file: String,
}

/// Broadcast after any tab/history mutation in the browser host; the browser
/// surface refetches its `browser_state` query.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct BrowserStateChanged;

/// Broadcast when a download finished but no project could be resolved to
/// file into; the browser surface opens the pick-a-project sheet.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DownloadNeedsProject {
    pub download_id: String,
    pub filename: String,
}

/// Broadcast when a download filed into a project's assets folder.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DownloadFiled {
    pub project: String,
    pub filename: String,
    pub dest_rel: String,
}

/// Broadcast when interception had a blind spot (blob:/data: downloads) and
/// the file went to ~/Downloads; the frontend shows a persistent notice.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DownloadFallback {
    pub filename: String,
}

/// Broadcast when a download errored before filing.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct DownloadFailed {
    pub filename: String,
}

/// Broadcast when the studio.db import job finishes, carrying the final
/// report the wizard renders.
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct StudioImportFinished {
    pub report: crate::import_studio::ImportReport,
}

/// Broadcast when a PNG lands in (or changes inside) the watched project's
/// `thumbnails/` folder; the detail card and project grid refetch.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct ThumbnailsChanged {
    pub slug: String,
}

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

/// Best-effort, same contract as [`events_appended`].
pub fn card_detected(app: &AppHandle) {
    let _ = CardDetected.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn card_removed(app: &AppHandle) {
    let _ = CardRemoved.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn sessions_changed(app: &AppHandle) {
    let _ = SessionsChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn session_state_changed(
    app: &AppHandle,
    id: &str,
    state: &crate::sessions::state::SessionState,
) {
    let _ = SessionStateChanged {
        id: id.to_string(),
        state: state.clone(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn vfx_render_landed(app: &AppHandle, slug: &str, effect: &str, file: &str) {
    let _ = VfxRenderLanded {
        slug: slug.to_string(),
        effect: effect.to_string(),
        file: file.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn studio_import_finished(app: &AppHandle, report: crate::import_studio::ImportReport) {
    let _ = StudioImportFinished { report }.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn thumbnails_changed(app: &AppHandle, slug: &str) {
    let _ = ThumbnailsChanged {
        slug: slug.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn browser_state_changed(app: &AppHandle) {
    let _ = BrowserStateChanged.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn download_needs_project(app: &AppHandle, download_id: &str, filename: &str) {
    let _ = DownloadNeedsProject {
        download_id: download_id.to_string(),
        filename: filename.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn download_filed(app: &AppHandle, project: &str, filename: &str, dest_rel: &str) {
    let _ = DownloadFiled {
        project: project.to_string(),
        filename: filename.to_string(),
        dest_rel: dest_rel.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn download_fallback(app: &AppHandle, filename: &str) {
    let _ = DownloadFallback {
        filename: filename.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn download_failed(app: &AppHandle, filename: &str) {
    let _ = DownloadFailed {
        filename: filename.to_string(),
    }
    .emit(app);
}

/// Best-effort, same contract as [`events_appended`]. Carries the parsed
/// `katto://` route to the frontend router.
pub fn deep_link_opened(app: &AppHandle, route: &str) {
    let _ = DeepLinkOpened {
        route: route.to_string(),
    }
    .emit(app);
}
