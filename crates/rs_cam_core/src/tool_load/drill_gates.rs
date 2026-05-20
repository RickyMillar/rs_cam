//! Drill-specific guardrails: chip welding, peck adequacy, plunge feed.
//!
//! Independent of the milling-side `chipload` / `power` / `deflection`
//! gates: those slot `Unmodeled(NotApplicableForOp)` for drill ops. This
//! module evaluates the substrate that drilling *does* produce — a
//! per-peck [`DrillSample`] stream and a per-toolpath
//! [`DrillToolpathSummary`] — and yields three verdicts surfaced on
//! [`ToolpathLoadVerdict::drill_gates`].
//!
//! See `planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.E / Step 3 PR2.

use crate::drill_metrics::{
    ChipWeldingRisk, DrillToolpathSummary, chip_welding_threshold, per_peck_max_depth_to_diameter,
};
use crate::drill_op::DrillOp;
use crate::material::Material;
use serde::{Deserialize, Serialize};

/// Outcome of a single drill gate. Mirrors the structure of the milling
/// verdicts (`ChiploadVerdict`, etc.) but with drill-relevant payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DrillGateOutcome {
    Within {
        /// The measurement being gated (units depend on the gate — see
        /// [`DrillGatesVerdict`]).
        observed: f64,
        /// Material-aware threshold the observation was compared against.
        threshold: f64,
    },
    Exceeds {
        observed: f64,
        threshold: f64,
        /// Coarse severity flag for UI styling. `Elevated` is a warning;
        /// `Critical` is a hard exceedance the operator should act on.
        severity: DrillGateSeverity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrillGateSeverity {
    Elevated,
    Critical,
}

/// Per-toolpath drill-gate trio. Carried as
/// `Option<DrillGatesVerdict>` on [`ToolpathLoadVerdict`]; `None` for
/// non-drill toolpaths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillGatesVerdict {
    /// Chip welding gate: `max_depth_to_diameter` vs material threshold.
    /// `Exceeds(Elevated)` when D/d is between 0.75× and 1.0× the
    /// threshold; `Exceeds(Critical)` when over.
    pub chip_welding: DrillGateOutcome,
    /// Peck adequacy gate: deepest single peck's D/d vs the per-peck
    /// safe ratio for the material. Always `Within` for `Peck` / `ChipBreak`
    /// cycles whose peck depth keeps single-peck D/d below the threshold;
    /// `Exceeds(Critical)` otherwise. Distinct from chip welding —
    /// pecking that breaks chips between pecks still fails if any single
    /// peck is too deep.
    pub peck_adequacy: DrillGateOutcome,
    /// Plunge feed sanity gate: `feed_rate / diameter` vs a
    /// material-aware envelope. Catches "feed too slow" (rubbing /
    /// burning) and "feed too fast" (cutter breakage) without needing
    /// vendor-LUT data.
    pub plunge_feed: DrillGateOutcome,
}

/// Material-aware plunge feed-per-diameter envelope (1/min). Above the
/// max: cutter breakage / stall risk. Below the min: rubbing / burning.
pub fn plunge_feed_envelope(material: &Material) -> (f64, f64) {
    match material {
        Material::SolidWood { .. } => (50.0, 400.0),
        Material::Plywood { .. } | Material::SheetGood { .. } => (40.0, 350.0),
        Material::Plastic { .. } => (60.0, 500.0),
        Material::Foam { .. } => (100.0, 1000.0),
        Material::Custom { .. } => (40.0, 500.0),
    }
}

/// Evaluate the three drill gates for a single drilling toolpath.
pub fn evaluate(drill_op: &DrillOp, summary: &DrillToolpathSummary) -> DrillGatesVerdict {
    DrillGatesVerdict {
        chip_welding: evaluate_chip_welding(summary, &drill_op.material),
        peck_adequacy: evaluate_peck_adequacy(drill_op, summary),
        plunge_feed: evaluate_plunge_feed(drill_op),
    }
}

fn evaluate_chip_welding(summary: &DrillToolpathSummary, material: &Material) -> DrillGateOutcome {
    let threshold = chip_welding_threshold(material);
    let observed = summary.max_depth_to_diameter;
    match summary.chip_welding_risk {
        ChipWeldingRisk::Low => DrillGateOutcome::Within {
            observed,
            threshold,
        },
        ChipWeldingRisk::Elevated => DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity: DrillGateSeverity::Elevated,
        },
        ChipWeldingRisk::High => DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity: DrillGateSeverity::Critical,
        },
    }
}

fn evaluate_peck_adequacy(drill_op: &DrillOp, summary: &DrillToolpathSummary) -> DrillGateOutcome {
    let threshold = per_peck_max_depth_to_diameter(&drill_op.material);
    let diameter = drill_op.tool_diameter_mm.max(f64::MIN_POSITIVE);
    // Reconstruct the worst single-peck D/d from cycle + total depth. For
    // Simple / Dwell that's the whole hole; for Peck / ChipBreak it's the
    // nominal peck depth (or the clamped final peck if smaller).
    let peak_peck_dtd = match drill_op.cycle {
        crate::drill::DrillCycle::Simple | crate::drill::DrillCycle::Dwell(_) => {
            summary.deepest_hole_mm / diameter
        }
        crate::drill::DrillCycle::Peck(peck) | crate::drill::DrillCycle::ChipBreak(peck, _) => {
            // Nominal peck depth wins unless every hole is shallower than peck —
            // in which case the deepest single peck equals the full hole.
            peck.min(summary.deepest_hole_mm) / diameter
        }
    };
    if peak_peck_dtd <= threshold {
        DrillGateOutcome::Within {
            observed: peak_peck_dtd,
            threshold,
        }
    } else {
        DrillGateOutcome::Exceeds {
            observed: peak_peck_dtd,
            threshold,
            severity: DrillGateSeverity::Critical,
        }
    }
}

fn evaluate_plunge_feed(drill_op: &DrillOp) -> DrillGateOutcome {
    let diameter = drill_op.tool_diameter_mm.max(f64::MIN_POSITIVE);
    let observed = drill_op.feed_rate_mm_min / diameter;
    let (lo, hi) = plunge_feed_envelope(&drill_op.material);
    if observed < lo {
        DrillGateOutcome::Exceeds {
            observed,
            threshold: lo,
            severity: DrillGateSeverity::Elevated,
        }
    } else if observed > hi {
        DrillGateOutcome::Exceeds {
            observed,
            threshold: hi,
            severity: DrillGateSeverity::Critical,
        }
    } else {
        // Report the closer envelope bound as the "threshold" so the UI
        // can show a single useful headroom number.
        let nearer = if (observed - lo).abs() < (hi - observed).abs() {
            lo
        } else {
            hi
        };
        DrillGateOutcome::Within {
            observed,
            threshold: nearer,
        }
    }
}

impl DrillGateOutcome {
    pub fn is_exceeded(&self) -> bool {
        matches!(self, DrillGateOutcome::Exceeds { .. })
    }

    pub fn observed(&self) -> f64 {
        match self {
            DrillGateOutcome::Within { observed, .. }
            | DrillGateOutcome::Exceeds { observed, .. } => *observed,
        }
    }
}

impl DrillGatesVerdict {
    pub fn any_exceeded(&self) -> bool {
        self.chip_welding.is_exceeded()
            || self.peck_adequacy.is_exceeded()
            || self.plunge_feed.is_exceeded()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::drill::DrillCycle;
    use crate::drill_metrics::{build_drill_toolpath_summary, emit_drill_samples};
    use crate::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    fn op(cycle: DrillCycle, diameter: f64, depth: f64, feed: f64) -> DrillOp {
        DrillOp {
            holes: vec![DrillHole {
                xy: [0.0, 0.0],
                top_z: 0.0,
                bottom_z: -depth,
            }],
            hole_source: HoleSource::ModelDerived,
            tool_profile: ToolProfile::StandardTwist,
            tool_diameter_mm: diameter,
            cycle,
            feed_rate_mm_min: feed,
            spindle_rpm: 18_000,
            flute_count: 2,
            material: Material::default(),
        }
    }

    fn evaluate_one(d: &DrillOp) -> DrillGatesVerdict {
        let samples = emit_drill_samples(0, d);
        let summary = build_drill_toolpath_summary(0, d, &samples);
        evaluate(d, &summary)
    }

    #[test]
    fn benign_drill_passes_all_three_gates() {
        // Ø6 hole, 18 mm deep softwood with Peck(2) and feed 300 mm/min.
        // D/d=3 (well below softwood threshold 8). Peck/diameter=0.33 (well below 2).
        // Feed/diameter=50 (at the low edge of envelope 50..400 — within).
        let v = evaluate_one(&op(DrillCycle::Peck(2.0), 6.0, 18.0, 300.0));
        assert!(!v.chip_welding.is_exceeded());
        assert!(!v.peck_adequacy.is_exceeded());
        assert!(!v.plunge_feed.is_exceeded());
    }

    #[test]
    fn deep_simple_drill_flags_chip_welding_critical() {
        // Ø4 hole, 40 mm deep softwood, Simple → D/d=10 vs threshold 8 → Critical.
        let v = evaluate_one(&op(DrillCycle::Simple, 4.0, 40.0, 300.0));
        match v.chip_welding {
            DrillGateOutcome::Exceeds { severity, .. } => {
                assert_eq!(severity, DrillGateSeverity::Critical);
            }
            other => panic!("expected critical chip-welding exceed, got {other:?}"),
        }
    }

    #[test]
    fn shallow_simple_drill_with_borderline_dtd_flags_elevated() {
        // Ø4 hole, 30 mm deep softwood, Simple → D/d=7.5 vs threshold 8.
        // 7.5 / 8 = 0.9375 → Elevated band [0.75, 1.0).
        let v = evaluate_one(&op(DrillCycle::Simple, 4.0, 30.0, 300.0));
        match v.chip_welding {
            DrillGateOutcome::Exceeds { severity, .. } => {
                assert_eq!(severity, DrillGateSeverity::Elevated);
            }
            other => panic!("expected elevated chip-welding exceed, got {other:?}"),
        }
    }

    #[test]
    fn oversize_peck_fails_peck_adequacy() {
        // Ø3 hole, 10 mm deep softwood, Peck(8): peck/diameter = 2.67
        // vs softwood threshold 2.0 → Critical.
        let v = evaluate_one(&op(DrillCycle::Peck(8.0), 3.0, 10.0, 300.0));
        assert!(v.peck_adequacy.is_exceeded());
    }

    #[test]
    fn too_slow_feed_flags_plunge_feed_elevated() {
        // Ø6, feed 100 mm/min → feed/diameter = 16.7, lo=50 → below.
        let v = evaluate_one(&op(DrillCycle::Peck(2.0), 6.0, 12.0, 100.0));
        match v.plunge_feed {
            DrillGateOutcome::Exceeds { severity, .. } => {
                assert_eq!(severity, DrillGateSeverity::Elevated);
            }
            other => panic!("expected elevated plunge-feed exceed, got {other:?}"),
        }
    }

    #[test]
    fn too_fast_feed_flags_plunge_feed_critical() {
        // Ø3, feed 1500 mm/min → feed/diameter = 500, hi=400 → above.
        let v = evaluate_one(&op(DrillCycle::Peck(1.0), 3.0, 6.0, 1500.0));
        match v.plunge_feed {
            DrillGateOutcome::Exceeds { severity, .. } => {
                assert_eq!(severity, DrillGateSeverity::Critical);
            }
            other => panic!("expected critical plunge-feed exceed, got {other:?}"),
        }
    }
}
