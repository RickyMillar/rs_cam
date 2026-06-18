//! Pre-flight gate — classify the baseline before running any stages
//! to short-circuit cases the search space can't reach. Two refusals:
//!
//!   - Deflection setup-locked: baseline is `Exceeds` on deflection
//!     AND the closed-form deflection model still exceeds the bound at
//!     the minimum-force corner of the search space (hard-floor DOC ×
//!     hard-floor stepover). F2.3: pre-F2 ANY deflection-Exceeds
//!     baseline refused here, asserting "the levers can't fix this"
//!     without computing it — which contradicted the
//!     `DeflectionDocRetargeter` (whose whole job is driving DOC down
//!     to fix exactly this) and left it dead code. Now the refusal
//!     fires only when even the lightest reachable cut stays over the
//!     bound — tool/material/stickout-driven failures the search
//!     space genuinely can't reach.
//!   - Bipolar chipload (steady-state samples straddle both `cl_min`
//!     and `cl_max`) → no single feed/RPM scaling fixes both
//!     extremes. Refuse with `BipolarEngagement`.
//!
//! Both refusals carry an op-aware prescription string from
//! [`super::refusal`]. The shape is "<diagnostic> — <lever>", where
//! the diagnostic explains *what* is wrong and the lever points at a
//! knob the user has access to.

use crate::compute::catalog::OperationConfig;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::MachineProfile;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::MillingCutter;
use crate::tool_load::RefuseReason;
use crate::tool_load::verdict::{DeflectionVerdict, ToolpathLoadVerdict};

use super::axes::SearchAxis;
use super::context::EvaluationContext;
use super::narrative::DeflectionSetupDetail;
use super::{refusal, search_policy};

/// Outcome of pre-flight: either a refusal that should short-circuit
/// the optimizer, or `None` to proceed to stages.
pub(crate) struct PreflightRefusal {
    pub reason: RefuseReason,
    pub explanation: String,
    /// Structured numbers behind a `DeflectionSetupLocked` refusal
    /// (peak / bound / target stickout), surfaced on
    /// [`super::narrative::OutcomeNarrative::deflection_setup`] so
    /// MCP/GUI render values instead of parsing the prose. `None` for
    /// other refusal kinds.
    pub deflection_setup: Option<DeflectionSetupDetail>,
}

/// Classify the baseline before running any stages. Returns `Some`
/// when the optimizer should refuse early without burning sims.
pub(crate) fn preflight_classify(
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    machine: &MachineProfile,
    baseline_trace: &SimulationCutTrace,
    operation_feed_rate_mm_min: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
) -> Option<PreflightRefusal> {
    // 1. Deflection — predicted tip deflection at baseline force.
    //    F2.3: COMPUTE whether the search space can reach Within
    //    instead of asserting it can't. Force scales with
    //    DOC × radial-width, so the lowest-force corner the search
    //    space can reach is hard-floor DOC × hard-floor stepover; the
    //    closed-form model (same canonical cantilever as the gate's
    //    per-sample evaluation) at that corner decides:
    //    - corner ≤ Exceeds bound → proceed; the per-gate retarget
    //      strategy dispatches `DeflectionDocRetargeter` downstream.
    //    - corner still over the bound (or no force lever exists) →
    //      genuinely setup-locked; refuse with the stickout/tool
    //      prescription.
    if let DeflectionVerdict::Exceeds {
        peak_mm: peak_delta_mm,
        ..
    } = baseline_verdict.deflection
    {
        let corner_mm = deflection_at_min_force_corner(ctx, baseline_op, machine, matched_lut_row);
        let reachable = corner_mm
            .is_some_and(|corner| corner <= crate::tool_load::deflection::EXCEEDS_BOUND_MM);
        if !reachable {
            let prescription = refusal::deflection_setup_prescription(&ctx.tool, peak_delta_mm);
            return Some(PreflightRefusal {
                reason: RefuseReason::DeflectionSetupLocked,
                deflection_setup: Some(DeflectionSetupDetail {
                    peak_um: prescription.peak_um,
                    bound_um: prescription.bound_um,
                    target_stickout_mm: prescription.target_stickout_mm,
                }),
                explanation: prescription.text,
            });
        }
    }

    // 2. Bipolar chipload — needs both LUT bounds to be defined,
    //    otherwise we can't classify against an undefined floor or
    //    ceiling. Many vendor rows publish only `chip_load_max_mm`,
    //    so this gate is opt-in by row coverage.
    if let Some(row) = matched_lut_row
        && let (Some(cl_min), Some(cl_max)) = (row.chip_load_min_mm, row.chip_load_max_mm)
    {
        let steady = crate::tool_load::chipload::steady_state_samples_for_toolpath(
            baseline_trace,
            ctx.toolpath_id,
            operation_feed_rate_mm_min,
        );
        if crate::tool_load::chipload::is_bipolar_engagement(&steady.samples, cl_min, cl_max) {
            return Some(PreflightRefusal {
                reason: RefuseReason::BipolarEngagement,
                explanation: refusal::bipolar_prescription(ctx.operation_kind, ctx.op_family),
                deflection_setup: None,
            });
        }
    }

    None
}

/// Closed-form tip deflection (mm) at the minimum-force corner of the
/// optimizer's search space.
///
/// Corner inputs:
/// - axial DOC: the `DepthPerPass` axis hard floor when the op exposes
///   the axis, else the op's fixed `depth_per_pass()`. `None` when the
///   op has neither — DOC isn't a reachable lever.
/// - radial width: the `Stepover` axis hard floor, else the op's fixed
///   `stepover()`, else the full tool diameter (contour-follow ops
///   can't reduce radial engagement below what geometry dictates —
///   full slot is the honest worst case).
///
/// Returns `None` when no DOC value is derivable or the canonical
/// closed-form model refuses (Custom material, zero stickout) — the
/// caller treats `None` as "cannot be shown reachable" and refuses.
fn deflection_at_min_force_corner(
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    machine: &MachineProfile,
    matched_lut_row: Option<&MatchedRow>,
) -> Option<f64> {
    use crate::compute::catalog::OptimizationSurface;

    let view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { .. } => return None,
    };
    let axis_ctx = super::axes::AxisContext {
        // Matches `run_retarget_strategy`'s fallback (Step 8 should
        // plumb a real project default through `EvaluationContext`).
        project_default_rpm: 18_000,
        machine,
        tool: &ctx.tool,
        material: &ctx.material,
    };
    let space =
        super::space::SearchSpace::build(&view, &axis_ctx, matched_lut_row, search_policy());

    let min_doc_mm = space
        .axis(SearchAxis::DepthPerPass)
        .map(|b| b.hard.lo)
        .or_else(|| baseline_op.depth_per_pass())?;
    let min_woc_mm = space
        .axis(SearchAxis::Stepover)
        .map(|b| b.hard.lo)
        .or_else(|| baseline_op.stepover())
        .unwrap_or_else(|| ctx.tool.radius() * 2.0);

    // Minimum-force corner: lowest feed and highest RPM both minimise feed
    // per tooth (chip thickness), so the feed-aware force is smallest here.
    // If even this corner exceeds the limit, no operating point is safe.
    let min_feed_mm_min = space
        .axis(SearchAxis::FeedRate)
        .map(|b| b.hard.lo)
        .unwrap_or_else(|| baseline_op.feed_rate());
    let max_rpm = space
        .axis(SearchAxis::SpindleRpm)
        .map(|b| b.hard.hi)
        .or_else(|| baseline_op.spindle_rpm().map(|r| r as f64))
        .unwrap_or(axis_ctx.project_default_rpm as f64);
    let flutes = ctx.tool.flute_count.max(1) as f64;
    let fz_min_mm = if max_rpm > 0.0 {
        min_feed_mm_min / (max_rpm * flutes)
    } else {
        0.0
    };
    let immersion_rad = crate::feeds::force::immersion_angle(min_woc_mm, ctx.tool.radius());

    crate::feeds::predict::tip_deflection_from_engagement(
        &ctx.tool,
        &ctx.material,
        min_doc_mm,
        immersion_rad,
        fz_min_mm,
    )
}
