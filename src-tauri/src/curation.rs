//! Nightly idea curation (Phase 6): optional discovery sweep, then a visible
//! dock session judging the unjudged `raw_signal` delta. The summary comes
//! from DB deltas, never from parsing session output; verdicts are binary
//! keep/discard with a rationale — no numeric scoring anywhere (D7).

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::db::{ideas, raw_signal};
use crate::sessions::{Program, SessionTask};

/// Judged rows older than this are pruned after each successful run (PRD
/// housekeeping); unjudged rows are never touched.
const PRUNE_AFTER_DAYS: u32 = 90;
/// The discovery subprocess gets this long before it is abandoned.
const DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);
/// The curation session gets this long to reach its first Stop.
const SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Scheduler-registry entry (also behind the palette's "run now"): spawns the
/// `nightly_curation` jobs row and reports the job's own outcome, because the
/// scheduler needs it to gate `last_success_at` while the jobs row stays the
/// visibility contract.
pub async fn run(app: &AppHandle) -> std::result::Result<(), String> {
    let state = app.state::<crate::state::AppState>();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let work_app = app.clone();
    state
        .jobs
        .spawn(
            "nightly_curation",
            "Nightly curation",
            None,
            move |ctx| async move {
                let outcome = work(work_app, ctx).await;
                let _ = done_tx.send(outcome.clone());
                outcome
            },
        )
        .await
        .map_err(|err| err.to_string())?;
    done_rx
        .await
        .unwrap_or(Err("curation job dropped".to_string()))
}

async fn work(app: AppHandle, ctx: crate::jobs::JobContext) -> std::result::Result<(), String> {
    let state = app.state::<crate::state::AppState>();
    let db = state.db.clone();
    // The session writes judged_at/first_seen with sqlite datetime('now') =
    // UTC, so the run marker must be UTC too for the delta counts to hold.
    let run_started = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let claude_present = {
        let configured = db
            .call(|conn| crate::db::settings::get(conn, "claude_path"))
            .await
            .map_err(|err| err.to_string())?
            .filter(|p| !p.is_empty());
        configured.is_some()
            || tauri::async_runtime::spawn_blocking(katto_engine::detect::detect_claude)
                .await
                .map_err(|err| err.to_string())?
                .is_some()
    };
    if !claude_present {
        record_event(
            &app,
            "curation_skipped",
            serde_json::json!({ "reason": "claude not on PATH" }),
        )
        .await;
        return Err("claude not on PATH".to_string());
    }

    let app_data = app.path().app_data_dir().map_err(|err| err.to_string())?;
    let db_path = app_data.join("katto.db");

    let discovery_enabled = db
        .call(|conn| crate::db::settings::get(conn, "discovery_enabled"))
        .await
        .ok()
        .flatten()
        .as_deref()
        == Some("true");
    let hyperframes_path = db
        .call(|conn| crate::db::settings::get(conn, "hyperframes_path"))
        .await
        .ok()
        .flatten()
        .filter(|p| !p.is_empty());

    let mut discovery_ran = false;
    let mut discovery_failed = false;
    if discovery_enabled && let Some(hp) = hyperframes_path {
        ctx.progress(0.2, Some("discovery".to_string())).await;
        discovery_ran = true;
        discovery_failed = !run_discovery(&app, Path::new(&hp), &db_path).await;
    }

    let unjudged = db
        .call(|conn| raw_signal::unjudged_count(conn))
        .await
        .map_err(|err| err.to_string())?;
    if unjudged == 0 && !discovery_ran {
        record_event(
            &app,
            "curation_done",
            serde_json::json!({ "kept": 0, "discarded": 0, "noop": true }),
        )
        .await;
        return Ok(());
    }

    ctx.progress(0.4, Some("judging".to_string())).await;
    let task = SessionTask {
        label: "ideas: nightly".to_string(),
        cwd: app_data.clone(),
        initial_prompt: Some(curation_prompt(&db_path)),
        append_system_prompt: None,
        permission_mode: None,
        permission_allow: vec!["Bash(sqlite3:*)".to_string()],
    };
    let session_id = state
        .sessions
        .spawn(&app, task, Program::Claude)
        .await
        .map_err(|err| err.to_string())?;

    let first_stop = state.sessions.watch_first_stop(&session_id);
    let outcome = tokio::time::timeout(SESSION_TIMEOUT, first_stop).await;
    match outcome {
        Err(_) => {
            // Leave the session alive and visible (D18) — the owner decides.
            record_event(
                &app,
                "curation_timeout",
                serde_json::json!({ "session_id": session_id }),
            )
            .await;
            return Err("curation session timed out".to_string());
        }
        Ok(Err(_)) => return Err("curation session dropped".to_string()),
        Ok(Ok(Err(error))) => return Err(error),
        Ok(Ok(Ok(()))) => {}
    }

    ctx.progress(0.9, Some("summarizing".to_string())).await;
    let marker = run_started.clone();
    let (kept, discarded) = db
        .call(move |conn| raw_signal::judged_counts_since(conn, &marker))
        .await
        .map_err(|err| err.to_string())?;
    let marker = run_started.clone();
    let new_ideas = db
        .call(move |conn| ideas::count_since(conn, &marker))
        .await
        .map_err(|err| err.to_string())?;
    if let Err(error) = db
        .call(|conn| raw_signal::prune_judged_older_than_days(conn, PRUNE_AFTER_DAYS))
        .await
    {
        // Housekeeping failure must not fail the judged run, but it surfaces.
        record_event(
            &app,
            "curation_prune_failed",
            serde_json::json!({ "error": error.to_string() }),
        )
        .await;
    }

    record_event(
        &app,
        "curation_done",
        serde_json::json!({
            "kept": kept,
            "discarded": discarded,
            "new_ideas": new_ideas,
            "discovery_failed": discovery_failed,
        }),
    )
    .await;
    crate::broadcast::ideas_changed(&app);
    let suffix = if discovery_failed {
        " — discovery failed"
    } else {
        ""
    };
    let _ = crate::notify::notify(
        &app,
        "Nightly curation",
        &format!("kept {kept} / discarded {discarded}{suffix}"),
        "katto://ideas",
    );
    Ok(())
}

/// Run the owner's studio-discover CLI against katto's DB (column parity, D7).
/// Returns false on failure/timeout — recorded, never fatal to the run.
async fn run_discovery(app: &AppHandle, hyperframes: &Path, db_path: &Path) -> bool {
    let discovery_dir = hyperframes.join("tools/studio/discovery");
    let output = tokio::time::timeout(
        DISCOVERY_TIMEOUT,
        tokio::process::Command::new("uv")
            .arg("run")
            .arg("studio-discover")
            .arg("--db")
            .arg(db_path)
            .current_dir(&discovery_dir)
            .output(),
    )
    .await;
    let failure = match output {
        Err(_) => Some("discovery timed out".to_string()),
        Ok(Err(err)) => Some(format!("discovery failed to start: {err}")),
        Ok(Ok(out)) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let tail: String = stderr
                .chars()
                .skip(stderr.chars().count().saturating_sub(500))
                .collect();
            Some(tail)
        }
        Ok(Ok(_)) => None,
    };
    match failure {
        None => true,
        Some(stderr_tail) => {
            record_event(
                app,
                "curation_discovery_failed",
                serde_json::json!({ "stderr_tail": stderr_tail }),
            )
            .await;
            false
        }
    }
}

async fn record_event(app: &AppHandle, kind: &'static str, payload: serde_json::Value) {
    let db = app.state::<crate::state::AppState>().db.clone();
    let payload = payload.to_string();
    let _ = db
        .call(move |conn| crate::db::events::record(conn, kind, None, Some(&payload)))
        .await;
    crate::broadcast::events_appended(app);
}

/// The ONLY judgment text: criteria verbatim from the studio-ideas skill
/// (hyper-frames mirror), schema adapted to katto (`status='backlog'`,
/// novelty guard over ideas + projects). The summary the session ends with is
/// display-only — katto computes the real numbers from DB deltas.
pub(crate) fn curation_prompt(db_path: &Path) -> String {
    let db = db_path.display();
    format!(
        r#"Curate the idea backlog in the katto SQLite database at {db}.

Work ONLY through the sqlite3 CLI against that path. Do not read any other files.

1. Read only the unjudged delta. First:
   sqlite3 {db} "SELECT source, count(*) FROM raw_signal WHERE judged_at IS NULL GROUP BY source;"
   Then pull compact batches (JSON mode), aggregator/video rows and
   youtube-comments:* rows SEPARATELY, capped (~120 aggregator rows, ~40 comment rows
   per batch); judge a batch, mark it, then pull the next. Never the whole table.
2. Novelty guard: existing idea titles —
   sqlite3 {db} "SELECT title FROM ideas WHERE status IN ('backlog','promoted');"
   and existing project folders —
   sqlite3 {db} "SELECT slug, title FROM projects;"
3. Judge each row keep or discard — binary, qualitative, with a one-line why-pursue
   rationale. NEVER a number, score, rank, percentage, or grade. Lenses:
   - Fit: a real computing / system-design / dev-tools angle that is on-brand.
     Off-topic, listicle-bait, pure news → discard.
   - Novelty: not already an idea or a made project; a near-duplicate of covered
     ground → discard (or keep as a deliberately fresh angle, said so in the
     rationale).
   - Demand shape (comment rows): cluster into themes; a RECURRING ask is a strong
     keep — capture 1-2 representative quotes and the count. One-off praise is not
     demand.
   When unsure, discard — the backlog is small on purpose.
4. For keepers, suggest a format kind: short (single tight mechanism, broad-appeal
   quick hit) / long (full system-design build-up) / series (too big for one video).
   It is a suggestion the human confirms — never a grade.
5. Insert keepers into ideas:
   id = lower(hex(randomblob(8))), status='backlog',
   type mapped from source (youtube-comments:* → comment_demand, youtube:* → mirror,
   hn / reddit:* / lobsters / dailydev → trend),
   kind = your suggestion, kind_source='ai', kind_why = one line,
   rationale = the one-line why-pursue,
   source, source_url, source_title from the raw row, raw_signal_id = its id,
   first_seen = datetime('now'),
   evidence_json = json('{{"lean":"hold|lean|strong", ...source metrics, quotes for
   comment_demand}}') — lean is a categorical meter hint, never a number.
   The title is YOUR framing of the video, not necessarily the raw item's title.
6. Mark EVERY row you read this pass:
   judged_at = datetime('now'), judged_verdict = 'kept' or 'discarded'.
   Discarded rows stay as the audit trail — never delete, never re-judge.
7. Finish with a one-paragraph summary: kept N by type, discarded M, titles with
   suggested kinds. Do not promote anything, do not act on any idea — the
   make-or-not call is the human's.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn prompt_carries_db_path_and_delta_read() {
        let prompt = curation_prompt(Path::new("/data/katto.db"));
        assert!(prompt.contains("/data/katto.db"));
        assert!(prompt.contains("judged_at IS NULL"));
        assert!(prompt.contains("status IN ('backlog','promoted')"));
    }

    #[test]
    fn prompt_forbids_numeric_scoring() {
        let prompt = curation_prompt(Path::new("/d/k.db"));
        assert!(prompt.contains("NEVER a number, score, rank, percentage, or grade"));
        assert!(prompt.contains("status='backlog'"));
        assert!(prompt.contains("judged_verdict = 'kept' or 'discarded'"));
    }

    #[test]
    fn prompt_keeps_the_human_in_charge() {
        let prompt = curation_prompt(Path::new("/d/k.db"));
        assert!(prompt.contains("do not act on any idea"));
        assert!(prompt.contains("the human"));
    }
}
