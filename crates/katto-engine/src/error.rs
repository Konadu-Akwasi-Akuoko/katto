//! The engine's single error type.

use std::path::PathBuf;

use crate::rational::Rational;

/// Errors returned by the pure media pipeline (ffprobe parsing, ingest logic).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// ffprobe output could not be parsed into [`crate::ffprobe::MediaInfo`].
    #[error("ffprobe: {0}")]
    Probe(String),
    /// Filesystem failure in bundle/import handling.
    #[error("io: {0}")]
    Io(String),
    /// Bundle artifact malformed or missing where required.
    #[error("bundle: {0}")]
    Bundle(String),
    /// ElevenLabs rejected the API key (401).
    #[error("elevenlabs auth: {0}")]
    TranscribeAuth(String),
    /// ElevenLabs quota/rate limit (429) after retry.
    #[error("elevenlabs quota: {0}")]
    TranscribeQuota(String),
    /// Any other transcription transport/response failure.
    #[error("elevenlabs: {0}")]
    Transcribe(String),
    /// A planner failure (transport or invalid output after retry).
    #[error("plan: {0}")]
    Plan(#[from] crate::planner::PlanError),
    /// A persisted cuts.json failed re-validation on load.
    #[error("cuts validation failed: {0}")]
    CutsInvalid(String),
    /// Every keep-window was removed or sub-epsilon; nothing remains to encode.
    #[error("whole duration removed: the cuts cover the entire source")]
    WholeDurationRemoved,
    /// MP4 render failed (ffmpeg stderr tail included).
    #[error("render: {0}")]
    Render(String),
    /// The manifest's source video is missing on open (Phase 5 adds relocation).
    #[error("source missing: expected {expected_path}", expected_path = .expected_path.display())]
    SourceMissing {
        /// Where the manifest says the source video lives.
        expected_path: PathBuf,
        /// The missing file's name, for UI copy.
        filename: String,
        /// The manifest's recorded duration, for UI copy.
        duration: Rational,
    },
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// Convenience alias for engine results.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_error_displays_its_message() {
        let err = Error::Probe("no video stream".to_string());
        assert_eq!(err.to_string(), "ffprobe: no video stream");
    }
}
