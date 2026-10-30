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

/// W7 (2026-10-30) split `toolpath_panel.rs` into the module root and the
/// children under `toolpath_panel/`. They are one module of the inspector, so
/// the reader takes the sub-folder too.
fn push_panel_children(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            push_panel_children(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

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
    push_panel_children(&dir.join("toolpath_panel"), &mut paths);
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
    // W7: a child of `toolpath_panel/` opens an item with `pub(in ..)`.
    let end = [
        "\nfn ",
        "\npub fn ",
        "\npub(crate) fn ",
        "\npub(super) fn ",
        "\npub(in ",
    ]
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

/// The tool-load caution — the sentence the review caught at "The whole
/// derat". IC1, W2b: a finding is one issue row (the NoticeStack issue
/// form truncates its face), and its hover holds the whole message first.
/// The footer row that drew it before W2b is gone.
#[test]
fn the_diagnostic_message_is_one_line_with_the_whole_message_on_hover() {
    let src = properties_src();
    assert!(
        !src.contains("fn render_diagnostic_row("),
        "the per-tab footer row is back; the issue stack replaces it (W2b)"
    );
    let stack = function_body(&src, "draw_issue_stack");
    // W7: the stack is in `toolpath_panel/issues.rs`, so it names the
    // placement module in full, and rustfmt breaks the call over lines.
    // The needles read the body without white space.
    let flat: String = stack.split_whitespace().collect();
    assert!(
        flat.contains("Notice::new(crate::ui::properties::placement::role_of(d),d.face.as_str(),)")
            && flat.contains(".detail(crate::ui::properties::placement::detail(d))")
            && flat.contains(".show_issues(ui)"),
        "a finding must be one issue row with its detail on hover:\n{stack}"
    );
    let detail = function_body(&src, "detail");
    assert!(
        detail.contains("let mut out = d.message.clone();"),
        "the hover detail must start with the whole message:\n{detail}"
    );
}

/// The third sentence-bearing site in the header: the validator's
/// messages. W2b: a validator refusal that no core finding states is ONE
/// issue row (`gate_refusals`); the issue row truncates and keeps its text
/// on hover. The behaviour is measured in
/// `the_inspector_text_is_bounded_ic1` (`a_validation_error_takes_one_line_g_e`).
#[test]
fn the_validation_errors_are_one_line() {
    let src = properties_src();
    let stack = function_body(&src, "draw_issue_stack");
    assert!(
        stack.contains("for gate in gates {")
            && stack.contains("Notice::new(Role::Danger, gate.as_str())"),
        "the validator refusals are issue rows:\n{stack}"
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
    // The header runs from the Name row to the tab strip.
    let src = properties_src();
    let start = src
        .find("// ── Shared header (always visible above tabs)")
        .expect("the header marker moved");
    let end = src[start..]
        .find("// ── Tab bar")
        .map(|at| start + at)
        .expect("the header's end marker moved");
    let header = &src[start..end];
    // W2b: the status line draws ONE state word from core `RunState`
    // (`HeaderRow::word`), as a truncating label with its text on hover.
    let state = function_body(&src, "draw_generate_row");
    assert!(
        state.contains("row.word()")
            && state.contains(".truncate()")
            && state.contains(".on_hover_text(hover)")
            && !state.contains(".wrap()"),
        "the state word is one truncating label with its text on hover:\n{state}"
    );
    let words = function_body(&src, "word");
    for existing in ["WORD_WAITING", "WORD_FAILED", "WORD_NOT_READY"] {
        assert!(
            words.contains(existing),
            "{existing:?} left the header state:\n{words}"
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
