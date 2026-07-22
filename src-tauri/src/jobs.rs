pub mod download;
pub mod hub;
pub mod pipeline;

use std::sync::Arc;

use tauri::AppHandle;

pub use hub::{JobProgress, ProgressHub, ProgressSink};

use crate::broadcast;
use crate::db::{DbHandle, jobs as jobs_repo};
use crate::error::Result;
use crate::tray;

/// Executes background work as jobs: DB row + state machine, progress fan-out
/// over the hub, tray mirror, and a terminal `events` row — nothing fails
/// silently.
#[derive(Clone)]
pub struct JobRuntime {
    app: AppHandle,
    db: DbHandle,
    hub: Arc<ProgressHub>,
}

/// Handed to a job's work future for reporting progress.
pub struct JobContext {
    runtime: JobRuntime,
    job_id: String,
    label: String,
}

impl JobContext {
    /// The id of the job this context reports for (e.g. to correlate a
    /// terminal events row with the job row).
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Persist and fan out a progress tick (`value` in `0.0..=1.0`) and mirror
    /// it to the tray. Best-effort: a lost tick never fails the job.
    pub async fn progress(&self, value: f64, message: Option<String>) {
        let id = self.job_id.clone();
        let _ = self
            .runtime
            .db
            .call(move |conn| jobs_repo::set_progress(conn, &id, value))
            .await;
        let update = JobProgress {
            job_id: self.job_id.clone(),
            progress: value,
            message,
        };
        self.runtime.hub.publish(&update);
        tray::set_active_job(
            &self.runtime.app,
            Some(&format!("{} — {:.0}%", self.label, value * 100.0)),
        );
    }
}

impl JobRuntime {
    pub fn new(app: AppHandle, db: DbHandle) -> Self {
        JobRuntime {
            app,
            db,
            hub: Arc::new(ProgressHub::default()),
        }
    }

    pub fn hub(&self) -> &ProgressHub {
        &self.hub
    }

    /// Create the job row and run `work` on the async runtime. Returns the
    /// queued job immediately; progress streams out-of-band and the terminal
    /// state lands in the `events` log.
    pub async fn spawn<F, Fut>(
        &self,
        kind: &str,
        label: &str,
        payload_json: Option<String>,
        work: F,
    ) -> Result<jobs_repo::Job>
    where
        F: FnOnce(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = std::result::Result<(), String>> + Send + 'static,
    {
        let id = uuid::Uuid::new_v4().to_string();
        let job = {
            let id = id.clone();
            let kind = kind.to_owned();
            let label = label.to_owned();
            self.db
                .call(move |conn| {
                    jobs_repo::create(conn, &id, &kind, &label, payload_json.as_deref())
                })
                .await?
        };
        broadcast::jobs_changed(&self.app);

        let runtime = self.clone();
        let spawned = job.clone();
        tauri::async_runtime::spawn(async move {
            runtime.run(spawned, work).await;
        });
        Ok(job)
    }

    async fn run<F, Fut>(self, job: jobs_repo::Job, work: F)
    where
        F: FnOnce(JobContext) -> Fut + Send + 'static,
        Fut: Future<Output = std::result::Result<(), String>> + Send + 'static,
    {
        let id = job.id.clone();
        let started = {
            let id = id.clone();
            self.db.call(move |conn| jobs_repo::start(conn, &id)).await
        };
        if let Err(err) = started {
            // The queued -> running transition failed, so the work never runs.
            // "Nothing fails silently" dominates the happy path: fail the row
            // (queued -> failed) and surface a terminal events row.
            self.finalize(&job, Err(format!("failed to start: {err}")))
                .await;
            return;
        }
        broadcast::jobs_changed(&self.app);
        tray::set_active_job(&self.app, Some(&format!("{} — 0%", job.label)));

        let ctx = JobContext {
            runtime: self.clone(),
            job_id: id.clone(),
            label: job.label.clone(),
        };
        // Isolate the work future on its own task so a panic unwinds there,
        // not through `run`. A join failure (panic/cancel) becomes a normal
        // job failure — the terminal path always runs.
        let outcome = match tauri::async_runtime::spawn(work(ctx)).await {
            Ok(outcome) => outcome,
            Err(_) => Err("job panicked".to_string()),
        };
        self.finalize(&job, outcome).await;
    }

    /// Terminal handling shared by every job outcome: transition the row
    /// (`finish` on success, `fail` otherwise), publish a final hub tick,
    /// prune the sink, write the `job_done`/`job_failed` events row, re-mirror
    /// the tray, and broadcast. `outcome` is `Ok(())` for success or
    /// `Err(message)` for any failure — a work error, a panic, or a job that
    /// could not start.
    async fn finalize(&self, job: &jobs_repo::Job, outcome: std::result::Result<(), String>) {
        let id = job.id.clone();
        let (event_kind, error) = match &outcome {
            Ok(()) => ("job_done", None),
            Err(message) => ("job_failed", Some(message.clone())),
        };
        {
            let id = id.clone();
            let error = error.clone();
            let _ = self
                .db
                .call(move |conn| match error {
                    None => jobs_repo::finish(conn, &id),
                    Some(message) => jobs_repo::fail(conn, &id, &message),
                })
                .await;
        }

        let final_progress = if outcome.is_ok() { 1.0 } else { job.progress };
        self.hub.publish(&JobProgress {
            job_id: id.clone(),
            progress: final_progress,
            message: error.clone(),
        });
        self.hub.clear(&id);

        let payload = serde_json::json!({
            "job_id": id,
            "label": job.label,
            "error": error,
        })
        .to_string();
        let _ = self
            .db
            .call(move |conn| crate::db::events::record(conn, event_kind, None, Some(&payload)))
            .await;

        self.mirror_next_active().await;
        broadcast::jobs_changed(&self.app);
        broadcast::events_appended(&self.app);
    }

    /// After a job ends, point the tray at whichever job is still active, or
    /// clear the line.
    async fn mirror_next_active(&self) {
        let active = self
            .db
            .call(|conn| jobs_repo::list(conn, true))
            .await
            .unwrap_or_default();
        match active.first() {
            Some(job) => tray::set_active_job(
                &self.app,
                Some(&format!("{} — {:.0}%", job.label, job.progress * 100.0)),
            ),
            None => tray::set_active_job(&self.app, None),
        }
    }
}

/// Adapts a tauri IPC channel to the hub; a failed send means the WebView is
/// gone and the sink gets pruned.
pub struct ChannelSink(pub tauri::ipc::Channel<JobProgress>);

impl ProgressSink for ChannelSink {
    fn send(&self, update: &JobProgress) -> bool {
        self.0.send(update.clone()).is_ok()
    }
}
