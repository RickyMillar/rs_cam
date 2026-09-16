//! G-HOLDERSTALE (F2.12) — the holder-clearance row must not report a verdict
//! it can no longer stand behind.
//!
//! Every other readiness row declares itself out of date through one counter.
//! [`crate::state::runtime::GuiState::edit_counter`] counts `mark_edited`
//! calls; the simulation stamps it at SUBMIT
//! ([`crate::state::simulation::SimulationState::submitted_edit_counter`],
//! F2.10) and again at DRAIN into `SimulationRunMeta::last_sim_edit_counter`,
//! and [`crate::state::simulation::SimulationState::is_stale`] compares the
//! two. The collision check stamps that counter zero times, so
//! [`crate::ui::readiness::holder_clearance_check`] answers from
//! `SimulationChecks` alone and its verdict survives every edit.
//!
//! The row is a safety row. It is the one that says the cutter will not hit
//! the workholding.
//!
//! # What this file walks
//!
//! A table of EDIT CLASSES, not one edit. Each entry names the collision
//! input it moves — the obstacle set, the holder assembly, or the motion —
//! and drives the real GUI route that moves it. The walk runs twice, and the
//! two runs pin OPPOSITE contracts:
//!
//! - A CLEAR verdict must be withdrawn. Absence of a strike was never proof
//!   of clearance, so once an input moves there is no evidence either way.
//! - A COLLIDING verdict must NOT be withdrawn. The strikes were measured,
//!   and an unrelated edit is not evidence that they are gone. The row keeps
//!   `Fail` and names the count; only the detail says the evidence is old.
//!
//! A fix that flattened both to one answer would satisfy neither.
//!
//! # Non-vacuity
//!
//! A gate handed an empty population passes and looks healthy. Three floors
//! stop that here:
//!
//! - [`edit_classes`] must not be empty.
//! - Every class must move `edit_counter`. A class that does not is reported
//!   BY NAME, because a collision input that no counter follows is the defect
//!   this row exists to catch.
//! - The collision verdict must arrive through the real drain arm
//!   (`collision_report` is `Some` and a submit reached the lane), so the
//!   walk cannot pass on a verdict the test wrote by hand.
//!
//! # Placement
//!
//! Beside the code, not in `tests/`, for the reason
//! `fixpoint_resolution_notice_g_resnotice.rs` gives: the two entry points
//! this file drives, `request_collision_check` and `drain_compute_results`,
//! are `pub(crate)`. Nothing here starts a GUI or an MCP server.

use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::stock::collision::{CollisionEvent, CollisionKind, CollisionReport};

use crate::compute::{
    CollisionRequest, CollisionResult, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use crate::controller::AppController;
use crate::state::job::{ModelKind, ModelUnits, SetupId};
use crate::state::toolpath::{StockSource, ToolpathId};
use crate::ui::AppEvent;
use crate::ui::readiness::{CheckStatus, holder_clearance_check};

// ── the lane double ────────────────────────────────────────────────────────

/// Accepts every submission, counts the collision ones, and hands back
/// whatever has been queued on it.
///
/// F2.13 widened this to `pub(super)` and made it RECORD the motion each
/// collision submit carried. `holder_clearance_scope_g_holderscope`'s
/// contract is about which toolpaths one check examines, and the only way
/// to read that off the real submit is to keep the request.
#[derive(Default)]
pub(super) struct ScriptedLane {
    pub(super) drained: Vec<ComputeMessage>,
    pub(super) collision_submits: usize,
    /// The motion each collision submit carried, in submit order.
    pub(super) collision_motion: Vec<Arc<rs_cam_core::toolpath_spans::AnnotatedToolpath>>,
}

impl ComputeBackend for ScriptedLane {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, request: CollisionRequest) {
        self.collision_submits += 1;
        self.collision_motion.push(Arc::clone(&request.annotated));
    }
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

// ── the project ────────────────────────────────────────────────────────────

pub(super) fn toolpath(id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: format!("Op {id}"),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Face),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: StockSource::Fresh,
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// One tool with a holder, one meshed model, one operation, one fixture.
///
/// `request_collision_check` refuses unless it can find a toolpath that has a
/// GUI result, a tool matching `tool_id` and a model carrying a mesh, so all
/// three are required before the check can run at all. The fixture is here so
/// the obstacle-edit classes have something to move.
pub(super) fn seeded_controller() -> AppController<ScriptedLane> {
    let mut controller = AppController::with_backend(ScriptedLane::default());
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    builder
        .add_toolpath(0, toolpath(0))
        .expect("the fixture project takes one operation");
    controller.state.session = builder.build();
    controller.handle_internal_event(AppEvent::AddFixture(SetupId(0)));
    land_a_result_for(&mut controller, ToolpathId(0));
    controller
}

/// Finish the operation the way the lane would, so the GUI holds the motion
/// the collision check reads.
pub(super) fn land_a_result_for(controller: &mut AppController<ScriptedLane>, tp_id: ToolpathId) {
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        rs_cam_core::toolpath::Toolpath::new(),
    ));
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: tp_id,
                revision: None,
                result: Ok(crate::state::toolpath::ToolpathResult {
                    annotated,
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
}

/// Which verdict the lane is asked to return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Verdict {
    Clear,
    Colliding,
}

pub(super) fn report_for(verdict: Verdict) -> CollisionReport {
    match verdict {
        Verdict::Clear => CollisionReport {
            collisions: Vec::new(),
            min_safe_stickout: 45.0,
        },
        Verdict::Colliding => CollisionReport {
            collisions: vec![CollisionEvent {
                move_idx: 0,
                position: P3::new(0.0, 0.0, 0.0),
                penetration_depth: 1.5,
                segment: "holder".to_owned(),
                kind: CollisionKind::Workpiece,
            }],
            min_safe_stickout: 58.0,
        },
    }
}

/// Run the whole collision check the way the operator's "Re-check" button
/// does: the real request, the real lane reply, the real drain arm.
pub(super) fn run_collision_check(controller: &mut AppController<ScriptedLane>, verdict: Verdict) {
    let before = controller.compute.collision_submits;
    controller.handle_internal_event(AppEvent::RunCollisionCheck);
    assert_eq!(
        controller.compute.collision_submits,
        before + 1,
        "the fixture must reach the lane, or the walk proves nothing"
    );
    controller
        .compute
        .drained
        .push(ComputeMessage::Collision(Ok(CollisionResult {
            report: report_for(verdict),
            positions: Vec::new(),
        })));
    controller.drain_compute_results();
    assert!(
        controller
            .state
            .simulation
            .checks
            .collision_report
            .is_some(),
        "the verdict must arrive through the drain arm, not from the test"
    );
}

// ── the edit classes ───────────────────────────────────────────────────────

/// One operator edit that moves an input the holder verdict is computed from.
struct EditClass {
    /// What the operator did.
    name: &'static str,
    /// Which collision input it moves.
    input: &'static str,
    /// The real GUI route.
    apply: fn(&mut AppController<ScriptedLane>),
}

fn edit_stock_height(controller: &mut AppController<ScriptedLane>) {
    // WP6: the GUI route is the stock panel's apply funnel, not the
    // deleted `AppEvent::StockChanged`.
    let mut stock = controller.state.session.stock_config().clone();
    stock.z += 5.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, stock);
}

fn edit_fixture_geometry(controller: &mut AppController<ScriptedLane>) {
    // WP6: the GUI route is `Command::ReplaceFixture` through the
    // fixture panel's apply funnel.
    let found = controller
        .state
        .session
        .list_setups()
        .first()
        .and_then(|setup| {
            setup
                .fixtures
                .first()
                .map(|fixture| (fixture.id, fixture.clone()))
        });
    if let Some((fixture_id, mut fixture)) = found {
        let setup_index = 0;
        fixture.origin_z += 12.0;
        fixture.size_z += 12.0;
        crate::ui::properties::apply_fixture_draft(
            &mut controller.state,
            setup_index,
            fixture_id,
            fixture,
        );
    }
}

fn add_another_fixture(controller: &mut AppController<ScriptedLane>) {
    controller.handle_internal_event(AppEvent::AddFixture(SetupId(0)));
}

fn add_a_keep_out_zone(controller: &mut AppController<ScriptedLane>) {
    controller.handle_internal_event(AppEvent::AddKeepOut(SetupId(0)));
}

fn edit_tool_stickout(controller: &mut AppController<ScriptedLane>) {
    let mut draft = controller
        .state
        .session
        .tools()
        .first()
        .expect("the fixture project carries one tool")
        .clone();
    draft.stickout += 15.0;
    crate::ui::properties::commit_tool_draft(&mut controller.state, ToolId(1), draft);
}

fn edit_holder_diameter(controller: &mut AppController<ScriptedLane>) {
    let mut draft = controller
        .state
        .session
        .tools()
        .first()
        .expect("the fixture project carries one tool")
        .clone();
    draft.holder_diameter += 10.0;
    crate::ui::properties::commit_tool_draft(&mut controller.state, ToolId(1), draft);
}

fn disable_the_operation(controller: &mut AppController<ScriptedLane>) {
    controller.handle_internal_event(AppEvent::ToggleToolpathEnabled(ToolpathId(0)));
}

/// Every edit class that moves an input `check_collisions` reads.
///
/// Deliberately NOT a list of everything that bumps `edit_counter`. The
/// counter is project-wide and coarse — a rename stales the simulation today
/// (F2.10 §3) — and pinning that coarseness here would make a later
/// narrowing fail this sentry. What is pinned is the other direction: an
/// edit that really does change the holder verdict's inputs must never leave
/// the previous verdict standing.
fn edit_classes() -> Vec<EditClass> {
    vec![
        EditClass {
            name: "stock height",
            input: "the motion, and the frame the fixtures sit in",
            apply: edit_stock_height,
        },
        EditClass {
            name: "fixture position and size",
            input: "the obstacle the holder must clear",
            apply: edit_fixture_geometry,
        },
        EditClass {
            name: "a fixture added",
            input: "the obstacle set",
            apply: add_another_fixture,
        },
        EditClass {
            name: "a keep-out zone added",
            input: "the obstacle set",
            apply: add_a_keep_out_zone,
        },
        EditClass {
            name: "tool stickout",
            input: "how far the holder sits from the tip",
            apply: edit_tool_stickout,
        },
        EditClass {
            name: "holder diameter",
            input: "the holder's own geometry",
            apply: edit_holder_diameter,
        },
        EditClass {
            name: "operation switched off",
            input: "which motion the job contains",
            apply: disable_the_operation,
        },
    ]
}

// ── the contract ───────────────────────────────────────────────────────────

/// THE sentry. Walk every edit class over a CLEAR verdict.
///
/// Two claims, and the first is why the row could never be believed even
/// before an edit. `holder_clearance_check` gated `Pass` on
/// `min_safe_stickout.is_some()`, and the drain writes that field only when
/// the check FOUND collisions — so a clean, current check reported "Not
/// checked". The row had no way to say the thing it exists to say.
#[test]
fn a_clear_verdict_is_reported_and_then_withdrawn_g_holderstale() {
    let classes = edit_classes();
    assert!(
        !classes.is_empty(),
        "an empty walk passes and proves nothing"
    );

    for class in &classes {
        let mut controller = seeded_controller();
        run_collision_check(&mut controller, Verdict::Clear);

        assert_eq!(
            holder_clearance_check(&controller.state),
            CheckStatus::Pass,
            "before the {} edit: a current check that found nothing must say \
             so — this is the row's whole purpose ({})",
            class.name,
            class.input
        );

        let counter_before = controller.state.gui.edit_counter;
        (class.apply)(&mut controller);
        assert!(
            controller.state.gui.edit_counter > counter_before,
            "the '{}' edit moves {} and no counter follows it — this class is \
             NOT COVERED by the project's freshness model",
            class.name,
            class.input
        );

        assert_eq!(
            holder_clearance_check(&controller.state),
            CheckStatus::Warning,
            "after the {} edit: the check answered a project the operator has \
             left, so the row must ask for a re-check, not report Clear",
            class.name
        );
    }
}

/// The same walk over a verdict that FOUND a collision — and here the
/// contract is the OPPOSITE one: the severity must survive the edit.
///
/// A stale clear verdict is an abstention. A stale colliding verdict is not:
/// the strikes were measured, and a feed-rate edit cannot move a holder out
/// of a clamp. Dropping to `Warning` would turn red into amber on the one row
/// an operator scans for red, and it would put this row out of step with the
/// export gate, which trips on the raw count either way.
///
/// This test passes on the unfixed code too, by construction — the pre-fix
/// row also answered `Fail` here. That is the point: it is a PRESERVATION
/// guard, and it is what stops a later change to the staleness rule from
/// de-escalating a measured strike.
#[test]
fn a_colliding_verdict_keeps_its_severity_after_an_edit_g_holderstale() {
    for class in &edit_classes() {
        let mut controller = seeded_controller();
        run_collision_check(&mut controller, Verdict::Colliding);
        let strikes = controller.state.simulation.checks.holder_collision_count;
        assert!(strikes > 0, "the colliding fixture must carry a strike");

        assert_eq!(
            holder_clearance_check(&controller.state),
            CheckStatus::Fail,
            "before the {} edit: a current check that found a strike must fail",
            class.name
        );

        (class.apply)(&mut controller);

        assert_eq!(
            holder_clearance_check(&controller.state),
            CheckStatus::Fail,
            "after the {} edit: the strike was MEASURED, and an edit is not \
             evidence that it is gone — this row must not de-escalate",
            class.name
        );
        assert_eq!(
            controller.state.simulation.checks.holder_collision_count, strikes,
            "and the count must survive the edit, so the row can still name it"
        );
    }
}

/// The other half of the contract: no edit, no withdrawal.
///
/// A fix that answered `Warning` to everything would satisfy both walks
/// above and be useless. This is what discriminates.
#[test]
fn a_check_with_no_edit_after_it_stays_current_g_holderstale() {
    let mut clear = seeded_controller();
    run_collision_check(&mut clear, Verdict::Clear);
    assert_eq!(holder_clearance_check(&clear.state), CheckStatus::Pass);

    let mut colliding = seeded_controller();
    run_collision_check(&mut colliding, Verdict::Colliding);
    assert_eq!(holder_clearance_check(&colliding.state), CheckStatus::Fail);
}

/// A project that has never been checked says so, and says so at every
/// stage before the verdict lands.
#[test]
fn an_unchecked_project_is_not_a_verdict_g_holderstale() {
    let controller = seeded_controller();
    assert_eq!(
        holder_clearance_check(&controller.state),
        CheckStatus::Warning,
        "no check has run; absence of a strike is not evidence of clearance"
    );
}

/// The late-arrival race, on the collision lane.
///
/// F2.10 closed this on the simulation: the stamp belongs to the moment of
/// SUBMIT, not the moment of arrival, so an edit made while the check runs
/// is not folded into the record of when it ran. The collision lane shares
/// the same analysis queue and had no stamp at all.
///
/// The dangerous direction is a stale ALL CLEAR, so that is the arm run
/// here: the operator fits a longer tool while the lane works, and the lane
/// comes back clear about the short one.
#[test]
fn an_edit_during_the_check_leaves_the_verdict_stale_g_holderstale() {
    let mut controller = seeded_controller();

    controller.handle_internal_event(AppEvent::RunCollisionCheck);
    assert_eq!(
        controller.compute.collision_submits, 1,
        "the check must have been submitted, or the race is not being run"
    );

    // The operator lengthens the stickout while the lane works.
    edit_tool_stickout(&mut controller);

    // The lane answers about the holder that was fitted when it started.
    controller
        .compute
        .drained
        .push(ComputeMessage::Collision(Ok(CollisionResult {
            report: report_for(Verdict::Clear),
            positions: Vec::new(),
        })));
    controller.drain_compute_results();

    assert!(
        controller
            .state
            .simulation
            .checks
            .collision_report
            .is_some(),
        "the result is KEPT — it is still the only evidence there is"
    );
    assert_eq!(
        holder_clearance_check(&controller.state),
        CheckStatus::Warning,
        "but it must not read Clear: it measured the holder the operator has \
         already replaced"
    );

    // The control. Same verdict, no edit in flight, and the row may say so.
    let mut control = seeded_controller();
    run_collision_check(&mut control, Verdict::Clear);
    assert_eq!(
        holder_clearance_check(&control.state),
        CheckStatus::Pass,
        "with no edit in flight the same verdict is current, or the guard \
         above is firing on everything"
    );
}

// ── the state and the words ────────────────────────────────────────────────
//
// `HolderClearance` and `holder_clearance_detail` do not exist on the
// unfixed code, so these two tests arrive with the fix that introduces them.
// They are not red-first and are not claimed to be: there is no earlier
// behaviour for them to contradict. They matter because the tier is the
// SAME in four of the five states — pre-fix and post-fix both answer `Fail`
// to a stale strike and `Warning` to a stale clear — so the detail string is
// where the staleness reaches the operator, and it needs pinning.

/// Walk the classes again and pin the STATE, not just the tier.
#[test]
fn the_stale_states_name_what_they_are_g_holderstale() {
    use crate::ui::readiness::{HolderClearance, holder_clearance_state};

    let classes = edit_classes();
    assert!(!classes.is_empty(), "an empty walk proves nothing");

    for class in &classes {
        let mut clear = seeded_controller();
        run_collision_check(&mut clear, Verdict::Clear);
        assert_eq!(
            holder_clearance_state(&clear.state),
            HolderClearance::Clear,
            "the walk starts from a current clear check"
        );
        (class.apply)(&mut clear);
        assert_eq!(
            holder_clearance_state(&clear.state),
            HolderClearance::StaleClear,
            "after the {} edit a clear verdict becomes an abstention",
            class.name
        );

        let mut colliding = seeded_controller();
        run_collision_check(&mut colliding, Verdict::Colliding);
        let strikes = colliding.state.simulation.checks.holder_collision_count;
        assert_eq!(
            holder_clearance_state(&colliding.state),
            HolderClearance::Collisions(strikes),
            "and from a current check that found strikes"
        );
        (class.apply)(&mut colliding);
        assert_eq!(
            holder_clearance_state(&colliding.state),
            HolderClearance::StaleCollisions(strikes),
            "after the {} edit a measured strike keeps its count and gains \
             the fact that the evidence is old",
            class.name
        );
    }
}

/// The row's words. A stale strike must say the count and say the evidence
/// is old, in that order, so neither fact needs a hover.
#[test]
fn the_detail_says_the_count_before_it_says_the_age_g_holderstale() {
    use crate::ui::readiness::holder_clearance_detail;

    let fresh = seeded_controller();
    assert_eq!(holder_clearance_detail(&fresh.state), "Not checked");

    let mut clear = seeded_controller();
    run_collision_check(&mut clear, Verdict::Clear);
    assert_eq!(holder_clearance_detail(&clear.state), "Clear");
    edit_tool_stickout(&mut clear);
    let stale_clear = holder_clearance_detail(&clear.state);
    assert!(
        !stale_clear.contains("Clear"),
        "a withdrawn clearance claim must not still read as one: {stale_clear}"
    );
    assert!(
        stale_clear.contains("re-check"),
        "and it must say what to do: {stale_clear}"
    );

    let mut colliding = seeded_controller();
    run_collision_check(&mut colliding, Verdict::Colliding);
    assert_eq!(holder_clearance_detail(&colliding.state), "1 collision(s)");
    edit_tool_stickout(&mut colliding);
    let stale_strike = holder_clearance_detail(&colliding.state);
    let count_at = stale_strike
        .find("1 collision(s)")
        .expect("the count survives the edit and is still named");
    let age_at = stale_strike
        .find("edited")
        .expect("and the row says the evidence is old");
    assert!(
        count_at < age_at,
        "count first, then the age: {stale_strike}"
    );
}
