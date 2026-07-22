//! The engine's single error type.

/// Errors returned by the pure media pipeline (ffprobe parsing, ingest logic).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// ffprobe output could not be parsed into [`crate::ffprobe::MediaInfo`].
    #[error("ffprobe: {0}")]
    Probe(String),
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
