use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::db;
use crate::db::ideas::Idea;
use crate::db::projects::Project;
use crate::error::{Error, Result};
use crate::paths;
use crate::projects::anatomy::{create_project_skeleton, remove_skeleton};
use crate::projects::manifest::{MANIFEST_SCHEMA_VERSION, ProjectManifest};
use crate::projects::project_slug;
use crate::state::AppState;

/// Fields for a manually captured idea. Everything else is defaulted server-side
/// (`type='manual'`, `status='backlog'`, a fresh id, and the capture timestamp).
#[derive(Debug, Deserialize, specta::Type)]
pub struct IdeaCreate {
    pub title: String,
    pub kind: Option<String>,
    pub notes: Option<String>,
}

/// A partial edit of an idea; a `None` field leaves that column unchanged.
#[derive(Debug, Deserialize, specta::Type)]
pub struct IdeaPatch {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub notes: Option<String>,
}

/// The slug of the project an idea was promoted into.
#[derive(Debug, Serialize, specta::Type)]
pub struct PromoteResult {
    pub slug: String,
}

/// Ideas with the given status, newest-first.
#[tauri::command]
#[specta::specta]
pub async fn list_ideas(state: State<'_, AppState>, status: String) -> Result<Vec<Idea>> {
    state
        .db
        .call(move |conn| db::ideas::list_by_status(conn, &status))
        .await
}

/// Capture a new idea into the backlog and broadcast.
#[tauri::command]
#[specta::specta]
pub async fn create_idea(
    state: State<'_, AppState>,
    app: AppHandle,
    input: IdeaCreate,
) -> Result<Idea> {
    let now = now_rfc3339()?;
    let idea = state
        .db
        .call(move |conn| create_idea_inner(conn, input, &now))
        .await?;
    crate::broadcast::ideas_changed(&app);
    Ok(idea)
}

/// Patch an idea's editable fields and return the updated row.
#[tauri::command]
#[specta::specta]
pub async fn update_idea(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
    patch: IdeaPatch,
) -> Result<Idea> {
    let idea = state
        .db
        .call(move |conn| update_idea_inner(conn, &id, patch))
        .await?;
    crate::broadcast::ideas_changed(&app);
    Ok(idea)
}

/// Discard an idea: it moves to `status='discarded'` and the row is retained as
/// an audit trail (never deleted).
#[tauri::command]
#[specta::specta]
pub async fn discard_idea(state: State<'_, AppState>, app: AppHandle, id: String) -> Result<()> {
    state
        .db
        .call(move |conn| db::ideas::set_status(conn, &id, "discarded"))
        .await?;
    crate::broadcast::ideas_changed(&app);
    Ok(())
}

/// Promote a backlog idea into a real project folder in one motion. All the DB
/// work runs inside a single rusqlite transaction on the writer thread; the
/// folder skeleton is filesystem state the transaction cannot cover, so a
/// DB-side failure rolls the rows back and then explicitly removes the folder.
/// On success the idea stays as a `promoted` audit row pointing at the new slug.
#[tauri::command]
#[specta::specta]
pub async fn promote_idea(
    state: State<'_, AppState>,
    app: AppHandle,
    id: String,
) -> Result<PromoteResult> {
    let now = now_rfc3339()?;
    let slug = state
        .db
        .call(move |conn| {
            let root = require_mounted(conn)?;
            let projects_root = Path::new(&root).join("Projects");
            let default_nle = db::settings::get(conn, "default_nle")?.unwrap_or_default();
            promote_inner(conn, &projects_root, &default_nle, &id, &now)
        })
        .await?;
    crate::broadcast::ideas_changed(&app);
    crate::broadcast::projects_changed(&app);
    Ok(PromoteResult { slug })
}

/// Build and insert a fresh backlog idea. Factored out so it can be tested
/// against an in-memory DB without a live Tauri app.
fn create_idea_inner(conn: &Connection, input: IdeaCreate, now: &str) -> Result<Idea> {
    let idea = Idea {
        id: uuid::Uuid::new_v4().to_string(),
        r#type: "manual".to_string(),
        kind: input.kind.unwrap_or_else(|| "unset".to_string()),
        status: "backlog".to_string(),
        title: input.title,
        rationale: None,
        source: None,
        source_url: None,
        source_title: None,
        evidence_json: None,
        raw_signal_id: None,
        first_seen: now.to_string(),
        notes: input.notes,
        promoted_slug: None,
        kind_source: None,
        kind_why: None,
    };
    db::ideas::create(conn, &idea)?;
    Ok(idea)
}

/// Apply a patch and return the resulting row.
fn update_idea_inner(conn: &Connection, id: &str, patch: IdeaPatch) -> Result<Idea> {
    db::ideas::update(
        conn,
        id,
        patch.title.as_deref(),
        patch.kind.as_deref(),
        patch.notes.as_deref(),
    )?;
    db::ideas::get(conn, id)?.ok_or_else(|| Error::Io(format!("no such idea: {id}")))
}

/// The transactional promote, delegating the in-transaction row writes to
/// [`promote_writes`]. See [`promote_with_writes`] for why the writes are a
/// separate injectable step.
fn promote_inner(
    conn: &mut Connection,
    projects_root: &Path,
    default_nle: &str,
    id: &str,
    now: &str,
) -> Result<String> {
    promote_with_writes(conn, projects_root, default_nle, id, now, promote_writes)
}

/// The three in-transaction row writes of a promote: insert the project row,
/// flip the idea to `promoted`, and log the `idea-promoted` event.
fn promote_writes(conn: &Connection, project: &Project, idea_id: &str) -> Result<()> {
    db::projects::insert(conn, project)?;
    db::ideas::mark_promoted(conn, idea_id, &project.slug)?;
    db::events::record(conn, "idea-promoted", Some(&project.slug), None)?;
    Ok(())
}

/// Promote a backlog idea, running `writes` for the in-transaction row mutations.
///
/// The `writes` step is injectable because the promote's own slug dedupe reads
/// the projects table inside the same transaction, so a natural `UNIQUE` slug
/// collision on the row insert is impossible to force through the public path —
/// the dedupe always routes around a taken slug. Tests substitute a `writes` that
/// performs the real insert and then errors, exercising both the SQL rollback and
/// the filesystem-folder cleanup that the transaction cannot cover.
fn promote_with_writes<W>(
    conn: &mut Connection,
    projects_root: &Path,
    default_nle: &str,
    id: &str,
    now: &str,
    writes: W,
) -> Result<String>
where
    W: FnOnce(&Connection, &Project, &str) -> Result<()>,
{
    let tx = conn.transaction()?;

    let idea = db::ideas::get(&tx, id)?
        .ok_or_else(|| Error::promote_failed("load", &format!("no such idea: {id}")))?;
    if idea.status != "backlog" {
        // Dropping `tx` here rolls back (nothing was written) and no folder exists yet.
        return Err(Error::promote_failed(
            "load",
            &format!(
                "idea {id} is '{}' (only backlog ideas promote)",
                idea.status
            ),
        ));
    }

    let date = &now[..10];
    let slug = project_slug(&idea.title, date, |candidate| {
        db::projects::slug_exists(&tx, candidate).unwrap_or(false)
    });

    let manifest = ProjectManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        slug: slug.clone(),
        title: idea.title.clone(),
        status: "idea".to_string(),
        target_nle: default_nle.to_string(),
        shoot_date: None,
        publish_date: None,
        created_at: now.to_string(),
        links: serde_json::Map::new(),
    };
    std::fs::create_dir_all(projects_root)?;
    // If skeleton creation fails the transaction is not committed and no folder
    // was produced, so there is nothing to clean up.
    let project_dir = create_project_skeleton(projects_root, &manifest)?;

    let project = Project {
        slug: slug.clone(),
        title: idea.title.clone(),
        root_path: project_dir.to_string_lossy().into_owned(),
        status: "idea".to_string(),
        target_nle: default_nle.to_string(),
        shoot_date: None,
        publish_date: None,
        created_at: now.to_string(),
        last_touched_at: Some(now.to_string()),
    };

    match writes(&tx, &project, id) {
        Ok(()) => {
            tx.commit()?;
            Ok(slug)
        }
        Err(e) => {
            // Roll the rows back (drop), then clean the folder the tx never covered.
            drop(tx);
            remove_skeleton(&project_dir)?;
            Err(Error::promote_failed("insert", &e.to_string()))
        }
    }
}

/// Resolve the configured studio root, failing with `StudioRootUnmounted` when it
/// is unset or unreachable. Promote touches the filesystem, so it guards the mount
/// before laying down a folder.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_db;

    fn seed_backlog(conn: &Connection, title: &str) -> Idea {
        create_idea_inner(
            conn,
            IdeaCreate {
                title: title.to_string(),
                kind: None,
                notes: None,
            },
            "2026-07-09T10:00:00Z",
        )
        .unwrap()
    }

    #[test]
    fn create_idea_inner_defaults_type_status_and_kind() {
        let conn = test_db();
        let idea = create_idea_inner(
            &conn,
            IdeaCreate {
                title: "NVMe Deep Dive".to_string(),
                kind: None,
                notes: Some("a note".to_string()),
            },
            "2026-07-09T10:00:00Z",
        )
        .unwrap();

        assert_eq!(idea.r#type, "manual");
        assert_eq!(idea.status, "backlog");
        assert_eq!(idea.kind, "unset");
        assert_eq!(idea.first_seen, "2026-07-09T10:00:00Z");
        assert!(!idea.id.is_empty());
        assert_eq!(idea.notes.as_deref(), Some("a note"));

        let got = db::ideas::get(&conn, &idea.id).unwrap().unwrap();
        assert_eq!(got.title, "NVMe Deep Dive");
    }

    #[test]
    fn update_idea_inner_returns_patched_row() {
        let conn = test_db();
        let idea = seed_backlog(&conn, "Original");
        let patched = update_idea_inner(
            &conn,
            &idea.id,
            IdeaPatch {
                title: Some("Renamed".to_string()),
                kind: Some("long".to_string()),
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(patched.title, "Renamed");
        assert_eq!(patched.kind, "long");
    }

    #[test]
    fn promote_inner_creates_project_folder_row_and_event() {
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        let idea = seed_backlog(&conn, "NVMe Deep Dive");

        let slug = promote_inner(
            &mut conn,
            &projects_root,
            "resolve",
            &idea.id,
            "2026-07-09T10:00:00Z",
        )
        .unwrap();

        assert_eq!(slug, "nvme-deep-dive-2026-07-09");
        assert!(projects_root.join(&slug).join("footage").is_dir());

        let row = db::projects::get(&conn, &slug).unwrap().unwrap();
        assert_eq!(row.title, "NVMe Deep Dive");
        assert_eq!(row.status, "idea");
        assert_eq!(row.last_touched_at.as_deref(), Some("2026-07-09T10:00:00Z"));

        let got = db::ideas::get(&conn, &idea.id).unwrap().unwrap();
        assert_eq!(got.status, "promoted");
        assert_eq!(got.promoted_slug.as_deref(), Some(slug.as_str()));

        let events = db::events::list(&conn, 10, None).unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.kind == "idea-promoted" && e.project_slug.as_deref() == Some(&slug)),
            "an idea-promoted event must be recorded"
        );
    }

    #[test]
    fn promote_inner_rejects_non_backlog_idea() {
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        let idea = seed_backlog(&conn, "Already Gone");
        db::ideas::set_status(&conn, &idea.id, "discarded").unwrap();

        let err = promote_inner(
            &mut conn,
            &projects_root,
            "resolve",
            &idea.id,
            "2026-07-09T10:00:00Z",
        )
        .unwrap_err();

        assert!(matches!(err, Error::PromoteFailed(_)));
        assert!(
            !projects_root.exists(),
            "no folder should be created for a rejected promote"
        );
    }

    #[test]
    fn promote_rolls_back_rows_and_removes_folder_on_write_failure() {
        let mut conn = test_db();
        let root = tempfile::tempdir().unwrap();
        let projects_root = root.path().join("Projects");
        let idea = seed_backlog(&conn, "NVMe Deep Dive");

        // The injected writes perform the real project insert (so the row exists
        // inside the transaction) and then error, forcing the rollback path.
        let err = promote_with_writes(
            &mut conn,
            &projects_root,
            "resolve",
            &idea.id,
            "2026-07-09T10:00:00Z",
            |tx, project, _idea_id| {
                db::projects::insert(tx, project)?;
                Err(Error::Db("simulated failure after insert".to_string()))
            },
        )
        .unwrap_err();

        assert!(matches!(err, Error::PromoteFailed(_)));

        // The SQL transaction rolled the inserted project row back.
        assert_eq!(
            db::projects::get(&conn, "nvme-deep-dive-2026-07-09").unwrap(),
            None,
            "the project row must be rolled back"
        );
        // The folder skeleton the transaction could not cover was removed.
        assert!(
            !projects_root.join("nvme-deep-dive-2026-07-09").exists(),
            "the folder skeleton must be cleaned up on rollback"
        );
        // The idea stays in the backlog, ready to retry.
        let got = db::ideas::get(&conn, &idea.id).unwrap().unwrap();
        assert_eq!(got.status, "backlog");
    }
}
