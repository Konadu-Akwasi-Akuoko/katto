//! Card-offer assembly: turn a recognized card's files into the wire DTO the
//! import sheet consumes. Publishing is two-phase so detection is instant: the
//! offer goes out with `duration_s: None`, then ffprobe durations are filled in
//! from a background task and the refreshed offer is re-broadcast.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use katto_engine::ingest::{Card, CardKind, FileEntry, enumerate::enumerate};

use crate::broadcast;
use crate::state::IngestState;

/// One clip in a card offer, as sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipDto {
    /// Source path relative to the volume root.
    pub path: String,
    /// File name.
    pub name: String,
    /// Byte size. Exported as `number`: real clip sizes never approach 2^53.
    #[specta(type = f64)]
    pub size: u64,
    /// Whether it is a video (importable) vs a sidecar.
    pub is_video: bool,
    /// Default selection state.
    pub selected: bool,
    /// Duration in seconds, if ffprobe succeeded.
    pub duration_s: Option<f64>,
}

/// A group of clips sharing card substructure.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipGroupDto {
    /// Group label (substructure dir name).
    pub label: String,
    /// Clips in the group.
    pub clips: Vec<ClipDto>,
}

/// The current detected card, offered to the import sheet.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CardOffer {
    /// Absolute mount path (`/Volumes/<NAME>`), also the eject target.
    pub volume: String,
    /// Recognized card kind, as a stable slug (`"sony"`/`"generic_dcim"`/`"iphone_dcim"`).
    pub kind: String,
    /// Total bytes of all video clips (for the free-space check).
    #[specta(type = f64)]
    pub total_bytes: u64,
    /// Grouped clips.
    pub groups: Vec<ClipGroupDto>,
}

/// Build a `CardOffer` from a recognized card and the files walked from its
/// clip roots. Pure over its inputs — no probing; every `duration_s` starts as
/// `None` and is filled asynchronously by [`fill_durations`].
pub fn build_offer(mount: &Path, card: &Card, files: &[FileEntry]) -> CardOffer {
    let kind = match card.kind {
        CardKind::Sony => "sony",
        CardKind::GenericDcim => "generic_dcim",
        CardKind::IphoneDcim => "iphone_dcim",
    };
    let under_roots: Vec<FileEntry> = files
        .iter()
        .filter(|f| card.clip_roots.iter().any(|r| f.path.starts_with(r)))
        .cloned()
        .collect();
    let groups = enumerate(card.kind, &under_roots);

    let mut total_bytes = 0u64;
    let groups: Vec<ClipGroupDto> = groups
        .into_iter()
        .map(|g| ClipGroupDto {
            label: g.label,
            clips: g
                .clips
                .into_iter()
                .map(|c| {
                    if c.is_video {
                        total_bytes += c.size;
                    }
                    ClipDto {
                        path: c.path.to_string_lossy().into_owned(),
                        name: c.name,
                        size: c.size,
                        is_video: c.is_video,
                        selected: c.selected,
                        duration_s: None,
                    }
                })
                .collect(),
        })
        .collect();

    CardOffer {
        volume: mount.to_string_lossy().into_owned(),
        kind: kind.to_string(),
        total_bytes,
        groups,
    }
}

/// Merge probed durations (keyed by clip `path`) into an offer. Clips without
/// an entry keep their current value.
pub fn apply_durations(offer: &mut CardOffer, durations: &HashMap<String, f64>) {
    for group in &mut offer.groups {
        for clip in &mut group.clips {
            if let Some(d) = durations.get(&clip.path) {
                clip.duration_s = Some(*d);
            }
        }
    }
}

/// Probe every video clip's duration in the background (bounded parallelism)
/// and republish the offer. Skips the update when the card was removed or
/// replaced while probing.
pub fn fill_durations(app: AppHandle, offer: CardOffer) {
    tauri::async_runtime::spawn(async move {
        let volume = offer.volume.clone();
        let videos: Vec<String> = offer
            .groups
            .iter()
            .flat_map(|g| g.clips.iter())
            .filter(|c| c.is_video)
            .map(|c| c.path.clone())
            .collect();

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
        let mut tasks = Vec::with_capacity(videos.len());
        for path in videos {
            let Ok(permit) = semaphore.clone().acquire_owned().await else {
                break;
            };
            let mount = std::path::PathBuf::from(&volume);
            tasks.push(tauri::async_runtime::spawn_blocking(move || {
                let _permit = permit;
                let duration = crate::ffprobe::probe_clip(&mount.join(&path))
                    .ok()
                    .and_then(|m| m.duration_s);
                (path, duration)
            }));
        }

        let mut durations = HashMap::new();
        for task in tasks {
            if let Ok((path, Some(d))) = task.await {
                durations.insert(path, d);
            }
        }
        if durations.is_empty() {
            return;
        }

        let mut updated = offer;
        apply_durations(&mut updated, &durations);

        // Only publish if this card is still the current offer.
        let state = app.state::<IngestState>();
        let still_current = {
            let Ok(mut guard) = state.current.lock() else {
                return;
            };
            match guard.as_ref() {
                Some(current) if current.volume == updated.volume => {
                    *guard = Some(updated);
                    true
                }
                _ => false,
            }
        };
        if still_current {
            broadcast::card_detected(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sony_card() -> Card {
        Card {
            kind: CardKind::Sony,
            clip_roots: vec![PathBuf::from("PRIVATE/M4ROOT/CLIP")],
        }
    }

    fn files() -> Vec<FileEntry> {
        vec![
            FileEntry {
                path: PathBuf::from("PRIVATE/M4ROOT/CLIP/C0001.MP4"),
                size: 100,
            },
            FileEntry {
                path: PathBuf::from("PRIVATE/M4ROOT/CLIP/C0001M01.XML"),
                size: 5,
            },
        ]
    }

    #[test]
    fn build_offer_totals_videos_and_starts_without_durations() {
        let offer = build_offer(Path::new("/Volumes/SONY"), &sony_card(), &files());
        assert_eq!(offer.kind, "sony");
        assert_eq!(offer.total_bytes, 100);
        let clips: Vec<&ClipDto> = offer.groups.iter().flat_map(|g| g.clips.iter()).collect();
        assert!(clips.iter().all(|c| c.duration_s.is_none()));
        assert_eq!(clips.len(), 2);
    }

    #[test]
    fn apply_durations_fills_matching_paths_only() {
        let mut offer = build_offer(Path::new("/Volumes/SONY"), &sony_card(), &files());
        let mut durations = HashMap::new();
        durations.insert("PRIVATE/M4ROOT/CLIP/C0001.MP4".to_string(), 12.5);
        durations.insert("not-on-card.mp4".to_string(), 1.0);
        apply_durations(&mut offer, &durations);
        let clip = offer
            .groups
            .iter()
            .flat_map(|g| g.clips.iter())
            .find(|c| c.name == "C0001.MP4")
            .unwrap();
        assert_eq!(clip.duration_s, Some(12.5));
        let sidecar = offer
            .groups
            .iter()
            .flat_map(|g| g.clips.iter())
            .find(|c| c.name == "C0001M01.XML")
            .unwrap();
        assert_eq!(sidecar.duration_s, None);
    }
}
