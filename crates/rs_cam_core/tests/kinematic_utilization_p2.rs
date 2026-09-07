//! Phase 2 sentries — the kinematic-utilisation instrument
//! (`planning/machine_kinematics_confidence_2026-09-07.md`).
//!
//! The instrument answers one operator question: what does the machine
//! actually do with this toolpath, and where is the headroom? These
//! tests pin the three parts that can drift:
//!
//! 1. the GEOMETRIC motion classes and their two angle thresholds,
//! 2. the plunge ratio, which must read the ACHIEVED Z rate and never
//!    the commanded one,
//! 3. the binding split over fed time, plus the headroom estimate that
//!    reads it.
//!
//! The machine model is the user's tuned Shapeoko XXL: per-axis accel
//! `$120/$121/$122 = 500/500/270`, per-axis rate
//! `$110/$111/$112 = 10000/10000/1000`, junction deviation
//! `$11 = 0.020`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::kinematic_utilization::{
    MotionClass, ToolpathKinematicUtilization, analyse_toolpath, classify_move,
};
use rs_cam_core::machine_kinematics::{KinematicBinding, MachineKinematics};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Machine travel-rate cap (mm/min) — the user's `$110/$111`.
const MAX_FEED: f64 = 10_000.0;
/// Rapid rate (mm/min). The instrument floors it at `MAX_FEED`, so the
/// two agree here on purpose.
const RAPID_FEED: f64 = 10_000.0;
/// The finish feed the wanaka measurement was taken at.
const FINISH_FEED: f64 = 925.0;
/// The operation plunge rate the 2026-09-07 incident was measured
/// against.
const PLUNGE_RATE: f64 = 512.0;

const TP: ToolpathId = ToolpathId(7);

/// The user's tuned Shapeoko XXL, per-axis rates INCLUDED.
fn tuned() -> MachineKinematics {
    MachineKinematics {
        acceleration_mm_s2: 423.3,
        acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
        max_rate_xyz_mm_min: Some([10_000.0, 10_000.0, 1_000.0]),
        junction_deviation_mm: 0.020,
        jerk_mm_s3: None,
        max_junction_velocity_mm_min: None,
    }
}

/// The same machine with `$110/$111/$112` unknown — the pre-Phase-1
/// model, where nothing clamps a Z-dominant command.
fn tuned_without_rates() -> MachineKinematics {
    MachineKinematics {
        max_rate_xyz_mm_min: None,
        ..tuned()
    }
}

/// Analyse one toolpath against the tuned machine.
fn analyse(toolpath: &Toolpath, plunge_rate: f64) -> ToolpathKinematicUtilization {
    analyse_toolpath(toolpath, TP, &tuned(), MAX_FEED, RAPID_FEED, plunge_rate)
}

/// A single fed move from `from` to `to` at `feed`, seeded by a rapid
/// so the fed move is move index 1.
fn one_move(from: P3, to: P3, feed: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(from);
    tp.feed_to(to, feed);
    tp
}

/// One lateral fed move of `length_mm` along +X at the finish feed.
fn lateral_move(length_mm: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, -1.0));
    tp.feed_to(P3::new(length_mm, 0.0, -1.0), FINISH_FEED);
    tp
}

// ---------------------------------------------------------------
// 1 — a pure-vertical descent commanded at 3x the plunge rate.
// ---------------------------------------------------------------

/// The incident shape: an untagged vertical descent carrying a lateral
/// feed. Two arms, because the answer differs and both are correct.
///
/// * With `$112` unknown the machine really does run 1536 mm/min down,
///   and the ratio reads the full 3.0.
/// * With `$112 = 1000` the controller clamps the descent, so the
///   binding is `RateBound { axis: 2 }` and the ratio reads
///   `1000 / 512 = 1.953`. The ratio must follow the ACHIEVED Z rate,
///   never the command — that is the whole point of the instrument.
#[test]
fn vertical_descent_at_three_times_plunge_rate_is_plunge_class() {
    let commanded = 3.0 * PLUNGE_RATE;
    let tp = one_move(P3::new(0.0, 0.0, 0.0), P3::new(0.0, 0.0, -20.0), commanded);

    // The classifier alone agrees: a pure descent is plunge-class.
    let direct = classify_move(
        MoveType::Linear {
            feed_rate: commanded,
        },
        [0.0, 0.0, -20.0],
    );
    assert_eq!(direct, MotionClass::Plunge);

    // Arm A — no per-axis rate: nothing clamps the descent.
    let no_rates = analyse_toolpath(
        &tp,
        TP,
        &tuned_without_rates(),
        MAX_FEED,
        RAPID_FEED,
        PLUNGE_RATE,
    );
    assert_eq!(no_rates.moves.len(), 1);
    assert_eq!(no_rates.moves[0].class, MotionClass::Plunge);
    assert_eq!(no_rates.moves[0].binding, KinematicBinding::FeedBound);
    assert!(no_rates.plunge_is_measured());
    let ratio_a = no_rates.plunge.peak_ratio.expect("arm A ratio");
    assert!(
        (ratio_a - 3.0).abs() < 1e-6,
        "arm A: expected ratio 3.0, read {ratio_a:.6} (achieved z-rate {:.3} mm/min)",
        no_rates.moves[0].achieved_z_rate_mm_min
    );
    assert_eq!(no_rates.plunge.over_1x, 1);
    assert_eq!(no_rates.plunge.over_2x, 1);

    // Arm B — `$112 = 1000` clamps it. The ratio must fall with the
    // ACHIEVED rate, and the binding must name the Z axis.
    let clamped = analyse(&tp, PLUNGE_RATE);
    assert_eq!(clamped.moves[0].class, MotionClass::Plunge);
    assert_eq!(
        clamped.moves[0].binding,
        KinematicBinding::RateBound { axis: 2 },
        "a 1536 mm/min pure-Z command must read RateBound on Z at $112 = 1000"
    );
    let ratio_b = clamped.plunge.peak_ratio.expect("arm B ratio");
    let expected_b = 1000.0 / PLUNGE_RATE;
    assert!(
        (ratio_b - expected_b).abs() < 1e-6,
        "arm B: expected {expected_b:.6} (1000 / 512), read {ratio_b:.6}"
    );
    assert_eq!(clamped.plunge.worst_move_index, Some(1));
    let worst_z = clamped.plunge.worst_position.expect("worst position");
    assert!((worst_z[2] + 20.0).abs() < 1e-9);
    assert_eq!(clamped.ramp.population, 0);
}

// ---------------------------------------------------------------
// 2 — a 10-degree ramp is a ramp, not a plunge.
// ---------------------------------------------------------------

/// The physics correction: a sloped fed descent cuts laterally, so the
/// chipload band governs it and the plunge grade must not touch it.
#[test]
fn ten_degree_ramp_is_ramp_class_and_not_plunge_graded() {
    let angle = 10.0_f64.to_radians();
    let length = 100.0;
    let end = P3::new(length * angle.cos(), 0.0, -length * angle.sin());
    let tp = one_move(P3::new(0.0, 0.0, 0.0), end, 3000.0);

    let util = analyse(&tp, PLUNGE_RATE);
    assert_eq!(util.moves[0].class, MotionClass::Ramp);
    assert_eq!(util.moves[0].plunge_ratio, None);
    assert_eq!(util.plunge.population, 0);
    assert!(!util.plunge_is_measured());
    assert_eq!(util.plunge.peak_ratio, None);
    assert_eq!(util.ramp.population, 1);
    let steep = util.ramp.steepest_deg_from_horizontal.expect("steepest");
    assert!(
        (steep - 10.0).abs() < 1e-6,
        "expected a 10 degree ramp, read {steep:.6} degrees"
    );
    assert_eq!(util.ramp.worst_move_index, Some(1));

    // The same verdict straight out of the classifier.
    let delta = [end.x, 0.0, end.z];
    let direct = classify_move(MoveType::Linear { feed_rate: 3000.0 }, delta);
    assert_eq!(direct, MotionClass::Ramp);
}

// ---------------------------------------------------------------
// 3 — bracket the 15-degree class boundary.
// ---------------------------------------------------------------

/// Angles here are measured from VERTICAL, not from horizontal. 14
/// degrees is inside the plunge cone; 16 degrees is outside it. Phase 3
/// caps exactly the moves this bracket admits, so the bracket is
/// load-bearing.
#[test]
fn plunge_cone_brackets_at_fourteen_and_sixteen_degrees_from_vertical() {
    for (deg, expected) in [(14.0_f64, MotionClass::Plunge), (16.0, MotionClass::Ramp)] {
        let a = deg.to_radians();
        let length = 20.0;
        let end = P3::new(length * a.sin(), 0.0, -length * a.cos());

        // Through the classifier.
        let delta = [end.x, 0.0, end.z];
        let direct = classify_move(MoveType::Linear { feed_rate: 900.0 }, delta);
        assert_eq!(
            direct, expected,
            "classify_move: {deg} degrees from vertical must be {expected:?}"
        );

        // Through the full analysis.
        let tp = one_move(P3::new(0.0, 0.0, 0.0), end, 900.0);
        let util = analyse(&tp, PLUNGE_RATE);
        assert_eq!(
            util.moves[0].class, expected,
            "analyse_toolpath: {deg} degrees from vertical must be {expected:?}"
        );
    }
}

/// The lateral threshold is the other end of the same scale: a descent
/// shallower than 2 degrees from HORIZONTAL is ordinary cutting.
#[test]
fn shallow_descent_below_two_degrees_is_lateral() {
    for (deg, expected) in [(1.0_f64, MotionClass::Lateral), (3.0, MotionClass::Ramp)] {
        let a = deg.to_radians();
        let length = 20.0;
        let delta = [length * a.cos(), 0.0, -length * a.sin()];
        let class = classify_move(MoveType::Linear { feed_rate: 900.0 }, delta);
        assert_eq!(class, expected, "{deg} degrees below horizontal");
    }
    // A rapid and an ascent are never graded, whatever their angle.
    let up = [0.0, 0.0, 20.0];
    assert_eq!(
        classify_move(MoveType::Linear { feed_rate: 900.0 }, up),
        MotionClass::Retract
    );
    assert_eq!(
        classify_move(MoveType::Rapid, [0.0, 0.0, -20.0]),
        MotionClass::Retract
    );
    assert_eq!(
        classify_move(MoveType::Linear { feed_rate: 900.0 }, [0.0, 0.0, 0.0]),
        MotionClass::Retract
    );
}

// ---------------------------------------------------------------
// 4 — an all-lateral pass has NO plunge observation.
// ---------------------------------------------------------------

/// An absent population must read absent, never "clean". A pass at
/// constant Z is measured, but its plunge observation is not.
#[test]
fn all_lateral_pass_has_no_plunge_population() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, -1.0));
    for i in 1..=10 {
        let x = f64::from(i) * 4.0;
        tp.feed_to(P3::new(x, 0.0, -1.0), FINISH_FEED);
    }

    let util = analyse(&tp, PLUNGE_RATE);
    assert!(util.is_measured());
    assert_eq!(util.fed_moves, 10);
    assert_eq!(util.plunge.population, 0);
    assert!(!util.plunge_is_measured());
    assert_eq!(util.plunge.peak_ratio, None);
    assert_eq!(util.ramp.population, 0);
    for m in &util.moves {
        assert_eq!(m.class, MotionClass::Lateral);
        assert!(m.achieved_z_rate_mm_min.abs() < 1e-12);
    }
    let bindings = util.bindings.expect("an all-lateral pass is measured");
    let sum = bindings.feed_bound + bindings.machine_bound;
    assert!((sum - 1.0).abs() < 1e-9, "binding fractions sum to {sum}");
}

// ---------------------------------------------------------------
// 5 — nothing fed means nothing measured.
// ---------------------------------------------------------------

/// The repo rule: an op with no fed move is ABSENT, never 0.0.
#[test]
fn empty_and_rapid_only_toolpaths_are_not_measured() {
    let empty = Toolpath::new();
    let util = analyse(&empty, PLUNGE_RATE);
    assert!(!util.is_measured());
    assert_eq!(util.fed_moves, 0);
    assert_eq!(util.utilization, None);
    assert_eq!(util.bindings, None);
    assert_eq!(util.time_weighted_commanded_mm_min, None);
    assert_eq!(util.time_weighted_achieved_mm_min, None);
    assert_eq!(util.headroom_estimate(1.3), None);
    assert!(util.moves.is_empty());

    let mut rapids = Toolpath::new();
    rapids.rapid_to(P3::new(0.0, 0.0, 10.0));
    rapids.rapid_to(P3::new(50.0, 0.0, 10.0));
    rapids.rapid_to(P3::new(50.0, 0.0, -1.0));
    let util = analyse(&rapids, PLUNGE_RATE);
    assert!(!util.is_measured());
    assert_eq!(util.fed_moves, 0);
    assert_eq!(util.utilization, None);
    assert_eq!(util.bindings, None);
    assert_eq!(util.headroom_estimate(1.3), None);
    // The rapids are still classified, and still carry no fed time.
    assert_eq!(util.moves.len(), 2);
    for m in &util.moves {
        assert!(m.is_rapid);
        assert_eq!(m.class, MotionClass::Retract);
    }
    assert!(util.fed_time_s.abs() < 1e-12);
    assert_eq!(util.rate_bound_axis_time_fraction, [0.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------
// 6 — long move reaches the command, short move does not.
// ---------------------------------------------------------------

/// The two ends of the utilisation scale on one machine and one feed.
/// At 925 mm/min and 500 mm/s2 the accel ramp needs 0.238 mm each way,
/// so 50 mm cruises and 0.3 mm never gets there.
#[test]
fn long_move_is_feed_bound_and_short_move_is_machine_bound() {
    let long = lateral_move(50.0);
    let util = analyse(&long, PLUNGE_RATE);
    assert_eq!(util.moves[0].binding, KinematicBinding::FeedBound);
    let u = util.utilization.expect("measured");
    assert!((u - 1.0).abs() < 1e-9, "long move utilisation {u:.6}");
    let bindings = util.bindings.expect("measured");
    assert!((bindings.feed_bound - 1.0).abs() < 1e-9);
    assert!(bindings.machine_bound.abs() < 1e-9);

    let short = lateral_move(0.3);
    let util = analyse(&short, PLUNGE_RATE);
    assert_eq!(
        util.moves[0].binding,
        KinematicBinding::AccelBound,
        "a 0.3 mm move at 925 mm/min cannot reach the command"
    );
    let u = util.utilization.expect("measured");
    assert!(u < 1.0, "short move utilisation {u:.6} should be below 1.0");
    let bindings = util.bindings.expect("measured");
    assert!(bindings.machine_bound > 0.0);
}

// ---------------------------------------------------------------
// 7 — a synthetic finish splits its time between the two bindings.
// ---------------------------------------------------------------

/// 200 alternating moves at constant Z, 0.86 mm and 0.30 mm long — a
/// mean move of 0.58 mm, which is the wanaka finish figure the plan
/// records. Every corner turns 90 degrees, so the junction velocity
/// drops to about 295 mm/min and the short moves cannot climb back to
/// 925 mm/min inside their own length.
///
/// NOTE on the short length. The plan proposed 0.5 mm. At 925 mm/min
/// and 500 mm/s2 the accel ramp needs only 0.475 mm even from a dead
/// stop, so a 0.5 mm move is `FeedBound` too and the whole population
/// reads 100 % feed-bound — a vacuous assertion. 0.30 mm is the honest
/// fixture.
///
/// The measured wanaka figure was 52 % feed-bound. It is NOT asserted
/// here: it comes from a user-local project, and this fixture is
/// synthetic. See the ignored instrument run at the end of this file.
fn synthetic_finish() -> Toolpath {
    const LONG_MM: f64 = 0.86;
    const SHORT_MM: f64 = 0.30;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, -1.0));
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    for i in 0..100 {
        if i % 2 == 0 {
            x += LONG_MM;
        } else {
            x -= LONG_MM;
        }
        tp.feed_to(P3::new(x, y, -1.0), FINISH_FEED);
        y += SHORT_MM;
        tp.feed_to(P3::new(x, y, -1.0), FINISH_FEED);
    }
    tp
}

#[test]
fn synthetic_finish_splits_fed_time_between_feed_and_machine_bound() {
    let tp = synthetic_finish();
    let util = analyse(&tp, PLUNGE_RATE);
    assert!(util.is_measured());
    assert_eq!(util.fed_moves, 200);
    assert_eq!(util.plunge.population, 0);

    let bindings = util.bindings.expect("measured");
    assert!(
        bindings.feed_bound > 0.2 && bindings.feed_bound < 0.8,
        "feed-bound fraction {:.4} should sit between 0.2 and 0.8",
        bindings.feed_bound
    );
    let sum = bindings.feed_bound + bindings.machine_bound;
    assert!(
        (sum - 1.0).abs() < 1e-9,
        "feed_bound + machine_bound = {sum:.12}, expected 1.0"
    );
    // The short moves run out of length; nothing here is rate-bound,
    // and the JunctionBound caveat says the corner cost lands on accel.
    assert!(bindings.accel_bound > 0.0);
    assert!(bindings.rate_bound.abs() < 1e-12);
    assert_eq!(util.rate_bound_axis_time_fraction, [0.0, 0.0, 0.0]);

    let u = util.utilization.expect("measured");
    assert!(u > 0.0 && u < 1.0, "utilisation {u:.4}");
}

// ---------------------------------------------------------------
// 8 — the headroom estimate reads the feed-bound share.
// ---------------------------------------------------------------

/// A 30 % constant-chipload feed rise cannot save 30 % of the time: the
/// accel-bound moves do not move at all. The saving must land strictly
/// inside `(0, 0.30)`.
#[test]
fn headroom_estimate_is_positive_and_below_the_feed_rise() {
    let tp = synthetic_finish();
    let util = analyse(&tp, PLUNGE_RATE);
    let saved = util.headroom_estimate(1.30).expect("measured toolpath");
    assert!(
        saved > 0.0 && saved < 0.30,
        "a 1.30x feed rise saved {saved:.4} of the fed time; expected 0 < x < 0.30"
    );
    // The stored field is the SAME solve, done once by `analyse_toolpath`.
    // Every display surface reads it instead of re-solving per row per
    // frame, and it is the only headroom that survives serialisation
    // (`moves` is `#[serde(skip)]`). If these two ever disagree, a surface
    // is showing a different number from the instrument.
    assert_eq!(
        util.headroom_at_1_30,
        Some(saved),
        "the precomputed headroom must equal headroom_estimate(1.30)"
    );
    // A rise of 1.0 changes nothing.
    let none = util.headroom_estimate(1.0).expect("measured toolpath");
    assert!(none.abs() < 1e-12, "a 1.0x rise saved {none:.12}");
    // A nonsense scale is refused, not guessed.
    assert_eq!(util.headroom_estimate(0.0), None);
    assert_eq!(util.headroom_estimate(f64::NAN), None);
}

// ---------------------------------------------------------------
// 9 — the real wanaka front rough (instrument run, user-local).
// ---------------------------------------------------------------

/// INSTRUMENT RUN, not a gate. `#[ignore]` by repo convention: it needs
/// a user-local project, it generates an adaptive3d rough and it runs a
/// full dexel simulation, so it is minutes of work and it cannot run on
/// any other machine.
///
/// Run it with:
/// `cargo test -p rs_cam_core --test kinematic_utilization_p2 -- --ignored`
///
/// This is the 2026-09-07 incident, measured through the instrument.
/// The adaptive3d rough emits its vertical step-down descents as plain
/// cutting moves with no `EntryPlunge` tag, so the feed modulator lifts
/// them to the lateral chipload-band maximum — 1807 mm/min against the
/// operation's 512 mm/min plunge rate. The modulated feed lives on the
/// cut trace, not on the stored toolpath, so the test rebuilds the
/// EMITTED motion before it measures (`feedback_measure_emitted_motion`).
///
/// The expected reading is NOT 1807 / 512 = 3.53. `$112 = 1000` clamps
/// the descent, so the achieved Z rate is 1000 mm/min and the ratio is
/// about 1000 / 512 = 1.95. The bar is `> 1.5`, and the actual value is
/// in the failure message. AFTER Phase 3 lands its geometric guard this
/// test must be re-read: the ratio should fall to 1.0 or below, and the
/// assertion below then becomes the proof the guard works.
#[test]
#[ignore = "instrument run: needs the user-local wanaka200 project and a full simulation"]
fn wanaka_front_rough_reports_the_plunge_class_peak() {
    use rs_cam_core::feed_modulation::ModulationStrategy;
    use rs_cam_core::session::{ProjectSession, SimulationOptions};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("..");
    path.push("planning");
    path.push("airrun_2026-08-19");
    path.push("wanaka200.toml");
    if !path.exists() {
        // Not this machine. A silent skip, because `print_stderr` is
        // denied in tests.
        return;
    }

    let mut session = ProjectSession::load(&path).expect("load wanaka200");
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_ricky_tuned());
    machine.max_feed_mm_min = MAX_FEED;
    session.set_machine(machine);

    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name.contains("Rough (front)"))
        .expect("wanaka200 carries a front rough");
    let front_id = session.toolpath_configs()[index].id;
    let skip_ids: Vec<ToolpathId> = session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .filter(|id| *id != front_id)
        .collect();

    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("generate the front rough");
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids,
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: true,
        modulation_strategy: ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulate the front rough");

    let plunge_rate = session
        .get_toolpath_params(index)
        .expect("front rough params")
        .plunge_rate();
    let kinematics = session.machine().effective_kinematics();
    let max_feed = session.machine().max_feed_mm_min;

    // Rebuild the EMITTED motion: the modulator writes its feeds onto
    // the cut trace, never back onto the stored toolpath.
    let sim = session.simulation_result().expect("simulation result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");
    let result = session.get_result(index).expect("front rough result");
    let mut emitted = result.toolpath().clone();
    for (i, m) in emitted.moves.iter_mut().enumerate() {
        if let Some((feed, _binding)) = trace.modulated_feeds.get(&(front_id, i)) {
            m.move_type = m.move_type.with_feed_rate(*feed);
        }
    }

    // The modulator returns silently when the tool has no vendor
    // chipload band. Separate that fixture failure from the reading:
    // without it an un-modulated path reads about 1.0 and looks exactly
    // like a Phase 3 fix.
    let modulated = trace
        .modulated_feeds
        .keys()
        .filter(|(id, _)| *id == front_id)
        .count();
    assert!(
        modulated > 0,
        "the modulator did not fire on the front rough (0 modulated feeds); \
         the plunge reading below would measure commanded, not emitted, motion"
    );

    let util = analyse_toolpath(
        &emitted,
        front_id,
        &kinematics,
        max_feed,
        max_feed,
        plunge_rate,
    );
    assert!(
        util.is_measured(),
        "the front rough must carry fed moves; fed_moves = {}",
        util.fed_moves
    );
    assert!(
        util.plunge_is_measured(),
        "the front rough must carry plunge-class descents; population = {}",
        util.plunge.population
    );
    let peak = util
        .plunge
        .peak_ratio
        .expect("a measured plunge population");
    assert!(
        peak > 1.5,
        "PRE-PHASE-3 READING: plunge-class peak ratio {peak:.3} against a \
         {plunge_rate:.0} mm/min plunge rate ({} of {} plunges above 1x, worst \
         achieved z-rate {:?} mm/min at move {:?}). Expected about 1.95 \
         (the 1807 mm/min command clamped to $112 = 1000). A reading at or \
         below 1.0 means the Phase 3 guard has landed — update this sentry.",
        util.plunge.over_1x,
        util.plunge.population,
        util.plunge.worst_achieved_z_rate_mm_min,
        util.plunge.worst_move_index
    );
}
