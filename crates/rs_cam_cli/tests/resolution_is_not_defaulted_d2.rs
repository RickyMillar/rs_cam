//! D2 — the batch CLI refuses to guess a simulation cell size.
//!
//! Package W5 of `planning/gen_sim_rest_ux_2026-09-18/`, item (f). The MCP
//! `generate_all` has refused a missing `simulation_resolution_mm` since
//! A/M10, because collision counts and engagement both move with cell size.
//! The `project` command carried `#[arg(long, default_value = "0.5")]`, so
//! the same project answered a question the operator never asked, in the one
//! machine-readable artifact a batch run leaves behind.
//!
//! `rs_cam_cli` declares no `[lib]`, so an integration test cannot call
//! `run_project_command`. Both existing CLI sentries are source scans and so
//! is this one.
//!
//! # Why the CONST IDENTIFIER is the assertion
//!
//! The refusal sentence is now
//! `rs_cam_core::compute::config::REST_NEEDS_RESOLUTION`, interpolated by the
//! MCP refusal and by this command. The text is therefore in NEITHER source,
//! so the identifier is the drift proof: two surfaces, one sentence, no
//! paraphrase.
//!
//! # NOT MEASURED
//!
//! That the refusal fires on a real project. That needs a fixture project
//! with a rest operation and a process run, which no CLI sentry has.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// One source file, with line comments stripped. A scan that reads a comment
/// reports code that is absent.
fn source(raw: &str) -> String {
    raw.lines()
        .map(|line| match line.find("//") {
            Some(at) => line.get(..at).unwrap_or(""),
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn main_source() -> String {
    source(include_str!("../src/main.rs"))
}

fn project_source() -> String {
    source(include_str!("../src/project.rs"))
}

#[test]
fn the_resolution_flag_carries_no_default() {
    let code = main_source();
    assert!(
        code.len() > 5_000,
        "the scan read {} bytes of main.rs; an empty population passes and \
         looks healthy",
        code.len()
    );
    assert!(
        code.contains("resolution: Option<f64>"),
        "the `project` command's --resolution carries a clap default again. \
         clap cannot see the project, so a default here answers the cell-size \
         question before anyone has asked whether the plan simulates (D2)."
    );
}

#[test]
fn the_project_command_reads_the_plan_before_it_answers() {
    let code = project_source();
    assert!(
        code.len() > 10_000,
        "the scan read {} bytes of project.rs; an empty population passes and \
         looks healthy",
        code.len()
    );
    assert!(
        code.contains("generation_plan::plan("),
        "the `project` command no longer walks `session::generation_plan`. \
         Its own ladder would let the CLI, the GUI and the MCP server \
         disagree about what 'make the project current' means."
    );
    assert!(
        code.contains("dependencies::primary_edges("),
        "the command no longer reads the dependency edges, so it cannot tell \
         an operation WAITING on upstream simulated stock from one that \
         failed."
    );
    assert!(
        code.contains("awaiting_prior_stock: Vec<"),
        "ProjectSummary cannot express a blocked operation, so an operation \
         that never generated appears nowhere in summary.json."
    );
}

#[test]
fn both_surfaces_interpolate_one_refusal_sentence() {
    const NEEDLE: &str = "REST_NEEDS_RESOLUTION";

    assert!(
        project_source().contains(NEEDLE),
        "the CLI refusal no longer interpolates {NEEDLE}, so it can drift \
         into a softer claim than the MCP one."
    );

    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let rel = "crates/rs_cam_viz/src/controller/events/compute.rs";
    let raw = std::fs::read_to_string(repo.join(rel))
        .unwrap_or_else(|e| panic!("D2 sentry needs retargeting: {rel}: {e}"));
    assert!(
        source(&raw).contains(NEEDLE),
        "the MCP `generate_all` refusal no longer interpolates {NEEDLE}. Two \
         surfaces refusing for the same reason must say the same words."
    );
}

/// The shared sentence survives a `cargo fmt`.
///
/// A backslash-continued string literal has been reformatted into one with
/// doubled spaces in this repository before. The const is the operator-facing
/// text of two refusals, so the damage would ship.
#[test]
fn the_shared_sentence_is_not_reformatted() {
    let text = rs_cam_core::compute::config::REST_NEEDS_RESOLUTION;
    assert!(
        !text.contains("  "),
        "the shared refusal sentence carries a doubled space: {text:?}"
    );
    assert!(
        text.contains("NOT guessed"),
        "the shared refusal sentence no longer says why: {text:?}"
    );
}
