//! VFX cockpit (Phase 6): per-project `assets/vfx/<effect>/` folders are
//! truth; a watcher notices renders landing and surfaces them on the project
//! card. Pure classification is TDD'd; the watcher thread stays thin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// Renders count only as top-level `mp4`/`mov` files inside one effect folder.
const RENDER_EXTENSIONS: [&str; 2] = ["mp4", "mov"];
/// Watch-list re-sync cadence (projects can appear/move between syncs).
const RESYNC_INTERVAL: Duration = Duration::from_secs(60);
/// Repeat Modify storms for one file within this window are one landing.
const LANDING_COOLDOWN: Duration = Duration::from_secs(10);

/// Kebab-case a display name into a folder slug; `None` when nothing survives.
pub fn effect_slug(name: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_dash = true;
    for ch in name.to_lowercase().chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            ' ' | '_' | '-' => None,
            _ => continue,
        };
        match mapped {
            Some(ch) => {
                slug.push(ch);
                last_dash = false;
            }
            None => {
                if !last_dash {
                    slug.push('-');
                    last_dash = true;
                }
            }
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() { None } else { Some(slug) }
}

/// A render file that landed in an effect folder.
#[derive(Debug, PartialEq)]
pub struct RenderLanding {
    pub effect: String,
    pub file_name: String,
}

/// Some iff `changed` is exactly `vfx_root/<effect>/<file>` with a render
/// extension and not a dotfile (in-flight `.tmp` writes are dotfiles too).
pub fn classify_render_event(vfx_root: &Path, changed: &Path) -> Option<RenderLanding> {
    let rel = changed.strip_prefix(vfx_root).ok()?;
    let mut parts = rel.components();
    let effect = parts.next()?.as_os_str().to_str()?.to_string();
    let file_name = parts.next()?.as_os_str().to_str()?.to_string();
    if parts.next().is_some() {
        return None;
    }
    if file_name.starts_with('.') {
        return None;
    }
    let extension = Path::new(&file_name).extension()?.to_str()?.to_lowercase();
    if !RENDER_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    Some(RenderLanding { effect, file_name })
}

/// One effect folder and its renders, newest-first by mtime.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct VfxEffect {
    pub effect: String,
    pub path: String,
    pub renders: Vec<String>,
}

/// Scan `<project_root>/assets/vfx/*/` — folders are truth, no DB row.
pub fn list_effects(project_root: &Path) -> Vec<VfxEffect> {
    let vfx_root = project_root.join("assets/vfx");
    let Ok(entries) = std::fs::read_dir(&vfx_root) else {
        return Vec::new();
    };
    let mut effects: Vec<VfxEffect> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let effect = entry.file_name().to_str()?.to_string();
            let dir = entry.path();
            let mut renders: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(&dir)
                .ok()?
                .flatten()
                .filter_map(|file| {
                    let name = file.file_name().to_str()?.to_string();
                    classify_render_event(&vfx_root, &file.path())?;
                    let mtime = file.metadata().ok()?.modified().ok()?;
                    Some((mtime, name))
                })
                .collect();
            renders.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
            Some(VfxEffect {
                effect,
                path: dir.to_string_lossy().into_owned(),
                renders: renders.into_iter().map(|(_, name)| name).collect(),
            })
        })
        .collect();
    effects.sort_by(|a, b| a.effect.cmp(&b.effect));
    effects
}

/// Watch every known project's `assets/vfx/` for renders landing. One
/// background thread; the watch list re-syncs every 60 s so new projects and
/// new effects get covered without restart. Thread death is an events row.
pub fn start_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                record_event(
                    &app,
                    "vfx_watcher_died",
                    serde_json::json!({ "error": err.to_string() }),
                );
                return;
            }
        };
        // vfx dir -> project slug, for labeling landings.
        let mut watched: HashMap<PathBuf, String> = HashMap::new();
        let mut cooldown: HashMap<PathBuf, Instant> = HashMap::new();
        let mut last_sync: Option<Instant> = None;
        loop {
            if last_sync.is_none_or(|at| at.elapsed() >= RESYNC_INTERVAL) {
                sync_watch_list(&app, &mut watcher, &mut watched);
                last_sync = Some(Instant::now());
            }
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                    ) {
                        for path in event.paths {
                            handle_change(&app, &watched, &mut cooldown, &path);
                        }
                    }
                }
                Ok(Err(err)) => {
                    eprintln!("vfx watcher stream error: {err}");
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    record_event(
                        &app,
                        "vfx_watcher_died",
                        serde_json::json!({ "error": "event stream disconnected" }),
                    );
                    return;
                }
            }
        }
    });
}

/// Diff the watch list against the projects index: watch new `assets/vfx`
/// dirs, unwatch removed ones. Missing dirs are skipped (created lazily by
/// `create_vfx_effect`, which re-syncs by the next tick).
fn sync_watch_list(
    app: &AppHandle,
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut HashMap<PathBuf, String>,
) {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return;
    };
    let projects =
        tauri::async_runtime::block_on(state.db.call(|conn| crate::db::projects::list(conn)))
            .unwrap_or_default();
    let wanted: HashMap<PathBuf, String> = projects
        .into_iter()
        .filter_map(|project| {
            let dir = PathBuf::from(&project.root_path).join("assets/vfx");
            dir.is_dir().then_some((dir, project.slug))
        })
        .collect();
    let stale: Vec<PathBuf> = watched
        .keys()
        .filter(|dir| !wanted.contains_key(*dir))
        .cloned()
        .collect();
    for dir in stale {
        let _ = watcher.unwatch(&dir);
        watched.remove(&dir);
    }
    for (dir, slug) in wanted {
        if watched.contains_key(&dir) {
            continue;
        }
        if watcher.watch(&dir, RecursiveMode::Recursive).is_ok() {
            watched.insert(dir, slug);
        }
    }
}

fn handle_change(
    app: &AppHandle,
    watched: &HashMap<PathBuf, String>,
    cooldown: &mut HashMap<PathBuf, Instant>,
    changed: &Path,
) {
    let Some((vfx_root, slug)) = watched
        .iter()
        .find(|(dir, _)| changed.starts_with(dir.as_path()))
    else {
        return;
    };
    let Some(landing) = classify_render_event(vfx_root, changed) else {
        return;
    };
    if cooldown
        .get(changed)
        .is_some_and(|at| at.elapsed() < LANDING_COOLDOWN)
    {
        return;
    }
    // Marked at event time (not landing time): dedupes the burst of notify
    // events one write produces, and keeps the up-to-30 s stability wait off
    // this thread — a stalled render must not block other landings.
    cooldown.insert(changed.to_path_buf(), Instant::now());
    cooldown.retain(|_, at| at.elapsed() < LANDING_COOLDOWN * 6);
    let app = app.clone();
    let slug = slug.clone();
    let path = changed.to_path_buf();
    std::thread::spawn(move || {
        if !wait_for_stable_size(&path) {
            // Still growing past the cap (or vanished): a later write event
            // after the cooldown expires retries against the finished file.
            return;
        }
        record_event(
            &app,
            "vfx_render_landed",
            serde_json::json!({
                "project": slug,
                "effect": landing.effect,
                "file": landing.file_name,
            }),
        );
        crate::broadcast::vfx_render_landed(&app, &slug, &landing.effect, &landing.file_name);
    });
}

/// Size-stability debounce (volumes.rs lesson): a render is "landed" once its
/// size stops changing; capped so a stalled writer never wedges the thread.
fn wait_for_stable_size(path: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_len: Option<u64> = None;
    while Instant::now() < deadline {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        let len = meta.len();
        if last_len == Some(len) {
            return true;
        }
        last_len = Some(len);
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn record_event(app: &AppHandle, kind: &'static str, payload: serde_json::Value) {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return;
    };
    let db = state.db.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let payload = payload.to_string();
        let _ = db
            .call(move |conn| crate::db::events::record(conn, kind, None, Some(&payload)))
            .await;
        crate::broadcast::events_appended(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn effect_slug_kebabs_and_strips() {
        assert_eq!(effect_slug("Intro Glitch"), Some("intro-glitch".into()));
        assert_eq!(
            effect_slug("  éclair_flash 2 "),
            Some("clair-flash-2".into())
        );
        assert_eq!(effect_slug("!!!"), None);
    }

    #[test]
    fn classify_accepts_one_level_mp4_and_mov() {
        let root = Path::new("/p/assets/vfx");
        let hit =
            classify_render_event(root, Path::new("/p/assets/vfx/intro-glitch/final.MP4")).unwrap();
        assert_eq!(hit.effect, "intro-glitch");
        assert_eq!(hit.file_name, "final.MP4");
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/x/out.mov")).is_some());
    }

    #[test]
    fn classify_rejects_wrong_depth_ext_and_dotfiles() {
        let root = Path::new("/p/assets/vfx");
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/loose.mp4")).is_none());
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/a/b/deep.mp4")).is_none());
        assert!(classify_render_event(root, Path::new("/p/assets/vfx/a/project.aep")).is_none());
        assert!(
            classify_render_event(root, Path::new("/p/assets/vfx/a/.render.mp4.tmp")).is_none()
        );
        assert!(classify_render_event(root, Path::new("/elsewhere/a/x.mp4")).is_none());
    }

    #[test]
    fn list_effects_scans_folders_and_sorts_renders() {
        let tmp = std::env::temp_dir().join(format!("vfx-test-{}", std::process::id()));
        let dir = tmp.join("assets/vfx/glitch");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("v1.mp4"), b"a").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let effects = list_effects(&tmp);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].effect, "glitch");
        assert_eq!(effects[0].renders, vec!["v1.mp4".to_string()]);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
