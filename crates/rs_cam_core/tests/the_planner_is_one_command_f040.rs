//! F040 — the multi-tool planner is one command, and its `Effects` answer
//! for the whole plan.
//!
//! `plan_multitool_finishing` was a public `&mut` writer outside the
//! command registry. It dropped the `Effects` of each removal, and the
//! GUI door mirrored neither `simulation_cleared` nor `Effects.stale`.
//! The planner is now the `PlanMultitoolFinishing` row. One `Effects`
//! window covers every removal and every append.
//!
//! Three claims:
//!
//! 1. A plan over a session that holds a simulation reports
//!    `simulation_cleared`, names every emitted index in `stale`, and
//!    carries the outcome on `Effects::planned`.
//! 2. A re-plan drops the result of a `DerivedRestRegions` consumer of a
//!    replaced op, and `stale` names the consumer. The core dependency
//!    walk finds it: no view-side sweep re-derives the edge.
//! 3. A refusal writes nothing and reports no `Effects`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;

use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource, StockSource};
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::multitool::MultitoolPlanSpec;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, AdoptSimulationArgs, Command, Effects,
    PlanMultitoolFinishingArgs, ProjectSession, ProjectSessionBuilder, ToolpathComputeResult,
};
use rs_cam_core::stock::stock_mesh::StockMesh;

/// A shallow dome on a plate. Nothing here generates, so the terrain only
/// has to pass the model check.
fn dome() -> TriangleMesh {
    common::meshes::height_field(8.0, 0.5, |x, y| {
        let r_sq = x * x + y * y;
        (25.0 - r_sq).max(0.0).sqrt() - 5.0
    })
}

/// One mesh model and a Ø4 / Ø3 ball pair. Returns the session and
/// `(coarse_id, fine_id, model_id)`.
fn session_with_ladder() -> (ProjectSession, usize, usize, usize) {
    let mut builder = ProjectSessionBuilder::new();
    builder = builder.stock(common::session::stock_over(9.0, 12.0));
    let coarse_idx = builder.add_tool(ball_tool_config(4.0));
    let fine_idx = builder.add_tool(ball_tool_config(3.0));
    let coarse_id = builder.tools()[coarse_idx].id.raw();
    let fine_id = builder.tools()[fine_idx].id.raw();
    let model_id = builder.add_model(common::session::mesh_model(dome(), "dome"));
    (builder.build(), coarse_id, fine_id, model_id.raw())
}

fn spec(tool_ids: Vec<usize>, model_id: usize) -> MultitoolPlanSpec {
    MultitoolPlanSpec {
        setup_index: 0,
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
        tool_ids: tool_ids
            .into_iter()
            .map(rs_cam_core::ids::ToolId::from_raw)
            .collect(),
        ..MultitoolPlanSpec::default()
    }
}

fn plan(session: &mut ProjectSession, spec: &MultitoolPlanSpec) -> Effects {
    session
        .apply(Command::PlanMultitoolFinishing(
            PlanMultitoolFinishingArgs {
                spec: Box::new(spec.clone()),
            },
        ))
        .expect("a two-ball ladder over a mesh plans")
}

/// An empty simulation, adopted at the live epoch, so the session holds
/// one before the plan.
fn adopt_an_empty_simulation(session: &mut ProjectSession) {
    let result = SimulationResult {
        mesh: Arc::new(StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
            material_slots: Vec::new(),
        }),
        total_moves: 0,
        deviations: None,
        column_deviations: None,
        boundaries: Vec::new(),
        checkpoints: Vec::new(),
        rapid_collisions: Vec::new(),
        rapid_collision_move_indices: Vec::new(),
        cut_trace: None,
        resolution_clamped: false,
        column_grid_cell_mm: 2.0,
        prior_stocks: std::collections::HashMap::new(),
        prior_stock_sources: std::collections::HashMap::new(),
        display_degrade: None,
        group_starts: Vec::new(),
        stock_change_volumes: Vec::new(),
        obstacle_collisions: Vec::new(),
    };
    let epoch = session.simulation_epoch();
    let _ = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(result),
            epoch,
            modulation: None,
        }))
        .expect("the adopt is at the live epoch");
    assert!(session.simulation_result().is_some());
}

/// An empty generated result, adopted at the current revision.
fn adopt_an_empty_result(session: &mut ProjectSession, index: usize) {
    let revision = session.toolpath_revision_at(index);
    let result = ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    };
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(result),
        }))
        .expect("the adopt is at the current revision");
    assert!(session.result_at(index).is_some());
}

/// Claim 1.
#[test]
fn a_plan_reports_its_effects_and_its_outcome() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    adopt_an_empty_simulation(&mut session);

    let effects = plan(&mut session, &spec(vec![coarse_id, fine_id], model_id));

    assert!(
        effects.simulation_cleared,
        "the plan appends ops, which drops the simulation; the answer must say so"
    );
    assert!(session.simulation_result().is_none());
    let outcome = effects.planned.expect("the row carries its outcome");
    assert_eq!(outcome.toolpath_ids.len(), 2);
    assert!(outcome.replaced.is_empty(), "a first plan replaces nothing");
    for id in &outcome.toolpath_ids {
        let (index, _) = session
            .find_toolpath_config_by_id(*id)
            .expect("the emitted op is in the session");
        assert!(
            effects.stale.contains(&index),
            "the stale set names the emitted op at index {index}: {:?}",
            effects.stale
        );
    }
    assert!(effects.modulated.is_empty());
}

/// Claim 2.
#[test]
fn a_re_plan_drops_the_regions_consumer_of_a_replaced_op() {
    let (mut session, coarse_id, fine_id, model_id) = session_with_ladder();
    let first = plan(&mut session, &spec(vec![coarse_id, fine_id], model_id))
        .planned
        .expect("the row carries its outcome");
    let source_toolpath_id = first.toolpath_ids[0];

    // A hand op that clips to the rest regions of tier 0. `Fresh` stock,
    // so only the Regions edge ties it to the tier.
    let consumer = rs_cam_core::session::ToolpathConfig {
        boundary: BoundaryConfig {
            enabled: true,
            source: BoundarySource::DerivedRestRegions { source_toolpath_id },
            ..BoundaryConfig::default()
        },
        stock_source: StockSource::Fresh,
        ..common::session::toolpath_config(
            "Rest consumer",
            OperationConfig::UnifiedFinish(Default::default()),
            rs_cam_core::ids::ToolId::from_raw(fine_id),
            rs_cam_core::ids::ModelId::from_raw(model_id),
        )
    };
    let consumer_index = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(consumer),
        }))
        .expect("add the consumer")
        .created
        .expect("the add names its index");
    let consumer_id = session.toolpath_configs()[consumer_index].id;
    adopt_an_empty_result(&mut session, consumer_index);

    let effects = plan(&mut session, &spec(vec![coarse_id, fine_id], model_id));

    let (consumer_index, _) = session
        .find_toolpath_config_by_id(consumer_id)
        .expect("the planner keeps a hand op");
    assert!(
        session.result_at(consumer_index).is_none(),
        "the consumer clipped to regions of a removed op; its result must drop"
    );
    assert!(
        effects.stale.contains(&consumer_index),
        "the stale set names the consumer at index {consumer_index}: {:?}",
        effects.stale
    );
    let outcome = effects.planned.expect("the row carries its outcome");
    assert_eq!(outcome.replaced, first.toolpath_ids);
}

/// Claim 3.
#[test]
fn a_refused_plan_writes_nothing() {
    let (mut session, coarse_id, _fine_id, model_id) = session_with_ladder();
    adopt_an_empty_simulation(&mut session);
    let refused = session.apply(Command::PlanMultitoolFinishing(
        PlanMultitoolFinishingArgs {
            spec: Box::new(spec(vec![coarse_id], model_id)),
        },
    ));
    assert!(refused.is_err(), "a one-tool ladder refuses");
    assert_eq!(session.toolpath_count(), 0);
    assert!(
        session.simulation_result().is_some(),
        "a refusal keeps the simulation"
    );
}
