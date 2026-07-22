//! Clip enumeration: classify files by extension and group by card substructure.

use std::path::Path;

use crate::ingest::{CardKind, ClipEntry, ClipGroup, FileEntry};

/// Video extensions katto imports (compared case-insensitively).
pub const VIDEO_EXTS: [&str; 4] = ["mp4", "mov", "mts", "m4v"];

/// Group and classify the files walked from a card's clip roots.
///
/// Videos (`VIDEO_EXTS`) are selected by default; everything else (sidecars such
/// as `.xml`/`.thm`) is listed but deselected. Files are grouped by the name of
/// their immediate parent directory (`CLIP`, `SUB`, `100APPLE`, …). Groups and
/// clips are returned in stable, path-sorted order so a plan is deterministic.
pub fn enumerate(_kind: CardKind, files: &[FileEntry]) -> Vec<ClipGroup> {
    let mut sorted: Vec<&FileEntry> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut groups: Vec<ClipGroup> = Vec::new();
    for f in sorted {
        let name = f
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let ext = f
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let is_video = VIDEO_EXTS.contains(&ext.as_str());
        let label = f
            .path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or("clips")
            .to_string();

        let clip = ClipEntry {
            path: f.path.clone(),
            name,
            size: f.size,
            ext,
            is_video,
            selected: is_video,
        };
        match groups.iter_mut().find(|g| g.label == label) {
            Some(g) => g.clips.push(clip),
            None => groups.push(ClipGroup {
                label,
                clips: vec![clip],
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fe(path: &str, size: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size,
        }
    }

    #[test]
    fn videos_selected_sidecars_deselected() {
        let groups = enumerate(
            CardKind::Sony,
            &[
                fe("PRIVATE/M4ROOT/CLIP/C0001.MP4", 100),
                fe("PRIVATE/M4ROOT/CLIP/C0001M01.XML", 5),
            ],
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "CLIP");
        let mp4 = groups[0]
            .clips
            .iter()
            .find(|c| c.name == "C0001.MP4")
            .unwrap();
        let xml = groups[0]
            .clips
            .iter()
            .find(|c| c.name == "C0001M01.XML")
            .unwrap();
        assert!(mp4.is_video && mp4.selected);
        assert!(!xml.is_video && !xml.selected);
    }

    #[test]
    fn groups_by_substructure_clip_and_sub() {
        let groups = enumerate(
            CardKind::Sony,
            &[
                fe("PRIVATE/M4ROOT/CLIP/C0001.MP4", 1),
                fe("PRIVATE/M4ROOT/SUB/C0001S01.MP4", 1),
            ],
        );
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["CLIP", "SUB"]);
    }

    #[test]
    fn extension_case_is_normalized() {
        let groups = enumerate(CardKind::IphoneDcim, &[fe("DCIM/100APPLE/IMG.MoV", 1)]);
        assert_eq!(groups[0].clips[0].ext, "mov");
        assert!(groups[0].clips[0].is_video);
    }
}
