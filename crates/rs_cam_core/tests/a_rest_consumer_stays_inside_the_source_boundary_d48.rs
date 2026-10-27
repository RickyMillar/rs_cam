//! D48 (G-RESTFRAME, `planning/adaptive_research_2026-10-23/DEFECTS.md`;
//! integration step 7.5, ported from the research branch 453d52571 onto the
//! product arm): the regions a 3D Rough publishes for a `DerivedRestRegions` consumer are
//! tool-centre positions inside the rough's own machining boundary.
//!
//! A left piece is a region outline of the planner, and the 2D planner
//! insets it by the tool radius. At a boundary the outer-band clip keeps the
//! cells a disc can reach, so the piece reaches the boundary line itself.
//! D44 published it as it was. A consumer with `Center` containment then
//! put its tool centre on the boundary line, and its tool cut up to one
//! radius past it (wanaka op 7, fix33: into the holding frame).
//!
//! Here the channel of `a_derived_rest_rough_cuts_only_the_leftovers_d44`
//! runs to the mesh edge. The rough's boundary is the model silhouette with
//! `Inside` containment, so its left piece in the channel touches the
//! boundary. The consumer is a contour-parallel rough on the derived rest
//! regions, `Center` containment, offset 0, with the same tool. Every
//! cutting move of the consumer must keep the tool disc inside the
//! silhouette.
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
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::boundary::{
    ToolContainment, effective_boundary_reported, silhouette_machining_outline,
};
use rs_cam_core::ids::ToolId;
use rs_cam_core::maps::geom_cache::cached_silhouette;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// The stepped pocket of the D44 sentries, with the channel run out to the
// mesh edge, so the silhouette (the mesh square) cuts across it.
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
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

fn mesh() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if y.abs() <= CHANNEL_HALF_WIDTH {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

/// Op 0 the rough on the silhouette (`Inside`), op 1 its consumer.
fn session() -> ProjectSession {
    let mesh = mesh();
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
    op0.rest_analysis.enabled = true;
    op0.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::ModelSilhouette,
        containment: BoundaryContainment::Inside,
        offset: 0.0,
    };
    let index0 = builder.add_toolpath(0, op0).expect("add the rough");
    let mut session = builder.build();
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
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(op1),
        }))
        .expect("add the consumer");
    session
}

fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    if let Err(e) = session.generate_toolpath(index, &cancel) {
        panic!("generating toolpath {index}: {e}");
    }
}

#[test]
fn a_rest_consumer_stays_inside_the_source_boundary() {
    let mut session = session();
    generate(&mut session, 0);
    let regions: Vec<Polygon2> = session
        .result_at(0)
        .expect("the rough's result")
        .annotated()
        .produced
        .rest_regions
        .as_deref()
        .expect("the rough states the regions it left")
        .clone();

    // The fixture is the case of the defect: the channel piece reaches the
    // boundary, so a region comes within one tool radius of the silhouette.
    let r = TOOL_DIAMETER / 2.0;
    let reach = regions
        .iter()
        .flat_map(|p| p.exterior.iter())
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(0.0_f64, f64::max);
    eprintln!(
        "rough: {} regions, farthest region point {reach:.2} mm from the centre (mesh half-width {MESH_HALF})",
        regions.len()
    );
    assert!(
        reach > MESH_HALF - r - TOOL_DIAMETER / 12.0,
        "no region reaches the boundary: the fixture does not test the defect"
    );

    generate(&mut session, 1);
    let tp = &session
        .result_at(1)
        .expect("the consumer's result")
        .annotated()
        .toolpath;
    // Op 0's machining boundary as generation resolves it: the silhouette
    // outline, and its `Inside` containment (the silhouette inset by the
    // radius) as the tool-centre polygons. The slack is the rough's own
    // tolerance.
    let silhouette = silhouette_machining_outline(&cached_silhouette(&Arc::new(mesh())))
        .expect("the silhouette outline");
    let centre = effective_boundary_reported(&silhouette, ToolContainment::Inside, r).0;
    assert!(!centre.is_empty(), "the silhouette has no inset");
    let tol = match &session.toolpath_configs()[0].operation {
        OperationConfig::Adaptive3d(c) => c.tolerance,
        _ => panic!("op 0 is not a 3D Rough"),
    };
    let mut cuts = 0usize;
    let mut outside: Vec<(f64, f64)> = Vec::new();
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, &w[1]);
        if !b.move_type.is_cutting() {
            continue;
        }
        cuts += 1;
        for q in [P2::new(a.x, a.y), P2::new(b.target.x, b.target.y)] {
            if !centre.iter().any(|c| c.contains_point_eps(&q, tol)) {
                outside.push((q.x, q.y));
            }
        }
    }
    let silhouette_half = silhouette
        .exterior
        .iter()
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(0.0_f64, f64::max);
    let worst = outside
        .iter()
        .map(|(x, y)| x.abs().max(y.abs()) - (silhouette_half - r))
        .fold(0.0_f64, f64::max);
    eprintln!(
        "consumer: {} moves, {cuts} cutting, {} ends with the disc past the boundary (silhouette half-width {silhouette_half:.2}), worst {worst:.3} mm",
        tp.moves.len(),
        outside.len()
    );
    assert!(cuts > 0, "the consumer cut nothing");
    assert!(
        outside.is_empty(),
        "the consumer's tool crosses the rough's boundary by up to {worst:.3} mm at {} move ends, first {:?}",
        outside.len(),
        outside.first()
    );
}
