//! The cross-field invariant pass of a speeds write
//! (`enforce_invariants_at_fixed_geometry`): plunge against feed, the
//! warn-only passes, the feed at the kept geometry (pass 9) and the power
//! re-check (pass 10), with the thresholds the envelope reads.
//!
//! Feeds contract P5 (lead ruling, 2026-11-03): the geometry passes of the
//! chain (0 the axial envelope, 2 and 3 the stepover clamps, 4 the rigidity
//! clamp, 5 the cutting-length clamp, 6 the deflection back-off, 6b the
//! dial) are deleted. No product door ran them after P5; the envelope
//! (`feeds::envelope`) holds their limits.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::PassRole;
use crate::machine::MachineProfile;
use crate::material::{FormulaConstant, Material};

use super::adaptive_entry::{
    check_plunge_entry_stability, pick_adaptive3d_clearing_strategy, recheck_power_after_rescale,
    rescale_feed_to_final_geometry,
};
use super::axial_envelope::recompute_chipload_bounds_for_dpp;
use super::{SuggestContext, SuggestWarning};

/// Deflection ceiling (µm) of the envelope's `Deflection` limit. Until
/// feeds contract P5 the v1.1 DPP back-off (pass 6, deleted) reduced the
/// DPP until the closed-form predictor cleared it.
///
/// Until 2026-08-13 it had a second consumer: the retired pass 8 feed-up
/// recalibration refused a solve whose post-write deflection landed within
/// a 10 µm headroom of this gate. That verify was a documented no-op on the
/// current feed-independent predictor and went with the pass (Checkpoint
/// J-1); the headroom constant was deleted alongside it.
///
/// 200 µm matches the post-sim `tool_load::deflection` critical
/// threshold directly. That is the whole justification, and it stands on
/// its own.
///
/// It used to carry a second one: "the predictor over-shoots post-sim by
/// ~36% (487 µm predicted vs 358 µm observed), so this is already
/// conservative". **That margin no longer exists and the claim is
/// withdrawn.** It was measured on 2026-06-07 (`ba01f7a8`) against the
/// bespoke single-section predictor that `3b0dc487` deleted on
/// 2026-06-17. Since that commit the predictor and the post-sim gate call
/// ONE beam — `ToolDefinition::tip_deflection_mm` — so no beam-level
/// over-shoot is left to lean on.
///
/// A divergence does remain, but it is a different quantity: the
/// predictor forecasts the engagement, and the gate measures it. Nobody
/// has measured that residual. Do not treat this threshold as carrying a
/// hidden safety margin. If a margin is wanted here, state it and source
/// it; do not inherit one from a retired model.
pub(crate) const DEFLECTION_BACKOFF_TARGET_UM: f64 = 200.0;

/// The F201 row of [`DEFLECTION_BACKOFF_TARGET_UM`] (feeds contract P5,
/// card row 27): the basis of the `Deflection` limit of the envelope.
///
/// Feeds contract P11c (review item M1): THE one typed 200 um is
/// [`DEFLECTION_BACKOFF_TARGET_UM`]; this row, the envelope edge, the
/// Suggest target, `cutter_constraints::DEFAULT_ROUGH_DEFLECTION_LIMIT_UM`
/// and the post-sim gate bound `tool_load::deflection::EXCEEDS_BOUND_MM`
/// read it.
pub(crate) const DEFLECTION_BACKOFF_TARGET_ROW: FormulaConstant = FormulaConstant::unsourced(
    "feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM",
    DEFLECTION_BACKOFF_TARGET_UM,
    "the post-sim tool_load::deflection critical threshold; no file in the repo prints it",
);

/// The runtime-sanity target of the envelope's `Runtime` limit (P5, card
/// row 25): the move count above which generation is too slow. Until
/// feeds contract P5 the stepover back-off (pass 3, deleted) raised the
/// stepover until the closed-form move-count predictor
/// [`crate::feeds::predict::predict_move_count`] cleared it.
///
/// 500 000 is the practical "this will hang generation" threshold on
/// the Wanaka motivating case. On a 140×150 mm stock the Wanaka 3D
/// Finish 0.03 mm stepover predicts ~4.6 M moves; clearing 500 k
/// requires raising stepover to roughly 0.3 mm.
pub(crate) const STEPOVER_BACKOFF_TARGET_MOVES_ROW: FormulaConstant = FormulaConstant::unsourced(
    "feeds.suggest.STEPOVER_BACKOFF_TARGET_MOVES",
    500_000.0,
    "the \"this will hang generation\" threshold of the Wanaka case; no record in the repo",
);

/// The ceiling of the `Runtime` limit as a fraction of the tool diameter
/// (P5, card row 25): beyond 50 % stepover a finish pass becomes a
/// roughing pass, so the runtime floor never asks for more.
pub(crate) const STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW: FormulaConstant =
    FormulaConstant::unsourced(
        "feeds.suggest.STEPOVER_BACKOFF_DIAMETER_FRACTION",
        0.5,
        "\"beyond 50 % stepover a finish pass becomes a roughing pass\"; no record in the repo",
    );

// RETIRED 2026-08-13 (Checkpoint J-1): `FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM
// = 10.0` — the refusal headroom under `DEFLECTION_BACKOFF_TARGET_UM` that the
// retired pass 8 feed-up verify measured against. Its only consumer went with
// the pass; the v1.1 DPP back-off uses the bare 200 µm target.

/// The invariant chain of a speeds write: the operation's geometry is a
/// fixed input (G-SPEEDSAPPLY, 2026-10-12). The feed-side passes (1, 9 and
/// 10) and the warn-only passes run, and they read the operation's
/// geometry. Pass 9 compares that geometry against the geometry the
/// calculator's result gives (rounded and snapped as the funnel writes it),
/// so the feed that ships is the calculator's feed for the geometry the
/// operation keeps.
#[expect(
    clippy::too_many_arguments,
    reason = "the chain needs the operation, tool/machine/material, pass_role and context, plus the two calculator geometry values that pass 9 compares against."
)]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "projection: only the named variant(s) carry this (named: SuggestWarning::FeedRescaledToFinalGeometry)"
)]
pub(super) fn enforce_invariants_at_fixed_geometry(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
    calculator_stepover: Option<f64>,
    calculator_dpp: Option<f64>,
) -> Vec<SuggestWarning> {
    // The baseline of pass 9 is the calculator's geometry, because the
    // operation's geometry was never replaced by it.
    let (entry_stepover, entry_dpp) = (calculator_stepover, calculator_dpp);
    let mut working_context = context;
    let mut warnings = Vec::new();
    // When the depth the operation keeps is not the calculator's depth, the
    // band that the pass 9 floor reads is re-derated at the kept depth.
    if let (Some(kept_dpp), Some(calculator_dpp)) = (operation.depth_per_pass(), entry_dpp)
        && kept_dpp.to_bits() != calculator_dpp.to_bits()
    {
        working_context.chipload_bounds = recompute_chipload_bounds_for_dpp(
            working_context.matched_lut_row,
            working_context.effective_diameter_mm,
            operation,
            kept_dpp,
        );
    }
    let context = working_context;
    warnings.extend(clamp_plunge_to_feed(operation));
    // Ruling Q11 (G10, 2026-09-25): no entry-style rewrite. The entry
    // style is the operator's. The stability warning below names a deep
    // plunge entry.
    // v3.3c: clearing-strategy recommendation is warn-only (auto-rewrite
    // deferred to v4 pending classifier calibration) — reads, never writes.
    warnings.extend(pick_adaptive3d_clearing_strategy(
        operation, pass_role, context,
    ));
    warnings.extend(check_plunge_entry_stability(operation, tool, pass_role));
    // Pass 8 — the arc-fit chipload feed recalibration — was RETIRED here on
    // 2026-08-13 (Checkpoint J-1, BINDING). See the retirement note on
    // `feeds::predict` and `planning/review_2026-08-08/ARC_FIT_RATIO_EVIDENCE.md`.
    //
    // Pass 9 reconciles the feed with the geometry the operation keeps.
    let rescale_warnings = rescale_feed_to_final_geometry(
        operation,
        tool,
        machine,
        entry_stepover,
        entry_dpp,
        context,
    );
    // Pass 10 (T-15) re-checks the spindle power ceiling at the geometry the
    // operation ships, and it runs ONLY when pass 9 moved the feed. The gate
    // is the warning pass 9 files when it acts, which also carries the
    // pre-rescale feed pass 10 falls back to.
    let pre_rescale_feed = rescale_warnings.iter().find_map(|w| match w {
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            ..
        } => Some(*requested_mm_per_min),
        _ => None,
    });
    warnings.extend(rescale_warnings);
    if let Some(pre_rescale_feed) = pre_rescale_feed {
        warnings.extend(recheck_power_after_rescale(
            operation,
            tool,
            machine,
            material,
            pre_rescale_feed,
            context,
        ));
    }
    warnings
}

/// Pass 1: clamp plunge_rate to feed_rate when plunge exceeds feed.
pub(super) fn clamp_plunge_to_feed(operation: &mut OperationConfig) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    // S11: an unset value has nothing to clamp.
    let (Some(feed_rate), Some(plunge_rate)) = (operation.feed_rate(), operation.plunge_rate())
    else {
        return warnings;
    };
    if feed_rate.is_finite() && plunge_rate.is_finite() && plunge_rate > feed_rate {
        operation.set_plunge_rate(feed_rate);
        warnings.push(SuggestWarning::PlungeClampedToFeed {
            requested: plunge_rate,
            capped: feed_rate,
        });
    }
    warnings
}

/// The `adaptive_doc_factor x D` cap for a depth of `depth_mm` (the
/// Adaptive family), or `None` when the family's rule is not a factor
/// (P13c: the Pocket and Profile families read the force budget, which the
/// envelope `Rigidity` limit solves per width) or no cap applies. P5: the
/// `Rigidity` limit of the envelope reads this one rule.
pub(crate) fn rigidity_cap_mm(
    family: crate::feeds::OperationFamily,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
    depth_mm: f64,
) -> Option<f64> {
    if !matches!(pass_role, PassRole::Roughing) || !depth_mm.is_finite() {
        return None;
    }
    let crate::machine::DepthCapRule::Factor(factor) =
        machine.rigidity.depth_cap_rule(family, pass_role)?
    else {
        return None;
    };
    let cutter = crate::compute::cutter::build_cutter(tool).ok()?;
    let cap_at = |depth: f64| -> Option<f64> {
        let diameter = crate::feeds::geometry::depth_cap_diameter_mm(&cutter, depth);
        Some(factor * diameter).filter(|cap| cap.is_finite() && *cap > 0.0)
    };
    let cap_at_depth = cap_at(depth_mm)?;
    Some(deepest_depth_within_cap(cap_at_depth, &cap_at))
}

/// The deepest depth at or below `first_cap` that is inside the cap
/// measured at that same depth. Feeds matrix R2 (2026-09-23).
///
/// When the cap diameter does not change with depth (flat, ball, bull,
/// V-bit), the answer is `first_cap` itself, returned unchanged. On a
/// tapered ball the cap grows with depth, so the cap at `first_cap` is
/// smaller than `first_cap`. A bisection then finds the depth `d` where
/// `d <= cap_at(d)`. The result is the low end of the bracket, so it is
/// always inside its own cap, and the gate reads it as `Within`.
fn deepest_depth_within_cap(first_cap: f64, cap_at: &dyn Fn(f64) -> Option<f64>) -> f64 {
    const BISECTION_STEPS: usize = 64;
    let inside = |d: f64| cap_at(d).is_some_and(|cap| d <= cap);
    if inside(first_cap) {
        return first_cap;
    }
    let mut lo = 0.0_f64;
    let mut hi = first_cap;
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

#[cfg(test)]
mod row_tests {
    use super::*;

    #[test]
    fn the_deflection_row_holds_the_target() {
        assert_eq!(
            DEFLECTION_BACKOFF_TARGET_ROW.value,
            DEFLECTION_BACKOFF_TARGET_UM
        );
    }
}
