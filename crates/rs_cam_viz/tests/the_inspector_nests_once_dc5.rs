//! DC5's sentry: the toolpath inspector nests ONCE.
//!
//! # The defect this exists to catch
//!
//! Seen on screen, 2026-09-14. The inspector's reading order was:
//!
//! > name field → Generate button → a sentence → `▾ Geometry` disclosure
//! > (3 fields) → `Hints (1)` disclosure → **tab strip (5 tabs)** → fields
//!
//! `Geometry` was a collapsible disclosure AND a tab at the same time,
//! holding different content, and the tab strip sat BELOW the disclosure.
//! `Feeds & Speeds` carried the same collision one level down: a default-open
//! `Feeds & Speeds` disclosure on the `Feeds & Speeds` tab. A reader cannot
//! tell what contains what when one name has two homes.
//!
//! Rule A of `planning/ui_declutter_2026-09-14/PLAN.md`: **one nesting
//! mechanism per level. Tabs are peer views of ONE object. A disclosure is
//! for genuinely optional detail and never duplicates a tab.**
//!
//! # Why these invariants, and not a screenshot diff
//!
//! A screenshot diff catches this once and then fails on every legitimate
//! change. The invariants that actually hold are narrower, and each one names
//! the defect it forbids:
//!
//! 1. No tab's name is also a disclosure's name. Driven from the inspector's
//!    own `ToolpathTab::label` match, so a NEW tab arrives here without
//!    anyone editing this file and cannot re-open the defect under a new
//!    name. The match is read rather than `ToolpathTab::ALL` because the
//!    match is exhaustive over the enum and `ALL` is a hand-written list: a
//!    variant left out of `ALL` still reaches this rule.
//! 2. The `Hints` block renders BELOW the tab strip, and so does the geometry
//!    wiring. This arm is a **source-order scan**, not a layout measurement,
//!    and that is a deliberate compromise: `draw_toolpath_panel` takes
//!    nineteen parameters, four of which are borrowed core contexts, so
//!    driving it headlessly to read the y of two widgets costs more fixture
//!    than the invariant is worth. Source order IS draw order inside one
//!    `Ui`, so the scan answers the question the operator asked. It fails
//!    open if the anchors are renamed, which arm 4 guards.
//! 3. The Feeds tab's worst annotation row stays inside the panel (F-3).
//! 4. Non-vacuity for arms 1 and 2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::ui::tokens;

/// The panel width the app gives the inspector (`app.rs`, `default_size`).
const PANEL_WIDTH: f32 = 280.0;

/// The inspector's source folder.
///
/// # Why a folder and not one file
///
/// P4 (2026-09-17) split `properties/mod.rs` into `mod.rs` plus seven panel
/// children beside it. The panel the scans below read is spread over those
/// files, so the reader is the folder. `operations/` is a sub-folder and is
/// NOT read: it carries its own sentries.
fn inspector_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("ui")
        .join("properties")
}

/// Every `.rs` file directly in that folder, concatenated in file-name order.
///
/// The order matters to `the_hints_block_renders_below_the_tab_strip_dc5`,
/// which compares byte offsets. It reads the CALL sites, all of which sit in
/// `toolpath_panel.rs`; that name sorts last, after the `tab_badges.rs`
/// DEFINITIONS its `rfind` must not pick. A new child whose name sorts after
/// `toolpath_panel.rs` and repeats one of those markers would break that arm,
/// and the arm says so when it fails.
fn inspector_src() -> String {
    let dir = inspector_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no source under {}; the scans below would read nothing",
        dir.display()
    );
    let mut out = String::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        out.push_str(&text);
        out.push('\n');
    }
    out
}

/// Every `ToolpathTab` label, read out of the inspector's own `label()`
/// match. Reading the match rather than a list written here is what makes a
/// new tab inherit the rule instead of escaping it.
fn tab_labels(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(start) = src.find("fn label(self) -> &'static str {") else {
        return out;
    };
    let body = &src[start..];
    let Some(end) = body.find("\n    }\n") else {
        return out;
    };
    for line in body[..end].lines() {
        let Some((lhs, rhs)) = line.split_once("=> \"") else {
            continue;
        };
        if !lhs.contains("ToolpathTab::") {
            continue;
        }
        let Some((label, _)) = rhs.split_once('"') else {
            continue;
        };
        out.push(label.to_owned());
    }
    out
}

#[test]
fn no_name_is_both_a_tab_and_a_disclosure_dc5() {
    let src = inspector_src();
    let labels = tab_labels(&src);
    assert!(
        labels.len() >= 5,
        "read only {} tab labels out of the inspector's `label()` match. The \
         scan below is driven by this list, so an empty one would pass for \
         the wrong reason.",
        labels.len()
    );
    for label in &labels {
        let pattern = format!("CollapsingHeader::new(\"{label}\")");
        assert!(
            !src.contains(&pattern),
            "`{label}` is a tab AND a disclosure. Tabs are peer views of one \
             object; a disclosure is for genuinely optional detail and never \
             duplicates a tab name. A reader who meets the same word twice at \
             two levels cannot tell what contains what. Either rename the \
             disclosure or fold its rows onto the tab."
        );
    }
}

#[test]
fn the_hints_block_renders_below_the_tab_strip_dc5() {
    let src = inspector_src();
    let strip = src
        .find("draw_toolpath_tabs(ui, &mut active_tab")
        .expect("the tab strip call moved; re-anchor this scan");
    let hints = src
        .find("{} hints")
        .expect("the hints count row moved; re-anchor this scan");
    assert!(
        hints > strip,
        "the hints block draws at byte {hints}, the tab strip at {strip}, so \
         the hints render ABOVE the strip. The operator's words were \"hints \
         is a lot of text and its at the top, why?\". A hint list is optional \
         detail: it belongs at the bottom, as a count that opens."
    );

    // The geometry WIRING belongs to the Geometry tab, so it must not draw
    // above the strip either. `rfind` picks the CALL; the definition sits
    // above `draw_toolpath_panel`.
    let wiring = src
        .rfind("draw_geometry_wiring(")
        .expect("the geometry wiring call moved; re-anchor this scan");
    assert!(
        wiring > strip,
        "the geometry wiring draws at byte {wiring}, the tab strip at \
         {strip}. Nothing that belongs to a tab may render above the strip \
         that selects it."
    );
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // epaint 0.36's `Drop for TexturesDelta` panics on a delta that was never
    // consumed, so every frame this file runs is drained.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// The width a block of rows asks for when laid out inside `PANEL_WIDTH`.
///
/// `Ui::min_rect` is what a `Panel` reads to decide how wide it must be, so
/// this is the same number the real layout consults. Copied from
/// `inspector_width_is_tab_independent_up4.rs`, which owns the mechanism.
fn requested_width(ctx: &egui::Context, build: impl Fn(&mut egui::Ui)) -> f32 {
    let mut width = 0.0_f32;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_max_width(PANEL_WIDTH);
        let inner = ui.scope(|ui| {
            ui.set_max_width(PANEL_WIDTH);
            build(ui);
        });
        width = inner.response.rect.width();
    });
    out.textures_delta.clear();
    width
}

/// The Feeds tab's worst row, verbatim.
///
/// `advance_gate_verdict_text` in `ui/properties/mod.rs` returns this string
/// for a recipe the rubbing-floor clamp parked on the band ceiling, and
/// `draw_advance_per_tooth_card` draws it beside its label in a two-column
/// grid. A grid cell and a horizontal row share one default wrap mode,
/// `Extend`, so this row reproduces the grid cell's constraint.
const VERDICT_LABEL: &str = "Gate verdict:";
const VERDICT_VALUE: &str = "CLAMPED to band ceiling \u{2014} not exceeded";

fn verdict_row(ui: &mut egui::Ui, wrap: bool) {
    ui.horizontal(|ui| {
        ui.label(VERDICT_LABEL);
        let mode = if wrap {
            egui::TextWrapMode::Wrap
        } else {
            egui::TextWrapMode::Extend
        };
        ui.add(egui::Label::new(VERDICT_VALUE).wrap_mode(mode));
    });
}

#[test]
fn the_feeds_verdict_row_stays_inside_the_panel_dc5() {
    let ctx = ctx();
    let wrapped = requested_width(&ctx, |ui| verdict_row(ui, true));
    assert!(
        wrapped <= PANEL_WIDTH + 0.5,
        "the gate-verdict row asked for {wrapped} inside a {PANEL_WIDTH} \
         point panel. F-3: the panel then clamps to its own maximum and draws \
         the over-wide content right-aligned, which puts its left end outside \
         the clip rect and slides the whole Feeds tab off its own left edge."
    );
}

#[test]
fn the_unwrapped_verdict_row_still_overflows_dc5() {
    // Non-vacuity for the arm above. If `Extend` ever stopped overflowing,
    // that arm would pass for the wrong reason and this programme would
    // believe F-3 was fixed when the toolkit had merely changed.
    let ctx = ctx();
    let extended = requested_width(&ctx, |ui| verdict_row(ui, false));
    assert!(
        extended > PANEL_WIDTH,
        "Extend asked for only {extended} inside {PANEL_WIDTH}, so this row \
         no longer reproduces F-3's mechanism. Re-derive it against the \
         current egui and the current worst string."
    );
}

#[test]
fn the_scans_have_something_to_scan_dc5() {
    // Non-vacuity for arms 1 and 2. Each reads the inspector by pattern, so
    // a renamed API or a moved file would make every pattern unsatisfiable
    // and every assertion vacuously true.
    let src = inspector_src();
    assert!(
        src.len() > 100_000,
        "the inspector source is {} bytes; the scans above are reading the \
         wrong file",
        src.len()
    );
    assert!(
        src.contains("CollapsingHeader::new(\""),
        "the inspector uses no literal-named disclosure at all, so arm 1's \
         pattern can never match and the rule it pins is unenforced"
    );
    assert!(
        src.contains("enum ToolpathTab"),
        "the tab enum moved out of the inspector, so arm 1 reads no labels"
    );
    let labels = tab_labels(&src);
    assert!(
        labels.iter().any(|l| l == "Geometry"),
        "`Geometry` is the tab the defect was named for; found {labels:?}"
    );
}
