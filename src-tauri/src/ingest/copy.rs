use std::path::{Path, PathBuf};

use tauri::AppHandle;

use katto_engine::ingest::Rename;
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

/// Copy one source file to `footage/<dest_name>.partial`, then rename to the
/// final name only after the byte count matches. Returns the copied byte count.
/// The source is opened read-only by `std::fs::copy`; the card is never written.
pub fn copy_one(source: &Path, footage_dir: &Path, dest_name: &str) -> std::io::Result<u64> {
    let partial = footage_dir.join(format!("{dest_name}.partial"));
    let final_path = footage_dir.join(dest_name);
    if final_path.exists() {
        // Sequence planning makes collisions impossible for a single job; this
        // backstop keeps a concurrent job from silently overwriting footage.
        return Err(std::io::Error::other(format!(
            "destination already exists: {dest_name}"
        )));
    }
    let copied = std::fs::copy(source, &partial)?;
    let source_size = std::fs::metadata(source)?.len();
    if copied != source_size {
        // Leave the `.partial` as quarantine; do not rename into the final name.
        return Err(std::io::Error::other(format!(
            "size mismatch for {dest_name}: {copied} != {source_size}"
        )));
    }
    std::fs::rename(&partial, &final_path)?;
    Ok(copied)
}

/// Run the copy job: per-file copy+verify with progress ticks, a final
/// count/size verification, and an `ingested` events row on success. Returns
/// `Err(message)` for the jobs runtime to record as a terminal failure.
pub async fn run_copy_job(
    ctx: JobContext,
    db: DbHandle,
    app: AppHandle,
    plan: CopyPlan,
) -> std::result::Result<(), String> {
    let total = plan.renames.len();
    let mut expected: Vec<(String, u64)> = Vec::with_capacity(total);
    let mut total_bytes: u64 = 0;

    for (i, rename) in plan.renames.iter().enumerate() {
        let source = plan.source_root.join(&rename.source);
        let source_size = std::fs::metadata(&source)
            .map_err(|e| format!("cannot stat source {}: {e}", source.display()))?
            .len();
        ctx.progress(
            i as f64 / total as f64,
            Some(format!("Copying {}", rename.dest_name)),
        )
        .await;

        let footage = plan.footage_dir.clone();
        let dest = rename.dest_name.clone();
        let src = source.clone();
        let copied = tauri::async_runtime::spawn_blocking(move || copy_one(&src, &footage, &dest))
            .await
            .map_err(|_| "copy task panicked".to_string())?
            .map_err(|e| e.to_string())?;

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
        return Err(format!("verification failed: {errors:?}"));
    }

    let payload = serde_json::json!({
        "count": total,
        "bytes": total_bytes,
        "project": plan.project_slug,
    })
    .to_string();
    let slug = plan.project_slug.clone();
    let _ = db
        .call(move |conn| events::record(conn, "ingested", Some(&slug), Some(&payload)))
        .await;
    broadcast::events_appended(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_one_renames_partial_to_final_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("C0001.MP4");
        std::fs::write(&src, b"footage-bytes").unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();

        let copied = copy_one(&src, &footage, "2026-07-22_001.mp4").unwrap();
        assert_eq!(copied, 13);
        assert!(footage.join("2026-07-22_001.mp4").exists());
        assert!(!footage.join("2026-07-22_001.mp4.partial").exists());
        // Source untouched.
        assert_eq!(std::fs::read(&src).unwrap(), b"footage-bytes");
    }

    #[test]
    fn copy_one_refuses_to_overwrite_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("C0001.MP4");
        std::fs::write(&src, b"new bytes").unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        std::fs::write(footage.join("2026-07-22_001.mp4"), b"earlier footage").unwrap();

        let err = copy_one(&src, &footage, "2026-07-22_001.mp4");
        assert!(err.is_err());
        // The earlier footage is untouched.
        assert_eq!(
            std::fs::read(footage.join("2026-07-22_001.mp4")).unwrap(),
            b"earlier footage"
        );
    }

    #[test]
    fn copy_one_missing_source_leaves_no_final_file() {
        let dir = tempfile::tempdir().unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        let err = copy_one(&dir.path().join("nope.mp4"), &footage, "2026-07-22_001.mp4");
        assert!(err.is_err());
        assert!(!footage.join("2026-07-22_001.mp4").exists());
    }
}
