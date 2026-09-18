//! DC1's sentry: the operation card stays one row of five elements.
//!
//! # The defect this exists to catch
//!
//! The card grew to **thirteen elements when selected and eight at rest** —
//! a drag grip, a colour swatch, a status chip, a `MAN` badge, a `TRACE`
//! badge, a name, a tool name, a rest badge, a `Sim` button, a `▶` generate
//! button, three figures on a stats row, and six glyph buttons that appeared
//! only on hover. Nine cards were about eighty elements in one panel.
//!
//! Nothing added any one of those in bad faith. Each arrived alone, each
//! answered a real question, and the panel became a wall by accumulation.
//! That is the failure mode a sentry can catch and a review cannot: the
//! thirteenth element looks exactly as reasonable as the third.
//!
//! # Why this invariant and not a screenshot diff
//!
//! A screenshot diff fails on every legitimate change — a new theme, a font
//! metric, a palette entry — so it gets regenerated, and a regenerated
//! baseline pins nothing. These arms name the specific elements the rulings
//! deleted. An arm fails only when a deleted element returns, which is the
//! one event DC1 cares about.
//!
//! # What it does NOT prove
//!
//! That the row looks right, that it fits at a narrow panel width, or that
//! the `…` menu is discoverable. DC1's acceptance is the operator looking at
//! the panel and finding less on it (`PLAN.md` §5), and no test replaces
//! that.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::config::AwaitingPriorStock;
use rs_cam_viz::state::freshness::FreshnessState;
use rs_cam_viz::ui::components::Role;
use rs_cam_viz::ui::toolpath_panel::status_chip;

const PANEL_SRC: &str = include_str!("../src/ui/toolpath_panel.rs");

/// One `//` comment stripped from a line, so a ruling that NAMES a deleted
/// element in prose does not read as the element itself. Every arm below
/// scans the stripped text, because the rulings are recorded in comments
/// directly above the code that carries them out.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) => &line[..at],
        None => line,
    }
}

fn code_only() -> String {
    PANEL_SRC
        .lines()
        .map(strip_comment)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The seven freshness states, each constructed here so the mapping is
/// driven rather than scanned.
fn every_freshness_state() -> Vec<FreshnessState> {
    vec![
        FreshnessState::Current,
        FreshnessState::EditedSince,
        FreshnessState::Regenerating,
        FreshnessState::WaitingOnUpstream(AwaitingPriorStock {
            blocking_toolpath_id: None,
            blocking_toolpath_index: None,
            message: "the upstream operation has not been simulated".to_owned(),
        }),
        FreshnessState::Error("generation failed".to_owned()),
        FreshnessState::NoResult,
        FreshnessState::Disabled,
    ]
}

// ── arm 1 — the deleted elements stay deleted ────────────────────────

/// Every element the DC1 rulings removed from the card.
///
/// Each entry is a needle plus the ruling that deleted it, so a failure
/// message tells the next reader where the decision lives rather than only
/// that a string came back.
const DELETED: &[(&str, &str)] = &[
    (
        "StatusChip",
        "R24 — one state dot carries the state; the chip, the `Sim` button \
         and the `▶` button were three renderings of one FreshnessState",
    ),
    (
        "\"MAN\"",
        "R25 — MAN is a property of the operation TYPE, so every 3D card \
         carried it and it separated nothing",
    ),
    (
        "draw_trace_badge",
        "R25 — TRACE is generator-debug provenance and belongs on the \
         simulation debug surface",
    ),
    (
        "\"Sim\"",
        "R24 — the Sim button was a state wearing a button's clothes; the \
         `…` menu keeps the explicit route",
    ),
    (
        "\\u{25B6}",
        "R24 — the ▶ generate button is the state dot's click",
    ),
    (
        "tp_card_full_rect",
        "Rule B — no action on this card is hover-only, so the rect the \
         hover test read has no job left",
    ),
    (
        "controls_visible",
        "Rule B — the row-4 reveal is gone with the machinery that timed it",
    ),
    (
        "\\u{25CE}",
        "UR5 — isolation is retired; the bullseye glyph must not return",
    ),
];

#[test]
fn the_card_names_no_deleted_element_dc1() {
    let code = code_only();
    let mut found = Vec::new();
    for (needle, ruling) in DELETED {
        if code.contains(needle) {
            found.push(format!("{needle} \u{2014} {ruling}"));
        }
    }
    assert!(
        found.is_empty(),
        "the operation card carries an element DC1 deleted:\n  {}\n\
         The card is ONE row of five elements (R32): swatch, state dot, \
         name, tool, eye, and the `…` menu. An element that needs to come \
         back needs a ruling in planning/ui_declutter_2026-09-14/PLAN.md \
         first.",
        found.join("\n  ")
    );
}

/// The five elements the card DOES carry, so arm 1 cannot pass by the card
/// having been emptied instead of trimmed.
#[test]
fn the_card_still_draws_its_five_elements_dc1() {
    let code = code_only();
    for needle in [
        "draw_swatch(",
        "draw_state_dot(",
        "draw_eye(",
        "card_menu(",
        // W3/R3 — the Rest `dep` badge folded into the gutter connector.
        // The card keeps the same count of elements: it lost two words
        // inside the row and gained one line outside the card frame.
        "draw_connectors(",
    ] {
        assert!(
            code.contains(needle),
            "the card no longer draws {needle}, so it lost an element DC1 \
             kept rather than one it deleted"
        );
    }
    assert!(
        code.contains("egui::DragAndDrop::set_payload"),
        "R26 made the swatch the drag source. Drag-and-drop reorder and \
         cross-setup move must still work."
    );
    assert!(
        code.contains("if state.viewport.show_all_toolpaths"),
        "UR5 keeps the eye only in all-toolpaths mode; selected-only has no visibility route"
    );
}

// ── arm 2 — the state vocabulary is complete ─────────────────────────

/// `status_chip` still maps all seven states to a role.
///
/// The dot reads its colour from the role and its word from hover, so a
/// state that fell out of this mapping would draw no dot at all. The
/// function is driven, not scanned: a scan cannot tell a match arm that
/// returns a role from one that returns a placeholder.
#[test]
fn the_state_dot_has_a_role_for_every_freshness_state_dc1() {
    let states = every_freshness_state();
    assert_eq!(
        states.len(),
        7,
        "non-vacuity: FreshnessState has seven arms and the sentry must \
         drive all of them"
    );

    let mut roles = Vec::new();
    for state in &states {
        let (word, role, _hover) = status_chip(state);
        assert!(
            !word.is_empty(),
            "{} maps to an empty word, so its hover says nothing",
            state.label()
        );
        roles.push(role);
    }

    // Three roles are enough for a dot to be a dot; the vocabulary collapses
    // onto them by design (UP4). What must never happen is one role for all
    // seven, which is a dot that carries no information.
    roles.sort();
    roles.dedup();
    assert!(
        roles.len() >= 2,
        "every freshness state resolved to one role, so the dot's colour \
         says nothing: {roles:?}"
    );
    assert!(
        roles.contains(&Role::Danger),
        "an Error state must still reach a Danger role \u{2014} R30 keeps \
         safety's voice while every other badge quietens"
    );
}

// ── arm 3 — DC2's deletions from the panel ───────────────────────────

#[test]
fn the_panel_names_neither_the_heading_nor_the_tool_library_dc2() {
    let code = code_only();
    assert!(
        !code.contains("\"Operations\""),
        "DC2 deleted the `Operations` heading and its rule. The workspace \
         tab already says Toolpaths, and Generate All takes that space."
    );
    assert!(
        !code.contains("Tool Library"),
        "R29 moved the Tool Library to the Setup workspace, where the \
         project's other RESOURCES live. It must not return to the \
         operation queue as a disclosure."
    );
    assert!(
        !code.contains("\"+ Add\""),
        "DC2 shortened the add menu's label to `+`. The menu's own items \
         say what each one adds."
    );
}

// ── arm 3b — the setup header keeps its one row ──────────────────────

/// The setup header carries TWO controls, and the setup menu carries ONE
/// item.
///
/// Operator ruling 2026-09-19: the setup row gains a `…` with "Regenerate
/// setup only". DC1's whole subject is accumulation, and a menu is the
/// easiest place for it: every future setup action will look as reasonable
/// as this one. The narrow regeneration route is the ONE thing this menu is
/// for, and anything else on the header needs a ruling first.
#[test]
fn the_setup_header_holds_two_controls_and_one_menu_item_dc1() {
    let code = code_only();
    let at = code
        .find("if multi_setup {")
        .expect("the setup header moved out of the panel");
    let header = &code[at..(at + 900).min(code.len())];
    let rows = header.matches("ui.horizontal(").count();
    assert_eq!(
        rows, 1,
        "the setup header is ONE row, and it opened {rows} of them:\n{header}"
    );
    for needle in ["add_toolpath_menu(", "setup_menu("] {
        assert!(
            header.contains(needle),
            "the header must still call {needle}:\n{header}"
        );
    }

    let at = code
        .find("fn setup_menu(")
        .expect("the setup menu moved out of the panel");
    let body = &code[at..(at + 1200).min(code.len())];
    let items = body.matches("egui::Button::new(").count() + body.matches("ui.button(").count();
    assert_eq!(
        items, 1,
        "the setup menu offers ONE item, and it offers {items}. A second \
         action there needs a ruling, not a line:\n{body}"
    );
    assert!(
        body.contains("\"Regenerate setup only\""),
        "the one item is the narrow regeneration route:\n{body}"
    );
    assert!(
        body.contains("on_disabled_hover_text("),
        "a disabled control always states the reason (DESIGN_SPEC §4.5)"
    );
}

// ── arm 4 — non-vacuity ──────────────────────────────────────────────

/// The scans above are `!contains` assertions, and a `!contains` over an
/// empty string passes. This arm proves the file was read, that stripping
/// comments did not empty it, and that the spellings the other arms depend
/// on are the spellings the file actually uses.
#[test]
fn the_scan_read_a_real_panel_dc1() {
    assert!(
        PANEL_SRC.len() > 10_000,
        "the panel source is {} bytes, which is too small to be the \
         operations panel",
        PANEL_SRC.len()
    );
    let code = code_only();
    assert!(
        code.len() > PANEL_SRC.len() / 2,
        "stripping comments removed more than half the file, so the arms \
         above are scanning almost nothing"
    );
    // The anchors. Each one is a spelling an arm above relies on: if the
    // card were renamed or moved, every `!contains` would pass for the
    // wrong reason.
    for anchor in [
        "fn draw_toolpath_card(",
        "pub fn status_chip(",
        "fn add_toolpath_menu(",
        "\"Generate All\"",
        "\"Regenerate setup only\"",
        "ToggleToolpathVisibility",
    ] {
        assert!(
            code.contains(anchor),
            "the anchor {anchor} is gone from the panel, so this file is \
             no longer scanning what it thinks it is"
        );
    }
}
