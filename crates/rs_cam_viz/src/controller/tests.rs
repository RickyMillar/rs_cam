use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{CentralPanel, Context, Panel};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::toolpath::Toolpath;

use super::*;
use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, JobRequest, LaneState, OptimizeRequest,
    SimulationRequest, SimulationResult, ToolpathSubmitOutcome,
};
use crate::state::job::{SetupId, ToolConfig, ToolId, ToolType};
use crate::state::runtime::ToolpathRuntime;
use crate::state::selection::Selection;
use crate::state::toolpath::{Adaptive3dConfig, OperationConfig, ToolpathId, ToolpathResult};
use crate::ui_command::{NoArgs, SimJumpToMoveArgs, UiCommand};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::session::{
    AddModelArgs, AddSetupArgs, AddToolpathArgs, AdoptResultArgs, Command,
    InvalidateToolpathInputsArgs, LoadedModel, ProjectSessionBuilder, ReplaceToolpathConfigArgs,
    ReplaceToolsArgs, SetBoundaryConfigArgs, SetFeedsProvenanceArgs, SetPostConfigArgs,
    SetSetupFaceArgs, SetSetupRotationArgs, SetStockConfigArgs, SetStockSourceArgs,
    SetToolpathEnabledArgs, ToolpathConfig,
};

struct ScriptedBackend {
    toolpath_lane: LaneSnapshot,
    analysis_lane: LaneSnapshot,
    optimize_lane: LaneSnapshot,
    reach_lane: LaneSnapshot,
    job_lane: LaneSnapshot,
    drained: Vec<ComputeMessage>,
    /// G-REGEN-RACE: the one piece of real lane state the supersede rule
    /// reads. Setting it stands for "the toolpath lane is currently
    /// running a job for this toolpath", which is the state
    /// `ThreadedComputeBackend::submit_toolpath` turns into
    /// `SupersededActive`. Modelled rather than stubbed so the controller
    /// under test makes the same decision the real lane makes.
    active_toolpath_id: Option<ToolpathId>,
    submitted: Vec<ToolpathId>,
    /// Optimize-lane submissions, kept WHOLE rather than counted. Only the
    /// project rollup rides this lane since WP14b.
    optimize_requests: Vec<OptimizeRequest>,
    /// `Job`-lane submissions, kept WHOLE for the same reason. The Phase U
    /// tier preview and the per-toolpath Optimize run both ride this lane
    /// since WP14b, and each handle carries what was actually asked for, so
    /// a test can witness the request rather than only that one happened.
    ///
    /// `ComputeBackend::submit_job` has an empty default body, so a backend
    /// that does not override it records nothing.
    job_requests: Vec<JobRequest>,
}

impl ScriptedBackend {
    fn new() -> Self {
        Self {
            toolpath_lane: LaneSnapshot::idle(ComputeLane::Toolpath),
            analysis_lane: LaneSnapshot::idle(ComputeLane::Analysis),
            optimize_lane: LaneSnapshot::idle(ComputeLane::Optimize),
            reach_lane: LaneSnapshot::idle(ComputeLane::Reach),
            job_lane: LaneSnapshot::idle(ComputeLane::Job),
            drained: Vec::new(),
            active_toolpath_id: None,
            submitted: Vec::new(),
            optimize_requests: Vec::new(),
            job_requests: Vec::new(),
        }
    }
}

impl ComputeBackend for ScriptedBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        self.submitted.push(id);
        if self.active_toolpath_id == Some(id) {
            ToolpathSubmitOutcome::SupersededActive
        } else {
            ToolpathSubmitOutcome::Queued
        }
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, request: OptimizeRequest) {
        self.optimize_requests.push(request);
    }
    fn submit_job(&mut self, request: JobRequest) {
        self.job_requests.push(request);
    }

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.state = LaneState::Cancelling,
            ComputeLane::Analysis => self.analysis_lane.state = LaneState::Cancelling,
            ComputeLane::Optimize => self.optimize_lane.state = LaneState::Cancelling,
            ComputeLane::Reach => self.reach_lane.state = LaneState::Cancelling,
            ComputeLane::Job => self.job_lane.state = LaneState::Cancelling,
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
            ComputeLane::Reach => self.reach_lane.clone(),
            ComputeLane::Job => self.job_lane.clone(),
        }
    }

    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
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

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn sample_controller() -> AppController<ScriptedBackend> {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    sample_project_into(&mut controller);
    controller
}

/// The shared fixture body, generic over the backend so tests that need a
/// result-echoing double (A/M11's fixpoint chain) get the same project.
fn sample_project_into<B: ComputeBackend>(controller: &mut AppController<B>) {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    // Every caller hands in a controller straight from `with_backend`, so the
    // builder replaces an empty session.
    let mut builder = ProjectSessionBuilder::new().tool(tool);

    let mesh = Arc::new(make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
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
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tp_config).unwrap();
    controller.state.session = builder.build();
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
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
}

/// The index of the toolpath carrying `id`.
///
/// The core setters take an index. A test that holds an id converts it here.
fn index_of<B: ComputeBackend>(controller: &AppController<B>, id: ToolpathId) -> usize {
    controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == id)
        .expect("the toolpath id belongs to this session")
}

/// Append another toolpath to setup 0 of a `sample_controller()` project,
/// sharing its tool and model. Returns the new toolpath's id.
fn push_toolpath<B: ComputeBackend>(controller: &mut AppController<B>, name: &str) -> ToolpathId {
    let next = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id.0 + 1)
        .max()
        .unwrap_or(0);
    let cfg = ToolpathConfig {
        id: ToolpathId(next),
        name: name.to_owned(),
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
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .unwrap();
    ToolpathId(next)
}

fn render_snapshot(
    controller: &mut AppController<ScriptedBackend>,
) -> crate::ui::automation::UiAutomationSnapshot {
    let ctx = Context::default();
    // UI-06: the real font set, not `FontDefinitions::empty()`. The
    // inspector reaches a weight through a NAMED family
    // (`tokens::FAMILY_SEMIBOLD`), and epaint panics on a named family that
    // no font is bound to. With empty fonts this harness could render only
    // the tabs that never ask for a weight.
    crate::ui::tokens::apply_fonts(&ctx);
    // epaint 0.36 asserts in `Drop for TexturesDelta` that the deltas were
    // either applied or cleared on purpose. eframe applies them in the real
    // app; this harness has no GPU and never uploads a texture, so it clears
    // them, which is what the assert's own message prescribes.
    let mut output = ctx.run_ui(Default::default(), |ui| {
        crate::ui::automation::begin_frame(ui.ctx());

        Panel::right("properties").show(ui, |ui| {
            let events = &mut controller.events;
            crate::ui::properties::draw(ui, &mut controller.state, events);
        });

        Panel::bottom("status_bar").show(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let collision_count = controller.collision_positions.len();
            crate::ui::status_bar::draw(
                ui,
                &controller.state,
                collision_count,
                &lanes,
                controller.load_warnings(),
            );
        });

        CentralPanel::default().show(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let events = &mut controller.events;
            crate::ui::viewport_overlay::draw(
                ui,
                &mut controller.state,
                crate::render::camera::ProjectionMode::Perspective,
                &lanes,
                events,
            );
        });

        // DC7 deleted the "Project Load Warnings" window this harness used to
        // mirror. The warnings reach the operator as a count on the status
        // bar, which the panel above already draws.
    });
    output.textures_delta.clear();
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
        active_toolpath_id: None,
        active_toolpath_index: None,
    };
    controller.compute.analysis_lane = LaneSnapshot {
        lane: ComputeLane::Analysis,
        state: LaneState::Queued,
        queue_depth: 1,
        current_job: Some("Simulation".to_owned()),
        current_phase: None,
        started_at: None,
        active_toolpath_id: None,
        active_toolpath_index: None,
    };

    let snapshot = render_snapshot(&mut controller);
    assert!(snapshot.widgets.contains_key("status_lane_toolpath"));
    assert!(snapshot.widgets.contains_key("status_lane_analysis"));
    assert!(snapshot.widgets.contains_key("overlay_cancel_all"));
    assert!(snapshot.widgets.contains_key("overlay_collision_check"));
    assert!(snapshot.widgets.contains_key("properties_stock_to_leave"));
}

/// G-HEIGHTSTAB (observed live 2026-08-23): `set_ui_view(toolpath_index = 1,
/// properties_tab = "heights")` on a healthy generated adaptive3d rough marked
/// it stale and auto-regenerated it to ZERO moves.
///
/// The Heights rows promoted `Auto` → `FromReference(sensible default)` while
/// *drawing*; `write_entry_config_to_session` wrote that into the session, and
/// the heights-changed check below it set `stale_since` and dirtied the
/// project. On a 3D operation (`DepthSemantics::None`, so
/// `default_depth_for_heights() == 0`) the promoted bottom row was "0 mm above
/// Stock Top" — a floor AT the stock top, plus `bottom_pinned` flipped true —
/// which clipped the whole operation away.
///
/// Viewing the tab must change nothing: no commit, no stale mark, no dirty.
///
/// UI-06 drives the loop from `ToolpathTab::ALL`, so a NEW tab inherits the
/// rule. The tab arrives as the enum, which is what `set_ui_view` now parses
/// the caller's key into.
#[test]
fn opening_heights_tab_does_not_pin_heights_or_mark_stale_g_heightstab() {
    use crate::ui::properties::ToolpathTab;

    let heights_repr = |controller: &AppController<ScriptedBackend>, tp_id| {
        format!(
            "{:?}",
            controller
                .state
                .session
                .find_toolpath_config_by_id(tp_id)
                .expect("toolpath present")
                .1
                .heights
        )
    };

    assert!(
        ToolpathTab::ALL.len() >= 5,
        "the tab list shrank; this loop would pass for the wrong reason"
    );
    let mut defects: Vec<String> = Vec::new();
    for &tab in ToolpathTab::ALL {
        // Every key the MCP validator accepts must name this variant, so
        // the refusal list and the parser cannot disagree.
        assert_eq!(
            ToolpathTab::parse(tab.key()),
            Some(tab),
            "`{}` does not parse back to the tab it names",
            tab.key()
        );

        let mut controller = sample_controller();
        let tp_id = controller.state.session.toolpath_configs()[0].id;
        let before = heights_repr(&controller, tp_id);
        assert!(
            before.contains("Auto"),
            "fixture must start with auto heights, got {before}"
        );
        controller.state.gui.dirty = false;

        // Exactly what MCP `set_ui_view(properties_tab = ...)` sets.
        controller.state.gui.pending_toolpath_tab = Some((tp_id, tab));
        let _ = render_snapshot(&mut controller);

        let after = heights_repr(&controller, tp_id);
        if after != before {
            defects.push(format!("the {tab:?} tab rewrote the stored heights"));
        }
        if controller.state.gui.toolpath_rt[&tp_id]
            .stale_since
            .is_some()
        {
            defects.push(format!("the {tab:?} tab marked the toolpath stale"));
        }
        if controller.state.gui.dirty {
            defects.push(format!("the {tab:?} tab dirtied the project"));
        }
    }
    // G-LINKFEEDOPT (2026-09-17): the Linking tab wrote
    // `feed_optimization = false` while drawing; fixed in
    // `ui/properties/linking_dressup.rs`, so this loop tolerates nothing.
    assert!(
        defects.is_empty(),
        "viewing a tab must commit nothing. Found: {defects:?}"
    );
}

/// DC7: a load warning reaches the operator as a COUNT on the status bar.
///
/// The count does not depend on `show_load_warnings`, which now gates the
/// modal alone. That is the point of the package: the old window could be
/// dismissed once and never reopened, so the warnings became unreachable.
#[test]
fn load_warnings_reach_the_status_bar_as_a_count_dc7() {
    let mut controller = sample_controller();
    let clean = render_snapshot(&mut controller);
    assert!(
        !clean.widgets.contains_key("status_load_warnings"),
        "a project with no warnings must show no count"
    );

    let project_path = fixture_path("missing_model_project.toml");
    controller
        .open_job_from_path(&project_path)
        .expect("open project with missing model");
    assert!(!controller.load_warnings().is_empty());

    let shown = render_snapshot(&mut controller);
    let count = shown
        .widgets
        .get("status_load_warnings")
        .expect("the status bar states the warning count");
    assert_eq!(
        count.label,
        crate::ui::status_bar::warnings_label(controller.load_warnings().len())
    );

    // Closing the modal must not take the count away with it.
    controller.set_show_load_warnings(false);
    let closed = render_snapshot(&mut controller);
    assert!(
        closed.widgets.contains_key("status_load_warnings"),
        "the count is the route back to the warnings and stays present"
    );
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
    // L8 deleted the `SetProjectName` row. The name was never read
    // here: the fixture carries tools and setups, so a save and a reopen
    // pass the "looks like a CAM project" check without one.
    let mut controller = sample_controller();
    let generated = {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(5.0, 5.0, -1.0), 500.0);
        toolpath
    };
    // G-STALEXPORT: seed BOTH stores, which is what a real generation
    // leaves (`drain_compute_results` inserts into the session and the
    // viz store together). Seeding only the viz store used to export
    // through the silent fallback; it now reads as an operation edited
    // since it was generated, and the export refuses by name.
    let revision = controller.state.session.toolpath_revision(0);
    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(rs_cam_core::session::ToolpathComputeResult {
                op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
                    rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(generated.clone()),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
            }),
        }))
        .expect("seed the core result");
    controller
        .state
        .gui
        .toolpath_rt
        .get_mut(&ToolpathId(0))
        .unwrap()
        .result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            generated,
        )),
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

    let setup_idx = controller
        .state
        .session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Bottom Side".to_owned()),
            face_up: rs_cam_core::compute::transform::FaceUp::default(),
        }))
        .expect("the session accepts a second setup")
        .created
        .expect("the AddSetup row reports the new setup index");
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
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: setup_idx,
            config: Box::new(tp2_config),
        }))
        .unwrap()
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let tp2_id = controller.state.session.toolpath_configs()[1].id;

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(
            crate::compute::SimulationResult {
                core: rs_cam_core::compute::simulate::SimulationResult {
                    mesh: rs_cam_core::stock::stock_mesh::StockMesh {
                        vertices: Vec::new(),
                        indices: Vec::new(),
                        colors: Vec::new(),
                    },
                    total_moves: 20,
                    deviations: None,
                    column_deviations: None,
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
                    rapid_collisions: Vec::new(),
                    rapid_collision_move_indices: Vec::new(),
                    cut_trace: None,
                    column_grid_cell_mm: 0.5,
                    resolution_clamped: false,
                    prior_stocks: std::collections::HashMap::new(),
                },
                playback_data: Vec::new(),
                cut_trace_path: None,
            },
        ))));

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

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

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
    generate_all_for_test(&mut controller);
    // UR3 (f97327c3): an UNSTAMPED result reads stale, never current. A
    // fresh-looking run must come through the submit that stamps the
    // capture revision, as a real Run Simulation does.
    controller.handle_internal_event(AppEvent::RunSimulation);
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
fn metric_capture_toggle_before_first_result_tracks_in_flight_mismatch_without_dirtying_project() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let dirty_before = controller.state.gui.dirty;
    let revision_before = controller.state.simulation.metric_options_revision;

    controller.state.simulation.set_metric_capture_enabled(true);
    assert_eq!(
        controller.state.simulation.metric_options_revision,
        revision_before + 1
    );
    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "a pre-first-run capture choice has no evidence to stale"
    );

    controller.handle_internal_event(AppEvent::RunSimulation);
    assert_eq!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision,
        Some(revision_before + 1),
        "the run must stamp the capture revision it answers"
    );
    controller
        .state
        .simulation
        .set_metric_capture_enabled(false);
    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "there is still no accepted evidence before the first result lands"
    );

    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.metric_options_are_stale(),
        "a first result answering the older in-flight capture revision is stale"
    );
    assert_eq!(
        controller.state.gui.dirty, dirty_before,
        "runtime capture choices must not dirty the machining project"
    );
}

#[test]
fn metric_capture_toggle_after_results_marks_existing_evidence_stale_without_dirtying_project() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);
    let dirty_before = controller.state.gui.dirty;
    let revision_before = controller.state.simulation.metric_options_revision;

    controller.state.simulation.set_metric_capture_enabled(true);

    assert_eq!(
        controller.state.simulation.metric_options_revision,
        revision_before + 1
    );
    assert!(controller.state.simulation.metric_options_are_stale());
    assert_eq!(controller.state.gui.dirty, dirty_before);
}

#[test]
fn metric_capture_matching_success_clears_stale_marker() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    assert!(controller.state.simulation.metric_options_are_stale());

    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "a successful result for the submitted capture revision is current"
    );
}

#[test]
fn metric_capture_unstamped_success_preserves_stale_marker() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );

    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.metric_options_are_stale(),
        "an unstamped result cannot clear a marker whose capture revision it cannot prove"
    );
}

#[test]
fn metric_capture_cancel_preserves_marker_and_consumes_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);
    assert!(controller.state.simulation.submitted_edit_counter.is_some());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_some()
    );

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
}

#[test]
fn metric_capture_error_preserves_marker_and_consumes_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Message("metric capture fixture".to_owned()),
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
}

#[test]
fn metric_capture_reset_clears_evidence_and_stamps_without_dirtying_project() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);
    let dirty_before = controller.state.gui.dirty;

    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert!(!controller.state.simulation.has_results());
    assert!(!controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
    assert_eq!(controller.state.gui.dirty, dirty_before);
}

#[test]
fn an_accepted_run_and_an_accepted_result_arrive_and_leave_together_ur3() {
    // The derived metric-options staleness reads `last_run`, not `results`.
    // The two must arrive and leave together on EVERY transition, or the
    // reader would ask a run that has no evidence (or evidence with no run).
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // Submit: stamps exist, no evidence yet.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // Accept.
    inject_sim_results(&mut controller, 1);
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // Cancel: stamps consumed, evidence kept.
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // Error: same shape.
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Message("invariant fixture".to_owned()),
        )));
    controller.drain_compute_results();
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // Reset: both leave together.
    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());

    // A second accept re-establishes the pairing.
    inject_sim_results(&mut controller, 1);
    let sim = &controller.state.simulation;
    assert_eq!(sim.has_results(), sim.last_run.is_some());
}

#[test]
fn the_primary_and_the_builder_agree_about_a_runnable_project_ur3() {
    // The Simulation primary and the Readiness Run-sim affordances gate on
    // `readiness::simulation_request_is_buildable`; the controller's
    // `build_simulation_groups` is the executable truth. Two predicates
    // drift, and the drift is an enabled button that starts nothing — or a
    // disabled button over work the controller would accept. Four fixtures,
    // chosen to cover every admission rule the builder has.

    // Fixture 1 — nothing generated.
    let mut controller = sample_controller();
    // The shared fixture seeds a GUI result. Fixture 1 needs the
    // ungenerated state, so it drops that result first.
    for rt in controller.state.gui.toolpath_rt.values_mut() {
        rt.result = None;
    }
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .values()
            .all(|rt| rt.result.is_none()),
        "fixture 1: the sample project must start ungenerated"
    );
    let predicate = crate::ui::readiness::simulation_request_is_buildable(
        &controller.state.session,
        &controller.state.gui,
    );
    let builder = controller
        .build_simulation_groups(|_, tc| tc.enabled, |_| false)
        .is_some();
    assert_eq!(
        predicate, builder,
        "fixture 1 (nothing generated): the primary and the builder disagree"
    );
    assert!(
        !predicate,
        "fixture 1: an ungenerated project must not be buildable"
    );

    // Fixture 2 — one generated op.
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .values()
            .any(|rt| rt.result.is_some()),
        "fixture 2: generate_all_for_test must leave a generated result"
    );
    let predicate = crate::ui::readiness::simulation_request_is_buildable(
        &controller.state.session,
        &controller.state.gui,
    );
    let builder = controller
        .build_simulation_groups(|_, tc| tc.enabled, |_| false)
        .is_some();
    assert_eq!(
        predicate, builder,
        "fixture 2 (one generated op): the primary and the builder disagree"
    );
    assert!(predicate, "fixture 2: a generated op must be buildable");

    // Fixture 3 — a generated op whose `tool_id` names no tool. The builder
    // drops it (`controller/events/simulation.rs`), so neither may admit it.
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.tool_id = ToolId(999);
    });
    assert_eq!(
        controller.state.session.toolpath_configs()[0].tool_id,
        999,
        "fixture 3 precondition failed: the corrupt tool id did not reach the session"
    );
    let predicate = crate::ui::readiness::simulation_request_is_buildable(
        &controller.state.session,
        &controller.state.gui,
    );
    let builder = controller
        .build_simulation_groups(|_, tc| tc.enabled, |_| false)
        .is_some();
    assert_eq!(
        predicate, builder,
        "fixture 3 (unresolvable tool): the primary and the builder disagree"
    );
    assert!(
        !predicate,
        "fixture 3: an op with no resolvable tool must not be buildable"
    );

    // Fixture 4 — an enabled pending `FromRemainingStock` op with nothing
    // generated. F.4's phantom prior stock: the group is still emitted, so a
    // run CAN simulate it and the primary must say so.
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.stock_source = crate::state::toolpath::StockSource::FromRemainingStock;
    });
    assert_eq!(
        controller.state.session.toolpath_configs()[0].stock_source,
        crate::state::toolpath::StockSource::FromRemainingStock,
        "fixture 4 precondition failed: the stock source did not reach the session"
    );
    // F2.2 keeps the GUI result across an edit so the viewport can go on
    // drawing it, so the fixture drops it by hand. The panel never clears
    // it; three other tests in this file clear it the same way.
    for rt in controller.state.gui.toolpath_rt.values_mut() {
        rt.result = None;
    }
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .values()
            .all(|rt| rt.result.is_none()),
        "fixture 4: the phantom prior stock needs an ungenerated op"
    );
    let predicate = crate::ui::readiness::simulation_request_is_buildable(
        &controller.state.session,
        &controller.state.gui,
    );
    let builder = controller
        .build_simulation_groups(|_, tc| tc.enabled, |_| false)
        .is_some();
    assert_eq!(
        predicate, builder,
        "fixture 4 (phantom prior stock): the primary and the builder disagree"
    );
    assert!(
        predicate,
        "fixture 4: the phantom prior-stock group must be buildable"
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

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert_eq!(controller.state.simulation.playback.current_move, 0);
    assert!(!controller.state.simulation.playback.playing);
    assert!((controller.state.simulation.playback.speed - 500.0).abs() < f32::EPSILON);
}

/// Helper: inject minimal simulation results into the controller.
fn inject_sim_results(controller: &mut AppController<ScriptedBackend>, num_setups: usize) {
    use rs_cam_core::stock::stock_mesh::StockMesh;

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
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh,
                total_moves,
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
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));

    controller.drain_compute_results();
}

#[test]
fn toolpath_results_persist_debug_trace_metadata() {
    let mut controller = sample_controller();
    let recorder =
        rs_cam_core::trace::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
    let ctx = recorder.root_context();
    let span = ctx.start_span("core_generate", "Generate");
    span.finish();
    let trace = Arc::new(recorder.finish());
    let semantic_recorder = rs_cam_core::trace::semantic_trace::ToolpathSemanticRecorder::new(
        "Adaptive 3D",
        "3D Rough",
    );
    let semantic_root = semantic_recorder.root_context();
    let pass = semantic_root.start_item(
        rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    let annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    pass.bind_to_toolpath(&annotated.toolpath, 0, 0);
    let semantic_trace = Arc::new(semantic_recorder.finish());
    let debug_path = temp_path("toolpath_trace_metadata", "json");

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
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
        )));

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
    let recorder =
        rs_cam_core::trace::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
    let ctx = recorder.root_context();
    let span = ctx.start_span("adaptive_pass", "Pass 1");
    span.finish();
    let trace = Arc::new(recorder.finish());
    let semantic_recorder = rs_cam_core::trace::semantic_trace::ToolpathSemanticRecorder::new(
        "Adaptive 3D",
        "3D Rough",
    );
    let semantic_root = semantic_recorder.root_context();
    let pass = semantic_root.start_item(
        rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    let toolpath = Toolpath::new();
    pass.bind_to_toolpath(&toolpath, 0, 0);
    let semantic_trace = Arc::new(semantic_recorder.finish());
    let debug_path = temp_path("cancelled_toolpath_trace_metadata", "json");

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
                result: Err(crate::compute::ComputeError::Cancelled),
                debug_trace: Some(Arc::clone(&trace)),
                semantic_trace: Some(Arc::clone(&semantic_trace)),
                debug_trace_path: Some(debug_path.clone()),
            },
        )));

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
// MCP `cancel_generation`: a cancelled generation must resolve any pending
// MCP `generate_toolpath` waiter instead of leaving it hanging — mirroring
// the fail-hard-at-submit fix immediately above, but for the
// cancel-in-flight path instead of the reject-before-submit path.
//
// WP23 deleted the lane-targeting test that shared this banner. It pinned
// `UiCommand::CancelToolpathGeneration`, a view row that no surface
// constructed: the MCP tool cancels the toolpath lane through
// `GenerationControl` on the server thread, and the GUI cancels every lane
// through `UiCommand::CancelCompute`.
// ---------------------------------------------------------------------------

/// A `Cancelled` outcome draining through `drain_compute_results` must
/// resolve a pending MCP `generate_toolpath` waiter, the same way a
/// submit-time fail-hard already does (see the pair of tests above this
/// section). Without this, cancelling a runaway generate over MCP would
/// stop the compute but still leave the original `generate_toolpath` call
/// hanging forever — trading a hang-on-completion for a hang-on-cancel.
#[cfg(feature = "mcp")]
#[test]
fn cancelled_drain_resolves_pending_mcp_generate_toolpath_waiter() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);

    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(tp_id, tx);

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: tp_id,
                revision: None,
                result: Err(crate::compute::ComputeError::Cancelled),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));

    controller.drain_compute_results();

    let response = rx
        .try_recv()
        .expect("a cancelled drain must resolve the pending MCP oneshot, not strand it");
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the cancellation");
    assert!(
        payload.to_lowercase().contains("cancel"),
        "mcp payload for a cancelled generate should say so plainly, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("toolpath runtime should exist after cancel");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Pending),
        "cancelled toolpath status should revert to Pending (not Done), got {:?}",
        rt.status
    );

    assert!(
        !controller
            .pending_mcp
            .as_ref()
            .expect("pending_mcp still set")
            .toolpath
            .contains_key(&tp_id),
        "resolved MCP waiter should be removed from the pending map"
    );
}

// ---------------------------------------------------------------------------
// Roadmap F.1 — session.results cache must repopulate when the threaded
// compute backend returns a fresh result. Before the fix, the callback
// only wrote to gui.toolpath_rt and session.results stayed empty after
// the undo snapshot path ran — breaking project_load_report's span
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

    let annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
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
        )));

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

/// Push one successful completion for `ToolpathId(0)` onto the fake
/// lane's queue, the way `drain_compute_results_repopulates_session_results`
/// does.
fn push_toolpath_completion(
    controller: &mut AppController<ScriptedBackend>,
    revision: Option<u64>,
) {
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision,
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
}

/// WP3 — the drain hands the lane's revision stamp to
/// `Command::AdoptResult`, and core refuses a completion whose toolpath
/// moved while the job ran.
///
/// The refusal costs the operator nothing visible: `rt.result` still
/// carries the geometry, so the viewport draws it. Only the CORE slot
/// stays empty, which is what derives `EditedSince` and puts STALE on
/// every surface F2.2 wired. This is the outcome the deleted viz gate
/// produced, now produced by the one door.
#[test]
fn drain_refuses_a_completion_whose_revision_moved() {
    let mut controller = sample_controller();
    let submitted = controller.state.session.toolpath_revision(0);

    // The edit an operator makes while the lane runs.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 0 },
        ))
        .expect("toolpath 0 exists");
    assert_ne!(
        controller.state.session.toolpath_revision(0),
        submitted,
        "the fixture must move the revision, or the test proves nothing"
    );

    push_toolpath_completion(&mut controller, Some(submitted));
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_none(),
        "a completion that answers a superseded revision must not reach \
         the core cache"
    );
    assert!(
        controller.state.gui.toolpath_rt[&ToolpathId(0)]
            .result
            .is_some(),
        "the viz copy is kept, so the viewport still draws the geometry"
    );
}

/// The sibling arm: a completion that answers the CURRENT revision is
/// adopted. This is what fails if the stamp reads a session-global
/// counter instead of the toolpath's own revision.
#[test]
fn drain_adopts_a_completion_whose_revision_is_current() {
    let mut controller = sample_controller();
    // Move the revision FIRST, so the test cannot pass on a stamp that
    // only ever matches the initial value.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 0 },
        ))
        .expect("toolpath 0 exists");
    let submitted = controller.state.session.toolpath_revision(0);

    push_toolpath_completion(&mut controller, Some(submitted));
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_some(),
        "a completion that answers the current revision is adopted"
    );
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
    let annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
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
        )));

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
    let annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
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
        )));

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
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
                result: Err(crate::compute::ComputeError::Message("boom".into())),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));

    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_none(),
        "session.results must not be written on compute error"
    );
}

// ---------------------------------------------------------------------------
// P2.2/P2.3 — DerivedRestRegions boundary dependent staleness (Fix 2).
// A toolpath whose enabled boundary is `DerivedRestRegions` referencing
// another toolpath's cached `rest_regions` must be marked stale whenever
// that source regenerates or is removed — otherwise its cached clip keeps
// reflecting regions that no longer match the source's latest state.
// ---------------------------------------------------------------------------

fn add_derived_rest_dependent(
    controller: &mut AppController<ScriptedBackend>,
    source_id: ToolpathId,
) -> ToolpathId {
    let dependent_config = ToolpathConfig {
        id: ToolpathId(0), // placeholder — add_toolpath assigns the real id
        name: "Rest scallop".to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: crate::state::toolpath::BoundaryConfig {
            enabled: true,
            source: crate::state::toolpath::BoundarySource::DerivedRestRegions {
                source_toolpath_id: source_id,
            },
            containment: crate::state::toolpath::BoundaryContainment::Center,
            offset: 0.0,
        },
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(dependent_config),
        }))
        .expect("dependent toolpath should be added to setup 0")
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let dependent_id = controller.state.session.toolpath_configs()[1].id;
    controller
        .state
        .gui
        .toolpath_rt
        .insert(dependent_id, ToolpathRuntime::new(true));
    dependent_id
}

#[test]
fn drain_compute_results_marks_derived_rest_dependents_stale() {
    let mut controller = sample_controller();
    let source_id = ToolpathId(0);
    let dependent_id = add_derived_rest_dependent(&mut controller, source_id);

    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&dependent_id)
            .expect("dependent runtime should exist")
            .stale_since
            .is_none(),
        "dependent should start non-stale"
    );

    let mut annotated = rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(Toolpath::new());
    annotated.rest_regions = Some(Arc::new(vec![rs_cam_core::polygon::Polygon2::rectangle(
        -5.0, -5.0, 5.0, 5.0,
    )]));
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: source_id,
                revision: None,
                result: Ok(ToolpathResult {
                    annotated: Arc::new(annotated),
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

    controller.drain_compute_results();

    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&dependent_id)
            .expect("dependent runtime should exist")
            .stale_since
            .is_some(),
        "dependent toolpath should be marked stale after its rest-regions source regenerates"
    );
}

// ---------------------------------------------------------------------------
// P2 pencil-panel consolidation — demand-driven rest analysis producer hook.
// Wiring a toolpath's boundary to `DerivedRestRegions { source_toolpath_id }`
// via `ProjectSession::set_boundary_config` (the setter both MCP's
// `set_boundary_config` tool and the GUI's Machining Boundary picker's
// write-back call into) must auto-enable the SOURCE toolpath's own rest
// analysis so it actually produces the regions the new consumer expects —
// see `session::mutation::auto_enable_rest_analysis_for_source`.
// ---------------------------------------------------------------------------

#[test]
fn set_boundary_config_auto_enables_source_rest_analysis() {
    let mut controller = sample_controller();
    let source_id = ToolpathId(0);
    assert!(
        !controller
            .state
            .session
            .find_toolpath_config_by_id(source_id)
            .expect("source toolpath should exist")
            .1
            .rest_analysis
            .enabled,
        "fixture source should start with rest analysis disabled"
    );

    // A plain second toolpath whose boundary we'll wire to the source via
    // `set_boundary_config` (not by embedding it at construction time, the
    // way `add_derived_rest_dependent` does above — this test exercises the
    // setter itself).
    let consumer_config = ToolpathConfig {
        id: ToolpathId(0), // placeholder — add_toolpath assigns the real id
        name: "Consumer".to_owned(),
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
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let consumer_index = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(consumer_config),
        }))
        .expect("consumer toolpath should be added to setup 0")
        .created
        .expect("the AddToolpath row reports the new toolpath index");

    let boundary = crate::state::toolpath::BoundaryConfig {
        enabled: true,
        source: crate::state::toolpath::BoundarySource::DerivedRestRegions {
            source_toolpath_id: source_id,
        },
        containment: crate::state::toolpath::BoundaryContainment::Center,
        offset: 0.0,
    };
    let _ = controller
        .state
        .session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: consumer_index,
            boundary,
        }))
        .expect("boundary set should succeed");

    let (_, source_tc) = controller
        .state
        .session
        .find_toolpath_config_by_id(source_id)
        .expect("source toolpath should still exist");
    assert!(
        source_tc.rest_analysis.enabled,
        "wiring a DerivedRestRegions boundary must auto-enable the source's rest analysis"
    );
}

#[test]
fn handle_remove_toolpath_marks_derived_rest_dependents_stale() {
    let mut controller = sample_controller();
    let source_id = ToolpathId(0);
    let dependent_id = add_derived_rest_dependent(&mut controller, source_id);

    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(source_id));

    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&dependent_id)
            .expect("dependent runtime should exist")
            .stale_since
            .is_some(),
        "dependent should be marked stale after its rest-regions source toolpath is removed"
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

/// WP28 part 3 deleted `Command::RemoveSetup` with its setter. The row
/// declared `Reach::Skip` on all three surfaces, so no operator path and
/// no agent path removed a setup (review §21.8). WP13 had already
/// deleted `AppEvent::RemoveSetup` for the same reading. What remains of
/// the old lifecycle test is the half the product still runs: the add.
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

/// F-024 (third-site fix, 2026-05-25): regression test that the controller's
/// world `stock_bbox` helper respects `StockConfig::origin_{x,y,z}`.
///
/// Pre-fix (rounds 03/04 evidence): `build_simulation_groups` constructed the
/// world `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
/// dropping the origin. For AS001 (`origin_z=-12`) this sent a bbox of
/// `(0,0,0)..(100,100,12)` to the worker even though the actual world stock
/// spans `(-10,-10,-12)..(90,90,0)`. The F-024 viz-worker follow-up
/// (commit `1dd1aa7`) made `build_core_simulation_request` forward
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
    let session = ProjectSessionBuilder::new().stock(stock).build();

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
    use rs_cam_core::stock::simulation_cut::CutKinematics;
    use std::sync::atomic::AtomicBool;

    use crate::compute::{
        SetupSimGroup, SetupSimToolpath, SimulationRequest as VizSimulationRequest,
    };

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
    let session = ProjectSessionBuilder::new().stock(stock).build();

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
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    tp,
                )),
                tool,
                semantic_trace: None,
                spindle_rpm: Some(18_000),
                metrics_not_applicable: false,
                drill_op: None,
                operation_config_hash: 0,
            }],
            local_stock_bbox,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: world_stock_bbox,
        stock_top_z: world_stock_bbox.max.z,
        resolution: 1.0,
        metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
        model_mesh: None,
        kinematics: None,
        memoize_prefix: false,
    };

    let cancel = AtomicBool::new(false);
    let result = crate::compute::worker::execute::run_simulation_with_phase(
        &request,
        &cancel,
        |_phase| {},
        None,
    )
    .expect("viz simulation completes");

    let cut_trace = result.core.cut_trace.as_ref().expect("metric cut trace");

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
// The original F-028 fix (commit `329c5cf`) only patched
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
// for identity setups (F-024 follow-up `1dd1aa7`) — so the dexel grid was
// rebuilt in *world* frame. The generator's local-frame cuts at Z=10 sat
// 10 mm above the world stock top at Z=0; the cutter swept through air
// the whole run. Round-07 MCP smoke evidence:
//   AS001 pocket: z_level = 10/8/6, peak_axial_doc_mm = 0,
//   total_removed_volume_est_mm3 = 0, chipload = Unmodeled
//   (all_samples_air_cut_or_rapid), air cut = 96 % of total runtime, 0 rapid
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
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        self.captured = Some(request);
        ToolpathSubmitOutcome::Queued
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
    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
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
/// Pre-fix readings (commit `f00c733`): `heights.top_z = 12.0`,
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

    // 6 mm endmill matching the AS001 fixture.
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    controller.state.session = ProjectSessionBuilder::new().tool(tool).build();

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
    let _ = controller
        .state
        .session
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(stock),
        }))
        .expect("the stock edit applies");

    // 2D polygon model (matches the SVG-driven AS001 pocket case).
    let model_id = controller
        .state
        .session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(LoadedModel {
                id: 0,
                path: std::path::PathBuf::from("demo_pocket.svg"),
                name: "demo_pocket".to_owned(),
                kind: Some(ModelKind::Svg),
                mesh: None,
                polygons: Some(Arc::new(vec![Polygon2::rectangle(20.0, 20.0, 80.0, 80.0)])),
                drill_targets: std::sync::Arc::new(Vec::new()),
                layers: std::sync::Arc::new(Vec::new()),
                enriched_mesh: None,
                units: Some(ModelUnits::Millimeters),
                winding_report: None,
                load_error: None,
            }),
        }))
        .expect("the session takes the model")
        .created
        .expect("the AddModel row reports the new model id");

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
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let tp_idx = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(tp_config),
        }))
        .expect("add pocket to default setup")
        .created
        .expect("the AddToolpath row reports the new toolpath index");
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

    // WP11b: the request carries the handle, whose fields are private. The
    // handle publishes the same facts as a snapshot, which is what the
    // debug artifact has always recorded — so the claim is unchanged and
    // the reader is the one door.
    let snapshot = request.handle.request_snapshot();
    let number = |path: [&str; 2]| -> f64 {
        snapshot[path[0]][path[1]]
            .as_f64()
            .unwrap_or_else(|| panic!("the snapshot must carry {path:?}: {snapshot}"))
    };
    let bbox_number = |corner: &str, axis: &str| -> f64 {
        snapshot["stock_bbox"][corner][axis]
            .as_f64()
            .unwrap_or_else(|| panic!("the snapshot must carry stock_bbox: {snapshot}"))
    };

    // The world stock top for AS001 sits at Z=0 (origin_z=-12 + stock.z=12).
    let top_z = number(["heights", "top_z"]);
    assert!(
        top_z.abs() < 1e-9,
        "F-028 viz-path: heights.top_z must resolve to the world stock top \
         (Z=0 for AS001 identity setup); got {top_z:.6}. Pre-fix this read \
         12.0 (the local zero-rooted stock_top), and the toolpath generator \
         emitted cuts at Z=10, 8, 6 in setup-local frame. The downstream \
         viz simulation drops `local_to_global = None` for identity setups \
         (F-024 viz-worker follow-up `1dd1aa7`) so the dexel grid is rebuilt \
         in world frame — and the generator's local-frame cuts at Z=10 sat \
         10 mm above the world stock top at Z=0. Round-07 MCP smoke: \
         peak_axial=0, total_removed=0, air_cut=96 %."
    );

    // Bottom of first pass: top_z - depth_per_pass*N or full depth.
    // With heights.top_z = 0 and depth = 6 (full pocket depth), bottom = -6.
    let bottom_z = number(["heights", "bottom_z"]);
    assert!(
        (bottom_z - -6.0).abs() < 1e-9,
        "F-028 viz-path: heights.bottom_z must resolve to top_z - depth = -6 \
         for the AS001 pocket; got {bottom_z:.6}. Pre-fix this read 6.0 \
         (12 - 6 in local frame)."
    );

    let max_z = bbox_number("max", "z");
    assert!(
        max_z.abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox max.z must equal the world \
         stock top (Z=0 for AS001 identity setup); got {max_z:.6}. Pre-fix \
         this read 12.0 — the controller built a zero-rooted local bbox \
         even for identity setups, and the worker then built boundary \
         rectangles in that frame."
    );

    let min_z = bbox_number("min", "z");
    assert!(
        (min_z - -12.0).abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox min.z must equal \
         stock.origin_z (-12 for AS001 identity setup); got {min_z:.6}. \
         Pre-fix this read 0.0."
    );

    // XY frame: for identity setups the bbox must respect stock.origin_x/y too.
    let min_x = bbox_number("min", "x");
    let min_y = bbox_number("min", "y");
    assert!(
        (min_x - -10.0).abs() < 1e-9 && (min_y - -10.0).abs() < 1e-9,
        "F-028 viz-path: the emission stock bbox min.{{x,y}} must equal \
         stock.origin_{{x,y}} (-10, -10 for AS001 identity setup); got \
         ({min_x:.6}, {min_y:.6})."
    );
}

// ---------------------------------------------------------------------------
// Submit-time fail-hard must resolve a pending MCP `generate_toolpath`
// waiter, not strand it. Confirmed live: an MCP `generate_toolpath` call
// hung ~9 hours because `submit_toolpath_compute` returned early on a
// precondition rejection (e.g. the `DerivedRestRegions` self-reference
// check, or the `FromRemainingStock` no-prior-sim check) without ever
// invoking `notify_mcp_toolpath_complete` — that notify only fired from
// `drain_compute_results`, which never runs for a request that never
// reached the compute worker.
// ---------------------------------------------------------------------------

#[cfg(feature = "mcp")]
#[test]
fn submit_toolpath_compute_self_referential_boundary_resolves_mcp_waiter() {
    let mut controller = sample_controller();
    let source_id = ToolpathId(0);
    let dependent_id = add_derived_rest_dependent(&mut controller, source_id);

    // Rewrite the dependent's boundary to reference itself — the
    // self-referential fail-hard precondition in `submit_toolpath_compute`.
    //
    // WP7: the whole-config row, not `SetBoundaryConfig`. The boundary
    // row runs `auto_enable_rest_analysis_for_source`, which would flip
    // the dependent's own `rest_analysis.enabled` — a field this test
    // does not intend to write.
    let (index, stored) = controller
        .state
        .session
        .find_toolpath_config_by_id(dependent_id)
        .expect("dependent toolpath config must exist");
    let mut config = stored.clone();
    config.boundary.source = crate::state::toolpath::BoundarySource::DerivedRestRegions {
        source_toolpath_id: dependent_id,
    };
    let _ = controller
        .state
        .session
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index,
            config: Box::new(config),
        }))
        .expect("the dependent sits at a live index");

    // Register a pending MCP `generate_toolpath` waiter for this toolpath,
    // mirroring what `app/mcp.rs::mcp_generate_toolpath` does before pushing
    // the `GenerateToolpath` event.
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(dependent_id, tx);

    // Drive the production submit path directly (mirrors how
    // `AppEvent::GenerateToolpath` is dispatched in `controller/events/mod.rs`).
    controller.submit_toolpath_compute(dependent_id);

    // The MCP oneshot must already be resolved — no drain step should be
    // required, because this request never reached the compute worker.
    let response = rx.try_recv().expect(
        "submit-time fail-hard must resolve the pending MCP oneshot immediately, \
         not leave the caller waiting on a compute result that will never arrive",
    );
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the error");
    assert!(
        payload.contains("own rest regions"),
        "mcp error payload should describe the self-referential boundary rejection, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&dependent_id)
        .expect("dependent runtime should exist after fail-hard");
    assert!(
        matches!(
            &rt.status,
            crate::state::toolpath::ComputeStatus::Error(e) if e.contains("own rest regions")
        ),
        "toolpath runtime status should be Error mentioning the self-reference, got {:?}",
        rt.status
    );

    // The pending_mcp map must no longer hold this toolpath's sender —
    // `notify_mcp_toolpath_complete` removes it on resolution.
    assert!(
        !controller
            .pending_mcp
            .as_ref()
            .expect("pending_mcp still set")
            .toolpath
            .contains_key(&dependent_id),
        "resolved MCP waiter should be removed from the pending map"
    );
}

/// Same waiter-resolution family, different precondition:
/// `FromRemainingStock` (rest machining) with no prior simulated stock.
///
/// A/M11 reclassified this from `Error` to `AwaitingPriorStock` — it is a
/// sequencing state, not a failure — but the MCP waiter must still be
/// resolved immediately, which is what this test was written for.
#[cfg(feature = "mcp")]
#[test]
fn submit_toolpath_compute_missing_prior_stock_resolves_mcp_waiter() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);

    // No prior simulation has run, so `self.state.simulation` has no
    // boundaries/checkpoints — `FromRemainingStock` must fail hard.
    let index = index_of(&controller, tp_id);
    let _ = controller
        .state
        .session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: crate::state::toolpath::StockSource::FromRemainingStock,
        }))
        .expect("the index comes from the session");

    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(tp_id, tx);

    controller.submit_toolpath_compute(tp_id);

    let response = rx
        .try_recv()
        .expect("submit-time fail-hard must resolve the pending MCP oneshot immediately");
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the error");
    assert!(
        payload.contains("waiting on simulated stock") || payload.contains("remaining stock"),
        "mcp payload should describe the missing-prior-stock block, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime should exist after the block");
    assert!(
        matches!(
            &rt.status,
            crate::state::toolpath::ComputeStatus::AwaitingPriorStock(_)
        ),
        "A/M11: a missing upstream snapshot is a sequencing state, not an Error —          conflating them is what made 'cannot yet' indistinguishable from          'cannot ever'. Got {:?}",
        rt.status
    );
}

/// A/M11 sentry — the message shape. The pre-A/M11 text ("run a simulation of
/// the preceding operations first, then regenerate") was true and useless: it
/// named no operation, so the operator could not tell a one-round wait from a
/// four-round one. Both message variants must name the blocking op AND its
/// index, and the not-yet-generated variant must warn that the cycle repeats.
#[test]
fn blocked_rest_op_names_its_blocking_upstream_operation() {
    let mut controller = sample_controller();
    // Build a two-op setup: index 0 is the blocker, index 1 is the rest op.
    push_toolpath(&mut controller, "Rest Finish");
    let blocker_id = controller.state.session.toolpath_configs()[0].id;
    let blocker_name = controller.state.session.toolpath_configs()[0].name.clone();
    let rest_id = controller.state.session.toolpath_configs()[1].id;
    let rest_index = index_of(&controller, rest_id);
    let _ = controller
        .state
        .session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index: rest_index,
            source: crate::state::toolpath::StockSource::FromRemainingStock,
        }))
        .expect("the index comes from the session");

    // Blocker not generated: the wait is at least two rounds.
    controller.submit_toolpath_compute(rest_id);
    let block = controller
        .state
        .gui
        .toolpath_rt
        .get(&rest_id)
        .and_then(|rt| rt.status.blocked_on().cloned())
        .expect("rest op with no prior stock must record AwaitingPriorStock");
    assert_eq!(block.blocking_toolpath_id, Some(blocker_id));
    assert_eq!(block.blocking_toolpath_index, Some(0));
    assert!(
        block.message.contains(&blocker_name),
        "the message must NAME the blocking operation, got: {}",
        block.message
    );
    assert!(
        block.message.contains("index 0"),
        "the message must give the blocker's index, got: {}",
        block.message
    );
    assert!(
        block.message.contains("may need repeating"),
        "when the blocker has not generated, the message must say the cycle may          repeat — that is the number the operator cannot otherwise know. Got: {}",
        block.message
    );

    // Blocker generated: exactly one simulation is enough, and the message
    // must say so rather than repeating the vague ladder warning.
    controller
        .state
        .gui
        .toolpath_rt_or_default(blocker_id)
        .status = crate::state::toolpath::ComputeStatus::Done;
    controller.submit_toolpath_compute(rest_id);
    let block = controller
        .state
        .gui
        .toolpath_rt
        .get(&rest_id)
        .and_then(|rt| rt.status.blocked_on().cloned())
        .expect("still blocked — no simulation has run");
    assert!(
        block.message.contains("ONE simulation"),
        "with the blocker generated the wait is a single round and the message must          say so, got: {}",
        block.message
    );
}

/// A/M11 defect 2 — a disabled op must report `Disabled`, never the error it
/// was carrying when it was switched off. Two ops read `3D Finish 6` and
/// `Rivers (back) (copy)` as broken in the live run when they were merely off.
#[test]
fn a_disabled_op_reports_disabled_not_its_last_error() {
    use crate::state::toolpath::ComputeStatus;
    let stale = ComputeStatus::Error("uses remaining stock but none is available".to_owned());

    let enabled = ComputeStatus::effective(true, &stale);
    assert_eq!(enabled.label(), "Error");
    assert!(enabled.error_text().is_some());

    let disabled = ComputeStatus::effective(false, &stale);
    assert_eq!(disabled.label(), "Disabled");
    assert!(
        disabled.error_text().is_none(),
        "a disabled op must contribute nothing to any error list"
    );
    assert!(disabled.detail().is_none());
    assert!(
        !disabled.needs_generation(),
        "a disabled op is not waiting to be generated"
    );
}

/// A blocked op is not an error and must not appear in `runtime_errors`; a
/// disabled one must appear in neither list. This is the channel an agent
/// triages, so the separation has to hold at the JSON boundary, not just in
/// the enum.
#[cfg(feature = "mcp")]
#[test]
fn diagnostics_separate_blocked_from_failed_and_exclude_disabled() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Rest Finish");
    push_toolpath(&mut controller, "Broken");
    push_toolpath(&mut controller, "Switched Off");
    let ids: Vec<_> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();

    controller.state.gui.toolpath_rt_or_default(ids[1]).status =
        crate::state::toolpath::ComputeStatus::AwaitingPriorStock(
            rs_cam_core::compute::AwaitingPriorStock {
                blocking_toolpath_id: Some(ids[0]),
                blocking_toolpath_index: Some(0),
                message: "waiting on simulated stock after 'Rough' (index 0)".to_owned(),
            },
        );
    controller.state.gui.toolpath_rt_or_default(ids[2]).status =
        crate::state::toolpath::ComputeStatus::Error("no 3D mesh".to_owned());
    controller.state.gui.toolpath_rt_or_default(ids[3]).status =
        crate::state::toolpath::ComputeStatus::Error("stale text from when it was on".to_owned());
    let off_index = index_of(&controller, ids[3]);
    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: off_index,
            enabled: false,
        }))
        .expect("the index comes from the session");

    let diag = controller.build_mcp_diagnostics();
    let errors = diag["runtime_errors"].as_array().expect("runtime_errors");
    let blocked = diag["awaiting_prior_stock"]
        .as_array()
        .expect("awaiting_prior_stock");

    assert_eq!(
        errors.len(),
        1,
        "only the genuinely-failing op belongs in runtime_errors, got: {errors:?}"
    );
    assert_eq!(errors[0]["error"], "no 3D mesh");
    assert_eq!(blocked.len(), 1, "got: {blocked:?}");
    assert_eq!(blocked[0]["blocking_toolpath_index"], 0);

    let rows = diag["per_toolpath"].as_array().expect("per_toolpath");
    let off = rows
        .iter()
        .find(|r| r["toolpath_id"] == serde_json::json!(ids[3]))
        .expect("disabled op should still be listed");
    assert_eq!(off["status"], "Disabled");
    assert!(
        off["error"].is_null(),
        "a disabled op must not present a live-looking error, got: {off}"
    );
}

// ── A/M11: generate_all as a fixpoint over the rest-stock chain ──────────

/// A backend that models the one rule that makes the ladder necessary:
/// prior stock for an operation appears only when a **simulation** runs
/// AFTER its predecessor has generated. Toolpath submits succeed
/// immediately; each simulation publishes a prior-stock snapshot for every
/// op whose immediate predecessor in `chain` has generated by then.
#[cfg(feature = "mcp")]
struct RestChainBackend {
    /// Toolpath ids in index order — the stock chain.
    chain: Vec<ToolpathId>,
    generated: std::collections::HashSet<ToolpathId>,
    drained: Vec<ComputeMessage>,
    pub simulations: usize,
    /// When set, this toolpath always fails to generate.
    poison: Option<ToolpathId>,
}

#[cfg(feature = "mcp")]
impl RestChainBackend {
    fn new() -> Self {
        Self {
            chain: Vec::new(),
            generated: std::collections::HashSet::new(),
            drained: Vec::new(),
            simulations: 0,
            poison: None,
        }
    }
}

#[cfg(feature = "mcp")]
impl ComputeBackend for RestChainBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        // The real lane stamps the revision `start` read (`worker.rs`), so the
        // drain adopts at that revision. A fake that reports `None` adopts at
        // the CURRENT revision and accepts a result the operator has edited past.
        let revision = Some(request.handle.revision);
        let result = if self.poison == Some(id) {
            Err(crate::compute::ComputeError::Message(
                "synthetic hard failure".to_owned(),
            ))
        } else {
            self.generated.insert(id);
            Ok(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            })
        };
        self.drained.push(ComputeMessage::Toolpath(Box::new(
            crate::compute::ComputeResult {
                toolpath_id: id,
                revision,
                result,
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
        ToolpathSubmitOutcome::Queued
    }

    fn submit_simulation(&mut self, _request: SimulationRequest) {
        self.simulations += 1;
        let bbox = rs_cam_core::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        let mut prior_stocks = std::collections::HashMap::new();
        for window in self.chain.windows(2) {
            let (prev, next) = (window[0], window[1]);
            if self.generated.contains(&prev) {
                prior_stocks.insert(
                    next,
                    Arc::new(rs_cam_core::dexel_stock::TriDexelStock::from_bounds(
                        &bbox, 2.0,
                    )),
                );
            }
        }
        self.drained
            .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
                core: rs_cam_core::compute::simulate::SimulationResult {
                    mesh: rs_cam_core::stock::stock_mesh::StockMesh {
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
                    column_grid_cell_mm: 0.5,
                    resolution_clamped: false,
                    prior_stocks,
                },
                playback_data: Vec::new(),
                cut_trace_path: None,
            }))));
    }

    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

/// Build a project whose toolpaths form a `depth`-deep rest chain: index 0
/// cuts fresh stock, every later op takes the remaining stock of the one
/// before it.
#[cfg(feature = "mcp")]
fn rest_chain_controller(depth: usize) -> AppController<RestChainBackend> {
    let mut controller = AppController::with_backend(RestChainBackend::new());
    sample_project_into(&mut controller);
    for i in 1..=depth {
        push_toolpath(&mut controller, &format!("Rest {i}"));
    }
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    for index in 1..controller.state.session.toolpath_count() {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index,
                source: crate::state::toolpath::StockSource::FromRemainingStock,
            }))
            .expect("the index comes from the session");
    }
    // The fixture op at index 0 starts with a cached result; clear it so the
    // run really is "from cold".
    for id in &ids {
        let rt = controller.state.gui.toolpath_rt_or_default(*id);
        rt.result = None;
        rt.status = crate::state::toolpath::ComputeStatus::Pending;
    }
    controller.compute.chain = ids;
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    controller
}

/// Drive the controller until the generate_all oneshot resolves, or give up.
/// Returns the parsed response and the number of pump iterations.
#[cfg(feature = "mcp")]
fn pump_until_resolved(
    controller: &mut AppController<RestChainBackend>,
    rx: &mut tokio::sync::oneshot::Receiver<crate::mcp_bridge::McpResponse>,
) -> serde_json::Value {
    for _ in 0..200 {
        let events = controller.drain_events();
        for event in events {
            controller.handle_internal_event(event);
        }
        controller.drain_compute_results();
        if let Ok(resp) = rx.try_recv() {
            let payload = resp.result.expect("generate_all replies Ok(json)");
            return serde_json::from_str(&payload)
                .unwrap_or_else(|e| panic!("generate_all reply is not JSON ({e}): {payload}"));
        }
    }
    panic!("generate_all never resolved — the fixpoint loop is not terminating");
}

/// THE A/M11 acceptance gate. A 3-deep rest chain reaches fully generated
/// from cold in ONE `generate_all`, and the call reports how many internal
/// rounds it took.
///
/// Before this, the same project needed three manual sim -> generate rounds
/// and nothing told the operator that `k` was three.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_drives_a_three_deep_rest_chain_to_fixpoint_in_one_call() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);

    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["ok"], true, "reply: {reply}");
    assert_eq!(
        reply["generated"], 4,
        "every op in the chain must end up generated, reply: {reply}"
    );
    assert_eq!(reply["failed"], 0);
    assert!(
        reply["awaiting_prior_stock"]
            .as_array()
            .expect("array")
            .is_empty(),
        "nothing may still be blocked at the fixpoint, reply: {reply}"
    );
    // One round per link plus the initial pass — and the caller is TOLD.
    assert_eq!(reply["rounds"], 4, "reply: {reply}");
    assert_eq!(reply["simulations"], 3, "reply: {reply}");
    assert_eq!(controller.compute.simulations, 3);

    for tc in controller.state.session.toolpath_configs() {
        let rt = controller.state.gui.toolpath_rt.get(&tc.id);
        assert!(
            rt.is_some_and(|rt| matches!(rt.status, crate::state::toolpath::ComputeStatus::Done)),
            "'{}' should be Done at the fixpoint, got {:?}",
            tc.name,
            rt.map(|rt| rt.status.label())
        );
    }
}

/// The loop must stop on a genuinely-failing op instead of spinning: a hard
/// failure is never retried, so condition (a) — "something is blocked purely
/// on sequencing" — goes false and the round count stays bounded.
#[cfg(feature = "mcp")]
#[test]
fn the_fixpoint_loop_terminates_on_a_genuinely_failing_op() {
    let mut controller = rest_chain_controller(3);
    // Poison the middle link. Everything downstream can then never see stock.
    let poisoned = controller.state.session.toolpath_configs()[1].id;
    controller.compute.poison = Some(poisoned);

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["ok"], false, "a hard failure must not report ok");
    assert_eq!(reply["failed"], 1, "reply: {reply}");
    assert!(
        reply["errors"][0]["message"]
            .as_str()
            .expect("error message")
            .contains("synthetic hard failure"),
        "the final error must be clear about what failed, reply: {reply}"
    );
    let rounds = reply["rounds"].as_u64().expect("rounds");
    assert!(
        (1..=4).contains(&rounds),
        "rounds must stay inside the hard bound (rest ops + 1 = 4), got {rounds}"
    );
    // The ops downstream of the failure are reported as still waiting, each
    // naming what it waits for — not as failures of their own.
    let blocked = reply["awaiting_prior_stock"].as_array().expect("array");
    assert!(
        !blocked.is_empty(),
        "downstream ops are blocked, not broken, reply: {reply}"
    );
}

/// N12 item 10 — the GUI's own simulation reaches the SESSION.
///
/// `ProjectSession::start` reads the rest snapshot from
/// `session.simulation.prior_stocks`. The GUI simulates on its own lane
/// and adopted the answer into viz state alone, so the session held no
/// simulation in the GUI process and `start` refused every
/// `FromRemainingStock` operation there. Two simulation states.
///
/// The arm drives the real doors, in the order the operator does:
///
/// 1. the submit door generates the first link;
/// 2. the GUI's own `run_simulation_with_all` publishes the snapshot;
/// 3. the drain adopts it, into viz state AND into the session.
///
/// It then asserts the two hold ONE snapshot, and that `start` on the
/// rest operation succeeds.
///
/// Pre-fix this fails at the `Arc::ptr_eq` assertion: the session holds
/// no simulation at all, and `start` reports the rest refusal.
///
/// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
/// §22 addendum. The core half is
/// `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs`.
#[cfg(feature = "mcp")]
#[test]
fn a_gui_simulation_reaches_the_session_so_start_sees_the_prior_stock_n12_item10() {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "the fixture holds one fresh-stock op and one rest op"
    );

    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&ids[0])
            .is_some_and(|rt| rt.result.is_some()),
        "the first link must generate, or the simulation below carves \
         nothing and this arm measures nothing"
    );

    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or this arm measures nothing"
    );
    controller.drain_compute_results();

    let viz_stock = controller
        .state
        .simulation
        .prior_stock_for(ids[1])
        .map(Arc::clone)
        .expect("the GUI adopts the snapshot into viz state");
    let session_stock = controller
        .state
        .session
        .simulation_result()
        .and_then(|simulation| simulation.prior_stocks.get(&ids[1]))
        .map(Arc::clone);
    assert!(
        session_stock
            .as_ref()
            .is_some_and(|stock| Arc::ptr_eq(stock, &viz_stock)),
        "the session and the viewport must hold ONE snapshot; the session \
         holds {}",
        if session_stock.is_some() {
            "another"
        } else {
            "none"
        }
    );

    let cancel = std::sync::atomic::AtomicBool::new(false);
    let refusal = controller
        .state
        .session
        .start(
            rs_cam_core::session::Job::GenerateToolpath(
                rs_cam_core::session::GenerateToolpathArgs { index: 1 },
            ),
            &cancel,
        )
        .err()
        .map(|error| error.to_string());
    assert!(
        refusal.is_none(),
        "start must see the prior stock the GUI simulated; got: {refusal:?}"
    );
}

/// WP17 (tech-debt review H1) — a save keeps the simulation.
///
/// `save_job_to_path` called `ProjectSession::set_post_config` on every
/// save, with the post block the session already held, and that setter
/// wrote `simulation = None`. `ProjectSession::start` reads the rest
/// snapshot from that field (WP11b), so every `FromRemainingStock`
/// operation was refused after a save, and nothing re-adopted.
///
/// The arm drives the real doors, in the order the operator does:
///
/// 1. the submit door generates the first link;
/// 2. the GUI's own simulation publishes the snapshot;
/// 3. the drain adopts it into the session;
/// 4. the Save menu's own function writes the file.
///
/// It then asserts the session still holds the simulation, and that
/// `start` on the rest operation succeeds.
///
/// Pre-fix this fails at the first assertion after the save: the session
/// holds no simulation, and `start` reports the rest refusal.
///
/// The core half is
/// `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs`.
#[cfg(feature = "mcp")]
#[test]
fn a_save_keeps_the_simulation_so_a_rest_op_still_starts_wp17() {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "the fixture holds one fresh-stock op and one rest op"
    );

    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or this arm measures nothing"
    );
    controller.drain_compute_results();
    assert!(
        controller.state.session.simulation_result().is_some(),
        "the control: the session holds a simulation BEFORE the save"
    );

    let path = temp_path("wp17_save_keeps_simulation", "toml");
    controller
        .save_job_to_path(&path)
        .expect("the save writes the fixture file");

    assert!(
        controller.state.session.simulation_result().is_some(),
        "a save keeps the simulation; the rest operations depend on it"
    );
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let refusal = controller
        .state
        .session
        .start(
            rs_cam_core::session::Job::GenerateToolpath(
                rs_cam_core::session::GenerateToolpathArgs { index: 1 },
            ),
            &cancel,
        )
        .err()
        .map(|error| error.to_string());
    assert!(
        refusal.is_none(),
        "the rest operation still starts after a save; got: {refusal:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// WP17 — the MCP `save_project` conversion writes no session state.
///
/// `app/mcp/commands.rs` states the contract at the top of the file: the
/// conversion step READS the session and turns a wire request into a
/// command. The `save_project` arm mutated instead — it called
/// `set_post_config` — and that write is what dropped the simulation.
///
/// The arm reads the source of the conversion rather than the running
/// app, because `RsCamApp` holds a window and a lane and no test builds
/// one. It measures the discriminator alone: `state_mut()` inside the
/// arm. The repaired arm still READS the session on both sides of the
/// comparison it makes.
///
/// Pre-fix the arm contains `state_mut()`.
#[cfg(feature = "mcp")]
#[test]
fn the_mcp_save_conversion_writes_no_session_state_wp17() {
    const COMMANDS_SRC: &str = include_str!("../app/mcp/commands.rs");
    let at = COMMANDS_SRC
        .find("CoreRequest::SaveProject(p) => {")
        .expect("core_command_for holds a save_project arm");
    let tail = &COMMANDS_SRC[at..];
    let arm = match tail.find("\n            CoreRequest::") {
        Some(end) => &tail[..end],
        None => tail,
    };
    assert!(
        !arm.contains("state_mut()"),
        "the conversion step reads; it must not write. The save_project \
         arm reads:\n{arm}"
    );
}

/// A/M10's rule, enforced: the loop never picks a simulation resolution for
/// you. Omitting it on a project with rest ops is refused, with instructions.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_refuses_to_guess_a_simulation_resolution() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, None, tx, None);

    let resp = rx
        .try_recv()
        .expect("the refusal is immediate — nothing is submitted");
    let payload = resp.result.expect("Ok(json)");
    let reply: serde_json::Value = serde_json::from_str(&payload).expect("json");
    assert_eq!(reply["ok"], false);
    let err = reply["error"].as_str().expect("error text");
    assert!(err.contains("simulation_resolution_mm"), "got: {err}");
    assert!(
        err.contains("fixpoint: false"),
        "the refusal must say how to opt out, got: {err}"
    );
    assert!(
        err.contains("NOT guessed"),
        "the refusal must say why, got: {err}"
    );
    assert!(
        controller.drain_events().is_empty(),
        "nothing was submitted"
    );
}

/// `fixpoint: false` is the pre-A/M11 single pass: no simulation, no
/// resolution needed, blocked ops reported rather than retried.
#[cfg(feature = "mcp")]
#[test]
fn fixpoint_false_keeps_the_old_single_pass_behaviour() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["rounds"], 1);
    assert_eq!(reply["simulations"], 0);
    assert_eq!(controller.compute.simulations, 0);
    assert_eq!(
        reply["generated"], 1,
        "only the fresh-stock op, reply: {reply}"
    );
    assert_eq!(
        reply["awaiting_prior_stock"]
            .as_array()
            .expect("array")
            .len(),
        3,
        "reply: {reply}"
    );
}

/// A disabled rest op is skipped entirely: not generated, not blocked, not an
/// error — and it does not extend the ladder.
#[cfg(feature = "mcp")]
#[test]
fn a_disabled_rest_op_is_not_generated_blocked_or_failed() {
    let mut controller = rest_chain_controller(3);
    let off = controller.state.session.toolpath_configs()[3].id;
    let off_index = index_of(&controller, off);
    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: off_index,
            enabled: false,
        }))
        .expect("the index comes from the session");

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["generated"], 3, "reply: {reply}");
    assert_eq!(reply["failed"], 0);
    let mentions_off = format!("{reply}").contains(&format!("\"toolpath_id\":{}", off.0));
    assert!(
        !mentions_off,
        "a disabled op must appear in neither the error nor the blocked list, reply: {reply}"
    );
    // And it reports Disabled rather than whatever it last recorded.
    let raw = controller
        .state
        .gui
        .toolpath_rt
        .get(&off)
        .map_or(&crate::state::toolpath::ComputeStatus::Pending, |rt| {
            &rt.status
        });
    assert_eq!(
        crate::state::toolpath::ComputeStatus::effective(false, raw).label(),
        "Disabled"
    );
}

// ── G-REGEN-RACE: generate_all and process_auto_regen stop cancelling ────
//
// Reproduced live by B-4b on a never-minimised window and re-derived from
// the code here: `load_project` marks every toolpath `auto_regen` + stale,
// `process_auto_regen` submits them 500 ms later, and an agent's
// `generate_all` arriving while one of them is still the lane's ACTIVE job
// resubmits that same toolpath. The lane's resubmit-cancels-and-requeues
// rule aborts the in-flight job and queues the replacement; the abandoned
// job's `Cancelled` then drained as the toolpath's *outcome*, so
// `generate_all` reported `"Back Rough: generation cancelled"`,
// `generated: 0` for work it had itself replaced — while the replacement it
// queued went on to succeed unobserved.

/// A backend that models the toolpath lane's two load-bearing rules and
/// nothing else: (1) a submit for the toolpath that is currently ACTIVE
/// cancels that job and queues the replacement; (2) a cancelled job still
/// reports, with `Err(Cancelled)`. `finish_active` stands for the worker
/// thread completing whatever it is running.
#[cfg(feature = "mcp")]
struct LaneModelBackend {
    active: Option<ToolpathId>,
    active_cancelled: bool,
    queue: std::collections::VecDeque<ToolpathId>,
    drained: Vec<ComputeMessage>,
    /// Every toolpath id ever handed to `submit_toolpath`, in order.
    submits: Vec<ToolpathId>,
    /// The revision each submit read, kept the way the real lane keeps it on
    /// the handle. The completion stamps it, so the drain adopts at the
    /// revision the submit answered and refuses a result the operator has
    /// edited past.
    revisions: std::collections::HashMap<ToolpathId, u64>,
}

#[cfg(feature = "mcp")]
impl LaneModelBackend {
    fn new() -> Self {
        Self {
            active: None,
            active_cancelled: false,
            queue: std::collections::VecDeque::new(),
            drained: Vec::new(),
            submits: Vec::new(),
            revisions: std::collections::HashMap::new(),
        }
    }

    fn start_next(&mut self) {
        if self.active.is_none() {
            self.active = self.queue.pop_front();
            self.active_cancelled = false;
        }
    }

    /// The worker finishing its current job. A job whose cancel flag was
    /// set reports `Cancelled` even though it ran, exactly as
    /// `spawn_toolpath_lane` does.
    fn finish_active(&mut self) {
        let Some(id) = self.active.take() else {
            return;
        };
        let result = if self.active_cancelled {
            Err(crate::compute::ComputeError::Cancelled)
        } else {
            Ok(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            })
        };
        self.active_cancelled = false;
        let revision = self.revisions.get(&id).copied();
        self.drained.push(ComputeMessage::Toolpath(Box::new(
            crate::compute::ComputeResult {
                toolpath_id: id,
                revision,
                result,
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
        self.start_next();
    }
}

#[cfg(feature = "mcp")]
impl ComputeBackend for LaneModelBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        self.submits.push(id);
        self.revisions.insert(id, request.handle.revision);
        self.queue.retain(|queued| *queued != id);
        let outcome = if self.active == Some(id) {
            self.active_cancelled = true;
            ToolpathSubmitOutcome::SupersededActive
        } else {
            ToolpathSubmitOutcome::Queued
        };
        self.queue.push_back(id);
        self.start_next();
        outcome
    }

    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {
        if self.active.is_some() {
            self.active_cancelled = true;
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }

    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

/// A one-op project in exactly the state `load_project` leaves behind:
/// auto-regen armed, stale, no cached result.
#[cfg(feature = "mcp")]
fn freshly_loaded_controller() -> (AppController<LaneModelBackend>, ToolpathId) {
    let mut controller = AppController::with_backend(LaneModelBackend::new());
    sample_project_into(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let rt = controller.state.gui.toolpath_rt_or_default(tp_id);
    rt.result = None;
    rt.status = crate::state::toolpath::ComputeStatus::Pending;
    rt.auto_regen = true;
    // Older than the 500 ms debounce, i.e. the sweep is due.
    rt.stale_since = Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    (controller, tp_id)
}

/// Pump the controller — events, then the lane finishing a job, then the
/// drain — until the `generate_all` oneshot resolves.
#[cfg(feature = "mcp")]
fn pump_lane_model(
    controller: &mut AppController<LaneModelBackend>,
    rx: &mut tokio::sync::oneshot::Receiver<crate::mcp_bridge::McpResponse>,
) -> serde_json::Value {
    for _ in 0..200 {
        let events = controller.drain_events();
        for event in events {
            controller.handle_internal_event(event);
        }
        controller.compute.finish_active();
        controller.drain_compute_results();
        if let Ok(resp) = rx.try_recv() {
            let payload = resp.result.expect("generate_all replies Ok(json)");
            return serde_json::from_str(&payload)
                .unwrap_or_else(|e| panic!("generate_all reply is not JSON ({e}): {payload}"));
        }
    }
    panic!("generate_all never resolved");
}

/// THE G-REGEN-RACE gate. `generate_all` issued while the GUI's own
/// auto-regen sweep still has that toolpath in flight must report the work
/// it actually got — not a failure for the job it replaced itself.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_does_not_report_its_own_supersede_as_a_failure() {
    let (mut controller, tp_id) = freshly_loaded_controller();

    // 1. The GUI's own sweep fires first (this is guaranteed after a
    //    load_project: the debounce is 500 ms and every toolpath is stale).
    controller.process_auto_regen();
    assert_eq!(
        controller.compute.active,
        Some(tp_id),
        "the auto-regen sweep should have put this toolpath on the lane"
    );

    // 2. The agent calls generate_all while that job is still running.
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);

    let reply = pump_lane_model(&mut controller, &mut rx);

    assert_eq!(reply["ok"], true, "reply: {reply}");
    assert_eq!(
        reply["generated"], 1,
        "the replacement generate_all queued DID produce a toolpath; the reply \
         must count it. Pre-fix this read 0. reply: {reply}"
    );
    assert_eq!(reply["failed"], 0, "reply: {reply}");
    let errors = reply["errors"].as_array().expect("errors array");
    assert!(
        errors.is_empty(),
        "generate_all must not report a failure for the job it superseded \
         itself — pre-fix this carried \"Scallop: generation cancelled\". \
         reply: {reply}"
    );
    assert!(
        controller.superseded_toolpaths.is_empty(),
        "the supersede ledger must be empty once the cancellation it was \
         expecting has drained"
    );
    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime exists");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "the toolpath really did generate, got {:?}",
        rt.status.label()
    );
}

/// The other half of the same contract, and the reason the fix is a
/// supersede ledger rather than "ignore Cancelled": a cancellation with no
/// supersede behind it — `cancel_generation`, the GUI's cancel button — is
/// still terminal and still reported.
#[cfg(feature = "mcp")]
#[test]
fn a_genuine_cancel_is_still_reported_by_generate_all() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);

    // Let generate_all's own submit reach the lane, then cancel it the way
    // the escape hatch does — nobody resubmitted, so nothing was superseded.
    let events = controller.drain_events();
    for event in events {
        controller.handle_internal_event(event);
    }
    assert_eq!(controller.compute.active, Some(tp_id));
    assert!(
        controller.superseded_toolpaths.is_empty(),
        "a submit onto an idle lane supersedes nothing"
    );
    controller.compute.cancel_lane(ComputeLane::Toolpath);

    let reply = pump_lane_model(&mut controller, &mut rx);

    assert_eq!(reply["generated"], 0, "reply: {reply}");
    assert_eq!(reply["failed"], 1, "reply: {reply}");
    let errors = reply["errors"].as_array().expect("errors array");
    assert_eq!(errors.len(), 1, "reply: {reply}");
    assert!(
        errors[0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cancelled"),
        "a real cancellation must still say so: {reply}"
    );
}

/// Interactive auto-regen keeps working, and the supersede path improves
/// it: a param edit while the previous generate is still running no longer
/// bounces the toolpath's status through `Pending` (which the operations
/// tree renders as "not generated") on its way to the new result.
#[cfg(feature = "mcp")]
#[test]
fn a_param_edit_mid_generate_regenerates_without_a_pending_flicker() {
    let (mut controller, tp_id) = freshly_loaded_controller();

    // First edit: the sweep puts it on the lane.
    controller.process_auto_regen();
    assert_eq!(controller.compute.active, Some(tp_id));

    // Second edit lands while that job runs — the GUI marks it stale again
    // and the next sweep resubmits it.
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
    controller.process_auto_regen();
    assert!(
        controller.superseded_toolpaths.contains(&tp_id),
        "resubmitting the running toolpath supersedes it"
    );

    // The abandoned job reports; the toolpath is still computing.
    controller.compute.finish_active();
    controller.drain_compute_results();
    let status = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .map(|rt| rt.status.label());
    assert_eq!(
        status,
        Some("Computing"),
        "a superseded job is not an outcome — the replacement is already \
         queued, so the toolpath is still computing"
    );

    // The replacement lands and the edit is honoured.
    controller.compute.finish_active();
    controller.drain_compute_results();
    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime exists");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "auto-regen must still deliver a fresh result, got {:?}",
        rt.status.label()
    );
    assert!(
        rt.stale_since.is_none(),
        "the submit clears the staleness it satisfies"
    );
}

/// The manual-regeneration arm of the same race — F2.4, G-LATERESULT.
///
/// The test above covers the AUTO arm, and that arm was already safe by a
/// route that has nothing to do with freshness: a second edit resubmits, the
/// resubmit supersedes, and G-REGEN-RACE drops the abandoned result before it
/// reaches the store. **Nothing supersedes on a 3D manual-regen operation.**
/// The operator presses G, edits a parameter while the long generation runs,
/// and the result that lands was computed from the parameter set they just
/// left. It was written into `session.results`, which is what
/// `FreshnessState` reads for `Current`, so every surface F2.2 wired reported
/// the edit as answered and the export gate F2.3 built would have let it
/// through.
///
/// The row's requirement is "stored but marked stale, never shown as
/// current", and all three halves are asserted here: the geometry survives on
/// `rt.result` (a discarded result costs a long generation and explains
/// nothing), the core slot stays empty, and the state reads `EditedSince`.
#[cfg(feature = "mcp")]
#[test]
fn a_param_edit_mid_generate_manual_arm_keeps_the_edit_g_lateresult() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    // The arm under test: 3D ops default to manual regeneration, so no sweep
    // will resubmit and nothing will supersede.
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .auto_regen = false;
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    // The operator presses G. One job, on the lane.
    controller.handle_internal_event(AppEvent::GenerateToolpath(tp_id));
    assert_eq!(
        controller.compute.active,
        Some(tp_id),
        "the manual generate must reach the lane"
    );
    // WP11b: the revision rides the HANDLE, not a GUI runtime field. The
    // lane holding this toolpath is the evidence that a job carrying it is
    // in flight.

    // While it runs, they change a parameter through the panel's own
    // write-back — the same door every inspector edit goes through.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(1234.0);
    });
    assert!(
        controller.state.session.get_result(0).is_none(),
        "the edit drops the core result (F2.1); this test is about what \
         happens when the in-flight job then lands"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Regenerating,
        "while the lane holds it the honest state is Regenerating — the lane's \
         status precedes the cache check in `freshness`, and the operator is \
         told work is in progress rather than that it is stale"
    );

    // The job finishes and reports. Nothing superseded it.
    controller.compute.finish_active();
    controller.drain_compute_results();

    assert!(
        !controller.superseded_toolpaths.contains(&tp_id),
        "no resubmit happened, so this is not the G-REGEN-RACE path"
    );
    let rt = &controller.state.gui.toolpath_rt[&tp_id];
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "the generation finished and the status says so, got {:?}",
        rt.status.label()
    );
    assert!(
        rt.result.is_some(),
        "STORED, not discarded: the geometry stays drawable so the operator \
         does not lose a long 3D generation and get told nothing"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "but NOT into the core cache — that is what `Current` is read from, \
         and this result answers the parameter set the operator just left"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "so the card, the header, the chips and the export gate all still say \
         the edit is unanswered"
    );
}

/// The other side of the same guard: with no edit in flight, a manual
/// generate's result IS accepted. A guard that rejected everything would pass
/// the test above and break the application.
#[cfg(feature = "mcp")]
#[test]
fn a_manual_generate_with_no_edit_is_accepted_g_lateresult() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .auto_regen = false;
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    controller.handle_internal_event(AppEvent::GenerateToolpath(tp_id));
    controller.compute.finish_active();
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_some(),
        "an unedited generation must land in the core cache"
    );
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
}

// ── Phase U: the multi-tool finishing planner dialog ─────────────────────
//
// The claim these carry is the VETO: opening the dialog and rejecting it must
// leave the project exactly as it was. The core sentry
// (`multitool_preview_u1.rs`) makes the byte-identical version of that claim
// about `preview_multitool_plan` itself; these make it about the GUI path,
// where the session is lent to a worker and handed back.

/// The sample project plus a two-ball ladder with distinct tip radii.
fn planner_controller() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    let mut tools = controller.state.session.tools().to_vec();
    for (raw_id, diameter) in [(2usize, 4.0f64), (3, 2.0)] {
        let mut tool = ToolConfig::new_default(ToolId(raw_id), ToolType::BallNose);
        tool.name = format!("Ball {diameter}");
        tool.diameter = diameter;
        tools.push(tool);
    }
    let _ = controller
        .state
        .session
        .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
        .expect("the session takes the tool list");
    controller
}

/// Tick every ball-nose row, which is the two-radius ladder the dialog needs.
fn tick_ball_tools(controller: &mut AppController<ScriptedBackend>) {
    let planner = controller
        .state
        .multitool_planner
        .as_mut()
        .expect("the dialog is open");
    for row in &mut planner.tools {
        row.selected = row.name.starts_with("Ball ");
    }
}

/// A fingerprint of everything the planner is allowed to leave alone.
fn project_fingerprint(controller: &AppController<ScriptedBackend>) -> Vec<String> {
    controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| {
            format!(
                "{}|{}|{}|{:?}",
                tc.name,
                tc.tool_id,
                tc.enabled,
                tc.planner_origin.as_ref().map(|o| (o.plan_id, o.tier))
            )
        })
        .collect()
}

#[test]
fn opening_the_planner_emits_nothing_and_pre_ticks_nothing() {
    let mut controller = planner_controller();
    let before = project_fingerprint(&controller);

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));

    let planner = controller
        .state
        .multitool_planner
        .as_ref()
        .expect("the dialog opened");
    assert!(planner.open);
    assert!(
        planner.tools.iter().all(|row| !row.selected),
        "which tools take part is the operator's call — nothing is pre-ticked"
    );
    assert!(
        planner.blocking_reason().is_some(),
        "and with nothing ticked, Preview and Apply say why they are refused"
    );
    // Coarse -> fine on TIP radius, the ordering the core ladder uses.
    let radii: Vec<f64> = planner.tools.iter().map(|r| r.cusp_radius_mm).collect();
    assert!(
        radii.windows(2).all(|pair| pair[0] >= pair[1]),
        "rows read in the order the chain runs, got {radii:?}"
    );
    assert_eq!(
        project_fingerprint(&controller),
        before,
        "no op was touched"
    );
}

#[test]
fn the_dialogs_dials_reach_the_submitted_spec() {
    let mut controller = planner_controller();
    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));
    tick_ball_tools(&mut controller);
    {
        let planner = controller.state.multitool_planner.as_mut().expect("open");
        planner.coarseness = 4.0;
        planner.overlap_mm = 1.5;
        planner.cusp_height_mm = 0.02;
        planner.tolerance_mm = 0.08;
        planner.cell_mm = 0.6;
        planner.margin_mm = 0.75;
        planner.close_radius_mm = Some(0.9);
        planner.min_region_area_mm2 = Some(12.0);
        planner.max_regions_per_tier = 8;
        planner.rim_erosion_mm = 3.0;
    }

    let before_ops = controller.state.session.toolpath_configs().len();
    controller.handle_internal_event(crate::ui::AppEvent::PreviewMultitoolPlan);

    assert!(
        controller.state.optimize_run.is_some(),
        "one Optimize run at a time — the walk holds the policy state"
    );
    assert!(
        controller
            .state
            .multitool_planner
            .as_ref()
            .is_some_and(|p| p.is_loading())
    );
    let request = controller
        .compute
        .job_requests
        .first()
        .expect("a preview was submitted");
    let rs_cam_core::session::JobHandle::PreviewTierMap(handle) = &request.handle else {
        panic!("the planner must submit the preview_tier_map Job row");
    };
    let spec = handle.spec();
    assert_eq!(
        controller.state.session.toolpath_configs().len(),
        before_ops,
        "WP14b: the session is NOT lent out — the handle carries the walk's \
         own inputs and the view keeps its project"
    );
    assert_eq!(spec.tool_ids.len(), 2);
    assert!((spec.cell_mm - 0.6).abs() < 1e-12);
    assert!((spec.tolerance_mm - 0.08).abs() < 1e-12);
    assert!((spec.margin_mm - 0.75).abs() < 1e-12);
    assert!((spec.cusp_height_mm - 0.02).abs() < 1e-12);
    assert!((spec.islands.coarseness - 4.0).abs() < 1e-12);
    assert!((spec.islands.overlap_mm - 1.5).abs() < 1e-12);
    assert!((spec.islands.rim_erosion_mm - 3.0).abs() < 1e-12);
    assert_eq!(spec.islands.max_regions_per_tier, 8);
    assert_eq!(spec.islands.close_radius_mm, Some(0.9));
    assert_eq!(spec.islands.min_region_area_mm2, Some(12.0));
}

/// THE VETO. Close leaves the project alone, drops the overlay, and keeps the
/// dials so re-opening resumes rather than restarts (§3.1).
#[test]
fn vetoing_the_planner_leaves_the_project_alone_and_drops_the_overlay() {
    let mut controller = planner_controller();
    let before = project_fingerprint(&controller);
    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));
    tick_ball_tools(&mut controller);
    controller
        .state
        .multitool_planner
        .as_mut()
        .expect("open")
        .coarseness = 6.5;
    controller.state.viewport.show_tier_preview = true;

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::CloseMultitoolPlanner(
        NoArgs,
    )));

    assert!(!controller.state.viewport.show_tier_preview);
    assert_eq!(project_fingerprint(&controller), before);
    let planner = controller
        .state
        .multitool_planner
        .as_ref()
        .expect("the dials survive a veto");
    assert!(!planner.open);
    assert!((planner.coarseness - 6.5).abs() < 1e-12, "dials survive");
    assert_eq!(planner.selected_tool_ids().len(), 2, "ticks survive");

    // Re-opening resumes the same dialog rather than a fresh one.
    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));
    let planner = controller.state.multitool_planner.as_ref().expect("open");
    assert!(planner.open);
    assert!((planner.coarseness - 6.5).abs() < 1e-12);
    assert_eq!(planner.selected_tool_ids().len(), 2);
}

/// Apply with nothing previewed is a no-op, not a plan. The whole point of
/// the veto is that the operator sees the territory first.
#[test]
fn applying_without_a_preview_plans_nothing() {
    let mut controller = planner_controller();
    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));
    tick_ball_tools(&mut controller);
    let before = project_fingerprint(&controller);

    controller.handle_internal_event(crate::ui::AppEvent::ApplyMultitoolPlan);

    assert_eq!(project_fingerprint(&controller), before);
    assert!(
        controller
            .state
            .multitool_planner
            .as_ref()
            .is_some_and(|p| p.open),
        "and the dialog stays up rather than silently closing"
    );
}

/// A ladder the core would refuse never reaches the lane — the dialog says
/// why instead of submitting a walk that ends in an error dialog.
#[test]
fn a_one_tool_ladder_never_reaches_the_worker() {
    let mut controller = planner_controller();
    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::OpenMultitoolPlanner(
        NoArgs,
    )));
    if let Some(row) = controller
        .state
        .multitool_planner
        .as_mut()
        .expect("open")
        .tools
        .first_mut()
    {
        row.selected = true;
    }

    controller.handle_internal_event(crate::ui::AppEvent::PreviewMultitoolPlan);

    assert!(controller.compute.job_requests.is_empty());
    assert!(controller.state.optimize_run.is_none());
}

/// The tier preview's setup gate (operator-observed 2026-08-27): the map is
/// in its own setup's emission frame, so viewing ANY other setup must read
/// as "not the previewed setup" — the overlay otherwise renders wrongly
/// shifted beside the flipped stock. `active_setup_index` is the predicate
/// both the callback gate and the GPU upload key share.
#[test]
fn active_setup_index_follows_the_selection() {
    let mut controller = sample_controller();
    let second = controller
        .state
        .session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Back".to_owned()),
            face_up: rs_cam_core::compute::transform::FaceUp::default(),
        }))
        .expect("the session accepts a second setup")
        .created
        .expect("the AddSetup row reports the new setup index");

    // Nothing setup-scoped selected: the first setup's frame is displayed.
    controller.state.selection = Selection::None;
    assert_eq!(controller.state.active_setup_index(), Some(0));

    // Selecting the second setup moves the display frame with it.
    let second_id = controller.state.session.list_setups()[second].id;
    controller.state.selection = Selection::Setup(crate::state::job::SetupId(second_id));
    assert_eq!(controller.state.active_setup_index(), Some(second));

    // A toolpath selection resolves through its owning setup.
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    controller.state.selection = Selection::Toolpath(tp_id);
    assert_eq!(
        controller.state.active_setup_index(),
        controller.state.session.setup_of_toolpath_id(tp_id)
    );
}

// ── G-FRESHSTATE — one freshness model (F2.1) ────────────────────────
//
// R0.1 (`planning/ui_fix_2026-09-09/research/R0.1.md`) §2.3 is a matrix of
// every mutation and what each of the three staleness stores did with it.
// The tests below walk that matrix against the ONE state the surfaces now
// read: `state::freshness::freshness_at`, derived from the core result
// cache. Each row asserts the state, and — where the row is one of the
// eight the core used to keep a result through — that the core result is
// actually gone, which is what stopped export emitting geometry for a
// configuration the project no longer had.

use crate::state::freshness::{FreshnessState, freshness_at};
use rs_cam_core::session::ToolpathComputeResult;

/// A cached core result, standing for "this toolpath has been generated".
fn core_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(Toolpath::new()),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Put every toolpath in the project into the `Current` state: a core
/// result, a `Done` status, and a drawable GUI result (the fixture already
/// gives toolpath 0 one).
fn generate_all_for_test<B: ComputeBackend>(controller: &mut AppController<B>) {
    let ids: Vec<(usize, ToolpathId)> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .enumerate()
        .map(|(idx, tc)| (idx, tc.id))
        .collect();
    for (index, id) in ids {
        let revision = controller.state.session.toolpath_revision(index);
        let _ = controller
            .state
            .session
            .apply(Command::AdoptResult(AdoptResultArgs {
                index,
                revision,
                result: Box::new(core_result()),
            }))
            .expect("index is in range");
        let rt = controller.state.gui.toolpath_rt_or_default(id);
        rt.status = crate::state::runtime::ComputeStatus::Done;
        rt.stale_since = None;
        if rt.result.is_none() {
            rt.result = Some(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            });
        }
    }
    controller.state.gui.dirty = false;
}

fn state_of<B: ComputeBackend>(controller: &AppController<B>, index: usize) -> FreshnessState {
    freshness_at(&controller.state.session, &controller.state.gui, index)
        .expect("toolpath index exists")
}

/// Drive the inspector's write-back exactly as the panel does: build the
/// entry from the session, mutate it the way the widget would, write it
/// back. `edit` receives the entry.
///
/// The stale stamp and the dirty flag live inside the write-back since
/// WP5, so this helper adds neither. A helper that added its own would
/// stop measuring what the panel does.
fn panel_edit<B: ComputeBackend>(
    controller: &mut AppController<B>,
    id: ToolpathId,
    edit: impl FnOnce(&mut crate::state::toolpath::ToolpathEntry),
) {
    let mut entry = crate::ui::properties::build_entry_from_session_and_gui(
        id,
        &controller.state.session,
        &controller.state.gui,
    )
    .expect("toolpath exists");
    edit(&mut entry);
    // The panel's own order. The runtime write-back runs first because it
    // copies `entry.stale_since`; the config write-back stamps the fresh
    // value. A helper that reversed the two would stop modelling the
    // panel, and the stale-stamp assertions below would pass over a
    // defect the operator sees as a green card on dropped geometry.
    crate::ui::properties::write_entry_runtime_to_gui(&entry, &mut controller.state.gui);
    let _ = crate::ui::properties::write_entry_config_to_session(&entry, &mut controller.state);
}

/// WP5. The projection protects the three fields the inspector entry
/// cannot supply.
///
/// `ToolpathEntry` carries sixteen of the nineteen `ToolpathConfig`
/// fields. `id`, `boundary_inherit` and `planner_origin` are not among
/// them. `Command::ReplaceToolpathConfig` writes whatever the caller
/// passes and protects none of the three — the core sentry
/// `replace_toolpath_config_gates_on_the_signature.rs` pins that. So the
/// protection has exactly one home: the projection clones the STORED
/// configuration and applies the sixteen fields onto the clone, never a
/// fresh literal. A fresh literal would clear `boundary_inherit`, which
/// reaches emitted geometry (G-BOUNDARYINHERIT), and would delete the
/// multi-tool planner's ownership stamp.
#[test]
fn the_projection_keeps_the_three_fields_the_entry_cannot_supply_wp5() {
    let mut controller = sample_controller();
    let id = controller.state.session.toolpath_configs()[0].id;
    let origin = rs_cam_core::session::PlannerOrigin {
        plan_id: 7,
        tier: 1,
        tier_count: 2,
    };
    {
        let stored = &controller.state.session.toolpath_configs()[0];
        assert!(
            stored.boundary_inherit,
            "the fixture must start with boundary_inherit true, or the \
             flip below asserts nothing"
        );
        let mut config = stored.clone();
        config.boundary_inherit = false;
        config.planner_origin = Some(origin.clone());
        let _ = controller
            .state
            .session
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index: 0,
                config: Box::new(config),
            }))
            .expect("index 0 exists");
    }

    let mut entry = crate::ui::properties::build_entry_from_session_and_gui(
        id,
        &controller.state.session,
        &controller.state.gui,
    )
    .expect("toolpath exists");
    // A widget cannot write these two, so the entry is the wrong place to
    // read them from. Set the id to a value the session does not carry,
    // and the projection must still answer with the stored one.
    entry.id = ToolpathId(4242);
    entry.name = "renamed".to_owned();

    let (_, stored) = controller
        .state
        .session
        .find_toolpath_config_by_id(id)
        .expect("toolpath exists");
    let projected = crate::ui::properties::project_entry_onto(stored, &entry);

    assert_eq!(
        projected.id, id,
        "the projection keeps the stored id. The entry's id names the row \
         to write, not a value to write."
    );
    assert!(
        !projected.boundary_inherit,
        "the projection keeps the stored boundary_inherit"
    );
    assert_eq!(
        projected.planner_origin.as_ref(),
        Some(&origin),
        "the projection keeps the stored planner_origin"
    );
    assert_eq!(
        projected.name, "renamed",
        "the sixteen supplied fields still land, or this test would pass \
         over a projection that copied the stored config and wrote nothing"
    );
}

/// WP5. An open panel with no edit drops no result.
///
/// The inspector holds no commit event: it rebuilds the entry, draws it,
/// and writes it back on every frame. WP5 makes that write-back one
/// `Command::ReplaceToolpathConfig` per frame, so the core gate — not the
/// panel — is what keeps a healthy card green while the operator only
/// looks at it. An ungated replacement would drop the geometry, and the
/// downstream chain with it, sixty times a second.
#[test]
fn an_open_panel_that_edits_nothing_drops_no_result_wp5() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "the fixture must start Current, or the assertions below are \
         vacuous"
    );

    // Three frames of an open panel, each one a build-draw-write cycle
    // with no widget edit at all.
    for _ in 0..3 {
        panel_edit(&mut controller, id, |_| {});
    }

    assert!(
        controller.state.session.get_result(0).is_some(),
        "a frame that moved no generation input must keep the cached \
         result"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "the card must still read OK after looking at the panel"
    );
    assert!(
        !controller.state.gui.dirty,
        "looking at the panel must not dirty the project (G-HEIGHTSTAB)"
    );
    assert!(
        controller.state.gui.toolpath_rt[&id].stale_since.is_none(),
        "no edit, no stale stamp"
    );
}

/// The whole point of the model: an edit through the panel leaves the card
/// no longer able to say "OK". Pre-fix this read `Current`, because the
/// panel wrote `tc.operation` through `find_toolpath_config_by_id_mut` and
/// no core setter ran — the core kept the previous parameter set's result
/// and `emitted_toolpaths` exported it.
#[test]
fn freshness_state_g_freshness_op_field_edit_is_edited_since() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);

    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(1234.0);
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert!(
        controller.state.session.get_result(0).is_none(),
        "the core must not keep a result for inputs that moved"
    );
    assert!(
        controller.state.gui.toolpath_rt[&id].result.is_some(),
        "the GUI keeps the old geometry so the viewport can draw it"
    );
    assert!(controller.state.gui.dirty);
    assert!(controller.state.gui.toolpath_rt[&id].stale_since.is_some());
}

/// The five inspector rows R0.1 §2.3 marked "S1 none, S2 none, S3 none" —
/// each one a green card over a result generated from other inputs.
#[test]
fn freshness_state_g_freshness_every_inspector_input_stales() {
    /// One inspector row: a label and the widget edit it stands for.
    type EntryEdit = Box<dyn Fn(&mut crate::state::toolpath::ToolpathEntry)>;
    let rows: Vec<(&str, EntryEdit)> = vec![
        (
            "heights",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.heights.clearance_z = rs_cam_core::compute::config::HeightMode::Manual(55.0);
            }),
        ),
        (
            "boundary",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.boundary.enabled = !e.boundary.enabled;
            }),
        ),
        (
            "dressup",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.dressups.arc_fitting = !e.dressups.arc_fitting;
            }),
        ),
        (
            "stock source",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.stock_source = rs_cam_core::compute::config::StockSource::FromRemainingStock;
            }),
        ),
        (
            "rest analysis",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.rest_analysis.enabled = !e.rest_analysis.enabled;
            }),
        ),
        (
            "face selection clear",
            Box::new(|e: &mut crate::state::toolpath::ToolpathEntry| {
                e.face_selection = Some(Vec::new());
            }),
        ),
    ];

    for (label, edit) in rows {
        let mut controller = sample_controller();
        generate_all_for_test(&mut controller);
        let id = controller.state.session.toolpath_configs()[0].id;
        panel_edit(&mut controller, id, edit);
        assert_eq!(
            state_of(&controller, 0),
            FreshnessState::EditedSince,
            "{label}: an edited input must not read Current"
        );
        assert!(
            controller.state.session.get_result(0).is_none(),
            "{label}: the core result must be gone"
        );
        assert!(controller.state.gui.dirty, "{label}: the project is dirty");
    }
}

/// Rebinding the tool or the model changes what is cut, and both used to
/// leave the core result, the regeneration request and the dirty flag
/// untouched (R0.1 §2.3, "Tool / model reassignment").
#[test]
fn freshness_state_g_freshness_tool_and_model_reassignment_stale() {
    for row in ["tool", "model"] {
        let mut controller = sample_controller();
        let mut tools = controller.state.session.tools().to_vec();
        tools.push(ToolConfig::new_default(ToolId(2), ToolType::EndMill));
        let _ = controller
            .state
            .session
            .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
            .expect("the session takes the tool list");
        generate_all_for_test(&mut controller);
        let id = controller.state.session.toolpath_configs()[0].id;
        panel_edit(&mut controller, id, |entry| {
            if row == "tool" {
                entry.tool_id = crate::state::job::ToolId(2);
            } else {
                entry.model_id = crate::state::job::ModelId(7);
            }
        });
        assert_eq!(
            state_of(&controller, 0),
            FreshnessState::EditedSince,
            "{row} reassignment"
        );
        assert!(controller.state.session.get_result(0).is_none());
    }
}

/// The other half of the contract: an edit that changes no motion leaves
/// the toolpath current. Without this the model would be a rename away
/// from staling the whole project.
#[test]
fn name_and_gcode_edits_do_not_stale() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    panel_edit(&mut controller, id, |entry| {
        entry.name = "Renamed".to_owned();
        entry.pre_gcode = "M8".to_owned();
        entry.post_gcode = "M9".to_owned();
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert!(controller.state.session.get_result(0).is_some());
    assert_eq!(
        controller.state.session.toolpath_configs()[0].name,
        "Renamed",
        "the edit still reached the session"
    );
}

/// A never-generated toolpath is `NoResult`, not `EditedSince`: the two
/// are the same absent core entry, and only the GUI's retained result
/// separates them.
#[test]
fn freshness_never_generated_is_no_result() {
    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    controller.state.gui.toolpath_rt.remove(&second);
    assert_eq!(state_of(&controller, 1), FreshnessState::NoResult);
}

/// `enabled: false` wins over everything, exactly as `ComputeStatus`
/// already ruled for the status chip.
#[test]
fn freshness_disabled_wins() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(crate::ui::AppEvent::ToggleToolpathEnabled(id));
    assert_eq!(state_of(&controller, 0), FreshnessState::Disabled);
    assert!(
        controller.state.gui.dirty,
        "toggling an operation off changes what the job cuts"
    );
}

/// R0.1 §7 Q4, operator-confirmed: swapping two `Fresh` operations does
/// not change either one's geometry, so both stay `Current`; only a
/// downstream `FromRemainingStock` op goes `EditedSince`.
#[test]
fn freshness_reorder_keeps_fresh_ops_current() {
    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    let third = push_toolpath(&mut controller, "Rest");
    if let Some((idx, _)) = controller.state.session.find_toolpath_config_by_id(third) {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index: idx,
                source: rs_cam_core::compute::config::StockSource::FromRemainingStock,
            }))
            .expect("index is in range");
    }
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(crate::ui::AppEvent::ReorderToolpath(second, 0));

    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert_eq!(state_of(&controller, 1), FreshnessState::Current);
    assert_eq!(state_of(&controller, 2), FreshnessState::EditedSince);
}

/// R0.1 §7 Q1, operator ruling 2026-09-10: ANY stock edit — dimensions,
/// pins, or the material alone — stales every toolpath, on both routes.
/// The GUI route used to stale none of them.
///
/// WP6 moved the GUI route onto the command door. The panel edits a
/// scratch copy and calls `ui::properties::apply_stock_draft`, which
/// applies `Command::SetStockConfig` and stamps every index
/// `Effects::stale` names. This drives that function, not the deleted
/// `AppEvent::StockChanged`.
#[test]
fn freshness_stock_edit_stales_every_toolpath() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);

    let mut draft = controller.state.session.stock_config().clone();
    // The fixture's stock carries `auto_from_model`, and the apply funnel
    // re-sizes such a draft around the first model BEFORE it compares. A
    // typed dimension alone would therefore arrive back at the stored
    // record and the funnel would apply nothing. The operator clears the
    // checkbox to type a dimension, so the draft does the same.
    draft.auto_from_model = false;
    draft.x = 321.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft);

    for index in 0..2 {
        assert_eq!(
            state_of(&controller, index),
            FreshnessState::EditedSince,
            "toolpath {index} after a stock edit"
        );
        assert!(controller.state.session.get_result(index).is_none());
    }
    assert!(controller.state.gui.dirty);
    assert!(
        controller.state.panel_side_effects.upload,
        "the stock box the viewport draws moved, so the panel owes an upload"
    );
    assert!(
        controller.state.panel_side_effects.pin_drill_sync,
        "the deleted StockChanged handler always ran the pin-drill sync"
    );
}

/// R0.1 §7 Q3, operator ruling: a machine kinematics edit stales the
/// SIMULATION only. Timing and feed modulation move; geometry does not.
#[test]
fn freshness_machine_kinematics_leaves_toolpaths_current() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let kinematics = controller.state.session.machine().effective_kinematics();
    let _ = controller
        .state
        .session
        .apply(Command::SetMachineKinematics(
            rs_cam_core::session::SetMachineKinematicsArgs {
                kinematics: Box::new(kinematics),
            },
        ));
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    assert!(controller.state.session.get_result(0).is_some());
    assert!(controller.state.session.simulation_result().is_none());
}

/// A setup orientation flip regenerates every toolpath in that setup in a
/// new frame. The panel wrote `face_up` / `z_rotation` straight into
/// `SetupData`, so no core setter ran and every result survived.
#[test]
fn freshness_setup_orientation_stales_the_setup() {
    for row in ["face", "rotation"] {
        let mut controller = sample_controller();
        push_toolpath(&mut controller, "Second");
        generate_all_for_test(&mut controller);

        if row == "face" {
            let _ = controller
                .state
                .session
                .apply(Command::SetSetupFace(SetSetupFaceArgs {
                    setup_index: 0,
                    face_up: rs_cam_core::compute::transform::FaceUp::Bottom,
                }))
                .expect("setup 0 exists");
        } else {
            let _ = controller
                .state
                .session
                .apply(Command::SetSetupRotation(SetSetupRotationArgs {
                    setup_index: 0,
                    z_rotation: rs_cam_core::compute::transform::ZRotation::Deg90,
                }))
                .expect("setup 0 exists");
        }

        for index in 0..2 {
            assert_eq!(
                state_of(&controller, index),
                FreshnessState::EditedSince,
                "{row}: toolpath {index}"
            );
        }
    }
}

/// A tool edit drops the results of every op that tool machines. It used
/// to drop them in the core and mark nothing in the GUI, which is the one
/// case the export fallback to the GUI's own copy was written for.
#[test]
fn freshness_tool_edit_stales_its_users_and_requests_regeneration() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    let mut draft = controller.state.session.tools()[0].clone();
    draft.diameter += 1.0;
    crate::ui::properties::commit_tool_draft(&mut controller.state, ToolId(1), draft);

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert!(
        controller.state.gui.toolpath_rt[&id].stale_since.is_some(),
        "the operator gets a regeneration request, not a silently stale card"
    );
    assert!(controller.state.gui.dirty);
}

/// The revision counter R0.1 §4.2 asks for: bumped at the one drop site,
/// never by recording an answer. F2.4's late-result guard reads it.
#[test]
fn toolpath_revision_bumps_on_every_input_drop() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let before = controller.state.session.toolpath_revision(0);

    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision: before,
            result: Box::new(core_result()),
        }))
        .expect("index is in range");
    assert_eq!(
        controller.state.session.toolpath_revision(0),
        before,
        "recording an answer is not a change of inputs"
    );

    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(999.0);
    });
    assert!(
        controller.state.session.toolpath_revision(0) > before,
        "an input edit must move the revision"
    );
}

// The F2.1 pre-fix reproduction stood here. It wrote
// `tc.operation.set_feed_rate` through `find_toolpath_config_by_id_mut`
// and asserted the core result SURVIVED, which is what made the card
// read OK over geometry from other inputs.
//
// WP7 deleted it. The nine mutation hatches are `pub(crate)`, so an
// un-commanded write from viz is a compile error and no test in this
// crate can reproduce one. The guarantee moved from a reproduction to
// the type system, and `crates/rs_cam_core/tests/
// hatches_are_crate_private_wp7.rs` is the sentry that holds it. The
// core half of the same reproduction survives in-crate, in
// `session/mutation.rs`, where the hatch is still in reach.

// ── F2.2 / G-FRESHRENDER — the surfaces draw the state F2.1 derived ──────
//
// F2.1 built one `FreshnessState` and proved every mutation lands on the
// right one. Nothing read it: the card chip, the inspector header, the two
// workspace chips and Readiness each still asked their own question, and on
// an edited operation every one of them answered "fine". The card asked
// `ComputeStatus`, which is `Done` — the generation really did finish.
// Readiness and the Readiness chip asked `gui.toolpath_rt[..].result`, the
// GUI's retained copy, which an edit deliberately KEEPS so the viewport can
// still draw something. So a project where no operation could be reproduced
// read `OK`, `2/2 computed` and no chip at all.
//
// These tests drive the real surface functions, not a source read: the chip
// vocabulary, the two badges, `operations_check` and the shared counter are
// all pure over `&AppState`. The three surfaces that cannot be driven from a
// test — the egui header, the card body, the wgpu draw — are asserted in
// `tests/freshness_surfaces_g_freshrender.rs` by reading their source, which
// says so in its own doc.
//
// NOT asserted anywhere, and deliberately: that an operation reading STALE
// cannot be exported. It still can. F2.3 owns the export gate; until it
// lands, `emitted_toolpaths` falls back to the GUI's retained result and
// will emit the old geometry. Nothing in this task's UI text says otherwise.

/// The chip vocabulary, one state at a time. `EditedSince` is the row that
/// did not exist before: it used to fall through to `Done` → `OK` green.
#[test]
fn freshness_chip_says_stale_and_never_says_ok() {
    use crate::ui::toolpath_panel::status_chip;

    let cases = [
        (FreshnessState::Current, "OK"),
        (FreshnessState::EditedSince, "STALE"),
        (FreshnessState::Regenerating, "GEN"),
        (FreshnessState::NoResult, "PEND"),
        (FreshnessState::Disabled, "OFF"),
        (FreshnessState::Error("boom".to_owned()), "ERR"),
    ];
    for (state, expected) in &cases {
        let (text, _, _) = status_chip(state);
        assert_eq!(&text, expected, "{state:?}");
    }

    // The one that matters, in detail. Amber and not the success colour;
    // red stays reserved for ERR and collisions.
    let (text, role, hover) = status_chip(&FreshnessState::EditedSince);
    assert_eq!(text, "STALE");
    // UP4: the mapping returns a ROLE rather than a raw colour, which makes
    // this assertion stronger. Under UP1's palette several theme names
    // collapsed onto one value, so comparing colours could have passed while
    // the meaning drifted; a role cannot.
    assert_eq!(role, crate::ui::components::Role::Caution);
    assert_ne!(role, crate::ui::components::Role::Ok);
    assert_ne!(role, crate::ui::components::Role::Danger);
    let hover = hover.expect("four letters cannot carry this on their own");
    assert!(
        hover.contains("PREVIOUS generation"),
        "the hover must say whose numbers these are: {hover}"
    );
    assert!(
        hover.contains("Regenerate"),
        "and what to do about it: {hover}"
    );
    // The caution the programme is under until F2.3: this text must not
    // imply the export is gated on it, because it is not.
    assert!(
        !hover.to_lowercase().contains("export"),
        "F2.3 owns the export gate; this hover must not promise it: {hover}"
    );
}

/// THE headline row. One panel edit on a generated project and every
/// surface that can be driven from a test moves together.
#[test]
fn freshness_surfaces_agree_after_one_edit_g_freshrender() {
    use crate::ui::readiness;
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    let total = controller.state.session.toolpath_configs().len();

    // Before: everything current, nothing to report.
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    let (status, current, enabled) = readiness::operations_check(&controller.state);
    assert_eq!((current, enabled), (total, total));
    assert_eq!(status, readiness::CheckStatus::Pass);
    assert_eq!(readiness::freshness_counts(&controller.state), (0, 0));
    assert!(workspace_bar::toolpath_badge(&controller.state).is_none());

    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });

    // After: one stale operation, said the same way everywhere.
    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert_eq!(
        readiness::freshness_counts(&controller.state),
        (1, 0),
        "one stale, none merely pending"
    );

    let (status, current, enabled) = readiness::operations_check(&controller.state);
    assert_eq!(
        current,
        total - 1,
        "F1.17: Readiness counted the GUI's retained result and read {total}/{total} here"
    );
    assert_eq!(enabled, total);
    assert_eq!(status, readiness::CheckStatus::Warning);

    let (chip, role) =
        workspace_bar::toolpath_badge(&controller.state).expect("the Toolpaths tab must say so");
    assert_eq!(chip, "1 stale");
    assert_eq!(role, crate::ui::components::Role::Caution);

    let (chip, _) = workspace_bar::readiness_badge(&controller.state)
        .expect("the Readiness tab must say so too");
    assert_eq!(chip, "1 stale");

    // And the card's own chip.
    let (text, _, _) = crate::ui::toolpath_panel::status_chip(&state_of(&controller, 0));
    assert_eq!(text, "STALE");
}

/// Stale outranks pending on both chips, and the two counts do not merge.
/// A project with one of each must report the stale one: it is the one
/// currently showing a wrong answer rather than no answer.
#[test]
fn freshness_chip_reports_stale_ahead_of_pending() {
    use crate::ui::readiness;
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    let second = push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    let first = controller.state.session.toolpath_configs()[0].id;

    // Toolpath 1 never generated, toolpath 0 generated then edited.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 1 },
        ))
        .expect("toolpath 1 exists");
    if let Some(rt) = controller.state.gui.toolpath_rt.get_mut(&second) {
        rt.result = None;
    }
    panel_edit(&mut controller, first, |entry| {
        entry.operation.set_feed_rate(999.0);
    });

    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);
    assert_eq!(state_of(&controller, 1), FreshnessState::NoResult);
    assert_eq!(readiness::freshness_counts(&controller.state), (1, 1));

    let (chip, _) = workspace_bar::toolpath_badge(&controller.state).expect("something to report");
    assert_eq!(
        chip, "1 stale",
        "a stale operation outranks a pending one; folding them into one \
         count is what let the chip read zero on a fully edited project"
    );
}

/// A collision still outranks staleness on the Readiness chip. SHE-003's
/// order is not weakened by adding a state above "uncomputed".
#[test]
fn freshness_does_not_outrank_a_collision() {
    use crate::ui::workspace_bar;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    controller.state.simulation.checks.rapid_collisions =
        vec![rs_cam_core::stock::collision::RapidCollision {
            move_index: 0,
            start: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            end: rs_cam_core::geo::P3::new(1.0, 0.0, 0.0),
        }];

    let (chip, role) =
        workspace_bar::readiness_badge(&controller.state).expect("a collision must be reported");
    // UR2 (e7901838): one shared safety text replaces the two per-tab
    // spellings. The count, not the word "collision", is the claim.
    assert_eq!(chip, "1 safety");
    // UP4: the badge producers hand back a ROLE now, not a colour, so the
    // `role_for` shim in the bar is gone and a safety badge cannot drift
    // onto a non-safety hue.
    assert_eq!(role, crate::ui::components::Role::Danger);
}

/// Disabled operations are not outstanding work, and an errored one is not
/// "still to do" — A/M11's rule that a block is never a failure, mirrored.
#[test]
fn freshness_counts_exclude_disabled_and_error() {
    use crate::ui::readiness;

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let id = controller.state.session.toolpath_configs()[0].id;

    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("index 0 exists");
    assert_eq!(state_of(&controller, 0), FreshnessState::Disabled);
    assert_eq!(readiness::freshness_counts(&controller.state).0, 0);

    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: true,
        }))
        .expect("index 0 exists");
    if let Some(rt) = controller.state.gui.toolpath_rt.get_mut(&id) {
        rt.status = crate::state::runtime::ComputeStatus::Error("nope".to_owned());
    }
    let (stale, pending) = readiness::freshness_counts(&controller.state);
    assert_eq!(
        (stale, pending),
        (0, 0),
        "an error is neither stale nor pending; it needs a fix, not a wait"
    );
}

// ── F2.5 / G-UNDOFRESH — an undo leaves the project where the same edit
//    made by hand would leave it ─────────────────────────────────────────
//
// The PLAN row names three symptoms — `ToolChange` undo does not call
// `invalidate_tool`, no arm calls `mark_edited`, `ToolpathParamChange` undo
// does not re-stale the simulation — and it was written before F2.1 existed.
// The rule underneath all three is one sentence: **after any undo or redo,
// the derived `FreshnessState` of every affected toolpath, the dirty flag and
// the simulation's staleness are what they would be if the operator had made
// that same edit by hand.** Each symptom is that rule broken in one place.
//
// On whether an undone edit restores `Current`: it does not, deliberately.
// The argument is in `reports/F2.5.md` §3 and in `apply_toolpath_snapshot`'s
// doc; `an_undone_param_edit_does_not_resurrect_the_old_result` is where it
// is pinned, so the decision cannot be reversed by accident.

/// A project with one generated toolpath, one simulation result recorded as
/// fresh, and a clean dirty flag — the state an operator is in when they
/// reach for Ctrl+Z.
fn controller_ready_for_undo() -> (AppController<ScriptedBackend>, ToolpathId) {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    controller.state.simulation.last_run = Some(crate::state::simulation::SimulationRunMeta {
        sim_generation: 1,
        last_sim_edit_counter: controller.state.gui.edit_counter,
        // A hand-built fresh run answers the capture revision now set.
        accepted_metric_options_revision: Some(controller.state.simulation.metric_options_revision),
    });
    controller.state.gui.dirty = false;
    (controller, tp_id)
}

/// THE rule, over every arm. A per-arm table lives in the report; this is
/// the executable half of it.
///
/// Pre-fix NONE of the five arms marked the project edited, so `dirty` stayed
/// false — an undone project was not offered for saving — and `edit_counter`
/// never moved, so the GUI went on presenting a simulation computed from the
/// configuration the undo had just discarded, with no staleness anywhere.
#[test]
fn every_undo_arm_marks_the_project_edited_g_undofresh() {
    use crate::state::history::UndoAction;

    /// One undo arm: a label and a builder for the action the operator's
    /// edit would have pushed.
    type ArmBuilder = Box<dyn Fn(&AppController<ScriptedBackend>) -> UndoAction>;

    let arms: Vec<(&str, ArmBuilder)> = vec![
        (
            "StockChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::StockChange {
                    old: c.state.session.stock_config().clone(),
                    new: c.state.session.stock_config().clone(),
                },
            ),
        ),
        (
            "PostChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::PostChange {
                    old: c.state.gui.post.clone(),
                    new: c.state.gui.post.clone(),
                },
            ),
        ),
        (
            "ToolChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::ToolChange {
                    tool_id: c.state.session.tools()[0].id,
                    old: c.state.session.tools()[0].clone(),
                    new: c.state.session.tools()[0].clone(),
                },
            ),
        ),
        (
            "ToolpathParamChange",
            Box::new(|c: &AppController<ScriptedBackend>| {
                let tc = &c.state.session.toolpath_configs()[0];
                UndoAction::ToolpathParamChange {
                    tp_id: tc.id,
                    old_op: tc.operation.clone(),
                    new_op: tc.operation.clone(),
                    old_dressups: tc.dressups.clone(),
                    new_dressups: tc.dressups.clone(),
                    old_face_selection: None,
                    new_face_selection: None,
                    old_feeds_provenance: tc.feeds_provenance.clone(),
                    new_feeds_provenance: tc.feeds_provenance.clone(),
                }
            }),
        ),
        (
            "MachineChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::MachineChange {
                    old: c.state.session.machine().clone(),
                    new: c.state.session.machine().clone(),
                },
            ),
        ),
    ];

    for (name, build) in arms {
        // Undo.
        let (mut controller, _) = controller_ready_for_undo();
        let action = build(&controller);
        controller.state.history.push(action.clone());
        controller.state.gui.dirty = false;
        let before = controller.state.gui.edit_counter;

        controller.handle_internal_event(AppEvent::Undo);

        assert!(
            controller.state.gui.dirty,
            "{name}: undo must leave the project unsaved, or the close \
             interception lets the operator walk away from it"
        );
        assert!(
            controller.state.gui.edit_counter > before,
            "{name}: undo must move the edit counter, or the simulation goes \
             on calling itself fresh"
        );
        // One property, whatever the treatment. Every arm leaves the
        // project where the same edit made by hand would. WP19 (plan §28)
        // made that one rule: a session that drops its simulation leaves
        // no viewport showing one, for a parameter edit as for a stock or
        // machine change. Either way the operator must not be shown the
        // old run as current evidence, and that is what is asserted:
        // cleared or stale, never present-and-fresh.
        let sim = &controller.state.simulation;
        assert!(
            !sim.has_results() || sim.is_stale(controller.state.gui.edit_counter),
            "{name}: the simulation was computed from the configuration this \
             undo just discarded, and is still being presented as fresh"
        );

        // Redo — the row does not name it; it carries the same defect and
        // takes the same fix, so it is asserted the same way.
        let (mut controller, _) = controller_ready_for_undo();
        controller.state.history.push(action);
        controller.handle_internal_event(AppEvent::Undo);
        controller.state.gui.dirty = false;
        let before = controller.state.gui.edit_counter;

        controller.handle_internal_event(AppEvent::Redo);

        assert!(
            controller.state.gui.dirty,
            "{name}: redo is the same edit in the other direction"
        );
        assert!(
            controller.state.gui.edit_counter > before,
            "{name}: redo must move the edit counter too"
        );
    }
}

/// The row's first clause. A tool undo must invalidate the operations that
/// tool machines, exactly as `commit_tool_draft` does — the undo arms used to
/// write `tools_mut()` directly and clear only the simulation, so every
/// dependent kept a CORE result generated with the other tool's geometry and
/// read `Current` on every surface.
#[test]
fn a_tool_undo_stales_the_operations_that_tool_machines_g_undofresh() {
    use crate::state::history::UndoAction;

    let (mut controller, tp_id) = controller_ready_for_undo();
    let tool_id = controller.state.session.tools()[0].id;
    let original = controller.state.session.tools()[0].clone();
    let mut widened = original.clone();
    widened.diameter += 3.0;

    // The hand edit, through the door the panel uses.
    crate::ui::properties::commit_tool_draft(&mut controller.state, tool_id, widened);
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "the hand edit stales its users (F2.1) — that is the behaviour undo has to match"
    );

    // Regenerate so the undo has something to invalidate.
    generate_all_for_test(&mut controller);
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    controller.state.gui.dirty = false;

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.tools()[0].diameter,
        original.diameter,
        "the undo restores the tool"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and every operation it machines is no longer current — the stored \
         result was generated with the OTHER tool's geometry"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "which means the core result is gone, not merely flagged"
    );
    assert!(
        controller.state.gui.toolpath_rt[&tp_id]
            .stale_since
            .is_some(),
        "and a regeneration is requested, as commit_tool_draft does — the \
         operator did not ask these operations to be invalidated"
    );
    // Sanity: the undo really did go through the shared door rather than a
    // second one that happens to agree today.
    assert!(
        matches!(
            controller.state.history.undo(),
            None | Some(UndoAction::ToolChange { .. })
        ),
        "the history holds what we think it holds"
    );
}

/// The decision of §3, pinned: an undo that restores the exact configuration
/// the retained geometry was generated from still reads `EditedSince`.
///
/// This is the one assertion in F2.5 that could reasonably have gone the
/// other way, so it is asserted explicitly rather than left as a consequence.
/// Reversing it must be a decision someone makes, not a side effect.
#[test]
fn an_undone_param_edit_does_not_resurrect_the_old_result() {
    let (mut controller, tp_id) = controller_ready_for_undo();
    let before = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();

    // A parameter edit with an undo entry behind it, as the panel pushes one.
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::ToolpathParamChange {
            tp_id,
            old_op: before.clone(),
            new_op: {
                let mut op = before.clone();
                op.set_feed_rate(4321.0);
                op
            },
            old_dressups: Default::default(),
            new_dressups: Default::default(),
            old_face_selection: None,
            new_face_selection: None,
            old_feeds_provenance: Default::default(),
            new_feeds_provenance: Default::default(),
        });
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0]
            .operation
            .feed_rate(),
        before.feed_rate(),
        "the undo restores the configuration the retained geometry answers"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and it STILL reads EditedSince. Nothing records which configuration \
         `rt.result` answers; the snapshot restores three of the nine fields \
         that decide the geometry; and `toolpath_revision` only moves forward. \
         Being wrong this way costs a regeneration — the other way exports a \
         program that does not match the project. See reports/F2.5.md §3."
    );
}

/// WP8 / N14 — an undo stales the whole chain, and it restores the feeds
/// provenance the edit stamped.
///
/// Two defects in one test, both of the same shape: the undo door knew
/// about ONE toolpath and ONE set of fields.
///
/// - The core call dropped the edited row's result alone, and the GUI
///   stamped `stale_since` on that row alone. A downstream
///   `FromRemainingStock` operation kept a result generated against
///   stock that had moved, read `Current` on every surface, and the
///   auto-regen sweep never queued it.
/// - The undo entry carried no `feeds_provenance`. Three optimizer apply
///   paths wrote the provenance in a second call right after the
///   snapshot, so an undo put the pre-optimizer numbers back and left
///   the `Optimizer` stamp standing on them.
#[test]
fn an_undo_stales_the_downstream_row_and_restores_the_provenance_n14() {
    let (mut controller, tp_id) = controller_ready_for_undo();
    let rest_id = push_toolpath(&mut controller, "Rest");
    if let Some((idx, _)) = controller.state.session.find_toolpath_config_by_id(rest_id) {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index: idx,
                source: rs_cam_core::compute::config::StockSource::FromRemainingStock,
            }))
            .expect("index is in range");
    }

    let before_op = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let before_provenance = controller.state.session.toolpath_configs()[0]
        .feeds_provenance
        .clone();
    let after_provenance = rs_cam_core::feeds::FeedsProvenance {
        feed_rate: Some(rs_cam_core::feeds::ValueProvenance::optimizer()),
        ..Default::default()
    };
    let mut after_op = before_op.clone();
    after_op.set_feed_rate(4321.0);
    assert_ne!(
        before_provenance, after_provenance,
        "the two stamps must differ, or the restore assertion is vacuous"
    );

    // The edit, with the entry the panel pushes for it.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    let _ = controller
        .state
        .session
        .apply(Command::SetFeedsProvenance(SetFeedsProvenanceArgs {
            index: 0,
            feeds_provenance: Box::new(after_provenance.clone()),
        }))
        .expect("index 0 exists");
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::ToolpathParamChange {
            tp_id,
            old_op: before_op.clone(),
            new_op: after_op,
            old_dressups: Default::default(),
            new_dressups: Default::default(),
            old_face_selection: None,
            new_face_selection: None,
            old_feeds_provenance: before_provenance.clone(),
            new_feeds_provenance: after_provenance.clone(),
        });

    // Regenerate, so the undo has something to invalidate on BOTH rows.
    generate_all_for_test(&mut controller);
    assert!(
        controller.state.session.get_result(1).is_some()
            && controller.state.gui.toolpath_rt[&rest_id]
                .stale_since
                .is_none(),
        "the downstream row starts fresh, or the assertions below are \
         vacuous"
    );

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0]
            .operation
            .feed_rate(),
        before_op.feed_rate(),
        "the undo restores the operation"
    );
    assert_eq!(
        controller.state.session.toolpath_configs()[0].feeds_provenance,
        before_provenance,
        "and the provenance the edit stamped"
    );
    assert!(
        controller.state.session.get_result(1).is_none(),
        "the downstream row reads the stock index 0 leaves, so its \
         result goes with the undo"
    );
    assert!(
        controller.state.gui.toolpath_rt[&rest_id]
            .stale_since
            .is_some(),
        "and the GUI stamps every index in Effects::stale, not the \
         edited one alone. The auto-regen sweep reads stale_since."
    );

    controller.handle_internal_event(AppEvent::Redo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0].feeds_provenance,
        after_provenance,
        "redo is the same edit in the other direction, so it carries the \
         provenance too"
    );
}

/// A machine-kinematics undo leaves the toolpaths current. F2.1's operator
/// ruling (R0.1 §7 Q3) is that kinematics change timing and modulation, not
/// geometry — so this arm must NOT acquire staleness it does not deserve
/// just because the other arms did.
#[test]
fn a_machine_undo_leaves_the_toolpaths_current_g_undofresh() {
    let (mut controller, _) = controller_ready_for_undo();
    let machine = controller.state.session.machine().clone();
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::MachineChange {
            old: machine.clone(),
            new: machine,
        });

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "kinematics do not move geometry; only the simulation goes"
    );
    assert!(
        controller.state.gui.dirty,
        "but the project is still edited"
    );
}

// ── F2.10 / G-LATESIM — the simulation arm of the late-arrival race ──────
//
// F2.4 closed the toolpath arm: a generation result that answers a
// superseded configuration is stored but not made current. This is the same
// defect on the simulation, and it matters at least as much — an operator
// makes machining decisions off collision counts, engagement and air-cut, so
// a run that answers a discarded configuration, presented as current
// evidence, is the export defect on the surface people trust to tell them it
// is safe to cut.
//
// `SimulationRunMeta::last_sim_edit_counter` was stamped in the DRAIN from
// the live `edit_counter`, which folded every edit made while the simulation
// ran into the record of when it was run. `is_stale` then said `false`.
//
// The counter is project-wide and that is deliberate — see `reports/F2.10.md`
// §3. In short: this is already `is_stale`'s semantics after a run, so
// stamping at submit adds no new coarseness; it makes the in-flight window
// behave like the window either side of it.

/// THE race. An edit lands while the simulation runs; the result that
/// arrives answers the configuration the operator has already left.
///
/// Pre-fix the drain stamped the live counter, so `is_stale` read `false` and
/// every surface — the Simulation tab chip, `readiness::simulation_check`,
/// the Readiness panel's `FreshnessGate` — presented the run as current.
#[test]
fn an_edit_during_a_simulation_leaves_the_result_stale_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;

    // Submit the run. The stamp is taken here, not when the result lands.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let submitted_at = controller.state.simulation.submitted_edit_counter;
    assert!(
        submitted_at.is_some(),
        "the submit must record the configuration this run answers"
    );

    // The operator changes a parameter while it runs.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    let after_edit = controller.state.gui.edit_counter;
    assert!(
        Some(after_edit) != submitted_at,
        "the edit must move the counter, or this test proves nothing"
    );

    // The result lands.
    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.has_results(),
        "STORED, not discarded: a simulation is minutes of work, and throwing \
         it away leaves the operator unable to tell a cancelled run from one \
         that never happened"
    );
    assert!(
        controller.state.simulation.is_stale(after_edit),
        "but NOT current — this run measured the configuration the operator \
         has already left, and it is the surface they read collision counts \
         off"
    );
    assert_eq!(
        controller
            .state
            .simulation
            .last_run
            .as_ref()
            .map(|m| m.last_sim_edit_counter),
        submitted_at,
        "the run is recorded against the counter it was SUBMITTED at, not the \
         one that happened to be live when it landed"
    );
}

/// The other side: with no edit in flight, the result is current. A guard
/// that staled every simulation would pass the test above and make the
/// Simulation workspace useless.
#[test]
fn a_simulation_with_no_edit_in_flight_is_current_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());
    assert!(
        !controller
            .state
            .simulation
            .is_stale(controller.state.gui.edit_counter),
        "an unedited run is current evidence"
    );
}

/// The stamp is consumed by the run it belongs to. A second result arriving
/// with no submit behind it falls back to the live counter — the pre-F2.10
/// behaviour, which is not a claim this guard can make either way.
#[test]
fn the_submit_stamp_belongs_to_one_run_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(AppEvent::RunSimulation);
    assert!(controller.state.simulation.submitted_edit_counter.is_some());
    inject_sim_results(&mut controller, 1);
    assert!(
        controller.state.simulation.submitted_edit_counter.is_none(),
        "the stamp is taken by the result it describes, so it cannot be \
         re-used by a later one that had no submit of its own"
    );
}

// ── F4.3 / G-MODELRELINK — the missing-model repair route ────────────────
//
// A project whose model file has moved had NO repair route in the UI. The
// only actions were "Reload from disk" — the same path that just failed —
// and "Delete", which is refused while any toolpath references the model,
// i.e. exactly when it matters. `load_error`, which holds the loader's own
// reason, was rendered nowhere in the GUI. And a project saved its model
// paths verbatim, so a folder copied to another machine could not resolve
// them even when the model travelled with it.

/// Write a project directory with a `models/` subfolder and a job that
/// points at a file inside it. Returns the temp directory.
fn relink_fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = temp_path(name, "dir");
    std::fs::create_dir_all(dir.join("models")).expect("create fixture dir");
    let toml = r#"format_version = 3

[job]
name = "Relink Fixture"

[[tools]]
id = 1
name = "Fixture End Mill"
type = "end_mill"

[[models]]
id = 1
path = "models/missing.stl"
name = "Plate"
kind = "stl"

[[setups]]
name = "Setup 1"

[[setups.toolpaths]]
id = 1
name = "Rough"
type = "adaptive3d"
enabled = true
tool_id = 1
model_id = 1
"#;
    std::fs::write(dir.join("job.toml"), toml).expect("write fixture job");
    dir
}

/// THE repair route, end to end: a project opens with its model missing, the
/// operator locates the file, and the operations built on it survive — but
/// are no longer current, because the geometry changed under them.
#[test]
fn missing_model_relink_g_modelrelink() {
    let dir = relink_fixture_dir("relink");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();

    controller
        .open_job_from_path(&job)
        .expect("a project with a missing model still opens");

    // The path is RESOLVED, not the raw string. Pre-fix the session stored
    // "models/missing.stl", which resolves against the process working
    // directory — so Reload looked somewhere nobody had searched and the
    // warning named a directory that did not exist.
    assert_eq!(
        controller.state.session.models()[0].path,
        dir.join("models/missing.stl"),
        "an in-memory model path is absolute, whatever the file said"
    );
    assert!(controller.state.session.models()[0].mesh.is_none());
    assert!(
        controller.state.session.models()[0].load_error.is_some(),
        "the loader's reason is kept, and is now rendered"
    );
    assert_eq!(controller.load_warnings().len(), 1);

    // Make the toolpath look generated, so the invalidation assertion below
    // is not comparing two empty caches.
    let revision = controller.state.session.toolpath_revision(0);
    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result()),
        }))
        .expect("index 0 exists");
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let rt = controller.state.gui.toolpath_rt_or_default(tp_id);
    rt.status = crate::state::toolpath::ComputeStatus::Done;
    rt.stale_since = None;
    // The GUI's drawable copy too, or the assertion below cannot tell
    // `EditedSince` (had geometry, now out of date) from `NoResult` (never
    // generated) — which is the distinction the whole card vocabulary rests
    // on.
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            Toolpath::new(),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);

    // The operator locates the file.
    let located = dir.join("models/plate.stl");
    std::fs::copy(fixture_path("flat_plate.stl"), &located).expect("stage the located file");
    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        located.clone(),
    ));

    let model = &controller.state.session.models()[0];
    assert!(model.mesh.is_some(), "the geometry loaded");
    assert!(model.load_error.is_none(), "and the complaint is gone");
    assert_eq!(model.path, located, "the model now points at the new file");
    assert_eq!(model.id, 1, "IDENTITY IS KEPT — this is the whole point");
    assert_eq!(
        model.name, "Plate",
        "and so is the name the operator gave it"
    );

    // The dependents. The model id did NOT change, so the signature
    // comparison that catches a re-pointed Input combo sees nothing here —
    // `invalidate_model` keys on the id, which is why it is the right
    // instrument for a relink.
    assert!(
        controller.state.session.get_result(0).is_none(),
        "every result built on the old geometry is dropped"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and the operation says so on every surface"
    );
    assert!(
        controller.state.gui.toolpath_rt[&tp_id]
            .stale_since
            .is_some(),
        "with a regeneration requested"
    );
    assert!(controller.state.gui.dirty);
    assert!(
        controller.load_warnings().is_empty(),
        "the complaint goes when the thing it complained about is repaired"
    );

    // The saved path is RELATIVE, because the model is under the project
    // directory — which is what lets the folder be copied or moved.
    controller.save_job_to_path(&job).expect("save");
    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains("path = \"models/plate.stl\""),
        "a model under the project directory is stored relative to it:\n{written}"
    );

    // And it reopens.
    controller.open_job_from_path(&job).expect("reopen");
    assert!(controller.state.session.models()[0].mesh.is_some());
    assert!(controller.load_warnings().is_empty());
    assert_eq!(controller.state.session.models()[0].path, located);

    let _ = std::fs::remove_dir_all(&dir);
}

/// A model OUTSIDE the project directory has no shorter honest description
/// than its absolute path, and gets one.
#[test]
fn a_model_outside_the_project_directory_saves_absolute_g_modelrelink() {
    let dir = relink_fixture_dir("relink_abs");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();
    controller.open_job_from_path(&job).expect("open");

    let outside = fixture_path("flat_plate.stl");
    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        outside.clone(),
    ));
    assert!(controller.state.session.models()[0].mesh.is_some());

    controller.save_job_to_path(&job).expect("save");
    let written = std::fs::read_to_string(&job).expect("read back");
    assert!(
        written.contains(&format!("path = \"{}\"", outside.display())),
        "a model the project folder does not contain is stored absolute:\n{written}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A relink to a different KIND is refused. "This file moved" and "use a
/// different model" are different operations; the second is the Input combo,
/// and a mesh operation cannot run on polygons.
#[test]
fn a_relink_to_another_kind_is_refused_g_modelrelink() {
    let dir = relink_fixture_dir("relink_kind");
    let job = dir.join("job.toml");
    let mut controller = sample_controller();
    controller.open_job_from_path(&job).expect("open");
    let before = controller.state.session.models()[0].path.clone();

    controller.handle_internal_event(AppEvent::RelinkModel(
        crate::state::job::ModelId(1),
        fixture_path("square.svg"),
    ));

    let model = &controller.state.session.models()[0];
    assert_eq!(model.kind, Some(crate::state::job::ModelKind::Stl));
    assert_eq!(model.path, before, "the model is untouched");
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("Cannot relink")),
        "and the operator is told why, rather than the refusal being silent"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── WP19 — every `Effects` answer reaches the view ───────────────────
//
// A core mutation answers with an `Effects`. `stale` reaches the
// toolpath cards; `simulation_cleared` reaches the viewport, because the
// session and the viewport hold ONE simulation (WP11b, N12 item 10).
// The source-scan half of this sentry is
// `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`.

/// A rest-chain controller whose SESSION holds a simulation.
///
/// The control is load-bearing. `Effects::simulation_cleared` is
/// `simulation_before && self.simulation.is_none()`
/// (`session/command.rs`), so a fixture whose session holds no
/// simulation reports `false` on every row, and an arm built on it
/// proves nothing. The preamble is
/// `a_save_keeps_the_simulation_so_a_rest_op_still_starts_wp17`'s.
#[cfg(feature = "mcp")]
fn controller_holding_a_simulation() -> AppController<RestChainBackend> {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or the arm measures nothing"
    );
    controller.drain_compute_results();
    assert!(
        controller.state.session.simulation_result().is_some(),
        "the control: the session holds a simulation before the edit"
    );
    assert!(
        controller.state.simulation.results.is_some(),
        "the control: the viewport holds one too"
    );
    controller
}

/// b2 — a panel edit that drops the session's simulation owes the view.
///
/// `Command::SetStockConfig` reaches `drop_all_results`, which clears
/// the session's simulation. A draw site holds an `AppState` and
/// nothing else, so it cannot call `invalidate_simulation` itself: it
/// raises a `PanelSideEffects` flag and the frame loop discharges it,
/// the mechanism WP6 built.
///
/// Red before the fix: `state.simulation.results` is still `Some`. The
/// operator reads collision counts off a simulation of a stock that no
/// longer exists.
#[cfg(feature = "mcp")]
#[test]
fn a_panel_stock_edit_clears_the_view_simulation_wp19() {
    let mut controller = controller_holding_a_simulation();

    let mut draft = controller.state.session.stock_config().clone();
    // The funnel re-fits an `auto_from_model` draft around the model
    // before it compares, so a typed dimension alone would arrive back
    // at the stored record. The operator clears the checkbox to type a
    // dimension; the draft does the same
    // (`freshness_stock_edit_stales_every_toolpath`).
    draft.auto_from_model = false;
    draft.x = 321.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft);

    assert!(
        controller.state.session.simulation_result().is_none(),
        "the control: the stock row drops every result, and the \
         session's simulation with them"
    );
    controller.discharge_panel_side_effects();
    assert!(
        controller.state.simulation.results.is_none(),
        "the session dropped its simulation and the viewport still \
         draws one. The panel owes the frame loop that work, and the \
         frame loop runs it."
    );
}

/// c1 — a stamp on a never-drawn toolpath creates the catalog's row.
///
/// `stamp_stale` writes through `toolpath_rt_or_default`, which CREATES
/// a missing row. The row then joins `process_auto_regen`, which reads
/// `rt.auto_regen`. Twelve of the twenty-four operations declare
/// `default_auto_regen: false`, so a row hardcoded to `true` queues a
/// 3D finishing pass the operator never asked for.
///
/// Red before the fix: the `auto_regen` half. The fixture's operation
/// is `Scallop`, which the catalog opts OUT of auto-regeneration, and
/// the created row reads `true`. The `stale_since` half is green — that
/// is today's behaviour, and the arm pins it.
#[test]
fn a_stamped_row_reads_the_catalog_auto_regen_h3() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Second");
    let (id, expected) = {
        let tc = &controller.state.session.toolpath_configs()[1];
        (tc.id, tc.operation.default_auto_regen())
    };
    assert!(
        !expected,
        "the fixture must use an operation the catalog opts OUT of, or \
         this arm cannot tell a hardcoded `true` from a catalog read"
    );
    // The MCP route reaches a toolpath whose card the operator has
    // never drawn, so no runtime row exists. That is the case H3 names.
    controller.state.gui.toolpath_rt.remove(&id);

    let mut stale = std::collections::BTreeSet::new();
    stale.insert(1_usize);
    crate::state::stale::stamp_stale(&mut controller.state, &stale);

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&id)
        .expect("the stamp creates the row a never-drawn toolpath lacks");
    assert!(rt.stale_since.is_some(), "and it stamps what it created");
    assert_eq!(
        rt.auto_regen, expected,
        "a created row must read the operation's own `default_auto_regen`"
    );
}

/// b3 — the undo of a stock change stales every toolpath.
///
/// This arm was GREEN when it was written. WP15a routed the undo door
/// through `apply_quietly`, which stamps `Effects::stale`, and H2 named
/// this site before that landed. It stays as a regression pin: the row
/// drops EVERY result (G-FRESHSTATE), so every card must follow.
#[test]
fn an_undone_stock_change_stales_every_toolpath_wp19() {
    use crate::state::history::UndoAction;

    let (mut controller, _) = controller_ready_for_undo();
    push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    let action = UndoAction::StockChange {
        old: controller.state.session.stock_config().clone(),
        new: controller.state.session.stock_config().clone(),
    };
    controller.state.history.push(action);

    controller.handle_internal_event(AppEvent::Undo);

    for index in 0..2 {
        assert_eq!(
            state_of(&controller, index),
            FreshnessState::EditedSince,
            "toolpath {index} after an undone stock change"
        );
        assert!(controller.state.session.get_result(index).is_none());
    }
}

/// b1 — a toolpath edit clears the view's simulation (plan §28).
///
/// The operator ruled ONE rule for every row: a session that drops its
/// simulation leaves no viewport showing one. A toolpath-scoped edit
/// used to STALE the view's instead, under the F2.5 banner, while
/// `ProjectSession::start` was already refusing every
/// `FromRemainingStock` operation because the session held none. A
/// banner does not say that; an empty viewport does.
///
/// RED: this arm lands in the same commit as its fix, so the red is
/// read by running it against the commit BEFORE. There the first
/// assertion passes — `set_toolpath_enabled` writes
/// `session.simulation = None` — and the last two fail, because
/// `apply_quietly` cleared nothing in the view.
#[cfg(feature = "mcp")]
#[test]
fn a_toolpath_edit_clears_the_view_simulation_wp19() {
    let mut controller = controller_holding_a_simulation();
    let id = controller.state.session.toolpath_configs()[0].id;

    controller.handle_internal_event(AppEvent::ToggleToolpathEnabled(id));

    assert!(
        controller.state.session.simulation_result().is_none(),
        "the control: the toggle drops the session's simulation"
    );
    assert!(
        controller.state.simulation.results.is_none(),
        "and the viewport must not go on drawing the run of a job that \
         no longer exists"
    );
    assert!(controller.state.simulation.last_run.is_none());
}

// ── WP24 — the Optimize run is state, not a lockout ──────────────────
//
// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
// §30 ruling 3 (operator, 2026-09-13). The draw half of this finding is
// `crates/rs_cam_viz/tests/optimize_run_is_non_modal_wp24.rs`; these three
// arms are the state half, and they need `ScriptedBackend` plus the
// `pub(crate)` drain, so they live here.
//
// All three arms drive the run with the TIER-MAP PREVIEW. It is the same
// `OptimizeRun` the per-toolpath Optimize carries, and it needs no cut
// trace: no controller fixture in this crate holds one
// (`cut_trace: None` on every one of them), and
// `capture_optimize_toolpath` refuses without one. So the `Toolpath` kind
// of `OptimizeRun` is NOT exercised in-crate, and this heading says so
// rather than hiding it.

/// The event loop never blocked, and a run no longer blocks the drawing.
///
/// The placeholder replaced the central panel; it never stopped an event
/// reaching the controller. So the BEHAVIOURAL half of this arm already
/// held before WP24, and the arm is a regression guard: it bites a future
/// writer who puts the lockout back into the event loop instead of into
/// the draw.
///
/// RED at the parent revision: COMPILE-red. The arm names
/// `AppState::optimize_run`, which does not exist there — the state is a
/// bare `is_optimizing: bool`.
///
/// The probe is `UiCommand::ToggleSimPlayback`, and NOT
/// `UiCommand::SwitchWorkspace`. The controller's arm for a workspace
/// switch is a no-op (`controller/events/mod.rs:586`); `RsCamApp` applies
/// that command in `app/input.rs`, because the switch also moves the
/// camera and the overlay set. So a controller test cannot read a
/// workspace switch at all, and the probe has to be a command the
/// controller itself applies.
#[test]
fn a_view_command_still_applies_while_an_optimize_run_is_in_flight() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    assert!(
        controller.state.optimize_run.is_some(),
        "the preview is in flight, so the run is on the state"
    );
    assert_eq!(controller.compute.job_requests.len(), 1);
    assert!(
        !controller.state.simulation.playback.playing,
        "the probe must flip a value, so read it first"
    );

    controller.handle_internal_event(AppEvent::Ui(UiCommand::ToggleSimPlayback(NoArgs)));

    assert!(
        controller.state.simulation.playback.playing,
        "a view command applies during a run — the GUI stays usable"
    );
    assert!(
        controller.state.optimize_run.is_some(),
        "and the unrelated command leaves the run alone"
    );
}

/// Cancel arms THIS submit's flag, closes no window, and clears nothing.
///
/// Three claims in one arm, because they are one design decision:
///
/// * the flag that moves is the per-submit `Arc<AtomicBool>`, not a lane.
///   The `Job` lane is FIFO and shared with the MCP surface, so
///   `cancel_lane(ComputeLane::Job)` would also kill an MCP caller's job
///   queued behind this one — which is why none of the three existing
///   cancels fits the progress row;
/// * the run STAYS on the state with `cancel_requested` set. The drain is
///   the one clearing site, and a cancelled Optimize still returns a
///   partial outcome;
/// * the arm closes no window. The brief names `optimize_modal` here; this
///   fixture has none, so the dialog that must survive is the planner's.
///
/// RED at the parent revision: COMPILE-red. The arm names
/// `UiCommand::CancelOptimizeRun` and `AppState::optimize_run`, and
/// neither exists there.
#[test]
fn cancelling_the_run_arms_one_flag_and_clears_nothing() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    let request = controller
        .compute
        .job_requests
        .first()
        .expect("a preview was submitted");
    let job_id = request.id;
    let cancel = Arc::clone(&request.cancel);
    assert!(!cancel.load(std::sync::atomic::Ordering::SeqCst));

    controller.handle_internal_event(AppEvent::Ui(UiCommand::CancelOptimizeRun(NoArgs)));

    assert!(
        cancel.load(std::sync::atomic::Ordering::SeqCst),
        "the cancel arms the flag of the submit that is running"
    );
    assert_eq!(
        controller.compute.job_lane.state,
        LaneState::Idle,
        "and it does NOT cancel the shared Job lane, which would kill an \
         MCP caller's job queued behind this one"
    );
    assert_eq!(
        controller.compute.optimize_lane.state,
        LaneState::Idle,
        "nor the Optimize lane, which this run does not ride"
    );
    assert_eq!(
        controller.compute.job_requests.len(),
        1,
        "and it submits nothing"
    );
    let run = controller
        .state
        .optimize_run
        .as_ref()
        .expect("the cancel does not clear the run — the drain does");
    assert!(
        run.cancel_requested,
        "the row reads 'cancelling' off this flag"
    );
    assert!(
        controller
            .state
            .multitool_planner
            .as_ref()
            .is_some_and(|planner| planner.open),
        "the cancel closes no window"
    );

    // The answer still lands, and the drain is the one clearing site.
    let result = crate::compute::JobResult {
        id: job_id,
        answer: Err(crate::compute::ComputeError::Cancelled),
    };
    let message = ComputeMessage::Job(Box::new(result));
    controller.compute.drained.push(message);
    controller.drain_compute_results();

    assert!(
        controller.state.optimize_run.is_none(),
        "the drain clears the run on every outcome, a cancel included"
    );
}

/// A second Optimize request is refused, and the refusal is VISIBLE.
///
/// One run at a time is the policy (§28 ruling 8), and the operator kept
/// it as a refusal. With the placeholder gone the buttons are clickable,
/// so the refusal is reachable and a `tracing::warn!` is not enough: a log
/// line tells the operator nothing about why their click did nothing.
///
/// `open_optimize_modal` refuses BEFORE it calls `ProjectSession::start`,
/// so this arm needs no cut trace either.
///
/// RED at the parent revision: ASSERTION-red — the refusal site reports
/// with `tracing::warn!` and pushes no notification, so the submit-count
/// assertion passes and the toast assertion finds nothing. That red is
/// MASKED in practice: the two arms above it are compile-red in the same
/// test target, so the verifier reads a compile error for the whole
/// `--lib` run.
#[test]
fn a_second_optimize_request_is_refused_with_a_toast() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);
    assert_eq!(controller.compute.job_requests.len(), 1);
    assert_eq!(
        controller.active_notifications().count(),
        0,
        "the control: starting one run says nothing"
    );

    let toolpath_id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::OpenOptimizeModal(toolpath_id));

    assert_eq!(
        controller.compute.job_requests.len(),
        1,
        "the second request submits nothing"
    );
    let warnings: Vec<&str> = controller
        .active_notifications()
        .filter(|note| matches!(note.severity, Severity::Warning))
        .map(|note| note.message.as_str())
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "the refusal must push exactly one Warning toast, got {warnings:?}"
    );
    assert!(
        warnings[0].contains("Tier-map preview"),
        "and the toast must NAME the run that is holding the policy, got \
         {warnings:?}"
    );
}

// ── WP29 — the Optimize run reports its stage and its candidate count ─
//
// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
// §33 (operator ruling, 2026-09-13). The row and the window showed a
// spinner, a label and the elapsed seconds. They now name the rung of the
// search ladder and the candidate inside it.
//
// The DRAW half is two source scans in
// `crates/rs_cam_viz/tests/optimize_run_is_non_modal_wp24.rs`. These two
// arms are the TEXT half: one pure builder serves the row and the window,
// so a test reads the sentence the operator reads without an egui pass.
//
// **The WP24 limit still stands.** No controller fixture in this crate
// holds a cut trace, and `capture_optimize_toolpath` refuses without one,
// so the `Toolpath` kind of `OptimizeRun` cannot be DRIVEN in-crate. Arm 1
// therefore fabricates the run and writes the progress by hand, and arm 2
// drives the tier-map preview for the `None` branch. A tier-map preview
// runs no optimizer, so its progress is `None` by construction and it can
// never prove a count.
//
// RED at the parent revision: COMPILE-red. Both arms name
// `OptimizeRun::progress_text`, `OptimizeRun::progress` and
// `rs_cam_core::tool_load::optimize::OptimizeProgress`, and none of the
// three exists there.

/// A run with progress names the rung, the candidate and the list.
///
/// Three claims in one arm, because one builder answers all three: the
/// row's sentence, the window's three-row list, and the mark that says
/// which rung is running. A second arm would fabricate the same run twice.
#[test]
fn a_running_optimize_reports_its_stage_and_its_candidate_count() {
    use rs_cam_core::tool_load::optimize::{OptimizeProgress, SearchPhase};

    let state = AppState::new();
    let progress = Arc::new(OptimizeProgress::default());
    progress.begin_phase(SearchPhase::FeedRpm, 1);
    progress.begin_candidate(0);
    progress.begin_phase(SearchPhase::AxisGrid, 8);
    progress.begin_candidate(2);

    let run = crate::state::OptimizeRun {
        kind: crate::state::OptimizeRunKind::Toolpath {
            toolpath_id: rs_cam_core::ToolpathId(0),
        },
        job_id: Some(crate::compute::JobRequestId(0)),
        started_at: std::time::Instant::now(),
        cancel_requested: false,
        progress: Some(Arc::clone(&progress)),
    };

    let text = run.progress_text(&state.session);
    assert!(
        text.contains("stage 2/3"),
        "the row must name the rung and the ladder length, got {text:?}"
    );
    assert!(
        text.contains("candidate 3/8"),
        "the row must name the candidate now running and the count the \
         rung formed, got {text:?}"
    );

    let rows = run.stage_rows();
    assert_eq!(
        rows.len(),
        3,
        "the window lists every rung of the ladder, not only the running \
         one"
    );
    let marks: Vec<crate::state::OptimizeStageMark> = rows.iter().map(|row| row.mark).collect();
    assert_eq!(
        marks,
        vec![
            crate::state::OptimizeStageMark::Done,
            crate::state::OptimizeStageMark::Current,
            crate::state::OptimizeStageMark::Pending,
        ],
        "the finished rung reads done, the running rung reads current, \
         and the rung the search has not reached reads pending"
    );
    assert!(
        rows.iter()
            .filter_map(|row| row.count.as_deref())
            .any(|count| count.contains("3 / 8")),
        "the running rung's row carries its own count, got {rows:?}"
    );
    assert!(
        rows.last().is_some_and(|row| row.count.is_none()),
        "a rung the search has not announced reports NO count, because \
         its total is not known until it starts: {rows:?}"
    );
}

/// A run with no progress keeps the WP24 sentence.
///
/// The rollup and the tier-map preview run no candidate ladder, so they
/// carry no progress and the row must not invent a rung for them. This is
/// the branch the planner fixture CAN drive.
#[test]
fn a_run_without_progress_keeps_the_elapsed_only_text() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    let run = controller
        .state
        .optimize_run
        .as_ref()
        .expect("the preview is in flight, so the run is on the state");
    assert!(
        run.progress.is_none(),
        "a tier-map preview runs no candidate ladder, so it carries no \
         progress"
    );

    let text = run.progress_text(&controller.state.session);
    assert!(
        !text.contains("stage"),
        "a run with no progress must not claim a rung, got {text:?}"
    );
    assert!(
        text.contains("Tier-map preview"),
        "and it keeps the WP24 label, got {text:?}"
    );
    assert!(
        run.stage_rows().is_empty(),
        "the window lists no rung for a run that walks no ladder"
    );
}

// --- SHL-01: one door rebuilds the post mirror --------------------------

/// The mirror must carry the WHOLE session block, not the one field a
/// route remembered to copy.
///
/// `PostConfig` has no `PartialEq`, so the comparison runs through
/// `post_to_session`, which is the writer half of the same resolver pair
/// `post_from_session` reads. A field that reaches neither half is a
/// field the project file does not carry either.
fn assert_post_mirror_matches_session<B: ComputeBackend>(
    controller: &AppController<B>,
    route: &str,
) {
    let mirrored = crate::state::runtime::GuiState::post_to_session(&controller.state.gui.post);
    assert_eq!(
        &mirrored,
        controller.state.session.post_config(),
        "SHL-01: after {route} the viz post mirror does not match the session block"
    );
}

/// Drive every mirror field away from the session, so a route that copies
/// ONE field back leaves the rest wrong.
fn drift_the_post_mirror<B: ComputeBackend>(controller: &mut AppController<B>) {
    let post = &mut controller.state.gui.post;
    post.format = rs_cam_core::gcode::PostFormat::Mach3;
    post.spindle_speed = post.spindle_speed.wrapping_add(1234);
    post.safe_z += 37.5;
    post.high_feedrate_mode = !post.high_feedrate_mode;
    post.high_feedrate += 111.0;
    post.spindle_strategy = rs_cam_core::feeds::SpindleStrategy::MaxSpeed;
}

/// The controller door that every GUI post route takes rebuilds the whole
/// mirror. W9 / P-1 is the `format` half of this: a reload reset a
/// grblHAL project's dropdown to GRBL because one copy site was wrong.
#[test]
fn the_post_door_rebuilds_the_whole_mirror_shl01() {
    let mut controller = sample_controller();

    let mut post = controller.state.session.post_config().clone();
    post.format = "grblhal".to_owned();
    post.safe_z = 42.0;
    post.spindle_speed = 21_000;
    let effects = controller
        .state
        .session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the session accepts the post block");

    drift_the_post_mirror(&mut controller);
    controller.adopt_post_effects(&effects);

    assert_post_mirror_matches_session(&controller, "adopt_post_effects");
    assert_eq!(
        controller.state.gui.post.format,
        rs_cam_core::gcode::PostFormat::GrblHal,
        "SHL-01 / W9-P-1: the mirror kept the drifted post format"
    );
    assert!(
        (controller.state.gui.post.safe_z - 42.0).abs() < 1e-9,
        "SHL-01: the mirror kept the drifted safe-Z"
    );
}

/// The Feeds & Speeds route writes the session and then reads the mirror
/// back through the same door. Before SHL-01 this arm copied
/// `spindle_strategy` alone, and it copied it even when the command was
/// refused.
#[test]
fn the_spindle_strategy_event_rebuilds_the_whole_mirror_shl01() {
    let mut controller = sample_controller();
    let strategy = rs_cam_core::feeds::SpindleStrategy::MaxSpeed;
    assert_ne!(
        controller.state.session.post_config().spindle_strategy,
        strategy,
        "the fixture must start on the other strategy, or the arm short-circuits"
    );

    drift_the_post_mirror(&mut controller);
    // The drift set the strategy too; put it back so the arm's own
    // guard sees a real change to make.
    controller.state.gui.post.spindle_strategy = rs_cam_core::feeds::SpindleStrategy::MatchChart;
    controller.handle_internal_event(AppEvent::SetSpindleStrategy(strategy));

    assert_eq!(
        controller.state.gui.post.spindle_strategy, strategy,
        "SHL-01: the strategy the operator picked did not reach the mirror"
    );
    assert_post_mirror_matches_session(&controller, "the SetSpindleStrategy event");
}
