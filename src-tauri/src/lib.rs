pub mod broadcast;
pub mod capture;
pub mod commands;
pub mod db;
pub mod drive;
pub mod error;
pub mod jobs;
pub mod keychain;
pub mod paths;
pub mod projects;
mod state;
mod tray;
mod window;

use tauri::Manager;
use tauri_specta::{Builder, collect_commands, collect_events};

/// The tauri-specta command registry. Shared by [`run`] and the bindings-export
/// test so the generated TypeScript can never drift from the wired handlers.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::drive::get_drive_status,
            commands::events::list_events,
            commands::jobs::list_jobs,
            commands::jobs::subscribe_job_progress,
            commands::jobs::dev_run_smoke_job,
            commands::onboarding::pick_studio_root,
            commands::onboarding::store_key,
            commands::onboarding::key_present,
            commands::onboarding::detect_claude,
            commands::onboarding::complete_onboarding,
            commands::projects::rescan_projects,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::set_project_status,
            commands::projects::set_project_dates,
            commands::projects::reveal_project_folder,
            commands::ideas::list_ideas,
            commands::ideas::create_idea,
            commands::ideas::update_idea,
            commands::ideas::discard_idea,
            commands::ideas::promote_idea,
            commands::ideas::capture_submit,
            commands::schedule::list_schedule,
            commands::schedule::upsert_schedule_entry,
            commands::schedule::delete_schedule_entry,
            commands::shell::set_autostart,
            commands::shell::get_autostart,
            commands::shell::sleep_to_tray,
            commands::shell::quit_app,
        ])
        .events(collect_events![
            broadcast::EventsAppended,
            broadcast::JobsChanged,
            broadcast::DriveStatusChanged,
            broadcast::ProjectsChanged,
            broadcast::IdeasChanged,
            broadcast::ScheduleChanged,
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
    let jobs = jobs::JobRuntime::new(app.handle().clone(), db.clone());
    Ok(state::AppState { db, jobs })
}

/// Reconcile the projects index against the studio-root folders at launch —
/// folders are truth. Skips (and records a `reconcile_skipped_unmounted` event)
/// when the root is configured but unreachable; does nothing pre-onboarding. Any
/// scan/DB failure is swallowed so a bad reconcile never blocks app startup.
fn launch_reconcile(app: &tauri::App) {
    let db = app.state::<state::AppState>().db.clone();
    let outcome = tauri::async_runtime::block_on(db.call(|conn| {
        match db::settings::get(conn, "studio_root")? {
            Some(root) if paths::root_mounted(std::path::Path::new(&root)) => {
                projects::reconcile::reconcile_root(conn, &root).map(Some)
            }
            Some(_) => {
                db::events::record(conn, "reconcile_skipped_unmounted", None, None)?;
                Ok(None)
            }
            None => Ok(None),
        }
    }));
    if let Err(err) = outcome {
        eprintln!("launch reconcile failed: {err}");
    }
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::AppleScript,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            keychain::init()?;
            app.manage(bootstrap_state(app)?);
            launch_reconcile(app);

            let handle = app.handle();
            tray::create(handle)?;
            tray::refresh_planner_lines(handle);
            window::setup(handle)?;
            capture::setup(handle);
            tauri::async_runtime::spawn(drive::watch(handle.clone()));
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
