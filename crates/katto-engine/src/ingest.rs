//! Pure SD-ingest logic: card recognition, clip enumeration/grouping,
//! rename/sequence math, and post-copy verification. Every function here is
//! pure over in-memory representations — the filesystem walk, the ffprobe
//! spawn, and the byte copy all live in the app crate.

use std::path::PathBuf;

pub mod enumerate;
pub mod recognize;

/// The kind of camera card recognized, by on-card layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// Sony `PRIVATE/M4ROOT/CLIP/` (ZV-E10 II XAVC HS/S).
    Sony,
    /// Generic `DCIM/100MEDIA`-style camera card.
    GenericDcim,
    /// iPhone `DCIM/100APPLE`-style card.
    IphoneDcim,
}

/// A flat, in-memory listing of a volume's directory tree: every directory and
/// file path, relative to the volume mount root. Built by the app crate's walk.
#[derive(Debug, Clone, Default)]
pub struct VolumeTree {
    /// All entries (dirs and files), relative to the volume root.
    pub entries: Vec<PathBuf>,
}

/// The result of recognizing a volume as a camera card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Which card layout matched.
    pub kind: CardKind,
    /// Directories (relative to the volume root) to enumerate clips from.
    pub clip_roots: Vec<PathBuf>,
}

/// A single file discovered under a clip root, with its byte size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Path relative to the volume root.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
}

/// One clip in an enumerated group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipEntry {
    /// Source path relative to the volume root.
    pub path: PathBuf,
    /// File name (final path component).
    pub name: String,
    /// Byte size.
    pub size: u64,
    /// Lowercased extension without the dot (e.g. `"mp4"`).
    pub ext: String,
    /// True for a video file (importable), false for a sidecar/thumbnail.
    pub is_video: bool,
    /// Whether the clip is selected by default (videos yes, sidecars no).
    pub selected: bool,
}

/// A group of clips sharing card substructure (e.g. `CLIP`, `SUB`, `100APPLE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipGroup {
    /// The group label (the substructure directory name).
    pub label: String,
    /// The clips in this group, in stable order.
    pub clips: Vec<ClipEntry>,
}
