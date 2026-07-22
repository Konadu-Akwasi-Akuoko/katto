//! Kept-only MP4 render: keep-window math, deterministic filtergraph text,
//! and the thin ffmpeg spawn site.

pub mod segments;

pub use segments::{Keep, coalesce_cuts, filter_complex_script, keep_windows};

use std::path::Path;

use tokio::io::AsyncBufReadExt;

use crate::bundle::Bundle;
use crate::error::{Error, Result};
use crate::merge::{CutPlan, effective_cuts};
use crate::rational::Rational;
use crate::schema::Edits;

/// Bundle-relative name of the regenerated filtergraph file (not an export
/// artifact; rewritten on every render).
pub const RENDER_FILTERGRAPH: &str = "render.filtergraph";

/// Pinned ffmpeg argv for the kept-only re-encode (never `-c copy`; graph via
/// script file so 500+ cuts stay inside argv limits).
pub fn render_argv(src: &Path, graph_path: &Path, out_tmp: &Path) -> Vec<String> {
    [
        "ffmpeg",
        "-nostdin",
        "-loglevel",
        "error",
        "-y",
        "-progress",
        "pipe:1",
        "-i",
        &src.to_string_lossy(),
        "-filter_complex_script",
        &graph_path.to_string_lossy(),
        "-map",
        "[v]",
        "-map",
        "[a]",
        "-c:v",
        "libx264",
        "-crf",
        "18",
        "-preset",
        "medium",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        &out_tmp.to_string_lossy(),
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Parse one `-progress pipe:1` line; `out_time_us=N` -> seconds rendered so far.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    let value = line.strip_prefix("out_time_us=")?;
    let us: i64 = value.trim().parse().ok()?;
    Some(us as f64 / 1_000_000.0)
}

/// Kept-source seconds total (progress denominator). Boundary float projection.
pub fn kept_total_secs(keeps: &[Keep]) -> f64 {
    keeps
        .iter()
        .filter_map(|k| k.end.checked_sub(k.start))
        .map(Rational::to_secs_f64)
        .sum()
}

/// Effective cuts (frame-snapped, inverted spans dropped) in the manifest's
/// frame timebase (den = fps.num); coalescing happens inside [`keep_windows`].
/// Shared by the renderer and the FCPXML emitter.
///
/// # Errors
/// [`Error::Bundle`] when cuts.json is absent or the frame rate is unusable.
pub fn effective_cut_spans(bundle: &Bundle) -> Result<Vec<(Rational, Rational)>> {
    let cuts = bundle
        .cuts
        .as_ref()
        .ok_or_else(|| Error::Bundle("no cuts.json yet".into()))?;
    let fps = bundle.manifest.frame_rate;
    let timebase = u32::try_from(fps.num)
        .map_err(|_| Error::Bundle(format!("unusable frame rate {}/{}", fps.num, fps.den)))?;
    let plan = CutPlan::from_wire(cuts, timebase);
    let default_edits = Edits::default();
    let edits = bundle.edits.as_ref().unwrap_or(&default_edits);
    let mut spans = Vec::new();
    for cut in effective_cuts(&plan, edits) {
        let (Some(start), Some(end)) = (
            cut.start.checked_snap_to_frame(fps),
            cut.end.checked_snap_to_frame(fps),
        ) else {
            continue;
        };
        if end > start {
            spans.push((start, end));
        }
    }
    Ok(spans)
}

/// Effective cuts -> keeps -> graph file -> ffmpeg -> atomic rename to `out`.
/// `on_progress` receives 0.0..=1.0.
///
/// # Errors
/// [`Error::WholeDurationRemoved`] before any spawn; [`Error::Render`] with the
/// stderr tail on ffmpeg failure (partial `.tmp` removed; bundle untouched).
pub async fn render_mp4(
    bundle: &Bundle,
    out: &Path,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<()> {
    let spans = effective_cut_spans(bundle)?;
    let fps = bundle.manifest.frame_rate;
    let keeps = keep_windows(&spans, bundle.manifest.duration, fps)?;
    let total = kept_total_secs(&keeps).max(f64::EPSILON);

    let graph_path = bundle.root.join(RENDER_FILTERGRAPH);
    crate::bundle::write_atomic(&graph_path, filter_complex_script(&keeps).as_bytes())?;

    let out_name = out
        .file_name()
        .ok_or_else(|| Error::Render(format!("no file name in {}", out.display())))?;
    let out_tmp = out.with_file_name(format!("{}.tmp", out_name.to_string_lossy()));

    let argv = render_argv(
        &bundle.manifest.source_video_absolute_path,
        &graph_path,
        &out_tmp,
    );
    let spawn = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(e) => return Err(Error::Render(format!("ffmpeg spawn failed: {e}"))),
    };

    // stderr drains on its own task so a chatty encoder can't deadlock the
    // stdout progress loop.
    let stderr_task = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let mut buf = String::new();
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        })
    });

    if let Some(stdout) = child.stdout.take() {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(secs) = parse_progress_line(&line) {
                on_progress((secs / total).min(1.0));
            }
        }
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(e) => {
            let _ = tokio::fs::remove_file(&out_tmp).await;
            return Err(Error::Render(format!("ffmpeg wait failed: {e}")));
        }
    };
    let stderr = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };
    if !status.success() {
        let _ = tokio::fs::remove_file(&out_tmp).await;
        let tail: String = stderr
            .chars()
            .skip(stderr.chars().count().saturating_sub(800))
            .collect();
        return Err(Error::Render(format!(
            "ffmpeg exited with {status}: {}",
            tail.trim()
        )));
    }
    tokio::fs::rename(&out_tmp, out).await?;
    on_progress(1.0);
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use crate::bundle::Bundle;
    use crate::rational::Rational;
    use crate::schema::manifest::ProjectManifest;
    use crate::schema::{Cut, CutReason, Cuts};

    pub(crate) fn wire_cuts(cuts: &[(f64, f64)], duration_secs: f64) -> Cuts {
        Cuts {
            source_duration_secs: duration_secs,
            cuts: cuts
                .iter()
                .map(|&(s, e)| Cut {
                    start: s,
                    end: e,
                    reason: CutReason::Filler,
                    excerpt: String::new(),
                })
                .collect(),
            discretionary: vec![],
            flags: vec![],
            total_cut_secs: cuts.iter().map(|(s, e)| e - s).sum(),
        }
    }

    pub(crate) fn bundle_literal(
        source: &Path,
        frame_rate: Rational,
        duration: Rational,
        cuts: &[(f64, f64)],
    ) -> Bundle {
        Bundle {
            root: PathBuf::new(),
            manifest: ProjectManifest {
                schema_version: 1,
                source_video_absolute_path: source.to_path_buf(),
                frame_rate,
                duration,
            },
            transcript: None,
            cuts: Some(wire_cuts(cuts, duration.to_secs_f64())),
            edits: None,
        }
    }

    /// Pure-value bundle: 25fps, 10s, absolute source path, hard cuts only.
    pub(crate) fn test_bundle_with_cuts(cuts: &[(f64, f64)]) -> Bundle {
        bundle_literal(
            Path::new("/media/clip.mp4"),
            Rational::new(25, 1),
            Rational::new(250, 25),
            cuts,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::test_support::test_bundle_with_cuts;
    use super::*;
    use crate::rational::Rational;

    #[test]
    fn render_argv_is_pinned_and_tmp_safe() {
        let argv = render_argv(
            Path::new("/a/clip.mp4"),
            Path::new("/b/g.txt"),
            Path::new("/c/out.mp4.tmp"),
        );
        assert_eq!(
            argv,
            vec![
                "ffmpeg",
                "-nostdin",
                "-loglevel",
                "error",
                "-y",
                "-progress",
                "pipe:1",
                "-i",
                "/a/clip.mp4",
                "-filter_complex_script",
                "/b/g.txt",
                "-map",
                "[v]",
                "-map",
                "[a]",
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-preset",
                "medium",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-movflags",
                "+faststart",
                "-f",
                "mp4",
                "/c/out.mp4.tmp",
            ]
        );
    }

    #[test]
    fn progress_line_parses_out_time_us() {
        assert_eq!(parse_progress_line("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress_line("frame=42"), None);
        assert_eq!(parse_progress_line("out_time_us=N/A"), None);
    }

    #[test]
    fn kept_total_sums_window_durations() {
        let keeps = vec![
            Keep {
                start: Rational::new(0, 25),
                end: Rational::new(25, 25),
            },
            Keep {
                start: Rational::new(50, 25),
                end: Rational::new(100, 25),
            },
        ];
        assert!((kept_total_secs(&keeps) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn effective_cut_spans_snaps_and_merges() {
        // manifest at 25fps; a wire cut at 1.013..2.017 snaps to 1.00..2.00 (nearest frame)
        let bundle = test_bundle_with_cuts(&[(1.013, 2.017)]);
        let spans = effective_cut_spans(&bundle).unwrap();
        assert_eq!(spans, vec![(Rational::new(25, 25), Rational::new(50, 25))]);
    }

    #[test]
    fn effective_cut_spans_requires_cuts_json() {
        let mut bundle = test_bundle_with_cuts(&[]);
        bundle.cuts = None;
        assert!(effective_cut_spans(&bundle).is_err());
    }

    #[tokio::test]
    #[ignore = "spawns real ffmpeg; owner checkpoint (KATTO_TEST_CLIP)"]
    async fn render_real_clip_end_to_end() {
        let Ok(clip) = std::env::var("KATTO_TEST_CLIP") else {
            eprintln!("KATTO_TEST_CLIP not set; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let imported = crate::import::import(Path::new(&clip), dir.path())
            .await
            .unwrap();
        let mut bundle = super::test_support::bundle_literal(
            Path::new(&clip),
            imported.manifest.frame_rate,
            imported.manifest.duration,
            &[(0.5, 1.0)],
        );
        bundle.root = imported.bundle_root.clone();
        let out = dir.path().join("out.mp4");
        render_mp4(&bundle, &out, &|_p| {}).await.unwrap();
        assert!(out.exists());
        assert!(!dir.path().join("out.mp4.tmp").exists());
    }
}
