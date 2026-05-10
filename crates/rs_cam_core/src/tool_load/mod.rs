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
pub mod locality;
pub mod optimize;
pub mod power;
pub mod verdict;

use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::material::Material;
use crate::tool::ToolDefinition;
use serde::{Deserialize, Serialize};

pub use verdict::{
    ChiploadVerdict, Confidence, DeflectionVerdict, PowerVerdict, ToolLoadReport,
    ToolpathLoadVerdict, UnmodeledReason,
};

/// Why the optimizer refused to produce a recommendation. Typed
/// reasons, no free-form fallback — the rollup view composes the
/// user-facing narrative from these via `explanation_for_optimize`.
///
/// Used by `optimize::OptimizeOutcome::{Skipped, NoSafeImprovement}`.
/// The variants are a superset of the old suggest module's refusal
/// kinds plus optimizer-specific ones (`NoImprovementFound`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum RefuseReason {
    /// No simulation trace was available — the optimizer needs the
    /// same per-sample data the gate uses to score candidates.
    SimulationRequired,
    /// The simulation lacks per-sample arc engagement; without it
    /// we can't compute the power-cap or steady-state engagement.
    ArcEngagementNotCaptured,
    /// `Material::Custom` without a validated Kc — the optimizer
    /// refuses rather than scoring against an unknown stiffness.
    MaterialUnvalidated,
    /// No vendor LUT row matches this (tool family, material family,
    /// operation family, pass role) tuple at the toolpath's
    /// diameter.
    NoVendorData,
    /// Steady-state samples are missing — typically a pure-plunge
    /// drill cycle. The optimizer is calibrated for steady-state
    /// cutting, so it refuses rather than tuning a plunge feed.
    SteadyStateSamplesNotPresent,
    /// Some samples ran below the row's chipload-min while others
    /// ran above the row's chipload-max in the same toolpath — no
    /// single feed fixes both. The user should reduce stepover
    /// variation, not change feed.
    BipolarEngagement,
    /// Tool L/D ratio is above the deflection guardrail. Stickout and
    /// diameter are tool-config inputs — feed/RPM/DOC/stepover can't
    /// move them, so the optimizer refuses rather than searching a
    /// space that can't reach a safe answer. The fix is a setup
    /// change (shorten stickout, swap to a stiffer tool).
    DeflectionSetupLocked,
    /// Every compatible LUT row has a chipload range that, even at
    /// the row's nominal RPM, would require a feed below the
    /// machine's minimum or above the machine's maximum feed.
    NoFeasibleRow,
    /// The matched row's RPM bracket has no overlap with the
    /// machine's spindle range. A different cutter would be needed.
    RpmBracketEmpty,
    /// Every must-match LUT row was rejected by the
    /// diameter-extrapolation gate.
    DiameterExtrapolationTooPoor,
    /// Optimizer-specific: every Stage-1/Stage-2 candidate was
    /// either slower than baseline or failed the gate.
    NoImprovementFound,
}

impl RefuseReason {
    /// One-line user-facing explanation for use in the optimizer
    /// rollup or per-toolpath modal. Matches Engineering Default 4
    /// in `planning/OPTIMIZER_UX_PLAN.md`. Generic shape — the
    /// optimizer orchestrator can append a more specific narrative
    /// ("gate-limited at chipload 0.0072") for cases where peak
    /// values are available.
    pub fn explanation_for_optimize(&self) -> &'static str {
        match self {
            Self::SimulationRequired => {
                "no simulation has been run yet — Optimize needs a baseline sim to score against"
            }
            Self::ArcEngagementNotCaptured => {
                "simulation trace lacks per-sample arc engagement — re-run sim with metrics enabled"
            }
            Self::MaterialUnvalidated => {
                "stock material has no validated Kc — Optimize cannot model power against an unknown material"
            }
            Self::NoVendorData => {
                "no vendor LUT row matches this tool, material, and operation — no calibrated chipload envelope to optimise against"
            }
            Self::SteadyStateSamplesNotPresent => {
                "no steady-state cutting samples — typically a drill cycle or all-ramp toolpath, which Optimize cannot tune"
            }
            Self::BipolarEngagement => {
                "stepover varies wildly across the toolpath — no single feed/RPM fixes both extremes; reduce stepover variation"
            }
            Self::DeflectionSetupLocked => {
                "tool stickout / diameter ratio (L/D) exceeds 6 — feed/RPM/DOC/stepover can't fix this; shorten the stickout or use a stiffer tool"
            }
            Self::NoFeasibleRow => {
                "every compatible LUT row falls outside the machine's feed or RPM range"
            }
            Self::RpmBracketEmpty => {
                "the matched LUT row's RPM bracket has no overlap with the machine's spindle range — a different cutter is needed"
            }
            Self::DiameterExtrapolationTooPoor => {
                "no LUT row is calibrated close enough to this tool's diameter to give a trustworthy recommendation"
            }
            Self::NoImprovementFound => {
                "no candidate was both faster than baseline and within the gate's safe envelope"
            }
        }
    }
}

/// Build a `toolpath_id -> chipload_envelope` map for every enabled
/// toolpath in the session. The envelope is the matched vendor LUT
/// row's `(chip_load_min, chip_load_max)` for that toolpath's
/// (tool family, material family, operation family, pass role,
/// diameter) tuple. Toolpaths with no LUT match (custom material,
/// unsupported op family, etc.) are absent from the map; the renderer
/// falls back to grey.
///
/// Used by the simulation viewport's chipload-coloring path and the
/// timeline's per-toolpath envelope readout. Replaces the old
/// `tool_load::suggest::project_suggestions` access pattern — this
/// function does just the LUT match without the full feed/RPM
/// recommendation machinery the optimizer made redundant.
pub fn chipload_envelopes_for_session(
    session: &crate::session::ProjectSession,
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
) -> std::collections::HashMap<usize, std::ops::Range<f64>> {
    use crate::feeds::vendor_lookup::{LookupCriteria, enumerate_matching_rows};
    use crate::feeds::vendor_normalize::material_to_lut;
    use crate::tool::MillingCutter;

    let mut out = std::collections::HashMap::new();
    let material = &session.stock_config().material;
    if matches!(material, Material::Custom { .. }) {
        return out;
    }
    let (material_family, hardness_kind, hardness_value) = material_to_lut(material);

    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = session.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))
        else {
            continue;
        };
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        let geometry_hint = tool_def.to_geometry_hint();
        let tool_family = chipload::tool_family_for(geometry_hint);
        let spec = tc.operation.spec();
        let lut_op_family = match spec.feeds_family {
            crate::feeds::OperationFamily::Adaptive => LutOperationFamily::Adaptive,
            crate::feeds::OperationFamily::Pocket => LutOperationFamily::Pocket,
            crate::feeds::OperationFamily::Contour => LutOperationFamily::Contour,
            crate::feeds::OperationFamily::Parallel => LutOperationFamily::Parallel,
            crate::feeds::OperationFamily::Scallop => LutOperationFamily::Scallop,
            crate::feeds::OperationFamily::Trace => LutOperationFamily::Trace,
            crate::feeds::OperationFamily::Face => LutOperationFamily::Face,
        };
        let lut_pass_role = match spec.feeds_pass_role {
            crate::feeds::PassRole::Roughing => LutPassRole::Roughing,
            crate::feeds::PassRole::SemiFinish => LutPassRole::SemiFinish,
            crate::feeds::PassRole::Finish => LutPassRole::Finish,
        };
        let Some((operation_family, pass_role)) = chipload::routed_lookup_family(
            tc.operation.op_type(),
            tool_family,
            lut_op_family,
            lut_pass_role,
        ) else {
            continue;
        };
        // Use sim_trace's per-sample axial DOC if available — same
        // input the gate's chipload evaluator uses. Falls back to 0.0
        // when no trace.
        let axial_doc = sim_trace
            .map(|t| {
                t.samples
                    .iter()
                    .filter(|s| s.toolpath_id == tc.id && s.is_cutting)
                    .map(|s| s.axial_doc_mm.max(0.0))
                    .fold(0.0_f64, f64::max)
            })
            .unwrap_or(0.0);
        let criteria = LookupCriteria {
            tool_family,
            tool_subfamily: None,
            diameter_mm: tool_def.lookup_diameter_at(axial_doc),
            flute_count: tool_def.flute_count,
            material_family,
            hardness_kind: Some(hardness_kind),
            hardness_value: Some(hardness_value),
            operation_family,
            pass_role,
        };
        let lut = chipload::embedded_lut();
        let Some(matched) = enumerate_matching_rows(lut, &criteria).into_iter().next() else {
            continue;
        };
        // Keep envelope rows where both bounds exist and are sane.
        if let (Some(lo), Some(hi)) = (matched.chip_load_min_mm, matched.chip_load_max_mm)
            && lo > 0.0
            && hi >= lo
        {
            out.insert(tc.id, lo..hi);
        }
    }
    out
}

/// Soft fractional widenings on the chipload + power hard-gate triggers.
/// Default is all-zeros (preserves strict LUT/machine-ceiling behaviour).
/// The optimizer derives non-zero values from
/// `optimize::policy::SearchPolicy::ranking` so single-sample transients
/// (e.g. wanaka TP4 5/2026: one sample 1.05% over `chip_load_max`) don't
/// flip a candidate to `Exceeds`. See `planning/OPTIMIZER_REFACTOR_G16.md`
/// §11.4 "Layer 1".
///
/// Tolerance widens the *trigger condition* only — the underlying
/// `peak_above` / `triggering` metrics on `Within` verdicts continue to
/// record the observed values for downstream display.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ToleranceBands {
    /// Fractional widening of the chipload high-side trigger.
    /// Trigger flips when `cl > max * (1.0 + breakage)`.
    pub breakage: f64,
    /// Fractional narrowing of the chipload low-side trigger.
    /// Trigger flips when `median_cl < min * (1.0 - burn)`.
    pub burn: f64,
    /// Fractional widening of the power-exceeds trigger.
    /// Trigger flips when `peak_power > peak_available * (1.0 + power_breach)`.
    pub power_breach: f64,
    /// Fractional widening of the deflection-exceeds trigger.
    /// Trigger flips when `peak_delta_mm > EXCEEDS_BOUND_MM * (1.0 + deflection_breach)`.
    /// Defaults to 0 — the existing `validated_within → exceeds` band on
    /// `DeflectionBounds` already provides the soft warning zone.
    pub deflection_breach: f64,
}

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
    /// Structural spans on the (annotated) toolpath. Threaded into the
    /// per-criterion evaluators so `tool_load::locality` can resolve
    /// each sample's `span_path` ancestry — used by the span-aware
    /// locality classifier (G17 D6) and the span-aware steady-state
    /// gate filter (G17 D7). `None` when no annotated toolpath is
    /// available; classifiers degrade to engagement-only labels.
    #[allow(clippy::struct_field_names)]
    pub spans: Option<&'a [crate::toolpath_spans::Span]>,
}

/// Evaluate every guardrail criterion for a single toolpath. All three
/// criteria are independent — caller passes the inputs needed for each
/// and the result carries per-criterion `Verdict`s.
///
/// `tolerance` widens the gate triggers per `ToleranceBands`; pass
/// `&ToleranceBands::default()` for strict LUT/machine-ceiling behaviour.
pub fn evaluate_toolpath(
    ctx: &ToolpathLoadContext<'_>,
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
    tolerance: &ToleranceBands,
) -> ToolpathLoadVerdict {
    let power = match machine {
        Some(m) => power::evaluate(
            ctx.toolpath_id,
            ctx.tool,
            ctx.material,
            m,
            sim_trace,
            ctx.spans,
            tolerance,
        ),
        None => PowerVerdict::Unmodeled {
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
            ctx.spans,
            ctx.operation_family,
            ctx.pass_role,
            ctx.operation_feed_rate_mm_min,
            ctx.operation_kind,
            tolerance,
        ),
        power,
        deflection: deflection::evaluate(
            ctx.toolpath_id,
            ctx.tool,
            ctx.material,
            sim_trace,
            ctx.spans,
            tolerance,
        ),
    }
}

/// Evaluate every toolpath in a project and roll up to a `ToolLoadReport`.
pub fn evaluate_project(
    contexts: &[ToolpathLoadContext<'_>],
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
    tolerance: &ToleranceBands,
) -> ToolLoadReport {
    let per_toolpath = contexts
        .iter()
        .map(|ctx| evaluate_toolpath(ctx, sim_trace, machine, tolerance))
        .collect();
    ToolLoadReport { per_toolpath }
}
