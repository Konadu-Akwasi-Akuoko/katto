//! The non-recursive `/Volumes` watcher: detects camera-card mounts, debounces
//! until the card is readable, and publishes a [`CardOffer`] via managed state,
//! a broadcast event, and a notification. The pure readiness gate and the walk
//! are unit-tested; the notify wiring is a single watch site covered by the
//! manual hardware checkpoint.

use std::path::{Path, PathBuf};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use katto_engine::ingest::{FileEntry, VolumeTree, recognize::recognize};

use crate::broadcast;
use crate::commands::ingest::build_offer;
use crate::state::IngestState;

/// True when the freshly-mounted volume already exposes a recognizable card
/// layout — used to debounce: a card that isn't ready yet returns `false` and
/// the watcher retries. Pure over the walked tree.
pub fn is_card_ready(tree: &VolumeTree) -> bool {
    recognize(tree).is_some()
}

/// Walk a mounted volume into the in-memory shapes the engine consumes: the full
/// `VolumeTree` (for recognition) and the per-file list with sizes (for
/// enumeration). Skips unreadable entries rather than failing the walk.
pub fn walk_volume(root: &Path) -> (VolumeTree, Vec<FileEntry>) {
    fn rec(base: &Path, dir: &Path, entries: &mut Vec<PathBuf>, files: &mut Vec<FileEntry>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(base) else {
                continue;
            };
            entries.push(rel.to_path_buf());
            if path.is_dir() {
                rec(base, &path, entries, files);
            } else if let Ok(meta) = entry.metadata() {
                files.push(FileEntry {
                    path: rel.to_path_buf(),
                    size: meta.len(),
                });
            }
        }
    }
    let mut entries = Vec::new();
    let mut files = Vec::new();
    rec(root, root, &mut entries, &mut files);
    (VolumeTree { entries }, files)
}

/// Start the non-recursive `/Volumes` watcher on a background thread. Kept alive
/// for the process lifetime by moving the watcher into the spawned thread.
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(tx) else {
            return;
        };
        if watcher
            .watch(Path::new("/Volumes"), RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }
        for res in rx {
            let Ok(event) = res else { continue };
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
    });
}

/// Debounce a new mount until it exposes a card layout (up to ~1 s), then build
/// and publish the offer.
fn handle_mount(app: &AppHandle, mount: &Path) {
    for _ in 0..10 {
        let (tree, files) = walk_volume(mount);
        if is_card_ready(&tree) {
            if let Some(offer) = build_offer(mount, &tree, &files) {
                if let Ok(mut guard) = app.state::<IngestState>().current.lock() {
                    *guard = Some(offer.clone());
                }
                broadcast::card_detected(app, offer);
                let _ = crate::notify::notify(
                    app,
                    "Camera card ready",
                    "Import clips into a project",
                    "katto://ingest",
                );
            }
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

    #[test]
    fn ready_when_tree_recognizes_as_card() {
        let tree = VolumeTree {
            entries: vec![
                PathBuf::from("PRIVATE/M4ROOT/CLIP"),
                PathBuf::from("PRIVATE/M4ROOT/CLIP/C0001.MP4"),
            ],
        };
        assert!(is_card_ready(&tree));
    }

    #[test]
    fn not_ready_when_empty() {
        assert!(!is_card_ready(&VolumeTree::default()));
    }

    #[test]
    fn walk_reads_a_fixture_tree_from_disk() {
        // Reuses the engine's committed Sony fixture via a relative path.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/katto-engine/tests/fixtures/cards/sony");
        if root.exists() {
            let (tree, files) = walk_volume(&root);
            assert!(is_card_ready(&tree));
            assert!(files.iter().any(|f| f.path.ends_with("C0001.MP4")));
        }
    }
}
