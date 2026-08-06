//! Pure tab/history model shared by both `BrowserTabHost` impls. The
//! multi-webview API exposes no back/forward/title, so history and titles are
//! katto-managed here; hosts only translate model decisions into webview calls.

/// Model-local tab identifier; also names the child webview label.
pub type TabId = u32;

struct Tab {
    id: TabId,
    history: Vec<String>,
    cursor: usize,
}

/// Ordered tabs plus which one is active. Owns all history decisions; hosts
/// call in and translate the returned URLs into webview navigations.
pub struct TabModel {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    next_id: TabId,
}

/// Wire snapshot of the whole browser for the frontend.
#[derive(Clone, Debug, PartialEq, serde::Serialize, specta::Type)]
pub struct BrowserState {
    pub tabs: Vec<TabSnapshot>,
    pub active: Option<TabId>,
}

/// Wire snapshot of one tab; `title` is URL-derived (the webview API exposes
/// no page title). `url` is `None` for a tab that has never navigated — the
/// start-page state, titled "New tab", owning no webview.
#[derive(Clone, Debug, PartialEq, serde::Serialize, specta::Type)]
pub struct TabSnapshot {
    pub id: TabId,
    pub title: String,
    pub url: Option<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// Title for a tab whose history is still empty.
const NEW_TAB_TITLE: &str = "New tab";

impl TabModel {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            next_id: 1,
        }
    }

    /// Open a new tab on `url`; the new tab becomes active. `None` opens a
    /// start-page tab: an empty history, which is what makes `navigated`,
    /// `go` and the `can_go_*` affordances correct with no special-casing.
    pub fn open(&mut self, url: Option<String>) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            history: url.map_or_else(Vec::new, |u| vec![u]),
            cursor: 0,
        });
        self.active = Some(id);
        id
    }

    /// Close a tab; returns the tab that becomes active, if any. Closing a
    /// background tab keeps the current active tab.
    pub fn close(&mut self, id: TabId) -> Option<TabId> {
        let idx = self.index_of(id)?;
        self.tabs.remove(idx);
        if self.active == Some(id) {
            let neighbor = idx.saturating_sub(1);
            self.active = self
                .tabs
                .get(neighbor)
                .or_else(|| self.tabs.first())
                .map(|t| t.id);
        }
        self.active
    }

    /// Make `id` the active tab; false when the tab doesn't exist.
    pub fn select(&mut self, id: TabId) -> bool {
        if self.index_of(id).is_some() {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    /// Record a committed page load: truncates forward history and pushes.
    /// Consecutive duplicates (reload, redirect echo) collapse into one entry.
    pub fn navigated(&mut self, id: TabId, url: String) {
        let Some(tab) = self.tab_mut(id) else { return };
        if tab
            .history
            .get(tab.cursor)
            .is_some_and(|current| *current == url)
        {
            return;
        }
        tab.history.truncate(tab.cursor + 1);
        tab.history.push(url);
        tab.cursor = tab.history.len() - 1;
    }

    /// Move the history cursor by `delta`; returns the URL to load, or `None`
    /// when out of range (cursor unmoved).
    pub fn go(&mut self, id: TabId, delta: i32) -> Option<String> {
        let tab = self.tab_mut(id)?;
        let target = i64::try_from(tab.cursor).ok()? + i64::from(delta);
        let target = usize::try_from(target).ok()?;
        let url = tab.history.get(target)?.clone();
        tab.cursor = target;
        Some(url)
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn contains(&self, id: TabId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn current_url(&self, id: TabId) -> Option<&str> {
        let tab = self.tabs.iter().find(|t| t.id == id)?;
        tab.history.get(tab.cursor).map(String::as_str)
    }

    pub fn snapshot(&self) -> BrowserState {
        BrowserState {
            tabs: self
                .tabs
                .iter()
                .map(|t| {
                    let url = t.history.get(t.cursor).cloned();
                    TabSnapshot {
                        id: t.id,
                        title: url
                            .as_deref()
                            .map_or_else(|| NEW_TAB_TITLE.to_string(), tab_title),
                        url,
                        can_go_back: t.cursor > 0,
                        can_go_forward: t.cursor + 1 < t.history.len(),
                    }
                })
                .collect(),
            active: self.active,
        }
    }

    fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }
}

impl Default for TabModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Cosmetic tab title derived from the URL: host without `www.`, plus the
/// prettified last path segment when there is one, joined with " — ".
pub fn tab_title(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    let Some(host) = parsed.host_str() else {
        return url.to_string();
    };
    let host = host.strip_prefix("www.").unwrap_or(host);
    let segment = parsed
        .path_segments()
        .into_iter()
        .flatten()
        .rfind(|s| !s.is_empty())
        .map(prettify_segment)
        .filter(|s| !s.is_empty());
    match segment {
        Some(seg) => format!("{host} — {seg}"),
        None => host.to_string(),
    }
}

fn prettify_segment(segment: &str) -> String {
    let decoded = percent_encoding_decode(segment);
    let stem = match decoded.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty() && ext.len() <= 5 && ext.chars().all(char::is_alphanumeric) =>
        {
            stem
        }
        _ => decoded.as_str(),
    };
    stem.replace(['-', '_'], " ").trim().to_string()
}

fn percent_encoding_decode(segment: &str) -> String {
    // url::Url keeps path segments percent-encoded; a lossy round-trip through
    // the crate's parser is heavier than needed for a cosmetic title.
    let mut out = String::with_capacity(segment.len());
    let mut bytes = segment.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next();
            let lo = bytes.next();
            match (hi, lo) {
                (Some(hi), Some(lo)) => {
                    let hex = [hi, lo];
                    match std::str::from_utf8(&hex)
                        .ok()
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                    {
                        Some(v) => out.push(v as char),
                        None => out.push('%'),
                    }
                }
                _ => out.push('%'),
            }
        } else {
            out.push(b as char);
        }
    }
    out
}

/// Navigation policy for child webviews: web schemes only — never `file:`,
/// never katto's own deep-link scheme.
pub fn allow_navigation(url: &url::Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "about" | "blob" | "data")
}

/// What a new-window request (`target="_blank"`, `window.open`) becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupDecision {
    /// Open this URL as a new foreground tab.
    OpenTab(String),
    /// Drop the request; the string is the reason for the events row.
    Refuse(String),
}

/// Stricter than [`allow_navigation`] on purpose. katto denies the request and
/// opens its own tab instead, so the opener's `window.open()` handle is null and
/// it can never write into that tab. `about:`/`blob:`/`data:` popups exist only
/// to be written into by that handle — re-opening one as a tab would leave a
/// dead blank tab, so refuse it and leave a trail.
pub fn popup_decision(raw: &str) -> PopupDecision {
    match url::Url::parse(raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => PopupDecision::OpenTab(raw.into()),
        Ok(url) => PopupDecision::Refuse(format!("refused scheme: {}", url.scheme())),
        Err(_) => PopupDecision::Refuse(format!("invalid url: {raw}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with_two_tabs() -> (TabModel, TabId, TabId) {
        let mut m = TabModel::new();
        let a = m.open(Some("https://elements.envato.com/".into()));
        let b = m.open(Some("https://example.com/".into()));
        (m, a, b)
    }

    #[test]
    fn open_without_url_leaves_the_tab_blank() {
        let mut m = TabModel::new();
        let t = m.open(None);
        assert_eq!(m.current_url(t), None);
        let snap = m.snapshot();
        assert_eq!(snap.tabs[0].url, None);
        assert_eq!(snap.tabs[0].title, "New tab");
        assert!(!snap.tabs[0].can_go_back);
        assert!(!snap.tabs[0].can_go_forward);
    }

    #[test]
    fn navigating_a_blank_tab_sets_its_url() {
        let mut m = TabModel::new();
        let t = m.open(None);
        m.navigated(t, "https://a.test/".into());
        assert_eq!(m.current_url(t), Some("https://a.test/"));
        let snap = m.snapshot();
        assert_eq!(snap.tabs[0].url.as_deref(), Some("https://a.test/"));
        assert_eq!(snap.tabs[0].title, "a.test");
    }

    #[test]
    fn first_navigation_of_a_blank_tab_is_not_a_history_entry() {
        let mut m = TabModel::new();
        let t = m.open(None);
        m.navigated(t, "https://a.test/".into());
        assert!(!m.snapshot().tabs[0].can_go_back);
    }

    #[test]
    fn go_on_a_blank_tab_is_a_noop() {
        let mut m = TabModel::new();
        let t = m.open(None);
        assert_eq!(m.go(t, -1), None);
        assert_eq!(m.go(t, 1), None);
        assert_eq!(m.current_url(t), None);
    }

    #[test]
    fn close_blank_tab_falls_back_to_neighbor() {
        let mut m = TabModel::new();
        let a = m.open(Some("https://a.test/".into()));
        let blank = m.open(None);
        assert_eq!(m.close(blank), Some(a));
    }

    #[test]
    fn contains_reports_tab_existence() {
        let mut m = TabModel::new();
        let a = m.open(None);
        assert!(m.contains(a));
        assert!(!m.contains(99));
    }

    #[test]
    fn open_activates_new_tab() {
        let (m, _a, b) = model_with_two_tabs();
        assert_eq!(m.active(), Some(b));
        assert_eq!(m.snapshot().tabs.len(), 2);
    }

    #[test]
    fn close_active_falls_back_to_neighbor() {
        let (mut m, a, b) = model_with_two_tabs();
        assert_eq!(m.close(b), Some(a));
        assert_eq!(m.active(), Some(a));
    }

    #[test]
    fn close_last_tab_leaves_none() {
        let mut m = TabModel::new();
        let a = m.open(Some("https://x.test/".into()));
        assert_eq!(m.close(a), None);
        assert_eq!(m.active(), None);
    }

    #[test]
    fn navigated_pushes_and_truncates_forward() {
        let mut m = TabModel::new();
        let t = m.open(Some("https://a.test/".into()));
        m.navigated(t, "https://b.test/".into());
        m.navigated(t, "https://c.test/".into());
        assert_eq!(m.go(t, -1), Some("https://b.test/".to_string()));
        // going somewhere new from the middle drops the forward entry
        m.navigated(t, "https://d.test/".into());
        assert_eq!(m.go(t, 1), None);
        assert_eq!(m.go(t, -1), Some("https://b.test/".to_string()));
    }

    #[test]
    fn navigated_collapses_consecutive_duplicates() {
        let mut m = TabModel::new();
        let t = m.open(Some("https://a.test/".into()));
        m.navigated(t, "https://a.test/".into()); // reload / redirect echo
        assert!(!m.snapshot().tabs[0].can_go_back);
    }

    #[test]
    fn go_out_of_range_is_none_and_keeps_cursor() {
        let mut m = TabModel::new();
        let t = m.open(Some("https://a.test/".into()));
        assert_eq!(m.go(t, -1), None);
        assert_eq!(m.current_url(t), Some("https://a.test/"));
    }

    #[test]
    fn snapshot_reports_history_affordances() {
        let mut m = TabModel::new();
        let t = m.open(Some("https://a.test/".into()));
        m.navigated(t, "https://b.test/".into());
        let snap = m.snapshot();
        assert!(snap.tabs[0].can_go_back);
        assert!(!snap.tabs[0].can_go_forward);
        m.go(t, -1);
        assert!(m.snapshot().tabs[0].can_go_forward);
    }

    #[test]
    fn ops_on_unknown_tab_are_noops() {
        let mut m = TabModel::new();
        assert!(!m.select(99));
        assert_eq!(m.go(99, -1), None);
        assert_eq!(m.close(99), None);
        m.navigated(99, "https://x.test/".into()); // must not panic
    }

    #[test]
    fn tab_title_derives_from_url() {
        assert_eq!(
            tab_title("https://elements.envato.com/"),
            "elements.envato.com"
        );
        assert_eq!(
            tab_title("https://elements.envato.com/dust-particles-overlay-ABC123"),
            "elements.envato.com — dust particles overlay ABC123"
        );
        assert_eq!(tab_title("not a url"), "not a url");
    }

    #[test]
    fn popup_decision_opens_web_urls() {
        assert_eq!(
            popup_decision("https://elements.envato.com/dust"),
            PopupDecision::OpenTab("https://elements.envato.com/dust".to_string())
        );
        assert_eq!(
            popup_decision("http://a.test/x"),
            PopupDecision::OpenTab("http://a.test/x".to_string())
        );
    }

    #[test]
    fn popup_decision_refuses_scriptable_blank_popups() {
        for raw in ["about:blank", "blob:https://a.test/x", "data:text/html,hi"] {
            assert!(
                matches!(popup_decision(raw), PopupDecision::Refuse(_)),
                "{raw}"
            );
        }
    }

    #[test]
    fn popup_decision_refuses_non_web_schemes() {
        for raw in ["file:///etc/passwd", "katto://ideas", "javascript:alert(1)"] {
            assert!(
                matches!(popup_decision(raw), PopupDecision::Refuse(_)),
                "{raw}"
            );
        }
    }

    #[test]
    fn popup_decision_refuses_unparseable_input() {
        assert!(matches!(
            popup_decision("not a url"),
            PopupDecision::Refuse(_)
        ));
    }

    #[test]
    fn allow_navigation_permits_web_schemes_only() {
        let ok = [
            "https://a.test/",
            "http://a.test/",
            "about:blank",
            "blob:https://a.test/x",
            "data:text/plain,hi",
        ];
        for u in ok {
            assert!(allow_navigation(&url::Url::parse(u).unwrap()), "{u}");
        }
        assert!(!allow_navigation(
            &url::Url::parse("file:///etc/passwd").unwrap()
        ));
        assert!(!allow_navigation(
            &url::Url::parse("katto://ideas").unwrap()
        ));
    }
}
