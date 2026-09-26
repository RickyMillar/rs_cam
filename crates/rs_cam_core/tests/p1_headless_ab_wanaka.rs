//! P1 unified-finishing headless A/B harness (2026-07-07).
//!
//! Rebuilds the full wanaka chain in-process — fresh load → generate →
//! F.4 ladder (sim → regenerate first errored rest op → sim → …) — then
//! prints every toolpath's F-034 `runtime_by_intent` decomposition and
//! the rapid-collision count, so the P1 linker/descent work can be
//! A/B'd against the P0 probe baseline
//! (`planning/unified_finishing_pass_plan.md`, "P0 results log") without
//! a live GUI session.
//!
//! `#[ignore]` — this runs the real dexel simulation over the full
//! project several times (minutes). Run with:
//! `cargo test -p rs_cam_core --test p1_headless_ab_wanaka -- --ignored --nocapture`
//!
//! The one hard assertion is the P1 safety gate: rapid collisions must
//! not exceed the ceiling (4 on the pre-P1 chain 2026-07-07; 0 since
//! G-PHANTOMSTAMP, 2026-09-26).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Ceiling on the chain's rapid collisions. 4 on the pre-P1 chain (P0
/// probe, 2026-07-07); lowered to the measured 0 after G-PECKSPLIT (Back
/// Rough) and G-PHANTOMSTAMP (3D Rough 6) removed the last two
/// (2026-09-26, release, 0.5 mm). New collisions above this fail the gate.
const BASELINE_RAPID_COLLISIONS: usize = 0;

/// The pinned wanaka play-file this A/B measures against.
///
/// FIN-06: the harness read `planning/airrun_2026-06-01/wanaka.toml`, a
/// document the operator edits between machining sessions. `planning/` is
/// evidence, not a fixture store. The fixture here is a BYTE-IDENTICAL copy
/// of that file (sha256 prefix `2c908dca`), so the baseline below still
/// measures what it was measured on.
///
/// It is NOT `wanaka_2026-08-16_f530995a.toml`, the other wanaka fixture:
/// that snapshot disables two toolpaths, and this test counts collisions
/// over the whole enabled chain.
fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_airrun_2026-06-01_2c908dca.toml")
}

#[test]
#[ignore = "full-project dexel simulation ladder; run with --ignored --nocapture"]
fn p1_headless_ab_full_chain_intent_decomposition() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "wanaka.toml not found at {} — harness requires the canonical project",
        path.display()
    );
    let mut s = ProjectSession::load(&path).expect("load wanaka.toml");
    let cancel = AtomicBool::new(false);

    // Every simulation below runs at `SimulationOptions::default()`'s cell,
    // and the P0 baseline was measured there. The project stores 0.1 mm, and
    // since G-RESTRES a rest op refuses a snapshot at another cell, so the
    // ladder unlocked nothing. Store the harness's cell as the project's.
    let cell_mm = SimulationOptions::default().resolution;
    let _ = s
        .apply(rs_cam_core::session::Command::SetSimulationResolution(
            rs_cam_core::session::SetSimulationResolutionArgs {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(cell_mm),
            },
        ))
        .expect("a positive cell");

    let n = s.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();

    // Pass 1: generate everything that can generate from fresh state.
    // Rest ops fail hard here by design (F.4) — collected for the ladder.
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }

    // F.4 ladder: each simulation unlocks exactly the first pending
    // rest op in its group; regenerate it and re-simulate.
    let mut ladder_rounds = 0usize;
    while !pending.is_empty() {
        ladder_rounds += 1;
        assert!(
            ladder_rounds <= enabled.len() + 2,
            "ladder failed to converge; still pending: {pending:?}"
        );
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder simulation");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(
            pending.len() < before,
            "ladder made no progress at round {ladder_rounds}; still pending: {pending:?}"
        );
    }

    // Final simulation over the complete chain — with the GUI's
    // modulation options so the trace is directly comparable to the
    // live P0 baseline (which the GUI produced with modulation on).
    let final_opts = SimulationOptions {
        adaptive_feed_modulation: true,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
        ..Default::default()
    };
    s.run_simulation(&final_opts, &cancel)
        .expect("final simulation");
    let sim = s.simulation_result().expect("sim result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");

    eprintln!("== P1 headless A/B — wanaka full chain ==");
    eprintln!(
        "rapid_collisions={} (baseline {BASELINE_RAPID_COLLISIONS})",
        sim.rapid_collisions.len()
    );
    // Name each strike, so a count that holds still cannot hide a swap
    // (G-PECKSPLIT: Back Rough's was move 3448 at (111.000, 30.732)).
    for c in &sim.rapid_collisions {
        eprintln!(
            "rapid_collision move={} start={:?} end={:?}",
            c.move_index, c.start, c.end
        );
    }
    for tp in &trace.toolpath_summaries {
        let name = (0..n)
            .filter_map(|i| s.get_toolpath_config(i))
            .find(|tc| tc.id == tp.toolpath_id)
            .map(|tc| tc.name.clone())
            .unwrap_or_else(|| format!("{:?}", tp.toolpath_id));
        match tp.runtime_by_intent {
            Some(b) => eprintln!(
                "op={name:<22} total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1}",
                tp.total_runtime_s,
                b.cutting_s,
                b.entry_s,
                b.linking_s,
                b.rapid_s,
                b.retract_s,
                b.unknown_s
            ),
            None => eprintln!(
                "op={name:<22} total={:8.1}s (no runtime_by_intent — kinematics off?)",
                tp.total_runtime_s
            ),
        }
    }
    if let Some(p) = trace.summary.runtime_by_intent {
        eprintln!(
            "PROJECT total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1}",
            trace.summary.total_runtime_s,
            p.cutting_s,
            p.entry_s,
            p.linking_s,
            p.rapid_s,
            p.retract_s,
            p.unknown_s
        );
    }

    // P1 safety gate: the descent/link work must not add collisions.
    // (`saturating_sub`, not `<=`: at a ceiling of 0 clippy reads `<=` as
    // an absurd comparison.)
    assert!(
        sim.rapid_collisions
            .len()
            .saturating_sub(BASELINE_RAPID_COLLISIONS)
            == 0,
        "P1 SAFETY GATE FAILED: {} rapid collisions vs baseline {}",
        sim.rapid_collisions.len(),
        BASELINE_RAPID_COLLISIONS
    );
}
