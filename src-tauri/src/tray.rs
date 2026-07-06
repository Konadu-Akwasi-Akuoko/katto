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
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Hide window", true, None::<&str>)?;
    let project = MenuItem::with_id(app, "project", "No project", false, None::<&str>)?;
    let shoot = MenuItem::with_id(app, "shoot", "No shoot scheduled", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle, &project, &shoot, &separator, &quit])?;

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

    app.manage(TrayState { toggle });

    Ok(())
}

/// Reflect the main window's presence in the toggle menu item's label.
pub fn set_window_shown(app: &AppHandle, shown: bool) {
    if let Some(state) = app.try_state::<TrayState>() {
        let label = if shown { "Hide window" } else { "Show window" };
        let _ = state.toggle.set_text(label);
    }
}
