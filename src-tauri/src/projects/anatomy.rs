use std::path::{Path, PathBuf};

use super::manifest::{MANIFEST_FILE, ProjectManifest, write_manifest};
use crate::error::{Error, Result};

/// The D6 project folder anatomy. Nested entries (`assets/envato`) are created by
/// `create_dir_all`, which builds the `assets/` intermediate on the way down.
pub const PROJECT_SUBFOLDERS: [&str; 8] = [
    "footage",
    "audio",
    "assets/envato",
    "assets/vfx",
    "assets/graphics",
    "thumbnails",
    "timelines",
    "exports",
];

/// Create `<projects_root>/<slug>/`, every D6 subfolder, and its validated
/// `project.json`. Returns the created project directory.
///
/// The slug is `manifest.slug`; `write_manifest` therefore validates the
/// manifest against a directory whose name already equals the slug.
///
/// # Errors
/// - `Error::Io` if the target project folder already exists (an explicit guard:
///   `create_dir_all` silently no-ops on an existing directory, so refusing an
///   occupied slug has to be a pre-check, not a side effect of creation).
/// - `Error::Io` if any directory creation fails.
/// - `Error::InvalidManifest` / `Error::Io` propagated from `write_manifest`.
pub fn create_project_skeleton(
    projects_root: &Path,
    manifest: &ProjectManifest,
) -> Result<PathBuf> {
    let project_dir = projects_root.join(&manifest.slug);
    if project_dir.exists() {
        return Err(Error::Io(format!(
            "project folder already exists: {}",
            project_dir.display()
        )));
    }
    std::fs::create_dir_all(&project_dir)?;
    for sub in PROJECT_SUBFOLDERS {
        std::fs::create_dir_all(project_dir.join(sub))?;
    }
    write_manifest(&project_dir, manifest)?;
    Ok(project_dir)
}

/// Best-effort removal of a project skeleton, used to roll back a failed promote.
///
/// Refuses to touch user data: the directory is removed only when its contents
/// are exactly the D6 skeleton plus `project.json`. Any stray file — a clip
/// dropped under `footage/`, an export, an unexpected top-level entry — leaves
/// the whole tree intact and returns `Error::Io`. Validation runs to completion
/// before a single entry is deleted, so a refusal never leaves a half-removed
/// folder behind.
///
/// # Errors
/// - `Error::Io` if any stray (non-skeleton) entry is present, or if a
///   filesystem read/remove fails.
pub fn remove_skeleton(project_dir: &Path) -> Result<()> {
    if !project_dir.exists() {
        return Ok(());
    }

    // 1. Every leaf subfolder must be empty.
    for sub in PROJECT_SUBFOLDERS {
        let dir = project_dir.join(sub);
        if dir.exists() && has_any_entry(&dir)? {
            return Err(stray_data(&dir));
        }
    }

    // 2. The `assets/` intermediate holds only its three known leaves.
    let assets = project_dir.join("assets");
    if assets.exists() {
        for entry in std::fs::read_dir(&assets)? {
            let entry = entry?;
            let name = entry.file_name();
            if !matches!(
                name.to_string_lossy().as_ref(),
                "envato" | "vfx" | "graphics"
            ) {
                return Err(stray_data(&entry.path()));
            }
        }
    }

    // 3. The project dir holds only skeleton top-level dirs + the manifest.
    let allowed_top = allowed_top_level();
    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == MANIFEST_FILE {
            continue;
        }
        if !allowed_top.iter().any(|a| *a == name.as_ref()) {
            return Err(stray_data(&entry.path()));
        }
    }

    // Validated clean — remove leaves, then `assets/`, then manifest, then root.
    for sub in PROJECT_SUBFOLDERS {
        let dir = project_dir.join(sub);
        if dir.exists() {
            std::fs::remove_dir(&dir)?;
        }
    }
    if assets.exists() {
        std::fs::remove_dir(&assets)?;
    }
    let manifest = project_dir.join(MANIFEST_FILE);
    if manifest.exists() {
        std::fs::remove_file(&manifest)?;
    }
    std::fs::remove_dir(project_dir)?;
    Ok(())
}

/// Top-level entries a bare skeleton may contain: the first path component of
/// each subfolder (`assets/envato` → `assets`), deduplicated.
fn allowed_top_level() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PROJECT_SUBFOLDERS
        .iter()
        .map(|s| s.split('/').next().unwrap_or(s))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn has_any_entry(dir: &Path) -> Result<bool> {
    Ok(std::fs::read_dir(dir)?.next().is_some())
}

fn stray_data(path: &Path) -> Error {
    Error::Io(format!(
        "refusing to remove project: stray data present at {}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::manifest::{MANIFEST_SCHEMA_VERSION, read_manifest};

    fn manifest(slug: &str) -> ProjectManifest {
        ProjectManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            slug: slug.to_string(),
            title: "NVMe Deep Dive".to_string(),
            status: "idea".to_string(),
            target_nle: "resolve".to_string(),
            shoot_date: None,
            publish_date: None,
            created_at: "2026-07-09T10:00:00Z".to_string(),
            links: serde_json::Map::new(),
        }
    }

    #[test]
    fn skeleton_creates_all_subfolders_and_manifest() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();

        assert_eq!(dir, root.path().join(slug));
        for sub in PROJECT_SUBFOLDERS {
            assert!(dir.join(sub).is_dir(), "missing subfolder {sub}");
        }
        // read_manifest validates schema_version and slug == dir name.
        let read = read_manifest(&dir).unwrap();
        assert_eq!(read.slug, slug);
    }

    #[test]
    fn skeleton_errors_when_dir_already_exists() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        std::fs::create_dir(root.path().join(slug)).unwrap();
        assert!(create_project_skeleton(root.path(), &manifest(slug)).is_err());
    }

    #[test]
    fn remove_skeleton_removes_a_fresh_skeleton() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();
        remove_skeleton(&dir).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn remove_skeleton_refuses_and_keeps_data_on_stray_file() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();
        let stray = dir.join("footage").join("clip.mov");
        std::fs::write(&stray, b"user footage").unwrap();

        assert!(remove_skeleton(&dir).is_err());
        assert!(dir.exists(), "project dir must survive a refused removal");
        assert!(
            stray.exists(),
            "stray user data must survive a refused removal"
        );
    }
}
