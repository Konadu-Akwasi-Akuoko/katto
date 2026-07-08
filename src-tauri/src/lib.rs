pub mod commands;
pub mod db;
pub mod error;
pub mod keychain;
pub mod paths;
mod state;
mod tray;
mod window;

use tauri::Manager;
use tauri_specta::{Builder, collect_commands};

/// The tauri-specta command registry. Shared by [`run`] and the bindings-export
/// test so the generated TypeScript can never drift from the wired handlers.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::settings::get_settings,
        commands::settings::set_settings,
        commands::events::list_events,
        commands::jobs::list_jobs,
        commands::onboarding::pick_studio_root,
        commands::onboarding::store_key,
        commands::onboarding::key_present,
        commands::onboarding::detect_claude,
        commands::onboarding::complete_onboarding,
    ])
}

/// Open the database and assemble the managed [`AppState`]: resolve the app data
/// dir, spawn the single-writer DB at `katto.db`, and record the `app_started`
/// event so the boot is visible in the activity log ("nothing fails silently").
fn bootstrap_state(app: &tauri::App) -> Result<state::AppState, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db = db::DbHandle::spawn(dir.join("katto.db"))?;
    tauri::async_runtime::block_on(
        db.call(|conn| db::events::record(conn, "app_started", None, None)),
    )?;
    Ok(state::AppState { db })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = window::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            keychain::init()?;
            app.manage(bootstrap_state(app)?);

            let handle = app.handle();
            tray::create(handle)?;
            window::setup(handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window::on_close_requested(window);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => window::on_exit_requested(app, &api),
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                let _ = window::show_main(app);
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    /// Regenerate the TypeScript bindings from the command registry. Runs as part
    /// of `just check`; commit the resulting `bindings.gen.ts` alongside the
    /// command change (see the add-tauri-command skill).
    #[test]
    fn export_bindings() {
        use specta_typescript::Typescript;
        use specta_typescript::semantic::Configuration;

        // Job progress is an `f64` in `0.0..=1.0`, never NaN/Infinity, so flatten
        // its export from `number | null` to `number`. (Row ids cross as `RowId`,
        // which already exports as `number`.)
        let config = Configuration::empty().enable_lossless_floats();
        super::specta_builder()
            .semantic_types(config)
            .export(Typescript::default(), "../src/lib/ipc/bindings.gen.ts")
            .expect("failed to export typescript bindings");
    }
}
