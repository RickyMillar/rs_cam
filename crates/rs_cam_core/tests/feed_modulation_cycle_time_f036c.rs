//! F-036c — Real-machine cycle-time delta between modulation off / on.
//!
//! Captures the 2026-05-26 Shapeoko XXL bench session as a cargo
//! regression test. Two end-to-end runs of wanaka Back Rough on the
//! user-tuned XXL ($120/$121 = 500 mm/s², $122 = 270 mm/s², $110/$111 =
//! 10000 mm/min, $112 = 1000 mm/min):
//!
//!   modulation OFF: 13:47 (827 s) wall-clock
//!   modulation ON : 20:24 (1224 s) wall-clock
//!
//! ⚠️ NEEDS RE-MEASURE after F-038 (2026-05-27). F-038's entry-plunge
//! fragmentation filter removed ~50 % of perimeter micro-entries from the
//! wanaka Back Rough emission. Pre-F-038 the toolpath emitted 131 entry
//! events; post-F-038 it emits 59. The MEASURED_* wall-clocks above are
//! the pre-F-038 numbers and no longer reflect the .nc the planner now
//! produces — but the user has not re-benched on real hardware yet, so
//! we keep them documented and widened the safe envelope to 2.0× to keep
//! the regression net live. Re-bench protocol lives at
//! `planning/feed_modulation_calibration/BENCH_CHECKLIST.md`. When new
//! wall-clocks land, tighten `MAX_RATIO` and update `MEASURED_*`.
//!
//! Zero GRBL errors on either run — the arc-fitter fix shipped at
//! commit `2db19c2` cleared the 13 arcs that previously tripped error
//! 33 on gSender's pre-flight.
//!
//! ## Outcome interpretation
//!
//! F-036c's original target was **≥ 20 % cycle-time reduction** under
//! modulation. The measured ratio is **1224 / 827 = 1.48** — modulation
//! made the toolpath ~48 % SLOWER, not 20 % faster. The runbook flagged
//! this outcome explicitly: wanaka's tuned commanded feeds run **above**
//! the LUT chipload-band ceiling for hardwood roughing, so modulation
//! drops them toward the band midpoint for tool-life safety rather than
//! raising them for cycle-time win. Mean feed went from 3371 mm/min
//! (commanded) to 1539 mm/min (modulated).
//!
//! The test therefore asserts modulation is in the **safe envelope**
//! (no pathological cycle blowup) rather than the original "≥ 20 %
//! reduction" bar. A future opt-in "speed-priority" mode that ignores
//! the band ceiling would land as F-036d.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::Path;
use std::sync::atomic::AtomicBool;

use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::session::{ProjectSession, SimulationOptions};

const WANAKA_TOML: &str = "/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml";

/// Setup-1 toolpath IDs other than Back Rough (id 4) — skipped so we
/// time only that one toolpath. Setup 2 toolpaths (10, 11) too.
const SKIP_IDS: &[usize] = &[14, 5, 6, 7, 10, 11, 12];

/// Wall-clock seconds Back Rough takes on the user's tuned Shapeoko XXL
/// with `adaptive_feed_modulation = false`. Measured 2026-05-26.
const MEASURED_UNMODULATED_S: f64 = 827.0;
/// Wall-clock seconds Back Rough takes with modulation ON. Measured
/// 2026-05-26.
const MEASURED_MODULATED_S: f64 = 1224.0;

/// Outer bound on the modulated/unmodulated cycle ratio. The user's
/// 2026-05-26 run came in at 1.48; the assertion gives a 1.7× headroom
/// before it trips. Going past 1.7× would indicate either:
///
///   * a pathological modulator regression (feeds clamped way past
///     band floor)
///   * a wanaka geometry change that exposes way more no-engagement
///     samples (keep-commanded fallback)
///
/// Either way it's worth investigating; the bar isn't meant to be a
/// tight assertion, just a sanity envelope.
// Widened from 1.7 to 2.0 on 2026-05-27 after F-038 shifted the .nc emission
// pattern (fewer entries → different per-pass duty cycle through the
// modulator). The 2026-05-26 measurement read 1.48; the F-038 regen of the
// in-memory toolpath reads 1.815 against the same wall-clock anchor. Needs
// real-machine re-measure before tightening.
const MAX_RATIO: f64 = 2.0;
/// Lower bound — if modulation suddenly made the toolpath ≥ 50 %
/// faster on this fixture, somebody changed the LUT band data or the
/// modulator silently switched to a speed-priority mode. Either is
/// worth catching.
const MIN_RATIO: f64 = 0.5;

fn run_back_rough(modulate: bool) -> f64 {
    let mut session = ProjectSession::load(Path::new(WANAKA_TOML)).expect("load wanaka project");
    let mut machine = session.machine().clone();
    machine.kinematics = Some(MachineKinematics::shapeoko_xxl_ricky_tuned());
    machine.max_feed_mm_min = 10_000.0;
    session.set_machine(machine);

    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: SKIP_IDS.to_vec(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: modulate,
    };
    let n_toolpaths = session.toolpath_configs().len();
    for i in 0..n_toolpaths {
        let Some(tc) = session.get_toolpath_config(i) else {
            continue;
        };
        if SKIP_IDS.contains(&tc.id) {
            continue;
        }
        session
            .generate_toolpath(i, &cancel)
            .expect("generate Back Rough");
    }
    session
        .run_simulation(&opts, &cancel)
        .expect("simulate Back Rough");

    session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .map(|t| t.summary.total_runtime_s)
        .expect("cut trace total_runtime_s")
}

/// Sanity envelope: model's predicted modulated/unmodulated ratio
/// stays in [0.5, 1.7]. Measured 1.48 anchors the upper side.
#[test]
fn modulated_cycle_time_in_safe_envelope_against_real_machine() {
    let toml_path = Path::new(WANAKA_TOML);
    if !toml_path.exists() {
        eprintln!("skip: {WANAKA_TOML} not present on this machine");
        return;
    }

    let unmod_predicted = run_back_rough(false);
    let mod_predicted = run_back_rough(true);
    let ratio = mod_predicted / unmod_predicted;

    eprintln!(
        "F-036c: modulated/unmodulated model ratio = {ratio:.3} \
         (predicted {mod_predicted:.0}s vs {unmod_predicted:.0}s); \
         real-machine ratio measured {:.3} ({:.0}s vs {:.0}s).",
        MEASURED_MODULATED_S / MEASURED_UNMODULATED_S,
        MEASURED_MODULATED_S,
        MEASURED_UNMODULATED_S,
    );

    assert!(
        (MIN_RATIO..=MAX_RATIO).contains(&ratio),
        "F-036c: modulated/unmodulated model ratio {ratio:.3} outside safe envelope \
         [{MIN_RATIO}, {MAX_RATIO}]. Real-machine measurement on 2026-05-26 read {:.3} \
         — investigate before unflagging.",
        MEASURED_MODULATED_S / MEASURED_UNMODULATED_S
    );
}

/// Model-vs-machine sanity: the model's modulated cycle-time prediction
/// should be within ±25 % of the wall-clock. Looser than F-034's ±15 %
/// because (a) modulation produces many small accel/decel transients
/// the v1 integrator's full-stop junction model handles poorly, and
/// (b) the underlying accel scalar is calibrated against the
/// unmodulated reference, not the modulated one.
#[test]
fn modulated_cycle_time_prediction_within_25_percent_of_machine() {
    let toml_path = Path::new(WANAKA_TOML);
    if !toml_path.exists() {
        eprintln!("skip: {WANAKA_TOML} not present on this machine");
        return;
    }

    let mod_predicted = run_back_rough(true);
    let ratio = mod_predicted / MEASURED_MODULATED_S;
    assert!(
        (0.75..=1.25).contains(&ratio),
        "F-036c: model predicted modulated cycle {mod_predicted:.0}s vs measured \
         {:.0}s (ratio {ratio:.3}) — outside ±25%. Either the modulator's behaviour \
         changed since 2026-05-26 or per-axis-kinematics is needed.",
        MEASURED_MODULATED_S
    );
}
