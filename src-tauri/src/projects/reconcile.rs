use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::{Error, Result};
use crate::projects::anatomy;
use crate::projects::manifest::{ProjectManifest, read_manifest};

/// One project-candidate folder found under the studio root's `Projects/` dir.
/// `manifest` is `Ok` when its `project.json` read and validated, `Err(message)`
/// when it was missing or invalid — an invalid manifest never removes a row.
pub struct ScanEntry {
    pub slug: String,
    pub path: PathBuf,
    pub manifest: std::result::Result<ProjectManifest, String>,
}

/// A folder whose `project.json` failed to read or validate. Surfaced in the
/// reconcile report so the UI can badge it; its DB row is left untouched.
#[derive(Debug, Serialize, specta::Type)]
pub struct InvalidManifest {
    pub slug: String,
    pub error: String,
}

/// The outcome of a reconcile pass: which rows were added (new valid folders),
/// which were removed (folders that vanished), and which folders carried an
/// invalid manifest (left in place).
#[derive(Debug, Default, Serialize, specta::Type)]
pub struct ReconcileReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub invalid_manifests: Vec<InvalidManifest>,
}

/// Scan `<projects_root>` for project-candidate folders. Every non-hidden
/// subdirectory becomes a [`ScanEntry`]; a missing `projects_root` yields an
/// empty scan (the studio root exists but no projects have been created yet).
///
/// A folder that exists but lacks a valid `project.json` carries an `Err`
/// manifest — it is never dropped from the scan, so a folder present on disk can
/// never cause its row to be deleted (folders are truth).
///
/// # Errors
/// `Error::Io` if the directory listing fails for a reason other than absence.
pub fn scan(projects_root: &Path) -> Result<Vec<ScanEntry>> {
    let read_dir = match std::fs::read_dir(projects_root) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::Io(e.to_string())),
    };

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().into_owned();
        if slug.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let manifest = read_manifest(&path).map_err(|e| e.to_string());
        entries.push(ScanEntry {
            slug,
            path,
            manifest,
        });
    }
    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

/// Pure diff of the DB index against the disk scan (see [`ReconcileReport`]).
///
/// - A valid folder absent from the index is `added`.
/// - A DB slug with no matching folder on disk is `removed` (folders are truth).
/// - A folder whose manifest is invalid is reported in `invalid_manifests` and
///   never added or removed — its row, if any, is left untouched.
pub fn diff(db_slugs: &[String], disk: &[ScanEntry]) -> ReconcileReport {
    use std::collections::HashSet;

    let db_set: HashSet<&str> = db_slugs.iter().map(String::as_str).collect();
    let disk_set: HashSet<&str> = disk.iter().map(|e| e.slug.as_str()).collect();

    let mut added = Vec::new();
    let mut invalid_manifests = Vec::new();
    for entry in disk {
        match &entry.manifest {
            Ok(_) => {
                if !db_set.contains(entry.slug.as_str()) {
                    added.push(entry.slug.clone());
                }
            }
            Err(error) => invalid_manifests.push(InvalidManifest {
                slug: entry.slug.clone(),
                error: error.clone(),
            }),
        }
    }

    let mut removed: Vec<String> = db_slugs
        .iter()
        .filter(|slug| !disk_set.contains(slug.as_str()))
        .cloned()
        .collect();

    added.sort();
    removed.sort();
    invalid_manifests.sort_by(|a, b| a.slug.cmp(&b.slug));
    ReconcileReport {
        added,
        removed,
        invalid_manifests,
    }
}

/// Apply a reconcile in a single transaction: upsert each added valid folder
/// from its manifest, delete each removed row and record a `project-vanished`
/// event for it. Invalid-manifest folders are not referenced here at all.
///
/// # Errors
/// `Error::Db` if any statement or the commit fails; the transaction rolls back
/// so a partial reconcile is never committed.
pub fn apply(conn: &mut Connection, disk: &[ScanEntry], report: &ReconcileReport) -> Result<()> {
    let tx = conn.transaction()?;
    for slug in &report.added {
        if let Some(entry) = disk.iter().find(|e| &e.slug == slug)
            && let Ok(manifest) = &entry.manifest
        {
            db::projects::upsert(&tx, &project_from(manifest, &entry.path))?;
            // Rebuild the schedule index from the manifest (folders are truth):
            // a Finder restore brings the folder + its dates back, and the pins
            // must reappear on the calendar with it.
            if let Some(date) = &manifest.shoot_date {
                db::schedule::upsert(&tx, slug, "shoot", date, None)?;
            }
            if let Some(date) = &manifest.publish_date {
                db::schedule::upsert(&tx, slug, "publish", date, None)?;
            }
        }
    }
    for slug in &report.removed {
        db::projects::delete(&tx, slug)?;
        db::events::record(&tx, "project-vanished", Some(slug), None)?;
    }
    tx.commit()?;
    Ok(())
}

/// Bring every scanned project folder up to the current D6 anatomy (see
/// [`anatomy::ensure_subfolders`]): folders are truth, so a project created
/// before an anatomy entry existed gains it on the next reconcile instead of
/// waiting for the owner to make the folder by hand.
///
/// Deliberately best-effort per project — a departure from this module's
/// otherwise all-or-nothing style. One unwritable folder (read-only mount, a
/// file squatting a subfolder name) must not abort the pass for the other
/// projects, and must never cost the index update, which is why this runs after
/// [`apply`] and collects instead of `?`. Folders whose manifest is invalid are
/// skipped: reconcile never touches them, and one may simply be a stray
/// directory the owner dropped under `Projects/`.
///
/// Failures land in a single aggregate event, not one per project: `events` is
/// append-only, and a read-only drive with 40 projects would otherwise flood the
/// log on every boot.
///
/// Infallible by design. It runs after [`apply`] has committed, so returning an
/// error here would fail a reconcile that already succeeded and throw away its
/// report. A failure of the aggregate event write itself — the one reporting
/// channel left — goes to stderr, as everywhere else the recorder is the thing
/// that broke.
fn backfill_anatomy(conn: &Connection, disk: &[ScanEntry]) {
    let mut failures: Vec<(String, String)> = Vec::new();
    for entry in disk {
        if entry.manifest.is_err() {
            continue;
        }
        if let Err(err) = anatomy::ensure_subfolders(&entry.path) {
            failures.push((entry.slug.clone(), err.to_string()));
        }
    }
    if failures.is_empty() {
        return;
    }
    let payload = serde_json::json!({
        "failed": failures.len(),
        "projects": failures
            .iter()
            .take(10)
            .map(|(slug, error)| serde_json::json!({ "slug": slug, "error": error }))
            .collect::<Vec<_>>(),
    });
    if let Err(err) = db::events::record(
        conn,
        "anatomy_backfill_failed",
        None,
        Some(&payload.to_string()),
    ) {
        eprintln!("failed to record anatomy_backfill_failed event: {err}");
    }
}

/// Full reconcile against a known-mounted studio `root`: scan `<root>/Projects`,
/// diff against the index, apply, and return the report. The caller is
/// responsible for the mount check (folder-touching commands reject an unmounted
/// root before reaching here). It also brings each scanned folder up to the
/// current D6 anatomy, best-effort (see [`backfill_anatomy`]).
///
/// # Errors
/// Propagates scan/DB failures from [`scan`] and [`apply`], both of which run
/// before anything is committed. The backfill that follows the commit cannot
/// fail the call, so an `Err` here always means nothing was written.
pub fn reconcile_root(conn: &mut Connection, root: &str) -> Result<ReconcileReport> {
    let projects_root = Path::new(root).join("Projects");
    let disk = scan(&projects_root)?;
    let db_slugs: Vec<String> = db::projects::list(conn)?
        .into_iter()
        .map(|p| p.slug)
        .collect();
    let report = diff(&db_slugs, &disk);
    apply(conn, &disk, &report)?;
    backfill_anatomy(conn, &disk);
    Ok(report)
}

/// Build a project row from a manifest and its on-disk directory. Reconcile never
/// stamps `last_touched_at` — discovering a folder is not an interaction.
fn project_from(manifest: &ProjectManifest, path: &Path) -> db::projects::Project {
    db::projects::Project {
        slug: manifest.slug.clone(),
        title: manifest.title.clone(),
        root_path: path.to_string_lossy().into_owned(),
        status: manifest.status.clone(),
        target_nle: manifest.target_nle.clone(),
        priority: manifest
            .priority
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        shoot_date: manifest.shoot_date.clone(),
        publish_date: manifest.publish_date.clone(),
        created_at: manifest.created_at.clone(),
        last_touched_at: None,
        kind: manifest.kind.clone().unwrap_or_else(|| "unset".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;
    use crate::projects::manifest::MANIFEST_SCHEMA_VERSION;

    fn manifest(slug: &str) -> ProjectManifest {
        ProjectManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            slug: slug.to_string(),
            title: "NVMe Deep Dive".to_string(),
            status: "idea".to_string(),
            target_nle: "resolve".to_string(),
            priority: None,
            kind: None,
            shoot_date: None,
            publish_date: None,
            created_at: "2026-07-09T00:00:00Z".to_string(),
            links: serde_json::Map::new(),
        }
    }

    fn valid_entry(slug: &str) -> ScanEntry {
        ScanEntry {
            slug: slug.to_string(),
            path: PathBuf::from(format!("/tmp/Projects/{slug}")),
            manifest: Ok(manifest(slug)),
        }
    }

    fn invalid_entry(slug: &str) -> ScanEntry {
        ScanEntry {
            slug: slug.to_string(),
            path: PathBuf::from(format!("/tmp/Projects/{slug}")),
            manifest: Err("malformed project.json".to_string()),
        }
    }

    #[test]
    fn apply_rebuilds_schedule_pins_from_a_restored_manifest() {
        let mut conn = test_db();
        let mut m = manifest("nvme-2026-07-09");
        m.shoot_date = Some("2026-08-01".to_string());
        let entry = ScanEntry {
            slug: m.slug.clone(),
            path: PathBuf::from("/tmp/Projects/nvme-2026-07-09"),
            manifest: Ok(m),
        };
        let report = diff(&[], std::slice::from_ref(&entry));
        apply(&mut conn, std::slice::from_ref(&entry), &report).unwrap();

        let rows = db::schedule::list_range(&conn, "2026-08-01", "2026-08-01").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "shoot");
        assert_eq!(rows[0].project_slug, "nvme-2026-07-09");
    }

    #[test]
    fn diff_reports_a_new_valid_folder_as_added() {
        let report = diff(&[], &[valid_entry("nvme-2026-07-09")]);
        assert_eq!(report.added, vec!["nvme-2026-07-09".to_string()]);
        assert!(report.removed.is_empty());
        assert!(report.invalid_manifests.is_empty());
    }

    #[test]
    fn diff_reports_an_orphaned_row_as_removed() {
        let db_slugs = vec!["gone-2026-07-09".to_string()];
        let report = diff(&db_slugs, &[]);
        assert_eq!(report.removed, vec!["gone-2026-07-09".to_string()]);
        assert!(report.added.is_empty());
        assert!(report.invalid_manifests.is_empty());
    }

    #[test]
    fn diff_keeps_a_present_folder_with_invalid_manifest() {
        let db_slugs = vec!["keep-2026-07-09".to_string()];
        let report = diff(&db_slugs, &[invalid_entry("keep-2026-07-09")]);
        assert!(
            report.removed.is_empty(),
            "present folder must not be removed"
        );
        assert!(report.added.is_empty());
        assert_eq!(report.invalid_manifests.len(), 1);
        assert_eq!(report.invalid_manifests[0].slug, "keep-2026-07-09");
    }

    #[test]
    fn diff_lists_a_new_invalid_folder_without_adding_it() {
        let report = diff(&[], &[invalid_entry("junk-2026-07-09")]);
        assert!(report.added.is_empty());
        assert!(report.removed.is_empty());
        assert_eq!(report.invalid_manifests.len(), 1);
    }

    #[test]
    fn diff_reports_nothing_when_disk_matches_db() {
        let db_slugs = vec!["nvme-2026-07-09".to_string()];
        let report = diff(&db_slugs, &[valid_entry("nvme-2026-07-09")]);
        assert!(report.added.is_empty());
        assert!(report.removed.is_empty());
        assert!(report.invalid_manifests.is_empty());
    }

    #[test]
    fn apply_deletes_vanished_row_and_records_project_vanished_event() {
        let mut conn = test_db();
        let row = db::projects::Project {
            slug: "gone-2026-07-09".to_string(),
            title: "Gone".to_string(),
            root_path: "/Volumes/Studio/Projects/gone-2026-07-09".to_string(),
            status: "idea".to_string(),
            target_nle: "resolve".to_string(),
            priority: "none".to_string(),
            shoot_date: None,
            publish_date: None,
            created_at: "2026-07-09T00:00:00Z".to_string(),
            last_touched_at: None,
            kind: "unset".to_string(),
        };
        db::projects::insert(&conn, &row).unwrap();

        let report = diff(&["gone-2026-07-09".to_string()], &[]);
        apply(&mut conn, &[], &report).unwrap();

        assert_eq!(db::projects::get(&conn, "gone-2026-07-09").unwrap(), None);
        let events = db::events::list(&conn, 10, None).unwrap();
        let vanished = events
            .iter()
            .find(|e| e.kind == "project-vanished")
            .expect("a project-vanished event must be recorded");
        assert_eq!(vanished.project_slug.as_deref(), Some("gone-2026-07-09"));
    }

    #[test]
    fn apply_upserts_a_new_valid_folder() {
        let mut conn = test_db();
        let disk = vec![valid_entry("nvme-2026-07-09")];
        let report = diff(&[], &disk);
        apply(&mut conn, &disk, &report).unwrap();

        let got = db::projects::get(&conn, "nvme-2026-07-09")
            .unwrap()
            .unwrap();
        assert_eq!(got.title, "NVMe Deep Dive");
        assert_eq!(got.status, "idea");
        assert_eq!(got.root_path, "/tmp/Projects/nvme-2026-07-09");
        assert_eq!(got.last_touched_at, None);
    }

    #[test]
    fn apply_leaves_invalid_manifest_row_untouched() {
        let mut conn = test_db();
        let row = db::projects::Project {
            slug: "keep-2026-07-09".to_string(),
            title: "Keep".to_string(),
            root_path: "/Volumes/Studio/Projects/keep-2026-07-09".to_string(),
            status: "editing".to_string(),
            target_nle: "resolve".to_string(),
            priority: "none".to_string(),
            shoot_date: None,
            publish_date: None,
            created_at: "2026-07-09T00:00:00Z".to_string(),
            last_touched_at: None,
            kind: "unset".to_string(),
        };
        db::projects::insert(&conn, &row).unwrap();

        let disk = vec![invalid_entry("keep-2026-07-09")];
        let report = diff(&["keep-2026-07-09".to_string()], &disk);
        apply(&mut conn, &disk, &report).unwrap();

        let got = db::projects::get(&conn, "keep-2026-07-09")
            .unwrap()
            .unwrap();
        assert_eq!(
            got.status, "editing",
            "invalid-manifest row must be untouched"
        );
    }

    #[test]
    fn scan_classifies_valid_and_invalid_folders() {
        use crate::projects::anatomy::create_project_skeleton;
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        create_project_skeleton(&projects_root, &manifest("good-2026-07-09")).unwrap();

        let bad = projects_root.join("bad-2026-07-09");
        std::fs::create_dir(&bad).unwrap();
        std::fs::write(bad.join("project.json"), b"{ not json").unwrap();

        let entries = scan(&projects_root).unwrap();
        let good = entries
            .iter()
            .find(|e| e.slug == "good-2026-07-09")
            .unwrap();
        assert!(good.manifest.is_ok());
        let bad = entries.iter().find(|e| e.slug == "bad-2026-07-09").unwrap();
        assert!(bad.manifest.is_err());
    }

    #[test]
    fn scan_of_a_missing_projects_dir_is_empty() {
        let root = tempfile::tempdir().unwrap();
        let entries = scan(&root.path().join("Projects")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn project_from_carries_the_manifests_priority() {
        let mut m = manifest("pri-2026-07-16");
        m.priority = Some("high".to_string());
        let p = project_from(&m, Path::new("/studio/Projects/pri-2026-07-16"));
        assert_eq!(p.priority, "high");
    }

    #[test]
    fn project_from_defaults_priority_when_the_manifest_omits_it() {
        let m = manifest("old-2026-07-16");
        let p = project_from(&m, Path::new("/studio/Projects/old-2026-07-16"));
        assert_eq!(p.priority, "none");
    }

    #[test]
    fn reconcile_root_backfills_missing_subfolders() {
        use crate::projects::anatomy::create_project_skeleton;
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        let dir = create_project_skeleton(&projects_root, &manifest("old-2026-07-09")).unwrap();
        // A project created before `assets/music` + `assets/sfx` joined the anatomy.
        std::fs::remove_dir(dir.join("assets").join("music")).unwrap();
        std::fs::remove_dir(dir.join("assets").join("sfx")).unwrap();

        reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();

        assert!(dir.join("assets").join("music").is_dir());
        assert!(dir.join("assets").join("sfx").is_dir());
    }

    #[test]
    fn reconcile_root_leaves_invalid_manifest_folders_untouched() {
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        let bad = projects_root.join("bad-2026-07-09");
        std::fs::create_dir(&bad).unwrap();
        std::fs::write(bad.join("project.json"), b"{ not json").unwrap();

        reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();

        assert!(
            !bad.join("assets").exists(),
            "katto must not materialise anatomy inside a folder it does not own"
        );
    }

    #[test]
    fn backfill_continues_past_a_failing_project() {
        use crate::projects::anatomy::create_project_skeleton;
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();

        let a = create_project_skeleton(&projects_root, &manifest("aaa-2026-07-09")).unwrap();
        let b = create_project_skeleton(&projects_root, &manifest("bbb-2026-07-09")).unwrap();
        for dir in [&a, &b] {
            std::fs::remove_dir(dir.join("assets").join("music")).unwrap();
            std::fs::remove_dir(dir.join("assets").join("sfx")).unwrap();
        }
        // A regular file squatting `assets/music` makes create_dir_all fail.
        std::fs::write(a.join("assets").join("music"), b"not a dir").unwrap();

        let report = reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();

        assert_eq!(report.added.len(), 2, "both rows must still be applied");
        assert!(
            db::projects::get(&conn, "bbb-2026-07-09")
                .unwrap()
                .is_some()
        );
        assert!(b.join("assets").join("music").is_dir());
        assert!(b.join("assets").join("sfx").is_dir());
    }

    #[test]
    fn backfill_failure_records_an_anatomy_backfill_failed_event() {
        use crate::projects::anatomy::create_project_skeleton;
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        let a = create_project_skeleton(&projects_root, &manifest("aaa-2026-07-09")).unwrap();
        std::fs::remove_dir(a.join("assets").join("music")).unwrap();
        std::fs::write(a.join("assets").join("music"), b"not a dir").unwrap();

        reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();

        let events = db::events::list(&conn, 50, None).unwrap();
        let failed = events
            .iter()
            .find(|e| e.kind == "anatomy_backfill_failed")
            .expect("a failing backfill must record an event");
        assert!(
            failed
                .payload_json
                .as_deref()
                .unwrap_or_default()
                .contains("aaa-2026-07-09"),
            "payload must name the failing slug: {:?}",
            failed.payload_json
        );
        assert_eq!(failed.project_slug, None);
    }

    #[test]
    fn backfill_does_not_resurrect_a_deleted_project_dir() {
        let conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let gone = root.path().join("gone-2026-07-09");
        let entry = ScanEntry {
            slug: "gone-2026-07-09".to_string(),
            path: gone.clone(),
            manifest: Ok(manifest("gone-2026-07-09")),
        };

        backfill_anatomy(&conn, std::slice::from_ref(&entry));

        assert!(!gone.exists(), "a vanished project dir must stay gone");
    }

    #[test]
    fn reconcile_root_survives_a_failed_backfill_event_write() {
        use crate::projects::anatomy::create_project_skeleton;
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        let a = create_project_skeleton(&projects_root, &manifest("aaa-2026-07-09")).unwrap();
        std::fs::remove_dir(a.join("assets").join("music")).unwrap();
        std::fs::write(a.join("assets").join("music"), b"not a dir").unwrap();
        // Read-only studio drive (every launch fails the backfill) meeting a
        // broken events table: the index update already committed, so the
        // report must still reach the caller.
        conn.execute("DROP TABLE events", []).unwrap();

        let report = reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();

        assert_eq!(report.added, vec!["aaa-2026-07-09".to_string()]);
        assert!(
            db::projects::get(&conn, "aaa-2026-07-09")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn reconcile_root_adds_disk_projects() {
        use crate::projects::anatomy::create_project_skeleton;
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        std::fs::create_dir_all(&projects_root).unwrap();
        create_project_skeleton(&projects_root, &manifest("nvme-2026-07-09")).unwrap();

        let report = reconcile_root(&mut conn, &root.path().to_string_lossy()).unwrap();
        assert_eq!(report.added, vec!["nvme-2026-07-09".to_string()]);
        assert!(
            db::projects::get(&conn, "nvme-2026-07-09")
                .unwrap()
                .is_some()
        );
    }
}
