//! Geometry selection P2.4: a pick of two BREP faces cuts both faces.
//!
//! # The claim
//!
//! Before P2 the face pick went through `faces_boundary_as_polygon`, which
//! kept only the largest polygon of a multi-face union, so a second face
//! that did not touch the first was not cut (DESIGN 5.3). Since P2.3b each
//! picked face is a Face item, and `view` gives each face as its own
//! region. A Pocket that picks two flat faces at disjoint XY cuts inside
//! both, and the `selection_status` report counts two faces.
//!
//! # The fixture
//!
//! Two flat up faces, each a [`FACE_SIDE`] square at Z [`FACE_Z`]: face 0
//! at X 0..10, face 1 at X [`FACE_B_X`]..+10. The two faces have the same
//! size and do not touch, so the old union kept one of them.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_two_face_pick_keeps_both_faces_p2_4
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{pick_faces, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::ItemKind;
use rs_cam_core::session::LoadedModel;
use rs_cam_core::toolpath::MoveType;

/// The side of each face (mm).
const FACE_SIDE: f64 = 10.0;
/// The Z of both faces (mm).
const FACE_Z: f64 = 10.0;
/// The X of the left edge of face 1 (mm): a 10 mm gap after face 0.
const FACE_B_X: f64 = 20.0;
/// The tool diameter (mm): it fits each face.
const TOOL_DIAMETER: f64 = 3.0;
/// The pocket depth (mm).
const POCKET_DEPTH: f64 = 2.0;

fn up_face(x0: f64) -> FaceTessellation {
    let v = vec![
        P3::new(x0, 0.0, FACE_Z),
        P3::new(x0 + FACE_SIDE, 0.0, FACE_Z),
        P3::new(x0 + FACE_SIDE, FACE_SIDE, FACE_Z),
        P3::new(x0, FACE_SIDE, FACE_Z),
    ];
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: FACE_Z,
        },
    }
}

fn part() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![up_face(0.0), up_face(FACE_B_X)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: 0,
        name: "p2_4_two_faces".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p2_4_two_faces.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
    }
}

#[test]
fn a_two_face_pick_keeps_both_faces_p2_4() {
    let stock = StockConfig {
        x: FACE_B_X + FACE_SIDE + 10.0,
        y: FACE_SIDE + 10.0,
        z: FACE_Z + 10.0,
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: -5.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut s = single_op_session_with(
        stock,
        endmill_tool_config(TOOL_DIAMETER),
        part(),
        "two faces",
        OperationConfig::Pocket(PocketConfig {
            depth: POCKET_DEPTH,
            depth_per_pass: POCKET_DEPTH,
            ..PocketConfig::default()
        }),
        |_| {},
    );
    pick_faces(&mut s, 0, &[0, 1]).unwrap();

    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc).unwrap();
    assert_eq!(report.refusal, None, "the two-face pick refuses");
    assert_eq!(
        report.counts.get(&ItemKind::Face),
        Some(&2),
        "the status does not count both faces: {:?}",
        report.counts
    );

    let cancel = AtomicBool::new(false);
    let result = s.generate_toolpath(0, &cancel).expect("the pick generates");
    let xs: Vec<f64> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target.x)
        .collect();
    assert!(
        xs.iter().any(|x| (0.0..FACE_SIDE).contains(x)),
        "face 0 (X 0..{FACE_SIDE}) is not cut"
    );
    assert!(
        xs.iter()
            .any(|x| (FACE_B_X..FACE_B_X + FACE_SIDE).contains(x)),
        "face 1 (X {FACE_B_X}..{}) is not cut",
        FACE_B_X + FACE_SIDE
    );
}
