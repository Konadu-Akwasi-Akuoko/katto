use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use katto_engine::ingest::{FileEntry, VolumeTree, enumerate::enumerate, recognize::recognize};

use katto_engine::ingest::naming::plan_renames;

use crate::db::jobs::Job;
use crate::error::{Error, Result};
use crate::ingest::copy::{CopyPlan, run_copy_job};
use crate::state::{AppState, IngestState};

/// One clip in a card offer, as sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipDto {
    /// Source path relative to the volume root.
    pub path: String,
    /// File name.
    pub name: String,
    /// Byte size. Exported as `number`: real clip sizes never approach 2^53.
    #[specta(type = f64)]
    pub size: u64,
    /// Whether it is a video (importable) vs a sidecar.
    pub is_video: bool,
    /// Default selection state.
    pub selected: bool,
    /// Duration in seconds, if ffprobe succeeded.
    pub duration_s: Option<f64>,
}

/// A group of clips sharing card substructure.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipGroupDto {
    /// Group label (substructure dir name).
    pub label: String,
    /// Clips in the group.
    pub clips: Vec<ClipDto>,
}

/// The current detected card, offered to the import sheet.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CardOffer {
    /// Absolute mount path (`/Volumes/<NAME>`), also the eject target.
    pub volume: String,
    /// Recognized card kind, as a stable slug (`"sony"`/`"generic_dcim"`/`"iphone_dcim"`).
    pub kind: String,
    /// Total bytes of all video clips (for the free-space check).
    #[specta(type = f64)]
    pub total_bytes: u64,
    /// Grouped clips.
    pub groups: Vec<ClipGroupDto>,
}

/// Build a `CardOffer` from a walked volume: recognize, enumerate, and attach
/// ffprobe durations (best-effort; a probe failure leaves `duration_s = None`).
pub fn build_offer(mount: &Path, tree: &VolumeTree, files: &[FileEntry]) -> Option<CardOffer> {
    let card = recognize(tree)?;
    let kind = match card.kind {
        katto_engine::ingest::CardKind::Sony => "sony",
        katto_engine::ingest::CardKind::GenericDcim => "generic_dcim",
        katto_engine::ingest::CardKind::IphoneDcim => "iphone_dcim",
    };
    let under_roots: Vec<FileEntry> = files
        .iter()
        .filter(|f| card.clip_roots.iter().any(|r| f.path.starts_with(r)))
        .cloned()
        .collect();
    let groups = enumerate(card.kind, &under_roots);

    let mut total_bytes = 0u64;
    let groups: Vec<ClipGroupDto> = groups
        .into_iter()
        .map(|g| ClipGroupDto {
            label: g.label,
            clips: g
                .clips
                .into_iter()
                .map(|c| {
                    if c.is_video {
                        total_bytes += c.size;
                    }
                    let duration_s = if c.is_video {
                        crate::ffprobe::probe_clip(&mount.join(&c.path))
                            .ok()
                            .and_then(|m| m.duration_s)
                    } else {
                        None
                    };
                    ClipDto {
                        path: c.path.to_string_lossy().into_owned(),
                        name: c.name,
                        size: c.size,
                        is_video: c.is_video,
                        selected: c.selected,
                        duration_s,
                    }
                })
                .collect(),
        })
        .collect();

    Some(CardOffer {
        volume: mount.to_string_lossy().into_owned(),
        kind: kind.to_string(),
        total_bytes,
        groups,
    })
}

/// The current detected card offer, if any.
#[tauri::command]
#[specta::specta]
pub async fn card_offer(ingest: State<'_, IngestState>) -> Result<Option<CardOffer>> {
    Ok(ingest.current.lock().map(|g| g.clone()).unwrap_or(None))
}

/// Validate the card mount, project, and free space, then spawn the copy job.
#[tauri::command]
#[specta::specta]
pub async fn start_ingest(
    state: State<'_, AppState>,
    app: AppHandle,
    volume: String,
    project_slug: String,
    selected_paths: Vec<String>,
) -> Result<Job> {
    let sources: Vec<PathBuf> = selected_paths.into_iter().map(PathBuf::from).collect();
    plan_and_spawn(&state, &app, PathBuf::from(&volume), project_slug, sources).await
}

/// Manual drag-in path (iPhone footage): same rename+verify pipeline, absolute
/// source paths, no watcher/card involvement.
#[tauri::command]
#[specta::specta]
pub async fn import_files(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    paths: Vec<String>,
) -> Result<Job> {
    // Sources arrive absolute; strip the leading "/" so each is relative to
    // source_root = "/" and `Rename::source` carries the path components.
    let rels: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            let p = Path::new(p);
            p.strip_prefix("/").unwrap_or(p).to_path_buf()
        })
        .collect();
    plan_and_spawn(&state, &app, PathBuf::from("/"), project_slug, rels).await
}

/// Eject the card by its mount path. `diskutil eject` accepts a mount point.
#[tauri::command]
#[specta::specta]
pub async fn eject_card(volume: String) -> Result<()> {
    let output = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("diskutil")
            .arg("eject")
            .arg(&volume)
            .output()
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;
    if !output.status.success() {
        return Err(Error::Io(format!(
            "eject failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Shared spawn path for `start_ingest` (sources relative to the card mount)
/// and `import_files` (sources relative to `/`): resolve the project's
/// `footage/` dir, pick the naming date, plan renames off the highest existing
/// sequence, guard free space, and spawn the copy job.
async fn plan_and_spawn(
    state: &AppState,
    app: &AppHandle,
    source_root: PathBuf,
    project_slug: String,
    sources: Vec<PathBuf>,
) -> Result<Job> {
    let slug = project_slug.clone();
    let (footage_dir, existing, date): (PathBuf, Vec<String>, String) = state
        .db
        .call(move |conn| {
            crate::commands::projects::require_mounted(conn)?;
            let project = crate::db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Db(format!("no such project: {slug}")))?;
            let footage = PathBuf::from(&project.root_path).join("footage");
            let existing: Vec<String> = std::fs::read_dir(&footage)
                .map(|rd| {
                    rd.flatten()
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            let date = project
                .shoot_date
                .clone()
                .filter(|d| !d.is_empty())
                .map(Ok)
                .unwrap_or_else(|| {
                    conn.query_row("SELECT date('now','localtime')", [], |r| {
                        r.get::<_, String>(0)
                    })
                })?;
            Ok((footage, existing, date))
        })
        .await?;

    // Free-space guard on the studio drive before any copy begins.
    let needed: u64 = sources
        .iter()
        .map(|s| {
            std::fs::metadata(source_root.join(s))
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .sum();
    std::fs::create_dir_all(&footage_dir)?;
    let free = fs4::available_space(&footage_dir).unwrap_or(0);
    if free < needed {
        return Err(Error::Io(format!(
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

    let label = format!("Import {} clips", renames.len());
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
            run_copy_job(ctx, db, app2, plan).await
        })
        .await
}
