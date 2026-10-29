//! Operator ruling R7a (2026-10-30): a face boundary spills into air only,
//! never up a wall, and faces next to each other link into one region.
//!
//! # The live case
//!
//! A Parallel finish (drop cutter) on `stepped_block.step` with the
//! boundary source `Selection` of the faces at Z 10 and Z 30, containment
//! `Center`, carried the offset 3.0 (the tool DIAMETER that
//! `BoundaryConfig::for_3d_op` copies for the silhouette). The tool centre
//! ran 3 mm past every face edge and the drop cutter followed the walls
//! down (`planning/viewport_parity_2026-10-28/FOLLOW_UPS.md` §12 item 2).
//!
//! # The rule
//!
//! A `BoundarySource::Selection` boundary resolves through THE open-sides
//! rule (`compute::open_sides`, reach `ToolRadius`): each face grows one
//! tool radius past each open side (the neighbour lies at or below the
//! face plane: air) and does not move at a side against a wall. The
//! containment stays the user's choice; the offset here is 0 (a click
//! that turns the source into a selection writes 0).
//!
//! # The part
//!
//! `stepped_block.step`: X 0..20 at Z 30, X 20..40 at Z 20, X 40..60 at
//! Z 10, Y 0..40 (the `step_height` of
//! `a_pocket_cuts_past_open_sides_opensides`).
//!
//! # The claims
//!
//! (a) With the faces Z 10 and Z 30, Center, offset 0, the region set
//!     of the boundary is the Z 30 face grown by R on every side (X 0,
//!     Y 0 and Y 40 are outer edges; X 20 drops onto the Z 20 step):
//!     X -R..20+R, Y -R..40+R; and the Z 10 face grown by R past the outer
//!     edges (X 60, Y 0, Y 40) and not past X 40, the wall up to the Z 20
//!     step: within Y 0..40 it starts at X 40 (in front of the step, at
//!     Y < 0 or Y > 40, its corner pieces lie in air and reach X 40 - R).
//!     The region set holds TWO polygons: the Z 20 step (20 mm wide) lies
//!     between the faces, and each grows by R only.
//! (b) The cutting moves follow it inside the raster area of the drop
//!     cutter (the model box, X 0..60, Y 0..40; the op emits no row past
//!     it): over the Z 30 face X 0..20+R, Y 0..40 (the open side at X 20
//!     spills R onto the step); over the Z 10 face X 40..60, Y 0..40, and
//!     within Y 0..40 no move passes the wall at X 40. Tolerance: one
//!     stepover.
//! (d) The faces Z 30 and Z 20 touch at X 20 (open for the Z 30 face, a
//!     wall for the Z 20 face), so they give ONE polygon: X -R..40+R,
//!     Y -R..40+R.
//! (c) The boundary report (`selection_status`, boundary target) shows
//!     the same grown areas (`extended`, one per face), as generation.
//!
//! Red proof: with `grow_boundary_open_sides` returning before it grows,
//! (a) fails (the regions are the faces), (b) fails (the Z 30 moves end at
//! X 20), (c) gives no `extended` area, and (d) fails on the extents of
//! the linked region (two faces that share an edge are one polygon also
//! without the rule).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step,test-support --test a_face_boundary_spills_into_air_only_r7a
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, HeightMode,
};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{Selection, Shape};
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, SelectionTarget};
use rs_cam_core::toolpath::MoveType;

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
/// The finish tool (mm): the tool of the live case.
const TOOL_DIAMETER: f64 = 3.0;
const R: f64 = TOOL_DIAMETER / 2.0;
/// The raster stepover (mm); also the tolerance of each extent.
const STEPOVER: f64 = 0.25;
/// The XY margin of the stock round the part (mm).
const XY_MARGIN: f64 = 5.0;

fn load() -> LoadedModel {
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

fn stock() -> StockConfig {
    StockConfig {
        x: 60.0 + 2.0 * XY_MARGIN,
        y: 40.0 + 2.0 * XY_MARGIN,
        z: Some(30.0),
        origin_x: -XY_MARGIN,
        origin_y: -XY_MARGIN,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The BREP id of the up face of the stepped block at `z`.
fn face_at(s: &ProjectSession, z: f64) -> u16 {
    let ids: Vec<u16> = s.models()[0]
        .items
        .iter()
        .filter_map(|item| match item.shape() {
            Shape::Face(face) if face.normal.z > 0.5 && (face.plane_z - z).abs() < 1e-6 => {
                face.brep.map(|id| id.0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 1, "one up face at Z {z}: {ids:?}");
    ids[0]
}

/// A drop cutter finish with a face boundary of the up faces at `zs`,
/// `Center`, offset 0.
fn session(zs: &[f64]) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool = builder.add_tool(common::tools::endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool].id;
    let model_id = builder.add_model(load());
    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: STEPOVER,
        ..DropCutterConfig::default()
    });
    let mut cfg = common::session::toolpath_config("face finish", op, tool_id, model_id);
    cfg.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::empty(),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    cfg.heights.top_z = HeightMode::Auto;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut s = builder.build();
    let faces: Vec<u16> = zs.iter().map(|&z| face_at(&s, z)).collect();
    common::session::pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &faces).unwrap();
    s
}

/// The XY end points of every cutting move.
fn cutting_points(s: &mut ProjectSession) -> Vec<P2> {
    let result = s
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the finish generates");
    let moves = &result.toolpath().moves;
    let mut points = Vec::new();
    for pair in moves.windows(2) {
        if let MoveType::Linear { .. } = pair[1].move_type {
            points.push(P2::new(pair[0].target.x, pair[0].target.y));
            points.push(P2::new(pair[1].target.x, pair[1].target.y));
        }
    }
    points
}

/// `[x_min, y_min, x_max, y_max]` of `points`.
fn extent(points: &[P2]) -> [f64; 4] {
    points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
    )
}

fn assert_extent(what: &str, got: [f64; 4], want: [f64; 4]) {
    for (k, name) in ["x_min", "y_min", "x_max", "y_max"].iter().enumerate() {
        assert!(
            (got[k] - want[k]).abs() <= STEPOVER + 1e-6,
            "{what}: {name} {:.3}, want {:.3} (± {STEPOVER}); got {got:?}",
            got[k],
            want[k]
        );
    }
}

/// The raster area of the drop cutter: the model box (mm).
const MODEL_BOX: [f64; 4] = [0.0, 0.0, 60.0, 40.0];

/// Claim (b).
#[test]
#[expect(clippy::print_stderr, reason = "the test prints the measured extents")]
fn a_face_finish_spills_past_open_sides_only_r7a() {
    let mut s = session(&[10.0, 30.0]);
    let points = cutting_points(&mut s);
    // The Z 30 face grows to X 21.5, the Z 10 face starts at X 38.5 (its
    // corner pieces in front of the wall): X 30 splits them.
    let (high, low): (Vec<P2>, Vec<P2>) = points.iter().partition(|p| p.x < 30.0);
    assert!(
        high.len() > 20 && low.len() > 20,
        "{} / {}",
        high.len(),
        low.len()
    );
    let high_extent = extent(&high);
    let low_extent = extent(&low);
    eprintln!("Z 30 face moves: {high_extent:?}; Z 10 face moves: {low_extent:?}");
    assert_extent(
        "the Z 30 face: the open side X 20 spills R onto the step",
        high_extent,
        [MODEL_BOX[0], MODEL_BOX[1], 20.0 + R, MODEL_BOX[3]],
    );
    assert_extent(
        "the Z 10 face: the wall at X 40 holds",
        low_extent,
        [40.0, MODEL_BOX[1], MODEL_BOX[2], MODEL_BOX[3]],
    );
    let inside_y: Vec<P2> = low
        .iter()
        .copied()
        .filter(|p| (0.0..=40.0).contains(&p.y))
        .collect();
    let inside_x_min = extent(&inside_y)[0];
    eprintln!("Z 10 face moves within Y 0..40: x_min {inside_x_min:.4}");
    assert!(
        inside_x_min >= 40.0 - 1e-3,
        "the Z 10 face: a move passes the wall at X 40: x_min {inside_x_min:.4}"
    );
}

/// The bounding box of `polygon` against `want`, within the chord error
/// of a grown corner.
fn assert_box(what: &str, got: [f64; 4], want: [f64; 4]) {
    for k in 0..4 {
        assert!(
            (got[k] - want[k]).abs() < 1e-2,
            "{what}: box {got:?}, want {want:?}"
        );
    }
}

/// Claims (a), (c) and (d).
#[test]
#[expect(clippy::print_stderr, reason = "the test prints the region extents")]
fn adjacent_faces_link_into_one_region_r7a() {
    let s = session(&[10.0, 30.0]);
    let regions = s.boundary_selection_regions(0).unwrap().unwrap();
    let mut boxes: Vec<[f64; 4]> = regions.iter().map(|p| p.bbox()).collect();
    boxes.sort_by(|a, b| a[0].total_cmp(&b[0]));
    eprintln!("faces Z 10 + Z 30: {} polygons {boxes:?}", regions.len());
    assert_eq!(
        regions.len(),
        2,
        "the Z 20 step lies between the faces: two polygons in one set"
    );
    assert_box("the Z 30 face", boxes[0], [-R, -R, 20.0 + R, 40.0 + R]);
    assert_box(
        "the Z 10 face",
        boxes[1],
        [40.0 - R, -R, 60.0 + R, 40.0 + R],
    );
    // Within Y 0..40 the Z 10 region starts at the wall X 40.
    let low = regions
        .iter()
        .find(|p| p.bbox()[0] > 30.0)
        .expect("the Z 10 region");
    for y in [0.5, 20.0, 39.5] {
        assert!(
            !low.contains_point(&P2::new(40.0 - 0.1, y)),
            "the Z 10 region passes the wall at Y {y}"
        );
        assert!(low.contains_point(&P2::new(40.0 + 0.1, y)));
    }
    // (c) the status report shows each grown face, as generation.
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Boundary).unwrap();
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(report.extended.len(), 2, "each face grew past an open side");

    // (d)
    let s = session(&[30.0, 20.0]);
    let regions = s.boundary_selection_regions(0).unwrap().unwrap();
    eprintln!(
        "faces Z 30 + Z 20: {} polygons {:?}",
        regions.len(),
        regions.iter().map(|p| p.bbox()).collect::<Vec<_>>()
    );
    assert_eq!(regions.len(), 1, "two faces that touch give one region");
    assert_box(
        "the linked region",
        regions[0].bbox(),
        [-R, -R, 40.0 + R, 40.0 + R],
    );
}
