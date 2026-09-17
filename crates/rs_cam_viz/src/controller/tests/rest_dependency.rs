//! P2.2 / P2.3 — derived rest regions go stale with their source, and the
//! pencil panel's demand-driven producer hook.

use super::*;

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
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
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
