//! G-SIM-IDENTITY-FRAME — the simulator's global / playback stock
//! mis-registered identity setups by the stock origin.
//!
//! `run_simulation` builds its global (checkpoint / playback / screenshot)
//! stock ZERO-ROOTED, from `(0,0,0)..(stock_dx, stock_dy, stock_dz)`,
//! because `SetupTransformInfo::local_to_global` deliberately returns
//! stock-relative coordinates and never re-adds the origin. Non-identity
//! setup groups were mapped into that frame correctly. **Identity setup
//! groups were stamped in verbatim, still in WORLD frame**, with no
//! `-stock_bbox.min` translation — so with `StockConfig::origin != 0`
//! every cut from an identity setup landed in the playback stock displaced
//! by exactly the stock origin.
//!
//! Found 2026-08-19 on the `wanaka200` from-scratch run (Setup 1 `bottom`
//! plus Setup 2 `top`, stock 240x250x25 at origin (-20,-25,-18)): the render
//! showed the whole centre of the part cut through, with survivors only in
//! a 20 mm and a 25 mm band on two edges — `origin_x` and `origin_y`
//! exactly. The Z component is what produced the false through-cut: a
//! world-frame cutter Z below the zero-rooted grid clears the ENTIRE dexel
//! ray. The emitted G-code was correct throughout; only the display /
//! playback object was wrong, and it was silent because every gate, metric
//! and collision check reads the per-group stock, which is framed
//! correctly per setup.
//!
//! Why it shipped: every prior fixture had `origin == 0`, or contained
//! only one setup class. This sentry is the first fixture that is BOTH
//! mixed (one identity + one non-identity group) and non-zero-origin.
//!
//! Fixture geometry — stock 100 x 80 x 20 at world origin (30, 40, -20),
//! so the stock top sits at world Z = 0 and the stock-relative frame is
//! `world - (30, 40, -20)`:
//!
//! - group 0, NON-IDENTITY (`face_up = Bottom`): cuts 3 mm down from its
//!   own local top at local (20, 15..30), which `local_to_global` maps to
//!   stock-relative (20, 50..65), Z 0..3 — i.e. into the stock's BOTTOM.
//! - group 1, IDENTITY: cuts a world-frame groove at Y = 60 from X = 40 to
//!   X = 90 at Z = -3, which is stock-relative (10..60, 20), Z 17..20.
//!
//! Pre-fix, group 1's moves went into the zero-rooted grid unshifted: the
//! groove appeared at (40..90, 60) instead of (10..60, 20), and because
//! Z = -3 sat below the whole grid it removed the FULL 20 mm ray depth
//! rather than 3 mm.

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
const STOCK_Y: f64 = 80.0;
const STOCK_Z: f64 = 20.0;
/// Stock origin — non-zero on every axis, which is the whole point of the
/// fixture. Stock top therefore sits at world Z = 0.
const ORIGIN_X: f64 = 30.0;
const ORIGIN_Y: f64 = 40.0;
const ORIGIN_Z: f64 = -20.0;
const CELL_MM: f64 = 1.0;
/// Commanded axial depth of both groups' cuts.
const CUT_DEPTH_MM: f64 = 3.0;

fn origin() -> P3 {
    P3::new(ORIGIN_X, ORIGIN_Y, ORIGIN_Z)
}

fn stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: origin(),
        max: P3::new(ORIGIN_X + STOCK_X, ORIGIN_Y + STOCK_Y, ORIGIN_Z + STOCK_Z),
    }
}

fn endmill_6mm() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
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
        tool: endmill_6mm(),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: 0,
    }
}

/// Group 0 — a `face_up = Bottom` setup. Its toolpath is authored in the
/// setup's own zero-rooted local frame; `local_to_global` maps it.
fn non_identity_group() -> SimGroupEntry {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(20.0, 15.0, STOCK_Z + 5.0));
    tp.feed_to(P3::new(20.0, 15.0, STOCK_Z - CUT_DEPTH_MM), 800.0);
    tp.feed_to(P3::new(20.0, 30.0, STOCK_Z - CUT_DEPTH_MM), 800.0);
    tp.rapid_to(P3::new(20.0, 30.0, STOCK_Z + 5.0));

    SimGroupEntry {
        toolpaths: vec![entry(0, "Flipped groove", tp)],
        direction: StockCutDirection::FromBottom,
        local_stock_bbox: Some(BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(STOCK_X, STOCK_Y, STOCK_Z),
        }),
        local_to_global: Some(SetupTransformInfo {
            face_up: FaceUp::Bottom,
            z_rotation: ZRotation::Deg0,
            stock_x: STOCK_X,
            stock_y: STOCK_Y,
            stock_z: STOCK_Z,
            stock_origin_x: ORIGIN_X,
            stock_origin_y: ORIGIN_Y,
            stock_origin_z: ORIGIN_Z,
        }),
        phantom_prior_stock: None,
    }
}

/// Group 1 — an identity setup (`face_up = Top`, `z_rotation = Deg0`).
/// Per F-024 it carries no `local_stock_bbox` and no `local_to_global`, and
/// its toolpath is emitted directly in WORLD coordinates.
fn identity_group() -> SimGroupEntry {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(40.0, 60.0, 5.0));
    tp.feed_to(P3::new(40.0, 60.0, -CUT_DEPTH_MM), 800.0);
    tp.feed_to(P3::new(90.0, 60.0, -CUT_DEPTH_MM), 800.0);
    tp.rapid_to(P3::new(90.0, 60.0, 5.0));

    SimGroupEntry {
        toolpaths: vec![entry(1, "World groove", tp)],
        direction: StockCutDirection::FromTop,
        local_stock_bbox: None,
        local_to_global: None,
        phantom_prior_stock: None,
    }
}

fn run_mixed_project() -> SimulationResult {
    let request = SimulationRequest {
        groups: vec![non_identity_group(), identity_group()],
        stock_bbox: stock_bbox(),
        stock_top_z: ORIGIN_Z + STOCK_Z,
        resolution: CELL_MM,
        metric_options: SimulationMetricOptions::default(),
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    };
    let cancel = AtomicBool::new(false);
    run_simulation(&request, &cancel).expect("mixed-setup simulation completes")
}

fn final_playback_stock(result: &SimulationResult) -> &TriDexelStock {
    &result
        .checkpoints
        .last()
        .expect("at least one checkpoint")
        .stock
}

/// World XY -> the zero-rooted stock-relative frame the playback stock
/// grid is built in.
fn stock_relative(world_x: f64, world_y: f64) -> (f64, f64) {
    (world_x - ORIGIN_X, world_y - ORIGIN_Y)
}

fn top_z(stock: &TriDexelStock, u: f64, v: f64) -> Option<f32> {
    let (row, col) = stock
        .z_grid
        .world_to_cell(u, v)
        .unwrap_or_else(|| panic!("probe ({u}, {v}) is outside the playback grid"));
    stock.z_grid.top_z_at(row, col)
}

fn material_length(stock: &TriDexelStock, u: f64, v: f64) -> f32 {
    let (row, col) = stock
        .z_grid
        .world_to_cell(u, v)
        .unwrap_or_else(|| panic!("probe ({u}, {v}) is outside the playback grid"));
    stock.z_grid.material_length_at(row, col)
}

/// The identity group's cut must land in the playback stock at the SAME
/// place it lands in that group's own (world-framed) stock — i.e. at the
/// stock-relative image of its world coordinates, to the commanded depth.
#[test]
fn identity_group_cut_registers_in_playback_stock() {
    let result = run_mixed_project();
    let stock = final_playback_stock(&result);

    // Mid-span of the world groove: world (65, 60) -> stock-relative (35, 20).
    let (u, v) = stock_relative(65.0, 60.0);
    let measured = top_z(stock, u, v).unwrap_or_else(|| {
        panic!(
            "G-SIM-IDENTITY-FRAME: playback stock has NO material left at stock-relative \
             ({u}, {v}) — the identity group's cut should have removed {CUT_DEPTH_MM} mm there, \
             not the full ray"
        )
    });

    let expected = (STOCK_Z - CUT_DEPTH_MM) as f32;
    assert!(
        (measured - expected).abs() <= 1.0,
        "G-SIM-IDENTITY-FRAME: identity-group cut mis-registered in the playback stock. At \
         stock-relative ({u}, {v}) — the image of the world groove at (65, 60) — expected \
         top_z ≈ {expected} (stock top {STOCK_Z} less the commanded {CUT_DEPTH_MM} mm), got \
         {measured}. Pre-fix the world-frame toolpath was stamped into the zero-rooted global \
         stock unshifted, so this cell read an untouched {STOCK_Z}."
    );
}

/// The mirror half of the same defect: nothing may be carved at the
/// UNSHIFTED world coordinates. Pre-fix that cell was not merely wrong, it
/// was cleared to full depth (the world Z sat below the zero-rooted grid,
/// so `ray_blend_above` took the whole ray) — the false through-cut.
#[test]
fn identity_group_cut_does_not_appear_at_unshifted_world_coords() {
    let result = run_mixed_project();
    let stock = final_playback_stock(&result);

    // World (65, 60) read AS IF it were stock-relative — the pre-fix landing site.
    let (u, v) = (65.0, 60.0);
    let measured = top_z(stock, u, v).unwrap_or_else(|| {
        panic!(
            "G-SIM-IDENTITY-FRAME: playback stock is empty at ({u}, {v}) — the identity group's \
             world-frame moves were stamped here unshifted and, sitting below the zero-rooted \
             grid, cleared the entire {STOCK_Z} mm dexel ray. This is the false through-cut."
        )
    });

    assert!(
        (measured - STOCK_Z as f32).abs() <= 1.0,
        "G-SIM-IDENTITY-FRAME: playback stock is carved at ({u}, {v}), where nothing cuts. \
         Expected untouched top_z ≈ {STOCK_Z}, got {measured}."
    );
    assert!(
        (material_length(stock, u, v) - STOCK_Z as f32).abs() <= 1.0,
        "G-SIM-IDENTITY-FRAME: material removed at ({u}, {v}), where nothing cuts."
    );
}

/// Guard the half of the mapping that was already right: a non-identity
/// group still carves through `local_to_global`, from the stock's underside.
#[test]
fn non_identity_group_cut_still_registers_in_playback_stock() {
    let result = run_mixed_project();
    let stock = final_playback_stock(&result);

    // Local (20, 22.5) under `FaceUp::Bottom` -> stock-relative (20, 57.5).
    let (u, v) = (20.0, STOCK_Y - 22.5);
    let remaining = material_length(stock, u, v);
    let expected = (STOCK_Z - CUT_DEPTH_MM) as f32;
    assert!(
        (remaining - expected).abs() <= 1.0,
        "non-identity group's cut should still remove {CUT_DEPTH_MM} mm from the underside at \
         stock-relative ({u}, {v}): expected {expected} mm of material left, got {remaining}"
    );

    // …and from the TOP that column is untouched, because it cut from below.
    let measured_top = top_z(stock, u, v).expect("material remains above the flipped cut");
    assert!(
        (measured_top - STOCK_Z as f32).abs() <= 1.0,
        "non-identity cut came in from the wrong side: top_z {measured_top}, expected ≈ {STOCK_Z}"
    );
}

/// The composite display mesh has the same contract: BOTH groups land in
/// the zero-rooted stock-relative frame. Pre-fix a mixed project
/// composited its identity group in world frame and its non-identity group
/// in stock-relative frame into one mesh, so the two setups' surfaces were
/// drawn a whole stock origin apart and the mesh spilled outside the stock
/// box on every axis the origin was non-zero on.
#[test]
fn composite_mesh_frames_both_group_classes_alike() {
    let result = run_mixed_project();
    let verts = &result.mesh.vertices;
    assert!(!verts.is_empty(), "composite mesh should not be empty");

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for chunk in verts.chunks_exact(3) {
        for axis in 0..3 {
            min[axis] = min[axis].min(chunk[axis]);
            max[axis] = max[axis].max(chunk[axis]);
        }
    }

    // One cell of slack for the marching-cubes surface extraction.
    let tol = (CELL_MM * 2.0) as f32;
    let bounds = [STOCK_X as f32, STOCK_Y as f32, STOCK_Z as f32];
    for axis in 0..3 {
        assert!(
            min[axis] >= -tol && max[axis] <= bounds[axis] + tol,
            "composite mesh escapes the zero-rooted stock box on axis {axis}: \
             [{}, {}] vs [0, {}]. Pre-fix the identity group's vertices stayed in WORLD frame \
             (origin ({ORIGIN_X}, {ORIGIN_Y}, {ORIGIN_Z})), displacing that setup's surface from the non-identity \
             setup's by exactly the stock origin.",
            min[axis],
            max[axis],
            bounds[axis]
        );
    }
}
