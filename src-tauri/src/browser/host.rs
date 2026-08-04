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

/// Logical-pixel rect of the browser surface's content area, in the coordinate
/// space of the main window's **content view** — where wry places child
/// webviews (`wkwebview::set_bounds` measures against `superview()`).
///
/// React does NOT report `getBoundingClientRect()` untransformed. Tauri gives
/// every macOS window `FullSizeContentView`, so the content view spans the whole
/// frame including the titlebar strip while WKWebView lays the document out
/// below it: measured on the owner's machine, the content view is 869 logical px
/// tall against an `innerHeight` of 837. A DOM-relative `y` fed to wry's y-flip
/// therefore lands one 32px inset too high, which is what used to bury the
/// browser toolbar under the page. `use-browser-bounds.ts` derives that inset
/// and adds it before sending; it collapses to 0 where no inset exists.
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
    /// `None` opens a start-page tab: no URL, no webview until it navigates.
    fn open_tab(&self, app: &AppHandle, url: Option<&str>) -> Result<TabId>;
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
    /// Serialises every read-`visible`-then-show/hide region. Tauri runs
    /// commands concurrently, so without it `set_bounds` could read
    /// `visible = true`, be descheduled while `set_visible(false)` hid
    /// everything, and then resume and `show()` the page back on top of
    /// whatever surface the user had switched to. Under the lock the losing
    /// task re-reads `visible` after the winner is done, so the last writer
    /// decides what is on screen — every guarded region must therefore derive
    /// its decision *inside* the guard, never carry one in.
    ///
    /// Two limits. `add_child` has no hidden-at-creation option in tauri 2.11
    /// (`WebviewAttributes` carries no `visible`), so a new child is briefly on
    /// screen before the guard can hide it — a flash the lock cannot close.
    /// And only command threads may take this: every call made under it is a
    /// fire-and-forget runtime message today, so if a wry callback (page load,
    /// title, download) ever starts taking `ops`, the main thread does too, and
    /// any blocking runtime getter added under the guard would then deadlock.
    ops: Mutex<()>,
}

impl HostInner {
    fn new() -> Self {
        Self {
            model: Mutex::new(TabModel::new()),
            webviews: Mutex::new(HashMap::new()),
            titles: Mutex::new(HashMap::new()),
            bounds: Mutex::new(None),
            visible: AtomicBool::new(false),
            ops: Mutex::new(()),
        }
    }

    /// Enter the visibility-application region. Poisoning is recovered rather
    /// than propagated: the guarded region protects ordering, not an
    /// invariant, and bailing out would strand a webview on screen over
    /// another surface — the exact failure the lock exists to prevent.
    fn ops(&self) -> std::sync::MutexGuard<'_, ()> {
        self.ops.lock().unwrap_or_else(|e| e.into_inner())
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

    fn store_rect(&self, rect: BrowserRect) {
        if let Ok(mut bounds) = self.bounds.lock() {
            *bounds = Some(rect);
        }
    }

    /// Whether a *usable* rect has landed. Deliberately the same predicate
    /// [`Self::require_rect`] enforces: a caller that guards on this must never
    /// go on to trip the error it is guarding against — a stored 0x0 rect
    /// (measured while an ancestor was collapsed) is not a report.
    fn bounds_reported(&self) -> bool {
        self.require_rect().is_ok()
    }

    /// The last rect React reported. Creating a webview before any report would
    /// park a 0x0 view at the window origin, so this refuses instead.
    fn require_rect(&self) -> Result<BrowserRect> {
        let rect = self.bounds.lock().ok().and_then(|b| *b).ok_or_else(|| {
            Error::BrowserUnavailable("browser surface bounds not reported yet".into())
        })?;
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Err(Error::BrowserUnavailable(format!(
                "browser surface rect is degenerate: {}x{}",
                rect.width, rect.height
            )));
        }
        Ok(rect)
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
/// WKWebView's stock user agent carries no `Version/… Safari/…` token, so
/// browser-sniffing sites classify it as unknown and degrade — YouTube Studio
/// refuses to load past an "unsupported browser" interstitial. This is the
/// engine's own identity stated properly, not a spoof: the tabs really are
/// Safari's WebKit. The `10_15_7` and `605.1.15` tokens are frozen values Safari
/// itself still reports on current macOS; only `Version/` tracks the release, so
/// that is the part to bump when the interstitials come back.
const TAB_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.5 Safari/605.1.15";

fn build_webview(label: &str, url: url::Url) -> tauri::webview::WebviewBuilder<Wry> {
    let event_label = label.to_string();
    let title_label = label.to_string();
    tauri::webview::WebviewBuilder::new(label, WebviewUrl::External(url))
        .user_agent(TAB_USER_AGENT)
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

/// One atomic bounds message. Splitting it into `set_position` + `set_size`
/// leaves an intermediate frame where the runtime's y-flip is computed against
/// the stale height, misplacing the webview for one event-loop turn.
fn apply_bounds(webview: &Webview<Wry>, rect: BrowserRect) {
    let _ = webview.set_bounds(tauri::Rect {
        position: tauri::LogicalPosition::new(rect.x, rect.y).into(),
        size: tauri::LogicalSize::new(rect.width, rect.height).into(),
    });
}

/// Exactly one webview is on screen: the active tab's, and only while the
/// surface itself is visible. A URL-less tab owns no webview at all, so it
/// never reaches here and every live webview hides — which is what lets the
/// DOM start page show through.
fn should_show(tab: TabId, active: Option<TabId>, surface_visible: bool) -> bool {
    surface_visible && active == Some(tab)
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

    /// Attach a hidden child webview for `id` at `url`; `sync_visibility`
    /// shows it.
    fn create_webview(&self, app: &AppHandle, id: TabId, url: url::Url) -> Result<Webview<Wry>> {
        let label = Self::label_for(id);
        let window = main_window(app)?;
        let rect = self.inner.require_rect()?;
        let webview = window
            .add_child(
                build_webview(&label, url),
                tauri::LogicalPosition::new(rect.x, rect.y),
                tauri::LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| {
                engage_fallback(app, &e.to_string());
                Error::BrowserUnavailable(format!("webview creation failed: {e}"))
            })?;
        // registering the label and hiding must be one region: a
        // `sync_visibility` that observed the label between the two would
        // `show()` the webview and then have this hide land on top of it,
        // leaving the active tab blank
        {
            let _guard = self.inner.ops();
            if let Ok(mut map) = self.inner.webviews.lock() {
                map.insert(id, label);
            }
            let _ = webview.hide();
        }
        Ok(webview)
    }

    /// The tab's live webview, creating it when the tab has a URL (lazily
    /// recreated after the window was rebuilt). `Ok(None)` = the tab exists but
    /// holds no URL — the start page, which by design owns no webview, so
    /// `sync_visibility` hides everything and the DOM page paints.
    fn ensure_webview(&self, app: &AppHandle, id: TabId) -> Result<Option<Webview<Wry>>> {
        if let Some(webview) = self.live_webview(app, id) {
            return Ok(Some(webview));
        }
        if !self.inner.model(|m| m.contains(id))? {
            return Err(Error::BrowserUnavailable(format!("no tab {id}")));
        }
        let Some(url) = self
            .inner
            .model(|m| m.current_url(id).map(str::to_string))?
        else {
            return Ok(None);
        };
        self.create_webview(app, id, parse_web_url(&url)?).map(Some)
    }

    /// Show only the active tab's webview (when the surface is visible),
    /// hide every other. The whole read-and-apply runs under `ops` so a
    /// concurrent `set_visible` can't slip its hide between this read of
    /// `visible` and the `show()` it authorises.
    fn sync_visibility(&self, app: &AppHandle) {
        let _guard = self.inner.ops();
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
                if should_show(id, active, visible) {
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
    fn open_tab(&self, app: &AppHandle, url: Option<&str>) -> Result<TabId> {
        // validate before the model mutates, so a bad URL never creates a tab
        if let Some(url) = url {
            parse_web_url(url)?;
        }
        let id = self.inner.model(|m| m.open(url.map(str::to_string)))?;
        if self.inner.visible.load(Ordering::Relaxed) && self.inner.bounds_reported() {
            let _ = self.ensure_webview(app, id)?;
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
        if self.inner.visible.load(Ordering::Relaxed) && self.inner.bounds_reported() {
            let _ = self.ensure_webview(app, id)?;
        }
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    fn navigate(&self, app: &AppHandle, id: TabId, url: &str) -> Result<()> {
        let parsed = parse_web_url(url)?;
        if !self.inner.model(|m| m.contains(id))? {
            return Err(Error::BrowserUnavailable(format!("no tab {id}")));
        }
        if let Some(webview) = self.live_webview(app, id) {
            return webview
                .navigate(parsed)
                .map_err(|e| Error::BrowserUnavailable(e.to_string()));
        }
        // No live webview: a start-page tab, or one lost with the window. Seed
        // the model so the tab leaves the start page the moment this returns,
        // then create the child at the target — `add_child` loads it, so there
        // is no second navigate.
        if self.inner.model(|m| m.current_url(id).is_none())? {
            self.inner.model(|m| m.navigated(id, url.to_string()))?;
        }
        let created = self.create_webview(app, id, parsed);
        // the model left the start page whether or not the webview came up, so
        // the broadcast is unconditional; skipping it on failure leaves React
        // rendering a start page for a tab that now holds a URL
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        created.map(|_| ())
    }

    fn go(&self, app: &AppHandle, id: TabId, delta: i32) -> Result<()> {
        let Some(url) = self.inner.model(|m| m.go(id, delta))? else {
            return Ok(());
        };
        let parsed = parse_web_url(&url)?;
        let Some(webview) = self.ensure_webview(app, id)? else {
            return Ok(());
        };
        webview
            .navigate(parsed)
            .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        // `ensure_webview` may have just created this one, and `create_webview`
        // attaches hidden by contract — without this, Back/Forward as the first
        // action after the window was rebuilt leaves it hidden for good
        self.sync_visibility(app);
        crate::broadcast::browser_state_changed(app);
        Ok(())
    }

    /// Also the convergence point for a first mount: `set_bounds` and
    /// `set_visible` arrive over independent async commands, so whichever lands
    /// second materialises the active tab's webview.
    fn set_bounds(&self, app: &AppHandle, rect: BrowserRect) -> Result<()> {
        self.inner.store_rect(rect);
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
        if self.inner.visible.load(Ordering::Relaxed)
            && let Some(active) = self.inner.model(|m| m.active())?
        {
            let _ = self.ensure_webview(app, active)?;
            self.sync_visibility(app);
        }
        Ok(())
    }

    fn set_visible(&self, app: &AppHandle, visible: bool) -> Result<()> {
        self.inner.visible.store(visible, Ordering::Relaxed);
        if visible
            && self.inner.bounds_reported()
            && let Some(active) = self.inner.model(|m| m.active())?
        {
            let _ = self.ensure_webview(app, active)?;
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

/// What the single host's one shared webview should do for the current active
/// tab — the counterpart of [`should_show`], pulled out of the `AppHandle`
/// plumbing so both gates are testable without a window.
#[derive(Clone, Debug, PartialEq)]
enum SharedAction {
    /// React has not measured the surface yet; `set_bounds` converges later.
    Wait,
    /// No active tab, or an active tab holding no URL: hide the shared webview
    /// so the DOM start page shows through.
    Hide,
    /// Load this URL into the shared webview.
    Load(String),
}

fn shared_action(bounds_reported: bool, active_url: Option<String>) -> SharedAction {
    if !bounds_reported {
        return SharedAction::Wait;
    }
    match active_url {
        Some(url) => SharedAction::Load(url),
        None => SharedAction::Hide,
    }
}

/// One reused child webview; selecting a tab navigates it, history is fully
/// model-driven. The settings-flag fallback for multi-webview instability.
pub struct SingleWebviewHost {
    inner: HostInner,
    /// The URL the shared webview currently holds. Switching to a start-page
    /// tab only *hides* it — the page stays loaded and keeps running script —
    /// so a download it fires belongs to this URL, not to whatever tab happens
    /// to be active when the event arrives.
    loaded: Mutex<Option<String>>,
}

impl SingleWebviewHost {
    pub fn new() -> Self {
        Self {
            inner: HostInner::new(),
            loaded: Mutex::new(None),
        }
    }

    fn live_webview(&self, app: &AppHandle) -> Option<Webview<Wry>> {
        app.get_webview(SINGLE_LABEL)
    }

    fn note_loaded(&self, url: &str) {
        if let Ok(mut loaded) = self.loaded.lock() {
            *loaded = Some(url.to_string());
        }
    }

    /// The shared webview, creating it at `url` when it isn't live. The flag is
    /// *created*: `add_child` already loads that URL, so callers must skip the
    /// navigate they would otherwise issue right after.
    fn ensure_webview(&self, app: &AppHandle, url: &str) -> Result<(Webview<Wry>, bool)> {
        if let Some(webview) = self.live_webview(app) {
            return Ok((webview, false));
        }
        let parsed = parse_web_url(url)?;
        let window = main_window(app)?;
        let rect = self.inner.require_rect()?;
        let webview = window
            .add_child(
                build_webview(SINGLE_LABEL, parsed),
                tauri::LogicalPosition::new(rect.x, rect.y),
                tauri::LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| Error::BrowserUnavailable(format!("webview creation failed: {e}")))?;
        {
            let _guard = self.inner.ops();
            if !self.inner.visible.load(Ordering::Relaxed) {
                let _ = webview.hide();
            }
        }
        Ok((webview, true))
    }

    /// Point the shared webview at `url`, creating it when it isn't live, and
    /// show it while the surface is visible.
    fn load(&self, app: &AppHandle, url: &str) -> Result<()> {
        let (webview, created) = self.ensure_webview(app, url)?;
        if !created {
            webview
                .navigate(parse_web_url(url)?)
                .map_err(|e| Error::BrowserUnavailable(e.to_string()))?;
        }
        self.note_loaded(url);
        // guarded for the same reason as the multi host's `sync_visibility`:
        // this read and the `show()` it authorises must not straddle a
        // concurrent `set_visible(false)`
        {
            let _guard = self.inner.ops();
            if self.inner.visible.load(Ordering::Relaxed) {
                let _ = webview.show();
            }
        }
        Ok(())
    }

    /// What the shared webview should be doing for the active tab right now.
    fn active_action(&self) -> Result<SharedAction> {
        let active_url = self.inner.model(|m| {
            m.active()
                .and_then(|id| m.current_url(id).map(str::to_string))
        })?;
        Ok(shared_action(self.inner.bounds_reported(), active_url))
    }

    /// Navigate the shared webview to the active tab's current URL, hiding it
    /// when the active tab has none (the start page). No-ops until React has
    /// reported bounds; `set_bounds` converges once they arrive.
    fn show_active(&self, app: &AppHandle) -> Result<()> {
        match self.active_action()? {
            SharedAction::Wait => Ok(()),
            SharedAction::Hide => {
                let _guard = self.inner.ops();
                // re-derive inside the guard: a navigate that landed in between
                // has already shown the webview for a tab that now holds a URL,
                // and hiding on the stale decision would blank a live page
                if self.active_action()? != SharedAction::Hide {
                    return Ok(());
                }
                if let Some(webview) = self.live_webview(app) {
                    let _ = webview.hide();
                }
                Ok(())
            }
            SharedAction::Load(url) => self.load(app, &url),
        }
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
    fn open_tab(&self, app: &AppHandle, url: Option<&str>) -> Result<TabId> {
        if let Some(url) = url {
            parse_web_url(url)?;
        }
        let id = self.inner.model(|m| m.open(url.map(str::to_string)))?;
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
        parse_web_url(url)?;
        if !self.inner.model(|m| m.contains(id))? {
            return Err(Error::BrowserUnavailable(format!("no tab {id}")));
        }
        if self.active_tab() != Some(id) {
            // navigating a background tab only moves its model state; the
            // shared webview keeps showing the active tab
            self.inner.model(|m| m.navigated(id, url.to_string()))?;
            crate::broadcast::browser_state_changed(app);
            return Ok(());
        }
        let was_blank = self.inner.model(|m| m.current_url(id).is_none())?;
        if was_blank {
            // seed the model first so the tab leaves the start page as soon as
            // this returns
            self.inner.model(|m| m.navigated(id, url.to_string()))?;
        }
        let loaded = self.load(app, url);
        if was_blank {
            // the model moved whether or not the webview came up; skipping the
            // broadcast on failure leaves React painting the start page over a
            // tab that now holds a URL
            crate::broadcast::browser_state_changed(app);
        }
        loaded
    }

    fn go(&self, app: &AppHandle, id: TabId, delta: i32) -> Result<()> {
        let Some(url) = self.inner.model(|m| m.go(id, delta))? else {
            return Ok(());
        };
        let loaded = if self.active_tab() == Some(id) {
            self.load(app, &url)
        } else {
            Ok(())
        };
        crate::broadcast::browser_state_changed(app);
        loaded
    }

    /// Also the convergence point for a first mount — see the multi host.
    fn set_bounds(&self, app: &AppHandle, rect: BrowserRect) -> Result<()> {
        self.inner.store_rect(rect);
        if let Some(webview) = self.live_webview(app) {
            apply_bounds(&webview, rect);
        }
        if self.inner.visible.load(Ordering::Relaxed) {
            self.show_active(app)?;
        }
        Ok(())
    }

    fn set_visible(&self, app: &AppHandle, visible: bool) -> Result<()> {
        self.inner.visible.store(visible, Ordering::Relaxed);
        if visible {
            self.show_active(app)?;
        } else {
            let _guard = self.inner.ops();
            // the shared flag, not the parameter: a `set_visible(true)` that
            // landed behind this one has already shown the webview, and hiding
            // on our own stale argument would undo the newer decision
            if !self.inner.visible.load(Ordering::Relaxed)
                && let Some(webview) = self.live_webview(app)
            {
                let _ = webview.hide();
            }
        }
        Ok(())
    }

    fn state(&self) -> BrowserState {
        self.inner.snapshot()
    }

    fn on_window_destroyed(&self) {
        self.inner.visible.store(false, Ordering::Relaxed);
        if let Ok(mut loaded) = self.loaded.lock() {
            *loaded = None;
        }
    }

    fn record_navigated(&self, app: &AppHandle, _label: &str, url: String) {
        self.note_loaded(&url);
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

    /// The one shared webview serves whichever page it last loaded, which is
    /// not necessarily the active tab's — resolving by active tab would hand
    /// the license sidecar an empty URL as soon as the user opened a start-page
    /// tab while a background download was still being negotiated.
    fn page_url_for_label(&self, _label: &str) -> String {
        self.loaded
            .lock()
            .ok()
            .and_then(|url| url.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;

    fn rect(width: f64, height: f64) -> BrowserRect {
        BrowserRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    #[test]
    fn require_rect_refuses_unreported_bounds() {
        let inner = HostInner::new();
        assert!(inner.require_rect().is_err());
    }

    #[test]
    fn require_rect_refuses_a_degenerate_rect() {
        let inner = HostInner::new();
        inner.store_rect(rect(0.0, 0.0));
        assert!(inner.require_rect().is_err());
        inner.store_rect(rect(900.0, 0.0));
        assert!(inner.require_rect().is_err());
        inner.store_rect(rect(900.0, 600.0));
        assert!(inner.require_rect().is_ok());
    }

    #[test]
    fn bounds_reported_flips_after_a_report() {
        let inner = HostInner::new();
        assert!(!inner.bounds_reported());
        inner.store_rect(rect(900.0, 600.0));
        assert!(inner.bounds_reported());
    }

    #[test]
    fn bounds_reported_rejects_what_require_rect_rejects() {
        let inner = HostInner::new();
        // measured while an ancestor was collapsed: stored, but unusable, and
        // the guards must not wave it through into require_rect's error path
        inner.store_rect(rect(900.0, 0.0));
        assert!(!inner.bounds_reported());
        assert!(inner.require_rect().is_err());
    }

    #[test]
    fn ops_admits_callers_after_a_poisoned_guard() {
        let inner = std::sync::Arc::new(HostInner::new());
        let poisoner = std::sync::Arc::clone(&inner);
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.ops();
            panic!("poison the ops lock");
        })
        .join();
        std::panic::set_hook(hook);
        // refusing a poisoned ops lock would skip the show/hide it guards,
        // stranding a webview on screen over another surface — the exact
        // failure the lock exists to prevent
        let _guard = inner.ops();
    }

    #[test]
    fn snapshot_titles_a_blank_tab_new_tab() {
        let inner = HostInner::new();
        let id = inner.model(|m| m.open(None)).unwrap();
        let snap = inner.snapshot();
        assert_eq!(snap.active, Some(id));
        assert_eq!(snap.tabs[0].url, None);
        assert_eq!(snap.tabs[0].title, "New tab");
    }

    #[rstest::rstest]
    #[case::hidden_surface_active_tab(1, Some(1), false, false)]
    #[case::hidden_surface_background_tab(2, Some(1), false, false)]
    #[case::visible_active_tab(1, Some(1), true, true)]
    #[case::visible_background_tab(2, Some(1), true, false)]
    #[case::visible_no_active_tab(1, None, true, false)]
    fn should_show_only_the_active_tab_of_a_visible_surface(
        #[case] tab: TabId,
        #[case] active: Option<TabId>,
        #[case] surface_visible: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(should_show(tab, active, surface_visible), expected);
    }

    #[rstest::rstest]
    #[case::unmeasured(false, Some("https://a.test/"), SharedAction::Wait)]
    #[case::unmeasured_blank(false, None, SharedAction::Wait)]
    #[case::blank_active_tab(true, None, SharedAction::Hide)]
    #[case::url_active_tab(
        true,
        Some("https://a.test/"),
        SharedAction::Load(String::from("https://a.test/"))
    )]
    fn shared_action_hides_the_webview_for_a_start_page_tab(
        #[case] bounds_reported: bool,
        #[case] active_url: Option<&str>,
        #[case] expected: SharedAction,
    ) {
        let action = shared_action(bounds_reported, active_url.map(str::to_string));
        assert_eq!(action, expected);
    }

    #[test]
    fn single_host_attributes_a_download_to_the_loaded_page() {
        let host = SingleWebviewHost::new();
        host.inner
            .model(|m| m.open(Some("https://elements.envato.com/dust".into())))
            .unwrap();
        host.note_loaded("https://elements.envato.com/dust");
        // "+" while the Envato page is still negotiating a download: the page
        // stays loaded behind the start page and its download is still its own
        host.inner.model(|m| m.open(None)).unwrap();
        assert_eq!(
            host.page_url_for_label(SINGLE_LABEL),
            "https://elements.envato.com/dust"
        );
    }

    #[test]
    fn multi_host_attributes_a_download_to_the_reporting_tab() {
        let host = MultiWebviewHost::new();
        let envato = host
            .inner
            .model(|m| m.open(Some("https://elements.envato.com/dust".into())))
            .unwrap();
        host.inner.model(|m| m.open(None)).unwrap();
        assert_eq!(
            host.page_url_for_label(&MultiWebviewHost::label_for(envato)),
            "https://elements.envato.com/dust"
        );
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
