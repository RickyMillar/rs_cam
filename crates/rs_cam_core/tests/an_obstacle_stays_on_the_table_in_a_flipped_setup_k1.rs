//! Sentry (K1 review, finding 3): **an obstacle is fixed to the table, so
//! a flipped setup does not move it**.
//!
//! ## The defect this pins
//!
//! A box obstacle was read as it is, in the setup frame. An outline
//! obstacle went through `apply_to_drawing_polygons` and a model obstacle
//! through `apply_to_mesh`: both moved WITH THE PART. On a Bottom setup the
//! outline was mirrored and the mesh was turned over in Z, so the clamp
//! the operator drew was checked somewhere else.
//!
//! ## The fix this pins
//!
//! `ObstacleSet::resolve` reads all three kinds in the setup frame with no
//! setup transform. The asymmetric outline below keeps its XY, and the
//! mesh keeps its XY and its top, on a Bottom setup.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::meshes::plateau;
use common::session::{mesh_model, polygon_model, stock_over};

use rs_cam_core::compute::config::SAFE_Z_CLEARANCE_MM;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::ids::ObstacleId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddObstacleArgs, Command, Obstacle, ObstacleGeometry, ProjectSession, ProjectSessionBuilder,
};

/// An outline far from the stock centre on both axes: a mirror or a turn
/// moves it.
const OUTLINE: [f64; 4] = [2.0, 4.0, 8.0, 30.0];
/// The mesh obstacle: a 10 mm square plateau about the origin, 15 mm deep,
/// top at Z 0.
const MESH_DEPTH: f64 = 15.0;

/// A Bottom setup (index 1) with the obstacle of `kind` on it, clearance 0.
fn flipped_with(outline: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(40.0, 20.0));
    let setup = builder.add_setup("Flipped".to_owned(), FaceUp::Bottom);
    let model_id = if outline {
        builder.add_model(polygon_model(
            vec![Polygon2::rectangle(
                OUTLINE[0], OUTLINE[1], OUTLINE[2], OUTLINE[3],
            )],
            "clamp outline",
        ))
    } else {
        builder.add_model(mesh_model(plateau(10.0, MESH_DEPTH), "vise jaw"))
    };
    let geometry = if outline {
        ObstacleGeometry::OutlineExtrude {
            model_ids: vec![model_id],
            z_bottom: 0.0,
            z_top: 15.0,
        }
    } else {
        ObstacleGeometry::Model { model_id }
    };
    let mut session = builder.build();
    let _ = session
        .apply(Command::AddObstacle(AddObstacleArgs {
            setup_index: setup,
            obstacle: Box::new(Obstacle {
                id: ObstacleId::from_raw(0),
                name: "Clamp".to_owned(),
                enabled: true,
                clearance: 0.0,
                geometry,
            }),
        }))
        .expect("add the obstacle");
    session
}

fn footprint_bbox(session: &ProjectSession) -> [f64; 4] {
    let setup = &session.list_setups()[1];
    assert_eq!(setup.face_up, FaceUp::Bottom);
    let set = session
        .obstacle_set_for_setup(setup)
        .expect("the obstacle resolves");
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in set.footprints() {
        let [x0, y0, x1, y1] = p.bbox();
        b = [b[0].min(x0), b[1].min(y0), b[2].max(x1), b[3].max(y1)];
    }
    b
}

#[test]
fn an_outline_obstacle_keeps_its_xy_on_a_bottom_setup_k1() {
    let session = flipped_with(true);
    let b = footprint_bbox(&session);
    for (got, want) in b.iter().zip(OUTLINE) {
        assert!(
            (got - want).abs() < 1e-9,
            "the clamp moved with the part: {b:?}, drawn at {OUTLINE:?}"
        );
    }
}

#[test]
fn a_model_obstacle_keeps_its_xy_and_top_on_a_bottom_setup_k1() {
    let session = flipped_with(false);
    let setup = &session.list_setups()[1];
    let set = session.obstacle_set_for_setup(setup).expect("resolves");
    // The top is the mesh top (Z 0), not the turned-over bottom.
    assert_eq!(set.top_z(), Some(0.0));
    assert_eq!(set.obstacles()[0].bottom_z, -MESH_DEPTH);
    assert_eq!(
        set.plane_z(),
        Some(SAFE_Z_CLEARANCE_MM),
        "the plane is over the mesh top"
    );
    assert_eq!(set.obstacles()[0].name, "Clamp");
    // The silhouette sits about the origin, where the mesh is. It grows by
    // up to two raster cells (1 mm), never more.
    let b = footprint_bbox(&session);
    for (got, want) in b.iter().zip([-5.0, -5.0, 5.0, 5.0]) {
        assert!(
            (got - want).abs() <= 1.0 + 1e-9,
            "the vise jaw moved with the part: {b:?}"
        );
    }
}
