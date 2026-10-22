//! P6.4 (geometry selection) — a Pocket with Extend to stock cuts the area
//! on the stated side of an open path, out to the stock edge (operator
//! ruling O-2).
//!
//! # The claim
//!
//! O-2 (2026-10-21): extend to stock is opt-in (`OpenPaths::Skip` is the
//! default); the pocket is on the LEFT of the drawn direction, a Flip gives
//! the right; the area ends one tool radius past the stock edge (B), so the
//! tool centre reaches the stock edge. Through the session door
//! (`generate_toolpath`), on a 100 x 60 mm board and an open line drawn
//! east along Y 10:
//!
//! - (a) `ExtendLeft` cuts north of the line only: no fed move puts the
//!   tool edge south of the line by more than `CLOSE_TOL_MM`.
//! - (b) The cut reaches the stock edge: the tool centre comes within
//!   `EDGE_TOL_MM` of the west, east and north stock edges. Under (A) (the
//!   area ends at the stock edge) it would stop one tool radius short.
//! - (c) `ExtendRight` (the Flip) cuts south of the line only, out to the
//!   south edge.
//! - (d) `selection_status` gives the path the state `Cut`, one extended
//!   area, and no note; `Skip` leaves the path out with the note that names
//!   Pocket with Extend to stock.
//! - (e) On a `Bottom` setup the side holds in the drawing frame: each fed
//!   point, mapped back with `SetupTransformInfo::local_to_drawing`, is
//!   north of the line (the door mirrors the drawing, so the setup frame
//!   takes the other side, as Trace does).
//!
//! Red proof (P6.4): with the stock edge walked the wrong way in
//! `selection::extend::close_to_stock` (clockwise for `Left`), (a), (c)
//! and (e) fail on the side.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_pocket_extends_an_open_path_to_the_stock_p6_4
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{OpenPaths, PocketConfig};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::join::CLOSE_TOL_MM;
use rs_cam_core::session::{ItemState, ProjectSession, ProjectSessionBuilder, SelectionTarget};
use rs_cam_core::toolpath::MoveType;

/// The board: X -50 to 50, Y -30 to 30, its top at Z0.
const HALF_X: f64 = 50.0;
const HALF_Y: f64 = 30.0;
/// The tool diameter (mm).
const TOOL_D: f64 = 6.0;
/// The open line, drawn east along Y 10.
const LINE_Y: f64 = 10.0;
const LINE: [[f64; 2]; 2] = [[-30.0, LINE_Y], [30.0, LINE_Y]];
/// How near the tool centre comes to a stock edge (mm): the finished-edge
/// segment merge moves a point by 0.010 mm at most (P6.0b), twice for
/// margin. A cut that ends at the stock edge (O-2 (A)) stops one tool
/// radius (3 mm) short.
const EDGE_TOL_MM: f64 = 0.02;

fn board() -> StockConfig {
    StockConfig {
        x: 2.0 * HALF_X,
        y: 2.0 * HALF_Y,
        z: 10.0,
        origin_x: -HALF_X,
        origin_y: -HALF_Y,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pocket(open_paths: OpenPaths) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        open_paths,
        ..PocketConfig::default()
    })
}

/// A session of one Pocket over the open line, on a `face_up` setup.
fn session(face_up: FaceUp, open_paths: OpenPaths) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(board());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.raw();
    let line = Polygon2::open_path(LINE.iter().map(|&[x, y]| P2::new(x, y)).collect());
    let model_id = builder.add_model(polygon_model(vec![line], "p6_4_open_line"));
    let setup = if face_up == FaceUp::Top {
        0
    } else {
        builder.add_setup("Flipped".to_owned(), face_up)
    };
    let _ = builder
        .add_toolpath(
            setup,
            toolpath_config(
                "p6_4",
                pocket(open_paths),
                rs_cam_core::ids::ToolId::from_raw(tool_id),
                model_id,
            ),
        )
        .expect("add the Pocket");
    builder.build()
}

/// The XY of every fed move (not a rapid) below the stock top, in the
/// DRAWING frame.
fn fed_in_drawing(face_up: FaceUp, open_paths: OpenPaths) -> Vec<P2> {
    let mut s = session(face_up, open_paths);
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{face_up:?} {open_paths:?}: the Pocket generates: {e}"));
    let info = s.setup_transform_info(face_up, ZRotation::Deg0);
    // The fed moves at the final level: the full area of the pocket.
    let tp = s.result_at(0).expect("a result").toolpath().clone();
    let cut_z = tp
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid && (m.target.z - cut_z).abs() < 1e-6)
        .map(|m| {
            let p = P2::new(m.target.x, m.target.y);
            if face_up == FaceUp::Top {
                p
            } else {
                info.local_to_drawing(p)
            }
        })
        .collect()
}

fn bounds(points: &[P2]) -> [f64; 4] {
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

#[test]
fn the_left_side_is_cut_out_to_the_stock_edge() {
    let r = TOOL_D / 2.0;
    let fed = fed_in_drawing(FaceUp::Top, OpenPaths::ExtendLeft);
    assert!(!fed.is_empty(), "the Pocket cuts");
    let [x0, y0, x1, y1] = bounds(&fed);
    // (a) nothing south of the line.
    assert!(
        y0 - r >= LINE_Y - CLOSE_TOL_MM,
        "the tool edge reaches Y {:.4}, south of the line Y {LINE_Y}",
        y0 - r
    );
    // (b) the tool centre reaches the west, east and north stock edges.
    for (name, got, edge) in [
        ("west", x0, -HALF_X),
        ("east", x1, HALF_X),
        ("north", y1, HALF_Y),
    ] {
        assert!(
            (got - edge).abs() <= EDGE_TOL_MM,
            "{name}: the tool centre reaches {got:.4}, the stock edge is {edge}"
        );
    }
}

#[test]
fn the_flip_cuts_the_right_side() {
    let r = TOOL_D / 2.0;
    let fed = fed_in_drawing(FaceUp::Top, OpenPaths::ExtendRight);
    assert!(!fed.is_empty(), "the Pocket cuts");
    let [x0, y0, x1, y1] = bounds(&fed);
    // (c) nothing north of the line, out to the south edge.
    assert!(
        y1 + r <= LINE_Y + CLOSE_TOL_MM,
        "the tool edge reaches Y {:.4}, north of the line Y {LINE_Y}",
        y1 + r
    );
    for (name, got, edge) in [
        ("west", x0, -HALF_X),
        ("east", x1, HALF_X),
        ("south", y0, -HALF_Y),
    ] {
        assert!(
            (got - edge).abs() <= EDGE_TOL_MM,
            "{name}: the tool centre reaches {got:.4}, the stock edge is {edge}"
        );
    }
}

#[test]
fn the_side_holds_in_the_drawing_frame_on_a_bottom_setup() {
    let r = TOOL_D / 2.0;
    let fed = fed_in_drawing(FaceUp::Bottom, OpenPaths::ExtendLeft);
    assert!(!fed.is_empty(), "the Pocket cuts");
    let [_, y0, _, y1] = bounds(&fed);
    // (e) north of the line in the drawing frame, out to the north edge.
    assert!(
        y0 - r >= LINE_Y - CLOSE_TOL_MM,
        "Bottom: the tool edge reaches drawing Y {:.4}, south of the line",
        y0 - r
    );
    assert!(
        (y1 - HALF_Y).abs() <= EDGE_TOL_MM,
        "Bottom: the tool centre reaches drawing Y {y1:.4}, the north edge is {HALF_Y}"
    );
}

#[test]
fn the_status_gives_the_path_the_state_cut() {
    // (d) Extend: the path is cut, one extended area, no note.
    let s = session(FaceUp::Top, OpenPaths::ExtendLeft);
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, SelectionTarget::Op)
        .expect("the status answers");
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(report.item_states, vec![ItemState::Cut]);
    assert_eq!(report.extended.len(), 1, "one extended area");
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    // The extended area holds the north-west corner of the grown stock.
    let ring = Polygon2::new(report.extended[0].clone());
    let r = TOOL_D / 2.0;
    assert!(ring.contains_point(&P2::new(-HALF_X - r + 0.5, HALF_Y + r - 0.5)));
    assert!(!ring.contains_point(&P2::new(0.0, LINE_Y - 1.0)));

    // Skip (the default): the path is left out with the note that names
    // the extend option, and the Pocket has nothing to cut.
    let s = session(FaceUp::Top, OpenPaths::Skip);
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, SelectionTarget::Op)
        .expect("the status answers");
    assert_eq!(report.item_states, vec![ItemState::Skipped]);
    let refusal = report
        .refusal
        .as_ref()
        .expect("Skip with only an open path refuses");
    assert!(
        refusal.to_string().contains("Pocket with Extend to stock"),
        "{refusal}"
    );
}
