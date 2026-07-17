use tauri::{State, ipc::Channel};

use crate::db::jobs::{self, Job};
use crate::error::{Error, Result};
use crate::jobs::{ChannelSink, JobProgress};
use crate::state::AppState;

/// List jobs newest-first. When `active_only`, restrict to `queued`/`running`.
#[tauri::command]
#[specta::specta]
pub async fn list_jobs(state: State<'_, AppState>, active_only: bool) -> Result<Vec<Job>> {
    state
        .db
        .call(move |conn| jobs::list(conn, active_only))
        .await
}

/// Stream a job's progress over an IPC channel. Sends the current snapshot
/// first (a reopened WebView subscribes late), then live ticks until the job
/// reaches a terminal state.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_job_progress(
    state: State<'_, AppState>,
    job_id: String,
    on_progress: Channel<JobProgress>,
) -> Result<()> {
    let job = {
        let id = job_id.clone();
        state.db.call(move |conn| jobs::get(conn, &id)).await?
    }
    .ok_or_else(|| Error::Db(format!("no such job: {job_id}")))?;

    let _ = on_progress.send(JobProgress {
        job_id: job.id.clone(),
        progress: job.progress,
        message: None,
    });
    if job.status == "queued" || job.status == "running" {
        state
            .jobs
            .hub()
            .subscribe(&job.id, Box::new(ChannelSink(on_progress)));
    }
    Ok(())
}

/// Dev helper: a 5-second synthetic job exercising the full pipeline
/// (progress stream, tray mirror, terminal events row). `fail` makes it die
/// halfway to exercise the failure path.
#[tauri::command]
#[specta::specta]
pub async fn dev_run_smoke_job(state: State<'_, AppState>, fail: bool) -> Result<Job> {
    state
        .jobs
        .spawn("smoke", "Smoke test", None, move |ctx| async move {
            for step in 1..=20u32 {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                if fail && step == 10 {
                    return Err("smoke job asked to fail".to_string());
                }
                ctx.progress(f64::from(step) / 20.0, Some(format!("step {step}/20")))
                    .await;
            }
            Ok(())
        })
        .await
}
