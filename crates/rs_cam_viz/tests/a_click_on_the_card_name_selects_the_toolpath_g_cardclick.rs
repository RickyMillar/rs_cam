//! G-CARDCLICK (operator, 2026-10-16): to select a toolpath the operator
//! had to click near the state dot. A click on the name did not select.
//!
//! The name and the tool summary were selectable labels. A selectable label
//! senses click-and-drag for text selection, and it sits above the card's
//! click sense, so it took the click. The labels are now
//! `selectable(false)`, as on the setup cards, and the card takes the click.
//!
//! ```text
//! cargo test -p rs_cam_viz --test a_click_on_the_card_name_selects_the_toolpath_g_cardclick
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::{tokens, toolpath_panel};
use rs_cam_viz::ui_command::UiCommand;

const PANEL_WIDTH: f32 = 240.0;
const NAME: &str = "Pocket";

fn model() -> LoadedModel {
    LoadedModel {
        id: 1,
        path: PathBuf::from("cardclick_fixture.svg"),
        name: "Card click fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn tool() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool
}

/// One setup, one tool, one Pocket, nothing selected.
fn state_with_a_toolpath() -> AppState {
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: NAME.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
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
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let mut builder = ProjectSessionBuilder::new().tool(tool()).model(model());
    builder
        .add_toolpath(0, config)
        .expect("add the fixture pocket");
    let mut state = AppState::new();
    state.session = builder.build();
    state.selection = Selection::None;
    state
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One text run the panel painted, with the centre of its galley.
struct Painted {
    text: String,
    centre: egui::Pos2,
}

/// Run one frame of the toolpath panel. Return every text it painted and
/// every event it emitted.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    input: egui::RawInput,
) -> (Vec<Painted>, Vec<AppEvent>) {
    let mut events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::left("cardclick_toolpath_tree")
            .default_size(PANEL_WIDTH)
            .show(ui, |ui| {
                let panel_ctx = toolpath_panel::PanelContext {
                    analysis_simulating: false,
                    plan: None,
                    pending_confirm: None,
                };
                toolpath_panel::draw(ui, state, &panel_ctx, &mut events);
            });
    });
    let painted = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some(Painted {
                text: text.galley.job.text.clone(),
                centre: egui::Rect::from_min_size(text.pos, text.galley.size()).center(),
            }),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    (painted, events)
}

fn click_at(pos: egui::Pos2) -> [egui::RawInput; 2] {
    let press = egui::RawInput {
        events: vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let release = egui::RawInput {
        events: vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    [press, release]
}

fn selects_the_pocket(event: &AppEvent) -> bool {
    matches!(
        event,
        AppEvent::Ui(UiCommand::Select(Selection::Toolpath(id))) if id.0 == 0
    )
}

#[test]
fn a_click_on_the_card_name_selects_the_toolpath_g_cardclick() {
    let ctx = ctx();
    let mut state = state_with_a_toolpath();
    // Two idle frames: egui hit-tests against the widget rects of the frame
    // before, so the card and its row must exist before the click.
    let _ = frame(&ctx, &mut state, egui::RawInput::default());
    let (painted, idle_events) = frame(&ctx, &mut state, egui::RawInput::default());
    let texts: Vec<&str> = painted.iter().map(|p| p.text.as_str()).collect();
    // Non-vacuity: the card painted its name, so the click lands on a label.
    let name = painted
        .iter()
        .find(|p| p.text == NAME)
        .unwrap_or_else(|| panic!("the card paints no `{NAME}` label: {texts:?}"));
    assert!(
        !idle_events.iter().any(selects_the_pocket),
        "an idle frame selects nothing: {idle_events:?}"
    );

    let [press, release] = click_at(name.centre);
    let (_, mut events) = frame(&ctx, &mut state, press);
    let (_, release_events) = frame(&ctx, &mut state, release);
    events.extend(release_events);
    assert!(
        events.iter().any(selects_the_pocket),
        "a click on the card's name selects the toolpath: {events:?}"
    );
}
