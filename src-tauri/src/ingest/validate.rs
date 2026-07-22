//! Source-path validation for the ingest commands. Everything here rejects
//! before any byte is copied: card sources must be traversal-free relative
//! paths that resolve strictly under the offered mount; manual drag-in sources
//! must be absolute, existing video files.

use std::path::{Component, Path, PathBuf};

use katto_engine::ingest::enumerate::VIDEO_EXTS;

use crate::error::{Error, Result};

/// Parse card-relative selection paths: the list must be non-empty and every
/// path purely relative (no root, no `..`, no prefix components).
///
/// # Errors
/// [`Error::IngestInvalid`] naming the offending path (or the empty selection).
pub fn relative_sources(paths: &[String]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(Error::IngestInvalid("no clips selected".to_string()));
    }
    paths
        .iter()
        .map(|raw| {
            let path = PathBuf::from(raw);
            if path.components().all(|c| matches!(c, Component::Normal(_))) {
                Ok(path)
            } else {
                Err(Error::IngestInvalid(format!(
                    "invalid source path (must be relative, no traversal): {raw}"
                )))
            }
        })
        .collect()
}

/// Resolve `rel` against `root` and require the canonical result to stay
/// strictly under the canonical root (catches symlinks pointing off the card)
/// and to be a regular file.
///
/// # Errors
/// [`Error::IngestInvalid`] when the path escapes the root or is not a file;
/// [`Error::Io`] when it cannot be resolved at all.
pub fn require_under_root(root: &Path, rel: &Path) -> Result<PathBuf> {
    let canonical_root = std::fs::canonicalize(root)
        .map_err(|e| Error::Io(format!("cannot resolve {}: {e}", root.display())))?;
    let joined = canonical_root.join(rel);
    let canonical = std::fs::canonicalize(&joined)
        .map_err(|e| Error::Io(format!("cannot resolve {}: {e}", joined.display())))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(Error::IngestInvalid(format!(
            "source escapes the card mount: {}",
            rel.display()
        )));
    }
    if !canonical.is_file() {
        return Err(Error::IngestInvalid(format!(
            "source is not a file: {}",
            rel.display()
        )));
    }
    Ok(canonical)
}

/// Parse manual drag-in paths: non-empty, absolute, traversal-free, each an
/// existing regular file with a video extension (sidecars and folders are
/// dropped by the frontend; the backend refuses them outright).
///
/// # Errors
/// [`Error::IngestInvalid`] naming the offending path (or the empty drop).
pub fn absolute_video_sources(paths: &[String]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(Error::IngestInvalid("no files dropped".to_string()));
    }
    paths
        .iter()
        .map(|raw| {
            let path = PathBuf::from(raw);
            let clean_components = path
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::RootDir));
            if !path.is_absolute() || !clean_components {
                return Err(Error::IngestInvalid(format!(
                    "invalid dropped path (must be absolute, no traversal): {raw}"
                )));
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !VIDEO_EXTS.contains(&ext.as_str()) {
                return Err(Error::IngestInvalid(format!("not a video file: {raw}")));
            }
            if !path.is_file() {
                return Err(Error::IngestInvalid(format!("no such file: {raw}")));
            }
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_sources_rejects_empty_selection() {
        assert!(matches!(
            relative_sources(&[]),
            Err(Error::IngestInvalid(_))
        ));
    }

    #[test]
    fn relative_sources_rejects_absolute_and_traversal() {
        for bad in ["/etc/passwd", "../outside.mp4", "CLIP/../../escape.mp4"] {
            assert!(
                matches!(
                    relative_sources(&[bad.to_string()]),
                    Err(Error::IngestInvalid(_))
                ),
                "{bad} must be rejected"
            );
        }
    }

    #[test]
    fn relative_sources_accepts_plain_card_paths() {
        let out = relative_sources(&["PRIVATE/M4ROOT/CLIP/C0001.MP4".to_string()]).unwrap();
        assert_eq!(out, vec![PathBuf::from("PRIVATE/M4ROOT/CLIP/C0001.MP4")]);
    }

    #[test]
    fn require_under_root_accepts_a_real_file_and_rejects_a_dir() {
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("C0001.MP4");
        std::fs::write(&clip, b"x").unwrap();
        assert!(require_under_root(dir.path(), Path::new("C0001.MP4")).is_ok());
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        assert!(matches!(
            require_under_root(dir.path(), Path::new("sub")),
            Err(Error::IngestInvalid(_))
        ));
    }

    #[test]
    fn require_under_root_rejects_a_symlink_escaping_the_root() {
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.mp4");
        std::fs::write(&secret, b"secret").unwrap();

        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(&secret, root.path().join("link.mp4")).unwrap();

        assert!(matches!(
            require_under_root(root.path(), Path::new("link.mp4")),
            Err(Error::IngestInvalid(_))
        ));
    }

    #[test]
    fn absolute_video_sources_rejects_relative_traversal_and_non_video() {
        let dir = tempfile::tempdir().unwrap();
        let mov = dir.path().join("clip.mov");
        std::fs::write(&mov, b"x").unwrap();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();

        assert!(absolute_video_sources(&[mov.to_string_lossy().into_owned()]).is_ok());
        for bad in [
            "relative.mov".to_string(),
            format!("{}/../clip.mov", dir.path().display()),
            txt.to_string_lossy().into_owned(),
            dir.path()
                .join("missing.mp4")
                .to_string_lossy()
                .into_owned(),
        ] {
            assert!(
                matches!(
                    absolute_video_sources(std::slice::from_ref(&bad)),
                    Err(Error::IngestInvalid(_))
                ),
                "{bad} must be rejected"
            );
        }
    }
}
