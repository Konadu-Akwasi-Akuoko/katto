//! The studio.db import command: dry-run answers synchronously with the
//! mapped preview; apply runs as a `studio_import` jobs row (D18) and
//! delivers the final report over the `StudioImportFinished` broadcast.

use rusqlite::OpenFlags;
use tauri::{AppHandle, Manager, State};

use crate::error::{Error, Result};
use crate::import_studio::{self, ImportReport};
use crate::state::AppState;

/// What the command returned: the dry-run preview, or the spawned job.
#[derive(serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportOutcome {
    Preview { report: ImportReport },
    Started { job_id: String },
}

/// Expand a leading `~/` against $HOME — the wizard's default path ships
/// with the literal tilde.
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return std::path::Path::new(&home)
            .join(rest)
            .to_string_lossy()
            .into_owned();
    }
    path.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn import_studio_db(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    dry_run: bool,
) -> Result<ImportOutcome> {
    let source_path = expand_home(&path);
    let (ideas, warnings) = tauri::async_runtime::spawn_blocking(move || {
        let conn =
            rusqlite::Connection::open_with_flags(&source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| Error::ImportFailed(format!("couldn't read {source_path}: {e}")))?;
        import_studio::read_source(&conn)
            .map_err(|e| Error::ImportFailed(format!("couldn't read {source_path}: {e}")))
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;

    if dry_run {
        let preview_ideas = ideas.clone();
        let report = state
            .db
            .call(move |conn| import_studio::preview(conn, &preview_ideas))
            .await?;
        return Ok(ImportOutcome::Preview {
            report: ImportReport { warnings, ..report },
        });
    }

    let job_app = app.clone();
    let job = state
        .jobs
        .spawn(
            "studio_import",
            "Import ideas — studio.db",
            Some(serde_json::json!({ "path": path }).to_string()),
            move |_ctx| run_apply(job_app, ideas, warnings),
        )
        .await?;
    Ok(ImportOutcome::Started { job_id: job.id })
}

async fn run_apply(
    app: AppHandle,
    ideas: Vec<crate::db::ideas::Idea>,
    warnings: Vec<String>,
) -> std::result::Result<(), String> {
    // the wizard's "Importing…" state resolves only via broadcast — a
    // failure must broadcast too, or it wedges forever
    let result = run_apply_inner(&app, ideas, warnings).await;
    if let Err(message) = &result {
        crate::broadcast::studio_import_failed(&app, message);
    }
    result
}

async fn run_apply_inner(
    app: &AppHandle,
    ideas: Vec<crate::db::ideas::Idea>,
    warnings: Vec<String>,
) -> std::result::Result<(), String> {
    let state = app.state::<AppState>();
    let mut report = state
        .db
        .call(move |conn| import_studio::apply(conn, &ideas))
        .await
        .map_err(|e| e.to_string())?;
    report.warnings = warnings;

    let payload = serde_json::json!({
        "imported": report.imported,
        "updated": report.updated,
        "skipped": report.skipped,
    })
    .to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let _ = state
        .db
        .call(move |conn| {
            crate::db::settings::set(conn, "studio_import_last_run", &now)?;
            crate::db::events::record(conn, "studio_imported", None, Some(&payload))
        })
        .await;
    crate::broadcast::events_appended(app);
    crate::broadcast::ideas_changed(app);
    crate::broadcast::studio_import_finished(app, report);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_home_only_touches_leading_tilde_slash() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_home("~/x/y.db"), format!("{home}/x/y.db"));
        assert_eq!(expand_home("/abs/path.db"), "/abs/path.db");
        assert_eq!(expand_home("relative.db"), "relative.db");
    }
}
