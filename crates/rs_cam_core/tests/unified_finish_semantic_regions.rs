//! A/M8 sentry — UnifiedFinish must emit SEMANTIC region annotations, not
//! only the structural region-node spans.
//!
//! Background (`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M8): the operation carries TWO independent region systems.
//!
//! * The STRUCTURAL one — `unified_finish::unified_finish_spans` emits a
//!   `SpanKind::Region` span per routed node plus the rapid-order barriers
//!   that make the barriered TSP safe. Its `move_range` tiling was fixed in
//!   `77f2b7a` (sentry `region_node_ranges_tile_the_stitched_toolpath`).
//! * The SEMANTIC one — `ToolpathSemanticTrace`, which `narrate_toolpath`
//!   reads and reports as `regions N`.
//!
//! Only the first was ever implemented, so `narrate_toolpath` reported
//! `regions 0` for the one operation whose entire premise is mixing
//! strategies — the agent-facing diagnostic could not see the band /
//! strategy structure at all.
//!
//! These tests run the REAL production path (`execute_operation_annotated`
//! with a semantic context, exactly as `session::compute::generate_toolpath`
//! calls it) on two fast synthetic fixtures — a tapered-ball tool and a
//! ball control — and assert:
//!
//! 1. narration reports a non-zero region count carrying band + strategy
//!    labels;
//! 2. the semantic Region items reconcile with the structural region-node
//!    spans — same count, same move ranges — so the two systems cannot
//!    drift apart again.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::ResolvedHeights;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::execute::execute_operation_annotated;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::narrate::narrate_toolpath;
use rs_cam_core::semantic_trace::{
    ToolpathSemanticItem, ToolpathSemanticKind, ToolpathSemanticRecorder, ToolpathSemanticTrace,
};
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, SpanKind};
use std::sync::atomic::AtomicBool;

// ── Fixture ──────────────────────────────────────────────────────────────

/// A hemisphere is the cheapest mixed-slope surface that exercises all
/// three bands: near-flat at the pole (raster), mid-slope on the flanks
/// (scallop), near-vertical at the equator (waterline).
fn hemisphere() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(20.0, 16);
    let index = SpatialIndex::build(&mesh, 12.0);
    (mesh, index)
}

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 3.0,
        ..ToolConfig::new_default(ToolId(1), ToolType::BallNose)
    }
}

/// Tapered ball: the tool class where `radius()` (shank) and
/// `cusp_radius()` (tip) diverge, so band structure is the most sensitive.
fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 1.5,
        taper_half_angle: 10.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(2), ToolType::TaperedBallNose)
    }
}

fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        // Coarse: this is a structure/annotation gate, not a surface-quality
        // one — it must run in seconds.
        tolerance: 0.5,
        sampling: 1.0,
        scallop_height: 0.3,
        raster_stepover: 2.0,
        z_step: 2.0,
        ..UnifiedFinishConfig::default()
    })
}

struct Generated {
    annotated: AnnotatedToolpath,
    trace: ToolpathSemanticTrace,
    tool_def: rs_cam_core::tool::ToolDefinition,
}

/// Run UnifiedFinish through the SAME entry point production uses, with a
/// semantic recorder attached exactly as `session::compute` attaches one.
fn generate(tool_cfg: &ToolConfig) -> Generated {
    let (mesh, index) = hemisphere();
    let tool_def = build_cutter(tool_cfg);
    let op = unified_finish_op();

    let bbox = mesh.bbox;
    let heights = ResolvedHeights {
        clearance_z: bbox.max.z + 10.0,
        retract_z: bbox.max.z + 5.0,
        feed_z: bbox.max.z + 1.0,
        top_z: bbox.max.z,
        bottom_z: bbox.min.z,
        top_pinned: true,
        bottom_pinned: true,
    };
    let stock_bbox = BoundingBox3 {
        min: bbox.min,
        max: bbox.max,
    };

    let recorder = ToolpathSemanticRecorder::new("Unified Finish 1", "Unified Finish");
    let root = recorder.root_context();
    let op_scope = root.start_item(ToolpathSemanticKind::Operation, "Unified Finish");
    let op_ctx = op_scope.context();

    let cancel = AtomicBool::new(false);
    let annotated = execute_operation_annotated(
        &op,
        Some(&mesh),
        Some(&index),
        None,
        &tool_def,
        tool_cfg,
        &heights,
        &[],
        &stock_bbox,
        None,
        None,
        None,
        &cancel,
        None,
        Some(&op_ctx),
        None,
    )
    .expect("UnifiedFinish must generate on the hemisphere fixture");
    op_scope.finish();

    let trace = recorder.finish();
    Generated {
        annotated,
        trace,
        tool_def,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn region_items(trace: &ToolpathSemanticTrace) -> Vec<&ToolpathSemanticItem> {
    trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::Region)
        .collect()
}

/// The structural region-node spans: `SpanKind::Region` spans whose
/// `region_id` indexes `UnifiedFinishReport::region_table`. The ring spans
/// that `spans_from_labeled_events` also emits share the kind, so they are
/// separated by the semantic trace's own move ranges — see the reconcile
/// test, which compares against the node table directly.
fn structural_region_spans(annotated: &AnnotatedToolpath) -> Vec<(usize, usize, String)> {
    annotated
        .spans
        .iter()
        .filter(|span| span.kind == SpanKind::Region && !span.is_boundary())
        .map(|span| {
            (
                span.start_move,
                span.end_move,
                span.label.clone().into_owned(),
            )
        })
        .collect()
}

const STRATEGY_WORDS: [&str; 4] = ["raster", "scallop", "waterline", "pencil"];
const BAND_WORDS: [&str; 4] = ["Shallow", "MidSteep", "VerySteep", "Crease"];

fn assert_labels_carry_band_and_strategy(label: &str, tool_label: &str) {
    assert!(
        BAND_WORDS.iter().any(|word| label.contains(word)),
        "[{tool_label}] semantic region label {label:?} must name its band \
         (one of {BAND_WORDS:?}) — A/M8 acceptance gate"
    );
    assert!(
        STRATEGY_WORDS.iter().any(|word| label.contains(word)),
        "[{tool_label}] semantic region label {label:?} must name the \
         strategy that generated it (one of {STRATEGY_WORDS:?}) — A/M8 \
         acceptance gate"
    );
}

// ── Gate 1: narration reports non-zero regions with band + strategy ──────

fn assert_narration_reports_regions(tool_cfg: &ToolConfig, tool_label: &str) {
    let generated = generate(tool_cfg);
    assert!(
        !generated.annotated.toolpath.moves.is_empty(),
        "[{tool_label}] fixture must generate a toolpath"
    );

    let regions = region_items(&generated.trace);
    println!(
        "[{tool_label}] {} moves, {} semantic items, {} Region items: {:?}",
        generated.annotated.toolpath.moves.len(),
        generated.trace.items.len(),
        regions.len(),
        regions.iter().map(|item| &item.label).collect::<Vec<_>>()
    );

    assert!(
        !regions.is_empty(),
        "[{tool_label}] UnifiedFinish must emit semantic Region items — \
         `narrate_toolpath` reports `regions 0` without them, which is the \
         A/M8 defect"
    );

    for item in &regions {
        assert_labels_carry_band_and_strategy(&item.label, tool_label);
        assert!(
            item.move_start.is_some() && item.move_end.is_some(),
            "[{tool_label}] semantic region {:?} must be move-linked",
            item.label
        );
    }

    let report = narrate_toolpath(
        &generated.annotated,
        Some(&generated.trace),
        None,
        None,
        &generated.tool_def,
    );
    for line in report.lines().take(4) {
        println!("[{tool_label}] narration | {line}");
    }
    assert!(
        !report.contains("regions 0,"),
        "[{tool_label}] narration must not report `regions 0` for \
         UnifiedFinish:\n{report}"
    );
    assert!(
        report.contains(&format!("regions {}", regions.len())),
        "[{tool_label}] narration must report the emitted region count \
         ({}):\n{report}",
        regions.len()
    );
    // The band/strategy structure must be legible in the narration itself,
    // not only in the trace — this is the channel H4's mix table reads.
    for word in BAND_WORDS {
        if regions.iter().any(|item| item.label.contains(word)) {
            assert!(
                report.contains(word),
                "[{tool_label}] narration must surface band label {word:?} \
                 that the semantic trace carries:\n{report}"
            );
        }
    }
}

#[test]
fn unified_finish_narration_reports_regions_on_a_tapered_ball() {
    assert_narration_reports_regions(&tapered_ball_tool(), "tapered-ball");
}

#[test]
fn unified_finish_narration_reports_regions_on_a_ball_control() {
    assert_narration_reports_regions(&ball_tool(), "ball");
}

// ── Gate 2: the two systems agree ────────────────────────────────────────

/// Drift sentry: every semantic Region item must correspond 1:1 to a
/// structural region-node span with the SAME move range, and vice versa.
///
/// Both are built from `UnifiedFinishReport::region_table` through one
/// shared function, so a divergence here means someone added a second
/// source of truth.
fn assert_semantic_and_structural_regions_agree(tool_cfg: &ToolConfig, tool_label: &str) {
    let generated = generate(tool_cfg);
    let semantic: Vec<(usize, usize)> = region_items(&generated.trace)
        .iter()
        .map(|item| {
            (
                item.move_start.expect("move-linked"),
                // Semantic ranges are INCLUSIVE at the end; spans are
                // exclusive.
                item.move_end.expect("move-linked") + 1,
            )
        })
        .collect();

    let structural = structural_region_spans(&generated.annotated);
    // The node spans are the ones whose ranges match a semantic item; any
    // semantic item without a structural twin (or the reverse, restricted
    // to node-labelled spans) is drift.
    let node_spans: Vec<(usize, usize)> = structural
        .iter()
        .filter(|(_, _, label)| {
            // Node spans are labelled `"<Band> band"` / `"Pencil claims"`;
            // the ring spans `spans_from_labeled_events` also emits share
            // the kind but are labelled `"Ring i/n"`.
            label.ends_with(" band") || label == "Pencil claims"
        })
        .map(|(start, end, _)| (*start, *end))
        .collect();

    println!(
        "[{tool_label}] semantic regions {semantic:?} vs structural nodes \
         {node_spans:?}"
    );

    assert!(
        !node_spans.is_empty(),
        "[{tool_label}] fixture must emit structural region-node spans"
    );
    assert_eq!(
        semantic.len(),
        node_spans.len(),
        "[{tool_label}] semantic Region count must equal the structural \
         region-node span count — the two systems must not drift"
    );
    assert_eq!(
        semantic, node_spans,
        "[{tool_label}] semantic Region move ranges must equal the \
         structural region-node span ranges"
    );
}

#[test]
fn unified_finish_semantic_regions_match_structural_spans_tapered() {
    assert_semantic_and_structural_regions_agree(&tapered_ball_tool(), "tapered-ball");
}

#[test]
fn unified_finish_semantic_regions_match_structural_spans_ball() {
    assert_semantic_and_structural_regions_agree(&ball_tool(), "ball");
}
