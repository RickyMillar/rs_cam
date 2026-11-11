//! **The cut force: the peak engaged-edge force of each sample, against the
//! machine force budget.** Feeds contract P17 (change 5).
//!
//! The deflection gate judges `compliance x force` at each sample
//! ([`super::deflection::sample_tip_deflection_mm`], through
//! [`crate::feeds::predict::tip_deflection_from_engagement`]). The force
//! half of that product is a reading of its own: the lateral load the
//! frame carries. This module reads it through the same engagement
//! ([`super::deflection::sample_engagement`]) and the same law
//! ([`crate::feeds::engagement::Engagement::edge_force`], the peak
//! resultant). One law feeds the deflection gate, the force card and the
//! MCP reading; this module adds no constant.
//!
//! ## The line (lead ruling 3, 2026-11-06)
//!
//! The force budget line exists only on an op whose
//! `machine.rigidity.depth_cap_of(kind.body_depth_limit())` is
//! `Some(`[`DepthCapRule`]`)`: the one rule the depth gate
//! (`super::depth`) and the P16 modulator read (every roughing cut, the
//! Adaptive family since P29-4, and a slot or a groove under every role
//! since P29-5). On any other op (a one-sided finish pass, a drill, no
//! machine) the force is a plain reading with no line.
//!
//! ## Report-only
//!
//! [`CutForceReading`] is a field of [`super::ToolpathLoadVerdict`], and
//! it joins neither `criteria()` nor the export gate. The S6 split
//! (`OwnMaterialVerdict`) names gates, and this reading is not a gate, so
//! the split does not carry it: the main verdict reads the stock
//! population, as the force card does.
//!
//! ## The population
//!
//! [`cut_points`] walks the trace with the deflection gate's own filters:
//! this toolpath, `is_cutting`, not [`RadialEngagementClass::Air`], an
//! arc captured, an engagement, and the one steady-state predicate
//! [`super::locality::is_steady_state_for_gate`]. The feed per tooth is
//! [`super::deflection::deflection_feed_per_tooth_mm`], which reads the one
//! P16 feed door (`effective_feed_for_sample`). The force card bins the
//! points whose `force_n` is `Some`.

use serde::{Deserialize, Serialize};

use crate::machine::DepthCapRule;
use crate::material::Material;
use crate::stock::simulation_cut::{RadialEngagementClass, SimulationCutSample};
use crate::tool::ToolDefinition;

use super::locality::SpanLookup;
use super::verdict::{GatePopulation, PopulationUnit};
use super::{GateEnv, ToolpathLoadContext};

/// The rule clause of a cut under the force budget.
pub const RULE_FORCE_BUDGET: &str = "force budget of the machine (the depth limit of this cut)";
/// The rule clause of a cut with no force line: a one-sided finish pass, a
/// drill, or no machine.
pub const RULE_NO_LIMIT: &str = "no force limit on this cut";

/// **The peak engaged-edge force (N) of one sample**: the force behind
/// [`crate::feeds::predict::tip_deflection_from_engagement`], so
/// `deflection = compliance x this force` at every sample.
///
/// `None` when the sample has no engagement
/// ([`super::deflection::sample_engagement`]), the material has no force
/// line, the feed per tooth is not positive (the deflection reader refuses
/// it too, so the two populations match), or the edge sum refuses.
#[must_use]
pub fn sample_peak_force_n(
    tool: &ToolDefinition,
    material: &Material,
    sample: &SimulationCutSample,
    fz_mm: f64,
) -> Option<f64> {
    if fz_mm <= 0.0 {
        return None;
    }
    let line = material.force_line().ok()?;
    // P29-1: the one load, on the deflection gate's engagement.
    crate::feeds::cut_load::CutLoad::of_line(
        &super::deflection::sample_engagement(tool, sample)?,
        crate::feeds::cut_load::OperatingPoint::of_chip(fz_mm, tool.flute_count),
        Some(tool),
        Some(line),
    )
    .force()
    .ok()
    .map(|f| f.peak_resultant_n)
}

/// One steady-state cutting sample on the width x depth plane, with its
/// force.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CutPoint3 {
    /// The engaged width (mm): the engagement's `ae_mm()`.
    pub ae_mm: f64,
    /// The engaged depth (mm): the sample's `axial_engagement_mm`.
    pub ap_mm: f64,
    /// The peak engaged-edge force (N), [`sample_peak_force_n`]. `None`
    /// when the force has no model at this sample.
    pub force_n: Option<f64>,
    /// The sample's `segment_time_s`: its share of cutting time.
    pub weight_s: f64,
    /// The sample's `move_index`, so a surface can seek the playhead.
    pub move_index: usize,
    /// The sample's index into `SimulationCutTrace::samples`.
    pub sample_index: usize,
}

/// **The deflection gate's population, on the width x depth plane.** The
/// filters are the gate's own (module doc). Empty with no trace.
#[must_use]
pub fn cut_points(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> Vec<CutPoint3> {
    let Some(trace) = env.sim_trace else {
        return Vec::new();
    };
    let span_lookup = ctx.spans().map(SpanLookup::new);
    let lookup = span_lookup.as_ref();
    trace
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.toolpath_id == ctx.toolpath_id() && s.is_cutting)
        .filter(|(_, s)| s.radial_engagement_class() != RadialEngagementClass::Air)
        .filter(|(_, s)| s.arc_engagement_radians.is_some())
        .filter_map(|(i, s)| {
            let engagement = super::deflection::sample_engagement(ctx.tool(), s)?;
            // EXEMPT(P28-21, P28-22, P28-23)
            if !super::locality::is_steady_state_for_gate(s, lookup) {
                return None;
            }
            let fz = super::deflection::deflection_feed_per_tooth_mm(s, &trace.predicted_feeds);
            Some(CutPoint3 {
                ae_mm: engagement.ae_mm(),
                ap_mm: s.axial_engagement_mm,
                force_n: sample_peak_force_n(ctx.tool(), ctx.material(), s, fz),
                weight_s: s.segment_time_s,
                move_index: s.move_index,
                sample_index: i,
            })
        })
        .collect()
}

/// **The force budget line (N) of this op, or `None`** (lead ruling 3).
/// `Some(budget_n)` only when the machine has a depth-cap rule
/// ([`DepthCapRule`]) for the op's body cut (every roughing cut; P29-4,
/// ruling R1, gave the Adaptive family the budget; P29-5, ruling R3, gave
/// it to a slot or a groove under every role). `None` for a one-sided
/// finish pass, a drill, or no machine. The depth limit comes from the
/// registry row of the operation kind
/// (`OperationType::body_depth_limit`), as `super::depth` reads it.
#[must_use]
pub fn budget_line(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> Option<f64> {
    let machine = env.machine?;
    let DepthCapRule { budget_n } = machine
        .rigidity
        .depth_cap_of(ctx.operation_kind().body_depth_limit())?;
    Some(budget_n)
}

/// **The cut-force reading of one toolpath**, report-only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutForceReading {
    /// The peak force (N) over the population. `0.0` when the population
    /// is vacuous; read `population` first.
    pub peak_n: f64,
    /// The trace index of the peak sample. `None` when vacuous.
    pub peak_sample_index: Option<usize>,
    /// The samples that reached the force law, over the samples this
    /// toolpath emitted.
    pub population: GatePopulation,
    /// The force budget line (N), [`budget_line`]. `None`: a reading with
    /// no limit.
    pub budget_n: Option<f64>,
    /// One short clause: [`RULE_FORCE_BUDGET`] or [`RULE_NO_LIMIT`].
    pub rule: String,
}

/// **Build the reading.** `None` with no trace, on a drill (no continuous
/// engagement), or when the material has no force line.
#[must_use]
pub fn evaluate(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> Option<CutForceReading> {
    let trace = env.sim_trace?;
    // EXEMPT(P28-17)
    if ctx.operation_kind().is_drill_kinematics() {
        return None;
    }
    ctx.material().force_line().ok()?;
    let offered = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == ctx.toolpath_id())
        .count();
    let points = cut_points(ctx, env);
    let mut contributing = 0_usize;
    let mut peak_n = 0.0_f64;
    let mut peak_sample_index = None;
    for p in &points {
        let Some(force_n) = p.force_n else {
            continue;
        };
        contributing += 1;
        if force_n > peak_n || peak_sample_index.is_none() {
            peak_n = force_n;
            peak_sample_index = Some(p.sample_index);
        }
    }
    let budget_n = budget_line(ctx, env);
    let rule = if budget_n.is_some() {
        RULE_FORCE_BUDGET
    } else {
        RULE_NO_LIMIT
    };
    Some(CutForceReading {
        peak_n,
        peak_sample_index,
        population: GatePopulation::new(contributing, offered, PopulationUnit::Samples),
        budget_n,
        rule: rule.to_owned(),
    })
}
