//! The single ffprobe spawn site. The JSON parse is the engine's pure
//! [`katto_engine::ffprobe::parse_probe`], so everything here stays thin.

use std::path::Path;
use std::process::Command;

use katto_engine::ffprobe::{MediaInfo, parse_probe};

use crate::error::{Error, Result};

/// Build the ffprobe argument vector for `path`. Pure and deterministic (pinned
/// flags, no clock/RNG) so it is unit-tested without spawning. Mirrors the
/// hyper-frames `cut-video` probe but uses `-show_streams -show_format` per the
/// Phase-3 PRD.
fn ffprobe_argv(path: &Path) -> Vec<String> {
    vec![
        "-loglevel".to_string(),
        "error".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "-show_format".to_string(),
        path.to_string_lossy().into_owned(),
    ]
}

/// Probe one clip's metadata by spawning `ffprobe`.
///
/// # Errors
/// [`Error::Io`] if ffprobe cannot be spawned or exits nonzero;
/// [`Error::Engine`] if its output does not parse.
pub fn probe_clip(path: &Path) -> Result<MediaInfo> {
    let output = Command::new("ffprobe").args(ffprobe_argv(path)).output()?;
    if !output.status.success() {
        return Err(Error::Io(format!(
            "ffprobe failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let json = String::from_utf8_lossy(&output.stdout);
    Ok(parse_probe(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_pins_json_streams_and_format_flags() {
        let argv = ffprobe_argv(Path::new("/tmp/C0001.MP4"));
        assert!(argv.contains(&"-show_streams".to_string()));
        assert!(argv.contains(&"-show_format".to_string()));
        assert_eq!(argv.last().unwrap(), "/tmp/C0001.MP4");
    }

    #[test]
    #[ignore = "hardware: needs ffprobe binary + a real sample file"]
    fn probe_clip_reads_real_sample() {
        // Manual: point at a real clip and assert duration_s.is_some().
        let info = probe_clip(Path::new("/tmp/s.mp4")).unwrap();
        assert!(info.duration_s.is_some());
    }
}
