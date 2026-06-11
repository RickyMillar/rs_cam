use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{CentralPanel, Context, FontDefinitions, Panel, Window};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::toolpath::Toolpath;

use super::*;
use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, LaneState, OptimizeRequest,
    SimulationRequest, SimulationResult,
};
use crate::state::job::{SetupId, ToolConfig, ToolId, ToolType};
use crate::state::runtime::ToolpathRuntime;
use crate::state::selection::Selection;
use crate::state::toolpath::{Adaptive3dConfig, OperationConfig, ToolpathId, ToolpathResult};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::session::{LoadedModel, ToolpathConfig};

struct ScriptedBackend {
    toolpath_lane: LaneSnapshot,
    analysis_lane: LaneSnapshot,
    optimize_lane: LaneSnapshot,
    drained: Vec<ComputeMessage>,
}

impl ScriptedBackend {
    fn new() -> Self {
        Self {
            toolpath_lane: LaneSnapshot::idle(ComputeLane::Toolpath),
            analysis_lane: LaneSnapshot::idle(ComputeLane::Analysis),
            optimize_lane: LaneSnapshot::idle(ComputeLane::Optimize),
            drained: Vec::new(),
        }
    }
}

impl ComputeBackend for ScriptedBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) {}
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.state = LaneState::Cancelling,
            ComputeLane::Analysis => self.analysis_lane.state = LaneState::Cancelling,
            ComputeLane::Optimize => self.optimize_lane.state = LaneState::Cancelling,
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.clone(),
            ComputeLane::Analysis => self.analysis_lane.clone(),
            ComputeLane::Optimize => self.optimize_lane.clone(),
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
        mesh: rs_cam_core::simulation::StockMesh {
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
    });

    controller.handle_internal_event(crate::ui::AppEvent::InspectToolpathInSimulation(
        ToolpathId(0),
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
        ToolpathId(0),
    ));
    let events = controller.drain_events();

    assert!(events.iter().any(|event| matches!(
        event,
        crate::ui::AppEvent::SwitchWorkspace(crate::state::Workspace::Simulation)
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
        .push(ComputeMessage::Simulation(Ok(SimulationResult {
            mesh: rs_cam_core::simulation::StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 8,
            deviations: None,
            boundaries: vec![crate::compute::worker::SimBoundary {
                id: ToolpathId(0),
                name: "Adaptive 3D".to_owned(),
                tool_name: "Tool".to_owned(),
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
            resolution_clamped: false,
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
    controller.state.session.tools_mut().push(tool);

    let mesh = Arc::new(make_test_flat(40.0));
    controller.state.session.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        polygons: None,
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Scallop".to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
    };
    controller.state.session.add_toolpath(0, tp_config).unwrap();
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
            Toolpath::new(),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    controller.state.gui.toolpath_rt.insert(tp_id, rt);

    controller.state.selection = Selection::Toolpath(tp_id);
    controller
}

fn render_snapshot(
    controller: &mut AppController<ScriptedBackend>,
) -> crate::ui::automation::UiAutomationSnapshot {
    let ctx = Context::default();
    ctx.set_fonts(FontDefinitions::empty());
    let _ = ctx.run_ui(Default::default(), |ui| {
        crate::ui::automation::begin_frame(ui.ctx());

        Panel::right("properties").show_inside(ui, |ui| {
            let events = &mut controller.events;
            crate::ui::properties::draw(ui, &mut controller.state, events);
        });

        Panel::bottom("status_bar").show_inside(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let collision_count = controller.collision_positions.len();
            crate::ui::status_bar::draw(ui, &controller.state, collision_count, &lanes);
        });

        CentralPanel::default().show_inside(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let events = &mut controller.events;
            crate::ui::viewport_overlay::draw(
                ui,
                controller.state.workspace,
                controller.state.simulation.has_results(),
                crate::render::camera::ProjectionMode::Perspective,
                None,
                &mut controller.state.viewport,
                &lanes,
                events,
            );
        });

        if controller.show_load_warnings() {
            let mut open = true;
            Window::new("Project Load Warnings")
                .open(&mut open)
                .show(ui.ctx(), |ui| {
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
        current_job: Some("Adaptive 3D".to_owned()),
        current_phase: Some("Pass 12".to_owned()),
        started_at: Some(std::time::Instant::now()),
    };
    controller.compute.analysis_lane = LaneSnapshot {
        lane: ComputeLane::Analysis,
        state: LaneState::Queued,
        queue_depth: 1,
        current_job: Some("Simulation".to_owned()),
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
    assert_eq!(controller.state.session.models().len(), 1);
    assert!(controller.state.session.models()[0].polygons.is_some());
    assert!(controller.state.session.models()[0].mesh.is_none());

    controller
        .open_job_from_path(&fixture_path("sample_3d_project.toml"))
        .expect("open 3d fixture");
    assert!(controller.load_warnings().is_empty());
    assert_eq!(controller.state.session.models().len(), 1);
    assert!(controller.state.session.models()[0].mesh.is_some());
    assert!(controller.state.session.models()[0].polygons.is_none());
}

#[test]
fn controller_save_open_and_export_smoke() {
    let mut controller = sample_controller();
    controller.state.session.set_name("Smoke".to_owned());
    controller
        .state
        .gui
        .toolpath_rt
        .get_mut(&ToolpathId(0))
        .unwrap()
        .result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new({
            let mut toolpath = Toolpath::new();
            toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
            toolpath.feed_to(P3::new(5.0, 5.0, -1.0), 500.0);
            toolpath
        })),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });

    // C1 (2026-06-11): the export gate now enforces the tool-load policy
    // on the viz path. This smoke test never simulates, so accept the
    // Unmodeled(SimulationRequired) verdicts like a user would.
    controller.state.gui.tool_load_overrides.accept_unmodeled = true;

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

    let setup_idx = controller.state.session.add_setup(
        "Bottom Side".to_owned(),
        rs_cam_core::compute::transform::FaceUp::default(),
    );
    let tp2_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Profile".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
    };
    controller
        .state
        .session
        .add_toolpath(setup_idx, tp2_config)
        .unwrap();
    let tp2_id = controller.state.session.toolpath_configs()[1].id;

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(
            crate::compute::SimulationResult {
                mesh: rs_cam_core::simulation::StockMesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    colors: Vec::new(),
                },
                total_moves: 20,
                deviations: None,
                boundaries: vec![
                    crate::compute::worker::SimBoundary {
                        id: ToolpathId(0),
                        name: "Adaptive 3D".to_owned(),
                        tool_name: "End Mill".to_owned(),
                        start_move: 0,
                        end_move: 10,
                        direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    },
                    crate::compute::worker::SimBoundary {
                        id: tp2_id,
                        name: "Profile".to_owned(),
                        tool_name: "End Mill".to_owned(),
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
                resolution_clamped: false,
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
            .is_stale(controller.state.gui.edit_counter),
        "Fresh simulation should not be stale"
    );

    // Mark an edit
    controller.state.gui.mark_edited();

    assert!(
        controller
            .state
            .simulation
            .is_stale(controller.state.gui.edit_counter),
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
    use rs_cam_core::stock_mesh::StockMesh;

    let mesh = StockMesh {
        vertices: vec![0.0; 9],
        indices: vec![0, 1, 2],
        colors: vec![0.5; 9],
    };

    let total_moves = 10 * num_setups;
    let mut boundaries = Vec::new();
    for i in 0..num_setups {
        boundaries.push(crate::compute::worker::SimBoundary {
            id: ToolpathId(0),
            name: format!("Op {}", i + 1),
            tool_name: "EndMill".to_owned(),
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
            resolution_clamped: false,
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
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    pass.bind_to_toolpath(&annotated.toolpath, 0, 0);
    let semantic_trace = Arc::new(semantic_recorder.finish());
    let debug_path = temp_path("toolpath_trace_metadata", "json");

    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Ok(ToolpathResult {
                annotated: Arc::clone(&annotated),
                stats: Default::default(),
                debug_trace: Some(Arc::clone(&trace)),
                semantic_trace: Some(Arc::clone(&semantic_trace)),
                debug_trace_path: Some(debug_path.clone()),
                drill_op: None,
            }),
            debug_trace: Some(Arc::clone(&trace)),
            semantic_trace: Some(Arc::clone(&semantic_trace)),
            debug_trace_path: Some(debug_path.clone()),
        },
    ));

    controller.drain_compute_results();

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&ToolpathId(0))
        .expect("toolpath runtime should exist");
    let result = rt.result.as_ref().expect("result should be stored");
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
        rt.semantic_trace
            .as_ref()
            .map(|trace| trace.summary.item_count),
        Some(1)
    );
    assert_eq!(
        rt.debug_trace
            .as_ref()
            .map(|trace| trace.summary.span_count),
        Some(1)
    );
    assert_eq!(result.debug_trace_path.as_ref(), Some(&debug_path));
    assert_eq!(rt.debug_trace_path.as_ref(), Some(&debug_path));
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
            toolpath_id: ToolpathId(0),
            result: Err(crate::compute::ComputeError::Cancelled),
            debug_trace: Some(Arc::clone(&trace)),
            semantic_trace: Some(Arc::clone(&semantic_trace)),
            debug_trace_path: Some(debug_path.clone()),
        },
    ));

    controller.drain_compute_results();

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&ToolpathId(0))
        .expect("toolpath runtime should exist");
    assert!(matches!(
        rt.status,
        crate::state::runtime::ComputeStatus::Pending
    ));
    assert!(rt.result.is_none());
    assert_eq!(
        rt.debug_trace
            .as_ref()
            .map(|trace| trace.summary.span_count),
        Some(1)
    );
    assert_eq!(
        rt.semantic_trace
            .as_ref()
            .map(|trace| trace.summary.item_count),
        Some(1)
    );
    assert_eq!(rt.debug_trace_path.as_ref(), Some(&debug_path));
}

// ---------------------------------------------------------------------------
// Roadmap F.1 — session.results cache must repopulate when the threaded
// compute backend returns a fresh result. Before the fix, the callback
// only wrote to gui.toolpath_rt and session.results stayed empty after
// apply_toolpath_param_snapshot — breaking project_load_report's span
// lookup for the just-applied toolpath. See planning/F1_RCA.md.
// ---------------------------------------------------------------------------

#[test]
fn drain_compute_results_repopulates_session_results() {
    let mut controller = sample_controller();
    // sample_controller pre-seeds rt.result but leaves session.results
    // empty — exactly the post-Apply pre-fix divergence state.
    assert!(
        controller.state.session.get_result(0).is_none(),
        "fixture should start with empty session.results[0]"
    );

    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));

    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Ok(ToolpathResult {
                annotated: Arc::clone(&annotated),
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
    ));

    controller.drain_compute_results();

    // The fix: session.results[0] is populated.
    let session_result = controller
        .state
        .session
        .get_result(0)
        .expect("session.results[0] should be populated after drain");
    // Both caches share the same Arc (no duplicated allocation).
    assert!(Arc::ptr_eq(session_result.annotated(), &annotated));
}

#[test]
fn drain_compute_results_clears_pending_apply_resim_on_success() {
    // Roadmap F.2 — auto-verify after Apply. The drain handler must
    // clear `pending_apply_resim` and trigger the project sim when
    // the regen for the just-applied candidate lands. We assert the
    // pending flag is cleared; the actual sim submission is a side
    // effect on the backend (covered by integration of F.2's behavior
    // in the GUI).
    let mut controller = sample_controller();
    controller.state.pending_apply_resim = Some(0);
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Ok(ToolpathResult {
                annotated,
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
    ));

    controller.drain_compute_results();

    assert!(
        controller.state.pending_apply_resim.is_none(),
        "pending_apply_resim should be cleared when the matching regen lands"
    );
}

#[test]
fn drain_compute_results_keeps_pending_apply_resim_for_other_toolpath() {
    // If a regen lands for a different TP than the one awaiting
    // verification, the pending flag stays set — we only kick the
    // post-apply sim when the matching TP's regen finishes.
    let mut controller = sample_controller();
    controller.state.pending_apply_resim = Some(42);
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Ok(ToolpathResult {
                annotated,
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
    ));

    controller.drain_compute_results();

    assert_eq!(
        controller.state.pending_apply_resim,
        Some(42),
        "pending_apply_resim should remain set when an unrelated regen lands"
    );
}

#[test]
fn drain_compute_results_skips_session_write_on_error() {
    let mut controller = sample_controller();
    controller.compute.drained.push(ComputeMessage::Toolpath(
        crate::compute::worker::ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Err(crate::compute::ComputeError::Message("boom".into())),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        },
    ));

    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_none(),
        "session.results must not be written on compute error"
    );
}

// ---------------------------------------------------------------------------
// Tool deletion safety tests (C5)
// ---------------------------------------------------------------------------

#[test]
fn remove_tool_blocked_when_toolpath_references_it() {
    let mut controller = sample_controller();
    let tool_count_before = controller.state.session.tools().len();
    assert_eq!(tool_count_before, 1);

    // ToolId(1) is referenced by the sample toolpath — deletion must be blocked.
    controller.handle_internal_event(crate::ui::AppEvent::RemoveTool(ToolId(1)));
    assert_eq!(
        controller.state.session.tools().len(),
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
    controller.state.session.tools_mut().push(extra_tool);
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
fn add_setup_and_remove_setup_lifecycle() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    // Starts with one default setup
    assert_eq!(controller.state.session.list_setups().len(), 1);
    let original_setup_id = SetupId(controller.state.session.list_setups()[0].id);

    // Add a second setup
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    assert_eq!(controller.state.session.list_setups().len(), 2);
    let new_setup_id = SetupId(controller.state.session.list_setups()[1].id);
    assert_ne!(original_setup_id, new_setup_id);

    // Remove the second setup
    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(new_setup_id));
    assert_eq!(controller.state.session.list_setups().len(), 1);
    assert_eq!(
        SetupId(controller.state.session.list_setups()[0].id),
        original_setup_id
    );

    // Min 1 setup enforced: try to remove the last one
    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(original_setup_id));
    assert_eq!(
        controller.state.session.list_setups().len(),
        1,
        "Cannot remove the last setup — minimum 1 enforced"
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
        mesh: rs_cam_core::simulation::StockMesh {
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
    });

    controller.handle_internal_event(crate::ui::AppEvent::ResetSimulation);

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

#[test]
fn delete_setup_containing_selected_fixture_clears_selection() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());

    // Add a second setup so we can delete it (min 1 enforced)
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    assert_eq!(controller.state.session.list_setups().len(), 2);
    let second_setup_id = SetupId(controller.state.session.list_setups()[1].id);

    // Add a fixture to the second setup
    controller.handle_internal_event(crate::ui::AppEvent::AddFixture(second_setup_id));
    let fixture_id = controller.state.session.list_setups()[1].fixtures[0].id;

    // Select the fixture
    controller.handle_internal_event(crate::ui::AppEvent::Select(Selection::Fixture(
        second_setup_id,
        fixture_id,
    )));
    assert_eq!(
        controller.state.selection,
        Selection::Fixture(second_setup_id, fixture_id)
    );

    // Remove the setup containing the fixture
    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(second_setup_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared when setup containing selected fixture is deleted"
    );
}

#[test]
fn delete_selected_toolpath_clears_selection() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);
    controller.state.selection = Selection::Toolpath(tp_id);

    // Remove the selected toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared after deleting the selected toolpath"
    );
}

#[test]
fn delete_unselected_toolpath_preserves_selection() {
    let mut controller = sample_controller();

    // Add another toolpath
    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));
    let Selection::Toolpath(new_tp_id) = controller.state.selection else {
        panic!("Selection should be new toolpath");
    };

    // Select back to the original toolpath
    controller.state.selection = Selection::Toolpath(ToolpathId(0));

    // Delete the other toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(new_tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::Toolpath(ToolpathId(0)),
        "Selection should be preserved when a different toolpath is deleted"
    );
}

#[test]
fn delete_setup_with_selected_keep_out_clears_selection() {
    let mut controller = AppController::with_backend(ScriptedBackend::new());

    // Add a second setup
    controller.handle_internal_event(crate::ui::AppEvent::AddSetup);
    let second_setup_id = SetupId(controller.state.session.list_setups()[1].id);

    // Add a keep-out zone to the second setup
    controller.handle_internal_event(crate::ui::AppEvent::AddKeepOut(second_setup_id));
    let keep_out_id = controller.state.session.list_setups()[1].keep_out_zones[0].id;

    // Select the keep-out
    controller.handle_internal_event(crate::ui::AppEvent::Select(Selection::KeepOut(
        second_setup_id,
        keep_out_id,
    )));
    assert_eq!(
        controller.state.selection,
        Selection::KeepOut(second_setup_id, keep_out_id)
    );

    // Remove the setup
    controller.handle_internal_event(crate::ui::AppEvent::RemoveSetup(second_setup_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared when setup containing selected keep-out is deleted"
    );
}

/// F-024 (third-site fix, 2026-05-25): regression test that the controller's
/// world `stock_bbox` helper respects `StockConfig::origin_{x,y,z}`.
///
/// Pre-fix (rounds 03/04 evidence): `build_simulation_groups` constructed the
/// world `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
/// dropping the origin. For AS001 (`origin_z=-12`) this sent a bbox of
/// `(0,0,0)..(100,100,12)` to the worker even though the actual world stock
/// spans `(-10,-10,-12)..(90,90,0)`. The F-024 viz-worker follow-up
/// (commit `0c907a6`) made `build_core_simulation_request` forward
/// `local_stock_bbox = None` for identity setups so the core would fall back
/// to `request.stock_bbox` — but the fallback bbox itself was the same
/// broken bbox. Result: toolpath cuts at world Z=-2 still sat below every
/// dexel ray (which spanned the wrong Z=[0,12] instead of Z=[-12,0]),
/// `axial_engagement_mm` read the full ray length, and the deflection gate
/// stayed at ~374 µm — byte-identical to round-02.
///
/// Fix: route the world bbox construction through `ProjectSession::
/// stock_bbox()` (which delegates to `StockConfig::bbox()` and applies the
/// origin correctly). Exposed via the pure free function
/// `controller::events::simulation::build_world_stock_bbox` so this test can
/// exercise it without spinning up a full `AppController<B>`.
#[test]
fn build_world_stock_bbox_respects_stock_origin_f024() {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::session::ProjectSession;

    let mut session = ProjectSession::new_empty();
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let bbox: BoundingBox3 =
        crate::controller::events::simulation::build_world_stock_bbox(&session);

    assert!(
        (bbox.min.x - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.x must equal stock.origin_x; got {}",
        bbox.min.x
    );
    assert!(
        (bbox.min.y - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.y must equal stock.origin_y; got {}",
        bbox.min.y
    );
    assert!(
        (bbox.min.z - -12.0).abs() < 1e-9,
        "F-024: world stock bbox min.z must equal stock.origin_z (= -12 for \
         AS001); got {}. Pre-fix the controller built bbox.min.z = 0.0 and \
         the per-setup dexel grid spanned Z=[0, 12] instead of Z=[-12, 0], \
         so cuts at world Z=-2 sat below every ray and axial_engagement_mm \
         read the full stock height (12 mm) instead of the commanded DOC.",
        bbox.min.z
    );
    assert!(
        (bbox.max.x - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.x must equal origin_x + stock.x; got {}",
        bbox.max.x
    );
    assert!(
        (bbox.max.y - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.y must equal origin_y + stock.y; got {}",
        bbox.max.y
    );
    assert!(
        (bbox.max.z - 0.0).abs() < 1e-9,
        "F-024: world stock bbox max.z must equal origin_z + stock.z (= 0 for \
         AS001, stock top at world Z=0); got {}",
        bbox.max.z
    );
}

/// F-024 (third-site fix, 2026-05-25): end-to-end regression that the viz
/// worker simulation, when fed the controller-built world `stock_bbox`,
/// reports per-sample `axial_engagement_mm` matching the commanded DOC
/// rather than the full stock height.
///
/// This is the controller-path equivalent of
/// `compute::worker::tests::as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`
/// — that test built the world `stock_bbox` by hand. This test builds the
/// bbox via `build_world_stock_bbox(&session)`, so it fails (per-sample
/// peak axial = ~12 mm) if the controller-side helper ever regresses to the
/// pre-fix zero-rooted construction even if the worker-side fix is intact.
#[test]
fn controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024() {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::session::ProjectSession;
    use rs_cam_core::simulation_cut::CutKinematics;
    use std::sync::atomic::AtomicBool;

    use crate::compute::{
        SetupSimGroup, SetupSimToolpath, SimulationRequest as VizSimulationRequest,
    };

    let mut session = ProjectSession::new_empty();
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();

    // First-pass-of-pocket-style linear cutting at world Z=-2. Same shape as
    // the worker-tests F-024 regression so the assertion threshold stays
    // comparable; the only difference is that we use the controller helper
    // to build the world `stock_bbox`.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 30.0, 5.0));
    tp.feed_to(P3::new(10.0, 30.0, -2.0), 385.0);
    for i in 0..40 {
        let x = 10.0 + (i as f64) * 1.5;
        tp.feed_to(P3::new(x, 30.0, -2.0), 770.0);
    }
    tp.rapid_to(P3::new(70.0, 30.0, 5.0));

    // World bbox via the controller helper. With the fix this respects the
    // stock origin (Z=[-12, 0]); pre-fix it was Z=[0, 12].
    let world_stock_bbox = crate::controller::events::simulation::build_world_stock_bbox(&session);

    // Identity-setup local bbox shape from `controller::events::simulation`
    // (always zero-rooted via `xform.effective_stock_bbox()`).
    let local_stock_bbox = rs_cam_core::geo::BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(100.0, 100.0, 12.0),
    };

    let request = VizSimulationRequest {
        groups: vec![SetupSimGroup {
            toolpaths: vec![SetupSimToolpath {
                id: ToolpathId(1),
                name: "AS001 Pocket Pass 1".to_owned(),
                annotated: Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(tp)),
                tool,
                semantic_trace: None,
                spindle_rpm: Some(18_000),
                metrics_not_applicable: false,
                drill_op: None,
                operation_config_hash: 0,
            }],
            local_stock_bbox,
            local_to_global: None,
        }],
        stock_bbox: world_stock_bbox,
        stock_top_z: world_stock_bbox.max.z,
        resolution: 1.0,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
        model_mesh: None,
        kinematics: None,
        use_predicted_feed_in_gates: false,
        max_feed_mm_min: 5_000.0,
    };

    let cancel = AtomicBool::new(false);
    let result =
        crate::compute::worker::execute::run_simulation_with_phase(&request, &cancel, |_phase| {})
            .expect("viz simulation completes");

    let cut_trace = result.cut_trace.as_ref().expect("metric cut trace");

    let mut first_pass_axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics == CutKinematics::Linear)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    assert!(
        !first_pass_axials.is_empty(),
        "expected at least one linear cutting sample near Z=-2; got 0"
    );

    let peak = *first_pass_axials.last().unwrap_or(&0.0);
    assert!(
        peak <= 3.0,
        "F-024 (third site): first-pass axial engagement with controller-built \
         world bbox should be <= 3.0 mm (commanded 2.0 + grid discretisation \
         margin); got peak = {peak:.4} mm across {} samples. If this is ~12 mm, \
         the controller is dropping `stock.origin_z` when constructing the \
         world bbox passed to the worker — see \
         `controller::events::simulation::build_world_stock_bbox` and the \
         F-024 finding notes.",
        first_pass_axials.len()
    );
}

// ---------------------------------------------------------------------------
// F-028 viz-path follow-up (2026-05-25)
//
// The original F-028 fix (commit `e48d7df`) only patched
// `session::compute::compute_simulation_groups` (site 1). Round-07 smoke
// verified through the MCP exposed that the viz / MCP / GUI pipeline takes
// a different path: `AppController::submit_toolpath_compute`
// (`controller/events/compute.rs`). That path always built the
// `HeightContext` from the **local zero-rooted** bbox even for identity
// setups, and ALSO transformed mesh/polygons by `-stock.origin` into a
// setup-local frame. The toolpath generator then anchored its depth
// stepping at `heights.top_z = stock.z = 12` (for AS001) and emitted cuts
// at Z=10, 8, 6 in the setup-local frame.
//
// The downstream simulation path, however, drops `local_to_global = None`
// for identity setups (F-024 follow-up `0c907a6`) — so the dexel grid was
// rebuilt in *world* frame. The generator's local-frame cuts at Z=10 sat
// 10 mm above the world stock top at Z=0; the cutter swept through air
// the whole run. Round-07 MCP smoke evidence:
//   AS001 pocket: z_level = 10/8/6, peak_axial_doc_mm = 0,
//   total_removed_volume_est_mm3 = 0, chipload = Unmodeled
//   (all_samples_air_cut_or_rapid), air_cut_percentage = 96 %, 0 rapid
//   collisions, 0 sampling errors.
//
// The fix mirrors `session::compute::compute` (site 1): identity setups
// (`face_up = Top`, `z_rotation = Deg0`) skip the geometry transform and
// build the `HeightContext` + worker `stock_bbox` from the world bbox.
// The `transform_setup = Some(...)` branch is now gated on
// `s.needs_transform()` — identity setups fall through to the
// `transform_setup = None` branch and emit cuts in world frame to match
// the downstream sim path's world-frame dexel grid.
//
// Test design: a `CapturingBackend` records the `ComputeRequest`
// submitted by `submit_toolpath_compute` so we can pin the heights frame
// directly. Pre-fix the captured `heights.top_z = 12` and
// `stock_bbox.max.z = 12` (local). Post-fix both are 0 (world stock top
// for AS001).
// ---------------------------------------------------------------------------

/// Capturing backend that records the most recent toolpath `ComputeRequest`.
/// Used by the F-028 viz-path follow-up regression test to assert that
/// `submit_toolpath_compute` resolves heights in the world frame for
/// identity setups.
#[derive(Default)]
struct CapturingBackend {
    captured: Option<ComputeRequest>,
}

impl crate::compute::ComputeBackend for CapturingBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) {
        self.captured = Some(request);
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: crate::compute::ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: crate::compute::ComputeLane) -> crate::compute::LaneSnapshot {
        crate::compute::LaneSnapshot::idle(lane)
    }
}

/// F-028 viz-path follow-up regression test.
///
/// AS001-shape session (stock origin_z=-12, identity setup, auto_from_model
/// =false, world stock top at Z=0) with a Pocket toolpath on a 2D polygon
/// model. `submit_toolpath_compute` must build a `HeightContext` from the
/// **world** stock bbox, so `heights.top_z = 0.0` (world stock top) and the
/// downstream toolpath generator emits cuts at Z=-2, -4, -6 to match the
/// downstream sim path's world-frame dexel grid.
///
/// Pre-fix readings (commit `db1fb69`): `heights.top_z = 12.0`,
/// `stock_bbox.max.z = 12.0`, cuts emitted at Z=10, 8, 6 → simulation peak
/// axial = 0, total removed = 0, air cut = 96 %.
///
/// Post-fix: both are 0.0 (world stock top for AS001), cuts emit at
/// Z=-2, -4, -6, simulation engages stock.
#[test]
fn as001_pocket_heights_resolve_in_world_frame_for_identity_setup_f028() {
    use rs_cam_core::compute::PocketConfig;
    use rs_cam_core::compute::operation_configs::PocketPattern;
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::material::{Material, WoodSpecies};
    use rs_cam_core::polygon::Polygon2;

    let mut controller = AppController::with_backend(CapturingBackend::default());

    // AS001 stock: origin_z=-12, z=12 → world stock top at Z=0.
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    controller.state.session.set_stock_config(stock);

    // 6 mm endmill matching the AS001 fixture.
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    controller.state.session.tools_mut().push(tool);

    // 2D polygon model (matches the SVG-driven AS001 pocket case).
    let model_id = controller.state.session.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("demo_pocket.svg"),
        name: "demo_pocket".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(20.0, 20.0, 80.0, 80.0)])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    // AS001 pocket params: depth=6, dpp=2, stepover=2.4, feed=900, plunge=350.
    let pocket = PocketConfig {
        stepover: 2.4,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 900.0,
        plunge_rate: 350.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };

    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket (AS001)".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
    };
    let tp_idx = controller
        .state
        .session
        .add_toolpath(0, tp_config)
        .expect("add pocket to default setup");
    let tp_id = controller
        .state
        .session
        .toolpath_configs()
        .get(tp_idx)
        .expect("toolpath_configs slot present after add_toolpath")
        .id;

    // Drive the production code path. The default setup is identity
    // (face_up=Top, z_rotation=Deg0).
    controller.submit_toolpath_compute(tp_id);

    let request = controller
        .compute
        .captured
        .as_ref()
        .expect("submit_toolpath_compute should have submitted a ComputeRequest");

    let captured_stock_bbox = request
        .stock_bbox
        .as_ref()
        .expect("ComputeRequest::stock_bbox should be Some");

    // The world stock top for AS001 sits at Z=0 (origin_z=-12 + stock.z=12).
    assert!(
        (request.heights.top_z - 0.0).abs() < 1e-9,
        "F-028 viz-path: heights.top_z must resolve to the world stock top \
         (Z=0 for AS001 identity setup); got {:.6}. Pre-fix this read 12.0 \
         (the local zero-rooted stock_top), and the toolpath generator \
         emitted cuts at Z=10, 8, 6 in setup-local frame. The downstream \
         viz simulation drops `local_to_global = None` for identity setups \
         (F-024 viz-worker follow-up `0c907a6`) so the dexel grid is rebuilt \
         in world frame — and the generator's local-frame cuts at Z=10 sat \
         10 mm above the world stock top at Z=0. Round-07 MCP smoke: \
         peak_axial=0, total_removed=0, air_cut=96 %.",
        request.heights.top_z
    );

    // Bottom of first pass: top_z - depth_per_pass*N or full depth.
    // With heights.top_z = 0 and depth = 6 (full pocket depth), bottom = -6.
    assert!(
        (request.heights.bottom_z - -6.0).abs() < 1e-9,
        "F-028 viz-path: heights.bottom_z must resolve to top_z - depth = -6 \
         for the AS001 pocket; got {:.6}. Pre-fix this read 6.0 \
         (12 - 6 in local frame).",
        request.heights.bottom_z
    );

    assert!(
        (captured_stock_bbox.max.z - 0.0).abs() < 1e-9,
        "F-028 viz-path: ComputeRequest.stock_bbox.max.z must equal the \
         world stock top (Z=0 for AS001 identity setup); got {:.6}. Pre-fix \
         this read 12.0 — the controller built a zero-rooted local bbox \
         even for identity setups. Downstream `generate_via_core` then \
         constructed boundary rectangles in the local zero-rooted XY frame, \
         compounding the frame mismatch.",
        captured_stock_bbox.max.z
    );

    assert!(
        (captured_stock_bbox.min.z - -12.0).abs() < 1e-9,
        "F-028 viz-path: ComputeRequest.stock_bbox.min.z must equal \
         stock.origin_z (-12 for AS001 identity setup); got {:.6}. Pre-fix \
         this read 0.0.",
        captured_stock_bbox.min.z
    );

    // XY frame: for identity setups the bbox must respect stock.origin_x/y too.
    assert!(
        (captured_stock_bbox.min.x - -10.0).abs() < 1e-9
            && (captured_stock_bbox.min.y - -10.0).abs() < 1e-9,
        "F-028 viz-path: ComputeRequest.stock_bbox.min.{{x,y}} must equal \
         stock.origin_{{x,y}} (-10, -10 for AS001 identity setup); got \
         ({:.6}, {:.6}).",
        captured_stock_bbox.min.x,
        captured_stock_bbox.min.y
    );
}
