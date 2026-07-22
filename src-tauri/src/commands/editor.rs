//! Thin command shells for the cut editor: debounced edit save, timeline
//! export with the sticky NLE target, MP4 render + thumbnail jobs, source
//! relocation, and the open/reveal actions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Channel;
use tauri_plugin_opener::OpenerExt;

use crate::commands::pipeline::validate_bundle_path;
use crate::db::{jobs as jobs_repo, settings as settings_repo};
use crate::error::{Error, Result};
use crate::jobs::JobProgress;
use crate::state::AppState;

/// Which NLE the export opens in. Stored in settings as its snake_case
/// string; only Final Cut has an open action this phase — Resolve/Premiere
/// export identically and fall back to reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NleTarget {
    FinalCut,
    Resolve,
    Premiere,
}

impl NleTarget {
    /// The settings-table string for this target (the wire form).
    fn as_setting(self) -> &'static str {
        match self {
            NleTarget::FinalCut => "final_cut",
            NleTarget::Resolve => "resolve",
            NleTarget::Premiere => "premiere",
        }
    }

    /// Parse a stored settings value; unknown strings read as no default.
    fn from_setting(s: &str) -> Option<NleTarget> {
        match s {
            "final_cut" => Some(NleTarget::FinalCut),
            "resolve" => Some(NleTarget::Resolve),
            "premiere" => Some(NleTarget::Premiere),
            _ => None,
        }
    }
}

/// What the export dialog shows before exporting.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ExportPreview {
    pub slug: String,
    pub version: u32,
    pub default_nle: Option<NleTarget>,
}

/// The written artifacts of one export plus what happened after.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ExportResult {
    pub fcpxml_path: String,
    pub srt_path: String,
    pub vtt_path: String,
    pub version: u32,
    pub opened_in_nle: bool,
    pub revealed: bool,
}

/// The configured studio root, or a typed onboarding error.
async fn studio_root(state: &State<'_, AppState>) -> Result<PathBuf> {
    state
        .db
        .call(|conn| {
            settings_repo::get(conn, "studio_root")?
                .map(PathBuf::from)
                .ok_or_else(|| Error::Onboarding("no studio root configured".to_string()))
        })
        .await
}

/// Containment-check any frontend-supplied path against the studio root
/// (blocking-safe). Used for bundle roots and timeline artifacts alike — no
/// path from the webview reaches the filesystem without it.
async fn checked_studio_path(state: &State<'_, AppState>, path: String) -> Result<PathBuf> {
    let root = studio_root(state).await?;
    tauri::async_runtime::spawn_blocking(move || validate_bundle_path(&root, Path::new(&path)))
        .await
        .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn save_edits(
    state: State<'_, AppState>,
    bundle_path: String,
    edits: katto_engine::schema::Edits,
) -> Result<()> {
    let canonical = checked_studio_path(&state, bundle_path).await?;
    tauri::async_runtime::spawn_blocking(move || {
        katto_engine::bundle::save_edits(&canonical, &edits).map_err(Error::from)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn preview_export(
    state: State<'_, AppState>,
    bundle_path: String,
) -> Result<ExportPreview> {
    let canonical = checked_studio_path(&state, bundle_path).await?;
    let default_nle = state
        .db
        .call(|conn| settings_repo::get(conn, "default_nle"))
        .await?
        .and_then(|s| NleTarget::from_setting(&s));
    tauri::async_runtime::spawn_blocking(move || {
        let (timelines_dir, slug) = katto_engine::timelines::project_context(&canonical);
        let version = katto_engine::timelines::next_version(&timelines_dir, &slug);
        Ok(ExportPreview {
            slug,
            version,
            default_nle,
        })
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn export_timeline(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    bundle_path: String,
    nle_target: NleTarget,
    open_after: bool,
) -> Result<ExportResult> {
    let canonical = checked_studio_path(&state, bundle_path).await?;

    let (paths, slug) = tauri::async_runtime::spawn_blocking(move || {
        // Checked open: a moved source surfaces as the typed SourceMissing
        // before anything is emitted.
        let bundle = katto_engine::bundle::open(&canonical)?;
        let (timelines_dir, slug) = katto_engine::timelines::project_context(&canonical);
        let paths = katto_engine::timelines::export_timeline(&bundle, &timelines_dir, &slug)?;
        Ok::<_, katto_engine::Error>((paths, slug))
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;

    // Sticky last-used NLE target + the export events row.
    let setting = nle_target.as_setting();
    let payload = serde_json::json!({
        "slug": slug,
        "version": paths.version,
        "fcpxml": paths.fcpxml,
    })
    .to_string();
    let slug_for_event = slug.clone();
    state
        .db
        .call(move |conn| {
            settings_repo::set(conn, "default_nle", setting)?;
            crate::db::events::record(
                conn,
                "timeline_exported",
                Some(&slug_for_event),
                Some(&payload),
            )
        })
        .await?;
    crate::broadcast::events_appended(&app);

    let mut result = ExportResult {
        fcpxml_path: paths.fcpxml.to_string_lossy().into_owned(),
        srt_path: paths.srt.to_string_lossy().into_owned(),
        vtt_path: paths.vtt.to_string_lossy().into_owned(),
        version: paths.version,
        opened_in_nle: false,
        revealed: false,
    };
    if open_after {
        if nle_target == NleTarget::FinalCut && open_in_fcp_blocking(&paths.fcpxml).await {
            result.opened_in_nle = true;
        } else {
            result.revealed = app.opener().reveal_item_in_dir(&paths.fcpxml).is_ok();
        }
    }
    Ok(result)
}

/// `open -a "Final Cut Pro" <path>` (PRD-literal); false when FCP is missing.
async fn open_in_fcp_blocking(path: &Path) -> bool {
    let path = path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("open")
            .args(["-a", "Final Cut Pro"])
            .arg(&path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub async fn render_mp4(
    state: State<'_, AppState>,
    bundle_path: String,
    on_progress: Channel<JobProgress>,
) -> Result<jobs_repo::Job> {
    let canonical = checked_studio_path(&state, bundle_path).await?;
    let name = canonical
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".into());

    // The output path is always allocated here, never supplied by the
    // frontend — a webview-chosen path could escape the studio root or
    // clobber an existing render. One-run-per-bundle guard: the version is
    // claimed before the file exists, so a concurrent render of the same
    // bundle would land on the same -vN.
    let bundle_needle = canonical.to_string_lossy().into_owned();
    let busy = state
        .db
        .call(move |conn| jobs_repo::active_with_payload(conn, "render_mp4", &bundle_needle))
        .await?;
    if busy {
        return Err(Error::PipelineBusy(format!(
            "a render for {name} is already in progress"
        )));
    }

    let out_path = {
        let target = canonical.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let (timelines_dir, slug) = katto_engine::timelines::project_context(&target);
            let exports = timelines_dir.with_file_name("exports");
            std::fs::create_dir_all(&exports)?;
            let render_slug = format!("{slug}-render");
            let version = katto_engine::timelines::next_version(&exports, &render_slug);
            Ok::<_, Error>(exports.join(format!("{render_slug}-v{version}.mp4")))
        })
        .await
        .map_err(|e| Error::Io(e.to_string()))??
    };

    let payload = serde_json::json!({
        "bundle_path": canonical,
        "out": out_path,
    })
    .to_string();
    state
        .jobs
        .spawn(
            "render_mp4",
            &format!("Render — {name}"),
            Some(payload),
            move |ctx| async move {
                let bundle = {
                    let root = canonical.clone();
                    tauri::async_runtime::spawn_blocking(move || katto_engine::bundle::open(&root))
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?
                };
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<f64>();
                let job_id = ctx.job_id().to_string();
                let consumer = tauri::async_runtime::spawn(async move {
                    while let Some(p) = rx.recv().await {
                        ctx.progress(p, None).await;
                        let _ = on_progress.send(JobProgress {
                            job_id: job_id.clone(),
                            progress: p,
                            message: None,
                        });
                    }
                });
                let callback = move |p: f64| {
                    let _ = tx.send(p);
                };
                let outcome = katto_engine::render::render_mp4(&bundle, &out_path, &callback).await;
                drop(callback);
                let _ = consumer.await;
                outcome.map_err(|e| e.to_string())
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn generate_thumbs(
    state: State<'_, AppState>,
    bundle_path: String,
    on_progress: Channel<JobProgress>,
) -> Result<jobs_repo::Job> {
    let canonical = checked_studio_path(&state, bundle_path).await?;
    let name = canonical
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".into());
    let payload = serde_json::json!({ "bundle_path": canonical }).to_string();
    state
        .jobs
        .spawn(
            "generate_thumbs",
            &format!("Thumbnails — {name}"),
            Some(payload),
            move |ctx| async move {
                let source = {
                    let root = canonical.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        katto_engine::bundle::open(&root)
                            .map(|b| b.manifest.source_video_absolute_path)
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?
                };
                ctx.progress(0.5, Some("extracting thumbnails".to_string()))
                    .await;
                let _ = on_progress.send(JobProgress {
                    job_id: ctx.job_id().to_string(),
                    progress: 0.5,
                    message: None,
                });
                let count = katto_engine::thumbs::generate_thumbs(&canonical, &source)
                    .await
                    .map_err(|e| e.to_string())?;
                ctx.progress(1.0, Some(format!("{count} thumbnails"))).await;
                let _ = on_progress.send(JobProgress {
                    job_id: ctx.job_id().to_string(),
                    progress: 1.0,
                    message: Some(format!("{count} thumbnails")),
                });
                Ok(())
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn relocate_source(
    state: State<'_, AppState>,
    bundle_path: String,
    new_path: String,
) -> Result<()> {
    let canonical = checked_studio_path(&state, bundle_path).await?;
    let new_path = PathBuf::from(new_path);

    // Probe the picked file's exact timing with the engine's pinned argv.
    let argv = katto_engine::import::ffprobe_argv(&new_path);
    let output = tokio::process::Command::new("ffprobe")
        .args(&argv)
        .output()
        .await
        .map_err(|e| Error::Io(format!("ffprobe: {e}")))?;
    if !output.status.success() {
        return Err(Error::Relocate(format!(
            "could not probe {}: {}",
            new_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let timing =
        katto_engine::ffprobe::parse_probe_timing(&String::from_utf8_lossy(&output.stdout))
            .map_err(Error::from)?;
    let probed_duration = timing
        .duration
        .ok_or_else(|| Error::Relocate("picked file has no video duration".to_string()))?;

    tauri::async_runtime::spawn_blocking(move || {
        katto_engine::bundle::apply_relocation(&canonical, probed_duration, new_path)
            .map_err(Error::from)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

/// Native open-file picker for relocation, filtered to the missing file's
/// extension. `None` when the user cancels (same pattern as
/// `pick_studio_root`).
#[tauri::command]
#[specta::specta]
pub async fn pick_relocation_file(
    app: tauri::AppHandle,
    filename: String,
) -> Result<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let ext = Path::new(&filename)
            .extension()
            .map(|e| e.to_string_lossy().into_owned());
        let mut dialog = app.dialog().file();
        if let Some(ext) = &ext {
            dialog = dialog.add_filter(filename.clone(), &[ext]);
        }
        Ok(dialog
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn open_in_fcp(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<bool> {
    let target = checked_studio_path(&state, path).await?;
    if open_in_fcp_blocking(&target).await {
        return Ok(true);
    }
    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(false)
}

#[tauri::command]
#[specta::specta]
pub async fn reveal_timeline(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<()> {
    let target = checked_studio_path(&state, path).await?;
    app.opener()
        .reveal_item_in_dir(target)
        .map_err(|e| Error::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nle_target_round_trips_snake_case() {
        assert_eq!(
            serde_json::to_string(&NleTarget::FinalCut).unwrap(),
            "\"final_cut\""
        );
        assert_eq!(
            serde_json::from_str::<NleTarget>("\"final_cut\"").unwrap(),
            NleTarget::FinalCut
        );
        assert_eq!(
            NleTarget::from_setting(NleTarget::Resolve.as_setting()),
            Some(NleTarget::Resolve)
        );
        assert_eq!(NleTarget::from_setting("garbage"), None);
    }
}
