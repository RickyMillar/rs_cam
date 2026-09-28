//! G-EMPTYADD (operator, 2026-09-28): "there is no way to add a toolpath
//! when there are none in the gui?"
//!
//! The way in existed, but it was a lone one-character `+` menu under the
//! "No toolpaths" empty state, and the empty state itself offered nothing.
//! The empty state now carries its one action (§4.8): a primary "Add
//! toolpath" button that opens the same menu as `+`. With rows in the list,
//! the `+` stays and the empty state (with its button) is gone.
//!
//! ```text
//! cargo test -p rs_cam_viz --test an_empty_queue_offers_add_toolpath_g_emptyadd
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
use rs_cam_viz::ui::{tokens, toolpath_panel};

const PANEL_WIDTH: f32 = 240.0;

fn model() -> LoadedModel {
    LoadedModel {
        id: 1,
        path: PathBuf::from("emptyadd_fixture.svg"),
        name: "Empty add fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
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

/// One setup, one tool, one 2D model, no toolpath.
fn empty_state() -> AppState {
    let mut state = AppState::new();
    state.session = ProjectSessionBuilder::new()
        .tool(tool())
        .model(model())
        .build();
    state
}

/// The same project with one Pocket.
fn state_with_a_toolpath() -> AppState {
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Pocket".to_owned(),
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
        face_selection: None,
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

/// Run one frame of the toolpath panel and return every text it painted.
fn frame(ctx: &egui::Context, state: &mut AppState, input: egui::RawInput) -> Vec<Painted> {
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::left("emptyadd_toolpath_tree")
            .default_size(PANEL_WIDTH)
            .show(ui, |ui| {
                let mut events = Vec::new();
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
    painted
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

#[test]
fn an_empty_queue_paints_an_add_toolpath_button_that_opens_the_add_menu_g_emptyadd() {
    let ctx = ctx();
    let mut state = empty_state();
    let _ = frame(&ctx, &mut state, egui::RawInput::default());
    let painted = frame(&ctx, &mut state, egui::RawInput::default());
    let texts: Vec<&str> = painted.iter().map(|p| p.text.as_str()).collect();
    assert!(texts.contains(&"No toolpaths"), "{texts:?}");
    let button = painted
        .iter()
        .find(|p| p.text == "Add toolpath")
        .unwrap_or_else(|| panic!("the empty queue paints no Add toolpath action: {texts:?}"));
    assert!(
        !texts.contains(&"+"),
        "an empty single-setup queue offers ONE way in, not the button and `+`: {texts:?}"
    );

    let [press, release] = click_at(button.centre);
    let _ = frame(&ctx, &mut state, press);
    let _ = frame(&ctx, &mut state, release);
    let open = frame(&ctx, &mut state, egui::RawInput::default());
    let open_texts: Vec<&str> = open.iter().map(|p| p.text.as_str()).collect();
    assert!(
        open_texts.contains(&"2.5D (from SVG)") && open_texts.contains(&"3D (from STL)"),
        "a click on Add toolpath opens the add menu: {open_texts:?}"
    );
}

#[test]
fn a_queue_with_rows_keeps_the_plus_menu_and_no_empty_state_g_emptyadd() {
    let ctx = ctx();
    let mut state = state_with_a_toolpath();
    let _ = frame(&ctx, &mut state, egui::RawInput::default());
    let painted = frame(&ctx, &mut state, egui::RawInput::default());
    let texts: Vec<&str> = painted.iter().map(|p| p.text.as_str()).collect();
    assert!(
        texts.contains(&"+"),
        "the add menu stays below the rows: {texts:?}"
    );
    assert!(
        !texts.contains(&"Add toolpath") && !texts.contains(&"No toolpaths"),
        "a queue with rows draws no empty state: {texts:?}"
    );
}
