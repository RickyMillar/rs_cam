//! WP14a — `recommend_clearing_strategy` and `preview_tier_map` are
//! `Job` rows whose work step holds no session.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §24 ruling 1, §16 rulings 2 and 3, §22 ruling 3.
//!
//! # The shape this file pins
//!
//! Both rows are READS. They mutate nothing, so neither carries an adopt
//! step. A row runs two synchronous steps:
//!
//! - step (i) is [`ProjectSession::start`]. It runs on the frame loop and
//!   captures every session read the work makes into a handle.
//! - step (ii) is a free function — `execute_recommend_clearing_strategy`
//!   or `execute_preview_tier_map`. It reads the handle and holds no
//!   session, so a caller runs it off the frame loop.
//!
//! Before WP14a both calls ran inline on the GUI frame loop. The advisor
//! plans, simulates and modulates one toolpath per candidate strategy;
//! the preview walks a full residual grid. Each is tens of seconds, and
//! the whole GUI stopped for the duration.
//!
//! # The arms
//!
//! Arm (a) reads the registry column. Both rows declare
//! [`CommandKind::Job`].
//!
//! Arm (b) is a function-pointer coercion, one per row. A `fn` pointer
//! captures nothing, so a step (ii) that held a session borrow, or that
//! took `&ProjectSession`, does not coerce to the pointer type at all.
//! The coercion is the proof; the arm exists to stop the signature
//! widening back.
//!
//! Arm (c) is parity. The two session methods — `&self` reads that
//! predate this package — and the `Job` door answer the same thing on
//! one fixture. The two doors share one capture step, so the equality is
//! true by construction; what the arm measures is that they STAY one
//! computation, and that the answer the `Job` door produces is not
//! empty. A non-vacuity guard runs first in each case, because a `None`
//! recommendation and an empty tier ladder would make an equality of two
//! nothings pass.
//!
//! # The fixtures
//!
//! The advisor answers for `Adaptive3d` alone and returns `Ok(None)` for
//! every other operation, so the fixture is a 40 mm corrugated plate, a
//! Ø6 mm end mill and one `Adaptive3d` operation. The preview needs a
//! mesh and a ladder of at least two tools, so its fixture adds a second
//! cutter of a different tip radius. Both meshes are small on purpose:
//! this sentry runs in the normal gate, not behind `heavy-tests`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{mesh_model, stock_over};
use common::{make_endmill_6mm, session::single_op_session};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::machine::strategy_advisor::StrategyRecommendation;
use rs_cam_core::session::{
    AddToolArgs, Command, CommandId, CommandKind, Job, JobHandle, MultitoolPlanSpec,
    MultitoolPreview, PreviewTierMapArgs, PreviewTierMapHandle, ProjectSession,
    RecommendClearingStrategyArgs, RecommendClearingStrategyHandle, SessionError,
};
use rs_cam_core::session::{execute_preview_tier_map, execute_recommend_clearing_strategy};

// ── fixtures ─────────────────────────────────────────────────────────

/// Half-extent of the fixture plate (mm).
const PLATE_HALF_MM: f64 = 20.0;

/// Height of the corrugation, and of the stock above world Z0 (mm).
const PLATE_HEIGHT_MM: f64 = 4.0;

/// The `Adaptive3d` configuration the advisor ranks.
fn adaptive3d_op() -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        stepover: 1.2,
        depth_per_pass: 2.0,
        stock_to_leave_axial: 0.2,
        feed_rate: 2500.0,
        plunge_rate: 500.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ramp_angle_deg: 3.0,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        fine_stepdown: 0.0,
        detect_flat_areas: false,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        z_blend: false,
        mill_shallow_areas: false,
        shallow_angle_deg: None,
        shallow_stepdown: None,
        spindle_rpm: Some(18_000),
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    })
}

/// One corrugated plate, one Ø6 mm end mill, one `Adaptive3d` operation.
fn advisor_session() -> ProjectSession {
    let mesh = common::meshes::sawtooth_plate(PLATE_HALF_MM, 8.0, PLATE_HEIGHT_MM);
    single_op_session(
        stock_over(PLATE_HALF_MM, PLATE_HEIGHT_MM),
        make_endmill_6mm(),
        mesh_model(mesh, "corrugated_plate"),
        "advisor adaptive3d",
        adaptive3d_op(),
    )
}

/// A ball-nose cutter record at the given diameter.
fn ball_tool(diameter_mm: f64, name: &str) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
    tool.diameter = diameter_mm;
    tool.stickout = 25.0;
    tool.flute_count = 2;
    tool.name = name.to_owned();
    tool
}

/// The advisor fixture plus a two-ball ladder, and the spec over it.
///
/// The ladder is two BALL cutters, not the fixture's flat end mill: a
/// ladder sorts on tip-sphere radius, and a flat end mill's is zero.
fn preview_session() -> (ProjectSession, MultitoolPlanSpec) {
    let mut session = advisor_session();
    let coarse_index = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ball_tool(4.0, "Ball 4mm")),
        }))
        .expect("the tool row refuses nothing")
        .created
        .expect("add_tool reports the new tool index");
    let coarse_id = session.tools()[coarse_index].id.0;
    let fine_index = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ball_tool(2.0, "Ball 2mm")),
        }))
        .expect("the tool row refuses nothing")
        .created
        .expect("add_tool reports the new tool index");
    let fine_id = session.tools()[fine_index].id.0;
    let model_id = session.models()[0].id;
    let spec = MultitoolPlanSpec {
        setup_index: 0,
        model_id,
        tool_ids: vec![coarse_id, fine_id],
        // A coarse cell keeps the walk in the normal gate's budget. The
        // shipped planning band is 0.3-0.6 mm; this fixture is 40 mm
        // wide, so 0.6 mm is about 5 000 cells.
        cell_mm: 0.6,
        tolerance_mm: 0.05,
        margin_mm: 0.5,
        treatment: rs_cam_core::maps::tier_map::ResidualTreatment::SlopeCompensated,
        islands: rs_cam_core::maps::tier_islands::TierIslandParams::default(),
        cusp_height_mm: 0.03,
        coarse_skips_fine_islands: false,
        monotone_cell_decomposition: true,
        tier_strategies: Vec::new(),
    };
    (session, spec)
}

// ── (a) the registry column ──────────────────────────────────────────

/// Both rows declare the `Job` kind.
#[test]
fn both_rows_declare_the_job_kind() {
    assert_eq!(
        CommandId::RecommendClearingStrategy.kind(),
        CommandKind::Job,
        "the strategy advisor is a job: it captures, runs off the frame \
         loop, and adopts nothing"
    );
    assert_eq!(
        CommandId::PreviewTierMap.kind(),
        CommandKind::Job,
        "the tier-map preview is a job: it captures, runs off the frame \
         loop, and adopts nothing"
    );
    assert_eq!(
        CommandId::RecommendClearingStrategy.wire_name(),
        "recommend_clearing_strategy",
        "the registry's wire name IS the MCP tool name"
    );
    assert_eq!(
        CommandId::PreviewTierMap.wire_name(),
        "preview_tier_map",
        "the registry's wire name IS the MCP tool name"
    );
}

// ── (b) step (ii) holds no session ───────────────────────────────────

/// `execute_recommend_clearing_strategy` coerces to a plain function
/// pointer, and runs after the session it was captured from is gone.
#[test]
fn execute_recommend_clearing_strategy_holds_no_session() {
    let step_two: fn(
        &RecommendClearingStrategyHandle,
        &AtomicBool,
    ) -> Result<Option<StrategyRecommendation>, SessionError> = execute_recommend_clearing_strategy;

    let mut session = advisor_session();
    let cancel = AtomicBool::new(false);
    let started = session.start(
        Job::RecommendClearingStrategy(RecommendClearingStrategyArgs { index: 0 }),
        &cancel,
    );
    let JobHandle::RecommendClearingStrategy(handle) =
        started.expect("step (i) captures the advisor's inputs")
    else {
        panic!("the advisor row answers its own handle variant");
    };
    // The session is dropped BEFORE step (ii) runs. A handle that
    // borrowed it would not survive this line.
    drop(session);

    let answer = step_two(&handle, &cancel).expect("step (ii) runs on the handle alone");
    assert!(
        answer.is_some(),
        "the advisor must rank the candidate strategies on an Adaptive3d \
         operation; a None here means the arm measured the fixture and not \
         the job door"
    );
}

/// `execute_preview_tier_map` coerces to a plain function pointer, and
/// runs after the session it was captured from is gone.
#[test]
fn execute_preview_tier_map_holds_no_session() {
    let step_two: fn(&PreviewTierMapHandle, &AtomicBool) -> Result<MultitoolPreview, SessionError> =
        execute_preview_tier_map;

    let (mut session, spec) = preview_session();
    let cancel = AtomicBool::new(false);
    let started = session.start(
        Job::PreviewTierMap(PreviewTierMapArgs {
            spec: Box::new(spec),
        }),
        &cancel,
    );
    let JobHandle::PreviewTierMap(handle) =
        started.expect("step (i) captures the ladder and the geometry")
    else {
        panic!("the preview row answers its own handle variant");
    };
    drop(session);

    let answer = step_two(&handle, &cancel).expect("step (ii) runs on the handle alone");
    assert!(
        answer.map.tier_count >= 2,
        "a two-tool ladder must produce at least two tiers; the preview \
         reported {}",
        answer.map.tier_count
    );
}

// ── (c) the two doors are one computation ────────────────────────────

/// The `Job` door and the session method rank the same candidates.
///
/// `Debug` over the answer compares every field, and it round-trips each
/// `f64` bit, so a wall-clock estimate that moved by one ulp fails here.
#[test]
fn the_advisor_job_door_answers_as_the_session_method_does() {
    let mut session = advisor_session();
    let cancel = AtomicBool::new(false);

    // The oracle first, on the same session, through the door that
    // predates this package.
    let oracle = session
        .recommend_clearing_strategy(0, &cancel)
        .expect("the advisor resolves the Adaptive3d operation");
    assert!(
        oracle.is_some(),
        "the oracle must rank candidates, or the comparison below reads \
         two Nones and asserts nothing"
    );
    let ranked = oracle.as_ref().map_or(0, |rec| rec.ranked.len());
    assert!(
        ranked > 0,
        "the oracle ranked no candidate, so the comparison measures an \
         empty list"
    );

    let started = session.start(
        Job::RecommendClearingStrategy(RecommendClearingStrategyArgs { index: 0 }),
        &cancel,
    );
    let JobHandle::RecommendClearingStrategy(handle) =
        started.expect("step (i) captures the advisor's inputs")
    else {
        panic!("the advisor row answers its own handle variant");
    };
    let job = execute_recommend_clearing_strategy(&handle, &cancel)
        .expect("step (ii) runs on the handle alone");

    assert_eq!(
        format!("{job:?}"),
        format!("{oracle:?}"),
        "the Job door and the session method must be one computation"
    );
}

/// The `Job` door and the session method preview the same ladder.
///
/// The comparison is field-wise rather than `Debug`-wise: the preview
/// carries a per-cell label grid, and a `Debug` string over it would be
/// megabytes of text for one equality.
#[test]
fn the_preview_job_door_answers_as_the_session_method_does() {
    let (mut session, spec) = preview_session();
    let cancel = AtomicBool::new(false);

    let oracle = session
        .preview_multitool_plan(&spec, &cancel)
        .expect("the preview walks the two-tool ladder");
    assert!(
        oracle.map.tier_count >= 2,
        "the oracle previewed {} tiers; a one-tier map makes the \
         comparison below vacuous",
        oracle.map.tier_count
    );
    let oracle_cells = oracle.map.tier_cell_counts();
    assert!(
        oracle_cells.iter().sum::<usize>() > 0,
        "the oracle labelled no cell, so the census comparison below \
         measures an empty grid"
    );

    let started = session.start(
        Job::PreviewTierMap(PreviewTierMapArgs {
            spec: Box::new(spec),
        }),
        &cancel,
    );
    let JobHandle::PreviewTierMap(handle) =
        started.expect("step (i) captures the ladder and the geometry")
    else {
        panic!("the preview row answers its own handle variant");
    };
    let job =
        execute_preview_tier_map(&handle, &cancel).expect("step (ii) runs on the handle alone");

    assert_eq!(job.tool_ids, oracle.tool_ids, "the ladder order must agree");
    assert_eq!(
        job.tool_names, oracle.tool_names,
        "the tool names must agree"
    );
    assert_eq!(
        format!("{:?}", job.cusp_radii_mm),
        format!("{:?}", oracle.cusp_radii_mm),
        "the cusp radii must agree bit for bit"
    );
    assert_eq!(
        job.map.grid.cell_mm, oracle.map.grid.cell_mm,
        "the planning cell must agree"
    );
    assert_eq!(
        job.map.grid.nx, oracle.map.grid.nx,
        "the grid width must agree"
    );
    assert_eq!(
        job.map.grid.ny, oracle.map.grid.ny,
        "the grid height must agree"
    );
    assert_eq!(
        job.map.tier_count, oracle.map.tier_count,
        "the tier count must agree"
    );
    assert_eq!(
        job.map.tier_cell_counts(),
        oracle_cells,
        "the per-tier cell census must agree"
    );
    assert_eq!(
        format!("{:?}", job.islands.total_owned_area_mm2()),
        format!("{:?}", oracle.islands.total_owned_area_mm2()),
        "the owned area must agree bit for bit"
    );
}
