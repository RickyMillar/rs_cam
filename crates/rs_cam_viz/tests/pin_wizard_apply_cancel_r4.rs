//! R4 (2026-10-12): the pin wizard's Apply and Cancel contract.
//!
//! The operator: "I think that this might almost want a alignment pin
//! wizard for more control / set up." The wizard ("Pin setup..." of the
//! stock panel) works on its OWN draft.
//!
//! 1. Cancel closes the wizard and changes nothing: no event, the same
//!    pins, the same tools, the same toolpaths.
//! 2. Apply on the summary raises ONE `PinWizardApply` with the wizard's
//!    pins and its queued drills. The pin door applies it (its controller
//!    test is `the_wizard_apply_adds_the_drill_and_writes_once_r4`).
//! 3. At 1600 x 900 every step paints inside the screen, and step 3 shows
//!    the plan view.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
use rs_cam_viz::state::AppState;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::pin_wizard::{self, PinWizardState};

const SCREEN: egui::Vec2 = egui::vec2(1600.0, 900.0);

fn square_model(id: usize, name: &str, lo: f64, hi: f64) -> LoadedModel {
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::new(vec![
            P2::new(lo, lo),
            P2::new(hi, lo),
            P2::new(hi, hi),
            P2::new(lo, hi),
        ])])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        enriched_mesh: None,
        units: None,
        winding_report: None,
        load_error: None,
    }
}

fn state() -> AppState {
    let stock = StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        alignment_pins: vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ],
        ..StockConfig::default()
    };
    let mut drill = ToolConfig::new_default(ToolId(3), ToolType::VBit);
    drill.diameter = 6.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .model(square_model(0, "stock_outline.dxf", 0.0, 65.0))
        .model(square_model(1, "back_cavity.dxf", 12.0, 53.0))
        .tool(drill);
    let _ = builder.add_setup("Front".to_owned(), FaceUp::Top);
    let _ = builder.add_setup("Back".to_owned(), FaceUp::Bottom);
    let mut state = AppState::new();
    state.session = builder.build();
    state.panels.pin_wizard = Some(PinWizardState::open(&state.session));
    state
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    rs_cam_viz::ui::tokens::apply(&ctx);
    rs_cam_viz::ui::tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One frame of the wizard on a 1600 x 900 screen: the painted texts and
/// the events.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    input: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect)>, Vec<AppEvent>) {
    let raw = egui::RawInput {
        events: input,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        ..Default::default()
    };
    let mut events = Vec::new();
    let mut out = ctx.run_ui(raw, |ui| {
        pin_wizard::draw(ui.ctx(), state, &mut events);
    });
    // The visible part of each text: a text in a scroll area that the
    // scroll clips away is not on the screen.
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            let visible = rect.intersect(clipped.clip_rect);
            if visible.is_positive() {
                texts.push((text.galley.job.text.clone(), visible));
            }
        }
    }
    out.textures_delta.clear();
    (texts, events)
}

fn rect_of(texts: &[(String, egui::Rect)], needle: &str) -> egui::Rect {
    texts
        .iter()
        .find(|(t, _)| t == needle)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no \"{needle}\" painted; runs were {texts:#?}"))
}

fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) -> Vec<AppEvent> {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(ctx, state, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(ctx, state, vec![press(true)]);
    frame(ctx, state, vec![press(false)]).1
}

/// Settle the window (egui sizes a new window over two frames).
fn settle(ctx: &egui::Context, state: &mut AppState) -> Vec<(String, egui::Rect)> {
    let _ = frame(ctx, state, Vec::new());
    let _ = frame(ctx, state, Vec::new());
    frame(ctx, state, Vec::new()).0
}

fn assert_on_screen(texts: &[(String, egui::Rect)]) {
    for button in ["Back", "Cancel"] {
        assert!(
            texts.iter().any(|(t, _)| t == button),
            "the button \"{button}\" is not in view: {texts:#?}"
        );
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    for (text, rect) in texts {
        assert!(
            screen.expand(0.5).contains_rect(*rect),
            "\"{text}\" paints off the 1600 x 900 screen at {rect:?}"
        );
    }
}

#[test]
fn cancel_changes_nothing() {
    let ctx = ctx();
    let mut state = state();
    let stock = state.session.stock_config().clone();
    let tools = state.session.tools().len();
    let toolpaths = state.session.toolpath_configs().len();

    // A draft edit inside the wizard: the 3-pin placement of step 2.
    if let Some(w) = state.panels.pin_wizard.as_mut() {
        w.place(&state.session);
        w.add_drills.push(8.0);
        assert_ne!(w.pins, stock.alignment_pins, "non-vacuity: the draft moved");
    }
    let texts = settle(&ctx, &mut state);
    let events = click(&ctx, &mut state, rect_of(&texts, "Cancel").center());
    assert!(
        state.panels.pin_wizard.is_none(),
        "Cancel closes the wizard"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AppEvent::PinWizardApply { .. })),
        "Cancel sends no Apply: {events:?}"
    );
    assert_eq!(*state.session.stock_config(), stock);
    assert_eq!(state.session.tools().len(), tools);
    assert_eq!(state.session.toolpath_configs().len(), toolpaths);
}

#[test]
fn apply_sends_the_draft_once() {
    let ctx = ctx();
    let mut state = state();
    if let Some(w) = state.panels.pin_wizard.as_mut() {
        w.place(&state.session);
    }
    let draft = state.panels.pin_wizard.as_ref().unwrap().pins.clone();
    let texts = settle(&ctx, &mut state);
    assert_on_screen(&texts);

    // Step 3 shows the plan view, on screen.
    let _ = click(&ctx, &mut state, rect_of(&texts, "3. Placement").center());
    let texts = settle(&ctx, &mut state);
    assert!(texts.iter().any(|(t, _)| t == "X0 Y0"), "{texts:#?}");
    assert_on_screen(&texts);

    // The summary, then Apply.
    let _ = click(&ctx, &mut state, rect_of(&texts, "4. Summary").center());
    let texts = settle(&ctx, &mut state);
    assert_on_screen(&texts);
    let events = click(&ctx, &mut state, rect_of(&texts, "Apply").center());
    let applies: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::PinWizardApply { pins, add_drills } => Some((pins, add_drills)),
            _ => None,
        })
        .collect();
    assert_eq!(applies.len(), 1, "{events:?}");
    assert_eq!(*applies[0].0, draft);
    assert!(state.panels.pin_wizard.is_none(), "Apply closes the wizard");
}
