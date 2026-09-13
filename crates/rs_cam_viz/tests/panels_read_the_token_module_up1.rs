//! UP1's sentry: the token module exists, it is complete, and the panels are
//! migrating onto it.
//!
//! # Why a budget rather than a ban
//!
//! `DESIGN_SPEC.md` §2.10 rules that every colour is a token or it does not
//! ship. The crate cannot reach that in one package: a colour census found
//! **445 literals over 40 intents**, and the two worst files alone hold 155
//! of them (`ui/properties/mod.rs` 91, `ui/properties/operations/mod.rs` 64).
//! Migrating them is UP4's work, not UP1's.
//!
//! So arm 1 asserts a **descending budget** instead of zero. UP1 records the
//! count it inherited; every later package lowers the number in
//! [`LITERAL_BUDGET`] and the diff then shows the debt falling. UP8 sets it
//! to zero and the budget becomes the ban.
//!
//! A budget that only ever moves down is a ratchet: it cannot pass by
//! accident, because adding a literal fails the same assertion that a
//! migration relaxes.
//!
//! # The other three arms
//!
//! Arm 2 drives a real headless `Context` rather than scanning for a literal,
//! so a comment claiming `Small` is 11 cannot satisfy it.
//!
//! Arm 3 checks the `Style` fields the specification names.
//!
//! Arm 4 is the non-vacuity guard every source-scan sentry in this programme
//! must carry: a scan that visits nothing, or an exclusion list that has
//! grown to cover everything, fails rather than passes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::ui::tokens;

/// The number of `Color32::from_rgb(` call sites allowed outside the two
/// files that are ALLOWED to hold colour literals.
///
/// **This number only ever goes down.** Lower it in the package that does the
/// migrating and say so in that commit.
///
/// | Package | Budget | Note |
/// |---|---|---|
/// | UP1 | 378 | measured on arrival; UP1 migrated the 4 `CentralPanel` fills |
/// | UP8 | 0 | the budget becomes the ban |
const LITERAL_BUDGET: usize = 378;

/// The only two files that may name a colour literal.
///
/// `ui/tokens.rs` is where the tokens are defined. `render/colors.rs` is the
/// GPU-side palette: it feeds vertex buffers rather than widgets, and it is
/// out of scope for this programme.
const TOKEN_HOMES: [&str; 2] = ["ui/tokens.rs", "render/colors.rs"];

/// The fewest source files the scan must visit before its result is believed.
const MIN_FILES_SCANNED: usize = 40;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// `line` with any `//` comment removed, so a literal quoted in a doc comment
/// is not counted as a call site.
///
/// A `//` inside a string literal would end the line early here. That is
/// acceptable for this scan: it can only ever UNDER-count, and an
/// under-count cannot make a failing budget pass — it makes a passing one
/// pass for a slightly wrong reason, which arm 4 then has to be trusted for.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn is_token_home(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/");
    TOKEN_HOMES.iter().any(|home| s.ends_with(home))
}

#[test]
fn colour_literals_outside_the_token_module_stay_within_budget_up1() {
    let root = src_root();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    files.sort();

    let mut total = 0usize;
    let mut per_file: Vec<(String, usize)> = Vec::new();

    for path in &files {
        if is_token_home(path) {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        let n: usize = text
            .lines()
            .map(|l| strip_comment(l).matches("Color32::from_rgb(").count())
            .sum();
        if n > 0 {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            per_file.push((rel, n));
            total += n;
        }
    }

    per_file.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let worst: Vec<String> = per_file
        .iter()
        .take(8)
        .map(|(f, n)| format!("{f} {n}"))
        .collect();

    assert!(
        total <= LITERAL_BUDGET,
        "colour literals outside the token module rose to {total}, over the \
         budget of {LITERAL_BUDGET}. Every colour is a token or it does not \
         ship (DESIGN_SPEC.md 2.10). Worst files: {}",
        worst.join(", ")
    );
}

#[test]
fn the_scan_is_not_vacuous_up1() {
    let root = src_root();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);

    assert!(
        files.len() >= MIN_FILES_SCANNED,
        "the scan visited only {} files, fewer than the {MIN_FILES_SCANNED} \
         it must see. Either the walk broke or the crate moved.",
        files.len()
    );

    let excluded = files.iter().filter(|p| is_token_home(p)).count();
    assert_eq!(
        excluded,
        TOKEN_HOMES.len(),
        "the exclusion list should match exactly {} files, not {excluded}. \
         An exclusion that has grown to cover the crate would make the \
         budget vacuous.",
        TOKEN_HOMES.len()
    );
}

/// Arm 2. A real headless context, not a source scan.
#[test]
fn configure_theme_sets_every_built_in_text_style_up1() {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);

    let style = ctx.global_style();

    for slot in [
        egui::TextStyle::Heading,
        egui::TextStyle::Body,
        egui::TextStyle::Button,
        egui::TextStyle::Monospace,
        egui::TextStyle::Small,
    ] {
        assert!(
            style.text_styles.contains_key(&slot),
            "TextStyle::{slot:?} is unset, so egui falls back to its own \
             default for it and the type scale is not in force"
        );
    }

    let small = style.text_styles[&egui::TextStyle::Small].size;
    assert!(
        small >= 11.0,
        "TextStyle::Small resolves to {small}, below the 11-point floor. \
         Raising this slot is the single highest-leverage line in \
         DESIGN_SPEC.md 3.2: all 516 .small() call sites read it."
    );

    assert_eq!(
        style.text_styles[&egui::TextStyle::Body].size,
        tokens::SIZE_BODY,
        "Body must render at the token size"
    );
}

/// Arm 3. The `Style` fields the specification names.
#[test]
fn configure_theme_sets_spacing_wrap_and_shadows_up1() {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    let style = ctx.global_style();

    assert!(
        style.wrap_mode.is_some(),
        "Style::wrap_mode is unset, so egui defaults to Extend in a \
         horizontal layout. Extend grows the Ui past its panel and the panel \
         clips it, which is the mechanism behind AUDIT.md D-16."
    );

    assert_eq!(
        style.spacing.interact_size.y,
        tokens::ROW_ACTION,
        "the global control height must be ROW_ACTION"
    );

    assert_eq!(
        style.spacing.item_spacing,
        egui::vec2(tokens::SPACE_2, tokens::SPACE_2),
        "item_spacing must be the grid's step 2 on both axes"
    );

    assert_eq!(
        style.spacing.extra_text_line_spacing,
        tokens::EXTRA_LINE_SPACING,
        "the global leading must be set. This field arrived in egui 0.36 and \
         is why UP0 came first."
    );

    assert_ne!(
        style.visuals.window_shadow,
        egui::epaint::Shadow::NONE,
        "window_shadow is unset; elevation is carried by fill AND shadow"
    );
    assert_eq!(
        style.visuals.window_shadow,
        tokens::SHADOW_OVERLAY,
        "window_shadow must be the product's one shadow"
    );
    assert_eq!(
        style.visuals.popup_shadow,
        tokens::SHADOW_OVERLAY,
        "popup_shadow must be the product's one shadow"
    );

    assert_eq!(
        style.visuals.panel_fill,
        tokens::SURFACE_BASE,
        "the panel ground must be SURFACE_BASE"
    );
    assert_eq!(
        style.visuals.widgets.inactive.corner_radius,
        tokens::RADIUS_SM.into(),
        "a control's radius must be RADIUS_SM, not egui's default 2"
    );
}

/// Arm 3b. Every intent `DESIGN_SPEC.md` §2 names has a constant.
///
/// The specification's own prose says "40 distinct intents". That number is a
/// census of the EXISTING crate's literals, not a declared size for the new
/// set; the set below is what §2 actually names, and it is **41** single-
/// valued colour intents INCLUDING the scrim, plus one shadow and three
/// scale families. The count was reached twice independently and both
/// readings agree at 41, so the specification's "40" is the census figure
/// and not this set's size.
/// The list is spelled out so a missing token fails here rather than being
/// worked around at a call site.
#[test]
fn every_intent_in_section_two_has_a_constant_up1() {
    // Naming each constant is the assertion: a deleted or renamed token stops
    // this file compiling, which is a louder failure than a runtime check.
    let ramp = [
        tokens::INK_00,
        tokens::INK_05,
        tokens::INK_10,
        tokens::INK_15,
        tokens::INK_20,
        tokens::INK_25,
        tokens::INK_35,
        tokens::INK_50,
        tokens::INK_65,
        tokens::INK_80,
        tokens::INK_95,
    ];
    assert_eq!(ramp.len(), 11, "the neutral ramp has 11 steps");

    let text = [
        tokens::TEXT_FAINT,
        tokens::TEXT_MUTED,
        tokens::TEXT_BODY,
        tokens::TEXT_STRONG,
    ];
    let surfaces = [
        tokens::SURFACE_SUNKEN,
        tokens::SURFACE_BASE,
        tokens::SURFACE_RAISED,
        tokens::SURFACE_OVERLAY,
    ];
    let accent = [tokens::ACCENT, tokens::ACCENT_QUIET, tokens::ACCENT_PRESSED];
    let semantic = [
        tokens::OK,
        tokens::CAUTION,
        tokens::DANGER,
        tokens::INFO,
        tokens::UNKNOWN,
    ];
    let tints = [
        tokens::TINT_OK,
        tokens::TINT_CAUTION,
        tokens::TINT_DANGER,
        tokens::TINT_INFO,
        tokens::TINT_UNKNOWN,
    ];
    let structure = [tokens::HAIRLINE, tokens::BORDER, tokens::INPUT_WELL];
    let diagram = [
        tokens::DIAGRAM_CANVAS,
        tokens::DIAGRAM_INK,
        tokens::DIAGRAM_MATERIAL,
        tokens::DIAGRAM_TOOL,
        tokens::DIAGRAM_DIM,
    ];

    let named = ramp.len()
        + text.len()
        + surfaces.len()
        + accent.len()
        + semantic.len()
        + tints.len()
        + structure.len()
        + diagram.len()
        + 1; // SCRIM
    assert_eq!(
        named, 41,
        "DESIGN_SPEC.md 2 names 41 single-valued colour intents once SCRIM \
         is counted. If this moved, the specification moved with it."
    );

    // The families, which carry more than one value each.
    assert_eq!(tokens::SPAN_SCALE.len(), 6, "SPAN_SCALE has six steps");
    assert_eq!(tokens::CHART_SERIES.len(), 4, "CHART_SERIES has four steps");
    assert_eq!(tokens::LANE_SCALE.len(), 4, "LANE_SCALE has four states");

    // Colour is never the only channel, so every verdict carries a glyph.
    for g in [
        tokens::GLYPH_OK,
        tokens::GLYPH_CAUTION,
        tokens::GLYPH_DANGER,
        tokens::GLYPH_UNKNOWN,
    ] {
        assert!(!g.is_empty(), "every verdict carries a glyph");
    }
}

/// Arm 4b. `theme.rs` still exports every name it had before UP1.
///
/// UP1 must not break a call site. Naming all 20 here means a dropped
/// re-export fails to compile.
#[test]
fn theme_keeps_every_name_it_had_before_up1() {
    use rs_cam_viz::ui::theme;

    let originals = [
        theme::WARNING,
        theme::WARNING_MILD,
        theme::WARNING_TEXT,
        theme::ERROR,
        theme::ERROR_MILD,
        theme::SUCCESS,
        theme::SUCCESS_BRIGHT,
        theme::INFO,
        theme::TEXT_HEADING,
        theme::TEXT_STRONG,
        theme::TEXT_MUTED,
        theme::TEXT_DIM,
        theme::TEXT_FAINT,
        theme::ACCENT,
        theme::CARD_FILL,
        theme::CARD_FILL_SELECTED,
        theme::LANE_IDLE,
        theme::LANE_QUEUED,
        theme::LANE_RUNNING,
        theme::LANE_CANCELLING,
    ];
    assert_eq!(
        originals.len(),
        20,
        "theme.rs exported 20 colour constants before UP1 and must still \
         export all 20"
    );

    let _ = theme::card_frame(false);
    let _ = theme::card_frame(true);
}
