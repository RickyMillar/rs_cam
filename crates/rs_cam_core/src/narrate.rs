//! Agent-friendly narration for generated toolpaths and simulation cut traces.
//!
//! This module intentionally sits above the raw `ToolpathDebugTrace`,
//! `ToolpathSemanticTrace`, and `SimulationCutTrace` data.  It produces a short
//! prose report that makes spatial/metric anomalies easy for an LLM or human to
//! notice without hand-filtering large JSON dumps.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::debug_trace::ToolpathDebugTrace;
use crate::geo::P3;
use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticTrace};
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::{MoveType, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, SpanKind, SpanPayload};

const Z_EPSILON_MM: f64 = 0.05;
const LARGE_ARC_RADIUS_MULTIPLIER: f64 = 30.0;
const AIR_CUT_WARNING_PERCENT: f64 = 50.0;
const DEEP_DOC_MULTIPLIER: f64 = 1.5;
const MAX_LEVEL_LINES: usize = 8;
const MAX_ANOMALY_LINES: usize = 8;

/// Optional metadata that lets narration tie raw traces back to the project.
#[derive(Debug, Clone, Default)]
pub struct ToolpathNarrationContext<'a> {
    pub toolpath_id: Option<usize>,
    pub toolpath_name: Option<&'a str>,
    pub operation_label: Option<&'a str>,
    pub depth_per_pass_mm: Option<f64>,
    pub stepover_mm: Option<f64>,
    pub tool_diameter_mm: Option<f64>,
    pub feed_rate_mm_min: Option<f64>,
    pub spindle_rpm: Option<u32>,
    pub flute_count: Option<u32>,
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
        output.push_str(&format!(
            "Semantic trace: {} items ({} move-linked); depth levels {}, regions {}, rings {}.\n",
            trace.summary.item_count,
            trace.summary.move_linked_item_count,
            depth_items,
            region_items,
            ring_items
        ));
    } else {
        output.push_str("Semantic trace: not available; Z-level structure inferred from moves.\n");
    }

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
        let engagement = sample.radial_engagement;
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
        let mut summary = ZLevelSummary::new(z);
        let mut prior_target: Option<P3> = None;
        let mut in_cut_run = false;
        let mut active_cut_run_id = 0usize;
        let end = span.end_move.min(toolpath.moves.len());
        for (idx, mv) in toolpath
            .moves
            .iter()
            .enumerate()
            .take(end)
            .skip(span.start_move)
        {
            let _ = idx;
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
            summary.cutting_moves += 1;
            summary.cut_run_ids.insert(active_cut_run_id);
            if matches!(
                mv.move_type,
                MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            ) {
                summary.arc_moves += 1;
            }
            if let Some(prev) = prior_target {
                summary.cutting_distance_mm += (mv.target - prev).norm();
            }
            prior_target = Some(mv.target);
        }
        let (center_x, center_y, count) = cutting_centroid_at_z(toolpath, z);
        if count > 0 {
            summary.max_radius_from_centroid_mm =
                max_radius_from_centroid(toolpath, z, center_x, center_y);
        }
        if let Some(trace) = semantic_trace {
            apply_semantic_level_metrics(&mut summary, trace);
        }
        levels.push(summary);
    }
    levels.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap_or(Ordering::Equal));
    levels
}

fn representative_cut_z_in_range(toolpath: &Toolpath, start: usize, end: usize) -> Option<f64> {
    toolpath
        .moves
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .find(|(_, mv)| mv.move_type.is_cutting())
        .map(|(_, mv)| mv.target.z)
}

/// Legacy Z-level discovery: cluster cutting move Z-coordinates with
/// `Z_EPSILON_MM`. Used only when the toolpath has no DepthPass spans.
fn summarize_z_levels_from_moves(
    toolpath: &Toolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
) -> Vec<ZLevelSummary> {
    let mut levels = Vec::<ZLevelSummary>::new();
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
        let summary = find_or_create_level(&mut levels, z);
        summary.cutting_moves += 1;
        summary.cut_run_ids.insert(active_cut_run_id);
        if matches!(
            mv.move_type,
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        ) {
            summary.arc_moves += 1;
        }
        if let Some(prev) = prior_target {
            summary.cutting_distance_mm += (mv.target - prev).norm();
        }
        prior_target = Some(mv.target);
    }

    for level in &mut levels {
        let (center_x, center_y, count) = cutting_centroid_at_z(toolpath, level.z);
        if count > 0 {
            level.max_radius_from_centroid_mm =
                max_radius_from_centroid(toolpath, level.z, center_x, center_y);
        }
        if let Some(trace) = semantic_trace {
            apply_semantic_level_metrics(level, trace);
        }
    }

    levels.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap_or(Ordering::Equal));
    levels
}

fn find_or_create_level(levels: &mut Vec<ZLevelSummary>, z: f64) -> &mut ZLevelSummary {
    if let Some(pos) = levels
        .iter()
        .position(|level| (level.z - z).abs() <= Z_EPSILON_MM)
    {
        // SAFETY: `pos` came from `levels.iter().position`, so it is in bounds.
        #[allow(clippy::indexing_slicing)]
        return &mut levels[pos];
    }
    levels.push(ZLevelSummary::new(z));
    let last_index = levels.len() - 1;
    // SAFETY: we just pushed one element, so the last element exists.
    #[allow(clippy::indexing_slicing)]
    &mut levels[last_index]
}

fn cutting_centroid_at_z(toolpath: &Toolpath, z: f64) -> (f64, f64, usize) {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0usize;
    for mv in &toolpath.moves {
        if mv.move_type.is_cutting() && (mv.target.z - z).abs() <= Z_EPSILON_MM {
            sum_x += mv.target.x;
            sum_y += mv.target.y;
            count += 1;
        }
    }
    if count == 0 {
        (0.0, 0.0, 0)
    } else {
        (sum_x / count as f64, sum_y / count as f64, count)
    }
}

fn max_radius_from_centroid(toolpath: &Toolpath, z: f64, center_x: f64, center_y: f64) -> f64 {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting() && (mv.target.z - z).abs() <= Z_EPSILON_MM)
        .map(|mv| ((mv.target.x - center_x).powi(2) + (mv.target.y - center_y).powi(2)).sqrt())
        .fold(0.0, f64::max)
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
            .values
            .get("marching_squares_regions")
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok())
        {
            level.marching_square_regions = Some(count);
        }
        if let Some(areas) = item
            .params
            .values
            .get("region_areas_mm2")
            .and_then(|value| value.as_array())
        {
            level.region_areas_mm2 = areas.iter().filter_map(|value| value.as_f64()).collect();
        }
        level.dropped_micro_regions = item
            .params
            .values
            .get("dropped_micro_region_count")
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok());
        level.perimeter_sweep_length_mm = item
            .params
            .values
            .get("perimeter_sweep_length_mm")
            .and_then(|value| value.as_f64());
        level.agent_walk_cut_length_mm = item
            .params
            .values
            .get("agent_walk_cut_length_mm")
            .and_then(|value| value.as_f64());
        level.residual_cleanup_cell_count = item
            .params
            .values
            .get("residual_cleanup_cell_count")
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
        .values
        .get("z_level")
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
    let threshold = (tool.radius() * LARGE_ARC_RADIUS_MULTIPLIER).max(0.001);
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
            "⚠ {} perimeter sweep arc(s) with R > tool_radius × {:.0} (smallest {:.1}mm, largest {:.1}mm). First: move {}, {direction}, z={:.3}, center=({:.1}, {:.1}), target=({:.1}, {:.1}). Suspiciously large arcs can indicate circumscribing-circle arc-fit after path simplification.",
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
        || "commanded depth_per_pass is unknown".to_owned(),
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
    let Some((air_cut_time_s, cutting_time_s, average_engagement)) =
        cut_summary_metrics(trace, context)
    else {
        return;
    };
    if cutting_time_s <= 0.0 {
        return;
    }
    let air_pct = air_cut_time_s / cutting_time_s * 100.0;
    let marker = if air_pct > AIR_CUT_WARNING_PERCENT {
        "⚠"
    } else {
        "ℹ"
    };
    anomalies.push(format!(
        "{marker} {:.1}% of cutting time is air-cut; average engagement {:.3}. Treat this as relative for 2D/SVG ops, but high values on 3D roughing suggest boundary/stepover tuning or stale remaining-stock assumptions.",
        air_pct, average_engagement
    ));
}

fn cut_summary_metrics(
    trace: &SimulationCutTrace,
    context: &ToolpathNarrationContext<'_>,
) -> Option<(f64, f64, f64)> {
    if let Some(id) = context.toolpath_id {
        return trace
            .toolpath_summaries
            .iter()
            .find(|summary| summary.toolpath_id == id)
            .map(|summary| {
                (
                    summary.air_cut_time_s,
                    summary.cutting_runtime_s,
                    summary.average_engagement,
                )
            });
    }
    Some((
        trace.summary.air_cut_time_s,
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
            Span::new(0, n, SpanKind::DepthPass)
                .with_payload(SpanPayload::DepthPass {
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
        let trace = SimulationCutTrace::from_samples(1.0, vec![sample(7, 1, 8.0, 0.0)]);
        let context = ToolpathNarrationContext {
            toolpath_id: Some(7),
            toolpath_name: Some("Back Rough"),
            operation_label: Some("adaptive3d"),
            depth_per_pass_mm: Some(3.0),
            stepover_mm: Some(0.84),
            tool_diameter_mm: Some(6.0),
            feed_rate_mm_min: Some(1000.0),
            spindle_rpm: Some(18_000),
            flute_count: Some(2),
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
        assert!(report.contains("tool_radius"));
        assert!(report.contains("Operation context"));
        assert!(report.contains("Engagement distribution"));
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
        params.insert("z_level", z_level);
        params.insert("marching_squares_regions", 1usize);
        params.insert("region_areas_mm2", vec![42.0_f64]);
        params.insert("perimeter_sweep_length_mm", 123.0_f64);
        params.insert("agent_walk_cut_length_mm", 456.0_f64);
        params.insert("residual_cleanup_cell_count", 0usize);
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
        toolpath_id: usize,
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
            radial_engagement,
            arc_engagement_radians: Some(0.1),
            chipload_mm_per_tooth: 0.03,
            effective_chip_thickness_mm: Some(0.0),
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: None,
            span_path: Vec::new(),
        }
    }
}
