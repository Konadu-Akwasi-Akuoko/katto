use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

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
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app.autolaunch();
        if enabled {
            manager.enable()
        } else {
            manager.disable()
        }
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
    .map_err(|e| Error::Autostart(e.to_string()))?;

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
    Ok(())
}

/// Whether launch-at-login is currently enabled at the OS level.
#[tauri::command]
#[specta::specta]
pub async fn get_autostart(app: AppHandle) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || app.autolaunch().is_enabled())
        .await
        .map_err(|e| Error::Io(e.to_string()))?
        .map_err(|e| Error::Autostart(e.to_string()))
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
