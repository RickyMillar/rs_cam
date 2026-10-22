//! The shared harness of the P6.7 sentries (geometry selection P6.7): a
//! project of one drawing, one tool and one toolpath, and the production
//! inspector (`properties::draw`) on a headless `Context`.
//!
//! A click follows the shape of a real pointer, as in
//! `the_selection_section_writes_edits_only_p4_3.rs`: three passes settle
//! the layout and find the label, one pass presses and one pass releases.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::selection::ItemSet;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::{AppEvent, properties, tokens};

/// The narrowest inspector: the right rail of the Simulation workspace.
pub const RAIL_WIDTH: f32 = 240.0;

/// Tall enough that every row of the Geometry tab is on screen.
const SCREEN_HEIGHT: f32 = 4000.0;

/// The id of the one tool and of the one model.
pub const TOOL: usize = 1;
pub const MODEL: usize = 1;

/// A drawing model of `items`.
pub fn drawing_model(items: ItemSet) -> LoadedModel {
    LoadedModel {
        id: MODEL,
        path: PathBuf::from("synthetic://p6_7.dxf"),
        name: "P6.7 drawing".to_owned(),
        kind: Some(ModelKind::Dxf),
        mesh: None,
        items: Arc::new(items),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// A toolpath of `operation` on the model with the tool.
pub fn toolpath(name: &str, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: TOOL,
        model_id: MODEL,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

/// A project of `model`, an end mill of `tool_diameter`, `stock` and one
/// toolpath of `operation`, shown on its Geometry tab.
pub fn state(
    model: LoadedModel,
    tool_diameter: f64,
    stock: StockConfig,
    operation: OperationConfig,
) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill);
    tool.diameter = tool_diameter;
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, toolpath("P6.7", operation))
        .expect("add the toolpath");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let parts = state.split_mut();
    for config in parts.session.toolpath_configs() {
        parts
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    let id = state.session().toolpath_configs()[0].id;
    state.selection = UiSelection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
    state
}

/// A headless context with the app theme and fonts.
pub fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One painted text galley.
#[derive(Debug, Clone)]
pub struct Painted {
    pub text: String,
    /// The number of rows: more than one is a wrap.
    pub rows: usize,
    /// The colour of the first section.
    pub color: Option<egui::Color32>,
    /// The centre of the text on screen.
    pub centre: egui::Pos2,
    /// The right edge of the text on screen.
    pub right: f32,
    /// The alpha of the paint colour of the text (the override colour, else
    /// the fallback colour): below 255 for a greyed (disabled) widget, which
    /// egui paints at `disabled_alpha`.
    pub alpha: u8,
}

/// One pass of the inspector at `width` with `input_events`: the painted
/// text and the app events.
fn pass(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
    input_events: Vec<egui::Event>,
) -> (Vec<Painted>, Vec<AppEvent>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, SCREEN_HEIGHT),
        )),
        events: input_events,
        ..Default::default()
    };
    let mut events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        ui.set_max_width(width);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(width);
            properties::draw(ui, state, &mut events);
        });
    });
    let painted = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if !text.galley.job.text.trim().is_empty() => {
                Some(Painted {
                    text: text.galley.job.text.clone(),
                    rows: text.galley.rows.len(),
                    color: text.galley.job.sections.first().map(|s| s.format.color),
                    centre: clipped.shape.visual_bounding_rect().center(),
                    right: clipped.shape.visual_bounding_rect().max.x,
                    alpha: text.override_text_color.unwrap_or(text.fallback_color).a(),
                })
            }
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    (painted, events)
}

/// Three passes with no input; the painted text of the last.
pub fn render(ctx: &egui::Context, state: &mut AppState, width: f32) -> Vec<Painted> {
    let mut last = Vec::new();
    for _ in 0..3 {
        last = pass(ctx, state, width, Vec::new()).0;
    }
    last
}

/// Click the `nth` painted text equal to `label` and return every event
/// of the five passes.
pub fn click(ctx: &egui::Context, state: &mut AppState, label: &str, nth: usize) -> Vec<AppEvent> {
    let mut target: Option<egui::Pos2> = None;
    let mut all = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for at in 0..5 {
        let mut input_events = Vec::new();
        if at >= 3 {
            let pos = target
                .unwrap_or_else(|| panic!("no painted text '{label}' (#{nth}); painted: {seen:?}"));
            input_events.push(egui::Event::PointerMoved(pos));
            input_events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: at == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
        let (painted, events) = pass(ctx, state, RAIL_WIDTH, input_events);
        if at == 2 {
            // A button paints its label in two shapes at one place, so the
            // matches count by place, not by shape.
            let mut places: Vec<egui::Pos2> = Vec::new();
            for p in &painted {
                if p.text == label && !places.iter().any(|q| q.distance(p.centre) < 1.0) {
                    places.push(p.centre);
                }
            }
            seen = painted.iter().map(|p| p.text.clone()).collect();
            target = places.get(nth).copied();
        }
        all.extend(events);
    }
    all
}

/// The stored operation of toolpath 0.
pub fn stored(state: &AppState) -> OperationConfig {
    state.session().toolpath_configs()[0].operation.clone()
}

/// The painted texts that equal `text`.
pub fn find<'a>(painted: &'a [Painted], text: &str) -> Vec<&'a Painted> {
    painted.iter().filter(|p| p.text == text).collect()
}

/// The painted texts that contain `text`.
pub fn containing<'a>(painted: &'a [Painted], text: &str) -> Vec<&'a Painted> {
    painted.iter().filter(|p| p.text.contains(text)).collect()
}

/// The texts that wrap to more than one row and do not paint in the
/// danger colour: the IC1 rule (`the_inspector_text_is_bounded_ic1`).
pub fn wrapped(painted: &[Painted]) -> Vec<String> {
    painted
        .iter()
        .filter(|p| p.rows > 1 && p.color != Some(tokens::DANGER))
        .map(|p| p.text.clone())
        .collect()
}

/// The texts that paint past the right edge of the 240-point rail: a row
/// that is too wide for the rail.
pub fn past_the_rail(painted: &[Painted]) -> Vec<String> {
    painted
        .iter()
        .filter(|p| p.right > RAIL_WIDTH + 1.0)
        .map(|p| p.text.clone())
        .collect()
}

/// Click the `nth` painted text equal to `label`, then return the painted
/// text after 30 idle passes: an open popup stays open, and its fade-in
/// (egui animates the opacity of a new area) ends.
pub fn click_and_look(
    ctx: &egui::Context,
    state: &mut AppState,
    label: &str,
    nth: usize,
) -> Vec<Painted> {
    let _ = click(ctx, state, label, nth);
    let mut last = Vec::new();
    for _ in 0..30 {
        last = pass(ctx, state, RAIL_WIDTH, Vec::new()).0;
    }
    last
}
