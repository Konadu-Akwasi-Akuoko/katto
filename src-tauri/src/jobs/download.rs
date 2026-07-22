//! The download-filing job: moves a finished download out of the app-data
//! staging dir into the project's assets folder, unzipping archives and
//! writing the license sidecar. The pure core is [`file_download`]; the jobs
//! row wrapper stays thin.

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::browser::downloads::{PendingDownload, dedupe_name, license_sidecar_json, plan_filing};
use crate::browser::{downloads::FilingKind, unzip::extract_archive};
use crate::error::{Error, Result};

/// What filing produced: where the item landed (project-relative) and the
/// unzip failure note, if the archive had to stay in place.
#[derive(Debug, PartialEq)]
pub struct FiledOutcome {
    pub dest_rel: String,
    pub unzip_note: Option<String>,
}

/// Files a completed download from staging into the project dir.
/// Pure orchestration over std::fs — tempdir-tested; the jobs-row wrapper
/// stays thin.
///
/// # Errors
/// I/O failures moving or writing map to `Error::Io`; the archive path never
/// errors on unzip failure — the zip stays in place and the sidecar notes it.
pub fn file_download(
    project_dir: &Path,
    pending: &PendingDownload,
    downloaded_at: &str,
) -> Result<FiledOutcome> {
    let parsed_url = url::Url::parse(&pending.url);
    let plan = match &parsed_url {
        Ok(u) => plan_filing(u, &pending.page_url, &pending.filename),
        // unparseable source: plain non-envato file plan from the filename only
        Err(_) => plan_filing(
            &url::Url::parse("https://invalid.local/").map_err(|e| Error::Io(e.to_string()))?,
            &pending.page_url,
            &pending.filename,
        ),
    };
    let dest_dir = project_dir.join(plan.dest_dir_rel);
    std::fs::create_dir_all(&dest_dir)?;
    let exists = |name: &str| dest_dir.join(name).exists();

    let mut unzip_note = None;
    let slug = project_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let (final_item, sidecar_base) = match plan.kind {
        FilingKind::Archive => {
            // move the archive in first (partial state stays out of assets
            // until the staging move lands), then extract beside it
            let archive_name = dedupe_name(exists, &pending.filename);
            let archive_path = dest_dir.join(&archive_name);
            move_file(&pending.staging_path, &archive_path)?;
            let item_name = dedupe_name(exists, &plan.item_name);
            let item_dir = dest_dir.join(&item_name);
            match extract_archive(&archive_path, &item_dir) {
                Ok(_) => {
                    std::fs::remove_file(&archive_path)?;
                    (item_name.clone(), item_name)
                }
                Err(e) => {
                    // keep the archive; the sidecar records what happened
                    let _ = std::fs::remove_dir_all(&item_dir);
                    unzip_note = Some(format!("unzip_failed: {e}"));
                    let base = archive_name
                        .strip_suffix(".zip")
                        .unwrap_or(&archive_name)
                        .to_string();
                    (archive_name.clone(), base)
                }
            }
        }
        FilingKind::File => {
            let name = dedupe_name(exists, &plan.item_name);
            move_file(&pending.staging_path, &dest_dir.join(&name))?;
            (name.clone(), name)
        }
    };

    let sidecar = license_sidecar_json(
        &pending.url,
        &pending.page_url,
        downloaded_at,
        &slug,
        unzip_note.as_deref(),
    );
    let sidecar_path = dest_dir.join(format!("{sidecar_base}.license.json"));
    let tmp = sidecar_path.with_extension("json.tmp");
    std::fs::write(&tmp, sidecar)?;
    std::fs::rename(&tmp, &sidecar_path)?;

    // the per-download staging uuid dir is now empty; best-effort cleanup
    if let Some(staging_dir) = pending.staging_path.parent() {
        let _ = std::fs::remove_dir_all(staging_dir);
    }

    let dest_rel = format!("{}/{}", plan.dest_dir_rel, final_item);
    Ok(FiledOutcome {
        dest_rel,
        unzip_note,
    })
}

/// Rename with a copy+remove fallback for cross-device moves (staging lives
/// in app-data on the boot volume; the studio root may be another disk).
fn move_file(from: &Path, to: &Path) -> Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)?;
            Ok(())
        }
    }
}

/// Spawn the filing job for a finished download: jobs row `browser_download`,
/// staging → assets move, `asset_filed` events row, `DownloadFiled` broadcast.
pub fn spawn_filing(app: &AppHandle, pending: PendingDownload, slug: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        let label = format!("Filing — {}", pending.filename);
        let payload =
            serde_json::json!({ "filename": pending.filename, "project": slug }).to_string();
        let jobs = state.jobs.clone();
        let filename = pending.filename.clone();
        let spawn_app = app.clone();
        let result = jobs
            .spawn("browser_download", &label, Some(payload), move |_ctx| {
                run_filing(spawn_app, pending, slug)
            })
            .await;
        if let Err(err) = result {
            eprintln!("failed to spawn filing job for {filename}: {err}");
        }
    });
}

async fn run_filing(
    app: AppHandle,
    pending: PendingDownload,
    slug: String,
) -> std::result::Result<(), String> {
    let state = app.state::<crate::state::AppState>();
    let root = state
        .db
        .call(|conn| crate::db::settings::get(conn, "studio_root"))
        .await
        .map_err(|e| e.to_string())?
        .ok_or("no studio root configured")?;
    let project_dir = std::path::Path::new(&root).join("Projects").join(&slug);
    if !project_dir.is_dir() {
        return Err(format!("project folder missing: {}", project_dir.display()));
    }
    let downloaded_at = now_rfc3339();
    let job_pending = pending.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        file_download(&project_dir, &job_pending, &downloaded_at)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let payload = serde_json::json!({
        "project": slug,
        "path": outcome.dest_rel,
        "source_url": pending.url,
        "unzip_note": outcome.unzip_note,
    })
    .to_string();
    let event_slug = slug.clone();
    let _ = state
        .db
        .call(move |conn| {
            crate::db::events::record(conn, "asset_filed", Some(&event_slug), Some(&payload))
        })
        .await;
    crate::broadcast::events_appended(&app);
    crate::broadcast::download_filed(
        &app,
        &pending.id,
        &slug,
        &pending.filename,
        &outcome.dest_rel,
    );
    Ok(())
}

/// Second-precision UTC RFC3339, matching the events log's precision.
fn now_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::downloads::PendingDownload;
    use std::io::Write;

    fn pending(staging: &std::path::Path, url: &str, page: &str, name: &str) -> PendingDownload {
        PendingDownload {
            id: "d1".into(),
            url: url.into(),
            page_url: page.into(),
            filename: name.into(),
            staging_path: staging.join(name),
            started_at: "2026-07-22T12:00:00Z".into(),
        }
    }

    fn project_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("assets/envato")).unwrap();
        dir
    }

    #[test]
    fn plain_file_moves_with_sidecar() {
        let staging = tempfile::tempdir().unwrap();
        std::fs::write(staging.path().join("lut.cube"), b"data").unwrap();
        let proj = project_dir();
        let p = pending(
            staging.path(),
            "https://cdn.example.com/lut.cube",
            "https://example.com/l",
            "lut.cube",
        );
        let out = file_download(proj.path(), &p, "2026-07-22T12:01:00Z").unwrap();
        assert_eq!(out.dest_rel, "assets/lut.cube");
        assert!(proj.path().join("assets/lut.cube").is_file());
        let sidecar = proj.path().join("assets/lut.cube.license.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(v["item_url"], "https://cdn.example.com/lut.cube");
        assert!(!p.staging_path.exists());
    }

    #[test]
    fn envato_zip_unzips_into_named_dir_and_removes_archive() {
        let staging = tempfile::tempdir().unwrap();
        let mut w =
            zip::ZipWriter::new(std::fs::File::create(staging.path().join("dust.zip")).unwrap());
        w.start_file("dust/a.mov", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
        let proj = project_dir();
        let p = pending(
            staging.path(),
            "https://dl.envatousercontent.com/dust.zip",
            "https://elements.envato.com/dust",
            "dust.zip",
        );
        let out = file_download(proj.path(), &p, "t").unwrap();
        assert_eq!(out.dest_rel, "assets/envato/dust");
        assert!(proj.path().join("assets/envato/dust/a.mov").is_file());
        assert!(!proj.path().join("assets/envato/dust.zip").exists());
        assert!(
            proj.path()
                .join("assets/envato/dust.license.json")
                .is_file()
        );
        assert!(out.unzip_note.is_none());
    }

    #[test]
    fn corrupt_zip_keeps_archive_and_notes_failure() {
        let staging = tempfile::tempdir().unwrap();
        std::fs::write(staging.path().join("bad.zip"), b"not a zip").unwrap();
        let proj = project_dir();
        let p = pending(
            staging.path(),
            "https://elements.envato.com/bad.zip",
            "https://elements.envato.com/bad",
            "bad.zip",
        );
        let out = file_download(proj.path(), &p, "t").unwrap();
        assert!(
            proj.path().join("assets/envato/bad.zip").is_file(),
            "archive kept"
        );
        let note = out.unzip_note.unwrap();
        assert!(note.starts_with("unzip_failed:"));
        let sidecar = proj.path().join("assets/envato/bad.license.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sidecar).unwrap()).unwrap();
        assert!(v["note"].as_str().unwrap().starts_with("unzip_failed:"));
    }

    #[test]
    fn name_collision_dedupes_instead_of_overwriting() {
        let staging = tempfile::tempdir().unwrap();
        std::fs::write(staging.path().join("lut.cube"), b"new").unwrap();
        let proj = project_dir();
        std::fs::write(proj.path().join("assets/lut.cube"), b"old").unwrap();
        let p = pending(
            staging.path(),
            "https://x.test/lut.cube",
            "https://x.test/",
            "lut.cube",
        );
        let out = file_download(proj.path(), &p, "t").unwrap();
        assert_eq!(out.dest_rel, "assets/lut-2.cube");
        assert_eq!(
            std::fs::read(proj.path().join("assets/lut.cube")).unwrap(),
            b"old"
        );
    }
}
