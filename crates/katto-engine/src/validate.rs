//! cuts.json invariant validation against its transcript. Pure: every
//! invariant is checked independently; an empty error vec means valid.

use crate::schema::{Cuts, Transcript, WordEntry};

/// Tolerance for all float comparisons at the model boundary.
pub const FLOAT_TOLERANCE: f64 = 1e-3;

/// Which span list a validation error points into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanList {
    /// The hard `cuts` list.
    Cuts,
    /// The `discretionary` candidate list.
    Discretionary,
    /// The `flags` list.
    Flags,
}

/// One violated cuts.json invariant; `Display` names the offending entry.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// A span's bounds fall outside `[0, source_duration_secs]` or are inverted.
    #[error(
        "{list:?}[{index}]: start {start} must be >= 0 and < end {end}, end <= source duration {duration}"
    )]
    OutOfBounds {
        /// Offending list.
        list: SpanList,
        /// Index into that list.
        index: usize,
        /// The span's start.
        start: f64,
        /// The span's end.
        end: f64,
        /// The stated source duration.
        duration: f64,
    },
    /// Cuts are not sorted by start.
    #[error("cuts[{index}]: not sorted by start (previous start {prev_start}, this start {start})")]
    Unsorted {
        /// Index of the out-of-order cut.
        index: usize,
        /// The previous cut's start.
        prev_start: f64,
        /// This cut's start.
        start: f64,
    },
    /// A cut overlaps the previous cut.
    #[error("cuts[{index}]: overlaps previous cut (previous end {prev_end}, this start {start})")]
    Overlap {
        /// Index of the overlapping cut.
        index: usize,
        /// The previous cut's end.
        prev_end: f64,
        /// This cut's start.
        start: f64,
    },
    /// A discretionary candidate overlaps a hard cut.
    #[error("discretionary[{d_index}]: overlaps cuts[{c_index}]")]
    DiscretionaryOverlapsCut {
        /// Index into `discretionary`.
        d_index: usize,
        /// Index into `cuts`.
        c_index: usize,
    },
    /// A flag shares a span with a hard cut (flagged words are never cut).
    #[error("flags[{f_index}]: shares a span with cuts[{c_index}] — flagged words are never cut")]
    FlagSharesCutSpan {
        /// Index into `flags`.
        f_index: usize,
        /// Index into `cuts`.
        c_index: usize,
    },
    /// A discretionary candidate has an empty note.
    #[error("discretionary[{index}]: note must be non-empty")]
    EmptyNote {
        /// Index into `discretionary`.
        index: usize,
    },
    /// `total_cut_secs` disagrees with the sum of cut durations.
    #[error("total_cut_secs {stated} does not match sum of cut durations {computed:.6}")]
    TotalMismatch {
        /// The stated total.
        stated: f64,
        /// The computed sum.
        computed: f64,
    },
    /// A boundary does not land on any transcript token boundary.
    #[error("{list:?}[{index}]: boundary {value} is not a token boundary in the transcript")]
    MisalignedBoundary {
        /// Offending list.
        list: SpanList,
        /// Index into that list.
        index: usize,
        /// The misaligned boundary value.
        value: f64,
    },
    /// A boundary falls strictly inside a word token.
    #[error("{list:?}[{index}]: boundary {value} falls inside word token {token:?}")]
    InsideWordToken {
        /// Offending list.
        list: SpanList,
        /// Index into that list.
        index: usize,
        /// The offending boundary value.
        value: f64,
        /// The word the boundary lands inside.
        token: String,
    },
}

/// Check every cuts.json invariant against its transcript. Empty vec == valid.
pub fn validate_cuts(cuts: &Cuts, transcript: &Transcript) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let dur = cuts.source_duration_secs;

    check_bounds(
        &mut errors,
        SpanList::Cuts,
        cuts.cuts.iter().map(|c| (c.start, c.end)),
        dur,
    );
    check_bounds(
        &mut errors,
        SpanList::Discretionary,
        cuts.discretionary.iter().map(|d| (d.start, d.end)),
        dur,
    );
    check_bounds(
        &mut errors,
        SpanList::Flags,
        cuts.flags.iter().map(|f| (f.start, f.end)),
        dur,
    );

    // 2. sorted + non-overlapping cuts (in given order — order is part of the contract)
    for i in 1..cuts.cuts.len() {
        let (prev, cur) = (&cuts.cuts[i - 1], &cuts.cuts[i]);
        if cur.start < prev.start - FLOAT_TOLERANCE {
            errors.push(ValidationError::Unsorted {
                index: i,
                prev_start: prev.start,
                start: cur.start,
            });
        } else if cur.start < prev.end - FLOAT_TOLERANCE {
            errors.push(ValidationError::Overlap {
                index: i,
                prev_end: prev.end,
                start: cur.start,
            });
        }
    }

    // 3. discretionary never overlaps a hard cut
    for (d_index, d) in cuts.discretionary.iter().enumerate() {
        for (c_index, c) in cuts.cuts.iter().enumerate() {
            if overlaps((d.start, d.end), (c.start, c.end)) {
                errors.push(ValidationError::DiscretionaryOverlapsCut { d_index, c_index });
            }
        }
    }

    // 4. flags never share a span with cuts
    for (f_index, f) in cuts.flags.iter().enumerate() {
        for (c_index, c) in cuts.cuts.iter().enumerate() {
            if overlaps((f.start, f.end), (c.start, c.end)) {
                errors.push(ValidationError::FlagSharesCutSpan { f_index, c_index });
            }
        }
    }

    // 5. non-empty notes (required fields / closed enums are serde's job)
    for (index, d) in cuts.discretionary.iter().enumerate() {
        if d.note.trim().is_empty() {
            errors.push(ValidationError::EmptyNote { index });
        }
    }

    // 6. stated total vs computed sum (cuts only)
    let computed: f64 = cuts.cuts.iter().map(|c| c.end - c.start).sum();
    if (computed - cuts.total_cut_secs).abs() > FLOAT_TOLERANCE {
        errors.push(ValidationError::TotalMismatch {
            stated: cuts.total_cut_secs,
            computed,
        });
    }

    // 7. token alignment for cuts and discretionary (not flags)
    let boundaries = token_boundaries(transcript);
    check_alignment(
        &mut errors,
        SpanList::Cuts,
        cuts.cuts.iter().map(|c| (c.start, c.end)),
        &boundaries,
        transcript,
    );
    check_alignment(
        &mut errors,
        SpanList::Discretionary,
        cuts.discretionary.iter().map(|d| (d.start, d.end)),
        &boundaries,
        transcript,
    );

    errors
}

fn check_bounds(
    errors: &mut Vec<ValidationError>,
    list: SpanList,
    spans: impl Iterator<Item = (f64, f64)>,
    duration: f64,
) {
    for (index, (start, end)) in spans.enumerate() {
        let in_bounds = start >= -FLOAT_TOLERANCE
            && start < end - FLOAT_TOLERANCE
            && end <= duration + FLOAT_TOLERANCE;
        if !in_bounds {
            errors.push(ValidationError::OutOfBounds {
                list,
                index,
                start,
                end,
                duration,
            });
        }
    }
}

/// Interval overlap with tolerance (same test as the Zod reference).
fn overlaps(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0 < b.1 - FLOAT_TOLERANCE && b.0 < a.1 - FLOAT_TOLERANCE
}

/// Every token's start and end, sorted for binary search.
fn token_boundaries(transcript: &Transcript) -> Vec<f64> {
    let mut b: Vec<f64> = transcript
        .words
        .iter()
        .flat_map(|w| [w.start(), w.end()])
        .collect();
    b.sort_by(f64::total_cmp);
    b
}

fn is_boundary(boundaries: &[f64], value: f64) -> bool {
    let i = boundaries.partition_point(|&b| b < value - FLOAT_TOLERANCE);
    boundaries
        .get(i)
        .is_some_and(|&b| (b - value).abs() <= FLOAT_TOLERANCE)
}

fn inside_word(transcript: &Transcript, value: f64) -> Option<String> {
    transcript.words.iter().find_map(|w| match w {
        WordEntry::Word {
            text, start, end, ..
        } if *start + FLOAT_TOLERANCE < value && value < *end - FLOAT_TOLERANCE => {
            Some(text.clone())
        }
        _ => None,
    })
}

fn check_alignment(
    errors: &mut Vec<ValidationError>,
    list: SpanList,
    spans: impl Iterator<Item = (f64, f64)>,
    boundaries: &[f64],
    transcript: &Transcript,
) {
    for (index, (start, end)) in spans.enumerate() {
        for value in [start, end] {
            if let Some(token) = inside_word(transcript, value) {
                errors.push(ValidationError::InsideWordToken {
                    list,
                    index,
                    value,
                    token,
                });
            } else if !is_boundary(boundaries, value) {
                errors.push(ValidationError::MisalignedBoundary { list, index, value });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn load(name: &str) -> Cuts {
        serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/tests/fixtures/{name}",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap()
    }

    fn transcript() -> Transcript {
        serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/tests/fixtures/transcript.valid.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn valid_fixture_passes() {
        assert_eq!(
            validate_cuts(&load("cuts.valid.json"), &transcript()),
            vec![]
        );
    }

    #[rstest]
    #[case("cuts.overlap.json")]
    #[case("cuts.out-of-bounds.json")]
    #[case("cuts.misaligned-token.json")]
    #[case("cuts.flag-overlap.json")]
    #[case("cuts.bad-total.json")]
    fn invalid_fixture_names_the_invariant(#[case] name: &str) {
        let errors = validate_cuts(&load(name), &transcript());
        assert!(!errors.is_empty(), "{name} should fail validation");
        for e in &errors {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn overlap_fixture_reports_overlap_variant() {
        let errors = validate_cuts(&load("cuts.overlap.json"), &transcript());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::Overlap { .. }))
        );
    }

    #[test]
    fn misaligned_fixture_reports_misaligned_variant() {
        let errors = validate_cuts(&load("cuts.misaligned-token.json"), &transcript());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::MisalignedBoundary { .. }))
        );
    }

    #[test]
    fn flag_overlap_fixture_reports_shared_span() {
        let errors = validate_cuts(&load("cuts.flag-overlap.json"), &transcript());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::FlagSharesCutSpan { .. }))
        );
    }

    #[test]
    fn bad_total_reports_total_mismatch() {
        let errors = validate_cuts(&load("cuts.bad-total.json"), &transcript());
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::TotalMismatch { .. }))
        );
    }

    #[test]
    fn boundary_inside_word_token_is_named() {
        // synthesize: transcript with one word 1.0..2.0; a cut 1.5..2.0 starts inside it
        let t: Transcript = serde_json::from_str(
            r#"{
            "audio_duration_secs": 3.0, "language_code": "en", "language_probability": 1.0,
            "text": "hi",
            "words": [{"text":"hi","type":"word","start":1.0,"end":2.0,"logprob":-0.1}]
        }"#,
        )
        .unwrap();
        let c: Cuts = serde_json::from_str(
            r#"{
            "source_duration_secs": 3.0,
            "cuts": [{"start":1.5,"end":2.0,"reason":"filler","excerpt":"hi"}],
            "flags": [], "total_cut_secs": 0.5
        }"#,
        )
        .unwrap();
        let errors = validate_cuts(&c, &t);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::InsideWordToken { .. }))
        );
    }
}
