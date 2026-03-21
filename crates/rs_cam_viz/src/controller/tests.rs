use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{CentralPanel, Context, FontDefinitions, SidePanel, TopBottomPanel, Window};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::toolpath::Toolpath;

use super::*;
use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, LaneState, SimulationRequest,
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
                controller.state.mode,
                controller.state.simulation.active,
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
        started_at: Some(std::time::Instant::now()),
    };
    controller.compute.analysis_lane = LaneSnapshot {
        lane: ComputeLane::Analysis,
        state: LaneState::Queued,
        queue_depth: 1,
        current_job: Some("Simulation".to_string()),
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
                    },
                    crate::compute::worker::SimBoundary {
                        id: ToolpathId(2),
                        name: "Profile".to_string(),
                        tool_name: "End Mill".to_string(),
                        start_move: 10,
                        end_move: 20,
                    },
                ],
                checkpoints: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
            },
        )));

    controller.drain_compute_results();

    assert_eq!(controller.state.simulation.setup_boundaries.len(), 2);
    assert_eq!(
        controller.state.simulation.setup_boundaries[0].setup_name,
        "Setup 1"
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries[1].setup_name,
        "Bottom Side"
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries[0].start_move,
        0
    );
    assert_eq!(
        controller.state.simulation.setup_boundaries[1].start_move,
        10
    );
}
