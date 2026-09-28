//! G-MODELOUTLINE — a `ModelOutline` boundary confines a toolpath to the
//! closed shapes of ANOTHER model, and it refuses when it cannot.
//!
//! # The need
//!
//! The operator machines a terrain STL, and a DXF with one ring (one
//! polygon with one hole) states where the machine may cut. A 3D op on the
//! STL must take its boundary from the DXF. Before this source existed,
//! `BoundarySource::Geometry` fell to the stock rectangle in silence.
//!
//! # The fixture
//!
//! - Model A: a flat 60 x 60 mm plate mesh at z = 1. The toolpath cuts it.
//! - Model B: a ring drawing, built through `detect_containment` as the
//!   DXF import builds it. The exterior is a 40.6 mm square and the hole a
//!   16.6 mm square, both centred. The odd sizes keep raster rows off the
//!   edges.
//! - A drop-cutter raster on model A, with the boundary on model B's
//!   outline, containment `Center`.
//!
//! # What is asserted
//!
//! 1. Every CUTTING move lies inside the ring: not outside it, not in the
//!    hole. The test samples each linear segment, not only its end, because
//!    a straight segment between two ring points can cross the hole.
//! 2. Non-vacuity: the same op with the boundary off cuts in the hole and
//!    outside the ring, so assertion 1 can fail.
//! 3. A model id that names no model refuses, and the message names the id.
//! 4. A model with no closed polygon refuses, and the message names the
//!    model. Two arms: a mesh (no 2D geometry) and a drawing with only an
//!    open path.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, polygon_model, square_polygon, stock_over, toolpath_config};
use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::{Polygon2, detect_containment};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder};
use rs_cam_core::toolpath::MoveType;

const PLATE_HALF: f64 = 30.0;
const RING_OUTER_HALF: f64 = 20.3;
const RING_HOLE_HALF: f64 = 8.3;
/// The clip puts its intersection points ON the ring edge, where a strict
/// point-in-polygon test is ambiguous.
const EPS_MM: f64 = 1e-3;
/// Samples per linear segment, ends included.
const SEGMENT_SAMPLES: usize = 11;

/// The ring drawing, nested by the import's own containment pass.
fn ring() -> Polygon2 {
    let nested = detect_containment(vec![
        square_polygon(RING_OUTER_HALF),
        square_polygon(RING_HOLE_HALF),
    ]);
    assert_eq!(nested.len(), 1, "the two squares must nest as one ring");
    assert_eq!(nested[0].holes.len(), 1, "the inner square must be a hole");
    nested.into_iter().next().unwrap()
}

struct Fixture {
    session: ProjectSession,
    mesh_id: usize,
    ring_id: usize,
    open_id: usize,
}

/// One session: the plate mesh, the ring drawing, a drawing with only an
/// open path, and one drop-cutter toolpath on the plate. `source` is the
/// boundary; `None` disables it.
fn fixture(source: impl FnOnce(usize, usize, usize) -> Option<BoundarySource>) -> Fixture {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(PLATE_HALF, 5.0));
    let tool_idx = builder.add_tool(ball_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.0;
    let mesh_id = builder.add_model(mesh_model(
        height_field(PLATE_HALF, 5.0, |_, _| 1.0),
        "terrain",
    ));
    let ring_id = builder.add_model(polygon_model(vec![ring()], "machinable_edge_band"));
    let open_path = Polygon2::with_holes_closed(
        vec![P2::new(-10.0, 0.0), P2::new(0.0, 5.0), P2::new(10.0, 0.0)],
        Vec::new(),
        false,
    );
    let open_id = builder.add_model(polygon_model(vec![open_path], "rivers"));

    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let mut cfg = toolpath_config("Rough", op, tool_id, mesh_id);
    if let Some(source) = source(mesh_id, ring_id, open_id) {
        cfg.boundary = BoundaryConfig {
            enabled: true,
            source,
            containment: BoundaryContainment::Center,
            offset: 0.0,
        };
    }
    builder
        .add_toolpath(0, cfg)
        .expect("add toolpath to a fresh session");
    Fixture {
        session: builder.build(),
        mesh_id,
        ring_id,
        open_id,
    }
}

/// XY samples along every cutting move, ends included.
fn cutting_samples(session: &ProjectSession) -> Vec<P2> {
    let moves = &session
        .get_result(0)
        .expect("a generated result")
        .toolpath()
        .moves;
    let mut samples = Vec::new();
    for pair in moves.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        match to.move_type {
            MoveType::Rapid => {}
            MoveType::Linear { .. } => {
                for k in 0..SEGMENT_SAMPLES {
                    let t = k as f64 / (SEGMENT_SAMPLES - 1) as f64;
                    samples.push(P2::new(
                        from.target.x + t * (to.target.x - from.target.x),
                        from.target.y + t * (to.target.y - from.target.y),
                    ));
                }
            }
            // An arc is not sampled along its sweep; its end is.
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                samples.push(P2::new(to.target.x, to.target.y));
            }
        }
    }
    samples
}

fn generate(session: &mut ProjectSession) -> Result<(), String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn every_cutting_move_lies_inside_the_ring_g_modeloutline() {
    let mut fx = fixture(|_, ring_id, _| Some(BoundarySource::ModelOutline { model_id: ring_id }));
    generate(&mut fx.session).expect("a ring outline boundary must generate");

    let ring = ring();
    let samples = cutting_samples(&fx.session);
    assert!(
        samples.len() > 100,
        "a confined op that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let escaped: Vec<&P2> = samples
        .iter()
        .filter(|p| !ring.contains_point_eps(p, EPS_MM))
        .collect();
    assert!(
        escaped.is_empty(),
        "{} of {} cutting samples lie outside the ring or in its hole. First: {:?}",
        escaped.len(),
        samples.len(),
        escaped.first()
    );
}

/// Non-vacuity for the test above: with the boundary off the same op cuts
/// in the hole AND outside the ring.
#[test]
fn without_the_boundary_the_op_cuts_the_hole_and_the_margin_g_modeloutline() {
    let mut fx = fixture(|_, _, _| None);
    generate(&mut fx.session).expect("the unbounded op must generate");

    let samples = cutting_samples(&fx.session);
    let in_hole = samples
        .iter()
        .filter(|p| p.x.abs() < RING_HOLE_HALF - 1.0 && p.y.abs() < RING_HOLE_HALF - 1.0)
        .count();
    let outside = samples
        .iter()
        .filter(|p| p.x.abs() > RING_OUTER_HALF + 1.0 || p.y.abs() > RING_OUTER_HALF + 1.0)
        .count();
    assert!(in_hole > 0, "the unbounded op must cut in the hole region");
    assert!(outside > 0, "the unbounded op must cut outside the ring");
}

#[test]
fn a_missing_model_refuses_and_names_the_id_g_modeloutline() {
    let mut fx = fixture(|_, _, _| Some(BoundarySource::ModelOutline { model_id: 999 }));
    let err = generate(&mut fx.session).expect_err("a missing outline model must refuse");
    assert!(
        err.contains("999"),
        "the refusal must name the missing model id: {err}"
    );
    assert!(
        fx.session.get_result(0).is_none(),
        "a refused generation must cache no result"
    );
    // The ids the operator can pick are listed.
    assert!(
        err.contains(&fx.ring_id.to_string()) && err.contains("machinable_edge_band"),
        "the refusal must list the project models: {err}"
    );
}

#[test]
fn a_model_with_no_closed_polygon_refuses_and_names_it_g_modeloutline() {
    // A mesh has no 2D geometry at all.
    let mut fx = fixture(|mesh_id, _, _| Some(BoundarySource::ModelOutline { model_id: mesh_id }));
    let err = generate(&mut fx.session).expect_err("a mesh outline must refuse");
    assert!(
        err.contains("terrain") && err.contains(&fx.mesh_id.to_string()),
        "the refusal must name the model: {err}"
    );
    assert!(fx.session.get_result(0).is_none());

    // A drawing with only an open path has no closed polygon.
    let mut fx = fixture(|_, _, open_id| Some(BoundarySource::ModelOutline { model_id: open_id }));
    let err = generate(&mut fx.session).expect_err("an open-path outline must refuse");
    assert!(
        err.contains("rivers") && err.contains(&fx.open_id.to_string()),
        "the refusal must name the model: {err}"
    );
    assert!(fx.session.get_result(0).is_none());
}
