//! R4; round 2 (2026-10-12): the pin wizard's contract.
//!
//! The operator, round 2: "the wizard first page has no options. loads of
//! text. bad ui. ... we can probably remove the summary page completely."
//! The wizard has two steps, Dowels and Place, and works on its OWN draft.
//!
//! 1. Cancel closes the wizard and changes nothing: no event, the same
//!    pins, the same tools, the same toolpaths.
//! 2. Apply raises ONE `PinWizardApply` with the wizard's pins and its
//!    queue. The pin door applies it (its controller test is
//!    `the_wizard_apply_adds_the_drill_and_writes_once_r4`).
//! 3. At 1600 x 900 every step paints inside the screen. Step 1 shows the
//!    flip diagram and its one-line caption; step 2 shows the plan view
//!    and the chips beside Apply.
//! 4. No painted text is longer than one short line, except a refusal.
//! 5. With no flipped setup, step 1 shows the refusal and its one-click
//!    fix in place of the diagram.
//! 6. A drag on the plan view ACROSS the flip axis moves the set only
//!    along the axis, in 0.1 mm steps, and only the draft.

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
use rs_cam_viz::ui::pin_wizard::{self, PinWizardState, WizardStep};

const SCREEN: egui::Vec2 = egui::vec2(1600.0, 900.0);

/// The longest text the wizard may paint, in characters, except a
/// refusal: one short line (round 2, ruling 3).
const MAX_LINE_CHARS: usize = 64;

fn square_model(id: usize, name: &str, lo: f64, hi: f64) -> LoadedModel {
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::new(vec![
                P2::new(lo, lo),
                P2::new(hi, lo),
                P2::new(hi, hi),
                P2::new(lo, hi),
            ]),
        ])),
        path: std::path::PathBuf::from(name),
        kind: None,
        enriched_mesh: None,
        units: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn state_with(back: Option<FaceUp>) -> AppState {
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
    if let Some(face) = back {
        let _ = builder.add_setup("Back".to_owned(), face);
    }
    let mut state = AppState::new();
    state.replace_session(builder.build());
    state.panels.pin_wizard = Some(PinWizardState::open(state.session()));
    state
}

fn state() -> AppState {
    state_with(Some(FaceUp::Bottom))
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

fn press(at: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}

fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) -> Vec<AppEvent> {
    let _ = frame(ctx, state, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(ctx, state, vec![press(at, true)]);
    frame(ctx, state, vec![press(at, false)]).1
}

/// Settle the window (egui sizes a new window over two frames).
fn settle(ctx: &egui::Context, state: &mut AppState) -> Vec<(String, egui::Rect)> {
    let _ = frame(ctx, state, Vec::new());
    let _ = frame(ctx, state, Vec::new());
    frame(ctx, state, Vec::new()).0
}

fn assert_on_screen(texts: &[(String, egui::Rect)]) {
    for button in ["Back", "Apply", "Cancel"] {
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

/// Round 2, ruling 3: no text block longer than one short line, except a
/// refusal (`refusals`).
fn assert_short(texts: &[(String, egui::Rect)], refusals: &[&str]) {
    for (text, _) in texts {
        if refusals.iter().any(|r| text.contains(r)) {
            continue;
        }
        assert!(
            text.chars().count() <= MAX_LINE_CHARS && !text.contains('\n'),
            "a text block longer than one line: \"{text}\""
        );
    }
}

#[test]
fn cancel_changes_nothing() {
    let ctx = ctx();
    let mut state = state();
    let stock = state.session().stock_config().clone();
    let tools = state.session().tools().len();
    let toolpaths = state.session().toolpath_configs().len();

    // A draft edit inside the wizard: the 3-pin placement.
    let parts = state.split_mut();
    if let Some(w) = parts.panels.pin_wizard.as_mut() {
        w.place(parts.session);
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
    assert_eq!(*state.session().stock_config(), stock);
    assert_eq!(state.session().tools().len(), tools);
    assert_eq!(state.session().toolpath_configs().len(), toolpaths);
}

#[test]
fn two_steps_and_apply_sends_the_draft_once() {
    let ctx = ctx();
    let mut state = state();
    let parts = state.split_mut();
    if let Some(w) = parts.panels.pin_wizard.as_mut() {
        w.place(parts.session);
    }
    let draft = state.panels.pin_wizard.as_ref().unwrap().pins.clone();

    // Step 1: the flip diagram with its legend and its one-line caption.
    let texts = settle(&ctx, &mut state);
    assert_on_screen(&texts);
    assert_short(&texts, &[]);
    for word in ["1. Dowels", "2. Place", "Stock", "Part", "Drills"] {
        assert!(texts.iter().any(|(t, _)| t == word), "{word}: {texts:#?}");
    }
    assert!(
        texts
            .iter()
            .any(|(t, _)| t == "Flip: Back, about y = 32.5 mm"),
        "{texts:#?}"
    );
    assert!(
        !texts.iter().any(|(t, _)| t.contains("Summary")),
        "the summary step is gone"
    );

    // Step 2: the plan view and the chips beside Apply. A chip paints its
    // words in capitals.
    let _ = click(&ctx, &mut state, rect_of(&texts, "2. Place").center());
    let texts = settle(&ctx, &mut state);
    assert_eq!(
        state.panels.pin_wizard.as_ref().unwrap().step,
        WizardStep::Place
    );
    assert!(texts.iter().any(|(t, _)| t == "X0 Y0"), "{texts:#?}");
    assert!(
        texts
            .iter()
            .any(|(t, _)| t.to_uppercase().contains("KEYED, MARGIN")),
        "{texts:#?}"
    );
    assert!(
        texts
            .iter()
            .any(|(t, _)| t.to_uppercase().contains("WALL ") && t.ends_with(" MM")),
        "{texts:#?}"
    );
    assert!(
        !texts.iter().any(|(t, _)| t.contains("Snap")),
        "no snap control"
    );
    assert_on_screen(&texts);
    assert_short(&texts, &[]);

    let events = click(&ctx, &mut state, rect_of(&texts, "Apply").center());
    let applies: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::PinWizardApply {
                pins,
                add_drills,
                use_tools,
            } => Some((pins, add_drills, use_tools)),
            _ => None,
        })
        .collect();
    assert_eq!(applies.len(), 1, "{events:?}");
    assert_eq!(*applies[0].0, draft);
    assert!(state.panels.pin_wizard.is_none(), "Apply closes the wizard");
}

#[test]
fn no_flip_shows_the_refusal_and_its_fix() {
    let ctx = ctx();
    let mut state = state_with(None);
    let texts = settle(&ctx, &mut state);
    let refusal = "No setup turns the part over in the pin plane.";
    assert!(texts.iter().any(|(t, _)| t == refusal), "{texts:#?}");
    assert!(
        !texts.iter().any(|(t, _)| t.starts_with("Flip:")),
        "no caption without a flip"
    );
    assert_short(&texts, &[refusal]);
    let events = click(
        &ctx,
        &mut state,
        rect_of(&texts, "Add a Bottom setup").center(),
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AppEvent::PinAddSetup(FaceUp::Bottom))),
        "{events:?}"
    );
}

/// Ruling 5: a drag on the plan view across the flip axis (y = 32.5)
/// moves the set only along the axis (X), in 0.1 mm steps. The session
/// does not change before Apply.
#[test]
fn a_plan_view_drag_across_the_axis_moves_the_set_along_it() {
    let ctx = ctx();
    let mut state = state();
    let parts = state.split_mut();
    if let Some(w) = parts.panels.pin_wizard.as_mut() {
        w.place(parts.session);
        w.step = WizardStep::Place;
    }
    let stored = state.session().stock_config().alignment_pins.clone();
    let before = state.panels.pin_wizard.as_ref().unwrap().pins.clone();
    let texts = settle(&ctx, &mut state);
    // "X0 Y0" sits outside the stock's min corner, right-top aligned.
    let origin = rect_of(&texts, "X0 Y0");
    let start = egui::pos2(origin.right() + 60.0, origin.top() - 60.0);

    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(start)]);
    let _ = frame(&ctx, &mut state, vec![press(start, true)]);
    for step in 1..=8 {
        // Mostly up the screen (across the axis), a little to the right.
        let at = start + egui::vec2(3.0 * step as f32, -12.0 * step as f32);
        let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
    }
    let end = start + egui::vec2(24.0, -96.0);
    let _ = frame(&ctx, &mut state, vec![press(end, false)]);
    let _ = frame(&ctx, &mut state, Vec::new());

    let after = state.panels.pin_wizard.as_ref().unwrap().pins.clone();
    assert_eq!(
        state.session().stock_config().alignment_pins,
        stored,
        "a drag writes the draft only"
    );
    let dx = after[0].at.x() - before[0].at.x();
    assert!(
        dx > 0.0,
        "the set moved along the axis: {before:?} -> {after:?}"
    );
    for (a, b) in before.iter().zip(&after) {
        assert!(
            (b.at.y() - a.at.y()).abs() < 1e-9,
            "a pin moved across the axis: {a:?} -> {b:?}"
        );
        assert!(
            (b.at.x() - a.at.x() - dx).abs() < 1e-9,
            "the set moved as one"
        );
    }
    assert!(
        ((dx * 10.0).round() - dx * 10.0).abs() < 1e-6,
        "the step is 0.1 mm: {dx}"
    );
    let report = state.session().pin_set_report_for(&after);
    assert!(
        report.flip.seats() && report.flip.keyed(),
        "the set stays keyed"
    );
}
