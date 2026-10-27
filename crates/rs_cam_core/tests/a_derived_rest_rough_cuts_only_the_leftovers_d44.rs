//! D44 (`planning/adaptive_research_2026-10-23/P3_C12_derived_rest_regions.md`;
//! integration step 7.5, ported from the research branch 453d52571 onto the
//! product arm) through the session door, as the job runs it:
//! 1. with no consumer (rest analysis off), the 3D Rough states no rest
//!    regions: the side channel is as before (parity);
//! 2. with a consumer (an enabled `DerivedRestRegions` boundary names the
//!    rough, which turns its rest analysis on), the rough (on the stepped
//!    pocket of `a_rough_publishes_the_regions_it_leaves_d44`) states the
//!    regions it left (the channel below the wide pocket), and the generic
//!    rest analysis keeps
//!    them and attaches only its heatmap grid (precedence per field);
//! 3. the consumer, a contour-parallel rough with that boundary, cuts only
//!    inside the stated regions, and is not empty.
//!
//! The rough is Geodesic Adaptive (`clearing_strategy = geodesic_adaptive`).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource, StockSource};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder};
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const CHANNEL_HALF_WIDTH: f64 = 4.8;
const WIDE_HALF_X: f64 = 34.0;
const WIDE_HALF_Y: f64 = 24.0;
const WIDE_FLOOR_Z: f64 = 7.0;
const TOOL_DIAMETER: f64 = 6.0;

fn rough(strategy: ClearingStrategy) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: 2.0,
        depth_per_pass: 3.0,
        clearing_strategy: strategy,
        ..Adaptive3dConfig::default()
    })
}

/// The channel session: op 0 the rough (Geodesic Adaptive), with its rest
/// analysis `demand`; with `consumer`, op 1 a
/// contour-parallel rough whose boundary is op 0's derived rest regions.
fn session(demand: bool, consumer: bool) -> ProjectSession {
    // The stepped pocket of sentry 1: the wide shallow pocket (cut at the
    // first level) with the narrow channel below it (left at the second).
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= CHANNEL_HALF_WIDTH {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "channel"));
    let mut op0 = toolpath_config(
        "rough",
        rough(ClearingStrategy::GeodesicAdaptive),
        tool_id,
        model_id,
    );
    op0.rest_analysis.enabled = demand;
    let index0 = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
    if consumer {
        let source_toolpath_id = session.toolpath_configs()[index0].id;
        let mut op1 = toolpath_config(
            "edges, CP rest",
            rough(ClearingStrategy::ContourParallel),
            tool_id,
            model_id,
        );
        op1.stock_source = StockSource::default();
        op1.boundary = BoundaryConfig {
            enabled: true,
            source: BoundarySource::DerivedRestRegions { source_toolpath_id },
            ..BoundaryConfig::default()
        };
        let _ = session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(op1),
            }))
            .expect("add the consumer");
    }
    session
}

fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    if let Err(e) = session.generate_toolpath(index, &cancel) {
        panic!("generating toolpath {index}: {e}");
    }
}

fn channel_points() -> Vec<P2> {
    let mut pts = Vec::new();
    let mut x = -CHANNEL_HALF_LENGTH + 3.0;
    while x <= CHANNEL_HALF_LENGTH - 3.0 {
        for y in [-1.5, 0.0, 1.5] {
            pts.push(P2::new(x, y));
        }
        x += 2.0;
    }
    pts
}

#[test]
fn a_derived_rest_rough_cuts_only_the_leftovers() {
    // 1. No consumer: no rest regions on the rough's result.
    let mut alone = session(false, false);
    generate(&mut alone, 0);
    let result = alone.result_at(0).expect("the rough's result");
    assert!(
        result.annotated().produced.rest_regions.is_none(),
        "the rough states rest regions with no consumer"
    );

    // 2. With a consumer: the rough states the channel, and the generic
    // analysis keeps them and adds its grid.
    let mut pair = session(true, true);
    generate(&mut pair, 0);
    let produced = &pair
        .result_at(0)
        .expect("the rough's result")
        .annotated()
        .produced;
    let regions: Vec<Polygon2> = produced
        .rest_regions
        .as_deref()
        .expect("the rough states the regions it left")
        .clone();
    let area: f64 = regions.iter().map(|r| r.area().abs()).sum();
    eprintln!(
        "rough: {} regions, {area:.1} mm2; heatmap grid {}",
        regions.len(),
        produced.rest_grid.is_some()
    );
    let missed: Vec<P2> = channel_points()
        .into_iter()
        .filter(|p| !regions.iter().any(|r| r.contains_point(p)))
        .collect();
    assert!(
        missed.is_empty(),
        "channel points outside the regions: {missed:?}"
    );
    // The regions are the arm's leftovers, not the generic valley set: the
    // channel's tool-centre region (the channel inset by R, the region the
    // planner plans) grown by one planner cell (`planner_cell_size`: R / 6
    // at this tolerance).
    let (r, cell) = (TOOL_DIAMETER / 2.0, TOOL_DIAMETER / 12.0);
    let centre_area =
        (2.0 * (CHANNEL_HALF_LENGTH - r + cell)) * (2.0 * (CHANNEL_HALF_WIDTH - r + cell));
    assert!(
        area > 0.8 * centre_area && area < 1.2 * centre_area,
        "the stated area {area:.1} mm2 is not the channel's centre region ({centre_area:.1} mm2)"
    );
    assert!(
        produced.rest_grid.is_some(),
        "the generic analysis did not attach its grid beside the rough's regions"
    );

    // 3. The consumer cuts only inside the regions.
    generate(&mut pair, 1);
    let tp = &pair
        .result_at(1)
        .expect("the consumer's result")
        .annotated()
        .toolpath;
    let tol = 0.5;
    let mut cuts = 0usize;
    let mut outside = Vec::new();
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, &w[1]);
        if !b.move_type.is_cutting() || (b.target.x - a.x).hypot(b.target.y - a.y) < 1e-6 {
            continue;
        }
        cuts += 1;
        for q in [P2::new(a.x, a.y), P2::new(b.target.x, b.target.y)] {
            if !regions.iter().any(|r| r.contains_point_eps(&q, tol)) {
                outside.push((q.x, q.y));
            }
        }
    }
    eprintln!(
        "consumer: {} moves, {cuts} cutting, {} ends outside",
        tp.moves.len(),
        outside.len()
    );
    assert!(cuts > 0, "the consumer cut nothing");
    assert!(
        outside.is_empty(),
        "consumer cuts outside the regions: {outside:?}"
    );
}
