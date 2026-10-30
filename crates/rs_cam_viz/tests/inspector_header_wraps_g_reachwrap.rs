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
//! # IC1 (2026-10-13): one line, the full text on hover
//!
//! The operator, 2026-10-13: "the inspector panel is obscene. there is so
//! much text." The inspector clean-up gives every finding and every reading
//! ONE line. A long message truncates with an ellipsis, and its hover gives
//! the whole message. The defect this file guards is unchanged — a sentence
//! must never run off the panel and clip without a sign — but the remedy
//! changed from "wrap" to "one truncated line with the full text on hover".
//! A caveat that removes a guarantee (the reach grid over-states) stays a
//! face line of its own in the caution colour, so it is never only on a
//! hover. The arms that read the reach block and the diagnostic row check
//! that contract. The validator's errors block Generate, so they still wrap.
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

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
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

/// R3 (operator ruling 2026-10-30): the reach readings left the inspector
/// footer. The dock and the legend own the reach map; the Geometry tab
/// keeps ONE read-only `PropRow`, "Reach", whose value is one short line.
/// The grid line, the area base and the over-statement are its hover, so
/// no caveat sentence is on a row where it can clip.
///
/// This replaces the three G-REACHWRAP arms that read the deleted footer
/// block (the checkbox row, the `DataTable` readings and the status words).
#[test]
fn the_reach_reading_is_one_row_on_the_geometry_tab_r3() {
    let src = properties_src();
    assert!(
        !src.contains("\"Show reach map\""),
        "the inspector draws the reach checkbox again; R3 moved it to the dock"
    );
    assert!(
        !src.contains("toolpath_reach_readings"),
        "the inspector draws the reach table again; R3 moved it to the legend"
    );
    let body = function_body(&src, "draw_reach_row");
    assert!(
        body.contains("PropRow::new(\"Reach\")") && body.contains(".show_read_only("),
        "the Reach row must be one read-only PropRow:\n{body}"
    );
    for sentence in ["grid_line", "area_basis_note", "over_statement_note"] {
        let at = body
            .find(sentence)
            .unwrap_or_else(|| panic!("{sentence:?} is gone from the Reach row"));
        let detail = body[at..]
            .find(".detail(")
            .expect("the Reach row must carry its sentences as the hover detail");
        assert!(detail > 0, "{sentence:?} must reach the row hover");
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
/// derat". IC1: the row is one truncated line, and its hover holds the whole
/// message, so the caveat is one hover away and the ellipsis says so.
#[test]
fn the_diagnostic_message_is_one_line_with_the_whole_message_on_hover() {
    let src = properties_src();
    let body = function_body(&src, "render_diagnostic_row");
    // P4.D (L-2): the drawn line is `face`, the whole message or, for a
    // selection refusal, one line that names the Selection section. The
    // window starts at the drawn label, not at the message format.
    let face_at = body
        .find("let face = ")
        .expect("the drawn diagnostic line moved");
    assert!(
        body[face_at..].contains("format!(\"{prefix}{}\", d.message)"),
        "the drawn line no longer carries the message"
    );
    let at = face_at
        + body[face_at..]
            .find("egui::RichText::new(face)")
            .expect("the drawn diagnostic label moved");
    let around = &body[at..(at + 400).min(body.len())];
    assert!(
        around.contains(".truncate()") && around.contains(".on_hover_text(hover)"),
        "the diagnostic message must be one truncated line with its hover:\n{around}"
    );
    let hover_at = body
        .find("let mut hover = format!(\"{prefix}{}\", d.message);")
        .expect("the hover no longer starts with the whole message");
    assert!(
        hover_at < at,
        "the hover must be built before the line is drawn"
    );
}

/// The third sentence-bearing site in the header: the validator's
/// messages. They wrapped until G-E (2026-10-28). Now the header gives them
/// ONE line (§7.2 rule 5): the Geometry refusal's short line, else the
/// first sentence truncated, with every validation error on the hover. The
/// behaviour is measured in `the_inspector_text_is_bounded_ic1`
/// (`a_validation_error_takes_one_line_g_e`).
#[test]
fn the_validation_errors_are_one_line() {
    let src = properties_src();
    let at = src
        .find("let face = if !owned.is_empty()")
        .expect("the validation-error face moved");
    let body = &src[at..(at + 400).min(src.len())];
    assert!(
        body.contains("OneLine::new(face)") && body.contains(".detail(detail)"),
        "the validation errors are one line with their text on hover:\n{body}"
    );
    assert!(
        !src.contains("for err in other {"),
        "a validator sentence is drawn on a row of its own again"
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
    // FR-2 F22: the state word of the Generate row is drawn by
    // `draw_header_state`, so the census reads that body too.
    let state = function_body(&src, "draw_header_state");

    // The two Generate-row statuses with a long text. They wrapped until
    // G-E (2026-10-28); now each is one truncating state word with the full
    // text on hover (the IC1 remedy), and the last run's refusal is the
    // word `Refused` (`REFUSED_STATE`).
    for existing in ["Waiting on upstream stock", "REFUSED_STATE"] {
        let at = state
            .find(existing)
            .unwrap_or_else(|| panic!("{existing:?} left the header state"));
        let before = &state[at.saturating_sub(120)..at];
        let after = &state[at..(at + 200).min(state.len())];
        assert!(
            before.contains("state_word(") && after.contains(".on_hover_text("),
            "{existing:?} is one truncating word with its text on hover:\n{before}{after}"
        );
        assert!(
            !after.contains(".wrap()"),
            "{existing:?} wraps again:\n{after}"
        );
    }

    // No bare `ui.label(` may carry a full stop followed by a space in the
    // header — that is the shape of a multi-sentence caveat, and it is the
    // shape that clipped.
    for (idx, line) in header.lines().chain(state.lines()).enumerate() {
        assert!(
            !(line.trim_start().starts_with("ui.label(") && line.contains(". ")),
            "header line {idx} draws a sentence through a bare ui.label: {line}"
        );
    }
}

/// Audit WRONG #2 (2026-10-02): the reach reading prints the area base and
/// the over-statement sentence once each. The grid line is
/// `ReachMap::grid_line`, which holds neither. `ReachMap::grid_note` holds
/// both, so a reading that prints it says each sentence twice. The core
/// sentry `reach_map_p5` pins what `grid_line` holds; this pins that the
/// Reach row (R3; the footer before it) reads it.
#[test]
fn the_reach_row_prints_the_grid_line_not_the_grid_note() {
    let src = properties_src();
    let body = function_body(&src, "draw_reach_row");
    assert!(
        body.contains("{grid_line}"),
        "the Reach row no longer prints the grid line:\n{body}"
    );
    assert!(
        !body.contains("grid_note"),
        "the Reach row prints `grid_note`, which already holds the area base \
         and the over-statement sentence that the row prints on their own:\n{body}"
    );
}
