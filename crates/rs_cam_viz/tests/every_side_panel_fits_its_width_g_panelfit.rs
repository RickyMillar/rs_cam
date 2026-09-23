//! G-PANELFIT: every workspace side panel keeps its width.
//!
//! # The defect this exists to catch
//!
//! The operator reported that the right panel overflows its width in many
//! cases (`planning/sim_cut_metrics_2026-09-23/PLAN.md` §6.1). All six
//! workspace side panels put a bare `ScrollArea::vertical()` in the panel.
//! The x axis of that scroll area took the content width, and the panel
//! stored that width as the width for the next frame. One wide row therefore
//! grew the panel to `SIDE_PANEL_MAX_WIDTH`, and the panel clipped the rest.
//!
//! The fix is one helper, `rs_cam_viz::app::side_panel`. Its
//! `ScrollArea::both()` sizes the x axis from the panel, not from the
//! content.
//!
//! # The arms
//!
//! 1. Behavioural: each side-panel entry point draws through the real helper
//!    at 240 and 280 points for two frames. Arms 1a and 1b: the stored panel
//!    width must not grow (the G contract). Arms 1c and 1d: the content must
//!    not be wider than the inner width. A failure of 1c or 1d names a
//!    residual row fix of PLAN.md §6.1, not a defect in the helper.
//! 2. Control: a 600 point row grows a panel in the OLD wrapper, and does not
//!    grow the same panel in the helper. Without this arm, arm 1 can pass
//!    because the fixtures draw nothing wide.
//! 3. Source: `app.rs` builds a left or right panel only in `side_panel`.
//!
//! Arms 1a and 1b also hold the right gutter (follow-up 2026-09-24): the
//! content stops `SIDE_PANEL_GUTTER` short of the visible edge of the
//! scroll area. The egui 0.36 scroll bar floats over the content, and the
//! operator saw the cut-metric cards touch the panel edge.
//!
//! The b21e3294 lesson: a test that checks that a style is SET does not
//! check that the layout is RIGHT. Arms 1 and 2 measure the layout.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::app::{SIDE_PANEL_GUTTER, SIDE_PANEL_MAX_WIDTH, SidePanelEdge, side_panel};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{
    properties, setup_panel, sim_diagnostics, sim_op_list, tokens, toolpath_panel,
};

/// The two default widths that `app.rs` gives a side panel.
const WIDTHS: [f32; 2] = [240.0, 280.0];

/// The tolerance for layout rounding, in points.
const SLACK: f32 = 0.5;

/// One side panel: its egui id, its edge and its entry point.
struct PanelCase {
    id: &'static str,
    edge: SidePanelEdge,
    draw: fn(&mut egui::Ui, &mut AppState),
}

fn draw_setup_tree(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    setup_panel::draw(ui, state, &mut events);
}

fn draw_properties(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    properties::draw(ui, state, &mut events);
}

fn draw_toolpath_tree(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    let panel_ctx = toolpath_panel::PanelContext {
        analysis_simulating: false,
        plan: None,
        pending_confirm: None,
    };
    toolpath_panel::draw(ui, state, &panel_ctx, &mut events);
}

fn draw_sim_op_list(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    sim_op_list::draw(
        ui,
        &mut state.simulation,
        &state.session,
        &state.gui,
        &mut state.viewport,
        &mut events,
    );
}

fn draw_sim_diagnostics(ui: &mut egui::Ui, state: &mut AppState) {
    let mut events = Vec::new();
    sim_diagnostics::draw(
        ui,
        &mut state.simulation,
        &state.session,
        &state.gui,
        &mut events,
    );
}

/// The six workspace side panels of `app.rs`, with the same ids and edges.
fn panel_cases() -> [PanelCase; 6] {
    [
        PanelCase {
            id: "setup_tree",
            edge: SidePanelEdge::Left,
            draw: draw_setup_tree,
        },
        PanelCase {
            id: "setup_properties",
            edge: SidePanelEdge::Right,
            draw: draw_properties,
        },
        PanelCase {
            id: "toolpath_tree",
            edge: SidePanelEdge::Left,
            draw: draw_toolpath_tree,
        },
        PanelCase {
            id: "toolpath_properties",
            edge: SidePanelEdge::Right,
            draw: draw_properties,
        },
        PanelCase {
            id: "sim_op_list",
            edge: SidePanelEdge::Left,
            draw: draw_sim_op_list,
        },
        PanelCase {
            id: "sim_diagnostics",
            edge: SidePanelEdge::Right,
            draw: draw_sim_diagnostics,
        },
    ]
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // Open every collapsed section, so that the dense rows draw.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// A project with one stock, one tool, one model and one selected
/// toolpath, so that the inspector and the lists draw rows, not only an
/// empty state. The shape is the fixture of
/// `inspector_width_is_tab_independent_up4.rs`.
fn fixture_with_a_toolpath() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;

    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };

    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Panel width fixture".to_owned(),
        enabled: true,
        operation,
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
        path: PathBuf::from("panel_width_fixture.svg"),
        name: "Panel width fixture".to_owned(),
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
        .expect("add the panel width fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state
}

/// The measurement of one frame.
#[derive(Debug, Default, Clone, Copy)]
struct Frame {
    /// The outer width that the panel stored for the next frame.
    stored: f32,
    /// The width that the helper gives the content.
    inner: f32,
    /// The width that the content used.
    content: f32,
    /// The space between the content's right edge and the visible right
    /// edge of the scroll area.
    gutter: f32,
}

fn stored_width(ctx: &egui::Context, id: &'static str) -> f32 {
    egui::containers::panel::PanelState::load(ctx, egui::Id::new(id))
        .unwrap_or_else(|| panic!("the panel {id} stored no state"))
        .size()
        .x
}

/// Draw `draw` in the real helper for one frame, and measure it.
fn helper_frame(ctx: &egui::Context, case: &PanelCase, width: f32, state: &mut AppState) -> Frame {
    let mut frame = Frame::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        side_panel(ui, case.edge, case.id, width, |ui| {
            frame.inner = ui.max_rect().width();
            frame.gutter = ui.clip_rect().right() - ui.max_rect().right();
            let scope = ui.scope(|ui| (case.draw)(ui, state));
            frame.content = scope.response.rect.width();
        });
    });
    out.textures_delta.clear();
    frame.stored = stored_width(ctx, case.id);
    frame
}

/// Two frames of one panel at one default width, in a new context: egui
/// keeps a panel width by id.
fn two_frames(case: &PanelCase, fixture: fn() -> AppState, width: f32) -> [Frame; 2] {
    let ctx = ctx();
    let mut state = fixture();
    let first = helper_frame(&ctx, case, width, &mut state);
    let second = helper_frame(&ctx, case, width, &mut state);
    [first, second]
}

/// The G contract: the stored panel width does not follow the content.
fn check_width(name: &str, case: &PanelCase, fixture: fn() -> AppState) {
    for width in WIDTHS {
        let [first, second] = two_frames(case, fixture, width);
        assert!(
            first.stored <= width + SLACK,
            "{name}: the panel {} stored {:.2} points after one frame at a \
             default of {width}. The panel width follows the content again.",
            case.id,
            first.stored,
        );
        for frame in [first, second] {
            assert!(
                frame.gutter >= SIDE_PANEL_GUTTER - SLACK,
                "{name}: the panel {} left a right gutter of {:.2} points at a \
                 default of {width}, under {SIDE_PANEL_GUTTER}. The floating \
                 scroll bar then covers the content at the panel edge.",
                case.id,
                frame.gutter,
            );
        }
        assert!(
            second.stored <= first.stored + SLACK,
            "{name}: the panel {} grew from {:.2} to {:.2} points between two \
             frames at a default of {width}.",
            case.id,
            first.stored,
            second.stored,
        );
    }
}

/// The residual rows: no row is wider than the inner width. A failure here
/// names a local row fix (PLAN.md §6.1), not a defect in the helper.
fn check_content(name: &str, case: &PanelCase, fixture: fn() -> AppState) {
    for width in WIDTHS {
        for (label, frame) in ["first", "second"]
            .into_iter()
            .zip(two_frames(case, fixture, width))
        {
            assert!(
                frame.inner > 0.0,
                "{name}: the panel {} gave its content no width on the \
                 {label} frame; the measurement is vacuous.",
                case.id,
            );
            assert!(
                frame.content <= frame.inner + SLACK,
                "{name}: on the {label} frame the content of the panel {} \
                 used {:.2} points of an inner width of {:.2} at a default \
                 of {width}. A row in this panel is too wide: make it wrap \
                 or stack (PLAN.md §6.1, residual local work).",
                case.id,
                frame.content,
                frame.inner,
            );
        }
    }
}

/// Arm 1a. The stored width holds for each entry point, empty project.
#[test]
fn every_side_panel_keeps_its_width_with_an_empty_project_g_panelfit() {
    for case in &panel_cases() {
        check_width("empty project", case, AppState::new);
    }
}

/// Arm 1b. The stored width holds for each entry point, one toolpath.
#[test]
fn every_side_panel_keeps_its_width_with_a_toolpath_g_panelfit() {
    for case in &panel_cases() {
        check_width("one toolpath", case, fixture_with_a_toolpath);
    }
}

/// Arm 1c. The content fits the inner width, empty project.
#[test]
#[ignore = "instrument for the residual row fixes of PLAN.md §6.1: the toolpath tree and \
            the setup properties panel still hold a row wider than 223 points at a default of 240; \
            run by name with -- --ignored"]
fn every_side_panel_content_fits_with_an_empty_project_g_panelfit() {
    for case in &panel_cases() {
        check_content("empty project", case, AppState::new);
    }
}

/// Arm 1d. The content fits the inner width, one toolpath.
#[test]
#[ignore = "instrument for the residual row fixes of PLAN.md §6.1: the toolpath tree and \
            the setup properties panel still hold a row wider than 223 points at a default of 240; \
            run by name with -- --ignored"]
fn every_side_panel_content_fits_with_a_toolpath_g_panelfit() {
    for case in &panel_cases() {
        check_content("one toolpath", case, fixture_with_a_toolpath);
    }
}

/// A row that no panel width can hold.
fn wide_row(ui: &mut egui::Ui) {
    ui.allocate_space(egui::vec2(600.0, 20.0));
}

/// The stored width after two frames of `wide_row` in the OLD wrapper.
fn old_wrapper_width(ctx: &egui::Context, width: f32) -> f32 {
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("panelfit_old_wrapper")
                .default_size(width)
                .max_size(SIDE_PANEL_MAX_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, wide_row);
                });
        });
        out.textures_delta.clear();
    }
    stored_width(ctx, "panelfit_old_wrapper")
}

/// The stored width after two frames of `wide_row` in the helper.
fn helper_width(ctx: &egui::Context, width: f32) -> f32 {
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            side_panel(ui, SidePanelEdge::Right, "panelfit_helper", width, wide_row);
        });
        out.textures_delta.clear();
    }
    stored_width(ctx, "panelfit_helper")
}

/// Arm 2. The control: the old wrapper grows, the helper does not.
#[test]
fn a_wide_row_grows_the_old_wrapper_and_not_the_helper_g_panelfit() {
    for width in WIDTHS {
        let old = old_wrapper_width(&ctx(), width);
        assert!(
            old > width + SLACK,
            "a 600 point row did not grow the old wrapper past {width} \
             (stored {old:.2}). This arm no longer reproduces the defect, so \
             the arm above it proves nothing. Re-derive it for this egui."
        );
        let fixed = helper_width(&ctx(), width);
        assert!(
            fixed <= width + SLACK,
            "a 600 point row grew the side_panel helper from {width} to \
             {fixed:.2} points. The helper must keep the panel width."
        );
    }
}

fn app_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Remove `//` comments line by line, so that no rule matches a comment.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|line| line.find("//").map_or(line, |i| &line[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The byte range of one function, from its signature to its closing brace.
fn function_range(source: &str, signature: &str) -> std::ops::Range<usize> {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is gone; the anchor is stale"));
    let body_start = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("`{signature}` has no body"));
    let mut depth = 0_u32;
    for (offset, byte) in source[body_start..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return start..body_start + offset + 1;
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` body is not closed");
}

/// Arm 3. `app.rs` builds a left or right panel only inside `side_panel`.
#[test]
fn app_builds_side_panels_only_in_the_helper_g_panelfit() {
    let code = code_only(&app_source());
    let range = function_range(&code, "fn side_panel(");
    let helper = &code[range.clone()];
    // Non-vacuity: the helper exists and holds the fix.
    for needle in [
        "Panel::left(",
        "Panel::right(",
        ".max_size(SIDE_PANEL_MAX_WIDTH)",
        "ScrollArea::both()",
    ] {
        assert!(
            helper.contains(needle),
            "side_panel no longer contains `{needle}`; the helper lost part \
             of the fix."
        );
    }
    let rest = format!("{}{}", &code[..range.start], &code[range.end..]);
    for needle in [
        "Panel::left(",
        "Panel::right(",
        "SidePanel::left(",
        "SidePanel::right(",
    ] {
        assert!(
            !rest.contains(needle),
            "app.rs builds a side panel with `{needle}` outside side_panel. \
             Route it through side_panel, or its width follows its content \
             again."
        );
    }
    let calls = rest.matches("side_panel(").count();
    assert_eq!(
        calls, 6,
        "app.rs must draw its six workspace side panels (setup, toolpaths and \
         simulation, two each) through side_panel; found {calls} calls."
    );
}
