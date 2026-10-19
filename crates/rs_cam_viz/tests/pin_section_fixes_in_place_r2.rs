//! R2, R5b; round 2 (2026-10-12): the stock panel's pin block is one
//! status line, at most one warning and ONE button that opens the pin
//! wizard. The wizard fixes a missing drill in place.
//!
//! The operator, round 2: "in the stock side bar. its too busy. lets
//! remove it all and just have the option to open the wizard to edit it.
//! with something smaller like '3 pins added'". Round 1: "I dont want to
//! have to move screen to sort it."
//!
//! The arms, on the mini test piece's shape (a 65 x 65 stock, a 3 mm end
//! mill, a 6 mm drill on a drill operation, a Top and a Bottom setup, two
//! 6 mm pins on the flip axis):
//!
//! 1. The pin block paints exactly its status, its one warning and one
//!    button, and the button opens the wizard. With no pins the button
//!    reads "Set up pins…".
//! 2. The wizard's mixed pattern never refuses for a missing drill: the
//!    size with no drill shows "no drill" and an "Add a <d> mm drill"
//!    button that queues the drill for Apply.
//! 3. No text the wizard paints, and no pin text in the source, tells the
//!    operator to go to another screen.
//! 4. R5b: at the two default inspector widths the pin block is no wider
//!    than the panel.

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
use rs_cam_viz::ui::pin_wizard::{self, PinWizardState};
use rs_cam_viz::ui::{AppEvent, properties};

const PANEL_WIDTH: f32 = 340.0;

/// Text that sends the operator to another screen. None of it may appear
/// in the pin wizard or in a pin refusal.
const GO_ELSEWHERE: [&str; 6] = [
    "Add a drill operation",
    "then choose the preset again",
    "operation tab",
    "Set its tool",
    "then press 'Create the pin drill'",
    "pin section",
];

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
        shoe: Default::default(),
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
        debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn mini_session(pins: Vec<AlignmentPin>) -> ProjectSession {
    let stock = StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        alignment_pins: pins,
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

fn mini_pins() -> Vec<AlignmentPin> {
    vec![
        AlignmentPin::new(59.0, 32.5, 6.0),
        AlignmentPin::new(6.0, 32.5, 6.0),
    ]
}

fn stock_state(pins: Vec<AlignmentPin>) -> AppState {
    let mut state = AppState::new();
    state.session = mini_session(pins);
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

type Texts = Vec<(String, egui::Rect)>;

fn painted(out: &egui::FullOutput) -> Texts {
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            texts.push((text.galley.job.text.clone(), rect));
        }
    }
    texts
}

/// One frame of the inspector: the painted texts and the events.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    input: Vec<egui::Event>,
) -> (Texts, Vec<AppEvent>) {
    let raw = egui::RawInput {
        events: input,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 2400.0),
        )),
        ..Default::default()
    };
    let mut app_events = Vec::new();
    let mut out = ctx.run_ui(raw, |ui| {
        egui::Panel::right("pin_block_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .show(ui, |ui| {
                properties::draw(ui, state, &mut app_events);
            });
    });
    let texts = painted(&out);
    out.textures_delta.clear();
    (texts, app_events)
}

/// One frame of the pin wizard on a 1600 x 900 screen.
fn wizard_frame(
    ctx: &egui::Context,
    state: &mut AppState,
    input: Vec<egui::Event>,
) -> (Texts, Vec<AppEvent>) {
    let raw = egui::RawInput {
        events: input,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        ..Default::default()
    };
    let mut events = Vec::new();
    let mut out = ctx.run_ui(raw, |ui| {
        pin_wizard::draw(ui.ctx(), state, &mut events);
    });
    let texts = painted(&out);
    out.textures_delta.clear();
    (texts, events)
}

fn has(texts: &Texts, needle: &str) -> bool {
    let needle = needle.to_uppercase();
    texts
        .iter()
        .any(|(t, _)| t.to_uppercase().contains(&needle))
}

fn rect_of(texts: &Texts, needle: &str) -> egui::Rect {
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

/// The texts the pin block paints: every text at or below its title. The
/// `Button` component paints its label twice (a transparent egui label
/// for the size, then its own), so one repeat in a row is one text.
fn block_texts(texts: &Texts) -> Vec<String> {
    let top = rect_of(texts, "Alignment pins").top();
    let mut out: Vec<String> = texts
        .iter()
        .filter(|(_, r)| r.top() >= top - 0.5)
        .map(|(t, _)| t.clone())
        .collect();
    out.dedup();
    out
}

#[test]
fn the_pin_block_is_one_line_and_one_button() {
    let ctx = ctx();
    let mut state = stock_state(mini_pins());
    let _ = frame(&ctx, &mut state, Vec::new());
    let (texts, _) = frame(&ctx, &mut state, Vec::new());
    // The pin block is the last part of the stock panel.
    assert_eq!(
        block_texts(&texts),
        vec![
            "Alignment pins".to_owned(),
            "2 pins \u{00B7} do not key".to_owned(),
            "Not keyed: the part also seats turned 180 deg.".to_owned(),
            "Edit pins\u{2026}".to_owned(),
        ],
        "the block paints its status, one warning and one button"
    );

    // The one button opens the wizard on the stored pins.
    assert!(state.panels.pin_wizard.is_none());
    let at = rect_of(&texts, "Edit pins").center();
    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
    let _ = frame(&ctx, &mut state, vec![press(at, true)]);
    let _ = frame(&ctx, &mut state, vec![press(at, false)]);
    let wizard = state.panels.pin_wizard.as_ref().expect("the wizard opens");
    assert_eq!(wizard.pins, mini_pins());
    assert_eq!(
        state.session.stock_config().alignment_pins,
        mini_pins(),
        "opening the wizard changes nothing"
    );

    // With no pins: "No pins" and "Set up pins…".
    let ctx = self::ctx();
    let mut state = stock_state(Vec::new());
    let _ = frame(&ctx, &mut state, Vec::new());
    let (texts, _) = frame(&ctx, &mut state, Vec::new());
    assert_eq!(
        block_texts(&texts),
        vec![
            "Alignment pins".to_owned(),
            "No pins".to_owned(),
            "Set up pins\u{2026}".to_owned(),
        ]
    );
}

fn assert_no_go_elsewhere(texts: &Texts) {
    for (text, _) in texts {
        for phrase in GO_ELSEWHERE {
            assert!(
                !text.contains(phrase),
                "the pin wizard sends the operator away: \"{text}\""
            );
        }
    }
}

#[test]
fn the_mixed_pattern_offers_the_missing_drill_in_place() {
    let ctx = ctx();
    let mut state = stock_state(mini_pins());
    state.panels.pin_wizard = Some(PinWizardState::open(&state.session));
    let _ = wizard_frame(&ctx, &mut state, Vec::new());
    let (texts, _) = wizard_frame(&ctx, &mut state, Vec::new());
    assert_no_go_elsewhere(&texts);

    let at = rect_of(&texts, "2 pins, two sizes").center();
    let _ = wizard_frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
    let _ = wizard_frame(&ctx, &mut state, vec![press(at, true)]);
    let _ = wizard_frame(&ctx, &mut state, vec![press(at, false)]);
    let wizard = state.panels.pin_wizard.as_ref().unwrap();
    assert_eq!(wizard.place_refusal, None, "the mixed pattern refused");
    assert_eq!(wizard.pins.len(), 2, "{:?}", wizard.pins);
    let large = wizard
        .pins
        .iter()
        .map(|p| p.diameter)
        .fold(f64::MIN, f64::max);
    // No drill larger than 6 mm: the start value is the small dowel plus
    // twice the hole slop.
    let expected = properties::pins::edit::mixed_start_large_diameter(6.0);
    assert_eq!(large, expected);

    let _ = wizard_frame(&ctx, &mut state, Vec::new());
    let (texts, _) = wizard_frame(&ctx, &mut state, Vec::new());
    assert_no_go_elsewhere(&texts);
    assert!(
        has(&texts, &format!("{expected:.2} mm: no drill")),
        "{texts:#?}"
    );
    let at = rect_of(&texts, &format!("Add a {expected:.2} mm drill")).center();
    let _ = wizard_frame(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
    let _ = wizard_frame(&ctx, &mut state, vec![press(at, true)]);
    let (_, events) = wizard_frame(&ctx, &mut state, vec![press(at, false)]);
    assert!(events.is_empty(), "the add waits for Apply: {events:?}");
    let wizard = state.panels.pin_wizard.as_ref().unwrap();
    assert_eq!(wizard.add_drills, vec![expected]);
    assert_eq!(
        state.session.tools().len(),
        2,
        "the queued drill is not a tool before Apply"
    );
}

/// The pin texts in the source never name another screen. The anchor
/// (the in-place button text) keeps the scan from passing on an empty
/// file.
#[test]
fn no_pin_text_sends_the_operator_away() {
    const SOURCES: [(&str, &str); 5] = [
        (
            "pins/drills.rs",
            include_str!("../src/ui/properties/pins/drills.rs"),
        ),
        (
            "pins/status.rs",
            include_str!("../src/ui/properties/pins/status.rs"),
        ),
        (
            "pins/mod.rs",
            include_str!("../src/ui/properties/pins/mod.rs"),
        ),
        ("ui/pin_wizard.rs", include_str!("../src/ui/pin_wizard.rs")),
        (
            "controller/events/pins.rs",
            include_str!("../src/controller/events/pins.rs"),
        ),
    ];
    let drills = SOURCES[0].1;
    assert!(
        drills.contains("\"Add a {d:.2} mm drill\""),
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

/// R5b: the pin block stays inside the inspector at the two default
/// widths, with its warning (two 6 mm pins on the axis) and with no pins.
#[test]
fn the_pin_block_fits_the_inspector_r5b() {
    for width in [240.0_f32, 280.0] {
        for pins in [mini_pins(), Vec::new()] {
            let ctx = ctx();
            let state = stock_state(pins.clone());
            let session = &state.session;
            let stock = session.stock_config();
            let report = session.pin_set_report_for(&stock.alignment_pins);
            let drills = properties::pins::PinDrillState::of(session, &stock.alignment_pins);
            let summary = properties::pins::summary(&report, &drills, stock.alignment_pins.len());
            for pass in 0..2 {
                let (mut inner, mut content) = (0.0_f32, 0.0_f32);
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    side_panel(ui, SidePanelEdge::Right, "setup_properties", width, |ui| {
                        inner = ui.max_rect().width();
                        content = ui
                            .scope(|ui| {
                                let _ = properties::pins::draw(ui, &summary);
                            })
                            .response
                            .rect
                            .width();
                    });
                });
                out.textures_delta.clear();
                assert!(inner > 0.0, "vacuous measurement");
                assert!(
                    content <= inner + 0.5,
                    "pass {pass} at {width} ({} pins): the pin block is {content:.1} wide in \
                     {inner:.1}",
                    pins.len()
                );
            }
        }
    }
}
