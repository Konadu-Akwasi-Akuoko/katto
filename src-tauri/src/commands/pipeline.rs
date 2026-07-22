//! The cut-pipeline job (`plan_rough_cut`) plus bundle read commands. One job
//! per run: import -> transcribe -> plan, streaming [`PipelineEvent`]s over
//! the command's `Channel` while the jobs framework mirrors tray/dashboard
//! state. Keys are resolved before the job spawns — no key, no job row.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use katto_engine::planner::subprocess::{PartialObserver, SubprocessClaudePlanner};
use katto_engine::planner::{CutPlanner, Planner, http::HttpAnthropicPlanner};
use katto_engine::schema::{Cut, Cuts, Edits, Transcript};
use katto_engine::transcribe::{ELEVENLABS_BASE_URL, TranscribeConfig};

use crate::db::{DbHandle, jobs as jobs_repo, settings as settings_repo};
use crate::error::{Error, Result};
use crate::keychain::{self, KeyService};
use crate::state::AppState;

/// One pipeline step, as the step indicator renders it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum StageName {
    ExtractingAudio,
    Transcribing,
    DetectingCuts,
}

/// Streamed pipeline progress for the footage-card step indicator.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PipelineEvent {
    /// A stage began (progress is 0..1 within the whole run).
    Stage { name: StageName, progress: f64 },
    /// transcript.json landed — the review surface can open early.
    TranscriptReady { bundle_path: String },
    /// Cuts parsed so far while the subprocess planner streams.
    CutsPartial { cuts_so_far: Vec<Cut> },
    /// The run finished; cuts.json is on disk.
    Done { bundle_path: String },
    /// The run failed; the jobs framework records the terminal state.
    Failed { error: String },
}

/// One `.kruproj` row in the project's bundle list.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BundleSummary {
    pub path: String,
    pub name: String,
    pub has_transcript: bool,
    pub has_cuts: bool,
}

/// The full bundle payload for the editor — one `open_bundle` call, JSON only
/// (media plays via the asset protocol).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BundleData {
    pub root: String,
    pub source_path: String,
    pub frame_rate: katto_engine::Rational,
    /// UI projection (`to_secs_f64`) — display only, never cut math.
    pub duration_secs: f64,
    pub transcript: Transcript,
    pub cuts: Option<Cuts>,
    pub edits: Option<Edits>,
}

/// `footage_path` must resolve inside `<project>/footage/` (same path-validation
/// stance as ingest). Returns the canonical path.
fn validate_footage_path(project_dir: &Path, footage_path: &Path) -> Result<PathBuf> {
    let footage_root = project_dir.join("footage");
    let canonical_root = footage_root
        .canonicalize()
        .map_err(|e| Error::Io(format!("footage dir {}: {e}", footage_root.display())))?;
    let canonical = footage_path
        .canonicalize()
        .map_err(|e| Error::Io(format!("footage clip {}: {e}", footage_path.display())))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(Error::IngestInvalid(format!(
            "clip {} is outside the project footage folder",
            footage_path.display()
        )));
    }
    Ok(canonical)
}

/// Stat a bundle directory into its list row; `None` when it has no manifest.
fn bundle_summary(root: &Path) -> Option<BundleSummary> {
    if !root.join(katto_engine::bundle::PROJECT_JSON).exists() {
        return None;
    }
    Some(BundleSummary {
        path: root.to_string_lossy().into_owned(),
        name: root
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        has_transcript: root.join(katto_engine::bundle::TRANSCRIPT_JSON).exists(),
        has_cuts: root.join(katto_engine::bundle::CUTS_JSON).exists(),
    })
}

/// One video file in the project's footage folder.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct FootageClip {
    pub path: String,
    pub name: String,
}

/// Video extensions the pipeline accepts (mirrors the ingest drop filter).
const VIDEO_EXTS: [&str; 4] = ["mp4", "mov", "mts", "m4v"];

/// Scan a footage directory into sorted clip rows. Missing dir -> empty.
fn footage_clips(footage_dir: &Path) -> Vec<FootageClip> {
    let Ok(entries) = std::fs::read_dir(footage_dir) else {
        return Vec::new();
    };
    let mut clips: Vec<FootageClip> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
        })
        .map(|p| FootageClip {
            name: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: p.to_string_lossy().into_owned(),
        })
        .collect();
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    clips
}

#[tauri::command]
#[specta::specta]
pub async fn list_footage(
    state: State<'_, AppState>,
    project_slug: String,
) -> Result<Vec<FootageClip>> {
    let footage_dir: PathBuf = state
        .db
        .call(move |conn| {
            let project = crate::db::projects::get(conn, &project_slug)?
                .ok_or_else(|| Error::NoSuchProject(format!("no such project: {project_slug}")))?;
            Ok(PathBuf::from(project.root_path).join("footage"))
        })
        .await?;
    tauri::async_runtime::spawn_blocking(move || Ok(footage_clips(&footage_dir)))
        .await
        .map_err(|e| Error::Io(e.to_string()))?
}

/// Streams `CutsPartial` while the subprocess planner emits.
struct ChannelObserver(Channel<PipelineEvent>);

impl PartialObserver for ChannelObserver {
    fn on_cuts(&self, cuts: &[Cut]) {
        let _ = self.0.send(PipelineEvent::CutsPartial {
            cuts_so_far: cuts.to_vec(),
        });
    }
}

/// Everything the job future needs, resolved before spawn.
struct PipelineSpec {
    project_slug: String,
    footage_path: PathBuf,
    audio_dir: PathBuf,
    elevenlabs_key: String,
    planner_kind: PlannerKind,
}

enum PlannerKind {
    Subprocess { claude_path: PathBuf },
    Http { api_key: String, model: String },
}

#[tauri::command]
#[specta::specta]
pub async fn plan_rough_cut(
    app: AppHandle,
    state: State<'_, AppState>,
    project_slug: String,
    footage_path: String,
    on_event: Channel<PipelineEvent>,
) -> Result<jobs_repo::Job> {
    // Resolve project + validate the clip path.
    let slug = project_slug.clone();
    let project_root: PathBuf = state
        .db
        .call(move |conn| {
            let project = crate::db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::NoSuchProject(format!("no such project: {slug}")))?;
            Ok(PathBuf::from(project.root_path))
        })
        .await?;
    let clip = PathBuf::from(&footage_path);
    let root_for_check = project_root.clone();
    let footage_canonical =
        tauri::async_runtime::spawn_blocking(move || validate_footage_path(&root_for_check, &clip))
            .await
            .map_err(|e| Error::Io(e.to_string()))??;

    // Resolve keys and planner BEFORE spawning: no key, no job row.
    let elevenlabs_key =
        tauri::async_runtime::spawn_blocking(|| keychain::read_key(KeyService::Elevenlabs))
            .await
            .map_err(|e| Error::Io(e.to_string()))??
            .ok_or_else(|| {
                Error::MissingKey(
                    "no ElevenLabs API key stored — add it in Settings to transcribe footage"
                        .to_string(),
                )
            })?;

    let (claude_setting, planner_model): (Option<String>, String) = state
        .db
        .call(|conn| {
            Ok((
                settings_repo::get(conn, "claude_path")?,
                settings_repo::get(conn, "planner_model")?
                    .unwrap_or_else(|| katto_engine::planner::http::DEFAULT_MODEL.to_string()),
            ))
        })
        .await?;
    let planner_kind = resolve_planner(claude_setting, planner_model).await?;

    let file_name = footage_canonical
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| footage_path.clone());
    let audio_dir = project_root.join("audio");
    let bundle_path = audio_dir.join(format!(
        "{}.kruproj",
        footage_canonical
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let payload = serde_json::json!({
        "project_slug": project_slug,
        "footage_path": footage_canonical,
        "bundle_path": bundle_path,
    })
    .to_string();

    let spec = PipelineSpec {
        project_slug: project_slug.clone(),
        footage_path: footage_canonical,
        audio_dir,
        elevenlabs_key,
        planner_kind,
    };
    let db = state.db.clone();
    state
        .jobs
        .spawn(
            "cut_pipeline",
            &format!("Rough cut — {file_name}"),
            Some(payload),
            move |ctx| async move { run_pipeline_job(ctx, db, app, spec, on_event).await },
        )
        .await
}

/// Settings `claude_path` (or a fresh login-shell detect) -> subprocess; else a
/// stored Anthropic key -> http; both missing -> typed error naming the fix.
async fn resolve_planner(
    claude_setting: Option<String>,
    planner_model: String,
) -> Result<PlannerKind> {
    let detected = tauri::async_runtime::spawn_blocking(move || {
        claude_setting
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                let out = std::process::Command::new("zsh")
                    .args(["-lc", "which claude"])
                    .output()
                    .ok()?;
                if !out.status.success() {
                    return None;
                }
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (!path.is_empty()).then(|| PathBuf::from(path))
            })
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?;
    if let Some(claude_path) = detected {
        return Ok(PlannerKind::Subprocess { claude_path });
    }
    let anthropic =
        tauri::async_runtime::spawn_blocking(|| keychain::read_key(KeyService::Anthropic))
            .await
            .map_err(|e| Error::Io(e.to_string()))??;
    match anthropic {
        Some(api_key) => Ok(PlannerKind::Http {
            api_key,
            model: planner_model,
        }),
        None => Err(Error::NoPlanner(
            "no planner available — install the claude CLI or store an Anthropic API key in Settings"
                .to_string(),
        )),
    }
}

/// The job future: each stage streams a `PipelineEvent` and ticks the jobs
/// framework's progress so the tray/dashboard mirror. Errors surface as a
/// best-effort `Failed` event plus the returned message (the framework writes
/// the terminal events row).
async fn run_pipeline_job(
    ctx: crate::jobs::JobContext,
    db: DbHandle,
    app: AppHandle,
    spec: PipelineSpec,
    on_event: Channel<PipelineEvent>,
) -> std::result::Result<(), String> {
    match run_pipeline_inner(&ctx, &db, &app, &spec, &on_event).await {
        Ok(()) => Ok(()),
        Err(message) => {
            let _ = on_event.send(PipelineEvent::Failed {
                error: message.clone(),
            });
            Err(message)
        }
    }
}

async fn run_pipeline_inner(
    ctx: &crate::jobs::JobContext,
    db: &DbHandle,
    app: &AppHandle,
    spec: &PipelineSpec,
    on_event: &Channel<PipelineEvent>,
) -> std::result::Result<(), String> {
    let send = |e: PipelineEvent| {
        let _ = on_event.send(e);
    };

    // Stage 1: import (probe + audio extraction).
    send(PipelineEvent::Stage {
        name: StageName::ExtractingAudio,
        progress: 0.0,
    });
    ctx.progress(0.02, Some("Extracting audio".to_string()))
        .await;
    let outcome = katto_engine::import::import(&spec.footage_path, &spec.audio_dir)
        .await
        .map_err(|e| e.to_string())?;
    let bundle_root = outcome.bundle_root.clone();
    let bundle_path = bundle_root.to_string_lossy().into_owned();

    // Stage 2: transcribe, with a heartbeat nudging progress while we await.
    send(PipelineEvent::Stage {
        name: StageName::Transcribing,
        progress: 0.33,
    });
    ctx.progress(0.33, Some("Transcribing".to_string())).await;
    let cfg = TranscribeConfig {
        api_key: spec.elevenlabs_key.clone(),
        base_url: ELEVENLABS_BASE_URL.to_string(),
    };
    let transcript = {
        let transcribe = katto_engine::transcribe::transcribe_into_bundle(&cfg, &bundle_root);
        let mut transcribe = std::pin::pin!(transcribe);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        let mut nudge = 0.33f64;
        loop {
            tokio::select! {
                result = &mut transcribe => break result.map_err(|e| e.to_string())?,
                _ = interval.tick() => {
                    nudge = (nudge + 0.02).min(0.6);
                    ctx.progress(nudge, Some("Transcribing".to_string())).await;
                }
            }
        }
    };
    send(PipelineEvent::TranscriptReady {
        bundle_path: bundle_path.clone(),
    });

    // Stage 3: plan cuts.
    send(PipelineEvent::Stage {
        name: StageName::DetectingCuts,
        progress: 0.66,
    });
    ctx.progress(0.66, Some("Detecting cuts".to_string())).await;
    let observer = Arc::new(ChannelObserver(on_event.clone()));
    let planner = match &spec.planner_kind {
        PlannerKind::Subprocess { claude_path } => Planner::Subprocess(
            SubprocessClaudePlanner::new(claude_path.clone(), bundle_root.clone())
                .with_observer(observer),
        ),
        PlannerKind::Http { api_key, model } => Planner::Http(HttpAnthropicPlanner {
            api_key: api_key.clone(),
            model: model.clone(),
            base_url: katto_engine::planner::http::ANTHROPIC_BASE_URL.to_string(),
        }),
    };
    let cuts = planner.plan(&transcript).await.map_err(|e| e.to_string())?;
    katto_engine::bundle::write_json_atomic(
        &bundle_root.join(katto_engine::bundle::CUTS_JSON),
        &cuts,
    )
    .map_err(|e| e.to_string())?;

    // Domain event + broadcast (the jobs framework writes job_done separately).
    let event_payload = serde_json::json!({
        "bundle": bundle_path,
        "cuts": cuts.cuts.len(),
        "flags": cuts.flags.len(),
    })
    .to_string();
    let slug = spec.project_slug.clone();
    let _ = db
        .call(move |conn| {
            crate::db::events::record(conn, "rough_cut_planned", Some(&slug), Some(&event_payload))
        })
        .await;
    crate::broadcast::events_appended(app);

    ctx.progress(1.0, Some("Rough cut ready".to_string())).await;
    send(PipelineEvent::Done { bundle_path });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn open_bundle(state: State<'_, AppState>, path: String) -> Result<BundleData> {
    let _ = &state; // bundle data comes from the filesystem; folders are truth
    let bundle =
        tauri::async_runtime::spawn_blocking(move || katto_engine::bundle::open(Path::new(&path)))
            .await
            .map_err(|e| Error::Io(e.to_string()))?
            .map_err(Error::from)?;

    let transcript = bundle
        .transcript
        .ok_or_else(|| Error::Engine("bundle has no transcript yet".to_string()))?;
    Ok(BundleData {
        root: bundle.root.to_string_lossy().into_owned(),
        source_path: bundle
            .manifest
            .source_video_absolute_path
            .to_string_lossy()
            .into_owned(),
        frame_rate: bundle.manifest.frame_rate,
        duration_secs: bundle.manifest.duration.to_secs_f64(),
        transcript,
        cuts: bundle.cuts,
        edits: bundle.edits,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn list_bundles(
    state: State<'_, AppState>,
    project_slug: String,
) -> Result<Vec<BundleSummary>> {
    let audio_dir: PathBuf = state
        .db
        .call(move |conn| {
            let project = crate::db::projects::get(conn, &project_slug)?
                .ok_or_else(|| Error::NoSuchProject(format!("no such project: {project_slug}")))?;
            Ok(PathBuf::from(project.root_path).join("audio"))
        })
        .await?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut bundles = Vec::new();
        let entries = match std::fs::read_dir(&audio_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(bundles), // no audio dir yet — empty list, not an error
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("kruproj")
                && let Some(summary) = bundle_summary(&path)
            {
                bundles.push(summary);
            }
        }
        bundles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(bundles)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footage_path_must_live_under_project_footage() {
        let dir = tempfile::tempdir().unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        let clip = footage.join("2026-07-22_001.mp4");
        std::fs::write(&clip, b"x").unwrap();
        assert!(validate_footage_path(dir.path(), &clip).is_ok());
        assert!(validate_footage_path(dir.path(), Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn footage_path_rejects_escape_via_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        let outside = dir.path().join("outside.mp4");
        std::fs::write(&outside, b"x").unwrap();
        let link = footage.join("sneaky.mp4");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(validate_footage_path(dir.path(), &link).is_err());
    }

    #[test]
    fn bundle_summary_from_dir_reads_artifact_presence() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("project.json"), b"{}").unwrap();
        std::fs::write(root.join("transcript.json"), b"{}").unwrap();
        let s = bundle_summary(&root).unwrap();
        assert_eq!(s.name, "clip");
        assert!(s.has_transcript);
        assert!(!s.has_cuts);
    }

    #[test]
    fn footage_clips_filters_and_sorts_videos() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.MP4"), b"x").unwrap();
        std::fs::write(dir.path().join("a.mov"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let clips = footage_clips(dir.path());
        assert_eq!(
            clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["a.mov", "b.MP4"]
        );
        assert!(footage_clips(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn bundle_summary_requires_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        assert!(bundle_summary(&root).is_none());
    }
}
