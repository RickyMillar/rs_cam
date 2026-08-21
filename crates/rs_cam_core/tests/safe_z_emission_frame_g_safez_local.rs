//! G-SAFEZ-LOCAL — `safe_z` must be floored in the **emission** frame,
//! not the zero-rooted local one.
//!
//! The heights/setup-frame audit of 2026-06-12 (finding 4, sentried by
//! `identity_setup_emission_frame_audit.rs`) moved `OpContext::stock_bbox`
//! onto `heights_stock_bbox` — world for identity setups, zero-rooted
//! local for non-identity ones — because that is the frame the toolpath
//! actually emits in. `SetupEvalContext::safe_z` was left behind on
//! `local_stock_bbox.max.z`, so within one `HeightContext` the depth
//! ladder anchors in the emission frame while the retract plane is
//! floored in the local one.
//!
//! `SetupEvalContext`'s own doc claimed the leftover was harmless —
//! "a conservatively-higher floor (never below the world stock top for
//! identity setups …) is always safe". Both halves of that are wrong,
//! and in opposite directions, because the local top is the stock
//! *thickness* while the world top is `origin_z + thickness`:
//!
//! - `origin_z > SAFE_Z_CLEARANCE_MM` makes the floor land **inside**
//!   the stock. It is not conservative, it is a rapid through material.
//! - `origin_z < 0` makes it needlessly high. On wanaka200 that is
//!   `25 + 5 = 30` against a world stock top of `+7`, which is what put
//!   five entire depth levels in air ahead of the first cutting one.
//!
//! Non-identity setups are unaffected either way: for them
//! `heights_stock_bbox == local_stock_bbox`, so the two readings agree.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, SAFE_Z_CLEARANCE_MM, StockSource,
};
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, SetupEvalContext, ToolpathConfig};

const STOCK_THICKNESS: f64 = 12.0;

/// Flat two-triangle surface at `z`, spanning XY [10, 50] x [10, 50].
fn flat_quad_mesh(z: f64) -> TriangleMesh {
    let verts = vec![
        P3::new(10.0, 10.0, z),
        P3::new(50.0, 10.0, z),
        P3::new(50.0, 50.0, z),
        P3::new(10.0, 50.0, z),
    ];
    let tris = vec![[0u32, 1, 2], [0, 2, 3]];
    TriangleMesh::from_raw(verts, tris)
}

/// Identity setup, stock `STOCK_THICKNESS` thick rooted at `origin_z`,
/// with a flat surface 6 mm below the world stock top.
fn build_session(origin_z: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let stock = StockConfig {
        x: 60.0,
        y: 60.0,
        z: STOCK_THICKNESS,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let world_top = origin_z + STOCK_THICKNESS;
    let model = LoadedModel {
        id: 0,
        name: "flat_plate".to_owned(),
        mesh: Some(Arc::new(flat_quad_mesh(world_top - 6.0))),
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://flat_plate.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let adaptive = Adaptive3dConfig {
        depth_per_pass: 2.0,
        stepover: 2.0,
        ..Adaptive3dConfig::default()
    };

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Rough".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(adaptive),
        dressups: DressupConfig::for_op(OperationType::Adaptive3d),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

fn identity_safe_z(session: &ProjectSession) -> f64 {
    SetupEvalContext::build(session, FaceUp::Top, ZRotation::Deg0).safe_z
}

/// What `effective_safe_z` would return for a given stock top — the raw
/// user value unless the clearance floor out-votes it.
fn floored(raw: f64, stock_top: f64) -> f64 {
    raw.max(stock_top + SAFE_Z_CLEARANCE_MM)
}

/// Highest Z any emitted move reaches — the retract plane, measured off
/// the motion rather than off the plan.
fn max_emitted_z(session: &mut ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");
    let tp = &result.op_data.annotated().toolpath;
    tp.moves
        .iter()
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// The half the doc got backwards: a positive stock origin puts the
/// local-rooted floor *below* the world stock top, so the retract plane
/// is inside the material.
#[test]
fn a_positive_stock_origin_does_not_put_the_retract_plane_inside_the_stock() {
    let origin_z = 20.0;
    let world_top = origin_z + STOCK_THICKNESS; // 32.0
    let mut session = build_session(origin_z);

    let safe_z = identity_safe_z(&session);
    assert!(
        safe_z >= floored(session.post_config().safe_z, world_top),
        "safe_z {safe_z} is below world stock top {world_top} + clearance \
         {SAFE_Z_CLEARANCE_MM}; the local-rooted floor reads the stock \
         THICKNESS ({STOCK_THICKNESS}), not the world top"
    );

    let top = max_emitted_z(&mut session);
    assert!(
        top >= world_top,
        "highest emitted Z {top} is below the world stock top {world_top} — \
         every rapid on this toolpath is cutting through material"
    );
}

/// The half the doc got right in direction but wrong in magnitude: a
/// negative stock origin lifts the floor above what clearing the stock
/// requires, and every retract pays the difference.
///
/// `origin_z = -thickness` is the documented 2D convention (stock top at
/// world Z0), so this is the ordinary case, not a corner one. The raw
/// `post.safe_z` of 10 already clears a world top of 0; the local-rooted
/// floor overrides it with `12 + 5 = 17`.
#[test]
fn a_negative_stock_origin_does_not_lift_the_retract_plane_above_the_stock() {
    let origin_z = -STOCK_THICKNESS;
    let world_top = origin_z + STOCK_THICKNESS; // 0.0
    let mut session = build_session(origin_z);
    let raw = session.post_config().safe_z;

    let expected = floored(raw, world_top);
    let local_rooted = floored(raw, STOCK_THICKNESS);
    assert!(
        (expected - local_rooted).abs() > 1e-9,
        "fixture is degenerate: both frames give {expected}"
    );

    let safe_z = identity_safe_z(&session);
    assert!(
        (safe_z - expected).abs() < 1e-9,
        "safe_z {safe_z} should be floored against the world stock top \
         {world_top} (giving {expected}); the local-rooted floor reads the \
         stock THICKNESS instead, giving {local_rooted}"
    );

    let top = max_emitted_z(&mut session);
    assert!(
        (top - expected).abs() < 1e-9,
        "retract plane sits at {top}, {} mm of dead lift above the {expected} \
         this stock needs",
        top - expected
    );
}

/// Non-identity setups emit in the local frame, where the two readings
/// are the same bbox. This arm must not move when the fix lands.
#[test]
fn a_flipped_setup_keeps_its_local_rooted_floor() {
    let session = build_session(-12.0);
    let flipped = SetupEvalContext::build(&session, FaceUp::Bottom, ZRotation::Deg0);
    assert!(
        flipped.local_to_global.is_some(),
        "fixture must be non-identity for this arm to mean anything"
    );
    let raw = session.post_config().safe_z;
    let expected = floored(raw, flipped.local_stock_bbox.max.z);
    assert!(
        (flipped.safe_z - expected).abs() < 1e-9,
        "non-identity safe_z {} should stay floored on the local top \
         ({expected})",
        flipped.safe_z
    );
}
