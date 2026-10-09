//! The accepted simulation run and its result: they arrive and leave
//! together, the two request builders agree, and a debug trace survives.

use super::*;

#[test]
fn an_accepted_run_and_an_accepted_result_arrive_and_leave_together_ur3() {
    // The derived simulation freshness reads `last_run` and `results`.
    // The two must arrive and leave together on EVERY transition, or the
    // reader would ask a run that has no evidence (or evidence with no run).
    //
    // W2-E: one state is the exception. A re-run releases the view and
    // keeps `last_run`, because the core still holds that run's evidence
    // (`SimFreshness::Released`). See the release steps at the end.
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

    // W2-E: a re-run releases the view and keeps `last_run`. The freshness
    // names that state, and only that state breaks the pairing.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let sim = &controller.state.simulation;
    assert!(!sim.has_results() && sim.last_run.is_some());
    assert_eq!(
        controller.state.simulation_freshness(),
        crate::state::freshness::SimFreshness::Released { in_flight: true }
    );

    // The re-run is cancelled: still released, `last_run` still names the
    // run whose evidence the core holds.
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();
    let sim = &controller.state.simulation;
    assert!(!sim.has_results() && sim.last_run.is_some());
    assert_eq!(
        controller.state.simulation_freshness(),
        crate::state::freshness::SimFreshness::Released { in_flight: false }
    );

    // Reset ends the release: both leave together again.
    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));
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
    let predicate =
        crate::ui::readiness::simulation_request_is_buildable(&controller.state.session);
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
    let predicate =
        crate::ui::readiness::simulation_request_is_buildable(&controller.state.session);
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
    let predicate =
        crate::ui::readiness::simulation_request_is_buildable(&controller.state.session);
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
    let predicate =
        crate::ui::readiness::simulation_request_is_buildable(&controller.state.session);
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

/// CONTRACT — CMP-19. The GUI request builder and
/// `ProjectSession::run_simulation` answer `metrics_not_applicable` with
/// ONE predicate, `rs_cam_core::compute::simulate::entry_metrics_not_applicable`.
///
/// Three signals answer it: the §6.E first-class drill op, a
/// `MoveIntent::Drilling` move, and the operation kind. The GUI builder
/// tested the operation kind ALONE until 2026-09-18, while core tested all
/// three. Only `ops/drill.rs` emits the intent today, and it emits it on
/// the two drilling op kinds, so the two builders agreed for every shipped
/// generator — the divergence was structural, and a new drilling generator
/// would have made it live: the GUI's simulation would emit per-sample
/// air-cut and low-engagement issues that core's would suppress.
///
/// The teeth are case 2: a milling op kind whose generated toolpath carries
/// a drilling move. An op-kind-only builder answers `false` there.
#[test]
fn the_gui_builder_reads_every_drill_signal_cmp19() {
    use rs_cam_core::compute::simulate::entry_metrics_not_applicable;
    use rs_cam_core::toolpath::MoveIntent;

    let mut controller = sample_controller();
    let tc = &controller.state.session.toolpath_configs()[0];
    let id = tc.id;
    let op_type = tc.operation.op_type();
    assert!(
        !matches!(
            op_type,
            rs_cam_core::compute::catalog::OperationType::Drill
                | rs_cam_core::compute::catalog::OperationType::AlignmentPinDrill
        ),
        "the fixture's op kind must NOT be a drilling kind, or signal 3 \
         would answer every case on its own"
    );

    // Replace the fixture's generated toolpath, keeping every other field.
    let seed = |controller: &mut AppController<ScriptedBackend>, tp: Toolpath| {
        controller
            .state
            .gui
            .toolpath_rt
            .get_mut(&id)
            .expect("the fixture seeds a runtime for toolpath 0")
            .result
            .as_mut()
            .expect("the fixture seeds a result")
            .annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            tp,
        ));
        // G-RESTSTALE (M-C): the GUI builder carves the CORE result, so the
        // seed writes the same toolpath there too.
        let (annotated, drill_op) = {
            let held = controller
                .state
                .gui
                .toolpath_rt
                .get(&id)
                .and_then(|rt| rt.result.as_ref())
                .expect("the seed above wrote a result");
            (Arc::clone(&held.annotated), held.drill_op.clone())
        };
        let op_data = match drill_op {
            Some(drill) => rs_cam_core::ops::drill_op::OpData::DrillOp(drill, annotated),
            None => rs_cam_core::ops::drill_op::OpData::Toolpath(annotated),
        };
        let revision = controller.state.session.toolpath_revision(0);
        let _ = controller
            .state
            .session
            .apply(Command::AdoptResult(AdoptResultArgs {
                index: 0,
                revision,
                result: Box::new(rs_cam_core::session::ToolpathComputeResult {
                    op_data,
                    stats: Default::default(),
                    debug_trace: None,
                    semantic_trace: None,
                }),
            }))
            .expect("the fixture revision is current");
    };
    let flag = |controller: &AppController<ScriptedBackend>| {
        let (groups, _, _) = controller
            .build_simulation_groups(|_, tc| tc.enabled, |_| false)
            .expect("the fixture project builds one group");
        groups[0].toolpaths[0].metrics_not_applicable
    };

    // Case 1 — a milling op with no drilling move.
    let mut milling = Toolpath::new();
    milling.rapid_to(P3::new(0.0, 0.0, 5.0));
    milling.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    seed(&mut controller, milling);
    assert!(
        !flag(&controller),
        "case 1: a milling op with no drilling signal reports measurable metrics"
    );

    // Case 2 — the same op kind, one drilling move.
    let mut drilled = Toolpath::new();
    drilled.rapid_to(P3::new(0.0, 0.0, 5.0));
    drilled.feed_to_with_intent(P3::new(0.0, 0.0, -1.0), 200.0, MoveIntent::Drilling);
    seed(&mut controller, drilled);
    assert!(
        flag(&controller),
        "case 2: a drilling move must set `metrics_not_applicable`, whatever \
         the op kind says. This is the signal the GUI builder used to miss."
    );

    // Case 3 — the first-class drill op, stated against the shared
    // predicate because a `DrillOp` literal carries fourteen fields.
    assert!(
        entry_metrics_not_applicable(true, &Toolpath::new(), op_type),
        "case 3: a first-class drill op sets the flag on its own"
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

/// S2. The GUI request builder carries a setup's ENABLED stock changes, in
/// order, through the shared `group_stock_changes`, so its list is the one
/// core's builder carries. A setup with a stock change and no generated
/// toolpath still emits a group, and the readiness predicate agrees.
#[test]
fn the_gui_builder_carries_the_stock_changes_s2() {
    use rs_cam_core::compute::stock_change::{
        StockChange, StockChangeOp, StockChangeSource, StockGeometry,
    };
    use rs_cam_core::ids::{ModelId, StockChangeId};

    let mut controller = sample_controller();
    for rt in controller.state.gui.toolpath_rt.values_mut() {
        rt.result = None;
    }
    let model_id = ModelId(controller.state.session.models()[0].id);
    let change = |id: usize, enabled: bool| StockChange {
        id: StockChangeId(id),
        name: format!("Change {id}"),
        enabled,
        op: StockChangeOp::Remove,
        geometry: StockGeometry::Model { model_id },
        material: Default::default(),
        display_colour: None,
    };
    for (id, enabled) in [(1, true), (2, false), (3, true)] {
        let _ = controller
            .state
            .session
            .apply(Command::AddStockChange(
                rs_cam_core::session::AddStockChangeArgs {
                    setup_index: 0,
                    change: Box::new(change(id, enabled)),
                },
            ))
            .expect("the flat mesh is a valid Model geometry");
    }

    let predicate =
        crate::ui::readiness::simulation_request_is_buildable(&controller.state.session);
    let (groups, _, _) = controller
        .build_simulation_groups(|_, tc| tc.enabled, |_| false)
        .expect("a setup with a stock change emits a group");
    assert!(predicate, "the primary and the builder agree");
    assert_eq!(groups.len(), 1);
    assert!(groups[0].toolpaths.is_empty(), "nothing is generated");
    let carried: Vec<usize> = groups[0]
        .stock_changes
        .iter()
        .map(|c| c.change.id.0)
        .collect();
    assert_eq!(carried, vec![1, 3], "the enabled changes, in list order");
    let mesh = controller.state.session.models()[0]
        .mesh
        .clone()
        .expect("the fixture model is a mesh");
    for resolved in &groups[0].stock_changes {
        match resolved.sources.as_slice() {
            [StockChangeSource::Mesh(source)] => assert!(Arc::ptr_eq(source, &mesh)),
            other => panic!("expected one mesh source, got {other:?}"),
        }
    }
    let core = rs_cam_core::compute::simulate::group_stock_changes(
        &controller.state.session.list_setups()[0],
        controller.state.session.models(),
    );
    assert_eq!(
        core.iter().map(|c| &c.change).collect::<Vec<_>>(),
        groups[0]
            .stock_changes
            .iter()
            .map(|c| &c.change)
            .collect::<Vec<_>>(),
        "one resolution, two builders"
    );
}

/// S5 (`planning/stock_additions_2026-10-09/PLAN.md`): the viewport legend
/// gives a row per material of the simulated stock, with the colour the
/// stock views draw it in.
mod stock_material_legend_s5 {
    use super::*;

    use rs_cam_core::compute::stock_change::{StockChange, StockChangeOp, StockGeometry};
    use rs_cam_core::compute::stock_change_apply::StockChangeVolume;
    use rs_cam_core::export::material_colour::{MaterialPalette, colour_from_rgb8};
    use rs_cam_core::ids::{ModelId, StockChangeId};
    use rs_cam_core::material::Material;
    use rs_cam_core::stock::material_slot::MaterialSlot;

    use crate::state::Workspace;
    use crate::state::simulation::StockVizMode;
    use crate::ui::overlays::legend_rail::{self, Categories, RailLine};

    const DISPLAY: [u8; 3] = [200, 40, 160];

    fn resin() -> Material {
        Material::Custom {
            name: "Resin".to_owned(),
            feed_scale_factor: 1.0,
        }
    }

    /// A controller with one `Add` change on setup 0 and a simulation that
    /// applied it (slot 1), the Simulation workspace and the stock drawn.
    fn controller_with_an_added_material(volumes: bool) -> AppController<ScriptedBackend> {
        let mut controller = sample_controller();
        let model_id = ModelId(controller.state.session.models()[0].id);
        let change = StockChange {
            id: StockChangeId(1),
            name: "Pour".to_owned(),
            enabled: true,
            op: StockChangeOp::Add,
            geometry: StockGeometry::Model { model_id },
            material: resin(),
            display_colour: Some(DISPLAY),
        };
        let _ = controller
            .state
            .session
            .apply(Command::AddStockChange(
                rs_cam_core::session::AddStockChangeArgs {
                    setup_index: 0,
                    change: Box::new(change),
                },
            ))
            .expect("the flat mesh is a valid Model geometry");
        let setup_id = controller.state.session.list_setups()[0].id;
        let stock_change_volumes = if volumes {
            vec![StockChangeVolume {
                setup_id,
                change_id: StockChangeId(1),
                name: "Pour".to_owned(),
                op: StockChangeOp::Add,
                material_slot: Some(MaterialSlot(1)),
                added_mm3: 1500.0,
                removed_mm3: 0.0,
            }]
        } else {
            Vec::new()
        };
        // The submit door stamps the epoch; the adopt reads it (D7).
        controller.state.simulation.submitted_simulation_epoch =
            Some(controller.state.session.simulation_epoch());
        controller
            .compute
            .drained
            .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
                core: rs_cam_core::compute::simulate::SimulationResult {
                    mesh: Arc::new(rs_cam_core::stock::stock_mesh::StockMesh::empty()),
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
                    prior_stocks: std::collections::HashMap::new(),
                    prior_stock_sources: std::collections::HashMap::new(),
                    display_degrade: None,
                    group_starts: Vec::new(),
                    stock_change_volumes,
                },
                playback_data: Vec::new(),
                cut_trace_path: None,
            }))));
        controller.drain_compute_results();
        controller.state.workspace = Workspace::Simulation;
        controller.state.viewport.show_sim_stock = true;
        controller.state.simulation.stock_viz_mode = StockVizMode::Solid;
        controller
    }

    fn has_materials_line(controller: &AppController<ScriptedBackend>) -> bool {
        legend_rail::active_lines(&controller.state)
            .contains(&RailLine::Categories(Categories::StockMaterials))
    }

    #[test]
    fn the_legend_lists_each_material_of_the_simulated_stock_s5() {
        let controller = controller_with_an_added_material(true);
        assert!(controller.state.simulation.has_results());
        let sim = controller.state.session.simulation_result();
        assert_eq!(
            sim.map(|s| s.stock_change_volumes.len()),
            Some(1),
            "the session holds the simulation and its volume"
        );
        assert_eq!(controller.state.session.stock_material_legend().len(), 2);
        assert!(has_materials_line(&controller), "no Stock materials line");

        let entries = legend_rail::category_entries(&controller.state, Categories::StockMaterials);
        let stock_label = controller.state.session.stock_config().material.label();
        let palette = MaterialPalette::default();
        assert_eq!(
            entries,
            vec![
                (stock_label, Some(palette.colour(MaterialSlot::STOCK))),
                ("Resin".to_owned(), Some(colour_from_rgb8(DISPLAY))),
            ]
        );
        // The legend colour is the colour the stock views draw.
        assert_eq!(
            controller
                .state
                .session
                .stock_material_palette()
                .colour(MaterialSlot(1)),
            colour_from_rgb8(DISPLAY)
        );
        assert_eq!(
            legend_rail::line_name(
                &controller.state,
                &RailLine::Categories(Categories::StockMaterials)
            ),
            "Stock materials \u{00B7} 2"
        );
    }

    #[test]
    fn the_materials_line_shows_only_when_the_plain_stock_colours_are_drawn_s5() {
        let mut controller = controller_with_an_added_material(true);
        controller.state.simulation.stock_viz_mode = StockVizMode::ByHeight;
        assert!(
            !has_materials_line(&controller),
            "the height colours hide the materials"
        );
        controller.state.simulation.stock_viz_mode = StockVizMode::Solid;
        controller.state.viewport.show_sim_stock = false;
        assert!(!has_materials_line(&controller), "no stock is drawn");
    }

    #[test]
    fn a_stock_with_no_added_material_has_no_materials_line_s5() {
        // The change exists, but the simulation added no material with it.
        let controller = controller_with_an_added_material(false);
        assert!(controller.state.simulation.has_results());
        assert!(controller.state.session.stock_material_legend().is_empty());
        assert!(!has_materials_line(&controller));
    }
}
