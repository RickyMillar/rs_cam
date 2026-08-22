//! Lateral (side-face) setups, end to end — the first machining fixture in
//! this repo that puts a toolpath on `FaceUp::Front`.
//!
//! # Why this file exists before the fix does
//!
//! Until 2026-08-22 `FaceUp::{Front,Back,Left,Right}` appeared **only** in
//! the transform's own unit tests, `stock_config` and the pin-keying test.
//! No project file in the repo had ever used one, and every claim about
//! lateral behaviour in `planning/lateral_setups_2026-08-22/SPEC.md` was
//! derived from reading rather than from running. So the fixture comes
//! first and the implementation second: without it, "make lateral setups
//! work" ships a fourth silent behaviour rather than removing the first
//! three.
//!
//! # The rule being pinned (operator ruling, 2026-08-22)
//!
//! > A 2D drawing is consumed in the **work plane of the setup that uses
//! > it.**
//!
//! For `FaceUp::Top` that is already what ships — the transform is skipped
//! entirely and the drawing's coordinates *are* machining coordinates. For
//! `Bottom` the drawing is mirrored, which is how a feature registers to the
//! same physical place on the other side. For the four lateral faces the
//! drawing plane and the work plane are **perpendicular**, so no mapping
//! exists, and `world_to_local(x, y, 0)` computes a correct orthographic
//! projection of a horizontal drawing onto a vertical face — which is a
//! **line**. The arithmetic was never buggy; the request was meaningless.
//! The rule carries the parallel cases' answer to the perpendicular ones:
//! consume the drawing verbatim in the new work plane.
//!
//! What that means for this fixture's `FaceUp::Front` setup on 100 x 60 x 40
//! stock: the work plane is 100 (local X, from stock X) by 40 (local Y, from
//! stock **Z**), and local Z — the tool axis — runs 0..60 through the stock's
//! Y depth. A drawing authored at local (20,8)..(80,32) is machined there.
//! Pre-fix every point of it landed on `y = 40`, a zero-area ring on the
//! work plane's top edge.
//!
//! # What is pinned here
//!
//! 1. A 2D pocket on a Front setup cuts a real two-dimensional region —
//!    asserted on **emitted motion**, with BOTH XY extents required to
//!    exceed the tool diameter. A degenerate line satisfies neither, and a
//!    one-axis assertion would have passed against the pre-fix code (the X
//!    extent survived the collapse; only Y was destroyed).
//! 2. Its cutting moves stay inside the lateral **effective** stock box and
//!    cut at local-Z depths measured down from the lateral top (60), not
//!    from the world stock top (40). The two differ, which is what makes
//!    this a frame assertion rather than a bounds check.
//! 3. A drill target picked off the drawing lands at the drawing's own
//!    coordinates, because picks and polygon centroids must arrive in the
//!    same frame or a drill and the pocket that locates it disagree
//!    (G-DRILLPICK-FRAME established the shape of that requirement; the
//!    work-plane rule changes what "the same frame" is for a lateral
//!    setup, and this arm is what keeps the two consistent).
//! 4. The no-mesh refusal: `face_up` only means something when there is a
//!    3D model to register against, so a lateral setup consuming a drawing
//!    on a mesh-less project refuses — and the message names the workaround
//!    the operator would physically use (a Top setup with the edge as the
//!    stock face).
//! 5. The keep-out refusal (G-LATERALKEEPOUT): a world-XY footprint has no
//!    extent in a vertical work plane, so letting it project would DELETE a
//!    keep-out silently. That is a safety regression, so generation refuses
//!    instead.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType, PocketConfig};
use rs_cam_core::compute::stock_config::{FixtureId, ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Fixture, FixtureKind, LoadedModel, ProjectSession, SessionError, ToolpathConfig,
};
use rs_cam_core::toolpath::{Move, MoveType};

// ── The block, the stock, and the lateral frame it implies ───────────

const STOCK_X: f64 = 100.0;
const STOCK_Y: f64 = 60.0;
const STOCK_Z: f64 = 40.0;

/// `FaceUp::Front` maps `(w, d, h)` to `(w, h, d)`, so the work plane is
/// stock X by stock **Z** and the tool axis runs the stock's Y depth.
const LOCAL_W: f64 = STOCK_X; // 100
const LOCAL_D: f64 = STOCK_Z; // 40
const LOCAL_TOP_Z: f64 = STOCK_Y; // 60 — the lateral top, NOT stock Z

/// The pocket drawn in the Front work plane.
const POCKET_MIN: [f64; 2] = [20.0, 8.0];
const POCKET_MAX: [f64; 2] = [80.0, 32.0];
const POCKET_DEPTH: f64 = 4.0;

/// The drill target picked off the same drawing.
const DRILL_XY: [f64; 2] = [50.0, 20.0];
const DRILL_DEPTH: f64 = 8.0;

const TOOL_DIAMETER: f64 = 6.0;

/// An axis-aligned box mesh — the 3D part a lateral setup registers
/// against. Winding is irrelevant here: nothing in this fixture ray-casts
/// it, it exists so the project HAS a mesh.
fn box_mesh() -> TriangleMesh {
    let verts = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(STOCK_X, 0.0, 0.0),
        P3::new(STOCK_X, STOCK_Y, 0.0),
        P3::new(0.0, STOCK_Y, 0.0),
        P3::new(0.0, 0.0, STOCK_Z),
        P3::new(STOCK_X, 0.0, STOCK_Z),
        P3::new(STOCK_X, STOCK_Y, STOCK_Z),
        P3::new(0.0, STOCK_Y, STOCK_Z),
    ];
    let tris = vec![
        [0u32, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    TriangleMesh::from_raw(verts, tris)
}

fn drawing_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        name: "front_face_drawing".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            POCKET_MIN[0],
            POCKET_MIN[1],
            POCKET_MAX[0],
            POCKET_MAX[1],
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://front_face.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn part_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        name: "block".to_owned(),
        mesh: Some(Arc::new(box_mesh())),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://block.stl"),
        kind: Some(ModelKind::Stl),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn toolpath_config(
    name: &str,
    operation: OperationConfig,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    let op_type = operation.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth: POCKET_DEPTH,
        depth_per_pass: 2.0,
        stepover: 3.0,
        feed_rate: 1500.0,
        ..PocketConfig::default()
    })
}

fn drill_op() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: DRILL_DEPTH,
        cycle: DrillCycleType::Simple,
        feed_rate: 300.0,
        selected_holes: Some(vec![DRILL_XY]),
        ..DrillConfig::default()
    })
}

/// The whole fixture: a block, a drawing authored in the Front work plane,
/// a Front setup, one pocket and one drill.
///
/// `with_mesh = false` strips the 3D part, which is the no-mesh refusal
/// case; `with_fixture = true` bolts a clamp to the setup, which is the
/// keep-out refusal case. Both are otherwise the identical project, so a
/// refusal cannot be confused with a broken fixture.
fn build_session(with_mesh: bool, with_fixture: bool) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    if with_mesh {
        session.add_model(part_model(0));
    }
    let drawing_id = session.add_model(drawing_model(if with_mesh { 1 } else { 0 }));

    session.setups_mut()[0].face_up = FaceUp::Front;
    if with_fixture {
        session
            .add_fixture(
                0,
                Fixture {
                    id: FixtureId(0),
                    name: "Clamp".to_owned(),
                    kind: FixtureKind::Clamp,
                    enabled: true,
                    origin_x: 0.0,
                    origin_y: 0.0,
                    origin_z: 0.0,
                    size_x: 20.0,
                    size_y: 20.0,
                    size_z: 20.0,
                    clearance: 2.0,
                },
            )
            .expect("add fixture");
    }

    session
        .add_toolpath(
            0,
            toolpath_config("Front pocket", pocket_op(), tool_id, drawing_id),
        )
        .expect("add pocket");
    session
        .add_toolpath(
            0,
            toolpath_config("Front drill", drill_op(), tool_id, drawing_id),
        )
        .expect("add drill");

    session
}

/// Moves that actually cut: anything not a rapid, below the lateral top.
/// Rapids and the retract plane are excluded because their XY says nothing
/// about the machined region.
fn cutting_moves(moves: &[Move]) -> Vec<&Move> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .filter(|m| m.target.z < LOCAL_TOP_Z - 1e-9)
        .collect()
}

// ── 1-2. The 2D op cuts a real region, in the lateral frame ──────────

#[test]
fn a_front_setup_pocket_cuts_a_two_dimensional_region_in_its_work_plane() {
    let mut session = build_session(true, false);
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("generate the Front-setup pocket");
    let moves = result.op_data.annotated().toolpath.moves.clone();

    let cuts = cutting_moves(&moves);
    assert!(
        !cuts.is_empty(),
        "the Front-setup pocket emitted no cutting moves at all ({} moves total). \
         A drawing projected onto a perpendicular face is a zero-area ring, and a \
         zero-area ring offsets to nothing.",
        moves.len()
    );

    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_z, mut max_z) = (f64::INFINITY, f64::NEG_INFINITY);
    for m in &cuts {
        min_x = min_x.min(m.target.x);
        max_x = max_x.max(m.target.x);
        min_y = min_y.min(m.target.y);
        max_y = max_y.max(m.target.y);
        min_z = min_z.min(m.target.z);
        max_z = max_z.max(m.target.z);
    }

    // The load-bearing assertion, and why it names BOTH extents: the
    // projection collapse destroyed only the Y extent — X survived intact,
    // 20..80 — so a bbox-area or single-axis check would have passed
    // against the defect.
    assert!(
        max_x - min_x > TOOL_DIAMETER,
        "cut region X extent {:.3} mm does not exceed the tool diameter \
         {TOOL_DIAMETER} mm — the pocket is a line, not a region",
        max_x - min_x
    );
    assert!(
        max_y - min_y > TOOL_DIAMETER,
        "cut region Y extent {:.3} mm does not exceed the tool diameter \
         {TOOL_DIAMETER} mm. This is the projection collapse: every drawing \
         point mapped to local y = {LOCAL_D}, the work plane's top edge",
        max_y - min_y
    );

    // Inside the drawing, allowing for tool compensation pulling the
    // cut ring inward by a radius.
    assert!(
        min_x >= POCKET_MIN[0] - 1e-6 && max_x <= POCKET_MAX[0] + 1e-6,
        "cut X range {min_x:.3}..{max_x:.3} escapes the drawn pocket \
         {:?}..{:?}",
        POCKET_MIN[0],
        POCKET_MAX[0]
    );
    assert!(
        min_y >= POCKET_MIN[1] - 1e-6 && max_y <= POCKET_MAX[1] + 1e-6,
        "cut Y range {min_y:.3}..{max_y:.3} escapes the drawn pocket \
         {:?}..{:?}",
        POCKET_MIN[1],
        POCKET_MAX[1]
    );

    // And inside the LATERAL effective stock box, which is 100 x 40 — not
    // the 100 x 60 the world stock is.
    assert!(
        min_x >= -1e-6 && max_x <= LOCAL_W + 1e-6 && min_y >= -1e-6 && max_y <= LOCAL_D + 1e-6,
        "cut XY {min_x:.3}..{max_x:.3} / {min_y:.3}..{max_y:.3} leaves the \
         Front setup's effective stock box 0..{LOCAL_W} / 0..{LOCAL_D}"
    );

    // Depth is measured down from the LATERAL top (60), which is the
    // stock's Y dimension — not from the world stock top (40). If the
    // emission frame were wrong these land at or below 40.
    assert!(
        max_z <= LOCAL_TOP_Z + 1e-6,
        "a cutting move at Z {max_z:.3} sits above the lateral stock top {LOCAL_TOP_Z}"
    );
    assert!(
        min_z > STOCK_Z,
        "deepest cut Z {min_z:.3} is at or below the WORLD stock top {STOCK_Z}. \
         A Front setup emits in the setup-local frame, where the stock top is \
         {LOCAL_TOP_Z} and a {POCKET_DEPTH} mm pocket bottoms at \
         {:.1}",
        LOCAL_TOP_Z - POCKET_DEPTH
    );
    assert!(
        min_z >= LOCAL_TOP_Z - POCKET_DEPTH - 1e-6,
        "deepest cut Z {min_z:.3} is deeper than the commanded \
         {POCKET_DEPTH} mm below the lateral top {LOCAL_TOP_Z}"
    );
}

// ── 3. The drill picks land in the same work plane ───────────────────

#[test]
fn a_front_setup_drill_descends_along_local_z_at_the_drawing_coordinates() {
    let mut session = build_session(true, false);
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(1, &cancel)
        .expect("generate the Front-setup drill");
    let moves = result.op_data.annotated().toolpath.moves.clone();
    assert!(!moves.is_empty(), "the Front-setup drill emitted no motion");

    for m in &moves {
        assert!(
            (m.target.x - DRILL_XY[0]).abs() < 1e-6 && (m.target.y - DRILL_XY[1]).abs() < 1e-6,
            "a drill move sits at ({:.3}, {:.3}) but the target was picked at \
             {DRILL_XY:?} in the Front work plane. Projecting the pick through \
             `world_to_local` puts it at ({:.3}, {LOCAL_D}) — the same collapse \
             the polygons suffer, and picks must arrive in the same frame the \
             drawing does or the drill and the feature that locates it disagree.",
            m.target.x,
            m.target.y,
            DRILL_XY[0]
        );
    }

    // Fed descents run along LOCAL Z, from the lateral top downward.
    let fed_min_z = moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        fed_min_z.is_finite(),
        "the drill emitted no fed descent at all"
    );
    let want_bottom = LOCAL_TOP_Z - DRILL_DEPTH;
    assert!(
        (fed_min_z - want_bottom).abs() < 1e-6,
        "the fed descent bottoms at local Z {fed_min_z:.3}; a {DRILL_DEPTH} mm \
         hole from the lateral stock top {LOCAL_TOP_Z} bottoms at {want_bottom}"
    );
}

// ── 4. The no-mesh refusal ───────────────────────────────────────────

#[test]
fn a_lateral_setup_with_no_mesh_refuses_and_names_the_workaround() {
    let mut session = build_session(false, false);
    let cancel = AtomicBool::new(false);
    // `ToolpathComputeResult` is not `Debug`, so the success arm is spelled
    // out rather than going through `expect_err`.
    let Err(err) = session.generate_toolpath(0, &cancel) else {
        panic!(
            "a lateral setup on a project with no 3D mesh must refuse: `face_up` \
             only means something when there is a part to register against, so a \
             side face on a bare drawing is a category error, not an \
             unimplemented feature"
        )
    };
    let SessionError::UnsupportedSetup(msg) = &err else {
        panic!("expected SessionError::UnsupportedSetup, got {err:?}");
    };
    assert!(
        msg.contains("Top setup"),
        "the refusal must name the workaround an operator would physically use \
         — a Top setup with the edge as the stock face — but says: {msg}"
    );
}

// ── 5. The keep-out refusal (G-LATERALKEEPOUT) ───────────────────────

#[test]
fn a_lateral_setup_carrying_a_keep_out_refuses_rather_than_deleting_it() {
    let mut session = build_session(true, true);
    let cancel = AtomicBool::new(false);
    let Err(err) = session.generate_toolpath(0, &cancel) else {
        panic!(
            "a fixture footprint is a world-XY rectangle and has no extent in a \
             vertical work plane; projecting it would silently DELETE the \
             keep-out, which is a safety regression, so generation must refuse"
        )
    };
    let SessionError::UnsupportedSetup(msg) = &err else {
        panic!("expected SessionError::UnsupportedSetup, got {err:?}");
    };
    assert!(
        msg.to_lowercase().contains("keep-out") || msg.to_lowercase().contains("fixture"),
        "the refusal must say what it is refusing, but says: {msg}"
    );
}
