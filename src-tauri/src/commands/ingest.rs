use std::path::PathBuf;

use tauri::{AppHandle, State};

use katto_engine::ingest::naming::plan_renames;

use crate::db::jobs::Job;
use crate::error::{Error, Result};
use crate::ingest::copy::{CopyPlan, run_copy_job};
use crate::ingest::offer::CardOffer;
use crate::ingest::validate;
use crate::state::{AppState, IngestState};

/// The current detected card offer, if any.
#[tauri::command]
#[specta::specta]
pub async fn card_offer(ingest: State<'_, IngestState>) -> Result<Option<CardOffer>> {
    Ok(ingest.current.lock().map(|g| g.clone()).unwrap_or(None))
}

/// Validate the request against the live card offer (volume must match, every
/// source must be an offered clip path resolving under the mount), then plan
/// and spawn the copy job.
#[tauri::command]
#[specta::specta]
pub async fn start_ingest(
    state: State<'_, AppState>,
    ingest: State<'_, IngestState>,
    app: AppHandle,
    volume: String,
    project_slug: String,
    selected_paths: Vec<String>,
) -> Result<Job> {
    let offer = ingest
        .current
        .lock()
        .map(|g| g.clone())
        .unwrap_or(None)
        .ok_or_else(|| Error::IngestInvalid("no camera card is currently offered".to_string()))?;
    if offer.volume != volume {
        return Err(Error::IngestInvalid(format!(
            "volume does not match the offered card: {volume}"
        )));
    }

    let sources = validate::relative_sources(&selected_paths)?;
    let offered: std::collections::HashSet<&str> = offer
        .groups
        .iter()
        .flat_map(|g| g.clips.iter())
        .map(|c| c.path.as_str())
        .collect();
    for raw in &selected_paths {
        if !offered.contains(raw.as_str()) {
            return Err(Error::IngestInvalid(format!(
                "path is not on the offered card: {raw}"
            )));
        }
    }

    // Symlink guard: every source must canonicalize strictly under the mount.
    let mount = PathBuf::from(&volume);
    let check_mount = mount.clone();
    let check_sources = sources.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        for rel in &check_sources {
            validate::require_under_root(&check_mount, rel)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;

    plan_and_spawn(&state, &app, mount, project_slug, sources).await
}

/// Manual drag-in path (iPhone footage): same rename+verify pipeline, absolute
/// video-file sources, no watcher/card involvement.
#[tauri::command]
#[specta::specta]
pub async fn import_files(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    paths: Vec<String>,
) -> Result<Job> {
    let raw = paths.clone();
    let validated =
        tauri::async_runtime::spawn_blocking(move || validate::absolute_video_sources(&raw))
            .await
            .map_err(|e| Error::Io(e.to_string()))??;

    // Strip the leading "/" so each source is relative to source_root = "/".
    let rels: Vec<PathBuf> = validated
        .iter()
        .map(|p| p.strip_prefix("/").unwrap_or(p).to_path_buf())
        .collect();
    plan_and_spawn(&state, &app, PathBuf::from("/"), project_slug, rels).await
}

/// Eject the offered card. Refuses any volume other than the current offer's
/// mount, so this can never eject the studio drive or an arbitrary path.
#[tauri::command]
#[specta::specta]
pub async fn eject_card(ingest: State<'_, IngestState>, volume: String) -> Result<()> {
    let offered = ingest
        .current
        .lock()
        .map(|g| g.as_ref().map(|o| o.volume.clone()))
        .unwrap_or(None);
    if offered.as_deref() != Some(volume.as_str()) {
        return Err(Error::IngestInvalid(format!(
            "not the offered card volume: {volume}"
        )));
    }
    let output = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("diskutil")
            .arg("eject")
            .arg(&volume)
            .output()
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;
    if !output.status.success() {
        return Err(Error::EjectFailed(format!(
            "eject failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Shared spawn path for `start_ingest` (sources relative to the card mount)
/// and `import_files` (sources relative to `/`): serialize on the project's
/// footage dir, resolve the naming date, plan renames off the highest existing
/// sequence, guard free space, and spawn the copy job (which holds the
/// serialization guard until it ends).
async fn plan_and_spawn(
    state: &AppState,
    app: &AppHandle,
    source_root: PathBuf,
    project_slug: String,
    sources: Vec<PathBuf>,
) -> Result<Job> {
    if sources.is_empty() {
        return Err(Error::IngestInvalid("no clips selected".to_string()));
    }

    let slug = project_slug.clone();
    let (footage_dir, date): (PathBuf, String) = state
        .db
        .call(move |conn| {
            crate::commands::projects::require_mounted(conn)?;
            let project = crate::db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::NoSuchProject(format!("no such project: {slug}")))?;
            let footage = PathBuf::from(&project.root_path).join("footage");
            let date = project
                .shoot_date
                .clone()
                .filter(|d| !d.is_empty())
                .map(Ok)
                .unwrap_or_else(|| crate::db::local_date_today(conn))?;
            Ok((footage, date))
        })
        .await?;

    // Serialize plan+copy per footage dir: held from the existing-sequence
    // read until the spawned job finishes, so a concurrent import cannot plan
    // the same sequence numbers.
    let footage_guard = crate::ingest::lock_footage_dir(&footage_dir).await;

    // Filesystem prep off both the DB writer and the async runtime: existing
    // footage names (for the sequence), the byte total, and the free space.
    let fs_footage = footage_dir.clone();
    let fs_root = source_root.clone();
    let fs_sources = sources.clone();
    let (existing, needed, free): (Vec<String>, u64, u64) =
        tauri::async_runtime::spawn_blocking(move || -> Result<(Vec<String>, u64, u64)> {
            let existing: Vec<String> = std::fs::read_dir(&fs_footage)
                .map(|rd| {
                    rd.flatten()
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            let mut needed = 0u64;
            for s in &fs_sources {
                let src = fs_root.join(s);
                needed += std::fs::metadata(&src)
                    .map_err(|e| Error::Io(format!("cannot read source {}: {e}", src.display())))?
                    .len();
            }
            std::fs::create_dir_all(&fs_footage)?;
            let free = fs4::available_space(&fs_footage)?;
            Ok((existing, needed, free))
        })
        .await
        .map_err(|e| Error::Io(e.to_string()))??;
    if free < needed {
        return Err(Error::InsufficientSpace(format!(
            "insufficient free space: need {needed} bytes, {free} free"
        )));
    }

    // Plan renames: (path, lowercased ext) in stable order.
    let mut typed: Vec<(PathBuf, String)> = sources
        .into_iter()
        .map(|p| {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            (p, ext)
        })
        .collect();
    typed.sort_by(|a, b| a.0.cmp(&b.0));
    let renames = plan_renames(&date, &existing, &typed);

    let count = renames.len();
    let label = format!("Import {count} clip{}", if count == 1 { "" } else { "s" });
    let plan = CopyPlan {
        source_root,
        footage_dir,
        renames,
        project_slug,
    };
    let db = state.db.clone();
    let app2 = app.clone();
    state
        .jobs
        .spawn("ingest", &label, None, move |ctx| async move {
            let _footage_guard = footage_guard;
            run_copy_job(ctx, db, app2, plan).await
        })
        .await
}
