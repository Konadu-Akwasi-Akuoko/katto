//! Webview glue for the in-app browser: child webviews attached to the main
//! window (tauri `unstable` multi-webview), download interception into the
//! app-data staging dir, and the trait both host impls share. All tab/history
//! decisions live in the pure [`TabModel`]; this module only translates them
//! into webview calls — thin spawn/glue, no unit tests beyond what's pure.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, Webview, WebviewUrl, Wry};

use crate::browser::downloads::{PendingDownload, safe_filename};
use crate::browser::tabs::{BrowserState, TabId, TabModel, allow_navigation};
use crate::error::{Error, Result};

/// CSS-pixel rect of the browser surface's content area, reported by React.
/// Child webview bounds are relative to the window content area the main
/// webview also fills, so these map 1:1 to logical position/size.
#[derive(Clone, Copy, Debug, serde::Deserialize, specta::Type)]
pub struct BrowserRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The host contract both webview strategies implement. Commands call these;
/// every mutation broadcasts `BrowserStateChanged`.
pub trait BrowserTabHost: Send + Sync {
    fn open_tab(&self, app: &AppHandle, url: &str) -> Result<TabId>;
    fn close_tab(&self, app: &AppHandle, id: TabId) -> Result<()>;
    fn select_tab(&self, app: &AppHandle, id: TabId) -> Result<()>;
    fn navigate(&self, app: &AppHandle, id: TabId, url: &str) -> Result<()>;
    fn go(&self, app: &AppHandle, id: TabId, delta: i32) -> Result<()>;
    fn set_bounds(&self, app: &AppHandle, rect: BrowserRect) -> Result<()>;
    fn set_visible(&self, app: &AppHandle, visible: bool) -> Result<()>;
    fn state(&self) -> BrowserState;
    /// The main window was destroyed (close-to-tray): forget webview handles,
    /// keep the model so tabs survive recreation.
    fn on_window_destroyed(&self);
    /// A committed page load reported by a webview.
    fn record_navigated(&self, app: &AppHandle, label: &str, url: String);
    /// WKWebView reported a document title for a webview.
    fn record_title(&self, app: &AppHandle, label: &str, title: String);
    /// The page URL a download originated from, for the license sidecar.
    fn page_url_for_label(&self, label: &str) -> String;
}

/// Registry of downloads between `Requested` (destination assigned) and
/// `Finished` (filing spawned), plus downloads parked for a project pick.
/// `Finished` only carries the URL (macOS never reports the path), so
/// duplicate concurrent downloads of one URL queue FIFO: each begin pushes,
/// each finish pops the oldest — never overwriting a still-downloading
/// entry with a later one (that filed partial files into projects).
#[derive(Default)]
pub struct DownloadRegistry {
    in_flight: Mutex<HashMap<String, Vec<PendingDownload>>>,
    parked: Mutex<Vec<PendingDownload>>,
}

impl DownloadRegistry {
    pub fn begin(&self, pending: PendingDownload) {
        if let Ok(mut map) = self.in_flight.lock() {
            map.entry(pending.url.clone()).or_default().push(pending);
        }
    }

    pub fn finish(&self, url: &str) -> Option<PendingDownload> {
        let mut map = self.in_flight.lock().ok()?;
        let list = map.get_mut(url)?;
        let pending = if list.is_empty() {
            None
        } else {
            Some(list.remove(0))
        };
        if list.is_empty() {
            map.remove(url);
        }
        pending
    }

    pub fn park(&self, pending: PendingDownload) {
        if let Ok(mut parked) = self.parked.lock() {
            parked.push(pending);
        }
    }

    pub fn take_parked(&self, id: &str) -> Option<PendingDownload> {
        let mut parked = self.parked.lock().ok()?;
        let idx = parked.iter().position(|p| p.id == id)?;
        Some(parked.remove(idx))
    }

    pub fn parked(&self) -> Vec<PendingDownload> {
        self.parked.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

/// `<app_data_dir>/browser/staging`, created on demand. Downloads land here
/// first so no partial file ever appears inside a project folder.
pub fn staging_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Io(e.to_string()))?
        .join("browser/staging");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Best-effort removal of staging entries older than 24 h. The registry is
/// in-memory, so anything still here after a relaunch is unfileable; each
/// swept dir gets a `download_failed` events row so nothing vanishes silently.
pub fn sweep_staging(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(dir) = staging_dir(&app) else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 60 * 60);
        let mut swept = Vec::new();
        for entry in entries.flatten() {
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|m| m < cutoff)
                .unwrap_or(false);
            if stale && std::fs::remove_dir_all(entry.path()).is_ok() {
                swept.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        if swept.is_empty() {
            return;
        }
        let state = app.state::<crate::state::AppState>();
        let payload =
            serde_json::json!({ "reason": "stale staging", "entries": swept }).to_string();
        let _ = state
            .db
            .call(move |conn| {
                crate::db::events::record(conn, "download_failed", None, Some(&payload))
            })
            .await;
        crate::broadcast::events_appended(&app);
    });
}

/// Shared internals: the pure model plus webview bookkeeping.
struct HostInner {
    model: Mutex<TabModel>,
    /// TabId -> webview label ("browser-tab-<id>"; the single host uses one
    /// label for whichever tab is active).
    webviews: Mutex<HashMap<TabId, String>>,
    /// Observed document titles overlaying the URL-derived fallback.
    titles: Mutex<HashMap<TabId, String>>,
    bounds: Mutex<Option<BrowserRect>>,
    visible: AtomicBool,
}

impl HostInner {
    fn new() -> Self {
        Self {
            model: Mutex::new(TabModel::new()),
            webviews: Mutex::new(HashMap::new()),
            titles: Mutex::new(HashMap::new()),
            bounds: Mutex::new(None),
            visible: AtomicBool::new(false),
        }
    }

    fn model<T>(&self, f: impl FnOnce(&mut TabModel) -> T) -> Result<T> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| Error::BrowserUnavailable("tab model poisoned".into()))?;
        Ok(f(&mut model))
    }

    fn snapshot(&self) -> BrowserState {
        let mut snap = self
            .model
            .lock()
            .map(|m| m.snapshot())
            .unwrap_or(BrowserState {
                tabs: Vec::new(),
                active: None,
            });
        if let Ok(titles) = self.titles.lock() {
            for tab in &mut snap.tabs {
                if let Some(title) = titles.get(&tab.id)
                    && !title.trim().is_empty()
                {
                    tab.title = title.clone();
                }
            }
        }
        snap
    }

    fn current_rect(&self) -> BrowserRect {
        self.bounds
            .lock()
            .ok()
            .and_then(|b| *b)
            .unwrap_or(BrowserRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            })
    }
}

fn main_window(app: &AppHandle) -> Result<tauri::Window> {
    app.get_window(crate::window::MAIN)
        .ok_or_else(|| Error::BrowserUnavailable("main window closed".into()))
}

fn parse_web_url(url: &str) -> Result<url::Url> {
    let parsed = url::Url::parse(url)
        .map_err(|_| Error::BrowserUnavailable(format!("invalid url: {url}")))?;
    if !allow_navigation(&parsed) {
        return Err(Error::BrowserUnavailable(format!(
            "refused scheme: {}",
            parsed.scheme()
        )));
    }
    Ok(parsed)
}

/// Build a child-webview builder wired for navigation policy, page-load
/// recording, title reporting, and download interception. `fixed_tab` is the
/// owning tab for the multi host; the single host resolves the active tab at
/// event time (its one webview serves whichever tab is selected).
fn build_webview(label: &str, url: url::Url) -> tauri::webview::WebviewBuilder<Wry> {
    let event_label = label.to_string();
    let title_label = label.to_string();
    tauri::webview::WebviewBuilder::new(label, WebviewUrl::External(url))
        .on_navigation(allow_navigation)
        .on_page_load(move |webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let app = webview.app_handle().clone();
                let state = app.state::<crate::state::AppState>();
                state
                    .browser
                    .record_navigated(&app, &event_label, payload.url().to_string());
            }
        })
        .on_document_title_changed(move |webview, title| {
            let app = webview.app_handle().clone();
            let state = app.state::<crate::state::AppState>();
            state.browser.record_title(&app, &title_label, title);
        })
        .on_download(move |webview, event| on_download(&webview, event))
}

/// Shared download interception. `Requested` rewrites the destination into
/// staging and registers the pending download; `Finished` hands off to the
/// filing job (or parks for a project pick). blob:/data: URLs can't be
/// redirected usefully — they fall through to ~/Downloads with a notice.
fn on_download(webview: &Webview<Wry>, event: tauri::webview::DownloadEvent<'_>) -> bool {
    let app = webview.app_handle().clone();
    match event {
        tauri::webview::DownloadEvent::Requested { url, destination } => {
            let scheme = url.scheme();
            if scheme == "blob" || scheme == "data" {
                record_fallback(&app, &filename_guess(&url));
                return true;
            }
            let filename = destination
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .filter(|f| !f.trim().is_empty())
                .map(|f| safe_filename(&f))
                .unwrap_or_else(|| safe_filename(&filename_guess(&url)));
            let Ok(staging) = staging_dir(&app) else {
                record_fallback(&app, &filename);
                return true;
            };
            let id = uuid::Uuid::new_v4().to_string();
            let dir = staging.join(&id);
            if std::fs::create_dir_all(&dir).is_err() {
                record_fallback(&app, &filename);
                return true;
            }
            let staging_path = dir.join(&filename);
            let state = app.state::<crate::state::AppState>();
            let page_url = state.browser.page_url_for_label(webview.label());
            state.downloads.begin(PendingDownload {
                id,
                url: url.to_string(),
                page_url,
                filename,
                staging_path: staging_path.clone(),
                started_at: now_rfc3339(),
            });
            *destination = staging_path;
            true
        }
        tauri::webview::DownloadEvent::Finished { url, success, .. } => {
            let url = url.to_string();
            tauri::async_runtime::spawn(async move {
                finish_download(app, url, success).await;
            });
            true
        }
        _ => true,
    }
}

async fn finish_download(app: AppHandle, url: String, success: bool) {
    let state = app.state::<crate::state::AppState>();
    // a miss is a fallback download we never staged — nothing to do
    let Some(pending) = state.downloads.finish(&url) else {
        return;
    };
    if !success {
        if let Some(dir) = pending.staging_path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
        let payload =
            serde_json::json!({ "filename": pending.filename, "url": pending.url }).to_string();
        let _ = state
            .db
            .call(move |conn| {
                crate::db::events::record(conn, "download_failed", None, Some(&payload))
            })
            .await;
        crate::broadcast::events_appended(&app);
        crate::broadcast::download_failed(&app, &pending.filename);
        return;
    }
    let override_slug = state
        .active_asset_project
        .lock()
        .ok()
        .and_then(|s| s.clone());
    let slug = match override_slug {
        Some(slug) => Some(slug),
        None => state
            .db
            .call(|conn| crate::db::projects::most_recently_touched(conn))
            .await
            .ok()
            .flatten()
            .map(|p| p.slug),
    };
    match slug {
        Some(slug) => crate::jobs::download::spawn_filing(&app, pending, slug),
        None => {
            let id = pending.id.clone();
            let filename = pending.filename.clone();
            state.downloads.park(pending);
            crate::broadcast::download_needs_project(&app, &id, &filename);
        }
    }
}

fn record_fallback(app: &AppHandle, filename: &str) {
    let app = app.clone();
    let filename = filename.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        let payload = serde_json::json!({ "filename": filename }).to_string();
        let _ = state
            .db
            .call(move |conn| {
                crate::db::events::record(conn, "download_fallback", None, Some(&payload))
            })
            .await;
        crate::broadcast::events_appended(&app);
        crate::broadcast::download_fallback(&app, &filename);
    });
}

fn filename_guess(url: &url::Url) -> String {
    url.path_segments()
        .into_iter()
        .flatten()
        .rfind(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "download".to_string())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A multi-webview `add_child` failed: flip the settings flag so the next
/// launch runs the single-webview fallback, and leave an events trail.
fn engage_fallback(app: &AppHandle, error: &str) {
    let app = app.clone();
    let error = error.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        let payload = serde_json::json!({ "error": error }).to_string();
        let _ = state
            .db
            .call(move |conn| {
                crate::db::settings::set(conn, "browser_single_webview", "true")?;
                crate::db::events::record(conn, "browser_fallback_engaged", None, Some(&payload))
            })
            .await;
        crate::broadcast::events_appended(&app);
    });
}

fn apply_bounds(webview: &Webview<Wry>, rect: BrowserRect) {
    let _ = webview.set_position(tauri::LogicalPosition::new(rect.x, rect.y));
    let _ = webview.set_size(tauri::LogicalSize::new(rect.width, rect.height));
}

/// One child webview per tab; only the active tab's webview is shown
/// (macOS multi-webview visibility bug sidestep).
pub struct MultiWebviewHost {
    inner: HostInner,
}

impl MultiWebviewHost {
    pub fn new() -> Self {
        Self {
            inner: HostInner::new(),
        }
    }

    fn label_for(id: TabId) -> String {
        format!("browser-tab-{id}")
    }

    fn live_webview(&self, app: &AppHandle, id: TabId) -> Option<Webview<Wry>> {
        let label = self.inner.webviews.lock().ok()?.get(&id)?.clone();
        app.get_webview(&label)
    }

    /// Create the tab's webview if it isn't live (lazily recreated after the
    /// window was rebuilt). Returns the webview.
    fn ensure_webview(&self, app: &AppHandle, id: TabId) -> Result<Webview<Wry>> {
        if let Some(webview) = self.live_webview(app, id) {
            return Ok(webview);
        }
        let url = self
            .inner
            .model(|m| m.current_url(id).map(str::to_string))?
            .ok_or_else(|| Error::BrowserUnavailable(format!("no tab {id}")))?;
        let parsed = parse_web_url(&url)?;
        let label = Self::label_for(id);
        let window = main_window(app)?;
        let rect = self.inner.current_rect();
        let webview = window
            .add_child(
                build_webview(&label, parsed),
                tauri::LogicalPosition::new(rect.x, rect.y),
                tauri::LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| {
                engage_fallback(app, &e.to_string());
                Error::BrowserUnavailable(format!("webview creation failed: {e}"))
            })?;
        if let Ok(mut map) = self.inner.webviews.lock() {
            map.insert(id, label);
        }
        let _ = webview.hide();
        Ok(webview)
    }

    /// Show only the active tab's webview (when the surface is visible),
    /// hide every other.
    fn sync_visibility(&self, app: &AppHandle) {
        let visible = self.inner.visible.load(Ordering::Relaxed);
        let active = self.inner.model(|m| m.active()).ok().flatten();
        let entries: Vec<(TabId, String)> = self
            .inner
            .webviews
            .lock()
            .map(|m| m.iter().map(|(k, v)| (*k, v.clone())).collect())
            .unwrap_or_default();
        for (id, label) in entries {
            if let Some(webview) = app.get_webview(&label) {
                if visible && Some(id) == active {
                    let _ = webview.show();
                    let _ = webview.set_focus();
                } else {
                    let _ = webview.hide();
                }
            }
        }
    }

    fn tab_for_label(&self, label: &str) -> Option<TabId> {
        label.strip_prefix("browser-tab-")?.parse().ok()
    }
}

impl Default for MultiWebviewHost {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserTabHost for MultiWebviewHost {
    fn open_tab(&self, app: &AppHandle, url: &str) -> Result<TabId> {
        parse_web_url(url)?;
        let id = self.inner.model(|m| m.open(url.to_string()))?;
        if self.inner.visible.load(Ordering::Relaxed) {
            self.ensure_webview(app, id)?;
        }
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        Ok(id)
    }

    fn close_tab(&self, app: &AppHandle, id: TabId) -> Result<()> {
        if let Some(webview) = self.live_webview(app, id) {
            let _ = webview.close();
        }
        if let Ok(mut map) = self.inner.webviews.lock() {
            map.remove(&id);
        }
        if let Ok(mut titles) = self.inner.titles.lock() {
            titles.remove(&id);
        }
        self.inner.model(|m| m.close(id))?;
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn select_tab(&self, app: &AppHandle, id: TabId) -> Result<()> {
        if !self.inner.model(|m| m.select(id))? {
            return Err(Error::BrowserUnavailable(format!("no tab {id}")));
        }
        if self.inner.visible.load(Ordering::Relaxed) {
            self.ensure_webview(app, id)?;
        }
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn navigate(&self, app: &AppHandle, id: TabId, url: &str) -> Result<()> {
        let parsed = parse_web_url(url)?;
        let webview = self.ensure_webview(app, id)?;
        webview
            .navigate(parsed)
            .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        Ok(())
    }

    fn go(&self, app: &AppHandle, id: TabId, delta: i32) -> Result<()> {
        let Some(url) = self.inner.model(|m| m.go(id, delta))? else {
            return Ok(());
        };
        let parsed = parse_web_url(&url)?;
        let webview = self.ensure_webview(app, id)?;
        webview
            .navigate(parsed)
            .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn set_bounds(&self, app: &AppHandle, rect: BrowserRect) -> Result<()> {
        if let Ok(mut bounds) = self.inner.bounds.lock() {
            *bounds = Some(rect);
        }
        let entries: Vec<String> = self
            .inner
            .webviews
            .lock()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        for label in entries {
            if let Some(webview) = app.get_webview(&label) {
                apply_bounds(&webview, rect);
            }
        }
        Ok(())
    }

    fn set_visible(&self, app: &AppHandle, visible: bool) -> Result<()> {
        self.inner.visible.store(visible, Ordering::Relaxed);
        if visible && let Some(active) = self.inner.model(|m| m.active())? {
            self.ensure_webview(app, active)?;
        }
        self.sync_visibility(app);
        Ok(())
    }

    fn state(&self) -> BrowserState {
        self.inner.snapshot()
    }

    fn on_window_destroyed(&self) {
        if let Ok(mut map) = self.inner.webviews.lock() {
            map.clear();
        }
        self.inner.visible.store(false, Ordering::Relaxed);
    }

    fn record_navigated(&self, app: &AppHandle, label: &str, url: String) {
        if let Some(id) = self.tab_for_label(label) {
            let _ = self.inner.model(|m| m.navigated(id, url));
            crate::broadcast::browser_state_changed(app);
        }
    }

    fn record_title(&self, app: &AppHandle, label: &str, title: String) {
        if let Some(id) = self.tab_for_label(label) {
            if let Ok(mut titles) = self.inner.titles.lock() {
                titles.insert(id, title);
            }
            crate::broadcast::browser_state_changed(app);
        }
    }

    fn page_url_for_label(&self, label: &str) -> String {
        self.tab_for_label(label)
            .and_then(|id| {
                self.inner
                    .model(|m| m.current_url(id).map(str::to_string))
                    .ok()
                    .flatten()
            })
            .unwrap_or_default()
    }
}

const SINGLE_LABEL: &str = "browser-tab";

/// One reused child webview; selecting a tab navigates it, history is fully
/// model-driven. The settings-flag fallback for multi-webview instability.
pub struct SingleWebviewHost {
    inner: HostInner,
}

impl SingleWebviewHost {
    pub fn new() -> Self {
        Self {
            inner: HostInner::new(),
        }
    }

    fn live_webview(&self, app: &AppHandle) -> Option<Webview<Wry>> {
        app.get_webview(SINGLE_LABEL)
    }

    fn ensure_webview(&self, app: &AppHandle, url: &str) -> Result<Webview<Wry>> {
        if let Some(webview) = self.live_webview(app) {
            return Ok(webview);
        }
        let parsed = parse_web_url(url)?;
        let window = main_window(app)?;
        let rect = self.inner.current_rect();
        let webview = window
            .add_child(
                build_webview(SINGLE_LABEL, parsed),
                tauri::LogicalPosition::new(rect.x, rect.y),
                tauri::LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| Error::BrowserUnavailable(format!("webview creation failed: {e}")))?;
        if !self.inner.visible.load(Ordering::Relaxed) {
            let _ = webview.hide();
        }
        Ok(webview)
    }

    /// Navigate the shared webview to the active tab's current URL.
    fn show_active(&self, app: &AppHandle) -> Result<()> {
        let Some(url) = self.inner.model(|m| {
            m.active()
                .and_then(|id| m.current_url(id).map(str::to_string))
        })?
        else {
            if let Some(webview) = self.live_webview(app) {
                let _ = webview.hide();
            }
            return Ok(());
        };
        let webview = self.ensure_webview(app, &url)?;
        let parsed = parse_web_url(&url)?;
        webview
            .navigate(parsed)
            .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        if self.inner.visible.load(Ordering::Relaxed) {
            let _ = webview.show();
        }
        Ok(())
    }

    /// The single webview always serves the active tab.
    fn active_tab(&self) -> Option<TabId> {
        self.inner.model(|m| m.active()).ok().flatten()
    }
}

impl Default for SingleWebviewHost {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserTabHost for SingleWebviewHost {
    fn open_tab(&self, app: &AppHandle, url: &str) -> Result<TabId> {
        parse_web_url(url)?;
        let id = self.inner.model(|m| m.open(url.to_string()))?;
        self.show_active(app)?;
        crate::broadcast::browser_state_changed(app);
        Ok(id)
    }

    fn close_tab(&self, app: &AppHandle, id: TabId) -> Result<()> {
        self.inner.model(|m| m.close(id))?;
        if let Ok(mut titles) = self.inner.titles.lock() {
            titles.remove(&id);
        }
        self.show_active(app)?;
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn select_tab(&self, app: &AppHandle, id: TabId) -> Result<()> {
        if !self.inner.model(|m| m.select(id))? {
            return Err(Error::BrowserUnavailable(format!("no tab {id}")));
        }
        self.show_active(app)?;
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn navigate(&self, app: &AppHandle, id: TabId, url: &str) -> Result<()> {
        let parsed = parse_web_url(url)?;
        if self.active_tab() != Some(id) {
            // navigating a background tab only moves its model state; the
            // shared webview keeps showing the active tab
            self.inner.model(|m| m.navigated(id, url.to_string()))?;
            crate::broadcast::browser_state_changed(app);
            return Ok(());
        }
        let webview = self.ensure_webview(app, url)?;
        webview
            .navigate(parsed)
            .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        Ok(())
    }

    fn go(&self, app: &AppHandle, id: TabId, delta: i32) -> Result<()> {
        let Some(url) = self.inner.model(|m| m.go(id, delta))? else {
            return Ok(());
        };
        if self.active_tab() == Some(id) {
            let parsed = parse_web_url(&url)?;
            let webview = self.ensure_webview(app, &url)?;
            webview
                .navigate(parsed)
                .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        }
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn set_bounds(&self, app: &AppHandle, rect: BrowserRect) -> Result<()> {
        if let Ok(mut bounds) = self.inner.bounds.lock() {
            *bounds = Some(rect);
        }
        if let Some(webview) = self.live_webview(app) {
            apply_bounds(&webview, rect);
        }
        Ok(())
    }

    fn set_visible(&self, app: &AppHandle, visible: bool) -> Result<()> {
        self.inner.visible.store(visible, Ordering::Relaxed);
        if visible {
            self.show_active(app)?;
        } else if let Some(webview) = self.live_webview(app) {
            let _ = webview.hide();
        }
        Ok(())
    }

    fn state(&self) -> BrowserState {
        self.inner.snapshot()
    }

    fn on_window_destroyed(&self) {
        self.inner.visible.store(false, Ordering::Relaxed);
    }

    fn record_navigated(&self, app: &AppHandle, _label: &str, url: String) {
        if let Some(id) = self.active_tab() {
            let _ = self.inner.model(|m| m.navigated(id, url));
            crate::broadcast::browser_state_changed(app);
        }
    }

    fn record_title(&self, app: &AppHandle, _label: &str, title: String) {
        if let Some(id) = self.active_tab() {
            if let Ok(mut titles) = self.inner.titles.lock() {
                titles.insert(id, title);
            }
            crate::broadcast::browser_state_changed(app);
        }
    }

    fn page_url_for_label(&self, _label: &str) -> String {
        self.active_tab()
            .and_then(|id| {
                self.inner
                    .model(|m| m.current_url(id).map(str::to_string))
                    .ok()
                    .flatten()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    fn pending(id: &str, url: &str) -> PendingDownload {
        PendingDownload {
            id: id.into(),
            url: url.into(),
            page_url: String::new(),
            filename: "f.zip".into(),
            staging_path: PathBuf::from(format!("/staging/{id}/f.zip")),
            started_at: String::new(),
        }
    }

    #[test]
    fn duplicate_url_downloads_finish_fifo_without_loss() {
        let registry = DownloadRegistry::default();
        registry.begin(pending("a", "https://x.test/f.zip"));
        registry.begin(pending("b", "https://x.test/f.zip"));
        let first = registry.finish("https://x.test/f.zip").unwrap();
        let second = registry.finish("https://x.test/f.zip").unwrap();
        assert_eq!(first.id, "a");
        assert_eq!(second.id, "b");
        assert!(registry.finish("https://x.test/f.zip").is_none());
    }

    #[test]
    fn unknown_url_is_a_miss() {
        let registry = DownloadRegistry::default();
        assert!(registry.finish("https://never.test/").is_none());
    }
}
