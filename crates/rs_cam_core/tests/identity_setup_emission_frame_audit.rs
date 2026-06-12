//! Heights/setup-frame audit 2026-06-12, finding 4 — identity setups
//! must feed ops the emission-frame (world) stock bbox on the session
//! path, matching the GUI controller.
//!
//! Pre-fix `session/compute.rs` passed the zero-rooted
//! `local_stock_bbox` as `OpContext::stock_bbox` for every setup.
//! Identity setups emit toolpaths in the *world* frame, so any op that
//! anchors depth on `ctx.stock_bbox.max.z` (adaptive3d's `stock_top_z`,
//! face, drill) planned its Z levels `-origin_z` too high whenever the
//! stock origin was non-zero — e.g. with stock spanning world
//! Z=[-12, 0], adaptive3d started its passes at local Z=12-DPP, ten or
//! more millimetres above the actual material, and the CLI/headless
//! output diverged from the GUI for the same project file.
//!
//! The fixture mirrors the wanaka200 research case in miniature:
//! a flat mesh surface at world Z=-6 inside stock world Z=[-12, 0],
//! identity setup. All cutting moves must stay at or below the world
//! stock top (0.0); pre-fix the first pass landed near +10.

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
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::MoveType;

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

fn build_identity_origin_session() -> ProjectSession {
    build_identity_origin_session_with_heights(HeightsConfig::default())
}

fn build_identity_origin_session_with_heights(heights: HeightsConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // Stock world Z=[-12, 0]: zero-rooted local top (12) differs from the
    // world top (0) by exactly -origin_z, which is what finding 4 is about.
    let stock = StockConfig {
        x: 60.0,
        y: 60.0,
        z: 12.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let mesh = flat_quad_mesh(-6.0);
    let model = LoadedModel {
        id: 0,
        name: "flat_plate".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
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
        heights,
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
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

/// Lateral (non-plunge) cutting-move Z values of toolpath 0.
fn lateral_cut_zs(session: &mut ProjectSession) -> Vec<f64> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");
    let tp = &result.op_data.annotated().toolpath;
    let mut prev = P3::new(0.0, 0.0, 0.0);
    let mut zs = Vec::new();
    for m in &tp.moves {
        let lateral = (m.target.x - prev.x).abs() > 1e-9 || (m.target.y - prev.y).abs() > 1e-9;
        if !matches!(m.move_type, MoveType::Rapid) && lateral {
            zs.push(m.target.z);
        }
        prev = m.target;
    }
    zs
}

#[test]
fn identity_setup_adaptive3d_anchors_on_world_stock_top() {
    let mut session = build_identity_origin_session();
    // Lateral cutting moves only: vertical feed descents (plunge to the
    // first pass) legitimately travel through air above the stock, so a
    // pure-Z move ending at feed_z is not evidence of a frame bug.
    let cut_zs = lateral_cut_zs(&mut session);
    assert!(
        !cut_zs.is_empty(),
        "adaptive3d should emit lateral cutting moves on the flat plate"
    );

    let max_cut_z = cut_zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_cut_z = cut_zs.iter().copied().fold(f64::INFINITY, f64::min);

    // World stock top is 0.0. Pre-fix the op received the zero-rooted
    // local bbox (top = 12.0) and planned its first pass near Z=+10,
    // i.e. in the air far above the stock.
    assert!(
        max_cut_z <= 0.0 + 1e-6,
        "identity-setup adaptive3d must not cut above the world stock top \
         (0.0); got max cut Z = {max_cut_z:.3} — the op was fed the \
         zero-rooted local bbox instead of the emission-frame bbox"
    );

    // Sanity: the rough actually descends toward the surface at -6.
    assert!(
        min_cut_z <= -1.0,
        "expected passes stepping down toward the surface at Z=-6; \
         got min cut Z = {min_cut_z:.3}"
    );
}

/// Heights audit finding 2 — a user-pinned `bottom_z` must floor the
/// adaptive3d Z-level plan. Pre-fix adaptive3d never read
/// `heights.top_z`/`bottom_z`: the plan ran from the stock bbox top down
/// to the surface-heightmap minimum, and on open meshes (holes) that
/// minimum is the mesh-bbox floor — there was no lever to stop the rough
/// diving below a chosen depth.
#[test]
fn pinned_bottom_z_floors_adaptive3d_plan() {
    use rs_cam_core::compute::config::HeightMode;

    // Surface at -6; unpinned, the final level lands near -6 + stock_to_leave.
    let mut unpinned = build_identity_origin_session();
    let unpinned_min = lateral_cut_zs(&mut unpinned)
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    assert!(
        unpinned_min < -4.0,
        "fixture sanity: unpinned rough should descend toward the surface \
         at -6 (got min lateral Z = {unpinned_min:.3})"
    );

    // Pin bottom_z at -3: no lateral cut may land below it.
    let pinned_floor = -3.0;
    let heights = HeightsConfig {
        bottom_z: HeightMode::Manual(pinned_floor),
        ..HeightsConfig::default()
    };
    let mut pinned = build_identity_origin_session_with_heights(heights);
    let zs = lateral_cut_zs(&mut pinned);
    assert!(
        !zs.is_empty(),
        "pinned rough should still emit passes above the floor"
    );
    let pinned_min = zs.into_iter().fold(f64::INFINITY, f64::min);
    assert!(
        pinned_min >= pinned_floor - 1e-6,
        "heights.bottom_z = {pinned_floor} must floor the Z-level plan; \
         got min lateral cut Z = {pinned_min:.3}"
    );
}
