use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, ExitRequestApi, Manager, WebviewUrl, WebviewWindowBuilder, Window};

use crate::tray;

const MAIN: &str = "main";

/// Lifecycle flag distinguishing "closed to the tray" from a genuine quit.
///
/// Set only just before we tear the window down to free its WebView. The
/// resulting `ExitRequested` (last window destroyed → `code: None`) is then
/// prevented so katto stays resident in the menu bar. Native quit paths
/// (⌘Q, app-menu Quit, Dock-menu Quit) reach `ExitRequested` without going
/// through our close/destroy, so the flag stays clear and they quit normally.
#[derive(Default)]
struct Lifecycle {
    closing_to_tray: AtomicBool,
}

/// Register lifecycle state and open the main window. Called once from setup.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    app.manage(Lifecycle::default());
    show_main(app)
}

/// Build the main window, maximized to fill the available screen work area.
/// The inner size is the pre-maximize restore size, not an override.
fn create_main(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("katto")
        .inner_size(1100.0, 720.0)
        .maximized(true)
        .build()?;
    Ok(())
}

/// Ensure the main window is on screen and focused, recreating it if it was
/// closed to the tray. Clears the lifecycle flag: a live window means the next
/// `ExitRequested` is a genuine quit unless a fresh teardown sets it again.
pub fn show_main(app: &AppHandle) -> tauri::Result<()> {
    set_closing_to_tray(app, false);
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        create_main(app)?;
    }
    tray::set_window_shown(app, true);
    Ok(())
}

/// Toggle the main window: destroy it (freeing the WebView) when present, or
/// recreate it when absent.
pub fn toggle_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        set_closing_to_tray(app, true);
        let _ = window.destroy();
        tray::set_window_shown(app, false);
    } else {
        let _ = show_main(app);
    }
}

/// Window red-button close: free the WebView but keep katto in the menu bar.
/// The close is not prevented, so the default teardown destroys the WebView;
/// the follow-on `ExitRequested` is caught by [`on_exit_requested`].
pub fn on_close_requested(window: &Window) {
    let app = window.app_handle();
    set_closing_to_tray(app, true);
    tray::set_window_shown(app, false);
}

/// Keep the process (and tray) alive when the window was closed to the tray;
/// let every genuine quit path through.
pub fn on_exit_requested(app: &AppHandle, api: &ExitRequestApi) {
    if let Some(state) = app.try_state::<Lifecycle>()
        && state.closing_to_tray.swap(false, Ordering::Relaxed)
    {
        api.prevent_exit();
    }
}

fn set_closing_to_tray(app: &AppHandle, value: bool) {
    if let Some(state) = app.try_state::<Lifecycle>() {
        state.closing_to_tray.store(value, Ordering::Relaxed);
    }
}
