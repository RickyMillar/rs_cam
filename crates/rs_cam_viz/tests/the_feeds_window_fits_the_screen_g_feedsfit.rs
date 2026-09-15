//! **G-FEEDSFIT — the Explore window never grows past the screen it sits on.**
//!
//! # The defect
//!
//! Reported 2026-09-15, on a real display. The Feeds Explore window opened
//! TALLER than the screen. Because the window is anchored `CENTER_CENTER`,
//! half of the overflow went off each edge, and the half that went off the
//! top took the title bar with it — and the title bar carries the only close
//! button. The operator could not shut the window they had opened.
//!
//! The cause is that a `Window` sizes itself to its content. The Explore body
//! is a 280-point nomogram over two 180-point mini charts, plus their
//! captions and the strategy row: about 700 points before any frame margin.
//! Nothing in the builder chain said "and no taller than the display".
//!
//! # Why this shape of test
//!
//! A screenshot proves one screen size. The invariant is about every screen
//! size, and specifically about the SMALL ones — the case no developer's
//! monitor reproduces. This arm drives the real `ui::feeds::draw` against a
//! deliberately short viewport and reads back the window's own area rect,
//! which is the rectangle egui would paint and the rectangle the pointer
//! would have to reach.
//!
//! The arms are ordered by what they protect:
//!
//! 1. the window fits inside a short screen;
//! 2. its title bar is ON that screen, so the close button is reachable;
//! 3. it still fits when the screen is short AND narrow;
//! 4. the cap is LIVE — on a screen shorter than the window's own default
//!    the window is smaller than that default, so arms 1–3 are not passing
//!    on a default that happens to fit;
//! 5. the mechanism still bites — an uncapped centred window with a tall
//!    body really does leave a short screen, so arm 4 has something to
//!    prevent;
//! 6. the builders that hold the cap are still in the source.
//!
//! # A note on what the fix changed
//!
//! `vscroll` means the BODY no longer drives the window height at all: the
//! window takes its default or its cap, whichever is smaller, and the body
//! scrolls inside. So "the body is taller than the screen" is no longer
//! measurable from outside the window, and arm 5 reproduces the mechanism
//! with its own window instead — the same shape as
//! `the_extend_mode_really_does_overflow_up4`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::{AppState, FeedsModalState};
use rs_cam_viz::ui::{feeds, tokens};

/// A laptop running at a scale factor that leaves egui a short viewport.
/// Below the body's natural height, which is the point.
const SHORT_SCREEN: egui::Vec2 = egui::Vec2::new(1024.0, 560.0);

/// Short AND narrow — the two mini charts are 360 points each, so this is
/// under the width the side-by-side row wants.
const SMALL_SCREEN: egui::Vec2 = egui::Vec2::new(700.0, 480.0);

/// The window's fixed area id. `ui/feeds/mod.rs` sets it explicitly because
/// the title carries the toolpath name, and egui derives an area id from the
/// title text unless told otherwise.
const WINDOW_ID: &str = "feeds_explore_window";

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx
}

fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;

    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };

    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Explore window fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };

    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("explore_window_fixture.svg"),
        name: "Explore window fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the Explore window fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let toolpath_id = state.session.toolpath_configs()[0].id;
    state.feeds_modal = Some(FeedsModalState {
        toolpath_id,
        explore: None,
    });
    state
}

fn raw_input(size: egui::Vec2) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    }
}

/// Draw the real window for several frames and return its area rect.
///
/// `Resize` settles over a frame or two — it remembers the size it last
/// measured — so a one-frame reading would report the builder's default
/// rather than what the operator sees.
fn window_rect(size: egui::Vec2) -> egui::Rect {
    let ctx = ctx();
    let state = fixture();
    for _ in 0..4 {
        let mut events = Vec::new();
        // `run_ui` hands the closure a root `Ui`. The window is a ctx-level
        // `Area`, so it is drawn through `ui.ctx()`, exactly as `app.rs`
        // draws it.
        let mut output = ctx.run_ui(raw_input(size), |ui| {
            feeds::draw(ui.ctx(), &state, &mut events);
        });
        output.textures_delta.clear();
    }
    ctx.memory(|memory| memory.area_rect(egui::Id::new(WINDOW_ID)))
        .expect("the Explore window drew no area; its id or its draw path moved")
}

// ── arm 1 — the window fits ──────────────────────────────────────────────

#[test]
fn the_explore_window_fits_a_short_screen_g_feedsfit() {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SHORT_SCREEN);
    let window = window_rect(SHORT_SCREEN);
    assert!(
        window.height() <= screen.height() + 0.5,
        "the Explore window asked for {:.1} points of height on a {:.0} point \
         screen. A window taller than the display is centred, so half the \
         overflow leaves the top edge and takes the title bar with it.",
        window.height(),
        screen.height(),
    );
}

// ── arm 2 — the close button is reachable ────────────────────────────────

#[test]
fn the_explore_window_keeps_its_title_bar_on_screen_g_feedsfit() {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SHORT_SCREEN);
    let window = window_rect(SHORT_SCREEN);
    assert!(
        window.min.y >= screen.min.y - 0.5,
        "the Explore window's top edge is at y = {:.1}, above the screen top \
         at y = {:.1}. The title bar carries the only close button, so the \
         operator cannot shut the window.",
        window.min.y,
        screen.min.y,
    );
    assert!(
        window.max.y <= screen.max.y + 0.5,
        "the Explore window's bottom edge is at y = {:.1}, below the screen \
         bottom at y = {:.1}",
        window.max.y,
        screen.max.y,
    );
}

// ── arm 3 — short and narrow ─────────────────────────────────────────────

#[test]
fn the_explore_window_fits_a_small_screen_g_feedsfit() {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SMALL_SCREEN);
    let window = window_rect(SMALL_SCREEN);
    assert!(
        screen.contains_rect(window.expand(-0.5)),
        "the Explore window drew {window:?} on a {screen:?} screen. It must \
         fit both axes: the two mini charts are 360 points each and stack \
         when the row cannot hold them."
    );
}

// ── arm 4 — the cap is live ──────────────────────────────────────────────

/// On a roomy display the window takes its own default size. On a display
/// shorter than that default it must take LESS, which is the cap working.
///
/// Without this arm, arms 1–3 would also pass on a build whose cap had been
/// deleted and whose default simply happened to fit.
#[test]
fn the_cap_shrinks_the_window_below_its_default_g_feedsfit() {
    let roomy = window_rect(egui::Vec2::new(1600.0, 1200.0));
    let small = window_rect(SMALL_SCREEN);
    assert!(
        small.height() < roomy.height() - 0.5,
        "the window drew {:.1} points high on a {:.0} point screen and {:.1} \
         points high on a roomy one. The cap is inert: nothing shrinks the \
         window when the display cannot hold its default.",
        small.height(),
        SMALL_SCREEN.y,
        roomy.height(),
    );
    assert!(
        small.width() < roomy.width() - 0.5,
        "the width cap is inert: {:.1} points on a {:.0} point screen, {:.1} \
         on a roomy one",
        small.width(),
        SMALL_SCREEN.x,
        roomy.width(),
    );
}

// ── arm 5 — the mechanism still bites ────────────────────────────────────

/// A centred window with a tall body and no cap really does leave the screen.
///
/// This is the defect reproduced with the test's own window. If egui ever
/// starts constraining such a window by itself, this arm fails and tells the
/// next reader that the production cap may now be redundant — rather than
/// leaving a cap nobody can justify.
#[test]
fn an_uncapped_centred_window_leaves_a_short_screen_g_feedsfit() {
    const BODY_HEIGHT: f32 = 900.0;
    let ctx = ctx();
    let id = egui::Id::new("feedsfit_uncapped_probe");
    for _ in 0..4 {
        let mut output = ctx.run_ui(raw_input(SHORT_SCREEN), |ui| {
            egui::Window::new("uncapped probe")
                .id(id)
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.allocate_space(egui::vec2(400.0, BODY_HEIGHT));
                });
        });
        output.textures_delta.clear();
    }
    let window = ctx
        .memory(|memory| memory.area_rect(id))
        .expect("the probe window drew no area");
    assert!(
        window.min.y < -0.5,
        "an uncapped {BODY_HEIGHT} point body centred on a {:.0} point screen \
         put its top edge at y = {:.1}, which is ON the screen. egui now \
         constrains this by itself, so re-derive whether ui/feeds/mod.rs \
         still needs its cap.",
        SHORT_SCREEN.y,
        window.min.y,
    );
}

// ── arm 5 — the builders that hold the cap are present ───────────────────

/// A source arm, because arms 1–3 measure a rect and a rect can be made to
/// fit by shrinking the content instead of capping the window. The cap is the
/// invariant; the content will keep growing.
#[test]
fn the_window_declares_its_cap_and_its_scroll_g_feedsfit() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds/mod.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    for builder in [".max_height(", ".max_width(", ".vscroll(true)", ".id("] {
        assert!(
            source.contains(builder),
            "ui/feeds/mod.rs no longer calls `{builder}`. The cap keeps the \
             title bar on screen; the scroll keeps the cap from hiding the \
             charts; the id keeps the window's size across a re-selection."
        );
    }
    assert!(
        source.contains("content_rect()"),
        "the cap must be derived from the screen egui reports, not from a \
         constant. A constant is wrong on every other display."
    );
}
