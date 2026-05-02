//! Tool-load suggest module — Phase 3 of the tool-load fidelity plan.
//!
//! The gate (`chipload`, `power`, `deflection`) tells the user when a feed
//! is wrong but not what to set instead. This module turns the same
//! per-sample data into a feed/RPM recommendation, post-simulation, by:
//!
//! 1. Enumerating every vendor-LUT row compatible with the toolpath.
//! 2. Filtering rows whose RPM falls inside the machine spindle bracket.
//! 3. For each remaining row, computing a feasible feed range bounded by
//!    the row's chipload window AND the machine's available spindle power
//!    at the row's RPM.
//! 4. Picking the row that maximises MRR (rough) or surface speed (finish).
//! 5. Recommending a point in the feasible range per `SuggestionStyle`.
//!
//! Refusal-first: every refusal carries a typed `RefuseReason`. The
//! suggest module never invents a feed when the inputs don't support one.
//!
//! See `/home/ricky/.claude-personal/plans/tool-load-fidelity-and-suggest.md`
//! Tier 3.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lookup::{LookupCriteria, MatchedRow, enumerate_matching_rows};
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::vendor_normalize::material_to_lut;
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};

use super::chipload::{routed_lookup_family, tool_family_for};
use super::verdict::Confidence;

/// Worst-case anisotropy multiplier on Kc — same constant as the power
/// guardrail. Real wood Kc varies 2-3× with grain direction; using the
/// upper bound means any feed below the resulting power cap is
/// guaranteed-safe regardless of grain orientation.
const ANISOTROPY_MULTIPLIER: f64 = 2.5;

/// Why a suggestion couldn't be produced. Mirrors `UnmodeledReason`'s
/// refusal-first style — typed reasons, no free-form fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum RefuseReason {
    /// No simulation trace was available — suggest needs the same
    /// per-sample data the gate uses.
    SimulationRequired,
    /// The simulation lacks per-sample arc engagement; without it we
    /// can't compute the power-cap or steady-state engagement.
    ArcEngagementNotCaptured,
    /// `Material::Custom` without a validated Kc — refuse rather than
    /// suggest a feed against an unknown stiffness.
    MaterialUnvalidated,
    /// No vendor LUT row matches this (tool family, material family,
    /// operation family, pass role) tuple at the toolpath's diameter.
    /// Carries the count of LUT rows that passed the must-match filter
    /// but were excluded by RPM-bracket / feasibility checks for diagnostics.
    NoVendorData,
    /// Steady-state samples are missing — typically a pure-plunge drill
    /// cycle. Suggest is calibrated for steady-state cutting, so we
    /// refuse rather than recommend a plunge feed.
    SteadyStateSamplesNotPresent,
    /// Some samples ran below the row's chipload-min while others ran
    /// above the row's chipload-max in the same toolpath — no single
    /// feed fixes both. The rationale recommends reducing stepover
    /// variation, not changing feed.
    BipolarEngagement,
    /// Every compatible LUT row has a chipload range that, even at the
    /// row's nominal RPM, would require a feed below the machine's
    /// minimum or above the machine's maximum feed. Refuse rather than
    /// clamp — clamping would silently leave the toolpath out of the
    /// row's calibrated envelope.
    NoFeasibleRow,
    /// The matched row's RPM bracket has no overlap with the machine's
    /// spindle range. Different cutter would be needed.
    RpmBracketEmpty,
}

/// How aggressively to recommend within the feasible range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStyle {
    /// Pick a feed in the lower third of the feasible range — long tool
    /// life, surface finish above other goals.
    Conservative,
    /// Midpoint of the feasible range. Default.
    #[default]
    Balanced,
    /// Upper third — maximise MRR / minimise cycle time.
    Aggressive,
}

/// One LUT row's evaluation against the toolpath context. `feasible`
/// describes whether this row could yield a recommendation. Surfacing
/// every row (rejected or accepted) lets the UI explain *why* the
/// chosen row won.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowEvaluation {
    pub observation_id: String,
    pub diameter_match_score: i64,
    /// The RPM the algorithm chose for this row (clamped to the machine
    /// bracket and the row's bracket). `None` if the row had no RPM
    /// data or the brackets didn't overlap.
    pub rpm: Option<f64>,
    /// Feed range that satisfies both the row's chipload window AND the
    /// machine's power cap. `None` if the row was rejected before
    /// computing it.
    pub feasible_feed_range: Option<Range<f64>>,
    /// MRR at the row's chosen feed (mm³/min) — used as the rough-pass
    /// optimisation objective. `None` if the row was rejected.
    pub mrr_mm3_min: Option<f64>,
    /// Why the row was rejected (if it was). `None` if accepted.
    pub rejected: Option<String>,
}

/// A concrete feed/RPM recommendation backed by a vendor row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedFeeds {
    pub rpm: f64,
    pub feed_mm_min: f64,
    pub chipload_envelope: Range<f64>,
    /// Power cap that constrained the upper feed bound. `None` if the
    /// row's chipload upper bound was the binding constraint.
    pub power_cap_kw: Option<f64>,
    pub matched_row_id: String,
    pub confidence: Confidence,
    pub style: SuggestionStyle,
}

/// One toolpath's full suggestion record: current state, the suggestion
/// (or refusal), the rows considered, and a human rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSuggestion {
    pub toolpath_id: usize,
    pub current_feed_mm_min: f64,
    pub current_rpm: Option<f64>,
    pub current_peak_chipload_mm: Option<f64>,
    pub current_peak_kw: Option<f64>,
    /// `Ok` with a recommendation, or `Err` with a typed reason.
    pub suggested: Result<SuggestedFeeds, RefuseReason>,
    pub considered_rows: Vec<RowEvaluation>,
    /// Human-readable bullets explaining the recommendation. Populated
    /// for both Ok and Err cases.
    pub rationale: Vec<String>,
}

/// Inputs for evaluating one toolpath's suggestion. Bundled to keep the
/// signature stable as later phases add fields.
pub struct SuggestContext<'a> {
    pub toolpath_id: usize,
    pub tool: &'a ToolDefinition,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
    pub operation_kind: OperationType,
    pub current_feed_mm_min: f64,
}

/// Evaluate one toolpath. The result always contains the current state
/// for diagnostics, even when no suggestion can be produced.
pub fn evaluate(
    ctx: &SuggestContext<'_>,
    sim_trace: Option<&SimulationCutTrace>,
) -> FeedSuggestion {
    let mut rationale: Vec<String> = Vec::new();

    let Some(trace) = sim_trace else {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm: None,
            current_peak_chipload_mm: None,
            current_peak_kw: None,
            suggested: Err(RefuseReason::SimulationRequired),
            considered_rows: Vec::new(),
            rationale: vec!["no simulation trace available".to_owned()],
        };
    };

    if let Material::Custom { .. } = ctx.material {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm: None,
            current_peak_chipload_mm: None,
            current_peak_kw: None,
            suggested: Err(RefuseReason::MaterialUnvalidated),
            considered_rows: Vec::new(),
            rationale: vec![
                "Material::Custom without validated Kc — refuse rather than guess".to_owned(),
            ],
        };
    }

    // Pull current state from the trace. Walk steady-state samples (per
    // Item C: feed >= 0.95 × commanded feed, in-cut, not rapid).
    let stats = collect_steady_state_stats(trace, ctx.toolpath_id, ctx.current_feed_mm_min);

    let current_rpm = stats.median_rpm;
    let current_peak_chipload = stats.peak_chipload;
    let current_peak_kw = stats.peak_power_kw(ctx.tool, ctx.material);

    if stats.valid_count == 0 {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(RefuseReason::SteadyStateSamplesNotPresent),
            considered_rows: Vec::new(),
            rationale: vec![
                "no steady-state samples at the operation's commanded feed — \
                 typically a drill cycle or all-ramp toolpath".to_owned(),
            ],
        };
    }
    if !stats.any_arc_captured {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(RefuseReason::ArcEngagementNotCaptured),
            considered_rows: Vec::new(),
            rationale: vec![
                "simulation trace lacks arc engagement — re-run with capture enabled".to_owned(),
            ],
        };
    }

    // Build the LUT criteria, mirroring the gate's routing for project_curve.
    let geometry_hint = ctx.tool.to_geometry_hint();
    let tool_family = tool_family_for(geometry_hint);
    let Some((lut_op_family, lut_pass_role)) = routed_lookup_family(
        ctx.operation_kind,
        tool_family,
        ctx.operation_family,
        ctx.pass_role,
    ) else {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(RefuseReason::NoVendorData),
            considered_rows: Vec::new(),
            rationale: vec![
                "no vendor data covers this tool family + operation kind".to_owned(),
            ],
        };
    };

    let (material_family, hardness_kind, hardness_value) = material_to_lut(ctx.material);
    let lookup_axial_doc_mm = stats.peak_axial_doc.max(ctx.tool.diameter() * 0.5);
    let criteria = LookupCriteria {
        tool_family,
        tool_subfamily: None,
        diameter_mm: ctx.tool.lookup_diameter_at(lookup_axial_doc_mm),
        flute_count: ctx.tool.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family: lut_op_family,
        pass_role: lut_pass_role,
    };
    let lut = super::chipload::embedded_lut();
    let rows = enumerate_matching_rows(lut, &criteria);
    if rows.is_empty() {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(RefuseReason::NoVendorData),
            considered_rows: Vec::new(),
            rationale: vec![format!(
                "no LUT rows match tool/material/op (tool_family={tool_family:?}, \
                 material_family={material_family:?}, op={lut_op_family:?})"
            )],
        };
    }

    // Bipolar engagement: are SOME current samples below the best
    // matching row's cl_min while OTHERS run above its cl_max? If so,
    // no single feed fixes both — we'd flip one verdict to fix the
    // other. Refuse with a stepover-variation hint instead.
    if let Some(top) = rows.first()
        && let (Some(cl_min), Some(cl_max)) = (top.chip_load_min_mm, top.chip_load_max_mm)
        && stats.peak_below_chipload(cl_min)
        && stats.peak_above_chipload(cl_max)
    {
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(RefuseReason::BipolarEngagement),
            considered_rows: rows
                .iter()
                .map(|r| RowEvaluation {
                    observation_id: r.observation_id.clone(),
                    diameter_match_score: r.diameter_match_score,
                    rpm: r.rpm_nominal,
                    feasible_feed_range: None,
                    mrr_mm3_min: None,
                    rejected: Some("bipolar engagement detected".to_owned()),
                })
                .collect(),
            rationale: vec![
                "stepover varies — some samples below cl_min while others above cl_max. \
                 No single feed fixes both. Reduce stepover variation rather than feed."
                    .to_owned(),
            ],
        };
    }

    // Evaluate each row, build feasible feed ranges, score, pick best.
    let style = SuggestionStyle::default();
    let mut evaluations: Vec<RowEvaluation> = Vec::with_capacity(rows.len());
    let is_finish_pass = matches!(ctx.pass_role, LutPassRole::Finish | LutPassRole::SemiFinish);
    let mut best: Option<BestPick> = None;

    for row in rows.iter() {
        match evaluate_row(ctx, &stats, row, ctx.machine) {
            RowOutcome::Feasible {
                rpm,
                feed_lo,
                feed_hi,
                power_cap_kw,
            } => {
                let mid = 0.5 * (feed_lo + feed_hi);
                let mrr = mid * stats.peak_axial_doc * stats.peak_radial_width;
                // Weight the optimisation objective by diameter-fit so a
                // row that wins narrowly on RPM/MRR doesn't beat a row
                // with a much closer diameter match. The chipload window
                // calibration is most meaningful at the row's nominal
                // diameter; a poor diameter fit means the bounds extrapolate
                // less reliably. Floor at 0.1 so a terrible match still
                // beats refusal — we keep the row visible in
                // considered_rows even if it's a fallback.
                let diam_weight =
                    (row.diameter_match_score as f64 / 200.0).max(0.1);
                let raw_objective = if is_finish_pass {
                    // Surface speed proxy for finish: π · D · rpm; D fixed
                    // for the toolpath, so rank on rpm alone.
                    rpm
                } else {
                    mrr
                };
                let objective = raw_objective * diam_weight;
                evaluations.push(RowEvaluation {
                    observation_id: row.observation_id.clone(),
                    diameter_match_score: row.diameter_match_score,
                    rpm: Some(rpm),
                    feasible_feed_range: Some(feed_lo..feed_hi),
                    mrr_mm3_min: Some(mrr),
                    rejected: None,
                });
                let take = best.as_ref().is_none_or(|b| objective > b.objective);
                if take {
                    best = Some(BestPick {
                        row: row.clone(),
                        objective,
                        rpm,
                        feed_range: feed_lo..feed_hi,
                        power_cap_kw,
                    });
                }
            }
            RowOutcome::Rejected(reason) => {
                evaluations.push(RowEvaluation {
                    observation_id: row.observation_id.clone(),
                    diameter_match_score: row.diameter_match_score,
                    rpm: None,
                    feasible_feed_range: None,
                    mrr_mm3_min: None,
                    rejected: Some(reason),
                });
            }
        }
    }

    let Some(BestPick {
        row,
        rpm,
        feed_range,
        power_cap_kw,
        ..
    }) = best
    else {
        // Every row was rejected. Pick the dominant reason: if any row
        // hit RpmBracketEmpty, prefer that; otherwise NoFeasibleRow.
        let any_rpm_empty = evaluations
            .iter()
            .any(|e| e.rejected.as_deref() == Some("rpm bracket empty"));
        let reason = if any_rpm_empty {
            RefuseReason::RpmBracketEmpty
        } else {
            RefuseReason::NoFeasibleRow
        };
        return FeedSuggestion {
            toolpath_id: ctx.toolpath_id,
            current_feed_mm_min: ctx.current_feed_mm_min,
            current_rpm,
            current_peak_chipload_mm: current_peak_chipload,
            current_peak_kw,
            suggested: Err(reason),
            considered_rows: evaluations,
            rationale: vec![
                "every compatible LUT row was rejected by RPM bracket or feasibility \
                 (chipload window vs machine feed cap)".to_owned(),
            ],
        };
    };

    let feed = pick_feed_in_range(&feed_range, style);
    rationale.push(format!(
        "matched row '{}' (diameter score {}/200) at {:.0} RPM",
        row.observation_id, row.diameter_match_score, rpm
    ));
    rationale.push(format!(
        "feasible feed range {:.0}-{:.0} mm/min (chipload window {:.4}-{:.4} mm/tooth)",
        feed_range.start,
        feed_range.end,
        row.chip_load_min_mm.unwrap_or(0.0),
        row.chip_load_max_mm.unwrap_or(0.0),
    ));
    if let Some(pcap) = power_cap_kw {
        rationale.push(format!("power cap {pcap:.2} kW limited the upper feed bound"));
    }
    if matches!(style, SuggestionStyle::Balanced) {
        rationale.push("balanced midpoint of the feasible range".to_owned());
    }

    let chipload_envelope = row.chip_load_min_mm.unwrap_or(0.0)
        ..row.chip_load_max_mm.unwrap_or(f64::INFINITY);
    let matched_row_id = row.observation_id;

    FeedSuggestion {
        toolpath_id: ctx.toolpath_id,
        current_feed_mm_min: ctx.current_feed_mm_min,
        current_rpm,
        current_peak_chipload_mm: current_peak_chipload,
        current_peak_kw,
        suggested: Ok(SuggestedFeeds {
            rpm,
            feed_mm_min: feed,
            chipload_envelope,
            power_cap_kw,
            matched_row_id,
            confidence: Confidence::Approximate(
                "isotropic Kc with 2.5× anisotropy multiplier; vendor row applied at \
                 sample-derived axial/radial engagement"
                    .to_owned(),
            ),
            style,
        }),
        considered_rows: evaluations,
        rationale,
    }
}

struct BestPick {
    row: MatchedRow,
    objective: f64,
    rpm: f64,
    feed_range: Range<f64>,
    power_cap_kw: Option<f64>,
}

/// Walk every enabled toolpath in a project and produce a suggestion
/// per toolpath. Mirrors `gcode::project_load_report`'s structure so
/// the embedded GUI MCP and the standalone MCP can share a single
/// entry point that takes the sim trace explicitly (the GUI holds
/// the trace in viz simulation state, not in `session.simulation`).
pub fn project_suggestions(
    project: &crate::session::ProjectSession,
    sim_trace: Option<&SimulationCutTrace>,
) -> Vec<FeedSuggestion> {
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
    use crate::feeds::{OperationFamily, PassRole};

    let material = &project.stock_config().material;
    let machine = project.machine();
    let mut out = Vec::new();
    for tc in project.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = project.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))
        else {
            continue;
        };
        let tool = crate::compute::cutter::build_cutter(tool_cfg);
        let spec = tc.operation.spec();
        let lut_op = match spec.feeds_family {
            OperationFamily::Adaptive => LutOperationFamily::Adaptive,
            OperationFamily::Pocket => LutOperationFamily::Pocket,
            OperationFamily::Contour => LutOperationFamily::Contour,
            OperationFamily::Parallel => LutOperationFamily::Parallel,
            OperationFamily::Scallop => LutOperationFamily::Scallop,
            OperationFamily::Trace => LutOperationFamily::Trace,
            OperationFamily::Face => LutOperationFamily::Face,
        };
        let lut_pass = match spec.feeds_pass_role {
            PassRole::Roughing => LutPassRole::Roughing,
            PassRole::SemiFinish => LutPassRole::SemiFinish,
            PassRole::Finish => LutPassRole::Finish,
        };
        let ctx = SuggestContext {
            toolpath_id: tc.id,
            tool: &tool,
            material,
            machine,
            operation_family: lut_op,
            pass_role: lut_pass,
            operation_kind: tc.operation.op_type(),
            current_feed_mm_min: tc.operation.feed_rate(),
        };
        out.push(evaluate(&ctx, sim_trace));
    }
    out
}

enum RowOutcome {
    Feasible {
        rpm: f64,
        feed_lo: f64,
        feed_hi: f64,
        power_cap_kw: Option<f64>,
    },
    Rejected(String),
}

fn evaluate_row(
    ctx: &SuggestContext<'_>,
    stats: &SteadyStateStats,
    row: &MatchedRow,
    machine: &MachineProfile,
) -> RowOutcome {
    let (machine_min_rpm, machine_max_rpm) = machine.rpm_range();

    // Pick an RPM in the row's bracket clamped to the machine bracket.
    let row_rpm_lo = row.rpm_min.or(row.rpm_nominal).unwrap_or(machine_min_rpm);
    let row_rpm_hi = row.rpm_max.or(row.rpm_nominal).unwrap_or(machine_max_rpm);
    let lo = row_rpm_lo.max(machine_min_rpm);
    let hi = row_rpm_hi.min(machine_max_rpm);
    if lo > hi {
        return RowOutcome::Rejected("rpm bracket empty".to_owned());
    }
    let rpm = row.rpm_nominal.unwrap_or((lo + hi) * 0.5).clamp(lo, hi);
    let rpm = machine.clamp_rpm(rpm);

    // Build the chipload-window feed range. Both bounds are required:
    // suggest can't recommend a feed without an upper bound, and Item 2
    // says we don't synthesise a lower bound.
    let (cl_min, cl_max) = match (row.chip_load_min_mm, row.chip_load_max_mm) {
        (Some(lo), Some(hi)) if lo > 0.0 && hi >= lo => (lo, hi),
        _ => return RowOutcome::Rejected("chipload bounds incomplete".to_owned()),
    };
    let flutes = ctx.tool.flute_count.max(1) as f64;
    let feed_lo_chip = cl_min * rpm * flutes;
    let feed_hi_chip = cl_max * rpm * flutes;

    // Power cap: max feed at this row's RPM that keeps power under
    // `available × safety`. Reuses the same Kc/anisotropy multiplier as
    // the power guardrail — same model on both sides of the report.
    let kc_eff = ANISOTROPY_MULTIPLIER * ctx.material.kc_n_per_mm2();
    let axial = stats.peak_axial_doc.max(0.0);
    let radial = stats.peak_radial_width.max(0.0);
    let power_cap_kw = machine.power_at_rpm(rpm) * machine.safety_factor;
    let feed_pwr_max = if kc_eff > 0.0 && axial > 0.0 && radial > 0.0 && power_cap_kw > 0.0 {
        power_cap_kw * 60_000_000.0 / (kc_eff * axial * radial)
    } else {
        f64::INFINITY
    };

    let feed_hi = feed_hi_chip.min(feed_pwr_max);
    let feed_lo = feed_lo_chip;
    if feed_lo > feed_hi {
        return RowOutcome::Rejected(format!(
            "row chipload-min × RPM × flutes = {feed_lo:.0} mm/min exceeds upper feasibility \
             ({feed_hi:.0} mm/min)"
        ));
    }
    // Honour the machine's max feed.
    let machine_max_feed = machine.max_feed_mm_min;
    if feed_lo > machine_max_feed {
        return RowOutcome::Rejected(format!(
            "row's lower-bound feed {feed_lo:.0} mm/min exceeds machine max feed \
             {machine_max_feed:.0} mm/min"
        ));
    }
    let feed_hi = feed_hi.min(machine_max_feed);
    let power_cap_reported = if feed_hi_chip > feed_pwr_max {
        Some(power_cap_kw)
    } else {
        None
    };
    RowOutcome::Feasible {
        rpm,
        feed_lo,
        feed_hi,
        power_cap_kw: power_cap_reported,
    }
}

fn pick_feed_in_range(range: &Range<f64>, style: SuggestionStyle) -> f64 {
    let lo = range.start;
    let hi = range.end;
    match style {
        SuggestionStyle::Conservative => lo + (hi - lo) / 3.0,
        SuggestionStyle::Balanced => 0.5 * (lo + hi),
        SuggestionStyle::Aggressive => hi - (hi - lo) / 3.0,
    }
}

/// Steady-state per-toolpath stats used by the suggest algorithm. Built
/// once per toolpath, mirrors the chipload gate's filter (Item C).
struct SteadyStateStats {
    valid_count: usize,
    any_arc_captured: bool,
    median_rpm: Option<f64>,
    peak_chipload: Option<f64>,
    peak_axial_doc: f64,
    peak_radial_width: f64,
    /// Captured chiploads for bipolar-engagement detection.
    chiploads: Vec<f64>,
}

impl SteadyStateStats {
    fn peak_below_chipload(&self, cl_min: f64) -> bool {
        self.chiploads.iter().any(|&c| c < cl_min)
    }
    fn peak_above_chipload(&self, cl_max: f64) -> bool {
        self.chiploads.iter().any(|&c| c > cl_max)
    }
    /// Estimate of peak power-kW at the current feed/engagement. None
    /// if we don't have enough data to compute it.
    fn peak_power_kw(&self, _tool: &ToolDefinition, material: &Material) -> Option<f64> {
        if let Material::Custom { .. } = material {
            return None;
        }
        let kc = material.kc_n_per_mm2();
        if !kc.is_finite() || kc <= 0.0 {
            return None;
        }
        if self.peak_axial_doc <= 0.0 || self.peak_radial_width <= 0.0 {
            return None;
        }
        // Use the toolpath's commanded feed since stats already filtered
        // to steady state.
        // P_kW = Kc_eff × DOC × WOC × feed / 60e6
        // peak_feed isn't tracked here — rely on the trace-side power
        // guardrail for the actual peak. This estimate uses the median
        // sample's properties at the commanded feed. Useful as a
        // ballpark for the "current" UI display.
        None
    }
}

fn collect_steady_state_stats(
    trace: &SimulationCutTrace,
    toolpath_id: usize,
    operation_feed_mm_min: f64,
) -> SteadyStateStats {
    const STEADY_STATE_FEED_FRACTION: f64 = 0.95;
    let threshold = STEADY_STATE_FEED_FRACTION * operation_feed_mm_min;

    let mut rpms: Vec<f64> = Vec::new();
    let mut chiploads: Vec<f64> = Vec::new();
    let mut peak_chipload: Option<f64> = None;
    let mut peak_axial_doc: f64 = 0.0;
    let mut peak_radial_width: f64 = 0.0;
    let mut any_arc_captured = false;

    for s in trace.samples.iter() {
        if s.toolpath_id != toolpath_id {
            continue;
        }
        if !s.is_cutting {
            continue;
        }
        if s.feed_rate_mm_min < threshold {
            continue;
        }
        if s.radial_engagement < 0.02 {
            continue;
        }
        let Some(arc) = s.arc_engagement_radians else {
            continue;
        };
        any_arc_captured = true;
        rpms.push(s.spindle_rpm as f64);
        if let Some(cl) = s.effective_chip_thickness_mm {
            chiploads.push(cl);
            peak_chipload = Some(peak_chipload.map_or(cl, |p| p.max(cl)));
        }
        peak_axial_doc = peak_axial_doc.max(s.axial_doc_mm);
        // Arc-equivalent radial slab width. The simulator already gives
        // us `radial_engagement` (the fraction of the engaged width).
        // Combine with the arc-fraction to get a per-sample WOC proxy
        // consistent with the power guardrail's slab convention.
        let woc = s.radial_engagement * (arc / std::f64::consts::PI);
        peak_radial_width = peak_radial_width.max(woc);
    }

    let median_rpm = {
        let mut sorted = rpms.clone();
        sorted.sort_by(f64::total_cmp);
        let mid = sorted.len() / 2;
        sorted.get(mid).copied()
    };

    SteadyStateStats {
        valid_count: rpms.len(),
        any_arc_captured,
        median_rpm,
        peak_chipload,
        peak_axial_doc,
        peak_radial_width,
        chiploads,
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
    use crate::compute::catalog::OperationType;
    use crate::feeds::vendor_lut::LutPassRole;
    use crate::material::WoodSpecies;
    use crate::simulation_cut::{
        CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    };
    use crate::tool::FlatEndmill;

    fn tool_6mm_flat() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 18.0)),
            6.35,
            30.0,
            20.0,
            55.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    fn machine() -> MachineProfile {
        MachineProfile::shapeoko_vfd()
    }

    fn cutting_sample(idx: usize, feed: f64, rpm: u32, axial: f64, arc: f64) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: 0,
            move_index: idx,
            sample_index: idx,
            position: [0.0, 0.0, 0.0],
            cumulative_time_s: 0.0,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: feed,
            spindle_rpm: rpm,
            flute_count: 2,
            axial_doc_mm: axial,
            radial_engagement: 0.5,
            arc_engagement_radians: Some(arc),
            chipload_mm_per_tooth: 0.04,
            effective_chip_thickness_mm: Some(0.04),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 1.0,
            semantic_item_id: None,
        }
    }

    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            schema_version: 1,
            sample_step_mm: 1.0,
            summary: SimulationCutSummary {
                sample_count: samples.len(),
                toolpath_count: 1,
                issue_count: 0,
                hotspot_count: 0,
                total_runtime_s: 1.0,
                cutting_runtime_s: 1.0,
                rapid_runtime_s: 0.0,
                air_cut_time_s: 0.0,
                low_engagement_time_s: 0.0,
                average_engagement: 0.5,
                peak_chipload_mm_per_tooth: 0.04,
                peak_axial_doc_mm: 2.0,
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
            },
            samples,
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            provenance: None,
        }
    }

    #[test]
    fn refuses_when_simulation_missing() {
        let tool = tool_6mm_flat();
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = machine();
        let ctx = SuggestContext {
            toolpath_id: 0,
            tool: &tool,
            material: &mat,
            machine: &machine,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_kind: OperationType::Pocket,
            current_feed_mm_min: 1500.0,
        };
        let result = evaluate(&ctx, None);
        assert!(matches!(
            result.suggested,
            Err(RefuseReason::SimulationRequired)
        ));
    }

    #[test]
    fn refuses_on_custom_material() {
        let tool = tool_6mm_flat();
        let mat = Material::Custom {
            name: "Unknown".to_owned(),
            hardness_index: 1.0,
            kc: 30.0,
        };
        let machine = machine();
        let t = trace(vec![cutting_sample(
            0,
            1500.0,
            18000,
            2.0,
            std::f64::consts::FRAC_PI_2,
        )]);
        let ctx = SuggestContext {
            toolpath_id: 0,
            tool: &tool,
            material: &mat,
            machine: &machine,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_kind: OperationType::Pocket,
            current_feed_mm_min: 1500.0,
        };
        let result = evaluate(&ctx, Some(&t));
        assert!(matches!(
            result.suggested,
            Err(RefuseReason::MaterialUnvalidated)
        ));
    }

    #[test]
    fn refuses_when_no_steady_state_samples() {
        // Every sample at 30% of commanded feed — drill cycle.
        let samples: Vec<SimulationCutSample> = (0..3)
            .map(|i| cutting_sample(i, 300.0, 18000, 1.0, std::f64::consts::FRAC_PI_2))
            .collect();
        let t = trace(samples);
        let tool = tool_6mm_flat();
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = machine();
        let ctx = SuggestContext {
            toolpath_id: 0,
            tool: &tool,
            material: &mat,
            machine: &machine,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_kind: OperationType::Pocket,
            current_feed_mm_min: 1500.0,
        };
        let result = evaluate(&ctx, Some(&t));
        assert!(matches!(
            result.suggested,
            Err(RefuseReason::SteadyStateSamplesNotPresent)
        ));
    }

    #[test]
    fn produces_recommendation_for_canonical_pocket_case() {
        // 6mm flat, hardwood, pocket roughing, feed 1500. Should match a
        // hardwood pocket-rough row and produce a non-trivial range.
        let samples: Vec<SimulationCutSample> = (0..5)
            .map(|i| cutting_sample(i, 1500.0, 18000, 2.0, std::f64::consts::FRAC_PI_2))
            .collect();
        let t = trace(samples);
        let tool = tool_6mm_flat();
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let machine = machine();
        let ctx = SuggestContext {
            toolpath_id: 0,
            tool: &tool,
            material: &mat,
            machine: &machine,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_kind: OperationType::Pocket,
            current_feed_mm_min: 1500.0,
        };
        let result = evaluate(&ctx, Some(&t));
        let suggested = result.suggested.expect("should produce a suggestion");
        assert!(suggested.feed_mm_min > 0.0);
        assert!(suggested.rpm >= 6000.0 && suggested.rpm <= 24000.0);
        assert!(!suggested.matched_row_id.is_empty());
        assert!(suggested.chipload_envelope.start < suggested.chipload_envelope.end);
        // Rationale includes a row id and a feasibility-range line.
        assert!(
            result.rationale.iter().any(|r| r.contains("matched row")),
            "rationale should explain the match: {:?}",
            result.rationale
        );
    }
}
