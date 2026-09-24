//! Controller smoke: the egui harness, the heights tab, load warnings,
//! the fixture projects, save / open / export, and setup boundaries.

use super::*;

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
                    prior_stock_sources: std::collections::HashMap::new(),
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
