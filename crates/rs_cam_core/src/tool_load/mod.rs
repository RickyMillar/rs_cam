//! Tool-load monitor: independent guardrails on the cutting envelope.
//!
//! Three criteria, each with its own typed `Verdict` (all fully
//! implemented):
//! - `chipload` — per-sample chipload-per-tooth vs vendor-LUT min/max
//! - `power` — per-sample spindle power vs available power × safety factor
//! - `deflection` — per-sample tip deflection from cutting force
//!   (`Kc · DOC · WOC` through a stepped-cantilever model) vs the
//!   50 µm / 200 µm bounds
//!
//! There is no aggregate scalar "load %". A scalar would conflate inputs
//! that are individually honest with inputs that are systematically
//! biased (the cylinder-side radial-WOC engagement fraction). Each
//! criterion is reported independently; UI and MCP render them
//! independently.

pub mod chipload;
pub mod deflection;
pub mod drill_gates;
pub mod locality;
pub mod optimize;
pub mod plunge_stress;
pub mod power;
pub mod verdict;

use crate::ids::ToolpathId;
use crate::simulation_cut::SimulationCutSample;

/// F-035 — Single source of truth for "what feed should this sample
/// be evaluated against?"
///
/// Gates (chipload, power, deflection) historically read
/// `sample.feed_rate_mm_min` — the *commanded* feed from the
/// toolpath IR. On hobby-class machines the controller's planner
/// decelerates through corners and the cutter often never reaches
/// the commanded feed in tight geometry. The chipload gate at
/// commanded feed reads `Within`; the chipload at the achieved feed
/// reads `Exceeds_LOW` (rubbing). F-034 introduced
/// `MachineKinematics` and a per-move predicted-feed integrator;
/// F-035 plumbs that prediction through to the gates.
///
/// When the trace carries a populated
/// [`crate::simulation_cut::SimulationCutTrace::predicted_feeds`]
/// map (the simulator stamps it when
/// `SimulationOptions::use_predicted_feed_in_gates` is on AND the
/// active `MachineProfile` carries kinematics) **and** the requested
/// `(toolpath_id, move_index)` lookup hits, the predicted feed
/// replaces commanded. Otherwise the sample's own
/// `feed_rate_mm_min` is returned — byte-identical to pre-F-035.
///
/// Every per-sample gate (chipload + power **+ deflection**) MUST call
/// this helper rather than reading `feed_rate_mm_min` directly, so the
/// three gates stay in lockstep. F-024's audit named site-level
/// duplication as a recurring class of bug.
///
/// Deflection joined this set with the unified feed-aware load model
/// (2026-06-20): the tip-displacement force is now affine in feed per
/// tooth (`F = ap·(Ks·fz·sinθ + F_edge)`), so the deflection gate must
/// evaluate at the same effective feed as power/chipload — that is what
/// lets a path the F-039 optimizer feeds down for deflection read `Within`
/// at the gate (the optimizer↔gate consistency the unified model closes).
#[inline]
pub fn effective_feed_for_sample(
    sample: &SimulationCutSample,
    predicted_feeds: &crate::machine_kinematics::PredictedFeedMap,
) -> f64 {
    if predicted_feeds.is_empty() {
        return sample.feed_rate_mm_min;
    }
    predicted_feeds
        .get(&(sample.toolpath_id, sample.move_index))
        .copied()
        .unwrap_or(sample.feed_rate_mm_min)
}

use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::material::Material;
use crate::tool::ToolDefinition;
use serde::{Deserialize, Serialize};

pub use verdict::{
    BindingConstraint, ChiploadVerdict, Confidence, DeflectionVerdict, ModulationStrategyTag,
    ModulationSummary, PowerVerdict, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason,
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
                "predicted tip deflection exceeds the 200 µm limit even at the lightest reachable cut — feed/RPM/DOC/stepover can't fix this; shorten the stickout or use a stiffer tool"
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
///
/// # ⚠ Unit caveat for the viewport heatmap — reported 2026-08-06, NOT fixed
///
/// The band this returns is a linear **advance per tooth**
/// (`CHIPLOAD_LITERATURE_VERDICT.md` §2, verified per source family).
/// Its one GUI consumer colours segments by
/// `max(effective_chip_thickness_mm)` per move
/// (`rs_cam_viz::app::gpu_upload::build_chipload_per_move` →
/// `render::toolpath_render::chipload_segment_color`), which is an
/// arc-mean **chip thickness**. Those are different quantities, and the
/// heatmap therefore paints "rubbing risk" blue over cuts that are not
/// rubbing — the same defect the post-sim chipload gate carried until
/// 2026-08-06 and the same direction (the chip reads low against an
/// advance band, by `1/mean_chip_factor(arc)`, which is 1.6× at a full
/// slot and larger at every narrower engagement).
///
/// Deliberately not fixed here. The band is correct; the consumer picks
/// the wrong per-move quantity, the fix is one line in a crate this wave
/// does not own, and it is a *visible* change to an operator-facing
/// surface that should ship with a screenshot rather than inside a
/// core-side unit conversion. Owner: the viz/MCP lane. Re-open
/// condition: none needed — it is named here and in the wave's log
/// entry. The honest per-move quantity is
/// `effective_feed_for_sample(s) / (rpm · flutes)`, which is what the
/// gate now reports.
pub fn chipload_envelopes_for_session(
    session: &crate::session::ProjectSession,
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
) -> std::collections::HashMap<ToolpathId, std::ops::Range<f64>> {
    use crate::feeds::vendor_normalize::op_family_to_lut;
    use crate::tool::MillingCutter;

    let mut out = std::collections::HashMap::new();
    let material = &session.stock_config().material;
    if matches!(material, Material::Custom { .. }) {
        return out;
    }

    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = session.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))
        else {
            continue;
        };
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        let spec = tc.operation.spec();
        let lut_op_family = op_family_to_lut(spec.feeds_family);
        let lut_pass_role = match spec.feeds_pass_role {
            crate::feeds::PassRole::Roughing => LutPassRole::Roughing,
            crate::feeds::PassRole::SemiFinish => LutPassRole::SemiFinish,
            crate::feeds::PassRole::Finish => LutPassRole::Finish,
        };
        // Peak axial DOC over the same sample population the gate
        // measures: steady-state only. F3.4 — pre-fix this folded over
        // every cutting sample, so a phantom transit reading (the F2
        // class) inflated the engaged-diameter lookup and the viewport
        // colors could disagree with the export verdict.
        let axial_doc = sim_trace
            .map(|t| {
                t.samples
                    .iter()
                    .filter(|s| {
                        s.toolpath_id == tc.id
                            && s.is_cutting
                            && locality::is_steady_state_for_gate(s, None)
                    })
                    .map(|s| s.axial_doc_mm.max(0.0))
                    .fold(0.0_f64, f64::max)
            })
            .unwrap_or(0.0);
        // F3.4 — same canonical resolver as the gate / optimizer.
        let Some(matched) = chipload::matched_chip_envelope(
            &tool_def,
            material,
            tc.operation.op_type(),
            lut_op_family,
            lut_pass_role,
            tool_def.lookup_diameter_at(axial_doc),
        ) else {
            continue;
        };
        // Keep envelope rows where both bounds exist and are sane,
        // DOC-derated exactly like the gate's trip bounds so the
        // operator-facing colors agree with the export verdict. Both
        // bounds are required here — the `Range<f64>` this function
        // returns can't express a one-sided band — via the shared
        // `geometry::derate_chipload_bounds` helper (S.8, single home
        // for this wrapper across Suggest and both `tool_load` gate
        // sites; see `planning/finishing_stack_review_2026-07.md`).
        let lookup_diameter = tool_def.lookup_diameter_at(axial_doc).max(1e-9);
        let doc_ratio = axial_doc / lookup_diameter;
        if let Some((lo, hi)) = crate::feeds::geometry::derate_chipload_bounds(
            matched.chip_load_min_mm,
            matched.chip_load_max_mm,
            doc_ratio,
            crate::feeds::geometry::ChiploadBoundPolicy::RequireBoth,
        )
        .and_then(crate::feeds::geometry::DeratedChiploadBand::into_pair)
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
    pub toolpath_id: ToolpathId,
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
    /// The toolpath's operation kind. Read by every gate: it drives the
    /// drill-kinematics short-circuit (`NotApplicableForOp`) and the LUT
    /// lookup family routing (`routed_lookup_family` reroutes
    /// `ProjectCurve` and `Adaptive3d` so they match an appropriate
    /// vendor row instead of falling through to `Unmodeled(NoVendorData)`
    /// — Item D of the tool-load fidelity plan).
    pub operation_kind: OperationType,
    /// Structural spans on the (annotated) toolpath. Threaded into the
    /// per-criterion evaluators so `tool_load::locality` can resolve
    /// each sample's `span_path` ancestry — used by the span-aware
    /// locality classifier (G17 D6) and the span-aware steady-state
    /// gate filter (G17 D7). `None` when no annotated toolpath is
    /// available; classifiers degrade to engagement-only labels.
    #[allow(clippy::struct_field_names)]
    pub spans: Option<&'a [crate::toolpath_spans::Span]>,
    /// `DrillOp` payload for drill toolpaths — `Some` for
    /// `OperationType::{Drill, AlignmentPinDrill}`, `None` otherwise.
    /// Drives the `drill_gates` field on the resulting verdict
    /// (chip-welding / peck-adequacy / plunge-feed). Mills supply
    /// `None` and `drill_gates` stays `None` on their verdict — the
    /// existing chipload / power / deflection criteria do all the
    /// load-checking for non-drill ops.
    pub drill_op: Option<&'a crate::drill_op::DrillOp>,
}

/// Evaluation environment shared by the per-criterion gates — the
/// "what evidence, which machine, how strict" half of the metric
/// context, complementing `ToolpathLoadContext`'s per-toolpath identity
/// half (Phase 6 task 1). Bundling the two collapses the gates' former
/// 7–10 positional args to `evaluate(ctx, env)`.
pub struct GateEnv<'a> {
    /// Simulation evidence; `None` routes evidence-driven gates to
    /// `Unmodeled(SimulationRequired)`.
    pub sim_trace: Option<&'a crate::simulation_cut::SimulationCutTrace>,
    /// Only the power gate reads this; `None` routes it to
    /// `Unmodeled(NotImplemented)`.
    pub machine: Option<&'a crate::machine::MachineProfile>,
    /// Gate-trigger widening; `&ToleranceBands::default()` for strict
    /// LUT/machine-ceiling behaviour.
    pub tolerance: &'a ToleranceBands,
}

/// Internal organizational trait (Phase 6 task 3): every milling gate
/// is a `(ctx, env) → typed verdict` function plus a projection into
/// the generic `CriterionStatus`. Lets tests sweep all gates uniformly
/// (e.g. "every gate refuses drill kinematics the same way") without
/// erasing the typed verdicts — `evaluate_toolpath` still names each
/// gate's field explicitly and report construction stays typed.
pub trait MetricEvaluator {
    type Verdict;
    fn evaluate_gate(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> Self::Verdict;
    fn status(verdict: &Self::Verdict) -> verdict::CriterionStatus<'_>;
}

/// `MetricEvaluator` handle for the chipload gate.
pub struct ChiploadGate;
impl MetricEvaluator for ChiploadGate {
    type Verdict = ChiploadVerdict;
    fn evaluate_gate(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> ChiploadVerdict {
        chipload::evaluate(ctx, env)
    }
    fn status(verdict: &ChiploadVerdict) -> verdict::CriterionStatus<'_> {
        verdict.as_criterion_status()
    }
}

/// `MetricEvaluator` handle for the power gate.
pub struct PowerGate;
impl MetricEvaluator for PowerGate {
    type Verdict = PowerVerdict;
    fn evaluate_gate(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> PowerVerdict {
        power::evaluate(ctx, env)
    }
    fn status(verdict: &PowerVerdict) -> verdict::CriterionStatus<'_> {
        verdict.as_criterion_status()
    }
}

/// `MetricEvaluator` handle for the deflection gate.
pub struct DeflectionGate;
impl MetricEvaluator for DeflectionGate {
    type Verdict = verdict::DeflectionVerdict;
    fn evaluate_gate(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
    ) -> verdict::DeflectionVerdict {
        deflection::evaluate(ctx, env)
    }
    fn status(verdict: &verdict::DeflectionVerdict) -> verdict::CriterionStatus<'_> {
        verdict.as_criterion_status()
    }
}

/// Evaluate every guardrail criterion for a single toolpath. All three
/// criteria are independent — caller passes the inputs needed for each
/// and the result carries per-criterion `Verdict`s.
///
/// `tolerance` widens the gate triggers per `ToleranceBands`; pass
/// `&ToleranceBands::default()` for strict LUT/machine-ceiling behaviour.
///
/// This is the **single** `ToolpathLoadVerdict` assembly site (Phase 6
/// task 2) — `gcode::project_load_report` and the optimizer both call
/// it, so per-field population (incl. `modulation_summary`, which the
/// two sites had silently diverged on pre-Phase-6) cannot drift again.
pub fn evaluate_toolpath(
    ctx: &ToolpathLoadContext<'_>,
    sim_trace: Option<&crate::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
    tolerance: &ToleranceBands,
) -> ToolpathLoadVerdict {
    let env = GateEnv {
        sim_trace,
        machine,
        tolerance,
    };
    // §6.E / Step 3 PR2: evaluate drill gates when this toolpath
    // carries a `DrillOp` payload. Mirrors what
    // `gcode::project_load_report` does for the user-facing report so
    // the optimizer and the gcode-export paths agree on drill-side
    // verdicts.
    let drill_gates = ctx.drill_op.map(|drill_op| {
        let samples = crate::drill_metrics::emit_drill_samples(ctx.toolpath_id, drill_op);
        let summary =
            crate::drill_metrics::build_drill_toolpath_summary(ctx.toolpath_id, drill_op, &samples);
        drill_gates::evaluate(drill_op, &summary)
    });
    // T1.1 — the chipload gate hands back its stage-labelled record
    // alongside the verdict. Assembled here, at the single
    // `ToolpathLoadVerdict` assembly site, so every report path (gcode
    // export, optimizer, GUI) sees the same one.
    let (chipload_verdict, feed_explanation) = chipload::evaluate_with_explanation(ctx, &env);
    ToolpathLoadVerdict {
        toolpath_id: ctx.toolpath_id,
        chipload: chipload_verdict,
        power: power::evaluate(ctx, &env),
        deflection: deflection::evaluate(ctx, &env),
        drill_gates,
        // Feed-modulation rollup captured by the simulator for this
        // toolpath, when the trace carries one. Populated here (not at
        // the call sites) so every report path agrees — this field is
        // the one that diverged when gcode re-assembled verdicts inline.
        modulation_summary: sim_trace
            .and_then(|trace| trace.modulation_summaries.get(&ctx.toolpath_id).cloned()),
        feed_explanation,
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::tool_config::ToolMaterial;
    use crate::tool::FlatEndmill;
    use verdict::LoadState;

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.35, 20.0)),
            6.35,
            30.0,
            20.0,
            30.0,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn ctx_for<'a>(
        tool: &'a ToolDefinition,
        material: &'a Material,
        operation_kind: OperationType,
    ) -> ToolpathLoadContext<'a> {
        ToolpathLoadContext {
            toolpath_id: ToolpathId(0),
            tool,
            material,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: 1000.0,
            operation_kind,
            spans: None,
            drill_op: None,
        }
    }

    /// Run one gate generically through `MetricEvaluator` and project
    /// the result into `(state, unmodeled-reason debug)`.
    fn sweep<G: MetricEvaluator>(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
    ) -> (LoadState, Option<String>) {
        let v = G::evaluate_gate(ctx, env);
        let s = G::status(&v);
        (s.state, s.unmodeled_reason.map(|r| format!("{r:?}")))
    }

    /// Phase 6 task 3 sentry — every milling gate refuses uniformly
    /// through the shared `(ctx, env)` surface:
    /// - drill kinematics → `Unmodeled(NotApplicableForOp)` on all gates
    /// - milling op with no sim trace → `Unmodeled(SimulationRequired)`
    ///
    /// If a gate ever stops honouring the shared short-circuits (e.g.
    /// a new gate forgets the F.8 drill check), this catches it at the
    /// trait level rather than in per-gate tests that each pin only
    /// their own module.
    #[test]
    fn metric_evaluators_share_refusal_semantics() {
        let tool = tool();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };
        let machine = crate::machine::MachineProfile::default();
        let tolerance = ToleranceBands::default();
        let env = GateEnv {
            sim_trace: None,
            machine: Some(&machine),
            tolerance: &tolerance,
        };

        let drill_ctx = ctx_for(&tool, &material, OperationType::Drill);
        for (label, (state, reason)) in [
            ("chipload", sweep::<ChiploadGate>(&drill_ctx, &env)),
            ("power", sweep::<PowerGate>(&drill_ctx, &env)),
            ("deflection", sweep::<DeflectionGate>(&drill_ctx, &env)),
        ] {
            assert_eq!(state, LoadState::Unmodeled, "{label}: drill kinematics");
            let reason = reason.unwrap_or_default();
            assert!(
                reason.contains("NotApplicableForOp"),
                "{label}: expected NotApplicableForOp, got {reason}"
            );
        }

        let mill_ctx = ctx_for(&tool, &material, OperationType::Pocket);
        for (label, (state, reason)) in [
            ("chipload", sweep::<ChiploadGate>(&mill_ctx, &env)),
            ("power", sweep::<PowerGate>(&mill_ctx, &env)),
            ("deflection", sweep::<DeflectionGate>(&mill_ctx, &env)),
        ] {
            assert_eq!(state, LoadState::Unmodeled, "{label}: no sim trace");
            let reason = reason.unwrap_or_default();
            assert!(
                reason.contains("SimulationRequired"),
                "{label}: expected SimulationRequired, got {reason}"
            );
        }

        // The power gate is the only machine consumer: no machine →
        // its dedicated `NotImplemented` refusal, evaluated before any
        // other input check (pre-Phase-6 `evaluate_toolpath` parity).
        let no_machine = GateEnv {
            sim_trace: None,
            machine: None,
            tolerance: &tolerance,
        };
        let (state, reason) = sweep::<PowerGate>(&drill_ctx, &no_machine);
        assert_eq!(state, LoadState::Unmodeled);
        assert!(
            reason.unwrap_or_default().contains("NotImplemented"),
            "power without machine must refuse NotImplemented"
        );
    }
}
