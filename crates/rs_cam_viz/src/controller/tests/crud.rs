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

/// A simulation result whose boundaries name `ids`, for the WP28 queue
/// tests below.
fn queued_simulation_naming(ids: &[ToolpathId]) -> ComputeMessage {
    let boundaries = ids
        .iter()
        .enumerate()
        .map(|(n, &id)| crate::compute::worker::SimBoundary {
            id,
            name: format!("Op {}", n + 1),
            tool_name: "EndMill".to_owned(),
            start_move: n * 10,
            end_move: (n + 1) * 10,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        })
        .collect();
    ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
        core: rs_cam_core::compute::simulate::SimulationResult {
            mesh: std::sync::Arc::new(rs_cam_core::stock::stock_mesh::StockMesh {
                vertices: vec![0.0; 9],
                indices: vec![0, 1, 2],
                colors: vec![0.5; 9],
                material_slots: Vec::new(),
            }),
            total_moves: 10 * ids.len(),
            deviations: None,
            column_deviations: None,
            boundaries,
            checkpoints: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            column_grid_cell_mm: 0.5,
            resolution_clamped: false,
            prior_stocks: std::collections::HashMap::new(),
            prior_stock_sources: std::collections::HashMap::new(),
            display_degrade: None,
            group_starts: Vec::new(),
            stock_change_volumes: Vec::new(),
            obstacle_collisions: Vec::new(),
        },
        playback_data: Vec::new(),
        cut_trace_path: None,
    })))
}

/// Add a second setup with one toolpath. Returns the new setup and the
/// toolpath id.
fn add_second_setup_with_a_toolpath(
    controller: &mut AppController<ScriptedBackend>,
) -> (SetupId, ToolpathId) {
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    let setup = SetupId(controller.state.session.list_setups()[1].id);
    let mut config = controller.state.session.toolpath_configs()[0].clone();
    config.name = "Removed setup operation".to_owned();
    let index = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 1,
            config: Box::new(config),
        }))
        .expect("add toolpath to the second setup")
        .created
        .expect("created toolpath index");
    let id = controller.state.session.toolpath_configs()[index].id;
    (setup, id)
}

/// WP28: a simulation result queued before a setup deletion names a
/// toolpath that no longer exists. The drain discards it before any view
/// install, and it closes the run the way a cancel does.
#[test]
fn remove_setup_discards_a_queued_simulation_that_names_a_removed_toolpath() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let (removed_setup, removed_id) = add_second_setup_with_a_toolpath(&mut controller);
    let retained_id = controller.state.session.toolpath_configs()[0].id;
    controller
        .compute
        .drained
        .push(queued_simulation_naming(&[retained_id, removed_id]));

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(removed_setup));
    controller.drain_compute_results();

    assert!(
        !controller.state.simulation.has_results(),
        "a result whose boundaries name a removed toolpath cannot be installed"
    );
    assert!(controller.state.simulation.last_run.is_none());
    assert!(controller.state.session.simulation_result().is_none());
}

/// WP28 with G-LATESIM: the setup deletion moves the epoch, but a queued
/// result that names only retained toolpaths can still be installed. It is
/// STORED and reads stale, and the core does not adopt it. Only a removed
/// toolpath makes a result a discard; an epoch change alone does not.
#[test]
fn remove_setup_stores_a_queued_simulation_of_retained_toolpaths_as_stale() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let (removed_setup, _removed_id) = add_second_setup_with_a_toolpath(&mut controller);
    let retained_id = controller.state.session.toolpath_configs()[0].id;
    controller
        .compute
        .drained
        .push(queued_simulation_naming(&[retained_id]));

    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(removed_setup));
    controller.drain_compute_results();

    assert!(
        controller.state.simulation.has_results(),
        "STORED, not discarded: every boundary still names a toolpath"
    );
    assert!(
        controller.state.session.simulation_result().is_none(),
        "the core does not adopt a run the deletion superseded"
    );
    assert!(controller.state.simulation_is_stale());
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
        mesh: std::sync::Arc::new(rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
            material_slots: Vec::new(),
        }),
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

// S4 — the setup panel's stock-change actions reach the session through
// the four S2 commands, and a refusal comes back into the editor.
//
// Plan: `planning/stock_additions_2026-10-09/PLAN.md`, package S4.
//
// The panel pushes `AppEvent::EditStockChanges`; the controller arm
// (`handle_stock_change_intent`) applies one command per intent. These
// tests drive the arm through `handle_internal_event`, the dispatch the
// frame loop takes, and read the session back.
mod stock_change_panel_s4 {
    use super::*;
    use crate::state::panels::StockChangeEditor;
    use crate::ui::AppEvent;
    use crate::ui::properties::stock_changes::StockChangeIntent;
    use rs_cam_core::compute::stock_change::{StockChange, StockChangeOp, StockGeometry};
    use rs_cam_core::ids::{ModelId, StockChangeId};

    fn change(id: usize, model_id: ModelId) -> StockChange {
        StockChange {
            id: StockChangeId(id),
            name: format!("Change {}", id + 1),
            enabled: true,
            op: StockChangeOp::Add,
            geometry: StockGeometry::Model { model_id },
            material: rs_cam_core::material::Material::Custom {
                name: "Resin".to_owned(),
                feed_scale_factor: 1.0,
            },
            display_colour: None,
            cut_as: Default::default(),
        }
    }

    fn ids(controller: &AppController<ScriptedBackend>) -> Vec<(usize, bool)> {
        controller.state.session.list_setups()[0]
            .stock_changes
            .iter()
            .map(|c| (c.id.0, c.enabled))
            .collect()
    }

    fn save(controller: &mut AppController<ScriptedBackend>, setup: SetupId, c: StockChange) {
        controller.state.panels.stock_change_editor = Some(StockChangeEditor {
            setup_id: setup,
            is_new: true,
            draft: c.clone(),
            refusal: None,
        });
        controller.handle_internal_event(AppEvent::EditStockChanges(
            setup,
            StockChangeIntent::Save {
                change: Box::new(c),
                is_new: true,
            },
        ));
    }

    #[test]
    fn every_panel_action_is_one_s2_command_s4() {
        let mut controller = sample_controller();
        let setup = SetupId(controller.state.session.list_setups()[0].id);
        let mesh = ModelId(controller.state.session.models()[0].id);

        // Add, twice. A saved draft closes the editor and dirties the project.
        save(&mut controller, setup, change(0, mesh));
        assert!(controller.state.panels.stock_change_editor.is_none());
        assert!(
            controller.state.gui.dirty,
            "an applied change marks the project edited"
        );
        save(&mut controller, setup, change(1, mesh));
        assert_eq!(ids(&controller), vec![(0, true), (1, true)]);

        // The checkbox: ReplaceStockChange with the stored record.
        controller.handle_internal_event(AppEvent::EditStockChanges(
            setup,
            StockChangeIntent::SetEnabled {
                change_id: StockChangeId(0),
                enabled: false,
            },
        ));
        assert_eq!(ids(&controller), vec![(0, false), (1, true)]);
        assert_eq!(
            controller.state.session.list_setups()[0].stock_changes[0].name,
            "Change 1",
            "the flip keeps the rest of the record"
        );

        // An edit from the editor: ReplaceStockChange with the draft.
        let mut renamed = change(1, mesh);
        renamed.name = "Riser".to_owned();
        controller.handle_internal_event(AppEvent::EditStockChanges(
            setup,
            StockChangeIntent::Save {
                change: Box::new(renamed),
                is_new: false,
            },
        ));
        assert_eq!(
            controller.state.session.list_setups()[0].stock_changes[1].name,
            "Riser"
        );

        // Move up: MoveStockChange.
        controller.handle_internal_event(AppEvent::EditStockChanges(
            setup,
            StockChangeIntent::Move {
                change_id: StockChangeId(1),
                to_position: 0,
            },
        ));
        assert_eq!(ids(&controller), vec![(1, true), (0, false)]);

        // Delete: RemoveStockChange, and an editor open on the change closes.
        controller.state.panels.stock_change_editor = Some(StockChangeEditor {
            setup_id: setup,
            is_new: false,
            draft: change(1, mesh),
            refusal: None,
        });
        controller.handle_internal_event(AppEvent::EditStockChanges(
            setup,
            StockChangeIntent::Remove(StockChangeId(1)),
        ));
        assert_eq!(ids(&controller), vec![(0, false)]);
        assert!(controller.state.panels.stock_change_editor.is_none());
    }

    #[test]
    fn a_refusal_reaches_the_editor_in_the_core_words_s4() {
        let mut controller = sample_controller();
        let setup = SetupId(controller.state.session.list_setups()[0].id);
        let mesh = ModelId(controller.state.session.models()[0].id);

        // A remove cannot use a fill: the S2 door refuses it by name.
        let mut removal = change(0, mesh);
        removal.op = StockChangeOp::Remove;
        removal.geometry = StockGeometry::OutlineFill {
            model_ids: vec![mesh],
            level_z: 0.0,
        };
        save(&mut controller, setup, removal);

        assert!(ids(&controller).is_empty(), "a refusal writes nothing");
        let editor = controller
            .state
            .panels
            .stock_change_editor
            .as_ref()
            .expect("the editor stays open on a refusal");
        let refusal = editor
            .refusal
            .as_deref()
            .expect("the editor holds the refusal");
        assert!(
            refusal.contains("a remove cannot use an outline fill"),
            "{refusal}"
        );
        assert!(
            controller
                .notifications()
                .iter()
                .any(|n| n.message.contains("a remove cannot use an outline fill")),
            "the refusal also reaches a notification"
        );
        assert!(
            !controller.state.gui.dirty,
            "a refusal does not dirty the project"
        );

        // Non-vacuity: the same draft as an Add with an outline the mesh does
        // not have is refused by its own rule, so the arm passes core's text.
        let mut no_outline = change(0, mesh);
        no_outline.geometry = StockGeometry::OutlineFill {
            model_ids: vec![mesh],
            level_z: 0.0,
        };
        save(&mut controller, setup, no_outline);
        let refusal = controller
            .state
            .panels
            .stock_change_editor
            .as_ref()
            .and_then(|e| e.refusal.clone())
            .expect("refused");
        assert!(refusal.contains("no closed outline"), "{refusal}");
    }
}

// K2 — the obstacle panel: every action reaches the session as one core
// command, and a refusal comes back into the editor in the core's words.
//
// Plan: `planning/op_architecture_2026-10-14/K2_PLAN.md`, test T2.
mod obstacle_panel_k2 {
    use super::*;
    use crate::state::job::ObstacleId;
    use crate::ui::AppEvent;
    use crate::ui::properties::apply_obstacle_draft;
    use rs_cam_core::ids::ModelId;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::session::{Obstacle, ObstacleGeometry};

    /// Add a 2D model with one closed outline; return its id.
    fn add_outline_model(controller: &mut AppController<ScriptedBackend>) -> ModelId {
        let id = controller
            .state
            .session
            .apply(Command::AddModel(AddModelArgs {
                model: Box::new(LoadedModel {
                    id: 0,
                    path: std::path::PathBuf::from("jaws.svg"),
                    name: "jaws".to_owned(),
                    kind: Some(ModelKind::Svg),
                    mesh: None,
                    polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 10.0, 5.0)])),
                    drill_targets: Arc::new(Vec::new()),
                    layers: Arc::new(Vec::new()),
                    enriched_mesh: None,
                    units: Some(ModelUnits::Millimeters),
                    winding_report: None,
                    load_error: None,
                }),
            }))
            .expect("the session takes the model")
            .created
            .expect("the AddModel row reports the new model id");
        ModelId(id)
    }

    fn stored(controller: &AppController<ScriptedBackend>, id: ObstacleId) -> Option<Obstacle> {
        controller.state.session.list_setups()[0]
            .obstacles
            .iter()
            .find(|o| o.id == id)
            .cloned()
    }

    #[test]
    fn every_obstacle_action_is_one_command_k2() {
        let mut controller = sample_controller();
        let setup = SetupId(controller.state.session.list_setups()[0].id);
        let mesh = ModelId(controller.state.session.models()[0].id);
        let outline = add_outline_model(&mut controller);

        // Add: one default box, selected, editor open on it.
        controller.state.gui.dirty = false;
        controller.handle_internal_event(AppEvent::AddObstacle(setup));
        let id = ObstacleId(0);
        let added = stored(&controller, id).expect("the add applies");
        assert!(matches!(added.geometry, ObstacleGeometry::Box { .. }));
        assert_eq!(controller.state.selection, Selection::Obstacle(setup, id));
        assert!(
            controller
                .state
                .panels
                .obstacle_editor
                .as_ref()
                .is_some_and(|e| e.obstacle_id == id && e.draft == added),
            "the editor opens on the new record"
        );
        assert!(controller.state.gui.dirty);

        // Apply an extrude on the outline model.
        controller.state.gui.dirty = false;
        let mut draft = added.clone();
        draft.geometry = ObstacleGeometry::OutlineExtrude {
            model_ids: vec![outline],
            z_bottom: -5.0,
            z_top: 10.0,
        };
        apply_obstacle_draft(&mut controller.state, 0, &added, draft.clone())
            .expect("an extrude on a closed outline applies");
        assert_eq!(stored(&controller, id), Some(draft.clone()));
        assert!(controller.state.gui.dirty);

        // The row checkbox: the stored record with the new flag only.
        controller.state.gui.dirty = false;
        controller.handle_internal_event(AppEvent::SetObstacleEnabled(setup, id, false));
        let toggled = stored(&controller, id).expect("still there");
        assert!(!toggled.enabled);
        assert_eq!(toggled.geometry, draft.geometry, "the rest is unchanged");
        assert!(controller.state.gui.dirty);

        // A model kind on the mesh model.
        controller.state.gui.dirty = false;
        let mut model_draft = toggled.clone();
        model_draft.geometry = ObstacleGeometry::Model { model_id: mesh };
        apply_obstacle_draft(&mut controller.state, 0, &toggled, model_draft.clone())
            .expect("a model obstacle on a mesh applies");
        assert_eq!(
            stored(&controller, id).map(|o| o.geometry),
            Some(model_draft.geometry)
        );
        assert!(controller.state.gui.dirty);

        // Remove: the list is empty, the editor closes, the setup is
        // selected.
        controller.state.selection = Selection::Obstacle(setup, id);
        controller.state.gui.dirty = false;
        controller.handle_internal_event(AppEvent::RemoveObstacle(setup, id));
        assert!(
            controller.state.session.list_setups()[0]
                .obstacles
                .is_empty()
        );
        assert!(controller.state.panels.obstacle_editor.is_none());
        assert_eq!(controller.state.selection, Selection::Setup(setup));
        assert!(controller.state.gui.dirty);
    }

    #[test]
    fn a_refusal_reaches_the_obstacle_editor_in_the_core_words_k2() {
        let mut controller = sample_controller();
        let setup = SetupId(controller.state.session.list_setups()[0].id);
        let outline = add_outline_model(&mut controller);
        controller.handle_internal_event(AppEvent::AddObstacle(setup));
        let id = ObstacleId(0);
        let before = stored(&controller, id).expect("added");
        controller.state.gui.dirty = false;

        // A model kind on the polygon-only model.
        let mut draft = before.clone();
        draft.geometry = ObstacleGeometry::Model { model_id: outline };
        let refusal = apply_obstacle_draft(&mut controller.state, 0, &before, draft)
            .expect_err("a model obstacle on a 2D model is refused");
        assert!(
            refusal.contains("a model obstacle needs a mesh"),
            "the core words: {refusal}"
        );
        assert_eq!(stored(&controller, id), Some(before.clone()));
        assert!(!controller.state.gui.dirty);

        // Non-vacuity: a different rule gives a different text.
        let mut empty = before.clone();
        empty.geometry = ObstacleGeometry::Box {
            min: [5.0, 0.0, 0.0],
            max: [5.0, 10.0, 10.0],
        };
        let refusal = apply_obstacle_draft(&mut controller.state, 0, &before, empty)
            .expect_err("an empty box is refused");
        assert!(
            refusal.contains("min below max on the x axis"),
            "the core words: {refusal}"
        );
        assert!(!controller.state.gui.dirty);
    }

    #[test]
    fn an_unchanged_draft_applies_nothing_k2() {
        let mut controller = sample_controller();
        let setup = SetupId(controller.state.session.list_setups()[0].id);
        controller.handle_internal_event(AppEvent::AddObstacle(setup));
        let before = stored(&controller, ObstacleId(0)).expect("added");
        generate_all_for_test(&mut controller);
        let tp_id = controller.state.session.toolpath_configs()[0].id;
        // Drop the add's undo entry, so the history reads only this apply.
        while controller.state.history.undo().is_some() {}
        controller.state.gui.dirty = false;

        apply_obstacle_draft(&mut controller.state, 0, &before, before.clone())
            .expect("an unchanged draft is not a refusal");
        // An editor draft with another enabled flag is still unchanged:
        // the row checkbox owns that field.
        let mut flipped = before.clone();
        flipped.enabled = !before.enabled;
        apply_obstacle_draft(&mut controller.state, 0, &before, flipped)
            .expect("the enabled flag is not the editor's field");

        assert!(!controller.state.gui.dirty);
        assert!(
            controller.state.gui.toolpath_rt[&tp_id]
                .stale_since
                .is_none(),
            "no stale stamp"
        );
        assert!(controller.state.history.undo().is_none(), "no undo entry");
    }
}

// H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the GUI head
// editor, the toolpath shoe row and the run sheet.
//
// - A head edit goes through `Command::SetMachineHead` with one undo step;
//   a refusal comes back in core's words and writes nothing.
// - A shoe edit goes through `Command::SetToolpathShoe` with one undo step
//   per edit, and one per drag of the gap.
// - The run sheet and the toolpath panel read the plan that
//   `inspect_machine` prints as `shoe_plan` (the surface number parity
//   rule).
mod machine_head_h2 {
    use super::*;
    use rs_cam_core::machine::head::{HeadPart, MachineHead, PartKind, ShoeSetting};
    use rs_cam_core::session::{ReplaceToolsArgs, SetToolpathShoeArgs, SetToolpathToolArgs};

    /// The sample project with a second tool and three toolpaths: two on
    /// tool 1, then one on tool 2. Toolpath 1 states a shoe gap of 10 mm, so
    /// it is a run of its own and its gap is below the solver gap. Every
    /// toolpath has a result.
    fn shoe_controller() -> AppController<ScriptedBackend> {
        let mut controller = sample_controller();
        let mut second = ToolConfig::new_default(ToolId(2), ToolType::BallNose);
        second.name = "Second tool".to_owned();
        let mut tools = controller.state.session.tools().to_vec();
        tools.push(second);
        let _ = controller
            .state
            .session
            .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
            .unwrap();
        let _ = push_toolpath(&mut controller, "Second");
        let _ = push_toolpath(&mut controller, "Third");
        let _ = controller
            .state
            .session
            .apply(Command::SetToolpathTool(SetToolpathToolArgs {
                index: 2,
                tool_id: 2,
            }))
            .unwrap();
        let _ = controller
            .state
            .session
            .apply(Command::SetToolpathShoe(SetToolpathShoeArgs {
                index: 1,
                shoe: ShoeSetting::Gap { gap_mm: 10.0 },
            }))
            .unwrap();
        crate::ui::properties::apply_head(&mut controller.state, MachineHead::dust_shoe()).unwrap();
        generate_all_for_test(&mut controller);
        controller
    }

    #[test]
    fn a_head_preset_applies_and_undoes_through_set_machine_head_h2() {
        let mut controller = sample_controller();
        let before = controller.state.session.machine().head.clone();
        let steps = controller.state.history.undo_len();
        crate::ui::properties::apply_head(&mut controller.state, MachineHead::dust_shoe()).unwrap();
        assert_eq!(
            controller.state.session.machine().head,
            MachineHead::dust_shoe()
        );
        assert_eq!(
            controller.state.history.undo_len(),
            steps + 1,
            "one undo step"
        );
        assert!(
            controller.state.gui.dirty,
            "the door marks the project edited"
        );

        // An equal head applies nothing and pushes nothing.
        crate::ui::properties::apply_head(&mut controller.state, MachineHead::dust_shoe()).unwrap();
        assert_eq!(controller.state.history.undo_len(), steps + 1);

        controller.undo();
        assert_eq!(controller.state.session.machine().head, before);
        controller.redo();
        assert_eq!(
            controller.state.session.machine().head,
            MachineHead::dust_shoe()
        );
    }

    #[test]
    fn a_refused_head_shows_core_words_and_writes_nothing_h2() {
        let mut controller = sample_controller();
        let before = controller.state.session.machine().head.clone();
        let steps = controller.state.history.undo_len();
        let mut head = MachineHead::bare_collet();
        head.parts.push(HeadPart {
            name: "Lid".to_owned(),
            diameter_mm: Some(10.0),
            height_mm: 5.0,
            kind: PartKind::Hard,
        });
        let refusal = head.validate().unwrap_err().to_string();
        let error = crate::ui::properties::apply_head(&mut controller.state, head).unwrap_err();
        assert!(
            error.contains(&refusal),
            "the banner text carries core's words: {error}"
        );
        assert_eq!(controller.state.session.machine().head, before);
        assert_eq!(controller.state.history.undo_len(), steps);
    }

    #[test]
    fn the_shoe_row_is_one_command_and_one_undo_step_per_edit_h2() {
        let mut controller = shoe_controller();
        let tp_id = controller.state.session.toolpath_configs()[0].id;
        let shoe = |c: &AppController<ScriptedBackend>| c.state.session.toolpath_configs()[0].shoe;
        let steps = controller.state.history.undo_len();
        let event =
            |shoe: ShoeSetting, merge: bool| AppEvent::SetToolpathShoe { tp_id, shoe, merge };

        controller.handle_internal_event(event(ShoeSetting::Raised, false));
        assert_eq!(shoe(&controller), ShoeSetting::Raised);
        assert_eq!(controller.state.history.undo_len(), steps + 1);

        // A gap drag: the first frame pushes, the later frames join it.
        controller.handle_internal_event(event(ShoeSetting::Gap { gap_mm: 30.0 }, false));
        controller.handle_internal_event(event(ShoeSetting::Gap { gap_mm: 31.0 }, true));
        controller.handle_internal_event(event(ShoeSetting::Gap { gap_mm: 32.0 }, true));
        assert_eq!(shoe(&controller), ShoeSetting::Gap { gap_mm: 32.0 });
        assert_eq!(controller.state.history.undo_len(), steps + 2);

        // A bad gap is refused and pushes nothing.
        controller.handle_internal_event(event(ShoeSetting::Gap { gap_mm: -1.0 }, false));
        assert_eq!(shoe(&controller), ShoeSetting::Gap { gap_mm: 32.0 });
        assert_eq!(controller.state.history.undo_len(), steps + 2);

        controller.undo();
        assert_eq!(shoe(&controller), ShoeSetting::Raised);
        controller.undo();
        assert_eq!(shoe(&controller), ShoeSetting::Auto);
        controller.redo();
        assert_eq!(shoe(&controller), ShoeSetting::Raised);
    }

    /// The surface number parity rule: the run sheet, the toolpath panel and
    /// `inspect_machine`'s `shoe_plan` read one plan and print one text.
    #[test]
    fn the_run_sheet_and_the_panel_match_inspect_machine_h2() {
        let controller = shoe_controller();
        let session = &controller.state.session;
        let wire = crate::app::mcp::project::shoe_plan_wire(session);
        let wire_rows = wire.as_array().expect("the plan builds");
        assert_eq!(wire_rows.len(), 3, "every toolpath has a result: {wire}");

        let plan = controller.state.gui.shoe_plan.plan(session);
        let lines = crate::state::shoe_plan::run_lines(session, plan.rows());
        let starts: Vec<&serde_json::Value> = wire_rows
            .iter()
            .filter(|row| row["run_start"] == serde_json::json!(true))
            .collect();
        assert_eq!(
            lines.len(),
            starts.len(),
            "one line per run: tool 1 Auto, tool 1 Gap, tool 2 Auto"
        );
        assert_eq!(lines.len(), 3);
        for (line, start) in lines.iter().zip(&starts) {
            assert_eq!(serde_json::json!(line.tool_id), start["tool_id"]);
            assert_eq!(serde_json::json!(line.toolpaths[0]), start["toolpath"]);
            assert_eq!(
                serde_json::json!(crate::ui::shoe_list::line_value(line)),
                start["resolved"],
                "the run sheet prints the MCP resolved text"
            );
            assert_eq!(
                serde_json::json!(line.needed_gap_mm),
                start["needed_gap_mm"]
            );
            assert_eq!(serde_json::json!(line.gap_too_low), start["gap_too_low"]);
        }
        assert_eq!(
            lines.iter().map(|line| line.toolpaths.len()).sum::<usize>(),
            wire_rows.len()
        );
        assert!(
            lines[1].gap_too_low,
            "the stated 10 mm is below the solver gap"
        );

        // The panel's Auto text is the MCP resolved text.
        for (index, row) in wire_rows.iter().enumerate() {
            let id = session.toolpath_configs()[index].id;
            let inputs = crate::ui::properties::toolpath_panel_inputs(
                id,
                session,
                &controller.state.gui,
                None,
                None,
                false,
            );
            let shoe = inputs.shoe.expect("the head has a shoe");
            if row["setting"]["mode"] == serde_json::json!("auto") {
                let resolved = row["resolved"].as_str().unwrap();
                assert_eq!(shoe.auto_text, format!("Auto: {resolved}"));
            }
            assert_eq!(
                shoe.finding.is_some(),
                row["gap_too_low"] == serde_json::json!(true),
                "the footer finding shows where the wire says the gap is too low"
            );
        }

        // The finding text is the project verdict's text.
        let id = session.toolpath_configs()[1].id;
        let inputs = crate::ui::properties::toolpath_panel_inputs(
            id,
            session,
            &controller.state.gui,
            None,
            None,
            false,
        );
        let message = inputs.shoe.unwrap().finding.unwrap().message;
        let verdict = plan.row(1).unwrap().gap_too_low_verdict().unwrap();
        assert!(message.starts_with(&verdict.headline), "{message}");
    }

    #[test]
    fn the_memo_rebuilds_when_an_input_moves_h2() {
        let mut controller = shoe_controller();
        let first = controller
            .state
            .gui
            .shoe_plan
            .plan(&controller.state.session);
        let again = controller
            .state
            .gui
            .shoe_plan
            .plan(&controller.state.session);
        assert!(
            Arc::ptr_eq(&first, &again),
            "an unchanged project hits the memo"
        );

        let tp_id = controller.state.session.toolpath_configs()[0].id;
        controller.handle_internal_event(AppEvent::SetToolpathShoe {
            tp_id,
            shoe: ShoeSetting::Off,
            merge: false,
        });
        let after = controller
            .state
            .gui
            .shoe_plan
            .plan(&controller.state.session);
        assert!(!Arc::ptr_eq(&first, &after));
        assert_eq!(
            after.row(0).map(|row| row.resolved.label()),
            Some(Some("remove".to_owned()))
        );
    }
}
