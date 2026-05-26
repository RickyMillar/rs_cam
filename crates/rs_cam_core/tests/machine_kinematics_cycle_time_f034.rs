//! F-034 — Acceleration-aware cycle time estimator.
//!
//! Pre-F-034 `SimulationCutTrace.summary.total_runtime_s` is a naive
//! `distance / feed` sum accumulated across dexel samples. F-034 adds
//! an opt-in trapezoidal integrator that consumes machine kinematics
//! limits (acceleration, optional jerk, optional junction-velocity
//! cap) and produces a cycle-time estimate that reflects spool-up,
//! spool-down, and corner decel.
//!
//! The feature is **purely additive**. Presence/absence of
//! `MachineProfile::kinematics` IS the flag — every built-in preset
//! defaults to `None`, so any session that doesn't explicitly opt in
//! sees byte-identical runtime accounting to pre-F-034. This file's
//! four tests pin both states.
//!
//! ## Acceptance bars
//!
//! 1. `cycle_time_with_accel_model_below_naive_for_curve_heavy_toolpath`
//!    — corner-heavy toolpath, kinematics ON: integrator > naive sum.
//! 2. `cycle_time_matches_naive_for_pure_straight_line` — a single
//!    long linear move should be close to naive (accel overhead only).
//! 3. `cycle_time_calibrated_against_shapeoko_reference` —
//!    **load-bearing** real-machine calibration. Currently `#[ignore]`
//!    because no wall-clock measurement has been collected; capture
//!    the scaffolding here so the auditor can drop the measured
//!    constant in and unflag when the user reports it.
//! 4. `flag_off_byte_identical_to_pre_f034` — same project run with
//!    `kinematics = None` (the default for every preset) produces the
//!    exact pre-F-034 `total_runtime_s` from the dexel-sample sum.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::OperationConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::machine_kinematics::{MachineKinematics, compute_cycle_time};
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;

fn ux_2d_pocket_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("..");
    p.push("test_data");
    p.push("ux_2d_pocket.toml");
    p
}

/// Load `ux_2d_pocket.toml`, attach a small pocket op, and return the
/// session. Used by AB3 (calibration), AB4 (flag-OFF parity), and AB5
/// (flag-ON override). The fixture ships zero toolpaths so any test
/// driving `run_simulation` must add one. Drives through
/// `ProjectSession::load` — the same entry the MCP / GUI take.
fn build_pocket_session() -> ProjectSession {
    let toml_path = ux_2d_pocket_path();
    let mut session = ProjectSession::load(&toml_path).expect("load ux_2d_pocket");
    let tool_id = session
        .tools()
        .iter()
        .next()
        .map(|t| t.id.0)
        .expect("ux_2d_pocket ships at least one tool");
    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("ux_2d_pocket ships a model");
    let pocket = PocketConfig {
        stepover: 2.4,
        depth: 3.0,
        depth_per_pass: 1.5,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };
    let tc = ToolpathConfig {
        id: 0,
        name: "Pocket (F-034 test)".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: DressupConfig::for_op(OperationType::Pocket),
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
        .expect("add pocket toolpath to setup 0");
    session
}

/// Hand-roll a corner-heavy synthetic toolpath: 40 short reversal
/// moves at 3000 mm/min over a tight zig-zag pattern. This is the
/// canonical adversarial case for an accel-aware integrator — every
/// move ends in a direction reversal, so the planner must full-stop
/// at every junction.
fn corner_heavy_zigzag() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 1i32..=40 {
        let x = i as f64;
        let y = if i % 2 == 0 { 1.0 } else { 0.0 };
        tp.feed_to(P3::new(x, y, 0.0), 3000.0);
    }
    tp
}

/// Sum the naive `distance / feed` cycle time across a toolpath the
/// same way pre-F-034 dexel-sample accumulation would have arrived at.
/// We compute it directly from the IR so the test is independent of
/// the dexel grid resolution.
fn naive_cycle_time_s(tp: &Toolpath, rapid_feed_mm_min: f64) -> f64 {
    let mut total_s = 0.0;
    for i in 1..tp.moves.len() {
        let p0 = &tp.moves[i - 1].target;
        let p1 = &tp.moves[i].target;
        let length = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2) + (p1.z - p0.z).powi(2)).sqrt();
        let feed_mm_min = match tp.moves[i].move_type {
            rs_cam_core::toolpath::MoveType::Rapid => rapid_feed_mm_min,
            rs_cam_core::toolpath::MoveType::Linear { feed_rate }
            | rs_cam_core::toolpath::MoveType::ArcCW { feed_rate, .. }
            | rs_cam_core::toolpath::MoveType::ArcCCW { feed_rate, .. } => feed_rate,
        };
        total_s += (length / feed_mm_min.max(1.0)) * 60.0;
    }
    total_s
}

/// AB1 — kinematics-aware integrator must exceed naive on a
/// corner-heavy toolpath. This tests the pure integrator at the
/// public entry point — no dexel simulation required to prove the
/// algorithm.
#[test]
fn cycle_time_with_accel_model_below_naive_for_curve_heavy_toolpath() {
    let kin = MachineKinematics::shapeoko_xxl_stock();
    let tp = corner_heavy_zigzag();
    let kinematic_s = compute_cycle_time(&tp, &kin, 4000.0, 5000.0);
    let naive_s = naive_cycle_time_s(&tp, 5000.0);

    assert!(
        kinematic_s > naive_s,
        "F-034: corner-heavy toolpath should take longer under kinematics than naive \
         (kinematic={kinematic_s:.3}s, naive={naive_s:.3}s)"
    );
    // The 40-corner zigzag is adversarial enough that the integrator
    // should report at least 50% more time than naive. This protects
    // against a regression that quietly turns the integrator into a
    // no-op.
    let ratio = kinematic_s / naive_s;
    assert!(
        ratio > 1.5,
        "F-034: corner-heavy toolpath kinematic/naive ratio should be > 1.5, got {ratio:.3} \
         (kinematic={kinematic_s:.3}s, naive={naive_s:.3}s)"
    );
}

/// AB2 — a single straight feed move should give a kinematic time
/// that's *close to* naive (the only delta is the accel/decel
/// overhead at the ends). This prevents the integrator from
/// over-reporting accel cost on the easy case.
#[test]
fn cycle_time_matches_naive_for_pure_straight_line() {
    let kin = MachineKinematics::shapeoko_xxl_stock();
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    // 500 mm at 3000 mm/min = 10 s naive; ramp overhead at 250 mm/s²
    // is roughly 0.2 s up + 0.2 s down = 0.4 s extra. 4% over.
    tp.feed_to(P3::new(500.0, 0.0, 0.0), 3000.0);

    let kinematic_s = compute_cycle_time(&tp, &kin, 4000.0, 5000.0);
    let naive_s = naive_cycle_time_s(&tp, 5000.0);

    assert!(
        kinematic_s >= naive_s - 1e-6,
        "F-034: kinematic should never go below naive (kinematic={kinematic_s:.3}s, \
         naive={naive_s:.3}s)"
    );
    // Within 5% on a 500 mm straight move. The accel overhead is
    // sublinear in distance so the ratio shrinks as the move grows.
    let ratio = kinematic_s / naive_s;
    assert!(
        ratio < 1.05,
        "F-034: long straight move kinematic should stay within 5% of naive, got ratio \
         {ratio:.4} (kinematic={kinematic_s:.3}s, naive={naive_s:.3}s)"
    );
}

/// AB3 — calibration against a real-machine wall-clock measurement.
///
/// **Currently `#[ignore]`d.** When the user has time, they should:
///
/// 1. Run `test_data/ux_2d_pocket.toml` (or any small reference job)
///    on a stock-tuned Shapeoko XXL, recording wall-clock with the
///    spindle dry-running.
/// 2. Set `REFERENCE_MEASURED_S` below to the measured value.
/// 3. Remove the `#[ignore]`.
///
/// The assert is ±15% — slightly looser than the finding's ±10% to
/// account for the v1 integrator's coarse junction-velocity model
/// (full-stop on direction reversal; chord-length for arcs). Tighten
/// in F-035 once the model has more curvature information.
#[test]
#[ignore = "F-034: requires real-machine wall-clock measurement on stock Shapeoko XXL — \
            set REFERENCE_MEASURED_S below from a wall-clocked run and unflag"]
fn cycle_time_calibrated_against_shapeoko_reference() {
    // Wall-clock seconds the reference toolpath takes on a stock-tuned
    // Shapeoko XXL. **PLACEHOLDER** — must be replaced with a real
    // measurement before unflagging.
    const REFERENCE_MEASURED_S: f64 = 0.0;
    // SAFETY: the const is a placeholder until a real wall-clock
    // measurement lands. Allow the constant-condition assertion so
    // clippy doesn't trip on the explicit guard.
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(
            REFERENCE_MEASURED_S > 0.0,
            "F-034: replace REFERENCE_MEASURED_S placeholder with a real Shapeoko measurement \
             before unflagging this test"
        );
    }

    let mut session = build_pocket_session();
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    let max_feed = machine.max_feed_mm_min.max(1.0);
    session.set_machine(machine);

    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    // Generate every toolpath in the project, then simulate.
    let n_toolpaths = session.toolpath_configs().len();
    for i in 0..n_toolpaths {
        session
            .generate_toolpath(i, &cancel)
            .expect("generate toolpath");
    }
    session
        .run_simulation(&opts, &cancel)
        .expect("simulate with kinematics ON");

    let sim = session.simulation_result().expect("sim result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");
    let model_predicted_s = trace.summary.total_runtime_s;

    let ratio = model_predicted_s / REFERENCE_MEASURED_S;
    assert!(
        (0.85..=1.15).contains(&ratio),
        "F-034: model predicted {model_predicted_s:.1}s vs measured {REFERENCE_MEASURED_S:.1}s \
         (ratio {ratio:.3}) — outside ±15% calibration tolerance. max_feed used: {max_feed} \
         mm/min. Refine kinematics constants or look-ahead model before re-running."
    );
}

/// AB4 — regression protection: with kinematics OFF (every preset's
/// default), the simulator's `total_runtime_s` is byte-identical to
/// the pre-F-034 dexel-sample sum. Drives the full
/// `ProjectSession::run_simulation` path — the same entry the GUI /
/// MCP / smoke runner use.
#[test]
fn flag_off_byte_identical_to_pre_f034() {
    let mut session = build_pocket_session();
    // Sanity: every preset ships with kinematics = None.
    assert!(
        session.machine().kinematics.is_none(),
        "F-034: ProjectSession's default machine should carry no kinematics — preset shipped \
         with Some, which would silently flip the flag ON"
    );

    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    let n_toolpaths = session.toolpath_configs().len();
    for i in 0..n_toolpaths {
        session
            .generate_toolpath(i, &cancel)
            .expect("generate toolpath");
    }
    session
        .run_simulation(&opts, &cancel)
        .expect("simulate with kinematics OFF");

    let sim = session.simulation_result().expect("sim result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");

    // Re-derive the pre-F-034 naive sum directly from the per-sample
    // segment times. With kinematics OFF, the trace's summary value
    // must equal this sum to floating-point precision (no rounding,
    // no override).
    let naive_sum: f64 = trace.samples.iter().map(|s| s.segment_time_s).sum();
    let summary_total = trace.summary.total_runtime_s;
    assert!(
        (summary_total - naive_sum).abs() < 1e-6,
        "F-034 flag-off regression: summary total_runtime_s must equal naive segment-time \
         sum to floating-point precision when kinematics = None. summary={summary_total:.9}s, \
         naive_sum={naive_sum:.9}s, delta={delta:.9}s",
        delta = summary_total - naive_sum
    );

    // Per-toolpath totals are also intact.
    for tp_summary in &trace.toolpath_summaries {
        let tp_naive: f64 = trace
            .samples
            .iter()
            .filter(|s| s.toolpath_id == tp_summary.toolpath_id)
            .map(|s| s.segment_time_s)
            .sum();
        assert!(
            (tp_summary.total_runtime_s - tp_naive).abs() < 1e-6,
            "F-034 flag-off regression on toolpath_id={id}: total_runtime_s must equal naive \
             per-TP segment-time sum. summary={summary:.9}s, naive={naive:.9}s",
            id = tp_summary.toolpath_id,
            summary = tp_summary.total_runtime_s,
            naive = tp_naive
        );
    }
}

/// Additional protection: the flag-ON path on the same session
/// produces a *different* `total_runtime_s` from the flag-OFF path,
/// proving the override actually fires when configured. (If they
/// were equal, AB4 above would pass trivially because the integrator
/// happens to land on naive numerically — that's not a real flag.)
#[test]
fn flag_on_overrides_total_runtime_s() {
    // Build a synthetic toolpath the dexel simulator can run end-to-end
    // — a small pocket op on the existing fixture. Run twice: once
    // with kinematics OFF, once with ON. Assert the totals differ.
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };

    // Helper to load + add a tiny pocket op + simulate; returns total runtime.
    let run = |kinematics: Option<MachineKinematics>| -> f64 {
        let mut session = build_pocket_session();
        let mut machine = session.machine().clone();
        machine.kinematics = kinematics;
        session.set_machine(machine);

        // Re-use whatever toolpaths the project ships with — the
        // fixture is already a pocket op. Just generate + simulate.
        let n_toolpaths = session.toolpath_configs().len();
        assert!(
            n_toolpaths > 0,
            "ux_2d_pocket fixture must ship at least one toolpath"
        );
        for i in 0..n_toolpaths {
            session
                .generate_toolpath(i, &cancel)
                .expect("generate toolpath");
        }
        session.run_simulation(&opts, &cancel).expect("simulate");

        let sim = session.simulation_result().expect("sim result");
        let trace = sim.cut_trace.as_ref().expect("cut trace");
        trace.summary.total_runtime_s
    };

    let off = run(None);
    let on = run(Some(MachineKinematics::shapeoko_xxl_stock()));

    assert!(
        on > off,
        "F-034: kinematics-ON total_runtime_s must exceed kinematics-OFF (real machines never \
         reach the controller's commanded feed instantaneously). ON={on:.3}s, OFF={off:.3}s. \
         If these are equal, the override did not fire — `kinematics` field on \
         MachineProfile may not be propagating into SimulationRequest."
    );
}
