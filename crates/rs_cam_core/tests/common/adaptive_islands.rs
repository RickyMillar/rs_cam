//! The six-island 2D Adaptive fixture (G-ADAPTORDER, G-ADAPTLINKLOAD).
//!
//! A 120 x 80 mm pocket with six round islands of radius 8 on a 3 x 2 grid,
//! a 6 mm flat end mill, stepover 2, Depth/Pass 3 over 6 mm. The session is
//! generated and then simulated (dexel stock, sim cell 0.5 mm, metrics on),
//! so the cut trace is the oracle, not the planner.

use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

use super::make_endmill_6mm;
use super::session::{polygon_model, single_op_session_with};

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::toolpath::Toolpath;

/// The simulation cell, mm.
pub const SIM_RESOLUTION_MM: f64 = 0.5;
/// The tool radius of [`make_endmill_6mm`], mm.
pub const TOOL_RADIUS_MM: f64 = 3.0;
/// The Adaptive stepover of the fixture, mm.
pub const STEPOVER_MM: f64 = 2.0;
/// The Adaptive tolerance of the fixture (`AdaptiveConfig::default()`), mm.
pub const TOLERANCE_MM: f64 = 0.1;

fn circle(cx: f64, cy: f64, r: f64) -> Vec<P2> {
    let n = 48;
    (0..n)
        .map(|i| {
            let t = -(i as f64) * TAU / f64::from(n);
            P2::new(cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}

/// A 120 x 80 pocket with six round islands of radius 8 on a 3 x 2 grid.
pub fn islands_pocket() -> Polygon2 {
    let exterior = vec![
        P2::new(-60.0, -40.0),
        P2::new(60.0, -40.0),
        P2::new(60.0, 40.0),
        P2::new(-60.0, 40.0),
    ];
    let mut holes = Vec::new();
    for &x in &[-32.0, 0.0, 32.0] {
        for &y in &[-16.0, 16.0] {
            holes.push(circle(x, y, 8.0));
        }
    }
    Polygon2::with_holes(exterior, holes)
}

pub fn stock() -> StockConfig {
    StockConfig {
        x: 130.0,
        y: 90.0,
        z: 12.0,
        origin_x: -65.0,
        origin_y: -45.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// Generate and simulate the fixture, with the rapid-order box as given.
pub fn adaptive_session(reorder: bool) -> ProjectSession {
    let cfg = AdaptiveConfig {
        stepover: STEPOVER_MM,
        depth: 6.0,
        depth_per_pass: 3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        ..AdaptiveConfig::default()
    };
    assert_eq!(
        cfg.tolerance, TOLERANCE_MM,
        "the fixture pins the default tolerance"
    );
    let mut session = single_op_session_with(
        stock(),
        make_endmill_6mm(),
        polygon_model(vec![islands_pocket()], "islands_pocket"),
        "Adaptive",
        OperationConfig::Adaptive(cfg),
        |tc| tc.dressups.optimize_rapid_order = reorder,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("adaptive generation");
    let opts = SimulationOptions {
        resolution: SIM_RESOLUTION_MM,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");
    session
}

pub fn toolpath_of(session: &ProjectSession) -> Toolpath {
    session
        .get_result(0)
        .expect("adaptive result")
        .toolpath()
        .clone()
}

pub fn trace_of(session: &ProjectSession) -> &SimulationCutTrace {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metrics-on simulation carries a cut trace")
}
