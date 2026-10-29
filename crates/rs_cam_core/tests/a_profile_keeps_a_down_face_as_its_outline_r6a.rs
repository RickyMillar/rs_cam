//! R6a (operator ruling 2026-10-30, live on `stepped_block.step`): "facing
//! down shouldn't really be an issue." A Profile reads a face as its RING
//! (`OperationConfig::face_feedback`), so it keeps a face that points down
//! in the setup and cuts its outline at the face plane. A fill reader (a
//! Pocket) still refuses `FaceFacesDown`: a fill needs a top to stand on.
//!
//! # The claims
//!
//! 1. The face with BREP id 1 of `stepped_block.step` is the bottom face:
//!    its anchor is (30, 20), its plane is at Z 0 and it points down on a
//!    Top setup.
//! 2. A Profile (THE draft door, then `AddToolpath`) that picks that face
//!    gives no refusal. The report holds one row: the face, by pick, at
//!    plane Z 0, in the state `Cut`, not the part outline. The report
//!    counts the face as down.
//! 3. Generation cuts the same loop at the same Z levels as the default
//!    Profile (the part outline at Z 0).
//! 4. A Pocket that picks the same face still refuses `FaceFacesDown`, in
//!    generation and in the report.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_profile_keeps_a_down_face_as_its_outline_r6a
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::session::pick_faces;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{MatchedBy, RefusalCause, Shape};
use rs_cam_core::session::{
    AddToolpathArgs, Command, ItemState, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SessionError,
};
use rs_cam_core::toolpath::MoveType;

const STEPPED: &str = "../../fixtures/gui_step/stepped_block.step";
/// The BREP id of the bottom face (60 x 40 at Z 0).
const BOTTOM_FACE: u16 = 1;
/// The model top of the stepped block (mm).
const MODEL_TOP: f64 = 30.0;
const TOOL_D: f64 = 6.0;
const EPS: f64 = 1e-6;

fn load_step() -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap()
}

/// The stock of the stepped block: X -5..65, Y -5..45, Z 0..30.
fn stepped_stock() -> StockConfig {
    StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(MODEL_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A session with the stepped block and one `op_type`, added as every add
/// door does: THE draft door, then the `AddToolpath` row.
fn session_with(op_type: OperationType) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stepped_stock());
    let _ = builder.add_tool(endmill_tool_config(TOOL_D));
    let _ = builder.add_model(load_step());
    let mut session = builder.build();
    let tool_id = session.tools()[0].id;
    let model_id = session.models()[0].id;
    let mut cfg = session
        .draft_toolpath(
            rs_cam_core::session::DraftRequest::new(op_type, 0, op_type.label().to_owned())
                .tool(tool_id)
                .model(model_id),
        )
        .expect("the tool fits the op")
        .config;
    let floor = cfg.operation.floor();
    rs_cam_core::compute::fixture_values::fill_unset(&mut cfg.operation);
    if let Some(floor) = floor {
        let _ = cfg.operation.set_floor(floor);
    }
    rs_cam_core::compute::fixture_values::fill_unset_dressups(&mut cfg.dressups);
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .expect("the add");
    session
}

/// The targets of the cutting moves (no rapids), in order.
fn cut_targets(session: &mut ProjectSession) -> Vec<P3> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("generation: {e}"));
    result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .collect()
}

/// The distance from `(x, y)` to the part box X 0..60, Y 0..40 (0 inside).
fn distance_to_part(x: f64, y: f64) -> f64 {
    let dx = (0.0 - x).max(x - 60.0).max(0.0);
    let dy = (0.0 - y).max(y - 40.0).max(0.0);
    dx.hypot(dy)
}

/// The XY points (in 0.001 mm) at `z` that lie on a straight side of the
/// tool-radius ring, in move order. A lead arc point is left out.
fn ring_at(pts: &[P3], z: f64) -> Vec<(i64, i64)> {
    let r = TOOL_D / 2.0;
    let on = |a: f64, b: f64| (a - b).abs() < 1e-3;
    pts.iter()
        .filter(|p| (p.z - z).abs() < EPS)
        .filter(|p| {
            ((on(p.x, -r) || on(p.x, 60.0 + r)) && (-EPS..=40.0 + EPS).contains(&p.y))
                || ((on(p.y, -r) || on(p.y, 40.0 + r)) && (-EPS..=60.0 + EPS).contains(&p.x))
        })
        .map(|p| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64))
        .collect()
}

/// The shoelace area of the closed ring `pts` (positive: counter-clockwise).
fn signed_area(pts: &[(i64, i64)]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            (a.0 as f64) * (b.1 as f64) - (b.0 as f64) * (a.1 as f64)
        })
        .sum::<f64>()
        / 2.0
}

/// Claim 1.
#[test]
fn brep_face_1_is_the_bottom_face_r6a() {
    let model = load_step();
    let face = model
        .items
        .iter()
        .find_map(|item| match item.shape() {
            Shape::Face(f) if f.brep.map(|b| b.0) == Some(BOTTOM_FACE) => {
                Some((item.key().clone(), f.clone()))
            }
            _ => None,
        })
        .expect("the stepped block has BREP face 1");
    let (key, face) = face;
    assert!(face.plane_z.abs() < EPS, "plane Z {}", face.plane_z);
    assert!(face.normal.z < 0.0, "the bottom face points down");
    assert!(
        (key.anchor[0] - 30.0).abs() < 1e-3 && (key.anchor[1] - 20.0).abs() < 1e-3,
        "anchor {:?}",
        key.anchor
    );
}

/// Claims 2 and 3.
#[test]
fn a_profile_keeps_a_down_face_as_its_outline_r6a() {
    let mut default = session_with(OperationType::Profile);
    let want = cut_targets(&mut default);
    assert!(!want.is_empty(), "the default Profile cuts");

    let mut s = session_with(OperationType::Profile);
    pick_faces(&mut s, 0, &[BOTTOM_FACE]).expect("the face is flat");
    // The click keeps the floor that the add set for the default outline.
    assert_eq!(
        s.toolpath_configs()[0].operation.floor(),
        Some(rs_cam_core::compute::floor::Floor::ModelBottom { depth: 0.0 }),
        "the pick does not change the floor row"
    );
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(
        report.refusal.as_ref().map(ToString::to_string),
        None,
        "a Profile keeps a down face"
    );
    assert_eq!(report.rows.len(), 1, "the face only: {:?}", report.rows);
    let row = &report.rows[0];
    assert_eq!(row.key.brep, Some(BOTTOM_FACE));
    assert_eq!(row.by, MatchedBy::Pick);
    let plane_z = row.plane_z.expect("a face row holds its plane Z");
    assert!(plane_z.abs() < EPS, "plane Z {plane_z}");
    assert!(!row.outline, "the face is not the part outline item");
    assert_eq!(report.row_state(row), ItemState::Cut);
    assert_eq!(report.faces_by_facing.down, 1);
    assert_eq!(report.faces_by_facing.up, 0);

    let got = cut_targets(&mut s);
    assert_eq!(
        got.len(),
        want.len(),
        "the picked face cuts {} moves, the default {}",
        got.len(),
        want.len()
    );
    // The two loops start at different corners: the part outline starts at
    // its lowest Y, then X; the face ring at its own first point. So the Z
    // of each move, the offset ring and the direction must agree, not the
    // start point.
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert!(
            (g.z - w.z).abs() < EPS,
            "cutting move {i}: the picked face cuts at Z {}, the default at Z {}",
            g.z,
            w.z
        );
    }
    for p in got.iter().chain(&want) {
        let off = distance_to_part(p.x, p.y);
        assert!(
            (off - TOOL_D / 2.0).abs() < 1e-3,
            "a cutting target {p:?} lies {off} mm from the part, not on the tool-radius ring"
        );
    }
    let deepest = |pts: &[P3]| pts.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    let (low_got, low_want) = (deepest(&got), deepest(&want));
    assert!((low_got - low_want).abs() < EPS);
    let (ring_got, ring_want) = (ring_at(&got, low_got), ring_at(&want, low_want));
    let mut sorted_got = ring_got.clone();
    let mut sorted_want = ring_want.clone();
    sorted_got.sort_unstable();
    sorted_got.dedup();
    sorted_want.sort_unstable();
    sorted_want.dedup();
    assert!(
        sorted_got.len() >= 8,
        "the ring holds its 8 side ends: {sorted_got:?}"
    );
    assert_eq!(sorted_got, sorted_want, "one loop at the deepest Z");
    assert_eq!(
        signed_area(&ring_got) > 0.0,
        signed_area(&ring_want) > 0.0,
        "one direction"
    );
    assert!(low_got.abs() < EPS, "the deepest cut is Z 0: {low_got}");
}

/// Claim 4.
#[test]
fn a_pocket_still_refuses_the_down_face_r6a() {
    let mut s = session_with(OperationType::Pocket);
    pick_faces(&mut s, 0, &[BOTTOM_FACE]).expect("the face is flat");
    let tc = s.toolpath_configs()[0].clone();
    let status = s
        .selection_status(&tc, SelectionTarget::Op)
        .unwrap()
        .refusal
        .clone()
        .expect("a Pocket refuses a down face");
    assert_eq!(status.cause, RefusalCause::FaceFacesDown, "{status}");
    let cancel = AtomicBool::new(false);
    let Err(SessionError::Selection(generated)) = s.generate_toolpath(0, &cancel) else {
        panic!("generation did not refuse with a selection refusal");
    };
    assert_eq!(*generated, status);
}
