use std::path::Path;

use tauri::State;

use crate::db;
use crate::error::{Error, Result};
use crate::paths;
use crate::projects::reconcile::{self, ReconcileReport};
use crate::state::AppState;

/// Reconcile the projects index against the studio-root folders on demand
/// (folders are truth). Reads the configured `studio_root`, refuses when it is
/// unmounted, then scans `<root>/Projects`, diffs against the index, and applies.
///
/// Returns an empty report when no studio root is configured yet.
#[tauri::command]
#[specta::specta]
pub async fn rescan_projects(state: State<'_, AppState>) -> Result<ReconcileReport> {
    state
        .db
        .call(|conn| {
            let Some(root) = db::settings::get(conn, "studio_root")? else {
                return Ok(ReconcileReport::default());
            };
            if !paths::root_mounted(Path::new(&root)) {
                return Err(Error::StudioRootUnmounted(format!(
                    "studio root is not mounted: {root}"
                )));
            }
            reconcile::reconcile_root(conn, &root)
        })
        .await
}
