//! Asset-protocol scope policy. The static scope in tauri.conf.json is empty
//! because the studio root is a runtime setting; grants happen here — at
//! launch and again whenever `studio_root` changes. Tauri's scope API has no
//! un-allow (and `forbid` would permanently outrank a re-allow), so a
//! replaced root stays readable until relaunch — accepted, single-user app.

use tauri::Manager;

/// Allow the whole studio root over `asset://` so footage plays via
/// `convertFileSrc`. Failure is logged, not fatal: the app works minus video
/// playback, and the grant re-runs on the next settings change.
pub fn allow_studio_root(app: &tauri::AppHandle, root: &str) {
    if let Err(err) = app.asset_protocol_scope().allow_directory(root, true) {
        eprintln!("asset scope grant failed for {root}: {err}");
    }
}

/// Launch-time grant from the persisted `studio_root` setting, if any.
pub fn grant_at_launch(app: &tauri::App) {
    let db = app.state::<crate::state::AppState>().db.clone();
    let root = tauri::async_runtime::block_on(
        db.call(|conn| crate::db::settings::get(conn, "studio_root")),
    );
    if let Ok(Some(root)) = root {
        allow_studio_root(app.handle(), &root);
    }
}
