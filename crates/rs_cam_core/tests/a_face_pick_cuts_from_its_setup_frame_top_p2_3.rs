//! Geometry selection P2.3b: a face pick reaches the op through the
//! selection, and the auto top follows the face in the SETUP frame.
//!
//! # The claim
//!
//! A Pocket whose toolpath picks one BREP face cuts that face, and its auto
//! top is the plane Z of the face as the setup sees it (DESIGN 7.2,
//! `ResolvedSelection::face_top_z`):
//!
//! - On an identity setup, a pick of the up face at model Z
//!   [`TOP_FACE_Z`] cuts from that Z down by the pocket depth.
//! - On a `FaceUp::Bottom` setup, a pick of the down face at model Z
//!   [`BOTTOM_FACE_Z`] cuts from the Z that the flip gives that face:
//!   `stock top - BOTTOM_FACE_Z` in the zero-rooted setup frame.
//!
//! Before P2.3b the session read the face top in the MODEL frame
//! (`group.bbox.max.z`), so the flipped case cut from model Z 0 in the
//! setup frame, under the face. P2.3a noted that no sentry covered this
//! path.
//!
//! # The fixture
//!
//! A two-face part built in memory (`build_enriched_mesh`, as the P2.1
//! sentry builds one): a 10 mm square up face at Z 10 and a down face at
//! Z 0. The stock reaches 5 mm past the part above and below, so the face
//! top, the stock top and the old model-frame top are three different
//! numbers in both frames.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_face_pick_cuts_from_its_setup_frame_top_p2_3
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

use common::session::{pick_faces, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetSetupFaceArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The model Z of the up face (the top of the part).
const TOP_FACE_Z: f64 = 10.0;
/// The model Z of the down face (the bottom of the part).
const BOTTOM_FACE_Z: f64 = 0.0;
/// The side of the square part (mm).
const PART_SIDE: f64 = 10.0;
/// The stock reaches this far past the part on every side and in Z (mm).
const STOCK_MARGIN: f64 = 5.0;
/// The pocket depth (mm). One pass: `depth_per_pass` equals it.
const POCKET_DEPTH: f64 = 2.0;
/// The tool diameter (mm): it fits the 10 mm face.
const TOOL_DIAMETER: f64 = 3.0;
/// The float noise of a Z read back from the toolpath (mm).
const Z_EPS: f64 = 1e-6;

/// A flat square face at `z`, wound so that its triangles face `up`.
fn square_face(z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(0.0, 0.0, z),
        P3::new(PART_SIDE, 0.0, z),
        P3::new(PART_SIDE, PART_SIDE, z),
        P3::new(0.0, PART_SIDE, z),
    ];
    let triangles = if up {
        vec![[0, 1, 2], [0, 2, 3]]
    } else {
        vec![[0, 2, 1], [0, 3, 2]]
    };
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles,
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: z,
        },
    }
}

/// The part: face 0 is the up face at [`TOP_FACE_Z`], face 1 the down face
/// at [`BOTTOM_FACE_Z`].
fn part_model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            square_face(TOP_FACE_Z, true),
            square_face(BOTTOM_FACE_Z, false),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: 0,
        name: "p2_3_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p2_3_part.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// The stock: [`STOCK_MARGIN`] past the part on every side and in Z.
fn stock() -> StockConfig {
    StockConfig {
        x: PART_SIDE + 2.0 * STOCK_MARGIN,
        y: PART_SIDE + 2.0 * STOCK_MARGIN,
        z: TOP_FACE_Z - BOTTOM_FACE_Z + 2.0 * STOCK_MARGIN,
        origin_x: -STOCK_MARGIN,
        origin_y: -STOCK_MARGIN,
        origin_z: BOTTOM_FACE_Z - STOCK_MARGIN,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// One Pocket on the part that picks face `face`, on a setup with `face_up`.
fn session(face: u16, face_up: FaceUp) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(part_model());
    let op = OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(POCKET_DEPTH),
        depth_per_pass: POCKET_DEPTH,
        ..PocketConfig::default()
    });
    let cfg = toolpath_config("face pocket", op, tool_id, model_id);
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut session = builder.build();
    pick_faces(&mut session, 0, &[face]).unwrap();
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    session
}

/// The lowest and the highest Z of the cutting moves.
fn cut_z_range(session: &mut ProjectSession) -> (f64, f64) {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("a face pick generates");
    let zs: Vec<f64> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .collect();
    assert!(!zs.is_empty(), "the face pick cut nothing");
    (
        zs.iter().copied().fold(f64::INFINITY, f64::min),
        zs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    )
}

#[test]
fn a_face_pick_on_an_identity_setup_cuts_from_the_face() {
    let mut s = session(0, FaceUp::Top);
    let (low, _) = cut_z_range(&mut s);
    let want = TOP_FACE_Z - POCKET_DEPTH;
    assert!(
        (low - want).abs() < Z_EPS,
        "the pocket floor is {low}, want {want} (face top {TOP_FACE_Z} minus the depth)"
    );
}

#[test]
fn a_face_pick_on_a_flipped_setup_cuts_from_the_face_in_the_setup_frame() {
    let mut s = session(1, FaceUp::Bottom);
    let (low, _) = cut_z_range(&mut s);
    // The setup frame is zero-rooted at the stock bottom as the setup sees
    // it. A Bottom flip turns the world stock top into the setup-frame
    // floor, so a world Z maps to `stock top - Z`.
    let stock = stock();
    let world_stock_top = stock.origin_z + stock.z;
    let face_top = world_stock_top - BOTTOM_FACE_Z;
    let want = face_top - POCKET_DEPTH;
    assert!(
        (low - want).abs() < Z_EPS,
        "the pocket floor is {low}, want {want}: the down face at model Z \
         {BOTTOM_FACE_Z} is at setup Z {face_top} on a Bottom setup (the model-frame \
         top gave {})",
        BOTTOM_FACE_Z - POCKET_DEPTH
    );
}
