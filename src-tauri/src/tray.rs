use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use crate::window;

/// Tray handles kept alive for the app's lifetime so the menu can be updated
/// without rebuilding the tray.
struct TrayState {
    toggle: MenuItem<Wry>,
    job: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Hide window", true, None::<&str>)?;
    let job = MenuItem::with_id(app, "job", "No active job", false, None::<&str>)?;
    let project = MenuItem::with_id(app, "project", "No project", false, None::<&str>)?;
    let shoot = MenuItem::with_id(app, "shoot", "No shoot scheduled", false, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let sep_bottom = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &sep_top,
            &job,
            &project,
            &shoot,
            &sep_bottom,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("katto")
        .icon(tauri::include_image!("icons/tray/menubar.png"))
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => window::toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    app.manage(TrayState { toggle, job });

    Ok(())
}

/// Reflect the main window's presence in the toggle menu item's label.
pub fn set_window_shown(app: &AppHandle, shown: bool) {
    if let Some(state) = app.try_state::<TrayState>() {
        let label = if shown { "Hide window" } else { "Show window" };
        let _ = state.toggle.set_text(label);
    }
}

/// Mirror the active job (label + percent) into the tray; `None` when idle.
/// MenuItem setters marshal to the main thread internally, so this is safe to
/// call from async tasks.
pub fn set_active_job(app: &AppHandle, label: Option<&str>) {
    if let Some(state) = app.try_state::<TrayState>() {
        let _ = state.job.set_text(label.unwrap_or("No active job"));
    }
}
