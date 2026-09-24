//! G-LADDER (step-ladder Phase 4): the 3D Rough panel sets `coarse_steps`.
//!
//! Before Phase 4 the core ran a step ladder, and the GUI could not set
//! one. This file drives the production `properties::draw` through a
//! headless `Context` and reads what it painted and what it wrote.
//!
//! - Arm 1: the rows are on screen, and the ladder line reads
//!   "10 → 5 mm" for `[10]` above a 5 mm Depth/Pass.
//! - Arm 2: "+ Add" writes a step into the SESSION. The panel write-back
//!   sends `Command::ReplaceToolpathConfig`, so the test reads the core
//!   config, not the panel clone.
//! - Arm 3: a ladder on the Adaptive strategy paints the adapter's own
//!   refusal, and that text is the core function's text.
//!
//! # NOT MEASURED
//!
//! The drag edit of one step row. It is a `ValueRow`, the same component
//! as every other numeric row; its write path is the one arm 2 measures.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector's own width in the Toolpaths workspace (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// Tall enough that the whole Geometry tab is on screen, so a click on a
/// row far down the tab lands on it.
const SCREEN_HEIGHT: f32 = 4000.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One 3D Rough on the Geometry tab, with the given ladder and strategy.
fn fixture(coarse_steps: Vec<f64>, strategy: ClearingStrategy) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    let operation = OperationConfig::Adaptive3d(Adaptive3dConfig {
        depth_per_pass: 5.0,
        coarse_steps,
        clearing_strategy: strategy,
        ..Adaptive3dConfig::default()
    });
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Ladder fixture".to_owned(),
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
        path: PathBuf::from("ladder_fixture.svg"),
        name: "Ladder fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 40.0, 40.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool)
        .model(model);
    builder.add_toolpath(0, config).expect("add the fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
    state
}

/// One frame of the inspector. Returns every painted text run with the
/// centre of its shape.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Pos2)> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(PANEL_WIDTH, SCREEN_HEIGHT),
        )),
        events,
        ..Default::default()
    };
    let mut app_events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        ui.set_max_width(PANEL_WIDTH);
        properties::draw(ui, state, &mut app_events);
    });
    let texts = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some((
                text.galley.job.text.clone(),
                clipped.shape.visual_bounding_rect().center(),
            )),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    texts
}

fn painted(state: &mut AppState) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for _ in 0..3 {
        texts = frame(&ctx, state, Vec::new());
    }
    texts.into_iter().map(|(text, _)| text).collect()
}

/// Click the painted text `label`: three passes to settle the layout and
/// find it, one to press, one to release. egui resolves a click from the
/// rects of the pass before.
fn click(ctx: &egui::Context, state: &mut AppState, label: &str) {
    let mut target = None;
    for pass in 0..5 {
        let mut events = Vec::new();
        if pass >= 3 {
            let pos = target.unwrap_or_else(|| panic!("{label:?} was not painted"));
            events.push(egui::Event::PointerMoved(pos));
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: pass == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
        let texts = frame(ctx, state, events);
        if pass == 2 {
            target = texts
                .iter()
                .find(|(text, _)| text == label)
                .map(|(_, pos)| *pos);
        }
    }
}

fn session_ladder(state: &AppState) -> Vec<f64> {
    match &state.session.toolpath_configs()[0].operation {
        OperationConfig::Adaptive3d(cfg) => cfg.coarse_steps.clone(),
        other => panic!("expected Adaptive3d, got {other:?}"),
    }
}

// ── arm 1: the rows and the ladder line are on screen ────────────────────

#[test]
fn the_panel_shows_the_ladder_rows_and_line_g_ladder() {
    let mut state = fixture(vec![10.0], ClearingStrategy::ContourParallel);
    let texts = painted(&mut state);
    for expected in ["Coarse Steps:", "Coarse Step 1:", "+ Add", "Remove"] {
        assert!(
            texts.iter().any(|t| t == expected),
            "the 3D Rough panel paints no {expected:?}; painted: {texts:?}"
        );
    }
    assert!(
        texts.iter().any(|t| t == "10 \u{2192} 5 mm"),
        "the ladder line must read \"10 → 5 mm\" for [10] above a 5 mm \
         Depth/Pass; painted: {texts:?}"
    );

    // With no coarse step the line says so, and no step row is painted.
    let mut state = fixture(Vec::new(), ClearingStrategy::ContourParallel);
    let texts = painted(&mut state);
    assert!(
        texts.iter().any(|t| t == "5 mm, one step"),
        "painted: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "Coarse Step 1:"),
        "an empty ladder paints no step row"
    );
}

// ── arm 2: "+ Add" and "Remove" write the session ────────────────────────

#[test]
fn add_and_remove_write_the_session_g_ladder() {
    let ctx = ctx();
    let mut state = fixture(Vec::new(), ClearingStrategy::ContourParallel);
    assert!(session_ladder(&state).is_empty());

    click(&ctx, &mut state, "+ Add");
    assert_eq!(
        session_ladder(&state),
        vec![10.0],
        "+ Add must write two times Depth/Pass (5 mm) into the session config"
    );

    click(&ctx, &mut state, "Remove");
    assert!(
        session_ladder(&state).is_empty(),
        "Remove must take the last step out of the session config"
    );
}

// ── arm 3: a ladder on the wrong strategy shows the adapter's refusal ────

#[test]
fn a_ladder_on_the_adaptive_strategy_shows_the_refusal_g_ladder() {
    let mut state = fixture(vec![10.0], ClearingStrategy::Adaptive);
    let expected = match &state.session.toolpath_configs()[0].operation {
        OperationConfig::Adaptive3d(cfg) => {
            rs_cam_core::compute::execute::adaptive3d_step_ladder_refusal(cfg)
                .expect("the core refuses a ladder on the Adaptive strategy")
        }
        other => panic!("expected Adaptive3d, got {other:?}"),
    };
    assert!(
        expected.contains("contour parallel strategy only"),
        "{expected}"
    );
    let texts = painted(&mut state);
    assert!(
        texts.iter().any(|t| t == &expected),
        "the panel must paint the adapter's refusal, word for word: \
         {expected:?}; painted: {texts:?}"
    );

    // The same ladder on Contour Parallel paints no refusal.
    let mut state = fixture(vec![10.0], ClearingStrategy::ContourParallel);
    let texts = painted(&mut state);
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("contour parallel strategy only")),
        "a good ladder must not paint a refusal"
    );
}
