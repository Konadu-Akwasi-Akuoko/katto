//! Browser surface commands: thin shells over the tab host, the download
//! registry, and the filing job. Address normalization lives in the frontend
//! (`address.ts`); these pass URLs through.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::browser::host::BrowserRect;
use crate::browser::tabs::{BrowserState, TabId};
use crate::error::{Error, Result};
use crate::state::AppState;

const DEFAULT_TAB_URL: &str = "https://elements.envato.com/";

#[tauri::command]
#[specta::specta]
pub async fn browser_open_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    url: Option<String>,
) -> Result<TabId> {
    let url = url.unwrap_or_else(|| DEFAULT_TAB_URL.to_string());
    state.browser.open_tab(&app, &url)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: TabId,
) -> Result<()> {
    state.browser.close_tab(&app, tab_id)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_select_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: TabId,
) -> Result<()> {
    state.browser.select_tab(&app, tab_id)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_navigate(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: TabId,
    url: String,
) -> Result<()> {
    state.browser.navigate(&app, tab_id, &url)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_go(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: TabId,
    delta: i32,
) -> Result<()> {
    state.browser.go(&app, tab_id, delta)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_state(state: State<'_, AppState>) -> Result<BrowserState> {
    Ok(state.browser.state())
}

#[tauri::command]
#[specta::specta]
pub async fn browser_set_bounds(
    app: AppHandle,
    state: State<'_, AppState>,
    rect: BrowserRect,
) -> Result<()> {
    state.browser.set_bounds(&app, rect)
}

#[tauri::command]
#[specta::specta]
pub async fn browser_set_visible(
    app: AppHandle,
    state: State<'_, AppState>,
    visible: bool,
) -> Result<()> {
    state.browser.set_visible(&app, visible)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_asset_project(
    state: State<'_, AppState>,
    slug: Option<String>,
) -> Result<()> {
    let mut active = state
        .active_asset_project
        .lock()
        .map_err(|_| Error::BrowserUnavailable("active-project lock poisoned".into()))?;
    *active = slug;
    Ok(())
}

/// The current filing target: the explicit override, else the most recently
/// touched project.
#[tauri::command]
#[specta::specta]
pub async fn active_asset_project(state: State<'_, AppState>) -> Result<Option<String>> {
    let override_slug = state
        .active_asset_project
        .lock()
        .map_err(|_| Error::BrowserUnavailable("active-project lock poisoned".into()))?
        .clone();
    if override_slug.is_some() {
        return Ok(override_slug);
    }
    let derived = state
        .db
        .call(|conn| crate::db::projects::most_recently_touched(conn))
        .await?;
    Ok(derived.map(|p| p.slug))
}

/// A parked download, listed for the pick-a-project sheet.
#[derive(serde::Serialize, specta::Type)]
pub struct ParkedDownload {
    pub id: String,
    pub filename: String,
}

#[tauri::command]
#[specta::specta]
pub async fn parked_downloads(state: State<'_, AppState>) -> Result<Vec<ParkedDownload>> {
    Ok(state
        .downloads
        .parked()
        .into_iter()
        .map(|p| ParkedDownload {
            id: p.id,
            filename: p.filename,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn file_parked_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
    slug: String,
) -> Result<()> {
    let pending = state
        .downloads
        .take_parked(&download_id)
        .ok_or(Error::DownloadMissing(download_id))?;
    crate::jobs::download::spawn_filing(&app, pending, slug);
    Ok(())
}

/// Reveal a filed asset in Finder. `rel_path` is project-relative; the join
/// is containment-checked against the project folder so a crafted path can
/// never escape the studio root.
#[tauri::command]
#[specta::specta]
pub async fn reveal_in_project(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    rel_path: String,
) -> Result<()> {
    let root = state
        .db
        .call(|conn| crate::db::settings::get(conn, "studio_root"))
        .await?
        .ok_or_else(|| Error::Onboarding("no studio root configured".to_string()))?;
    let project_dir = Path::new(&root).join("Projects").join(&slug);
    let target =
        tauri::async_runtime::spawn_blocking(move || contained_path(&project_dir, &rel_path))
            .await
            .map_err(|e| Error::Io(e.to_string()))??;
    app.opener()
        .reveal_item_in_dir(target)
        .map_err(|e| Error::Io(e.to_string()))
}

/// Canonicalize `<project_dir>/<rel>` and require it to stay inside the
/// canonical project dir.
fn contained_path(project_dir: &Path, rel: &str) -> Result<PathBuf> {
    let canonical_project = project_dir
        .canonicalize()
        .map_err(|e| Error::Io(format!("project {}: {e}", project_dir.display())))?;
    let target = project_dir.join(rel);
    let canonical = target
        .canonicalize()
        .map_err(|e| Error::Io(format!("asset {}: {e}", target.display())))?;
    if !canonical.starts_with(&canonical_project) {
        return Err(Error::Io(format!("path escapes the project folder: {rel}")));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contained_path_refuses_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(project.join("assets")).unwrap();
        std::fs::write(project.join("assets/a.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("outside.txt"), b"x").unwrap();

        let ok = contained_path(&project, "assets/a.txt").unwrap();
        assert!(ok.ends_with("assets/a.txt"));
        assert!(contained_path(&project, "../outside.txt").is_err());
    }
}
