//! WP24 — an Optimize run does not take the whole window.
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §30
//! ruling 3 (operator, 2026-09-13): the full-screen Optimize placeholder
//! is replaced by a non-modal progress row with a cancel, and the GUI
//! stays usable during a run.
//!
//! # The finding this file pins
//!
//! `crates/rs_cam_viz/src/app.rs` gated the workspace layout dispatch on
//! `AppState::is_optimizing`. While any of the three runs was in flight —
//! the per-toolpath Optimize `Job`, the project rollup, or the tier-map
//! preview — the whole central panel was one spinner and two labels. The
//! modal's own hint said "The GUI is responsive", which the placeholder
//! made false.
//!
//! The placeholder was a POLICY and not a necessity. WP14b
//! (`04d09453`) stopped every route lending the session to a worker, so
//! the panels read the real project throughout; nothing needed the
//! placeholder after that date.
//!
//! # What each arm measures
//!
//! The DRAW half is a source scan. No test in this crate drives an egui
//! panel: every surface draws through `egui::Ui` and `RsCamApp` needs an
//! `eframe::CreationContext`, so the source check is what stands in —
//! the same argument `workspace_menu_complete_g_wsmenu.rs` and
//! `mcp_toasts_report_outcome_g_mcptoast.rs` make.
//!
//! The STATE half is in-crate, in
//! `crates/rs_cam_viz/src/controller/tests.rs`, under the heading "WP24 —
//! the Optimize run is state, not a lockout". It needs
//! `ScriptedBackend` and the `pub(crate)` drain, so it cannot live here.
//!
//! # Red-first
//!
//! Every arm in THIS file is ASSERTION-red at the parent revision, and
//! every needle here is a string: the file names no new Rust symbol, so
//! the integration binary still compiles against the pre-fix library and
//! the verifier reads real assertion output. The in-crate arms are
//! COMPILE-red instead, because they name `AppState::optimize_run` and
//! `UiCommand::CancelOptimizeRun`.
//!
//! Arms 4 and 5 (WP29) hold to the same rule. Both needles are strings,
//! so both are ASSERTION-red at WP29's parent revision.
//!
//! # What this file does NOT measure
//!
//! - Anything on screen. No cargo ran and no screenshot was taken.
//! - What the row and the window SAY about a rung. Arms 4 and 5 check
//!   that each file reads the shared text builder; the sentence itself is
//!   measured in-crate, under the heading "WP29 — the Optimize run
//!   reports its stage and its candidate count". This entry used to read
//!   "the progress observer … a phase field with no writer is noise",
//!   which WP29 made false: §33 gave the search a writer.
//! - That the three refusal toasts reach the operator's eye. The toast
//!   stack itself is measured in-crate.
//! - Whether every Optimize entry point is DISABLED during a run. Four
//!   are not — three "Optimize this op" buttons in `ui/sim_diagnostics.rs`
//!   and the rollup's "Review ▸" — and each of them refuses with a toast
//!   instead. A uniform refusal is the operator's ruling; a disabled
//!   control is not.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

const APP_SRC: &str = include_str!("../src/app.rs");
const BAR_SRC: &str = include_str!("../src/ui/workspace_bar.rs");
const REGISTRY_SRC: &str = include_str!("../src/ui_command.rs");
const MODAL_SRC: &str = include_str!("../src/ui/optimize_modal.rs");

/// Strip every `//` comment from one source text.
///
/// A scan that matches a doc comment reports code that does not exist.
/// This file's own subject is named in the comments of `app.rs` and of
/// `state/mod.rs`, so the scan must not read one as the gate.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── arm 1 — the workspace layout draws unconditionally ───────────────

/// `app.rs` dispatches the workspace layout with no Optimize gate.
///
/// The SUBJECT is the gate that stood in front of the `match
/// self.controller.state().workspace` dispatch. A blanket "no
/// `is_optimizing` in `app.rs`" is the assertion, because that file has
/// exactly one read of the flag and the repaint check beside it reads
/// lane snapshots instead. Should a future unrelated read of the flag
/// appear here, it is NOT this defect — read the dispatch first.
#[test]
fn app_draws_the_workspace_layout_during_an_optimize_run() {
    let code = strip_comments(APP_SRC);
    assert!(
        !code.contains("is_optimizing"),
        "app.rs still gates drawing on the Optimize flag. The subject is \
         the `if self.controller.state().is_optimizing` in front of the \
         `match self.controller.state().workspace` dispatch: delete it \
         and de-indent the match, per §30 ruling 3."
    );
    assert!(
        code.contains("Workspace::Setup => self.draw_setup_layout(ui)"),
        "the workspace dispatch itself must survive the deletion"
    );
}

/// Neither placeholder heading survives.
///
/// The narrower and more durable half of arm 1: the placeholder picked
/// one of two headings, because the shared `Job` lane also carries the
/// tier-map preview. Both strings go with it.
#[test]
fn no_full_screen_optimize_heading_survives() {
    for heading in ["Optimize is running", "Building the tier map"] {
        assert!(
            !APP_SRC.contains(heading),
            "app.rs still carries the full-screen placeholder heading \
             '{heading}'"
        );
    }
}

// ── arm 2 — the progress row and its cancel ──────────────────────────

/// The workspace bar draws the run, and constructs the cancel row.
///
/// `workspace_bar::draw` runs for EVERY workspace and outside the old
/// gate, which is why the row lives there and not on the status bar (three
/// call sites, none in Simulation) or the viewport overlay (one call site,
/// and Readiness has no viewport).
///
/// The needle `Ui(UiCommand::CancelOptimizeRun(` is the one the WP23
/// constructor census uses, so this arm and
/// `command_surface_completeness::every_gui_reached_view_row_is_constructed_in_the_view`
/// agree about what counts as a caller.
#[test]
fn the_workspace_bar_draws_the_optimize_progress_row() {
    let bar = strip_comments(BAR_SRC);
    assert!(
        bar.contains("optimize_run"),
        "ui/workspace_bar.rs draws no Optimize progress row. It is the \
         one widget that renders in every workspace, so it is where the \
         operator learns a run is in flight once the placeholder is gone."
    );
    assert!(
        bar.contains("Ui(UiCommand::CancelOptimizeRun("),
        "the progress row must construct UiCommand::CancelOptimizeRun. \
         None of the three existing cancels fits: CancelCompute kills \
         every lane, and both Close arms close a window the operator may \
         not have open."
    );
}

/// The registry declares the new row, with a reason on each skip.
#[test]
fn the_registry_declares_the_cancel_optimize_run_row() {
    let registry = strip_comments(REGISTRY_SRC);
    assert!(
        registry.contains("CancelOptimizeRun, \"cancel_optimize_run\""),
        "ui_command.rs declares no CancelOptimizeRun row. A view control \
         that is not a registry row is the hatch WP13 closed."
    );
}

// ── arm 4 — the row reads the shared text builder (WP29) ─────────────

/// `ui/workspace_bar.rs` builds its sentence with `progress_text`.
///
/// §33 (operator, 2026-09-13) asks the row for the stage boundaries and
/// the candidate count. The row must not `format!` those itself: one pure
/// builder on `state::OptimizeRun` serves the row and the in-crate arm, so
/// the sentence a test reads is the sentence the operator reads, and the
/// draw stays a draw.
#[test]
fn the_workspace_bar_reads_the_shared_progress_text() {
    let bar = strip_comments(BAR_SRC);
    assert!(
        bar.contains("progress_text("),
        "ui/workspace_bar.rs still builds the row's sentence itself, so \
         it reports no stage and no candidate count. Call \
         state::OptimizeRun::progress_text instead — one builder for the \
         row and for the in-crate arm, per §33."
    );
}

// ── arm 5 — the window lists the rungs (WP29) ────────────────────────

/// `ui/optimize_modal.rs` draws the rung list while a run is loading.
///
/// The needle is `stage_rows(`, and it is deliberately NOT `optimize_run`
/// or `OptimizeRun`: that file already carries `optimize_run_provenance`
/// and `OptimizeRunStatus`, so either of those would match at the parent
/// revision and this arm would assert nothing.
#[test]
fn the_optimize_window_lists_the_search_rungs() {
    let modal = strip_comments(MODAL_SRC);
    assert!(
        modal.contains("stage_rows("),
        "the Optimize window still shows a bare spinner. §33 asks for the \
         rung list with the running rung marked and its candidate count; \
         read state::OptimizeRun::stage_rows in the Loading arm."
    );
}

// ── arm 6 — non-vacuity ──────────────────────────────────────────────

/// Each scanned file is the file this suite thinks it is.
///
/// Arms 1, 2 and 3 are absence assertions, so a renamed or moved file
/// turns all of them green by finding nothing. This pins one control per
/// scanned source. Precedent:
/// `command_surface_completeness::the_p2_view_locator_finds_a_known_construction`.
///
/// Arms 4 and 5 are presence assertions, so a renamed file turns them RED
/// rather than green. They are covered here all the same, because a
/// writer who reads a red arm needs to know whether the needle moved or
/// the file did.
#[test]
fn the_source_scans_find_a_known_control() {
    let app = strip_comments(APP_SRC);
    assert!(
        app.contains("egui::Panel::top(\"workspace_bar\")"),
        "app.rs no longer shows the workspace bar panel, so the arms \
         that scan it assert nothing"
    );
    let bar = strip_comments(BAR_SRC);
    assert!(
        bar.contains("Ui(UiCommand::SwitchWorkspace("),
        "ui/workspace_bar.rs no longer constructs the tab command, so \
         the progress-row arm asserts nothing"
    );
    let registry = strip_comments(REGISTRY_SRC);
    assert!(
        registry.contains("CloseOptimizeModal, \"close_optimize_modal\""),
        "ui_command.rs no longer declares the neighbouring Optimize row, \
         so the registry arm asserts nothing"
    );
    let modal = strip_comments(MODAL_SRC);
    assert!(
        modal.contains("OptimizeRunStatus::Loading"),
        "ui/optimize_modal.rs no longer draws the Loading arm, so the \
         rung-list arm scans the wrong file"
    );
}
