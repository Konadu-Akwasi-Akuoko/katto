use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::NaiveDateTime;
use tauri::{AppHandle, Manager};

use crate::db::scheduled_jobs::{self, ScheduledJob};
use crate::scheduler::due::{is_due, parse_spec, retry_backoff};

/// Stored/parsed format of `last_success_at`: naive LOCAL time, matching the
/// due-math (`Local::now().naive_local()`).
const LAST_SUCCESS_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// What one evaluation pass decides for one row.
#[derive(Debug)]
pub(crate) enum RowVerdict {
    Run,
    NotDue,
    Disabled,
    BadSpec,
    Backoff,
}

/// Pure decision glue over the due-math: disabled and unparseable rows never
/// run; a failure-backoff window suppresses an otherwise-due row.
pub(crate) fn evaluate_row(
    job: &ScheduledJob,
    now_local: NaiveDateTime,
    next_retry_at: Option<NaiveDateTime>,
) -> RowVerdict {
    if !job.enabled {
        return RowVerdict::Disabled;
    }
    let Some(spec) = parse_spec(&job.spec) else {
        return RowVerdict::BadSpec;
    };
    if let Some(retry_at) = next_retry_at
        && now_local < retry_at
    {
        return RowVerdict::Backoff;
    }
    let last_success = job
        .last_success_at
        .as_deref()
        .and_then(|s| NaiveDateTime::parse_from_str(s, LAST_SUCCESS_FORMAT).ok());
    if is_due(&spec, last_success, now_local) {
        RowVerdict::Run
    } else {
        RowVerdict::NotDue
    }
}

/// Managed handle: nudges wake the evaluate loop early (Mac wake), and the
/// palette/Settings "run now" path routes through it so the in-flight guard
/// covers manual runs too.
pub struct SchedulerHandle {
    shared: Arc<Shared>,
}

struct Shared {
    app: AppHandle,
    nudge: tokio::sync::mpsc::UnboundedSender<()>,
    in_flight: Mutex<HashSet<String>>,
    consecutive_failures: Mutex<HashMap<String, u32>>,
    next_retry_at: Mutex<HashMap<String, NaiveDateTime>>,
    reported_bad_specs: Mutex<HashSet<String>>,
}

impl SchedulerHandle {
    /// Wake the evaluate loop now (60 s tick otherwise).
    pub fn nudge(&self) {
        let _ = self.shared.nudge.send(());
    }

    /// Run one job immediately, clearing its failure backoff. The in-flight
    /// guard still applies — a manual run never doubles a live one.
    pub fn run_now(&self, name: &str) {
        {
            let mut retries = lock(&self.shared.next_retry_at);
            retries.remove(name);
        }
        {
            let mut failures = lock(&self.shared.consecutive_failures);
            failures.remove(name);
        }
        spawn_run(&self.shared, name.to_string());
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Start the evaluate loop (60 s tick + wake nudges) and the macOS
/// did-wake observer. Called once from the setup hook.
pub fn start(app: AppHandle) -> SchedulerHandle {
    let (nudge_tx, mut nudge_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let shared = Arc::new(Shared {
        app,
        nudge: nudge_tx.clone(),
        in_flight: Mutex::new(HashSet::new()),
        consecutive_failures: Mutex::new(HashMap::new()),
        next_retry_at: Mutex::new(HashMap::new()),
        reported_bad_specs: Mutex::new(HashSet::new()),
    });

    #[cfg(target_os = "macos")]
    install_wake_observer(nudge_tx);

    let loop_shared = Arc::clone(&shared);
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                received = nudge_rx.recv() => {
                    if received.is_none() {
                        break;
                    }
                }
            }
            evaluate_pass(&loop_shared).await;
        }
    });

    SchedulerHandle { shared }
}

async fn evaluate_pass(shared: &Arc<Shared>) {
    let db = shared.app.state::<crate::state::AppState>().db.clone();
    let rows = match db.call(|conn| scheduled_jobs::list(conn)).await {
        Ok(rows) => rows,
        Err(_) => return,
    };
    let now_local = chrono::Local::now().naive_local();
    for row in rows {
        let next_retry = lock(&shared.next_retry_at).get(&row.name).copied();
        match evaluate_row(&row, now_local, next_retry) {
            RowVerdict::Run => spawn_run(shared, row.name),
            RowVerdict::BadSpec => report_bad_spec(shared, &row),
            RowVerdict::NotDue | RowVerdict::Disabled | RowVerdict::Backoff => {}
        }
    }
}

/// A misconfigured spec surfaces once per launch (D18), then stays quiet.
fn report_bad_spec(shared: &Arc<Shared>, row: &ScheduledJob) {
    {
        let mut reported = lock(&shared.reported_bad_specs);
        if !reported.insert(row.name.clone()) {
            return;
        }
    }
    record_event(
        shared,
        "scheduler_bad_spec",
        serde_json::json!({ "name": row.name, "spec": row.spec }),
    );
}

fn spawn_run(shared: &Arc<Shared>, name: String) {
    {
        let mut in_flight = lock(&shared.in_flight);
        // Never run twice for one slot: a 60 s tick during a long run is a no-op.
        if !in_flight.insert(name.clone()) {
            return;
        }
    }
    let shared = Arc::clone(shared);
    tauri::async_runtime::spawn(async move {
        let outcome = run_job(&shared.app, &name).await;
        let now_local = chrono::Local::now().naive_local();
        match outcome {
            Ok(()) => {
                let stamp = now_local.format(LAST_SUCCESS_FORMAT).to_string();
                let db = shared.app.state::<crate::state::AppState>().db.clone();
                let job_name = name.clone();
                let _ = db
                    .call(move |conn| scheduled_jobs::set_last_success(conn, &job_name, &stamp))
                    .await;
                lock(&shared.consecutive_failures).remove(&name);
                lock(&shared.next_retry_at).remove(&name);
            }
            Err(error) => {
                let failures = {
                    let mut map = lock(&shared.consecutive_failures);
                    let count = map.entry(name.clone()).or_insert(0);
                    *count += 1;
                    *count
                };
                let backoff = retry_backoff(failures);
                lock(&shared.next_retry_at).insert(name.clone(), now_local + backoff);
                record_event(
                    &shared,
                    "scheduler_job_failed",
                    serde_json::json!({
                        "name": name,
                        "error": error,
                        "retry_in_secs": backoff.num_seconds(),
                    }),
                );
            }
        }
        lock(&shared.in_flight).remove(&name);
    });
}

/// The job registry. The scheduler needs the outcome to gate
/// `last_success_at`; each entry's jobs row is its visibility contract.
async fn run_job(app: &AppHandle, name: &str) -> std::result::Result<(), String> {
    let _ = app;
    if name == "nightly-curation" {
        // Wired in the curation task: crate::curation::run(app).await.
        return Err("nightly curation is not wired yet".to_string());
    }
    Err(format!("unknown scheduled job: {name}"))
}

fn record_event(shared: &Arc<Shared>, kind: &'static str, payload: serde_json::Value) {
    let db = shared.app.state::<crate::state::AppState>().db.clone();
    let app = shared.app.clone();
    tauri::async_runtime::spawn(async move {
        let payload = payload.to_string();
        let _ = db
            .call(move |conn| crate::db::events::record(conn, kind, None, Some(&payload)))
            .await;
        crate::broadcast::events_appended(&app);
    });
}

/// Register for `NSWorkspaceDidWakeNotification`; the block nudges the
/// evaluate loop so a slept-through slot runs promptly on wake.
#[cfg(target_os = "macos")]
fn install_wake_observer(nudge: tokio::sync::mpsc::UnboundedSender<()>) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceDidWakeNotification};
    use objc2_foundation::NSNotification;

    let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        let _ = nudge.send(());
    });
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: the notification name is a valid AppKit static; the block only
    // captures a Send channel sender, and the returned observer token is
    // deliberately leaked below so it (and the block) outlive every delivery.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidWakeNotification),
            None,
            None,
            &block,
        )
    };
    // Deliberate leak: the observer lives for the whole process.
    std::mem::forget(token);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 22)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn job(spec: &str, last_success_at: Option<&str>, enabled: bool) -> ScheduledJob {
        ScheduledJob {
            name: "nightly-curation".to_string(),
            spec: spec.to_string(),
            last_success_at: last_success_at.map(str::to_string),
            enabled,
        }
    }

    #[test]
    fn disabled_row_never_runs() {
        let row = job("daily@00:00;catchup=20h", None, false);
        assert!(matches!(
            evaluate_row(&row, at(8, 0), None),
            RowVerdict::Disabled
        ));
    }

    #[test]
    fn bad_spec_is_flagged_not_fatal() {
        let row = job("garbage", None, true);
        assert!(matches!(
            evaluate_row(&row, at(8, 0), None),
            RowVerdict::BadSpec
        ));
    }

    #[test]
    fn backoff_window_suppresses_due_row() {
        let row = job("daily@00:00;catchup=20h", None, true);
        assert!(matches!(
            evaluate_row(&row, at(8, 0), Some(at(9, 0))),
            RowVerdict::Backoff
        ));
    }

    #[test]
    fn due_enabled_row_runs() {
        let row = job("daily@00:00;catchup=20h", Some("2026-07-21 00:05:00"), true);
        assert!(matches!(
            evaluate_row(&row, at(8, 0), None),
            RowVerdict::Run
        ));
    }

    #[test]
    fn ran_today_row_not_due() {
        let row = job("daily@00:00;catchup=20h", Some("2026-07-22 00:05:00"), true);
        assert!(matches!(
            evaluate_row(&row, at(9, 0), None),
            RowVerdict::NotDue
        ));
    }
}
