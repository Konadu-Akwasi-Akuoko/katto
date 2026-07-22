//! Quick capture: a global hotkey opens a small always-on-top window that drops a
//! single idea into the backlog, so a thought can be caught from anywhere without
//! surfacing the main window. Registration is entirely backend; the window loads
//! the same SPA and branches on its `capture` label to render only the form.

use std::str::FromStr;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::db;
use crate::error::Error;
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
/// (Settings → General rebinds via `set_capture_shortcut`). Called once from the
/// setup hook.
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

/// Parse-validate `accel` against the plugin's accelerator grammar without
/// registering it. Rejects combos with none of ⌘/⌃/⌥: the grammar itself allows
/// a bare key or ⇧-only combo, but registering one would shadow ordinary typing
/// system-wide.
pub fn validate_accelerator(accel: &str) -> crate::error::Result<()> {
    let parsed = Shortcut::from_str(accel).map_err(|err| {
        Error::ShortcutInvalid(format!("'{accel}' is not a valid shortcut: {err}"))
    })?;
    if !parsed
        .mods
        .intersects(Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT)
    {
        return Err(Error::ShortcutInvalid(format!(
            "'{accel}' needs at least one of ⌘, ⌃, ⌥"
        )));
    }
    Ok(())
}

/// Swap the registered capture hotkey from `old` to `new`. The old accelerator
/// is unregistered first (the OS may refuse a probe while the app still owns a
/// conflicting combo); if registering `new` fails — another app owns it — `old`
/// is re-registered so the previous binding survives, and the plugin error is
/// returned for the command layer to surface. If that rollback also fails (a
/// race with another app grabbing `old` in the same instant), the loss is
/// recorded as a `capture_hotkey_unavailable` events row rather than silently.
pub fn rebind_capture_hotkey(
    app: &AppHandle,
    old: &str,
    new: &str,
) -> std::result::Result<(), tauri_plugin_global_shortcut::Error> {
    // The startup registration may itself have failed, so `old` may not be held.
    let _ = app.global_shortcut().unregister(old);
    if let Err(err) = register_capture_hotkey(app, new) {
        if let Err(rollback_err) = register_capture_hotkey(app, old) {
            let db = app.state::<AppState>().db.clone();
            let detail = format!("capture hotkey rollback to '{old}' failed: {rollback_err}");
            tauri::async_runtime::spawn(async move {
                let _ = db
                    .call(move |conn| {
                        db::events::record(conn, "capture_hotkey_unavailable", None, Some(&detail))
                    })
                    .await;
            });
        }
        return Err(err);
    }
    Ok(())
}

/// Show the capture window, focusing an existing one or building a fresh
/// borderless, always-on-top, centered ~420×160 window that loads the SPA (which
/// renders only the capture form for the `capture` label).
///
/// `visible_on_all_workspaces` (`CanJoinAllSpaces`) makes the window follow the
/// active desktop Space instead of belonging to the Space it was born on;
/// `FullScreenAuxiliary` is OR-ed in natively below (tauri has no builder
/// option) as the documented requirement for overlaying another app's
/// fullscreen Space. KNOWN GAP: in practice the window still opens on katto's
/// own Space when another app is fullscreen — the pair is necessary but not
/// sufficient. Parked in TODO.md ("Parked issues") with the remaining leads
/// (non-activating NSPanel, activation policy, window level).
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
    let window = WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::default())
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
    #[cfg(target_os = "macos")]
    allow_fullscreen_spaces(&window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}

/// OR `FullScreenAuxiliary` into the capture window's `collectionBehavior` —
/// the documented prerequisite for a panel joining another app's fullscreen
/// Space (tauri#11488), though the manual pass shows it is not sufficient on
/// its own (see TODO.md). `collectionBehavior` is NSWindow instance state, so
/// applying it once after build covers the show/focus reuse path too. `NSWindow`
/// is main-thread-only, so the mutation hops via `run_on_main_thread` and the
/// raw pointer is obtained inside the closure (it is not `Send`). Best-effort:
/// a failed hop only means the pre-existing desktop-Spaces-only behavior.
#[cfg(target_os = "macos")]
fn allow_fullscreen_spaces(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let handle = window.clone();
    let _ = window.run_on_main_thread(move || {
        if let Ok(ptr) = handle.ns_window() {
            // SAFETY: ns_window returns a live NSWindow* for this window, and we
            // are on the main thread (NSWindow is MainThreadOnly).
            let ns_window = unsafe { &*ptr.cast::<NSWindow>() };
            ns_window.setCollectionBehavior(
                ns_window.collectionBehavior()
                    | NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[test]
    fn validate_accepts_default_and_common_forms() {
        for accel in ["alt+cmd+k", "ctrl+shift+f5", "cmd+,", "CmdOrCtrl+P"] {
            assert!(validate_accelerator(accel).is_ok(), "rejected '{accel}'");
        }
    }

    #[test]
    fn validate_rejects_garbage() {
        for accel in ["", "cmd+", "notakey+cmd", "cmd+k+j"] {
            assert!(
                matches!(validate_accelerator(accel), Err(Error::ShortcutInvalid(_))),
                "accepted '{accel}'"
            );
        }
    }

    #[test]
    fn validate_rejects_modifierless_and_shift_only() {
        for accel in ["k", "shift+k"] {
            assert!(
                matches!(validate_accelerator(accel), Err(Error::ShortcutInvalid(_))),
                "accepted '{accel}'"
            );
        }
    }
}
