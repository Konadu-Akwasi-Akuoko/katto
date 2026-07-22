//! Wire `cuts.json` types (clean-audio / cut-decider contract, restated
//! normatively in the PRD). Decimal seconds: conversion to `Rational` happens
//! exactly once, in `merge::CutPlan::from_wire`.

use serde::{Deserialize, Serialize};

/// Why a span is a hard cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum CutReason {
    Filler,
    Stutter,
    FalseStart,
    SelfCorrection,
    LongSilence,
    AudioEvent,
}

/// Why a span is a discretionary (human-decided) cut candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum DiscretionaryReason {
    Filler,
    Stutter,
    FalseStart,
    SelfCorrection,
    LongSilence,
    AudioEvent,
    Other,
}

/// Why a span is flagged for review (flags are never cut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum FlagReason {
    LowConfidence,
}

/// The model's confidence in a discretionary suggestion — the locked enum;
/// never rendered as a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// One hard cut span with its reason and transcript excerpt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cut {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Why this span is cut.
    pub reason: CutReason,
    /// The transcript text the span covers.
    pub excerpt: String,
}

/// A discretionary cut candidate the human decides on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Discretionary {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Why this span is a candidate.
    pub reason: DiscretionaryReason,
    /// The transcript text the span covers.
    pub excerpt: String,
    /// The model's rationale, shown to the human.
    pub note: String,
    /// The model's confidence (enum, never a number).
    pub confidence: Confidence,
}

/// A span flagged for review with its raw recognition logprob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Why this span is flagged.
    pub reason: FlagReason,
    /// The transcript text the span covers.
    pub excerpt: String,
    /// Raw log probability that triggered the flag.
    pub logprob: f64,
}

/// The `cuts.json` document: the planner's proposed cut list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cuts {
    /// Source duration in seconds as the planner saw it.
    pub source_duration_secs: f64,
    /// Hard cuts, sorted by start.
    pub cuts: Vec<Cut>,
    /// Discretionary candidates (absent key defaults to empty).
    #[serde(default)]
    pub discretionary: Vec<Discretionary>,
    /// Review flags (absent key defaults to empty).
    #[serde(default)]
    pub flags: Vec<Flag>,
    /// Stated sum of cut durations; validated against the computed sum.
    pub total_cut_secs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn parses_cuts_valid_fixture_with_defaulted_discretionary() {
        let c: Cuts = serde_json::from_str(&fixture("cuts.valid.json")).unwrap();
        assert_eq!(c.cuts.len(), 2);
        assert!(c.discretionary.is_empty()); // key absent in fixture -> default
        assert_eq!(c.cuts[0].reason, CutReason::Filler);
        assert_eq!(c.flags[0].reason, FlagReason::LowConfidence);
    }

    #[test]
    fn reason_enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&CutReason::FalseStart).unwrap(),
            "\"false_start\""
        );
        assert_eq!(
            serde_json::to_string(&DiscretionaryReason::Other).unwrap(),
            "\"other\""
        );
        assert_eq!(
            serde_json::to_string(&Confidence::Medium).unwrap(),
            "\"medium\""
        );
    }

    #[test]
    fn round_trips_the_valid_fixture() {
        let c: Cuts = serde_json::from_str(&fixture("cuts.valid.json")).unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Cuts>(&json).unwrap(), c);
    }
}
