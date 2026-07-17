use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::broadcast::{self, DriveStatusChanged};
use crate::db::{self, DbHandle};
use crate::error::Result;
use crate::paths;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const GIB: u64 = 1024 * 1024 * 1024;

/// Snapshot of the studio root's reachability. `path: None` means no root is
/// configured yet (pre-onboarding) — reported as mounted so no banner shows.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DriveStatus {
    pub mounted: bool,
    pub path: Option<String>,
    pub free_gb: Option<u32>,
}

/// Read the configured root and compute the current status.
pub async fn current(db: &DbHandle) -> Result<DriveStatus> {
    let root = db
        .call(|conn| db::settings::get(conn, "studio_root"))
        .await?;
    Ok(status_for(root.as_deref()))
}

fn status_for(root: Option<&str>) -> DriveStatus {
    let Some(path) = root else {
        return DriveStatus {
            mounted: true,
            path: None,
            free_gb: None,
        };
    };
    let mounted = paths::root_mounted(Path::new(path));
    let free_gb = if mounted {
        fs4::available_space(Path::new(path))
            .ok()
            .map(|bytes| (bytes / GIB) as u32)
    } else {
        None
    };
    DriveStatus {
        mounted,
        path: Some(path.to_owned()),
        free_gb,
    }
}

/// Poll the studio root for the app's lifetime; broadcast the state once at
/// start and whenever it changes (mount flips, but also a re-picked root —
/// the dashboard's drive card must never show a stale path), and log mount
/// transitions to the events feed.
pub async fn watch(app: AppHandle) {
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    let mut last: Option<(bool, String)> = None;
    loop {
        interval.tick().await;
        let Some(state) = app.try_state::<AppState>() else {
            continue;
        };
        let Ok(status) = current(&state.db).await else {
            continue;
        };
        let Some(path) = status.path.clone() else {
            last = None;
            continue;
        };

        let observed = (status.mounted, path.clone());
        if last.as_ref().is_some_and(|(was, _)| *was != status.mounted) {
            let kind = if status.mounted {
                "drive_reconnected"
            } else {
                "drive_disconnected"
            };
            let payload = serde_json::json!({ "path": path }).to_string();
            let _ = state
                .db
                .call(move |conn| db::events::record(conn, kind, None, Some(&payload)))
                .await;
            broadcast::events_appended(&app);
        }
        if last.as_ref() != Some(&observed) {
            let _ = DriveStatusChanged {
                mounted: status.mounted,
                path,
            }
            .emit(&app);
        }
        last = Some(observed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_root_reports_mounted_with_no_path() {
        let status = status_for(None);
        assert!(status.mounted);
        assert_eq!(status.path, None);
        assert_eq!(status.free_gb, None);
    }

    #[test]
    fn existing_local_root_is_mounted_with_free_space() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let status = status_for(Some(&path));
        assert!(status.mounted);
        assert_eq!(status.path.as_deref(), Some(path.as_str()));
        assert!(status.free_gb.is_some());
    }

    #[test]
    fn missing_local_root_is_unmounted_without_free_space() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone");
        let path = missing.to_string_lossy().into_owned();
        let status = status_for(Some(&path));
        assert!(!status.mounted);
        assert_eq!(status.path.as_deref(), Some(path.as_str()));
        assert_eq!(status.free_gb, None);
    }
}
