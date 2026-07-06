//! End-to-end smoke test for [`ProjectSession::recommend_clearing_strategy`]
//! — the strategy advisor's orchestration on a real terrain + adaptive3d op
//! (`planning/STRATEGY_ADVISOR_2026-06-17.md`).
//!
//! Proves the glue end-to-end: resolve the generation inputs ONCE, then for
//! each candidate clearing strategy run Suggest (load-limited params) + plan
//! the clearing toolpath off that shared resolution, and rank them by
//! accel-aware wall-clock. Asserts structural invariants (a recommendation is
//! produced, candidates are measurable and sorted, the regime/why are
//! populated) rather than a specific winner — the winner depends on the
//! fixture machine's acceleration, which the `strategy_advisor` unit sentry
//! (`winner_flips_with_machine_acceleration`) pins directly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::repo_root;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, ToolpathConfig};
use rs_cam_core::strategy_advisor::LoadRegime;

/// Load `ux_3d_terrain.toml` and add the AS013-shape adaptive3d toolpath
/// (6 mm end mill, depth_per_pass=3, stepover=1.2) — the same fixture the
/// F-027/F-029 adaptive3d sentries use.
fn build_terrain_adaptive3d_session() -> ProjectSession {
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

#[test]
fn recommend_clearing_strategy_ranks_candidates_on_terrain() {
    let session = build_terrain_adaptive3d_session();
    let cancel = AtomicBool::new(false);

    let rec = session
        .recommend_clearing_strategy(0, &cancel)
        .expect("resolve must succeed for the AS013 adaptive3d op")
        .expect("advisor must return a recommendation for an adaptive3d op");

    // Chose one of the candidate strategies.
    assert!(
        matches!(
            rec.chosen,
            ClearingStrategy::ContourParallel | ClearingStrategy::ContourSpiral
        ),
        "chosen must be in the candidate set, got {:?}",
        rec.chosen
    );
    // At least one candidate planned a measurable (positive wall-clock) path.
    assert!(!rec.ranked.is_empty(), "ranked must be non-empty");
    assert!(
        rec.ranked.iter().all(|r| r.wall_clock_s > 0.0),
        "every ranked candidate must have a positive wall-clock estimate"
    );
    // Ranked ascending by wall-clock (fastest first).
    assert!(
        rec.ranked
            .windows(2)
            .all(|w| w[0].wall_clock_s <= w[1].wall_clock_s),
        "ranked must be sorted ascending by wall-clock: {:?}",
        rec.ranked
    );
    // The chosen strategy is the fastest measurable one (no geometry override
    // in this fixture), so it heads the ranked list.
    assert_eq!(
        rec.ranked.first().map(|r| r.strategy),
        Some(rec.chosen),
        "with no geometry override the chosen strategy must be the fastest ranked"
    );
    // The "why" is populated and the regime is a modelled value.
    assert!(!rec.reason.is_empty(), "recommendation must carry a reason");
    assert!(matches!(
        rec.regime,
        LoadRegime::ToolLimited | LoadRegime::MachineLimited | LoadRegime::Unconstrained
    ));
    // Speed margin is well-formed (≥ 1.0 means the choice is the faster one).
    assert!(
        rec.time_ratio_vs_runner_up >= 1.0,
        "the chosen (fastest, non-forced) strategy can't be slower than the runner-up; got {}",
        rec.time_ratio_vs_runner_up
    );
}
