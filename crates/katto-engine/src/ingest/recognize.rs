//! Card recognition: classify a volume by the marker directories it contains.

use std::path::{Path, PathBuf};

use crate::ingest::{Card, CardKind, VolumeTree};

/// Recognize a volume as a camera card, or `None` for a non-camera volume.
///
/// A volume is a camera card iff it contains Sony `PRIVATE/M4ROOT/CLIP/` **or**
/// a `DCIM/` tree. Sony wins if both are present. For `DCIM/`, a subfolder
/// matching `NNNAPPLE` marks an iPhone card; otherwise it is generic. Clip roots
/// are the `CLIP`/`SUB` dirs (Sony) or the immediate `DCIM/` subfolders.
pub fn recognize(tree: &VolumeTree) -> Option<Card> {
    let has = |p: &str| tree.entries.iter().any(|e| e == Path::new(p));

    if has("PRIVATE/M4ROOT/CLIP") {
        let mut clip_roots = vec![PathBuf::from("PRIVATE/M4ROOT/CLIP")];
        if has("PRIVATE/M4ROOT/SUB") {
            clip_roots.push(PathBuf::from("PRIVATE/M4ROOT/SUB"));
        }
        return Some(Card {
            kind: CardKind::Sony,
            clip_roots,
        });
    }

    let dcim_subdirs: Vec<PathBuf> = tree
        .entries
        .iter()
        .filter(|e| e.parent() == Some(Path::new("DCIM")))
        .filter(|e| is_dir_marker(tree, e))
        .cloned()
        .collect();

    if has("DCIM") && !dcim_subdirs.is_empty() {
        let iphone = dcim_subdirs.iter().any(|d| {
            d.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(is_apple_dir)
        });
        let kind = if iphone {
            CardKind::IphoneDcim
        } else {
            CardKind::GenericDcim
        };
        return Some(Card {
            kind,
            clip_roots: dcim_subdirs,
        });
    }

    None
}

/// True when `name` is a `DCIM/NNNAPPLE` directory name (three digits then `APPLE`).
fn is_apple_dir(name: &str) -> bool {
    name.len() == 8 && name.ends_with("APPLE") && name[..3].chars().all(|c| c.is_ascii_digit())
}

/// A `DCIM` child counts as a clip-root dir if the tree contains any entry
/// nested beneath it (a real card always has media under the subfolder).
fn is_dir_marker(tree: &VolumeTree, dir: &Path) -> bool {
    tree.entries.iter().any(|e| e.parent() == Some(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> VolumeTree {
        VolumeTree {
            entries: paths.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn sony_layout_is_recognized_with_clip_and_sub_roots() {
        let t = tree(&[
            "PRIVATE",
            "PRIVATE/M4ROOT",
            "PRIVATE/M4ROOT/CLIP",
            "PRIVATE/M4ROOT/SUB",
            "PRIVATE/M4ROOT/CLIP/C0001.MP4",
            "PRIVATE/M4ROOT/SUB/C0001S01.MP4",
        ]);
        let card = recognize(&t).unwrap();
        assert_eq!(card.kind, CardKind::Sony);
        assert_eq!(
            card.clip_roots,
            vec![
                PathBuf::from("PRIVATE/M4ROOT/CLIP"),
                PathBuf::from("PRIVATE/M4ROOT/SUB"),
            ]
        );
    }

    #[test]
    fn iphone_dcim_is_recognized() {
        let t = tree(&["DCIM", "DCIM/100APPLE", "DCIM/100APPLE/IMG_0001.MOV"]);
        let card = recognize(&t).unwrap();
        assert_eq!(card.kind, CardKind::IphoneDcim);
        assert_eq!(card.clip_roots, vec![PathBuf::from("DCIM/100APPLE")]);
    }

    #[test]
    fn generic_dcim_is_recognized() {
        let t = tree(&["DCIM", "DCIM/100MEDIA", "DCIM/100MEDIA/MVI_0001.MOV"]);
        assert_eq!(recognize(&t).unwrap().kind, CardKind::GenericDcim);
    }

    #[test]
    fn non_camera_volume_is_none() {
        let t = tree(&["Documents", "Documents/notes.txt"]);
        assert!(recognize(&t).is_none());
    }

    #[test]
    fn sony_wins_when_both_markers_present() {
        let t = tree(&[
            "DCIM",
            "DCIM/100APPLE",
            "DCIM/100APPLE/IMG.MOV",
            "PRIVATE/M4ROOT/CLIP",
            "PRIVATE/M4ROOT/CLIP/C0001.MP4",
        ]);
        assert_eq!(recognize(&t).unwrap().kind, CardKind::Sony);
    }
}
