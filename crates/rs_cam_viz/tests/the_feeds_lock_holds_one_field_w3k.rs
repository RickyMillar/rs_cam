//! **W3k — the Feeds tab lock: one of feed, spindle speed and chipload
//! holds.**
//!
//! Operator, 2026-10-31: "can we have a lock button next to feed, spindle
//! and chipload? and you can lock any one of them at a time. by default
//! it's chipload." Each of the three rows has a padlock at the right end of
//! its label column. A click on one moves the lock to that row, and the
//! panel write-back puts it in the project (`ToolpathConfig::feeds_lock`).
//! The arithmetic of each lock against each edit is held by the lib tests
//! `each_lock_moves_the_dependent_field_and_holds_the_locked_one_w3k` and
//! core `feeds::lock`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::ui::{properties, tokens};

/// Room for the whole window, so the plot is drawn unclipped.
// P5b: the speed rows sit under the envelope plot, the nomogram and the op
// rows of the merged Passes tab; a tall screen keeps them on it unscrolled.
const SCREEN: egui::Vec2 = egui::Vec2::new(1400.0, 2400.0);

/// The inspector width the chart draws in.
const PANEL_WIDTH: f32 = 340.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx.all_styles_mut(|style| style.interaction.tooltip_delay = 0.0);
    ctx
}

fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;

    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };

    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = Some(1_291.0);
        config.spindle_rpm = Some(17_000);
    }
    let config = ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Hover bound fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
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
        path: PathBuf::from("hover_bound_fixture.svg"),
        name: "Hover bound fixture".to_owned(),
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
        .expect("add the hover bound fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let toolpath_id = state.session().toolpath_configs()[0].id;
    // W3i: the chart is on the Feeds tab of the selected toolpath.
    state.gui.toolpath_rt.insert(
        toolpath_id,
        rs_cam_viz::state::runtime::ToolpathRuntime::new(true),
    );
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(
        toolpath_id,
    ));
    state.gui.pending_toolpath_tab =
        Some((toolpath_id, rs_cam_viz::ui::properties::ToolpathTab::Passes));
    state
}

/// The text runs of one frame and their rects.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.job.text.clone(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect()
}

/// One frame with these events; the rects of the painted text.
fn run(
    ctx: &egui::Context,
    state: &mut AppState,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("feeds_lock_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    });
    let found = texts(&out);
    out.textures_delta.clear();
    found
}

/// The centre of the padlock of the row labelled `label`: the right end of
/// the label column (`tokens::LABEL_COL_WIDTH`), 10 points in.
fn padlock_of(found: &[(String, egui::Rect)], label: &str) -> egui::Pos2 {
    let rect = found
        .iter()
        .find(|(t, _)| t == label)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("the `{label}` row paints"));
    egui::pos2(
        rect.left() + tokens::LABEL_COL_WIDTH - 10.0,
        rect.center().y,
    )
}

fn lock_of(state: &AppState) -> rs_cam_core::feeds::FeedsLock {
    state.session().toolpath_configs()[0].feeds_lock
}

/// The default lock is the chipload; a click on the Feed padlock and then
/// on the Spindle padlock moves the one lock, and the project holds it.
#[test]
fn a_padlock_click_moves_the_one_lock_w3k() {
    use rs_cam_core::feeds::FeedsLock;
    let ctx = ctx();
    let mut state = fixture();
    let mut found = Vec::new();
    for _ in 0..3 {
        found = run(&ctx, &mut state, Vec::new());
    }
    assert_eq!(lock_of(&state), FeedsLock::Chipload, "the default lock");
    for (label, lock) in [
        ("Feed", FeedsLock::Feed),
        ("Spindle", FeedsLock::Spindle),
        ("Chipload", FeedsLock::Chipload),
    ] {
        let at = padlock_of(&found, label);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let _ = run(&ctx, &mut state, vec![egui::Event::PointerMoved(at)]);
        let _ = run(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(at), press(true)],
        );
        let _ = run(&ctx, &mut state, vec![press(false)]);
        found = run(&ctx, &mut state, Vec::new());
        assert_eq!(lock_of(&state), lock, "a click on the {label} padlock");
        assert!(
            state.gui.dirty,
            "a padlock click is an operator choice the file keeps: the project is edited"
        );
        state.gui.dirty = false;
    }
}
