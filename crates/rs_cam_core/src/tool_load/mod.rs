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
//!   bound is the machine force budget, which stands in for an unmeasured
//!   frame.
//! - `cut_force` (P17) — the peak engaged-edge force of the deflection
//!   gate's population against the machine force budget. A report-only
//!   reading, not a criterion.
//!
//! There is no aggregate scalar "load %". A scalar would conflate inputs
//! that are individually honest with inputs that are systematically
//! biased (the cylinder-side radial-WOC engagement fraction). Each
//! criterion is reported independently; UI and MCP render them
//! independently.

pub mod boundary;
pub mod chipload;
mod context;
pub mod cut_force;
pub mod deflection;
pub mod depth;
pub mod display;
pub mod distribution;
pub mod drill_gates;
pub mod force_field;
pub mod groups;
pub mod load_fraction;
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

use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::material::Material;
use crate::tool::ToolDefinition;
use serde::{Deserialize, Serialize};

pub use context::ToolpathLoadContext;
pub use distribution::{
    DistributionMetric, DistributionOutcome, Histogram, MetricDistribution, MetricText,
    NotMeasuredReason, PopulationSample, metric_distribution,
};
pub use metric_guide::{MetricGuide, cut_force_guide, engagement_guide};
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

/// The chipload target of a toolpath (A2, point mode): the printed
/// printed band, or the printed point (P11b2: no depth de-rate).
///
/// A vendor row that prints one value gives a point, not a band
/// ([`crate::feeds::vendor_lookup::PrintedChipload`]). No consumer derives
/// a band from a point.
#[derive(Debug, Clone, PartialEq)]
pub enum ChipTarget {
    /// Two printed limits, as printed (mm/tooth), `start < end`.
    Band(std::ops::Range<f64>),
    /// One printed value, as printed (mm/tooth).
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
        let Some(tool_cfg) = session.toolpath_tool(tc) else {
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

/// The printed chipload target of ONE toolpath (A2, point mode), and
/// the only place the target is derived.
///
/// The matched row's [`crate::feeds::vendor_lookup::LookupResult::printed_chipload`]
/// decides the shape:
///
/// - `Band` gives [`ChipTarget::Band`], both limits as printed.
/// - `Point` gives [`ChipTarget::Point`], the point as printed.
///   No band is derived from it.
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
    // The row key depth: the peak removed layer of the body group, the
    // depth the gate keys its row with (`groups::ToolpathSamples::
    // row_key_depth_mm`). No spans and no cap table reach this door, so the
    // group comes from the intent; the key reads no depth (finding F5).
    let body_facts = operation
        .cuts(&crate::feeds::entry_rule::ConfiguredEntry::registry_default(operation))
        .body_facts();
    let sample_ctx = crate::limits::SampleContext::new(
        &tool_def,
        None,
        crate::limits::CapTable::empty(),
        body_facts,
    );
    let axial_doc = sim_trace
        .map(|t| {
            t.samples
                .iter()
                .filter(|s| {
                    s.toolpath_id == toolpath_id
                        && locality::judged_group(s, &sample_ctx)
                            == Some(crate::limits::CutGroup::Body)
                })
                .map(|s| s.axial_doc_mm.max(0.0))
                .fold(0.0_f64, f64::max)
        })
        .unwrap_or(0.0);
    let matched = chip_envelope_row_for(
        &tool_def,
        material,
        operation,
        lut_op_family,
        lut_pass_role,
        axial_doc,
    )?;
    // P11b2 (L6): the printed band at every depth, as the gate and Suggest
    // read it, through the shared validation (S.8). The peak steady-state
    // depth above keys the row only.
    match matched.printed_chipload() {
        // Two printed limits: both must pass the shared validation.
        crate::feeds::vendor_lookup::PrintedChipload::Band { min_mm, max_mm } => {
            crate::feeds::geometry::validate_chipload_bounds(
                Some(min_mm),
                Some(max_mm),
                crate::feeds::geometry::ChiploadBoundPolicy::RequireBoth,
            )
            .and_then(crate::feeds::geometry::ValidatedChiploadBand::into_pair)
            .map(|(lo, hi)| ChipTarget::Band(lo..hi))
        }
        // One printed value: the printed point, as the gate's point v
        // (`chipload::evaluate_inner`) and Suggest's `chip_point_for_dpp`.
        crate::feeds::vendor_lookup::PrintedChipload::Point { value_mm } => {
            crate::feeds::geometry::validate_chipload_bounds(
                None,
                Some(value_mm),
                crate::feeds::geometry::ChiploadBoundPolicy::AllowHalfBand,
            )
            .map(|band| ChipTarget::Point(band.max_mm_per_tooth))
        }
        crate::feeds::vendor_lookup::PrintedChipload::Unpublished => None,
    }
}

/// The chipload row that the gate judges this operation against, at the
/// axial depth `axial_doc_mm`: the envelope resolver
/// (`chipload::matched_chip_envelope`) at the gate's lookup key. `None` for
/// a custom material, a tool outside its domain, no matched row, or a
/// refused size basis.
///
/// [`chip_target_for_toolpath`] reads its row here, so the modulator, the
/// advisor and a sentry read the row that the gate reads (ruling
/// 2026-10-31: feed, ramp and gate read one chart cell).
pub fn chip_envelope_row(
    material: &Material,
    tool_cfg: &crate::compute::tool_config::ToolConfig,
    operation: &crate::compute::catalog::OperationConfig,
    axial_doc_mm: f64,
) -> Option<crate::feeds::vendor_lookup::LookupResult> {
    if matches!(material, Material::Custom { .. }) {
        return None;
    }
    let tool_def = crate::compute::cutter::build_cutter(tool_cfg).ok()?;
    let spec = operation.spec();
    chip_envelope_row_for(
        &tool_def,
        material,
        operation,
        spec.feeds_family.into(),
        spec.feeds_pass_role.into(),
        axial_doc_mm,
    )
}

/// F3.4 — the canonical resolver of the gate, at the same lookup key
/// (ruling A1: the tip of a tapered ball; ruling B4: the nominal diameter
/// of a V-bit).
fn chip_envelope_row_for(
    tool_def: &crate::tool::ToolDefinition,
    material: &Material,
    operation: &crate::compute::catalog::OperationConfig,
    lut_op_family: LutOperationFamily,
    lut_pass_role: LutPassRole,
    axial_doc_mm: f64,
) -> Option<crate::feeds::vendor_lookup::LookupResult> {
    chipload::matched_chip_envelope(
        tool_def,
        material,
        operation.op_type(),
        lut_op_family,
        lut_pass_role,
        crate::feeds::geometry::lut_key_diameter_for_cutter(tool_def, axial_doc_mm),
    )
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
    /// Trigger flips when `peak_delta_mm > limit * (1.0 + deflection_breach)`,
    /// where the limit is `deflection::exceeds_limit` (P17: the finish
    /// target or the radial stock to leave of a rough pass).
    /// Defaults to 0 — the existing `validated_within → exceeds` band on
    /// `DeflectionBounds` already provides the soft warning zone.
    pub deflection_breach: f64,
}

/// F070: the parts of one operation's [`ToolpathLoadContext`], with the
/// cutter and the cap table OWNED.
///
/// The context borrows its `&ToolDefinition` and its `&CapTable`, and both
/// are built at the call site, so no door can hand back a finished context.
/// This type holds the built parts and the borrowed rest, and [`Self::ctx`]
/// lends the context through its one constructor
/// ([`ToolpathLoadContext::of_operation`]). The parts hold the operation,
/// not facts of it: the facts come from the operation. The export load
/// report and the GUI cut-metric cards build it through one door,
/// `ProjectSession::load_context_parts`; neither assembles the fields by
/// hand.
pub struct LoadContextParts<'s> {
    pub toolpath_id: ToolpathId,
    pub operation: &'s crate::compute::catalog::OperationConfig,
    pub tool: ToolDefinition,
    pub material: &'s Material,
    pub steady_feed: crate::machine::SteadyFeedMmMin,
    pub spans: Option<&'s [crate::trace::toolpath_spans::Span]>,
    pub caps: crate::limits::CapTable,
    pub drill_op: Option<&'s crate::ops::drill_op::DrillOp>,
}

impl LoadContextParts<'_> {
    /// The gate context, borrowed from these parts. The destructure names
    /// every field, so a new part must be passed here.
    #[must_use]
    pub fn ctx(&self) -> ToolpathLoadContext<'_> {
        let Self {
            toolpath_id,
            operation,
            tool,
            material,
            steady_feed,
            spans,
            caps,
            drill_op,
        } = self;
        ToolpathLoadContext::of_operation(
            *toolpath_id,
            operation,
            tool,
            material,
            *steady_feed,
            *spans,
            caps,
            *drill_op,
        )
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

pub use groups::{GroupCell, MetricEvaluator};

/// The NotApplicableForOp verdict of a gate on a group that
/// `limits::scope` does not judge.
fn not_applicable(cell: crate::limits::NotJudged) -> UnmodeledReason {
    UnmodeledReason::NotApplicableForOp(cell.reason.to_owned())
}

/// `MetricEvaluator` handle for the chipload gate.
pub struct ChiploadGate;
impl MetricEvaluator for ChiploadGate {
    const LIMITS: &'static [crate::limits::LimitId] = &[
        crate::limits::LimitId::ChipMax,
        crate::limits::LimitId::ChipMin,
    ];
    type Verdict = ChiploadVerdict;
    type Extra = Option<Box<crate::feeds::FeedExplanation>>;
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: groups::GroupPopulation<'_>,
    ) -> (ChiploadVerdict, Self::Extra) {
        chipload::judge_with_explanation(ctx, env, population)
    }
    fn not_judged(cell: crate::limits::NotJudged) -> ChiploadVerdict {
        ChiploadVerdict::Unmodeled {
            reason: not_applicable(cell),
        }
    }
}

/// `MetricEvaluator` handle for the power gate.
pub struct PowerGate;
impl MetricEvaluator for PowerGate {
    const LIMITS: &'static [crate::limits::LimitId] = &[crate::limits::LimitId::SpindlePower];
    type Verdict = PowerVerdict;
    type Extra = ();
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: groups::GroupPopulation<'_>,
    ) -> (PowerVerdict, ()) {
        (power::judge(ctx, env, population), ())
    }
    fn not_judged(cell: crate::limits::NotJudged) -> PowerVerdict {
        PowerVerdict::Unmodeled {
            reason: not_applicable(cell),
        }
    }
}

/// `MetricEvaluator` handle for the deflection gate.
pub struct DeflectionGate;
impl MetricEvaluator for DeflectionGate {
    const LIMITS: &'static [crate::limits::LimitId] = &[crate::limits::LimitId::ToolDeflection];
    type Verdict = DeflectionVerdict;
    type Extra = ();
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: groups::GroupPopulation<'_>,
    ) -> (DeflectionVerdict, ()) {
        (deflection::judge(ctx, env, population), ())
    }
    fn not_judged(cell: crate::limits::NotJudged) -> DeflectionVerdict {
        DeflectionVerdict::Unmodeled {
            reason: not_applicable(cell),
        }
    }
}

/// `MetricEvaluator` handle for the depth gate (the force budget, S3).
pub struct DepthGate;
impl MetricEvaluator for DepthGate {
    const LIMITS: &'static [crate::limits::LimitId] = &[crate::limits::LimitId::ForceBudget];
    type Verdict = DepthVerdict;
    type Extra = ();
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: groups::GroupPopulation<'_>,
    ) -> (DepthVerdict, ()) {
        (depth::judge(ctx, env, population), ())
    }
    fn not_judged(cell: crate::limits::NotJudged) -> DepthVerdict {
        DepthVerdict::Unmodeled {
            reason: not_applicable(cell),
        }
    }
}

/// `MetricEvaluator` handle for the cut-force reading (P17, report-only;
/// the force budget). `None`: no reading (no trace, no force line, or a
/// group the scope does not judge).
pub struct CutForceGate;
impl MetricEvaluator for CutForceGate {
    const LIMITS: &'static [crate::limits::LimitId] = &[crate::limits::LimitId::ForceBudget];
    type Verdict = Option<cut_force::CutForceReading>;
    type Extra = ();
    fn judge(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        population: groups::GroupPopulation<'_>,
    ) -> (Option<cut_force::CutForceReading>, ()) {
        (cut_force::judge(ctx, env, population), ())
    }
    fn not_judged(_cell: crate::limits::NotJudged) -> Option<cut_force::CutForceReading> {
        None
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
    let populations = trace.material_populations(ctx.toolpath_id(), ctx.material());
    if populations.is_empty() {
        return evaluate_population(ctx, Some(trace), machine, tolerance);
    }
    // The stock population: every sample judged as the stock material.
    let stock_view = trace.population_view(ctx.toolpath_id(), |s| {
        trace.judged_as(s, ctx.material(), ctx.material())
    });
    let mut verdict = evaluate_population(ctx, Some(&stock_view), machine, tolerance);
    let stock_samples = stock_view
        .samples
        .iter()
        .filter(|s| s.toolpath_id == ctx.toolpath_id())
        .count();
    drop(stock_view);
    let mut own = Vec::with_capacity(populations.own.len());
    for (material, samples) in &populations.own {
        let view = trace.population_view(ctx.toolpath_id(), |s| {
            trace.judged_as(s, ctx.material(), material)
        });
        let own_ctx = ctx.with_own_material(material);
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
    // `ProjectSession::build_tool_load_report` does for the user-facing report so
    // every report path agrees on drill-side verdicts.
    let drill_gates = ctx.drill_op().map(|drill_op| {
        let samples = crate::ops::drill_metrics::emit_drill_samples(ctx.toolpath_id(), drill_op);
        let summary = crate::ops::drill_metrics::build_drill_toolpath_summary(
            ctx.toolpath_id(),
            drill_op,
            &samples,
        );
        drill_gates::evaluate(drill_op, &summary)
    });
    // P29-6 6d: one walk puts each removing sample in its group; the one
    // driver runs every gate over every group (`groups::judge_by_group`).
    // The headline field of each gate is the verdict of the headline group;
    // `groups` holds every other group. T1.1: the chipload gate hands back
    // its stage-labelled record beside the headline verdict.
    let populations = groups::populations(ctx, sim_trace);
    let chipload = groups::judge_by_group::<ChiploadGate>(ctx, &env, &populations);
    let power = groups::judge_by_group::<PowerGate>(ctx, &env, &populations);
    let deflection = groups::judge_by_group::<DeflectionGate>(ctx, &env, &populations);
    // S3 — the depth row, at the single assembly site.
    let depth = groups::judge_by_group::<DepthGate>(ctx, &env, &populations);
    // P17: the peak cutting force, report-only.
    let cut_force = groups::judge_by_group::<CutForceGate>(ctx, &env, &populations);
    let feed_explanation = chipload.extra;
    ToolpathLoadVerdict {
        toolpath_id: ctx.toolpath_id(),
        chipload: chipload.headline,
        power: power.headline,
        deflection: deflection.headline,
        depth: depth.headline,
        cut_force: cut_force.headline,
        groups: verdict::GroupVerdicts {
            headline: ctx.headline_group(),
            chipload: chipload.groups,
            power: power.groups,
            deflection: deflection.groups,
            depth: depth.groups,
            cut_force: cut_force.groups,
        },
        drill_gates,
        // Feed-modulation rollup captured by the simulator for this
        // toolpath, when the trace carries one. Populated here (not at
        // the call sites) so every report path agrees — this field is
        // the one that diverged when gcode re-assembled verdicts inline.
        modulation_summary: sim_trace
            .and_then(|trace| trace.modulation_summaries.get(&ctx.toolpath_id()).cloned()),
        feed_explanation,
        // P4b (verifier): wire kinematic_utilization — needs the EMITTED
        // `&Toolpath` (the move list) and the operation's own
        // `plunge_rate_mm_min`. Neither is on `ToolpathLoadContext`, and
        // `GateEnv` carries only the trace, the machine and the tolerance
        // bands, so `analyse_toolpath` cannot be called here.
        //
        // `ProjectSession::build_tool_load_report` DOES hold both (it has the
        // `ProjectSession`), so it fills this slot after the call — see
        // `ProjectSession::kinematic_utilization_for`. A caller that calls
        // `evaluate_toolpath` directly therefore gets `None`. Closing that divergence — the exact
        // shape the Phase 6 doc comment above warns about — means threading
        // `toolpath: Option<&'a Toolpath>` and `plunge_rate_mm_min: f64`
        // through `ToolpathLoadContext`. Since P29-6 the context has one
        // constructor (`ToolpathLoadContext::of_operation`), so the cost is
        // one signature. That is the verifier's call, not a scaffold's.
        kinematic_utilization: None,
        material_split: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
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
        // P29-6: the facts come from the op. A drill op (not a Pocket
        // family with a drill kind) proves the drill short-circuit.
        ToolpathLoadContext::of_operation(
            ToolpathId::from_raw(0),
            &crate::compute::catalog::OperationConfig::new_default(operation_kind),
            tool,
            material,
            crate::machine::SteadyFeedMmMin::op_feed_only(1000.0),
            None,
            crate::limits::CapTable::empty(),
            None,
        )
    }

    /// Run one gate through the one driver and project the headline into
    /// `(state, unmodeled-reason debug)`.
    fn sweep<G: MetricEvaluator>(
        ctx: &ToolpathLoadContext<'_>,
        env: &GateEnv<'_>,
        status: impl Fn(&G::Verdict) -> verdict::CriterionStatus<'_>,
    ) -> (LoadState, Option<String>) {
        let v = groups::headline::<G>(ctx, env);
        let s = status(&v);
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
            (
                "chipload",
                sweep::<ChiploadGate>(&drill_ctx, &env, ChiploadVerdict::as_criterion_status),
            ),
            (
                "power",
                sweep::<PowerGate>(&drill_ctx, &env, PowerVerdict::as_criterion_status),
            ),
            (
                "deflection",
                sweep::<DeflectionGate>(&drill_ctx, &env, DeflectionVerdict::as_criterion_status),
            ),
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
            (
                "chipload",
                sweep::<ChiploadGate>(&mill_ctx, &env, ChiploadVerdict::as_criterion_status),
            ),
            (
                "power",
                sweep::<PowerGate>(&mill_ctx, &env, PowerVerdict::as_criterion_status),
            ),
            (
                "deflection",
                sweep::<DeflectionGate>(&mill_ctx, &env, DeflectionVerdict::as_criterion_status),
            ),
        ] {
            assert_eq!(state, LoadState::Unmodeled, "{label}: no sim trace");
            let reason = reason.unwrap_or_default();
            assert!(
                reason.contains("SimulationRequired"),
                "{label}: expected SimulationRequired, got {reason}"
            );
        }

        // The power gate is the only machine consumer: no machine on a
        // milling op → its dedicated `NotImplemented` refusal, before any
        // other input check. P29-6 6d: on a drill the scope of the drill
        // group answers first (`NotApplicableForOp`), with or without a
        // machine.
        let no_machine = GateEnv {
            sim_trace: None,
            machine: None,
            tolerance: &tolerance,
        };
        let (state, reason) =
            sweep::<PowerGate>(&mill_ctx, &no_machine, PowerVerdict::as_criterion_status);
        assert_eq!(state, LoadState::Unmodeled);
        assert!(
            reason.unwrap_or_default().contains("NotImplemented"),
            "power without machine must refuse NotImplemented"
        );
        let (_, reason) =
            sweep::<PowerGate>(&drill_ctx, &no_machine, PowerVerdict::as_criterion_status);
        assert!(
            reason.unwrap_or_default().contains("NotApplicableForOp"),
            "a drill is not judged by the side gates, with or without a machine"
        );
    }
}
