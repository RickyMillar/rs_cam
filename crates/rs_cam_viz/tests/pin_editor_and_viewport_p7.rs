//! P7 — the stock panel's pin editor and the viewport pin overlays.
//!
//! The operator: "the ux is a little clunky at the moment! ITs hard to see
//! whats going on. id also like existing pins to be ajustable as a set, not
//! individually." The arms, on the mini test piece's shape (a 65 x 65 x
//! 24.5 stock, a stock outline 0..65, a cavity 12..53, a 3 mm end mill as
//! `tools[0]`, a 6 mm drill on a drill operation, a Top and a Bottom
//! setup, two 6 mm pins on the flip axis y = 32.5):
//!
//! 1. the panel paints the status chips of the one report: the pins seat,
//!    they do NOT key (a half turn seats them), the walls, and the pin
//!    drill;
//! 2. "Use 3-pin preset" writes the placer's set in one edit, and the
//!    chips then say "Keyed: yes";
//! 3. a drag of a pattern dimension writes the draft every frame and the
//!    session ONCE, on release;
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
use rs_cam_viz::ui::overlays::registry::{self, Precondition};
use rs_cam_viz::ui::properties;

const PANEL_WIDTH: f32 = 340.0;

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

fn tool(id: usize, tool_type: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(id), tool_type);
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
        id: rs_cam_core::ids::ToolpathId(0),
        name: "Dowel holes, 6 mm drill".to_owned(),
        enabled: true,
        operation: OperationConfig::Drill(DrillConfig::default()),
        dressups: DressupConfig::for_op(OperationType::Drill),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        face_selection: None,
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
    state.session = mini_session((0.0, 0.0));
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

/// One frame of the inspector; the painted texts and their rects.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let input = egui::RawInput {
        events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 2400.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("pin_editor_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .show(ui, |ui| {
                let mut app_events = Vec::new();
                properties::draw(ui, state, &mut app_events);
            });
    });
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            texts.push((text.galley.job.text.clone(), rect));
        }
    }
    out.textures_delta.clear();
    texts
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

fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(ctx, state, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(ctx, state, vec![press(true)]);
    let _ = frame(ctx, state, vec![press(false)]);
}

#[test]
fn the_chips_say_the_mini_pins_seat_and_do_not_key() {
    let ctx = ctx();
    let mut state = stock_state();
    let _ = frame(&ctx, &mut state, Vec::new());
    let texts = frame(&ctx, &mut state, Vec::new());
    assert!(has(&texts, "Seats after flip: yes"), "{texts:#?}");
    assert!(has(&texts, "Keyed: NO: seats turned 180 deg"), "{texts:#?}");
    assert!(has(&texts, "Walls: min"), "{texts:#?}");
    // No pin-drill operation yet; the door could create one.
    assert!(has(&texts, "Pin drill: not created"), "{texts:#?}");
    assert!(has(&texts, "Use 3-pin preset"), "{texts:#?}");
    assert!(has(&texts, "Move the pair off the axis"), "{texts:#?}");
    // The single-pin drags sit in a closed fold.
    assert!(has(&texts, "Edit single pins"), "{texts:#?}");
    assert!(!has(&texts, "Pin 1 X"), "the fold must start closed");
}

#[test]
fn the_three_pin_preset_keys_the_set_in_one_edit() {
    let ctx = ctx();
    let mut state = stock_state();
    let _ = frame(&ctx, &mut state, Vec::new());
    let texts = frame(&ctx, &mut state, Vec::new());
    let button = rect_of(&texts, "Use 3-pin preset").center();
    assert!(!state.history.can_undo());
    click(&ctx, &mut state, button);
    let pins = &state.session.stock_config().alignment_pins;
    assert_eq!(pins.len(), 3, "{pins:?}");
    assert!(state.session.pin_set_report().flip.keyed());
    assert!(state.history.can_undo(), "the edit is one undo step");
    let texts = frame(&ctx, &mut state, Vec::new());
    assert!(has(&texts, "Keyed: yes, margin"), "{texts:#?}");
    assert!(has(&texts, "3 pins: axis + mirror pair"), "{texts:#?}");
    assert!(has(&texts, "Pair off axis"), "{texts:#?}");
}

#[test]
fn a_pattern_drag_writes_the_session_once_on_release() {
    let ctx = ctx();
    let mut state = stock_state();
    // Start from the keyed 3-pin set.
    let _ = frame(&ctx, &mut state, Vec::new());
    let texts = frame(&ctx, &mut state, Vec::new());
    click(
        &ctx,
        &mut state,
        rect_of(&texts, "Use 3-pin preset").center(),
    );
    let texts = frame(&ctx, &mut state, Vec::new());
    let label = rect_of(&texts, "Pair off");
    let before = state.session.stock_config().alignment_pins.clone();

    // The value field sits right of the label column.
    let start = egui::pos2(
        label.left() + rs_cam_viz::ui::tokens::LABEL_COL_WIDTH + 24.0,
        label.center().y,
    );
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(start)]);
    let _ = frame(&ctx, &mut state, vec![press(start, true)]);
    let mut session_writes = 0;
    let mut draft_moved = false;
    let mut last = before.clone();
    for step in 1..=6 {
        let at = start - egui::vec2(3.0 * step as f32, 0.0);
        let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
        let now = state.session.stock_config().alignment_pins.clone();
        if now != last {
            session_writes += 1;
            last = now;
        }
        if let Some(draft) = &state.history.stock_draft {
            draft_moved |= draft.alignment_pins != before;
        }
    }
    assert_eq!(session_writes, 0, "a drag frame wrote the session");
    assert!(draft_moved, "the drag did not move the draft pins");
    let end = start - egui::vec2(18.0, 0.0);
    let _ = frame(&ctx, &mut state, vec![press(end, false)]);
    let _ = frame(&ctx, &mut state, Vec::new());
    let after = state.session.stock_config().alignment_pins.clone();
    assert_ne!(after, before, "the release did not commit the drag");
    let report = state.session.pin_set_report();
    assert!(
        matches!(
            report.preset,
            Some(rs_cam_core::session::PinPreset::AxisPlusMirrorPair(_))
        ),
        "the edit left the preset"
    );
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
    let report = state.session.pin_set_report();
    let pins = state.session.stock_config().alignment_pins.clone();
    for (ghost, pin) in report.wrong_seating_images.iter().zip(&pins) {
        let on_a_pin = pins
            .iter()
            .any(|p| (p.x - ghost.0).hypot(p.y - ghost.1) < 1e-6);
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
        .session
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
    state.session = mini_session((-15.0, -20.0));
    let back = state
        .session
        .list_setups()
        .iter()
        .find(|s| s.name == "Back")
        .unwrap()
        .id;
    state.selection = Selection::Setup(rs_cam_viz::state::job::SetupId(back));
    let (frame, _) = active_display_setup(&state.session, &state.selection).unwrap();
    assert_eq!(frame.face_up, FaceUp::Bottom);

    let stock = state.session.stock_config().clone();
    // Pin 2 at (6, 32.5): the Bottom flip maps y to 65 - y = 32.5, so use
    // a pin off the axis to see the flip.
    let pin = AlignmentPin::new(10.0, 12.0, 6.0);
    let mut stock_with_pin = stock.clone();
    stock_with_pin.alignment_pins = vec![pin.clone()];
    let drawn = pin_display_point(&frame, &stock_with_pin, pin.x, pin.y);
    let raw = (pin.x, pin.y, stock.z);
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
