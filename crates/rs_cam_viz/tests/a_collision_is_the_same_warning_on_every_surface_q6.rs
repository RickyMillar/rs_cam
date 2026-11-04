//! Q6 (feeds contract P8, the agreed mock `phases/P8_Q6_MOCK.md`): a rapid
//! collision is the same K6 warning on every surface.
//!
//! The K6 project (`rs_cam_core/tests/common/k6_project.rs`) gets a stored
//! simulation with rapid collisions on two toolpaths: two moves on
//! `Inverted`, one move on `Gouge`. Core `export_warnings` files one
//! `project.rapid_collision` row per toolpath. The test reads the rows on
//! each surface:
//!
//! - the CLI door: `ProjectSession::export_gcode_checked`. The CLI
//!   `summary.json` `warnings` are the `ExportWarning::header_line` of
//!   these rows (`rs_cam_cli` `report_export_checks`). The CLI is a binary
//!   crate, so this test reads the core rows and applies the same map.
//! - the GUI export door and the MCP `export_gcode` reply text.
//! - the GUI pre-flight: `preflight_warnings`, the rapid-collision card
//!   (`readiness::rapid_collision_check`, `rapid_collision_detail`) and the
//!   Export button (no refusal, the Danger style).
//!
//! The card counts toolpaths, not moves (operator, 2026-11-05). Before Q6
//! the card read the GUI move list (`sim.checks.rapid_collisions`): it gave
//! "3 detected", and it passed when the GUI list was empty.
//!
//! `cargo test -p rs_cam_viz -q --test a_collision_is_the_same_warning_on_every_surface_q6`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "../../rs_cam_core/tests/common/k6_project.rs"]
mod k6_project;

use std::collections::HashMap;
use std::sync::Arc;

use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::ExportWarning;
use rs_cam_core::geo::P3;
use rs_cam_core::session::{AdoptSimulationArgs, Command, ProjectSession};
use rs_cam_core::stock::collision::RapidCollision;
use rs_cam_viz::io::export::{
    export_gcode_from_session_reporting, export_reply_text, preflight_warnings,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::state::simulation::{SimulationResults, SimulationState};
use rs_cam_viz::ui::preflight::export_asks_a_click;
use rs_cam_viz::ui::readiness::{self, CheckStatus, rapid_collision_check, rapid_collision_detail};

/// The local move indices of the rapid collisions on each toolpath.
const HITS: [(usize, &[usize]); 2] = [(k6_project::INVERTED, &[0, 1]), (k6_project::GOUGE, &[0])];

/// The K6 project with a stored simulation that holds the rapid collisions
/// of [`HITS`]. Gives the session and the GUI move list of the run.
fn session_with_rapid_collisions() -> (ProjectSession, Vec<RapidCollision>) {
    let mut session = k6_project::generated_and_simulated();
    let mut simulation = session
        .simulation_result()
        .cloned()
        .expect("the K6 project is simulated");
    let mut collisions = Vec::new();
    let mut global = Vec::new();
    for (index, locals) in HITS {
        let id = session.toolpath_configs()[index].id;
        let boundary = simulation
            .boundaries
            .iter()
            .find(|b| b.id == id)
            .expect("the simulation holds the toolpath");
        for &local in locals {
            assert!(
                boundary.start_move + local < boundary.end_move,
                "the hit is inside the toolpath"
            );
            collisions.push(RapidCollision {
                move_index: local,
                start: P3::new(0.0, 0.0, 5.0),
                end: P3::new(1.0, 1.0, -1.0),
            });
            global.push(boundary.start_move + local);
        }
    }
    simulation.rapid_collisions = collisions.clone();
    simulation.rapid_collision_move_indices = global;
    let epoch = session.simulation_epoch();
    let _ = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(simulation),
            epoch,
            modulation: None,
        }))
        .expect("the adopt stores the simulation");
    (session, collisions)
}

/// The `(id, toolpath)` of each `project.rapid_collision` row, sorted.
fn rapid_rows(warnings: &[ExportWarning]) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = warnings
        .iter()
        .filter(|w| w.id == ids::PROJECT_RAPID_COLLISION)
        .map(|w| (w.id.to_owned(), w.toolpath.clone()))
        .collect();
    rows.sort();
    rows
}

/// An `AppState` that holds `session`, a GUI simulation result and the GUI
/// move list `gui_moves` (the list the GUI drain writes).
fn app_state(session: ProjectSession, gui_moves: Vec<RapidCollision>) -> AppState {
    let simulation_result = session
        .simulation_result()
        .expect("the session holds the simulation");
    let mut simulation = SimulationState::new();
    simulation.results = Some(SimulationResults {
        mesh: Arc::new(rs_cam_core::stock::stock_mesh::StockMesh::empty()),
        total_moves: simulation_result.total_moves,
        boundaries: simulation_result.boundaries.clone(),
        setup_boundaries: Vec::new(),
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: session.stock_bbox(),
        cut_trace: simulation_result.cut_trace.clone(),
        cut_trace_path: None,
        column_grid_cell_mm: simulation_result.column_grid_cell_mm,
        prior_stocks: HashMap::new(),
    });
    simulation.checks.rapid_collisions = gui_moves;
    let mut state = AppState::new();
    state.replace_session(session);
    state.simulation = simulation;
    state
}

#[test]
fn a_collision_is_the_same_warning_on_every_surface_q6() {
    let (session, moves) = session_with_rapid_collisions();
    let expected: Vec<(String, String)> = {
        let mut rows: Vec<(String, String)> = HITS
            .iter()
            .map(|(index, _)| {
                (
                    ids::PROJECT_RAPID_COLLISION.to_owned(),
                    k6_project::NAMES[*index].to_owned(),
                )
            })
            .collect();
        rows.sort();
        rows
    };
    // Non-vacuity: the move count and the toolpath count differ, so a card
    // that counts moves cannot pass.
    assert_ne!(moves.len(), expected.len());

    // The CLI door and its summary lines.
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    let cli = session
        .export_gcode_checked(trace.as_deref())
        .expect("a rapid collision does not refuse the CLI export");
    assert_eq!(rapid_rows(&cli.warnings), expected, "the core rows");
    let cli_summary: Vec<String> = cli
        .warnings
        .iter()
        .filter(|w| w.id == ids::PROJECT_RAPID_COLLISION)
        .map(ExportWarning::header_line)
        .collect();

    // The GUI export door and the MCP reply text.
    let gui = GuiState::new();
    let exported = export_gcode_from_session_reporting(&session, &gui, &SimulationState::new())
        .expect("a rapid collision does not refuse the GUI export");
    assert_eq!(rapid_rows(&exported.warnings), expected, "the GUI export");
    let reply = export_reply_text("G-code exported to /tmp/q6.nc".to_owned(), &exported);
    for line in &cli_summary {
        assert!(
            reply.contains(line.as_str()),
            "the MCP reply does not hold the CLI line `{line}`: {reply}"
        );
    }
    assert_eq!(
        reply
            .lines()
            .filter(|l| l.contains(ids::PROJECT_RAPID_COLLISION))
            .count(),
        expected.len(),
        "the MCP reply names one row per toolpath: {reply}"
    );

    // The GUI pre-flight: the rows, the card and the Export button.
    let mut state = app_state(session, moves);
    let warnings = preflight_warnings(state.session(), &state.gui, &state.simulation);
    assert_eq!(rapid_rows(&warnings), expected, "the pre-flight rows");
    assert_eq!(readiness::rapid_collision_rows(&warnings), expected.len());
    assert_eq!(rapid_collision_check(&state, &warnings), CheckStatus::Fail);
    assert_eq!(
        rapid_collision_detail(&state, &warnings),
        format!("{} toolpaths", expected.len()),
        "the card counts toolpaths, the unit of the MCP and CLI rows"
    );
    assert!(
        export_asks_a_click(&warnings),
        "a rapid collision is a Machine warning: the Export button is Danger"
    );

    // The card reads the core rows, not the GUI move list: with the GUI
    // list empty, the card still fails.
    state.simulation.checks.rapid_collisions.clear();
    assert_eq!(rapid_collision_check(&state, &warnings), CheckStatus::Fail);
    assert_eq!(
        rapid_collision_detail(&state, &warnings),
        format!("{} toolpaths", expected.len())
    );
}

/// The card with no rapid collision: Pass, "None detected", and the Export
/// button has no Danger style from this card.
#[test]
fn no_collision_row_passes_the_card_q6() {
    let session = k6_project::generated_and_simulated();
    let state = app_state(session, Vec::new());
    let warnings = preflight_warnings(state.session(), &state.gui, &state.simulation);
    assert!(rapid_rows(&warnings).is_empty(), "{warnings:?}");
    assert_eq!(rapid_collision_check(&state, &warnings), CheckStatus::Pass);
    assert_eq!(rapid_collision_detail(&state, &warnings), "None detected");
}
