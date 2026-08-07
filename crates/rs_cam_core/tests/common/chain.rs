//! The F.4 generate/simulate fixpoint ladder, extracted from the now-archived
//! `p2c_headless_ab_wanaka.rs` and `v3_cascade_ab.rs` mega harnesses
//! (Checkpoint E, 2026-08-05, `planning/review_2026-08-04/MEGA_HARNESS_POLICY.md`
//! §2/§4; archived copies live at
//! `planning/archive/mega_harnesses_2026-08-05/`).
//!
//! **BIT-IDENTITY.** [`run_fixpoint_ladder`] is copied verbatim from its
//! donor, **`p2c_headless_ab_wanaka.rs::run_chain`** (pre-archive line
//! numbers `:107-147`) — `v3_cascade_ab.rs::run_chain` (`:799-839`) carried
//! a byte-for-byte identical copy of exactly this inner sub-range (verified
//! by direct diff, not just read-through). The two donors' `run_chain`
//! functions diverge immediately before this range (p2c resolves an
//! `enabled_finish_index`/`finish_name` up front for later `+=`
//! accumulation; v3 does not) and immediately after it (different
//! `ChainOutcome` shapes — p2c tracks `finish_total_s`, v3 tracks a
//! per-op `Vec<(String, f64)>` — plus a different `eprintln!` banner), so
//! only this ladder + final-simulation block was ever a true duplicate.
//! Those diverging halves are NOT extracted; they stay in the archived
//! files as-is.
//!
//! The one deviation from the donor: the trailing
//! `let sim = s.simulation_result().expect(...); let trace = sim.cut_trace...`
//! borrow-fetch lines are dropped. Returning those as borrowed references
//! from a `&mut ProjectSession` helper would fight the borrow checker for
//! no benefit — neither donor did anything with them before the point
//! this function now returns at, so callers re-fetch via
//! `s.simulation_result()` themselves.
//!
//! No parameters were introduced for cross-donor deltas: this particular
//! sub-range had zero differences between the two harnesses to begin
//! with, unlike `bandmap.rs::build_band_map` (which needed a
//! `tool_radius` parameter).

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Runs the F.4 generate → simulate fixpoint ladder over every currently
/// ENABLED toolpath in `s`: rest ops fail hard from fresh state by design
/// (pass 1), and each subsequent simulation round unlocks whichever
/// pending op(s) it can, until nothing is left pending. Finishes with one
/// more simulation using `adaptive_feed_modulation` at
/// `ModulationStrategy::ConstrainedMax` / aggressiveness `1.0` — the
/// setting both donor harnesses standardized their "final" chain
/// measurement on.
///
/// `label` is folded into every panic message so a caller running
/// multiple branches (A/B/C/D…) through this same helper can tell which
/// branch's ladder stalled.
///
/// After this returns, read `s.simulation_result()` /
/// `s.simulation_result().and_then(|r| r.cut_trace.as_ref())` for
/// whatever the caller needs — this mirrors what both donors did
/// immediately after the equivalent point in their own bodies.
///
/// # Panics
///
/// If the ladder does not converge within `enabled.len() + 2` rounds, or
/// if a round makes no progress (fewer ops pending than before) — both
/// exactly as the donor asserted.
pub fn run_fixpoint_ladder(label: &str, s: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let n = s.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();

    // Pass 1: rest ops fail hard from fresh state by design (F.4).
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }

    // F.4 ladder: each simulation unlocks the first pending rest op.
    let mut ladder_rounds = 0usize;
    while !pending.is_empty() {
        ladder_rounds += 1;
        assert!(
            ladder_rounds <= enabled.len() + 2,
            "[{label}] ladder failed to converge; still pending: {pending:?}"
        );
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder simulation");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(
            pending.len() < before,
            "[{label}] ladder made no progress at round {ladder_rounds}; still pending: {pending:?}"
        );
    }

    let final_opts = SimulationOptions {
        adaptive_feed_modulation: true,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
        ..Default::default()
    };
    s.run_simulation(&final_opts, &cancel)
        .expect("final simulation");
}

/// Re-simulate at measurement resolution (0.25 mm — the default 0.5 mm
/// dexel grid aliases away the terrain texture the P2.f band-fidelity
/// defect beheads). Call AFTER [`run_fixpoint_ladder`] so the ladder's own
/// timings/collisions still come from the standard chain options; this
/// sim exists only to populate `deviations` / `column_deviations` for a
/// subsequent fidelity read (see `bandmap.rs`).
///
/// Donor: `p2c_headless_ab_wanaka.rs::run_measurement_sim` (`:432-448`) —
/// byte-identical body to `v3_cascade_ab.rs::run_measurement_sim`
/// (`:1295-1303`); only the two doc comments had drifted (this one is a
/// fresh synthesis of both, not a copy of either verbatim).
pub fn run_measurement_sim(s: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.25,
        ..Default::default()
    };
    s.run_simulation(&opts, &cancel)
        .expect("hi-res measurement simulation");
}
