//! Sentry: **a collision check that FAILED must never read as a clean one**
//! (G-COLFAIL, 2026-09-17).
//!
//! ## The defect this pins
//!
//! `run_project_command` runs a collision check per toolpath and inserts into
//! `collision_reports` only on success. The `Err(e)` arm logged a warning and
//! inserted nothing. Both readers then did `.unwrap_or(0)`, and the status
//! line read `if total_collisions > 0 { "error" } else { "ok" }`.
//!
//! So a toolpath whose collision check FAILED shipped:
//!
//! ```text
//! collision_count: 0, status: "ok"
//! ```
//!
//! A clean bill of health asserted with no evidence, in the CLI's only
//! machine-readable artifact, on the one question that can wreck a machine.
//!
//! ## Why a source scan, and why the PRODUCER
//!
//! The identical shape has been found and fixed twice already on this crate's
//! own CSV reader. Both fixes landed on a CONSUMER. The producer kept writing
//! the zero, so the next consumer inherited the defect.
//!
//! This sentry therefore watches the producer. It reads the source of
//! `project.rs` and asserts the shape that made the lie possible cannot come
//! back: the summary entry's collision count must stay an `Option`, and the
//! failed-check set must still be consulted where the status is decided.
//!
//! A behavioural test would be better. It needs a project fixture whose
//! collision check fails for a reason that is not `MissingGeometry`, and no
//! such fixture exists — `MissingGeometry` is the expected 2D case and is
//! handled separately. Until one exists, the structural guard is what can be
//! written honestly, and it says so here rather than pretending otherwise.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// The producer's source, with line comments stripped. This file discusses the
/// defect by name, and a scan that reads a comment reports code that is absent.
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
fn the_summary_can_express_an_unchecked_toolpath_g_colfail() {
    let code = producer_source();

    assert!(
        code.contains("collision_count: Option<usize>"),
        "ToolpathSummaryEntry::collision_count is no longer an Option. A bare \
         count cannot say \"not checked\", so a failed check will report 0 \
         again — which is what this sentry exists to stop."
    );

    // The failed-check set must exist and must be consulted. Without the
    // second half the field could be `Option` and always `Some`.
    assert!(
        code.contains("collision_check_failed"),
        "the failed-check set is gone from project.rs"
    );
    assert!(
        code.contains("collision_check_failed.contains"),
        "nothing consults the failed-check set when deciding a toolpath's \
         status. The Option would then always be Some and the lie returns."
    );
}

#[test]
fn a_failed_check_is_not_rounded_to_ok_g_colfail() {
    let code = producer_source();

    // The old status expression. Its return is the lie: `else { "ok" }` on a
    // count that a failed check leaves at zero.
    assert!(
        !code.contains(r#"if total_collisions > 0 { "error" } else { "ok" }"#),
        "project.rs decides the per-toolpath status from a bare count again. \
         A failed check leaves that count at zero and the toolpath reads ok."
    );

    assert!(
        code.contains("collision_check_failed"),
        "the status decision no longer knows which checks failed"
    );
}

#[test]
fn the_verdict_does_not_claim_ok_over_a_failed_check_g_colfail() {
    let code = producer_source();

    // The roll-up must test the failed set BEFORE the collision count, so
    // "some checks did not run" is not hidden by a finding from the rest.
    let verdict_at = code
        .find("let verdict = ")
        .expect("the verdict expression is gone");
    let tail = &code[verdict_at..];
    let failed_at = tail
        .find("collision_check_failed")
        .expect("the verdict no longer consults the failed-check set");
    let count_at = tail
        .find("total_collision_count")
        .expect("the verdict no longer consults the collision count");

    assert!(
        failed_at < count_at,
        "the verdict tests the collision COUNT before it tests whether any \
         check failed. A run with failed checks and no findings elsewhere \
         would report a clean verdict."
    );
}

#[test]
fn the_operator_line_says_not_checked_g_colfail() {
    // The machine-readable half is only half. A person reads the printed
    // summary, and it must not render an absent check as a number.
    let code = producer_source();
    assert!(
        code.contains("NOT CHECKED"),
        "the printed per-toolpath line no longer distinguishes an unchecked \
         collision result from a clean one"
    );
}
