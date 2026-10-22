//! B5 — modulated feeds are current while the stock they read is current.
//!
//! `planning/op_architecture_2026-10-14/PLAN.md` backlog B5, seen live on
//! 2026-10-15: after `generate_all` every modulated operation reported
//! "feeds: modulated from simulation run N; the current simulation is run
//! M", although nothing it depends on had changed.
//!
//! # The defect
//!
//! `ProjectSession::feed_stage_of` compared the run that modulated a
//! toolpath with the LATEST run. A generation plan stores one prefix
//! simulation per rest operation, so the latest run moves past every
//! earlier modulation.
//!
//! # The rule
//!
//! The modulation of one toolpath reads its own moves and the stock the
//! simulation carved before it. The tag records that stock as a
//! `SourceStock`; the feeds are current while the record is current.
//!
//! # The claims
//!
//! 1. A plan walk with modulation on, then a closing simulation: every
//!    modulated toolpath reads current, also the ones a run before the
//!    latest one modulated. The header states the run, with no stale clause.
//! 2. An edit of the first operation, a regenerate of it, and a simulation
//!    that skips the last operation: the last operation reads superseded,
//!    and its header says which run is current.
//!
//! # The fixture
//!
//! The W1.5b chain (a 40 mm square, a 6 mm end mill, a 6 mm board): setup 1
//! = `Pocket` (fresh, 3 mm); setup 2 = `Rest 2` (4 mm); setup 3 = `Rest 3`
//! (5 mm) and `Finish` (fresh, 5 mm). `Finish` takes the fresh stock, so an
//! edit upstream keeps its result. A kinematics block makes the modulation
//! change feeds (F-036b AB2).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    AddSetupArgs, AddToolpathArgs, Command, FeedStage, ProjectSession, ReplaceToolpathConfigArgs,
    SetMachineKinematicsArgs, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
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

fn chain() -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_to(3.0),
    );
    let tool_id = session.tools()[0].id.raw();
    let model_id = session.models()[0].id;
    let mut setup_index = 0;
    for (name, depth, source) in [
        ("Rest 2", 4.0, StockSource::FromRemainingStock),
        ("Rest 3", 5.0, StockSource::FromRemainingStock),
        ("Finish", 5.0, StockSource::Fresh),
    ] {
        if name != "Finish" {
            setup_index = session
                .apply(Command::AddSetup(AddSetupArgs {
                    name: Some(format!("Setup of {name}")),
                    face_up: FaceUp::Top,
                }))
                .expect("the session accepts a setup")
                .created
                .expect("AddSetup reports the new setup index");
        }
        let mut config = toolpath_config(
            name,
            pocket_to(depth),
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id,
        );
        config.stock_source = source;
        let _ = session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index,
                config: Box::new(config),
            }))
            .expect("the setup accepts a toolpath");
    }
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let _ = session
        .apply(Command::SetMachineKinematics(SetMachineKinematicsArgs {
            kinematics: Box::new(MachineKinematics::shapeoko_xxl_stock()),
        }))
        .expect("a kinematics block");
    session
}

fn simulate(session: &mut ProjectSession, skip_ids: Vec<ToolpathId>) {
    let opts = SimulationOptions {
        resolution: session.simulation_resolution(),
        skip_ids,
        adaptive_feed_modulation: true,
        ..SimulationOptions::default()
    };
    let _ = session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("simulate");
}

/// `generate_all` the way the CLI walks it: every plan step once, then the
/// closing simulation of the whole project.
fn generate_all(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    for step in plan(session, Scope::Project) {
        match step {
            Step::Simulate { .. } => simulate(session, Vec::new()),
            Step::Generate { index, .. } => {
                let _ = session
                    .generate_toolpath(index, &cancel)
                    .expect("one walk generates every row");
            }
        }
    }
    simulate(session, Vec::new());
}

fn index_of(session: &ProjectSession, name: &str) -> usize {
    session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name == name)
        .expect("the fixture holds that row")
}

/// Claim 1: after `generate_all`, no modulated toolpath reads superseded.
#[test]
fn generate_all_leaves_every_modulated_toolpath_current_b5() {
    let mut session = chain();
    generate_all(&mut session);
    let latest = session.simulation_run().expect("the closing run is stored");
    assert!(latest >= 3, "two prefix runs and the closing run: {latest}");

    let mut modulated_runs = Vec::new();
    for index in 0..session.toolpath_configs().len() {
        let name = session.toolpath_configs()[index].name.clone();
        let stage = session.feed_stage(index).expect("every row is generated");
        assert!(
            !stage.is_superseded(),
            "'{name}' reads superseded after generate_all: {}",
            stage.statement()
        );
        if let FeedStage::ModulatedCurrent { simulation_run } = stage {
            assert_eq!(
                stage.statement(),
                format!("feeds: modulated from simulation run {simulation_run}"),
                "the header states the run and no stale clause"
            );
            modulated_runs.push(simulation_run);
        }
    }
    // Non-vacuity: the old rule (run == latest) fails on this state.
    assert!(
        modulated_runs.iter().any(|&run| run < latest),
        "a toolpath modulated before the latest run {latest} must be in the \
         fixture, or the claim measures nothing: {modulated_runs:?}"
    );
}

/// Claim 2: an upstream change makes the downstream modulation superseded.
#[test]
fn an_upstream_change_supersedes_the_downstream_modulation_b5() {
    let mut session = chain();
    generate_all(&mut session);
    let finish = index_of(&session, "Finish");
    let Some(FeedStage::ModulatedCurrent { simulation_run }) = session.feed_stage(finish) else {
        panic!(
            "precondition: 'Finish' is modulated and current: {:?}",
            session.feed_stage(finish)
        );
    };

    // Edit the first operation and regenerate it alone.
    // P3.2: the floor is not a parameter; the whole config carries it (the
    // GUI door).
    let pocket = index_of(&session, "Pocket");
    let mut edited = session.toolpath_configs()[pocket].clone();
    edited.operation = pocket_to(2.0);
    let _ = session
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: pocket,
            config: Box::new(edited),
        }))
        .expect("the pocket takes a floor");
    let _ = session
        .generate_toolpath(pocket, &AtomicBool::new(false))
        .expect("regenerate the pocket");
    assert!(
        session.result_at(finish).is_some(),
        "'Finish' takes the fresh stock, so the edit keeps its result"
    );

    // Simulate again, without 'Finish'.
    let finish_id = session.toolpath_configs()[finish].id;
    simulate(&mut session, vec![finish_id]);
    let current = session.simulation_run().expect("the partial run is stored");
    let stage = session.feed_stage(finish).expect("'Finish' holds a result");
    assert_eq!(
        stage,
        FeedStage::ModulatedSuperseded {
            simulation_run,
            current_run: Some(current),
        }
    );
    assert_eq!(
        stage.statement(),
        format!(
            "feeds: modulated from simulation run {simulation_run}; \
             the current simulation is run {current}"
        )
    );
}
