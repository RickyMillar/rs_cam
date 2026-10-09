//! W2-E (memory programme 2026-10-01) — the session copy of a simulation
//! keeps only the fields a session reader reads, and the freshness names a
//! released view.
//!
//! `core_simulation_from_lane` builds the session copy. Before W2-E it
//! shared the display mesh and the checkpoint `Arc`s with the view. When
//! `SimulationState::release_for_new_run` dropped the view's handles for a
//! new run, the session handles still held the old mesh and checkpoints for
//! the whole run. These tests hold:
//! - the session copy has an empty mesh, no checkpoints and no deviations;
//! - the session copy keeps the prior stocks that rest generation reads;
//! - the freshness reads `Released` while a re-run works and after it is
//!   cancelled, and the preflight card names the release;
//! - Reset is not a release.
//!
//! This file replaces `session_and_view_share_one_mesh_m4.rs`. That sentry
//! held the session and the view to ONE mesh. Now the session holds none.

use rs_cam_core::compute::simulate::{ColumnDeviation, SimCheckpointMesh};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::stock::stock_mesh::StockMesh;

use super::*;
use crate::state::freshness::SimFreshness;

/// The heavy artifacts of one lane answer. The test keeps one handle on
/// each `Arc`, so a strong count of 2 means "the test and the view", and a
/// count of 3 means a third holder (the session) pins it.
struct LaneArtifacts {
    mesh: Arc<StockMesh>,
    checkpoint: Arc<SimCheckpointMesh>,
    prior: Arc<TriDexelStock>,
    toolpath: ToolpathId,
}

fn stock_bbox() -> rs_cam_core::geo::BoundingBox3 {
    rs_cam_core::geo::BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    }
}

fn lane_artifacts(controller: &AppController<ScriptedBackend>) -> LaneArtifacts {
    let toolpath = controller.state.session.toolpath_configs()[0].id;
    let stock = TriDexelStock::from_bounds(&stock_bbox(), 1.0);
    LaneArtifacts {
        mesh: Arc::new(StockMesh {
            vertices: vec![0.0; 9],
            indices: vec![0, 1, 2],
            colors: vec![0.5; 9],
            material_slots: Vec::new(),
        }),
        checkpoint: Arc::new(SimCheckpointMesh {
            boundary_index: 0,
            mesh_stock: Arc::new(stock.clone()),
            mesh_drill_ops: Vec::new(),
            mesh_frame: None,
            mesh_stock_min: stock_bbox().min,
            stock,
            stock_local_to_global: None,
            display_stride: 1,
        }),
        prior: Arc::new(TriDexelStock::from_bounds(&stock_bbox(), 1.0)),
        toolpath,
    }
}

/// Queue one lane answer that carries every heavy field, and drain it.
fn land_run(controller: &mut AppController<ScriptedBackend>, artifacts: &LaneArtifacts) {
    let mut prior_stocks = std::collections::HashMap::new();
    let _ = prior_stocks.insert(artifacts.toolpath, Arc::clone(&artifacts.prior));
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh: Arc::clone(&artifacts.mesh),
                total_moves: 10,
                deviations: Some(vec![0.1; 3]),
                column_deviations: Some(vec![ColumnDeviation {
                    x: 1.0,
                    y: 1.0,
                    dev: 0.1,
                    group: 0,
                    top_z: 0.0,
                    row: 0,
                    col: 0,
                }]),
                boundaries: vec![crate::compute::worker::SimBoundary {
                    id: artifacts.toolpath,
                    name: "Op 1".to_owned(),
                    tool_name: "EndMill".to_owned(),
                    start_move: 0,
                    end_move: 10,
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                }],
                checkpoints: vec![Arc::clone(&artifacts.checkpoint)],
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: None,
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks,
                prior_stock_sources: std::collections::HashMap::new(),
                display_degrade: None,
                group_starts: Vec::new(),
                stock_change_volumes: Vec::new(),
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));
    controller.drain_compute_results();
}

/// A controller whose view and session both hold a landed run.
fn controller_with_a_landed_run() -> (AppController<ScriptedBackend>, LaneArtifacts) {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let artifacts = lane_artifacts(&controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    land_run(&mut controller, &artifacts);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current,
        "the control: the fixture run landed and the core adopted it"
    );
    (controller, artifacts)
}

#[test]
fn the_session_copy_holds_no_mesh_checkpoints_or_deviations_after_adopt_w2e() {
    let (controller, artifacts) = controller_with_a_landed_run();

    // Non-vacuity: the view holds the heavy artifacts the lane sent.
    let view = controller
        .state
        .simulation
        .results
        .as_ref()
        .expect("the control: the view holds the run");
    assert!(!view.mesh.indices.is_empty());
    assert_eq!(view.checkpoints.len(), 1);
    assert!(
        controller
            .state
            .simulation
            .playback
            .display_deviations
            .is_some()
    );

    let session = controller
        .state
        .session
        .simulation_result()
        .expect("the session adopted the run");
    assert!(
        session.mesh.vertices.is_empty() && session.mesh.indices.is_empty(),
        "no session reader reads the display mesh; the view keeps its own"
    );
    assert!(
        session.checkpoints.is_empty(),
        "no session reader reads a checkpoint; the view keeps its own"
    );
    assert!(
        session.deviations.is_none(),
        "the view keeps the per-vertex deviations"
    );
    assert!(
        session.column_deviations.is_none(),
        "no reader in the GUI process reads the per-column deviations"
    );
    // The memory claim: only the test and the view hold the mesh and the
    // checkpoint. A third holder would pin them across a release.
    assert_eq!(
        Arc::strong_count(&artifacts.mesh),
        2,
        "test + view, no session"
    );
    assert_eq!(
        Arc::strong_count(&artifacts.checkpoint),
        2,
        "test + view, no session"
    );

    // What session readers DO read stays.
    let prior = session
        .prior_stocks
        .get(&artifacts.toolpath)
        .expect("rest generation reads this snapshot; the session must keep it");
    assert!(
        Arc::ptr_eq(prior, &artifacts.prior),
        "the session shares the lane's prior stock, it does not copy it"
    );
    assert_eq!(session.boundaries.len(), 1, "triage reads the boundaries");
    assert_eq!(session.total_moves, 10);
    assert!((session.column_grid_cell_mm - 0.5).abs() < f64::EPSILON);
}

#[test]
fn a_released_view_reads_released_during_a_run_and_after_a_cancel_w2e() {
    let (mut controller, artifacts) = controller_with_a_landed_run();

    // A re-run releases the view of the landed run.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let freshness = controller.state.simulation_freshness();
    assert_eq!(freshness, SimFreshness::Released { in_flight: true });
    assert!(freshness.is_in_flight(), "a re-run is a run in flight");
    assert!(
        freshness.is_stale(),
        "the core run is not the run asked for"
    );
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Running — previous result released"
    );
    // After the release only the test holds the mesh and the checkpoint.
    assert_eq!(
        Arc::strong_count(&artifacts.mesh),
        1,
        "the release frees the mesh"
    );
    assert_eq!(
        Arc::strong_count(&artifacts.checkpoint),
        1,
        "the release frees the checkpoint"
    );

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();

    let freshness = controller.state.simulation_freshness();
    assert_eq!(freshness, SimFreshness::Released { in_flight: false });
    assert!(!freshness.is_in_flight());
    assert!(freshness.is_stale());
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Stale — previous result released, run again"
    );
    assert!(
        controller.state.simulation.last_run.is_some(),
        "last_run names the released run whose evidence the core holds"
    );
    assert!(
        controller
            .state
            .session
            .simulation_result()
            .is_some_and(|sim| sim.prior_stocks.contains_key(&artifacts.toolpath)),
        "a rest cascade still starts from the released run"
    );
    assert!(
        controller
            .notifications()
            .iter()
            .any(|n| n.message.starts_with("Simulation cancelled.")),
        "the drain names the release"
    );
}

/// The control for the derivation: Reset clears the view AND `last_run`,
/// so it reads `NoRun`, not `Released`, although the core still holds the
/// run.
#[test]
fn a_reset_is_not_a_release_w2e() {
    let (mut controller, _artifacts) = controller_with_a_landed_run();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert!(
        controller.state.session.simulation_result().is_some(),
        "the control: Reset leaves the core simulation"
    );
    assert_eq!(controller.state.simulation_freshness(), SimFreshness::NoRun);
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Not run"
    );
}
