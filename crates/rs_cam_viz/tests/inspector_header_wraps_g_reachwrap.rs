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

/// Every `.rs` file directly in `src/ui/properties/`, concatenated in
/// file-name order.
///
/// P4 (2026-09-17) split `properties/mod.rs` into `mod.rs` plus seven panel
/// children beside it, so the inspector's source is the folder, not one
/// file. `operations/` is a sub-folder and is not read; it carries its own
/// sentries.
fn properties_src() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no source under {}", dir.display());
    let mut out = String::new();
    for path in paths {
        out.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
        );
        out.push('\n');
    }
    out
}

/// The body of `fn <name>` up to the next top-level `fn` or the end.
///
/// # Why the end marker reads the visibility prefix
///
/// It used to be the bare string `"\nfn "`. P4 (2026-09-17) moved these
/// helpers into child modules of `ui/properties/` and gave several of their
/// neighbours `pub(super)`, which a bare `"\nfn "` walks straight past: the
/// slice then swallows the next two or three items. Every assertion below is
/// NEGATIVE, so a longer slice passes MORE easily — the silent direction.
/// The marker now reads each visibility a top-level `fn` can carry.
fn function_body<'a>(src: &'a str, name: &str) -> &'a str {
    let needle = format!("fn {name}(");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("no fn {name} under ui/properties/"));
    let rest = &src[start + needle.len()..];
    let end = ["\nfn ", "\npub fn ", "\npub(crate) fn ", "\npub(super) fn "]
        .iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .unwrap_or(rest.len());
    &rest[..end]
}

/// The `supports_reach_map()` block — the reach checkbox and its readings.
///
/// # Why the end marker is an indent and not a comment
///
/// This function used to slice from the `if` to the comment
/// `// Contextual diagnostics`, which sat immediately after it. DC5
/// (2026-09-14) moved the reach block to the panel FOOTER, below the tab
/// content, and left the diagnostic tiers where they were, because the tab
/// strip's badges read them. The end marker then sat ABOVE the start marker
/// and this function panicked.
///
/// The file's own instruction was "find it before weakening this test", so
/// it was found: the block moved, the guarantee did not. The readings this
/// file pins are still off the checkbox row and still constructed through
/// `wrapped_small_label`.
///
/// The end is now the `if`'s OWN closing brace, found by indent — the first
/// line after the start that is exactly four spaces, a brace and a newline.
/// Everything inside the block is indented deeper, so this cannot match
/// early. A marker that is part of the block's own syntax travels with the
/// block; a neighbouring comment does not, which is the defect this
/// paragraph exists to stop recurring.
///
/// # Why `"\n    }\n"` is safe here, and what would break it
///
/// The marker is NOT unique: it occurs **103 times** in `properties/mod.rs`
/// (measured 2026-09-14). The cut is correct only because it takes the
/// NEAREST occurrence after the start offset. Read that number alone and
/// this looks fragile; it is not, and the reason is worth stating so nobody
/// weakens the test to "fix" it.
///
/// **The invariant is upheld by rustfmt, and rustfmt is gated.** The `if`
/// sits at four spaces inside `fn draw_toolpath_panel`, so everything nested
/// inside it is indented to eight or more — verified: of the 104 lines in
/// the block, NONE is at indent four or less. A line of exactly four spaces,
/// a brace and a newline cannot occur inside the block in formatted source,
/// because it would need a construct closing at the `if`'s own indent while
/// still inside the `if`, which rustfmt does not emit. And
/// `cargo fmt --all -- --check` runs on every commit, so the source cannot
/// drift out of that shape without the format gate going red first.
///
/// So this rests on a CHECKED invariant, not on luck. It is not an absolute
/// one, and the next reader should know which kind it is:
///
/// - A multi-line string literal containing a line of `    }` would end the
///   slice early. There are none in `properties/mod.rs` today (measured
///   2026-09-14), and this scan does not model string literals.
/// - Both failure modes shorten the slice, and every assertion below is
///   NEGATIVE — "these readings are not inside that row". A shorter slice
///   therefore passes more easily. That is the silent direction. See the
///   stripper note in `tests/the_feeds_modal_holds_one_scope_dc5a.rs`.
fn reach_block(src: &str) -> &str {
    let start = src
        .find("if entry.operation.op_type().supports_reach_map() {")
        .expect("the reach block moved; find it before weakening this test");
    let rest = &src[start..];
    let end = rest
        .find("\n    }\n")
        .expect("the reach block has no closing brace at its own indent");
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
    let src = properties_src();
    let block = reach_block(&src);
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
    let src = properties_src();
    let block = reach_block(&src);
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
    let src = properties_src();
    let block = reach_block(&src);
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
    let src = properties_src();
    let body = function_body(&src, "wrapped_small_label");
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
    let src = properties_src();
    let body = function_body(&src, "render_diagnostic_row");
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
    let src = properties_src();
    let at = src
        .find("for err in &validation_errors {")
        .expect("the validation-error loop moved");
    let body = &src[at..(at + 500).min(src.len())];
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
    let src = properties_src();
    let start = src
        .find("// ── Shared header (always visible above tabs)")
        .expect("the header marker moved");
    let end = src[start..]
        .find("// Contextual diagnostics")
        .map(|at| start + at)
        .expect("the header's end marker moved");
    let header = &src[start..end];

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
