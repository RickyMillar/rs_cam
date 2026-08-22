//! **G-LATERALSCRUB** — the object the live-scrub viewport replays into must
//! carry a lateral setup's cut.
//!
//! # The defect
//!
//! `dexel_stock_to_mesh` builds a **closed** marching-cubes solid from the
//! Z grid and then *appends* the X/Y grids as **open** per-segment heightmap
//! surfaces (`side_grid_to_mesh`). There is no boolean and the three grids are
//! never reconciled — `ensure_grid` allocates a pristine, full-material X/Y
//! grid at the first lateral stamp, knowing nothing about what the Z grid has
//! already lost. So a lateral cut stamped into the global playback stock
//! removes nothing from the rendered solid: the open side surface is drawn
//! *inside* an intact block and is occluded by it.
//!
//! Checkpoint meshes were never affected — they are extracted from the
//! per-setup **local** `group_stock`, which is simulated with `direction`
//! hardcoded to `FromTop` (`compute/simulate.rs`), and only then frame-mapped.
//! So the checkpoint mesh and the live-scrub mesh *disagree* for a lateral
//! setup, and only the live-scrub one is wrong.
//!
//! # Why this file asserts on material volume and not on a mesh
//!
//! The obvious probe — "are there mesh vertices inside the pocket?" — passes
//! against the **broken** code, because `dexel_stock_to_mesh` really does
//! append a surface that shows the cut. It is just an open sheet buried inside
//! a solid that still occludes it. The honest measure of what the viewport
//! *shows* is therefore the closed solid, i.e. the Z grid, and the
//! frame-independent reading of that solid is the material volume its dexel
//! rays still carry.
//!
//! Volume is also what lets the two routes be compared at all: the local stock
//! and the playback stock live in different frames, and every mapping between
//! them (`SetupTransformInfo::local_to_global`) is a rigid axis permutation
//! plus a translation, which preserves volume exactly.
//!
//! # The fixture
//!
//! Stock 100 × 60 × 40 at the world origin, one `FaceUp::Front` setup. `Front`
//! maps `(w, d, h)` to `(w, h, d)`, so the work plane is 100 (stock X) by 40
//! (stock **Z**) and the tool axis runs 0..60 through the stock's Y depth —
//! local top is 60, not 40.
//!
//! Two toolpaths, because one is not enough to compare the routes: the second
//! one's `prior_stocks` snapshot **is** the local stock as it stood after the
//! first carved, and that is the local-route reading of the first toolpath's
//! removal. `checkpoints[0].stock` is the playback-route reading of the same
//! removal. They must agree.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult, run_simulation,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationMetricOptions;
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

const STOCK_X: f64 = 100.0;
const STOCK_Y: f64 = 60.0;
const STOCK_Z: f64 = 40.0;
const CELL_MM: f64 = 1.0;

/// `FaceUp::Front`'s effective stock: local W = stock X, local D = stock Z,
/// local H (the tool axis) = stock Y.
const LOCAL_W: f64 = STOCK_X;
const LOCAL_D: f64 = STOCK_Z;
const LOCAL_TOP: f64 = STOCK_Y;

const TOOL_DIAMETER: f64 = 6.0;
const CUT_DEPTH: f64 = 4.0;
/// The first toolpath's groove, in the Front work plane.
const GROOVE_X0: f64 = 20.0;
const GROOVE_X1: f64 = 80.0;
const GROOVE_Y: f64 = 20.0;

fn stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(STOCK_X, STOCK_Y, STOCK_Z),
    }
}

fn local_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(LOCAL_W, LOCAL_D, LOCAL_TOP),
    }
}

fn front_transform() -> SetupTransformInfo {
    SetupTransformInfo {
        face_up: FaceUp::Front,
        z_rotation: ZRotation::Deg0,
        stock_x: STOCK_X,
        stock_y: STOCK_Y,
        stock_z: STOCK_Z,
        stock_origin_x: 0.0,
        stock_origin_y: 0.0,
        stock_origin_z: 0.0,
    }
}

fn endmill() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(TOOL_DIAMETER, 25.0)),
        TOOL_DIAMETER,
        20.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn entry(id: usize, name: &str, tp: Toolpath) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(id),
        name: name.to_owned(),
        annotated: Arc::new(AnnotatedToolpath::new(tp)),
        tool: endmill(),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 0,
    }
}

/// A straight groove in the Front work plane, cut `CUT_DEPTH` down from the
/// lateral top. Authored in setup-local coordinates, like every group's
/// toolpath is.
fn groove(y: f64) -> Toolpath {
    let z = LOCAL_TOP - CUT_DEPTH;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(GROOVE_X0, y, LOCAL_TOP + 5.0));
    tp.feed_to(P3::new(GROOVE_X0, y, z), 400.0);
    tp.feed_to(P3::new(GROOVE_X1, y, z), 800.0);
    tp.rapid_to(P3::new(GROOVE_X1, y, LOCAL_TOP + 5.0));
    tp
}

fn front_group() -> SimGroupEntry {
    SimGroupEntry {
        toolpaths: vec![
            entry(0, "Front groove", groove(GROOVE_Y)),
            entry(1, "Front groove 2", groove(GROOVE_Y + 10.0)),
        ],
        direction: StockCutDirection::FromTop,
        local_stock_bbox: Some(local_bbox()),
        local_to_global: Some(front_transform()),
        phantom_prior_stock: None,
    }
}

/// The control arm: the same two grooves on an ordinary identity (Top) setup,
/// where the playback stock has always worked. Its toolpath is emitted in
/// world coordinates, as identity groups are.
fn top_group() -> SimGroupEntry {
    let cut = |y: f64| {
        let z = STOCK_Z - CUT_DEPTH;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(GROOVE_X0, y, STOCK_Z + 5.0));
        tp.feed_to(P3::new(GROOVE_X0, y, z), 400.0);
        tp.feed_to(P3::new(GROOVE_X1, y, z), 800.0);
        tp.rapid_to(P3::new(GROOVE_X1, y, STOCK_Z + 5.0));
        tp
    };
    SimGroupEntry {
        toolpaths: vec![
            entry(0, "Top groove", cut(GROOVE_Y)),
            entry(1, "Top groove 2", cut(GROOVE_Y + 10.0)),
        ],
        direction: StockCutDirection::FromTop,
        local_stock_bbox: None,
        local_to_global: None,
        phantom_prior_stock: None,
    }
}

fn run(group: SimGroupEntry) -> SimulationResult {
    let request = SimulationRequest {
        groups: vec![group],
        stock_bbox: stock_bbox(),
        stock_top_z: STOCK_Z,
        resolution: CELL_MM,
        metric_options: SimulationMetricOptions::default(),
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    };
    let cancel = AtomicBool::new(false);
    run_simulation(&request, &cancel).expect("simulation completes")
}

/// Material volume still standing in the stock's **closed solid** — the Z
/// grid, which is the only grid `dexel_stock_to_mesh` turns into a closed
/// body. Rigid frame maps preserve it, so the local and the playback route
/// can be compared directly.
fn solid_volume_mm3(stock: &TriDexelStock) -> f64 {
    let grid = &stock.z_grid;
    let cell_area = grid.cell_size * grid.cell_size;
    let mut total = 0.0;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            total += f64::from(grid.material_length_at(row, col)) * cell_area;
        }
    }
    total
}

/// How much the closed solid lost, relative to an uncut block of the same
/// bounding box at the same cell size.
fn removed_mm3(stock: &TriDexelStock) -> f64 {
    let fresh = TriDexelStock::from_bounds(&stock.stock_bbox, stock.z_grid.cell_size);
    solid_volume_mm3(&fresh) - solid_volume_mm3(stock)
}

/// Swept volume of one groove: a `GROOVE_X1 - GROOVE_X0` long slot of the
/// tool's width, plus the two end caps, `CUT_DEPTH` deep.
fn expected_groove_mm3() -> f64 {
    let length = GROOVE_X1 - GROOVE_X0;
    let r = TOOL_DIAMETER / 2.0;
    (length * TOOL_DIAMETER + std::f64::consts::PI * r * r) * CUT_DEPTH
}

/// The local route's reading of toolpath 0's removal: the snapshot taken of
/// the per-setup local stock immediately before toolpath 1 carved.
fn local_route_removed(result: &SimulationResult) -> f64 {
    let prior = result
        .prior_stocks
        .get(&ToolpathId(1))
        .expect("toolpath 1 has a prior-stock snapshot of the local stock");
    removed_mm3(prior)
}

/// The playback route's reading of the same removal: the stock the live-scrub
/// viewport resets to and replays forward from.
fn playback_route_removed(result: &SimulationResult) -> f64 {
    removed_mm3(&result.checkpoints[0].stock)
}

/// The fixture must actually cut, or every comparison below is vacuous.
#[test]
fn the_lateral_groove_removes_material_on_the_local_route() {
    let result = run(front_group());
    let local = local_route_removed(&result);
    let expected = expected_groove_mm3();
    assert!(
        local > expected * 0.7 && local < expected * 1.4,
        "fixture is not cutting what it claims: the local (per-setup) stock lost {local:.0} mm³ \
         after the first Front groove, expected ≈ {expected:.0} mm³. Every other assertion in \
         this file compares against this number, so it must be real first."
    );
}

/// **The defect.** The playback stock is what the live-scrub viewport resets
/// to on a backward scrub and replays forward from. If a lateral cut never
/// reaches its closed solid, the viewport can never show that cut.
#[test]
fn a_lateral_cut_reaches_the_playback_stocks_solid() {
    let result = run(front_group());
    let playback = playback_route_removed(&result);
    let expected = expected_groove_mm3();
    assert!(
        playback > 0.0,
        "G-LATERALSCRUB: the playback stock's closed solid lost NOTHING ({playback:.1} mm³) \
         after a Front-setup groove that removed ≈ {expected:.0} mm³. The lateral stamp went \
         into a freshly allocated side (X/Y) grid, which `dexel_stock_to_mesh` appends as an \
         OPEN surface inside an intact marching-cubes solid — so the cut is invisible on every \
         surface fed by the playback stock, while the checkpoint mesh (built from the local \
         stock) shows it. The two disagree, and this one is the wrong one."
    );
}

/// The two routes are two readings of one physical removal, so they must
/// agree. This is the assertion that stays honest if the playback stock ever
/// grows a *partial* lateral capability.
#[test]
fn the_playback_route_agrees_with_the_local_route_on_a_lateral_setup() {
    let result = run(front_group());
    let local = local_route_removed(&result);
    let playback = playback_route_removed(&result);
    assert!(
        (playback - local).abs() <= local * 0.05,
        "G-LATERALSCRUB: the checkpoint/local route says the Front groove removed \
         {local:.0} mm³; the live-scrub playback route says {playback:.0} mm³. These are two \
         readings of the same cut and must agree within 5%."
    );
}

/// The control. An identity setup's playback stock must keep working exactly
/// as it does today — the fix for the lateral case must not reroute the
/// common one. (`sim_identity_setup_playback_frame.rs` pins the *frame*; this
/// pins that the removal still lands in the playback solid at all.)
#[test]
fn an_identity_setups_playback_stock_still_carries_its_cut() {
    let result = run(top_group());
    let local = local_route_removed(&result);
    let playback = playback_route_removed(&result);
    let expected = expected_groove_mm3();
    assert!(
        local > expected * 0.7 && local < expected * 1.4,
        "control arm is not cutting: local route removed {local:.0} mm³, expected ≈ \
         {expected:.0} mm³"
    );
    assert!(
        (playback - local).abs() <= local * 0.05,
        "an identity setup's playback stock stopped agreeing with its local stock: local \
         {local:.0} mm³ vs playback {playback:.0} mm³"
    );
}
