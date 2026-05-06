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
    toolpath: &Toolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
    cut_trace: Option<&SimulationCutTrace>,
    debug_trace: Option<&ToolpathDebugTrace>,
    tool: &ToolDefinition,
) -> String {
    narrate_toolpath_with_context(
        toolpath,
        semantic_trace,
        cut_trace,
        debug_trace,
        tool,
        &ToolpathNarrationContext::default(),
    )
}

/// Produce a concise prose narration for a generated toolpath, using optional
/// project metadata to filter simulation samples and label the report.
pub fn narrate_toolpath_with_context(
    toolpath: &Toolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
    cut_trace: Option<&SimulationCutTrace>,
    debug_trace: Option<&ToolpathDebugTrace>,
    tool: &ToolDefinition,
    context: &ToolpathNarrationContext<'_>,
) -> String {
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

    let z_levels = summarize_z_levels(toolpath, semantic_trace);
    output.push_str("\nZ-level structure (highest to lowest, setup-local frame):\n");
    if z_levels.is_empty() {
        output.push_str("  No cutting moves found.\n");
    } else {
        append_z_level_lines(&mut output, &z_levels);
    }

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

fn summarize_z_levels(
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
            level.marching_square_regions = Some(semantic_region_count_at_z(trace, level.z));
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

fn semantic_region_count_at_z(trace: &ToolpathSemanticTrace, z: f64) -> usize {
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

fn append_z_level_lines(output: &mut String, levels: &[ZLevelSummary]) {
    for (idx, level) in levels.iter().enumerate() {
        if levels.len() > MAX_LEVEL_LINES && idx == 5 {
            output.push_str(&format!(
                "  … {} intermediate Z levels compressed (similar inferred structure).\n",
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
            "perimeter sweep estimate: radius {:.1}mm from centroid; {:.0}mm cutting at this Z.\n",
            level.max_radius_from_centroid_mm, level.cutting_distance_mm
        ));
    }
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
        };

        let report =
            narrate_toolpath_with_context(&toolpath, None, Some(&trace), None, &tool, &context);

        assert!(report.contains("perimeter sweep"));
        assert!(report.contains("axial DOC"));
        assert!(report.contains("Anomalies"));
        assert!(report.contains("tool_radius"));
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
        let report = narrate_toolpath(&toolpath, Some(&semantic_trace), None, None, &tool);

        assert!(report.contains("2 cut run(s), 1 marching-squares region(s)"));
        assert!(!report.contains("region/run"));
    }

    fn one_region_semantic_trace(z_level: f64) -> ToolpathSemanticTrace {
        let mut params = ToolpathSemanticParams::default();
        params.insert("z_level", z_level);
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
        }
    }
}
