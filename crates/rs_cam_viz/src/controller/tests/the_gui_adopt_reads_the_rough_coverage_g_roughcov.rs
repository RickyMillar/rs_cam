//! G-ROUGHCOV A2 (adaptive research 2026-10-23, evidence run Finding 0).
//!
//! Claim: a 3D Rough adopted through the GUI path has a coverage reading on
//! every session surface, although the session copy keeps no checkpoint
//! (W2-E).
//!
//! The defect: `core_simulation_from_lane` gives the session
//! `checkpoints: Vec::new()`, so the session found no checkpoint stock and
//! published `triage.coverage == []`, which reads like a measured zero.
//! The drain now reads the coverage once at adopt from the VIEW's
//! checkpoints (`RoughCoverageMemo::read`), and the evidence carries the
//! readings (`ProjectEvidence::rough_coverage`).
//!
//! These tests hold:
//! - a run whose checkpoint holds a standing block reads ONE island through
//!   the session's triage, and the session copy still holds no checkpoint;
//! - a run with no checkpoint for the rough reads an abstention, never an
//!   empty list.
//!
//! Falsifier: remove the memo assignment in the drain
//! (`controller/events/compute.rs`), and arm 1 reads `RunNotStored`, not an
//! island.

use rs_cam_core::compute::simulate::SimCheckpointMesh;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::stock::standing_islands::{RoughCoverageAbstention, RoughCoverageOutcome};

use super::*;

/// The flat fixture model spans [-20, 20]² at Z 0.
fn stock_bbox() -> rs_cam_core::geo::BoundingBox3 {
    rs_cam_core::geo::BoundingBox3 {
        min: P3::new(-20.0, -20.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    }
}

const STL_MM: f64 = 0.5;
const CELL_MM: f64 = 0.25;

/// The sample project with its one toolpath turned into a 3D Rough.
fn rough_controller() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    let (index, stored) = controller
        .state
        .session()
        .toolpath_configs()
        .iter()
        .enumerate()
        .next()
        .map(|(i, tc)| (i, tc.clone()))
        .expect("the sample holds one toolpath");
    let mut config = stored;
    let mut operation = rs_cam_core::compute::fixture_values::fixture_op(
        rs_cam_core::compute::catalog::OperationType::Adaptive3d,
    );
    if let rs_cam_core::compute::catalog::OperationConfig::Adaptive3d(c) = &mut operation {
        c.stock_to_leave_axial = STL_MM;
    }
    config.operation = operation;
    // The flat sample model and the sample stock top both sit at Z 0, so
    // the 3D Rough's Auto bottom equals its top (the stock top) and the
    // height order refuses. Pin the bottom at the sample stock bottom: it is
    // below the model, so the coverage target (model + leave) is unchanged.
    config.heights.bottom_z = rs_cam_core::compute::config::HeightMode::Manual(
        -rs_cam_core::compute::fixture_values::FIXTURE_STOCK_Z_MM,
    );
    let _ = controller
        .state
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index,
            config: Box::new(config),
        }))
        .expect("the toolpath sits at a live index");
    generate_all_for_test(&mut controller);
    controller
}

/// A checkpoint stock cut to the rough's target (model Z 0 + 0.5) except a
/// block at x, y in [2, 4] × [2, 6] left at the stock top.
fn checkpoint_with_a_block() -> Arc<SimCheckpointMesh> {
    let mut stock = TriDexelStock::from_bounds(&stock_bbox(), CELL_MM);
    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            let (x, y) = stock.z_grid.cell_to_world(row, col);
            let in_block = (2.0..=4.0).contains(&x) && (2.0..=6.0).contains(&y);
            if !in_block {
                stock.clear_above_at(row, col, STL_MM as f32);
            }
        }
    }
    Arc::new(SimCheckpointMesh {
        boundary_index: 0,
        mesh_stock: Arc::new(stock.clone()),
        mesh_drill_ops: Vec::new(),
        mesh_frame: None,
        mesh_stock_min: stock_bbox().min,
        stock,
        stock_local_to_global: None,
        display_stride: 1,
    })
}

/// Submit a run and land a lane answer for the rough with `checkpoints`.
fn land_run(
    controller: &mut AppController<ScriptedBackend>,
    checkpoints: Vec<Arc<SimCheckpointMesh>>,
) -> ToolpathId {
    let toolpath = controller.state.session().toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::RunSimulation);
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh: Arc::new(rs_cam_core::stock::stock_mesh::StockMesh::empty()),
                total_moves: 10,
                deviations: None,
                column_deviations: None,
                boundaries: vec![crate::compute::worker::SimBoundary {
                    id: toolpath,
                    name: "3D Rough".to_owned(),
                    tool_name: "Sample tool".to_owned(),
                    start_move: 0,
                    end_move: 10,
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                }],
                checkpoints,
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: Some(Arc::new(
                    rs_cam_core::stock::simulation_cut::SimulationCutTrace::from_samples(
                        0.5,
                        Vec::new(),
                    ),
                )),
                column_grid_cell_mm: CELL_MM,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
                prior_stock_sources: std::collections::HashMap::new(),
                display_degrade: None,
                group_starts: Vec::new(),
                stock_change_volumes: Vec::new(),
                obstacle_collisions: Vec::new(),
                non_centre_cutting_descents: Vec::new(),
                flute_overreach: Vec::new(),
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));
    controller.drain_compute_results();
    toolpath
}

#[test]
fn a_gui_adopted_rough_reads_its_island_through_the_session_g_roughcov() {
    let mut controller = rough_controller();
    let toolpath = land_run(&mut controller, vec![checkpoint_with_a_block()]);

    // W2-E stays: the session copy holds no checkpoint.
    let session_sim = controller
        .state
        .session()
        .simulation_result()
        .expect("the session adopted the run");
    assert!(
        session_sim.checkpoints.is_empty(),
        "W2-E: no checkpoint in the session"
    );

    let evidence = controller.state.simulation.project_evidence();
    assert!(
        evidence.rough_coverage.is_some(),
        "the view hands the session the readings it read at adopt"
    );
    let triage = controller.state.session().simulation_triage(&evidence);
    let reading = triage
        .coverage
        .iter()
        .find(|r| r.toolpath_id == toolpath)
        .expect("the rough has a coverage reading on the session's triage");
    let coverage = match &reading.outcome {
        RoughCoverageOutcome::Measured(c) => c,
        RoughCoverageOutcome::NotMeasurable(why) => {
            panic!("expected a measured reading, got {why:?}")
        }
    };
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 9 * 17, "{island:?}");
    assert!(
        triage.findings().any(|f| f.diagnostic.id.as_str()
            == rs_cam_core::diagnostics::ids::PROJECT_ROUGH_LEAVES_ISLANDS),
        "the island raises its finding"
    );

    // The same reading reaches the per-op doors.
    let one = controller
        .state
        .session()
        .rough_coverage_reading(toolpath, &evidence)
        .expect("narrate_toolpath's door reads it");
    assert_eq!(&one, reading);
}

#[test]
fn a_gui_run_with_no_checkpoint_for_the_rough_abstains_g_roughcov() {
    let mut controller = rough_controller();
    let toolpath = land_run(&mut controller, Vec::new());
    let evidence = controller.state.simulation.project_evidence();
    let triage = controller.state.session().simulation_triage(&evidence);
    let reading = triage
        .coverage
        .iter()
        .find(|r| r.toolpath_id == toolpath)
        .expect("a rough in the run is never absent from the coverage list");
    assert_eq!(
        reading.outcome,
        RoughCoverageOutcome::NotMeasurable(RoughCoverageAbstention::NoCheckpoint)
    );
}
