//! WP14b — no view file lends the session to a worker any more.
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §24
//! ruling 2 and §28 items 6 and 7.
//!
//! # The finding this file pins
//!
//! Three view sites moved the whole `ProjectSession` into a worker
//! request and left `ProjectSession::new_empty()` on `AppState` until the
//! worker gave it back:
//!
//! | Site | Request |
//! |---|---|
//! | `controller/events/mod.rs` `open_optimize_modal` | `OptimizeRequest::Toolpath` |
//! | `controller/events/mod.rs` `open_optimize_project` | `OptimizeRequest::Project` |
//! | `controller/events/planner.rs` `request_multitool_preview` | `MultitoolPreview` |
//!
//! Every GUI panel drew against the placeholder for the length of the
//! run, and a panic on the Optimize lane lost the project — which is why
//! that lane alone carries no `catch_unwind`.
//!
//! WP14b clones instead. The per-toolpath run and the tier-map preview
//! become `Job` rows whose handles own their own copy; the project rollup
//! stays on the Optimize lane and takes `session.clone()`. Nothing lends
//! the session out, so `OptimizeResult` carries none back.
//!
//! # Red-first
//!
//! Both arms are ASSERTION-red at the parent revision: the three
//! `mem::replace` expressions are in the tree, `OptimizeResult` declares
//! `pub session`, and the drain assigns `self.state.session =
//! result.session`. The arms name the files they find.
//!
//! # What this file does NOT measure
//!
//! - That the GUI submits on `ComputeLane::Job`. The existing scan
//!   `command_surface_completeness::every_gui_reached_core_row_is_constructed_in_the_view`
//!   holds that: the `OptimizeToolpath` row declares `gui:
//!   Reach::Reached`, so a non-MCP view file must spell
//!   `Job::OptimizeToolpath(`.
//! - Anything about the running controller. The brief measured that no
//!   optimizer-modal controller fixture exists in
//!   `crates/rs_cam_viz/src/controller/tests.rs`, so this package adds no
//!   in-crate arm and states the gap rather than hiding it.
//! - The core half. That is
//!   `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

// ── the source walk ──────────────────────────────────────────────────

/// Every `.rs` file under `crates/rs_cam_viz/src`, recursively.
///
/// Copied from `command_surface_completeness.rs`, which is the only
/// other source scan in this crate. An integration test cannot share a
/// helper with another integration test without a `mod` of its own, and
/// one twelve-line walk is cheaper than that.
fn viz_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir: {e}"));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Strip every `//` comment from one source text.
///
/// A scan that matches a doc comment reports a caller that does not
/// exist. This file's own module doc names two of the deleted symbols,
/// and so does the module doc of `controller/events/planner.rs`.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read one source file, relative to this crate's manifest.
fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(
        path.is_file(),
        "scanned path {} no longer exists",
        path.display()
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The window a `mem::replace` needle is judged on.
///
/// The three deleted sites spell the call over four lines, so a
/// same-line test reads nothing. Three lines after the needle is the
/// whole expression in each case.
const NEEDLE_WINDOW_LINES: usize = 3;

// ── (a) no view file lends the session ───────────────────────────────

/// No production view file moves a session out of `AppState`.
///
/// The needle is `mem::replace(` with the word `session` inside the next
/// few lines. It is scoped to a session receiver on purpose: a
/// `mem::replace` over a modal slot or a `Vec` is ordinary view
/// bookkeeping, and this property is about the project.
#[test]
fn no_view_file_lends_the_session_to_a_worker() {
    let files = viz_sources();
    assert!(
        files.len() > 50,
        "the view source walk found {} files, so the scan measured \
         almost nothing",
        files.len()
    );

    let mut offenders: Vec<String> = Vec::new();
    for path in &files {
        let text = strip_comments(&std::fs::read_to_string(path).unwrap_or_default());
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            if !line.contains("mem::replace(") {
                continue;
            }
            let end = (at + NEEDLE_WINDOW_LINES + 1).min(lines.len());
            let window = lines[at..end].join(" ");
            if window.contains("session") {
                offenders.push(format!("{}:{}", path.display(), at + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a view file still lends the session to a worker: {offenders:?}. \
         §24 ruling 2 and §28 ruling 6 replace all three sites — the \
         per-toolpath run and the tier-map preview clone into a Job \
         handle, and the project rollup passes session.clone()."
    );
}

// ── (b) the Optimize result carries no session ───────────────────────

/// `OptimizeResult` no longer carries the session back, and the drain no
/// longer writes one.
///
/// Two file-scoped needles rather than a walk: both symbols live at one
/// site each, and naming the file is what makes a failure readable.
#[test]
fn the_optimize_result_carries_no_session_back() {
    let worker = strip_comments(&source("src/compute/worker.rs"));
    let at = worker
        .find("pub struct OptimizeResult")
        .expect("the Optimize lane still declares its result type");
    let end = worker[at..]
        .find('}')
        .map_or(worker.len(), |offset| at + offset);
    let declaration = &worker[at..end];
    assert!(
        !declaration.contains("session"),
        "OptimizeResult still carries the session back: {declaration:?}. \
         §28 ruling 6 deletes the field with the three lends."
    );

    let drain = strip_comments(&source("src/controller/events/compute.rs"));
    assert!(
        !drain.contains("self.state.session = result.session"),
        "the compute drain still restores a lent session. Nothing lends \
         one any more, so nothing puts one back."
    );
}
