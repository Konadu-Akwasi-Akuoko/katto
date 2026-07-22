//! Serde schemas for the cut-decider artifact files (`projects.json`,
//! `cuts.json`, `edits.json`), mirrored verbatim from the hyper-frames studio
//! pipeline.

use crate::rational::Rational;
use serde::{Deserialize, Serialize};

/// Why a span is a hard cut in `cuts.json`.
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

/// The model's confidence in a discretionary suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// The `projects.json` document: source video identity and timing.
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    source_video_absolute_path: String,
    frame_rate: Rational,
    total_duration: Rational,
    schema_version: String,
}

/// The `cuts.json` document: the decider's proposed cut list.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cuts {
    source_duration_secs: Rational,
    cuts: Vec<Cut>,
    discretionary: Vec<Discretionary>,
    flags: Vec<Flag>,
    total_cut_secs: Rational,
}

/// The `edits.json` document: the human-reviewed edit state over a cut list.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edits {
    #[serde(flatten)]
    cuts_obj: Cuts,
    toggles: Vec<String>,
    manual_cuts: Vec<Cut>,
    boundary_adjustments: String,
}

/// One cut span with its reason and transcript excerpt.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cut {
    start: Rational,
    end: Rational,
    reason: String,
    excerpt: String,
}

/// A discretionary cut candidate: a [`Cut`] plus a note and confidence.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discretionary {
    #[serde(flatten)]
    cut: Cut,
    note: String,
    confidence: String,
}

/// A flagged span the decider is unsure about, with its raw logprob.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flag {
    #[serde(flatten)]
    cut: Cut,
    logprob: f64,
}
