//! One thumbnails watch at a time (the open project detail): NonRecursive on
//! `<project>/thumbnails/`, 500 ms debounce, `ThumbnailsChanged` broadcast.
//! Watcher death is an events row, never a crash. Thin spawn site — the pure
//! pick logic lives in `naming.rs`.

use std::path::PathBuf;
use std::time::Duration;

use notify::Watcher;
use tauri::{AppHandle, Manager};

/// A live watch on one project's `thumbnails/` dir. Dropping it stops the
/// watcher (its channel disconnects and the debounce thread exits).
pub struct ThumbWatch {
    pub slug: String,
    _watcher: notify::RecommendedWatcher,
}

/// Start watching `dir` for PNG exports landing.
pub fn start(app: AppHandle, slug: String, dir: PathBuf) -> crate::error::Result<ThumbWatch> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)
        .map_err(|e| crate::error::Error::Io(format!("thumbnails watcher: {e}")))?;
    watcher
        .watch(&dir, notify::RecursiveMode::NonRecursive)
        .map_err(|e| crate::error::Error::Io(format!("watch {}: {e}", dir.display())))?;

    let thread_slug = slug.clone();
    std::thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if !is_png_change(&event) {
                        continue;
                    }
                    // debounce: drain the storm until 500 ms of quiet
                    while let Ok(next) = rx.recv_timeout(Duration::from_millis(500)) {
                        let _ = next;
                    }
                    crate::broadcast::thumbnails_changed(&app, &thread_slug);
                }
                Ok(Err(err)) => {
                    record_watch_failed(&app, &thread_slug, &err.to_string());
                    return;
                }
                // watcher dropped (replaced or unwatched) — normal stop
                Err(_) => return,
            }
        }
    });
    Ok(ThumbWatch {
        slug,
        _watcher: watcher,
    })
}

fn is_png_change(event: &notify::Event) -> bool {
    // Remove matters too: deleting the newest PNG must re-pick the card
    if !matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    ) {
        return false;
    }
    event.paths.iter().any(|path| {
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_event(kind: notify::EventKind) -> notify::Event {
        notify::Event::new(kind).add_path(PathBuf::from("/p/thumbnails/x.png"))
    }

    #[test]
    fn create_modify_and_remove_of_png_all_signal() {
        use notify::event::{CreateKind, ModifyKind, RemoveKind};
        assert!(is_png_change(&png_event(notify::EventKind::Create(
            CreateKind::File
        ))));
        assert!(is_png_change(&png_event(notify::EventKind::Modify(
            ModifyKind::Any
        ))));
        assert!(is_png_change(&png_event(notify::EventKind::Remove(
            RemoveKind::File
        ))));
    }

    #[test]
    fn non_png_and_access_events_are_ignored() {
        use notify::event::{AccessKind, RemoveKind};
        assert!(!is_png_change(
            &notify::Event::new(notify::EventKind::Remove(RemoveKind::File))
                .add_path(PathBuf::from("/p/thumbnails/x.psd"))
        ));
        assert!(!is_png_change(&png_event(notify::EventKind::Access(
            AccessKind::Any
        ))));
    }
}

fn record_watch_failed(app: &AppHandle, slug: &str, error: &str) {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return;
    };
    let payload = serde_json::json!({ "slug": slug, "error": error }).to_string();
    let slug = slug.to_string();
    let db = state.db.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = db
            .call(move |conn| {
                crate::db::events::record(
                    conn,
                    "thumbnail_watch_failed",
                    Some(&slug),
                    Some(&payload),
                )
            })
            .await;
        crate::broadcast::events_appended(&app);
    });
}
