//! Inspect-in-simulation: the workspace switch, the targeted run and the
//! result that lands on a pending start.

use super::*;

#[test]
fn inspect_toolpath_in_simulation_queues_workspace_switch_and_jump_when_results_exist() {
    let mut controller = sample_controller();
    controller.state.simulation.results = Some(crate::state::simulation::SimulationResults {
        mesh: rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 12,
        boundaries: vec![crate::state::simulation::ToolpathBoundary {
            id: ToolpathId(0),
            name: "Adaptive 3D".to_owned(),
            tool_name: "Tool".to_owned(),
            start_move: 4,
            end_move: 12,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        setup_boundaries: vec![crate::state::simulation::SetupBoundary {
            setup_id: crate::state::job::SetupId(1),
            setup_name: "Setup 1".to_owned(),
            start_move: 0,
        }],
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: rs_cam_core::geo::BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(10.0, 10.0, 10.0),
        },
        cut_trace: None,
        cut_trace_path: None,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    });

    controller.handle_internal_event(crate::ui::AppEvent::Ui(
        UiCommand::InspectToolpathInSimulation(ToolpathId(0)),
    ));
    let events = controller.drain_events();

    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::Ui(UiCommand::SwitchWorkspace(
            crate::state::Workspace::Simulation
        ))
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: 4
        },))
    )));
    assert!(
        controller
            .state
            .simulation
            .debug
            .pending_inspect_toolpath
            .is_none()
    );
}

#[test]
fn inspect_toolpath_in_simulation_queues_targeted_run_when_results_missing() {
    let mut controller = sample_controller();

    controller.handle_internal_event(crate::ui::AppEvent::Ui(
        UiCommand::InspectToolpathInSimulation(ToolpathId(0)),
    ));
    let events = controller.drain_events();

    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::Ui(UiCommand::SwitchWorkspace(
            crate::state::Workspace::Simulation
        ))
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::RunSimulationWith(ids) if ids == &vec![ToolpathId(0)]
    )));
    assert_eq!(
        controller.state.simulation.debug.pending_inspect_toolpath,
        Some(ToolpathId(0))
    );
}

#[test]
fn simulation_results_land_on_pending_inspect_toolpath_start() {
    let mut controller = sample_controller();
    controller.state.simulation.debug.pending_inspect_toolpath = Some(ToolpathId(0));
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh: rs_cam_core::stock::stock_mesh::StockMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    colors: Vec::new(),
                },
                total_moves: 8,
                deviations: None,
                column_deviations: None,
                boundaries: vec![crate::compute::worker::SimBoundary {
                    id: ToolpathId(0),
                    name: "Adaptive 3D".to_owned(),
                    tool_name: "Tool".to_owned(),
                    start_move: 2,
                    end_move: 8,
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                }],
                checkpoints: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: None,
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));

    controller.drain_compute_results();

    assert_eq!(controller.state.simulation.playback.current_move, 2);
    assert!(
        controller
            .state
            .simulation
            .debug
            .pending_inspect_toolpath
            .is_none()
    );
}
