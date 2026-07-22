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
/// fcpxml v3 too. A scan, not a reservation: writers must go through
/// [`claim_next_version`].
///
/// # Errors
/// [`Error::Io`] when the directory exists but cannot be scanned — silently
/// answering v1 there would aim a write at an existing version.
pub fn next_version(timelines_dir: &Path, slug: &str) -> Result<u32> {
    let prefix = format!("{slug}-v");
    let mut max = 0;
    let entries = match std::fs::read_dir(timelines_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(e) => {
            return Err(Error::Io(format!("{}: {e}", timelines_dir.display())));
        }
    };
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
    Ok(max + 1)
}

/// Reserve the next free version by atomically creating `<slug>-vN.<ext>`
/// with `create_new`, walking N upward on `AlreadyExists`. The claim — not
/// the directory scan — is what enforces never-overwrite under concurrent
/// writers; the caller later renames its finished artifact over its own
/// claim (and removes the claim on failure).
///
/// # Errors
/// As [`next_version`], plus [`Error::Io`] when the claim file cannot be
/// created for any reason other than already existing.
pub fn claim_next_version(dir: &Path, slug: &str, ext: &str) -> Result<(u32, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let mut version = next_version(dir, slug)?;
    loop {
        let path = dir.join(format!("{slug}-v{version}.{ext}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok((version, path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => version += 1,
            Err(e) => return Err(Error::Io(format!("{}: {e}", path.display()))),
        }
    }
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

    // Claim the version first (the project name is baked into the FCPXML);
    // any failure past this point removes this export's own files, so a
    // failed run still leaves the directory as it found it.
    let (version, fcpxml_path) = claim_next_version(timelines_dir, slug, "fcpxml")?;
    let project_name = format!("{slug}-v{version}");
    let paths = ExportPaths {
        srt: timelines_dir.join(format!("{project_name}.srt")),
        vtt: timelines_dir.join(format!("{project_name}.vtt")),
        fcpxml: fcpxml_path,
        version,
    };

    let write_all = || -> Result<()> {
        // Emit everything before writing anything real: a validation failure
        // must leave only the (removed-below) claim behind.
        let fcpxml_text = emit_fcpxml(bundle, &project_name)?;
        let spans = effective_cut_spans(bundle)?;
        let coalesced = coalesce_cuts(&spans);
        let fps = bundle.manifest.frame_rate;
        let timebase = u32::try_from(fps.num)
            .map_err(|_| Error::Bundle(format!("unusable frame rate {}/{}", fps.num, fps.den)))?;
        let words = retime_kept_words(&transcript.words, &coalesced, timebase);
        let grouped = group_captions(&words);
        write_atomic(&paths.fcpxml, fcpxml_text.as_bytes())?;
        write_atomic(&paths.srt, emit_srt(&grouped).as_bytes())?;
        write_atomic(&paths.vtt, emit_vtt(&grouped).as_bytes())?;
        Ok(())
    };
    if let Err(e) = write_all() {
        for p in [&paths.fcpxml, &paths.srt, &paths.vtt] {
            let _ = std::fs::remove_file(p);
        }
        return Err(e);
    }
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
        assert_eq!(next_version(dir.path(), "demo").unwrap(), 4);
        assert_eq!(next_version(dir.path(), "fresh").unwrap(), 1);
    }

    #[test]
    fn next_version_surfaces_scan_failure_and_absent_dir_reads_v1() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_version(&dir.path().join("nope"), "demo").unwrap(), 1);
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(next_version(&file, "demo"), Err(Error::Io(_))));
    }

    #[test]
    fn claim_next_version_reserves_atomically_and_walks_past_collisions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("demo-v1.fcpxml"), b"x").unwrap();
        std::fs::write(dir.path().join("demo-v2.srt"), b"x").unwrap();
        let (version, path) = claim_next_version(dir.path(), "demo", "mp4").unwrap();
        assert_eq!(version, 3);
        assert!(path.exists()); // the claim itself blocks the version
        let (again, _) = claim_next_version(dir.path(), "demo", "mp4").unwrap();
        assert_eq!(again, 4);
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
