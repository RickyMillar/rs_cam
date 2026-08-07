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

pub use crate::drill::DrillCycleKind;
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
        /// **The bound that decided this verdict**, which is not always
        /// the material threshold.
        ///
        /// For two-sided gates (plunge feed) it is the *nearer*
        /// envelope bound; pre-F1 this overload made an at-the-floor
        /// reading (`threshold == observed == envelope_lo`) look like a
        /// healthy headroom number.
        ///
        /// For chip welding it is the **advisory boundary `0.75 × t`**,
        /// not `t` (R-3, 2026-08-04). `Low` is decided at `0.75t`, and
        /// displaying `t` beside a `Within` verdict overstated headroom
        /// by the width of the whole Elevated band: a Ø4 × 23.6 mm
        /// softwood hole read `within (5.90)` against `8.00` — 26 %
        /// implied headroom on a reading with 1.7 % real headroom. The
        /// hard ceiling rides alongside in `envelope_hi`.
        threshold: f64,
        /// The band this verdict sits in. `None` on a side means the
        /// band is unbounded there (chip welding `Low` has no
        /// meaningful floor; the `High` band has no ceiling).
        ///
        /// Carried by every gate since R-3, not just the two-sided
        /// one — a consumer can always ask "what band am I in, and what
        /// is next?" without knowing which gate it is looking at.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        envelope_lo: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        envelope_hi: Option<f64>,
    },
    Exceeds {
        observed: f64,
        /// The bound this outcome is measured against. **Read
        /// `severity` before wording it**: at `Elevated` on a
        /// one-sided-up gate the observation is *below* this bound
        /// (the advisory band is `[0.75t, t)`), and at `Elevated` on
        /// the plunge gate it is below the envelope FLOOR. Only
        /// `Critical` on an upward gate means the bound was crossed
        /// upward. Wording this as "exceeds: 7.33 vs 8.00" was R-1.
        threshold: f64,
        /// Coarse severity flag for UI styling. `Elevated` is a warning;
        /// `Critical` is a hard exceedance the operator should act on.
        severity: DrillGateSeverity,
        /// Full envelope for two-sided gates; `None` for one-sided gates.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        envelope_lo: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        envelope_hi: Option<f64>,
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
    /// Index into `DrillOp::holes` of the hole the two depth-derived
    /// verdicts (chip welding, peck adequacy) are about — both key off
    /// the deepest hole. `None` when no hole has positive depth.
    ///
    /// R-7: carried so a diagnostic can name the offending hole instead
    /// of fabricating a sample range. The plunge-feed verdict is
    /// hole-independent (`feed / diameter`) and is deliberately not
    /// attributed to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst_hole_id: Option<usize>,
    /// The cycle these verdicts are about. Consumed only by remedy
    /// wording (R-4) — no gate reads it.
    #[serde(default = "default_cycle_kind")]
    pub cycle: DrillCycleKind,
}

/// Pre-R-4 wire payloads carry no cycle. `Peck` is the safe default for
/// a *remedy*: it suppresses "switch to a peck cycle" rather than
/// asserting a cycle the payload never named.
fn default_cycle_kind() -> DrillCycleKind {
    DrillCycleKind::Peck
}

/// Thin convenience wrapper around
/// [`Material::drill_plunge_feed_envelope_per_mm`]. Canonical dispatch
/// lives on `Material` to match the per-material accessor pattern;
/// see `chip_welding_threshold` in `drill_metrics.rs` for the same
/// rationale. New code should call the method directly.
pub fn plunge_feed_envelope(material: &Material) -> (f64, f64) {
    material.drill_plunge_feed_envelope_per_mm()
}

/// Evaluate the three drill gates for a single drilling toolpath.
pub fn evaluate(drill_op: &DrillOp, summary: &DrillToolpathSummary) -> DrillGatesVerdict {
    DrillGatesVerdict {
        chip_welding: evaluate_chip_welding(summary, &drill_op.material),
        peck_adequacy: evaluate_peck_adequacy(drill_op, summary),
        plunge_feed: evaluate_plunge_feed(drill_op),
        worst_hole_id: summary.deepest_hole_index,
        cycle: DrillCycleKind::of(drill_op.cycle),
    }
}

fn evaluate_chip_welding(summary: &DrillToolpathSummary, material: &Material) -> DrillGateOutcome {
    let threshold = chip_welding_threshold(material);
    // R-3: the classifier's bands are Low [0, 0.75t), Elevated
    // [0.75t, t), High [t, ∞). Every outcome now carries the band it
    // sits in, so no consumer has to re-derive `0.75 ×` to know what
    // decided the verdict or what comes next.
    let advisory = threshold * 0.75;
    // Evacuation-credited ratio (F1): for peck cycles the deepest single
    // peck governs chip packing, not the total hole — `observed` must be
    // the value the risk was actually classified from.
    let observed = summary.chip_welding_dtd;
    match summary.chip_welding_risk {
        ChipWeldingRisk::Low => DrillGateOutcome::Within {
            observed,
            // The boundary that decided `Low` — NOT the material
            // threshold, which is a band further away (R-3).
            threshold: advisory,
            envelope_lo: None,
            envelope_hi: Some(advisory),
        },
        ChipWeldingRisk::Elevated => DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity: DrillGateSeverity::Elevated,
            envelope_lo: Some(advisory),
            envelope_hi: Some(threshold),
        },
        ChipWeldingRisk::High => DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity: DrillGateSeverity::Critical,
            envelope_lo: Some(threshold),
            envelope_hi: None,
        },
    }
}

fn evaluate_peck_adequacy(drill_op: &DrillOp, summary: &DrillToolpathSummary) -> DrillGateOutcome {
    let threshold = per_peck_max_depth_to_diameter(&drill_op.material);
    // R-6: read the number the summary published rather than
    // re-deriving it here. Pre-fix this gate recomputed the worst
    // single-peck D/d from `cycle` + `deepest_hole_mm` while
    // `build_drill_toolpath_summary` computed the same quantity from
    // the sample stream and stored only a boolean — two
    // implementations of one number, neither visible to a consumer.
    // `drill_metrics::per_peck_max_depth_to_diameter_of` is now the
    // single implementation and this reads its result.
    let peak_peck_dtd = summary.per_peck_max_dtd;
    // One-sided gate: `t` really is the bound that decides both arms,
    // so `threshold` needs no correction here. It carries its band for
    // uniformity (R-3) — this gate has no advisory tier at all, which
    // is itself worth being able to see from the outside.
    if peak_peck_dtd <= threshold {
        DrillGateOutcome::Within {
            observed: peak_peck_dtd,
            threshold,
            envelope_lo: None,
            envelope_hi: Some(threshold),
        }
    } else {
        DrillGateOutcome::Exceeds {
            observed: peak_peck_dtd,
            threshold,
            severity: DrillGateSeverity::Critical,
            envelope_lo: Some(threshold),
            envelope_hi: None,
        }
    }
}

fn evaluate_plunge_feed(drill_op: &DrillOp) -> DrillGateOutcome {
    classify_plunge_feed(
        drill_op.feed_rate_mm_min,
        drill_op.tool_diameter_mm,
        &drill_op.material,
    )
}

/// Classify a plunge feed against the material envelope. Shared by the
/// gate and `narrate` so the comparison has exactly one implementation
/// (F1 — pre-fix, narrate re-derived the band check and could drift).
pub fn classify_plunge_feed(
    feed_rate_mm_min: f64,
    diameter_mm: f64,
    material: &Material,
) -> DrillGateOutcome {
    let diameter = diameter_mm.max(f64::MIN_POSITIVE);
    let observed = feed_rate_mm_min / diameter;
    let (lo, hi) = plunge_feed_envelope(material);
    if observed < lo {
        DrillGateOutcome::Exceeds {
            observed,
            threshold: lo,
            severity: DrillGateSeverity::Elevated,
            envelope_lo: Some(lo),
            envelope_hi: Some(hi),
        }
    } else if observed > hi {
        DrillGateOutcome::Exceeds {
            observed,
            threshold: hi,
            severity: DrillGateSeverity::Critical,
            envelope_lo: Some(lo),
            envelope_hi: Some(hi),
        }
    } else {
        // `threshold` keeps the closer envelope bound for single-number
        // headroom displays; the full band rides alongside so consumers
        // can't mistake an at-the-floor reading for healthy headroom.
        let nearer = if (observed - lo).abs() < (hi - observed).abs() {
            lo
        } else {
            hi
        };
        DrillGateOutcome::Within {
            observed,
            threshold: nearer,
            envelope_lo: Some(lo),
            envelope_hi: Some(hi),
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

    /// Adapt to the generic criterion summary so drill gates join
    /// `ToolpathLoadVerdict::criteria()` — and with it export gating
    /// (F1.7, 2026-06-10; pre-fix a Critical drill exceedance did not
    /// block g-code export while an equivalent milling trip did).
    ///
    /// Severity mapping: `Exceeds(Critical)` → `LoadState::Exceeds`
    /// (gates export); `Exceeds(Elevated)` → `LoadState::Within`
    /// (warning band — surfaced by the drill badges and narrate, not
    /// an export blocker). Drill outcomes are always modeled, so
    /// `Unmodeled` never appears here.
    pub fn as_criterion_status(
        &self,
        kind: crate::tool_load::verdict::CriterionKind,
        cycle: DrillCycleKind,
    ) -> crate::tool_load::verdict::CriterionStatus<'_> {
        use crate::tool_load::verdict::{
            CriterionKind, CriterionStatus, ExceededCriterion, LoadState,
        };
        let (state, exceeded) = match self {
            DrillGateOutcome::Within { .. }
            | DrillGateOutcome::Exceeds {
                severity: DrillGateSeverity::Elevated,
                ..
            } => (LoadState::Within, None),
            DrillGateOutcome::Exceeds {
                severity: DrillGateSeverity::Critical,
                ..
            } => (
                LoadState::Exceeds,
                Some(match kind {
                    CriterionKind::DrillChipWelding => ExceededCriterion::drill_chip_welding(cycle),
                    CriterionKind::DrillPeckAdequacy => {
                        ExceededCriterion::drill_peck_adequacy(cycle)
                    }
                    // The plunge-feed arm — and the fallback for any
                    // milling kind passed in error.
                    _ => ExceededCriterion::drill_plunge_feed(),
                }),
            ),
        };
        CriterionStatus {
            kind,
            state,
            confidence: None,
            unmodeled_reason: None,
            sample_range: None,
            display_peak: Some(self.observed()),
            unit: kind.unit(),
            exceeded,
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
    use crate::ids::ToolpathId;

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
            // R-2: no R-plane air in this fixture — it models the cycle
            // from the material surface, which is what this test's numbers
            // were written against. Production sets
            // `effective_safe_z(cfg.retract_z, stock_top)` (= stock top +
            // 5 mm by default); `drill_evidence_wording_d3.rs` is the
            // sentry that pins the emitter-matching case.
            retract_z_mm: 0.0,
        }
    }

    fn evaluate_one(d: &DrillOp) -> DrillGatesVerdict {
        let samples = emit_drill_samples(ToolpathId(0), d);
        let summary = build_drill_toolpath_summary(ToolpathId(0), d, &samples);
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
        // Ø3 hole softwood Peck. Post-2026-06-03 Janka-banded
        // `drill_per_peck_max_dtd` raised the softwood threshold from
        // 2.0 to 6.0×D; trip the gate with a 22 mm peck (peck/D ≈
        // 7.33 > 6.0).
        let v = evaluate_one(&op(DrillCycle::Peck(22.0), 3.0, 25.0, 300.0));
        assert!(v.peck_adequacy.is_exceeded());
    }

    /// F1 (2026-06-10): pecking credits chip welding. The WANAKA pin
    /// drill — Ø6, 27 mm deep, Peck(2), medium hardwood (threshold 6) —
    /// read total D/d 4.5 = exactly 0.75× the threshold → Elevated,
    /// while its own remedy text said "switch to a peck cycle" and the
    /// cycle WAS Peck. Post-fix the deepest single peck governs for
    /// peck cycles. Test material is the softwood default (threshold
    /// 8), so the contrast hole is 40 mm: Simple reads D/d 6.67 →
    /// Elevated, Peck(2) reads 0.33 → Within.
    #[test]
    fn pecked_deep_hole_credits_evacuation_in_chip_welding() {
        let v = evaluate_one(&op(DrillCycle::Peck(2.0), 6.0, 40.0, 300.0));
        assert!(
            !v.chip_welding.is_exceeded(),
            "pecked hole must credit evacuation, got {:?}",
            v.chip_welding
        );
        // The same hole drilled Simple keeps the total-depth reading.
        let v_simple = evaluate_one(&op(DrillCycle::Simple, 6.0, 40.0, 300.0));
        assert!(
            v_simple.chip_welding.is_exceeded(),
            "Simple cycle at 6.67 D/d (softwood threshold 8) must still warn"
        );
    }

    /// F1: band boundary is half-open — exactly 0.75× the threshold
    /// reads Elevated (`[0.75t, t)`), documented on
    /// `classify_chip_welding`. Softwood threshold 8 → Ø4 × 24 mm = 6.0
    /// = 0.75×8 exactly.
    #[test]
    fn chip_welding_band_edge_at_exactly_three_quarters_is_elevated() {
        let v = evaluate_one(&op(DrillCycle::Simple, 4.0, 24.0, 300.0));
        match v.chip_welding {
            DrillGateOutcome::Exceeds { severity, .. } => {
                assert_eq!(severity, DrillGateSeverity::Elevated);
            }
            other => panic!("expected elevated at exact band edge, got {other:?}"),
        }
    }

    /// F1.7: drill gates participate in the criterion tier — a Critical
    /// drill exceedance reads as `Exceeds` through
    /// `ToolpathLoadVerdict::criteria()`-style consumption, an Elevated
    /// one stays `Within` (warning band, not an export blocker).
    #[test]
    fn criterion_status_maps_critical_to_exceeds_and_elevated_to_within() {
        use crate::tool_load::verdict::{CriterionKind, LoadState};
        // Ø3 @ 1500 mm/min → 500 per mm Ø > 400 hi → Critical.
        let critical = evaluate_one(&op(DrillCycle::Peck(1.0), 3.0, 6.0, 1500.0));
        let status = critical
            .plunge_feed
            .as_criterion_status(CriterionKind::DrillPlungeFeed, DrillCycleKind::Peck);
        assert_eq!(status.state, LoadState::Exceeds);
        assert!(status.exceeded.is_some());

        // Ø6 @ 100 mm/min → 16.7 < 50 lo → Elevated (rubbing — warn only).
        let elevated = evaluate_one(&op(DrillCycle::Peck(2.0), 6.0, 12.0, 100.0));
        let status = elevated
            .plunge_feed
            .as_criterion_status(CriterionKind::DrillPlungeFeed, DrillCycleKind::Peck);
        assert_eq!(status.state, LoadState::Within);
        assert!(status.exceeded.is_none());
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
