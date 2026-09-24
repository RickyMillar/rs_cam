//! The accepted simulation run and its result: they arrive and leave
//! together, the two request builders agree, and a debug trace survives.

use super::*;

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
