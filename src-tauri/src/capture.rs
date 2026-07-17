//! Quick capture: a global hotkey opens a small always-on-top window that drops a
//! single idea into the backlog, so a thought can be caught from anywhere without
//! surfacing the main window. Registration is entirely backend; the window loads
//! the same SPA and branches on its `capture` label to render only the form.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::db;
use crate::state::AppState;

/// Default quick-capture accelerator (`⌥⌘K`). Rebindable via the `capture_shortcut`
/// settings key; parsed by the global-shortcut plugin's accelerator grammar.
///
/// Not `⌥⌘I`: that is WKWebView's Web Inspector binding, so it fires the inspector
/// alongside the capture window in a dev build, and browsers claim it for devtools.
pub const DEFAULT_CAPTURE_SHORTCUT: &str = "alt+cmd+k";

/// Window label for the capture window. The frontend switches on this label to
/// render only the capture form; the capability file scopes its permissions to it.
const CAPTURE: &str = "capture";

/// Read the configured accelerator (falling back to [`DEFAULT_CAPTURE_SHORTCUT`])
/// and register the quick-capture hotkey. A registration conflict — another app
/// already owns the accelerator — is recorded as a `capture_hotkey_unavailable`
/// event and otherwise swallowed: it must never block startup or raise a dialog
/// (the settings surface offers a rebind). Called once from the setup hook.
pub fn setup(app: &AppHandle) {
    let db = app.state::<AppState>().db.clone();
    let accel =
        tauri::async_runtime::block_on(db.call(|conn| db::settings::get(conn, "capture_shortcut")))
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_CAPTURE_SHORTCUT.to_string());

    if let Err(err) = register_capture_hotkey(app, &accel) {
        let detail = format!("capture hotkey '{accel}' unavailable: {err}");
        let _ = tauri::async_runtime::block_on(db.call(move |conn| {
            db::events::record(conn, "capture_hotkey_unavailable", None, Some(&detail))
        }));
    }
}

/// Register `accel` so pressing it opens the capture window. Only the key-press
/// edge acts; the release edge is ignored. Returns the plugin error on a
/// registration conflict so the caller can record it.
pub fn register_capture_hotkey(
    app: &AppHandle,
    accel: &str,
) -> std::result::Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = open_capture_window(app);
            }
        })
}

/// Show the capture window, focusing an existing one or building a fresh
/// borderless, always-on-top, centered ~420×160 window that loads the SPA (which
/// renders only the capture form for the `capture` label).
///
/// `visible_on_all_workspaces` is what makes capture-from-anywhere true: without
/// it the `NSWindow` belongs to the Space it was born on, so a hotkey pressed on
/// another Space opens the window out of sight (and drags the app's Space forward
/// instead). It maps to `NSWindowCollectionBehaviorCanJoinAllSpaces`.
///
/// The frame is transparent because the form paints its own rounded surface;
/// a decorationless `NSWindow` is otherwise a hard-cornered rectangle. This is
/// why the app opts into `macOSPrivateApi`.
pub fn open_capture_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(CAPTURE) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::default())
        .title("katto — capture")
        .inner_size(420.0, 160.0)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}
