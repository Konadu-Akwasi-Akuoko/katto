//! Asset-protocol scope policy. The static scope in tauri.conf.json is empty
//! and there is deliberately NO blanket studio-root grant: custom protocols
//! are registered on every webview — including the browser's child webviews,
//! which load remote, untrusted pages — so a recursive root grant would let
//! a hostile page `fetch("asset://…")` any file under the studio root
//! (tauri#13224). Instead, each surface grants exactly the files/dirs it is
//! about to render: the opened bundle, its source video, listed thumbnail
//! PNGs, listed vfx render dirs. Tauri's scope API has no un-allow, so
//! grants live until relaunch — accepted, single-user app.

use tauri::Manager;

/// Allow one file over `asset://` — a source video (possibly outside the
/// studio root) or a thumbnail PNG a card is about to render. Failure is
/// logged, not fatal: the app works minus that media, and the grant re-runs
/// on the next open/list.
pub fn allow_source_file(app: &tauri::AppHandle, path: &std::path::Path) {
    if let Err(err) = app.asset_protocol_scope().allow_file(path) {
        eprintln!("asset scope grant failed for {}: {err}", path.display());
    }
}

/// Allow one directory over `asset://` for media the app itself generated
/// (bundle dirs with `thumbs/` + `cached_audio.wav`, vfx effect render
/// dirs). Same failure policy as [`allow_source_file`].
pub fn allow_media_dir(app: &tauri::AppHandle, path: &std::path::Path, recursive: bool) {
    if let Err(err) = app.asset_protocol_scope().allow_directory(path, recursive) {
        eprintln!("asset scope grant failed for {}: {err}", path.display());
    }
}
