use std::path::{Path, PathBuf};

use super::manifest::{MANIFEST_FILE, ProjectManifest, write_manifest};
use crate::error::{Error, Result};

/// The D6 project folder anatomy. Nested entries (`assets/envato`) are created by
/// `create_dir_all`, which builds the `assets/` intermediate on the way down.
pub const PROJECT_SUBFOLDERS: [&str; 10] = [
    "footage",
    "audio",
    "assets/envato",
    "assets/vfx",
    "assets/graphics",
    "assets/music",
    "assets/sfx",
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
    ensure_subfolders(&project_dir)?;
    write_manifest(&project_dir, manifest)?;
    Ok(project_dir)
}

/// Create every missing D6 subfolder inside an existing project directory.
/// Idempotent — `create_dir_all` no-ops on a folder that is already there — so
/// this is both the skeleton's creation loop and the launch-time backfill that
/// brings projects created before an anatomy change up to date.
///
/// A `project_dir` that is not a directory is a no-op: `create_dir_all` builds
/// missing ancestors, so without this guard a reconcile racing a folder the
/// owner just trashed would resurrect it as a hollow shell with no manifest.
///
/// # Errors
/// `Error::Io` if a directory creation fails — a read-only volume, or a regular
/// file squatting a subfolder's name.
pub fn ensure_subfolders(project_dir: &Path) -> Result<()> {
    if !project_dir.is_dir() {
        return Ok(());
    }
    for sub in PROJECT_SUBFOLDERS {
        std::fs::create_dir_all(project_dir.join(sub))?;
    }
    Ok(())
}

/// Whether `name` is exactly one of the anatomy's relative subfolder paths.
/// The reveal-in-Finder allowlist: only a value that came out of
/// [`PROJECT_SUBFOLDERS`] is ever joined onto a project directory, so a
/// traversal segment can never reach outside it.
pub fn is_project_subfolder(name: &str) -> bool {
    PROJECT_SUBFOLDERS.contains(&name)
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
/// Both the intermediate-directory check and the removal order are derived from
/// [`PROJECT_SUBFOLDERS`], so a new nested entry cannot drift out of validation:
/// an entry another entry nests inside is skipped by the leaf passes and handled
/// as an intermediate instead (see [`leaves`]), and intermediates are removed
/// deepest-first. Together that is what stops a `remove_dir` ever running on a
/// parent whose anatomy children are still there.
///
/// # Errors
/// - `Error::Io` if any stray (non-skeleton) entry is present, or if a
///   filesystem read/remove fails.
pub fn remove_skeleton(project_dir: &Path) -> Result<()> {
    if !project_dir.exists() {
        return Ok(());
    }

    // 1. Every leaf subfolder must be empty.
    for sub in leaves(&PROJECT_SUBFOLDERS) {
        let dir = project_dir.join(sub);
        if dir.exists() && has_any_entry(&dir)? {
            return Err(stray_data(&dir));
        }
    }

    // 2. Every intermediate directory holds only the children the anatomy puts
    //    there. Derived from the const so a new nested entry cannot drift out of
    //    this check — and a non-directory child (a browser download filed
    //    straight into `assets/`) is still stray data.
    for inter in intermediates(&PROJECT_SUBFOLDERS) {
        let dir = project_dir.join(inter);
        if !dir.exists() {
            continue;
        }
        let allowed = allowed_children(inter);
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !entry.file_type()?.is_dir() || !allowed.iter().any(|a| *a == name.as_ref()) {
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

    // Validated clean — remove leaves, then intermediates, then manifest, then root.
    for sub in leaves(&PROJECT_SUBFOLDERS) {
        let dir = project_dir.join(sub);
        if dir.exists() {
            std::fs::remove_dir(&dir)?;
        }
    }
    for inter in intermediates(&PROJECT_SUBFOLDERS) {
        let dir = project_dir.join(inter);
        if dir.exists() {
            std::fs::remove_dir(&dir)?;
        }
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

/// The anatomy entries no other entry nests inside. Only these must be empty for
/// a removal to proceed, and only these are removed directly. An entry that
/// acquires an anatomy child (`assets/envato`, the day `assets/envato/audio`
/// joins the const) is by then also an intermediate, so it is validated against
/// its allowed children and removed after them — never treated as a leaf that
/// must be empty, and never removed ahead of its children.
///
/// Takes the set rather than reading the const so a hypothetical deeper anatomy
/// is testable before it exists.
fn leaves(subfolders: &[&'static str]) -> Vec<&'static str> {
    subfolders
        .iter()
        .copied()
        .filter(|sub| {
            !subfolders.iter().any(|other| {
                other
                    .strip_prefix(*sub)
                    .is_some_and(|rest| rest.starts_with('/'))
            })
        })
        .collect()
}

/// Intermediate directories implied by the nested anatomy entries
/// (`assets/envato` implies `assets`), deepest first — the order they must be
/// removed in, since a parent cannot be removed before its children.
fn intermediates(subfolders: &[&'static str]) -> Vec<&'static str> {
    let mut dirs: Vec<&'static str> = subfolders
        .iter()
        .copied()
        .flat_map(|sub| sub.match_indices('/').map(move |(i, _)| &sub[..i]))
        .collect();
    dirs.sort_unstable();
    dirs.dedup();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    dirs
}

/// The entry names an intermediate directory may contain: the next path
/// component of every anatomy entry that lives under it.
fn allowed_children(intermediate: &str) -> Vec<&'static str> {
    let prefix = format!("{intermediate}/");
    let mut names: Vec<&'static str> = PROJECT_SUBFOLDERS
        .into_iter()
        .filter_map(|sub| sub.strip_prefix(prefix.as_str()))
        .map(|rest| rest.split('/').next().unwrap_or(rest))
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
            priority: None,
            kind: None,
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
    fn ensure_subfolders_creates_missing_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("footage")).unwrap();

        ensure_subfolders(dir.path()).unwrap();

        for sub in PROJECT_SUBFOLDERS {
            assert!(dir.path().join(sub).is_dir(), "missing subfolder {sub}");
        }
    }

    #[test]
    fn ensure_subfolders_is_idempotent_on_a_full_skeleton() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();

        ensure_subfolders(&dir).unwrap();

        for sub in PROJECT_SUBFOLDERS {
            assert!(dir.join(sub).is_dir(), "missing subfolder {sub}");
        }
    }

    #[test]
    fn ensure_subfolders_on_a_missing_dir_creates_nothing() {
        let root = tempfile::tempdir().unwrap();
        let gone = root.path().join("trashed-2026-07-09");

        ensure_subfolders(&gone).unwrap();

        assert!(
            !gone.exists(),
            "a missing project dir must not be recreated"
        );
    }

    #[test]
    fn is_project_subfolder_accepts_the_anatomy_and_rejects_escapes() {
        for sub in PROJECT_SUBFOLDERS {
            assert!(is_project_subfolder(sub), "anatomy entry rejected: {sub}");
        }
        assert!(is_project_subfolder("assets/music"));
        assert!(is_project_subfolder("assets/sfx"));

        for bad in ["", "..", "../..", "assets", "assets/music/../..", "/etc"] {
            assert!(!is_project_subfolder(bad), "escape accepted: {bad:?}");
        }
    }

    #[test]
    fn allowed_children_of_assets_covers_every_assets_leaf() {
        let mut expected: Vec<&str> = PROJECT_SUBFOLDERS
            .into_iter()
            .filter_map(|sub| sub.strip_prefix("assets/"))
            .collect();
        expected.sort_unstable();

        assert_eq!(allowed_children("assets"), expected);
        assert!(expected.contains(&"music"));
        assert!(expected.contains(&"sfx"));
    }

    #[test]
    fn leaves_of_the_current_anatomy_is_every_entry() {
        assert_eq!(leaves(&PROJECT_SUBFOLDERS), PROJECT_SUBFOLDERS.to_vec());
    }

    #[test]
    fn leaves_excludes_an_entry_a_deeper_entry_nests_inside() {
        // The next anatomy growth this module has to survive: a third level.
        // `assets/envato` stops being a leaf the moment a child joins, or
        // remove_skeleton would demand it be empty and then remove it before
        // that child.
        let deeper = ["assets/envato", "assets/envato/audio", "assets/sfx"];
        assert_eq!(
            leaves(&deeper),
            vec!["assets/envato/audio", "assets/sfx"],
            "an entry with an anatomy child is handled as an intermediate"
        );
        let inter = intermediates(&deeper);
        assert!(
            inter.contains(&"assets/envato"),
            "and it must be covered as one: {inter:?}"
        );
        assert_eq!(
            inter.first(),
            Some(&"assets/envato"),
            "removed before its own parent `assets`: {inter:?}"
        );
    }

    #[test]
    fn remove_skeleton_refuses_an_unknown_assets_child() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();
        let junk = dir.join("assets").join("junk");
        std::fs::create_dir(&junk).unwrap();

        assert!(remove_skeleton(&dir).is_err());
        assert!(dir.exists(), "project dir must survive a refused removal");
        assert!(junk.exists(), "unknown assets child must survive");
    }

    #[test]
    fn remove_skeleton_refuses_a_file_directly_under_assets() {
        let root = tempfile::tempdir().unwrap();
        let slug = "nvme-deep-dive-2026-07-09";
        let dir = create_project_skeleton(root.path(), &manifest(slug)).unwrap();
        let stray = dir.join("assets").join("notes.txt");
        std::fs::write(&stray, b"a filed download").unwrap();

        assert!(remove_skeleton(&dir).is_err());
        assert!(dir.exists(), "project dir must survive a refused removal");
        assert!(stray.exists(), "stray user data must survive");
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
