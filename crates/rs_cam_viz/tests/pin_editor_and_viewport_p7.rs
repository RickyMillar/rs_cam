//! P7 — the pin editor (since round 2, 2026-10-12: the pin wizard) and the
//! viewport pin overlays.
//!
//! The operator: "the ux is a little clunky at the moment! ITs hard to see
//! whats going on. id also like existing pins to be ajustable as a set, not
//! individually." The arms, on the mini test piece's shape (a 65 x 65 x
//! 24.5 stock, a stock outline 0..65, a cavity 12..53, a 3 mm end mill as
//! `tools[0]`, a 6 mm drill on a drill operation, a Top and a Bottom
//! setup, two 6 mm pins on the flip axis y = 32.5):
//!
//! 1. the wizard's Place step paints the chips of the one report: the
//!    pins do NOT key (a half turn seats them) and the walls; the Dowels
//!    step names the drill of the pin size;
//! 2. "Auto-place" writes the placer's set into the draft in one click,
//!    and the chips then say "Keyed"; Apply writes it in one undo step;
//! 3. a drag of a pattern value writes the draft every frame, keeps the
//!    preset, and never writes the session before Apply;
//! 4. the ghost overlay row is ready while the set does not key, refused
//!    with a reason when it keys, and on by default in the Setup view;
//! 5. a pick on a flipped setup with the stock origin not at 0 hits the
//!    pin where the upload draws it (C8).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::sync::Arc;

use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::overlays::registry::{self, Precondition};
use rs_cam_viz::ui::pin_wizard::{self, PinWizardState, WizardStep};

fn square_model(id: usize, name: &str, lo: f64, hi: f64) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
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

fn tool(id: usize, tool_type: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(id), tool_type);
    t.diameter = diameter;
    t
}

fn drill_op(tool_id: usize, model_id: usize) -> rs_cam_core::session::ToolpathConfig {
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::config::{
        BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
    };
    use rs_cam_core::compute::operation_configs::DrillConfig;

    rs_cam_core::session::ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ids::ToolpathId::from_raw(0),
        name: "Dowel holes, 6 mm drill".to_owned(),
        enabled: true,
        operation: OperationConfig::Drill(DrillConfig::default()),
        dressups: DressupConfig::for_op(OperationType::Drill),
        heights: HeightsConfig::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

/// The mini test piece in shape, with its stored pins. The model squares
/// are shifted by the stock origin in X and Y alike (`ox`), so a test
/// passes an origin with equal parts when it needs the models to follow.
fn mini_session(origin: (f64, f64)) -> ProjectSession {
    let stock = StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_x: origin.0,
        origin_y: origin.1,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        alignment_pins: vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ],
        ..StockConfig::default()
    };
    let (ox, _) = origin;
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .model(square_model(0, "stock_outline.dxf", ox, ox + 65.0))
        .model(square_model(1, "back_cavity.dxf", ox + 12.0, ox + 53.0))
        .tool(tool(0, ToolType::EndMill, 3.0))
        .tool(tool(3, ToolType::VBit, 6.0));
    let front = builder.add_setup("Front".to_owned(), FaceUp::Top);
    let _ = builder.add_setup("Back".to_owned(), FaceUp::Bottom);
    let _ = builder.add_toolpath(front, drill_op(3, 1)).unwrap();
    builder.build()
}

fn stock_state() -> AppState {
    let mut state = AppState::new();
    state.replace_session(mini_session((0.0, 0.0)));
    state.selection = Selection::Stock;
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

/// The mini state with the wizard open on `step`.
fn wizard_state(step: WizardStep) -> AppState {
    let mut state = stock_state();
    let mut wizard = PinWizardState::open(state.session());
    wizard.step = step;
    state.panels.pin_wizard = Some(wizard);
    state
}

/// One frame of the pin wizard; the painted texts and the events.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    events: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect)>, Vec<AppEvent>) {
    let input = egui::RawInput {
        events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        ..Default::default()
    };
    let mut app_events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        pin_wizard::draw(ui.ctx(), state, &mut app_events);
    });
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            texts.push((text.galley.job.text.clone(), rect));
        }
    }
    out.textures_delta.clear();
    (texts, app_events)
}

fn settle(ctx: &egui::Context, state: &mut AppState) -> Vec<(String, egui::Rect)> {
    let _ = frame(ctx, state, Vec::new());
    let _ = frame(ctx, state, Vec::new());
    frame(ctx, state, Vec::new()).0
}

/// A chip paints its words in capitals, so the match ignores case.
fn has(texts: &[(String, egui::Rect)], needle: &str) -> bool {
    let needle = needle.to_uppercase();
    texts
        .iter()
        .any(|(t, _)| t.to_uppercase().contains(&needle))
}

fn rect_of(texts: &[(String, egui::Rect)], needle: &str) -> egui::Rect {
    texts
        .iter()
        .find(|(t, _)| t.contains(needle))
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no \"{needle}\" painted; runs were {texts:#?}"))
}

fn press(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
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

fn draft(state: &AppState) -> Vec<AlignmentPin> {
    state.panels.pin_wizard.as_ref().unwrap().pins.clone()
}

#[test]
fn the_chips_say_the_mini_pins_do_not_key() {
    let ctx = ctx();
    let mut state = wizard_state(WizardStep::Place);
    let texts = settle(&ctx, &mut state);
    assert!(has(&texts, "Not keyed: seats turned 180 deg"), "{texts:#?}");
    assert!(has(&texts, "Wall "), "{texts:#?}");
    // The two axis pins fit no preset: no pattern values, no snap.
    assert!(!has(&texts, "Pair off axis"), "{texts:#?}");
    assert!(!has(&texts, "Snap"), "{texts:#?}");

    // The Dowels step names the drill of the 6 mm size (R1: one row for
    // each pin size).
    let mut state = wizard_state(WizardStep::Dowels);
    let texts = settle(&ctx, &mut state);
    assert!(has(&texts, "6.00 mm: "), "{texts:#?}");
    assert!(!has(&texts, "6.00 mm: no drill"), "{texts:#?}");
}

#[test]
fn auto_place_keys_the_set_in_one_edit() {
    let ctx = ctx();
    let mut state = wizard_state(WizardStep::Place);
    let texts = settle(&ctx, &mut state);
    let stored = state.session().stock_config().alignment_pins.clone();
    let _ = click(&ctx, &mut state, rect_of(&texts, "Auto-place").center());
    let pins = draft(&state);
    assert_eq!(pins.len(), 3, "{pins:?}");
    assert!(state.session().pin_set_report_for(&pins).flip.keyed());
    assert_eq!(
        state.session().stock_config().alignment_pins,
        stored,
        "the draft only, before Apply"
    );
    let texts = settle(&ctx, &mut state);
    assert!(has(&texts, "Keyed, margin"), "{texts:#?}");
    assert!(has(&texts, "Pair off axis"), "{texts:#?}");

    // Apply: ONE event with the draft. The pin door writes it in one undo
    // step (`the_wizard_apply_adds_the_drill_and_writes_once_r4`).
    let events = click(&ctx, &mut state, rect_of(&texts, "Apply").center());
    let applied: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::PinWizardApply { pins, .. } => Some(pins.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(applied, vec![pins]);
}

#[test]
fn a_pattern_drag_keeps_the_preset_and_the_session() {
    let ctx = ctx();
    let mut state = wizard_state(WizardStep::Place);
    let parts = state.split_mut();
    if let Some(w) = parts.panels.pin_wizard.as_mut() {
        w.place(parts.session);
    }
    let stored = state.session().stock_config().alignment_pins.clone();
    let texts = settle(&ctx, &mut state);
    let label = rect_of(&texts, "Pair off");
    let before = draft(&state);

    // The value field sits right of the label column.
    let start = egui::pos2(
        label.left() + rs_cam_viz::ui::tokens::LABEL_COL_WIDTH + 24.0,
        label.center().y,
    );
    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(start)]);
    let _ = frame(&ctx, &mut state, vec![press(start, true)]);
    let mut draft_moved = false;
    for step in 1..=6 {
        let at = start - egui::vec2(3.0 * step as f32, 0.0);
        let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
        assert_eq!(
            state.session().stock_config().alignment_pins,
            stored,
            "a drag frame wrote the session"
        );
        draft_moved |= draft(&state) != before;
    }
    assert!(draft_moved, "the drag did not move the draft pins");
    let end = start - egui::vec2(18.0, 0.0);
    let _ = frame(&ctx, &mut state, vec![press(end, false)]);
    let _ = frame(&ctx, &mut state, Vec::new());
    let after = draft(&state);
    assert_ne!(after, before);
    let report = state.session().pin_set_report_for(&after);
    assert!(
        matches!(
            report.preset,
            Some(rs_cam_core::session::PinPreset::AxisPlusMirrorPair(_))
        ),
        "the edit left the preset"
    );
    assert!(report.flip.seats(), "the edit broke the symmetry");
}

fn ghost_row() -> &'static registry::OverlayRow {
    registry::rows()
        .iter()
        .find(|r| r.id == "pin_ghost")
        .expect("the ghost row is registered")
}

#[test]
fn the_ghost_row_is_ready_exactly_while_the_set_does_not_key() {
    let row = ghost_row();
    assert_eq!(
        (row.default_for)(rs_cam_viz::state::Workspace::Setup),
        Some(true),
        "the ghost is on by default in the Setup view"
    );
    let mut state = stock_state();
    assert!(matches!((row.precondition)(&state), Precondition::Ready));

    // The ghost of the two axis pins lands ON the real pins (a half turn
    // swaps them). Its ring must not lie on the pin circle, or it hides.
    let report = state.session().pin_set_report();
    let pins = state.session().stock_config().alignment_pins.clone();
    for (ghost, pin) in report.wrong_seating_images.iter().zip(&pins) {
        let on_a_pin = pins
            .iter()
            .any(|p| (p.at.x() - ghost.0).hypot(p.at.y() - ghost.1) < 1e-6);
        assert!(on_a_pin, "the mini ghost lands on a real pin: {ghost:?}");
        let ring = rs_cam_viz::state::pin_view::ghost_ring_radius(pin.diameter);
        assert!(
            ring - 0.5 * pin.diameter >= rs_cam_core::compute::alignment_pins::PIN_MATCH_TOL_MM,
            "the ghost ring {ring} would lie on the pin circle"
        );
    }

    let keyed = vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(6.0, 6.0, 6.0),
        AlignmentPin::new(6.0, 59.0, 6.0),
    ];
    let _ = state
        .apply(rs_cam_core::session::Command::SetAlignmentPins(
            rs_cam_core::session::SetAlignmentPinsArgs { pins: keyed },
        ))
        .unwrap();
    match (row.precondition)(&state) {
        Precondition::Disabled { reason, .. } => {
            assert!(reason.contains("key"), "{reason}");
        }
        Precondition::Ready => panic!("a keyed set has no ghost to show"),
    }
}

/// C8: the pick and the drawing read one point. On a Bottom setup with
/// the stock origin at (-15, -20), the drawn pin is NOT at the raw
/// stock-local point the old pick used, and a click on the drawn point
/// hits the pin.
#[test]
fn a_pick_on_a_flipped_setup_hits_the_drawn_pin() {
    use rs_cam_viz::interaction::picking::{PickContext, PickHit, pick_alignment_pins};
    use rs_cam_viz::render::camera::OrbitCamera;
    use rs_cam_viz::state::pin_view::{active_display_setup, pin_display_point};

    let mut state = AppState::new();
    state.replace_session(mini_session((-15.0, -20.0)));
    let back = state
        .session()
        .list_setups()
        .iter()
        .find(|s| s.name == "Back")
        .unwrap()
        .id;
    state.selection = Selection::Setup(back);
    let (frame, _) = active_display_setup(state.session(), &state.selection).unwrap();
    assert_eq!(frame.face_up, FaceUp::Bottom);

    let stock = state.session().stock_config().clone();
    // Pin 2 at (6, 32.5): the Bottom flip maps y to 65 - y = 32.5, so use
    // a pin off the axis to see the flip.
    let pin = AlignmentPin::new(10.0, 12.0, 6.0);
    let mut stock_with_pin = stock.clone();
    stock_with_pin.alignment_pins = vec![pin.clone()];
    let drawn = pin_display_point(&frame, &stock_with_pin, pin.at.x(), pin.at.y());
    let raw = (pin.at.x(), pin.at.y(), stock.z);
    assert!(
        (drawn.y - raw.1).abs() > 1.0,
        "the flipped setup must move the pin: drawn {drawn:?}, raw {raw:?}"
    );

    let mut camera = OrbitCamera::new();
    camera.fit_to_bounds([0.0, 0.0, 0.0], [65.0, 65.0, 24.5]);
    let (w, h) = (800.0_f32, 600.0_f32);
    let screen = camera
        .project_to_screen(
            [drawn.x as f32, drawn.y as f32, drawn.z as f32],
            w / h,
            w,
            h,
        )
        .unwrap();
    let ctx = PickContext {
        camera: &camera,
        screen_x: screen[0],
        screen_y: screen[1],
        aspect: w / h,
        vw: w,
        vh: h,
        visibility: &state.overlays.model_visibility,
    };
    let hit = pick_alignment_pins(&ctx, &stock_with_pin, &frame);
    assert!(
        matches!(hit, Some(PickHit::AlignmentPin { pin_index: 0 })),
        "the click on the drawn pin missed: {hit:?}"
    );

    // The raw point of the old pick is somewhere else on the screen.
    let old = camera
        .project_to_screen([raw.0 as f32, raw.1 as f32, raw.2 as f32], w / h, w, h)
        .unwrap();
    assert!(
        (old[0] - screen[0]).hypot(old[1] - screen[1]) > 12.0,
        "the test does not separate the two frames"
    );
}
