//! Resolve commands: availability gate for the export dialog's button and
//! the import job. Preflights answer synchronously with typed remedies; the
//! script run happens inside a jobs row.

use tauri::{AppHandle, Manager, State};

use crate::error::{Error, Result};
use crate::resolve::script::ScriptOutcome;
use crate::state::AppState;

/// Resolve is installed — gates the "Open in Resolve" button.
#[tauri::command]
#[specta::specta]
pub async fn resolve_available() -> Result<bool> {
    Ok(crate::resolve::resolve_installed())
}

const NOT_INSTALLED: &str = "DaVinci Resolve is not installed at /Applications/DaVinci Resolve/";
const NOT_RUNNING: &str =
    "DaVinci Resolve Studio isn't running. Open it first — katto never launches it.";
const SCRIPTING_UNAVAILABLE: &str = "Resolve is running but refused the connection. Enable \
    Preferences → System → General → External scripting using: Local. Note: DaVinci Resolve \
    Studio is required — the free edition does not support external scripting.";

#[tauri::command]
#[specta::specta]
pub async fn open_in_resolve(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    timeline_version: Option<u32>,
) -> Result<()> {
    // preflights spawn pgrep and stat the bundle — off the runtime thread
    let (installed, running) = tauri::async_runtime::spawn_blocking(|| {
        (
            crate::resolve::resolve_installed(),
            crate::resolve::resolve_running(),
        )
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?;
    if !installed {
        return Err(Error::ResolveNotInstalled(NOT_INSTALLED.to_string()));
    }
    if !running {
        return Err(Error::ResolveNotRunning(NOT_RUNNING.to_string()));
    }

    let lookup = slug.clone();
    let project = state
        .db
        .call(move |conn| crate::db::projects::get(conn, &lookup))
        .await?
        .ok_or_else(|| Error::NoSuchProject(format!("no project {slug}")))?;
    let timelines_dir = std::path::Path::new(&project.root_path).join("timelines");

    let version = match timeline_version {
        Some(version) => version,
        None => {
            let scan_dir = timelines_dir.clone();
            let scan_slug = slug.clone();
            let next = tauri::async_runtime::spawn_blocking(move || {
                katto_engine::timelines::next_version(&scan_dir, &scan_slug)
            })
            .await
            .map_err(|e| Error::Io(e.to_string()))??;
            if next <= 1 {
                return Err(Error::ResolveFailed(
                    "no exported timeline — export to FCPXML first".to_string(),
                ));
            }
            next - 1
        }
    };
    let fcpxml = timelines_dir.join(format!("{slug}-v{version}.fcpxml"));
    if !fcpxml.is_file() {
        return Err(Error::ResolveFailed(format!(
            "no exported timeline at {}",
            fcpxml.display()
        )));
    }

    let job_app = app.clone();
    let job_slug = slug.clone();
    let project_name = format!("{slug} v{version}");
    state
        .jobs
        .spawn(
            "resolve_import",
            &format!("Resolve — {slug}"),
            Some(serde_json::json!({ "slug": slug, "version": version }).to_string()),
            move |_ctx| run_resolve_job(job_app, job_slug, version, project_name, fcpxml),
        )
        .await?;
    Ok(())
}

async fn run_resolve_job(
    app: AppHandle,
    slug: String,
    version: u32,
    project_name: String,
    fcpxml: std::path::PathBuf,
) -> std::result::Result<(), String> {
    let outcome = crate::resolve::run_import(&project_name, &fcpxml)
        .await
        .map_err(|e| e.to_string())?;
    match outcome {
        ScriptOutcome::Ok { project } => {
            let state = app.state::<AppState>();
            let payload = serde_json::json!({
                "slug": slug,
                "version": version,
                "project": project,
            })
            .to_string();
            let event_slug = slug.clone();
            let _ = state
                .db
                .call(move |conn| {
                    crate::db::events::record(
                        conn,
                        "resolve_imported",
                        Some(&event_slug),
                        Some(&payload),
                    )
                })
                .await;
            crate::broadcast::events_appended(&app);
            Ok(())
        }
        ScriptOutcome::NotConnected => Err(SCRIPTING_UNAVAILABLE.to_string()),
        ScriptOutcome::CreateFailed(detail) => {
            Err(format!("Resolve couldn't create the project: {detail}"))
        }
        ScriptOutcome::ImportFailed(detail) => {
            Err(format!("Resolve couldn't import the timeline: {detail}"))
        }
        ScriptOutcome::Garbled(text) => Err(format!("Resolve answered unexpectedly: {text}")),
    }
}
