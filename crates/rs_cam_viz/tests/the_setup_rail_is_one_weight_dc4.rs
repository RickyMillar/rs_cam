//! DC4's sentry: the Setup rail draws one kind of object at one weight.
//!
//! # The defect this exists to catch
//!
//! `planning/ui_declutter_2026-09-14/PLAN.md` Pattern D. The rail drew the
//! same kind of object at three weights at once: Stock was a full card,
//! Machine was a full card, Models was a `▸` disclosure, and the Tool
//! Library was a `▸` disclosure in a different workspace altogether. The two
//! setup cards were different HEIGHTS from each other, because one carried
//! an extra line that the other did not.
//!
//! All four of those are the project's RESOURCES. You click one to open its
//! editor in the right sidebar, so each is NAVIGATION, and a navigation item
//! is one line.
//!
//! # Why this invariant and not a screenshot diff
//!
//! A screenshot pair answers "did the pixels change", which is the question
//! the previous phase kept answering while the product got denser. §5 says
//! this phase is judged by a COUNT, and a count is a property of the source:
//! one row helper, one card call, no disclosure. A source scan states that
//! property directly, fails the moment someone re-adds a card around a
//! resource, and costs nothing to run.
//!
//! It also pins the half a screenshot cannot see: ruling R29 moved the Tool
//! Library here, and the WP23 command census
//! (`command_surface_completeness.rs`) requires every command it carried to
//! keep a route. Arm 3 names each one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::ui::tokens;

/// The file this sentry is about.
const PANEL: &str = "ui/setup_panel.rs";

/// The four resources the rail lists, in the order it draws them.
const RESOURCES: [&str; 4] = ["Stock", "Machine", "Models", "Tools"];

/// The commands the Tool Library and the Models list carry. Each must still
/// be CONSTRUCTED in this file after DC4 moved both here.
const ROUTED_COMMANDS: [&str; 8] = [
    "OpenToolLibrary",
    "DuplicateTool",
    "RemoveTool",
    "AddTool(",
    "AddToolFromLibrary",
    "ReloadModel",
    "RemoveModel",
    "AddSetup",
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Read the panel with every `//` comment stripped.
///
/// A doc comment naming `CollapsingHeader` is prose, not a call, and a scan
/// that matches one reports a widget that does not exist.
fn panel_source() -> String {
    let path = src_root().join(PANEL);
    assert!(path.is_file(), "{PANEL} no longer exists; DC4 is stale");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source between two `fn` items, so an arm can ask where a thing sits.
fn between<'a>(src: &'a str, from: &str, to: &str) -> &'a str {
    let start = src
        .find(from)
        .unwrap_or_else(|| panic!("{from} is gone from {PANEL}"));
    let end = src
        .find(to)
        .unwrap_or_else(|| panic!("{to} is gone from {PANEL}"));
    assert!(start < end, "{from} must be declared before {to}");
    &src[start..end]
}

// ── arm 1 ────────────────────────────────────────────────────────────

#[test]
fn only_a_setup_wears_a_card_dc4() {
    let src = panel_source();

    assert_eq!(
        src.matches("theme::card_frame").count(),
        0,
        "Stock and Machine each wore a full card frame. A resource is \
         navigation, so it is one line and it carries no card."
    );

    let cards = src.matches("Card::new()").count();
    assert_eq!(
        cards, 1,
        "the rail may build exactly ONE kind of card, the setup card. \
         Found {cards} Card::new() call sites."
    );

    let card_at = src.find("Card::new()").unwrap();
    let setup_card_at = src.find("fn draw_setup_card").unwrap();
    assert!(
        card_at > setup_card_at,
        "the one card must belong to draw_setup_card. A card above it means \
         a resource grew a frame again."
    );
}

// ── arm 2 ────────────────────────────────────────────────────────────

#[test]
fn every_resource_row_is_one_dense_line_dc4() {
    let src = panel_source();

    let helpers = src.matches("fn resource_row").count();
    assert_eq!(
        helpers, 1,
        "one weight means one implementation. Found {helpers} row helpers."
    );

    let rows = src.matches("resource_row(ui,").count();
    assert_eq!(
        rows,
        RESOURCES.len(),
        "the rail lists {} resources — {} — and every one must go through \
         the single row helper.",
        RESOURCES.len(),
        RESOURCES.join(", ")
    );
    for label in RESOURCES {
        assert!(
            src.contains(&format!("\"{label}\"")),
            "the {label} row is gone from the rail"
        );
    }

    assert!(
        src.contains("tokens::ROW_DENSE"),
        "a resource row takes its height from the token, never from a \
         literal and never from its content"
    );

    // No resource row sets a variable height, and nothing in the file sets a
    // bare spacing. Every step is a token, so the rhythm cannot drift.
    let spaces = src.matches("add_space(").count();
    let token_spaces = src.matches("add_space(tokens::").count();
    assert_eq!(
        spaces,
        token_spaces,
        "{} of {spaces} add_space calls name a bare float. Every spacing \
         comes from tokens::SPACE_*.",
        spaces - token_spaces
    );

    let literals =
        src.matches("Color32::from_rgb(").count() + src.matches("Color32::from_rgba_").count();
    assert_eq!(
        literals, 0,
        "the UP1 budget sentry sits at 1 with one documented exception \
         elsewhere. Every colour here is a token."
    );

    let resource_row = between(&src, "fn resource_row", "fn draw_setup_card");
    assert!(
        resource_row.contains("match menu"),
        "a resource row chooses exactly one trailing affordance"
    );
    let menu_arm = between(resource_row, "Some(menu) =>", "None if has_resources");
    assert!(
        menu_arm.contains("ui.menu_button(ELLIPSIS, menu)") && !menu_arm.contains("CHEVRON"),
        "a row with actions exposes its menu as its only trailing affordance"
    );
    let chevron_arm = between(resource_row, "None if has_resources", "None =>");
    assert!(
        chevron_arm.contains("CHEVRON") && !chevron_arm.contains("ELLIPSIS"),
        "a resource row draws a chevron only when it has no menu"
    );
}

#[test]
fn a_setup_card_cannot_grow_a_second_line_dc4() {
    let src = panel_source();
    let card = between(&src, "fn draw_setup_card", "fn setup_detail");

    assert!(
        card.contains("set_min_height(tokens::ROW_DENSE)"),
        "the setup card's body is one ROW_DENSE line"
    );
    assert!(
        card.contains(".truncate()"),
        "the setup name truncates. A name that wraps is what made one card \
         taller than its neighbour."
    );

    // The lines that used to VARY between cards now live in the hover text,
    // which is a string and cannot change a card's height.
    for varying in ["flip_instruction", "keep_out_zones", "xy_method"] {
        assert!(
            !card.contains(varying),
            "{varying} is back on the card body. Rule D: a card's height \
             does not vary with its content, so a line that only some \
             setups carry belongs in setup_detail's hover text."
        );
    }
    assert!(
        src.contains("fn setup_detail"),
        "the varying lines need somewhere to go, and that is the hover text"
    );
}

// ── arm 3 ────────────────────────────────────────────────────────────

#[test]
fn every_moved_command_still_has_a_route_dc4() {
    let src = panel_source();
    let mut missing = Vec::new();
    for command in ROUTED_COMMANDS {
        if !src.contains(command) {
            missing.push(command);
        }
    }
    assert!(
        missing.is_empty(),
        "DC4 moved the Tool Library here (ruling R29) and kept the Models \
         list. These commands lost their last route in this file: {}. §4 of \
         the plan: a control may move, but the command stays reachable, and \
         command_surface_completeness.rs measures the same thing crate-wide.",
        missing.join(", ")
    );
}

// ── arm 4 ────────────────────────────────────────────────────────────

#[test]
fn the_rail_holds_no_disclosure_dc4() {
    let src = panel_source();
    assert!(
        !src.contains("CollapsingHeader"),
        "Models and the Tool Library were both `▸` disclosures, drawn at a \
         different weight from the cards beside them. That IS Pattern D. \
         A resource opens its editor; it does not unfold in place."
    );
}

#[test]
fn the_scan_is_not_vacuous_dc4() {
    // A source scan that reads an empty string passes every arm above.
    let src = panel_source();
    assert!(
        src.len() > 4_000,
        "the comment-stripped panel is {} bytes. The scans above cannot be \
         believed against a file this small.",
        src.len()
    );
    assert!(
        src.contains("pub fn draw("),
        "the panel must still expose the entry point app.rs calls"
    );

    // Non-vacuity for the token names the scans grep for: a renamed token
    // must fail HERE rather than making an assertion unsatisfiable in
    // silence.
    const _: () = assert!(tokens::ROW_DENSE == 22.0);
    let _ = tokens::ACCENT_QUIET;
    let _ = tokens::TEXT_FAINT;
    let _ = tokens::TEXT_MUTED;
    let _ = tokens::TEXT_STRONG;
    let _ = tokens::HAIRLINE;
}
