//! G-HOLDERSCOPE (F2.13) — the holder-clearance row must not present a
//! one-operation answer as a job-wide verdict.
//!
//! `AppController::request_collision_check` runs `find_map` over the
//! operation list and submits the FIRST toolpath that has a result, a cutter
//! and a mesh. One toolpath, one tool, one holder. Both surfaces that consume
//! the reply — the Readiness panel and the export pre-flight card — then title
//! the row "Holder clearance" for the whole job.
//!
//! So a job whose second operation carries a longer holder, or a shorter
//! stickout, is never asked about, and the row reads `Clear`. The operator
//! reads that as "the tool will not hit the workholding".
//!
//! The defect is the SCOPE, not the arithmetic. The verdict about operation 1
//! is correct; the label it is published under is not.
//!
//! # What this file pins
//!
//! One rule: **a safety row must not overstate its scope.** A verdict that
//! covers one operation of two must not read `Pass`, and the row must say
//! which operation it measured.
//!
//! It deliberately does NOT pin how many toolpaths a check examines. Widening
//! the check to every toolpath is the other honest fix, and a lane that later
//! measures the cost and widens it must not have to edit this file: a widened
//! check covers the job, and every assertion here still holds.
//!
//! # What this file cannot show
//!
//! The lane double returns whatever the test queues on it — the real collision
//! sweep never runs here — so the second operation's STRIKE is the motivating
//! scenario, not a measured fact. What is measured is the POPULATION: two
//! operations carry motion, exactly one submit reaches the lane, and the
//! motion it carries is operation 1's.
//!
//! # Non-vacuity
//!
//! A gate handed an empty or partial population passes and looks healthy.
//! Four floors stop that here:
//!
//! - the job carries more than one enabled operation;
//! - every operation carries a result, so every one of them COULD have been
//!   examined;
//! - the two operations carry distinct motion, so `Arc::ptr_eq` discriminates;
//! - exactly one submit reaches the lane, and it carries operation 1's motion.
//!
//! A single-operation control asserts the row still reads `Pass` where the
//! evidence really does cover the job, so a fix that answered `Warning` to
//! everything would fail here.
//!
//! # Placement
//!
//! Beside the code, for the reason F2.12 gives in the file next door:
//! `request_collision_check` and `drain_compute_results` are `pub(crate)`, so
//! an integration test could only assert over a hand-written `AppState` and
//! would prove the derivation rather than the wiring. Nothing here starts a
//! GUI or an MCP server.

use std::sync::Arc;

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

use crate::controller::AppController;
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::readiness::{CheckStatus, holder_clearance_check, holder_clearance_detail};

use super::holder_clearance_staleness_g_holderstale::{
    ScriptedLane, Verdict, land_a_result_for, run_collision_check, seeded_controller, toolpath,
};

// ── the job ────────────────────────────────────────────────────────────────

/// Two enabled operations, each with its own cutter and its own motion.
///
/// The second tool carries a longer stickout than the first, which is the
/// case the row cannot see: a holder that clears the workholding on operation
/// 1 need not clear it on operation 2.
fn two_operation_job() -> AppController<ScriptedLane> {
    let mut controller = seeded_controller();

    let mut second_tool = ToolConfig::new_default(ToolId(2), ToolType::EndMill);
    second_tool.stickout += 20.0;
    let mut tools = controller.state.session.tools().to_vec();
    tools.push(second_tool);
    let _ = controller.state.session.replace_tools(tools);

    let mut second_op = toolpath(1);
    second_op.tool_id = 2;
    controller
        .state
        .session
        .add_toolpath(0, second_op)
        .expect("the fixture project takes a second operation");
    land_a_result_for(&mut controller, ToolpathId(1));

    controller
}

/// The motion the GUI holds for one operation. `expect` rather than an
/// `Option`, because a fixture that landed no result would make every
/// assertion below vacuous.
fn motion_of(controller: &AppController<ScriptedLane>, id: ToolpathId) -> Arc<AnnotatedToolpath> {
    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&id)
        .expect("the fixture lands a result for every operation");
    let result = rt
        .result
        .as_ref()
        .expect("the fixture lands a result for every operation");
    Arc::clone(&result.annotated)
}

/// Enabled operations in the job — the population the row is labelled for.
fn enabled_operations(controller: &AppController<ScriptedLane>) -> usize {
    controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .count()
}

// ── the contract ───────────────────────────────────────────────────────────

/// THE sentry. Two operations, one examined, and the row must not read a
/// clean job-wide verdict off that one.
#[test]
fn a_two_operation_job_does_not_read_clear_off_one_operation_g_holderscope() {
    let mut controller = two_operation_job();

    assert_eq!(
        enabled_operations(&controller),
        2,
        "the job must carry more than one operation, or there is no scope to \
         overstate"
    );

    let first = motion_of(&controller, ToolpathId(0));
    let second = motion_of(&controller, ToolpathId(1));
    assert!(
        !Arc::ptr_eq(&first, &second),
        "the two operations must carry distinct motion, or the check below \
         cannot be told apart from a job-wide one"
    );

    run_collision_check(&mut controller, Verdict::Clear);

    let submitted = &controller.compute.collision_motion;
    assert_eq!(
        submitted.len(),
        1,
        "one check, one submit — this is the fact the row is published on top of"
    );
    assert!(
        Arc::ptr_eq(&submitted[0], &first),
        "the check examines operation 1 and never asks about operation 2"
    );

    assert_eq!(
        controller.state.simulation.checks.holder_collision_count, 0,
        "operation 1 is clear; the whole defect is what the row then says \
         about operation 2"
    );

    assert_ne!(
        holder_clearance_check(&controller.state),
        CheckStatus::Pass,
        "the evidence covers one operation of two, so the row must not read a \
         clean job-wide verdict"
    );
}

/// The row must say what it measured, not only decline to say more.
///
/// A `Warning` with the word "Clear" beside it is still an operator reading
/// clearance for a job. The detail names the operation the check examined.
#[test]
fn a_partial_verdict_names_the_operation_it_measured_g_holderscope() {
    let mut controller = two_operation_job();
    run_collision_check(&mut controller, Verdict::Clear);

    let detail = holder_clearance_detail(&controller.state);
    assert_ne!(
        detail, "Clear",
        "a verdict about one operation must not be published as the job's"
    );
    assert!(
        detail.contains("Op 0"),
        "the row must name the operation it measured: {detail}"
    );
}

/// The discriminator. Where the evidence really does cover the job, the row
/// still says so.
///
/// A fix that answered `Warning` to every holder verdict would satisfy the
/// two tests above and destroy the row F2.12 made reachable. This is also the
/// arm that keeps F2.12's own contract green.
#[test]
fn a_single_operation_job_still_reads_clear_g_holderscope() {
    let mut controller = seeded_controller();
    assert_eq!(
        enabled_operations(&controller),
        1,
        "the control must be a job of one, or it is not controlling anything"
    );

    run_collision_check(&mut controller, Verdict::Clear);

    assert_eq!(
        holder_clearance_check(&controller.state),
        CheckStatus::Pass,
        "one operation examined, one operation in the job — the evidence \
         covers it and the row may say so"
    );
    assert_eq!(holder_clearance_detail(&controller.state), "Clear");
}

/// The examined operation need not be one the job will cut.
///
/// `find_map` carries no `tc.enabled` test, and `AppEvent::ToggleToolpathEnabled`
/// does not drop `gui.toolpath_rt[..].result` — it flips the config and calls
/// `mark_edited` (`controller/events/mod.rs:120-129`). So a DISABLED first
/// operation whose result the GUI still holds is the one that gets checked,
/// the verdict describes motion the job does not contain, and every operation
/// the job DOES contain is unexamined.
///
/// This is the same overstatement as the test above, through the door the
/// selection rule leaves open. The arithmetic closes it; the selection is not
/// changed.
#[test]
fn a_disabled_operation_is_not_a_verdict_about_the_job_g_holderscope() {
    let mut controller = two_operation_job();
    controller.handle_internal_event(AppEvent::ToggleToolpathEnabled(ToolpathId(0)));

    assert_eq!(
        enabled_operations(&controller),
        1,
        "one operation is switched off, so the job is the other one"
    );
    // `expect` inside: the toggle must LEAVE the result, or the check would
    // not reach this operation at all and the test would prove nothing.
    let disabled_motion = motion_of(&controller, ToolpathId(0));

    run_collision_check(&mut controller, Verdict::Clear);

    let submitted = &controller.compute.collision_motion;
    assert_eq!(submitted.len(), 1, "one check, one submit");
    assert!(
        Arc::ptr_eq(&submitted[0], &disabled_motion),
        "the switched-off operation is still the one the check examines"
    );

    assert_ne!(
        holder_clearance_check(&controller.state),
        CheckStatus::Pass,
        "the one operation the job will cut was never examined, so the row \
         must not read a clean job-wide verdict"
    );
}

// ── the population, and the fix that introduces it ─────────────────────────
//
// `HolderCheckScope` and `HolderClearance::PartialClear` do not exist on the
// unfixed code, so the test below arrives with the fix. It is not red-first
// and is not claimed to be: there is no earlier behaviour for it to
// contradict. It matters because the population is what the three tests above
// assert THROUGH, and an empty population is the shape that passes every gate
// and looks healthy.

/// The verdict carries the population it was measured over, and an empty
/// population never reads as a clean job.
#[test]
fn a_partial_verdict_carries_its_own_population_g_holderscope() {
    use crate::state::simulation::HolderCheckScope;
    use crate::ui::readiness::{HolderClearance, holder_clearance_state};

    let mut controller = two_operation_job();
    run_collision_check(&mut controller, Verdict::Clear);

    let scope = controller.state.simulation.checks.checked_scope;
    assert_eq!(scope.examined, 1, "one toolpath reached the checker");
    assert_eq!(scope.population, 2, "the job carries two operations");
    assert_eq!(scope.position, Some(1), "the first one is what it read");
    assert!(
        !scope.covers_the_job(),
        "one of two is not the job, and the row is titled for the job"
    );
    assert_eq!(scope.unexamined(), 1, "one operation was never asked");

    assert_eq!(
        holder_clearance_state(&controller.state),
        HolderClearance::PartialClear(scope),
        "the state names the partiality; it does not leave it to the words"
    );

    assert!(
        !HolderCheckScope::default().covers_the_job(),
        "an EMPTY population must not read as a covered job — a gate handed \
         one passes and looks healthy"
    );
}
