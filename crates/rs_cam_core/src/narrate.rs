//! Agent-friendly narration for generated toolpaths and simulation cut traces.
//!
//! This module intentionally sits above the raw `ToolpathDebugTrace`,
//! `ToolpathSemanticTrace`, and `SimulationCutTrace` data.  It produces a short
//! prose report that makes spatial/metric anomalies easy for an LLM or human to
//! notice without hand-filtering large JSON dumps.

use crate::ids::ToolpathId;
use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::debug_trace::ToolpathDebugTrace;
use crate::geo::P3;
use crate::semantic_trace::{SemanticKey, ToolpathSemanticKind, ToolpathSemanticTrace};
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::{Move, MoveType, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, SpanKind, SpanPayload};

const Z_EPSILON_MM: f64 = 0.05;
/// Threshold for flagging arc moves whose radius is suspiciously large
/// relative to the tool radius. Also used by `arcfit::try_fit_arc` to
/// reject implausible Kåsa-bias fits before they reach narration — see
/// Roadmap F.10 RCA at `planning/F10_RCA.md`.
///
/// **ENVELOPE-relative, deliberately** (H2.6, resolved in
/// `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md` §5). This is a
/// post-condition on `arcfit::try_fit_arc`'s OWN radius cap — a detector
/// for a defect in our arc fitter — not a feature-scale machining
/// judgement. Both sides MUST resolve to the same radius: the fitter's
/// bound arrives as `tool_diameter / 2.0` (`compute/execute.rs`) and
/// narration reads `envelope_radius_mm()` off the same `ToolDefinition`.
/// Moving either side to `cusp_radius_mm()` would drop that side's
/// threshold 6× on a tapered tool and make narration fire on every arc
/// the fitter legitimately accepted. Pinned by
/// `tests/tool_scale_semantics_pr2.rs`.
pub(crate) const LARGE_ARC_RADIUS_MULTIPLIER: f64 = 30.0;
const AIR_CUT_WARNING_PERCENT: f64 = 50.0;
const DEEP_DOC_MULTIPLIER: f64 = 1.5;
const MAX_LEVEL_LINES: usize = 8;
/// Distinct region LABELS listed by `append_region_mix` before it elides.
const MAX_REGION_MIX_GROUPS: usize = 8;
const MAX_ANOMALY_LINES: usize = 8;

/// Optional metadata that lets narration tie raw traces back to the project.
#[derive(Debug, Clone, Default)]
pub struct ToolpathNarrationContext<'a> {
    pub toolpath_id: Option<ToolpathId>,
    pub toolpath_name: Option<&'a str>,
    /// Human label, used for DISPLAY only. All routing decisions key on
    /// [`Self::operation_kind`] (Phase 1 T13) so label edits can never
    /// silently change which narration branch fires.
    pub operation_label: Option<&'a str>,
    /// Typed operation kind for narration routing (threshold phrasing,
    /// air-cut hints). `None` falls back to the generic phrasing.
    pub operation_kind: Option<crate::compute::catalog::OperationType>,
    pub depth_per_pass_mm: Option<f64>,
    pub stepover_mm: Option<f64>,
    pub tool_diameter_mm: Option<f64>,
    pub feed_rate_mm_min: Option<f64>,
    pub spindle_rpm: Option<u32>,
    pub flute_count: Option<u32>,
    /// True for `OperationType::{Drill, AlignmentPinDrill}`. The
    /// engagement model is XY-only (cylinder side-engagement against
    /// the dexel grid), so drill cycles always read 0 % engagement and
    /// 100 % air-cut even when actively making chips. Setting this
    /// suppresses the air-cut anomaly in narration. F4 of
    /// `planning/OPTIMIZER_UX_DIALIN_FIXES.md`.
    pub is_drill_cycle: bool,
    /// Stock material — used by the drill-cycle narration block to
    /// evaluate the plunge-feed envelope (material-aware mm/min per mm
    /// of cutter diameter). `None` falls back to envelope-free reporting.
    pub material: Option<&'a crate::material::Material>,
    /// A/M9: the generation-time standing-material finding for this
    /// toolpath, straight off
    /// [`crate::compute::config::ToolpathStats::standing_material_mm2`].
    ///
    /// `None` = not measured (no ring cascade ran, or the caller has no
    /// stats), `Some(0.0)` = a cascade measured zero. Narration states
    /// which — an agent reading this report must never have to guess
    /// whether a silent zero means "clean" or "unknown".
    pub standing_material_mm2: Option<f64>,
    /// Wave D1: a planned finish band whose cutting was entirely erased by
    /// height resolution, straight off
    /// [`crate::compute::config::ToolpathStats::dropped_band`]. `None` =
    /// nothing dropped or nothing measured; narration says which, using
    /// [`Self::operation_kind`] to tell those two apart.
    pub dropped_band: Option<crate::compute::config::DroppedBandFinding>,
    /// C8: a planned finish band whose Z ladder height resolution SHORTENED
    /// while it still cut, straight off
    /// [`crate::compute::config::ToolpathStats::clipped_band`]. `None` =
    /// nothing clipped or nothing measured; narration says which, using
    /// [`Self::operation_kind`] to tell those two apart — the same contract
    /// [`Self::dropped_band`] carries.
    pub clipped_band: Option<crate::compute::config::ClippedBandFinding>,
    /// C8: what the ramp-finish reach clamp did, off
    /// [`crate::compute::config::ToolpathStats::ramp_reach_clamp`]. `None` =
    /// no ramp descent ran, so nothing was measured — NOT "the tool reached
    /// everywhere". A/M9's contract, applied to a third measure.
    pub ramp_reach_clamp: Option<crate::ramp_finish::RampReachClamp>,
    /// Wave D1: the centreline TIP-FLOAT tally, off
    /// [`crate::compute::config::ToolpathStats::tip_float`]. `None` = the
    /// operation emits no valley centrelines, so nothing was measured — NOT
    /// "the tool reached everywhere".
    pub tip_float: Option<crate::compute::config::TipFloatFinding>,
    /// A/M7 gate 1: the retract round-trip count, off
    /// [`crate::compute::config::ToolpathStats::retract_trips`]. `None` =
    /// this stats struct never walked a move list (a placeholder, never a
    /// real generation) — narration says so rather than staying silent.
    pub retract_trips: Option<crate::compute::config::RetractTripCount>,
}

#[derive(Debug, Clone)]
struct ZLevelSummary {
    z: f64,
    cutting_moves: usize,
    arc_moves: usize,
    cutting_distance_mm: f64,
    max_radius_from_centroid_mm: f64,
    cut_run_ids: BTreeSet<usize>,
    marching_square_regions: Option<usize>,
    region_areas_mm2: Vec<f64>,
    dropped_micro_regions: Option<usize>,
    perimeter_sweep_length_mm: Option<f64>,
    agent_walk_cut_length_mm: Option<f64>,
    residual_cleanup_cell_count: Option<usize>,
}

impl ZLevelSummary {
    fn new(z: f64) -> Self {
        Self {
            z,
            cutting_moves: 0,
            arc_moves: 0,
            cutting_distance_mm: 0.0,
            max_radius_from_centroid_mm: 0.0,
            cut_run_ids: BTreeSet::new(),
            marching_square_regions: None,
            region_areas_mm2: Vec::new(),
            dropped_micro_regions: None,
            perimeter_sweep_length_mm: None,
            agent_walk_cut_length_mm: None,
            residual_cleanup_cell_count: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ZLevelAccumulator {
    summary: ZLevelSummary,
    centroid_sum_x: f64,
    centroid_sum_y: f64,
    cutting_xy: Vec<(f64, f64)>,
}

impl ZLevelAccumulator {
    fn new(z: f64) -> Self {
        Self {
            summary: ZLevelSummary::new(z),
            centroid_sum_x: 0.0,
            centroid_sum_y: 0.0,
            cutting_xy: Vec::new(),
        }
    }

    fn observe_cutting_move(&mut self, mv: &Move, prior_target: Option<P3>, cut_run_id: usize) {
        self.summary.cutting_moves += 1;
        self.summary.cut_run_ids.insert(cut_run_id);
        if matches!(
            mv.move_type,
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        ) {
            self.summary.arc_moves += 1;
        }
        if let Some(prev) = prior_target {
            self.summary.cutting_distance_mm += (mv.target - prev).norm();
        }
        self.centroid_sum_x += mv.target.x;
        self.centroid_sum_y += mv.target.y;
        self.cutting_xy.push((mv.target.x, mv.target.y));
    }

    fn finish(mut self) -> ZLevelSummary {
        let count = self.cutting_xy.len();
        if count > 0 {
            let center_x = self.centroid_sum_x / count as f64;
            let center_y = self.centroid_sum_y / count as f64;
            self.summary.max_radius_from_centroid_mm = self
                .cutting_xy
                .iter()
                .map(|(x, y)| ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt())
                .fold(0.0, f64::max);
        }
        self.summary
    }
}

#[derive(Debug, Clone)]
struct ArcObservation {
    move_index: usize,
    z: f64,
    radius_mm: f64,
    center_x: f64,
    center_y: f64,
    target_x: f64,
    target_y: f64,
    clockwise: bool,
}

/// Produce a concise prose narration for a generated toolpath.
///
/// Use [`narrate_toolpath_with_context`] when the caller knows the project
/// toolpath id or commanded depth per pass; this wrapper keeps the simple API
/// available for ad-hoc use.
pub fn narrate_toolpath(
    annotated: &AnnotatedToolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
    cut_trace: Option<&SimulationCutTrace>,
    debug_trace: Option<&ToolpathDebugTrace>,
    tool: &ToolDefinition,
) -> String {
    narrate_toolpath_with_context(
        annotated,
        semantic_trace,
        cut_trace,
        debug_trace,
        tool,
        &ToolpathNarrationContext::default(),
    )
}

/// Produce a concise prose narration for a generated toolpath, using optional
/// project metadata to filter simulation samples and label the report.
///
/// When `annotated.spans_valid` is true and the toolpath carries
/// [`SpanKind::DepthPass`] spans, the Z-level structure is read directly from
/// those spans (one level per pass). This eliminates the phantom-passes
/// artifact produced by clustering raw move Z-coordinates with `Z_EPSILON_MM`
/// when one DepthPass legitimately spans multiple Z values
/// (e.g. lead-in / ramp moves above the pass plane).
pub fn narrate_toolpath_with_context(
    annotated: &AnnotatedToolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
    cut_trace: Option<&SimulationCutTrace>,
    debug_trace: Option<&ToolpathDebugTrace>,
    tool: &ToolDefinition,
    context: &ToolpathNarrationContext<'_>,
) -> String {
    let toolpath = &annotated.toolpath;
    let title = context
        .toolpath_name
        .or_else(|| semantic_trace.map(|trace| trace.toolpath_name.as_str()))
        .unwrap_or("Toolpath");
    let operation = context
        .operation_label
        .or_else(|| semantic_trace.map(|trace| trace.operation_label.as_str()))
        .or_else(|| debug_trace.map(|trace| trace.operation_label.as_str()))
        .unwrap_or("unknown operation");

    let cutting_distance = toolpath.total_cutting_distance();
    let rapid_distance = toolpath.total_rapid_distance();
    let mut output = String::new();
    output.push_str(&format!(
        "{title} — {operation}, {} moves, {:.0}mm cutting, {:.0}mm rapid\n\n",
        toolpath.moves.len(),
        cutting_distance,
        rapid_distance
    ));

    if let Some(trace) = semantic_trace {
        let depth_items = trace
            .items
            .iter()
            .filter(|item| item.kind == ToolpathSemanticKind::DepthLevel)
            .count();
        let region_items = trace
            .items
            .iter()
            .filter(|item| item.kind == ToolpathSemanticKind::Region)
            .count();
        let ring_items = trace
            .items
            .iter()
            .filter(|item| item.kind == ToolpathSemanticKind::Ring)
            .count();
        // C8: `chains` joins the line. Trace and Pencil express their whole
        // structure as `Chain` items, which the previous three counters did
        // not mention at all — so an operation with 40 engraved contours
        // read as `depth levels 3, regions 0, rings 0`, i.e. as no structure
        // whatsoever.
        let chain_items = trace
            .items
            .iter()
            .filter(|item| item.kind == ToolpathSemanticKind::Chain)
            .count();
        output.push_str(&format!(
            "Semantic trace: {} items ({} move-linked); depth levels {}, \
             regions {}, rings {}, chains {}.\n",
            trace.summary.item_count,
            trace.summary.move_linked_item_count,
            depth_items,
            region_items,
            ring_items,
            chain_items
        ));
        append_region_mix(&mut output, trace);
    } else {
        output.push_str("Semantic trace: not available.\n");
    }
    append_standing_material(&mut output, context.standing_material_mm2);
    append_dropped_band(&mut output, context);
    append_clipped_band(&mut output, context);
    append_ramp_reach_clamp(&mut output, context);
    append_tip_float(&mut output, context.tip_float);
    append_retract_trips(&mut output, context.retract_trips);
    output.push_str("Z-level source: ");
    output.push_str(z_level_source_label(annotated));
    output.push_str(".\n");

    if let Some(trace) = debug_trace {
        output.push_str(&format!(
            "Generation debug: {} spans, {} hotspots; dominant span {}.\n",
            trace.summary.span_count,
            trace.summary.hotspot_count,
            trace
                .summary
                .dominant_span_label
                .as_deref()
                .unwrap_or("unknown")
        ));
    }

    append_operation_context(&mut output, context);

    let z_levels = summarize_z_levels(annotated, semantic_trace);
    output.push_str("\nZ-level structure (highest to lowest, setup-local frame):\n");
    if z_levels.is_empty() {
        output.push_str("  No cutting moves found.\n");
    } else {
        append_z_level_lines(&mut output, &z_levels, debug_trace);
    }

    append_engagement_histogram(&mut output, cut_trace, context);

    let anomalies = collect_anomalies(toolpath, cut_trace, tool, context);
    output.push_str("\nAnomalies (most surprising first):\n");
    if anomalies.is_empty() {
        output.push_str("  No high-priority anomalies found by v0.1 heuristics.\n");
    } else {
        for line in anomalies.iter().take(MAX_ANOMALY_LINES) {
            output.push_str("  ");
            output.push_str(line);
            output.push('\n');
        }
        if anomalies.len() > MAX_ANOMALY_LINES {
            output.push_str(&format!(
                "  … {} additional anomaly observations suppressed.\n",
                anomalies.len() - MAX_ANOMALY_LINES
            ));
        }
    }

    output
}

fn append_operation_context(output: &mut String, context: &ToolpathNarrationContext<'_>) {
    let mut parts = Vec::new();
    if let Some(depth) = context.depth_per_pass_mm {
        parts.push(format!("depth_per_pass {:.2}mm", depth));
    }
    if let Some(stepover) = context.stepover_mm {
        let stepover_text = context.tool_diameter_mm.map_or_else(
            || format!("stepover {:.2}mm", stepover),
            |diameter| {
                let pct = if diameter > 0.0 {
                    stepover / diameter * 100.0
                } else {
                    0.0
                };
                let expectation = if pct < 20.0 {
                    " — narrow, expect many low-engagement samples"
                } else {
                    ""
                };
                format!(
                    "stepover {:.2}mm ({:.0}% of tool diameter){expectation}",
                    stepover, pct
                )
            },
        );
        parts.push(stepover_text);
    }
    if let Some(feed) = context.feed_rate_mm_min {
        parts.push(format!("feed {:.0}mm/min", feed));
    }
    if let (Some(feed), Some(rpm), Some(flutes)) = (
        context.feed_rate_mm_min,
        context.spindle_rpm,
        context.flute_count,
    ) && rpm > 0
        && flutes > 0
    {
        let chipload = feed / f64::from(rpm) / f64::from(flutes);
        parts.push(format!("nominal chipload {:.4}mm/tooth", chipload));
    }

    if !parts.is_empty() {
        output.push_str("Operation context: ");
        output.push_str(&parts.join(", "));
        output.push_str(".\n");
    }
}

fn append_engagement_histogram(
    output: &mut String,
    cut_trace: Option<&SimulationCutTrace>,
    context: &ToolpathNarrationContext<'_>,
) {
    let Some(trace) = cut_trace else {
        return;
    };

    let mut buckets = [0usize; 5];
    let mut total = 0usize;
    for sample in trace.samples.iter().filter(|sample| sample.is_cutting) {
        if context
            .toolpath_id
            .is_some_and(|id| sample.toolpath_id != id)
        {
            continue;
        }
        total += 1;
        let engagement = sample.engagement.radial_woc_fraction;
        let bucket = if engagement < 0.02 {
            0
        } else if engagement < 0.10 {
            1
        } else if engagement < 0.30 {
            2
        } else if engagement < 0.70 {
            3
        } else {
            4
        };
        if let Some(slot) = buckets.get_mut(bucket) {
            *slot += 1;
        }
    }

    if total == 0 {
        return;
    }

    output.push_str("\nEngagement distribution (in-cut samples only, n=");
    output.push_str(&format_count(total));
    output.push_str("):\n");
    let labels = [
        ("air   ", "[0.00 .. 0.02]"),
        ("thin  ", "[0.02 .. 0.10]"),
        ("light ", "[0.10 .. 0.30]"),
        ("normal", "[0.30 .. 0.70]"),
        ("heavy ", "[0.70 ..     ]"),
    ];
    for ((label, range), count) in labels.iter().zip(buckets) {
        if count == 0 {
            continue;
        }
        let pct = count as f64 / total as f64 * 100.0;
        output.push_str(&format!(
            "  {label} {range} — {:5.1}% ({})\n",
            pct,
            format_count(count)
        ));
    }
}

fn format_count(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn z_level_source_label(annotated: &AnnotatedToolpath) -> &'static str {
    if annotated.spans_valid
        && annotated
            .spans_of_kind(SpanKind::DepthPass)
            .next()
            .is_some()
    {
        "DepthPass spans"
    } else if annotated.spans_valid {
        "raw-move fallback (no DepthPass spans present)"
    } else {
        "raw-move fallback (spans invalidated by a move-remapping transform)"
    }
}

fn summarize_z_levels(
    annotated: &AnnotatedToolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
) -> Vec<ZLevelSummary> {
    if annotated.spans_valid {
        let depth_passes: Vec<_> = annotated.spans_of_kind(SpanKind::DepthPass).collect();
        if !depth_passes.is_empty() {
            return summarize_z_levels_from_spans(
                &annotated.toolpath,
                depth_passes.into_iter(),
                semantic_trace,
            );
        }
    }
    summarize_z_levels_from_moves(&annotated.toolpath, semantic_trace)
}

/// Build one [`ZLevelSummary`] per [`SpanKind::DepthPass`] span, using the
/// span's payload `z_level` (when present) as the canonical Z. Aggregates
/// per-move cut metrics inside the span's move range.
fn summarize_z_levels_from_spans<'a, I>(
    toolpath: &Toolpath,
    depth_passes: I,
    semantic_trace: Option<&ToolpathSemanticTrace>,
) -> Vec<ZLevelSummary>
where
    I: Iterator<Item = &'a crate::toolpath_spans::Span>,
{
    let mut levels = Vec::<ZLevelSummary>::new();
    for span in depth_passes {
        let z = match &span.payload {
            Some(SpanPayload::DepthPass { z_level, .. }) => *z_level,
            _ => representative_cut_z_in_range(toolpath, span.start_move, span.end_move)
                .unwrap_or(0.0),
        };
        let mut summary = summarize_depth_pass_range(toolpath, span.start_move, span.end_move, z);
        if let Some(trace) = semantic_trace {
            apply_semantic_level_metrics(&mut summary, trace);
        }
        levels.push(summary);
    }
    levels.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap_or(Ordering::Equal));
    levels
}

fn summarize_depth_pass_range(
    toolpath: &Toolpath,
    start: usize,
    end: usize,
    z: f64,
) -> ZLevelSummary {
    let mut accumulator = ZLevelAccumulator::new(z);
    let mut prior_target: Option<P3> = None;
    let mut in_cut_run = false;
    let mut active_cut_run_id = 0usize;
    let end = end.min(toolpath.moves.len());
    for mv in toolpath.moves.iter().take(end).skip(start) {
        let is_cutting = mv.move_type.is_cutting();
        if !is_cutting {
            prior_target = Some(mv.target);
            in_cut_run = false;
            continue;
        }
        if !in_cut_run {
            active_cut_run_id += 1;
            in_cut_run = true;
        }
        accumulator.observe_cutting_move(mv, prior_target, active_cut_run_id);
        prior_target = Some(mv.target);
    }
    accumulator.finish()
}

fn representative_cut_z_in_range(toolpath: &Toolpath, start: usize, end: usize) -> Option<f64> {
    toolpath
        .moves
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .find(|mv| mv.move_type.is_cutting())
        .map(|mv| mv.target.z)
}

/// Legacy Z-level discovery: cluster cutting move Z-coordinates with
/// `Z_EPSILON_MM`. Kept for toolpaths that genuinely lack valid DepthPass spans:
/// the core session bridge still wraps raw Toolpaths with empty spans (S1.5
/// pending), and boundary clipping currently invalidates spans until the S83
/// remap lands.
fn summarize_z_levels_from_moves(
    toolpath: &Toolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
) -> Vec<ZLevelSummary> {
    let mut levels = Vec::<ZLevelAccumulator>::new();
    let mut prior_target: Option<P3> = None;
    let mut in_cut_run = false;
    let mut active_cut_run_id = 0usize;

    for mv in &toolpath.moves {
        let is_cutting = mv.move_type.is_cutting();
        if !is_cutting {
            prior_target = Some(mv.target);
            in_cut_run = false;
            continue;
        }

        if !in_cut_run {
            active_cut_run_id += 1;
            in_cut_run = true;
        }

        let z = mv.target.z;
        let accumulator = find_or_create_level_accumulator(&mut levels, z);
        accumulator.observe_cutting_move(mv, prior_target, active_cut_run_id);
        prior_target = Some(mv.target);
    }

    let mut summaries: Vec<_> = levels.into_iter().map(ZLevelAccumulator::finish).collect();
    for level in &mut summaries {
        if let Some(trace) = semantic_trace {
            apply_semantic_level_metrics(level, trace);
        }
    }

    summaries.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap_or(Ordering::Equal));
    summaries
}

fn find_or_create_level_accumulator(
    levels: &mut Vec<ZLevelAccumulator>,
    z: f64,
) -> &mut ZLevelAccumulator {
    if let Some(pos) = levels
        .iter()
        .position(|level| (level.summary.z - z).abs() <= Z_EPSILON_MM)
    {
        // SAFETY: `pos` came from `levels.iter().position`, so it is in bounds.
        #[allow(clippy::indexing_slicing)]
        return &mut levels[pos];
    }
    levels.push(ZLevelAccumulator::new(z));
    let last_index = levels.len() - 1;
    // SAFETY: we just pushed one element, so the last element exists.
    #[allow(clippy::indexing_slicing)]
    &mut levels[last_index]
}

/// One line naming the semantic `Region` items by label, grouped, in
/// first-appearance (cut) order.
///
/// Plan A/M8: the region COUNT alone cannot say which strategy earned its
/// time. `UnifiedFinish` labels its regions with band and strategy
/// (`"MidSteep band (scallop)"`), so this line is the mix table H4 needs;
/// for operations whose regions are plain ordinals it degrades to a short
/// enumeration and is capped at [`MAX_REGION_MIX_GROUPS`].
fn append_region_mix(output: &mut String, trace: &ToolpathSemanticTrace) {
    let mut groups: Vec<(&str, usize, usize)> = Vec::new();
    for item in trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::Region)
    {
        let moves = match (item.move_start, item.move_end) {
            (Some(start), Some(end)) if end >= start => end - start + 1,
            _ => 0,
        };
        if let Some(group) = groups
            .iter_mut()
            .find(|(label, _, _)| *label == item.label.as_str())
        {
            group.1 += 1;
            group.2 += moves;
        } else {
            groups.push((item.label.as_str(), 1, moves));
        }
    }
    if groups.is_empty() {
        return;
    }

    let hidden = groups.len().saturating_sub(MAX_REGION_MIX_GROUPS);
    let shown: Vec<String> = groups
        .iter()
        .take(MAX_REGION_MIX_GROUPS)
        .map(|(label, count, moves)| format!("{label} x{count} ({moves} moves)"))
        .collect();
    output.push_str("Region mix: ");
    output.push_str(&shown.join(", "));
    if hidden > 0 {
        output.push_str(&format!(", and {hidden} more label(s)"));
    }
    output.push_str(".\n");
}

/// Area (mm²) below which standing material is worth reporting as a
/// number but not as a defect. Mirrors the diagnostics adapter's floor
/// (`diagnostics::adapters::from_generation`) so the two surfaces cannot
/// disagree about what counts as an island.
const STANDING_MATERIAL_NARRATION_FLOOR_MM2: f64 = 1.0;

/// A/M9: one line, always, saying whether material was left standing —
/// **including when the answer is "nobody measured"**.
///
/// The channel exists because the figure previously lived only in a
/// `tracing::warn!`: a 28 mm block of unmachined material shipped for weeks
/// because no harness installed a subscriber. Narration is the agent-facing
/// surface, so silence here is the same failure mode. Every phrasing states
/// the measurement's domain, stage and resolution (M1) — an unlabelled mm²
/// invites exactly the cross-domain comparison the audit found.
fn append_standing_material(output: &mut String, measured: Option<f64>) {
    use crate::compute::config::{
        STANDING_MATERIAL_DOMAIN, STANDING_MATERIAL_RESOLUTION, STANDING_MATERIAL_STAGE,
    };
    match measured {
        // NaN falls in here too: an unmeasured cascade is not a claim.
        Some(area) if area > STANDING_MATERIAL_NARRATION_FLOOR_MM2 => {
            output.push_str(&format!(
                "Standing material: {area:.0} mm² left UNCUT inside the machining region — \
                 the ring cascade hit its ring cap before the offsets collapsed, so the \
                 part will carry a raised island. {STANDING_MATERIAL_DOMAIN}; \
                 {STANDING_MATERIAL_STAGE}; {STANDING_MATERIAL_RESOLUTION}. \
                 Report-only — no gate consumes this.\n"
            ));
        }
        Some(area) if area.is_finite() => {
            output.push_str(&format!(
                "Standing material: none — {area:.0} mm² measured, the ring cascade collapsed \
                 normally. {STANDING_MATERIAL_DOMAIN}; {STANDING_MATERIAL_STAGE}.\n"
            ));
        }
        _ => {
            output.push_str(
                "Standing material: not measured — no ring cascade ran for this toolpath \
                 (or the caller supplied no generation stats), so no XY-projected \
                 generation-time residual exists. Absence of a number is not a zero.\n",
            );
        }
    }
}

/// Wave D1: one line, always, about bands that height resolution erased.
///
/// The three states are deliberately distinct. `Some` is a defect report.
/// `None` on a banded operation is a measured-clean statement. `None` on
/// anything else is "this question does not apply here" — and saying so is
/// the point: a reader must never infer "clean" from a line that is absent
/// because nothing looked.
fn append_dropped_band(output: &mut String, context: &ToolpathNarrationContext<'_>) {
    use crate::compute::catalog::OperationType;
    let plans_bands = matches!(context.operation_kind, Some(OperationType::UnifiedFinish));
    match context.dropped_band {
        Some(f) => {
            output.push_str(&format!(
                "Unmachined band: {area:.1} mm² across {count} planned \
                 region(s) — the {band} band emitted NO cutting because the \
                 resolved {clip} = {clip_z:.3} mm clipped its Z range away. \
                 That feature will be left standing at full stock. Pin \
                 {clip} to the real depth of the feature. [{provenance}. \
                 Report-only — no gate consumes this.]\n",
                area = f.area_mm2,
                count = f.region_count,
                band = f.band_label,
                clip = f.clip_label,
                clip_z = f.clip_z_mm,
                provenance = f.provenance.describe(),
            ));
        }
        None if plans_bands => {
            output.push_str(
                "Unmachined band: none — every planned finish band still cut \
                 after height resolution.\n",
            );
        }
        None => {
            output.push_str(
                "Unmachined band: not measured — this operation plans no \
                 finish bands, so no band could be dropped by height \
                 resolution. Absence of a number is not a zero.\n",
            );
        }
    }
}

/// C8: one line, always, about material a ramp descent knowingly left.
///
/// PR-8b gave the clamp a typed finding and a diagnostic but no narration
/// line, so the agent-facing report — the surface an MCP session actually
/// reads — never mentioned it. The line leads with the AREA, because a
/// worst-case lift DEPTH with no extent could mean one stray point or half
/// the part and nothing said which.
fn append_ramp_reach_clamp(output: &mut String, context: &ToolpathNarrationContext<'_>) {
    use crate::compute::catalog::OperationType;
    let ramps = matches!(context.operation_kind, Some(OperationType::RampFinish));
    match context.ramp_reach_clamp {
        Some(f) if !f.is_inert() => {
            let area = match f.lifted_area() {
                Some((area, provenance)) => format!(
                    "{:.1} mm² of ramp swath ({})",
                    area.mm2(),
                    provenance.describe()
                ),
                None => "an unmeasured extent".to_owned(),
            };
            output.push_str(&format!(
                "Ramp reach clamp: {area} left standing — {clamped} of \
                 {total} ramp points raised by up to {lift:.3} mm, and the \
                 descent bottom lifted from {requested:.3} to {holdable:.3} \
                 mm. The emitted pass is SAFE; the material is simply left, \
                 and no simulation can see it. [Report-only — no gate \
                 consumes this.]\n",
                clamped = f.clamped_points,
                total = f.ramp_points,
                lift = f.max_lift_mm,
                requested = f.requested_bottom_z_mm,
                holdable = f.holdable_bottom_z_mm,
            ));
        }
        Some(_) => {
            output.push_str(
                "Ramp reach clamp: none — a ramp descent ran and every \
                 commanded depth was holdable.\n",
            );
        }
        None if ramps => {
            output.push_str(
                "Ramp reach clamp: not measured — this ramp-finish run built \
                 no ramp path. Absence of a number is not a zero.\n",
            );
        }
        None => {
            output.push_str(
                "Ramp reach clamp: not measured — this operation runs no ramp \
                 descent, so nothing could be clamped. Absence of a number is \
                 not a zero.\n",
            );
        }
    }
}

/// C8: one line, always, about a finish band that was only PARTLY machined.
///
/// The quiet sibling of [`append_dropped_band`], and it follows exactly the
/// same three-branch contract: a finding, a measured-clean statement on an
/// operation that plans bands, and an explicit "not measured" on one that
/// does not. Absence of a number is never a zero.
fn append_clipped_band(output: &mut String, context: &ToolpathNarrationContext<'_>) {
    use crate::compute::catalog::OperationType;
    let plans_bands = matches!(context.operation_kind, Some(OperationType::UnifiedFinish));
    match context.clipped_band {
        Some(f) => {
            output.push_str(&format!(
                "Partly machined band: {area:.1} mm² across {count} planned \
                 region(s) — the {band} band cut only part of its depth. The \
                 resolved {clip} = {clip_z:.3} mm shortened its ladder from \
                 {req_lo:.3}..{req_hi:.3} mm to {del_lo:.3}..{del_hi:.3} mm \
                 ({planned} levels planned, {resolved} laddered), leaving up \
                 to {lost:.3} mm of the feature unfinished. [{provenance}. \
                 Report-only — no gate consumes this.]\n",
                area = f.area_mm2,
                count = f.region_count,
                band = f.band_label,
                clip = f.clip_label,
                clip_z = f.clip_z_mm,
                req_lo = f.requested_bottom_z_mm,
                req_hi = f.requested_top_z_mm,
                del_lo = f.delivered_bottom_z_mm,
                del_hi = f.delivered_top_z_mm,
                planned = f.planned_levels,
                resolved = f.resolved_levels,
                lost = f.max_lost_height_mm,
                provenance = f.provenance.describe(),
            ));
        }
        None if plans_bands => {
            output.push_str(
                "Partly machined band: none — every planned finish band \
                 laddered its whole Z range.\n",
            );
        }
        None => {
            output.push_str(
                "Partly machined band: not measured — this operation plans no \
                 finish bands, so no band Z ladder could be shortened. \
                 Absence of a number is not a zero.\n",
            );
        }
    }
}

/// Wave D1: one line, always, about material the cutter physically could not
/// reach on a valley centreline.
fn append_tip_float(
    output: &mut String,
    measured: Option<crate::compute::config::TipFloatFinding>,
) {
    use crate::compute::config::{
        TIP_FLOAT_DOMAIN, TIP_FLOAT_RESOLUTION, TIP_FLOAT_STAGE, TIP_FLOAT_THRESHOLD_MM,
    };
    match measured {
        Some(f) if f.floating_points > 0 => {
            let pct = 100.0 * f.floating_fraction().unwrap_or(0.0);
            output.push_str(&format!(
                "Tip float: {floating} of {total} centreline points ({pct:.0}%) \
                 sit over material the tool CANNOT reach — it wedges on the \
                 valley walls and rides above the floor. Worst residual \
                 {max:.3} mm left uncut beneath the emitted line (float > \
                 {threshold} mm counts). A smaller tip, or handing these \
                 valleys to a finer tool, is the only fix — the pass as \
                 emitted cannot remove it. {TIP_FLOAT_DOMAIN}; \
                 {TIP_FLOAT_STAGE}; {TIP_FLOAT_RESOLUTION}. Report-only — no \
                 gate consumes this.\n",
                floating = f.floating_points,
                total = f.centreline_points,
                max = f.max_float_mm,
                threshold = TIP_FLOAT_THRESHOLD_MM,
            ));
        }
        Some(f) if f.centreline_points > 0 => {
            output.push_str(&format!(
                "Tip float: none — {total} centreline points measured and the \
                 tool reached the traced valley floor on every one. \
                 {TIP_FLOAT_DOMAIN}; {TIP_FLOAT_STAGE}.\n",
                total = f.centreline_points,
            ));
        }
        Some(_) => {
            output.push_str(
                "Tip float: nothing to measure — the detector emitted no \
                 centreline points at all.\n",
            );
        }
        None => {
            output.push_str(
                "Tip float: not measured — this operation emits no valley \
                 centrelines, so no reach residual exists to report. Absence \
                 of a number is not a zero.\n",
            );
        }
    }
}

/// A/M7 gate 1: one line, always, about retract round trips — the number
/// that actually costs finishing air. §8's lesson: air is COUNT-bound (a
/// hop pays two ~`safe_z` Z legs whatever its XY length), not
/// distance-bound, so this is the figure a reader must reach for, not the
/// `rapid_distance`(mm) total printed in the header line above.
fn append_retract_trips(
    output: &mut String,
    measured: Option<crate::compute::config::RetractTripCount>,
) {
    use crate::compute::config::{
        RETRACT_TRIP_DOMAIN, RETRACT_TRIP_RESOLUTION, RETRACT_TRIP_STAGE,
    };
    match measured {
        Some(f) => match f.in_node.zip(f.between_nodes) {
            Some((in_n, out_n)) => {
                output.push_str(&format!(
                    "Retract trips: {total} round trip(s) — {in_n} inside a \
                     routing node, {out_n} between nodes. {RETRACT_TRIP_DOMAIN}; \
                     {RETRACT_TRIP_STAGE}; {RETRACT_TRIP_RESOLUTION}. \
                     Report-only — no gate consumes this.\n",
                    total = f.total,
                ));
            }
            None => {
                output.push_str(&format!(
                    "Retract trips: {total} round trip(s) — in-node/between-nodes \
                     split not available (no trustworthy routing-node spans on \
                     this toolpath). {RETRACT_TRIP_DOMAIN}; {RETRACT_TRIP_STAGE}.\n",
                    total = f.total,
                ));
            }
        },
        None => {
            output.push_str(
                "Retract trips: not measured — this toolpath's stats were \
                 never computed from its move list. Absence of a number is \
                 not a zero.\n",
            );
        }
    }
}

fn apply_semantic_level_metrics(level: &mut ZLevelSummary, trace: &ToolpathSemanticTrace) {
    if let Some(item) = trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::DepthLevel)
        .find(|item| semantic_item_matches_z(item, level.z))
    {
        if let Some(count) = item
            .params
            .get(SemanticKey::MarchingSquaresRegions)
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok())
        {
            level.marching_square_regions = Some(count);
        }
        if let Some(areas) = item
            .params
            .get(SemanticKey::RegionAreasMm2)
            .and_then(|value| value.as_array())
        {
            level.region_areas_mm2 = areas.iter().filter_map(|value| value.as_f64()).collect();
        }
        level.dropped_micro_regions = item
            .params
            .get(SemanticKey::DroppedMicroRegionCount)
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok());
        level.perimeter_sweep_length_mm = item
            .params
            .get(SemanticKey::PerimeterSweepLengthMm)
            .and_then(|value| value.as_f64());
        level.agent_walk_cut_length_mm = item
            .params
            .get(SemanticKey::AgentWalkCutLengthMm)
            .and_then(|value| value.as_f64());
        level.residual_cleanup_cell_count = item
            .params
            .get(SemanticKey::ResidualCleanupCellCount)
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok());
    }

    if level.marching_square_regions.is_none() {
        level.marching_square_regions = Some(fallback_semantic_region_count_at_z(trace, level.z));
    }
}

fn fallback_semantic_region_count_at_z(trace: &ToolpathSemanticTrace, z: f64) -> usize {
    let region_ids: BTreeSet<_> = trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::DepthLevel)
        .filter(|item| semantic_item_matches_z(item, z))
        .filter_map(|item| item.parent_id)
        .filter(|parent_id| {
            trace
                .items
                .iter()
                .any(|item| item.id == *parent_id && item.kind == ToolpathSemanticKind::Region)
        })
        .collect();
    if !region_ids.is_empty() {
        return region_ids.len();
    }

    trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::Region)
        .filter(|item| semantic_item_matches_z(item, z))
        .count()
}

fn semantic_item_matches_z(item: &crate::semantic_trace::ToolpathSemanticItem, z: f64) -> bool {
    if let Some(z_level) = item
        .params
        .get(SemanticKey::ZLevel)
        .and_then(|value| value.as_f64())
    {
        return (z - z_level).abs() <= Z_EPSILON_MM;
    }

    match (item.z_min, item.z_max) {
        (Some(min_z), Some(max_z)) => z >= min_z - Z_EPSILON_MM && z <= max_z + Z_EPSILON_MM,
        (Some(min_z), None) => (z - min_z).abs() <= Z_EPSILON_MM,
        (None, Some(max_z)) => (z - max_z).abs() <= Z_EPSILON_MM,
        (None, None) => false,
    }
}

fn append_z_level_lines(
    output: &mut String,
    levels: &[ZLevelSummary],
    debug_trace: Option<&ToolpathDebugTrace>,
) {
    for (idx, level) in levels.iter().enumerate() {
        if levels.len() > MAX_LEVEL_LINES && idx == 5 {
            output.push_str(&format!(
                "  … {} intermediate Z levels compressed (similar inferred structure). \
                 For per-Z planner gate / floor-cell / emission counters on suppressed \
                 levels, query get_generation_debug_trace(span_kind=\"z_level_clear\").\n",
                levels.len().saturating_sub(7)
            ));
            continue;
        }
        if levels.len() > MAX_LEVEL_LINES && idx > 5 && idx < levels.len().saturating_sub(2) {
            continue;
        }
        let pass_label = if idx == 0 {
            "1st pass".to_owned()
        } else if idx + 1 == levels.len() {
            "last pass".to_owned()
        } else {
            format!("pass {}", idx + 1)
        };
        let region_text = level.marching_square_regions.map_or_else(
            || "marching-squares regions unknown".to_owned(),
            |count| format!("{count} marching-squares region(s)"),
        );
        output.push_str(&format!(
            "  z={:.3} ({pass_label}): {} cut run(s), {region_text}, {} cutting moves, {} arcs. ",
            level.z,
            level.cut_run_ids.len(),
            level.cutting_moves,
            level.arc_moves
        ));
        output.push_str(&format!(
            "perimeter sweep estimate: radius {:.1}mm from centroid; {:.0}mm cutting at this Z",
            level.max_radius_from_centroid_mm, level.cutting_distance_mm
        ));
        if let Some(length) = level.perimeter_sweep_length_mm
            && length > 0.0
        {
            output.push_str(&format!(", true perimeter sweep {:.0}mm", length));
        }
        if let Some(length) = level.agent_walk_cut_length_mm
            && length > 0.0
        {
            output.push_str(&format!(", agent walk {:.0}mm", length));
        }
        if let Some(dropped) = level.dropped_micro_regions
            && dropped > 0
        {
            output.push_str(&format!(", dropped {dropped} sub-tool region(s)"));
        }
        if !level.region_areas_mm2.is_empty() {
            let areas: Vec<_> = level
                .region_areas_mm2
                .iter()
                .take(3)
                .map(|area| format!("{area:.0}"))
                .collect();
            output.push_str(&format!(", top areas [{}] mm²", areas.join(", ")));
        }
        if let Some(cells) = level.residual_cleanup_cell_count
            && cells > 0
        {
            output.push_str(&format!(", residual cleanup {cells} cell(s)"));
        }
        output.push('.');
        if let Some(debug) = debug_trace
            && let Some(suffix) = z_level_debug_suffix(debug, level.z)
        {
            output.push(' ');
            output.push_str(&suffix);
        }
        output.push('\n');
    }
}

/// Build a one-line diagnostic suffix from the matching `z_level_clear`
/// debug span for this Z level. Returns None when no matching span
/// exists (debug trace was empty, or the span's z_level didn't match).
///
/// Surfaces the planner-side gate readings + planner emission counts so
/// the agent can spot levels where the planner emitted cuts but
/// removed nothing (planner↔sim mismatch) or where the surface sits
/// above the cut plane everywhere (gate-bypass / surface-above bug).
fn z_level_debug_suffix(debug: &ToolpathDebugTrace, z: f64) -> Option<String> {
    let span = debug
        .spans
        .iter()
        .filter(|s| s.kind == "z_level_clear")
        .filter(|s| {
            s.z_level
                .map(|sz| (sz - z).abs() <= Z_EPSILON_MM)
                .unwrap_or(false)
        })
        .min_by(|a, b| {
            let da = (a.z_level.unwrap_or(f64::INFINITY) - z).abs();
            let db = (b.z_level.unwrap_or(f64::INFINITY) - z).abs();
            da.partial_cmp(&db).unwrap_or(Ordering::Equal)
        })?;
    let c = &span.counters;
    let get = |k: &str| c.get(k).copied();
    let remaining_pre = get("material_remaining_pre")?;
    let remaining_post = get("material_remaining_post").unwrap_or(remaining_pre);
    let cells_total = get("floor_cells_total")? as u64;
    let cells_at_z = get("floor_cells_at_z")? as u64;
    let cells_with_material = get("floor_cells_with_material")? as u64;
    let cut_segs = get("planner_cut_segments")? as u64;
    let rapid_segs = get("planner_rapid_segments")? as u64;
    let cut_mm = get("planner_cut_mm")?;
    let cut_path_points = get("planner_cut_path_points").unwrap_or(0.0) as u64;
    let mut tags = Vec::new();
    if cells_at_z == 0 && cells_total > 0 {
        tags.push("surface above plane".to_owned());
    }
    if cells_with_material == 0 && cut_segs > 0 {
        tags.push("planner emitted cuts but no material to cut".to_owned());
    }
    let tag_str = if tags.is_empty() {
        String::new()
    } else {
        format!(" ⚠ {}", tags.join("; "))
    };
    Some(format!(
        "[gate: pre {:.3} → post {:.3}, floor {}/{} at-plane ({} with material); planner: {} cut ({:.0}mm, {} stamps) / {} rapid{}]",
        remaining_pre,
        remaining_post,
        cells_at_z,
        cells_total,
        cells_with_material,
        cut_segs,
        cut_mm,
        cut_path_points,
        rapid_segs,
        tag_str,
    ))
}

fn collect_anomalies(
    toolpath: &Toolpath,
    cut_trace: Option<&SimulationCutTrace>,
    tool: &ToolDefinition,
    context: &ToolpathNarrationContext<'_>,
) -> Vec<String> {
    let mut anomalies = Vec::new();
    append_large_arc_anomalies(&mut anomalies, toolpath, tool);
    if let Some(trace) = cut_trace {
        append_peak_doc_anomaly(&mut anomalies, toolpath, trace, context);
        append_air_cut_anomaly(&mut anomalies, trace, context);
    } else {
        anomalies.push("ℹ no simulation cut trace available, so axial DOC and air-cut heuristics were skipped.".to_owned());
    }
    anomalies
}

fn append_large_arc_anomalies(
    anomalies: &mut Vec<String>,
    toolpath: &Toolpath,
    tool: &ToolDefinition,
) {
    // ENVELOPE, matching `arcfit`'s cap exactly — see the multiplier's doc.
    let threshold = (tool.envelope_radius_mm() * LARGE_ARC_RADIUS_MULTIPLIER).max(0.001);
    let large_arcs: Vec<_> = arc_observations(toolpath)
        .into_iter()
        .filter(|arc| arc.radius_mm > threshold)
        .collect();
    if large_arcs.is_empty() {
        return;
    }

    let min_radius = large_arcs
        .iter()
        .map(|arc| arc.radius_mm)
        .fold(f64::INFINITY, f64::min);
    let max_radius = large_arcs
        .iter()
        .map(|arc| arc.radius_mm)
        .fold(0.0, f64::max);
    if let Some(first) = large_arcs.first() {
        let direction = if first.clockwise { "CW" } else { "CCW" };
        anomalies.push(format!(
            "⚠ {} perimeter sweep arc(s) with R > envelope_radius × {:.0} (smallest {:.1}mm, largest {:.1}mm). First: move {}, {direction}, z={:.3}, center=({:.1}, {:.1}), target=({:.1}, {:.1}). Suspiciously large arcs can indicate circumscribing-circle arc-fit after path simplification.",
            large_arcs.len(),
            LARGE_ARC_RADIUS_MULTIPLIER,
            min_radius,
            max_radius,
            first.move_index,
            first.z,
            first.center_x,
            first.center_y,
            first.target_x,
            first.target_y
        ));
    }
}

fn arc_observations(toolpath: &Toolpath) -> Vec<ArcObservation> {
    let mut arcs = Vec::new();
    let mut previous: Option<P3> = None;
    for (move_index, mv) in toolpath.moves.iter().enumerate() {
        if let Some(start) = previous {
            match mv.move_type {
                MoveType::ArcCW { i, j, .. } => arcs.push(ArcObservation {
                    move_index,
                    z: mv.target.z,
                    radius_mm: (i * i + j * j).sqrt(),
                    center_x: start.x + i,
                    center_y: start.y + j,
                    target_x: mv.target.x,
                    target_y: mv.target.y,
                    clockwise: true,
                }),
                MoveType::ArcCCW { i, j, .. } => arcs.push(ArcObservation {
                    move_index,
                    z: mv.target.z,
                    radius_mm: (i * i + j * j).sqrt(),
                    center_x: start.x + i,
                    center_y: start.y + j,
                    target_x: mv.target.x,
                    target_y: mv.target.y,
                    clockwise: false,
                }),
                MoveType::Rapid | MoveType::Linear { .. } => {}
            }
        }
        previous = Some(mv.target);
    }
    arcs
}

fn append_peak_doc_anomaly(
    anomalies: &mut Vec<String>,
    toolpath: &Toolpath,
    trace: &SimulationCutTrace,
    context: &ToolpathNarrationContext<'_>,
) {
    // Drill cycles cut on Z-only moves; the per-segment cutting-sample
    // stream is empty by design (analytical removal — see Step 3 PR1).
    // Peak axial DOC is meaningless here; the drill block in
    // `append_air_cut_anomaly` surfaces the drill-native metrics instead.
    if context.is_drill_cycle {
        return;
    }
    let peak = trace
        .samples
        .iter()
        .filter(|sample| sample.is_cutting)
        .filter(|sample| {
            context
                .toolpath_id
                .is_none_or(|id| sample.toolpath_id == id)
        })
        .max_by(|a, b| {
            a.axial_doc_mm
                .partial_cmp(&b.axial_doc_mm)
                .unwrap_or(Ordering::Equal)
        });

    let Some(sample) = peak else {
        anomalies.push("ℹ cut trace contains no cutting samples for this toolpath.".to_owned());
        return;
    };

    let [x, y, z] = sample.position;
    let move_kind = toolpath
        .moves
        .get(sample.move_index)
        .map(|mv| move_type_label(mv.move_type))
        .unwrap_or("unknown move");
    let threshold_text = context.depth_per_pass_mm.map_or_else(
        || {
            use crate::compute::catalog::OperationType;
            match context.operation_kind {
                Some(OperationType::DropCutter) => {
                    "this op follows surface heights — no commanded DOC".to_owned()
                }
                Some(OperationType::ProjectCurve) => {
                    "this op follows the curve at a fixed surface offset — no commanded DOC"
                        .to_owned()
                }
                Some(OperationType::Drill | OperationType::AlignmentPinDrill) => {
                    "this op advances by peck depth — no continuous DOC".to_owned()
                }
                Some(OperationType::VCarve) => {
                    "this op cuts to a target V-bit depth — no commanded DOC".to_owned()
                }
                // Generic phrasing for every other kind (and for callers
                // that did not supply a kind).
                _ => "commanded depth_per_pass is unknown".to_owned(),
            }
        },
        |depth| format!("commanded depth_per_pass = {:.2}mm", depth),
    );
    let severity = context.depth_per_pass_mm.map_or("ℹ", |depth| {
        if sample.axial_doc_mm > depth * DEEP_DOC_MULTIPLIER {
            "⚠"
        } else {
            "ℹ"
        }
    });
    anomalies.push(format!(
        "{severity} peak axial DOC {:.2}mm at sample {} (move {}, {move_kind}, z={:.3}, position ({:.1}, {:.1})). {threshold_text}. Large DOC spikes often point to arc-fit overshoot, lift-function bridging, or an uncleared-stock edge case.",
        sample.axial_doc_mm,
        sample.sample_index,
        sample.move_index,
        z,
        x,
        y
    ));
}

fn append_air_cut_anomaly(
    anomalies: &mut Vec<String>,
    trace: &SimulationCutTrace,
    context: &ToolpathNarrationContext<'_>,
) {
    // Drill TPs are marked `metrics_not_applicable` (Step 3 PR2) so
    // they're absent from `toolpath_summaries` — `cut_summary_metrics`
    // returns None for them. Handle the drill case BEFORE that early
    // return so the drill-native block surfaces regardless of whether
    // the milling-side per-toolpath summary exists.
    if context.is_drill_cycle {
        // F4 — engagement is XY-only (cylinder side-engagement). Drill
        // cycles cut on Z-only moves so they always read 100 % air-cut
        // and 0 engagement; that's a model limitation, not an actual
        // anomaly, and surfacing it as ⚠ trains the operator to ignore
        // legitimate warnings on milling ops.
        //
        // §6.E / Step 3 PR2: when a `DrillToolpathSummary` is available
        // (post-PR1, every drill op produces one), surface peck pattern
        // adequacy + chip-welding risk + cycle time + plunge-feed
        // envelope so operators get drill-relevant signal instead of
        // just "metrics N/A".
        let drill_summary = context
            .toolpath_id
            .and_then(|id| trace.drill_summary_for(id));
        if let Some(d) = drill_summary {
            let risk = match d.chip_welding_risk {
                crate::drill_metrics::ChipWeldingRisk::Low => "low",
                crate::drill_metrics::ChipWeldingRisk::Elevated => "elevated",
                crate::drill_metrics::ChipWeldingRisk::High => "high",
            };
            let cycle_time_s = d.feed_time_s + d.dwell_time_s;
            anomalies.push(format!(
                "ℹ drill cycle — engagement / air-cut% are not modeled. {} hole(s), {} peck(s), deepest hole {:.2} mm, depth-to-diameter {:.1}× total / {:.1}× evacuation-credited (chip-welding risk {}), peck pattern {}.",
                d.hole_count,
                d.peck_count,
                d.deepest_hole_mm,
                d.max_depth_to_diameter,
                d.chip_welding_dtd,
                risk,
                if d.peck_pattern_adequate { "adequate" } else { "INADEQUATE — reduce peck depth" },
            ));
            // Cycle-time + chip-evacuation breakdown (one line, all
            // drill-natural metrics — no engagement or air-cut here).
            anomalies.push(format!(
                "ℹ drill cycle time: {:.1}s feed-down + {:.1}s dwell = {:.1}s; mean chip-evacuation score {:.2} (0=trapped, 1=cleared).",
                d.feed_time_s,
                d.dwell_time_s,
                cycle_time_s,
                d.avg_chip_evacuation_score,
            ));
            // Plunge-feed envelope check (drill-specific gate). Reports
            // feed/diameter (1/min) and the material's safe band when
            // both feed and diameter are present; out-of-band readings
            // become ⚠ markers.
            if let (Some(feed), Some(dia)) = (context.feed_rate_mm_min, context.tool_diameter_mm)
                && dia > 0.0
            {
                let ratio = feed / dia;
                let (mark, envelope_label) = if let Some(material) = context.material {
                    // Single-source the band check through the gate's
                    // classifier (F1) — narrate only formats the outcome.
                    let outcome =
                        crate::tool_load::drill_gates::classify_plunge_feed(feed, dia, material);
                    let (lo, hi) = crate::tool_load::drill_gates::plunge_feed_envelope(material);
                    let mark = if outcome.is_exceeded() { "⚠" } else { "ℹ" };
                    (
                        mark,
                        format!(" (material envelope {:.0}–{:.0} mm/min per mm Ø)", lo, hi),
                    )
                } else {
                    ("ℹ", String::new())
                };
                anomalies.push(format!(
                    "{mark} plunge intensity: {:.0} mm/min per mm Ø{envelope_label} — feed {:.0} mm/min ÷ Ø {:.2} mm.",
                    ratio, feed, dia,
                ));
            }
        } else {
            anomalies.push(
                "ℹ engagement and air-cut% are not modeled for drill cycles (the dexel uses XY cylinder side-engagement; drill chips on Z-only moves). Treat MRR / feed / power separately for drilling."
                    .to_owned(),
            );
        }
        return;
    }
    // Milling-side path: pull the per-toolpath cutting / air-cut / engagement
    // numbers; bail out cleanly if no summary exists (e.g. zero cutting time).
    //
    // LH-1: air cut has two denominators and both ship. This line reports the
    // CUTTING-time reading (what it has always reported, and what CLAUDE.md's
    // metric caveats describe) and now prints the TOTAL-runtime reading beside
    // it - the one the GUI banner and every `air_cut_high_threshold_pct` band
    // are tuned against. Naming them is the whole fix: neither number moved.
    let Some((air_pct_of_cutting, air_pct_of_total, cutting_time_s, average_engagement)) =
        cut_summary_metrics(trace, context)
    else {
        return;
    };
    if cutting_time_s <= 0.0 {
        return;
    }
    let marker = if air_pct_of_cutting > AIR_CUT_WARNING_PERCENT {
        "⚠"
    } else {
        "ℹ"
    };
    let hint = {
        use crate::compute::catalog::OperationType;
        match context.operation_kind {
            Some(OperationType::Adaptive3d | OperationType::Adaptive | OperationType::Rest) => {
                " High values on roughing ops usually mean boundary/stepover tuning or stale remaining-stock assumptions."
            }
            Some(
                OperationType::DropCutter
                | OperationType::Scallop
                | OperationType::Waterline
                | OperationType::Pencil
                | OperationType::SteepShallow
                | OperationType::RampFinish
                | OperationType::SpiralFinish
                | OperationType::RadialFinish
                | OperationType::HorizontalFinish,
            ) => {
                " For finishing ops, air-cut% is dominated by surface terrain — relative comparison across runs is more useful than the absolute number."
            }
            Some(OperationType::ProjectCurve | OperationType::VCarve | OperationType::Trace) => {
                " Curve-following ops cut along a single path; air-cut% mostly reflects rapids and approach segments rather than wasted cutting."
            }
            // No specialised hint for the remaining kinds (Face, Pocket,
            // Profile, Zigzag, Inlay, Chamfer, Drill, AlignmentPinDrill)
            // or for callers without a kind.
            _ => "",
        }
    };
    anomalies.push(format!(
        "{marker} {:.1}% of CUTTING time is air-cut ({:.1}% of TOTAL runtime, the measure \
         the GUI banner and the per-operation thresholds use); average engagement {:.3}.{}",
        air_pct_of_cutting, air_pct_of_total, average_engagement, hint
    ));
}

/// `(air-cut % of CUTTING time, air-cut % of TOTAL runtime, cutting seconds,
/// average engagement)` for the narrated toolpath, or the whole trace when no
/// toolpath is pinned. Both percentages are returned because both ship under
/// the name "air cut %" (`MEASUREMENT_DOMAINS.md` LH-1); a caller that takes
/// only one must say which in its output.
fn cut_summary_metrics(
    trace: &SimulationCutTrace,
    context: &ToolpathNarrationContext<'_>,
) -> Option<(f64, f64, f64, f64)> {
    use crate::simulation_cut::AirCutRatios;
    if let Some(id) = context.toolpath_id {
        return trace
            .toolpath_summaries
            .iter()
            .find(|summary| summary.toolpath_id == id)
            .map(|summary| {
                (
                    summary.air_cut_pct_of_cutting_time(),
                    summary.air_cut_pct_of_total_runtime(),
                    summary.cutting_runtime_s,
                    summary.average_engagement,
                )
            });
    }
    Some((
        trace.summary.air_cut_pct_of_cutting_time(),
        trace.summary.air_cut_pct_of_total_runtime(),
        trace.summary.cutting_runtime_s,
        trace.summary.average_engagement,
    ))
}

fn move_type_label(move_type: MoveType) -> &'static str {
    match move_type {
        MoveType::Rapid => "Rapid",
        MoveType::Linear { .. } => "Linear",
        MoveType::ArcCW { .. } => "ArcCW",
        MoveType::ArcCCW { .. } => "ArcCCW",
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
    use crate::compute::build_cutter;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::geo::P3;
    use crate::semantic_trace::{
        ToolpathSemanticItem, ToolpathSemanticParams, ToolpathSemanticSummary,
    };
    use crate::simulation_cut::{CutKinematics, SimulationCutSample, SimulationCutTrace};
    use crate::toolpath_spans::{AnnotatedToolpath, Span, SpanKind, SpanPayload};

    /// Tooth of S2.3: when a DepthPass span legitimately covers moves whose
    /// raw Z values straddle the pass plane (e.g. an entry move at z=22.5
    /// and the cut moves at z=22.0), narration should report ONE pass at the
    /// payload `z_level`, not two phantom passes from `Z_EPSILON_MM`
    /// clustering of move Z values.
    #[test]
    fn narrate_uses_depth_pass_spans_to_avoid_phantom_z_levels() {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 25.0));
        toolpath.feed_to(P3::new(0.0, 0.0, 22.5), 1000.0); // entry above pass plane
        toolpath.feed_to(P3::new(5.0, 0.0, 22.0), 1000.0); // pass plane
        toolpath.feed_to(P3::new(10.0, 0.0, 22.0), 1000.0);

        let n = toolpath.moves.len();
        let spans = vec![
            Span::new(0, n, SpanKind::Operation),
            Span::new(0, n, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
                z_level: 22.0,
                pass_index: 0,
            }),
        ];
        let annotated = AnnotatedToolpath::with_spans(toolpath, spans);
        let tool = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));

        let report = narrate_toolpath(&annotated, None, None, None, &tool);
        // Exactly one Z-level line should appear — at the payload z=22.000.
        let z_lines: Vec<&str> = report.lines().filter(|l| l.contains("z=")).collect();
        assert_eq!(
            z_lines.len(),
            1,
            "expected one z-level line from DepthPass, got {z_lines:?}"
        );
        assert!(z_lines[0].contains("22.00"), "got: {}", z_lines[0]);
    }

    #[test]
    fn narrate_flags_large_arc_peak_doc_and_air_cut() {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 10.0));
        toolpath.feed_to(P3::new(10.0, 0.0, 2.0), 1000.0);
        toolpath.arc_cw_to(P3::new(20.0, 0.0, 2.0), 100.0, 0.0, 1000.0);

        let tool = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let trace = SimulationCutTrace::from_samples(1.0, vec![sample(ToolpathId(7), 1, 8.0, 0.0)]);
        let context = ToolpathNarrationContext {
            toolpath_id: Some(ToolpathId(7)),
            toolpath_name: Some("Back Rough"),
            operation_label: Some("adaptive3d"),
            operation_kind: Some(crate::compute::catalog::OperationType::Adaptive3d),
            depth_per_pass_mm: Some(3.0),
            stepover_mm: Some(0.84),
            tool_diameter_mm: Some(6.0),
            feed_rate_mm_min: Some(1000.0),
            spindle_rpm: Some(18_000),
            flute_count: Some(2),
            is_drill_cycle: false,
            material: None,
            // Adaptive3d runs no ring cascade: not measured.
            standing_material_mm2: None,
            // Nor bands, nor centrelines (Wave D1): not measured either.
            dropped_band: None,
            clipped_band: None,
            ramp_reach_clamp: None,
            tip_float: None,
            retract_trips: None,
        };

        let report = narrate_toolpath_with_context(
            &AnnotatedToolpath::new(toolpath),
            None,
            Some(&trace),
            None,
            &tool,
            &context,
        );

        assert!(report.contains("perimeter sweep"));
        assert!(report.contains("axial DOC"));
        assert!(report.contains("Anomalies"));
        // PR-2 (H1) renamed the printed label: "tool_radius" was ambiguous
        // between the envelope and the cutting-tip scale. The threshold is
        // and stays ENVELOPE-relative (§5 / H2.6).
        assert!(report.contains("envelope_radius"));
        assert!(report.contains("Operation context"));
        assert!(report.contains("Engagement distribution"));
        // A/M9: an op with no cascade still gets a line — silence would be
        // indistinguishable from a measured zero.
        assert!(report.contains("Standing material: not measured"));
    }

    /// A/M9: the three standing-material states must READ differently.
    /// A number, a measured none, and an absent measurement are three
    /// different facts, and the one that used to be missing entirely
    /// (`Some(a)`) is the one that shipped a raised island.
    #[test]
    fn standing_material_line_distinguishes_all_three_states() {
        let mut standing = String::new();
        append_standing_material(&mut standing, Some(837.0));
        assert!(standing.contains("837 mm² left UNCUT"), "{standing}");
        assert!(standing.contains("Report-only"), "{standing}");
        assert!(
            standing.contains(crate::compute::config::STANDING_MATERIAL_DOMAIN),
            "the number must declare its domain: {standing}"
        );

        let mut clean = String::new();
        append_standing_material(&mut clean, Some(0.0));
        assert!(clean.contains("Standing material: none"), "{clean}");
        assert!(clean.contains("measured"), "{clean}");

        let mut unknown = String::new();
        append_standing_material(&mut unknown, None);
        assert!(
            unknown.contains("Standing material: not measured"),
            "{unknown}"
        );
        assert!(
            unknown.contains("Absence of a number is not a zero"),
            "the silent-zero trap must be named where a reader will hit it: {unknown}"
        );

        // A sliver below the floor is not an island — same floor as the
        // diagnostics adapter, so the two surfaces cannot disagree.
        let mut sliver = String::new();
        append_standing_material(&mut sliver, Some(0.5));
        assert!(sliver.contains("Standing material: none"), "{sliver}");
    }

    #[test]
    fn narrate_separates_cut_runs_from_marching_square_regions() {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 10.0));
        toolpath.feed_to(P3::new(0.0, 0.0, 2.0), 1000.0);
        toolpath.feed_to(P3::new(1.0, 0.0, 2.0), 1000.0);
        toolpath.rapid_to(P3::new(5.0, 0.0, 10.0));
        toolpath.feed_to(P3::new(5.0, 0.0, 2.0), 1000.0);
        toolpath.feed_to(P3::new(6.0, 0.0, 2.0), 1000.0);

        let tool = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));
        let semantic_trace = one_region_semantic_trace(2.0);
        let report = narrate_toolpath(
            &AnnotatedToolpath::new(toolpath),
            Some(&semantic_trace),
            None,
            None,
            &tool,
        );

        assert!(report.contains("2 cut run(s), 1 marching-squares region(s)"));
        assert!(report.contains("true perimeter sweep 123mm"));
        assert!(report.contains("agent walk 456mm"));
        assert!(!report.contains("region/run"));
    }

    #[test]
    fn narrate_appends_z_level_debug_suffix_with_warning_tags() {
        // A z_level_clear span where the planner emitted cuts but the
        // surface sat above the cut plane everywhere — the exact pattern
        // observed on wanaka's z=10/z=7 dead passes. Both warning tags
        // should fire.
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 10.0));
        toolpath.feed_to(P3::new(0.0, 0.0, 2.0), 1000.0);
        toolpath.feed_to(P3::new(1.0, 0.0, 2.0), 1000.0);

        let tool = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));

        let mut counters = std::collections::BTreeMap::new();
        counters.insert("material_remaining_pre".to_owned(), 0.886);
        counters.insert("material_remaining_post".to_owned(), 0.886);
        counters.insert("floor_cells_total".to_owned(), 1247.0);
        counters.insert("floor_cells_at_z".to_owned(), 0.0);
        counters.insert("floor_cells_surf_above".to_owned(), 1247.0);
        counters.insert("floor_cells_with_material".to_owned(), 0.0);
        counters.insert("planner_cut_segments".to_owned(), 26.0);
        counters.insert("planner_rapid_segments".to_owned(), 3.0);
        counters.insert("planner_link_segments".to_owned(), 0.0);
        counters.insert("planner_cut_mm".to_owned(), 4846.0);
        counters.insert("planner_cut_path_points".to_owned(), 1234.0);

        let span = crate::debug_trace::ToolpathDebugSpan {
            id: 1,
            parent_id: None,
            kind: "z_level_clear".to_owned(),
            label: "Z 2.000 (1/1)".to_owned(),
            start_us: 0,
            elapsed_us: 1000,
            xy_bbox: None,
            z_level: Some(2.0),
            move_start: None,
            move_end: None,
            exit_reason: None,
            counters,
        };
        let debug_trace = crate::debug_trace::ToolpathDebugTrace {
            schema_version: crate::debug_trace::TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: "Back Rough".to_owned(),
            operation_label: "adaptive3d".to_owned(),
            summary: crate::debug_trace::ToolpathDebugSummary {
                total_elapsed_us: 1000,
                span_count: 1,
                hotspot_count: 0,
                dominant_span_kind: Some("z_level_clear".to_owned()),
                dominant_span_label: Some("Z 2.000 (1/1)".to_owned()),
                dominant_span_elapsed_us: Some(1000),
            },
            spans: vec![span],
            hotspots: vec![],
            annotations: vec![],
        };

        let report = narrate_toolpath(
            &AnnotatedToolpath::new(toolpath),
            None,
            None,
            Some(&debug_trace),
            &tool,
        );

        assert!(
            report.contains("gate: pre 0.886 → post 0.886"),
            "expected gate readings in narration; got:\n{report}"
        );
        assert!(
            report.contains("floor 0/1247 at-plane (0 with material)"),
            "expected floor histogram; got:\n{report}"
        );
        assert!(
            report.contains("planner: 26 cut (4846mm, 1234 stamps) / 3 rapid"),
            "expected planner emission counters; got:\n{report}"
        );
        assert!(
            report.contains("surface above plane"),
            "expected surface-above warning tag; got:\n{report}"
        );
        assert!(
            report.contains("planner emitted cuts but no material to cut"),
            "expected planner-vs-material mismatch warning tag; got:\n{report}"
        );
    }

    fn one_region_semantic_trace(z_level: f64) -> ToolpathSemanticTrace {
        let mut params = ToolpathSemanticParams::default();
        params.insert(SemanticKey::ZLevel, z_level);
        params.insert(SemanticKey::MarchingSquaresRegions, 1usize);
        params.insert(SemanticKey::RegionAreasMm2, vec![42.0_f64]);
        params.insert(SemanticKey::PerimeterSweepLengthMm, 123.0_f64);
        params.insert(SemanticKey::AgentWalkCutLengthMm, 456.0_f64);
        params.insert(SemanticKey::ResidualCleanupCellCount, 0usize);
        ToolpathSemanticTrace {
            schema_version: crate::debug_trace::TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: "Back Rough".to_owned(),
            operation_label: "adaptive3d".to_owned(),
            summary: ToolpathSemanticSummary {
                item_count: 2,
                move_linked_item_count: 1,
            },
            items: vec![
                ToolpathSemanticItem {
                    id: 1,
                    parent_id: None,
                    kind: ToolpathSemanticKind::Region,
                    label: "Region 1".to_owned(),
                    move_start: None,
                    move_end: None,
                    xy_bbox: None,
                    z_min: None,
                    z_max: None,
                    params: ToolpathSemanticParams::default(),
                    debug_span_id: None,
                },
                ToolpathSemanticItem {
                    id: 2,
                    parent_id: Some(1),
                    kind: ToolpathSemanticKind::DepthLevel,
                    label: "Z 2.00".to_owned(),
                    move_start: Some(1),
                    move_end: Some(5),
                    xy_bbox: None,
                    z_min: Some(z_level),
                    z_max: Some(z_level),
                    params,
                    debug_span_id: None,
                },
            ],
        }
    }

    fn sample(
        toolpath_id: ToolpathId,
        move_index: usize,
        axial_doc_mm: f64,
        radial_engagement: f64,
    ) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id,
            move_index,
            sample_index: 42,
            position: [12.0, 3.0, 2.0],
            cumulative_time_s: 1.0,
            segment_time_s: 1.0,
            is_cutting: true,
            cut_kinematics: CutKinematics::Arc,
            feed_rate_mm_min: 1000.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm,
            axial_engagement_mm: axial_doc_mm,
            arc_engagement_radians: Some(0.1),
            chipload_mm_per_tooth: 0.03,
            effective_chip_thickness_mm: Some(0.0),
            engagement: crate::simulation_cut::Engagement::with_radial_woc(radial_engagement),
            ..SimulationCutSample::test_fixture()
        }
    }
}
