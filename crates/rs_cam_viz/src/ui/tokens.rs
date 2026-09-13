//! The design tokens. One module owns every colour, space, radius, elevation
//! and text style in the GUI.
//!
//! This is `planning/ui_premium_2026-09-13/DESIGN_SPEC.md` §2 and §3.2 as
//! code. Read the specification for why a value is what it is; this file
//! records only what the value IS.
//!
//! Three rules govern this module.
//!
//! 1. **Every colour in the product is a token or it does not ship**
//!    (`DESIGN_SPEC.md` §2.10). `Color32::from_rgb` belongs here and in
//!    `render/colors.rs`, and nowhere else. The sentry
//!    `tests/panels_read_the_token_module_up1.rs` counts the rest and drives
//!    the count down package by package.
//! 2. **A verdict colour never carries category, order or decoration**
//!    (§2.6). The diagram palette and the data scales exist so a drawing
//!    never has to borrow `OK` or `DANGER`.
//! 3. **Colour is never the only channel** (§2.6). Every verdict also carries
//!    a glyph, which is why [`glyph_ok`] and its siblings live beside the
//!    colours rather than in a component.
//!
//! `ui/theme.rs` keeps its original public names and re-exports from here, so
//! no call site had to change when this module arrived.

use egui::{Color32, FontFamily, FontId, TextStyle, epaint::Shadow};

// ===========================================================================
// §2.1 Grid
// ===========================================================================

/// The base unit. Every spacing step is a multiple of this, except the two
/// row-rhythm tokens below, which are deliberately off the scale.
pub const GRID: f32 = 4.0;

/// Flush.
pub const SPACE_0: f32 = 0.0;
/// Inside a chip, between a glyph and its word.
pub const SPACE_1: f32 = 2.0;
/// Between rows of one group. Also `item_spacing`.
pub const SPACE_2: f32 = 4.0;
/// Between groups, and panel side padding.
pub const SPACE_3: f32 = 8.0;
/// Between sections.
pub const SPACE_4: f32 = 12.0;
/// Panel top padding, window inner margin.
pub const SPACE_5: f32 = 16.0;
/// Between major blocks on a page.
pub const SPACE_6: f32 = 24.0;
/// Above a page-level action row.
pub const SPACE_7: f32 = 32.0;

/// A parameter row, a key-value row, a table row.
///
/// A parameter row sets a FIXED line box of 22 points. It does not derive its
/// height from `item_spacing` plus a text ascent, because that drifts with the
/// font and with any caption inside the row. Eight parameter rows occupy
/// exactly 176 points.
///
/// Operator ruling 2026-09-13: at 26 points an inspector reads as a settings
/// dialog; at 22 it reads as a control panel.
pub const ROW_DENSE: f32 = 22.0;

/// A row carrying a control, and the minimum height of any control.
pub const ROW_ACTION: f32 = 26.0;

// ===========================================================================
// §2.2 Radius
// ===========================================================================

/// Chips, buttons, inputs, combo boxes, swatches.
pub const RADIUS_SM: u8 = 4;
/// Cards, windows, popovers, banners.
pub const RADIUS_MD: u8 = 8;

// ===========================================================================
// §2.4 The neutral ramp
// ===========================================================================
//
// The bias is cold, toward blue-green, at very low chroma (HSL saturation 6 %
// to 10 %, hue 205). It replaces the accidental violet that came from two
// unrelated literals (`AUDIT.md` D-07).
//
// Contrast is COMPUTED, not estimated (WCAG 2.1 relative luminance). The
// four text rungs and their worst ratio over the four surfaces:
//
//   INK_50  TEXT_FAINT   3.28
//   INK_65  TEXT_MUTED   4.57
//   INK_80  TEXT_BODY    7.23
//   INK_95  TEXT_STRONG  11.15

/// Pure ground, behind the scrim.
pub const INK_00: Color32 = Color32::from_rgb(0x0F, 0x11, 0x13);
/// [`SURFACE_SUNKEN`].
pub const INK_05: Color32 = Color32::from_rgb(0x15, 0x17, 0x1A);
/// [`SURFACE_BASE`].
pub const INK_10: Color32 = Color32::from_rgb(0x1B, 0x1E, 0x22);
/// [`SURFACE_RAISED`].
pub const INK_15: Color32 = Color32::from_rgb(0x22, 0x26, 0x2B);
/// [`SURFACE_OVERLAY`].
pub const INK_20: Color32 = Color32::from_rgb(0x28, 0x2D, 0x33);
/// Hairline rules, disabled fills.
pub const INK_25: Color32 = Color32::from_rgb(0x31, 0x37, 0x3E);
/// Control borders, chip strokes.
pub const INK_35: Color32 = Color32::from_rgb(0x45, 0x4D, 0x56);
/// [`TEXT_FAINT`] — units, provenance.
pub const INK_50: Color32 = Color32::from_rgb(0x73, 0x7C, 0x87);
/// [`TEXT_MUTED`] — secondary text, labels.
pub const INK_65: Color32 = Color32::from_rgb(0x8C, 0x95, 0x9F);
/// [`TEXT_BODY`] — default label text.
pub const INK_80: Color32 = Color32::from_rgb(0xB4, 0xBC, 0xC5);
/// [`TEXT_STRONG`] — values, names, headings.
pub const INK_95: Color32 = Color32::from_rgb(0xE2, 0xE7, 0xEC);

// ---- Text rungs, named by role rather than by ramp position --------------

/// Units and provenance only.
///
/// FORBIDDEN below 12 points and FORBIDDEN for any sentence. It exists for a
/// unit suffix beside the number it belongs to.
pub const TEXT_FAINT: Color32 = INK_50;
/// Secondary text and labels.
pub const TEXT_MUTED: Color32 = INK_65;
/// The default label text.
pub const TEXT_BODY: Color32 = INK_80;
/// Values, names and headings.
pub const TEXT_STRONG: Color32 = INK_95;

// ===========================================================================
// §2.3 Elevation
// ===========================================================================
//
// Four planes. Elevation is carried by FILL and SHADOW, never by a border
// alone.

/// Viewport ground, text-input wells, timeline track.
pub const SURFACE_SUNKEN: Color32 = INK_05;
/// Panels, status bar, menu bar.
pub const SURFACE_BASE: Color32 = INK_10;
/// Cards, list rows, grouped fields.
pub const SURFACE_RAISED: Color32 = INK_15;
/// Windows, popovers, toasts, menus. The only surface that casts a shadow.
pub const SURFACE_OVERLAY: Color32 = INK_20;

/// The product's ONLY shadow.
pub const SHADOW_OVERLAY: Shadow = Shadow {
    offset: [0, 8],
    blur: 24,
    spread: 0,
    color: Color32::from_rgba_premultiplied(0, 0, 0, 110),
};

/// The full-viewport rect a modal paints behind its window.
pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(8, 9, 11, 140);

// ===========================================================================
// §2.5 The accent
// ===========================================================================
//
// ONE accent. It carries selection, keyboard focus and the primary button
// fill, and nothing else. The accent must NEVER carry a verdict.

/// Selection, keyboard focus, primary button fill.
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x9D, 0xD9);
/// Selected row fill, active tab fill.
pub const ACCENT_QUIET: Color32 = Color32::from_rgb(0x2C, 0x44, 0x59);
/// Primary button, pressed.
pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(0x4A, 0x85, 0xBB);

// ===========================================================================
// §2.6 Semantic colours
// ===========================================================================
//
// Five roles. Nothing else in the product may use these hues. Each role has a
// text tone here and a quiet chip fill in the TINT_* family below; §2.7 rules
// that the two are one family, because a chip is a small tinted surface.

/// Within a band, current, clear, pass. Glyph `✓`.
pub const OK: Color32 = Color32::from_rgb(0x5F, 0xBF, 0x7A);
/// Stale, waiting, elevated, review. Glyph `!`.
///
/// CAUTION means a measurement came back and it needs review. "Not run" is
/// [`UNKNOWN`], never this.
pub const CAUTION: Color32 = Color32::from_rgb(0xE0, 0xA8, 0x3C);
/// Exceeds, collision, error, refusal. Glyph `✕`.
pub const DANGER: Color32 = Color32::from_rgb(0xE8, 0x7B, 0x77);
/// Informational. The accent reused. Glyph none.
pub const INFO: Color32 = ACCENT;
/// NOT MEASURED. Glyph `—`, and never with a number.
///
/// The most important addition in this section. Today "not measured" is drawn
/// as an em dash in whatever grey is nearby, so a gate that abstained and a
/// gate that passed look alike at a glance.
pub const UNKNOWN: Color32 = Color32::from_rgb(0x8D, 0x9A, 0xA8);

// ---- §2.6 / §2.7 the chip fills, which are also the surface tints --------

/// Chip fill and tinted ground for [`OK`].
pub const TINT_OK: Color32 = Color32::from_rgb(0x1C, 0x33, 0x23);
/// Chip fill and tinted ground for [`CAUTION`].
pub const TINT_CAUTION: Color32 = Color32::from_rgb(0x3A, 0x2E, 0x12);
/// Chip fill and tinted ground for [`DANGER`].
pub const TINT_DANGER: Color32 = Color32::from_rgb(0x3A, 0x1D, 0x1E);
/// Chip fill and tinted ground for [`INFO`].
pub const TINT_INFO: Color32 = Color32::from_rgb(0x1E, 0x2E, 0x3D);
/// Chip fill and tinted ground for [`UNKNOWN`].
pub const TINT_UNKNOWN: Color32 = Color32::from_rgb(0x23, 0x28, 0x2E);

// ---- §2.6 rule 3: colour is never the only channel -----------------------

/// The glyph that accompanies [`OK`].
pub const GLYPH_OK: &str = "✓";
/// The glyph that accompanies [`CAUTION`].
pub const GLYPH_CAUTION: &str = "!";
/// The glyph that accompanies [`DANGER`].
pub const GLYPH_DANGER: &str = "✕";
/// The glyph that accompanies [`UNKNOWN`].
pub const GLYPH_UNKNOWN: &str = "—";

// ===========================================================================
// §2.7 Structure
// ===========================================================================

/// Every rule and separator. Replaces 11 different values over 20 sites.
pub const HAIRLINE: Color32 = INK_25;
/// Control borders and chip strokes.
pub const BORDER: Color32 = INK_35;
/// The ground inside a text input or a read-only value well.
pub const INPUT_WELL: Color32 = INK_05;

// ===========================================================================
// §2.8 The diagram palette
// ===========================================================================
//
// The operation preview thumbnails, the entry diagram and the height diagram
// are DRAWINGS, not UI. They carry 41 literals over 5 intents today, all
// outside `theme.rs`, and they are why TEXT_FAINT collides with an op-diagram
// dim colour at 11 sites.

/// The drawing's ground.
pub const DIAGRAM_CANVAS: Color32 = Color32::from_rgb(0x14, 0x17, 0x1A);
/// The path being described.
///
/// Takes the accent because a diagram is explanatory, not a verdict. It must
/// never take [`OK`] or [`DANGER`].
pub const DIAGRAM_INK: Color32 = ACCENT;
/// Stock or material body.
pub const DIAGRAM_MATERIAL: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x36);
/// The cutter body.
pub const DIAGRAM_TOOL: Color32 = INK_65;
/// Construction lines and retired geometry.
pub const DIAGRAM_DIM: Color32 = INK_35;

// ===========================================================================
// §2.9 Data scales
// ===========================================================================
//
// A data scale is derived from ONE hue by lightness, never assembled from
// separate hues, because a category wheel spends the semantic palette.
//
// §2.9 names these three scales and their step counts but gives no values.
// UP1 derived them from the stated rule and verified every step against the
// four surfaces. The derivation is in the commit that added this file.

/// Six steps, cool blue to cool cyan, hue 210 to 186 with lightness rising.
///
/// Colours span kinds, which today carry 17 distinct values over 24 sites and
/// are the worst drift in the product.
///
/// Every step clears 3:1 against all four surfaces — the WCAG floor for a
/// graphical object — with the darkest at 3.44. Adjacent steps separate by
/// 1.15 to 1.29.
pub const SPAN_SCALE: [Color32; 6] = [
    Color32::from_rgb(0x44, 0x82, 0xC1),
    Color32::from_rgb(0x5A, 0x98, 0xC6),
    Color32::from_rgb(0x70, 0xAC, 0xCB),
    Color32::from_rgb(0x86, 0xBE, 0xD1),
    Color32::from_rgb(0x9B, 0xCD, 0xD8),
    Color32::from_rgb(0xAF, 0xDA, 0xDE),
];

/// Four steps, hue 205 to 175 with lightness rising.
///
/// The feeds charts. Every step clears 3:1 on all four surfaces; the darkest
/// is 3.75.
///
/// §2.9 named the simulation signal strip as this scale's other consumer and
/// that was WRONG, measured during the UP5–UP7 migration: **the strip has six
/// tracks and this scale has four steps.** The strip uses [`SPAN_SCALE`],
/// which has six. Extending this one to six was the alternative and was
/// rejected — four is right for a chart, where more series than that is a
/// legibility problem rather than a palette problem.
pub const CHART_SERIES: [Color32; 4] = [
    Color32::from_rgb(0x3D, 0x8B, 0xC2),
    Color32::from_rgb(0x64, 0xB0, 0xC9),
    Color32::from_rgb(0x89, 0xCB, 0xD1),
    Color32::from_rgb(0xAD, 0xDC, 0xD8),
];

/// The compute lane is idle.
pub const LANE_IDLE: Color32 = INK_65;
/// The compute lane is queued.
pub const LANE_QUEUED: Color32 = ACCENT;
/// The compute lane is running.
pub const LANE_RUNNING: Color32 = CAUTION;
/// The compute lane is cancelling.
pub const LANE_CANCELLING: Color32 = DANGER;

/// The four lane states in their natural order, retuned onto the ramp.
pub const LANE_SCALE: [Color32; 4] = [LANE_IDLE, LANE_QUEUED, LANE_RUNNING, LANE_CANCELLING];

// ===========================================================================
// §3.2 The type scale
// ===========================================================================
//
// Eight rungs. FIVE map onto egui's built-in `TextStyle` slots and apply
// automatically. THREE do not exist as global styles and are helpers here,
// because a custom `TextStyle::Name` is a legal map key that `ui.label()`
// never reads (`DESIGN_SPEC.md` §10.3).

/// `TextStyle::Heading`. Panel title, modal step title.
pub const SIZE_HEADING: f32 = 15.0;
/// `TextStyle::Body`. Default label and sentence.
pub const SIZE_BODY: f32 = 13.0;
/// `TextStyle::Button`. Every button.
pub const SIZE_BUTTON: f32 = 13.0;
/// `TextStyle::Monospace`. EVERY measured value.
pub const SIZE_NUMERIC: f32 = 13.0;
/// `TextStyle::Small`. Units, provenance, supporting sentence.
///
/// Raising this slot from 9 to 11 is the single highest-leverage line in the
/// whole specification. All 516 `.small()` call sites read it, so one
/// assignment in [`apply`] lifts the product's entire caption layer off the
/// floor without editing a single call site.
pub const SIZE_CAPTION: f32 = 11.0;
/// Helper rung. A window title, a page title. Rare.
pub const SIZE_DISPLAY: f32 = 20.0;
/// Helper rung. A section header.
pub const SIZE_SUBHEAD: f32 = 12.0;
/// Helper rung. Chip text ONLY. The one rung allowed below 11 points.
pub const SIZE_MICRO: f32 = 10.0;

/// Extra tracking on [`SIZE_MICRO`], in points. Applied per call site.
pub const MICRO_TRACKING: f32 = 0.8;

/// Additional leading, in points, applied globally by [`apply`].
///
/// The specification asks for a 17-point line on 13-point body and a 15-point
/// line on 11-point caption. Both are the same `+4`, so one global value
/// serves both. This field arrived in egui 0.36 and is why UP0 came first;
/// `DESIGN_SPEC.md` §10.2 records the per-call-site workaround it replaces.
pub const EXTRA_LINE_SPACING: f32 = 4.0;

// ---- Font family names ---------------------------------------------------
//
// egui reaches a weight through a NAMED family, not through a weight axis.
// `app.rs::configure_fonts` registers these names.

/// Inter Medium. Buttons.
pub const FAMILY_MEDIUM: &str = "inter_medium";
/// Inter SemiBold. Headings, subheads and chips.
pub const FAMILY_SEMIBOLD: &str = "inter_semibold";
/// JetBrains Mono Medium. An emphasised measured value.
pub const FAMILY_MONO_MEDIUM: &str = "mono_medium";

/// The `Display` rung: 20 points, SemiBold.
#[must_use]
pub fn font_display() -> FontId {
    FontId::new(SIZE_DISPLAY, FontFamily::Name(FAMILY_SEMIBOLD.into()))
}

/// The `Subhead` rung: 12 points, SemiBold. Use it for a section header.
#[must_use]
pub fn font_subhead() -> FontId {
    FontId::new(SIZE_SUBHEAD, FontFamily::Name(FAMILY_SEMIBOLD.into()))
}

/// The `Micro` rung: 10 points, SemiBold. Chip text only.
///
/// The caller adds [`MICRO_TRACKING`] and upper-cases the string; neither is
/// expressible in a `FontId`.
#[must_use]
pub fn font_micro() -> FontId {
    FontId::new(SIZE_MICRO, FontFamily::Name(FAMILY_SEMIBOLD.into()))
}

/// The `Numeric` rung: 13 points, JetBrains Mono. EVERY measured value.
#[must_use]
pub fn font_numeric() -> FontId {
    FontId::new(SIZE_NUMERIC, FontFamily::Monospace)
}

// ===========================================================================
// Applying the tokens
// ===========================================================================

/// Write the whole token set onto a context's `Style`.
///
/// This sets BOTH themes through `all_styles_mut`. The pair this replaced set
/// `Visuals` and `Style` separately, which writes one theme only
/// (`DESIGN_SPEC.md` §10.3).
///
/// It does NOT load fonts. `app.rs::configure_fonts` owns that, because the
/// font data is `include_bytes!` from the binary and belongs beside the
/// other asset loading.
pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        apply_to_style(style);
    });
}

/// The body of [`apply`], separated so a test can drive it on a bare `Style`.
pub fn apply_to_style(style: &mut egui::Style) {
    // ---- §3.2 the five built-in text styles ----
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(SIZE_HEADING, FontFamily::Name(FAMILY_SEMIBOLD.into())),
        ),
        (
            TextStyle::Body,
            FontId::new(SIZE_BODY, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(SIZE_BUTTON, FontFamily::Name(FAMILY_MEDIUM.into())),
        ),
        (
            TextStyle::Monospace,
            FontId::new(SIZE_NUMERIC, FontFamily::Monospace),
        ),
        (
            TextStyle::Small,
            FontId::new(SIZE_CAPTION, FontFamily::Proportional),
        ),
    ]
    .into();

    // ---- §2.1 grid ----
    style.spacing.item_spacing = egui::vec2(SPACE_2, SPACE_2);
    style.spacing.interact_size.y = ROW_ACTION;
    style.spacing.extra_text_line_spacing = EXTRA_LINE_SPACING;

    // ---- `AUDIT.md` D-16: NOT closed globally, and here is why ----
    //
    // UP1 set `wrap_mode = Some(Wrap)` to stop `Extend` growing a `Ui` past
    // its panel. It was WRONG and the screenshots showed it: a label in a
    // narrow grid column wraps MID-WORD. The inspector read "Spoilb / oard:",
    // "Retrac / t (R):" and "Dressu / p".
    //
    // egui's own `None` resolves per layout — `Wrap` in a vertical one,
    // `Extend` in a horizontal one — and that default is right for a label
    // whose column sizes to its content. `Truncate` is no better: it hides
    // the end of a word the operator has to read.
    //
    // So the global default stays UNSET and D-16 is closed per component
    // instead, where the component knows whether its text is a label (never
    // wraps) or a sentence (always does). `KeyValueRow`'s trailing slot is
    // the worked example.
    style.wrap_mode = None;

    let v = &mut style.visuals;

    // ---- §2.3 elevation ----
    v.panel_fill = SURFACE_BASE;
    v.window_fill = SURFACE_OVERLAY;
    v.extreme_bg_color = SURFACE_SUNKEN;
    v.faint_bg_color = SURFACE_RAISED;
    v.window_shadow = SHADOW_OVERLAY;
    v.popup_shadow = SHADOW_OVERLAY;

    // ---- §2.2 radius ----
    v.window_corner_radius = RADIUS_MD.into();
    v.menu_corner_radius = RADIUS_MD.into();

    // ---- §2.4 / §2.7 widget states ----
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = RADIUS_SM.into();
    }

    v.widgets.noninteractive.bg_fill = SURFACE_RAISED;
    v.widgets.noninteractive.weak_bg_fill = SURFACE_RAISED;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, HAIRLINE);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_BODY);

    v.widgets.inactive.bg_fill = SURFACE_RAISED;
    v.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_BODY);

    v.widgets.hovered.bg_fill = SURFACE_OVERLAY;
    v.widgets.hovered.weak_bg_fill = SURFACE_OVERLAY;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, INK_50);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_STRONG);

    v.widgets.active.bg_fill = ACCENT_QUIET;
    v.widgets.active.weak_bg_fill = ACCENT_QUIET;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT_STRONG);

    v.widgets.open.bg_fill = SURFACE_OVERLAY;
    v.widgets.open.weak_bg_fill = SURFACE_OVERLAY;
    v.widgets.open.bg_stroke = egui::Stroke::new(1.0, BORDER);
    v.widgets.open.fg_stroke = egui::Stroke::new(1.0, TEXT_STRONG);

    // ---- §2.5 the accent, and only these three things ----
    v.selection.bg_fill = ACCENT_QUIET;
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;

    // ---- §2.7 structure ----
    v.window_stroke = egui::Stroke::new(1.0, BORDER);
    v.override_text_color = None;
    v.warn_fg_color = CAUTION;
    v.error_fg_color = DANGER;
}

/// Load the two families and register the three named weights.
///
/// Kept beside the tokens rather than in `app.rs` so the family names the
/// `Style` asks for and the family names registered here cannot drift apart.
/// They are the same constants, and
/// `tests/panels_read_the_token_module_up1.rs` lays out text in every style
/// to prove it: epaint panics with "FontFamily::.. is not bound to any
/// fonts" at the first galley, which would otherwise be a crash on launch
/// that no other gate catches.
pub fn apply_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    use std::sync::Arc;

    let mut fonts = FontDefinitions::default();

    // `DESIGN_SPEC.md` §3.1. Inter carries the prose and JetBrains Mono
    // carries every measured value. Both are SIL OFL 1.1 and both ship as
    // `.ttf` in `assets/fonts/`, loaded the same way the two Noto symbol
    // fonts already were. Neither adds a Cargo dependency.
    //
    // egui reaches a WEIGHT through a named family, not a weight axis, so
    // Medium and SemiBold are registered as families of their own. Inter
    // replaces Ubuntu-Light because a light face has no room below it and
    // `.strong()` then has to do all the work.
    for (name, bytes) in [
        (
            "inter_regular",
            &include_bytes!("../../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            "inter_medium",
            &include_bytes!("../../assets/fonts/Inter-Medium.ttf")[..],
        ),
        (
            "inter_semibold",
            &include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
        (
            "mono_regular",
            &include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf")[..],
        ),
        (
            "mono_medium",
            &include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf")[..],
        ),
        // NotoSansSymbols covers geometric shapes (▶●○), arrows (→),
        // math (≤), checkmarks (✓)
        (
            "noto_symbols",
            &include_bytes!("../../assets/fonts/NotoSansSymbols-Regular.ttf")[..],
        ),
        // NotoSansSymbols2 covers braille (⠿), dingbats (✗✅❌), and
        // extended symbols
        (
            "noto_symbols2",
            &include_bytes!("../../assets/fonts/NotoSansSymbols2-Regular.ttf")[..],
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    // The symbol fonts are the tail of EVERY family, in their original
    // order. egui's own defaults stay behind our faces rather than being
    // replaced, so nothing that rendered before loses its glyph.
    let symbols = ["noto_symbols".to_owned(), "noto_symbols2".to_owned()];

    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "inter_regular".to_owned());
        family.extend(symbols.iter().cloned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.insert(0, "mono_regular".to_owned());
        family.extend(symbols.iter().cloned());
    }

    // The three named weight families. Each falls back through the
    // proportional stack, so a glyph Inter lacks still renders.
    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family_name, head) in [
        (FAMILY_MEDIUM, "inter_medium"),
        (FAMILY_SEMIBOLD, "inter_semibold"),
        (FAMILY_MONO_MEDIUM, "mono_medium"),
    ] {
        let mut stack = vec![head.to_owned()];
        stack.extend(proportional.iter().cloned());
        fonts
            .families
            .insert(FontFamily::Name(family_name.into()), stack);
    }

    ctx.set_fonts(fonts);
}

// ===========================================================================
// §4.13 rulings — values the component set needs and §4 did not give
// ===========================================================================

/// The minimum width of a value well, in points (ruling R7).
///
/// §4.6 requires a minimum so a column of values with different digit counts
/// keeps one right edge, and gives no number. 58 is the value the drawn
/// specimen used, and the operator approved that specimen's density.
pub const WELL_MIN_WIDTH: f32 = 58.0;

/// The `BodyStrong` rung: 13 points, Inter Medium (ruling R6).
///
/// The emphasis rung INSIDE body text. Medium rather than SemiBold, because
/// SemiBold at 13 would compete with `Heading` at 15 SemiBold and flatten the
/// hierarchy.
#[must_use]
pub fn font_body_strong() -> FontId {
    FontId::new(SIZE_BODY, FontFamily::Name(FAMILY_MEDIUM.into()))
}

/// Lift a surface one step for hover (ruling R9).
///
/// "One ramp step" is the next SURFACE token, not the next ink: the ramp is
/// unevenly spaced, so a lightness delta and a token step are different
/// things. The ladder clamps at `SURFACE_OVERLAY`, and a transparent fill —
/// a quiet button, an odd zebra row — lifts to `SURFACE_RAISED`.
///
/// Any other fill is returned unchanged. A tinted chip signals hover with its
/// stroke instead, because lifting a tint would walk it off its own hue.
#[must_use]
pub fn hover_lift(fill: Color32) -> Color32 {
    if fill == Color32::TRANSPARENT {
        return SURFACE_RAISED;
    }
    match fill {
        c if c == SURFACE_SUNKEN => SURFACE_BASE,
        c if c == SURFACE_BASE => SURFACE_RAISED,
        c if c == SURFACE_RAISED => SURFACE_OVERLAY,
        c if c == SURFACE_OVERLAY => SURFACE_OVERLAY,
        other => other,
    }
}

// ===========================================================================
// §5 Motion
// ===========================================================================
//
// Durations in SECONDS, because that is what `animate_bool_with_time` takes.
//
// Every helper in `components::motion` uses
// `animate_bool_with_time_and_easing`. The plain `_with_time` call is
// hardcoded to linear (`DESIGN_SPEC.md` §10.4), and linear motion is the
// single most recognisable sign that nobody chose the easing.

/// A hover or press state change.
pub const MOTION_FAST: f32 = 0.12;
/// A disclosure opening, a panel swapping, a chip changing verdict.
pub const MOTION_BASE: f32 = 0.18;
/// A toast arriving, a modal scrim fading in.
pub const MOTION_SLOW: f32 = 0.24;

// ---- Component dimensions (ruling R20) ---------------------------------
//
// §4 gives these as bare numbers. They are named here for the same reason
// every colour is: a number repeated at call sites drifts, and a reader
// cannot tell a considered 34 from a typed one. §2.10's sentry counts only
// COLOUR literals, so nothing forced this — it is consistency, not a gate.

/// A chip's minimum width, so a COLUMN of chips aligns (§4.3).
pub const CHIP_MIN_WIDTH: f32 = 34.0;

/// The height of a value well INSIDE a `ROW_DENSE` row (§4.6).
///
/// 18 inside 22 leaves 2 points of air above and below, so a column of wells
/// reads as a stack rather than as a list of boxes.
pub const WELL_HEIGHT: f32 = 18.0;

/// The widest an empty state may be (§4.8). Centred in the available space.
pub const EMPTY_STATE_MAX_WIDTH: f32 = 280.0;

/// The outline glyph above an empty state's headline (§4.8), in `INK_35`.
pub const EMPTY_STATE_GLYPH_SIZE: f32 = 24.0;

/// A token's channels as `0.0..=1.0`, for a wgpu clear value.
///
/// Measured 2026-09-14: the 3D pass's target consumes this value DIRECTLY as
/// the 8-bit output — a clear of `(0.102, 0.102, 0.149)` displayed as exactly
/// `(26, 26, 38)` — so no sRGB transfer is applied and a plain divide by 255
/// is correct. Sampled from `shot_18_stock_panel_up3.png` rather than
/// reasoned about, because the two conventions differ by a factor of three
/// and guessing would have been visible.
#[must_use]
pub fn as_unit_rgb(c: Color32) -> [f64; 3] {
    [
        f64::from(c.r()) / 255.0,
        f64::from(c.g()) / 255.0,
        f64::from(c.b()) / 255.0,
    ]
}

/// Convert a linear `[f32; 3]` from `render::colors` into a `Color32`.
///
/// The 3D palette lives in `render/colors.rs` as float triples because that
/// is what the vertex buffers take. A panel that draws a swatch for a
/// toolpath needs the same colour as an egui value, and hand-writing the
/// conversion at each site is how a raw `Color32::from_rgb` ends up in a
/// panel that has no colour of its own.
#[must_use]
pub fn from_linear_rgb(c: [f32; 3]) -> Color32 {
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(to_u8(c[0]), to_u8(c[1]), to_u8(c[2]))
}

/// [`ACCENT`] at a given alpha, for a highlight wash over a diagram.
#[must_use]
pub fn accent_wash(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), alpha)
}

// ---- The axis triple (ruling R22) ---------------------------------------
//
// `AUDIT.md` §2.10 names `#DC3C3C` serving BOTH error text and the viewport
// X-axis gizmo as one of four load-bearing collisions, and it is the one
// this programme's first principle exists for: an axis that is the same red
// as an error.
//
// Two agents migrating this crate independently reached the same wall, and
// both stopped rather than force it: **no scale in §2.9 carries three
// separable HUES.** Every data scale here is one hue by lightness, by
// design, because a category wheel spends the semantic palette.
//
// The ruling is that an AXIS IS THE EXCEPTION, and a narrow one. X red,
// Y green, Z blue is a convention every CAD operator reads without being
// taught, and re-coding it by lightness would cost more than it saves. So
// the axes keep their hues, and what the token set buys is the thing the
// audit actually complained about: these are SEPARATE constants from the
// verdict roles, at different values, so moving one cannot move the other.
//
// The limitation is real and stated rather than hidden: `AXIS_X` sits near
// `DANGER` and `AXIS_Y` near `OK` in hue, because the convention puts them
// there. What keeps them from being confused is CONTEXT — an axis gizmo is
// drawn in the 3D viewport and a verdict on a chip in a panel, and the two
// are never adjacent. An axis colour must NEVER appear in a panel.

/// The X axis. Distinct from [`DANGER`]; 4.72 on the viewport ground.
pub const AXIS_X: Color32 = Color32::from_rgb(0xD6, 0x5C, 0x55);
/// The Y axis. Distinct from [`OK`]; 7.89 on the viewport ground.
pub const AXIS_Y: Color32 = Color32::from_rgb(0x6F, 0xBF, 0x4A);
/// The Z axis. 5.41 on the viewport ground.
pub const AXIS_Z: Color32 = Color32::from_rgb(0x5B, 0x8D, 0xE0);
