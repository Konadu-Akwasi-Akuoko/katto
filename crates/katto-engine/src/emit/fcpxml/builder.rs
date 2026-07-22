//! Typed FCPXML 1.11 document builder over `quick_xml::Writer` — the emitter
//! can't produce malformed structure. Attribute order is fixed and frozen by
//! snapshots (byte-stable output is contract).

use std::io::Write;

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};

use crate::error::{Error, Result};
use crate::rational::Rational;

/// Render a Rational as an FCPXML time attribute: `"0s"` for zero, whole
/// `"<n>s"` when den divides num evenly, else `"<num>/<den>s"`. Never a decimal.
pub fn time_attr(t: Rational) -> String {
    if t.num == 0 {
        return "0s".into();
    }
    let den = i64::from(t.den);
    if t.num % den == 0 {
        return format!("{}s", t.num / den);
    }
    format!("{}/{}s", t.num, t.den)
}

/// One rescue (removed) segment connected below a spine clip.
#[derive(Debug, Clone, PartialEq)]
pub struct RescueClip {
    /// Clip name shown in FCP.
    pub name: String,
    /// In-point in the asset.
    pub source_start: Rational,
    /// Clip duration.
    pub duration: Rational,
    /// Offset on the PARENT's local timeline (origin = the parent's `start`).
    pub offset: Rational,
}

/// One kept segment on the primary spine, carrying its anchored rescue clips.
#[derive(Debug, Clone, PartialEq)]
pub struct SpineClip {
    /// Clip name shown in FCP.
    pub name: String,
    /// Sequence-time offset.
    pub offset: Rational,
    /// In-point in the asset (also the local-timeline origin).
    pub source_start: Rational,
    /// Clip duration.
    pub duration: Rational,
    /// Removed segments anchored to this clip, disabled on lane -1.
    pub rescues: Vec<RescueClip>,
}

/// Everything the document needs; times already frame-aligned by the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct FcpxmlDoc {
    /// Library event name (`"katto"`).
    pub event_name: String,
    /// Project name (`"<slug>-v<N>"`).
    pub project_name: String,
    /// Format resource id (`"r1"`).
    pub format_id: String,
    /// Asset resource id (`"r2"`).
    pub asset_id: String,
    /// One frame as a time value (`fps.den/fps.num`).
    pub frame_duration: Rational,
    /// Format pixel width; omitted from the format element when `None`
    /// (FCP derives real dimensions from the media).
    pub width: Option<u32>,
    /// Format pixel height; omitted when `None`.
    pub height: Option<u32>,
    /// Drop-frame timecode (29.97/59.94 families).
    pub tc_format_df: bool,
    /// Asset display name (source file name).
    pub asset_name: String,
    /// Full source duration.
    pub asset_duration: Rational,
    /// Percent-encoded `file:///` URL of the source media.
    pub media_src: String,
    /// Sum of kept-clip durations.
    pub sequence_duration: Rational,
    /// Kept clips in spine order.
    pub clips: Vec<SpineClip>,
}

/// Serialize the whole document (XML decl + DOCTYPE + fcpxml tree). Pure.
///
/// # Errors
/// [`Error::Io`] if the underlying writer fails (unreachable for the in-memory
/// buffer in practice); [`Error::Bundle`] if the output is not UTF-8.
pub fn write_document(doc: &FcpxmlDoc) -> Result<String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = Writer::new_with_indent(&mut buf, b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(Event::DocType(BytesText::from_escaped("fcpxml")))?;
    writer
        .create_element("fcpxml")
        .with_attribute(("version", "1.11"))
        .write_inner_content(|w| {
            write_resources(w, doc)?;
            write_event_project(w, doc)
        })?;
    String::from_utf8(buf).map_err(|e| Error::Bundle(format!("fcpxml not utf-8: {e}")))
}

fn write_resources<W: Write>(w: &mut Writer<W>, doc: &FcpxmlDoc) -> std::io::Result<()> {
    w.create_element("resources").write_inner_content(|w| {
        let frame_duration = time_attr(doc.frame_duration);
        let width = doc.width.map(|v| v.to_string());
        let height = doc.height.map(|v| v.to_string());
        let mut format = w
            .create_element("format")
            .with_attribute(("id", doc.format_id.as_str()))
            .with_attribute(("frameDuration", frame_duration.as_str()));
        if let Some(width) = &width {
            format = format.with_attribute(("width", width.as_str()));
        }
        if let Some(height) = &height {
            format = format.with_attribute(("height", height.as_str()));
        }
        format
            .with_attribute(("colorSpace", "1-1-1 (Rec. 709)"))
            .write_empty()?;

        let asset_duration = time_attr(doc.asset_duration);
        w.create_element("asset")
            .with_attribute(("id", doc.asset_id.as_str()))
            .with_attribute(("name", doc.asset_name.as_str()))
            .with_attribute(("start", "0s"))
            .with_attribute(("duration", asset_duration.as_str()))
            .with_attribute(("hasVideo", "1"))
            .with_attribute(("hasAudio", "1"))
            .with_attribute(("format", doc.format_id.as_str()))
            .with_attribute(("audioSources", "1"))
            .with_attribute(("audioChannels", "2"))
            .with_attribute(("audioRate", "48000"))
            .write_inner_content(|w| {
                w.create_element("media-rep")
                    .with_attribute(("kind", "original-media"))
                    .with_attribute(("src", doc.media_src.as_str()))
                    .write_empty()?;
                Ok(())
            })?;
        Ok(())
    })?;
    Ok(())
}

fn write_event_project<W: Write>(w: &mut Writer<W>, doc: &FcpxmlDoc) -> std::io::Result<()> {
    w.create_element("event")
        .with_attribute(("name", doc.event_name.as_str()))
        .write_inner_content(|w| {
            w.create_element("project")
                .with_attribute(("name", doc.project_name.as_str()))
                .write_inner_content(|w| write_sequence(w, doc))?;
            Ok(())
        })?;
    Ok(())
}

fn write_sequence<W: Write>(w: &mut Writer<W>, doc: &FcpxmlDoc) -> std::io::Result<()> {
    let duration = time_attr(doc.sequence_duration);
    let tc_format = if doc.tc_format_df { "DF" } else { "NDF" };
    w.create_element("sequence")
        .with_attribute(("format", doc.format_id.as_str()))
        .with_attribute(("duration", duration.as_str()))
        .with_attribute(("tcStart", "0s"))
        .with_attribute(("tcFormat", tc_format))
        .with_attribute(("audioLayout", "stereo"))
        .with_attribute(("audioRate", "48k"))
        .write_inner_content(|w| {
            w.create_element("spine").write_inner_content(|w| {
                for clip in &doc.clips {
                    write_spine_clip(w, doc, clip)?;
                }
                Ok(())
            })?;
            Ok(())
        })?;
    Ok(())
}

fn write_spine_clip<W: Write>(
    w: &mut Writer<W>,
    doc: &FcpxmlDoc,
    clip: &SpineClip,
) -> std::io::Result<()> {
    let offset = time_attr(clip.offset);
    let start = time_attr(clip.source_start);
    let duration = time_attr(clip.duration);
    let element = w
        .create_element("asset-clip")
        .with_attribute(("ref", doc.asset_id.as_str()))
        .with_attribute(("offset", offset.as_str()))
        .with_attribute(("name", clip.name.as_str()))
        .with_attribute(("start", start.as_str()))
        .with_attribute(("duration", duration.as_str()))
        .with_attribute(("audioRole", "dialogue"));
    if clip.rescues.is_empty() {
        element.write_empty()?;
        return Ok(());
    }
    element.write_inner_content(|w| {
        for rescue in &clip.rescues {
            let offset = time_attr(rescue.offset);
            let start = time_attr(rescue.source_start);
            let duration = time_attr(rescue.duration);
            w.create_element("asset-clip")
                .with_attribute(("ref", doc.asset_id.as_str()))
                .with_attribute(("lane", "-1"))
                .with_attribute(("offset", offset.as_str()))
                .with_attribute(("name", rescue.name.as_str()))
                .with_attribute(("start", start.as_str()))
                .with_attribute(("duration", duration.as_str()))
                .with_attribute(("enabled", "0"))
                .with_attribute(("audioRole", "dialogue"))
                .write_empty()?;
        }
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational::Rational;

    #[test]
    fn time_attr_forms() {
        assert_eq!(time_attr(Rational::new(0, 30000)), "0s");
        assert_eq!(time_attr(Rational::new(60000, 30000)), "2s");
        assert_eq!(time_attr(Rational::new(1001, 30000)), "1001/30000s");
    }

    fn tiny_doc() -> FcpxmlDoc {
        FcpxmlDoc {
            event_name: "katto".into(),
            project_name: "demo-v1".into(),
            format_id: "r1".into(),
            asset_id: "r2".into(),
            frame_duration: Rational::new(1, 25),
            width: Some(3840),
            height: Some(2160),
            tc_format_df: false,
            asset_name: "clip.mp4".into(),
            asset_duration: Rational::new(250, 25),
            media_src: "file:///a/clip.mp4".into(),
            sequence_duration: Rational::new(225, 25),
            clips: vec![SpineClip {
                name: "keep 1".into(),
                offset: Rational::new(0, 25),
                source_start: Rational::new(0, 25),
                duration: Rational::new(225, 25),
                rescues: vec![RescueClip {
                    name: "removed 1".into(),
                    source_start: Rational::new(100, 25),
                    duration: Rational::new(25, 25),
                    offset: Rational::new(100, 25),
                }],
            }],
        }
    }

    #[test]
    fn document_snapshot_tiny() {
        insta::assert_snapshot!("fcpxml_builder_tiny", write_document(&tiny_doc()).unwrap());
    }

    #[test]
    fn rescue_clips_are_disabled_on_lane_minus_one() {
        let xml = write_document(&tiny_doc()).unwrap();
        assert!(xml.contains(r#"lane="-1""#));
        assert!(xml.contains(r#"enabled="0""#));
        assert!(xml.contains("<!DOCTYPE fcpxml>"));
        assert!(xml.contains(r#"<fcpxml version="1.11">"#));
        assert!(
            !xml.contains(r#"duration="9.0"#) && !xml.contains(r#"offset="4.0"#),
            "no decimal times"
        );
    }

    #[test]
    fn xml_special_chars_in_names_are_escaped() {
        let mut doc = tiny_doc();
        doc.asset_name = r#"a<b&"c".mp4"#.into();
        let xml = write_document(&doc).unwrap();
        assert!(xml.contains("a&lt;b&amp;"));
    }

    #[test]
    fn format_without_dimensions_omits_the_attributes() {
        let mut doc = tiny_doc();
        doc.width = None;
        doc.height = None;
        let xml = write_document(&doc).unwrap();
        assert!(!xml.contains("width="));
        assert!(!xml.contains("height="));
    }
}
