//! The cut-pipeline job future: import -> transcribe -> plan, streaming
//! [`PipelineEvent`]s over the command's `Channel` while the jobs framework
//! mirrors tray/dashboard state. The command layer
//! (`commands::pipeline::plan_rough_cut`) resolves keys and validates paths,
//! then hands a fully-resolved [`PipelineSpec`] here.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::AppHandle;
use tauri::ipc::Channel;

use katto_engine::planner::subprocess::{PartialObserver, SubprocessClaudePlanner};
use katto_engine::planner::{CutPlanner, PlanError, Planner, http::HttpAnthropicPlanner};
use katto_engine::schema::Cut;
use katto_engine::transcribe::{ELEVENLABS_BASE_URL, TranscribeConfig};

use crate::commands::pipeline::{FailureKind, PipelineEvent, StageName};
use crate::db::DbHandle;
use crate::jobs::JobContext;

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
pub struct PipelineSpec {
    pub project_slug: String,
    pub footage_path: PathBuf,
    pub audio_dir: PathBuf,
    pub elevenlabs_key: String,
    pub planner_kind: PlannerKind,
}

/// Which planner backend the run uses (resolved before the job row exists).
pub enum PlannerKind {
    /// Visible dock session writing cuts.json in the bundle (Phase 6 default).
    Dock {
        claude_path: PathBuf,
    },
    Subprocess {
        claude_path: PathBuf,
    },
    Http {
        api_key: String,
        model: String,
    },
}

/// Classify an engine failure for UI copy: "re-enter your key" (auth) is a
/// different owner action than "retry later" (quota) or "the model output
/// never validated" (invalid).
fn classify(error: &katto_engine::Error) -> FailureKind {
    match error {
        katto_engine::Error::TranscribeAuth(_) => FailureKind::Auth,
        katto_engine::Error::TranscribeQuota(_) => FailureKind::Quota,
        katto_engine::Error::Plan(PlanError::Auth(_)) => FailureKind::Auth,
        katto_engine::Error::Plan(PlanError::InvalidAfterRetry { .. }) => {
            FailureKind::InvalidOutput
        }
        _ => FailureKind::Other,
    }
}

fn fail(error: katto_engine::Error) -> (FailureKind, String) {
    (classify(&error), error.to_string())
}

/// Dock planning: a visible claude session (acceptEdits, cut-decider system
/// prompt) writes `cuts.json` in the bundle; katto polls and validates it,
/// pushes exactly one correction into the session on invalid output, and
/// fails the stage visibly on the second — never a silent fallback (D18).
/// On success the session is left open; it idles and the reaper closes it.
async fn plan_via_dock(
    app: &AppHandle,
    bundle_root: &std::path::Path,
    transcript: &katto_engine::schema::Transcript,
    claude_path: &std::path::Path,
    footage_path: &std::path::Path,
) -> std::result::Result<katto_engine::schema::Cuts, PlanError> {
    use tauri::Manager;

    use crate::sessions::planfile::{PlanFileVerdict, evaluate_plan_file};
    use crate::sessions::{Program, SessionTask};

    let _ = claude_path; // resolution already proved claude exists; spawn re-resolves
    let pool = app.state::<crate::state::AppState>().sessions.clone();
    let cuts_path = bundle_root.join(katto_engine::bundle::CUTS_JSON);
    let root = bundle_root.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || clear_previous_plan(&root))
        .await
        .map_err(|e| PlanError::Subprocess(e.to_string()))??;
    let stem = footage_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "footage".to_string());
    let task = SessionTask {
        label: format!("cut plan: {stem}"),
        cwd: bundle_root.to_path_buf(),
        initial_prompt: Some(
            "Read transcript.json in this directory and produce the rough-cut plan. \
             Write the result as cuts.json in this directory, exactly matching the \
             schema from your instructions — a single JSON object, no prose in the \
             file. Then stop."
                .to_string(),
        ),
        append_system_prompt: Some(katto_engine::planner::CUT_DECIDER_PROMPT.to_string()),
        permission_mode: Some("acceptEdits".to_string()),
        permission_allow: vec![],
    };
    let session_id = pool
        .spawn(app, task, Program::Claude)
        .await
        .map_err(|e| PlanError::Subprocess(e.to_string()))?;

    let mut corrected = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10 * 60);
    loop {
        if std::time::Instant::now() >= deadline {
            return Err(PlanError::Subprocess(
                "dock planning timed out after 10 minutes; the session is still open in the dock"
                    .to_string(),
            ));
        }
        let read = {
            let path = cuts_path.clone();
            tauri::async_runtime::spawn_blocking(move || std::fs::read_to_string(path))
                .await
                .map_err(|e| PlanError::Subprocess(e.to_string()))?
        };
        match evaluate_plan_file(read, transcript) {
            PlanFileVerdict::Valid(cuts) => return Ok(cuts),
            PlanFileVerdict::Invalid { errors_message } => {
                if corrected {
                    return Err(PlanError::InvalidAfterRetry {
                        error: errors_message,
                        raw: String::new(),
                    });
                }
                corrected = true;
                // Keep the bad attempt as an audit artifact, then push the
                // correction INTO the visible session (retry contract: once).
                // The invalid file MUST leave cuts.json's path: if it survived,
                // the next poll would re-read it and fail as a second attempt
                // the session never made.
                if let Err(rename_err) =
                    std::fs::rename(&cuts_path, bundle_root.join("cuts.invalid-1.json"))
                {
                    std::fs::remove_file(&cuts_path).map_err(|remove_err| {
                        PlanError::Subprocess(format!(
                            "could not clear invalid cuts.json before the correction \
                             (rename: {rename_err}; remove: {remove_err})"
                        ))
                    })?;
                }
                let correction = format!(
                    "the cuts.json you wrote was invalid: {errors_message}; rewrite \
                     cuts.json as a single valid JSON object matching the schema, then stop."
                );
                pool.write(&session_id, correction.as_bytes())
                    .and_then(|()| pool.write(&session_id, b"\r"))
                    .map_err(|e| {
                        PlanError::Subprocess(format!(
                            "could not push the correction into the dock session: {e}"
                        ))
                    })?;
            }
            PlanFileVerdict::Missing => {
                // Fail fast on a dead session on ANY turn (a first-turn-only
                // stop watch would sleep through a death during the correction
                // turn and burn the whole timeout).
                if let Some(error) = pool.terminal_error(&session_id) {
                    return Err(PlanError::Subprocess(format!(
                        "dock session ended before writing cuts.json: {error}"
                    )));
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Move a previous run's `cuts.json` aside (kept as `cuts.prev.json`) and
/// confirm it is gone. A surviving stale file would satisfy the dock-planning
/// poll with LAST run's plan before the session writes a byte, so failure
/// here fails the run (surfaced through the job's Failed event + events row).
fn clear_previous_plan(bundle_root: &std::path::Path) -> std::result::Result<(), PlanError> {
    let cuts_path = bundle_root.join(katto_engine::bundle::CUTS_JSON);
    if cuts_path.exists() {
        std::fs::rename(&cuts_path, bundle_root.join("cuts.prev.json")).map_err(|e| {
            PlanError::Subprocess(format!(
                "could not move the previous cuts.json aside before planning: {e}"
            ))
        })?;
    }
    if cuts_path.exists() {
        return Err(PlanError::Subprocess(
            "previous cuts.json is still present after moving it aside".to_string(),
        ));
    }
    Ok(())
}

/// The job future: each stage streams a `PipelineEvent` and ticks the jobs
/// framework's progress so the tray/dashboard mirror. Errors surface as a
/// best-effort `Failed` event (with a UI-facing kind) plus the returned
/// message (the framework writes the terminal events row).
pub async fn run_pipeline_job(
    ctx: JobContext,
    db: DbHandle,
    app: AppHandle,
    spec: PipelineSpec,
    on_event: Channel<PipelineEvent>,
) -> std::result::Result<(), String> {
    match run_pipeline_inner(&ctx, &db, &app, &spec, &on_event).await {
        Ok(()) => Ok(()),
        Err((kind, message)) => {
            let _ = on_event.send(PipelineEvent::Failed {
                error: message.clone(),
                kind,
            });
            Err(message)
        }
    }
}

async fn run_pipeline_inner(
    ctx: &JobContext,
    db: &DbHandle,
    app: &AppHandle,
    spec: &PipelineSpec,
    on_event: &Channel<PipelineEvent>,
) -> std::result::Result<(), (FailureKind, String)> {
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
        .map_err(fail)?;
    let bundle_root = outcome.bundle_root.clone();
    let bundle_path = bundle_root.to_string_lossy().into_owned();

    // Stage 2: transcribe, with a heartbeat nudging progress while we await.
    // A valid existing transcript.json short-circuits inside the engine.
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
                result = &mut transcribe => break result.map_err(fail)?,
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
    let cuts = match &spec.planner_kind {
        PlannerKind::Dock { claude_path } => {
            // Visible dock session; a failure here fails the stage (D18) —
            // there is no silent fallback to the subprocess path.
            plan_via_dock(
                app,
                &bundle_root,
                &transcript,
                claude_path,
                &spec.footage_path,
            )
            .await
            .map_err(|e| fail(katto_engine::Error::Plan(e)))?
        }
        PlannerKind::Subprocess { claude_path } => {
            let planner = Planner::Subprocess(
                SubprocessClaudePlanner::new(claude_path.clone(), bundle_root.clone())
                    .with_observer(observer),
            );
            planner
                .plan(&transcript)
                .await
                .map_err(|e| fail(katto_engine::Error::Plan(e)))?
        }
        PlannerKind::Http { api_key, model } => {
            let planner = Planner::Http(HttpAnthropicPlanner {
                api_key: api_key.clone(),
                model: model.clone(),
                base_url: katto_engine::planner::http::ANTHROPIC_BASE_URL.to_string(),
            });
            planner
                .plan(&transcript)
                .await
                .map_err(|e| fail(katto_engine::Error::Plan(e)))?
        }
    };
    katto_engine::bundle::write_json_atomic(
        &bundle_root.join(katto_engine::bundle::CUTS_JSON),
        &cuts,
    )
    .map_err(fail)?;

    // Domain event + broadcast (the jobs framework writes job_done separately).
    let event_payload = serde_json::json!({
        "bundle": bundle_path,
        "cuts": cuts.cuts.len(),
        "flags": cuts.flags.len(),
    })
    .to_string();
    let slug = spec.project_slug.clone();
    if let Err(err) = db
        .call(move |conn| {
            crate::db::events::record(conn, "rough_cut_planned", Some(&slug), Some(&event_payload))
        })
        .await
    {
        // The pipeline succeeded; only the domain event write failed. The jobs
        // framework still records job_done, so log rather than fail the run.
        eprintln!("rough_cut_planned event write failed: {err}");
    }
    crate::broadcast::events_appended(app);

    ctx.progress(1.0, Some("Rough cut ready".to_string())).await;
    send(PipelineEvent::Done { bundle_path });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_failures_classify_for_ui_copy() {
        assert!(matches!(
            classify(&katto_engine::Error::TranscribeAuth("x".into())),
            FailureKind::Auth
        ));
        assert!(matches!(
            classify(&katto_engine::Error::TranscribeQuota("x".into())),
            FailureKind::Quota
        ));
        assert!(matches!(
            classify(&katto_engine::Error::Plan(PlanError::Auth("x".into()))),
            FailureKind::Auth
        ));
        assert!(matches!(
            classify(&katto_engine::Error::Plan(PlanError::InvalidAfterRetry {
                error: "e".into(),
                raw: "r".into()
            })),
            FailureKind::InvalidOutput
        ));
        assert!(matches!(
            classify(&katto_engine::Error::Io("disk".into())),
            FailureKind::Other
        ));
    }

    #[test]
    fn clear_previous_plan_moves_stale_cuts_aside() {
        let dir = tempfile::tempdir().unwrap();
        let cuts = dir.path().join(katto_engine::bundle::CUTS_JSON);
        std::fs::write(&cuts, "{\"stale\":true}").unwrap();

        clear_previous_plan(dir.path()).unwrap();

        assert!(!cuts.exists(), "stale cuts.json must be gone");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("cuts.prev.json")).unwrap(),
            "{\"stale\":true}",
            "the previous plan is kept as an audit artifact"
        );
    }

    #[test]
    fn clear_previous_plan_is_a_noop_without_a_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        clear_previous_plan(dir.path()).unwrap();
        assert!(!dir.path().join("cuts.prev.json").exists());
    }
}
