//! FCPXML 1.11 emitter: bundle -> validated document -> XML text. The typed
//! builder lives in [`builder`]. The format element is emitted without
//! width/height — probe dimensions are not in the manifest and FCP derives
//! real dimensions from the media; `frameDuration` is what matters.

pub mod builder;

pub use builder::{FcpxmlDoc, RescueClip, SpineClip, time_attr, write_document};

use crate::bundle::Bundle;
use crate::error::{Error, Result};
use crate::rational::Rational;
use crate::render::{coalesce_cuts, effective_cut_spans, keep_windows};

/// Named invariant violations checked before any write (PRD: "a decimal second
/// anywhere is a validation failure").
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FcpxmlInvariant {
    /// A sequence-timeline value is not an integer multiple of the frame duration.
    #[error(
        "clip {index}: {attr} {value:?} is not on the frame grid (frameDuration {frame_duration:?})"
    )]
    OffFrameGrid {
        /// Spine clip index.
        index: usize,
        /// Which attribute is off-grid.
        attr: &'static str,
        /// The offending value.
        value: Rational,
        /// The sequence format's frame duration.
        frame_duration: Rational,
    },
    /// Spine clips are not contiguous (a gap or overlap in sequence time).
    #[error("clips are not contiguous at index {index}: expected offset {expected:?}, got {got:?}")]
    NotContiguous {
        /// Spine clip index.
        index: usize,
        /// Sum of preceding kept durations.
        expected: Rational,
        /// The clip's actual offset.
        got: Rational,
    },
    /// A rescue clip has zero or negative duration.
    #[error("rescue {index} of clip {clip}: zero or negative duration")]
    EmptyRescue {
        /// Rescue index within its parent clip.
        index: usize,
        /// Parent spine clip index.
        clip: usize,
    },
}

/// Floor `t` to a whole number of frames of `fps`, returned in the frame
/// timebase (den = fps.num). A trailing partial frame cannot land on the
/// sequence grid, so the source duration truncates rather than rounds up.
fn floor_to_frame_grid(t: Rational, fps: Rational) -> Option<Rational> {
    let frames =
        (i128::from(t.num) * i128::from(fps.num)) / (i128::from(t.den) * i128::from(fps.den));
    let num = i64::try_from(frames.checked_mul(i128::from(fps.den))?).ok()?;
    Some(Rational::new(num, u32::try_from(fps.num).ok()?))
}

/// `value` is an integer multiple of `frame_duration` (exact integer math).
fn on_frame_grid(value: Rational, frame_duration: Rational) -> bool {
    let num = i128::from(value.num) * i128::from(frame_duration.den);
    let den = i128::from(value.den) * i128::from(frame_duration.num);
    den != 0 && num % den == 0
}

/// Exact instant equality by cross-multiplication (dens may differ).
fn same_instant(a: Rational, b: Rational) -> bool {
    i128::from(a.num) * i128::from(b.den) == i128::from(b.num) * i128::from(a.den)
}

/// Build the document model from a bundle's effective state. Pure.
///
/// # Errors
/// [`Error::WholeDurationRemoved`] when nothing survives the cuts;
/// [`Error::Bundle`] when cuts.json is absent or the source path is unusable.
pub fn build_document(bundle: &Bundle, project_name: &str) -> Result<FcpxmlDoc> {
    let fps = bundle.manifest.frame_rate;
    let spans = effective_cut_spans(bundle)?;
    let duration = floor_to_frame_grid(bundle.manifest.duration, fps)
        .ok_or_else(|| Error::Bundle("source duration overflows the frame grid".into()))?;
    let keeps = keep_windows(&spans, duration, fps)?;

    let zero = Rational::new(0, duration.den);
    let removed: Vec<(Rational, Rational)> = coalesce_cuts(&spans)
        .into_iter()
        .map(|(s, e)| (s.max(zero), e.min(duration)))
        .filter(|&(s, e)| e > s)
        .collect();

    let mut clips = Vec::with_capacity(keeps.len());
    let mut running = zero;
    for (i, keep) in keeps.iter().enumerate() {
        let clip_duration = keep
            .end
            .checked_sub(keep.start)
            .ok_or_else(|| Error::Bundle("keep-window subtraction overflow".into()))?;
        clips.push(SpineClip {
            name: format!("keep {}", i + 1),
            offset: running,
            source_start: keep.start,
            duration: clip_duration,
            rescues: Vec::new(),
        });
        running = running
            .checked_add(clip_duration)
            .ok_or_else(|| Error::Bundle("sequence duration overflow".into()))?;
    }
    let sequence_duration = running;

    for (j, &(start, end)) in removed.iter().enumerate() {
        let rescue_duration = end
            .checked_sub(start)
            .ok_or_else(|| Error::Bundle("removed-span subtraction overflow".into()))?;
        let name = format!("removed {}", j + 1);
        // Anchor to the last keep ending at or before the removed span's
        // start; a span before the first keep anchors to the first keep and
        // projects before sequence time 0 (anchored items may extend past
        // parent edges).
        let parent_index = clips.iter().rposition(|c| {
            c.source_start
                .checked_add(c.duration)
                .is_some_and(|e| e <= start)
        });
        match parent_index {
            Some(parent_index) => {
                let parent = &mut clips[parent_index];
                let offset = parent
                    .source_start
                    .checked_add(parent.duration)
                    .ok_or_else(|| Error::Bundle("rescue offset overflow".into()))?;
                let lane = -(i32::try_from(parent.rescues.len())
                    .map_err(|_| Error::Bundle("too many rescues on one clip".into()))?
                    + 1);
                parent.rescues.push(RescueClip {
                    name,
                    source_start: start,
                    duration: rescue_duration,
                    offset,
                    lane,
                });
            }
            None => {
                let first = clips.first_mut().ok_or(Error::WholeDurationRemoved)?;
                let offset = first
                    .source_start
                    .checked_sub(rescue_duration)
                    .ok_or_else(|| Error::Bundle("rescue offset overflow".into()))?;
                let lane = -(i32::try_from(first.rescues.len())
                    .map_err(|_| Error::Bundle("too many rescues on one clip".into()))?
                    + 1);
                first.rescues.push(RescueClip {
                    name,
                    source_start: start,
                    duration: rescue_duration,
                    offset,
                    lane,
                });
            }
        }
    }

    let source = &bundle.manifest.source_video_absolute_path;
    let media_src = url::Url::from_file_path(source)
        .map_err(|()| Error::Bundle(format!("source path is not absolute: {}", source.display())))?
        .to_string();
    let asset_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "source".into());

    Ok(FcpxmlDoc {
        event_name: "katto".into(),
        project_name: project_name.into(),
        format_id: "r1".into(),
        asset_id: "r2".into(),
        frame_duration: Rational::new(
            i64::from(fps.den),
            u32::try_from(fps.num).map_err(|_| {
                Error::Bundle(format!("unusable frame rate {}/{}", fps.num, fps.den))
            })?,
        ),
        width: None,
        height: None,
        tc_format_df: fps.den == 1001 && (fps.num == 30000 || fps.num == 60000),
        asset_name,
        asset_duration: bundle.manifest.duration,
        media_src,
        sequence_duration,
        clips,
    })
}

/// Validate every emitted time attribute against the frame grid + spine contiguity.
pub fn validate_document(doc: &FcpxmlDoc) -> Vec<FcpxmlInvariant> {
    let fd = doc.frame_duration;
    let mut errs = Vec::new();
    let mut expected = Rational::new(0, fd.den);
    for (index, clip) in doc.clips.iter().enumerate() {
        for (attr, value) in [
            ("offset", clip.offset),
            ("start", clip.source_start),
            ("duration", clip.duration),
        ] {
            if !on_frame_grid(value, fd) {
                errs.push(FcpxmlInvariant::OffFrameGrid {
                    index,
                    attr,
                    value,
                    frame_duration: fd,
                });
            }
        }
        if !same_instant(clip.offset, expected) {
            errs.push(FcpxmlInvariant::NotContiguous {
                index,
                expected,
                got: clip.offset,
            });
        }
        expected = clip.offset.checked_add(clip.duration).unwrap_or(expected);
        for (rescue_index, rescue) in clip.rescues.iter().enumerate() {
            for (attr, value) in [
                ("offset", rescue.offset),
                ("start", rescue.source_start),
                ("duration", rescue.duration),
            ] {
                if !on_frame_grid(value, fd) {
                    errs.push(FcpxmlInvariant::OffFrameGrid {
                        index,
                        attr,
                        value,
                        frame_duration: fd,
                    });
                }
            }
            if rescue.duration.num <= 0 {
                errs.push(FcpxmlInvariant::EmptyRescue {
                    index: rescue_index,
                    clip: index,
                });
            }
        }
    }
    if !on_frame_grid(doc.sequence_duration, fd) {
        errs.push(FcpxmlInvariant::OffFrameGrid {
            index: doc.clips.len(),
            attr: "duration",
            value: doc.sequence_duration,
            frame_duration: fd,
        });
    }
    errs
}

/// build + validate + serialize; validation failure aborts before any caller write.
///
/// # Errors
/// As [`build_document`], plus [`Error::Fcpxml`] naming every violated invariant.
pub fn emit_fcpxml(bundle: &Bundle, project_name: &str) -> Result<String> {
    let doc = build_document(bundle, project_name)?;
    let errs = validate_document(&doc);
    if !errs.is_empty() {
        let joined = errs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::Fcpxml(joined));
    }
    write_document(&doc)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::rational::Rational;
    use crate::render::test_support::bundle_literal;

    fn bundle_25fps(cuts: &[(f64, f64)]) -> crate::bundle::Bundle {
        crate::render::test_support::test_bundle_with_cuts(cuts)
    }

    fn bundle_ntsc(cuts: &[(f64, f64)]) -> crate::bundle::Bundle {
        bundle_literal(
            Path::new("/media/clip.mp4"),
            Rational::new(30000, 1001),
            Rational::new(300_000, 30000), // 10s; floor-snapped to the frame grid by the builder
            cuts,
        )
    }

    fn bundle_named_25fps(name: &str, cuts: &[(f64, f64)]) -> crate::bundle::Bundle {
        bundle_literal(
            &Path::new("/media").join(name),
            Rational::new(25, 1),
            Rational::new(250, 25),
            cuts,
        )
    }

    fn bundle_25fps_with_duration(
        cuts: &[(f64, f64)],
        duration_secs: f64,
    ) -> crate::bundle::Bundle {
        bundle_literal(
            Path::new("/media/clip.mp4"),
            Rational::new(25, 1),
            Rational::new((duration_secs * 25.0) as i64, 25),
            cuts,
        )
    }

    #[test]
    fn snapshot_basic_two_cuts() {
        let b = bundle_25fps(&[(1.0, 2.0), (4.0, 5.0)]); // duration 10s
        insta::assert_snapshot!("fcpxml_basic", emit_fcpxml(&b, "demo-v1").unwrap());
    }

    #[test]
    fn snapshot_rescue_track_every_removed_segment_present() {
        let b = bundle_25fps(&[(0.0, 1.0), (5.0, 6.0)]); // includes a t=0 cut
        let xml = emit_fcpxml(&b, "demo-v1").unwrap();
        assert_eq!(xml.matches(r#"enabled="0""#).count(), 2); // one rescue per removed span
        // Both rescues anchor to the same parent: they must sit on distinct
        // lanes (FCP rejects overlapping items in one lane of one parent).
        assert!(xml.contains(r#"lane="-1""#));
        assert!(xml.contains(r#"lane="-2""#));
        insta::assert_snapshot!("fcpxml_rescue", xml);
    }

    #[test]
    fn rescues_sharing_a_parent_stack_on_distinct_lanes() {
        // A sub-frame sliver between two cuts is dropped, so both removed
        // spans anchor to the surviving keep — lanes must differ.
        let b = bundle_25fps(&[(1.0, 2.0), (2.02, 5.0)]);
        let doc = build_document(&b, "demo-v1").unwrap();
        let clip = doc.clips.iter().find(|c| c.rescues.len() > 1).unwrap();
        let mut lanes: Vec<i32> = clip.rescues.iter().map(|r| r.lane).collect();
        lanes.sort_unstable();
        lanes.dedup();
        assert_eq!(lanes.len(), clip.rescues.len());
    }

    #[test]
    fn snapshot_ntsc_df() {
        let b = bundle_ntsc(&[(1.0, 2.0)]); // 30000/1001
        let xml = emit_fcpxml(&b, "demo-v1").unwrap();
        assert!(xml.contains(r#"tcFormat="DF""#));
        assert!(xml.contains("1001/30000s") || xml.contains("/30000s"));
        insta::assert_snapshot!("fcpxml_ntsc_df", xml);
    }

    #[test]
    fn unicode_filename_is_percent_encoded() {
        let b = bundle_named_25fps("clip – プレビュー.mp4", &[(1.0, 2.0)]);
        let xml = emit_fcpxml(&b, "demo-v1").unwrap();
        assert!(xml.contains("file:///"));
        assert!(
            !xml.contains("src=\"file:///media/clip – "),
            "raw non-ASCII must not appear in media_src"
        );
        insta::assert_snapshot!("fcpxml_unicode_src", xml);
    }

    #[test]
    fn five_hundred_cuts_document_is_valid_and_bounded() {
        let cuts: Vec<(f64, f64)> = (0..500)
            .map(|i| (f64::from(i) * 4.0 + 1.0, f64::from(i) * 4.0 + 1.5))
            .collect();
        let b = bundle_25fps_with_duration(&cuts, 2001.0);
        let xml = emit_fcpxml(&b, "demo-v1").unwrap();
        assert_eq!(xml.matches("<asset-clip").count(), 500 + 501); // rescues + keeps
    }

    #[test]
    fn off_grid_time_is_a_named_validation_failure() {
        let b = bundle_25fps(&[(1.0, 2.0)]);
        let mut doc = build_document(&b, "demo-v1").unwrap();
        doc.clips[0].duration = Rational::new(3, 1000); // not a 25fps frame multiple
        let errs = validate_document(&doc);
        assert!(errs.iter().any(|e| matches!(
            e,
            FcpxmlInvariant::OffFrameGrid {
                attr: "duration",
                ..
            }
        )));
    }

    #[test]
    fn non_contiguous_spine_is_a_named_validation_failure() {
        let b = bundle_25fps(&[(1.0, 2.0)]);
        let mut doc = build_document(&b, "demo-v1").unwrap();
        doc.clips[1].offset = Rational::new(50, 25); // expected 25/25 (1s kept before)
        let errs = validate_document(&doc);
        assert!(
            errs.iter()
                .any(|e| matches!(e, FcpxmlInvariant::NotContiguous { index: 1, .. }))
        );
    }

    #[test]
    fn emit_refuses_invalid_document_before_any_write() {
        let b = bundle_25fps(&[]); // no cuts at all -> single keep, still valid
        assert!(emit_fcpxml(&b, "demo-v1").is_ok());
        let all_removed = bundle_25fps(&[(0.0, 10.0)]);
        assert!(emit_fcpxml(&all_removed, "demo-v1").is_err());
    }

    #[test]
    #[ignore = "requires Final Cut Pro's shipped DTD + xmllint; owner checkpoint"]
    fn dtd_validates_against_fcpxml_v1_11() {
        const DTD: &str = "/Applications/Final Cut Pro.app/Contents/Frameworks/Interchange.framework/Versions/A/Resources/FCPXMLv1_11.dtd";
        let b = bundle_25fps(&[(0.0, 1.0), (5.0, 6.0)]);
        let xml = emit_fcpxml(&b, "demo-v1").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo-v1.fcpxml");
        std::fs::write(&path, &xml).unwrap();
        // xmllint treats the --dtdvalid argument as a URI; the spaces in
        // "Final Cut Pro.app" break entity resolution, so validate a copy.
        let dtd = dir.path().join("FCPXMLv1_11.dtd");
        std::fs::copy(DTD, &dtd).unwrap();
        let out = std::process::Command::new("xmllint")
            .arg("--noout")
            .arg("--dtdvalid")
            .arg(&dtd)
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "xmllint rejected the document:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
