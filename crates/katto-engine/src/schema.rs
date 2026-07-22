//! Serde schemas for the cut-pipeline artifact files. Wire shapes (`cuts.json`,
//! `transcript.json`) are decimal seconds at the model boundary; engine-domain
//! Rational projections live in `merge.rs`.

pub mod cuts;
pub mod edits;
pub mod manifest;
pub mod transcript;

pub use cuts::{
    Confidence, Cut, CutReason, Cuts, Discretionary, DiscretionaryReason, Flag, FlagReason,
};
pub use edits::{BoundaryAdjustment, CutEdge, Edits, ManualCut};
pub use manifest::ProjectManifest;
pub use transcript::{Transcript, WordEntry};
