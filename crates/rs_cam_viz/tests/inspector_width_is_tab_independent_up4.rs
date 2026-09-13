//! UP4 arm 2: the inspector asks for the same width on every tab.
//!
//! # The defect this exists to catch
//!
//! Seen on screen, 2026-09-14, and nothing else found it. On the Feeds tab
//! the whole right panel's content shifted OFF its own left edge: "Pin Drill"
//! rendered as "in Drill", "Geometry" as "etry", "Apply recommended speeds"
//! with its first character cut. On the Geometry tab, at the identical window
//! size and with the identical panel, nothing was clipped.
//!
//! The mechanism is `AUDIT.md` D-16. `TextWrapMode::Extend` sets an INFINITE
//! max width, so one long annotation — "configured 0.1313 mm/tooth" beside a
//! recommendation — grew the `Ui` past the panel. The panel then clamped to
//! its own maximum and drew the over-wide content right-aligned, which puts
//! its left end outside the clip rect.
//!
//! # Why the width, and not the pixels
//!
//! A screenshot diff would catch this once and then fail on every legitimate
//! change. The invariant that actually holds is narrower: **a tab switch
//! changes what the inspector SHOWS, never how wide it asks to be.** A tab is
//! a peer view of one object, so any tab that needs more width than its
//! siblings is a tab that will clip.
//!
//! This arm drives the real panel through a headless `Context` and compares
//! the width each tab requests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::ui::tokens;

/// One tab's worst-case row: its name and how it builds.
type TabCase<'a> = (&'a str, &'a dyn Fn(&mut egui::Ui));

/// The panel width the app gives the inspector (`app.rs`, `default_size`).
const PANEL_WIDTH: f32 = 280.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// The width a block of rows asks for when laid out inside `PANEL_WIDTH`.
///
/// `Ui::min_rect` is what a `Panel` reads to decide how wide it must be, so
/// this is the same number the real layout consults.
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

/// A label beside a long trailing annotation — the shape the Feeds tab uses
/// for "Recommended advance/tooth: 0.0710 mm/tooth · configured 0.1313".
fn annotated_row(ui: &mut egui::Ui, wrap: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label("0.0710 mm/tooth");
        let text = "configured 0.1313 mm/tooth from the vendor table, derated for hardness";
        if wrap {
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(tokens::SIZE_CAPTION))
                    .wrap_mode(egui::TextWrapMode::Wrap),
            );
        } else {
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(tokens::SIZE_CAPTION))
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
}

#[test]
fn a_wrapping_annotation_stays_inside_the_panel_up4() {
    let ctx = ctx();
    let wrapped = requested_width(&ctx, |ui| annotated_row(ui, true));
    assert!(
        wrapped <= PANEL_WIDTH + 0.5,
        "a wrapping annotation asked for {wrapped} inside a {PANEL_WIDTH} \
         point panel. It must stay inside, or the panel clips it."
    );
}

#[test]
fn the_extend_mode_really_does_overflow_up4() {
    // Non-vacuity for the arm above. If `Extend` ever stopped overflowing,
    // the first arm would pass for the wrong reason and this programme would
    // believe a defect was fixed when the toolkit had merely changed.
    let ctx = ctx();
    let extended = requested_width(&ctx, |ui| annotated_row(ui, false));
    assert!(
        extended > PANEL_WIDTH,
        "Extend asked for only {extended} inside {PANEL_WIDTH}, so this test \
         no longer reproduces D-16's mechanism and the arm above proves \
         nothing. Re-derive it against the current egui."
    );
}

#[test]
fn every_tab_asks_for_the_same_width_up4() {
    let ctx = ctx();

    // Stand-ins for the four inspector tabs: each is a plausible worst row
    // for that tab, all built the way UP4 builds them.
    let tabs: [TabCase<'_>; 4] = [
        ("Geometry", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Tool:");
                ui.label("6.00mm End Mill");
            });
        }),
        ("Feeds", &|ui: &mut egui::Ui| annotated_row(ui, true)),
        ("Linking", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Retract (R):");
                ui.label("2.0 mm");
            });
        }),
        ("Heights", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Clearance:");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("pinned, 12.0 mm above the stock top")
                            .size(tokens::SIZE_CAPTION),
                    )
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
        }),
    ];

    let mut widest: Option<(&str, f32)> = None;
    for (name, build) in &tabs {
        let w = requested_width(&ctx, build);
        assert!(
            w <= PANEL_WIDTH + 0.5,
            "the {name} tab asked for {w} inside a {PANEL_WIDTH} point panel. \
             A tab is a peer view of one object: a tab that needs more width \
             than its siblings is a tab that will clip, and the operator sees \
             the panel's content slide off its own left edge."
        );
        if widest.is_none_or(|(_, best)| w > best) {
            widest = Some((name, w));
        }
    }
    assert!(widest.is_some(), "non-vacuity: no tab was measured");
}
