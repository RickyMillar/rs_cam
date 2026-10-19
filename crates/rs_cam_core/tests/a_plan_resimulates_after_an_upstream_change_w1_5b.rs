//! W1.5b — the plan simulates before a rest operation whose upstream gains a
//! result inside the same plan.
//!
//! `planning/op_architecture_2026-10-14/PLAN.md` row W1.5b, seen live on
//! 2026-10-14: `generate_all` on a 3-setup project refused the rest
//! operation first in setup 3 ("awaiting prior stock"). A second
//! `generate_all` then finished.
//!
//! # The defect
//!
//! Rule (3) of `ProjectSession::source_is_current` skips an enabled row that
//! holds no result. So a snapshot taken while an upstream row had no result
//! reads current. `generation_plan::plan` read that answer once, when it
//! built the plan, and put no Simulate step before the rest operation. The
//! upstream row then generated inside the plan, the snapshot went stale, and
//! the rest operation refused.
//!
//! # The claims
//!
//! 1. Cold (no simulation): one Simulate step before each rest operation.
//!    The old rule gave this too; the arm guards the count.
//! 2. A simulation of the cold project gives both rest operations a
//!    snapshot that reads current. The plan still simulates before each of
//!    them, and one walk of the plan generates every operation.
//! 3. After that walk the plan holds no Simulate step: a simulation runs
//!    only when an upstream row changes.
//!
//! # Red before the fix
//!
//! With the old rule (`has_snapshot` alone), claim 2 failed: the plan held
//! no Simulate step, and the walk refused `Rest 2` ("no current simulated
//! remaining-stock snapshot is available: 'Pocket' changed after the stock
//! was simulated").
//!
//! # The fixture
//!
//! A 40 mm square polygon, a 6 mm end mill, a 6 mm board below Z0. Setup 1
//! = a fresh pocket; setup 2 and setup 3 = one rest pocket each, every one
//! 1 mm deeper than the one before (G-RESTLEVELS), all face up `Top`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::ids::{SetupId, ToolpathId};
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    AddSetupArgs, AddToolpathArgs, Command, ProjectSession, SetSimulationResolutionArgs,
    SimulationOptions, SimulationResolution,
};

mod common;
use common::session::{
    polygon_model, single_op_session, square_polygon, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;

const HALF: f64 = 20.0;
const STOCK_Z: f64 = 6.0;
const TOOL_D: f64 = 6.0;
const CELL_MM: f64 = 1.0;

fn pocket_to(depth: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(depth),
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// Setup 1 = `Pocket` (fresh, 3 mm); setup 2 = `Rest 2` (4 mm); setup 3 =
/// `Rest 3` (5 mm). No row holds a result.
fn three_setups() -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_to(3.0),
    );
    let tool_id = session.tools()[0].id.0;
    let model_id = session.models()[0].id;
    for (name, depth) in [("Rest 2", 4.0), ("Rest 3", 5.0)] {
        let setup_index = session
            .apply(Command::AddSetup(AddSetupArgs {
                name: Some(format!("Setup of {name}")),
                face_up: FaceUp::Top,
            }))
            .expect("the session accepts a setup")
            .created
            .expect("AddSetup reports the new setup index");
        let mut rest = toolpath_config(name, pocket_to(depth), tool_id, model_id);
        rest.stock_source = StockSource::FromRemainingStock;
        let _ = session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index,
                config: Box::new(rest),
            }))
            .expect("the new setup accepts a toolpath");
    }
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    assert_eq!(session.list_setups().len(), 3, "the fixture holds 3 setups");
    assert!(
        (0..3).all(|index| session.get_result(index).is_none()),
        "the fixture starts with no result"
    );
    session
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    let _ = session.run_simulation(&opts, &cancel).expect("simulate");
}

/// The id and the index of the row named `name`.
fn row(session: &ProjectSession, name: &str) -> (ToolpathId, usize) {
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name == name)
        .expect("the fixture holds that row");
    (session.toolpath_configs()[index].id, index)
}

fn setup_of(session: &ProjectSession, position: usize) -> SetupId {
    SetupId(session.list_setups()[position].id)
}

/// `[Generate(Pocket), Simulate(Rest 2), Generate(Rest 2), Simulate(Rest 3),
/// Generate(Rest 3)]`.
fn expected_plan(session: &ProjectSession) -> Vec<Step> {
    let generate = |name: &str| {
        let (toolpath, index) = row(session, name);
        Step::Generate { toolpath, index }
    };
    let simulate = |position: usize, name: &str| Step::Simulate {
        setup: setup_of(session, position),
        upto: row(session, name).0,
    };
    vec![
        generate("Pocket"),
        simulate(1, "Rest 2"),
        generate("Rest 2"),
        simulate(2, "Rest 3"),
        generate("Rest 3"),
    ]
}

/// Walk the plan the way the CLI does: one call, every step once. Answers
/// the number of simulations it ran.
fn walk(session: &mut ProjectSession) -> usize {
    let cancel = AtomicBool::new(false);
    let mut simulations = 0;
    for step in plan(session, Scope::Project) {
        match step {
            Step::Simulate { .. } => {
                simulations += 1;
                simulate(session);
            }
            Step::Generate { index, .. } => {
                let name = session.toolpath_configs()[index].name.clone();
                session
                    .generate_toolpath(index, &cancel)
                    .unwrap_or_else(|e| panic!("one walk must generate '{name}': {e}"));
            }
        }
    }
    simulations
}

fn simulate_steps(session: &ProjectSession) -> usize {
    plan(session, Scope::Project)
        .iter()
        .filter(|step| matches!(step, Step::Simulate { .. }))
        .count()
}

/// Claim 1: the cold plan simulates once before each rest operation.
#[test]
fn the_cold_plan_simulates_before_each_rest_op_w1_5b() {
    let mut session = three_setups();
    assert_eq!(plan(&session, Scope::Project), expected_plan(&session));
    assert_eq!(walk(&mut session), 2);
    assert!((0..3).all(|index| session.get_result(index).is_some()));
}

/// Claims 2 and 3: a snapshot that reads current only because an upstream
/// row holds no result does not remove the Simulate step.
#[test]
fn a_snapshot_taken_before_the_upstream_generates_is_not_current_for_the_plan_w1_5b() {
    let mut session = three_setups();
    simulate(&mut session);
    let (rest_2, _) = row(&session, "Rest 2");
    let (rest_3, _) = row(&session, "Rest 3");
    // Precondition: the defect's shape. Both snapshots read current, because
    // rule (3) skips the rows with no result.
    assert!(
        session.snapshot_is_current(rest_2).is_ok() && session.snapshot_is_current(rest_3).is_ok(),
        "precondition: both rest snapshots read current while no row holds a result"
    );

    assert_eq!(
        plan(&session, Scope::Project),
        expected_plan(&session),
        "each rest operation's upstream generates inside the plan, so the plan \
         simulates before each of them"
    );
    assert_eq!(
        walk(&mut session),
        2,
        "one Simulate step per rest operation"
    );
    for index in 0..3 {
        assert!(
            session.get_result(index).is_some(),
            "one walk generates row {index}"
        );
    }

    // Claim 3: nothing upstream changes now, so no Simulate step.
    assert_eq!(
        simulate_steps(&session),
        0,
        "a current project plans no simulation"
    );
}
