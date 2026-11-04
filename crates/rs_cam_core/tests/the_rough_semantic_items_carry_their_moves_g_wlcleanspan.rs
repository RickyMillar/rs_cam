//! G-WLCLEANSPAN (DEFECTS.md): every Region, DepthLevel and Cleanup item of
//! a 3D Rough's semantic trace carries a move range, the range of its
//! structural span.
//!
//! The items used to end at the next annotation of any kind. The first
//! entry of a level or of a cleanup is at the same move, so the range was
//! empty and the item was never bound: the narration and the span tools
//! could not place or time a level or a cleanup. Now each item binds to the
//! structural span built from the same events (`compute::spans`).
//!
//! The fixture is C4b's: one block with two pockets and a ridge between
//! them; pocket B has a shallow floor and a deep hole. Three cases: Contour
//! Parallel in Global order, Geodesic Adaptive in Global order, and Contour
//! Parallel in By Area order with "Contour each level" on (the C4b masked
//! wall cleanups).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: an expect or a panic is how a test reports a failure; the test prints its counts for the reader"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy, RegionOrdering};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_core::trace::semantic_trace::ToolpathSemanticKind;
use rs_cam_core::trace::toolpath_spans::SpanKind;
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const DEEP_Z: f64 = 1.0;
const SHALLOW_Z: f64 = 7.0;

/// `(kind, label, move_start, move_end)` of every structure item.
type Items = Vec<(ToolpathSemanticKind, String, Option<usize>, Option<usize>)>;

/// Generate the case and check every structure item against the spans.
/// Returns the structure items and the number of linked cleanups.
fn check(strategy: ClearingStrategy, order: RegionOrdering, contour: bool) -> (Items, usize) {
    let mesh = height_field(30.0, 0.5, |x, y| {
        let in_pocket = (x.abs() - 13.0).abs() <= 10.0 && y.abs() <= 15.0;
        let in_hole = (x + 13.0).abs() <= 4.0 && y.abs() <= 6.0;
        if !in_pocket {
            TOP_Z
        } else if x > 0.0 || in_hole {
            DEEP_Z
        } else {
            SHALLOW_Z
        }
    });
    let mut stock = stock_over(30.0, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(6.0));
    let tool = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "two pockets, one with a hole"));
    let op = toolpath_config(
        "rough",
        OperationConfig::Adaptive3d(Adaptive3dConfig {
            stepover: Some(2.0),
            depth_per_pass: Some(3.0),
            clearing_strategy: strategy,
            region_ordering: order,
            contour_each_level: contour,
            ..Adaptive3dConfig::default()
        }),
        tool,
        model_id,
    );
    builder.add_toolpath(0, op).expect("add the rough");
    let mut session = builder.build();
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("generate the rough");
    let result = session.result_at(0).expect("the result");
    let spans = &result.annotated().spans;
    let trace = result.semantic_trace.as_ref().expect("the semantic trace");
    let case = format!("{strategy:?} {order:?} contour {contour}");

    let mut items: Items = Vec::new();
    let mut linked_cleanups = 0usize;
    for item in &trace.items {
        let span_kind = match item.kind {
            ToolpathSemanticKind::Region => SpanKind::Region,
            ToolpathSemanticKind::DepthLevel => SpanKind::DepthPass,
            ToolpathSemanticKind::Cleanup => SpanKind::WaterlineCleanup,
            ToolpathSemanticKind::Operation
            | ToolpathSemanticKind::Pass
            | ToolpathSemanticKind::Entry
            | ToolpathSemanticKind::SlotClearing
            | ToolpathSemanticKind::ForcedClear
            | ToolpathSemanticKind::Contour
            | ToolpathSemanticKind::Raster
            | ToolpathSemanticKind::Row
            | ToolpathSemanticKind::Slice
            | ToolpathSemanticKind::Hole
            | ToolpathSemanticKind::Cycle
            | ToolpathSemanticKind::Chain
            | ToolpathSemanticKind::Band
            | ToolpathSemanticKind::Ramp
            | ToolpathSemanticKind::Ring
            | ToolpathSemanticKind::Ray
            | ToolpathSemanticKind::Curve
            | ToolpathSemanticKind::Dressup
            | ToolpathSemanticKind::FinishPass
            | ToolpathSemanticKind::OffsetPass
            | ToolpathSemanticKind::Centerline
            | ToolpathSemanticKind::BoundaryClip
            | ToolpathSemanticKind::Optimization => continue,
        };
        let (Some(start), Some(end)) = (item.move_start, item.move_end) else {
            panic!(
                "{case}: {:?} item '{}' has no move range",
                item.kind, item.label
            );
        };
        assert!(
            spans
                .iter()
                .any(|s| s.kind == span_kind && s.start_move == start && s.end_move == end + 1),
            "{case}: {:?} item '{}' covers [{start}, {}) and no {span_kind:?} span does",
            item.kind,
            item.label,
            end + 1
        );
        if item.kind == ToolpathSemanticKind::Cleanup && end >= start {
            linked_cleanups += 1;
        }
        items.push((
            item.kind.clone(),
            item.label.clone(),
            item.move_start,
            item.move_end,
        ));
    }
    eprintln!(
        "{case}: {} structure items, {linked_cleanups} linked cleanups",
        items.len()
    );
    (items, linked_cleanups)
}

#[test]
fn the_rough_semantic_items_carry_their_moves_g_wlcleanspan() {
    for (strategy, order, contour) in [
        (
            ClearingStrategy::ContourParallel,
            RegionOrdering::Global,
            false,
        ),
        (
            ClearingStrategy::GeodesicAdaptive,
            RegionOrdering::Global,
            false,
        ),
        (
            ClearingStrategy::ContourParallel,
            RegionOrdering::ByArea,
            true,
        ),
    ] {
        let (_, linked_cleanups) = check(strategy, order, contour);
        assert!(
            linked_cleanups > 0,
            "{strategy:?} {order:?}: no cleanup item covers a move: the case tests nothing"
        );
    }
}

#[test]
fn the_rough_semantic_items_are_deterministic_g_wlcleanspan() {
    let run = || {
        check(
            ClearingStrategy::ContourParallel,
            RegionOrdering::ByArea,
            true,
        )
        .0
    };
    assert_eq!(run(), run(), "two runs give different item ranges");
}
