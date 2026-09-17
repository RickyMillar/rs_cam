//! The multi-tool planner dialog, and the active setup the selection names.

use super::*;

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
