//! H2 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the Head
//! section of the Machine panel, the dust shoe row of the toolpath Heights
//! tab and the run sheet fit the side panel.
//!
//! The harness is the G-PANELFIT one (`every_side_panel_fits_its_width_g_panelfit.rs`):
//! the real `side_panel` helper at 240 and 280 points, two frames, and the
//! content width against the inner width. Each case also asserts that the
//! section painted its anchor text (a non-vacuity anchor): a section that
//! does not draw fits any width.

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
use rs_cam_core::machine::head::{MachineHead, ShoeSetting, ShoeTravel};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSessionBuilder, SetMachineHeadArgs, SetToolpathShoeArgs,
    ToolpathConfig,
};
use rs_cam_viz::app::{SidePanelEdge, side_panel};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::properties::ToolpathTab;
use rs_cam_viz::ui::properties::head::PartList;
use rs_cam_viz::ui::{properties, shoe_list, tokens};

const WIDTHS: [f32; 2] = [240.0, 280.0];
const SLACK: f32 = 0.5;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One tool, one 2D model, one toolpath, and the Dust shoe head with a
/// travel range.
fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 6.0;
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Shoe row fixture".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
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
        path: PathBuf::from("shoe_row_fixture.svg"),
        name: "Shoe row fixture".to_owned(),
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
        .stock(StockConfig::default())
        .tool(tool)
        .model(model);
    builder.add_toolpath(0, config).unwrap();
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let mut head = MachineHead::dust_shoe();
    if let Some(shoe) = head.shoe.as_mut() {
        shoe.travel_mm = Some(ShoeTravel {
            min_mm: -40.0,
            max_mm: 10.0,
        });
    }
    let _ = state
        .apply(Command::SetMachineHead(SetMachineHeadArgs {
            head: Box::new(head),
        }))
        .unwrap();
    let id = state.session().toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state
}

/// Two frames of `draw` in the real helper at `width`. Returns the inner
/// width, the widest content and the painted text of the last frame.
fn measure(
    width: f32,
    state: &mut AppState,
    draw: fn(&mut egui::Ui, &mut AppState),
) -> (f32, f32, Vec<String>) {
    let ctx = ctx();
    let mut inner = 0.0_f32;
    let mut content = 0.0_f32;
    let mut painted = Vec::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            side_panel(ui, SidePanelEdge::Right, "h2_fit", width, |ui| {
                inner = ui.max_rect().width();
                let scope = ui.scope(|ui| draw(ui, state));
                content = content.max(scope.response.rect.width());
            });
        });
        painted = out
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
    }
    (inner, content, painted)
}

fn draw_properties(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    properties::draw(ui, state, &mut events);
}

fn draw_run_sheet(ui: &mut egui::Ui, state: &mut AppState) {
    shoe_list::draw_run_sheet(ui, state);
}

fn check(
    name: &str,
    anchor: &str,
    mut state: impl FnMut() -> AppState,
    draw: fn(&mut egui::Ui, &mut AppState),
) {
    for width in WIDTHS {
        let mut state = state();
        let (inner, content, painted) = measure(width, &mut state, draw);
        assert!(
            inner > 0.0,
            "{name}: no inner width; the measure is vacuous"
        );
        assert!(
            painted.iter().any(|text| text.contains(anchor)),
            "{name}: the anchor {anchor:?} did not paint; the measure is vacuous"
        );
        assert!(
            content <= inner + SLACK,
            "{name}: the content used {content:.2} of {inner:.2} points at {width}"
        );
    }
}

/// The Head section alone. The Machine panel above it is wider than the
/// rail at HEAD 9e466f2b (249 of 215 points at 240, measured with the Head
/// section removed), so the whole panel is not the instrument here.
#[test]
fn the_head_section_with_an_open_part_and_a_refusal_fits_the_rail_h2() {
    for open in [
        None,
        Some((PartList::Fixed, 0)),
        Some((PartList::Shoe, 0)),
        Some((PartList::Shoe, 1)),
    ] {
        check(
            "head section",
            "Fixed parts, from the holder face up",
            || {
                let mut state = fixture();
                state.panels.head_part_open = open;
                state
            },
            draw_head_only,
        );
    }
    // Non-vacuity: the section draws the part list, so a click target
    // exists; the open part survives a frame with no edit.
    let mut state = fixture();
    state.panels.head_part_open = Some((PartList::Shoe, 1));
    let _ = measure(240.0, &mut state, draw_head_only);
    assert_eq!(state.panels.head_part_open, Some((PartList::Shoe, 1)));
}

/// The Machine panel host: a draw with no edit keeps no draft, so an MCP
/// edit of the head shows on the next frame.
#[test]
fn a_machine_panel_draw_with_no_edit_keeps_no_head_draft_h2() {
    let mut state = fixture();
    state.selection = Selection::Machine;
    state.panels.head_part_open = Some((PartList::Shoe, 1));
    let _ = measure(240.0, &mut state, draw_properties);
    assert!(state.panels.head_editor.is_none());
    assert_eq!(state.panels.head_part_open, Some((PartList::Shoe, 1)));
}

#[test]
fn the_dust_shoe_row_in_the_heights_tab_fits_the_rail_h2() {
    for (shoe, anchor) in [
        (ShoeSetting::Auto, "Auto: generate to solve"),
        (ShoeSetting::Gap { gap_mm: 30.0 }, "Shoe gap"),
    ] {
        check(
            "dust shoe row",
            anchor,
            || {
                let mut state = fixture();
                let _ = state
                    .apply(Command::SetToolpathShoe(SetToolpathShoeArgs {
                        index: 0,
                        shoe,
                    }))
                    .unwrap();
                let id = state.session().toolpath_configs()[0].id;
                state.selection = Selection::Toolpath(id);
                state.gui.pending_toolpath_tab = Some((id, ToolpathTab::Heights));
                state
            },
            draw_properties,
        );
    }
}

#[test]
fn the_run_sheet_fits_the_rail_h2() {
    check("run sheet", "Run sheet", fixture, draw_run_sheet);
}

/// The Head section over a draft with a refusal, so the banner draws too.
fn draw_head_only(ui: &mut egui::Ui, state: &mut AppState) {
    let stored = state.session().machine().head.clone();
    let mut editor = rs_cam_viz::state::panels::HeadEditor {
        draft: stored.clone(),
        refusal: Some(
            "the head part 'Lid' stands above a shelf; a shelf has no width limit, so it \
             must be the last part"
                .to_owned(),
        ),
    };
    let mut open = state.panels.head_part_open;
    let _ = properties::head::draw_head(ui, &stored, &mut editor, &mut open);
    state.panels.head_part_open = open;
}
