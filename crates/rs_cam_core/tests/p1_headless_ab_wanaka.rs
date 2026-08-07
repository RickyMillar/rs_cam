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
//! not exceed the pre-P1 baseline (4 on the full chain, 2026-07-07).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Baseline rapid-collision count measured on the pre-P1 chain
/// (P0 probe, 2026-07-07). New collisions above this fail the gate.
const BASELINE_RAPID_COLLISIONS: usize = 4;

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("airrun_2026-06-01")
        .join("wanaka.toml")
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
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
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
    assert!(
        sim.rapid_collisions.len() <= BASELINE_RAPID_COLLISIONS,
        "P1 SAFETY GATE FAILED: {} rapid collisions vs baseline {}",
        sim.rapid_collisions.len(),
        BASELINE_RAPID_COLLISIONS
    );
}
