//! Orchestrator that turns project state into a unified
//! [`super::Diagnostic`] list.
//!
//! Two entry points:
//!
//! - [`diagnose_toolpath_inputs`] — pure function. Take the four
//!   borrowed inputs (config, tool, optional sim/load/feeds
//!   evidence) and produce a deduped [`Vec<Diagnostic>`]. Useful for
//!   unit tests and for callers that don't have a `ProjectSession`.
//! - [`ProjectSession::diagnose_toolpath`] / `diagnose_project` —
//!   session helpers that extract the inputs and call the pure
//!   function. Used by MCP and the GUI consumer cutover.

use super::Diagnostic;
use super::adapters::{
    from_feeds, from_preconditions, from_project_diagnostics, from_stale_default,
    from_static_checks, from_tool_load,
};
use super::supersession::apply_supersession;
use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::compute::validate::StaleDefault;
use crate::feeds::FeedsResult;
use crate::tool_load::ToolpathLoadVerdict;

pub use from_preconditions::{
    PreconditionContext, PriorToolpathSummary, TargetModelGeometry, ToolDiameterEntry,
};

/// Inputs the pure orchestrator needs to compute a toolpath's
/// diagnostic list. All fields except the operation/tool are
/// optional — missing inputs cause the relevant gates to surface as
/// `DiagnosticState::NeedsSimulation` rather than to crash.
pub struct ToolpathDiagnoseInputs<'a> {
    pub toolpath_id: usize,
    pub operation: &'a OperationConfig,
    pub tool: &'a ToolConfig,
    pub heights: Option<&'a from_static_checks::ResolvedHeights>,
    /// Feeds-calculator output. When present, calculator warnings and
    /// pre-sim heuristic hints are emitted.
    pub feeds_result: Option<&'a FeedsResult>,
    /// Sim-backed verdict for this toolpath. When present, supersedes
    /// the pre-sim heuristics.
    pub load_verdict: Option<&'a ToolpathLoadVerdict>,
    /// Stale-default defects detected for this toolpath.
    pub stale_defaults: &'a [StaleDefault],
    /// Op-precondition context (prior toolpaths in the same setup,
    /// target-model geometry, tool diameters). `None` for callers that
    /// don't have a session in hand — precondition checks are then
    /// silently skipped.
    pub preconditions: Option<&'a PreconditionContext>,
}

/// Compute the unified diagnostic list for a single toolpath.
///
/// Order of producers:
/// 1. static checks (cheap, always relevant)
/// 2. stale-default defects (per-rule, carries a fix)
/// 3. feeds-calculator warnings (`FeedsResult::warnings`)
/// 4. feeds heuristic hints (`feed_vs_lut`, etc.)
/// 5. tool-load gates (sim-backed)
///
/// Then [`apply_supersession`] drops the heuristic shadows once
/// rigorous evidence is current.
pub fn diagnose_toolpath_inputs(inputs: &ToolpathDiagnoseInputs<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(from_static_checks::diagnostics_from_static_checks(
        inputs.toolpath_id,
        inputs.operation,
        inputs.tool,
        inputs.heights,
    ));
    if let Some(ctx) = inputs.preconditions {
        out.extend(from_preconditions::diagnostics_from_preconditions(
            inputs.toolpath_id,
            inputs.operation,
            inputs.tool.id.0,
            ctx,
        ));
    }
    out.extend(from_stale_default::diagnostics_from_stale_defaults(
        inputs.stale_defaults,
    ));
    if let Some(feeds) = inputs.feeds_result {
        out.extend(from_feeds::diagnostics_from_feeds_result(
            inputs.toolpath_id,
            feeds,
        ));
        out.extend(from_feeds::heuristic_hints_from_recommendation(
            inputs.toolpath_id,
            inputs.operation.feed_rate(),
            inputs.operation.stepover(),
            inputs.operation.depth_per_pass(),
            feeds,
        ));
    }
    if let Some(load) = inputs.load_verdict {
        out.extend(from_tool_load::diagnostics_from_load_verdict(load));
    }
    apply_supersession(out)
}

/// Compute project-wide diagnostics from a
/// [`crate::session::ProjectDiagnostics`] snapshot. No supersession
/// is applied at the project level — the per-toolpath calls are
/// where heuristic-vs-rigorous resolution happens.
pub fn diagnose_project_diagnostics(diag: &crate::session::ProjectDiagnostics) -> Vec<Diagnostic> {
    from_project_diagnostics::diagnostics_from_project(diag)
}
