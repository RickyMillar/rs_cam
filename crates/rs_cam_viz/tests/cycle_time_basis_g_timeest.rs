//! G-TIMEEST sentry — a cycle time must never travel without its basis.
//!
//! Found 2026-08-22 at the machine: the GUI timeline read **25 min** for a job
//! the simulator measured at **10,781.7 s**. Four operator-facing surfaces —
//! the readiness panel, the pre-flight gate, the timeline readout, and the
//! playback speed baseline derived from that readout — each carried their own
//! copy of
//!
//! ```text
//! let feed = tc.operation.feed_rate();
//! let op_time = (result.stats.cutting_distance / feed) * 60.0;
//! ```
//!
//! and printed the result under the word "cycle time".
//!
//! The gap is acceleration, and `accel_limited_short_segments_are_a_different_
//! quantity` below shows it is arithmetic rather than fudge: on segments short
//! enough that the machine never reaches commanded feed, `distance / feed` is
//! not an approximation of cycle time, it is a different quantity. The rest of
//! the file pins that whichever quantity a surface ends up showing, it also
//! reports which one it is.
//!
//! Evidence class: pure-function unit tests over
//! [`rs_cam_viz::ui::readiness::toolpath_cycle_time`] (the single decision all
//! four surfaces now route through) plus one integrator arithmetic check
//! against `rs_cam_core::machine_kinematics`. No egui is rendered; the
//! basis → rendered-string mapping is read from source at each surface.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use rs_cam_core::ToolpathId;
use rs_cam_core::geo::P3;
use rs_cam_core::machine_kinematics::{CycleTimeBreakdown, MachineKinematics, compute_cycle_time};
use rs_cam_core::simulation_cut::{
    SimulationCutTrace, SimulationToolpathCutSummary, ToolpathKinematicRuntime,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_viz::ui::readiness::{
    CycleTime, CycleTimeBasis, format_cycle_time, toolpath_cycle_time,
};

const TP: ToolpathId = ToolpathId(7);
const OTHER_TP: ToolpathId = ToolpathId(9);

/// A per-toolpath trace summary claiming `total_runtime_s` seconds.
///
/// `runtime_by_intent` is the load-bearing field: `apply_kinematics_cycle_time`
/// (`compute/simulate.rs`) is its only writer, and it stamps that field in the
/// same pass that OVERWRITES `total_runtime_s` with the integrator's answer.
/// Its presence therefore is the answer to "is this runtime accel-aware?".
fn summary(id: ToolpathId, total_runtime_s: f64, integrated: bool) -> SimulationToolpathCutSummary {
    SimulationToolpathCutSummary {
        toolpath_id: id,
        sample_count: 100,
        total_runtime_s,
        cutting_runtime_s: total_runtime_s,
        rapid_runtime_s: 0.0,
        air_cut_time_s: 0.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.3,
        peak_chipload_mm_per_tooth: 0.05,
        peak_axial_doc_mm: 2.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 500.0,
        average_mrr_mm3_s: 20.0,
        metrics_not_applicable: false,
        per_kinematics: BTreeMap::new(),
        runtime_by_intent: integrated.then(|| CycleTimeBreakdown {
            total_s: total_runtime_s,
            cutting_s: total_runtime_s,
            ..CycleTimeBreakdown::default()
        }),
    }
}

fn trace(summaries: Vec<SimulationToolpathCutSummary>) -> SimulationCutTrace {
    SimulationCutTrace {
        toolpath_summaries: summaries,
        ..SimulationCutTrace::test_fixture()
    }
}

/// A trace where the integrator walked `runtimes` but only `summaries` have
/// engagement metrics — the shape a project with drill toolpaths produces.
fn trace_with_runtimes(
    summaries: Vec<SimulationToolpathCutSummary>,
    runtimes: Vec<(ToolpathId, f64)>,
) -> SimulationCutTrace {
    SimulationCutTrace {
        toolpath_summaries: summaries,
        toolpath_runtimes: runtimes
            .into_iter()
            .map(|(toolpath_id, total_s)| ToolpathKinematicRuntime {
                toolpath_id,
                breakdown: CycleTimeBreakdown {
                    total_s,
                    cutting_s: total_s,
                    ..CycleTimeBreakdown::default()
                },
            })
            .collect(),
        ..SimulationCutTrace::test_fixture()
    }
}

// ── G-DRILLTIME ─────────────────────────────────────────────────────────
//
// Found 2026-08-22 in the live validation of the G-TIMEEST consolidation
// above, by reading the GUI rather than the code: a project that *carries*
// machine kinematics still read `2:02:34 (cutting only, no accel)` — the
// weakest basis — on every surface.
//
// Every layer behaved as designed. Drill toolpaths set
// `metrics_not_applicable` and publish `drill_summaries` instead of a
// `toolpath_summaries` row, `apply_kinematics_cycle_time` folded the project
// total over `toolpath_summaries`, so a drill's runtime was computed and then
// discarded; `toolpath_cycle_time` found no summary, correctly fell back to
// `cutting_distance / feed`, and `worse()` correctly degraded the project.
//
// The composite answer was useless: **61.66 s of drill motion — 0.8 % of the
// runtime — relabelled 100 % of the estimate.** The root cause is that one
// slot carried two different facts, "has no engagement metrics" and "was not
// integrated". `SimulationCutTrace::toolpath_runtimes` is the second fact
// given its own slot.
//
// Note what is deliberately NOT done: drills are not *exempted* from the fold.
// That would restore the overclaim the basis exists to prevent. They are
// integrated, which is a different thing.

/// A toolpath the integrator walked reads `MachineModel` even with no
/// engagement summary — the drill case.
#[test]
fn an_integrated_toolpath_without_an_engagement_summary_is_machine_model() {
    let drill = ToolpathId(7);
    let t = trace_with_runtimes(Vec::new(), vec![(drill, 46.8)]);

    let ct = toolpath_cycle_time(Some(&t), drill, 234.0, 300.0);
    assert_eq!(
        ct.basis,
        Some(CycleTimeBasis::MachineModel),
        "a drill toolpath has no `toolpath_summaries` row by design; reading \
         only that list is what sent it down the CuttingOnly fallback"
    );
    assert!((ct.seconds - 46.8).abs() < 1e-9, "got {}", ct.seconds);

    // Non-vacuity, and the pin on the actual defect: the fallback this
    // replaces would have produced a *different, plausible* number from the
    // same inputs — 234 mm / 300 mm/min = 46.8 s. Identical here on purpose,
    // so the test above cannot pass by accidentally taking the old path.
    // Re-ask with a runtime the fallback cannot produce.
    let t2 = trace_with_runtimes(Vec::new(), vec![(drill, 61.66)]);
    let ct2 = toolpath_cycle_time(Some(&t2), drill, 234.0, 300.0);
    assert!(
        (ct2.seconds - 61.66).abs() < 1e-9,
        "the integrated runtime must win over `distance / feed`, got {}",
        ct2.seconds
    );
}

/// The operator-visible statement: one un-integrated drill no longer drags a
/// fully-modelled project to the weakest label.
#[test]
fn drill_ops_no_longer_degrade_the_project_basis() {
    let mill = ToolpathId(4);
    let drill = ToolpathId(7);
    let t = trace_with_runtimes(
        vec![summary(mill, 7292.0, true)],
        vec![(mill, 7292.0), (drill, 61.66)],
    );

    let mut total = CycleTime::NONE;
    total.fold(toolpath_cycle_time(Some(&t), mill, 10_000.0, 1200.0));
    total.fold(toolpath_cycle_time(Some(&t), drill, 234.0, 300.0));

    assert_eq!(
        total.basis,
        Some(CycleTimeBasis::MachineModel),
        "0.8 % of runtime must not relabel the other 99.2 %"
    );
    assert!(
        (total.seconds - 7353.66).abs() < 1e-9,
        "and the drill's time is still counted, not exempted: {}",
        total.seconds
    );
}

/// The control. Remove the integration and the old behaviour comes back — so
/// the test above is measuring the fix and not the absence of a fold.
#[test]
fn without_integration_a_drill_still_degrades_the_basis() {
    let mill = ToolpathId(4);
    let drill = ToolpathId(7);
    let t = trace(vec![summary(mill, 7292.0, true)]);

    let mut total = CycleTime::NONE;
    total.fold(toolpath_cycle_time(Some(&t), mill, 10_000.0, 1200.0));
    total.fold(toolpath_cycle_time(Some(&t), drill, 234.0, 300.0));

    assert_eq!(
        total.basis,
        Some(CycleTimeBasis::CuttingOnly),
        "a toolpath the integrator never walked must still weaken the claim"
    );
}

/// The mechanism, in isolation: a path of 0.40 mm segments commanded at F3000
/// on the wanaka machine's real limits (`$120/$121` = 500 mm/s², `$11` = 0.02).
///
/// Reaching 50 mm/s from rest at 500 mm/s² needs `v²/2a` = 2.5 mm of runway —
/// six times the segment length — so the machine spends every segment on a
/// ramp and never sees commanded feed. This is why the operator's 25 min and
/// the simulator's ~3 h are not two estimates of one number.
#[test]
fn accel_limited_short_segments_are_a_different_quantity() {
    const SEG_MM: f64 = 0.4;
    const FEED_MM_MIN: f64 = 3000.0;
    const LEGS: usize = 400;

    // A staircase: every junction turns 90°, which is what a corner-heavy 3D
    // finish looks like to the planner even though its geometry is smooth.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    let (mut x, mut y) = (0.0, 0.0);
    for leg in 0..LEGS {
        if leg % 2 == 0 {
            x += SEG_MM;
        } else {
            y += SEG_MM;
        }
        tp.feed_to(P3::new(x, y, 0.0), FEED_MM_MIN);
    }

    let kin = MachineKinematics {
        acceleration_mm_s2: 500.0,
        acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
        junction_deviation_mm: 0.02,
        ..MachineKinematics::default()
    };
    let integrated_s = compute_cycle_time(&tp, &kin, 5000.0, 5000.0);

    // The naive quantity every pre-G-TIMEEST surface published.
    let naive_s = (SEG_MM * LEGS as f64 / FEED_MM_MIN) * 60.0;

    assert!(
        integrated_s > naive_s * 3.0,
        "on 0.40 mm segments the machine model must be multiples of distance/feed, \
         not a refinement of it: integrated {integrated_s:.3}s vs naive {naive_s:.3}s"
    );
}

/// A trace that carries `runtime_by_intent` came through the F-034 integrator,
/// so its runtime is a wall-clock prediction and is published as one.
#[test]
fn integrated_trace_reads_as_machine_model() {
    let t = trace(vec![summary(TP, 10_781.7, true)]);

    let got = toolpath_cycle_time(Some(&t), TP, 58_318.0, 3000.0);

    assert_eq!(got.basis, Some(CycleTimeBasis::MachineModel));
    assert!((got.seconds - 10_781.7).abs() < 1e-6);
}

/// Same trace shape, no integrator: every shipped `MachineProfile` preset has
/// `kinematics: None`, which bypasses `apply_kinematics_cycle_time` entirely
/// and leaves `total_runtime_s` as the dexel-sample `distance / feed` sum.
/// Real distance and real per-move feeds, but no acceleration — so it is
/// published under its own name, not as a wall clock.
#[test]
fn un_integrated_trace_reads_as_simulated_no_accel() {
    let t = trace(vec![summary(TP, 1500.0, false)]);

    let got = toolpath_cycle_time(Some(&t), TP, 58_318.0, 3000.0);

    assert_eq!(got.basis, Some(CycleTimeBasis::SimulatedNoAccel));
    assert!((got.seconds - 1500.0).abs() < 1e-6);
}

/// No simulated evidence for this toolpath — the pre-G-TIMEEST formula is
/// still the only thing available, and it is still published, but it now says
/// what it is instead of calling itself a cycle time.
#[test]
fn no_trace_coverage_falls_back_to_cutting_only_and_says_so() {
    // Trace exists but covers a different toolpath — the "I generated one more
    // op after simulating" case, which must not silently borrow the other op's
    // basis.
    let t = trace(vec![summary(OTHER_TP, 10_781.7, true)]);

    let covered = toolpath_cycle_time(Some(&t), OTHER_TP, 0.0, 3000.0);
    let uncovered = toolpath_cycle_time(Some(&t), TP, 58_318.0, 3000.0);
    let no_trace = toolpath_cycle_time(None, TP, 58_318.0, 3000.0);

    assert_eq!(covered.basis, Some(CycleTimeBasis::MachineModel));
    assert_eq!(uncovered.basis, Some(CycleTimeBasis::CuttingOnly));
    assert_eq!(no_trace.basis, Some(CycleTimeBasis::CuttingOnly));

    // 58_318 mm at F3000 = 1166.36 s — the ~19 min that reads as "25 min" once
    // the other ops are added, against the ~3 h the machine model measured.
    assert!((uncovered.seconds - 1166.36).abs() < 0.01);
    assert!((no_trace.seconds - 1166.36).abs() < 0.01);
}

/// A zero-feed op yields no estimate at all rather than a division blow-up.
/// `basis: None` means "no estimate", never "zero seconds" — the surfaces
/// render it as a dash.
#[test]
fn zero_feed_yields_no_estimate_not_zero() {
    let got = toolpath_cycle_time(None, TP, 58_318.0, 0.0);

    assert_eq!(got.basis, None);
    assert_eq!(got.seconds, 0.0);
}

/// A project total is only as trustworthy as its least-modelled op, so folding
/// keeps the WEAKEST basis. Without this a single wall-clock op would let a
/// mostly-unsimulated project present itself as measured.
#[test]
fn project_basis_degrades_to_the_weakest_contributor() {
    let mut total = CycleTime::NONE;
    assert_eq!(total.basis, None);

    total.fold(CycleTime::of(100.0, CycleTimeBasis::MachineModel));
    assert_eq!(total.basis, Some(CycleTimeBasis::MachineModel));

    total.fold(CycleTime::of(50.0, CycleTimeBasis::SimulatedNoAccel));
    assert_eq!(total.basis, Some(CycleTimeBasis::SimulatedNoAccel));

    total.fold(CycleTime::of(25.0, CycleTimeBasis::CuttingOnly));
    assert_eq!(total.basis, Some(CycleTimeBasis::CuttingOnly));

    // A better op arriving later must not upgrade the claim back.
    total.fold(CycleTime::of(5.0, CycleTimeBasis::MachineModel));
    assert_eq!(total.basis, Some(CycleTimeBasis::CuttingOnly));

    assert!((total.seconds - 180.0).abs() < 1e-9);

    // Folding "no estimate" neither adds time nor claims modelling.
    let mut only_none = CycleTime::NONE;
    only_none.fold(CycleTime::NONE);
    assert_eq!(only_none.basis, None);
    assert_eq!(only_none.seconds, 0.0);
}

/// Every basis names itself, and the two that under-report say which way they
/// err. A surface can print `qualifier()` without deciding anything, which is
/// what keeps the four of them from drifting apart again.
#[test]
fn every_basis_carries_a_qualifier_and_a_direction() {
    use rs_cam_viz::ui::readiness::CheckStatus;

    for basis in [
        CycleTimeBasis::MachineModel,
        CycleTimeBasis::SimulatedNoAccel,
        CycleTimeBasis::CuttingOnly,
    ] {
        assert!(!basis.qualifier().is_empty());
        assert!(!basis.caveat().is_empty());
    }

    assert_eq!(CycleTimeBasis::MachineModel.status(), CheckStatus::Pass);
    for optimistic in [
        CycleTimeBasis::SimulatedNoAccel,
        CycleTimeBasis::CuttingOnly,
    ] {
        assert_eq!(
            optimistic.status(),
            CheckStatus::Warning,
            "a basis that reads faster than the machine must not render as a clean Pass"
        );
        assert!(
            optimistic.caveat().contains("LONGER") || optimistic.caveat().contains("faster"),
            "the caveat must state which way the number errs: {}",
            optimistic.caveat()
        );
    }
}

/// Naming the defect is not enough — both optimistic bases carry the concrete
/// next step, and the wall-clock one carries none because there is nothing to
/// do. `SimulatedNoAccel` is the COMMON case (every shipped `MachineProfile`
/// preset has `kinematics: None`), so its remedy names the shipped affordance
/// that fixes it rather than telling the operator to go find one.
#[test]
fn the_optimistic_bases_name_their_remedy() {
    assert_eq!(CycleTimeBasis::MachineModel.remedy(), None);

    let no_kinematics = CycleTimeBasis::SimulatedNoAccel
        .remedy()
        .expect("the no-kinematics case must be actionable");
    assert!(
        no_kinematics.contains("Import GRBL $$"),
        "the remedy must name the affordance that exists \
         (properties/mod.rs draw_grbl_import): {no_kinematics}"
    );

    let no_sim = CycleTimeBasis::CuttingOnly
        .remedy()
        .expect("the un-simulated case must be actionable");
    assert!(no_sim.contains("simulation"), "{no_sim}");
}

/// The single duration formatter, which replaced TWO that disagreed: the
/// timeline's `m:ss` and the setup sheet's `"{m}m {s}s"`. Both rendered the
/// three-hour job that motivated this row as ~180 *minutes*, which is exactly
/// the misreading the whole finding is about.
#[test]
fn the_formatter_rolls_over_to_hours() {
    assert_eq!(format_cycle_time(0.0), "0:00");
    assert_eq!(format_cycle_time(125.0), "2:05");
    // The measured job. Pre-fix this printed "179:41" / "179m 41s".
    assert_eq!(format_cycle_time(10_781.7), "2:59:42");
    // Non-finite renders as the same dash an absent estimate uses, never 0:00.
    assert_eq!(format_cycle_time(f64::NAN), "\u{2014}");
}
