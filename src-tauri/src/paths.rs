use std::path::Path;

use serde::Serialize;

/// Bytes under which the free-space warning fires (100 GB) — camera footage is
/// large; an external SSD is the recommended studio root.
const LOW_FREE_SPACE_BYTES: u64 = 100 * 1024 * 1024 * 1024;

/// What onboarding/Settings show after the user picks a studio-root candidate.
/// Everything except `writable` is advisory — any directory is allowed; katto
/// warns and lets the owner proceed.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RootCheck {
    pub path: String,
    pub writable: bool,
    pub on_boot_volume: bool,
    /// Whole gigabytes free, `None` if the filesystem query failed.
    pub free_gb: Option<u32>,
    pub low_free_space: bool,
}

/// Inspect a studio-root candidate: writability, boot-volume placement, free
/// space. Never fails — an unreadable filesystem just degrades the answer.
pub fn check_root(path: &Path) -> RootCheck {
    let free = fs4::available_space(path).ok();
    RootCheck {
        path: path.to_string_lossy().into_owned(),
        writable: is_writable(path),
        on_boot_volume: is_on_boot_volume(path),
        free_gb: free.map(|b| (b / (1024 * 1024 * 1024)) as u32),
        low_free_space: free.is_some_and(|b| b < LOW_FREE_SPACE_BYTES),
    }
}

/// True when the configured studio root is reachable right now. External
/// roots (under `/Volumes`) are detected by listing `/Volumes` instead of
/// stat-ing the mount point itself — a stat on an unhealthy mount can hang,
/// listing its healthy parent cannot. Anything else is a local path where a
/// plain existence check is safe.
pub fn root_mounted(root: &Path) -> bool {
    match volume_name(root) {
        Some(name) => volume_present(Path::new("/Volumes"), name),
        None => root.exists(),
    }
}

/// The `<name>` in `/Volumes/<name>[/...]`, when the path is an external mount.
fn volume_name(root: &Path) -> Option<&std::ffi::OsStr> {
    use std::path::Component;
    let mut components = root.components();
    match (components.next(), components.next(), components.next()) {
        (
            Some(Component::RootDir),
            Some(Component::Normal(volumes)),
            Some(Component::Normal(name)),
        ) if volumes == "Volumes" => Some(name),
        _ => None,
    }
}

/// Whether `name` appears among the entries of the volumes directory.
fn volume_present(volumes_dir: &Path, name: &std::ffi::OsStr) -> bool {
    std::fs::read_dir(volumes_dir)
        .map(|entries| entries.flatten().any(|entry| entry.file_name() == name))
        .unwrap_or(false)
}

/// A path is on the boot volume unless it resolves under `/Volumes` (external
/// mounts live there on macOS; `/Volumes/Macintosh HD` is a symlink back to
/// `/`, which canonicalization unmasks).
fn is_on_boot_volume(path: &Path) -> bool {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    !resolved.starts_with("/Volumes")
}

/// Probe writability by creating and removing a marker file.
fn is_writable(path: &Path) -> bool {
    let probe = path.join(".katto-write-probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempdir_is_writable_on_boot_volume_with_free_space() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_root(dir.path());
        assert!(check.writable);
        assert!(check.on_boot_volume);
        assert!(check.free_gb.is_some());
        assert_eq!(check.path, dir.path().to_string_lossy());
    }

    #[test]
    fn readonly_dir_is_not_writable() {
        let dir = tempfile::tempdir().unwrap();
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o555);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        assert!(!check_root(dir.path()).writable);
    }

    #[test]
    fn volumes_path_is_not_boot_volume() {
        // A nonexistent /Volumes path can't canonicalize; the raw prefix decides.
        assert!(!check_root(std::path::Path::new("/Volumes/NO-SUCH-SSD")).on_boot_volume);
    }

    #[test]
    fn volume_name_extracted_from_volumes_path() {
        assert_eq!(
            volume_name(Path::new("/Volumes/Studio/footage")),
            Some(std::ffi::OsStr::new("Studio"))
        );
    }

    #[test]
    fn volume_name_absent_for_local_and_bare_paths() {
        assert_eq!(volume_name(Path::new("/Users/a/Studio")), None);
        assert_eq!(volume_name(Path::new("/Volumes")), None);
    }

    #[test]
    fn volume_present_matches_directory_entries() {
        let volumes = tempfile::tempdir().unwrap();
        std::fs::create_dir(volumes.path().join("Studio")).unwrap();

        assert!(volume_present(
            volumes.path(),
            std::ffi::OsStr::new("Studio")
        ));
        assert!(!volume_present(
            volumes.path(),
            std::ffi::OsStr::new("Gone")
        ));
    }

    #[test]
    fn local_root_mounted_iff_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        assert!(root_mounted(dir.path()));
        assert!(!root_mounted(&dir.path().join("missing")));
    }
}
