//! `OptimizationStrategy` — pure candidate-patch generators.
//!
//! Step 6 of G16. Each strategy returns a list of [`CandidatePatch`]
//! values; the orchestrator applies each patch list to the baseline op
//! via [`super::patches::apply_patches_to_op`] and runs the sim
//! separately. Strategies do NOT mutate session state and do NOT
//! evaluate candidates — that's the orchestrator's job.
//!
//! See `planning/OPTIMIZER_REFACTOR_G16.md` §3.5.

use super::axes::AxisView;
use super::patches::AxisPatch;
use crate::tool_load::verdict::ToolpathLoadVerdict;

pub mod headroom;

/// One candidate's patch list with provenance. The strategy's name + a
/// human-readable rationale are surfaced in MCP / GUI output.
#[derive(Debug, Clone)]
pub struct CandidatePatch {
    pub patches: Vec<AxisPatch>,
    pub strategy: &'static str,
    pub rationale: String,
}

/// Pure candidate-patch generator. Each strategy is constructed once
/// per optimization run (per-toolpath in practice) with whatever
/// run-level inputs it needs (machine, LUT row, baseline RPM, …) — the
/// trait method itself takes only the per-call context.
pub trait OptimizationStrategy {
    /// Stable name used in candidate provenance and tracing.
    fn name(&self) -> &'static str;

    /// Generate candidate patches for the given baseline. Pure: no sim,
    /// no session mutation.
    fn candidates(
        &self,
        baseline: &AxisView<'_>,
        baseline_verdict: &ToolpathLoadVerdict,
    ) -> Vec<CandidatePatch>;
}
