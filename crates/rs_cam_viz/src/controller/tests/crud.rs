//! Controller CRUD (E-crud), tool-deletion safety (C5) and `AddToolpath`
//! validation (C16).

use super::*;

#[test]
fn remove_tool_succeeds_when_no_toolpath_references_it() {
    let mut controller = sample_controller();

    // Add a second tool that is not referenced by any toolpath.
    let unreferenced_id = ToolId(99);
    let extra_tool = ToolConfig::new_default(unreferenced_id, ToolType::EndMill);
    let mut tools = controller.state.session.tools().to_vec();
    tools.push(extra_tool);
    let _ = controller
        .state
        .session
        .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
        .expect("the session takes the tool list");
    let tool_count_before = controller.state.session.tools().len();

    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(unreferenced_id));
    assert_eq!(
        controller.state.session.tools().len(),
        tool_count_before - 1,
        "Unreferenced tool should be removed"
    );
    assert!(
        controller
            .state
            .session
            .tools()
            .iter()
            .all(|t| t.id != unreferenced_id),
        "The specific tool should no longer be in the list"
    );
}

// ---------------------------------------------------------------------------
// AddToolpath validation tests (C16)
// ---------------------------------------------------------------------------

#[test]
fn add_toolpath_blocked_when_no_tools_exist() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    // No tools added — controller.state.session.tools() is empty.
    assert!(controller.state.session.tools().is_empty());

    let tp_count_before = controller.state.session.toolpath_count();

    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Adaptive3d,
    ));

    let tp_count_after = controller.state.session.toolpath_count();
    assert_eq!(
        tp_count_before, tp_count_after,
        "No toolpath should be created when no tools exist"
    );
}

// ---------------------------------------------------------------------------
// E-crud: Controller CRUD tests
// ---------------------------------------------------------------------------

#[test]
fn add_tool_and_remove_tool_lifecycle() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    assert!(controller.state.session.tools().is_empty());

    // Add a tool
    controller.handle_internal_event(crate::ui::AppEvent::AddTool(ToolType::EndMill));
    assert_eq!(controller.state.session.tools().len(), 1);
    let tool_id = controller.state.session.tools()[0].id;
    assert_eq!(
        controller.state.session.tools()[0].tool_type,
        ToolType::EndMill
    );

    // Verify selection was set to the new tool
    assert_eq!(controller.state.selection, Selection::Tool(tool_id));

    // Remove the tool (no toolpaths reference it)
    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(tool_id));
    assert!(
        controller.state.session.tools().is_empty(),
        "Tool should be removed when no toolpaths reference it"
    );
}

#[test]
fn add_setup_appends_a_second_setup() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    // Starts with one default setup
    assert_eq!(controller.state.session.list_setups().len(), 1);
    let original_setup_id = SetupId(controller.state.session.list_setups()[0].id);

    // Add a second setup
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    assert_eq!(controller.state.session.list_setups().len(), 2);
    let new_setup_id = SetupId(controller.state.session.list_setups()[1].id);
    assert_ne!(original_setup_id, new_setup_id);
    assert_eq!(
        SetupId(controller.state.session.list_setups()[0].id),
        original_setup_id,
        "the add appends; it never re-keys the setup that stood first"
    );
}

#[test]
fn remove_setup_selects_previous_then_next_and_cancels_generation() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    let first = SetupId(controller.state.session.list_setups()[0].id);
    let second = SetupId(controller.state.session.list_setups()[1].id);
    let third = SetupId(controller.state.session.list_setups()[2].id);

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(second));

    assert_eq!(controller.state.session.list_setups().len(), 2);
    assert_eq!(controller.state.selection, Selection::Setup(first));
    assert_eq!(
        controller.compute.toolpath_lane.state,
        LaneState::Cancelling
    );

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(first));

    assert_eq!(controller.state.session.list_setups().len(), 1);
    assert_eq!(controller.state.selection, Selection::Setup(third));
}

#[test]
fn remove_final_setup_is_refused_without_cancelling_or_editing() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    let only = SetupId(controller.state.session.list_setups()[0].id);
    let epoch = controller.state.session.simulation_epoch();
    let edits = controller.state.gui.edit_counter;

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(only));

    assert_eq!(controller.state.session.list_setups().len(), 1);
    assert_eq!(controller.compute.toolpath_lane.state, LaneState::Idle);
    assert_eq!(controller.state.session.simulation_epoch(), epoch);
    assert_eq!(controller.state.gui.edit_counter, edits);
}

#[test]
fn remove_setup_cleans_runtime_and_ignores_a_queued_late_completion() {
    let mut controller = sample_controller();
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    let removed_setup = SetupId(controller.state.session.list_setups()[1].id);
    let retained_id = controller.state.session.toolpath_configs()[0].id;
    let mut config = controller.state.session.toolpath_configs()[0].clone();
    config.name = "Removed setup operation".to_owned();
    let removed_index = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 1,
            config: Box::new(config),
        }))
        .expect("add toolpath to removed setup")
        .created
        .expect("created toolpath index");
    let removed_id = controller.state.session.toolpath_configs()[removed_index].id;
    controller
        .state
        .gui
        .toolpath_rt
        .insert(removed_id, ToolpathRuntime::new(true));
    assert!(controller.start_gui_plan(
        rs_cam_core::session::generation_plan::Scope::Ancestors(removed_id),
        Some(removed_id),
    ));
    assert!(controller.plan.as_ref().is_some_and(|plan| plan.in_flight));
    #[cfg(feature = "mcp")]
    let mut late_mcp_rx = {
        controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
        let (tx, rx) = tokio::sync::oneshot::channel();
        controller
            .pending_mcp
            .as_mut()
            .expect("pending MCP state")
            .toolpath
            .insert(removed_id, tx);
        rx
    };
    controller.state.pending_reconciliation_for_ids = vec![retained_id, removed_id];
    controller.state.pending_apply_resim = Some(removed_id.0);
    controller.state.gui.pending_toolpath_tab =
        Some((removed_id, crate::ui::properties::ToolpathTab::Geometry));
    controller.state.simulation.debug.pending_inspect_toolpath = Some(removed_id);
    controller.superseded_toolpaths.insert(removed_id);
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: removed_id,
                revision: None,
                result: Ok(ToolpathResult {
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(Toolpath::new()),
                    ),
                    stats: Default::default(),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                    drill_op: None,
                }),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
    let edits = controller.state.gui.edit_counter;
    controller.pending_upload = false;

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(removed_setup));

    assert!(controller.state.gui.toolpath_rt.contains_key(&retained_id));
    assert!(!controller.state.gui.toolpath_rt.contains_key(&removed_id));
    assert_eq!(
        controller.state.pending_reconciliation_for_ids,
        vec![retained_id]
    );
    assert!(controller.state.pending_apply_resim.is_none());
    assert!(controller.state.gui.pending_toolpath_tab.is_none());
    assert!(
        controller
            .state
            .simulation
            .debug
            .pending_inspect_toolpath
            .is_none()
    );
    assert!(!controller.superseded_toolpaths.contains(&removed_id));
    assert!(controller.pending_upload);
    assert_eq!(controller.state.gui.edit_counter, edits + 1);

    controller.drain_compute_results();

    assert!(
        controller
            .state
            .session
            .find_toolpath_config_by_id(removed_id)
            .is_none()
    );
    assert!(!controller.state.gui.toolpath_rt.contains_key(&removed_id));
    assert!(controller.state.session.get_result(0).is_none());
    assert!(
        controller.plan.is_none(),
        "the deleted target's terminal message must close the cancelled plan"
    );
    #[cfg(feature = "mcp")]
    {
        let response = late_mcp_rx
            .try_recv()
            .expect("the deleted target's terminal message must resolve its MCP waiter");
        let payload = response.result.expect("MCP completion payload");
        assert!(payload.contains("Toolpath not found"), "payload: {payload}");
        assert!(
            !controller
                .pending_mcp
                .as_ref()
                .expect("pending MCP state remains allocated")
                .toolpath
                .contains_key(&removed_id)
        );
    }
}

/// Q1: the add-toolpath door holds the model before it calls Suggest.
///
/// `SuggestContext::model_bbox` gates the runtime-sanity stepover
/// back-off, and every surface used to pass `SuggestContext::default()`,
/// so the back-off read "no constraint signal" on every add. The GUI
/// door resolved `model_id` AFTER the Suggest call; it now resolves it
/// before, and passes `ProjectSession::model_bbox` of that id.
///
/// What this pins is the plumbing precondition: the id the door writes
/// onto the toolpath resolves to a real bbox at the moment Suggest runs.
/// The engine-side effect of a `Some` bbox lives in
/// `rs_cam_core::feeds`, which this programme must not edit, so it is
/// not observable from here.
#[test]
fn the_add_door_holds_a_model_bbox_for_the_toolpath_it_writes() {
    let mut controller = sample_controller();
    let before = controller.state.session.toolpath_count();
    controller.handle_add_toolpath(crate::state::toolpath::OperationType::Pocket);
    assert_eq!(
        controller.state.session.toolpath_count(),
        before + 1,
        "the add door must create the toolpath"
    );

    let added = &controller.state.session.toolpath_configs()[before];
    let bbox = controller
        .state
        .session
        .model_bbox(added.model_id)
        .expect("the model the door named must carry a bbox");
    let model = controller
        .state
        .session
        .models()
        .iter()
        .find(|m| m.id == added.model_id)
        .expect("the door must name a model the session holds");
    let mesh = model.mesh.as_ref().expect("the fixture model is a mesh");
    assert!(
        (bbox.min.x - mesh.bbox.min.x).abs() < 1e-9 && (bbox.max.z - mesh.bbox.max.z).abs() < 1e-9,
        "the bbox must be the model's own, not a placeholder: {bbox:?}"
    );
}

#[test]
fn add_toolpath_and_remove_toolpath_lifecycle() {
    let mut controller = sample_controller();
    let tp_count_before = controller.state.session.toolpath_count();
    assert!(
        tp_count_before >= 1,
        "sample_controller starts with 1 toolpath"
    );

    // Add a toolpath
    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));
    let tp_count_after = controller.state.session.toolpath_count();
    assert_eq!(
        tp_count_after,
        tp_count_before + 1,
        "Toolpath should be added"
    );

    // Find the newly added toolpath ID (it should be the selected one)
    let Selection::Toolpath(new_tp_id) = controller.state.selection else {
        panic!("Selection should be the new toolpath");
    };

    // Remove it
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(new_tp_id));
    assert_eq!(
        controller.state.session.toolpath_count(),
        tp_count_before,
        "Toolpath should be removed"
    );
    assert!(
        controller
            .state
            .session
            .find_toolpath_config_by_id(new_tp_id)
            .is_none(),
        "Removed toolpath should not be findable"
    );
}

#[test]
fn add_toolpath_requires_geometry_for_polygon_operations() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    controller.handle_internal_event(crate::ui::AppEvent::AddTool(ToolType::EndMill));

    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));

    assert_eq!(
        controller.state.session.toolpath_count(),
        0,
        "Polygon-based toolpaths should not be created without an imported model"
    );
}

#[test]
fn reset_simulation_cancels_analysis_lane() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    controller.state.simulation.results = Some(crate::state::simulation::SimulationResults {
        mesh: rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 1,
        boundaries: Vec::new(),
        setup_boundaries: Vec::new(),
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: rs_cam_core::geo::BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(1.0, 1.0, 1.0),
        },
        cut_trace: None,
        cut_trace_path: None,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    });

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert_eq!(
        controller.compute.analysis_lane.state,
        LaneState::Cancelling,
        "Reset should cancel in-flight analysis work"
    );
    assert!(controller.state.simulation.results.is_none());
}

#[test]
fn duplicate_tool_creates_independent_copy() {
    let mut controller = sample_controller();
    let original_tool = controller.state.session.tools()[0].clone();
    let original_id = original_tool.id;

    controller.handle_internal_event(crate::ui::AppEvent::DuplicateTool(original_id));
    assert_eq!(
        controller.state.session.tools().len(),
        2,
        "Should have original + copy"
    );

    let copy = &controller.state.session.tools()[1];
    assert_ne!(copy.id, original_id, "Copy should have a different ID");
    assert!(
        copy.name.contains("(copy)"),
        "Copy name should contain '(copy)': {}",
        copy.name
    );
    assert_eq!(copy.tool_type, original_tool.tool_type);
    assert_eq!(copy.diameter, original_tool.diameter);

    // Modifying the copy should not affect the original (they are independent)
    // We verify they are separate entries in the tools list
    assert_eq!(controller.state.session.tools()[0].id, original_id);
    assert_ne!(controller.state.session.tools()[1].id, original_id);
}

#[test]
fn rename_setup_updates_name() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    let setup_id = SetupId(controller.state.session.list_setups()[0].id);

    controller.handle_internal_event(crate::ui::AppEvent::RenameSetup(
        setup_id,
        "My Custom Setup".to_owned(),
    ));

    assert_eq!(
        controller.state.session.list_setups()[0].name,
        "My Custom Setup",
        "Setup name should be updated"
    );
}

// ---------------------------------------------------------------------------
// E-sel: Selection cascade tests
// ---------------------------------------------------------------------------

#[test]
fn delete_selected_tool_clears_selection() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());

    // Add a tool and select it
    controller.handle_internal_event(crate::ui::AppEvent::AddTool(ToolType::BallNose));
    let tool_id = controller.state.session.tools()[0].id;
    assert_eq!(controller.state.selection, Selection::Tool(tool_id));

    // Remove the tool
    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(tool_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared after deleting the selected tool"
    );
}
