//! G-PHANTOMSTAMP (2026-09-25) — Wanaka "3D Rough 6" rapids into no stock.
//!
//! ## The strike
//!
//! Wanaka airrun (`wanaka_airrun_2026-06-01_2c908dca.toml`), "3D Rough 6"
//! (id 10, Ø6 flat, AgentSearch, Global, plunge entry,
//! `min_region_cut_length_mm` 15): a rapid from Z 30 to 21.07 at
//! (89.640, 78.874), then a plunge feed, over a column that stood at the
//! level-2 floor 22.4. The rapid ended 1.33 mm inside stock at the 0.5,
//! 0.25 and 0.125 mm cells. The planner had stamped an entry that the F-038
//! coalescing pass then deleted, so its rapid floor read the phantom column
//! (`planning/entry_stock_awareness_2026-09-24/PHANTOMSTAMP_PLAN.md`).
//!
//! ## What this asserts
//!
//! The chain is built as `p1_headless_ab_wanaka` builds it (0.5 mm project
//! cell, generate, F.4 ladder). Then the chain up to "3D Rough 6" is
//! simulated at 0.25 mm and:
//!
//! 1. "3D Rough 6" has no rapid collision;
//! 2. every rapid that ends at the strike XY ends at or above the level-2
//!    floor 22.4 + the 0.5 mm descent buffer, before the entry feeds.
//!
//! Red before the fix (2026-09-26, release): moves 1405 / 1406 are rapids
//! to Z 30 and then Z 21.070 at the strike XY, below 22.9. After the fix no
//! rapid ends at the strike XY (the entry is no longer emitted there, so
//! check 2 holds vacuously) and check 1 carries the claim: 0 rapid
//! collisions in "3D Rough 6" at 0.25 mm (0 in the chain up to it).
//!
//! `#[ignore]`: the chain takes minutes (420 s in release). Run in release:
//!
//! ```text
//! cargo test --release -p rs_cam_core --test adaptive3d_wanaka_rough6_no_phantom_rapid_g_phantomstamp -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::toolpath::MoveType;

/// "3D Rough 6" in the pinned fixture.
const ROUGH6_ID: ToolpathId = ToolpathId(10);
/// The strike's entry XY (the rapid at move 1406 before the fix).
const STRIKE_XY: (f64, f64) = (89.640, 78.874);
/// The level-2 floor the column stood at, plus the emitter's rapid descent
/// buffer (`path.rs::segments_to_toolpath`, `RAPID_DESCENT_BUFFER_MM`).
const MIN_RAPID_END_Z: f64 = 22.4 + 0.5;
const XY_EPS: f64 = 1e-3;

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_airrun_2026-06-01_2c908dca.toml")
}

/// Load, generate and run the F.4 ladder exactly as `p1_headless_ab_wanaka`.
fn built_chain() -> ProjectSession {
    let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka");
    let cancel = AtomicBool::new(false);
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
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }
    let mut rounds = 0usize;
    while !pending.is_empty() {
        rounds += 1;
        assert!(rounds <= enabled.len() + 2, "ladder did not converge");
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder simulation");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(pending.len() < before, "ladder made no progress");
    }
    s
}

fn rough6_index(s: &ProjectSession) -> usize {
    (0..s.toolpath_count())
        .find(|&i| {
            s.get_toolpath_config(i)
                .is_some_and(|tc| tc.id == ROUGH6_ID)
        })
        .expect("3D Rough 6 in the fixture")
}

/// Rapid collisions inside "3D Rough 6" when the chain is simulated at
/// `cell_mm` (the ops after it are skipped; they cannot change its count).
fn rough6_rapid_collisions(s: &mut ProjectSession, cell_mm: f64) -> usize {
    let cancel = AtomicBool::new(false);
    let later: Vec<ToolpathId> = (rough6_index(s) + 1..s.toolpath_count())
        .filter_map(|i| s.get_toolpath_config(i).map(|tc| tc.id))
        .collect();
    let opts = SimulationOptions {
        resolution: cell_mm,
        skip_ids: later,
        adaptive_feed_modulation: false,
        ..Default::default()
    };
    s.run_simulation(&opts, &cancel).expect("simulation");
    let sim = s.simulation_result().expect("sim result");
    let b = sim
        .boundaries
        .iter()
        .find(|b| b.id == ROUGH6_ID)
        .expect("3D Rough 6 boundary");
    let hits: Vec<_> = sim
        .rapid_collisions
        .iter()
        .filter(|c| c.move_index >= b.start_move && c.move_index < b.end_move)
        .collect();
    for c in &hits {
        eprintln!(
            "cell {cell_mm}: 3D Rough 6 rapid_collision move={} start={:?} end={:?}",
            c.move_index - b.start_move,
            c.start,
            c.end
        );
    }
    eprintln!(
        "cell {cell_mm}: 3D Rough 6 rapid_collisions={} (chain {})",
        hits.len(),
        sim.rapid_collisions.len()
    );
    hits.len()
}

#[test]
#[ignore = "wanaka chain and two simulations (minutes); run with --release --ignored"]
fn wanaka_rough6_rapids_into_no_stock_g_phantomstamp() {
    let mut s = built_chain();

    // (2) The strike entry: every rapid ending at its XY stops at or above
    // the level-2 floor plus the descent buffer.
    let idx = rough6_index(&s);
    let tp = s.get_result(idx).expect("3D Rough 6 generated").toolpath();
    let at_strike: Vec<(usize, f64)> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            matches!(m.move_type, MoveType::Rapid)
                && (m.target.x - STRIKE_XY.0).abs() < XY_EPS
                && (m.target.y - STRIKE_XY.1).abs() < XY_EPS
        })
        .map(|(i, m)| (i, m.target.z))
        .collect();
    eprintln!("rapids ending at the strike XY: {at_strike:?}");
    for &(i, z) in &at_strike {
        assert!(
            z >= MIN_RAPID_END_Z - 1e-6,
            "move {i}: a rapid ends at Z {z:.3} at the strike XY, below the level-2 \
             floor + buffer {MIN_RAPID_END_Z:.3} (G-PHANTOMSTAMP)"
        );
    }

    // (1) No rapid of the op ends in stock.
    let hits = rough6_rapid_collisions(&mut s, 0.25);
    assert_eq!(
        hits, 0,
        "3D Rough 6 rapids into stock at 0.25 mm (G-PHANTOMSTAMP)"
    );
}
