//! Kept-only caption retiming and the SRT / WebVTT text emitters. Pure; all
//! math in [`Rational`], floats only at the final timestamp formatting.

use crate::rational::Rational;
use crate::schema::WordEntry;

/// A caption cue in output (kept-only) time.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    /// Cue start in kept-only time.
    pub start: Rational,
    /// Cue end in kept-only time.
    pub end: Rational,
    /// Cue text.
    pub text: String,
}

/// Maximum caption line length before a break (characters).
const MAX_LINE_CHARS: usize = 42;

/// Exact `a > b` by cross-multiplication (avoids `Ord`'s den tiebreak for
/// equal instants in different timebases).
fn strictly_after(a: Rational, b: Rational) -> bool {
    i128::from(a.num) * i128::from(b.den) > i128::from(b.num) * i128::from(a.den)
}

/// Drop words inside cuts; shift survivors left by the preceding removed total.
/// Cuts must be sorted+disjoint (the caller passes coalesced spans). Word
/// containment is by midpoint (cut boundaries sit on token edges per
/// validation invariant 7). One `Caption` per kept Word token; spacing and
/// audio-event tokens are dropped.
pub fn retime_kept_words(
    words: &[WordEntry],
    cuts: &[(Rational, Rational)],
    timebase: u32,
) -> Vec<Caption> {
    let zero = Rational::new(0, timebase);
    let mut out = Vec::new();
    for word in words {
        let WordEntry::Word {
            text, start, end, ..
        } = word
        else {
            continue;
        };
        let s = Rational::from_seconds(*start, timebase);
        let e = Rational::from_seconds(*end, timebase);
        let mid = Rational::from_seconds((*start + *end) / 2.0, timebase);
        if cuts.iter().any(|&(cs, ce)| cs <= mid && mid < ce) {
            continue;
        }
        let mut removed = zero;
        for &(cs, ce) in cuts {
            if strictly_after(mid, ce) || ce == mid {
                let Some(d) = ce.checked_sub(cs) else {
                    continue;
                };
                let Some(sum) = removed.checked_add(d) else {
                    continue;
                };
                removed = sum;
            }
        }
        let shifted_start = s.checked_sub(removed).unwrap_or(zero).max(zero);
        let shifted_end = e.checked_sub(removed).unwrap_or(zero).max(zero);
        out.push(Caption {
            start: shifted_start,
            end: shifted_end,
            text: text.clone(),
        });
    }
    out
}

/// Group per-word cues into caption lines: break at sentence end (`.`/`?`/`!`),
/// or when a line would exceed 42 chars, or at a >1.0s kept-time gap.
pub fn group_captions(words: &[Caption]) -> Vec<Caption> {
    let one_second = Rational::new(1, 1);
    let mut lines: Vec<Caption> = Vec::new();
    let mut current: Option<Caption> = None;
    for word in words {
        match current.take() {
            None => current = Some(word.clone()),
            Some(mut line) => {
                let gap = word
                    .start
                    .checked_sub(line.end)
                    .is_some_and(|d| strictly_after(d, one_second));
                let would_overflow = line.text.len() + 1 + word.text.len() > MAX_LINE_CHARS;
                if gap || would_overflow {
                    lines.push(line);
                    current = Some(word.clone());
                } else {
                    line.text.push(' ');
                    line.text.push_str(&word.text);
                    line.end = word.end;
                    current = Some(line);
                }
            }
        }
        if let Some(line) = &current
            && line.text.ends_with(['.', '?', '!'])
        {
            lines.push(current.take().unwrap_or_default());
        }
    }
    if let Some(line) = current {
        lines.push(line);
    }
    lines
}

impl Default for Caption {
    fn default() -> Self {
        Caption {
            start: Rational::new(0, 1),
            end: Rational::new(0, 1),
            text: String::new(),
        }
    }
}

/// `HH:MM:SS<sep>mmm` — the only float projection, at the text boundary.
fn timestamp(t: Rational, sep: char) -> String {
    let total_ms = (t.to_secs_f64() * 1000.0).round().max(0.0) as u64;
    let (ms, total_s) = (total_ms % 1000, total_ms / 1000);
    let (s, total_m) = (total_s % 60, total_s / 60);
    let (m, h) = (total_m % 60, total_m / 60);
    format!("{h:02}:{m:02}:{s:02}{sep}{ms:03}")
}

/// SRT text: 1-based index, `HH:MM:SS,mmm --> HH:MM:SS,mmm`, blank-line separated.
pub fn emit_srt(captions: &[Caption]) -> String {
    let mut out = String::new();
    for (i, cap) in captions.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            timestamp(cap.start, ','),
            timestamp(cap.end, ','),
            cap.text
        ));
    }
    out
}

/// WebVTT text: `WEBVTT` + blank line; `HH:MM:SS.mmm` timestamps; `&`->`&amp;`
/// and `<`->`&lt;` escaped in payloads.
pub fn emit_vtt(captions: &[Caption]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for cap in captions {
        let text = cap.text.replace('&', "&amp;").replace('<', "&lt;");
        out.push_str(&format!(
            "{} --> {}\n{}\n\n",
            timestamp(cap.start, '.'),
            timestamp(cap.end, '.'),
            text
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational::Rational;
    use crate::schema::WordEntry;

    const TB: u32 = 1000;
    fn r(ms: i64) -> Rational {
        Rational::new(ms, TB)
    }
    fn w(text: &str, s: f64, e: f64) -> WordEntry {
        WordEntry::Word {
            text: text.into(),
            start: s,
            end: e,
            logprob: None,
            speaker_id: None,
        }
    }

    #[test]
    fn words_inside_cuts_are_dropped_and_survivors_shift_left() {
        let words = vec![w("keep", 0.0, 0.5), w("cut", 1.0, 1.5), w("tail", 2.0, 2.5)];
        let out = retime_kept_words(&words, &[(r(1000), r(2000))], TB);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].text, "keep");
        assert_eq!(out[1].start, r(1000)); // 2.0 - 1.0s removed
    }

    #[test]
    fn spacing_and_audio_events_are_dropped() {
        let words = vec![
            w("keep", 0.0, 0.5),
            WordEntry::Spacing {
                text: " ".into(),
                start: 0.5,
                end: 0.6,
            },
            WordEntry::AudioEvent {
                text: "[breath]".into(),
                start: 0.6,
                end: 0.9,
            },
        ];
        let out = retime_kept_words(&words, &[], TB);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn grouping_breaks_at_sentence_end_and_42_chars() {
        let words: Vec<Caption> = ["This", "is", "a", "sentence."]
            .iter()
            .enumerate()
            .map(|(i, t)| Caption {
                start: r(i as i64 * 300),
                end: r(i as i64 * 300 + 250),
                text: (*t).into(),
            })
            .chain([Caption {
                start: r(1500),
                end: r(1800),
                text: "Next".into(),
            }])
            .collect();
        let lines = group_captions(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "This is a sentence.");
        assert_eq!(lines[0].end, r(1150));
    }

    #[test]
    fn grouping_breaks_when_a_line_would_pass_42_chars() {
        let words: Vec<Caption> = (0..12)
            .map(|i| Caption {
                start: r(i * 300),
                end: r(i * 300 + 250),
                text: "abcdef".into(), // 12 * 7 - 1 chars if unbroken
            })
            .collect();
        let lines = group_captions(&words);
        assert!(lines.iter().all(|l| l.text.len() <= 42));
        assert!(lines.len() > 1);
    }

    #[test]
    fn grouping_breaks_at_gaps_over_one_second() {
        let words = vec![
            Caption {
                start: r(0),
                end: r(400),
                text: "before".into(),
            },
            Caption {
                start: r(1500),
                end: r(1900),
                text: "after".into(),
            },
        ];
        let lines = group_captions(&words);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn srt_and_vtt_snapshots() {
        let caps = vec![
            Caption {
                start: r(0),
                end: r(1200),
                text: "Hello <world> & co".into(),
            },
            Caption {
                start: r(3_600_000),
                end: r(3_601_500),
                text: "Past the hour".into(),
            },
        ];
        insta::assert_snapshot!("captions_srt", emit_srt(&caps));
        insta::assert_snapshot!("captions_vtt", emit_vtt(&caps));
    }
}
