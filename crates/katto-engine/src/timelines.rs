//! Versioned timeline export: `-vN` allocation (never overwrite) and the
//! FCPXML + SRT + VTT export orchestration.

use std::path::{Path, PathBuf};

use crate::bundle::{Bundle, write_atomic};
use crate::emit::captions::{emit_srt, emit_vtt, group_captions, retime_kept_words};
use crate::emit::fcpxml::emit_fcpxml;
use crate::error::{Error, Result};
use crate::render::{coalesce_cuts, effective_cut_spans};

/// Where a bundle's exports belong: when the bundle sits at
/// `<project>/audio/<x>.kruproj`, timelines land in `<project>/timelines` and
/// the slug is the project folder name; a loose bundle uses a sibling
/// `timelines/` and its own basename.
pub fn project_context(bundle_root: &Path) -> (PathBuf, String) {
    if let Some(audio) = bundle_root.parent()
        && audio.file_name().is_some_and(|n| n == "audio")
        && let Some(project) = audio.parent()
        && let Some(slug) = project.file_name()
    {
        return (
            project.join("timelines"),
            slug.to_string_lossy().into_owned(),
        );
    }
    let parent = bundle_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let slug = bundle_root
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".into());
    (parent.join("timelines"), slug)
}

/// Next free version in `timelines_dir`: max N over `<slug>-v<N>.<any ext>`
/// plus 1 (1 when none). Any extension counts — an existing `-v3.srt` blocks
/// fcpxml v3 too.
pub fn next_version(timelines_dir: &Path, slug: &str) -> u32 {
    let prefix = format!("{slug}-v");
    let mut max = 0;
    if let Ok(entries) = std::fs::read_dir(timelines_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            let digits: &str = rest.split('.').next().unwrap_or("");
            if !digits.is_empty()
                && digits.chars().all(|c| c.is_ascii_digit())
                && let Ok(n) = digits.parse::<u32>()
            {
                max = max.max(n);
            }
        }
    }
    max + 1
}

/// The written artifact locations of one export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPaths {
    /// The FCPXML timeline.
    pub fcpxml: PathBuf,
    /// Kept-only SRT captions.
    pub srt: PathBuf,
    /// Kept-only WebVTT captions.
    pub vtt: PathBuf,
    /// The allocated version number.
    pub version: u32,
}

/// Emit + validate FCPXML and captions, then write all three atomically at
/// `<timelines_dir>/<slug>-v<N>.{fcpxml,srt,vtt}`. Nothing is written when any
/// emitter/validation step fails; existing versions are never touched.
///
/// # Errors
/// [`Error::Bundle`] when transcript or cuts are absent; any
/// [`emit_fcpxml`] error; [`Error::Io`] on write failure.
pub fn export_timeline(bundle: &Bundle, timelines_dir: &Path, slug: &str) -> Result<ExportPaths> {
    let transcript = bundle
        .transcript
        .as_ref()
        .ok_or_else(|| Error::Bundle("no transcript yet".into()))?;

    let version = next_version(timelines_dir, slug);
    let project_name = format!("{slug}-v{version}");

    // Emit everything before writing anything: a validation failure must
    // leave the timelines directory untouched.
    let fcpxml_text = emit_fcpxml(bundle, &project_name)?;
    let spans = effective_cut_spans(bundle)?;
    let coalesced = coalesce_cuts(&spans);
    let fps = bundle.manifest.frame_rate;
    let timebase = u32::try_from(fps.num)
        .map_err(|_| Error::Bundle(format!("unusable frame rate {}/{}", fps.num, fps.den)))?;
    let words = retime_kept_words(&transcript.words, &coalesced, timebase);
    let grouped = group_captions(&words);
    let srt_text = emit_srt(&grouped);
    let vtt_text = emit_vtt(&grouped);

    std::fs::create_dir_all(timelines_dir)?;
    let paths = ExportPaths {
        fcpxml: timelines_dir.join(format!("{project_name}.fcpxml")),
        srt: timelines_dir.join(format!("{project_name}.srt")),
        vtt: timelines_dir.join(format!("{project_name}.vtt")),
        version,
    };
    write_atomic(&paths.fcpxml, fcpxml_text.as_bytes())?;
    write_atomic(&paths.srt, srt_text.as_bytes())?;
    write_atomic(&paths.vtt, vtt_text.as_bytes())?;
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{self, Bundle};
    use crate::rational::Rational;
    use crate::render::test_support::wire_cuts;
    use crate::schema::Transcript;
    use crate::schema::manifest::ProjectManifest;

    #[test]
    fn next_version_scans_any_extension_and_ignores_noise() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "demo-v1.fcpxml",
            "demo-v3.srt",
            "demo-v2.fcpxml",
            "other-v9.fcpxml",
            "demo-vX.fcpxml",
            "notes.txt",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert_eq!(next_version(dir.path(), "demo"), 4);
        assert_eq!(next_version(dir.path(), "fresh"), 1);
    }

    fn transcript_3_words() -> Transcript {
        let json = r#"{
            "audio_duration_secs": 10.0,
            "language_code": "en",
            "language_probability": 0.99,
            "text": "keep cut tail",
            "words": [
                {"type": "word", "text": "keep", "start": 0.0, "end": 1.0},
                {"type": "word", "text": "cut", "start": 1.0, "end": 2.0},
                {"type": "word", "text": "tail", "start": 2.0, "end": 3.0}
            ]
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn disk_bundle(cuts: &[(f64, f64)]) -> (tempfile::TempDir, Bundle) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("clip.mp4");
        std::fs::write(&source, b"fake").unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        let manifest = ProjectManifest {
            schema_version: 1,
            source_video_absolute_path: source,
            frame_rate: Rational::new(25, 1),
            duration: Rational::new(250, 25),
        };
        bundle::write_json_atomic(&root.join(bundle::PROJECT_JSON), &manifest).unwrap();
        bundle::write_json_atomic(&root.join(bundle::CUTS_JSON), &wire_cuts(cuts, 10.0)).unwrap();
        bundle::write_json_atomic(&root.join(bundle::TRANSCRIPT_JSON), &transcript_3_words())
            .unwrap();
        let opened = bundle::open(&root).unwrap();
        (dir, opened)
    }

    #[test]
    fn export_writes_all_three_versioned_and_never_overwrites() {
        let (dir, bundle) = disk_bundle(&[(1.0, 2.0)]);
        let timelines = dir.path().join("timelines");
        std::fs::create_dir(&timelines).unwrap();
        let one = export_timeline(&bundle, &timelines, "demo").unwrap();
        assert_eq!(one.version, 1);
        let first = std::fs::read(&one.fcpxml).unwrap();
        let two = export_timeline(&bundle, &timelines, "demo").unwrap();
        assert_eq!(two.version, 2);
        assert_eq!(std::fs::read(&one.fcpxml).unwrap(), first); // v1 untouched
        assert!(one.srt.exists() && one.vtt.exists());
        assert!(!timelines.join("demo-v1.fcpxml.tmp").exists());
        let srt = std::fs::read_to_string(&one.srt).unwrap();
        assert!(srt.contains("keep"));
        assert!(!srt.contains("cut\n"), "removed span text must not appear");
    }

    #[test]
    fn export_aborts_before_any_write_on_emitter_failure() {
        let (dir, bundle) = disk_bundle(&[(0.0, 10.0)]); // cuts cover everything
        let timelines = dir.path().join("timelines");
        std::fs::create_dir(&timelines).unwrap();
        assert!(export_timeline(&bundle, &timelines, "demo").is_err());
        assert_eq!(std::fs::read_dir(&timelines).unwrap().count(), 0);
    }

    #[test]
    fn export_requires_a_transcript() {
        let (dir, mut bundle) = disk_bundle(&[(1.0, 2.0)]);
        bundle.transcript = None;
        let timelines = dir.path().join("timelines");
        std::fs::create_dir(&timelines).unwrap();
        assert!(export_timeline(&bundle, &timelines, "demo").is_err());
        assert_eq!(std::fs::read_dir(&timelines).unwrap().count(), 0);
    }
}
