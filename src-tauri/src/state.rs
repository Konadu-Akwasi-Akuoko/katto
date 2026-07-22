use std::sync::Mutex;

use crate::commands::ingest::CardOffer;
use crate::db::DbHandle;
use crate::jobs::JobRuntime;

/// Application-wide managed state, injected into every command via
/// `State<'_, AppState>`.
pub struct AppState {
    pub db: DbHandle,
    pub jobs: JobRuntime,
}

/// Holds the currently-detected card offer, if any. Populated by the volume
/// watcher, read by the `card_offer` command, cleared on unmount. Managed
/// separately from [`AppState`] so the watcher (started in `setup`) can reach
/// it via `app.state()` before any command runs.
#[derive(Default)]
pub struct IngestState {
    pub current: Mutex<Option<CardOffer>>,
}
