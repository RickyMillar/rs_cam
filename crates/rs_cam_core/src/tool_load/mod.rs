//! Tool-load monitor: independent guardrails on the cutting envelope.
//!
//! Three criteria, each with its own `Verdict`:
//! - `chipload` — per-sample chipload-per-tooth vs vendor-LUT min/max
//! - `power` — per-sample spindle power vs available power × safety factor
//! - `deflection` — purely geometric L/D ratio
//!
//! There is no aggregate scalar "load %". A scalar would conflate inputs
//! that are individually honest (geometric L/D) with inputs that are
//! systematically biased (current cylinder-volume engagement) and inputs
//! that don't exist yet (force prediction). Each criterion is reported
//! independently; UI and MCP render them independently.
//!
//! Phase status:
//! - Phase 1a (this commit): `chipload` (per-sample vs vendor LUT) and
//!   `deflection` (L/D only); `power` stubbed `Unmodeled(NotImplemented)`.
//! - Phase 2 → Phase 1b power: arc-engagement metric lands, then `power`.
//!
//! See `/home/ricky/.claude-personal/plans/cheerful-popping-spring.md` for
//! the full plan, including the deferred Phase 6 force model.

pub mod chipload;
pub mod deflection;
pub mod power;
pub mod suggest;
pub mod verdict;

use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::material::Material;
use crate::tool::ToolDefinition;

pub use verdict::{
    Confidence, ExceedsReason, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason, Verdict,
};

/// Per-toolpath inputs passed to `evaluate_toolpath`. Bundling avoids a
/// 7-argument function and keeps call sites stable when later phases add
/// inputs (e.g. `MachineProfile` for the power criterion).
pub struct ToolpathLoadContext<'a> {
    /// Stable simulator-side toolpath id (matches `SimulationCutSample::toolpath_id`).
    pub toolpath_id: usize,
    pub tool: &'a ToolDefinition,
    pub material: &'a Material,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
    /// The toolpath operation's commanded feed rate (mm/min). Used by the
    /// chipload guardrail to filter the sample set down to steady-state
    /// cutting moves — samples whose feed matches the commanded feed
    /// within a small tolerance — and exclude transient entry/plunge/ramp
    /// moves at lower feeds. Item C of the tool-load fidelity plan.
    pub operation_feed_rate_mm_min: f64,
    /// The toolpath's operation kind. Item D of the tool-load fidelity
    /// plan will branch on this in the LUT lookup so project_curve ops
    /// match an appropriate vendor row instead of falling through to
    /// `Unmodeled(NoVendorData)`. Currently unused; populated here so the
    /// `ToolpathLoadContext` and its construction sites are touched once.
    pub operation_kind: OperationType,
}

/// Evaluate every guardrail criterion for a single toolpath. All three
/// criteria are independent — caller passes the inputs needed for each
/// and the result carries per-criterion `Verdict`s.
pub fn evaluate_toolpath(
    ctx: &ToolpathLoadContext<'_>,
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
) -> ToolpathLoadVerdict {
    let power = match machine {
        Some(m) => power::evaluate(ctx.toolpath_id, ctx.tool, ctx.material, m, sim_trace),
        None => Verdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "machine profile not provided to evaluator".to_owned(),
            ),
        },
    };
    ToolpathLoadVerdict {
        toolpath_id: ctx.toolpath_id,
        chipload: chipload::evaluate(
            ctx.toolpath_id,
            ctx.tool,
            ctx.material,
            sim_trace,
            ctx.operation_family,
            ctx.pass_role,
            ctx.operation_feed_rate_mm_min,
            ctx.operation_kind,
        ),
        power,
        deflection: deflection::evaluate(ctx.tool),
    }
}

/// Evaluate every toolpath in a project and roll up to a `ToolLoadReport`.
pub fn evaluate_project(
    contexts: &[ToolpathLoadContext<'_>],
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
) -> ToolLoadReport {
    let per_toolpath = contexts
        .iter()
        .map(|ctx| evaluate_toolpath(ctx, sim_trace, machine))
        .collect();
    ToolLoadReport { per_toolpath }
}
