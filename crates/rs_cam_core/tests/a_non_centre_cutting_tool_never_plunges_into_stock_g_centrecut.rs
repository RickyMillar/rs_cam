//! G-CENTRECUT part 2 — a tool that does not cut at its centre never goes
//! down into stock faster than its blades clear the core.
//!
//! The tool owns the limit (`ToolConfig::non_centre_cutting`,
//! `NonCentreCuttingGeometry::limits`): a straight plunge into stock of at
//! most the blade protrusion, and a descent into stock of at most
//! `atan(protrusion / core)`. Here the operator's 18 mm 2-flute bulk bit:
//! core 8 mm, protrusion 1 mm, limit 7.125 deg.
//!
//! ## The sentry (red before the fix)
//!
//! A 3D Rough clears a small synthetic pocket (a flat plate under 12 mm of
//! stock, Depth/Pass 6). With entry style Plunge the planner pecks 6 mm
//! straight down into uncut stock, and with Ramp or Helix the shared
//! emitters take the requested angle and pitch. The fix makes every entry
//! read the limits from the tool: the plunge becomes the shared ramp at the
//! limit angle, the ramp and the helix clamp to it. The simulation gate
//! (`SimulationResult::non_centre_cutting_descents`) is the oracle; the
//! gate tests below prove that it sees a plunge and a steep ramp.
//!
//! Inject the defect (`adaptive3d::path::effective_entry_style` returns the
//! style unchanged and `emit_ramp` / `emit_helix` skip the clamp) and the
//! sentry goes red: the peck plunge reads as a core strike.
//!
//! ## The gate tests
//!
//! The live walk of the simulator (`RapidClearanceCheck` with the tool's
//! limits) on hand-made moves over fresh stock: a 5 mm plunge and a 30 deg
//! ramp into stock strike; a 0.8 mm plunge, a 5 deg ramp and a long descent
//! in air do not. A centre-cutting tool never strikes. The export door
//! refuses on a strike with no override.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy,
};
use rs_cam_core::compute::tool_config::{NonCentreCuttingGeometry, ToolConfig, ToolId, ToolType};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{SimulationOptions, SimulationResolution};
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::Toolpath;

const CORE_MM: f64 = 8.0;
const PROTRUSION_MM: f64 = 1.0;
const HALF: f64 = 30.0;
const STOCK_TOP_Z: f64 = 12.0;

fn bulk_bit(non_centre_cutting: bool) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.name = "18 mm 2F".to_owned();
    tool.diameter = 18.0;
    tool.cutting_length = 30.0;
    tool.stickout = 45.0;
    tool.shank_diameter = 12.0;
    tool.flute_count = 2;
    tool.non_centre_cutting = non_centre_cutting.then_some(NonCentreCuttingGeometry {
        centre_core_diameter_mm: CORE_MM,
        blade_protrusion_mm: PROTRUSION_MM,
    });
    tool
}

/// The 3D Rough of the plate pocket with `entry_style`; the session after
/// generation and simulation.
fn rough_and_simulate(entry_style: Adaptive3dEntryStyle) -> rs_cam_core::session::ProjectSession {
    let cfg = Adaptive3dConfig {
        stepover: 6.0,
        depth_per_pass: 6.0,
        stock_to_leave_axial: 0.5,
        feed_rate: Some(3000.0),
        entry_style,
        helix_radius_factor: 0.3,
        // A pitch of 6 mm on a 5.4 mm helix is about 10 deg: steeper than
        // the limit, so the clamp must act.
        helix_pitch: 6.0,
        // A 15 deg ramp is steeper than the limit.
        ramp_angle_deg: 15.0,
        clearing_strategy: ClearingStrategy::ContourParallel,
        max_stay_down_distance_mm: Some(0.0),
        ..Adaptive3dConfig::default()
    };
    let mut session = common::session::single_op_session_with(
        common::session::stock_over(HALF, STOCK_TOP_Z),
        bulk_bit(true),
        common::session::mesh_model(make_test_flat(2.0 * HALF), "plate"),
        "3D Rough",
        OperationConfig::Adaptive3d(cfg),
        |tc| tc.dressups.arc_fitting = None,
    );
    common::session::generate(&mut session, 0);
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        skip_ids: Vec::new(),
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("simulation completes");
    session
}

/// The sentry: every entry style of a 3D Rough keeps the bulk bit inside
/// its limits, and the export door does not refuse.
#[test]
fn a_rough_with_a_non_centre_cutting_bit_keeps_its_limits() {
    for style in [
        Adaptive3dEntryStyle::Plunge,
        Adaptive3dEntryStyle::Ramp,
        Adaptive3dEntryStyle::Helix,
    ] {
        let session = rough_and_simulate(style);
        let toolpath = session.result_at(0).expect("a result").toolpath().clone();
        assert!(
            toolpath.moves.len() > 20,
            "{style:?}: the rough must cut ({} moves)",
            toolpath.moves.len()
        );
        let sim = session.simulation_result().expect("a simulation");
        assert!(
            sim.non_centre_cutting_descents.is_empty(),
            "{style:?}: {} core strikes, first {:?}",
            sim.non_centre_cutting_descents.len(),
            sim.non_centre_cutting_descents.first()
        );
        let refusal = session.export_gcode_checked(
            sim.cut_trace.as_deref(),
            rs_cam_core::gcode::ToolLoadExportPolicy::default(),
        );
        assert!(
            !matches!(
                refusal,
                Err(rs_cam_core::gcode::ExportRefusal::NonCentreCuttingDescent { .. })
            ),
            "{style:?}: the export must not refuse on the centre"
        );
    }
}

/// The core strikes of a hand-made toolpath over a fresh 12 mm board, read
/// by the live walk of the simulator.
fn strikes(tool: &ToolConfig, moves: &[P3]) -> Vec<usize> {
    let def = rs_cam_core::compute::cutter::build_cutter(tool).expect("a valid tool");
    let lut = RadialProfileLUT::from_cutter(&def, LUT_SAMPLES);
    let mut stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-HALF, -HALF, 0.0),
            max: P3::new(HALF, HALF, STOCK_TOP_Z),
        },
        0.5,
    );
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(moves[0].x, moves[0].y, STOCK_TOP_Z + 5.0));
    tp.rapid_to(moves[0]);
    for p in &moves[1..] {
        tp.feed_to(*p, 1000.0);
    }
    let mut check = RapidClearanceCheck::new(&def);
    let never = || false;
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            &tp,
            &lut,
            &def,
            def.radius(),
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            18_000,
            2,
            5000.0,
            0.25,
            None,
            &[],
            &[],
            true,
            &never,
            Some(&mut check),
        )
        .expect("never cancelled");
    let (_rapids, core) = check.into_parts();
    core.into_iter().map(|hit| hit.move_index).collect()
}

/// A descent from 0.5 mm over the board to `depth` mm into it, over a run
/// of `run` mm in X.
fn descent(depth: f64, run: f64) -> Vec<P3> {
    vec![
        P3::new(-10.0, 0.0, STOCK_TOP_Z + 0.5),
        P3::new(-10.0 + run, 0.0, STOCK_TOP_Z - depth),
    ]
}

/// The gate: a plunge or a steep ramp into stock strikes; a shallow plunge,
/// a ramp under the limit and a descent in air do not.
#[test]
fn the_gate_sees_the_core_go_into_wood() {
    let tool = bulk_bit(true);
    assert_eq!(strikes(&tool, &descent(5.0, 0.0)), vec![2], "a 5 mm plunge");
    // 5.5 mm down over 9.5 mm: 30 deg.
    assert_eq!(
        strikes(&tool, &descent(5.0, 5.5 / 30f64.to_radians().tan())),
        vec![2],
        "a 30 deg ramp"
    );
    assert!(
        strikes(&tool, &descent(0.4, 0.0)).is_empty(),
        "a plunge 0.9 mm under the start, 0.4 mm into stock, is inside the protrusion"
    );
    // 5.5 mm down at 5 deg.
    assert!(
        strikes(&tool, &descent(5.0, 5.5 / 5f64.to_radians().tan())).is_empty(),
        "a 5 deg ramp is inside the limit"
    );
    // A plunge inside the protrusion, then a level slot cut: no strike.
    let level = vec![
        P3::new(-20.0, 0.0, STOCK_TOP_Z + 0.5),
        P3::new(-20.0, 0.0, STOCK_TOP_Z - 0.5),
        P3::new(20.0, 0.0, STOCK_TOP_Z - 0.5),
    ];
    assert!(
        strikes(&tool, &level).is_empty(),
        "a level cut does not strike"
    );
    let mut through_air = descent(0.5, 0.0);
    through_air.extend([
        P3::new(10.0, 0.0, STOCK_TOP_Z - 0.5),
        P3::new(10.0, 0.0, STOCK_TOP_Z + 3.0),
        P3::new(-10.0, 0.0, STOCK_TOP_Z + 3.0),
        P3::new(-10.0, 0.0, STOCK_TOP_Z - 0.5),
    ]);
    assert!(
        strikes(&tool, &through_air).is_empty(),
        "a plunge into the cleared slot is a descent in air"
    );
    assert!(
        strikes(&bulk_bit(false), &descent(5.0, 0.0)).is_empty(),
        "a centre-cutting tool never strikes"
    );
}

/// The export door refuses a simulation that holds a core strike, with no
/// override.
#[test]
fn a_core_strike_refuses_the_export_with_no_override() {
    let refusal = rs_cam_core::gcode::ExportRefusal::NonCentreCuttingDescent {
        rows: vec!["Toolpath 'x': a strike".to_owned()],
    };
    assert!(refusal.override_needed().is_none());
    assert!(refusal.to_string().contains("does not cut at its centre"));
}
