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
    local_hits: &[usize],
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
    let rapid_collisions: Vec<RapidCollision> = local_hits
        .iter()
        .enumerate()
        .map(|(i, &move_index)| RapidCollision {
            move_index,
            start: P3::new(0.0, 0.0, 5.0),
            // The second hit is the deepest; the verdict cites it.
            end: P3::new(1.0, 1.0, if i == 1 { -3.0 } else { -1.0 }),
        })
        .collect();
    let rapid_collision_move_indices: Vec<usize> = local_hits
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
                // An empty trace, so the triage (the GUI Inspector line)
                // is built.
                cut_trace: Some(Arc::new(
                    rs_cam_core::stock::simulation_cut::SimulationCutTrace::from_samples(
                        0.5,
                        Vec::new(),
                    ),
                )),
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
                prior_stock_sources: std::collections::HashMap::new(),
                display_degrade: None,
                group_starts: Vec::new(),
                stock_change_volumes: Vec::new(),
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));
    controller.drain_compute_results();
}

/// A controller with two generated toolpaths and an adopted run whose rapid
/// collisions all sit on the second one.
fn controller_with_rapids_on_second() -> (AppController<ScriptedBackend>, ToolpathId) {
    let (controller, _, second) = controller_with_hits_on_second(&LOCAL_HITS);
    (controller, second)
}

/// The same run, with the rapid collisions at `local_hits` on the second
/// toolpath. Gives `(controller, first, second)`.
fn controller_with_hits_on_second(
    local_hits: &[usize],
) -> (AppController<ScriptedBackend>, ToolpathId, ToolpathId) {
    let mut controller = sample_controller();
    let first = controller.state.session.toolpath_configs()[0].id;
    let second = push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    land_run_with_rapids_on_second(&mut controller, first, second, local_hits);
    (controller, first, second)
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

/// G-RAPIDFRAME sentry: a rapid collision at the boundary edge (run move ==
/// `end_move` of the first toolpath, local move 0 of the second) has ONE
/// owner on every surface: the attribution helper, the playback cursor, MCP
/// `inspect_collisions`, the core verdict, the triage line (the GUI
/// Inspector) and the GUI issue list.
#[test]
fn every_surface_gives_the_edge_move_to_the_later_toolpath_g_rapidframe() {
    let (mut controller, first, second) = controller_with_hits_on_second(&[0, 5]);
    let state = &controller.state;
    let sim = &state.simulation;

    // The helper and the cursor: `[start, end)`, so move 100 is the
    // second toolpath's local move 0.
    let edge = sim
        .locate_move(SECOND_START)
        .expect("the run holds the move");
    assert_eq!((edge.toolpath_id, edge.local_move), (second, 0));
    assert_eq!(
        sim.locate_move(SECOND_START - 1).map(|loc| loc.toolpath_id),
        Some(first)
    );
    assert_eq!(sim.locate_move(SECOND_END), None, "end_move is not a move");
    assert_eq!(
        sim.cursor_to_local_toolpath_move(SECOND_START)
            .map(|(_, id, local)| (id, local)),
        Some((second, 0))
    );
    assert_eq!(
        sim.cursor_to_local_toolpath_move(SECOND_END)
            .map(|(_, id, local)| (id, local)),
        Some((second, SECOND_END - SECOND_START)),
        "the cursor at the end of the run stays on the last toolpath"
    );

    // MCP `inspect_collisions`.
    #[cfg(feature = "mcp")]
    {
        let json = crate::app::mcp::inspect_collisions_json(sim);
        let rows = json["by_toolpath"].as_array().expect("rows");
        assert_eq!(rows.len(), 1, "{json:#}");
        assert_eq!(rows[0]["toolpath_id"].as_u64(), Some(second.0 as u64));
        let hits = rows[0]["rapid_collisions"].as_array().expect("hits");
        assert_eq!(hits[0]["global_move"].as_u64(), Some(SECOND_START as u64));
        assert_eq!(hits[0]["local_move"].as_u64(), Some(0));
    }

    // The core verdict names the same toolpath with both hits.
    let evidence = sim.project_evidence();
    let summary = state.session.diagnostics_with_evidence(&evidence);
    let count_of = |id: ToolpathId| {
        summary
            .per_toolpath
            .iter()
            .find(|row| row.toolpath_id == id)
            .map(|row| row.rapid_collision_count)
    };
    assert_eq!(count_of(second), Some(2));
    assert_eq!(count_of(first), Some(0));

    // The triage line names the toolpath and the local move.
    let name = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == second)
        .map(|tc| tc.name.clone())
        .expect("the second toolpath has a config");
    let triage = state.session.simulation_triage(&evidence);
    let lines: Vec<&str> = triage
        .safety
        .iter()
        .filter(|f| {
            f.diagnostic.id.as_str() == rs_cam_core::diagnostics::ids::PROJECT_RAPID_COLLISION
        })
        .map(|f| f.diagnostic.message.as_str())
        .collect();
    let want = format!("on TP{second} '{name}' at local move 0 (run move {SECOND_START})");
    assert!(lines.iter().any(|line| line.contains(&want)), "{lines:?}");
    assert!(
        lines.iter().all(|line| !line.contains("retract_z")),
        "the line gives no invented fix: {lines:?}"
    );

    // The GUI issue list.
    let issues = controller
        .state
        .simulation
        .issues(&controller.state.gui, 1000.0);
    let rapid: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == crate::state::simulation::SimulationIssueKind::RapidCollision)
        .collect();
    assert_eq!(rapid.len(), 2);
    assert!(rapid.iter().all(|i| i.toolpath_id == Some(second)));
}

/// G-RAPIDFRAME sentry: the holder check walks ONE toolpath, so its event
/// index is that toolpath's own move index. A hit at local move 3 on the
/// second toolpath is run move 103 and belongs to the second toolpath, not
/// to the first toolpath that holds run move 3.
#[test]
fn a_holder_hit_on_the_second_toolpath_stays_on_it_g_rapidframe() {
    use rs_cam_core::stock::collision::{
        AssemblySegment, CollisionEvent, CollisionKind, CollisionReport, HolderCollisionCheck,
    };
    let (mut controller, _first, second) = controller_with_hits_on_second(&[]);
    let checks = &mut controller.state.simulation.checks;
    checks.collision_report = Some(CollisionReport {
        collisions: vec![CollisionEvent {
            move_index: 3,
            position: P3::new(1.0, 1.0, -1.0),
            penetration_depth: 0.5,
            segment: AssemblySegment::Holder,
            kind: CollisionKind::Workpiece,
        }],
        min_safe_stickout: 40.0,
    });
    checks.checked_scope.toolpath_id = Some(second);
    checks.holder_collision_count = 1;
    let sim = &controller.state.simulation;

    let located = sim.located_holder_collisions();
    assert_eq!(located.len(), 1);
    assert_eq!(located[0].toolpath_id, Some(second));
    assert_eq!(located[0].local_move, 3);
    assert_eq!(located[0].global_move, Some(SECOND_START + 3));

    assert!(matches!(
        sim.holder_collision_counts_by_tp().as_slice(),
        [(id, HolderCollisionCheck::Measured(1))] if *id == second
    ));

    #[cfg(feature = "mcp")]
    {
        let json = crate::app::mcp::inspect_collisions_json(sim);
        let rows = json["by_toolpath"].as_array().expect("rows");
        assert_eq!(rows.len(), 1, "{json:#}");
        assert_eq!(rows[0]["toolpath_id"].as_u64(), Some(second.0 as u64));
        let hits = rows[0]["holder_collisions"].as_array().expect("hits");
        assert_eq!(
            hits[0]["global_move"].as_u64(),
            Some((SECOND_START + 3) as u64)
        );
        assert_eq!(hits[0]["local_move"].as_u64(), Some(3));
    }
}
