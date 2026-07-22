use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::db::events;
use crate::error::{Error, Result};
use crate::state::AppState;

/// Enable or disable launch-at-login, recording the change in the activity
/// log. The AppleScript-backed call can block, so it runs off the runtime.
#[tauri::command]
#[specta::specta]
pub async fn set_autostart(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<()> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let manager = handle.autolaunch();
        if enabled {
            manager.enable()
        } else {
            manager.disable()
        }
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;

    state
        .db
        .call(move |conn| {
            events::record(
                conn,
                "autostart_changed",
                None,
                Some(if enabled {
                    r#"{"enabled":true}"#
                } else {
                    r#"{"enabled":false}"#
                }),
            )
        })
        .await?;
    crate::broadcast::events_appended(&app);
    Ok(())
}

/// Whether launch-at-login is currently enabled at the OS level.
#[tauri::command]
#[specta::specta]
pub async fn get_autostart(app: AppHandle) -> Result<bool> {
    let enabled = tauri::async_runtime::spawn_blocking(move || app.autolaunch().is_enabled())
        .await
        .map_err(|e| Error::Io(e.to_string()))??;
    Ok(enabled)
}

/// Close the main window: the WebView is destroyed and katto stays resident
/// in the tray (same path as clicking the window's close button).
#[tauri::command]
#[specta::specta]
pub async fn sleep_to_tray(app: AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.close().map_err(|e| Error::Io(e.to_string()))?;
    }
    Ok(())
}

/// Exit katto completely (same exit path as the tray's Quit item).
#[tauri::command]
#[specta::specta]
pub async fn quit_app(app: AppHandle) -> Result<()> {
    app.exit(0);
    Ok(())
}

/// Open a web link in the default browser. Only http(s) URLs are accepted —
/// idea `source_url` values come from external tools, and anything else
/// (`file:`, `javascript:`, custom schemes) must not reach the OS opener.
#[tauri::command]
#[specta::specta]
pub async fn open_external_url(app: AppHandle, url: String) -> Result<()> {
    validate_external_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Io(e.to_string()))
}

/// The http(s)-only guard for [`open_external_url`], factored out for
/// testing. Schemes compare case-insensitively (RFC 3986).
fn validate_external_url(url: &str) -> Result<()> {
    let lowered = url
        .get(..8.min(url.len()))
        .unwrap_or("")
        .to_ascii_lowercase();
    if lowered.starts_with("https://") || lowered.starts_with("http://") {
        Ok(())
    } else {
        Err(Error::Io(format!("refusing to open non-http url: {url}")))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;

    #[test]
    fn validate_external_url_accepts_http_and_https() {
        assert!(validate_external_url("https://news.ycombinator.com/item?id=1").is_ok());
        assert!(validate_external_url("http://example.com").is_ok());
        // URL schemes are case-insensitive (RFC 3986); an uppercase scheme is
        // the same safe scheme, not a different one.
        assert!(validate_external_url("HTTPS://cased.example").is_ok());
    }

    #[test]
    fn validate_external_url_rejects_other_schemes() {
        assert!(validate_external_url("file:///etc/passwd").is_err());
        assert!(validate_external_url("javascript:alert(1)").is_err());
        assert!(validate_external_url("FILE:///etc/passwd").is_err());
        assert!(validate_external_url("not a url").is_err());
    }
}
