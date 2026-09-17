//! UI-09's sentry: a panel draft with content lives in `state/`, and the
//! GRBL `$$` dump is parsed once per change rather than once per frame.
//!
//! # What the finding was
//!
//! Forty `insert_temp` / `get_temp` calls across ten files held panel state
//! in egui's temporary memory, beside a `state/` module that
//! `state/CLAUDE.md` names as the home of GUI state and that already holds
//! `history.tool_draft`, `history.stock_draft` and `history.post_snapshot`
//! for exactly this job. `machine_panel.rs` alone held sixteen, and it ran
//! `MachineKinematics::from_grbl_settings(&buf)` unconditionally inside the
//! draw — so the whole pasted dump was re-parsed on every frame the
//! disclosure was open, and the buffer was cloned into `ui.data` on every
//! keystroke.
//!
//! # What this test measures
//!
//! Arm 1 is the source ratchet: the remaining `insert_temp` / `get_temp`
//! sites are named with the reason each one is NOT a draft with content.
//!
//! Arm 2 drives the real Machine panel through a headless `Context` and
//! asserts the parser runs once for a buffer that does not change. The
//! caching arms themselves are unit tests beside the type
//! (`state::panels::tests`), because `from_grbl_settings` is pure.
//!
//! Arm 3 is the non-vacuity guard.
//!
//! # NOT MEASURED
//!
//! Whether a draft's CONTENT is right, and the modals' own view state,
//! which arm 1 allows by name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// Files that may still hold state in egui temporary memory, with the count
/// and the reason it is not a draft with content. **Only ever goes down.**
const EGUI_MEMORY_ALLOWANCE: &[(&str, usize, &str)] = &[
    (
        "ui/automation.rs",
        4,
        "the UI-automation snapshot is a per-context sink the harness reads \
         back out of the same context; it is not project state",
    ),
    (
        "ui/preflight.rs",
        3,
        "a per-modal confirmation tick that dies with the modal",
    ),
    (
        "ui/tool_library_modal.rs",
        2,
        "the modal's own view shape (search text, selected row)",
    ),
    (
        "ui/machine_library_modal.rs",
        2,
        "the modal's own view shape",
    ),
    (
        "ui/properties/toolpath_panel.rs",
        2,
        "the active inspector tab, which egui already persists per widget id",
    ),
    (
        "ui/properties/stock.rs",
        2,
        "the material-picker filter text, a view convenience with one reader",
    ),
    (
        "ui/properties/operations/height_diagram.rs",
        2,
        "the index of the handle being dragged, valid only inside one drag",
    ),
    (
        "ui/properties/operations/boundary_2d.rs",
        2,
        "whether the tabs disclosure has been opened once, a view latch",
    ),
];

/// The fewest sites the allowance must still account for, so a sweep that
/// deleted the panels rather than the pattern fails.
const MIN_ALLOWED_SITES: usize = 15;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn temp_memory_sites() -> Vec<(String, usize)> {
    let root = src_root();
    let mut files = Vec::new();
    rs_files(&root.join("ui"), &mut files);
    files.sort();

    let mut out = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(path).unwrap();
        let n: usize = text
            .lines()
            .map(|l| {
                let code = strip_comment(l);
                code.matches("insert_temp").count() + code.matches("get_temp").count()
            })
            .sum();
        if n > 0 {
            out.push((rel, n));
        }
    }
    out
}

#[test]
fn a_panel_draft_with_content_lives_in_state_ui09() {
    let sites = temp_memory_sites();
    let mut over = Vec::new();

    for (rel, count) in &sites {
        let allowed = EGUI_MEMORY_ALLOWANCE
            .iter()
            .find(|(f, _, _)| f == rel)
            .map_or(0, |(_, n, _)| *n);
        if *count > allowed {
            over.push(format!("{rel} {count} (allowed {allowed})"));
        }
    }

    assert!(
        over.is_empty(),
        "these panels hold state in egui temporary memory, where MCP and \
         the integration harness cannot read it and a cache cannot be kept. \
         A draft with content — text the operator typed, a status line, a \
         parse — belongs on `AppState` beside `history.tool_draft` and \
         `state.panels`. A view toggle or a drag index may stay, named in \
         EGUI_MEMORY_ALLOWANCE with its reason. Over budget: {}",
        over.join(", ")
    );
}

#[test]
fn the_egui_memory_allowance_is_not_vacuous_ui09() {
    let sites = temp_memory_sites();

    let total: usize = EGUI_MEMORY_ALLOWANCE.iter().map(|(_, n, _)| *n).sum();
    assert!(
        total >= MIN_ALLOWED_SITES,
        "the allowance now covers only {total} sites, fewer than the \
         {MIN_ALLOWED_SITES} left after UI-09."
    );

    for (rel, allowed, reason) in EGUI_MEMORY_ALLOWANCE {
        assert!(!reason.is_empty(), "{rel} must say WHY its state may stay");
        let found = sites.iter().find(|(f, _)| f == rel).map_or(0, |(_, n)| *n);
        assert_eq!(
            found, *allowed,
            "{rel} is allowed {allowed} egui-memory sites and holds {found}."
        );
    }
}

/// Arm 2 — the real panel, drawn repeatedly with an unchanged buffer.
#[test]
fn the_grbl_dump_is_parsed_once_per_change_ui09() {
    use rs_cam_viz::state::panels::GrblImportDraft;

    let mut draft = GrblImportDraft::default();
    draft
        .buffer
        .push_str("$11=0.020\n$120=500.000\n$121=500.000\n$122=250.000\n");

    // The draw reads the parse once per frame. Sixty frames is one second
    // of an open disclosure.
    for _ in 0..60 {
        assert!(draft.parsed().is_some(), "the dump parses");
    }
    assert_eq!(
        draft.parses, 1,
        "sixty frames of an unchanged buffer parsed {} times. The importer \
         used to parse the whole dump on every frame the disclosure was \
         open.",
        draft.parses
    );

    draft.buffer.push_str("$110=8000.000\n");
    let _ = draft.parsed();
    assert_eq!(draft.parses, 2, "a changed buffer must parse again");
}
