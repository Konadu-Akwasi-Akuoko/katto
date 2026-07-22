//! Keep-window math and ffmpeg filtergraph text. Semantics ported verbatim
//! from hyper-frames `tools/cut-video/src/cut_video/segments.py`; katto
//! computes in [`Rational`] and projects to floats only in the emitted text.

use crate::error::{Error, Result};
use crate::rational::Rational;

/// A kept (retained) window in source time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keep {
    /// Window start in source time.
    pub start: Rational,
    /// Window end in source time.
    pub end: Rational,
}

/// Sort removed spans by start and merge overlapping/touching ones; drop empty spans.
pub fn coalesce_cuts(cuts: &[(Rational, Rational)]) -> Vec<(Rational, Rational)> {
    let mut spans: Vec<(Rational, Rational)> =
        cuts.iter().copied().filter(|&(s, e)| e > s).collect();
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut out: Vec<(Rational, Rational)> = Vec::new();
    for (start, end) in spans {
        match out.last_mut() {
            Some(prev) if start <= prev.1 => {
                if end > prev.1 {
                    prev.1 = end;
                }
            }
            _ => out.push((start, end)),
        }
    }
    out
}

/// Complement removed spans into kept windows over `[0, duration]`; drop keeps
/// whose duration is at most one frame of `fps`.
///
/// # Errors
/// [`Error::WholeDurationRemoved`] when the cuts cover the entire source.
pub fn keep_windows(
    cuts: &[(Rational, Rational)],
    duration: Rational,
    fps: Rational,
) -> Result<Vec<Keep>> {
    let zero = Rational::new(0, duration.den);
    if duration <= zero {
        return Err(Error::WholeDurationRemoved);
    }
    let clipped: Vec<(Rational, Rational)> = cuts
        .iter()
        .map(|&(s, e)| (s.max(zero), e.min(duration)))
        .collect();
    let merged = coalesce_cuts(&clipped);

    let mut windows = Vec::with_capacity(merged.len() + 1);
    let mut cursor = zero;
    for (start, end) in merged {
        windows.push(Keep {
            start: cursor,
            end: start,
        });
        cursor = end;
    }
    windows.push(Keep {
        start: cursor,
        end: duration,
    });

    let keeps: Vec<Keep> = windows
        .into_iter()
        .filter(|k| {
            k.end
                .checked_sub(k.start)
                .is_some_and(|d| exceeds_one_frame(d, fps))
        })
        .collect();
    if keeps.is_empty() {
        return Err(Error::WholeDurationRemoved);
    }
    Ok(keeps)
}

/// Exact `d > fps.den/fps.num` (one frame in seconds), cross-multiplied so
/// equal instants in different timebases compare equal (no `Ord` den tiebreak).
fn exceeds_one_frame(d: Rational, fps: Rational) -> bool {
    i128::from(d.num) * i128::from(fps.num) > i128::from(fps.den) * i128::from(d.den)
}

/// Deterministic ffmpeg filter_complex_script text for the keep list (both streams).
/// Boundaries formatted `%.6f`; byte-identical across runs for identical input.
pub fn filter_complex_script(keeps: &[Keep]) -> String {
    let mut lines = Vec::with_capacity(keeps.len() * 2 + 1);
    for (i, keep) in keeps.iter().enumerate() {
        let s = format!("{:.6}", keep.start.to_secs_f64());
        let e = format!("{:.6}", keep.end.to_secs_f64());
        lines.push(format!(
            "[0:v]trim=start={s}:end={e},setpts=PTS-STARTPTS[v{i}]"
        ));
        lines.push(format!(
            "[0:a]atrim=start={s}:end={e},asetpts=PTS-STARTPTS[a{i}]"
        ));
    }
    let inputs: String = (0..keeps.len()).map(|i| format!("[v{i}][a{i}]")).collect();
    lines.push(format!("{inputs}concat=n={}:v=1:a=1[v][a]", keeps.len()));
    lines.join(";\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::rational::Rational;

    const TB: u32 = 1000;
    fn r(ms: i64) -> Rational {
        Rational::new(ms, TB)
    }
    const FPS25: Rational = Rational { num: 25, den: 1 };

    #[test]
    fn coalesce_merges_touching_and_overlapping() {
        // parity: segments.py merges start <= prev_end (touching included)
        let out = coalesce_cuts(&[(r(2000), r(3000)), (r(1000), r(2000)), (r(2500), r(2600))]);
        assert_eq!(out, vec![(r(1000), r(3000))]);
    }

    #[test]
    fn coalesce_drops_empty_spans() {
        assert_eq!(
            coalesce_cuts(&[(r(1000), r(1000)), (r(2000), r(1500))]),
            vec![]
        );
    }

    #[test]
    fn keep_windows_walks_cursor_and_emits_tail() {
        let keeps = keep_windows(&[(r(1000), r(2000))], r(5000), FPS25).unwrap();
        assert_eq!(
            keeps,
            vec![
                Keep {
                    start: r(0),
                    end: r(1000)
                },
                Keep {
                    start: r(2000),
                    end: r(5000)
                },
            ]
        );
    }

    #[test]
    fn keep_windows_clips_out_of_range_cuts() {
        // parity: clipped_start = max(cut_start, 0), clipped_end = min(cut_end, duration)
        let keeps =
            keep_windows(&[(r(-500), r(1000)), (r(4500), r(9000))], r(5000), FPS25).unwrap();
        assert_eq!(
            keeps,
            vec![Keep {
                start: r(1000),
                end: r(4500)
            }]
        );
    }

    #[test]
    fn sub_epsilon_keep_between_cuts_is_dropped() {
        // 25fps epsilon = 40ms; the only surviving window is the 30ms sliver between
        // the two cuts -> dropped -> nothing remains -> loud error (segments.py parity)
        assert!(matches!(
            keep_windows(&[(r(0), r(1000)), (r(1030), r(5000))], r(5000), FPS25),
            Err(Error::WholeDurationRemoved)
        ));
        // and a sliver bigger than epsilon survives
        let keeps = keep_windows(&[(r(0), r(1000)), (r(1050), r(5000))], r(5000), FPS25).unwrap();
        assert_eq!(
            keeps,
            vec![Keep {
                start: r(1000),
                end: r(1050)
            }]
        );
    }

    #[test]
    fn exactly_one_frame_keep_is_dropped_strictly() {
        // a keep of exactly one frame (40ms at 25fps) must NOT survive: the
        // comparison is strict (end - start > epsilon)
        assert!(matches!(
            keep_windows(&[(r(0), r(1000)), (r(1040), r(5000))], r(5000), FPS25),
            Err(Error::WholeDurationRemoved)
        ));
    }

    #[test]
    fn whole_duration_removed_is_loud() {
        assert!(matches!(
            keep_windows(&[(r(0), r(5000))], r(5000), FPS25),
            Err(Error::WholeDurationRemoved)
        ));
    }

    #[test]
    fn filtergraph_snapshot_basic() {
        let keeps = keep_windows(&[(r(1000), r(2000))], r(5000), FPS25).unwrap();
        insta::assert_snapshot!("filtergraph_basic", filter_complex_script(&keeps));
    }

    #[test]
    fn filtergraph_500_cuts_is_structurally_sound() {
        // argv-limit-safety fixture: 600 alternating cuts over a long source
        let cuts: Vec<_> = (0..600)
            .map(|i| (r(i * 2000 + 1000), r(i * 2000 + 1500)))
            .collect();
        let keeps = keep_windows(&cuts, r(600 * 2000 + 1000), FPS25).unwrap();
        assert_eq!(keeps.len(), 601);
        let graph = filter_complex_script(&keeps);
        let lines: Vec<_> = graph.trim_end().split(";\n").collect();
        assert_eq!(lines.len(), 2 * 601 + 1);
        assert!(lines.last().unwrap().contains("concat=n=601:v=1:a=1[v][a]"));
        assert!(graph.ends_with('\n'));
    }

    #[test]
    fn filtergraph_is_byte_deterministic() {
        let keeps = keep_windows(&[(r(1234), r(4567))], r(10000), FPS25).unwrap();
        assert_eq!(filter_complex_script(&keeps), filter_complex_script(&keeps));
        assert!(filter_complex_script(&keeps).contains("trim=start=0.000000:end=1.234000"));
    }
}
