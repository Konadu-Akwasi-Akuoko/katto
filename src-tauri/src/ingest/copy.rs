use std::path::{Path, PathBuf};

use tauri::AppHandle;

use katto_engine::ingest::Rename;
use katto_engine::ingest::VerifyError;
use katto_engine::ingest::verify::verify;

use crate::broadcast;
use crate::db::{DbHandle, events};
use crate::jobs::JobContext;

/// Everything the copy job needs, resolved by the command before spawning.
pub struct CopyPlan {
    /// Absolute path to the volume/source root that `Rename::source` is relative to.
    pub source_root: PathBuf,
    /// Absolute path to the project's `footage/` directory.
    pub footage_dir: PathBuf,
    /// The planned copies.
    pub renames: Vec<Rename>,
    /// The owning project slug (for the events row).
    pub project_slug: String,
}

/// Claim `final_path` from a fully-copied `partial`: `hard_link` fails with
/// `AlreadyExists` if the final name is taken (unlike `rename`, which silently
/// replaces), so a concurrent job can never overwrite verified footage. The
/// partial is unlinked only after the claim succeeds.
pub fn claim_final(partial: &Path, final_path: &Path) -> std::io::Result<()> {
    std::fs::hard_link(partial, final_path)?;
    // Best-effort: a leftover partial after a successful claim is harmless.
    let _ = std::fs::remove_file(partial);
    Ok(())
}

/// Copy one source file to a job-unique `footage/<dest>.<token>.partial`, then
/// claim the final name via [`claim_final`]. Returns the copied byte count.
/// On any failure the partial is renamed to the PRD's `<dest>.partial`
/// quarantine name and the final name is left untouched. The source is opened
/// read-only by `std::fs::copy`; the card is never written.
pub fn copy_one(
    source: &Path,
    footage_dir: &Path,
    dest_name: &str,
    token: &str,
) -> std::io::Result<u64> {
    let partial = footage_dir.join(format!("{dest_name}.{token}.partial"));
    let quarantine = footage_dir.join(format!("{dest_name}.partial"));
    let final_path = footage_dir.join(dest_name);

    let copied = std::fs::copy(source, &partial)?;
    let source_size = std::fs::metadata(source)?.len();
    if copied != source_size {
        let _ = std::fs::rename(&partial, &quarantine);
        return Err(std::io::Error::other(format!(
            "size mismatch for {dest_name}: {copied} != {source_size}"
        )));
    }
    if let Err(err) = claim_final(&partial, &final_path) {
        let _ = std::fs::rename(&partial, &quarantine);
        return Err(std::io::Error::other(format!(
            "cannot claim {dest_name}: {err}"
        )));
    }
    Ok(copied)
}

/// Rename every size-mismatched destination back to its `<name>.partial`
/// quarantine name so a failed verification never leaves a bad file sitting in
/// `footage/` under a valid final name (PRD: offending file quarantined).
pub fn quarantine_offenders(footage_dir: &Path, errors: &[VerifyError]) {
    for error in errors {
        if let VerifyError::SizeMismatch { name, .. } = error {
            let _ = std::fs::rename(
                footage_dir.join(name),
                footage_dir.join(format!("{name}.partial")),
            );
        }
    }
}

/// Names of the destinations a verification pass flagged (missing or wrong
/// size) — the "remaining" set a retry must re-copy.
fn offender_names(errors: &[VerifyError]) -> Vec<String> {
    errors
        .iter()
        .filter_map(|e| match e {
            VerifyError::SizeMismatch { name, .. } | VerifyError::Missing { name } => {
                Some(name.clone())
            }
            VerifyError::CountMismatch { .. } => None,
        })
        .collect()
}

/// Record an `ingest_failed` events row carrying the sources that did NOT land
/// (relative to the plan's source root) so the import sheet can offer "retry
/// remaining". Best-effort — the job error itself is already the terminal
/// signal via the jobs runtime.
async fn record_ingest_failed(
    db: &DbHandle,
    app: &AppHandle,
    plan: &CopyPlan,
    job_id: &str,
    remaining: Vec<String>,
) {
    let payload = serde_json::json!({
        "job_id": job_id,
        "remaining": remaining,
        "volume": plan.source_root.to_string_lossy(),
    })
    .to_string();
    let slug = plan.project_slug.clone();
    let outcome = db
        .call(move |conn| events::record(conn, "ingest_failed", Some(&slug), Some(&payload)))
        .await;
    if let Err(err) = outcome {
        eprintln!("failed to record ingest_failed event: {err}");
    }
    broadcast::events_appended(app);
}

/// Run the copy job: per-file copy+claim with progress ticks, a final on-disk
/// verification with quarantine, an `ingested` events row on success, and an
/// `ingest_failed` events row (with the un-imported remainder) on failure.
/// Returns `Err(message)` for the jobs runtime to record as a terminal failure.
pub async fn run_copy_job(
    ctx: JobContext,
    db: DbHandle,
    app: AppHandle,
    plan: CopyPlan,
) -> std::result::Result<(), String> {
    let total = plan.renames.len();
    let token = uuid::Uuid::new_v4().to_string();
    let mut expected: Vec<(String, u64)> = Vec::with_capacity(total);
    let mut total_bytes: u64 = 0;

    for (i, rename) in plan.renames.iter().enumerate() {
        let source = plan.source_root.join(&rename.source);
        let stat = std::fs::metadata(&source)
            .map_err(|e| format!("cannot stat source {}: {e}", source.display()));
        let source_size = match stat {
            Ok(meta) => meta.len(),
            Err(message) => {
                let remaining = remaining_sources(&plan, i);
                record_ingest_failed(&db, &app, &plan, ctx.job_id(), remaining).await;
                return Err(message);
            }
        };
        ctx.progress(
            i as f64 / total as f64,
            Some(format!("Copying {}", rename.dest_name)),
        )
        .await;

        let footage = plan.footage_dir.clone();
        let dest = rename.dest_name.clone();
        let src = source.clone();
        let tok = token.clone();
        let copied =
            tauri::async_runtime::spawn_blocking(move || copy_one(&src, &footage, &dest, &tok))
                .await
                .map_err(|_| "copy task panicked".to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
        let copied = match copied {
            Ok(n) => n,
            Err(message) => {
                let remaining = remaining_sources(&plan, i);
                record_ingest_failed(&db, &app, &plan, ctx.job_id(), remaining).await;
                return Err(message);
            }
        };

        expected.push((rename.dest_name.clone(), source_size));
        total_bytes += copied;
    }

    // Verify against what actually landed on disk, not the loop's bookkeeping:
    // re-stat every destination so a vanished or truncated file is caught.
    let footage = plan.footage_dir.clone();
    let names: Vec<String> = plan.renames.iter().map(|r| r.dest_name.clone()).collect();
    let actual: Vec<(String, u64)> = tauri::async_runtime::spawn_blocking(move || {
        names
            .into_iter()
            .filter_map(|n| {
                let size = std::fs::metadata(footage.join(&n)).ok()?.len();
                Some((n, size))
            })
            .collect()
    })
    .await
    .map_err(|_| "verify task panicked".to_string())?;

    let errors = verify(&expected, &actual);
    if !errors.is_empty() {
        let offenders = offender_names(&errors);
        let footage = plan.footage_dir.clone();
        let quarantine_errors = errors.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            quarantine_offenders(&footage, &quarantine_errors);
        })
        .await;
        let remaining: Vec<String> = plan
            .renames
            .iter()
            .filter(|r| offenders.contains(&r.dest_name))
            .map(|r| r.source.to_string_lossy().into_owned())
            .collect();
        record_ingest_failed(&db, &app, &plan, ctx.job_id(), remaining).await;
        return Err(format!("verification failed: {errors:?}"));
    }

    let payload = serde_json::json!({
        "count": total,
        "bytes": total_bytes,
        "project": plan.project_slug,
    })
    .to_string();
    let slug = plan.project_slug.clone();
    let outcome = db
        .call(move |conn| events::record(conn, "ingested", Some(&slug), Some(&payload)))
        .await;
    if let Err(err) = outcome {
        eprintln!("failed to record ingested event: {err}");
    }
    broadcast::events_appended(&app);
    Ok(())
}

/// The sources not yet copied when the job failed at `failed_index`, including
/// the failed one itself (it did not land).
fn remaining_sources(plan: &CopyPlan, failed_index: usize) -> Vec<String> {
    plan.renames[failed_index..]
        .iter()
        .map(|r| r.source.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_one_claims_final_and_removes_partials_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("C0001.MP4");
        std::fs::write(&src, b"footage-bytes").unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();

        let copied = copy_one(&src, &footage, "2026-07-22_001.mp4", "tok").unwrap();
        assert_eq!(copied, 13);
        assert!(footage.join("2026-07-22_001.mp4").exists());
        let leftovers: Vec<_> = std::fs::read_dir(&footage)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("partial"))
            .collect();
        assert!(leftovers.is_empty());
        // Source untouched.
        assert_eq!(std::fs::read(&src).unwrap(), b"footage-bytes");
    }

    #[test]
    fn claim_final_refuses_when_final_appears_after_copy_and_keeps_partial() {
        // The concurrent-job race: another job claims the same final name
        // between this job's copy and its claim. `rename` would silently
        // replace the other job's verified file; the hard-link claim must fail
        // and leave both the existing final and this job's partial intact.
        let dir = tempfile::tempdir().unwrap();
        let partial = dir.path().join("2026-07-22_001.mp4.tok.partial");
        std::fs::write(&partial, b"job-two-bytes").unwrap();
        let final_path = dir.path().join("2026-07-22_001.mp4");
        std::fs::write(&final_path, b"job-one-verified").unwrap();

        let err = claim_final(&partial, &final_path);
        assert!(err.is_err());
        assert_eq!(std::fs::read(&final_path).unwrap(), b"job-one-verified");
        assert!(partial.exists());
    }

    #[test]
    fn copy_one_quarantines_when_destination_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("C0001.MP4");
        std::fs::write(&src, b"new bytes").unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        std::fs::write(footage.join("2026-07-22_001.mp4"), b"earlier footage").unwrap();

        let err = copy_one(&src, &footage, "2026-07-22_001.mp4", "tok");
        assert!(err.is_err());
        // The earlier footage is untouched; the loser is quarantined.
        assert_eq!(
            std::fs::read(footage.join("2026-07-22_001.mp4")).unwrap(),
            b"earlier footage"
        );
        assert!(footage.join("2026-07-22_001.mp4.partial").exists());
    }

    #[test]
    fn copy_one_missing_source_leaves_no_final_file() {
        let dir = tempfile::tempdir().unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        let err = copy_one(
            &dir.path().join("nope.mp4"),
            &footage,
            "2026-07-22_001.mp4",
            "tok",
        );
        assert!(err.is_err());
        assert!(!footage.join("2026-07-22_001.mp4").exists());
    }

    #[test]
    fn quarantine_offenders_renames_size_mismatches_to_partial() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2026-07-22_001.mp4"), b"truncated").unwrap();
        std::fs::write(dir.path().join("2026-07-22_002.mp4"), b"good").unwrap();

        quarantine_offenders(
            dir.path(),
            &[VerifyError::SizeMismatch {
                name: "2026-07-22_001.mp4".to_string(),
                expected: 100,
                actual: 9,
            }],
        );

        assert!(!dir.path().join("2026-07-22_001.mp4").exists());
        assert!(dir.path().join("2026-07-22_001.mp4.partial").exists());
        assert!(dir.path().join("2026-07-22_002.mp4").exists());
    }
}
