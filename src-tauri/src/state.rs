use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::db::DbHandle;
use crate::ingest::offer::CardOffer;
use crate::jobs::JobRuntime;

/// Application-wide managed state, injected into every command via
/// `State<'_, AppState>`.
pub struct AppState {
    pub db: DbHandle,
    pub jobs: JobRuntime,
    /// Bundle roots with a timeline export in flight (exports are short
    /// direct commands with no jobs row; this stops a double-fire).
    pub active_exports: Arc<Mutex<HashSet<PathBuf>>>,
}

/// Holds the currently-detected card offer, if any. Populated by the volume
/// watcher, read by the `card_offer` command, cleared on unmount. Managed
/// separately from [`AppState`] so the watcher (started in `setup`) can reach
/// it via `app.state()` before any command runs.
#[derive(Default)]
pub struct IngestState {
    pub current: Mutex<Option<CardOffer>>,
}
