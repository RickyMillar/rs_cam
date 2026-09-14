//! DC7's sentry: the load warnings never cover the workspace tab bar.
//!
//! # The defect this exists to catch — F-4
//!
//! `app.rs` opened the project load warnings in a NON-MODAL
//! `egui::Window::new("Project Load Warnings")`. A non-modal window is a thing
//! the operator can LEAVE OPEN, and this one landed over the workspace tab
//! bar, covering Setup, Toolpaths and Simulation. Navigation stopped while it
//! was open. Ruling R31 makes it a modal, because a modal is a thing you open,
//! read and close.
//!
//! # Why this invariant and not a screenshot diff
//!
//! A screenshot pair answers "does it overlap TODAY, at THIS window size, with
//! THIS warning count". The defect is not a pixel. It is the CONTAINER KIND: a
//! non-modal floating window over chrome that the operator must reach. A
//! window re-introduced at a different anchor, or at a different size, or with
//! a longer warning list, is the same defect again and a pixel diff taken at
//! one size can easily miss it. So this sentry asserts the kind.
//!
//! The second half asserts the BOUND. The warnings list is unbounded — a
//! project can carry 300 — and a modal that grows with its content covers the
//! tab bar just as surely as a window does. `NoticeStack` is the bounded
//! renderer, and the arm below drives its rules directly, because `resolve` is
//! a pure function and a `Ui` proves nothing extra here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::ui::components::{Notice, NoticeStack, Role};
use rs_cam_viz::ui::status_bar;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read(rel: &str) -> String {
    let path = src_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The lines of `app.rs` between the modal's id and the toast stack's id.
///
/// `app.rs` holds two `NoticeStack` consumers — this modal and the toast
/// stack — so a scan of the whole file cannot tell them apart. The two ids
/// are the markers, and arm 4 asserts both still exist.
fn load_warnings_modal_block() -> String {
    let src = read("app.rs");
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains("load_warnings_modal"))
        .expect("the modal's id names the block this sentry reads");
    let tail = lines.iter().skip(start);
    let len = tail
        .clone()
        .position(|l| l.contains("toast_notifications"))
        .unwrap_or(lines.len() - start);
    tail.take(len).copied().collect::<Vec<&str>>().join("\n")
}

/// Arm 1 — the window is gone.
#[test]
fn app_opens_no_load_warnings_window_dc7() {
    let src = read("app.rs");

    assert!(
        !src.contains("Window::new(\"Project Load Warnings\")"),
        "F-4 is back: `app.rs` opens a floating window for the load \
         warnings. A non-modal window can be left over the workspace tab \
         bar, which blocks Setup, Toolpaths and Simulation. Ruling R31: the \
         warnings surface is a modal."
    );
    assert!(
        src.contains("egui::Modal::new("),
        "the warnings must open in an `egui::Modal`, which the operator \
         cannot leave open over the chrome"
    );
    assert!(
        src.contains("load_warnings_modal"),
        "that modal is the warnings one, by its id"
    );
}

/// Arm 2 — the surface consumes the bounded renderer, and the status bar
/// carries the count that opens it.
#[test]
fn the_warnings_surface_consumes_the_notice_stack_dc7() {
    // Read the MODAL's own block, not the whole file: the toast stack is a
    // second `NoticeStack` consumer in `app.rs`, and a scan of the file would
    // pass on that one alone.
    let modal_block = load_warnings_modal_block();
    assert!(
        modal_block.contains("NoticeStack::new"),
        "the modal must render through `NoticeStack`. A raw loop over the \
         warning list draws one row per warning, so a project with 300 of \
         them fills the screen — which is the defect §4.11 names this \
         component's sixth consumer for. The block read was:\n{modal_block}"
    );

    let bar = read("ui/status_bar.rs");
    assert!(
        bar.contains("status_load_warnings"),
        "the status bar must carry the warnings count. It is the only route \
         back to the warnings once the modal is closed; the old window could \
         be dismissed once and never reopened."
    );
    assert!(
        bar.contains("tokens::CAUTION"),
        "the count is quiet and takes CAUTION"
    );
    // Rule C: the bar states a count, not a sentence about the warnings.
    assert!(
        !bar.contains("some references need attention"),
        "the bar states a count, never the window's explanatory sentence"
    );
    assert_eq!(status_bar::warnings_label(11), "11 warnings");
    assert_eq!(status_bar::warnings_label(1), "1 warning");
}

/// Arm 3 — the four rules still hold on a realistic load-warning population.
///
/// The population is the shape a broken project really produces: many copies
/// of one missing-model line, a spread of distinct ones, and a handful of
/// hard failures that must never be hidden.
#[test]
fn the_bounded_stack_still_obeys_its_four_rules_dc7() {
    let mut notices: Vec<Notice> = Vec::new();

    // Six models missing the same shared file. Identical text, so rule 3
    // must collapse them to one row BEFORE the cap.
    for _ in 0..6 {
        notices.push(Notice::new(
            Role::Caution,
            "Model 'Panel' could not be loaded because 'panel.stl' was not found.",
        ));
    }
    // Twenty distinct stale-result lines.
    for i in 0..20 {
        notices.push(Notice::new(
            Role::Caution,
            format!("Toolpath {i} was marked stale on load."),
        ));
    }
    // Three hard failures. Rule 1 must render every one of them.
    for i in 0..3 {
        notices.push(Notice::new(
            Role::Danger,
            format!("Tool {i} names a holder that is not in the library."),
        ));
    }
    let total = notices.len();
    assert_eq!(total, 29, "the fixture population");

    let resolved = NoticeStack::new(notices).cap(4).resolve();

    // Rule 1 — severity outranks the cap. This is the safety rule.
    let dangers = resolved
        .rows
        .iter()
        .filter(|r| r.notice.role == Role::Danger)
        .count();
    assert_eq!(
        dangers, 3,
        "every DANGER must render past the visible cap. A warning that \
         blocks a cut must never be pushed out by twenty stale-result lines."
    );

    // Rule 4 — severity, then arrival. The dangers lead.
    for row in resolved.rows.iter().take(dangers) {
        assert_eq!(row.notice.role, Role::Danger);
    }

    // Rule 3 — identical notices collapse, and they collapse BEFORE the cap.
    let collapsed = resolved
        .rows
        .iter()
        .find(|r| r.notice.text.contains("panel.stl"))
        .expect("the repeated missing-model line reaches the visible rows");
    assert_eq!(
        collapsed.count, 6,
        "six identical warnings cost one slot and carry a ×6 multiplier"
    );

    // Rule 2 — the overflow row states the TRUE total, never a remainder.
    assert!(resolved.truncated, "29 notices do not fit a cap of 4");
    let overflow = resolved
        .overflow_text()
        .expect("a truncated stack states what it withheld");
    assert!(
        overflow.contains("of 29"),
        "the overflow row states the true total, got {overflow:?}"
    );
    assert!(
        !overflow.contains("25"),
        "a remainder makes the reader do arithmetic to find the scale of \
         what is hidden, got {overflow:?}"
    );
    assert_eq!(resolved.total, total);
}

/// Arm 4 — non-vacuity.
///
/// Every scan above greps a real file for a real string. A renamed file or a
/// renamed symbol must fail HERE, loudly, rather than quietly making a scan
/// unsatisfiable and therefore always green.
#[test]
fn the_sentry_reads_real_files_and_real_symbols_dc7() {
    for rel in ["app.rs", "ui/status_bar.rs"] {
        let path = src_root().join(rel);
        assert!(
            path.is_file(),
            "{rel} no longer exists; this sentry is stale"
        );
        assert!(
            !read(rel).is_empty(),
            "{rel} is empty; the source scans above would pass vacuously"
        );
    }

    let app = read("app.rs");

    // The scans assert on a window name that must still be spellable, so a
    // reader can see what the arms are looking for.
    assert!(
        app.contains("Project Load Warnings"),
        "the deleted window's name must survive in the comment that records \
         why it went, or arm 1 tests a string nothing would ever produce"
    );

    // The block reader must isolate the MODAL, not the toast stack beside
    // it, or arm 2 passes on the wrong consumer.
    assert!(
        app.contains("toast_notifications"),
        "the block reader stops at the toast stack's id. Without that id it \
         reads to the end of the file and arm 2 stops distinguishing the two \
         `NoticeStack` consumers."
    );
    let block = load_warnings_modal_block();
    assert!(!block.is_empty(), "the modal block reader returned nothing");
    assert!(
        !block.contains("toast_notifications"),
        "the block reader ran past the modal and into the toast stack"
    );
    assert!(
        block.len() < app.len(),
        "the block must be a slice of the file, not the whole file"
    );

    // `NoticeStack` must still apply a cap at all, or arm 3 is vacuous.
    let notices = (0..10)
        .map(|i| Notice::new(Role::Caution, format!("w{i}")))
        .collect::<Vec<_>>();
    let resolved = NoticeStack::new(notices).cap(4).resolve();
    assert_eq!(resolved.rows.len(), 4);
    assert!(resolved.truncated);
}
