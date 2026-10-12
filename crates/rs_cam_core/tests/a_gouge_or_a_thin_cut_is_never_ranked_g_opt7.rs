//! **G-OPT7 — a gouge or a thin cut is never ranked.**
//!
//! Pre-registered in `planning/optimize_review_2026-10-12/PLAN.md` §3
//! (review finding F8). The 2026-05 search read a candidate for cycle
//! time, air fraction and the gates only, so a candidate that cut less
//! material, drove a rapid through the stock, or bit deeper through
//! standing material could rank first.
//!
//! The arms:
//!
//! 1. The three full-cut rules on hand-built readings
//!    (`FullCutCheck::judge`): the removed volume within one floor layer,
//!    no new rapid collision, and no over-cut peak beyond one cell.
//! 2. End to end: a candidate whose total depth is cut short removes
//!    less material, fails the full-cut check, and the outcome is not
//!    `Ranked` although the candidate is faster.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session_with, square_polygon, stock_under,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{Command, SetSimulationResolutionArgs, SimulationResolution};
use rs_cam_core::tool_load::optimize::{
    FullCutCheck, FullCutReadings, FullCutVerdict, OutcomeKind, SearchNotes, candidate_sim_options,
    measure_candidate, rank_candidates,
};

/// The dexel cell of the hand-built readings, in mm.
const CELL_MM: f64 = 0.2;
/// The total depth of the hand-built readings, in mm.
const DEPTH_MM: f64 = 22.0;

fn readings(volume: f64, collisions: usize, overcut: f64) -> FullCutReadings {
    FullCutReadings {
        removed_volume_mm3: Some(volume),
        rapid_collisions: Some(collisions),
        overcut_peak_mm: Some(overcut),
    }
}

fn baseline() -> FullCutCheck {
    FullCutCheck::reference(readings(22_000.0, 0, 0.6))
}

/// One floor layer is 22 000 x 0.2 / 22 = 200 mm³.
#[test]
fn a_thin_cut_fails_the_volume_rule() {
    let thin = FullCutCheck::judge(
        &baseline(),
        readings(21_799.0, 0, 0.6),
        CELL_MM,
        Some(DEPTH_MM),
    );
    assert!(
        matches!(thin.verdict, FullCutVerdict::Fails { .. }),
        "{thin:?}"
    );
    let within = FullCutCheck::judge(
        &baseline(),
        readings(21_801.0, 0, 0.6),
        CELL_MM,
        Some(DEPTH_MM),
    );
    assert!(within.verdict.holds(), "{within:?}");
}

#[test]
fn a_new_rapid_collision_fails() {
    let hit = FullCutCheck::judge(
        &baseline(),
        readings(22_000.0, 1, 0.6),
        CELL_MM,
        Some(DEPTH_MM),
    );
    assert!(
        matches!(hit.verdict, FullCutVerdict::Fails { .. }),
        "{hit:?}"
    );
}

/// The review's CLI sweep saw a 1.47 mm peak at dpp 1.5. Against a
/// 0.6 mm baseline peak and a 0.2 mm cell, that fails.
#[test]
fn a_deeper_bite_through_standing_material_fails() {
    let gouge = FullCutCheck::judge(
        &baseline(),
        readings(22_000.0, 0, 1.47),
        CELL_MM,
        Some(DEPTH_MM),
    );
    assert!(
        matches!(gouge.verdict, FullCutVerdict::Fails { .. }),
        "{gouge:?}"
    );
    let reason = gouge.verdict.reason().unwrap_or_default();
    assert!(reason.contains("1.47 mm"), "{reason}");
}

#[test]
fn a_short_total_depth_is_not_ranked() {
    let mut session = single_op_session_with(
        stock_under(20.0, 6.0),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(20.0)], "square"),
        "Pocket",
        OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth: 3.0,
            depth_per_pass: 1.5,
            ..PocketConfig::default()
        }),
        |_| {},
    );
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(0.5),
            },
        ))
        .expect("the resolution row refuses nothing");
    generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    let opts = candidate_sim_options(&session);
    let trace = session
        .run_simulation(&opts, &cancel)
        .expect("the fixture pocket simulates")
        .cut_trace
        .clone()
        .expect("the simulation keeps its cut trace");

    // The candidate cuts 1.5 mm of the 3 mm: a thin cut, and faster.
    let tc = session.toolpath_configs()[0].clone();
    let mut short = tc.operation.clone();
    let OperationConfig::Pocket(cfg) = &mut short else {
        panic!("the fixture is a pocket");
    };
    cfg.depth = 1.5;
    let (baseline, candidate) =
        measure_candidate(&mut session, &trace, 0, short, tc.dressups, &cancel)
            .expect("the short candidate simulates");
    assert!(
        candidate.cycle_time_s < baseline.cycle_time_s,
        "the arm needs a FASTER thin cut: {} against {}",
        candidate.cycle_time_s,
        baseline.cycle_time_s
    );
    assert!(
        matches!(candidate.full_cut.verdict, FullCutVerdict::Fails { .. }),
        "half the depth removes about half the volume: {:?} against {:?}",
        candidate.full_cut,
        baseline.full_cut
    );

    let outcome = rank_candidates(
        baseline,
        vec![candidate],
        SearchNotes {
            has_lever: true,
            ..SearchNotes::default()
        },
    );
    assert_ne!(outcome.kind, OutcomeKind::Ranked, "{:?}", outcome.narrative);
    assert!(
        outcome.narrative.explanation.contains("removed volume"),
        "the explanation names the short cut: {}",
        outcome.narrative.explanation
    );
}
