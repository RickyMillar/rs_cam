//! Sentry: the default-findings validator reaches the batch CLI (CMP-25,
//! 2026-09-18).
//!
//! W5 item (g) renamed the artifact key from `stale_defaults` to
//! `default_findings`. `stale` means "out of date against the current
//! inputs"; a finding here means "never chosen". The CORE function keeps its
//! name, because `rs_cam_core::compute::validate` is not W5's file set, so
//! this scan still looks for `validate_stale_defaults(`.
//!
//! ## The gap this pins
//!
//! `compute::validate::validate_stale_defaults` ran on the MCP project
//! summary, and `validate_one_toolpath` ran in the GUI properties panel, the
//! core export precondition and `diagnose_toolpath_with_trace`. The batch
//! CLI called neither. `rg -n 'stale' crates/rs_cam_cli/src` returned three
//! comment lines and no call, so `summary.json` — the CLI's only
//! machine-readable artifact — carried no stale-default row at all.
//!
//! An operator running the CLI therefore got a project report that was
//! silent on a class of finding the GUI showed.
//!
//! ## Why a source scan, and why the PRODUCER
//!
//! Same reason as `a_failed_collision_check_is_not_a_clean_one_g_colfail`:
//! the artifact's shape is decided in one place and the defect is an ABSENT
//! call, which no consumer can detect. A behavioural test needs a project
//! fixture that trips one of the four rules; the rules key on a
//! pre-improvement default (`DropCutter::min_z <= -49.999`, a wood adaptive
//! stepover below `0.15 x diameter`, and so on), so such a fixture is a
//! dated artefact rather than something a test can honestly construct from
//! today's defaults. The structural guard is what can be written honestly,
//! and it says so here rather than pretending otherwise.
//!
//! ## Non-vacuity
//!
//! The scan asserts the producer source is present and large before it
//! asserts anything about its content. A scan that reads nothing passes and
//! looks healthy.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// The producer's source, with line comments stripped. This file names the
/// call it looks for, and a scan that reads a comment reports code that is
/// absent.
fn producer_source() -> String {
    let raw = include_str!("../src/project.rs");
    raw.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_project_command_runs_the_default_findings_validator_cmp25() {
    let code = producer_source();
    assert!(
        code.len() > 10_000,
        "the scan read {} bytes of project.rs; an empty or tiny population \
         passes and looks healthy",
        code.len()
    );

    assert!(
        code.contains("validate::validate_stale_defaults("),
        "the CLI `project` command no longer calls \
         `compute::validate::validate_stale_defaults`. Its summary.json then \
         reports nothing about stale defaults, while the GUI and MCP both do \
         — which is the gap CMP-25 closed."
    );
    assert!(
        code.contains("default_findings: Vec<rs_cam_core::compute::validate::StaleDefault>"),
        "ProjectSummary no longer carries the validator's findings, so the \
         call above has nowhere to land in the artifact."
    );
    assert!(
        code.contains("default_findings,"),
        "ProjectSummary is built without its `default_findings` field, so the \
         findings are computed and discarded."
    );
}

/// The rule library states that it is CLOSED, so an empty result is not read
/// as "this project has no stale defaults".
///
/// The header used to claim "adding a rule per future B-roadmap entry is the
/// ongoing convention". No rule had been added since 2026-05-20, and a
/// convention nobody follows reads as a promise the module keeps.
#[test]
fn the_rule_library_states_that_it_is_closed_cmp25() {
    let header = include_str!("../../rs_cam_core/src/compute/validate.rs");
    assert!(
        header.contains("CLOSED at four rules"),
        "`compute/validate.rs`'s header no longer says the rule library is \
         closed. The next reader will treat an empty result as \"no stale \
         defaults\" rather than as \"none of these four rules fired\"."
    );
}
