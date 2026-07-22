//! Thin command shells for the cut editor: debounced edit save, timeline
//! export with the sticky NLE target, MP4 render + thumbnail jobs, source
//! relocation, and the open/reveal actions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Channel;
use tauri_plugin_opener::OpenerExt;

use katto_engine::Rational;
use katto_engine::schema::manifest::ProjectManifest;

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

/// The manifest's source can only be swapped for the same recording: same
/// file name, duration within one frame. Pure; unit-tested.
fn relocation_matches(
    manifest: &ProjectManifest,
    probed_duration: Rational,
    new_path: &Path,
) -> Result<()> {
    let expected = manifest
        .source_video_absolute_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let got = new_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if expected != got {
        return Err(Error::Relocate(format!(
            "file name mismatch: expected {expected}, picked {got}"
        )));
    }
    let fps = manifest.frame_rate;
    let diff = manifest
        .duration
        .checked_sub(probed_duration)
        .ok_or_else(|| Error::Relocate("duration comparison overflowed".to_string()))?;
    // |diff| <= one frame (fps.den/fps.num seconds), cross-multiplied exactly.
    let abs = i128::from(diff.num).unsigned_abs() * u128::from(fps.num.unsigned_abs());
    let one_frame = u128::from(fps.den) * u128::from(diff.den);
    if abs > one_frame {
        return Err(Error::Relocate(format!(
            "duration mismatch: manifest {:.3}s, picked file {:.3}s",
            manifest.duration.to_secs_f64(),
            probed_duration.to_secs_f64()
        )));
    }
    Ok(())
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

/// Containment-check `bundle_path` against the studio root (blocking-safe).
async fn checked_bundle_path(state: &State<'_, AppState>, bundle_path: String) -> Result<PathBuf> {
    let root = studio_root(state).await?;
    tauri::async_runtime::spawn_blocking(move || {
        validate_bundle_path(&root, Path::new(&bundle_path))
    })
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
    let canonical = checked_bundle_path(&state, bundle_path).await?;
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
    let canonical = checked_bundle_path(&state, bundle_path).await?;
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
    let canonical = checked_bundle_path(&state, bundle_path).await?;

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
    out: Option<String>,
    on_progress: Channel<JobProgress>,
) -> Result<jobs_repo::Job> {
    let canonical = checked_bundle_path(&state, bundle_path).await?;
    let name = canonical
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".into());

    let out_path = match out {
        Some(out) => PathBuf::from(out),
        None => {
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
        }
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
    let canonical = checked_bundle_path(&state, bundle_path).await?;
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
    let canonical = checked_bundle_path(&state, bundle_path).await?;
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
        let manifest_path = canonical.join(katto_engine::bundle::PROJECT_JSON);
        let raw = std::fs::read_to_string(&manifest_path)
            .map_err(|e| Error::Io(format!("{}: {e}", manifest_path.display())))?;
        let mut manifest: ProjectManifest = serde_json::from_str(&raw)
            .map_err(|e| Error::Engine(format!("{}: {e}", manifest_path.display())))?;
        relocation_matches(&manifest, probed_duration, &new_path)?;
        manifest.source_video_absolute_path = new_path;
        katto_engine::bundle::write_json_atomic(&manifest_path, &manifest).map_err(Error::from)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn open_in_fcp(app: tauri::AppHandle, path: String) -> Result<bool> {
    let target = PathBuf::from(&path);
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
pub async fn reveal_timeline(app: tauri::AppHandle, path: String) -> Result<()> {
    app.opener()
        .reveal_item_in_dir(PathBuf::from(path))
        .map_err(|e| Error::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_25fps(name: &str) -> ProjectManifest {
        ProjectManifest {
            schema_version: 1,
            source_video_absolute_path: PathBuf::from("/media").join(name),
            frame_rate: Rational::new(25, 1),
            duration: Rational::new(250, 25),
        }
    }

    #[test]
    fn relocation_requires_same_filename_and_duration() {
        let m = manifest_25fps("clip.mp4");
        assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/clip.mp4")).is_ok());
        assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/other.mp4")).is_err());
        let off = m.duration.checked_add(Rational::new(2, 1)).unwrap();
        assert!(relocation_matches(&m, off, Path::new("/elsewhere/clip.mp4")).is_err());
    }

    #[test]
    fn relocation_tolerates_one_frame_of_duration_drift() {
        let m = manifest_25fps("clip.mp4");
        let one_frame_less = m.duration.checked_sub(Rational::new(1, 25)).unwrap();
        assert!(relocation_matches(&m, one_frame_less, Path::new("/e/clip.mp4")).is_ok());
        let two_frames_less = m.duration.checked_sub(Rational::new(2, 25)).unwrap();
        assert!(relocation_matches(&m, two_frames_less, Path::new("/e/clip.mp4")).is_err());
    }

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
