//! Pure parser for `ffprobe -show_streams -show_format -print_format json`
//! output. The single spawn site lives in the app crate; this module only
//! turns captured JSON into a [`MediaInfo`], so it is unit-tested without
//! running ffprobe.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::rational::Rational;

/// The subset of ffprobe metadata katto needs: the first video stream's codec
/// and dimensions, container duration in seconds, and frame rate as an exact
/// [`Rational`] (so drop-frame rates like `30000/1001` survive). Every field is
/// optional — enumeration never blocks on metadata, so a clip with missing or
/// unparseable fields is still importable.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// First video stream's `codec_name` (e.g. `"hevc"`, `"h264"`).
    pub codec_name: Option<String>,
    /// First video stream pixel width.
    pub width: Option<u32>,
    /// First video stream pixel height.
    pub height: Option<u32>,
    /// Container duration in seconds (`format.duration`, falling back to the
    /// first video stream's `duration`). `f64` because this is a display-only
    /// boundary value — clips are copied, not retimed, in this phase.
    pub duration_s: Option<f64>,
    /// First video stream's `r_frame_rate` parsed as an exact ratio.
    pub fps: Option<Rational>,
}

/// Parse ffprobe JSON into a [`MediaInfo`].
///
/// # Errors
/// Returns [`Error::Probe`] when `json` is not valid JSON. Missing or malformed
/// individual fields degrade to `None` rather than erroring.
pub fn parse_probe(json: &str) -> Result<MediaInfo> {
    let root: Value = serde_json::from_str(json).map_err(|e| Error::Probe(e.to_string()))?;
    let streams = root.get("streams").and_then(Value::as_array);
    let video = streams.and_then(|s| {
        s.iter()
            .find(|st| st.get("codec_type").and_then(Value::as_str) == Some("video"))
    });

    let codec_name = video
        .and_then(|v| v.get("codec_name"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let width = video
        .and_then(|v| v.get("width"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let height = video
        .and_then(|v| v.get("height"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let fps = video
        .and_then(|v| v.get("r_frame_rate"))
        .and_then(Value::as_str)
        .and_then(parse_ratio);

    let duration_s = root
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .or_else(|| {
            video
                .and_then(|v| v.get("duration"))
                .and_then(Value::as_str)
        })
        .and_then(|s| s.parse::<f64>().ok());

    Ok(MediaInfo {
        codec_name,
        width,
        height,
        duration_s,
        fps,
    })
}

/// Parse an ffprobe `"num/den"` frame-rate string into a [`Rational`]. Returns
/// `None` for a zero denominator or a `0/0` (unknown) rate.
fn parse_ratio(s: &str) -> Option<Rational> {
    let (num, den) = s.split_once('/')?;
    let num: i64 = num.trim().parse().ok()?;
    let den: u32 = den.trim().parse().ok()?;
    if den == 0 || num == 0 {
        return None;
    }
    Some(Rational { num, den })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_h264_4k60_stream_and_duration() {
        let json = include_str!("../tests/fixtures/ffprobe/xavc-hs-4k60.json");
        let info = parse_probe(json).unwrap();
        assert_eq!(info.width, Some(3840));
        assert_eq!(info.height, Some(2160));
        assert_eq!(info.codec_name.as_deref(), Some("h264"));
        assert!(info.duration_s.unwrap() > 1.9 && info.duration_s.unwrap() < 2.1);
    }

    #[test]
    fn parses_drop_frame_rate_as_exact_ratio() {
        let json = include_str!("../tests/fixtures/ffprobe/df-2997.json");
        let info = parse_probe(json).unwrap();
        assert_eq!(
            info.fps,
            Some(Rational {
                num: 30000,
                den: 1001
            })
        );
    }

    #[test]
    fn invalid_json_is_a_probe_error() {
        assert!(matches!(parse_probe("not json"), Err(Error::Probe(_))));
    }

    #[test]
    fn missing_fields_degrade_to_none_not_error() {
        let info = parse_probe(r#"{"streams":[],"format":{}}"#).unwrap();
        assert_eq!(
            info,
            MediaInfo {
                codec_name: None,
                width: None,
                height: None,
                duration_s: None,
                fps: None
            }
        );
    }

    #[test]
    fn zero_denominator_rate_is_none() {
        assert_eq!(parse_ratio("0/0"), None);
    }
}
