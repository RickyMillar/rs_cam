//! P22-0c-3 (design section 9 item 3), the viz twin of the core sentry
//! `one_contact_finding_on_every_surface_p22_0c`: the core contact of the
//! K6 project (`rs_cam_core/tests/common/k6_project.rs`) reaches every GUI
//! surface.
//!
//! - The inspector ribbon (`collect_diagnostics`) lists
//!   `safety.non_centre_cutting_descent` from the session's measured core
//!   answer, and no static `safety.non_centre_cutting_plunge` beside it.
//! - The GUI evidence (`SimulationState::project_evidence`, the adopt copy
//!   of the contacts in `SimulationChecks`) gives the project list, the MCP
//!   toolpath list and the triage safety list the same finding.
//!
//! `cargo test -p rs_cam_viz -q --test a_contact_finding_reaches_every_gui_surface_p22_0c`

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "../../rs_cam_core/tests/common/k6_project.rs"]
mod k6_project;

use rs_cam_core::diagnostics::ids;
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::ui::properties::{collect_diagnostics, toolpath_panel_snapshot};

#[test]
fn a_core_contact_reaches_every_gui_surface_p22_0c() {
    let session = k6_project::generated_and_simulated();
    let index = k6_project::PLUNGE;
    let tc = &session.toolpath_configs()[index];
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());

    // The inspector ribbon.
    let snapshot =
        toolpath_panel_snapshot(tc.id, &session, &GuiState::default()).expect("the snapshot");
    let tool = session.toolpath_tool(tc).cloned().expect("the tool");
    let heights = session.toolpath_heights(tc).expect("the heights resolve");
    let core_contact = session.core_contact_measured(index, trace.as_deref());
    assert!(core_contact.is_some(), "a fresh run measured the core");
    let ribbon = collect_diagnostics(
        &snapshot.entry,
        Some(&tool),
        &[],
        Some(&heights),
        &snapshot.preconditions,
        &snapshot.model_refs,
        snapshot.suggest_warnings.as_deref(),
        None,
        None,
        core_contact.as_ref(),
    );
    assert!(
        ribbon
            .iter()
            .any(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT),
        "the ribbon lists the contact"
    );
    assert!(
        !ribbon
            .iter()
            .any(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE),
        "the ribbon drops the superseded static plunge"
    );

    // The GUI evidence: the adopt copies the contacts into the checks.
    let mut sim = SimulationState::new();
    let core_result = session.simulation_result().expect("a simulation");
    sim.checks.core_contacts = core_result.non_centre_cutting_descents.clone();
    sim.checks.flute_contacts = core_result.flute_overreach.clone();
    // This bare view holds no run results, so the cut trace (which the
    // triage needs) comes from the session run; every other part is the
    // GUI's own evidence.
    let evidence = rs_cam_core::session::ProjectEvidence {
        cut_trace: trace.as_deref(),
        ..sim.project_evidence()
    };
    let project = session.diagnose_project_with_evidence(&evidence);
    assert!(
        project
            .iter()
            .any(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT),
        "the GUI project list names the contact"
    );
    let own = session
        .diagnose_toolpath_with_evidence(index, &evidence)
        .expect("the toolpath diagnoses");
    assert_eq!(
        own.iter()
            .filter(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT)
            .count(),
        1,
        "the MCP toolpath list names the contact once"
    );
    let triage = session.simulation_triage(&evidence);
    assert!(
        triage
            .safety
            .iter()
            .any(|f| f.diagnostic.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT),
        "the triage safety list names the contact"
    );
}

/// P22-0c-5 (P22-0c-4 open Q2): the GUI holder outcome reads the session
/// door. With a fresh simulation every walked toolpath reads the stock
/// answer (`MeasuredOnStock`), where the raw GUI evidence (no mesh check
/// ran) holds no outcome; the GUI triage and the MCP diagnostics build from
/// `SimulationState::project_evidence_for`.
#[test]
fn the_gui_holder_outcome_reads_the_session_door_p22_0c_5() {
    use rs_cam_core::stock::collision::HolderCollisionCheck;
    let session = k6_project::generated_and_simulated();
    let sim = SimulationState::new();
    assert!(sim.project_evidence().holder_collisions.is_empty());
    let evidence = sim.project_evidence_for(&session);
    assert_eq!(
        evidence.holder_collisions,
        session.holder_checks_with_stock(Vec::new())
    );
    assert!(
        !evidence.holder_collisions.is_empty(),
        "a fresh run measured the stock"
    );
    assert!(
        evidence
            .holder_collisions
            .iter()
            .all(|(_, check)| matches!(check, HolderCollisionCheck::MeasuredOnStock(_))),
        "{:?}",
        evidence.holder_collisions
    );
}
