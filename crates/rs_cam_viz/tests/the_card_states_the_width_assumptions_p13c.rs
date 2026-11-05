//! Feeds contract P13c (operator ruling O3, 2026-11-05): the card states
//! the assumptions of a computed width as a visible warning.
//!
//! A roughing Pocket's width is the envelope proposal (the most removal per
//! minute inside the limits, bounded by the corner-coverage rule). No limit
//! models chip evacuation or tool life, so:
//!
//! 1. The Passes tab paints one caution ("⚠") on the SAFE CUT header row
//!    (lead fix C: it adds no row): the short core face
//!    `force_budget::COMPUTED_WIDTH_ASSUMPTIONS_FACE` (it fits at 340
//!    points), with the core text `force_budget::computed_width_assumptions_line`
//!    ("chips clear", "tool life not modelled") on its hover.
//! 2. The MCP `get_suggest_rationale` rationale (`SuggestRationale` of the
//!    profile warnings, the handler's own call) carries the same text.
//! 3. A finishing op (Parallel finish) has no such line.
//!
//! The CLI half is `rs_cam_cli` `suggest::tests::the_suggest_record_states_the_width_assumptions_p13c`;
//! the core half `the_add_door_stepover_is_the_proposal_p13c`.
//!
//! Red before P13c (HEAD 4efcb4eaa): `force_budget` does not exist (no
//! compile); no warning named the assumptions.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_card_states_the_width_assumptions_p13c
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::ToolFact;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::DressupConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::force_budget::{
    COMPUTED_WIDTH_ASSUMPTIONS, COMPUTED_WIDTH_ASSUMPTIONS_FACE, computed_width_assumptions_line,
};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), kind);
    tool.name = "P13c card fixture".to_owned();
    tool.diameter = diameter;
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_cutting_length(ToolFact::Stated((diameter * 3.0).max(12.0)));
    tool.set_shank_diameter(ToolFact::Stated(diameter.max(3.175)));
    tool.shaft_diameter = diameter.max(3.175);
    tool.set_stickout(ToolFact::Stated(tool.cutting_length_planning_mm() + 8.0));
    tool
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

/// The inspector rail the Toolpaths workspace draws at.
const PANEL_WIDTH: f32 = 280.0;

fn state_for(
    tool: ToolConfig,
    material: Material,
    operation: OperationConfig,
    dressups: DressupConfig,
) -> AppState {
    let stock = StockConfig {
        material,
        ..Default::default()
    };
    let config = ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "P13c card fixture".to_owned(),
        enabled: true,
        operation,
        dressups,
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
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
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("p13c_card_fixture.svg"),
        name: "P13c card fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the P13c card fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let id = state.session().toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    let _ = state.set_focus(Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Passes));
    state
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips too, which is where the plunge detail lives.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Every text run the Feeds tab paints on its steady-state frame.
fn painted_text(state: &mut AppState) -> Vec<String> {
    open_explore(state);
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("p13c_card_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
                    });
                });
            let mut events = Vec::new();
            rs_cam_viz::ui::feeds::draw(ui.ctx(), &*state, &mut events);
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

#[test]
fn the_card_and_mcp_state_the_width_assumptions_p13c() {
    let line = computed_width_assumptions_line();
    for note in COMPUTED_WIDTH_ASSUMPTIONS {
        assert!(line.contains(note), "the core line names {note:?}: {line}");
    }
    let mut state = state_for(
        tool_of(ToolType::EndMill, 6.0),
        softwood(),
        OperationConfig::new_default(OperationType::Pocket),
        DressupConfig::default(),
    );
    // 2. MCP: the handler's own call on the profile warnings.
    {
        let session = state.session();
        let tc = &session.toolpath_configs()[0];
        let profile = session.cutter_op_profile(tc).expect("the profile");
        let rationale =
            rs_cam_core::feeds::rationale::SuggestRationale::from_warnings(&profile.warnings);
        let text = serde_json::to_string(&rationale).expect("json");
        assert!(text.contains(&line), "MCP rationale lacks the line: {text}");
    }
    // 1. The SAFE CUT header row: the short core face (it fits at 340
    // points, lead gate W3g), the whole core line on its hover.
    let texts = painted_text(&mut state);
    let face = format!("⚠ {COMPUTED_WIDTH_ASSUMPTIONS_FACE}");
    assert!(
        texts.iter().any(|t| t == &face),
        "the tab lacks the caution `{face}`; runs were {texts:#?}"
    );
    assert!(
        texts.iter().any(|t| t.contains(&line)),
        "no hover carries the core line `{line}`; runs were {texts:#?}"
    );
    // 3. A finishing op has no computed width.
    let mut finish = state_for(
        tool_of(ToolType::BallNose, 6.0),
        softwood(),
        OperationConfig::new_default(OperationType::DropCutter),
        DressupConfig::default(),
    );
    let texts = painted_text(&mut finish);
    assert!(
        !texts
            .iter()
            .any(|t| t.contains(&line) || t.contains(COMPUTED_WIDTH_ASSUMPTIONS_FACE)),
        "a finish op paints no computed-width line"
    );
}

/// W3c (2026-10-30): the comparison card left the Feeds tab face for the
/// Explore window, so the reader opens the window for the toolpath on the
/// Feeds tab and paints it beside the inspector.
fn open_explore(state: &mut AppState) {
    let id = state
        .gui
        .pending_toolpath_tab
        .map(|(id, _)| id)
        .or_else(|| state.session().toolpath_configs().first().map(|tc| tc.id))
        .expect("a toolpath to explore");
    // W3i: the chart and the card are on the Feeds tab; no window opens.
    let _ = id;
}
