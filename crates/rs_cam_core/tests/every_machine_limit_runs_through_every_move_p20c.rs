//! P20c (feeds contract, `planning/feeds_contract_2026-11-03/phases/P20.md`
//! section 4): every machine limit runs through every move. Operator goal
//! (2026-11-08): "All limits should run through all moves." Every commanded
//! feed passes the machine's per-axis rates, and the gates grade the feed
//! the machine reaches.
//!
//! The machine is `MachineKinematics::shapeoko_xxl_ricky_tuned` (`$110` =
//! `$111` = 10 000 mm/min, `$112` = 1000 mm/min). The per-axis rate on a
//! unit direction `d` is `min_i(rate_i / |d_i|)`
//! (`MachineKinematics::effective_max_rate_mm_min`), so:
//!
//! - a 45° XZ move is bound at 1000 / sin 45° = 1414.21 mm/min (Z binds);
//! - a vertical move is bound at 1000 mm/min;
//! - an X move is bound at 10 000 mm/min.
//!
//! The arms:
//!
//! - (a) `cap_feeds_at_axis_rates` lowers each fed move to its bound and
//!   leaves a move below its bound alone; a machine with no per-axis rates
//!   changes nothing;
//! - (b) the modulator's chipload-min floor does not lift a move above its
//!   bound (the floor 0.05 x 18 000 x 2 = 1800 mm/min is above 1414.21);
//! - (c) a Pocket's dressup ramp entry at 60° (the dressup entry feed, P29-8
//!   8c: `min(plunge move feed, op plunge rate)` = 3000 >= 1500 mm/min)
//!   descends above its Z-bound rate 1000 / sin 60° = 1154.70; it is
//!   emitted at that rate, and no emitted fed move is above its bound;
//!   without per-axis rates the same ramp is above it;
//! - (d) on a machine with the lowest admitted acceleration, every cutting
//!   sample is graded at most at its move's command and rate
//!   (`effective_feed_for_sample`, the door every gate reads), and some
//!   sample is graded below its command (the accel reach); after modulation
//!   each modulated move is graded at `min(modulated command, reached feed
//!   of the modulated toolpath)`. On this fixture the modulator's own
//!   `KinematicReach` cap already holds every modulated command at or below
//!   the reach (no chipload floor lifts a move above it here), so this
//!   arm pins the identity and does not go red without the stamp; the stamp
//!   binds where a banded row's floor lifts a move above its accel reach.
//!
//! Red: remove the `cap_feeds_at_axis_rates` call in
//! `session::compute::JobPhases::run_stages` (c), the P20c rate cap in
//! `adaptive_feed_modulate_with_materials` (b), or set the default of
//! `SimulationOptions::use_predicted_feed_in_gates` back to `false` (d).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::{
    ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement, adaptive_feed_modulate,
};
use rs_cam_core::feeds::geometry::RadialThinning;
use rs_cam_core::geo::P3;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::{
    MachineKinematics, cap_feeds_at_axis_rates, rate_bound_feed_of_move,
};
use rs_cam_core::session::{
    Command, ProjectSession, SetMachineKinematicsArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// `$112` of the machine (mm/min).
const Z_RATE: f64 = 1000.0;

fn machine() -> MachineKinematics {
    let k = MachineKinematics::shapeoko_xxl_ricky_tuned();
    assert_eq!(
        k.max_rate_xyz_mm_min.unwrap()[2],
        Z_RATE,
        "the fixture Z rate"
    );
    k
}

/// 1000 / sin 45°.
fn diagonal_bound() -> f64 {
    Z_RATE / std::f64::consts::FRAC_1_SQRT_2
}

/// The bound of every fed move of `tp` on `k`, with its index.
fn bounds(tp: &Toolpath, k: &MachineKinematics) -> Vec<(usize, f64, f64)> {
    (1..tp.moves.len())
        .filter_map(|i| {
            let m = &tp.moves[i];
            let feed = m.move_type.feed_rate()?;
            let bound =
                rate_bound_feed_of_move(&tp.moves[i - 1].target, &m.target, m.move_type, feed, k);
            Some((i, feed, bound))
        })
        .collect()
}

#[test]
fn a_move_commands_at_most_its_projected_axis_rate_p20c() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    // 45° XZ, 3000 commanded: Z binds at 1414.21.
    tp.feed_to(P3::new(10.0, 0.0, 0.0), 3000.0);
    // X only, 3000 commanded: the X rate 10 000 does not bind.
    tp.feed_to(P3::new(40.0, 0.0, 0.0), 3000.0);
    // Vertical, 2000 commanded: Z binds at 1000.
    tp.feed_to(P3::new(40.0, 0.0, -5.0), 2000.0);
    // Vertical, 500 commanded: below the bound, kept.
    tp.feed_to(P3::new(40.0, 0.0, -8.0), 500.0);

    let k = machine();
    let mut free = tp.clone();
    assert_eq!(
        cap_feeds_at_axis_rates(&mut free, &MachineKinematics::generic_wood_router()),
        0,
        "a machine with no per-axis rates changes nothing"
    );

    let lowered = cap_feeds_at_axis_rates(&mut tp, &k);
    let feeds: Vec<f64> = tp
        .moves
        .iter()
        .filter_map(|m| m.move_type.feed_rate())
        .collect();
    assert_eq!(lowered, 2, "feeds {feeds:?}");
    assert!((feeds[0] - diagonal_bound()).abs() < 1e-9, "{feeds:?}");
    assert!(
        (feeds[0]
            - k.effective_max_rate_mm_min(&[0.5_f64.sqrt(), 0.0, -0.5_f64.sqrt()])
                .unwrap())
        .abs()
            < 1e-9,
        "the cap reads the one projection: {feeds:?}"
    );
    assert_eq!(feeds[1], 3000.0);
    assert_eq!(feeds[2], Z_RATE);
    assert_eq!(feeds[3], 500.0);
}

#[test]
fn the_modulator_floor_does_not_lift_a_move_above_its_axis_rate_p20c() {
    let k = machine();
    let profile = MachineProfile {
        max_feed_mm_min: 4000.0,
        max_cutting_feed_mm_min: Some(4000.0),
        ..MachineProfile::default()
    };
    // A long 45° XZ cut, so the accel reach is the rate bound.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to_with_intent(P3::new(50.0, 0.0, -50.0), 1000.0, MoveIntent::ClearingCut);
    let engagements: Vec<PerMoveEngagement> = (0..tp.moves.len())
        .map(|_| PerMoveEngagement {
            radial_woc_fraction: 0.5,
            axial_doc_fraction: 1.0,
            axial_doc_mm: 2.0,
            thinning_lift: None,
        })
        .collect();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: profile.cutting_ceiling(),
        rapid_feed_mm_min: 5000.0,
        // Floor 0.05 x 18 000 x 2 = 1800 mm/min, above 1414.21.
        chipload_band: Some(ChiploadBand::new(0.05, 0.08).unwrap()),
        radial_thinning: RadialThinning::Applied,
        kinematics: &k,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: 0.0,
        // The geometric plunge guard is off: the move is not vertical, and
        // the claim is the rate, not the op plunge rate.
        plunge_rate_mm_min: f64::INFINITY,
    };
    let floor = 0.05 * 18_000.0 * 2.0;
    assert!(
        floor > diagonal_bound(),
        "the fixture floor is above the bound"
    );
    let _ = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
    let feed = tp.moves[1].move_type.feed_rate().unwrap();
    assert!(
        feed <= diagonal_bound() + 1e-9,
        "the floor lifted the move to {feed} above its rate {}",
        diagonal_bound()
    );
}

/// The cut feed and the plunge feed of the Pocket (mm/min). The dressup
/// entry ramps at `min(plunge move feed, op plunge rate)` (P29-8 8c) =
/// `min(3000, 3000)` = 3000, above 1154.70.
const FEED: f64 = 5000.0;
const PLUNGE: f64 = 3000.0;
const POCKET_HALF: f64 = 20.0;
const POCKET_DEPTH: f64 = 3.0;
/// The ramp angle (degrees from the horizontal).
const RAMP_ANGLE_DEG: f64 = 60.0;

fn pocket_session(kinematics: MachineKinematics) -> ProjectSession {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let mut op = OperationConfig::new_default(OperationType::Pocket);
    if let OperationConfig::Pocket(cfg) = &mut op {
        cfg.floor = rs_cam_core::compute::floor::Floor::top(POCKET_DEPTH);
        cfg.depth_per_pass = Some(POCKET_DEPTH);
        cfg.stepover = Some(2.0);
        cfg.feed_rate = Some(FEED);
        cfg.plunge_rate = Some(PLUNGE);
    }
    let mut session = common::session::single_op_session_with(
        common::session::stock_under(POCKET_HALF + 5.0, 10.0),
        tool,
        common::session::polygon_model(
            vec![common::session::square_polygon(POCKET_HALF)],
            "square",
        ),
        "Pocket",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::Ramp;
            tc.dressups.ramp_angle =
                rs_cam_core::feeds::entry_rule::EntryAngle::Stated(RAMP_ANGLE_DEG);
            tc.dressups.feed_optimization = false;
            tc.dressups.arc_fitting = None;
        },
    );
    let _ = session
        .apply(Command::SetMachineKinematics(SetMachineKinematicsArgs {
            kinematics: Box::new(kinematics),
        }))
        .expect("the kinematics block");
    common::session::generate(&mut session, 0);
    session
}

/// The fixture machine with no per-axis rates (the control arm).
fn no_rates() -> MachineKinematics {
    MachineKinematics {
        max_rate_xyz_mm_min: None,
        ..machine()
    }
}

/// `(feed, bound)` of each descending `EntryRamp` move, the bound on `k`.
fn ramp_descents(tp: &Toolpath, k: &MachineKinematics) -> Vec<(f64, f64)> {
    bounds(tp, k)
        .into_iter()
        .filter(|&(i, _, _)| {
            tp.moves[i].intent == MoveIntent::EntryRamp
                && tp.moves[i].target.z < tp.moves[i - 1].target.z - 1e-9
        })
        .map(|(_, feed, bound)| (feed, bound))
        .collect()
}

#[test]
fn a_dressup_entry_above_its_axis_rate_is_emitted_at_the_rate_p20c() {
    let k = machine();
    let z_bound = Z_RATE / RAMP_ANGLE_DEG.to_radians().sin();
    let free = pocket_session(no_rates());
    let free_ramps = ramp_descents(free.result_at(0).unwrap().toolpath(), &k);
    assert!(
        free_ramps.iter().any(|(feed, bound)| *feed > bound + 1.0),
        "control: without per-axis rates the ramp descends above its rate: {free_ramps:?}"
    );

    let session = pocket_session(machine());
    let tp = session.result_at(0).unwrap().toolpath().clone();
    let ramps = ramp_descents(&tp, &k);
    assert!(
        ramps.iter().any(|(feed, _)| (*feed - z_bound).abs() < 0.5),
        "some ramp descent is emitted at 1000 / sin 60° = {z_bound}: {ramps:?}"
    );
    for (i, feed, bound) in bounds(&tp, &k) {
        assert!(
            feed <= bound + 1e-9,
            "move {i} commands {feed} above its rate {bound}"
        );
    }
}

/// The fixture machine with the lowest acceleration the domain admits
/// (10 mm/s², `MachineDimension::Acceleration`), so the accel reach binds on
/// the Pocket's moves and the reached feed is below the command.
fn slow_machine() -> MachineKinematics {
    MachineKinematics {
        acceleration_mm_s2: 10.0,
        acceleration_xyz_mm_s2: Some([10.0, 10.0, 10.0]),
        ..machine()
    }
}

fn simulate(session: &mut ProjectSession, modulation: bool) {
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        adaptive_feed_modulation: modulation,
        ..SimulationOptions::default()
    };
    assert!(
        opts.use_predicted_feed_in_gates,
        "P20c: the default grades the reached feed"
    );
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(&opts, &cancel)
        .expect("the session simulates");
}

#[test]
fn the_gates_grade_the_feed_the_machine_reaches_p20c() {
    // Arm 1, no modulation: the simulator's reached feed.
    let mut session = pocket_session(slow_machine());
    simulate(&mut session, false);
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone())
        .expect("a cut trace");
    let tp = session.result_at(0).unwrap().toolpath().clone();
    let id = session.toolpath_configs()[0].id;
    let k = slow_machine();

    let mut graded = 0usize;
    let mut below_command = 0usize;
    for s in trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.toolpath_id == id)
    {
        let m = &tp.moves[s.move_index];
        let Some(commanded) = m.move_type.feed_rate() else {
            continue;
        };
        let bound = rate_bound_feed_of_move(
            &tp.moves[s.move_index - 1].target,
            &m.target,
            m.move_type,
            commanded,
            &k,
        );
        let feed = rs_cam_core::tool_load::effective_feed_for_sample(s, &trace.predicted_feeds);
        assert!(
            feed <= commanded + 1e-6 && feed <= bound + 1e-6,
            "move {}: graded at {feed}, commanded {commanded}, rate {bound}",
            s.move_index
        );
        graded += 1;
        if feed < commanded - 1.0 {
            below_command += 1;
        }
    }
    assert!(graded > 0, "the gates grade some cutting sample");
    assert!(
        below_command > 0,
        "some sample is graded at its reached feed below the command ({graded} graded)"
    );

    // Arm 2, with modulation: each modulated move is graded at the reached
    // feed of the MODULATED toolpath (the stamp of
    // `session::compute::simulation`), on the re-time clock: the effective
    // kinematics, the cutting ceiling and the post's rapid feed.
    let mut session = pocket_session(slow_machine());
    simulate(&mut session, true);
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone())
        .expect("a cut trace");
    let tp = session.result_at(0).unwrap().toolpath().clone();
    let machine = session.machine().clone();
    let reached = rs_cam_core::machine::kinematics::predicted_feeds_for_toolpath(
        &tp,
        &machine.effective_kinematics(),
        machine.cutting_ceiling().at_least(1.0).mm_min(),
        session.post_config().rapid_feed_mm_min(&machine),
    );
    let mut checked = 0usize;
    for (&(tp_id, move_index), &(modulated, _)) in &trace.modulated_feeds {
        let (Some(&stamped), Some(&reach)) = (
            trace.predicted_feeds.get(&(tp_id, move_index)),
            reached.get(&move_index),
        ) else {
            continue;
        };
        let want = modulated.min(reach);
        assert!(
            (stamped - want).abs() < 1e-6,
            "move {move_index}: stamped {stamped}, modulated {modulated}, reached {reach}"
        );
        checked += 1;
    }
    assert!(checked > 0, "the modulator touched some move");
}
