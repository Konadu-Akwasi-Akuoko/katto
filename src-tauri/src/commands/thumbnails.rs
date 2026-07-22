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

/// A project's newest exported thumbnail PNG. `mtime_ms` exists so the
/// frontend can cache-bust `convertFileSrc` — a re-export over the same
/// filename must show the new bytes.
#[derive(serde::Serialize, specta::Type)]
pub struct LatestThumb {
    pub slug: String,
    pub path: String,
    pub mtime_ms: f64,
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

fn newest_png_in(dir: &std::path::Path) -> Option<(String, f64)> {
    let entries: Vec<(String, std::time::SystemTime)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((name, mtime))
        })
        .collect();
    let name = naming::newest_png(&entries)?;
    let mtime_ms = entries
        .iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, t)| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    Some((dir.join(name).to_string_lossy().into_owned(), mtime_ms))
}

#[tauri::command]
#[specta::specta]
pub async fn latest_thumbnail(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
) -> Result<Option<LatestThumb>> {
    let dir = project_dir(&state, &slug).await?.join("thumbnails");
    let found = tauri::async_runtime::spawn_blocking(move || newest_png_in(&dir))
        .await
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(found.map(|(path, mtime_ms)| {
        // per-use asset grant (no blanket studio-root grant — see assets.rs)
        crate::assets::allow_source_file(&app, std::path::Path::new(&path));
        LatestThumb {
            slug,
            path,
            mtime_ms,
        }
    }))
}

#[tauri::command]
#[specta::specta]
pub async fn list_latest_thumbnails(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<LatestThumb>> {
    let projects = state
        .db
        .call(|conn| crate::db::projects::list(conn))
        .await?;
    let thumbs: Vec<LatestThumb> = tauri::async_runtime::spawn_blocking(move || {
        projects
            .into_iter()
            .filter_map(|project| {
                let dir = std::path::Path::new(&project.root_path).join("thumbnails");
                // unreadable/missing dirs skip silently — reconcile owns folder truth
                let (path, mtime_ms) = newest_png_in(&dir)?;
                Some(LatestThumb {
                    slug: project.slug,
                    path,
                    mtime_ms,
                })
            })
            .collect()
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?;
    for thumb in &thumbs {
        // per-use asset grant (no blanket studio-root grant — see assets.rs)
        crate::assets::allow_source_file(&app, std::path::Path::new(&thumb.path));
    }
    Ok(thumbs)
}

#[tauri::command]
#[specta::specta]
pub async fn watch_thumbnails(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
) -> Result<()> {
    let dir = project_dir(&state, &slug).await?.join("thumbnails");
    let mkdir = dir.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::create_dir_all(&mkdir))
        .await
        .map_err(|e| Error::Io(e.to_string()))??;
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
