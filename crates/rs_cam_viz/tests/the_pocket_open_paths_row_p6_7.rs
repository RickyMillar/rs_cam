//! Geometry selection P6.7: the Pocket "Open paths" row, its summary line
//! and the side mark of the viewport.
//!
//! # The claims
//!
//! The fixture is the board of the P6.4 core sentry
//! (`a_pocket_extends_an_open_path_to_the_stock_p6_4`): 100 x 60 mm, an
//! open line drawn east along Y 10, a 6 mm end mill.
//!
//! 1. Flip: under Extend to stock, Left, a click on the Flip button stores
//!    `ExtendRight` in ONE command (the toolpath revision moves by one).
//! 2. The summary line of the Selection section counts the areas of the
//!    report: "1 path extended" and `SelectionReport::extended` has one
//!    ring.
//! 3. The draw data: the viewport side mark is the tool at
//!    `selection_highlight::side_point` from the middle of the path
//!    (`path_middle`). For Left it lies inside the extended area of the
//!    report, and for Right inside the area of `ExtendRight` and outside
//!    the area of `ExtendLeft`. On a Bottom setup the area is in the
//!    drawing frame too, so the same Left point is inside it: the viewport
//!    computes the mark in the drawing frame and the setup mirror moves
//!    both together.
//!
//! The report gives no extension rays, so the viewport draws the outline
//! of the area only.
//!
//! Red proof (P6.7): make `flipped` in `boundary_2d.rs` return its input;
//! arm 1 fails, because the stored value stays `ExtendLeft`.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_pocket_open_paths_row_p6_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod inspector_harness_p6_7;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{OpenPaths, PocketConfig};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{ItemSet, PathSide};
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, SelectionTarget};
use rs_cam_viz::state::selection_highlight::{open_path_side, path_middle, side_point};

const HALF_X: f64 = 50.0;
const HALF_Y: f64 = 30.0;
const TOOL_D: f64 = 6.0;
const LINE_Y: f64 = 10.0;

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

fn line() -> Vec<P2> {
    vec![P2::new(-30.0, LINE_Y), P2::new(30.0, LINE_Y)]
}

fn drawing() -> ItemSet {
    ItemSet::from_polygons(vec![Polygon2::open_path(line())])
}

fn pocket(open_paths: OpenPaths) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        open_paths,
        ..PocketConfig::default()
    })
}

fn open_paths(op: &OperationConfig) -> OpenPaths {
    match op {
        OperationConfig::Pocket(cfg) => cfg.open_paths,
        other => panic!("not a Pocket: {other:?}"),
    }
}

#[test]
fn flip_writes_the_other_side_in_one_command_p6_7() {
    let ctx = h::ctx();
    let mut state = h::state(
        h::drawing_model(drawing()),
        TOOL_D,
        board(),
        pocket(OpenPaths::ExtendLeft),
    );
    // 2. The summary line counts the areas of the report.
    let painted = h::render(&ctx, &mut state, h::RAIL_WIDTH);
    let tc = &state.session().toolpath_configs()[0];
    let report = state
        .session()
        .selection_status(tc, SelectionTarget::Op)
        .expect("the report");
    assert!(report.refusal.is_none(), "the line extends: {report:?}");
    assert_eq!(report.extended.len(), 1, "one extended area");
    assert!(
        !h::containing(&painted, "1 path extended").is_empty(),
        "the summary line names the one extended path; painted: {:?}",
        painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    let wrapped = h::wrapped(&painted);
    assert!(wrapped.is_empty(), "IC1: no text wraps: {wrapped:?}");
    let wide = h::past_the_rail(&painted);
    assert!(
        wide.is_empty(),
        "every row fits the 240-point rail: {wide:?}"
    );

    // 1. Flip: one command, the other side.
    let before = state.session().toolpath_revision_at(0);
    let _ = h::click(&ctx, &mut state, "\u{2194} Flip", 0);
    assert_eq!(
        open_paths(&h::stored(&state)),
        OpenPaths::ExtendRight,
        "Flip must store the other side"
    );
    assert_eq!(
        state.session().toolpath_revision_at(0) - before,
        1,
        "Flip writes ONE command"
    );
}

/// A Pocket over the line on a `face_up` setup, with no GUI.
fn session(face_up: FaceUp, open_paths: OpenPaths) -> ProjectSession {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(h::TOOL), ToolType::EndMill);
    tool.diameter = TOOL_D;
    let mut builder = ProjectSessionBuilder::new()
        .stock(board())
        .tool(tool)
        .model(h::drawing_model(drawing()));
    let setup = if face_up == FaceUp::Top {
        0
    } else {
        builder.add_setup("Flipped".to_owned(), face_up)
    };
    builder
        .add_toolpath(setup, h::toolpath("P6.7", pocket(open_paths)))
        .expect("add the Pocket");
    builder.build()
}

/// The one extended area of the report, in the drawing frame.
fn extended(session: &ProjectSession) -> Vec<P2> {
    let tc = &session.toolpath_configs()[0];
    let report = session
        .selection_status(tc, SelectionTarget::Op)
        .expect("the report");
    assert!(report.refusal.is_none(), "the line extends: {report:?}");
    assert_eq!(report.extended.len(), 1, "one extended area");
    report.extended[0].clone()
}

/// Even-odd ray test.
fn inside(ring: &[P2], p: P2) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

/// The side mark of the viewport: the tool centre beside the middle of
/// the line, in the drawing frame.
fn mark(side: PathSide) -> P2 {
    let (middle, direction) = path_middle(&line()).expect("the line has a middle");
    assert!((middle.x).abs() < 1e-9 && (middle.y - LINE_Y).abs() < 1e-9);
    side_point(middle, direction, side, TOOL_D / 2.0)
}

#[test]
fn the_side_mark_lies_in_the_extended_area_p6_7() {
    let left = mark(PathSide::Left);
    let right = mark(PathSide::Right);
    assert!(left.y > LINE_Y && right.y < LINE_Y, "Left of east is north");

    let s = session(FaceUp::Top, OpenPaths::ExtendLeft);
    assert_eq!(
        open_path_side(&s.toolpath_configs()[0].operation),
        Some(PathSide::Left)
    );
    let area = extended(&s);
    assert!(inside(&area, left), "the Left mark is in the Left area");
    assert!(
        !inside(&area, right),
        "the Right mark is not in the Left area"
    );

    let s = session(FaceUp::Top, OpenPaths::ExtendRight);
    assert_eq!(
        open_path_side(&s.toolpath_configs()[0].operation),
        Some(PathSide::Right)
    );
    let area = extended(&s);
    assert!(inside(&area, right), "the Right mark is in the Right area");
    assert!(
        !inside(&area, left),
        "the Left mark is not in the Right area"
    );

    // A Bottom setup: the report keeps the area in the drawing frame, and
    // the side holds there (core P6.4 (e)), so the drawing-frame mark is
    // still inside. The viewport moves both through one transform.
    let area = extended(&session(FaceUp::Bottom, OpenPaths::ExtendLeft));
    assert!(
        inside(&area, left),
        "on a Bottom setup the Left mark is in the Left area, in the drawing frame"
    );
}
