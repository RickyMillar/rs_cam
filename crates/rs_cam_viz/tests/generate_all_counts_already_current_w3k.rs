//! Wave 3 group K (memory programme 2026-10-01): the `generate_all` reply
//! counts the operations it did not generate because they were already
//! current.
//!
//! Wave 2 measured "Generated 5" on one run and "Generated 7" on the next,
//! for one project. The GUI's auto-regeneration (500 ms after load) had made
//! two 2.5D operations current, or had not. The plan skips a `Generate` step
//! whose operation already holds a current result, and it counted only the
//! operations it generated. The reply now names the skipped ones and the
//! enabled count, so the two runs read "5 + 2 of 7" and "7 + 0 of 7".

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_viz::controller::generate_all::{
    GenerateAllSink, GenerationPlan, PlanStep, generate_all_headline,
};

/// A plan with seven `Generate` steps and the closing simulation, as the
/// project-scope plan builds it.
fn seven_op_plan() -> GenerationPlan {
    let mut steps: Vec<PlanStep> = (1..=7)
        .map(|id| PlanStep::Generate(ToolpathId(id)))
        .collect();
    steps.push(PlanStep::SimulateAll);
    GenerationPlan::new(steps, GenerateAllSink::Gui)
}

#[test]
fn the_headline_names_the_already_current_count_and_the_enabled_count() {
    let mut plan = seven_op_plan();
    plan.generated = 5;
    plan.note_already_current();
    plan.note_already_current();
    plan.simulations = 1;

    let summary = plan.completed_summary();
    assert_eq!(summary.already_current, 2);
    assert_eq!(summary.enabled, 7, "one Generate step per enabled op");

    let headline = generate_all_headline(&summary);
    assert!(
        headline.starts_with("Generated 5 toolpaths, 2 already current (7 enabled;"),
        "{headline}"
    );
}

#[test]
fn a_run_with_nothing_current_reads_as_before_plus_the_enabled_count() {
    let mut plan = seven_op_plan();
    plan.generated = 7;
    let summary = plan.completed_summary();
    assert_eq!(summary.already_current, 0);
    let headline = generate_all_headline(&summary);
    assert!(
        !headline.contains("already current"),
        "a zero count is not printed: {headline}"
    );
    assert!(
        headline.starts_with("Generated 7 toolpaths (7 enabled; 8 steps"),
        "{headline}"
    );
}

#[test]
fn a_simulation_step_is_not_an_enabled_operation() {
    let plan = GenerationPlan::new(vec![PlanStep::SimulateAll], GenerateAllSink::Gui);
    assert_eq!(plan.enabled_in_scope(), 0);
}

#[cfg(feature = "mcp")]
#[test]
fn the_mcp_reply_carries_already_current_and_enabled() {
    use rs_cam_viz::mcp_bridge::build_generate_all_response;

    let mut plan = seven_op_plan();
    plan.generated = 5;
    plan.note_already_current();
    plan.note_already_current();
    let reply: serde_json::Value =
        serde_json::from_str(&build_generate_all_response(&plan.completed_summary()))
            .expect("the reply is JSON");
    assert_eq!(reply["generated"], serde_json::json!(5), "{reply}");
    assert_eq!(reply["already_current"], serde_json::json!(2), "{reply}");
    assert_eq!(reply["enabled"], serde_json::json!(7), "{reply}");
    assert_eq!(reply["ok"], serde_json::json!(true), "{reply}");
}
