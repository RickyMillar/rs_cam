//! DC5a's sentry: one container, one scope — in the STATE, not just the draw.
//!
//! # The defect
//!
//! `ui/feeds_modal.rs` was 3 450 lines, and its draw functions divided into
//! four jobs sharing nothing but a window, at **two scopes**.
//! `FeedsModalMode` was `{ Toolpath, Project }`, so a project-wide view of
//! every toolpath was reachable only by selecting one toolpath, opening its
//! modal, and switching mode.
//!
//! **The mode flip was the symptom. The cause was one layer down.** Four
//! project-scope fields — the sort order, the selection set, the scatter
//! toggle and the seed that filled the selection — lived on
//! `FeedsModalState`, the PER-OPERATION record. So the rollup's controls
//! only functioned while a per-operation modal was open:
//!
//! - `SetFeedsProjectSort` wrote `modal.project_sort`;
//! - `ToggleFeedsProjectRow` wrote `modal.project_selected`;
//! - `SetFeedsProjectScatter` wrote `modal.project_show_scatter`;
//! - `apply_feeds_project_selected` READ `modal.project_selected`, so
//!   `Apply selected` applied to nothing once the modal closed. That is the
//!   worst of the four: a control that accepts a click and writes nothing,
//!   while the operator believes the recipe was applied.
//!
//! Moving only the drawing would have left every one of those intact. That
//! is why arm 2 below is about STATE and not about which file a function
//! sits in.
//!
//! # Why these invariants and not a screenshot or a line count
//!
//! A line-count bar passes the moment a function lands in the wrong file — the
//! exact regression that grew the 3 450-line file. A screenshot cannot see a
//! field's owner at all, and the owner is the defect. Each arm below names a
//! fact that is cheap to check and expensive to get wrong.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::state::{AppState, ProjectFeedsSort, ProjectFeedsState};

/// The per-operation feeds jobs. Every one of them answers about ONE
/// operation; none may answer about the project.
const OPERATION_SCOPE_FILES: [&str; 6] = ["mod", "window", "shared", "compare", "why", "explore"];

/// The commands and fields whose scope is THE WHOLE PROJECT.
const PROJECT_SCOPE_NAMES: [&str; 5] = [
    "ProjectFeedsSort",
    "SetFeedsProjectSort",
    "ToggleFeedsProjectRow",
    "SetFeedsProjectScatter",
    "SetFeedsProjectSelectAll",
];

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(relative: &str) -> String {
    let path = crate_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn feeds_file(stem: &str) -> String {
    read(&format!("src/ui/feeds/{stem}.rs"))
}

/// The text of ONE dispatch arm: from its `UiCommand::<Name>(` to the next
/// `UiCommand::` at any indent.
///
/// A fixed-size window would bleed into the following arm. `SetFeedsExplore`
/// sits directly after `SetFeedsProjectSort` and legitimately reaches into
/// the per-operation modal, so a coarse window reports the wrong arm.
fn dispatch_arm<'a>(dispatch: &'a str, name: &str) -> &'a str {
    let at = dispatch
        .find(&format!("UiCommand::{name}("))
        .unwrap_or_else(|| panic!("{name} has no dispatch arm"));
    let rest = &dispatch[at..];
    let next = rest[1..]
        .find("UiCommand::")
        .map_or(rest.len(), |offset| offset + 1);
    &rest[..next]
}

/// Strip every `//` comment. Each header below discusses the defect by name,
/// so a scan that reads a comment reports code that is not there.
///
/// # This does NOT model string literals, and the direction matters
///
/// A `//` inside a string literal ends the line early here, so real code
/// after it is dropped. This stripper therefore **under-counts code**.
///
/// Which way that errs depends on the ARM, and the two directions are not
/// equally safe:
///
/// - On a `contains(…)` arm, an under-count is a false FAIL. Somebody
///   investigates it.
/// - On a `!contains(…)` arm, an under-count is a false PASS. **Nobody
///   investigates a green test.**
///
/// Six arms in this file and its DC3 sibling are negative, so the pairing to
/// watch in this repo is *negative assertion plus under-counting stripper*.
/// Measured 2026-09-14: across all ten files these two sentries scan, there
/// is no line where a `//` sits inside a string literal, so both are sound
/// on today's sources. That is a property of the sources, not of the
/// scanner.
///
/// # Do not copy UP1's reasoning onto a `!contains` arm
///
/// `tests/panels_read_the_token_module_up1.rs` documents the same blind spot
/// and concludes that "an under-count cannot make a failing budget pass".
/// **That is correct there and false here.** Its arm is a BUDGET — a `<=`
/// comparison — and hiding a call site can only lower the count toward the
/// bar. A `!contains` arm is the opposite: hiding a call site is exactly
/// what makes it pass. The sentence is true of its own arm and licenses the
/// bug if copied, which is the hardest kind of trap to see — one laid by a
/// comment that is right.
///
/// The mirror-image failure is worth knowing too, because it is how this was
/// found. A scan that strips comments but not string literals and asks
/// "is this import used?" **over-counts usage**: `.radio(…, "Match chart")`
/// satisfied `\bchart\b`, so a genuinely unused import read as used and four
/// dead imports survived an audit that the compiler then caught.
///
/// The fix for all of this is one string-literal-aware stripper shared by
/// every source scanner in the crate. It is recorded as a follow-up in
/// `planning/ui_declutter_2026-09-14/PLAN.md` and deliberately not done in
/// this phase, which is judged by what it deletes.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `.rs` file directly under `src/ui/feeds`, by stem.
fn feeds_files() -> Vec<String> {
    let dir = crate_root().join("src/ui/feeds");
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir: {e}"));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            out.push(path.file_stem().unwrap().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

// ── arm 1 — the mode is gone and the window holds one scope ──────────────

/// `FeedsModalMode` does not exist, and no feeds file switches a mode.
#[test]
fn the_feeds_window_has_no_mode_to_switch() {
    let state = strip_comments(&read("src/state/mod.rs"));
    assert!(
        !state.contains("enum FeedsModalMode"),
        "FeedsModalMode is back. It let ONE window hold two scopes: a \
         project-wide rollup reachable only by selecting one toolpath and \
         switching mode."
    );
    let registry = strip_comments(&read("src/ui_command.rs"));
    assert!(
        !registry.contains("SetFeedsModalMode"),
        "the command registry declares SetFeedsModalMode again"
    );

    for stem in OPERATION_SCOPE_FILES {
        let src = strip_comments(&feeds_file(stem));
        assert!(
            !src.contains("FeedsModalMode"),
            "ui/feeds/{stem}.rs names FeedsModalMode"
        );
    }
    // And the per-operation window no longer draws a rollup. D-3 moved the
    // window out of `mod.rs` into `window.rs`; both files are read, so the
    // rollup cannot come back through the module root either.
    for stem in ["mod", "window"] {
        let window = strip_comments(&feeds_file(stem));
        assert!(
            !window.contains("draw_project"),
            "ui/feeds/{stem}.rs draws a project-scope view again"
        );
    }
}

// ── UR4 — the modal has Explore scope only ─────────────────────────────

/// The modal is a focused nomogram, not a second inspector. Its body may
/// compose Explore helpers only; Compare and Why belong to the Feeds tab.
#[test]
fn the_feeds_modal_body_is_explore_only() {
    let modal = strip_comments(&feeds_file("window"));
    let body_at = modal
        .find(".show(ctx, |ui| {")
        .expect("feeds modal body moved");
    let body = &modal[body_at..];
    assert!(
        body.contains("explore::"),
        "the modal body must draw Explore"
    );
    assert!(
        !body.contains("compare::") && !body.contains("why::"),
        "the modal draws Compare or Why again; the Feeds tab is authoritative"
    );
}

// ── arm 2 — THE CAUSE: the state is split, not just the drawing ──────────

/// **The invariant that makes the split real.** Project-scope state lives on
/// `AppState::project_feeds`, never on the per-operation `FeedsModalState`.
///
/// This is driven against the real types, not the source: a field moved back
/// onto `FeedsModalState` would stop this file compiling, which is a stronger
/// signal than a text scan.
#[test]
fn the_project_scope_state_is_not_on_the_per_operation_modal() {
    // The project-scope record exists and carries the four fields.
    let rollup = ProjectFeedsState::default();
    assert!(!rollup.open, "the rollup starts closed");
    assert_eq!(
        rollup.sort,
        ProjectFeedsSort::Index,
        "project order default"
    );
    assert!(
        rollup.selected.is_empty(),
        "nothing is selected until seeded"
    );
    assert!(!rollup.show_scatter);

    // And a fresh AppState carries it beside the modal, not inside it.
    let state = AppState::new();
    assert!(
        state.feeds_modal.is_none(),
        "no feeds modal is open in a fresh project"
    );
    assert!(
        state.project_feeds.selected.is_empty(),
        "the rollup's selection is project-scope state and must exist \
         whether or not a per-operation modal is open"
    );

    // The source half: FeedsModalState must not carry a project field.
    let state_src = strip_comments(&read("src/state/mod.rs"));
    let at = state_src
        .find("pub struct FeedsModalState {")
        .expect("FeedsModalState is gone");
    let end = state_src[at..].find("\n}").expect("unterminated struct");
    let block = &state_src[at..at + end];
    for field in ["project_sort", "project_selected", "project_show_scatter"] {
        assert!(
            !block.contains(field),
            "FeedsModalState carries `{field}` again. A per-operation record \
             holding the project's state is the CAUSE of the defect DC5a \
             fixed: the rollup's controls then work only while a \
             per-operation modal is open."
        );
    }
}

/// Every writer and reader of the project selection reads the project-scope
/// record.
///
/// `apply_feeds_project_selected` is the one that WRITES a recipe, so it is
/// named explicitly: before DC5a it read the selection off the modal and
/// applied to nothing when the modal was shut.
#[test]
fn the_project_handlers_read_the_project_state() {
    let events = strip_comments(&read("src/controller/events/mod.rs"));
    for handler in [
        "SetFeedsProjectSort",
        "ToggleFeedsProjectRow",
        "SetFeedsProjectScatter",
        "SetFeedsProjectSelectAll",
    ] {
        let arm = dispatch_arm(&events, handler);
        assert!(
            arm.contains("project_feeds"),
            "{handler}'s arm does not write state.project_feeds. A \
             project-scope command writing to the per-operation modal only \
             functions while that modal is open."
        );
        assert!(
            !arm.contains("feeds_modal.as_mut()"),
            "{handler}'s arm still reaches into the per-operation modal"
        );
    }
    assert!(
        events.contains("self.state.project_feeds.selected.iter()"),
        "apply_feeds_project_selected must read the project-scope selection. \
         Reading it off the modal made `Apply selected` a control that \
         accepts a click and writes nothing."
    );
}

// ── arm 3 — the seed moved with the state ────────────────────────────────

/// The selection is SEEDED when the rollup opens.
///
/// Before DC5a the seed ran as a side effect of opening the per-operation
/// modal. If only the field had moved, the rollup would open with every row
/// unticked — an abstention drawn as "nothing to report" on a project-scope
/// surface, which is the failure mode this repo cares about most.
#[test]
fn the_rollup_seeds_its_selection_when_it_opens() {
    let events = strip_comments(&read("src/controller/events/mod.rs"));
    let arm = dispatch_arm(&events, "SetProjectFeedsOpen");
    assert!(
        arm.contains("project_feeds.selected") && arm.contains("enabled_toolpath_ids"),
        "opening the rollup must SEED its selection from every enabled \
         toolpath. Without the seed the rollup opens with every row \
         unticked, which reads as 'nothing to report'."
    );

    // The seed is one expansion, shared with Select all, so the two cannot
    // drift. Before DC5a the same expression was written out twice.
    assert!(
        events.contains("fn enabled_toolpath_ids("),
        "the seed and Select all must share one expansion"
    );

    // And the seed no longer rides the per-operation modal's door.
    let at = events
        .find("fn open_feeds_modal(")
        .expect("open_feeds_modal is gone");
    let body = &events[at..(at + 1200).min(events.len())];
    assert!(
        !body.contains("project_selected") && !body.contains("project_feeds"),
        "opening a PER-OPERATION modal still seeds project-scope state"
    );
}

// ── arm 4 — the rollup answers on the project-scope workspace ────────────

/// The rollup lives in Readiness, and no per-operation feeds file names a
/// project-scope command.
#[test]
fn only_the_readiness_panel_holds_the_project_rollup() {
    let readiness = strip_comments(&read("src/ui/readiness_panel.rs"));
    assert!(
        readiness.contains("fn draw_project_rollup"),
        "the rollup is not in the Readiness panel. It asks a project-wide \
         question, and Readiness is the workspace that answers those."
    );
    assert!(
        readiness.contains("UiCommand::SetProjectFeedsOpen"),
        "the Readiness check row must be the door that opens the rollup"
    );

    let mut offenders = Vec::new();
    for stem in OPERATION_SCOPE_FILES {
        let src = strip_comments(&feeds_file(stem));
        for name in PROJECT_SCOPE_NAMES {
            if src.contains(name) {
                offenders.push(format!("{stem}.rs names {name}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a per-operation feeds file answers a PROJECT-wide question: {}. \
         One container holds one scope.",
        offenders.join(", ")
    );
}

// ── arm 5 — the apply contract still covers the whole surface ────────────

/// The split must not open a hole in the apply contract.
///
/// `apply_contract_a3.rs` asserts that NO feeds surface pushes a per-field
/// apply — the affordance that wrote 4.445 mm of DOC where the funnel writes
/// 1.27 mm (Checkpoint I-1). That assertion is NEGATIVE, so a file it does
/// not name is a file that may reintroduce the affordance silently.
#[test]
fn the_apply_contract_scans_every_feeds_surface() {
    let contract = read("tests/apply_contract_a3.rs");
    for stem in feeds_files() {
        let needle = format!("src/ui/feeds/{stem}.rs");
        assert!(
            contract.contains(&needle),
            "apply_contract_a3.rs does not scan ui/feeds/{stem}.rs. Add the \
             include_str! to FEEDS_MODAL_SRCS."
        );
    }
    assert!(
        contract.contains("src/ui/readiness_panel.rs"),
        "the rollup moved to readiness_panel.rs and still writes through the \
         funnel, so the contract must follow it there"
    );
}

// ── arm 6 — non-vacuity ──────────────────────────────────────────────────

/// Every scan above can pass by accident. This closes each way.
#[test]
fn the_scans_are_not_vacuous() {
    // 1. The walk must find the job files, or arm 5 iterates nothing.
    let found = feeds_files();
    assert!(
        found.len() >= OPERATION_SCOPE_FILES.len(),
        "the walk found {} files under ui/feeds; it must find every job",
        found.len()
    );
    for stem in OPERATION_SCOPE_FILES {
        assert!(
            found.iter().any(|f| f == stem),
            "ui/feeds/{stem}.rs is missing; the split was undone"
        );
    }

    // 2. The comment stripper must strip. Unstripped, every negative scan
    //    above would read these headers' prose as code — and they name the
    //    deleted enum, the moved fields and the old handlers.
    let raw = feeds_file("mod");
    assert!(
        strip_comments(&raw).len() < raw.len(),
        "the comment stripper removed nothing, so the negative scans are \
         reading comments as code"
    );
    assert!(raw.contains("//!"), "non-vacuity: the header must exist");

    // 3. The names the scans hunt for must be live. A rename would make the
    //    negative arms unsatisfiable and silently green.
    let events = strip_comments(&read("src/controller/events/mod.rs"));
    let live = PROJECT_SCOPE_NAMES
        .iter()
        .filter(|n| events.contains(**n))
        .count();
    assert!(
        live >= 4,
        "only {live} of the project-scope commands appear in the dispatch. \
         Either they were deleted without this sentry, or they were renamed \
         and arm 4 now asserts nothing."
    );

    // 4. The Explore-only scan has a live, non-comment anchor.
    let modal = strip_comments(&feeds_file("window"));
    assert!(
        modal.contains("explore::draw_modal_body"),
        "Explore modal body moved or was renamed"
    );

    // 5. The old single file stays deleted.
    assert!(
        !crate_root().join("src/ui/feeds_modal.rs").is_file(),
        "src/ui/feeds_modal.rs is back — 3 450 lines, four jobs, two scopes"
    );
}
