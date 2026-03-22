use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{CentralPanel, Context, FontDefinitions, SidePanel, TopBottomPanel, Window};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::toolpath::Toolpath;

use super::*;
use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, LaneState, SimulationRequest,
    SimulationResult,
};
use crate::state::job::{
    LoadedModel, ModelId, ModelKind, ModelUnits, Setup, ToolConfig, ToolId, ToolType,
};
use crate::state::selection::Selection;
use crate::state::toolpath::{
    Adaptive3dConfig, OperationConfig, ToolpathEntry, ToolpathId, ToolpathResult,
};

struct ScriptedBackend {
    toolpath_lane: LaneSnapshot,
    analysis_lane: LaneSnapshot,
    drained: Vec<ComputeMessage>,
}

impl ScriptedBackend {
    fn new() -> Self {
        Self {
            toolpath_lane: LaneSnapshot::idle(ComputeLane::Toolpath),
            analysis_lane: LaneSnapshot::idle(ComputeLane::Analysis),
            drained: Vec::new(),
        }
    }
}

impl ComputeBackend for ScriptedBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) {}
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.state = LaneState::Cancelling,
            ComputeLane::Analysis => self.analysis_lane.state = LaneState::Cancelling,
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.clone(),
            ComputeLane::Analysis => self.analysis_lane.clone(),
        }
    }
}

fn temp_path(name: &str, extension: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rs_cam_{name}_{nanos}.{extension}"))
}

#[test]
fn inspect_toolpath_in_simulation_queues_workspace_switch_and_jump_when_results_exist() {
    let mut controller = sample_controller();
    controller.state.simulation.results = Some(crate::state::simulation::SimulationResults {
        mesh: rs_cam_core::simulation::HeightmapMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 12,
        boundaries: vec![crate::state::simulation::ToolpathBoundary {
            id: ToolpathId(1),
            name: "Adaptive 3D".to_string(),
            tool_name: "Tool".to_string(),
            start_move: 4,
            end_move: 12,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        setup_boundaries: vec![crate::state::simulation::SetupBoundary {
            setup_id: crate::state::job::SetupId(1),
            setup_name: "Setup 1".to_string(),
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
    });

    controller.handle_internal_event(crate::ui::AppEvent::InspectToolpathInSimulation(
        ToolpathId(1),
    ));
    let events = controller.drain_events();

    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::SwitchWorkspace(crate::state::Workspace::Simulation)
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, crate::ui::AppEvent::SimJumpToMove(4)))
    );
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

    controller.handle_internal_event(crate::ui::AppEvent::InspectToolpathInSimulation(
        ToolpathId(1),
    ));
    let events = controller.drain_events();

    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::SwitchWorkspace(crate::state::Workspace::Simulation)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::RunSimulationWith(ids) if ids == &vec![ToolpathId(1)]
    )));
    assert_eq!(
        controller.state.simulation.debug.pending_inspect_toolpath,
        Some(ToolpathId(1))
    );
}

#[test]
fn simulation_results_land_on_pending_inspect_toolpath_start() {
    let mut controller = sample_controller();
    controller.state.simulation.debug.pending_inspect_toolpath = Some(ToolpathId(1));
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(SimulationResult {
            mesh: rs_cam_core::simulation::HeightmapMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 8,
            deviations: None,
            boundaries: vec![crate::compute::worker::SimBoundary {
                id: ToolpathId(1),
                name: "Adaptive 3D".to_string(),
                tool_name: "Tool".to_string(),
                start_move: 2,
                end_move: 8,
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
            }],
            checkpoints: Vec::new(),
            playback_data: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            cut_trace_path: None,
        })));

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

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn sample_controller() -> AppController<ScriptedBackend> {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    controller.state.job.tools.push(tool);

    let mesh = make_test_flat(40.0);
    controller.state.job.models.push(LoadedModel {
        id: ModelId(1),
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_string(),
        kind: ModelKind::Stl,
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    });

    let mut entry = ToolpathEntry::from_init(
        crate::state::toolpath::ToolpathEntryInit::from_loaded_state(
            ToolpathId(1),
            "Adaptive 3D".to_string(),
            ToolId(1),
            ModelId(1),
            OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        ),
    );
    entry.result = Some(ToolpathResult {
        toolpath: Arc::new(Toolpath::new()),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
    });
    controller.state.job.push_toolpath(entry);
    controller.state.selection = Selection::Toolpath(ToolpathId(1));
    controller
}

fn render_snapshot(
    controller: &mut AppController<ScriptedBackend>,
) -> crate::ui::automation::UiAutomationSnapshot {
    let ctx = Context::default();
    ctx.set_fonts(FontDefinitions::empty());
    let _ = ctx.run(Default::default(), |ctx| {
        crate::ui::automation::begin_frame(ctx);

        SidePanel::right("properties").show(ctx, |ui| {
            let events = &mut controller.events;
            crate::ui::properties::draw(ui, &mut controller.state, events);
        });

        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let lanes = controller.lane_snapshots();
            let collision_count = controller.collision_positions.len();
            crate::ui::status_bar::draw(ui, &controller.state, collision_count, &lanes);
        });

        CentralPanel::default().show(ctx, |ui| {
            let lanes = controller.lane_snapshots();
            let events = &mut controller.events;
            crate::ui::viewport_overlay::draw(
                ui,
                controller.state.workspace,
                controller.state.simulation.has_results(),
                &mut controller.state.viewport,
                &lanes,
                events,
            );
        });

        if controller.show_load_warnings() {
            let mut open = true;
            Window::new("Project Load Warnings")
                .open(&mut open)
                .show(ctx, |ui| {
                    let response =
                        ui.label("The project loaded, but some references need attention:");
                    crate::ui::automation::record(
                        ui,
                        "project_load_warnings",
                        &response,
                        "Project Load Warnings",
                    );
                });
        }
    });
    crate::ui::automation::snapshot(&ctx)
}

#[test]
fn ui_harness_records_lane_status_overlay_and_stock_to_leave() {
    let mut controller = sample_controller();
    controller.compute.toolpath_lane = LaneSnapshot {
        lane: ComputeLane::Toolpath,
        state: LaneState::Running,
        queue_depth: 1,
        current_job: Some("Adaptive 3D".to_string()),
        current_phase: Some("Pass 12".to_string()),
        started_at: Some(std::time::Instant::now()),
    };
    controller.compute.analysis_lane = LaneSnapshot {
        lane: ComputeLane::Analysis,
        state: LaneState::Queued,
        queue_depth: 1,
        current_job: Some("Simulation".to_string()),
        current_phase: None,
        started_at: None,
    };

    let snapshot = render_snapshot(&mut controller);
    assert!(snapshot.widgets.contains_key("status_lane_toolpath"));
    assert!(snapshot.widgets.contains_key("status_lane_analysis"));
    assert!(snapshot.widgets.contains_key("overlay_cancel_all"));
    assert!(snapshot.widgets.contains_key("overlay_collision_check"));
    assert!(snapshot.widgets.contains_key("properties_stock_to_leave"));
}

#[test]
fn load_warning_window_can_be_shown_and_dismissed() {
    let mut controller = sample_controller();
    let project_path = fixture_path("missing_model_project.toml");

    controller
        .open_job_from_path(&project_path)
        .expect("open project with missing model");
    let shown = render_snapshot(&mut controller);
    assert!(shown.widgets.contains_key("project_load_warnings"));

    controller.set_show_load_warnings(false);
    let hidden = render_snapshot(&mut controller);
    assert!(!hidden.widgets.contains_key("project_load_warnings"));
}

#[test]
fn fixture_projects_load_2d_and_3d_models() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());

    controller
        .open_job_from_path(&fixture_path("sample_2d_project.toml"))
        .expect("open 2d fixture");
    assert!(controller.load_warnings().is_empty());
    assert_eq!(controller.state.job.models.len(), 1);
    assert!(controller.state.job.models[0].polygons.is_some());
    assert!(controller.state.job.models[0].mesh.is_none());

    controller
        .open_job_from_path(&fixture_path("sample_3d_project.toml"))
        .expect("open 3d fixture");
    assert!(controller.load_warnings().is_empty());
    assert_eq!(controller.state.job.models.len(), 1);
    assert!(controller.state.job.models[0].mesh.is_some());
    assert!(controller.state.job.models[0].polygons.is_none());
}

#[test]
fn controller_save_open_and_export_smoke() {
    let mut controller = sample_controller();
    controller.state.job.name = "Smoke".to_string();
    controller
        .state
        .job
        .find_toolpath_mut(ToolpathId(1))
        .unwrap()
        .result = Some(ToolpathResult {
        toolpath: Arc::new({
            let mut toolpath = Toolpath::new();
            toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
            toolpath.feed_to(P3::new(5.0, 5.0, -1.0), 500.0);
            toolpath
        }),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
    });

    let gcode = controller.export_gcode().expect("export gcode");
    assert!(gcode.contains("G"));

    let svg = controller.export_svg_preview().expect("export svg preview");
    assert!(svg.contains("<svg"));

    let setup = controller.export_setup_sheet_html();
    assert!(setup.contains("<html"));

    let save_path = temp_path("controller_save", "toml");
    controller
        .save_job_to_path(&save_path)
        .expect("save job through controller");
    controller
        .open_job_from_path(&save_path)
        .expect("reopen saved job through controller");

    assert!(controller.export_gcode().is_err());
    assert!(controller.export_svg_preview().is_err());

    let setup = controller.export_setup_sheet_html();
    assert!(setup.contains("<html"));
}

#[test]
fn simulation_results_capture_setup_boundaries() {
    let mut controller = sample_controller();

    let second_setup_id = controller.state.job.next_setup_id();
    controller
        .state
        .job
        .setups
        .push(Setup::new(second_setup_id, "Bottom Side".to_string()));
    controller.state.job.push_toolpath_to_setup(
        second_setup_id,
        ToolpathEntry::from_init(
            crate::state::toolpath::ToolpathEntryInit::from_loaded_state(
                ToolpathId(2),
                "Profile".to_string(),
                ToolId(1),
                ModelId(1),
                OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            ),
        ),
    );

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(
            crate::compute::SimulationResult {
                mesh: rs_cam_core::simulation::HeightmapMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    colors: Vec::new(),
                },
                total_moves: 20,
                deviations: None,
                boundaries: vec![
                    crate::compute::worker::SimBoundary {
                        id: ToolpathId(1),
                        name: "Adaptive 3D".to_string(),
                        tool_name: "End Mill".to_string(),
                        start_move: 0,
                        end_move: 10,
                        direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    },
                    crate::compute::worker::SimBoundary {
                        id: ToolpathId(2),
                        name: "Profile".to_string(),
                        tool_name: "End Mill".to_string(),
                        start_move: 10,
                        end_move: 20,
                        direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    },
                ],
                checkpoints: Vec::new(),
                playback_data: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: None,
                cut_trace_path: None,
            },
        )));

    controller.drain_compute_results();

    assert_eq!(controller.state.simulation.setup_boundaries().len(), 2);
    assert_eq!(
        controller.state.simulation.setup_boundaries()[0].setup_name,
        "Setup 1"
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries()[1].setup_name,
        "Bottom Side"
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries()[0].start_move,
        0
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries()[1].start_move,
        10
    );
}

// ---------------------------------------------------------------------------
// Workspace behavior tests (Phase 7)
// ---------------------------------------------------------------------------

#[test]
fn workspace_defaults_to_toolpaths() {
    let controller = AppController::with_backend(ScriptedBackend::new());
    assert_eq!(
        controller.state.workspace,
        crate::state::Workspace::Toolpaths
    );
}

#[test]
fn workspace_switch_preserves_simulation_results() {
    let mut controller = sample_controller();

    // Inject simulation results
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());

    // Switch to Toolpaths
    controller.state.workspace = crate::state::Workspace::Toolpaths;
    assert!(
        controller.state.simulation.has_results(),
        "Simulation results should persist when leaving Simulation workspace"
    );

    // Switch to Setup
    controller.state.workspace = crate::state::Workspace::Setup;
    assert!(
        controller.state.simulation.has_results(),
        "Simulation results should persist in Setup workspace"
    );
}

#[test]
fn reset_simulation_clears_results_and_checks() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());

    controller.handle_internal_event(crate::ui::AppEvent::ResetSimulation);

    assert!(
        !controller.state.simulation.has_results(),
        "Reset should clear results"
    );
    assert!(
        controller
            .state
            .simulation
            .checks
            .rapid_collisions
            .is_empty(),
        "Reset should clear rapid collisions"
    );
    assert_eq!(
        controller.state.simulation.checks.holder_collision_count, 0,
        "Reset should clear holder collision count"
    );
    assert!(
        controller.state.simulation.last_run.is_none(),
        "Reset should clear run metadata"
    );
}

#[test]
fn simulation_staleness_tracks_edits() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);

    // Should not be stale immediately
    assert!(
        !controller
            .state
            .simulation
            .is_stale(controller.state.job.edit_counter),
        "Fresh simulation should not be stale"
    );

    // Mark an edit
    controller.state.job.mark_edited();

    assert!(
        controller
            .state
            .simulation
            .is_stale(controller.state.job.edit_counter),
        "Simulation should be stale after job edit"
    );
}

#[test]
fn playback_defaults_after_reset() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);

    // Mutate playback
    controller.state.simulation.playback.current_move = 5;
    controller.state.simulation.playback.playing = true;
    controller.state.simulation.playback.speed = 9999.0;

    controller.handle_internal_event(crate::ui::AppEvent::ResetSimulation);

    assert_eq!(controller.state.simulation.playback.current_move, 0);
    assert!(!controller.state.simulation.playback.playing);
    assert!((controller.state.simulation.playback.speed - 500.0).abs() < f32::EPSILON);
}

/// Helper: inject minimal simulation results into the controller.
fn inject_sim_results(controller: &mut AppController<ScriptedBackend>, num_setups: usize) {
    use rs_cam_core::simulation::HeightmapMesh;

    let mesh = HeightmapMesh {
        vertices: vec![0.0; 9],
        indices: vec![0, 1, 2],
        colors: vec![0.5; 9],
    };

    let total_moves = 10 * num_setups;
    let mut boundaries = Vec::new();
    for i in 0..num_setups {
        boundaries.push(crate::compute::worker::SimBoundary {
            id: ToolpathId(1),
            name: format!("Op {}", i + 1),
            tool_name: "EndMill".to_string(),
            start_move: i * 10,
            end_move: (i + 1) * 10,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        });
    }

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(SimulationResult {
            mesh,
            total_moves,
            deviations: None,
            boundaries,
            checkpoints: Vec::new(),
            playback_data: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            cut_trace_path: None,
        })));

    controller.drain_compute_results();
}

#[test]
fn toolpath_results_persist_debug_trace_metadata() {
    let mut controller = sample_controller();
    let recorder = rs_cam_core::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
    let ctx = recorder.root_context();
    let span = ctx.start_span("core_generate", "Generate");
    span.finish();
    let trace = Arc::new(recorder.finish());
    let semantic_recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("Adaptive 3D", "3D Rough");
    let semantic_root = semantic_recorder.root_context();
    let pass = semantic_root.start_item(
        rs_cam_core::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    let toolpath = Arc::new(Toolpath::new());
    pass.bind_to_toolpath(toolpath.as_ref(), 0, 0);
    let semantic_trace = Arc::new(semantic_recorder.finish());
    let debug_path = temp_path("toolpath_trace_metadata", "json");

    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(1),
            result: Ok(ToolpathResult {
                toolpath: Arc::clone(&toolpath),
                stats: Default::default(),
                debug_trace: Some(Arc::clone(&trace)),
                semantic_trace: Some(Arc::clone(&semantic_trace)),
                debug_trace_path: Some(debug_path.clone()),
            }),
            debug_trace: Some(Arc::clone(&trace)),
            semantic_trace: Some(Arc::clone(&semantic_trace)),
            debug_trace_path: Some(debug_path.clone()),
        },
    ));

    controller.drain_compute_results();

    let entry = controller
        .state
        .job
        .find_toolpath(ToolpathId(1))
        .expect("toolpath should exist");
    let result = entry.result.as_ref().expect("result should be stored");
    let stored_trace = result
        .debug_trace
        .as_ref()
        .expect("debug trace should be preserved");
    assert_eq!(stored_trace.summary.span_count, trace.summary.span_count);
    assert_eq!(
        result
            .semantic_trace
            .as_ref()
            .map(|trace| trace.summary.item_count),
        Some(1)
    );
    assert_eq!(
        entry
            .semantic_trace
            .as_ref()
            .map(|trace| trace.summary.item_count),
        Some(1)
    );
    assert_eq!(
        entry
            .debug_trace
            .as_ref()
            .map(|trace| trace.summary.span_count),
        Some(1)
    );
    assert_eq!(result.debug_trace_path.as_ref(), Some(&debug_path));
    assert_eq!(entry.debug_trace_path.as_ref(), Some(&debug_path));
}

#[test]
fn cancelled_toolpath_preserves_debug_trace_metadata() {
    let mut controller = sample_controller();
    let recorder = rs_cam_core::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
    let ctx = recorder.root_context();
    let span = ctx.start_span("adaptive_pass", "Pass 1");
    span.finish();
    let trace = Arc::new(recorder.finish());
    let semantic_recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("Adaptive 3D", "3D Rough");
    let semantic_root = semantic_recorder.root_context();
    let pass = semantic_root.start_item(
        rs_cam_core::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    let toolpath = Toolpath::new();
    pass.bind_to_toolpath(&toolpath, 0, 0);
    let semantic_trace = Arc::new(semantic_recorder.finish());
    let debug_path = temp_path("cancelled_toolpath_trace_metadata", "json");

    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(1),
            result: Err(crate::compute::ComputeError::Cancelled),
            debug_trace: Some(Arc::clone(&trace)),
            semantic_trace: Some(Arc::clone(&semantic_trace)),
            debug_trace_path: Some(debug_path.clone()),
        },
    ));

    controller.drain_compute_results();

    let entry = controller
        .state
        .job
        .find_toolpath(ToolpathId(1))
        .expect("toolpath should exist");
    assert!(matches!(
        entry.status,
        crate::state::toolpath::ComputeStatus::Pending
    ));
    assert!(entry.result.is_none());
    assert_eq!(
        entry
            .debug_trace
            .as_ref()
            .map(|trace| trace.summary.span_count),
        Some(1)
    );
    assert_eq!(
        entry
            .semantic_trace
            .as_ref()
            .map(|trace| trace.summary.item_count),
        Some(1)
    );
    assert_eq!(entry.debug_trace_path.as_ref(), Some(&debug_path));
}

// ---------------------------------------------------------------------------
// Tool deletion safety tests (C5)
// ---------------------------------------------------------------------------

#[test]
fn remove_tool_blocked_when_toolpath_references_it() {
    let mut controller = sample_controller();
    let tool_count_before = controller.state.job.tools.len();
    assert_eq!(tool_count_before, 1);

    // ToolId(1) is referenced by the sample toolpath — deletion must be blocked.
    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(ToolId(1)));
    assert_eq!(
        controller.state.job.tools.len(),
        tool_count_before,
        "Tool should not be removed while a toolpath references it"
    );
}

#[test]
fn remove_tool_succeeds_when_no_toolpath_references_it() {
    let mut controller = sample_controller();

    // Add a second tool that is not referenced by any toolpath.
    let unreferenced_id = ToolId(99);
    let extra_tool = ToolConfig::new_default(unreferenced_id, ToolType::EndMill);
    controller.state.job.tools.push(extra_tool);
    let tool_count_before = controller.state.job.tools.len();

    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(unreferenced_id));
    assert_eq!(
        controller.state.job.tools.len(),
        tool_count_before - 1,
        "Unreferenced tool should be removed"
    );
    assert!(
        controller
            .state
            .job
            .tools
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
    // No tools added — controller.state.job.tools is empty.
    assert!(controller.state.job.tools.is_empty());

    let tp_count_before: usize = controller
        .state
        .job
        .setups
        .iter()
        .map(|s| s.toolpaths.len())
        .sum();

    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Adaptive3d,
    ));

    let tp_count_after: usize = controller
        .state
        .job
        .setups
        .iter()
        .map(|s| s.toolpaths.len())
        .sum();
    assert_eq!(
        tp_count_before, tp_count_after,
        "No toolpath should be created when no tools exist"
    );
}
