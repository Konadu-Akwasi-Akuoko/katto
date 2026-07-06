use tauri::State;

use crate::db::jobs::{self, Job};
use crate::error::Result;
use crate::state::AppState;

/// List jobs newest-first. When `active_only`, restrict to `queued`/`running`.
#[tauri::command]
#[specta::specta]
pub async fn list_jobs(state: State<'_, AppState>, active_only: bool) -> Result<Vec<Job>> {
    state
        .db
        .call(move |conn| jobs::list(conn, active_only))
        .await
}
