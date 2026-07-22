//! `project.json` bundle manifest: source identity plus exact timing.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::rational::Rational;

/// The `.kruproj` bundle manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Wire format version (currently 1).
    pub schema_version: u32,
    /// Absolute path to the source video at import time.
    pub source_video_absolute_path: PathBuf,
    /// The source's `r_frame_rate` as an exact ratio.
    pub frame_rate: Rational,
    /// Source duration as an exact ratio in the stream timebase.
    pub duration: Rational,
}
