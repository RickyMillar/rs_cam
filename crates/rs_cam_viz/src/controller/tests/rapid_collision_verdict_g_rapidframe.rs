//! G-RAPIDFRAME (2026-10-01): a rapid collision on a toolpath that does not
//! start at move 0 reaches the project diagnostics and the per-toolpath
//! diagnostics.
//!
//! The simulator writes two parallel lists. `RapidCollision::move_index` is
//! the toolpath's OWN (local) move index. `rapid_collision_move_indices`
//! holds the run-global index (`start_move + local`). The rapid-collision
//! verdict counted the collisions through the global list, but it looked up
//! the worst collision by comparing the LOCAL index against the GLOBAL
//! boundary range. Only a toolpath at `start_move == 0` matched. Every other
//! toolpath got a count and no verdict, so MCP `get_project_diagnostics`
//! answered `[]` while `inspect_collisions` and `get_diagnostics` reported
//! the collisions (rivmap350, "3D Rough 8", 8 rapid collisions).
//!
//! The per-toolpath route (`get_toolpath_diagnostics`) took no collision
//! evidence at all, so it never listed the finding.

use rs_cam_core::diagnostics::{DiagnosticEvidence, Scope};
use rs_cam_core::stock::collision::RapidCollision;

use super::*;

/// The first toolpath covers moves 0..100. The second covers 100..200.
const SECOND_START: usize = 100;
const SECOND_END: usize = 200;
/// Local move indices of the collisions on the second toolpath.
const LOCAL_HITS: [usize; 3] = [7, 40, 63];

/// Land one simulation result in the shape the simulator produces: local
/// indices on each `RapidCollision`, global indices in the parallel list.
fn land_run_with_rapids_on_second(
    controller: &mut AppController<ScriptedBackend>,
    first: ToolpathId,
    second: ToolpathId,
) {
    let boundary = |id: ToolpathId, name: &str, start_move: usize, end_move: usize| {
        crate::compute::worker::SimBoundary {
            id,
            name: name.to_owned(),
            tool_name: "EndMill".to_owned(),
            start_move,
            end_move,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }
    };
    let rapid_collisions: Vec<RapidCollision> = LOCAL_HITS
        .iter()
        .enumerate()
        .map(|(i, &move_index)| RapidCollision {
            move_index,
            start: P3::new(0.0, 0.0, 5.0),
            // The second hit is the deepest; the verdict cites it.
            end: P3::new(1.0, 1.0, if i == 1 { -3.0 } else { -1.0 }),
        })
        .collect();
    let rapid_collision_move_indices: Vec<usize> = LOCAL_HITS
        .iter()
        .map(|&local| SECOND_START + local)
        .collect();
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh: Arc::new(rs_cam_core::stock::stock_mesh::StockMesh::empty()),
                total_moves: SECOND_END,
                deviations: None,
                column_deviations: None,
                boundaries: vec![
                    boundary(first, "Scallop", 0, SECOND_START),
                    boundary(second, "Second", SECOND_START, SECOND_END),
                ],
                checkpoints: Vec::new(),
                rapid_collisions,
                rapid_collision_move_indices,
                cut_trace: None,
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
                prior_stock_sources: std::collections::HashMap::new(),
                display_degrade: None,
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));
    controller.drain_compute_results();
}

/// A controller with two generated toolpaths and an adopted run whose rapid
/// collisions all sit on the second one.
fn controller_with_rapids_on_second() -> (AppController<ScriptedBackend>, ToolpathId) {
    let mut controller = sample_controller();
    let first = controller.state.session.toolpath_configs()[0].id;
    let second = push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    land_run_with_rapids_on_second(&mut controller, first, second);
    (controller, second)
}

fn is_rapid_finding_for(diag: &rs_cam_core::diagnostics::Diagnostic, id: ToolpathId) -> bool {
    diag.id.as_str() == rs_cam_core::diagnostics::ids::PROJECT_RAPID_COLLISION
        && diag.scope == Scope::Toolpath { id }
}

#[test]
fn a_rapid_collision_off_move_zero_reaches_project_diagnostics_g_rapidframe() {
    let (controller, second) = controller_with_rapids_on_second();
    let state = &controller.state;
    let evidence = state.simulation.project_evidence();

    // Control: the view holds the collisions, and the count per toolpath
    // was right before the fix (it reads the global list).
    assert_eq!(
        state.simulation.checks.rapid_collision_move_indices.len(),
        LOCAL_HITS.len()
    );
    let summary = state.session.diagnostics_with_evidence(&evidence);
    let row = summary
        .per_toolpath
        .iter()
        .find(|row| row.toolpath_id == second)
        .expect("the second toolpath has a core row");
    assert_eq!(row.rapid_collision_count, LOCAL_HITS.len());

    // The defect: the project list (MCP `get_project_diagnostics`) must
    // carry the finding, scoped to the toolpath, with the count.
    let project = state.session.diagnose_project_with_evidence(&evidence);
    let finding = project
        .iter()
        .find(|d| is_rapid_finding_for(d, second))
        .unwrap_or_else(|| {
            panic!("no rapid-collision finding for the second toolpath in {project:#?}")
        });
    match &finding.evidence {
        Some(DiagnosticEvidence::Counts {
            count,
            offender_toolpath_ids,
        }) => {
            assert_eq!(*count, LOCAL_HITS.len());
            assert_eq!(offender_toolpath_ids, &vec![second]);
        }
        other => panic!("the finding carries a count, not {other:?}"),
    }
    // The verdict cites the deepest hit by the toolpath's own move index,
    // the same number `inspect_collisions` reports as `local_move`.
    assert!(
        finding.message.contains("local move 40"),
        "{}",
        finding.message
    );

    // The per-toolpath list (MCP `get_toolpath_diagnostics`) lists it too.
    let index = index_of(&controller, second);
    let per_toolpath = state
        .session
        .diagnose_toolpath_with_evidence(index, &evidence)
        .expect("the index is in range");
    let listed: Vec<_> = per_toolpath
        .iter()
        .filter(|d| is_rapid_finding_for(d, second))
        .collect();
    assert_eq!(
        listed.len(),
        1,
        "the per-toolpath list carries the finding once: {per_toolpath:#?}"
    );

    // The first toolpath has no collision, so its list has no such finding.
    let first_list = state
        .session
        .diagnose_toolpath_with_evidence(0, &evidence)
        .expect("the index is in range");
    assert!(
        !first_list
            .iter()
            .any(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::PROJECT_RAPID_COLLISION),
        "{first_list:#?}"
    );
}
