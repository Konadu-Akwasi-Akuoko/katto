//! `.kruproj` bundle open/save. Folders are truth: a bundle is a directory of
//! artifact files; all writes are atomic (`<name>.tmp` -> rename).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::schema::manifest::ProjectManifest;
use crate::schema::{Cuts, Edits, Transcript};

/// Manifest file name inside a bundle.
pub const PROJECT_JSON: &str = "project.json";
/// Transcript artifact file name.
pub const TRANSCRIPT_JSON: &str = "transcript.json";
/// Cut-plan artifact file name.
pub const CUTS_JSON: &str = "cuts.json";
/// Human edit-state artifact file name.
pub const EDITS_JSON: &str = "edits.json";
/// Extracted mono 16 kHz audio cache file name.
pub const CACHED_AUDIO_WAV: &str = "cached_audio.wav";

/// An opened `.kruproj` bundle; optional artifacts are `None` until produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    /// The bundle directory.
    pub root: PathBuf,
    /// The parsed manifest.
    pub manifest: ProjectManifest,
    /// Parsed transcript.json when present.
    pub transcript: Option<Transcript>,
    /// Parsed cuts.json when present.
    pub cuts: Option<Cuts>,
    /// Parsed edits.json when present.
    pub edits: Option<Edits>,
}

/// Atomically write `bytes` to `path` via a sibling `<file_name>.tmp` ->
/// fsync -> rename, retrying the whole write once (PRD: transient save
/// failures — a briefly full disk, an interrupted write — get one more shot).
///
/// # Errors
/// Returns [`Error::Io`] when both attempts fail.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    match write_atomic_once(path, bytes) {
        Ok(()) => Ok(()),
        Err(_) => write_atomic_once(path, bytes),
    }
}

fn write_atomic_once(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::Io(format!("no file name in {}", path.display())))?;
    let tmp = path.with_file_name(format!("{}.tmp", file_name.to_string_lossy()));
    let write = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        // fsync before rename: without it a crash can leave the renamed file
        // empty — the rename is only atomic for data already on disk.
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if write.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return write.map_err(Into::into);
    }
    // Make the rename itself durable; failure here is not worth failing the
    // save over (the data landed), so best-effort.
    if let Some(dir) = path.parent()
        && let Ok(dir_file) = std::fs::File::open(dir)
    {
        let _ = dir_file.sync_all();
    }
    Ok(())
}

/// Serialize `value` as pretty JSON and write atomically.
///
/// # Errors
/// Returns [`Error::Bundle`] on serialization failure, [`Error::Io`] on write failure.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| Error::Bundle(e.to_string()))?;
    write_atomic(path, &json)
}

/// Open a bundle directory; missing optional artifacts are `None`.
///
/// # Errors
/// [`Error::Bundle`] when project.json is missing/malformed or a present
/// artifact fails to parse; [`Error::SourceMissing`] when the manifest's
/// source video does not exist (relocation is Phase 5).
pub fn open(root: &Path) -> Result<Bundle> {
    let bundle = open_unchecked(root)?;
    let source = &bundle.manifest.source_video_absolute_path;
    if !source.exists() {
        return Err(Error::SourceMissing {
            expected_path: source.clone(),
            filename: source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            duration: bundle.manifest.duration,
        });
    }
    Ok(bundle)
}

/// Open without checking the source video exists (pipeline-internal steps).
///
/// # Errors
/// [`Error::Bundle`] when project.json is missing/malformed or a present
/// artifact fails to parse.
pub fn open_unchecked(root: &Path) -> Result<Bundle> {
    let manifest_path = root.join(PROJECT_JSON);
    let manifest_raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| Error::Bundle(format!("{}: {e}", manifest_path.display())))?;
    let manifest: ProjectManifest = serde_json::from_str(&manifest_raw)
        .map_err(|e| Error::Bundle(format!("{}: {e}", manifest_path.display())))?;

    Ok(Bundle {
        root: root.to_path_buf(),
        manifest,
        transcript: read_optional(root, TRANSCRIPT_JSON)?,
        cuts: read_optional(root, CUTS_JSON)?,
        edits: read_optional(root, EDITS_JSON)?,
    })
}

/// Write edits.json atomically (Phase 5 calls this on auto-save).
///
/// # Errors
/// [`Error::Io`] / [`Error::Bundle`] as for [`write_json_atomic`].
pub fn save_edits(root: &Path, edits: &Edits) -> Result<()> {
    write_json_atomic(&root.join(EDITS_JSON), edits)
}

/// The manifest's source can only be swapped for the same recording: same
/// file name, duration within one frame of the manifest's. Pure.
///
/// # Errors
/// [`Error::Relocate`] naming the mismatch (file name or duration).
pub fn relocation_matches(
    manifest: &ProjectManifest,
    probed_duration: crate::rational::Rational,
    new_path: &Path,
) -> Result<()> {
    let expected = manifest
        .source_video_absolute_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let got = new_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if expected != got {
        return Err(Error::Relocate(format!(
            "file name mismatch: expected {expected}, picked {got}"
        )));
    }
    let fps = manifest.frame_rate;
    let diff = manifest
        .duration
        .checked_sub(probed_duration)
        .ok_or_else(|| Error::Relocate("duration comparison overflowed".to_string()))?;
    // |diff| <= one frame (fps.den/fps.num seconds), cross-multiplied exactly.
    let abs = i128::from(diff.num).unsigned_abs() * u128::from(fps.num.unsigned_abs());
    let one_frame = u128::from(fps.den) * u128::from(diff.den);
    if abs > one_frame {
        return Err(Error::Relocate(format!(
            "duration mismatch: manifest {:.3}s, picked file {:.3}s",
            manifest.duration.to_secs_f64(),
            probed_duration.to_secs_f64()
        )));
    }
    Ok(())
}

/// Swap the manifest's source for `new_path` once [`relocation_matches`]
/// passes; the manifest rewrite is atomic (`.tmp` -> rename).
///
/// # Errors
/// [`Error::Relocate`] on a mismatch; [`Error::Io`] / [`Error::Bundle`] when
/// the manifest cannot be read or rewritten.
pub fn apply_relocation(
    root: &Path,
    probed_duration: crate::rational::Rational,
    new_path: PathBuf,
) -> Result<()> {
    let manifest_path = root.join(PROJECT_JSON);
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| Error::Io(format!("{}: {e}", manifest_path.display())))?;
    let mut manifest: ProjectManifest = serde_json::from_str(&raw)
        .map_err(|e| Error::Bundle(format!("{}: {e}", manifest_path.display())))?;
    relocation_matches(&manifest, probed_duration, &new_path)?;
    manifest.source_video_absolute_path = new_path;
    write_json_atomic(&manifest_path, &manifest)
}

/// Parse an optional artifact: absent file is `None`; a present file that
/// fails to parse is a typed error, never a silent `None`.
fn read_optional<T: serde::de::DeserializeOwned>(root: &Path, name: &str) -> Result<Option<T>> {
    let path = root.join(name);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Bundle(format!("{}: {e}", path.display()))),
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| Error::Bundle(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational::Rational;

    fn manifest(source: &Path) -> ProjectManifest {
        ProjectManifest {
            schema_version: 1,
            source_video_absolute_path: source.to_path_buf(),
            frame_rate: Rational::new(30000, 1001),
            duration: Rational::new(3_843_840, 30000),
        }
    }

    #[test]
    fn write_atomic_failure_leaves_no_tmp_and_surfaces_io() {
        let dir = tempfile::tempdir().unwrap();
        let missing_parent = dir.path().join("gone").join("file.json");
        assert!(matches!(
            write_atomic(&missing_parent, b"x"),
            Err(Error::Io(_))
        ));
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn bundle_round_trip_open_mutate_save_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("clip.mp4");
        std::fs::write(&source, b"fake").unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        write_json_atomic(&root.join(PROJECT_JSON), &manifest(&source)).unwrap();

        let mut b = open(&root).unwrap();
        assert!(b.transcript.is_none() && b.cuts.is_none() && b.edits.is_none());

        let edits = Edits {
            schema_version: 1,
            toggled_off: vec![0],
            ..Default::default()
        };
        save_edits(&root, &edits).unwrap();
        b = open(&root).unwrap();
        assert_eq!(b.edits, Some(edits));
        // no .tmp litter
        assert!(!root.join("edits.json.tmp").exists());
    }

    #[test]
    fn missing_source_is_typed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        write_json_atomic(
            &root.join(PROJECT_JSON),
            &manifest(&dir.path().join("gone.mp4")),
        )
        .unwrap();
        match open(&root) {
            Err(Error::SourceMissing { filename, .. }) => assert_eq!(filename, "gone.mp4"),
            other => panic!("expected SourceMissing, got {other:?}"),
        }
        // unchecked open still succeeds for pipeline-internal steps
        assert!(open_unchecked(&root).is_ok());
    }

    #[test]
    fn present_but_malformed_artifact_is_a_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("clip.mp4");
        std::fs::write(&source, b"fake").unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        write_json_atomic(&root.join(PROJECT_JSON), &manifest(&source)).unwrap();
        std::fs::write(root.join(CUTS_JSON), b"not json").unwrap();
        assert!(matches!(open(&root), Err(Error::Bundle(_))));
    }

    #[test]
    fn write_atomic_replaces_not_appends() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.json");
        write_atomic(&p, b"first-longer-content").unwrap();
        write_atomic(&p, b"second").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"second");
    }

    fn manifest_25fps(name: &str) -> ProjectManifest {
        ProjectManifest {
            schema_version: 1,
            source_video_absolute_path: PathBuf::from("/media").join(name),
            frame_rate: Rational::new(25, 1),
            duration: Rational::new(250, 25),
        }
    }

    #[test]
    fn relocation_requires_same_filename_and_duration() {
        let m = manifest_25fps("clip.mp4");
        assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/clip.mp4")).is_ok());
        assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/other.mp4")).is_err());
        let off = m.duration.checked_add(Rational::new(2, 1)).unwrap();
        assert!(relocation_matches(&m, off, Path::new("/elsewhere/clip.mp4")).is_err());
    }

    #[test]
    fn relocation_tolerates_one_frame_of_duration_drift() {
        let m = manifest_25fps("clip.mp4");
        let one_frame_less = m.duration.checked_sub(Rational::new(1, 25)).unwrap();
        assert!(relocation_matches(&m, one_frame_less, Path::new("/e/clip.mp4")).is_ok());
        let two_frames_less = m.duration.checked_sub(Rational::new(2, 25)).unwrap();
        assert!(relocation_matches(&m, two_frames_less, Path::new("/e/clip.mp4")).is_err());
    }

    #[test]
    fn apply_relocation_rewrites_only_the_source_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("clip.kruproj");
        std::fs::create_dir(&root).unwrap();
        let m = manifest_25fps("clip.mp4");
        write_json_atomic(&root.join(PROJECT_JSON), &m).unwrap();

        apply_relocation(&root, m.duration, PathBuf::from("/elsewhere/clip.mp4")).unwrap();
        let raw = std::fs::read_to_string(root.join(PROJECT_JSON)).unwrap();
        let rewritten: ProjectManifest = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            rewritten.source_video_absolute_path,
            PathBuf::from("/elsewhere/clip.mp4")
        );
        assert_eq!(rewritten.duration, m.duration);
        assert_eq!(rewritten.frame_rate, m.frame_rate);

        let err = apply_relocation(&root, m.duration, PathBuf::from("/x/other.mp4"));
        assert!(matches!(err, Err(Error::Relocate(_))));
    }
}
