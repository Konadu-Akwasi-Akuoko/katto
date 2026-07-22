//! katto-owned `edits.json` wire format (net-new, no mirror antecedent).
//! Rational-native: human edit state must not lose precision.

use serde::{Deserialize, Serialize};

use crate::rational::Rational;

/// Which edge of a cut a boundary adjustment moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CutEdge {
    /// The cut's start edge.
    Start,
    /// The cut's end edge.
    End,
}

/// A human nudge to one edge of a base cut (index into cuts.json `cuts[]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BoundaryAdjustment {
    /// Index of the base cut being adjusted (original cuts.json position).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub cut_index: usize,
    /// Which edge moves.
    pub edge: CutEdge,
    /// The new edge time (any timebase; rescaled to the plan's on merge).
    pub new_time: Rational,
}

/// A human-added cut span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ManualCut {
    /// Start time.
    pub start: Rational,
    /// End time.
    pub end: Rational,
    /// Optional human note.
    pub note: Option<String>,
}

/// The `edits.json` document: human review state layered over cuts.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Edits {
    /// Wire format version (currently 1).
    pub schema_version: u32,
    /// Indices into cuts.json `cuts[]` the human switched off.
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Vec<u32>))]
    pub toggled_off: Vec<usize>,
    /// Indices into `discretionary[]` the human applied.
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Vec<u32>))]
    pub applied_discretionary: Vec<usize>,
    /// Human-added cuts.
    #[serde(default)]
    pub manual_cuts: Vec<ManualCut>,
    /// Human nudges to base-cut edges.
    #[serde(default)]
    pub boundary_adjustments: Vec<BoundaryAdjustment>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational::Rational;

    #[test]
    fn edits_round_trip_preserves_rational() {
        let e = Edits {
            schema_version: 1,
            toggled_off: vec![2],
            applied_discretionary: vec![],
            manual_cuts: vec![ManualCut {
                start: Rational::new(1, 30000),
                end: Rational::new(2, 30000),
                note: None,
            }],
            boundary_adjustments: vec![BoundaryAdjustment {
                cut_index: 0,
                edge: CutEdge::Start,
                new_time: Rational::new(5, 1000),
            }],
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Edits>(&json).unwrap(), e);
    }
}
