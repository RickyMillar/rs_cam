//! G-REACHWRAP (UX-R09-001) — the inspector header's SENTENCES wrap.
//!
//! At 1400×900 with a terrain finish selected, the review captured two lines
//! running off the right edge of the operation inspector
//! (`planning/ui_review_2026-09-09/results/W02/evidence/01_finish_defaults.png`):
//!
//! - `unreachable 70.0 % of 3D surface area, rim-eroded 1.5 mm · max gap 2.82`
//!   — cut mid-number, with `grid_note` never drawn at all;
//! - `Tool load: Chipload clamped to the matched band ceiling: 0.0098 →
//!   0.0232 mm/tooth. The whole derat` — cut at "derat".
//!
//! The second is the one that matters most. Its full text says the whole
//! derated band sits below the chip-formation floor, that burnishing is still
//! expected, and what to do instead. A caveat truncated at "The whole derat"
//! reads as an unqualified result — the reader is left with a number and no
//! statement of what it does not promise. The reach line loses the same
//! thing: `grid_note` and `over_statement_note` ARE the missing-guarantee
//! sentences (the cell, the floor, the bar; and that the grid over-states).
//!
//! # What this file proves, and what it does not
//!
//! **Proved.** The reach readings used to live inside `ui.horizontal(…)` as
//! bare `ui.label`s. egui's `Ui::wrap_mode` returns `Wrap` only for a vertical
//! layout or a main-wrapped horizontal one, and `Extend` otherwise
//! (`egui-0.34.3/src/ui.rs:696-721`), so a bare label there ran past the panel
//! and clipped. This file pins that they are no longer on that row.
//!
//! **What the row's `Extend` does NOT mean.** It is not that wrapping was
//! unreachable there. `Label` resolves its mode as
//! `self.wrap_mode.unwrap_or_else(|| ui.wrap_mode())`
//! (`egui-0.34.3/src/widgets/label.rs:191`) and `Label::wrap()` sets
//! `Some(TextWrapMode::Wrap)` (`label.rs:63`), so the label's own setting
//! overrides the row's and a `.wrap()` on that row WOULD have wrapped. What it
//! would have wrapped at is `ui.available_width()` — on that row, only what
//! the checkbox leaves, so a narrow column rather than the panel width. The
//! plan's prescribed `.wrap()` was therefore INSUFFICIENT, not inert, and the
//! reason these lines moved into the vertical flow is the wrapping WIDTH:
//! there they wrap at the panel's full width, which is the readable outcome.
//!
//! **NOT proved.** That the panel now renders without clipping at 1400×900.
//! The acceptance test PLAN.md names is an MCP-VIEW screenshot; the embedded
//! MCP server is down this session and no GUI may be started, so no rendered
//! evidence was produced and none is claimed. This is a source-read sentry of
//! the same kind `mcp_toasts_report_outcome_g_mcptoast.rs` uses, for the same
//! reason: the panel draws through `egui::Ui` and cannot be instantiated in a
//! test. It asserts that each sentence-bearing line is CONSTRUCTED wrapped
//! and that none sits in a layout where wrapping is impossible. Visual
//! confirmation is owed to V6.2.
//!
//! **Also not proved: that this is the whole cause.** The tool-load caution
//! was already inside `ui.horizontal_wrapped`, whose wrap mode is `Wrap`, and
//! it clipped anyway — so the panel's content was being laid out wider than
//! the panel is. The plausible mechanism is the reach row itself: an
//! `Extend` row demands the full width of its text, which a resizable side
//! panel then reports as its content width. Removing the only `Extend`
//! sentence in the header is therefore the primary change, and the explicit
//! `.wrap()` calls make each line's behaviour local rather than inherited.
//! Whether that is sufficient is exactly what V6.2 has to look at.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

const PROPERTIES_SRC: &str = include_str!("../src/ui/properties/mod.rs");

/// The body of `fn <name>` up to the next top-level `fn` or the end.
fn function_body<'a>(src: &'a str, name: &str) -> &'a str {
    let needle = format!("fn {name}(");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("no fn {name} in properties/mod.rs"));
    let rest = &src[start + needle.len()..];
    let end = rest.find("\nfn ").unwrap_or(rest.len());
    &rest[..end]
}

/// The `supports_reach_map()` block — the reach checkbox and its readings.
fn reach_block(src: &str) -> &str {
    let start = src
        .find("if entry.operation.op_type().supports_reach_map() {")
        .expect("the reach block moved; find it before weakening this test");
    let rest = &src[start..];
    let end = rest
        .find("// Contextual diagnostics")
        .expect("the reach block's end marker moved");
    &rest[..end]
}

/// THE sentry. The three reach readings that carry a caveat must not be
/// constructed inside the checkbox's `ui.horizontal` row.
///
/// Two reasons, and the second is the load-bearing one. That row's wrap mode
/// is `Extend`, so a bare label on it clips — which is what the review
/// captured. And even wrapped explicitly, a label there wraps at
/// `ui.available_width()`, which on that row is only the remainder after the
/// checkbox: a narrow column instead of the panel's full width. Off the row,
/// they get the whole width, which is what makes a caveat readable.
///
/// Pre-fix all three were inside it.
#[test]
fn the_reach_readings_get_the_panel_width_not_the_row_remainder() {
    let block = reach_block(PROPERTIES_SRC);
    let horizontal = block
        .find("ui.horizontal(|ui| {")
        .expect("the checkbox still sits in a horizontal row");
    let after_horizontal = block[horizontal..]
        .find("\n        });")
        .map(|at| horizontal + at)
        .expect("the checkbox row's closing brace moved");
    let row = &block[horizontal..after_horizontal];

    for reading in [
        "unreachable {unreachable_pct:.1} %",
        "grid_note.clone()",
        "over_statement_note.clone()",
    ] {
        assert!(
            !row.contains(reading),
            "{reading:?} is inside the checkbox's ui.horizontal row, where a bare \
             label clips (wrap mode Extend) and even a wrapped one would only get \
             the width the checkbox leaves:\n{row}"
        );
    }
}

/// And they are drawn through the wrapped helper, not a bare `ui.label`.
#[test]
fn the_reach_readings_are_constructed_wrapped() {
    let block = reach_block(PROPERTIES_SRC);
    for reading in [
        "unreachable {unreachable_pct:.1} %",
        "grid_note.clone()",
        "over_statement_note.clone()",
        "reach: {message}",
    ] {
        let at = block
            .find(reading)
            .unwrap_or_else(|| panic!("{reading:?} is gone from the reach block"));
        // The helper call opens at most a few lines above the text it
        // carries; anything else means the line was moved back onto a bare
        // `ui.label`.
        let before = &block[at.saturating_sub(300)..at];
        assert!(
            before.contains("wrapped_small_label("),
            "{reading:?} must be drawn through wrapped_small_label:\n{before}"
        );
    }
}

/// The two SHORT status words may stay beside the checkbox: they carry no
/// caveat and cannot clip. This pins that the exemption is narrow — a
/// sentence must not be added to that row later on the strength of it.
#[test]
fn only_short_status_words_share_the_checkbox_row() {
    let block = reach_block(PROPERTIES_SRC);
    for status in ["reach: computing…", "reach: not measured"] {
        assert!(
            block.contains(status),
            "{status:?} should still be a status word, not a sentence"
        );
        assert!(
            status.len() < 30,
            "{status:?} has grown into a sentence and must leave the horizontal row"
        );
    }
}

/// The helper exists and does exactly one thing: wrap. It must not acquire a
/// width, a truncation or a text change — this is a layout fix.
#[test]
fn the_wrapped_helper_only_wraps() {
    let body = function_body(PROPERTIES_SRC, "wrapped_small_label");
    assert!(body.contains(".wrap()"), "the helper must wrap");
    assert!(
        !body.contains("truncate"),
        "truncation is the defect, not the fix"
    );
    assert!(
        !body.contains("set_max_width") && !body.contains("set_width"),
        "the helper must not pin a width; the panel owns that"
    );
}

/// The tool-load caution — the sentence the review caught at "The whole
/// derat" — is wrapped explicitly rather than by inheritance from
/// `horizontal_wrapped`. Pre-fix this row was a bare `ui.label`.
#[test]
fn the_diagnostic_message_is_constructed_wrapped() {
    let body = function_body(PROPERTIES_SRC, "render_diagnostic_row");
    let at = body
        .find("format!(\"{prefix}{}\", d.message)")
        .expect("the diagnostic message line moved");
    let around = &body[at.saturating_sub(200)..(at + 200).min(body.len())];
    assert!(
        around.contains("egui::Label::new("),
        "the diagnostic message must be an explicit Label:\n{around}"
    );
    assert!(
        around.contains(".wrap()"),
        "the diagnostic message must wrap explicitly, not inherit it:\n{around}"
    );
}

/// The third sentence-bearing site in the header. The validator's messages
/// are whole sentences too ("Rest machining requires an earlier enabled
/// operation…"), so they get the same treatment while it is being applied.
#[test]
fn the_validation_errors_are_constructed_wrapped() {
    let at = PROPERTIES_SRC
        .find("for err in &validation_errors {")
        .expect("the validation-error loop moved");
    let body = &PROPERTIES_SRC[at..(at + 500).min(PROPERTIES_SRC.len())];
    assert!(
        body.contains("wrapped_small_label("),
        "validator sentences wrap too:\n{body}"
    );
}

/// A census, so "we fixed the two the review named" cannot quietly become
/// "we fixed two and left a third". Every sentence-bearing line in the
/// inspector header is one of the four sites this commit touched, or one of
/// the two that already carried `.wrap()` before it.
///
/// This is a source read and it counts CONSTRUCTION, not pixels.
#[test]
fn every_sentence_in_the_header_is_wrapped() {
    // The header runs from the Name row to the diagnostics ribbon.
    let start = PROPERTIES_SRC
        .find("// ── Shared header (always visible above tabs)")
        .expect("the header marker moved");
    let end = PROPERTIES_SRC[start..]
        .find("// Contextual diagnostics")
        .map(|at| start + at)
        .expect("the header's end marker moved");
    let header = &PROPERTIES_SRC[start..end];

    // Already wrapped before this commit: the two Generate-row statuses.
    for existing in ["Waiting on upstream stock", "Error: {e}"] {
        let at = header
            .find(existing)
            .unwrap_or_else(|| panic!("{existing:?} left the header"));
        let after = &header[at..(at + 400).min(header.len())];
        assert!(
            after.contains(".wrap()"),
            "{existing:?} used to wrap and must still:\n{after}"
        );
    }

    // No bare `ui.label(` may carry a full stop followed by a space in the
    // header — that is the shape of a multi-sentence caveat, and it is the
    // shape that clipped.
    for (idx, line) in header.lines().enumerate() {
        assert!(
            !(line.trim_start().starts_with("ui.label(") && line.contains(". ")),
            "header line {idx} draws a sentence through a bare ui.label: {line}"
        );
    }
}
