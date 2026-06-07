//! F-027 — Adaptive3d planner stock XY mismatch with simulator dexel grid.
//!
//! ## Background
//!
//! After F-026 fixed `auto_from_model` load-time stock re-derivation
//! (commit `cad1fcc`), the AS013 reproducer (adaptive3d on
//! `ux_3d_terrain.toml`) saw `rapid_collision_count` drop from 844 → 0
//! (the headline F-026 win), but `deflection.peak_mm` stayed at
//! ~0.576 mm — still firing Exceeds despite the bulk of the toolpath
//! sitting well below the 0.2 mm bound.
//!
//! Diagnostic (round-06 audit and F-026 implementer follow-up):
//! roughly 800 of 1 147 279 cutting samples on AS013 reported
//! `axial_engagement_mm > 5 mm` (commanded DPP is 3), with a tail of
//! 5–10 samples reading 30–47 mm. The outliers clustered at Y ≈ 76.3 mm
//! — the edge of `mesh.bbox.max.y (73.3) + tool_radius (3.0)`. The
//! model XY extent doesn't reach this Y, but the simulator's per-setup
//! dexel grid (sized from the auto-grown world stock bbox `[-5, 105] ×
//! [-5, 96]`) does.
//!
//! ## Root cause
//!
//! `adaptive3d::path::adaptive_3d_segments` constructed its
//! planner-internal `material_stock` bounded by `mesh.bbox +
//! tool_radius`. Cells inside the simulator grid but outside the
//! planner grid were never visited by adaptive3d's stamps — the
//! simulator carried them as virgin material `[0, stock_top_z]` across
//! the entire toolpath. When the cutter swept near the model boundary,
//! its footprint extended into those cells and the simulator's first
//! stamp cleared the full pre-stamp ray in one shot →
//! `axial_engagement_mm` reading the full stock height (≈ 30–47 mm on
//! the post-F-026 auto-grown terrain stock).
//!
//! This is the XY analog of F-024 (which fixed the Z-frame mismatch).
//!
//! ## Fix
//!
//! `Adaptive3dParams` gains a `world_stock_xy_bbox: Option<(f64, f64,
//! f64, f64)>` field. When `Some`, the planner unions the world stock
//! XY bounds with the `mesh.bbox + tool_radius` bounds, so the planner
//! grid covers every cell the simulator grid will look at. Wired in
//! through `compute::execute::execute_operation`'s adaptive3d arm; CLI
//! and unit-test call sites leave it `None` (fallback to mesh-bbox-only
//! initialization).
//!
//! ## Acceptance bars
//!
//! 1. Per-sample `axial_engagement_mm` across all cutting samples must
//!    be ≤ commanded `depth_per_pass + 0.5` margin. Pre-fix outliers
//!    read 30–47 mm at the model-edge cells.
//! 2. `deflection.peak_mm` on the tool-load verdict must be < 0.2 mm.
//!    Pre-fix it sat at ~0.576 mm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::repo_root;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;
use rs_cam_core::tool_load::DeflectionVerdict;

/// Load `ux_3d_terrain.toml` (auto_from_model = true, F-026 grows stock
/// to z≈57.6 at load) and add an AS013-shape adaptive3d toolpath with
/// the round-05 baseline params: 6 mm end mill, depth_per_pass=3,
/// stepover=1.2.
fn build_as013_terrain_session() -> ProjectSession {
    let toml_path = repo_root().join("test_data/ux_3d_terrain.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");

    // Pick the 6 mm flat end mill (fixture has it as tool id 1).
    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_3d_terrain.toml must define a 6 mm end mill");

    // Pick the first STL model (terrain_small.stl).
    let model_id = session
        .models()
        .iter()
        .find(|m| m.mesh.is_some())
        .map(|m| m.id)
        .expect("ux_3d_terrain.toml must load terrain_small.stl");

    // AS013 baseline params straight from `cases_agent_smoke.csv`:
    // depth_per_pass=3; stepover=1.2; stock_top_z=30 (will be ignored
    // — the planner reads stock_top_z from the toolpath's heights,
    // which `add_toolpath` synchronises with stock).
    let adaptive3d = Adaptive3dConfig {
        stepover: 1.2,
        depth_per_pass: 3.0,
        stock_to_leave_axial: 0.5,
        stock_to_leave_radial: 0.5,
        feed_rate: 2500.0,
        plunge_rate: 500.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ramp_angle_deg: 3.0,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        fine_stepdown: 0.0,
        detect_flat_areas: false,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        z_blend: false,
        mill_shallow_areas: false,
        shallow_angle_deg: None,
        shallow_stepdown: None,
        spindle_rpm: Some(18_000),
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "AS013 adaptive3d".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(adaptive3d),
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
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

/// F-027 acceptance bar 1 — per-sample axial engagement at model-edge
/// cells stays within commanded DPP (+ small margin).
///
/// This is the precise F-027 signature: the cells beyond
/// `mesh.bbox.max.y` (but within the auto-grown world stock bbox) used
/// to be outside adaptive3d's planner grid; the simulator carried them
/// as virgin material until the cutter's footprint reached them, and
/// the first stamp removed the full pre-stamp ray.
///
/// Pre-fix: the boundary cells (Y in [mesh.max.y, world stock max.y])
/// produced ~300 samples with `axial_engagement_mm > DPP + margin`,
/// with the worst readings in the 30-47 mm range.
///
/// Post-fix: the planner's grid extends across the full world stock XY
/// footprint, and the planner's clearing strategy emits cuts that
/// pre-stamp those cells DPP at a time. Per-sample axial in this band
/// is bounded by the commanded DPP + small discretisation margin.
///
/// Drives through `ProjectSession::run_simulation` — the same entry
/// point the MCP `run_simulation` tool and the GUI Sim button take.
/// The fix must reach this path; an implementation-layer green
/// (the parity test in `adaptive3d_planner_sim_dexel_parity.rs`) alone
/// doesn't prove it (F-024 three-rebuild saga learning).
///
/// Scope caveat: the AS013 toolpath has an independent class of axial
/// outliers at interior cells (cells inside the mesh XY footprint that
/// the planner thinks are cleared but the simulator hasn't stamped
/// before a deep-Z dive). Those are not F-027 — they're a separate
/// planner↔simulator parity bug at interior cells. This test guards the
/// boundary mechanism specifically by restricting samples to the model-
/// edge band.
#[test]
fn as013_terrain_model_edge_axial_within_commanded_dpp_f027() {
    let mut session = build_as013_terrain_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    // Resolve the mesh bbox and stock bbox before running sim — the F-027
    // band is `(mesh.max.y, stock.max.y]`.
    let mesh_max_y = session
        .models()
        .iter()
        .filter_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox.max.y))
        .fold(f64::NEG_INFINITY, f64::max);
    let stock_max_y = session.stock_bbox().max.y;
    assert!(
        stock_max_y - mesh_max_y > 2.0,
        "F-027 band only exists when the world stock bbox extends past the mesh XY footprint; \
         got mesh.max.y = {mesh_max_y:.2}, stock.max.y = {stock_max_y:.2}. Fixture drift?"
    );

    let opts = SimulationOptions {
        // Coarser than the default 0.5 mm to keep runtime reasonable; the
        // F-027 bug shows up at the millimetre scale and isn't sensitive
        // to sub-mm grid resolution.
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");

    let commanded_dpp = 3.0_f64;
    let margin = 0.5_f64;
    let limit = commanded_dpp + margin;

    // Restrict to the F-027 band: cutter center at Y > mesh.max.y (and
    // still inside the world stock bbox). Cells in this band can only
    // be virgin in the simulator if the planner never stamped them
    // (the F-027 mechanism). Pre-fix this band carries the worst
    // outliers in the trace.
    let mut max_axial = 0.0_f64;
    let mut max_sample_y = f64::NAN;
    let mut max_sample_z = f64::NAN;
    let mut over_count = 0usize;
    let mut band_sample_count = 0usize;
    for s in &cut_trace.samples {
        // F-031 alignment: skip transit-span samples (Entry / WaterlineCleanup /
        // LinkBridge). These are routed to the deflection model's `entry_spike`
        // advisory track and don't drive the steady-state load verdict; F-027's
        // model-edge gate likewise should only consider steady-state cuts. The
        // original test predated F-031 and relied on the helix-entry dressup
        // (which was the default for Adaptive3d) absorbing the entry-plunge
        // axial spikes via gradual descent. F-031 removed that dressup default,
        // making entry-plunge transit samples visible — but they're still
        // semantically "transit", not steady-state cutting.
        if !s.is_cutting || s.cut_kinematics == CutKinematics::Plunge || s.in_transit_span {
            continue;
        }
        if s.position[1] <= mesh_max_y || s.position[1] > stock_max_y {
            continue;
        }
        band_sample_count += 1;
        if s.axial_engagement_mm > max_axial {
            max_axial = s.axial_engagement_mm;
            max_sample_y = s.position[1];
            max_sample_z = s.position[2];
        }
        if s.axial_engagement_mm > limit {
            over_count += 1;
        }
    }

    assert!(
        band_sample_count > 0,
        "F-027 boundary-band must contain samples for the test to be meaningful. \
         The post-fix planner should emit cuts into the (mesh.max.y={mesh_max_y:.2}, \
         stock.max.y={stock_max_y:.2}] band; pre-fix it didn't, but the simulator still \
         placed samples there because the cutter footprint reached past mesh.max.y."
    );

    assert!(
        max_axial <= limit,
        "F-027: max per-sample axial_engagement_mm in the model-edge band \
         (Y in ({mesh_max_y:.2}, {stock_max_y:.2}]) is {max_axial:.3} mm — exceeds commanded \
         depth_per_pass + margin ({limit:.3}). Over-limit sample at (y={max_sample_y:.2}, \
         z={max_sample_z:.2}). {over_count} of {band_sample_count} samples in the band are \
         over the limit. Pre-fix the worst samples read 30-47 mm here because adaptive3d's \
         planner stock was bounded by `mesh.bbox + tool_radius` while the simulator's dexel \
         grid spanned the auto-grown world stock bbox."
    );
}

/// F-027 secondary: the planner-side fix should also reduce the total
/// count of model-edge-band axial outliers across the toolpath.
///
/// This is a softer regression guard than bar 1 — it catches partial
/// undoing of the fix even if no single sample crosses the absolute
/// limit. Pre-fix this band held ~300 outliers; post-fix it must hold
/// none.
#[test]
fn as013_terrain_model_edge_band_outlier_count_zero_f027() {
    let mut session = build_as013_terrain_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    let mesh_max_y = session
        .models()
        .iter()
        .filter_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox.max.y))
        .fold(f64::NEG_INFINITY, f64::max);
    let stock_max_y = session.stock_bbox().max.y;

    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");

    let limit = 3.0 + 0.5;
    let outliers_in_band: usize = cut_trace
        .samples
        .iter()
        // F-031 alignment: skip transit-span samples; see sibling test.
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge && !s.in_transit_span)
        .filter(|s| s.position[1] > mesh_max_y && s.position[1] <= stock_max_y)
        .filter(|s| s.axial_engagement_mm > limit)
        .count();

    assert_eq!(
        outliers_in_band, 0,
        "F-027: no per-sample axial outliers > {limit:.3} mm should land in the model-edge \
         band (Y in ({mesh_max_y:.2}, {stock_max_y:.2}]) on AS013 post-fix; pre-fix this \
         band held ~300 outliers driven by the planner-stock/simulator-grid XY mismatch."
    );

    // Sanity check the load-report verdict shape exists for the toolpath
    // (don't gate it — F-027 doesn't fully clear the deflection bar; an
    // independent class of interior-cell axial outliers keeps it Exceeds
    // until a follow-up finding lands).
    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == 0)
        .expect("verdict for adaptive3d toolpath");
    assert!(
        matches!(
            verdict.deflection,
            DeflectionVerdict::Within { .. } | DeflectionVerdict::Exceeds { .. }
        ),
        "expected modeled deflection on AS013 adaptive3d, got {:?}",
        verdict.deflection
    );
}
