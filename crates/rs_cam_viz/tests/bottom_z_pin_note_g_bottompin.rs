//! G-BOTTOMPIN, GUI half (F1.19): the Heights tab annotates the Bottom Z row
//! on every operation that ignores a pinned Bottom Z.
//!
//! The core half measured the fact (`planning/ui_fix_2026-09-09/reports/
//! J7.md`): a pinned Bottom Z reaches emitted motion on THREE of the
//! twenty-four operations — `Adaptive3d`, `UnifiedFinish`, `Waterline`. On
//! the other twenty-one the Heights tab offered a dial that moves no motion,
//! and its tooltip said "pinning this row overrides that". The operator read
//! two untrue sentences beside one field.
//!
//! The GUI half annotates rather than disables. [`bottom_z_pin_note`] is the
//! one decision, and this file is its sentry.
//!
//! # What each arm proves, and what it does not
//!
//! Arm 1 walks `OperationType::ALL` and ties the panel's answer to the core
//! declaration for all twenty-four. It is NOT a tautology: the panel carries
//! its own exhaustive `match`, because each family names a different dial as
//! the real floor. The arm fails when the two enumerations drift apart, which
//! is the failure a hand-written list of three names would hide.
//!
//! Arm 4 is a SOURCE-LEVEL READ of the panel module, and it is the weaker
//! instrument of the four. It proves the draw function mentions the decision
//! function; it does not prove any pixel. No test in this crate renders the
//! properties panel, the MCP server was down when this was written, and the
//! GUI must not be started, so a rendered check was out of reach. Read arm 4
//! as a wiring guard, never as evidence that the operator saw the note.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_viz::ui::properties::bottom_z_pin_note;

/// The panel module the Heights tab draws from.
fn panel_source() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("src/ui/properties/operations/mod.rs");
    assert!(path.is_file(), "panel source {} is gone", path.display());
    path
}

// ── arm 1: the panel answers exactly as the core declaration ────────────

#[test]
fn every_operation_annotates_the_bottom_row_unless_it_honours_the_pin() {
    let mut annotated: Vec<&'static str> = Vec::new();
    let mut live: Vec<&'static str> = Vec::new();
    let mut walked = 0usize;

    for &op in OperationType::ALL {
        walked += 1;
        let note = bottom_z_pin_note(op);
        let honours = op.honors_pinned_bottom_z();
        assert_eq!(
            note.is_some(),
            !honours,
            "{op:?}: the Heights tab annotates the Bottom row iff the \
             operation ignores the pin (honors_pinned_bottom_z = {honours})"
        );
        if note.is_some() {
            annotated.push(op.label());
        } else {
            live.push(op.label());
        }
    }

    eprintln!("G-BOTTOMPIN GUI: annotated = {annotated:?}");
    eprintln!("G-BOTTOMPIN GUI: no note (the pin drives the cut) = {live:?}");

    // Non-vacuity floor. An empty walk, or a walk that lands on one side
    // only, would pass every assertion above.
    assert_eq!(
        walked,
        OperationType::ALL.len(),
        "the walk must cover every declared operation"
    );
    assert!(walked >= 24, "the catalog shrank below 24 operations");
    assert!(!annotated.is_empty(), "no operation carries a note");
    assert!(!live.is_empty(), "every operation carries a note");
}

// ── arm 2: the note names the dial that DOES set the floor ──────────────

#[test]
fn the_note_names_the_dial_that_sets_the_floor() {
    let mut checked = 0usize;
    for &op in OperationType::ALL {
        let Some(note) = bottom_z_pin_note(op) else {
            continue;
        };
        checked += 1;
        assert!(!note.trim().is_empty(), "{op:?}: empty note");
        assert!(
            note.ends_with('.'),
            "{op:?}: the note is a sentence, it ends with a full stop: {note}"
        );
        assert!(
            note.contains("floor"),
            "{op:?}: the note must say what DOES set the floor: {note}"
        );
        assert!(
            note.len() <= 120,
            "{op:?}: the note is drawn on one panel line, keep it short: {note}"
        );
    }
    assert!(checked > 0, "no note was checked");
    eprintln!("G-BOTTOMPIN GUI: {checked} notes checked");
}

// ── arm 3: the three that honour the pin are named ──────────────────────

#[test]
fn only_the_three_pin_honouring_operations_keep_a_plain_bottom_row() {
    let mut live: Vec<&'static str> = OperationType::ALL
        .iter()
        .filter(|&&op| bottom_z_pin_note(op).is_none())
        .map(|&op| op.label())
        .collect();
    live.sort_unstable();
    let mut expected = vec![
        OperationType::Adaptive3d.label(),
        OperationType::UnifiedFinish.label(),
        OperationType::Waterline.label(),
    ];
    expected.sort_unstable();
    assert_eq!(
        live, expected,
        "a fourth operation reading a pinned Bottom Z needs a generator site \
         behind it (J7.md), not a silent panel change"
    );
}

// ── arm 4: the Heights tab consults the decision (SOURCE-LEVEL) ─────────

#[test]
fn the_heights_tab_consults_the_note_weaker_source_level_arm() {
    let path = panel_source();
    let src = std::fs::read_to_string(path).expect("panel source reads");
    let start = src
        .find("pub(super) fn draw_heights_params")
        .expect("draw_heights_params is the Heights tab entry point");
    let body = &src[start..];
    let end = body
        .find("\n// ── Stepover Pattern Diagram")
        .expect("the Heights panel section ends at the stepover diagram");
    let body = &body[..end];
    assert!(
        body.contains("bottom_z_pin_note"),
        "draw_heights_params must read bottom_z_pin_note; without it the \
         Bottom row is drawn with no annotation on the twenty-one \
         operations that ignore the pin"
    );
}
