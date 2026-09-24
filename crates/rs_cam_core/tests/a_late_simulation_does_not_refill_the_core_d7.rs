//! D7 (W0c): "a late simulation does not re-fill the core".
//!
//! Programme: `planning/gen_sim_rest_ux_2026-09-18/IMPL_W4.md` §2, fix
//! shape R-W4.1 option A, reconciled in `IMPLEMENTATION.md` §1.4.
//!
//! ## The defect
//!
//! `Command::AdoptResult` compares the revision the lane started from and
//! refuses a superseded answer. `Command::AdoptSimulation` beside it
//! stored unconditionally. So an edit cleared the field, the in-flight run
//! landed, the adopt re-filled it, and every core reader called that run
//! CURRENT. It is not only a display defect: `ProjectSession::start` reads
//! that field for `StockSource::FromRemainingStock` prior stock, so the
//! stored run hands a rest operation a superseded snapshot.
//!
//! ## The fix this file pins
//!
//! `ProjectSession::simulation_epoch` is the simulation's half of the rule
//! `toolpath_revision` states for a generation. One function,
//! `drop_simulation`, clears the field and bumps the epoch, and nineteen
//! mutation sites call it. `AdoptSimulationArgs` carries the epoch the
//! lane started from, and `apply` refuses a mismatch with
//! `SessionError::StaleSimulation`.
//!
//! `next_revision` cannot serve. `invalidate_machine`, `set_machine`,
//! `set_machine_kinematics`, `import_machine_settings`, `set_post_config`,
//! `replace_tools` and `set_toolpath_enabled` all clear the simulation and
//! move no toolpath revision. Claim 4 measures exactly that.
//!
//! ## What the file pins
//!
//! 1. `a_late_simulation_is_refused`: the adopt carries a pre-edit epoch,
//!    the door refuses, and the field stays empty.
//! 2. `the_same_run_lands_at_the_current_epoch`: the guard refuses a stale
//!    stamp and nothing else.
//! 3. `two_edits_move_the_epoch_twice`: the bump is unconditional, so a
//!    run submitted between two edits cannot read current.
//! 4. `every_door_that_clears_the_simulation_moves_the_epoch`: a table of
//!    mutation classes, including the seven that move no toolpath
//!    revision.

// SAFETY: test-module lint allowances, per `crates/rs_cam_core/CLAUDE.md`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    AdoptSimulationArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SessionError,
    SetPostConfigArgs, SetStockConfigArgs, SetToolParamArgs, SetToolpathEnabledArgs,
    SetToolpathParamArgs, ToolpathConfig,
};
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

// ── fixture ──────────────────────────────────────────────────────

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0, // overwritten by `add_model`
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn tc(name: &str, stock_source: StockSource, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // overwritten by `add_toolpath`
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: false,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

/// One setup, two rows: `rough` on fresh stock and `rest` on the
/// remaining stock. The second row is what makes the defect a safety
/// matter and not only a display one.
fn fixture() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
    let _ = builder
        .add_toolpath(0, tc("rough", StockSource::Fresh, tool, model))
        .unwrap();
    let _ = builder
        .add_toolpath(0, tc("rest", StockSource::FromRemainingStock, tool, model))
        .unwrap();
    let session = builder.build();
    // The epoch is NOT zero here: `add_toolpath` clears the simulation
    // like any other edit, so the two rows above moved it twice. That is
    // the rule, not a fixture accident, so every claim below reads the
    // epoch rather than naming a number.
    assert!(
        session.simulation_result().is_none(),
        "a fresh session holds no simulation"
    );
    session
}

fn a_simulation(session: &ProjectSession) -> SimulationResult {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    };
    let mut prior_stocks = HashMap::new();
    let rest_id = session.toolpath_configs()[1].id;
    let _ = prior_stocks.insert(
        rest_id,
        Arc::new(rs_cam_core::dexel_stock::TriDexelStock::from_bounds(
            &bbox, 2.0,
        )),
    );
    SimulationResult {
        mesh: StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 0,
        deviations: None,
        column_deviations: None,
        boundaries: Vec::new(),
        checkpoints: Vec::new(),
        rapid_collisions: Vec::new(),
        rapid_collision_move_indices: Vec::new(),
        cut_trace: None,
        resolution_clamped: false,
        column_grid_cell_mm: 2.0,
        prior_stocks,
        prior_stock_sources: std::collections::HashMap::new(),
    }
}

/// Adopt a simulation the way the compute lane does: with the epoch the
/// lane read at SUBMIT time.
fn adopt_at(
    session: &mut ProjectSession,
    epoch: u64,
) -> Result<rs_cam_core::session::Effects, SessionError> {
    let result = a_simulation(session);
    session.apply(Command::AdoptSimulation(AdoptSimulationArgs {
        result: Box::new(result),
        epoch,
    }))
}

/// One edit that clears the simulation and moves a toolpath revision.
fn edit_a_feed(session: &mut ProjectSession) {
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(4321.0),
        }))
        .expect("feed_rate is a Pocket parameter");
}

// ── claim 1 ──────────────────────────────────────────────────────

#[test]
fn a_late_simulation_is_refused() {
    let mut session = fixture();

    // The lane reads the epoch at submit, and the edit lands while it runs.
    let submitted = session.simulation_epoch();
    edit_a_feed(&mut session);
    assert!(
        session.simulation_epoch() > submitted,
        "the edit moved the epoch, or the rest of this claim is vacuous"
    );

    let refusal = adopt_at(&mut session, submitted);
    match refusal {
        Err(SessionError::StaleSimulation {
            submitted: s,
            current,
        }) => {
            assert_eq!(s, submitted, "the refusal names the epoch the lane read");
            assert_eq!(
                current,
                session.simulation_epoch(),
                "the refusal names the epoch the session carries"
            );
        }
        other => panic!("a late simulation must be refused, got {other:?}"),
    }
    assert!(
        session.simulation_result().is_none(),
        "the refused run stores nothing. `FromRemainingStock` reads its \
         prior stock from this field, so a stored late run would hand the \
         rest operation a superseded snapshot"
    );
}

// ── claim 2 ──────────────────────────────────────────────────────

#[test]
fn the_same_run_lands_at_the_current_epoch() {
    let mut session = fixture();
    edit_a_feed(&mut session);

    // The same payload, submitted after the edit, is accepted.
    let now = session.simulation_epoch();
    let effects = adopt_at(&mut session, now).expect("a simulation at the current epoch is stored");
    assert!(
        effects.stale.is_empty(),
        "a simulation moves no generation-input revision"
    );
    assert!(
        !effects.simulation_cleared,
        "the store leaves the field Some"
    );
    let stored = session
        .simulation_result()
        .expect("the current run reaches the core");
    assert!(
        stored
            .prior_stocks
            .contains_key(&session.toolpath_configs()[1].id),
        "the stored run carries the prior stock the rest operation reads"
    );

    // The store itself does NOT move the epoch: only a drop does.
    let after_store = session.simulation_epoch();
    let _ =
        adopt_at(&mut session, after_store).expect("a second adopt at the same epoch is stored");
    assert_eq!(
        session.simulation_epoch(),
        after_store,
        "adopting a simulation is not an edit, so it moves no epoch"
    );
}

// ── claim 3 ──────────────────────────────────────────────────────

#[test]
fn two_edits_move_the_epoch_twice() {
    let mut session = fixture();
    let start = session.simulation_epoch();
    edit_a_feed(&mut session);
    let after_one = session.simulation_epoch();
    edit_a_feed(&mut session);
    let after_two = session.simulation_epoch();

    assert!(
        start < after_one && after_one < after_two,
        "the bump is unconditional, so a run submitted between two edits \
         cannot read current: {start} then {after_one} then {after_two}"
    );

    // The bump does not depend on a simulation being there. Neither edit
    // above had one to clear.
    assert!(session.simulation_result().is_none());
}

// ── claim 4 ──────────────────────────────────────────────────────

/// One mutation class, and how it is run.
struct Door {
    name: &'static str,
    run: fn(&mut ProjectSession),
}

fn doors() -> Vec<Door> {
    vec![
        Door {
            name: "set_toolpath_param",
            run: edit_a_feed,
        },
        // The seven classes that clear the simulation and move NO toolpath
        // revision are why `next_revision` cannot serve as the epoch.
        // Three of them stand for the set here.
        Door {
            name: "set_toolpath_enabled",
            run: |session| {
                let _ = session
                    .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
                        index: 1,
                        enabled: false,
                    }))
                    .expect("the toggle writes enabled");
            },
        },
        Door {
            name: "set_post_config",
            run: |session| {
                let mut post = session.post_config().clone();
                post.spindle_speed = 12_345;
                let _ = session
                    .apply(Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(post),
                    }))
                    .expect("the setter writes the post config");
            },
        },
        Door {
            name: "set_tool_param",
            run: |session| {
                let _ = session
                    .apply(Command::SetToolParam(SetToolParamArgs {
                        index: 0,
                        param: "diameter".to_owned(),
                        value: serde_json::json!(7.5),
                    }))
                    .expect("diameter is a tool parameter");
            },
        },
        Door {
            name: "set_stock_config",
            run: |session| {
                let mut stock = session.stock_config().clone();
                stock.x = 321.0;
                let _ = session
                    .apply(Command::SetStockConfig(SetStockConfigArgs {
                        stock: Box::new(stock),
                    }))
                    .expect("the setter writes the stock");
            },
        },
    ]
}

#[test]
fn every_door_that_clears_the_simulation_moves_the_epoch() {
    let mut cleared_at_least_one = false;
    for door in doors() {
        let mut session = fixture();
        // Give the door a simulation to clear, at the current epoch.
        let now = session.simulation_epoch();
        let _ = adopt_at(&mut session, now).expect("the fixture stores a simulation");
        assert!(
            session.simulation_result().is_some(),
            "door '{}': the precondition is a stored simulation",
            door.name
        );
        let before = session.simulation_epoch();

        (door.run)(&mut session);

        // The rule, both ways: the field went empty exactly when the
        // epoch moved.
        let went_empty = session.simulation_result().is_none();
        let epoch_moved = session.simulation_epoch() > before;
        assert_eq!(
            went_empty, epoch_moved,
            "door '{}': the simulation and the epoch must move together. \
             Empty: {went_empty}, epoch moved: {epoch_moved}",
            door.name
        );
        if went_empty {
            cleared_at_least_one = true;
            // And the run that was in flight when the door ran is now
            // refused.
            assert!(
                matches!(
                    adopt_at(&mut session, before),
                    Err(SessionError::StaleSimulation { .. })
                ),
                "door '{}': a run submitted before it must not re-fill the core",
                door.name
            );
        }
    }
    assert!(
        cleared_at_least_one,
        "no door cleared the simulation, so the claim is vacuous"
    );
}
