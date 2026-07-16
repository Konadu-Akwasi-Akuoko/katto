use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::db;
use crate::db::projects::{PriorityLevel, Project};
use crate::error::{Error, Result};
use crate::paths;
use crate::projects::anatomy::{PROJECT_SUBFOLDERS, create_project_skeleton};
use crate::projects::freshness::{FolderFreshness, folder_freshness};
use crate::projects::manifest::{
    MANIFEST_SCHEMA_VERSION, ProjectManifest, read_manifest, write_manifest,
};
use crate::projects::project_slug;
use crate::projects::reconcile::{self, ReconcileReport};
use crate::state::AppState;

/// A project row plus the on-disk facts the detail surface needs: whether its
/// `project.json` still validates (`manifest_error` = `Some` message when not —
/// folders are truth, so a bad manifest badges the project rather than dropping
/// it) and per-subfolder freshness.
#[derive(Debug, Serialize, specta::Type)]
pub struct ProjectDetail {
    pub project: Project,
    pub manifest_error: Option<String>,
    pub freshness: Vec<FolderFreshness>,
}

/// Reconcile the projects index against the studio-root folders on demand
/// (folders are truth). Reads the configured `studio_root`, refuses when it is
/// unmounted, then scans `<root>/Projects`, diffs against the index, and applies.
///
/// Returns an empty report when no studio root is configured yet.
#[tauri::command]
#[specta::specta]
pub async fn rescan_projects(state: State<'_, AppState>) -> Result<ReconcileReport> {
    state
        .db
        .call(|conn| {
            let Some(root) = db::settings::get(conn, "studio_root")? else {
                return Ok(ReconcileReport::default());
            };
            if !paths::root_mounted(Path::new(&root)) {
                return Err(Error::StudioRootUnmounted(format!(
                    "studio root is not mounted: {root}"
                )));
            }
            reconcile::reconcile_root(conn, &root)
        })
        .await
}

/// Every project row, most-recently-touched first.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>> {
    state.db.call(|conn| db::projects::list(conn)).await
}

/// One project with its manifest-validity flag and per-subfolder freshness.
/// Guards the studio-root mount before reading any folder.
#[tauri::command]
#[specta::specta]
pub async fn get_project(state: State<'_, AppState>, slug: String) -> Result<ProjectDetail> {
    let lookup = slug.clone();
    let project = state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            db::projects::get(conn, &lookup)
        })
        .await?
        .ok_or_else(|| Error::Io(format!("no such project: {slug}")))?;

    let dir = PathBuf::from(&project.root_path);
    let manifest_error = read_manifest(&dir).err().map(|e| e.to_string());
    let freshness = folder_freshness(&dir)?;
    Ok(ProjectDetail {
        project,
        manifest_error,
        freshness,
    })
}

/// Create a new project: dedupe the slug against the index, lay down the D6
/// folder skeleton with a fresh `project.json`, insert and touch the row, record
/// a `project-created` event, and broadcast. All folder + DB work runs on the
/// single writer thread so the slug dedupe and the folder creation cannot race.
#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    app: AppHandle,
    title: String,
    shoot_date: Option<String>,
) -> Result<Project> {
    let now = now_rfc3339()?;
    let project = state
        .db
        .call(move |conn| {
            let root = require_mounted(conn)?;
            let projects_root = Path::new(&root).join("Projects");
            let target_nle = db::settings::get(conn, "default_nle")?.unwrap_or_default();
            create_project_inner(
                conn,
                &projects_root,
                &title,
                shoot_date.as_deref(),
                &target_nle,
                &now,
            )
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    Ok(project)
}

/// Move a project through the status vocabulary. Writes both the manifest
/// (atomic) and the row — folders are truth — then touches the row, records an
/// event, and broadcasts.
#[tauri::command]
#[specta::specta]
pub async fn set_project_status(
    state: State<'_, AppState>,
    app: AppHandle,
    slug: String,
    status: String,
) -> Result<()> {
    let now = now_rfc3339()?;
    state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            let project = db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {slug}")))?;
            let dir = Path::new(&project.root_path);
            let mut manifest = read_manifest(dir)?;
            manifest.status = status.clone();
            write_manifest(dir, &manifest)?;
            db::projects::set_status(conn, &slug, &status)?;
            db::projects::touch(conn, &slug, &now)?;
            db::events::record(conn, "project-status-changed", Some(&slug), None)?;
            Ok(())
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    Ok(())
}

/// Set a project's priority. Writes both the manifest (atomic) and the row —
/// folders are truth — then touches the row, records an event, and broadcasts.
/// `PriorityLevel` is an enum, so an out-of-vocabulary value is rejected at the
/// IPC boundary and never reaches the manifest.
#[tauri::command]
#[specta::specta]
pub async fn set_project_priority(
    state: State<'_, AppState>,
    app: AppHandle,
    slug: String,
    priority: PriorityLevel,
) -> Result<()> {
    let now = now_rfc3339()?;
    state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            let project = db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {slug}")))?;
            let dir = Path::new(&project.root_path);
            let mut manifest = read_manifest(dir)?;
            manifest.priority = Some(priority.as_str().to_string());
            write_manifest(dir, &manifest)?;
            db::projects::set_priority(conn, &slug, &priority)?;
            db::projects::touch(conn, &slug, &now)?;
            db::events::record(conn, "project-priority-changed", Some(&slug), None)?;
            Ok(())
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    Ok(())
}

/// Set (or clear) a project's shoot and publish dates. Writes the manifest
/// (atomic) and the row, touches, records an event, and broadcasts.
#[tauri::command]
#[specta::specta]
pub async fn set_project_dates(
    state: State<'_, AppState>,
    app: AppHandle,
    slug: String,
    shoot: Option<String>,
    publish: Option<String>,
) -> Result<()> {
    let now = now_rfc3339()?;
    state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            let project = db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {slug}")))?;
            let dir = Path::new(&project.root_path);
            let mut manifest = read_manifest(dir)?;
            manifest.shoot_date = shoot.clone();
            manifest.publish_date = publish.clone();
            write_manifest(dir, &manifest)?;
            db::projects::set_dates(conn, &slug, shoot.as_deref(), publish.as_deref())?;
            db::projects::touch(conn, &slug, &now)?;
            db::events::record(conn, "project-dates-changed", Some(&slug), None)?;
            Ok(())
        })
        .await?;
    crate::broadcast::projects_changed(&app);
    Ok(())
}

/// Reveal a project folder (or one of its D6 subfolders) in Finder. `subfolder`
/// is validated against the anatomy allowlist so it can never escape the project
/// directory.
#[tauri::command]
#[specta::specta]
pub async fn reveal_project_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    subfolder: Option<String>,
) -> Result<()> {
    if let Some(sub) = &subfolder
        && !PROJECT_SUBFOLDERS.contains(&sub.as_str())
    {
        return Err(Error::Io(format!("unknown project subfolder: {sub}")));
    }
    let project = state
        .db
        .call(move |conn| {
            require_mounted(conn)?;
            db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Io(format!("no such project: {slug}")))
        })
        .await?;

    let mut path = PathBuf::from(&project.root_path);
    if let Some(sub) = subfolder {
        path.push(sub);
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(())
}

/// Resolve the configured studio root, failing with `StudioRootUnmounted` when it
/// is unset or unreachable. Every folder-touching command guards through here
/// before walking the filesystem.
fn require_mounted(conn: &Connection) -> Result<String> {
    let root = db::settings::get(conn, "studio_root")?
        .ok_or_else(|| Error::StudioRootUnmounted("no studio root configured".to_string()))?;
    if !paths::root_mounted(Path::new(&root)) {
        return Err(Error::StudioRootUnmounted(format!(
            "studio root is not mounted: {root}"
        )));
    }
    Ok(root)
}

/// The current instant as a second-precision UTC RFC3339 string
/// (`YYYY-MM-DDTHH:MM:SSZ`), matching the events log's precision.
fn now_rfc3339() -> Result<String> {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .map_err(|e| Error::Io(e.to_string()))?
        .format(&Rfc3339)
        .map_err(|e| Error::Io(e.to_string()))
}

/// The folder + DB half of `create_project`, factored out so it can be tested
/// against a tempdir and an in-memory DB without a live Tauri app. Runs entirely
/// on the writer thread: the slug dedupe (`slug_exists`) and the row insert share
/// one connection, so a concurrent create cannot claim the same slug.
fn create_project_inner(
    conn: &Connection,
    projects_root: &Path,
    title: &str,
    shoot_date: Option<&str>,
    target_nle: &str,
    now: &str,
) -> Result<Project> {
    std::fs::create_dir_all(projects_root)?;
    let date = &now[..10];
    let slug = project_slug(title, date, |candidate| {
        db::projects::slug_exists(conn, candidate).unwrap_or(false)
    });

    let manifest = ProjectManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        slug: slug.clone(),
        title: title.to_string(),
        status: "idea".to_string(),
        target_nle: target_nle.to_string(),
        priority: None,
        shoot_date: shoot_date.map(str::to_string),
        publish_date: None,
        created_at: now.to_string(),
        links: serde_json::Map::new(),
    };
    let project_dir = create_project_skeleton(projects_root, &manifest)?;

    let project = Project {
        slug: slug.clone(),
        title: title.to_string(),
        root_path: project_dir.to_string_lossy().into_owned(),
        status: "idea".to_string(),
        target_nle: target_nle.to_string(),
        priority: "none".to_string(),
        shoot_date: shoot_date.map(str::to_string),
        publish_date: None,
        created_at: now.to_string(),
        last_touched_at: Some(now.to_string()),
    };
    db::projects::insert(conn, &project)?;
    db::events::record(conn, "project-created", Some(&slug), None)?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    #[test]
    fn create_project_inner_creates_folder_row_and_event() {
        let conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");

        let project = create_project_inner(
            &conn,
            &projects_root,
            "NVMe Deep Dive",
            Some("2026-08-01"),
            "resolve",
            "2026-07-09T10:00:00Z",
        )
        .unwrap();

        assert_eq!(project.slug, "nvme-deep-dive-2026-07-09");
        let project_dir = projects_root.join(&project.slug);
        assert!(project_dir.join("footage").is_dir());

        let manifest = read_manifest(&project_dir).unwrap();
        assert_eq!(manifest.title, "NVMe Deep Dive");
        assert_eq!(manifest.status, "idea");
        assert_eq!(manifest.shoot_date.as_deref(), Some("2026-08-01"));

        let row = db::projects::get(&conn, &project.slug).unwrap().unwrap();
        assert_eq!(row.status, "idea");
        assert_eq!(row.last_touched_at.as_deref(), Some("2026-07-09T10:00:00Z"));

        let events = db::events::list(&conn, 10, None).unwrap();
        assert!(
            events.iter().any(|e| e.kind == "project-created"
                && e.project_slug.as_deref() == Some(project.slug.as_str())),
            "a project-created event must be recorded"
        );
    }

    #[test]
    fn create_project_inner_dedupes_same_title_same_day() {
        let conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");

        let first = create_project_inner(
            &conn,
            &projects_root,
            "NVMe Deep Dive",
            None,
            "resolve",
            "2026-07-09T10:00:00Z",
        )
        .unwrap();
        let second = create_project_inner(
            &conn,
            &projects_root,
            "NVMe Deep Dive",
            None,
            "resolve",
            "2026-07-09T11:00:00Z",
        )
        .unwrap();

        assert_eq!(first.slug, "nvme-deep-dive-2026-07-09");
        assert_eq!(second.slug, "nvme-deep-dive-2-2026-07-09");
        assert!(projects_root.join(&second.slug).is_dir());
    }

    #[test]
    fn now_rfc3339_is_second_precision_utc() {
        let now = now_rfc3339().unwrap();
        assert!(now.ends_with('Z'), "got {now}");
        assert!(!now.contains('.'), "expected seconds precision, got {now}");
        assert_eq!(now.len(), 20, "YYYY-MM-DDTHH:MM:SSZ, got {now}");
    }
}
