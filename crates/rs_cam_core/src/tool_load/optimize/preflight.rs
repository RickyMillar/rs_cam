//! Pre-flight gate — classify the baseline before running any stages
//! to short-circuit cases the search space can't reach. Two refusals:
//!
//!   - Deflection setup-locked: baseline is `Exceeds` on deflection.
//!     The optimizer's feed/RPM/DOC/stepover levers reduce force
//!     linearly with DOC × radial-width, but for tool/material/
//!     stickout-driven failures even the smallest viable DOC won't
//!     bring δ below threshold. The search space can't reach a
//!     Within answer because L/D depends only on the tool config.
//!     Refuse with `DeflectionSetupLocked`.
//!   - Bipolar chipload (steady-state samples straddle both `cl_min`
//!     and `cl_max`) → no single feed/RPM scaling fixes both
//!     extremes. Refuse with `BipolarEngagement`.
//!
//! Both refusals carry an op-aware prescription string from
//! [`super::refusal`]. The shape is "<diagnostic> — <lever>", where
//! the diagnostic explains *what* is wrong and the lever points at a
//! knob the user has access to.

use crate::feeds::vendor_lookup::MatchedRow;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool_load::RefuseReason;
use crate::tool_load::verdict::{DeflectionVerdict, ToolpathLoadVerdict};

use super::context::EvaluationContext;
use super::refusal;

/// Outcome of pre-flight: either a refusal that should short-circuit
/// the optimizer, or `None` to proceed to stages.
pub(crate) struct PreflightRefusal {
    pub reason: RefuseReason,
    pub explanation: String,
}

/// Classify the baseline before running any stages. Returns `Some`
/// when the optimizer should refuse early without burning sims.
pub(crate) fn preflight_classify(
    ctx: &EvaluationContext,
    baseline_trace: &SimulationCutTrace,
    operation_feed_rate_mm_min: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
) -> Option<PreflightRefusal> {
    // 1. Deflection — predicted tip deflection at baseline force. The
    //    search-space levers (feed/RPM/DOC/stepover) reduce force
    //    linearly with DOC × radial-width, but for tool/material/
    //    stickout-driven failures even the smallest viable DOC won't
    //    bring δ below the threshold. We refuse pre-flight; the
    //    prescription points the user at the setup levers (stickout,
    //    tool material) the search space can't reach.
    if let DeflectionVerdict::Exceeds {
        peak_mm: peak_delta_mm,
        ..
    } = baseline_verdict.deflection
    {
        return Some(PreflightRefusal {
            reason: RefuseReason::DeflectionSetupLocked,
            explanation: refusal::deflection_setup_prescription(&ctx.tool, peak_delta_mm),
        });
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
            });
        }
    }

    None
}
