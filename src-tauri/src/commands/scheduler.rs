use tauri::State;

use crate::db::scheduled_jobs::{self, ScheduledJob};
use crate::error::{Error, Result};
use crate::scheduler::runtime::SchedulerHandle;
use crate::state::AppState;

/// Every scheduled job row (Settings' nightly-curation section).
#[tauri::command]
#[specta::specta]
pub async fn get_scheduler_state(state: State<'_, AppState>) -> Result<Vec<ScheduledJob>> {
    state.db.call(|conn| scheduled_jobs::list(conn)).await
}

/// Run one scheduled job immediately (palette / Settings "Run now").
#[tauri::command]
#[specta::specta]
pub async fn run_scheduled_job_now(
    state: State<'_, AppState>,
    scheduler: State<'_, SchedulerHandle>,
    name: String,
) -> Result<()> {
    let row = {
        let name = name.clone();
        state
            .db
            .call(move |conn| scheduled_jobs::get(conn, &name))
            .await?
    };
    if row.is_none() {
        return Err(Error::Db(format!("no scheduled job named {name}")));
    }
    {
        let name = name.clone();
        state
            .db
            .call(move |conn| {
                let payload = serde_json::json!({ "name": name }).to_string();
                crate::db::events::record(conn, "scheduler_manual_run", None, Some(&payload))
            })
            .await?;
    }
    scheduler.run_now(&name);
    Ok(())
}

/// Update a job's daily time and enabled flag. The catch-up window is fixed
/// (20 h) — the schedule spec is validated before it is written.
#[tauri::command]
#[specta::specta]
pub async fn set_scheduled_job(
    state: State<'_, AppState>,
    name: String,
    hour: u32,
    minute: u32,
    enabled: bool,
) -> Result<()> {
    let spec = format!("daily@{hour:02}:{minute:02};catchup=20h");
    if crate::scheduler::due::parse_spec(&spec).is_none() {
        return Err(Error::Db(format!(
            "invalid schedule time {hour:02}:{minute:02}"
        )));
    }
    state
        .db
        .call(move |conn| scheduled_jobs::update(conn, &name, &spec, enabled))
        .await
}
