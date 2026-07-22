//! Import step: probe a clip's exact timing and extract its mono 16 kHz audio
//! cache into a fresh `.kruproj` bundle. The argv builders are pure and
//! unit-tested; the two spawn call sites stay thin.

use std::path::{Path, PathBuf};

use crate::bundle::{CACHED_AUDIO_WAV, PROJECT_JSON, write_json_atomic};
use crate::error::{Error, Result};
use crate::ffprobe::parse_probe_timing;
use crate::schema::manifest::ProjectManifest;

/// Argv for probing (identical flags to the app crate's Phase-3 probe).
pub fn ffprobe_argv(path: &Path) -> Vec<String> {
    vec![
        "-loglevel".into(),
        "error".into(),
        "-print_format".into(),
        "json".into(),
        "-show_streams".into(),
        "-show_format".into(),
        path.to_string_lossy().into_owned(),
    ]
}

/// Argv for mono 16 kHz WAV extraction into the bundle (PRD: `-vn -ar 16000 -ac 1`).
pub fn extract_audio_argv(src: &Path, out: &Path) -> Vec<String> {
    vec![
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        src.to_string_lossy().into_owned(),
        "-vn".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        "16000".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        out.to_string_lossy().into_owned(),
    ]
}

/// A completed import: the bundle location and its written manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome {
    /// The created `.kruproj` directory.
    pub bundle_root: PathBuf,
    /// The manifest written to `project.json`.
    pub manifest: ProjectManifest,
}

/// Probe `video`, create `<parent>/<basename>.kruproj/` with project.json, and
/// extract cached_audio.wav. Idempotent: re-importing overwrites both artifacts.
///
/// # Errors
/// [`Error::Probe`] when ffprobe fails or yields no video timing; [`Error::Io`]
/// on filesystem/spawn failures (ffmpeg stderr is surfaced in the message).
pub async fn import(video: &Path, parent: &Path) -> Result<ImportOutcome> {
    let probe_json = run_capturing("ffprobe", &ffprobe_argv(video))
        .await
        .map_err(|e| Error::Probe(e.to_string()))?;
    let timing = parse_probe_timing(&probe_json)?;
    let (frame_rate, duration) = match (timing.frame_rate, timing.duration) {
        (Some(f), Some(d)) => (f, d),
        _ => {
            return Err(Error::Probe(format!(
                "no video timing in probe of {}",
                video.display()
            )));
        }
    };

    let stem = video
        .file_stem()
        .ok_or_else(|| Error::Io(format!("no file stem in {}", video.display())))?;
    let bundle_root = parent.join(format!("{}.kruproj", stem.to_string_lossy()));
    tokio::fs::create_dir_all(&bundle_root).await?;

    let manifest = ProjectManifest {
        schema_version: 1,
        source_video_absolute_path: std::fs::canonicalize(video)?,
        frame_rate,
        duration,
    };
    write_json_atomic(&bundle_root.join(PROJECT_JSON), &manifest)?;

    let wav = bundle_root.join(CACHED_AUDIO_WAV);
    let wav_tmp = bundle_root.join(format!("{CACHED_AUDIO_WAV}.tmp"));
    if let Err(e) = run_capturing("ffmpeg", &extract_audio_argv(video, &wav_tmp)).await {
        let _ = tokio::fs::remove_file(&wav_tmp).await;
        return Err(Error::Io(format!("audio extraction failed: {e}")));
    }
    tokio::fs::rename(&wav_tmp, &wav).await?;

    Ok(ImportOutcome {
        bundle_root,
        manifest,
    })
}

/// Thin spawn helper: run `cmd` with `argv`, return stdout; non-zero exit
/// surfaces stderr text. Untested-by-unit per the testing rules.
async fn run_capturing(cmd: &str, argv: &[String]) -> std::result::Result<String, String> {
    let output = tokio::process::Command::new(cmd)
        .args(argv)
        .output()
        .await
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{cmd} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_audio_argv_is_pinned() {
        let argv = extract_audio_argv(
            Path::new("/a/clip.mp4"),
            Path::new("/b/cached_audio.wav.tmp"),
        );
        assert_eq!(
            argv,
            vec![
                "-nostdin",
                "-loglevel",
                "error",
                "-y",
                "-i",
                "/a/clip.mp4",
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
                "-f",
                "wav",
                "/b/cached_audio.wav.tmp",
            ]
        );
    }

    #[test]
    fn ffprobe_argv_matches_phase3_flags() {
        let argv = ffprobe_argv(Path::new("/a/clip.mp4"));
        assert_eq!(
            argv,
            vec![
                "-loglevel",
                "error",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                "/a/clip.mp4",
            ]
        );
    }

    #[tokio::test]
    #[ignore = "spawns real ffmpeg/ffprobe; run manually with a real clip"]
    async fn import_real_clip_end_to_end() {
        let clip = std::env::var("KATTO_TEST_CLIP").expect("set KATTO_TEST_CLIP=/path/to/clip.mp4");
        let dir = tempfile::tempdir().unwrap();
        let out = import(Path::new(&clip), dir.path()).await.unwrap();
        assert!(out.bundle_root.join(CACHED_AUDIO_WAV).exists());
        assert!(out.manifest.duration.num > 0);
    }
}
