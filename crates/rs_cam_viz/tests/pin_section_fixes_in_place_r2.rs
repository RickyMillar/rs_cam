//! R2, R3, R5b (2026-10-12): the pin section fixes a missing drill in
//! place, shows the pins from above, and fits the inspector.
//!
//! The operator: "I see pins. I want to just use the pins area to adjust
//! it. I dont want to have to move screen to sort it." The mixed preset
//! refused with "Add a drill operation with that drill, then choose the
//! preset again" and sent the operator to another screen.
//!
//! The arms, on the mini test piece's shape (a 65 x 65 stock, a 3 mm end
//! mill, a 6 mm drill on a drill operation, a Top and a Bottom setup, two
//! 6 mm pins on the flip axis):
//!
//! 1. The mixed preset never refuses for a missing drill. It writes the
//!    pins; the size with no drill shows "no drill" and an "Add a <d> mm
//!    drill" button in the section, which raises `PinDrillAddTool`.
//! 2. No text the section paints, and no pin text in the source, tells
//!    the operator to go to another screen.
//! 3. A drag on the plan view moves the pin SET: the draft every frame,
//!    the session once, on release, in 0.1 mm steps.
//! 4. R5b: at the two default inspector widths the section with the
//!    keying warning is no wider than the panel.

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
use rs_cam_viz::app::{SidePanelEdge, side_panel};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{AppEvent, properties};

const PANEL_WIDTH: f32 = 340.0;

/// Text that sends the operator to another screen. None of it may appear
/// in the pin section or in a pin refusal.
const GO_ELSEWHERE: [&str; 5] = [
    "Add a drill operation",
    "then choose the preset again",
    "operation tab",
    "Set its tool",
    "then press 'Create the pin drill'",
];

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

fn mini_session() -> ProjectSession {
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
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .model(square_model(0, "stock_outline.dxf", 0.0, 65.0))
        .model(square_model(1, "back_cavity.dxf", 12.0, 53.0))
        .tool(tool(0, ToolType::EndMill, 3.0))
        .tool(tool(3, ToolType::VBit, 6.0));
    let front = builder.add_setup("Front".to_owned(), FaceUp::Top);
    let _ = builder.add_setup("Back".to_owned(), FaceUp::Bottom);
    let _ = builder.add_toolpath(front, drill_op(3, 1)).unwrap();
    builder.build()
}

fn stock_state() -> AppState {
    let mut state = AppState::new();
    state.session = mini_session();
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

/// One frame of the inspector: the painted texts with their rects, and
/// the events the inspector raised.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    input_events: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect)>, Vec<AppEvent>) {
    let input = egui::RawInput {
        events: input_events,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 2400.0),
        )),
        ..Default::default()
    };
    let mut app_events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("pin_section_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .show(ui, |ui| {
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
    (texts, app_events)
}

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

/// Click at `at`; the events of the release frame.
fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) -> Vec<AppEvent> {
    let _ = frame(ctx, state, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(ctx, state, vec![press(at, true)]);
    frame(ctx, state, vec![press(at, false)]).1
}

fn assert_no_go_elsewhere(texts: &[(String, egui::Rect)]) {
    for (text, _) in texts {
        for phrase in GO_ELSEWHERE {
            assert!(
                !text.contains(phrase),
                "the pin section sends the operator away: \"{text}\""
            );
        }
    }
}

#[test]
fn the_mixed_preset_offers_the_missing_drill_in_place() {
    let ctx = ctx();
    let mut state = stock_state();
    let _ = frame(&ctx, &mut state, Vec::new());
    let (texts, _) = frame(&ctx, &mut state, Vec::new());
    assert_no_go_elsewhere(&texts);

    let _ = click(
        &ctx,
        &mut state,
        rect_of(&texts, "2 pins, mixed diameter").center(),
    );
    assert!(
        state.panels.pin_preset_refusal.is_none(),
        "the mixed preset refused: {:?}",
        state.panels.pin_preset_refusal
    );
    let pins = state.session.stock_config().alignment_pins.clone();
    assert_eq!(pins.len(), 2, "{pins:?}");
    let large = pins.iter().map(|p| p.diameter).fold(f64::MIN, f64::max);
    // No drill larger than 6 mm: the start value is the small dowel plus
    // twice the hole slop.
    let expected = rs_cam_viz::ui::properties::pins::mixed_start_large_diameter(6.0);
    assert_eq!(large, expected);

    let _ = frame(&ctx, &mut state, Vec::new());
    let (texts, _) = frame(&ctx, &mut state, Vec::new());
    assert_no_go_elsewhere(&texts);
    let label = format!("Add a {expected:.2} mm drill");
    assert!(
        has(&texts, &format!("{expected:.2} mm pins (1): no drill")),
        "{texts:#?}"
    );
    let events = click(&ctx, &mut state, rect_of(&texts, &label).center());
    assert!(
        events.iter().any(|e| matches!(
            e,
            AppEvent::PinDrillAddTool { diameter } if *diameter == expected
        )),
        "the button raises the pin door's add-drill event: {events:?}"
    );
}

/// The pin texts in the source never name another screen. The anchor
/// (the in-place button text) keeps the scan from passing on an empty
/// file.
#[test]
fn no_pin_text_sends_the_operator_away() {
    const SOURCES: [(&str, &str); 4] = [
        (
            "pins/drills.rs",
            include_str!("../src/ui/properties/pins/drills.rs"),
        ),
        (
            "pins/controls.rs",
            include_str!("../src/ui/properties/pins/controls.rs"),
        ),
        (
            "pins/status.rs",
            include_str!("../src/ui/properties/pins/status.rs"),
        ),
        (
            "controller/events/pins.rs",
            include_str!("../src/controller/events/pins.rs"),
        ),
    ];
    let drills = SOURCES[0].1;
    assert!(
        drills.contains("\"Add a {:.2} mm drill\""),
        "non-vacuity: the in-place button text is in drills.rs"
    );
    for (name, src) in SOURCES {
        for line in src.lines() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for phrase in GO_ELSEWHERE {
                assert!(
                    !code.contains(phrase),
                    "{name}: a pin text sends the operator away: {line}"
                );
            }
        }
    }
}

#[test]
fn a_plan_view_drag_moves_the_set_and_commits_once() {
    let ctx = ctx();
    let mut state = stock_state();
    let _ = frame(&ctx, &mut state, Vec::new());
    let (texts, _) = frame(&ctx, &mut state, Vec::new());
    // "X0 Y0" sits outside the stock's min corner, right-top aligned.
    let origin_label = rect_of(&texts, "X0 Y0");
    let start = egui::pos2(origin_label.right() + 40.0, origin_label.top() - 40.0);
    let before = state.session.stock_config().alignment_pins.clone();

    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(start)]);
    let _ = frame(&ctx, &mut state, vec![press(start, true)]);
    let mut session_writes = 0;
    let mut draft_moved = false;
    for step in 1..=6 {
        let at = start + egui::vec2(4.0 * step as f32, 0.0);
        let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
        if state.session.stock_config().alignment_pins != before {
            session_writes += 1;
        }
        if let Some(draft) = &state.history.stock_draft {
            draft_moved |= draft.alignment_pins != before;
        }
    }
    assert_eq!(session_writes, 0, "a drag frame wrote the session");
    assert!(draft_moved, "the drag did not move the draft pins");
    let end = start + egui::vec2(24.0, 0.0);
    let _ = frame(&ctx, &mut state, vec![press(end, false)]);
    let _ = frame(&ctx, &mut state, Vec::new());
    let after = state.session.stock_config().alignment_pins.clone();
    assert_ne!(after, before, "the release did not commit the drag");
    assert!(state.history.can_undo(), "the drag is one undo step");
    // The set moves as one, along X, in 0.1 mm steps.
    let dx = after[0].x - before[0].x;
    assert!(dx > 0.0, "{before:?} -> {after:?}");
    for (a, b) in before.iter().zip(&after) {
        assert!((b.x - a.x - dx).abs() < 1e-9 && (b.y - a.y).abs() < 1e-9);
    }
    assert!(
        ((dx * 10.0).round() - dx * 10.0).abs() < 1e-6,
        "the step is 0.1 mm: {dx}"
    );
}

/// The pin section alone, on the stored stock of `state`. Returns the
/// width the section used.
fn draw_pin_section(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) -> f32 {
    use rs_cam_viz::ui::properties::pins;
    let session = &state.session;
    let mut stock = session.stock_config().clone();
    let setups: Vec<_> = session
        .list_setups()
        .iter()
        .map(|s| (s.face_up, s.z_rotation))
        .collect();
    let keyed = pins::KeyedPinInputs::from_session(session);
    let report = session.pin_set_report_for(&stock.alignment_pins);
    let pin_drill = pins::PinDrillState::of(session, &stock.alignment_pins);
    let drills = [6.0];
    let inputs = pins::PinSectionInputs {
        setups: &setups,
        keyed: &keyed,
        report: &report,
        models: &[],
        drill_diameters: &drills,
        pin_drill: &pin_drill,
        selected_pin: None,
        preset_refusal: None,
    };
    let mut memo = rs_cam_viz::state::panels::KeyedPinPlanMemo::default();
    ui.scope(|ui| {
        let _ = pins::draw(ui, &mut stock, &inputs, &mut memo, events);
    })
    .response
    .rect
    .width()
}

/// R5b: the pin section stays inside the inspector at the two default
/// widths: the keying warning and its buttons (two 6 mm pins on the axis),
/// and the "Drills" block of a mixed set with no drill of the large size.
#[test]
fn the_pin_section_fits_the_inspector_r5b() {
    let mixed = vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(6.0, 32.5, 7.0),
    ];
    for width in [240.0_f32, 280.0] {
        for pins in [None, Some(mixed.clone())] {
            let ctx = ctx();
            let mut state = stock_state();
            if let Some(pins) = &pins {
                let _ = state
                    .session
                    .apply(rs_cam_core::session::Command::SetAlignmentPins(
                        rs_cam_core::session::SetAlignmentPinsArgs { pins: pins.clone() },
                    ))
                    .unwrap();
            }
            for pass in 0..2 {
                let (mut inner, mut content) = (0.0_f32, 0.0_f32);
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    side_panel(ui, SidePanelEdge::Right, "setup_properties", width, |ui| {
                        inner = ui.max_rect().width();
                        let mut events = Vec::new();
                        content = draw_pin_section(ui, &state, &mut events);
                    });
                });
                out.textures_delta.clear();
                assert!(inner > 0.0, "vacuous measurement");
                assert!(
                    content <= inner + 0.5,
                    "pass {pass} at {width} ({pins:?}): the pin section is {content:.1} \
                     wide in {inner:.1}"
                );
            }
        }
    }
}
