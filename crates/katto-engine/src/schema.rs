use crate::rational::Rational;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutReason {
    Filler,
    Stutter,
    FalseStart,
    SelfCorrection,
    LongSilence,
    AudioEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscretionaryReason {
    Filler,
    Stutter,
    FalseStart,
    SelfCorrection,
    LongSilence,
    AudioEvent,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

// projects.json
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    source_video_absolute_path: String,
    frame_rate: Rational,
    total_duration: Rational,
    schema_version: String,
}

// cuts.json
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cuts {
    source_duration_secs: Rational,
    cuts: Vec<Cut>,
    discretionary: Vec<Discretionary>,
    flags: Vec<Flag>,
    total_cut_secs: Rational,
}

// edits.json
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edits {
    #[serde(flatten)]
    cuts_obj: Cuts,
    toggles: Vec<String>,
    manual_cuts: Vec<Cut>,
    boundary_adjustments: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cut {
    start: Rational,
    end: Rational,
    reason: String,
    excerpt: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discretionary {
    #[serde(flatten)]
    cut: Cut,
    note: String,
    confidence: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flag {
    #[serde(flatten)]
    cut: Cut,
    logprob: f64,
}
