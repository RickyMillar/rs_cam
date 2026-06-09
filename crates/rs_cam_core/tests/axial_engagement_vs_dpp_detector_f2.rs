//! F2.4 (defect class C3) — phantom-axial detector across op families.
//!
//! A3's audit found that no harness surfaced per-sample
//! `axial_engagement_mm` against the commanded depth-per-pass, which is
//! how a transit sample carrying "stock height we're flying over" as
//! cutter engagement (the WANAKA 622 µm DeflectionSetupLocked sample,
//! same family as F-024) could reach the gates undetected.
//!
//! Detector bar: over a matrix of 2D op families generated through the
//! production funnel (`ProjectSession::generate_toolpath` →
//! `run_simulation` with metrics — the same path the GUI Sim button and
//! the MCP `run_simulation` tool take), NO steady-state cutting sample
//! may report `axial_engagement_mm > 1.5 × commanded depth_per_pass`.
//! Steady-state mirrors `tool_load::locality::is_steady_state_for_gate`'s
//! cheap form: cutting, non-plunge kinematics, not `in_transit_span`.
//!
//! Adaptive3d is covered by the stricter whole-toolpath bar in
//! `adaptive3d_interior_cell_parity_f029.rs` (F-031).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::repo_root;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, PocketConfig, ProfileConfig, TraceConfig, ZigzagConfig,
};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;

/// Detector tolerance: a steady-state sample may read up to 1.5× the
/// commanded depth-per-pass (grid discretisation + Z-blend rounding),
/// anything beyond is phantom engagement leaking past the transit
/// filters.
const AXIAL_VS_DPP_LIMIT: f64 = 1.5;

fn op_matrix() -> Vec<(&'static str, OperationConfig)> {
    vec![
        (
            "pocket",
            OperationConfig::Pocket(PocketConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..PocketConfig::default()
            }),
        ),
        (
            "profile",
            OperationConfig::Profile(ProfileConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..ProfileConfig::default()
            }),
        ),
        (
            "adaptive",
            OperationConfig::Adaptive(AdaptiveConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..AdaptiveConfig::default()
            }),
        ),
        (
            "zigzag",
            OperationConfig::Zigzag(ZigzagConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..ZigzagConfig::default()
            }),
        ),
        (
            "trace",
            OperationConfig::Trace(TraceConfig {
                depth: 6.0,
                depth_per_pass: 2.0,
                ..TraceConfig::default()
            }),
        ),
    ]
}

#[test]
fn steady_state_axial_engagement_stays_within_commanded_dpp() {
    let toml_path = repo_root().join("test_data/ux_2d_pocket.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_2d_pocket");

    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_2d_pocket.toml must define a 6 mm end mill");
    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("ux_2d_pocket.toml must load demo_pocket.svg");

    let matrix = op_matrix();
    for (i, (name, op)) in matrix.iter().enumerate() {
        let op_type = op.op_type();
        let tc = ToolpathConfig {
            id: i,
            name: format!("detector {name}"),
            enabled: true,
            operation: op.clone(),
            dressups: DressupConfig::for_op(op_type),
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
        session.add_toolpath(0, tc).expect("add detector toolpath");
    }

    let cancel = AtomicBool::new(false);
    for (i, (name, _)) in matrix.iter().enumerate() {
        session
            .generate_toolpath(i, &cancel)
            .unwrap_or_else(|e| panic!("generate {name}: {e:?}"));
    }

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
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");

    let mut failures = Vec::new();
    for (i, (name, op)) in matrix.iter().enumerate() {
        let dpp = op
            .depth_per_pass()
            .unwrap_or_else(|| panic!("{name} must expose depth_per_pass"));
        let limit = dpp * AXIAL_VS_DPP_LIMIT;

        let mut peak = 0.0_f64;
        let mut peak_pos = [f64::NAN; 3];
        let mut over = 0usize;
        let mut steady = 0usize;
        for s in &cut_trace.samples {
            if s.toolpath_id != i
                || !s.is_cutting
                || s.cut_kinematics == CutKinematics::Plunge
                || s.in_transit_span
            {
                continue;
            }
            steady += 1;
            if s.axial_engagement_mm > peak {
                peak = s.axial_engagement_mm;
                peak_pos = s.position;
            }
            if s.axial_engagement_mm > limit {
                over += 1;
            }
        }
        assert!(steady > 0, "{name}: expected steady-state cutting samples");
        if peak > limit {
            failures.push(format!(
                "{name}: peak steady-state axial_engagement_mm {peak:.3} exceeds \
                 {AXIAL_VS_DPP_LIMIT}× commanded depth_per_pass ({limit:.3}); \
                 {over} of {steady} samples over, worst at \
                 (x={:.2}, y={:.2}, z={:.2})",
                peak_pos[0], peak_pos[1], peak_pos[2],
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "F2.4 phantom-axial detector tripped:\n{}",
        failures.join("\n")
    );
}
