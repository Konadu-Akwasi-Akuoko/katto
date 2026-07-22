//! The non-recursive `/Volumes` watcher: detects camera-card mounts, debounces
//! until the card is readable, and publishes a [`CardOffer`] via managed state,
//! a broadcast event, and a notification. Recognition never walks a whole
//! volume — it probes only the fixed marker paths (`PRIVATE/M4ROOT/CLIP`,
//! `DCIM/*`), and the file walk is bounded to the recognized clip roots, so a
//! mounted multi-terabyte non-camera drive costs a handful of stats. The
//! notify wiring is a single watch site covered by the manual hardware
//! checkpoint.

use std::path::{Path, PathBuf};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use katto_engine::ingest::{Card, FileEntry, VolumeTree, recognize::recognize};

use crate::broadcast;
use crate::ingest::offer::{build_offer, fill_durations};
use crate::state::IngestState;

/// Shallow-probe a mount's card markers into a minimal [`VolumeTree`] that
/// [`recognize`] can classify — a few `stat`s and at most two one-level
/// `read_dir`s (`DCIM/` and its subdirs' first entry), never a full walk.
pub fn probe_tree(mount: &Path) -> VolumeTree {
    let mut entries: Vec<PathBuf> = Vec::new();

    for marker in ["PRIVATE/M4ROOT/CLIP", "PRIVATE/M4ROOT/SUB"] {
        if mount.join(marker).is_dir() {
            entries.push(PathBuf::from(marker));
        }
    }

    let dcim = mount.join("DCIM");
    if dcim.is_dir() {
        entries.push(PathBuf::from("DCIM"));
        if let Ok(read) = std::fs::read_dir(&dcim) {
            for entry in read.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(name) = path.file_name().map(PathBuf::from) else {
                    continue;
                };
                let rel = PathBuf::from("DCIM").join(&name);
                // `recognize` requires evidence of content beneath a DCIM
                // subfolder; surface the first child only.
                if let Some(child) = std::fs::read_dir(&path)
                    .ok()
                    .and_then(|mut rd| rd.next())
                    .and_then(|e| e.ok())
                {
                    entries.push(rel.clone());
                    entries.push(rel.join(child.file_name()));
                }
            }
        }
    }

    VolumeTree { entries }
}

/// Walk ONLY the recognized clip roots into the per-file list the engine's
/// `enumerate` consumes. Skips unreadable entries rather than failing.
pub fn walk_clip_roots(mount: &Path, card: &Card) -> Vec<FileEntry> {
    fn rec(base: &Path, dir: &Path, files: &mut Vec<FileEntry>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(base) else {
                continue;
            };
            if path.is_dir() {
                rec(base, &path, files);
            } else if let Ok(meta) = entry.metadata() {
                files.push(FileEntry {
                    path: rel.to_path_buf(),
                    size: meta.len(),
                });
            }
        }
    }
    let mut files = Vec::new();
    for root in &card.clip_roots {
        rec(mount, &mount.join(root), &mut files);
    }
    files
}

/// Start the non-recursive `/Volumes` watcher on a background thread. Kept
/// alive for the process lifetime by moving the watcher into the spawned
/// thread; if the event stream ever ends, that is recorded as an events row —
/// card detection dying silently would violate "nothing fails silently".
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(err) => {
                record_watcher_event(&app, "volume_watcher_failed", err.to_string());
                return;
            }
        };
        if let Err(err) = watcher.watch(Path::new("/Volumes"), RecursiveMode::NonRecursive) {
            record_watcher_event(&app, "volume_watcher_failed", err.to_string());
            return;
        }
        for res in rx {
            let event = match res {
                Ok(event) => event,
                Err(err) => {
                    eprintln!("volume watcher stream error: {err}");
                    continue;
                }
            };
            match event.kind {
                notify::EventKind::Create(_) => {
                    for path in event.paths {
                        handle_mount(&app, &path);
                    }
                }
                notify::EventKind::Remove(_) => {
                    for path in event.paths {
                        handle_unmount(&app, &path);
                    }
                }
                _ => {}
            }
        }
        record_watcher_event(
            &app,
            "volume_watcher_died",
            "event channel closed".to_string(),
        );
    });
}

/// Record a watcher lifecycle failure as an events row ("nothing fails
/// silently") — card detection is off for the session and the activity log is
/// where that becomes visible.
fn record_watcher_event(app: &AppHandle, kind: &'static str, message: String) {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return;
    };
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let payload = serde_json::json!({ "error": message }).to_string();
        let outcome = db
            .call(move |conn| crate::db::events::record(conn, kind, None, Some(&payload)))
            .await;
        if let Err(err) = outcome {
            eprintln!("failed to record {kind} event: {err}");
        }
    });
}

/// Debounce a new mount until its marker paths expose a card layout (up to
/// ~1 s of cheap probes), then walk only the clip roots, publish the offer
/// (durations pending), and kick off the async duration fill.
fn handle_mount(app: &AppHandle, mount: &Path) {
    for _ in 0..10 {
        let tree = probe_tree(mount);
        if let Some(card) = recognize(&tree) {
            let files = walk_clip_roots(mount, &card);
            let offer = build_offer(mount, &card, &files);
            if let Ok(mut guard) = app.state::<IngestState>().current.lock() {
                *guard = Some(offer.clone());
            }
            broadcast::card_detected(app);
            if let Err(err) = crate::notify::notify(
                app,
                "Camera card ready",
                "Import clips into a project",
                "katto://ingest",
            ) {
                eprintln!("card-ready notification failed: {err}");
            }
            fill_durations(app.clone(), offer);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Clear the offer if the unmounted volume was the detected card.
fn handle_unmount(app: &AppHandle, mount: &Path) {
    let mount_str = mount.to_string_lossy().into_owned();
    let ingest = app.state::<IngestState>();
    let Ok(mut guard) = ingest.current.lock() else {
        return;
    };
    if guard.as_ref().is_some_and(|o| o.volume == mount_str) {
        *guard = None;
        drop(guard);
        broadcast::card_removed(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use katto_engine::ingest::CardKind;

    fn sony_fixture() -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/katto-engine/tests/fixtures/cards/sony");
        assert!(
            root.exists(),
            "engine sony card fixture is missing at {}",
            root.display()
        );
        root
    }

    #[test]
    fn probe_tree_recognizes_the_sony_fixture_without_a_full_walk() {
        let tree = probe_tree(&sony_fixture());
        let card = recognize(&tree).expect("sony fixture must recognize");
        assert_eq!(card.kind, CardKind::Sony);
        // Shallow probe: nothing beneath CLIP/ is enumerated here (the clip
        // walk happens later, bounded to the recognized roots).
        assert!(
            !tree
                .entries
                .iter()
                .any(|e| e.starts_with("PRIVATE/M4ROOT/CLIP")
                    && e != Path::new("PRIVATE/M4ROOT/CLIP"))
        );
    }

    #[test]
    fn probe_tree_ignores_a_non_camera_volume() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/katto-engine/tests/fixtures/cards/not-a-card");
        assert!(root.exists(), "not-a-card fixture is missing");
        assert!(recognize(&probe_tree(&root)).is_none());
    }

    #[test]
    fn walk_clip_roots_lists_only_files_under_the_roots() {
        let mount = sony_fixture();
        let tree = probe_tree(&mount);
        let card = recognize(&tree).expect("sony fixture must recognize");
        let files = walk_clip_roots(&mount, &card);
        assert!(files.iter().any(|f| f.path.ends_with("C0001.MP4")));
        assert!(
            files
                .iter()
                .all(|f| card.clip_roots.iter().any(|r| f.path.starts_with(r)))
        );
    }

    #[test]
    fn probe_tree_classifies_an_iphone_dcim_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/katto-engine/tests/fixtures/cards/iphone-dcim");
        assert!(root.exists(), "iphone-dcim fixture is missing");
        let card = recognize(&probe_tree(&root)).expect("iphone fixture must recognize");
        assert_eq!(card.kind, CardKind::IphoneDcim);
    }
}
