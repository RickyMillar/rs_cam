//! F-031 — Adaptive3d planner↔dressup entry-style parity gap (was F-029).
//!
//! ## Background
//!
//! After F-027 fixed the model-edge band of axial outliers and F-029's
//! partial landing added a cleanup-raster per-cell DPP clamp + diagnostic
//! probe, AS013 still tripped the deflection gate at ~0.66 mm. F-029's
//! diagnostic confirmed the planner's `material_stock` ended fully cleared
//! at the worst cell (top = 0.5 mm) — the residual was a sim-side parity
//! gap, not a planner-side stamping issue. F-031 was opened to carry that
//! scope.
//!
//! ## Root cause (per F-031 investigation, 2026-05-26)
//!
//! The planner-side `material_stock` is correct, and the simulator's
//! per-segment stamping mechanics (`stamp_segment_on_grid`) are identical
//! to the planner's. The divergence is in **what segments are stamped**:
//!
//! `DressupConfig::for_op(Adaptive3d)` returns `entry_style =
//! DressupEntryStyle::Helix` by default (via the `prefer_helix` override
//! in `normalize_for_op`). The dressup pass (`apply_dressups` →
//! `apply_entry`) walks the planner-emitted toolpath, detects each
//! plunge (Linear + downward + no XY change), and **replaces it with a
//! helix** at radius ≈ `helix_radius` around the entry XY.
//!
//! But the planner-side `stamp_emitted_segment(Adaptive3dSegment::Rapid)`
//! stamps a *vertical cylinder* at the entry XY — what the planner-
//! emitted peck-plunge feeds would produce. After dressup transforms
//! those plunges into helices, the simulator's actual stamps follow the
//! helical path: the centre of the entry cylinder receives only partial-
//! coverage stamps (the helix passes by at radius 2 mm, not through the
//! axis) and cells at distance `helix_radius + tool_radius` outside the
//! planner's stamping radius receive new stamps that the planner didn't
//! mirror.
//!
//! The planner's `material_stock` is now out-of-sync with the simulator's
//! actual swept-tube coverage. Subsequent clearing passes that the
//! planner believes will sweep through cleared air actually bite into
//! uncut material — producing per-sample `axial_engagement_mm` readings
//! up to ~44 mm (full stock height minus a few clearing passes) on a 3
//! mm-commanded DPP. The deflection gate then trips on those samples.
//!
//! ## Fix
//!
//! `DressupConfig::normalize_for_op(Adaptive3d)` now forces
//! `entry_style = DressupEntryStyle::None`. The planner-emitted
//! peck-plunge feeds pass through the dressup unchanged, so the
//! simulator's stamping matches the planner's `stamp_emitted_segment`
//! vertical-cylinder shape. Users who want Helix entries on Adaptive3d
//! can set `Adaptive3dEntryStyle::Helix` at the planner level (where
//! `segments_to_toolpath` emits a helix natively and the planner's
//! `stamp_emitted_segment` is also a single-column approximation but
//! reasonably close because helix radius is small relative to tool
//! radius), or override `DressupConfig.entry_style` post-construction.
//!
//! ## Acceptance bar
//!
//! 1. Whole-toolpath worst-case `axial_engagement_mm` across all
//!    lateral (non-plunge, non-transit) cutting samples ≤
//!    `depth_per_pass + 0.5` margin. Transit-span samples (entry /
//!    waterline cleanup / link bridges) are excluded — they're the same
//!    samples the deflection model already filters via
//!    `is_steady_state_for_gate`.
//! 2. `deflection.peak_mm < 0.2` on the tool-load verdict.
//!
//! Pre-fix on AS013: max axial ≈ 44.8 mm (transit sample), steady-state
//! max axial = 3.13 mm, deflection peak = 0.66 mm.
//! Post-fix on AS013: max steady-state axial ≤ 3.5, deflection ≤ 0.13.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::collapsible_if
)]

mod common;
use common::repo_root;
use rs_cam_core::ids::ToolpathId;

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

/// Load `ux_3d_terrain.toml` and add an AS013-shape adaptive3d toolpath.
fn build_as013_terrain_session() -> ProjectSession {
    let toml_path = repo_root().join("test_data/ux_3d_terrain.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");

    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_3d_terrain.toml must define a 6 mm end mill");

    let model_id = session
        .models()
        .iter()
        .find(|m| m.mesh.is_some())
        .map(|m| m.id)
        .expect("ux_3d_terrain.toml must load terrain_small.stl");

    let adaptive3d = Adaptive3dConfig {
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
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
        id: ToolpathId(0),
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
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

fn run_as013_simulation() -> ProjectSession {
    let mut session = build_as013_terrain_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    let cancel2 = AtomicBool::new(false);
    session
        .run_simulation(&opts, &cancel2)
        .expect("simulation completes");

    session
}

/// F-031 acceptance bar 1 — whole-toolpath worst-case
/// `axial_engagement_mm` across all *steady-state* lateral (non-plunge,
/// non-transit) cutting samples ≤ commanded `depth_per_pass + 0.5` margin.
///
/// Drives through `ProjectSession::run_simulation` (the production entry
/// point the MCP `run_simulation` tool and the GUI Sim button take). The
/// transit-span filter mirrors `tool_load::deflection`'s
/// `is_steady_state_for_gate` — transit moves (Entry / WaterlineCleanup
/// / LinkBridge / DressupArtifact) can legitimately register high
/// per-sample axial readings without indicating a tool-load problem,
/// because the deflection model routes them to the `entry_spike`
/// advisory track instead of the steady-state peak.
///
/// Pre-fix on AS013: max steady-state axial = 3.13 mm (within bar) but
/// 282 transit samples exceed the bar; the headline "worst sample"
/// figure (44.8 mm) was a waterline-cleanup transit sample that the
/// planner-↔-dressup helix mismatch left mid-toolpath at high residual
/// material height.
#[test]
fn as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031() {
    let session = run_as013_simulation();

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");

    let commanded_dpp = 3.0_f64;
    let margin = 0.5_f64;
    let limit = commanded_dpp + margin;

    let mut max_axial = 0.0_f64;
    let mut max_sample_pos = [f64::NAN, f64::NAN, f64::NAN];
    let mut over_count = 0usize;
    let mut sample_count = 0usize;
    for s in &cut_trace.samples {
        if !s.is_cutting || s.cut_kinematics == CutKinematics::Plunge || s.in_transit_span {
            continue;
        }
        sample_count += 1;
        if s.axial_engagement_mm > max_axial {
            max_axial = s.axial_engagement_mm;
            max_sample_pos = s.position;
        }
        if s.axial_engagement_mm > limit {
            over_count += 1;
        }
    }
    assert!(
        sample_count > 0,
        "expected steady-state cutting samples on AS013"
    );
    assert!(
        max_axial <= limit,
        "F-031: steady-state max per-sample axial_engagement_mm = {max_axial:.3} mm \
         exceeds commanded depth_per_pass + margin ({limit:.3}). Worst sample at \
         (x={:.2}, y={:.2}, z={:.2}). {over_count} of {sample_count} steady-state \
         samples exceed the limit.",
        max_sample_pos[0],
        max_sample_pos[1],
        max_sample_pos[2],
    );
}

/// F-031 acceptance bar 2 — AS013 adaptive3d deflection reads its TRUE
/// engagement-driven value, not the un-stamped-cell parity artifact.
///
/// The F-031 parity fix closed a planner-↔-dressup helix gap that left
/// cells un-stamped between intermediate Z passes and inflated deflection
/// to a false ~0.66 mm (Exceeds). With the gap closed the reading reflects
/// the real geometry.
///
/// Milling-Kc calibration (2026-06-17, `MILLING_KC_FACTOR = 2.7`,
/// `material.rs`) then lifted the deflection force ~2.7×, moving the true
/// reading ~0.118 → ~0.32 mm — past the 200 µm bound. AS013 at
/// `depth_per_pass=3` in hardwood is therefore genuinely **tool-limited**
/// under milling Kc (the same regime flip the wanaka Back Rough shows;
/// pre-calibration this read ~0.12 mm `Within`). This is a post-sim GATE
/// reading at the raw configured DPP — no Suggest back-off — so the honest
/// verdict is `Exceeds`.
///
/// The test now pins both invariants: the parity gap stays closed (the
/// reading is the real ~0.32 mm, NOT the ~0.66 mm un-stamped artifact) AND
/// the milling-Kc physics (`Exceeds`, ~0.32 mm).
#[test]
fn as013_terrain_deflection_milling_kc_tool_limited_f031() {
    let session = run_as013_simulation();

    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == ToolpathId(0))
        .expect("verdict for adaptive3d toolpath");

    let peak_mm = match &verdict.deflection {
        DeflectionVerdict::Within { peak_mm, .. } | DeflectionVerdict::Exceeds { peak_mm, .. } => {
            *peak_mm
        }
        DeflectionVerdict::Unmodeled { reason } => {
            panic!(
                "F-031: expected modeled deflection on AS013 adaptive3d, got Unmodeled: {reason:?}"
            );
        }
    };

    // The reading is the real milling-Kc value (~0.32 mm): far below the
    // ~0.66 mm un-stamped-cell artifact (parity gap stays closed) and well
    // above an artificially-low reading (milling-Kc force is applied).
    assert!(
        (0.25..=0.45).contains(&peak_mm),
        "F-031: AS013 deflection should read the true milling-Kc value (~0.32 mm); got \
         peak_mm = {peak_mm:.4} mm. Above ~0.45 mm would signal the ~0.66 mm un-stamped-cell \
         parity artifact has returned; below ~0.25 mm would signal the milling-Kc force is \
         under-reading."
    );

    // Under milling Kc, AS013 at DPP=3 in hardwood is genuinely tool-limited.
    assert!(
        matches!(verdict.deflection, DeflectionVerdict::Exceeds { .. }),
        "F-031: AS013 deflection is tool-limited under milling Kc (peak ~0.32 mm > 200 µm bound); \
         expected Exceeds, got {:?}",
        verdict.deflection
    );
}
