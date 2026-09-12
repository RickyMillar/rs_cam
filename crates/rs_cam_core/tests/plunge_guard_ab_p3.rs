//! Phase 3 paired A/B — the instrument judges the guard
//! (`planning/machine_kinematics_confidence_2026-09-07.md`).
//!
//! INSTRUMENT RUN, not a gate. `#[ignore]` by repo convention: it needs
//! the user-local wanaka200 project, it generates an adaptive3d rough
//! and it runs a full dexel simulation, so it is minutes of work and it
//! cannot run on any other machine.
//!
//! ```text
//! cargo test -p rs_cam_core --test plunge_guard_ab_p3 -- --ignored --nocapture
//! ```
//!
//! # Protocol — how the two arms are produced
//!
//! The guard has no runtime dial in production, and it must not get one:
//! a test-only switch on a safety cap is exactly the thing that later
//! ships disarmed. The arms are therefore produced by ONE temporary edit
//! at the modulator's single build site, `session/compute.rs`:
//!
//! * **Arm A (pre-guard)** — `plunge_rate_mm_min: f64::INFINITY`. The
//!   documented disable: the guard's own finiteness test declines, and
//!   `should_skip_modulation` is not touched, so the intent skip behaves
//!   exactly as it does in production.
//! * **Arm B (post-guard)** — `plunge_rate_mm_min:
//!   operation.plunge_rate()`, i.e. the shipped code.
//!
//! Deliberately NOT done by raising the operation's own `plunge_rate`
//! parameter: the generator reads that value for its TAGGED entry moves,
//! so arm A would carry a different toolpath, not a different modulation
//! of the same one. The analysis denominator reads
//! `tc.operation.plunge_rate()` separately, so it stays at the op's real
//! 541 mm/min in both arms and the two ratios are comparable.
//!
//! Each arm writes a per-move dump to `target/plunge_guard_ab/<arm>.txt`
//! (`PLUNGE_GUARD_AB_ARM`, default `b`), so the byte-identity bar can be
//! checked ACROSS the two processes with `diff`.
//!
//! # Measured, 2026-09-07 — wanaka200 "6 3D Rough (front)"
//!
//! Machine: user-tuned Shapeoko XXL (`$120/$121/$122 = 500/500/270`,
//! `$110/$111/$112 = 10000/10000/1000`, `$11 = 0.020`). Simulation
//! resolution 0.5 mm, `ConstrainedMax`, aggressiveness 1.0. The
//! operation's own dials: plunge rate **541 mm/min**, feed rate **750
//! mm/min** (the plan's "512" was the earlier hand-analysis figure; the
//! project file says 541, and both arms divide by the same 541).
//!
//! ```text
//!                              ARM A (no guard)   ARM B (guard)
//!   plunge.peak_ratio               1.8484            1.3863
//!   plunge.over_1x                    125                 7
//!   plunge.over_2x                      0                 0
//!   plunge.population                 588               588
//!   worst achieved z-rate      1000.0 mm/min      750.0 mm/min
//!   worst move index                  959              8367
//!   fed_time_s                   2056.136          2065.014
//!   fed_moves                       11579             11579
//!   utilization                    0.99966           0.99977
//!   lateral/ramp median dF        127.163 %         127.163 %
//!   modulated feeds                 11109             11109
//! ```
//!
//! Bars:
//!
//! * **Lateral / Ramp byte-identity — HELD.** `diff` over the two dumps:
//!   236 differing lines, **all 118 of them `Plunge`-class** (each move
//!   appears once per arm). All **6116** `Lateral` and `Ramp` lines are
//!   byte-identical. The lateral median feed delta is unchanged to the
//!   last printed digit.
//! * **Fed time — REPORTED.** 2056.136 s → 2065.014 s, **+8.878 s
//!   (+0.43 %)**. Positive and small, as expected: 118 descents got
//!   slower and nothing else moved. (This is the instrument's
//!   peak-velocity time base, not `compute_cycle_time`.)
//! * **`peak_ratio <= 1.0` and `over_1x == 0` — NOT HELD as written, and
//!   the reason is not the guard.** Inside the guard's own population —
//!   the moves the modulator VISITED — the reading is **0 of 588 over
//!   1x**, down from 118. The residual **7** are all
//!   `MoveIntent::EntryPlunge` (moves 8367, 8375, 8385, 8396, 8442,
//!   8466, 8612), which `should_skip_modulation` claims by the
//!   reviewer's ruling, so the guard never sees them. They descend at
//!   **750 mm/min — the operation's FEED rate, not its 541 mm/min plunge
//!   rate**, so `1.3863 = 750 / 541` exactly. That is a GENERATOR defect
//!   (the adaptive3d entry planner emits a correctly-TAGGED plunge at
//!   the wrong rate), and the plan already carries its fix as a separate
//!   item: "optionally tag the adaptive3d vertical descents
//!   `EntryPlunge` at the generator ... as a separate commit".
//!   Phase 4's non-blocking backstop will speak on this fixture until
//!   that lands, which is what a monitor is for.
//!
//! The A/B therefore isolates the guard exactly: it removed **118 of the
//! 125** over-1x plunges — every one it is allowed to touch — for 0.43 %
//! of fed time and no change to a single lateral feed.
//!
//! **Update 2026-09-07 (pre-merge).** The residual 7 are attributed to the
//! boundary clipper rather than the adaptive3d entry planner — by exact
//! feed match plus clustered move indices, NOT by a trace — and that
//! mechanism is FIXED as G-BOUNDARYPLUNGE:
//! `boundary::clip_toolpath_to_boundary_set_with_provenance` now emits its
//! re-entry descent at the operation's plunge rate (sentry
//! `boundary_reentry_plunge_rate_g_boundaryplunge.rs`). The sentry proves
//! the mechanism, not that these seven moves came through it. The 1.3863
//! whole-population peak recorded above is the reading BEFORE the fix; a
//! re-run of this instrument should read 1.0 if the attribution is right,
//! and is what would confirm it. The bars below are unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // This file's whole purpose is to publish a measurement.
    clippy::print_stderr
)]

use rs_cam_core::ids::ToolpathId;
use rs_cam_core::kinematic_utilization::{MotionClass, analyse_toolpath};
use rs_cam_core::machine_kinematics::MachineKinematics;

/// Machine travel-rate cap (mm/min) — the user's `$110/$111`.
const MAX_FEED: f64 = 10_000.0;

/// Which arm this process is measuring. Names the dump file only; the
/// arm itself is decided by the `session/compute.rs` edit described
/// above.
fn arm_name() -> String {
    std::env::var("PLUNGE_GUARD_AB_ARM").unwrap_or_else(|_| "b".to_owned())
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = xs.len() / 2;
    Some(if xs.len() % 2 == 1 {
        xs[mid]
    } else {
        (xs[mid - 1] + xs[mid]) / 2.0
    })
}

#[test]
#[ignore = "instrument run: needs the user-local wanaka200 project and a full simulation"]
fn wanaka_front_rough_plunge_guard_ab() {
    use rs_cam_core::feed_modulation::ModulationStrategy;
    use rs_cam_core::session::{Command, ProjectSession, SetMachineArgs, SimulationOptions};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("..");
    path.push("planning");
    path.push("airrun_2026-08-19");
    path.push("wanaka200.toml");
    if !path.exists() {
        eprintln!("SKIP: {} is not on this machine.", path.display());
        return;
    }

    let mut session = ProjectSession::load(&path).expect("load wanaka200");
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_ricky_tuned());
    machine.max_feed_mm_min = MAX_FEED;
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");

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

    let op = session
        .get_toolpath_params(index)
        .expect("front rough params");
    let plunge_rate = op.plunge_rate();
    let commanded_feed = op.feed_rate();
    let kinematics = session.machine().effective_kinematics();
    let max_feed = session.machine().max_feed_mm_min;

    // EMITTED motion. Post-simulation the stored result already carries
    // the modulated feeds (`session/compute.rs` writes the modulated
    // clone back), and re-applying the trace's map is idempotent — the
    // rebuild is kept because it also proves the modulator fired.
    let sim = session.simulation_result().expect("simulation result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");
    let result = session.get_result(index).expect("front rough result");
    let mut emitted = result.toolpath().clone();
    for (i, m) in emitted.moves.iter_mut().enumerate() {
        if let Some((feed, _binding)) = trace.modulated_feeds.get(&(front_id, i)) {
            m.move_type = m.move_type.with_feed_rate(*feed);
        }
    }
    let modulated = trace
        .modulated_feeds
        .keys()
        .filter(|(id, _)| *id == front_id)
        .count();
    assert!(
        modulated > 0,
        "the modulator did not fire on the front rough (0 modulated feeds); \
         both arms would measure commanded motion and the A/B would be vacuous"
    );

    let util = analyse_toolpath(
        &emitted,
        front_id,
        &kinematics,
        max_feed,
        max_feed,
        plunge_rate,
    );
    assert!(util.is_measured(), "the front rough must carry fed moves");

    // The modulator's OWN median, over every visited move against the
    // op's single commanded feed. Capped plunges pull it down, so it is
    // reported but it is NOT the bar.
    let summary_median = trace
        .modulation_summaries
        .get(&front_id)
        .map(|s| s.median_feed_delta_pct);
    // The bar's median: lateral and ramp classes only — the population
    // the guard must leave untouched.
    let lateral_deltas: Vec<f64> = util
        .moves
        .iter()
        .filter(|m| matches!(m.class, MotionClass::Lateral | MotionClass::Ramp))
        .filter(|m| {
            trace
                .modulated_feeds
                .contains_key(&(front_id, m.move_index))
        })
        .filter(|_| commanded_feed > 0.0)
        .map(|m| (m.commanded_mm_min - commanded_feed) / commanded_feed * 100.0)
        .collect();
    let lateral_population = lateral_deltas.len();
    let lateral_median = median(lateral_deltas);

    let arm = arm_name();
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("..");
    dir.push("..");
    dir.push("target");
    dir.push("plunge_guard_ab");
    std::fs::create_dir_all(&dir).expect("create the dump directory");
    let mut dump = String::new();
    for m in &util.moves {
        // `commanded_mm_min` on a fed move IS the emitted `F` word: the
        // instrument reads the move list it was handed, and this one
        // carries emitted feeds. Bits, not a rounded decimal — the bar is
        // byte-identity.
        dump.push_str(&format!(
            "{} {:?} {:016x}\n",
            m.move_index,
            m.class,
            m.commanded_mm_min.to_bits()
        ));
    }
    let path_out = dir.join(format!("{arm}.txt"));
    std::fs::write(&path_out, dump).expect("write the per-move dump");

    // Every plunge still above 1x, named. The guard owns exactly the moves
    // the modulator VISITED; a move the intent skip claimed keeps its
    // commanded feed by reviewer ruling, so the two populations must be
    // told apart before any residual is read as a guard failure.
    let mut residual = String::new();
    let mut residual_visited = 0_usize;
    let mut residual_skipped = 0_usize;
    for m in &util.moves {
        let Some(ratio) = m.plunge_ratio else {
            continue;
        };
        if ratio <= 1.0 {
            continue;
        }
        let visited = trace
            .modulated_feeds
            .contains_key(&(front_id, m.move_index));
        if visited {
            residual_visited += 1;
        } else {
            residual_skipped += 1;
        }
        if residual.lines().count() < 20 {
            residual.push_str(&format!(
                "    move {:>6} intent {:?} visited-by-modulator {} feed {:.1} \
                 z-rate {:.1} ratio {:.3}\n",
                m.move_index,
                emitted.moves[m.move_index].intent,
                visited,
                m.commanded_mm_min,
                m.achieved_z_rate_mm_min,
                ratio,
            ));
        }
    }
    eprintln!(
        "ARM {arm} — plunges above 1x: {} visited by the modulator (the guard's own \
         population), {} skipped by INTENT (the guard never sees them)\n{}",
        residual_visited, residual_skipped, residual
    );

    eprintln!(
        "ARM {arm} — wanaka200 front rough (plunge_rate {plunge_rate:.0} mm/min, \
         commanded feed {commanded_feed:.0} mm/min)\n  \
         plunge.peak_ratio  {:?}\n  \
         plunge.over_1x     {}\n  \
         plunge.over_2x     {}\n  \
         plunge.population  {}\n  \
         worst z-rate       {:?} mm/min at move {:?}\n  \
         fed_time_s         {:.3}\n  \
         fed_moves          {}\n  \
         utilization        {:?}\n  \
         summary median dF  {:?} %  (ALL visited moves — capped plunges pull it down)\n  \
         lateral/ramp median dF {:?} %  over {} moves  (THE BAR)\n  \
         modulated feeds    {}\n  \
         per-move dump      {}",
        util.plunge.peak_ratio,
        util.plunge.over_1x,
        util.plunge.over_2x,
        util.plunge.population,
        util.plunge.worst_achieved_z_rate_mm_min,
        util.plunge.worst_move_index,
        util.fed_time_s,
        util.fed_moves,
        util.utilization,
        summary_median,
        lateral_median,
        lateral_population,
        modulated,
        path_out.display(),
    );

    // The bars. Arm A is expected to FAIL them — that is what makes it the
    // control; read its printed figures, not its verdict.
    assert!(
        util.plunge_is_measured(),
        "the front rough must carry plunge-class descents; population = {}",
        util.plunge.population
    );
    let peak = util
        .plunge
        .peak_ratio
        .expect("a measured plunge population");
    // THE BAR, scoped to what the guard governs. The plan wrote it as a
    // whole-population `peak_ratio <= 1.0`; the measurement says that bar
    // spans two owners, and only one of them is Phase 3's. See the doc
    // block: the residual is seven generator-emitted `EntryPlunge` moves
    // carrying the op's FEED rate, which `should_skip_modulation` claims
    // by ruling and the guard therefore never sees.
    assert_eq!(
        residual_visited, 0,
        "BAR: no plunge the modulator VISITED may exceed the operation's own plunge \
         rate. {residual_visited} of {} plunge-class moves are over 1x inside the \
         guard's own population (peak over the whole population reads {peak:.4}).",
        util.plunge.population
    );
    // The whole-population reading, reported against the pre-guard control
    // rather than against an absolute that a generator defect can hold
    // above 1.0. 1.85 was arm A; anything at or above it means the guard
    // stopped firing.
    assert!(
        peak < 1.5,
        "BAR: the whole-population peak must fall well below the pre-guard 1.848; \
         read {peak:.4}"
    );
}
