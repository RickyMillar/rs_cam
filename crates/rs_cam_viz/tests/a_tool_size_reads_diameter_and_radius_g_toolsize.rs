//! **G-TOOLSIZE — a tool size says which figure is a diameter and which is
//! a radius.**
//!
//! # The defect (operator, 2026-09-24)
//!
//! Vendor names give the tip RADIUS: "R1.0mm x 6mm x 20mm 2F Tapered Ball"
//! is a 1.0 mm tip radius on a 6 mm shank. `ToolConfig::diameter` holds the
//! tip DIAMETER, 2.0. The GUI printed that number bare, under labels that
//! did not agree: "Diameter:" in the tool panel and "2.00 mm" in the Feeds
//! card. The operator read "R1.0" beside "2.00 mm" and could not tell which
//! was which.
//!
//! # The convention
//!
//! `ToolConfig::size_label` is the one formatter: `Ø` prefixes a diameter
//! and `R` a radius, and a tapered ball says "tip". This file renders the
//! real panels headless and reads every painted text run:
//!
//! * the tool panel labels a tapered ball's `diameter` "Tip Ø:", shows
//!   "= R…" beside it, and never paints a bare "Diameter" label;
//! * the tool panel paints the name/size CAUTION when the name's tip size
//!   disagrees with the geometry, and paints nothing when they agree;
//! * the Feeds card header paints "tip Ø… (R…)" for a tapered ball;
//! * no surface paints the other diameter glyph `⌀` (U+2300).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

const PANEL_WIDTH: f32 = 320.0;

/// The vendor name of the rivmap R1.0 tapered ball.
const R1_NAME: &str = "R1.0mm x 6mm x 20mm 2F Tapered Ball";

fn tapered_ball(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    tool.name = R1_NAME.to_owned();
    tool.diameter = diameter;
    tool.taper_half_angle = 5.7;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.cutting_length = 20.0;
    tool.flute_count = 2;
    tool
}

/// A session with one tool and one pocket on it. `select_tool` puts the
/// tool panel in the inspector; otherwise the toolpath's Feeds tab.
fn state_for(tool: ToolConfig, select_tool: bool) -> AppState {
    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Tool size fixture".to_owned(),
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
    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("tool_size_fixture.svg"),
        name: "Tool size fixture".to_owned(),
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
        .expect("add the tool size fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    if select_tool {
        state.selection = Selection::Tool(ToolId(1));
    } else {
        state.selection = Selection::Toolpath(id);
        state.gui.pending_toolpath_tab =
            Some((id, rs_cam_viz::ui::properties::ToolpathTab::FeedsSpeeds));
    }
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

/// Every text run the inspector paints on the second (steady) frame.
fn painted_text(mut state: AppState) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("toolsize_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, &mut state, &mut events);
                    });
                });
        });
        if pass == 1 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

fn assert_no_bare_diameter(texts: &[String]) {
    // Non-vacuity: the panel painted its name.
    assert!(
        texts.iter().any(|t| t.contains(R1_NAME)),
        "the panel painted nothing recognisable; runs were {texts:#?}"
    );
    let bare: Vec<&String> = texts
        .iter()
        .filter(|t| t.trim_start().starts_with("Diameter"))
        .collect();
    assert!(
        bare.is_empty(),
        "a tapered-ball tip was labelled \"Diameter\": {bare:?}"
    );
    let other_glyph: Vec<&String> = texts.iter().filter(|t| t.contains('\u{2300}')).collect();
    assert!(
        other_glyph.is_empty(),
        "a surface painted \u{2300} instead of \u{00D8}: {other_glyph:?}"
    );
}

#[test]
fn the_tool_panel_labels_the_tip_and_its_radius_g_toolsize() {
    let texts = painted_text(state_for(tapered_ball(2.0), true));
    assert_no_bare_diameter(&texts);
    assert!(
        texts.iter().any(|t| t == "Tip \u{00D8}:"),
        "no \"Tip \u{00D8}:\" label; runs were {texts:#?}"
    );
    assert!(
        texts.iter().any(|t| t == "= R1.00"),
        "no derived radius beside the tip field; runs were {texts:#?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains("tip \u{00D8}2.00 mm (R1.00), shank \u{00D8}6.00")),
        "no size summary under the name; runs were {texts:#?}"
    );
    // The name and the geometry agree, so no caution paints.
    assert!(
        !texts.iter().any(|t| t.contains("The name says")),
        "a caution painted on a correct tool; runs were {texts:#?}"
    );
}

#[test]
fn the_tool_panel_cautions_when_the_name_disagrees_g_toolsize() {
    // The fixture defect: an "R1.0" name on a Ø1.00 tip.
    let texts = painted_text(state_for(tapered_ball(1.0), true));
    assert_no_bare_diameter(&texts);
    let caution = "The name says R1.00 (\u{00D8}2.00 mm) but the tool is \u{00D8}1.00 mm. \
                   Check the diameter.";
    assert!(
        texts.iter().any(|t| t.contains(caution)),
        "no name/size caution; runs were {texts:#?}"
    );
}

#[test]
fn the_feeds_card_header_names_the_tip_g_toolsize() {
    let texts = painted_text(state_for(tapered_ball(2.0), false));
    // Non-vacuity: the card's context chip painted.
    assert!(
        texts.iter().any(|t| t == "Tool:"),
        "the Feeds card painted no context chip; runs were {texts:#?}"
    );
    let header: Vec<&String> = texts
        .iter()
        .filter(|t| t.contains("flute \u{00B7}"))
        .collect();
    assert_eq!(header.len(), 1, "expected one header run, got {header:?}");
    assert!(
        header[0].starts_with("tip \u{00D8}2.00 mm (R1.00), shank \u{00D8}6.00"),
        "the header does not name the tip: {:?}",
        header[0]
    );
    let other_glyph: Vec<&String> = texts.iter().filter(|t| t.contains('\u{2300}')).collect();
    assert!(
        other_glyph.is_empty(),
        "a surface painted \u{2300} instead of \u{00D8}: {other_glyph:?}"
    );
}

// ── Imperial (operator ruling 2026-09-24: "if it's in imperial, we just need
// to keep that convention") ─────────────────────────────────────────────────

fn end_mill(name: &str, diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.name = name.to_owned();
    tool.diameter = diameter;
    tool.shank_diameter = 6.35;
    tool.flute_count = 2;
    tool
}

/// One frame of the inspector: every painted text run with its rectangle.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let input = egui::RawInput {
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("toolsize_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut app_events = Vec::new();
                    properties::draw(ui, state, &mut app_events);
                });
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

#[test]
fn an_imperial_tool_reads_in_inches_g_toolsize() {
    // Carbide-style name: the 1/4" selects Imperial with no stored unit.
    let texts = painted_text(state_for(end_mill("#201 1/4\" Square", 6.35), true));
    assert!(
        texts
            .iter()
            .any(|t| t.contains("End Mill \u{00D8}1/4\" (6.35 mm)")),
        "no imperial size summary; runs were {texts:#?}"
    );
    // The field shows the size in inches, and the name agrees with it.
    assert!(
        texts.iter().any(|t| t == "1/4\""),
        "the diameter field is not in inches; runs were {texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t.contains("The name says")),
        "a caution painted on a correct imperial tool; runs were {texts:#?}"
    );
}

#[test]
fn the_size_selector_switches_the_field_unit_g_toolsize() {
    // A metric name on a 1/4" tool: the field starts in mm.
    let mut state = state_for(end_mill("Square end mill", 6.35), true);
    let ctx = ctx();
    let _ = frame(&ctx, &mut state, Vec::new());
    let before = frame(&ctx, &mut state, Vec::new());
    assert!(
        !before.iter().any(|(t, _)| t == "1/4\""),
        "the field is in inches before the switch; runs were {before:#?}"
    );
    assert!(
        before.iter().any(|(t, _)| t == "Size in"),
        "no size unit selector; runs were {before:#?}"
    );
    let inch = before
        .iter()
        .find(|(t, _)| t == "inch")
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| panic!("no \"inch\" option; runs were {before:#?}"));

    // Click "inch": move, press, release, then one frame to repaint.
    let _ = frame(&ctx, &mut state, vec![egui::Event::PointerMoved(inch)]);
    let press = |pressed| egui::Event::PointerButton {
        pos: inch,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(&ctx, &mut state, vec![press(true)]);
    let _ = frame(&ctx, &mut state, vec![press(false)]);
    let after = frame(&ctx, &mut state, Vec::new());
    assert!(
        after.iter().any(|(t, _)| t == "1/4\""),
        "the field did not switch to inches; runs were {after:#?}"
    );
    assert!(
        after
            .iter()
            .any(|(t, _)| t.contains("End Mill \u{00D8}1/4\" (6.35 mm)")),
        "the summary did not switch to inches; runs were {after:#?}"
    );
    // Display only: the stored diameter is still mm.
    let draft = state
        .history
        .tool_draft
        .as_ref()
        .map(|(_, t)| (t.diameter, t.size_units))
        .expect("the tool panel holds a draft");
    assert_eq!(
        draft,
        (
            6.35,
            Some(rs_cam_core::compute::tool_config::SizeUnits::Imperial)
        )
    );
}
