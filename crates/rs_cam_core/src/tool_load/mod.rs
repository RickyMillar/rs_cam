//! Tool-load monitor: independent guardrails on the cutting envelope.
//!
//! Three criteria, each with its own typed `Verdict` (all fully
//! implemented):
//! - `chipload` — per-sample chipload-per-tooth vs vendor-LUT min/max
//! - `power` — per-sample spindle power vs available power × safety factor
//! - `deflection` — per-sample tip deflection from cutting force
//!   (`Kc · DOC · WOC` through a stepped-cantilever model) vs the
//!   50 µm / 200 µm bounds
//! - `depth` — per-sample engaged depth vs the machine rigidity cap
//!   (S3, 2026-09-18). It reports and never refuses an export: its
//!   bound is a rule of thumb with no published source.
//!
//! There is no aggregate scalar "load %". A scalar would conflate inputs
//! that are individually honest with inputs that are systematically
//! biased (the cylinder-side radial-WOC engagement fraction). Each
//! criterion is reported independently; UI and MCP render them
//! independently.

pub mod boundary;
pub mod chipload;
pub mod deflection;
pub mod depth;
pub mod display;
pub mod distribution;
pub mod drill_gates;
pub mod locality;
pub mod metric_guide;
pub mod plunge_stress;
pub mod power;
pub mod verdict;

use crate::ids::ToolpathId;
use crate::stock::simulation_cut::SimulationCutSample;

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
/// [`crate::stock::simulation_cut::SimulationCutTrace::predicted_feeds`]
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
/// lets a path the F-039 feed modulation feeds down for deflection read
/// `Within` at the gate (the modulation↔gate consistency the unified model
/// closes).
#[inline]
pub fn effective_feed_for_sample(
    sample: &SimulationCutSample,
    predicted_feeds: &crate::machine::kinematics::PredictedFeedMap,
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

pub use distribution::{
    DistributionMetric, DistributionOutcome, Histogram, MetricDistribution, NotMeasuredReason,
    PopulationSample, metric_distribution,
};
pub use metric_guide::{MetricGuide, engagement_guide};
pub use verdict::{
    BindingConstraint, ChiploadVerdict, Confidence, DeflectionVerdict, DepthVerdict,
    ModulationStrategyTag, ModulationSummary, PowerVerdict, ToolLoadReport, ToolpathLoadVerdict,
    UnmodeledReason,
};

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
/// recommendation machinery.
///
/// # The unit, and who consumes it — F-HEATMAP, fixed 2026-08-08
///
/// The band this returns is a linear **advance per tooth**
/// (`CHIPLOAD_LITERATURE_VERDICT.md` §2, verified per source family).
/// Wrap it in [`crate::feeds::VendorChiploadBand::from_advance_range`]
/// at any display boundary so the unit is carried by the type rather
/// than by this sentence.
///
/// **What was wrong until 2026-08-08.** The viewport heat-map coloured
/// segments by `max(effective_chip_thickness_mm)` per move — an arc-mean
/// **chip thickness** — against this advance band, so it painted
/// "rubbing risk" blue over cuts that were not rubbing (the chip reads
/// low against an advance band by `1/mean_chip_factor(arc)`, which is
/// 1.6× at a full slot and larger at every narrower engagement). Two
/// sim-timeline surfaces carried the identical comparison and a fourth
/// panel blended the two quantities under one label. Checkpoint H
/// (`planning/review_2026-08-08/ORCHESTRATION_LOG.md`, binding) ruled all
/// four onto [`display::achieved_advance_per_tooth`], which is the
/// expression the chipload gate itself observes.
///
/// **This docstring previously claimed the band had "its one GUI
/// consumer". It had four, and the claim is what let a one-surface fix
/// look complete.** Counted at 2026-08-08, after the fix, it has
/// **three** GUI consumers and **two** core-side ones:
///
/// - `rs_cam_viz::app::gpu_upload::build_advance_bands` → the viewport
///   heat-map (`render::toolpath_render::advance_per_tooth_segment_color`)
/// - `rs_cam_viz::ui::sim_timeline` → the `"advance/tooth vs band max"`
///   summary track (normalised by [`Self`]'s ceiling)
/// - `rs_cam_viz::ui::sim_diagnostics` → the `advance/tooth` verdict
///   badge's bound readout
/// - `crate::session::compute` → the strategy advisor's timing model and
///   the adaptive feed-modulation pass, both via
///   `feed_modulation::ChiploadBand`
///
/// The sim-timeline's fourth consumer is **gone on purpose**: the
/// surviving arc-mean chip-thickness track is drawn **unbanded**,
/// because a shaded envelope is a comparison and no source publishes one
/// for that quantity.
///
/// # A printed point (A2, point mode)
///
/// This map holds two-limit bands only. A toolpath whose row prints one
/// value ([`ChipTarget::Point`]) is absent, so the viewport draws it grey
/// (decision Q3). The modulator reads [`modulation_bands_for_session`],
/// which carries the point.
pub fn chipload_envelopes_for_session(
    session: &crate::session::ProjectSession,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
) -> std::collections::HashMap<ToolpathId, std::ops::Range<f64>> {
    chip_targets_for_session(session, sim_trace)
        .into_iter()
        .filter_map(|(id, target)| target.band_range().map(|band| (id, band)))
        .collect()
}

/// The chipload target of a toolpath (A2, point mode): the DOC-derated
/// printed band, or the DOC-derated printed point.
///
/// A vendor row that prints one value gives a point, not a band
/// ([`crate::feeds::vendor_lookup::PrintedChipload`]). No consumer derives
/// a band from a point.
#[derive(Debug, Clone, PartialEq)]
pub enum ChipTarget {
    /// Two printed limits, DOC-derated (mm/tooth), `start < end`.
    Band(std::ops::Range<f64>),
    /// One printed value, DOC-derated (mm/tooth).
    Point(f64),
}

impl ChipTarget {
    /// The modulator's band: [`ChiploadBand::new`] for a band,
    /// [`ChiploadBand::point`] for a point. `None` when the values are
    /// not valid for a `ChiploadBand`.
    ///
    /// [`ChiploadBand::new`]: crate::dressup::feed_modulation::ChiploadBand::new
    /// [`ChiploadBand::point`]: crate::dressup::feed_modulation::ChiploadBand::point
    pub fn modulation_band(&self) -> Option<crate::dressup::feed_modulation::ChiploadBand> {
        use crate::dressup::feed_modulation::ChiploadBand;
        match self {
            Self::Band(range) => ChiploadBand::new(range.start, range.end),
            Self::Point(value) => ChiploadBand::point(*value),
        }
    }

    /// The two-limit band, or `None` for a point.
    pub fn band_range(&self) -> Option<std::ops::Range<f64>> {
        match self {
            Self::Band(range) => Some(range.clone()),
            Self::Point(_) => None,
        }
    }
}

/// The chipload target of every enabled toolpath in the session. A
/// toolpath with no target (see [`chip_target_for_toolpath`]) is absent.
fn chip_targets_for_session(
    session: &crate::session::ProjectSession,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
) -> std::collections::HashMap<ToolpathId, ChipTarget> {
    let mut out = std::collections::HashMap::new();
    let material = &session.stock_config().material;
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = session.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))
        else {
            continue;
        };
        if let Some(target) =
            chip_target_for_toolpath(material, tool_cfg, &tc.operation, tc.id, sim_trace)
        {
            out.insert(tc.id, target);
        }
    }
    out
}

/// The modulator's chipload band for every enabled toolpath in the
/// session (A2, point mode). A band row gives [`ChiploadBand::new`]; a
/// point row gives [`ChiploadBand::point`]. A toolpath with no target is
/// absent, and the modulator runs it bandless.
///
/// The adaptive feed modulation pass in `session/` reads this map, so a
/// point toolpath modulates capped at its point with no floor.
///
/// [`ChiploadBand::new`]: crate::dressup::feed_modulation::ChiploadBand::new
/// [`ChiploadBand::point`]: crate::dressup::feed_modulation::ChiploadBand::point
pub fn modulation_bands_for_session(
    session: &crate::session::ProjectSession,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
) -> std::collections::HashMap<ToolpathId, crate::dressup::feed_modulation::ChiploadBand> {
    chip_targets_for_session(session, sim_trace)
        .into_iter()
        .filter_map(|(id, target)| target.modulation_band().map(|band| (id, band)))
        .collect()
}

/// The DOC-derated chipload target of ONE toolpath (A2, point mode), and
/// the only place the target is derived.
///
/// The matched row's [`crate::feeds::vendor_lookup::LookupResult::printed_chipload`]
/// decides the shape:
///
/// - `Band` gives [`ChipTarget::Band`], both limits DOC-derated.
/// - `Point` gives [`ChipTarget::Point`], the point DOC-derated the same
///   way. No band is derived from it.
/// - `Unpublished` gives `None`.
///
/// `None` also means a custom material, or no matched row for this tool
/// and pass role.
///
/// The band arm is the pre-A2 envelope. The point arm reaches the
/// modulator through [`modulation_bands_for_session`] (the sim pass) and
/// through the strategy advisor's `optimized_candidate`.
pub fn chip_target_for_toolpath(
    material: &Material,
    tool_cfg: &crate::compute::tool_config::ToolConfig,
    operation: &crate::compute::catalog::OperationConfig,
    toolpath_id: ToolpathId,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
) -> Option<ChipTarget> {
    if matches!(material, Material::Custom { .. }) {
        return None;
    }
    // F029: no chip target for a tool outside its domain.
    let tool_def = crate::compute::cutter::build_cutter(tool_cfg).ok()?;
    let spec = operation.spec();
    let lut_op_family: LutOperationFamily = spec.feeds_family.into();
    let lut_pass_role: LutPassRole = spec.feeds_pass_role.into();
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
                    s.toolpath_id == toolpath_id
                        && s.is_cutting
                        && locality::is_steady_state_for_gate(s, None)
                })
                .map(|s| s.axial_doc_mm.max(0.0))
                .fold(0.0_f64, f64::max)
        })
        .unwrap_or(0.0);
    // F3.4 — same canonical resolver as the gate, at the same
    // lookup key (ruling A1: the tip of a tapered ball; ruling B4: the
    // nominal diameter of a V-bit).
    let matched = chipload::matched_chip_envelope(
        &tool_def,
        material,
        operation.op_type(),
        lut_op_family,
        lut_pass_role,
        crate::feeds::geometry::lut_key_diameter_for_cutter(&tool_def, axial_doc),
    )?;
    // DOC-derate exactly like the gate's trip bounds, so the
    // operator-facing colors agree with the export verdict, via the
    // shared `geometry::derate_chipload_bounds` helper (S.8, single home
    // for this wrapper across Suggest and both `tool_load` gate sites;
    // see `planning/finishing_stack_review_2026-07.md`).
    // The de-rate divides by `depth_derate_diameter_for_cutter` at the
    // peak DOC, not by the lookup key: the engaged (cone) diameter on a
    // tapered ball, the nominal diameter on every other shape (a V-bit
    // included since ruling B4 step 2). The depth half of R2 stands for a
    // tapered ball (ruling A1).
    let lookup_diameter =
        crate::feeds::geometry::depth_derate_diameter_for_cutter(&tool_def, axial_doc).max(1e-9);
    let doc_ratio = axial_doc / lookup_diameter;
    match matched.printed_chipload() {
        // Two printed limits: both must pass the shared validation.
        crate::feeds::vendor_lookup::PrintedChipload::Band { min_mm, max_mm } => {
            crate::feeds::geometry::derate_chipload_bounds(
                Some(min_mm),
                Some(max_mm),
                doc_ratio,
                crate::feeds::geometry::ChiploadBoundPolicy::RequireBoth,
            )
            .and_then(crate::feeds::geometry::DeratedChiploadBand::into_pair)
            .map(|(lo, hi)| ChipTarget::Band(lo..hi))
        }
        // One printed value: the same de-rate as the gate's point v'
        // (`chipload::evaluate_inner`) and Suggest's `chip_point_for_dpp`.
        crate::feeds::vendor_lookup::PrintedChipload::Point { value_mm } => {
            crate::feeds::geometry::derate_chipload_bounds(
                None,
                Some(value_mm),
                doc_ratio,
                crate::feeds::geometry::ChiploadBoundPolicy::AllowHalfBand,
            )
            .map(|derated| ChipTarget::Point(derated.max_mm_per_tooth))
        }
        crate::feeds::vendor_lookup::PrintedChipload::Unpublished => None,
    }
}

/// Soft fractional widenings on the chipload + power hard-gate triggers.
/// Default is all-zeros (preserves strict LUT/machine-ceiling behaviour).
/// Every production caller passes the default. The non-zero fields exist
/// for the gate unit tests.
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
    pub spans: Option<&'a [crate::trace::toolpath_spans::Span]>,
    /// `DrillOp` payload for drill toolpaths — `Some` for
    /// `OperationType::{Drill, AlignmentPinDrill}`, `None` otherwise.
    /// Drives the `drill_gates` field on the resulting verdict
    /// (chip-welding / peck-adequacy / plunge-feed). Mills supply
    /// `None` and `drill_gates` stays `None` on their verdict — the
    /// existing chipload / power / deflection criteria do all the
    /// load-checking for non-drill ops.
    pub drill_op: Option<&'a crate::ops::drill_op::DrillOp>,
}

/// F070: the parts of one operation's [`ToolpathLoadContext`], with the
/// cutter OWNED.
///
/// The context borrows its `&ToolDefinition`, and the cutter is built
/// from the tool config at the call site, so no door can hand back a
/// finished context. This type holds the built cutter and the borrowed
/// rest, and [`Self::ctx`] lends the context. The export load report and
/// the GUI cut-metric cards build it through one door,
/// `ProjectSession::load_context_parts`; neither assembles the fields by
/// hand.
pub struct LoadContextParts<'s> {
    pub toolpath_id: ToolpathId,
    pub tool: ToolDefinition,
    pub material: &'s Material,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
    pub operation_feed_rate_mm_min: f64,
    pub operation_kind: OperationType,
    pub spans: Option<&'s [crate::trace::toolpath_spans::Span]>,
    pub drill_op: Option<&'s crate::ops::drill_op::DrillOp>,
}

impl LoadContextParts<'_> {
    /// The gate context, borrowed from these parts. The destructure names
    /// every field, so a new context field must be added here.
    #[must_use]
    pub fn ctx(&self) -> ToolpathLoadContext<'_> {
        let Self {
            toolpath_id,
            tool,
            material,
            operation_family,
            pass_role,
            operation_feed_rate_mm_min,
            operation_kind,
            spans,
            drill_op,
        } = self;
        ToolpathLoadContext {
            toolpath_id: *toolpath_id,
            tool,
            material,
            operation_family: *operation_family,
            pass_role: *pass_role,
            operation_feed_rate_mm_min: *operation_feed_rate_mm_min,
            operation_kind: *operation_kind,
            spans: *spans,
            drill_op: *drill_op,
        }
    }
}

/// Evaluation environment shared by the per-criterion gates — the
/// "what evidence, which machine, how strict" half of the metric
/// context, complementing `ToolpathLoadContext`'s per-toolpath identity
/// half (Phase 6 task 1). Bundling the two collapses the gates' former
/// 7–10 positional args to `evaluate(ctx, env)`.
pub struct GateEnv<'a> {
    /// Simulation evidence; `None` routes evidence-driven gates to
    /// `Unmodeled(SimulationRequired)`.
    pub sim_trace: Option<&'a crate::stock::simulation_cut::SimulationCutTrace>,
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
/// task 2) — every report path calls it, so per-field population (incl. `modulation_summary`, which the
/// two sites had silently diverged on pre-Phase-6) cannot drift again.
///
/// S6 (stock changes, "cut as"): the gates judge each sample as the
/// material that [`SimulationCutTrace::effective_material_for_sample`]
/// gives. When that is the stock material for every sample (always so
/// under the default `CutAs::StockMaterial`), the gates read the trace
/// itself and the verdict is the one a stock-only run gives. Otherwise
/// the samples split into populations: the main verdict judges the stock
/// population, [`ToolpathLoadVerdict::material_split`] holds one verdict
/// per own material (the same gates, with that material in the context),
/// and the not-judged samples are counted per material. No gate holds a
/// material special case.
///
/// [`SimulationCutTrace::effective_material_for_sample`]: crate::stock::simulation_cut::SimulationCutTrace::effective_material_for_sample
pub fn evaluate_toolpath(
    ctx: &ToolpathLoadContext<'_>,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    machine: Option<&crate::machine::MachineProfile>,
    tolerance: &ToleranceBands,
) -> ToolpathLoadVerdict {
    let Some(trace) = sim_trace else {
        return evaluate_population(ctx, None, machine, tolerance);
    };
    let populations = trace.material_populations(ctx.toolpath_id, ctx.material);
    if populations.is_empty() {
        return evaluate_population(ctx, Some(trace), machine, tolerance);
    }
    // The stock population: every sample judged as the stock material.
    let stock_view = trace.population_view(ctx.toolpath_id, |s| {
        trace.judged_as(s, ctx.material, ctx.material)
    });
    let mut verdict = evaluate_population(ctx, Some(&stock_view), machine, tolerance);
    let stock_samples = stock_view
        .samples
        .iter()
        .filter(|s| s.toolpath_id == ctx.toolpath_id)
        .count();
    drop(stock_view);
    let mut own = Vec::with_capacity(populations.own.len());
    for (material, samples) in &populations.own {
        let view = trace.population_view(ctx.toolpath_id, |s| {
            trace.judged_as(s, ctx.material, material)
        });
        let own_ctx = ToolpathLoadContext {
            material,
            spans: ctx.spans,
            drill_op: None,
            ..*ctx
        };
        let v = evaluate_population(&own_ctx, Some(&view), machine, tolerance);
        own.push(OwnMaterialVerdict {
            material: material.label(),
            samples: *samples,
            chipload: v.chipload,
            power: v.power,
            deflection: v.deflection,
            depth: v.depth,
        });
    }
    verdict.material_split = Some(Box::new(MaterialSplit {
        stock_samples,
        own,
        not_judged: populations.not_judged,
    }));
    verdict
}

/// One verdict per own material of a toolpath (S6, `CutAs::OwnMaterial`):
/// the four milling gates, run over the samples judged as that material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnMaterialVerdict {
    /// The material label.
    pub material: String,
    /// The number of samples judged as this material.
    pub samples: usize,
    pub chipload: ChiploadVerdict,
    pub power: PowerVerdict,
    pub deflection: DeflectionVerdict,
    pub depth: DepthVerdict,
}

/// The material populations of a toolpath that is not judged as the stock
/// material alone (S6). The main verdict of the toolpath judges the
/// `stock_samples`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialSplit {
    /// The number of samples judged as the stock material.
    pub stock_samples: usize,
    /// One verdict per own material.
    pub own: Vec<OwnMaterialVerdict>,
    /// The samples not judged, per material.
    pub not_judged: Vec<crate::stock::cut_as::NotJudgedCount>,
}

/// The gates over one population of samples. The body of
/// [`evaluate_toolpath`] before S6.
fn evaluate_population(
    ctx: &ToolpathLoadContext<'_>,
    sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
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
    // every report path agrees on drill-side verdicts.
    let drill_gates = ctx.drill_op.map(|drill_op| {
        let samples = crate::ops::drill_metrics::emit_drill_samples(ctx.toolpath_id, drill_op);
        let summary = crate::ops::drill_metrics::build_drill_toolpath_summary(
            ctx.toolpath_id,
            drill_op,
            &samples,
        );
        drill_gates::evaluate(drill_op, &summary)
    });
    // T1.1 — the chipload gate hands back its stage-labelled record
    // alongside the verdict. Assembled here, at the single
    // `ToolpathLoadVerdict` assembly site, so every report path (gcode
    // export, GUI) sees the same one.
    let (chipload_verdict, feed_explanation) = chipload::evaluate_with_explanation(ctx, &env);
    ToolpathLoadVerdict {
        toolpath_id: ctx.toolpath_id,
        chipload: chipload_verdict,
        power: power::evaluate(ctx, &env),
        deflection: deflection::evaluate(ctx, &env),
        // S3 — the depth row. Assembled HERE, at the single
        // `ToolpathLoadVerdict` assembly site, so the g-code export path
        // and the GUI see one depth verdict.
        depth: depth::evaluate(ctx, &env),
        drill_gates,
        // Feed-modulation rollup captured by the simulator for this
        // toolpath, when the trace carries one. Populated here (not at
        // the call sites) so every report path agrees — this field is
        // the one that diverged when gcode re-assembled verdicts inline.
        modulation_summary: sim_trace
            .and_then(|trace| trace.modulation_summaries.get(&ctx.toolpath_id).cloned()),
        feed_explanation,
        // P4b (verifier): wire kinematic_utilization — needs the EMITTED
        // `&Toolpath` (the move list) and the operation's own
        // `plunge_rate_mm_min`. Neither is on `ToolpathLoadContext`, and
        // `GateEnv` carries only the trace, the machine and the tolerance
        // bands, so `analyse_toolpath` cannot be called here.
        //
        // `gcode::project_load_report` DOES hold both (it has the
        // `ProjectSession`), so it fills this slot after the call — see
        // `ProjectSession::kinematic_utilization_for`. A caller that calls
        // `evaluate_toolpath` directly therefore gets `None`. Closing that divergence — the exact
        // shape the Phase 6 doc comment above warns about — means threading
        // `toolpath: Option<&'a Toolpath>` and `plunge_rate_mm_min: f64`
        // through `ToolpathLoadContext`, which has 22 literal sites across
        // the workspace. That is the verifier's call, not a scaffold's.
        kinematic_utilization: None,
        material_split: None,
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
    use crate::compute::tool_config::ToolMaterial;
    use crate::tool::FlatEndmill;
    use verdict::LoadState;

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.35, 20.0)),
            6.35,
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
