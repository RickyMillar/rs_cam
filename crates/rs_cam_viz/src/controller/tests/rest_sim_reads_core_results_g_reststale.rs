//! G-RESTSTALE (M-C) — the GUI simulation carves the CORE results, never the
//! GUI copy that outlives a core drop.
//!
//! Plan: `planning/rest_stock_identity_2026-09-24/PLAN.md` §2 and §5 test 8.
//!
//! # The defect
//!
//! An edit drops the edited row and its rest dependents in core, and
//! `stamp_stale` leaves `gui.toolpath_rt[id].result` in place so the
//! viewport keeps drawing it. The GUI simulation builder read that copy. So
//! a Run Simulation after an edit carved the superseded toolpath, stored a
//! `prior_stocks` snapshot for the rest row, and core adopted it: the next
//! rest generation read stock cut by a toolpath the project no longer held.
//! That is the only path that puts the OLD rough's geometry into the
//! scallop's snapshot while the new rough is in place (245 584 moves, B5's
//! count, with the A rough restored).
//!
//! # Red before the fix
//!
//! With the builder reading `rt.result` again, the request after the edit
//! admits all three rows instead of one, and the test fails on the count.

use super::*;

/// A three-row rest chain with every row generated, then one edit.
///
/// Returns the controller after the edit, and the number of rows the
/// simulation request admitted per group before and after it.
#[test]
fn a_simulation_after_an_edit_carves_only_what_core_holds() {
    let mut controller = rest_chain_controller(2);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();

    // Make every row current through the one plan.
    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    pump_until_idle(&mut controller);
    for index in 0..ids.len() {
        assert!(
            controller.state.session.get_result(index).is_some(),
            "row {index} must be generated before the edit"
        );
    }

    // The edit: core drops row 1 and its rest dependent row 2. The GUI copy
    // of both stays for the viewport.
    let effects = controller
        .state
        .session
        .apply(Command::SetToolpathParam(
            rs_cam_core::session::SetToolpathParamArgs {
                index: 1,
                param: "scallop_height".to_owned(),
                value: serde_json::json!(0.02),
            },
        ))
        .expect("scallop_height is a scallop parameter");
    crate::state::stale::stamp_stale(&mut controller.state, &effects.stale);
    assert!(controller.state.session.get_result(1).is_none());
    assert!(controller.state.session.get_result(2).is_none());
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&ids[1])
            .is_some_and(|rt| rt.result.is_some()),
        "non-vacuity: the GUI copy of the edited row must outlive the core drop"
    );

    let before = controller.compute.sim_requests.len();
    assert!(
        controller.run_simulation_with_all(),
        "row 0 is still carved"
    );
    let request = controller
        .compute
        .sim_requests
        .get(before)
        .expect("the run submitted one request");
    let admitted: usize = request.iter().map(|(count, _)| *count).sum();
    assert_eq!(
        admitted, 1,
        "M-C: the simulation must carve only the row core holds, not the \
         superseded GUI copies of rows 1 and 2: {request:?}"
    );
}
