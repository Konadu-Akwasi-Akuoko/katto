//! Thumbnail commands: scaffold a template PSD, open it (Photoshop or
//! Finder), report newest exported PNGs, and manage the single folder watch.
//! Thin shells — naming/psd logic is pure and tested in `thumbnails/`.

use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::error::{Error, Result};
use crate::state::AppState;
use crate::thumbnails::{self, ThumbFormat, ThumbOpen, naming};

/// What `create_thumbnail` produced and how it opened.
#[derive(serde::Serialize, specta::Type)]
pub struct CreateThumbnailResult {
    pub psd_path: String,
    pub opened: ThumbOpen,
}

/// A project's newest exported thumbnail PNG.
#[derive(serde::Serialize, specta::Type)]
pub struct LatestThumb {
    pub slug: String,
    pub path: String,
}

async fn project_dir(state: &State<'_, AppState>, slug: &str) -> Result<PathBuf> {
    let lookup = slug.to_string();
    let project = state
        .db
        .call(move |conn| crate::db::projects::get(conn, &lookup))
        .await?
        .ok_or_else(|| Error::NoSuchProject(format!("no project {slug}")))?;
    Ok(PathBuf::from(project.root_path))
}

#[tauri::command]
#[specta::specta]
pub async fn create_thumbnail(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    format: ThumbFormat,
) -> Result<CreateThumbnailResult> {
    let dir = project_dir(&state, &slug).await?;
    let scaffold_app = app.clone();
    let scaffold_slug = slug.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        thumbnails::scaffold(&scaffold_app, &dir, &scaffold_slug, format)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))??;
    let open_app = app.clone();
    let open_path = path.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || {
        thumbnails::open_scaffold(&open_app, &open_path)
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?;
    let payload = serde_json::json!({ "slug": slug, "path": path.to_string_lossy() }).to_string();
    let event_slug = slug.clone();
    let _ = state
        .db
        .call(move |conn| {
            crate::db::events::record(
                conn,
                "thumbnail_scaffolded",
                Some(&event_slug),
                Some(&payload),
            )
        })
        .await;
    crate::broadcast::events_appended(&app);
    Ok(CreateThumbnailResult {
        psd_path: path.to_string_lossy().into_owned(),
        opened,
    })
}

fn newest_png_in(dir: &std::path::Path) -> Option<String> {
    let entries: Vec<(String, std::time::SystemTime)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((name, mtime))
        })
        .collect();
    naming::newest_png(&entries).map(|name| dir.join(name).to_string_lossy().into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn latest_thumbnail(state: State<'_, AppState>, slug: String) -> Result<Option<String>> {
    let dir = project_dir(&state, &slug).await?.join("thumbnails");
    tauri::async_runtime::spawn_blocking(move || newest_png_in(&dir))
        .await
        .map_err(|e| Error::Io(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn list_latest_thumbnails(state: State<'_, AppState>) -> Result<Vec<LatestThumb>> {
    let projects = state
        .db
        .call(|conn| crate::db::projects::list(conn))
        .await?;
    tauri::async_runtime::spawn_blocking(move || {
        projects
            .into_iter()
            .filter_map(|project| {
                let dir = std::path::Path::new(&project.root_path).join("thumbnails");
                // unreadable/missing dirs skip silently — reconcile owns folder truth
                let path = newest_png_in(&dir)?;
                Some(LatestThumb {
                    slug: project.slug,
                    path,
                })
            })
            .collect()
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn watch_thumbnails(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
) -> Result<()> {
    let dir = project_dir(&state, &slug).await?.join("thumbnails");
    std::fs::create_dir_all(&dir)?;
    let watch = thumbnails::watch::start(app, slug, dir)?;
    let mut slot = state
        .thumb_watch
        .lock()
        .map_err(|_| Error::Io("thumbnail watch lock poisoned".into()))?;
    *slot = Some(watch);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn unwatch_thumbnails(state: State<'_, AppState>) -> Result<()> {
    let mut slot = state
        .thumb_watch
        .lock()
        .map_err(|_| Error::Io("thumbnail watch lock poisoned".into()))?;
    *slot = None;
    Ok(())
}
