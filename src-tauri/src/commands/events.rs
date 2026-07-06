use tauri::State;

use crate::db::RowId;
use crate::db::events::{self, Event};
use crate::error::Result;
use crate::state::AppState;

/// Most-recent-first page of activity-log events. `before_id` pages backward.
#[tauri::command]
#[specta::specta]
pub async fn list_events(
    state: State<'_, AppState>,
    limit: u32,
    before_id: Option<RowId>,
) -> Result<Vec<Event>> {
    state
        .db
        .call(move |conn| events::list(conn, limit, before_id))
        .await
}
