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
    Subprocess { claude_path: PathBuf },
    Http { api_key: String, model: String },
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
    let cuts = planner
        .plan(&transcript)
        .await
        .map_err(|e| fail(katto_engine::Error::Plan(e)))?;
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
}
