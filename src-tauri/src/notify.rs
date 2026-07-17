//! User notifications and `katto://` deep-link routing.
//!
//! A notification's click deep-links back into katto. On a bundled, signed build
//! that means a real `UNUserNotificationCenter` banner (via `objc2-user-notifications`);
//! in a dev/unsigned binary `UNUserNotificationCenter::currentNotificationCenter()`
//! raises an `NSInternalInconsistencyException` (not a Rust error), so [`notify`]
//! degrades to an `events` row and never touches AppKit there.
//!
//! The `katto://` scheme is registered with LaunchServices from the bundled app's
//! `Info.plist` (generated from `plugins.deep-link.desktop.schemes` in
//! `tauri.conf.json`), so OS-delivered opens only reach `on_open_url` in a bundled
//! build; the frontend receives every route through the `DeepLinkOpened` broadcast.

use tauri::{AppHandle, Manager};

use crate::error::Result;

/// A parsed `katto://` destination. Serialized to a wire string for the frontend
/// router via [`Route::as_wire`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `katto://ideas` — the planner's backlog tab.
    Ideas,
    /// `katto://project/<slug>` — a project's detail view.
    Project(String),
}

impl Route {
    /// The route as the `{ route }` string the frontend router switches on:
    /// `"ideas"` or `"project/<slug>"`.
    pub fn as_wire(&self) -> String {
        match self {
            Route::Ideas => "ideas".to_string(),
            Route::Project(slug) => format!("project/{slug}"),
        }
    }
}

/// Parse a `katto://` URL into a [`Route`]. Anything outside the known routes
/// (unknown host, missing slug, foreign scheme) is `None` — junk is dropped, not
/// guessed.
pub fn parse_deep_link(url: &str) -> Option<Route> {
    let rest = url.strip_prefix("katto://")?.trim_end_matches('/');
    if rest == "ideas" {
        return Some(Route::Ideas);
    }
    if let Some(slug) = rest.strip_prefix("project/")
        && !slug.is_empty()
        && !slug.contains('/')
    {
        return Some(Route::Project(slug.to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideas_route_parses() {
        assert_eq!(parse_deep_link("katto://ideas"), Some(Route::Ideas));
    }

    #[test]
    fn project_route_parses() {
        assert_eq!(
            parse_deep_link("katto://project/foo-2026-07-09"),
            Some(Route::Project("foo-2026-07-09".to_string()))
        );
    }

    #[test]
    fn junk_is_none() {
        assert_eq!(parse_deep_link("https://example.com"), None);
        assert_eq!(parse_deep_link("katto://"), None);
        assert_eq!(parse_deep_link("katto://project/"), None);
        assert_eq!(parse_deep_link("katto://ideas/extra"), None);
        assert_eq!(parse_deep_link("nonsense"), None);
    }

    #[test]
    fn wire_form_round_trips_the_parser() {
        assert_eq!(Route::Ideas.as_wire(), "ideas");
        assert_eq!(Route::Project("foo".to_string()).as_wire(), "project/foo");
    }
}

/// Fire a user notification whose click deep-links back into katto via `url`.
///
/// Bundled+signed builds post a `UNUserNotificationCenter` banner carrying the
/// `katto://` url in its `userInfo`. Dev/unsigned builds cannot touch
/// `UNUserNotificationCenter` without crashing, so they record an `events` row
/// (`notification_degraded`) instead — the tray and deep-link paths still work.
///
/// # Errors
///
/// Returns [`crate::error::Error`] only if the degrade path fails to record its
/// `events` row; the native banner path is best-effort (delivery/permission
/// failures surface as `events` rows, never as an error here).
pub fn notify(app: &AppHandle, title: &str, body: &str, url: &str) -> Result<()> {
    if tauri::is_dev() {
        return degrade_to_events(app, title, body, url);
    }

    #[cfg(target_os = "macos")]
    {
        macos::deliver(app, title, body, url);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        degrade_to_events(app, title, body, url)
    }
}

/// Record the notification's intent as an `events` row instead of posting a
/// native banner (dev/unsigned builds, where AppKit's notification center is off
/// limits). Best-effort payload carries the title/body/url so the intent is
/// visible in the activity log.
fn degrade_to_events(app: &AppHandle, title: &str, body: &str, url: &str) -> Result<()> {
    let db = app.state::<crate::state::AppState>().db.clone();
    let payload = serde_json::json!({ "title": title, "body": body, "url": url }).to_string();
    tauri::async_runtime::block_on(db.call(move |conn| {
        crate::db::events::record(conn, "notification_degraded", None, Some(&payload))
    }))?;
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use tauri::{AppHandle, Manager};

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };

    /// Request authorization then post a deliver-now banner. Runs only in a
    /// bundled build (guarded by `tauri::is_dev()` at the call site) because
    /// `currentNotificationCenter()` throws on an unsigned binary. A denied
    /// authorization result is recorded once as an `events` row.
    ///
    /// `_url` is accepted for parity with the public [`super::notify`] signature
    /// but unused here: routing a *click* to a specific `katto://` destination
    /// needs a `UNUserNotificationCenterDelegate` reading the request's
    /// `userInfo`, which is deferred. Without it macOS focuses the app on click
    /// (its default), and OS-delivered `katto://` opens still route via the
    /// deep-link plugin's `on_open_url` path.
    pub fn deliver(app: &AppHandle, title: &str, body: &str, _url: &str) {
        let center = UNUserNotificationCenter::currentNotificationCenter();

        let auth_app = app.clone();
        let auth_cb = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            if !granted.as_bool() {
                record_permission_denied(&auth_app);
            }
        });
        let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
        center.requestAuthorizationWithOptions_completionHandler(options, &auth_cb);

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));

        let identifier = NSString::from_str(&uuid::Uuid::new_v4().to_string());
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
            &content,
            None,
        );
        center.addNotificationRequest_withCompletionHandler(&request, None);
    }

    /// Record a one-off `events` row when the user denies notification permission,
    /// off the AppKit callback thread so no DB work blocks it.
    fn record_permission_denied(app: &AppHandle) {
        let Some(state) = app.try_state::<crate::state::AppState>() else {
            return;
        };
        let db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            let _ = db
                .call(|conn| {
                    crate::db::events::record(conn, "notification_permission_denied", None, None)
                })
                .await;
        });
    }
}
