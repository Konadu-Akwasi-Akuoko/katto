//! Engine-domain cut plan and the deterministic cuts↔edits merge.
//! `CutPlan::from_wire` is THE float→Rational boundary: wire seconds convert
//! exactly once, in the video frame timebase.

use serde::{Deserialize, Serialize};

use crate::rational::Rational;
use crate::schema::Cuts;
use crate::schema::edits::{CutEdge, Edits};

/// Where an effective cut came from, preserving provenance through the merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CutSource {
    /// A base cut from cuts.json `cuts[index]`.
    Base {
        /// Original cuts.json index.
        index: usize,
    },
    /// An applied discretionary candidate `discretionary[index]`.
    Discretionary {
        /// Original discretionary index.
        index: usize,
    },
    /// A human-added cut `edits.manual_cuts[index]`.
    Manual {
        /// Position in `edits.manual_cuts`.
        index: usize,
    },
}

/// One span of the merged cut list, in the plan's timebase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveCut {
    /// Span start.
    pub start: Rational,
    /// Span end.
    pub end: Rational,
    /// Which input produced this span.
    pub source: CutSource,
}

/// Wire cuts projected into the engine domain (Rational, one timebase).
#[derive(Debug, Clone, PartialEq)]
pub struct CutPlan {
    /// Ticks per second (the video frame timebase).
    pub timebase: u32,
    /// Base cut spans in cuts.json order.
    pub base: Vec<(Rational, Rational)>,
    /// Discretionary candidate spans in cuts.json order.
    pub discretionary: Vec<(Rational, Rational)>,
}

impl CutPlan {
    /// THE float->Rational boundary: convert wire cuts to engine Rational
    /// exactly once, in the video frame timebase (ticks per second).
    pub fn from_wire(cuts: &Cuts, timebase: u32) -> CutPlan {
        let span = |s: f64, e: f64| {
            (
                Rational::from_seconds(s, timebase),
                Rational::from_seconds(e, timebase),
            )
        };
        CutPlan {
            timebase,
            base: cuts.cuts.iter().map(|c| span(c.start, c.end)).collect(),
            discretionary: cuts
                .discretionary
                .iter()
                .map(|d| span(d.start, d.end))
                .collect(),
        }
    }
}

/// Base cuts minus toggled-off, plus applied discretionary, plus manual cuts,
/// with boundary adjustments applied to base cuts; sorted by `(start, end)`.
/// Pure and deterministic; overlaps are preserved (the Phase-5 renderer
/// coalesces at apply time) and out-of-range indices are ignored so edits.json
/// can outlive a re-plan.
pub fn effective_cuts(plan: &CutPlan, edits: &Edits) -> Vec<EffectiveCut> {
    let mut out = Vec::new();

    for (index, &(start, end)) in plan.base.iter().enumerate() {
        if edits.toggled_off.contains(&index) {
            continue;
        }
        let (mut start, mut end) = (start, end);
        for adj in &edits.boundary_adjustments {
            if adj.cut_index != index {
                continue;
            }
            let t = adj.new_time.rescale(plan.timebase);
            match adj.edge {
                CutEdge::Start => start = t,
                CutEdge::End => end = t,
            }
        }
        if end <= start {
            continue; // inverted by adjustment: vanishes rather than errors
        }
        out.push(EffectiveCut {
            start,
            end,
            source: CutSource::Base { index },
        });
    }

    for &index in &edits.applied_discretionary {
        if let Some(&(start, end)) = plan.discretionary.get(index) {
            out.push(EffectiveCut {
                start,
                end,
                source: CutSource::Discretionary { index },
            });
        }
    }

    for (index, m) in edits.manual_cuts.iter().enumerate() {
        let start = m.start.rescale(plan.timebase);
        let end = m.end.rescale(plan.timebase);
        if end <= start {
            continue;
        }
        out.push(EffectiveCut {
            start,
            end,
            source: CutSource::Manual { index },
        });
    }

    out.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational::Rational;
    use crate::schema::edits::{BoundaryAdjustment, CutEdge, Edits, ManualCut};
    use crate::schema::{Confidence, Cut, CutReason, Cuts, Discretionary, DiscretionaryReason};

    fn wire(cuts: &[(f64, f64)], disc: &[(f64, f64)]) -> Cuts {
        Cuts {
            source_duration_secs: 100.0,
            cuts: cuts
                .iter()
                .map(|&(s, e)| Cut {
                    start: s,
                    end: e,
                    reason: CutReason::Filler,
                    excerpt: String::new(),
                })
                .collect(),
            discretionary: disc
                .iter()
                .map(|&(s, e)| Discretionary {
                    start: s,
                    end: e,
                    reason: DiscretionaryReason::Other,
                    excerpt: String::new(),
                    note: "n".into(),
                    confidence: Confidence::Medium,
                })
                .collect(),
            flags: vec![],
            total_cut_secs: cuts.iter().map(|(s, e)| e - s).sum(),
        }
    }

    const TB: u32 = 1000;

    #[test]
    fn from_wire_converts_once_at_given_timebase() {
        let plan = CutPlan::from_wire(&wire(&[(4.21, 4.68)], &[]), TB);
        assert_eq!(
            plan.base,
            vec![(Rational::new(4210, TB), Rational::new(4680, TB))]
        );
    }

    #[test]
    fn toggle_off_removes_base_cut() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0), (3.0, 4.0)], &[]), TB);
        let edits = Edits {
            toggled_off: vec![0],
            ..Default::default()
        };
        let out = effective_cuts(&plan, &edits);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, CutSource::Base { index: 1 });
    }

    #[test]
    fn applied_discretionary_joins_sorted() {
        let plan = CutPlan::from_wire(&wire(&[(3.0, 4.0)], &[(1.0, 2.0)]), TB);
        let edits = Edits {
            applied_discretionary: vec![0],
            ..Default::default()
        };
        let out = effective_cuts(&plan, &edits);
        assert_eq!(out[0].source, CutSource::Discretionary { index: 0 });
        assert_eq!(out[1].source, CutSource::Base { index: 0 });
    }

    #[test]
    fn manual_cut_may_overlap_toggled_off_span() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
        let edits = Edits {
            toggled_off: vec![0],
            manual_cuts: vec![ManualCut {
                start: Rational::new(1500, TB),
                end: Rational::new(2500, TB),
                note: None,
            }],
            ..Default::default()
        };
        let out = effective_cuts(&plan, &edits);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, CutSource::Manual { index: 0 });
        assert_eq!(out[0].start, Rational::new(1500, TB));
    }

    #[test]
    fn boundary_adjustment_moves_base_edge() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
        let edits = Edits {
            boundary_adjustments: vec![BoundaryAdjustment {
                cut_index: 0,
                edge: CutEdge::End,
                new_time: Rational::new(1800, TB),
            }],
            ..Default::default()
        };
        let out = effective_cuts(&plan, &edits);
        assert_eq!(out[0].end, Rational::new(1800, TB));
    }

    #[test]
    fn inverting_adjustment_drops_the_cut() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
        let edits = Edits {
            boundary_adjustments: vec![BoundaryAdjustment {
                cut_index: 0,
                edge: CutEdge::End,
                new_time: Rational::new(500, TB),
            }],
            ..Default::default()
        };
        assert!(effective_cuts(&plan, &edits).is_empty());
    }

    #[test]
    fn out_of_range_indices_are_ignored() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
        let edits = Edits {
            toggled_off: vec![7],
            applied_discretionary: vec![3],
            ..Default::default()
        };
        assert_eq!(effective_cuts(&plan, &edits).len(), 1);
    }

    #[test]
    fn adjustment_on_toggled_off_cut_is_a_noop() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
        let edits = Edits {
            toggled_off: vec![0],
            boundary_adjustments: vec![BoundaryAdjustment {
                cut_index: 0,
                edge: CutEdge::End,
                new_time: Rational::new(1800, TB),
            }],
            ..Default::default()
        };
        assert!(effective_cuts(&plan, &edits).is_empty());
    }

    #[test]
    fn manual_cut_rescales_to_plan_timebase() {
        let plan = CutPlan::from_wire(&wire(&[], &[]), TB);
        let edits = Edits {
            manual_cuts: vec![ManualCut {
                start: Rational::new(1, 2), // 0.5 s in den 2
                end: Rational::new(3, 2),   // 1.5 s
                note: None,
            }],
            ..Default::default()
        };
        let out = effective_cuts(&plan, &edits);
        assert_eq!(out[0].start, Rational::new(500, TB));
        assert_eq!(out[0].end, Rational::new(1500, TB));
    }

    #[test]
    fn empty_edits_is_identity_on_base() {
        let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0), (3.0, 4.0)], &[(5.0, 6.0)]), TB);
        let out = effective_cuts(&plan, &Edits::default());
        assert_eq!(out.len(), 2); // discretionary not applied by default
    }
}
