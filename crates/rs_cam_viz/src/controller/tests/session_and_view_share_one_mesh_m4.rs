//! M4 (memory programme 2026-10-01) — the session and the viewport hold ONE
//! simulation display mesh, not two.
//!
//! `core_simulation_from_lane` clones the lane's core record into the
//! session, and the view state takes the lane's `mesh` field. While that
//! field was a plain `StockMesh`, the clone was a deep copy of about 96 B
//! per grid cell per setup group. The field is now an `Arc`, and this sentry
//! holds the two holders to the same allocation.

use super::*;

/// After a simulation lands, the view's mesh and the session's mesh are the
/// same `Arc`.
#[test]
fn the_session_and_the_view_share_one_simulation_mesh_m4() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    let view_mesh = &controller
        .state
        .simulation
        .results
        .as_ref()
        .expect("the control: the viewport holds the run")
        .mesh;
    let session_mesh = &controller
        .state
        .session
        .simulation_result()
        .expect("the control: the session adopted the run")
        .mesh;

    // Non-vacuity: two empty meshes would pass a value comparison for free.
    assert!(
        !view_mesh.indices.is_empty(),
        "the fixture mesh must carry a triangle, or the arm measures nothing"
    );
    assert!(
        std::sync::Arc::ptr_eq(view_mesh, session_mesh),
        "the session and the view must share the display mesh; a second \
         allocation is a deep copy of the whole composite mesh"
    );
}
