//! Sentry — the air-cut numerator and BOTH its denominators sit on one
//! time base (G-AIRDENOM, 2026-09-08).
//!
//! The defect: `SummaryAccumulator` accumulates `air_cut_time_s`,
//! `cutting_runtime_s` and `rapid_runtime_s` from the dexel's naive
//! `segment_time_s` — `segment_len / COMMANDED feed × 60`, no
//! acceleration. Two later passes then overwrite `total_runtime_s` alone
//! with the kinematics-integrated wall clock at the MODULATED feed
//! (`compute/simulate.rs` F-034, `session/compute.rs` F-036b). One
//! numerator, two time models. Where modulation raised the feed,
//! `cutting_runtime_s` exceeded `total_runtime_s` and
//! `air_cut_pct_of_cutting_time` read BELOW
//! `air_cut_pct_of_total_runtime` — the opposite of the order the trait
//! doc and `CLAUDE.md` both stated. Measured 1.76× on a wanaka rough
//! (`planning/ab_instrument_flags_2026-09-08.md` Flag 1).
//!
//! The fix is `simulation_cut::rebase_cutting_times`, called from the
//! F-036b re-timing loop: each cutting sample's naive seconds are scaled
//! by its own move's `commanded ÷ modulated` ratio, then the sum is
//! normalised onto the integrator's fed clock.
//!
//! This file pins four things:
//!
//!  (a) With modulation OFF nothing is rebased — the three fields still
//!      equal an independent naive re-derivation from the samples.
//!  (b) With modulation ON and the feed RAISED, the documented order
//!      holds again.
//!  (c) The same with the feed LOWERED.
//!  (d) The absolute seconds recovered from each percentage equal the
//!      published field, and `cutting + rapid == total` exactly — plus
//!      unit-level proof that the rebase is PER SAMPLE, which a single
//!      per-toolpath factor could not produce.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::{CycleTimeBreakdown, MachineKinematics};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetMachineArgs, SimulationOptions,
    ToolpathConfig,
};
use rs_cam_core::stock::simulation_cut::{
    AirCutRatios, Engagement, SimulationCutSample, SimulationCutTrace, rebase_cutting_times,
};
use rs_cam_core::tool_load::BindingConstraint;

// ── AS001 pocket fixture (the F-024 / F-035 / F-036b fixture) ───────────

fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm (G-AIRDENOM test)".to_owned();
    tool
}

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(40.0 + 10.0 * (-t).cos(), 30.0 + 10.0 * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

/// `feed_rate` is the dial that decides the modulator's DIRECTION. The
/// shipped AS001 value of 770 mm/min is under-fed for the Ø6 two-flute
/// hardwood pocket band, so the modulator raises it; 4000 mm/min is above
/// every ceiling in scope, so the modulator lowers it.
fn build_as001_pocket_session(feed_rate: f64) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    });

    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;

    let model_id = builder.add_model(LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![rounded_rect_with_island()])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth: 6.0,
            depth_per_pass: 2.0,
            feed_rate,
            plunge_rate: 385.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
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
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tc).expect("add pocket toolpath");
    let mut session = builder.build();

    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_stock());
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    session
}

fn opts(adaptive_feed_modulation: bool) -> SimulationOptions {
    SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn run(feed_rate: f64, modulate: bool) -> ProjectSession {
    let mut session = build_as001_pocket_session(feed_rate);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");
    session
        .run_simulation(&opts(modulate), &cancel)
        .expect("simulation completes");
    session
}

fn trace_of(session: &ProjectSession) -> Arc<SimulationCutTrace> {
    session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone())
        .expect("the simulation produced a cut trace")
}

/// The naive accumulator, re-derived from the sample stream. This is what
/// the three fields held before any rebase.
struct NaiveTimes {
    cutting_s: f64,
    rapid_s: f64,
    air_s: f64,
}

fn naive_times(trace: &SimulationCutTrace, toolpath_id: ToolpathId) -> NaiveTimes {
    let mut out = NaiveTimes {
        cutting_s: 0.0,
        rapid_s: 0.0,
        air_s: 0.0,
    };
    for sample in &trace.samples {
        if sample.toolpath_id != toolpath_id {
            continue;
        }
        if sample.is_cutting {
            out.cutting_s += sample.segment_time_s;
            if sample.engagement.radial_woc_fraction < 0.02 {
                out.air_s += sample.segment_time_s;
            }
        } else {
            out.rapid_s += sample.segment_time_s;
        }
    }
    out
}

/// How many cutting moves the modulator moved, and in which direction.
/// A verdict taken over an empty population proves nothing (CLAUDE.md),
/// so every modulated case asserts this first.
fn modulation_direction_counts(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> (usize, usize) {
    let mut commanded: BTreeMap<usize, f64> = BTreeMap::new();
    for sample in &trace.samples {
        if sample.toolpath_id == toolpath_id && sample.is_cutting {
            commanded
                .entry(sample.move_index)
                .or_insert(sample.feed_rate_mm_min);
        }
    }
    let (mut raised, mut lowered) = (0usize, 0usize);
    for (&(tp, move_index), &(feed, _binding)) in &trace.modulated_feeds {
        if tp != toolpath_id {
            continue;
        }
        let Some(&cmd) = commanded.get(&move_index) else {
            continue;
        };
        if feed > cmd * 1.001 {
            raised += 1;
        } else if feed < cmd * 0.999 {
            lowered += 1;
        }
    }
    (raised, lowered)
}

// ── (a) modulation OFF changes nothing ─────────────────────────────────

/// The rebase runs only inside `apply_adaptive_feed_modulation`, which
/// `modulate_simulation_trace` refuses to enter when the flag is off. So
/// with modulation off the three fields must still be the naive dexel
/// sums, bit-for-bit. This also guards a future move of the rebase to the
/// F-034 site, which would silently change every unmodulated fixture.
#[test]
fn modulation_off_leaves_the_naive_time_base_untouched() {
    let session = run(770.0, false);
    let trace = trace_of(&session);
    let tp = &trace.toolpath_summaries[0];
    let naive = naive_times(&trace, tp.toolpath_id);

    assert!(
        naive.cutting_s > 0.0 && naive.air_s > 0.0,
        "empty population: the fixture must produce cutting AND air samples \
         (cutting {:.3} s, air {:.3} s)",
        naive.cutting_s,
        naive.air_s
    );
    assert_eq!(
        tp.cutting_runtime_s, naive.cutting_s,
        "modulation off must leave cutting_runtime_s at the naive dexel sum"
    );
    assert_eq!(
        tp.rapid_runtime_s, naive.rapid_s,
        "modulation off must leave rapid_runtime_s at the naive dexel sum"
    );
    assert_eq!(
        tp.air_cut_time_s, naive.air_s,
        "modulation off must leave air_cut_time_s at the naive dexel sum"
    );
    assert!(
        trace.modulated_feeds.is_empty(),
        "modulation off must stamp no per-move feed map"
    );
}

// ── (b) + (c) the documented order, both modulation directions ─────────

/// The wanaka rough's shape: an under-fed pass the modulator RAISES. This
/// is the arm that inverted the order pre-fix — the raised feed shortened
/// `total_runtime_s` below the untouched naive `cutting_runtime_s`.
#[test]
fn raised_feed_keeps_cutting_pct_at_or_above_total_pct() {
    let session = run(770.0, true);
    let trace = trace_of(&session);
    let tp = &trace.toolpath_summaries[0];

    let (raised, _lowered) = modulation_direction_counts(&trace, tp.toolpath_id);
    assert!(
        raised > 0,
        "empty population: the modulator must actually RAISE feeds on this fixture"
    );
    assert!(
        tp.air_cut_time_s > 0.0 && tp.cutting_runtime_s > 0.0,
        "empty population: air {:.3} s of cutting {:.3} s",
        tp.air_cut_time_s,
        tp.cutting_runtime_s
    );
    assert!(
        tp.air_cut_pct_of_cutting_time() >= tp.air_cut_pct_of_total_runtime(),
        "G-AIRDENOM: cutting-time reading {:.4}% must be >= total-runtime reading {:.4}%",
        tp.air_cut_pct_of_cutting_time(),
        tp.air_cut_pct_of_total_runtime()
    );
    assert!(
        trace.summary.air_cut_pct_of_cutting_time() >= trace.summary.air_cut_pct_of_total_runtime(),
        "the project summary must satisfy the same order"
    );

    // **The pre-fix reproduction, in the same run.** Leave the numerator
    // and the cutting denominator on the naive clock — which is exactly
    // what shipped — and divide the same air seconds by the integrated
    // total. The order INVERTS. Without this the assertions above could
    // pass on a fixture that never carried the defect.
    let naive = naive_times(&trace, tp.toolpath_id);
    let prefix_total_pct = naive.air_s / tp.total_runtime_s * 100.0;
    let prefix_cutting_pct = naive.air_s / naive.cutting_s * 100.0;
    assert!(
        prefix_cutting_pct < prefix_total_pct,
        "this fixture no longer reproduces G-AIRDENOM (naive cutting {prefix_cutting_pct:.4}% \
         vs mixed-base total {prefix_total_pct:.4}%); the sentry above proves nothing until \
         the fixture inverts again"
    );
}

/// The finish's shape: an over-fed pass the modulator LOWERS. Pre-fix
/// this arm kept the documented order by accident, which is how the
/// defect survived — the sign of the inequality tracked the modulation
/// direction, not the air content.
#[test]
fn lowered_feed_keeps_cutting_pct_at_or_above_total_pct() {
    let session = run(4000.0, true);
    let trace = trace_of(&session);
    let tp = &trace.toolpath_summaries[0];

    let (_raised, lowered) = modulation_direction_counts(&trace, tp.toolpath_id);
    assert!(
        lowered > 0,
        "empty population: the modulator must actually LOWER feeds on this fixture"
    );
    assert!(
        tp.air_cut_time_s > 0.0 && tp.cutting_runtime_s > 0.0,
        "empty population: air {:.3} s of cutting {:.3} s",
        tp.air_cut_time_s,
        tp.cutting_runtime_s
    );
    assert!(
        tp.air_cut_pct_of_cutting_time() >= tp.air_cut_pct_of_total_runtime(),
        "G-AIRDENOM: cutting-time reading {:.4}% must be >= total-runtime reading {:.4}%",
        tp.air_cut_pct_of_cutting_time(),
        tp.air_cut_pct_of_total_runtime()
    );
}

// ── (d) the seconds recovered from a percentage, and ONE base ──────────

/// Both percentages must recover the same absolute air seconds, and the
/// per-toolpath clock must close: `cutting + rapid == total`. That
/// identity is the whole fix — pre-fix the left side was naive dexel
/// seconds and the right side an integrated wall clock.
#[test]
fn recovered_air_seconds_agree_and_the_clock_closes() {
    let session = run(770.0, true);
    let trace = trace_of(&session);
    let tp = &trace.toolpath_summaries[0];

    let from_total = tp.air_cut_pct_of_total_runtime() * tp.total_runtime_s / 100.0;
    let from_cutting = tp.air_cut_pct_of_cutting_time() * tp.cutting_runtime_s / 100.0;
    assert!(
        (from_total - from_cutting).abs() < 1e-9,
        "the two percentages must recover ONE air duration: {from_total:.9} vs {from_cutting:.9}"
    );
    assert!(
        (from_total - tp.air_cut_time_s).abs() < 1e-9,
        "recovered {from_total:.9} s must equal the published {:.9} s",
        tp.air_cut_time_s
    );

    let closed = tp.cutting_runtime_s + tp.rapid_runtime_s;
    assert!(
        (closed - tp.total_runtime_s).abs() < 1e-6,
        "one time base: cutting {:.6} + rapid {:.6} = {closed:.6} must equal total {:.6}",
        tp.cutting_runtime_s,
        tp.rapid_runtime_s,
        tp.total_runtime_s
    );

    // The rebase moved the cutting clock off the naive sum — otherwise
    // this test would pass on a tree where nothing changed.
    let naive = naive_times(&trace, tp.toolpath_id);
    assert!(
        (tp.cutting_runtime_s - naive.cutting_s).abs() > 1e-6,
        "the raised-feed arm must move cutting_runtime_s off the naive {:.6} s",
        naive.cutting_s
    );
}

/// **The rebase is PER SAMPLE, and a single per-toolpath factor cannot
/// reproduce it.** The modulator leaves a zero-engagement move at its
/// commanded feed and lifts an engaged move, so one average factor would
/// shrink the air seconds along with the engaged seconds — the same
/// defect in a smaller coat. Two moves, one untouched and one doubled,
/// make the difference arithmetic rather than incidental.
#[test]
fn the_rebase_is_per_sample_not_one_factor_per_toolpath() {
    let tp = ToolpathId(0);
    let air_sample = SimulationCutSample {
        toolpath_id: tp,
        move_index: 0,
        is_cutting: true,
        feed_rate_mm_min: 1000.0,
        segment_time_s: 10.0,
        engagement: Engagement {
            radial_woc_fraction: 0.0,
            ..Engagement::default()
        },
        ..SimulationCutSample::test_fixture()
    };
    let engaged_sample = SimulationCutSample {
        toolpath_id: tp,
        move_index: 1,
        is_cutting: true,
        feed_rate_mm_min: 1000.0,
        segment_time_s: 10.0,
        engagement: Engagement {
            radial_woc_fraction: 0.5,
            ..Engagement::default()
        },
        ..SimulationCutSample::test_fixture()
    };
    let samples = vec![air_sample, engaged_sample];

    // Move 0 keeps its commanded feed (no engagement); move 1 is doubled.
    let mut modulated: BTreeMap<(ToolpathId, usize), (f64, BindingConstraint)> = BTreeMap::new();
    modulated.insert((tp, 0), (1000.0, BindingConstraint::MachineMaxFeed));
    modulated.insert((tp, 1), (2000.0, BindingConstraint::ChiploadMax));

    // Fed clock = 15 s, matching the per-sample sum exactly, so the accel
    // normaliser is 1.0 and the arithmetic below is exact.
    let breakdown = CycleTimeBreakdown {
        total_s: 20.0,
        rapid_s: 4.0,
        cutting_s: 15.0,
        entry_s: 0.0,
        linking_s: 0.0,
        retract_s: 1.0,
        unknown_s: 0.0,
    };

    let rebased = rebase_cutting_times(&samples, tp, &modulated, &breakdown)
        .expect("a populated cutting set rebases");

    assert!((rebased.cutting_runtime_s - 15.0).abs() < 1e-9);
    // Air keeps its 10 s; the engaged move halves to 5 s.
    assert!(
        (rebased.air_cut_time_s - 10.0).abs() < 1e-9,
        "the untouched air move must keep its seconds, got {:.9}",
        rebased.air_cut_time_s
    );
    // A uniform factor would have given `10 × (15/20) = 7.5`. The
    // difference is exactly what the defect hid.
    assert!(
        (rebased.air_cut_time_s - 7.5).abs() > 1e-6,
        "a uniform per-toolpath factor would read 7.5 s here"
    );
    assert!((rebased.rapid_runtime_s - 5.0).abs() < 1e-9);
    assert!(
        (rebased.cutting_runtime_s + rebased.rapid_runtime_s - breakdown.total_s).abs() < 1e-9,
        "one time base: the rebased clock must close on the integrator's total"
    );
}

/// A toolpath with no cutting samples must be left alone, not zeroed —
/// `None` means "not rebased", and the caller keeps the accumulator's
/// value. Absent is not zero (CLAUDE.md).
#[test]
fn a_toolpath_with_no_cutting_samples_is_not_rebased() {
    let tp = ToolpathId(0);
    let rapid_only = vec![SimulationCutSample {
        toolpath_id: tp,
        move_index: 0,
        is_cutting: false,
        feed_rate_mm_min: 1000.0,
        segment_time_s: 10.0,
        ..SimulationCutSample::test_fixture()
    }];
    let breakdown = CycleTimeBreakdown {
        total_s: 10.0,
        rapid_s: 10.0,
        ..CycleTimeBreakdown::default()
    };
    assert!(
        rebase_cutting_times(&rapid_only, tp, &BTreeMap::new(), &breakdown).is_none(),
        "no cutting samples means no rebase"
    );

    // The same refusal when the integrator reports no fed time at all.
    let all_rapid = CycleTimeBreakdown {
        total_s: 4.0,
        rapid_s: 4.0,
        ..CycleTimeBreakdown::default()
    };
    let cutting = vec![SimulationCutSample {
        toolpath_id: tp,
        move_index: 0,
        is_cutting: true,
        feed_rate_mm_min: 1000.0,
        segment_time_s: 10.0,
        ..SimulationCutSample::test_fixture()
    }];
    assert!(
        rebase_cutting_times(&cutting, tp, &BTreeMap::new(), &all_rapid).is_none(),
        "an integrator total with no fed time means no rebase"
    );
}
