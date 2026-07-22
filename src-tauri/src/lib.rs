mod assets;
pub mod broadcast;
pub mod browser;
pub mod capture;
pub mod commands;
pub mod curation;
pub mod db;
pub mod drive;
pub mod error;
pub mod ffprobe;
pub mod ingest;
pub mod jobs;
pub mod keychain;
pub mod notify;
pub mod paths;
pub mod projects;
pub mod scheduler;
pub mod sessions;
mod state;
mod tray;
pub mod vfx;
mod volumes;
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
            commands::settings::set_capture_shortcut,
            commands::drive::get_drive_status,
            commands::events::list_events,
            commands::jobs::list_jobs,
            commands::jobs::subscribe_job_progress,
            commands::jobs::dev_run_smoke_job,
            commands::pipeline::plan_rough_cut,
            commands::pipeline::open_bundle,
            commands::pipeline::list_bundles,
            commands::pipeline::list_footage,
            commands::editor::save_edits,
            commands::editor::preview_export,
            commands::editor::export_timeline,
            commands::editor::render_mp4,
            commands::editor::generate_thumbs,
            commands::editor::relocate_source,
            commands::editor::pick_relocation_file,
            commands::editor::open_in_fcp,
            commands::editor::reveal_timeline,
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
            commands::projects::set_project_priority,
            commands::projects::set_project_dates,
            commands::projects::reveal_project_folder,
            commands::projects::trash_project,
            commands::ingest::card_offer,
            commands::ingest::start_ingest,
            commands::ingest::eject_card,
            commands::ingest::import_files,
            commands::ideas::list_ideas,
            commands::ideas::create_idea,
            commands::ideas::update_idea,
            commands::ideas::discard_idea,
            commands::ideas::promote_idea,
            commands::ideas::capture_submit,
            commands::scheduler::get_scheduler_state,
            commands::scheduler::run_scheduled_job_now,
            commands::scheduler::set_scheduled_job,
            commands::sessions::spawn_session,
            commands::sessions::attach_session,
            commands::sessions::detach_session,
            commands::sessions::write_session,
            commands::sessions::resize_session,
            commands::sessions::close_session,
            commands::sessions::list_sessions,
            commands::sessions::set_dock_focus,
            commands::schedule::list_schedule,
            commands::schedule::upsert_schedule_entry,
            commands::schedule::delete_schedule_entry,
            commands::vfx::create_vfx_effect,
            commands::vfx::list_vfx_effects,
            commands::browser::browser_open_tab,
            commands::browser::browser_close_tab,
            commands::browser::browser_select_tab,
            commands::browser::browser_navigate,
            commands::browser::browser_go,
            commands::browser::browser_state,
            commands::browser::browser_set_bounds,
            commands::browser::browser_set_visible,
            commands::browser::set_active_asset_project,
            commands::browser::active_asset_project,
            commands::browser::parked_downloads,
            commands::browser::file_parked_download,
            commands::browser::reveal_in_project,
            commands::shell::set_autostart,
            commands::shell::get_autostart,
            commands::shell::sleep_to_tray,
            commands::shell::quit_app,
            commands::shell::open_external_url,
        ])
        .events(collect_events![
            broadcast::EventsAppended,
            broadcast::JobsChanged,
            broadcast::DriveStatusChanged,
            broadcast::ProjectsChanged,
            broadcast::IdeasChanged,
            broadcast::ScheduleChanged,
            broadcast::DeepLinkOpened,
            broadcast::CardDetected,
            broadcast::CardRemoved,
            broadcast::SessionsChanged,
            broadcast::SessionStateChanged,
            broadcast::VfxRenderLanded,
            broadcast::BrowserStateChanged,
            broadcast::DownloadNeedsProject,
            broadcast::DownloadFiled,
            broadcast::DownloadFallback,
            broadcast::DownloadFailed,
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
    let single_webview = tauri::async_runtime::block_on(
        db.call(|conn| db::settings::get(conn, "browser_single_webview")),
    )
    .ok()
    .flatten()
    .is_some_and(|v| v == "true");
    let browser: std::sync::Arc<dyn browser::host::BrowserTabHost> = if single_webview {
        std::sync::Arc::new(browser::host::SingleWebviewHost::new())
    } else {
        std::sync::Arc::new(browser::host::MultiWebviewHost::new())
    };
    Ok(state::AppState {
        db,
        jobs,
        sessions: sessions::pool::SessionPool::new(),
        active_exports: std::sync::Arc::default(),
        browser,
        downloads: browser::host::DownloadRegistry::default(),
        active_asset_project: std::sync::Mutex::new(None),
    })
}

/// Fail over jobs rows stranded queued/running by a crash — otherwise the
/// per-bundle busy guards would refuse work forever. Each interrupted job is
/// marked failed and gets an events row; a DB failure here is logged, never
/// fatal to startup.
fn fail_interrupted_jobs(app: &tauri::App) {
    let db = app.state::<state::AppState>().db.clone();
    let result = tauri::async_runtime::block_on(db.call(|conn| {
        let interrupted = db::jobs::fail_interrupted(conn)?;
        for job in &interrupted {
            let payload = serde_json::json!({ "id": job.id, "kind": job.kind }).to_string();
            db::events::record(conn, "job_interrupted", Some(&job.label), Some(&payload))?;
        }
        Ok(interrupted.len())
    }));
    match result {
        Ok(0) => {}
        Ok(n) => eprintln!("failed over {n} interrupted job(s) from the previous run"),
        Err(err) => eprintln!("interrupted-jobs failover failed: {err}"),
    }
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

/// Wire `katto://` deep links: register the scheme (Windows/Linux only — macOS
/// registers it from the bundled `Info.plist`, so this is a no-op there and dev
/// builds never receive OS opens) and forward every recognized open to the
/// frontend router as a `DeepLinkOpened` broadcast. Unparseable urls are dropped.
fn setup_deep_links(app: &tauri::AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;

    #[cfg(any(windows, target_os = "linux"))]
    {
        let _ = app.deep_link().register_all();
    }

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            if let Some(route) = notify::parse_deep_link(url.as_str()) {
                broadcast::deep_link_opened(&handle, &route.as_wire());
            }
        }
    });
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
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            keychain::init()?;
            app.manage(bootstrap_state(app)?);
            app.manage(state::IngestState::default());
            assets::grant_at_launch(app);
            fail_interrupted_jobs(app);
            launch_reconcile(app);

            let handle = app.handle();
            tray::create(handle)?;
            notify::init(handle);
            tray::refresh_planner_lines(handle);
            window::setup(handle)?;
            capture::setup(handle);
            setup_deep_links(handle);
            tauri::async_runtime::spawn(drive::watch(handle.clone()));
            volumes::start_watcher(handle.clone());
            app.state::<state::AppState>()
                .sessions
                .start(handle.clone())?;
            app.manage(scheduler::runtime::start(handle.clone()));
            vfx::start_watch(handle.clone());
            browser::host::sweep_staging(handle);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { .. } => {
                window::on_close_requested(window);
            }
            tauri::WindowEvent::Destroyed if window.label() == window::MAIN => {
                let state: tauri::State<state::AppState> = window.app_handle().state();
                state.browser.on_window_destroyed();
            }
            _ => {}
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
