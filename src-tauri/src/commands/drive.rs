use tauri::State;

use crate::drive::{self, DriveStatus};
use crate::error::Result;
use crate::state::AppState;

/// Current studio-root reachability snapshot; broadcasts keep it fresh after
/// this initial query.
#[tauri::command]
#[specta::specta]
pub async fn get_drive_status(state: State<'_, AppState>) -> Result<DriveStatus> {
    drive::current(&state.db).await
}
