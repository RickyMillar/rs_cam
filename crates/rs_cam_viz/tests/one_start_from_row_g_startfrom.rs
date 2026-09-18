//! G-STARTFROM: the inspector has ONE rest path, and a draw never writes it.
//!
//! # The defect this exists to catch
//!
//! The Geometry tab carried four routes into one idea. A generic "Use
//! remaining stock" checkbox and a pencil-only "Rest reference" pair both
//! wrote `stock_source`, and they disagreed on screen: the checkbox won at
//! generation while the pencil pair still showed the reference tool. A "Rest
//! Analysis" checkbox offered a third rest control, and the draw that
//! rendered it WROTE `rest_analysis.enabled` when a consumer existed (D5) —
//! a paint pass that edits the project.
//!
//! W2 leaves one row, `Start from`, for every operation, and one
//! demand-titled disclosure for the rest dials. This file pins that shape.
//!
//! # Why painted text, not source text
//!
//! A source scan cannot tell a control that draws from one behind a `false`
//! branch. Every arm here drives the production `properties::draw` through a
//! headless `Context` and reads the glyphs it painted.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource, StockSource};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::finish::pencil::PencilDetector;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::components::ChoiceRow;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector's own width in the Toolpaths workspace (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn model() -> LoadedModel {
    LoadedModel {
        id: 1,
        path: PathBuf::from("start_from_fixture.svg"),
        name: "Start-from fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 40.0, 40.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(id: usize, name: &str, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: name.to_owned(),
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
    }
}

/// An `AppState` whose selection is the first toolpath of `configs`.
fn state_from(configs: Vec<ToolpathConfig>) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool)
        .model(model());
    for (index, config) in configs.into_iter().enumerate() {
        builder
            .add_toolpath(0, config)
            .unwrap_or_else(|error| panic!("add fixture toolpath {index}: {error:?}"));
    }
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    for config in state.session.toolpath_configs() {
        state
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
    state
}

/// A Scallop that another, enabled toolpath consumes as its rest-region
/// boundary. A Scallop draws neither the pencil grid nor the Unified Finish
/// claims block, so every "rest" string on its Geometry tab belongs to the
/// routes this file measures.
fn producer_with_consumer() -> AppState {
    let producer = toolpath(
        0,
        "Rough Scallop",
        OperationConfig::Scallop(Default::default()),
    );
    let mut consumer = toolpath(1, "Lakes", OperationConfig::Scallop(Default::default()));
    consumer.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::DerivedRestRegions {
            source_toolpath_id: rs_cam_core::ToolpathId(0),
        },
        ..Default::default()
    };
    state_from(vec![producer, consumer])
}

fn pencil(detector: PencilDetector, stock_source: StockSource) -> AppState {
    let config = rs_cam_core::compute::operation_configs::PencilConfig {
        detector,
        ..Default::default()
    };
    let mut tp = toolpath(0, "Pencil", OperationConfig::Pencil(config));
    tp.stock_source = stock_source;
    state_from(vec![tp])
}

/// Draw the production inspector for `state` and return every painted text
/// run. Three passes, read the third: egui settles a layout over two.
fn painted(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
    let mut texts = Vec::new();
    for pass in 0..3 {
        let mut events = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(PANEL_WIDTH);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(PANEL_WIDTH);
                properties::draw(ui, state, &mut events);
            });
        });
        if pass == 2 {
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

/// Every painted run whose lowercase holds "rest", deduped.
fn rest_strings(texts: &[String]) -> BTreeSet<String> {
    texts
        .iter()
        .filter(|text| text.to_lowercase().contains("rest"))
        .cloned()
        .collect()
}

#[test]
fn the_geometry_tab_names_rest_once_g_startfrom() {
    let ctx = ctx();
    let mut state = producer_with_consumer();
    let texts = painted(&ctx, &mut state);

    // Non-vacuity: the tab really rendered, and the new row is on it.
    assert!(
        texts.iter().any(|text| text == "Start from"),
        "the Geometry tab painted no Start from row; painted: {texts:?}"
    );

    let found = rest_strings(&texts);
    let expected: BTreeSet<String> = ["Rest regions \u{2192} Lakes".to_owned()]
        .into_iter()
        .collect();
    assert_eq!(
        found, expected,
        "the producer's Geometry tab must name rest exactly once, in the demand-titled \
         disclosure. A second rest control is a second route into one idea."
    );
}

#[test]
fn a_draw_never_writes_rest_analysis_g_startfrom() {
    let ctx = ctx();
    let mut state = producer_with_consumer();
    // The producer has a consumer, and its analysis is off. The old panel
    // flipped the flag from inside its draw (D5).
    let id = state.session.toolpath_configs()[0].id;
    assert!(
        !state.session.toolpath_configs()[0].rest_analysis.enabled,
        "the fixture must start with the producer's rest analysis off"
    );
    let _ = painted(&ctx, &mut state);
    let after = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == id)
        .expect("the producer survives the draw");
    assert!(
        !after.rest_analysis.enabled,
        "a draw pass wrote rest_analysis.enabled. A paint must not edit the project; \
         the demand doors are the boundary picker, set_boundary_config and the overlay \
         action."
    );
}

#[test]
fn pencil_hides_its_reference_under_prior_stock_g_startfrom() {
    let ctx = ctx();
    let mut state = pencil(PencilDetector::RestDepth, StockSource::FromRemainingStock);
    let texts = painted(&ctx, &mut state);
    assert!(
        texts.iter().any(|text| text == "Start from"),
        "non-vacuity: the pencil Geometry tab painted no Start from row; painted: {texts:?}"
    );
    assert!(
        !texts.iter().any(|text| text == "Reference:"),
        "a rest-depth pencil under After previous ops read the machined stock, so its \
         analytic reference is not a choice; painted: {texts:?}"
    );
    assert!(
        !texts
            .iter()
            .any(|text| text.contains("Machined stock (requires simulation)")),
        "the pencil rest-reference pair is deleted; the Start from row is the one writer"
    );

    // The flip makes the arm non-vacuous: the same fixture under Stock DOES
    // show the picker.
    let mut fresh = pencil(PencilDetector::RestDepth, StockSource::Fresh);
    let fresh_texts = painted(&ctx, &mut fresh);
    assert!(
        fresh_texts.iter().any(|text| text == "Reference:"),
        "a rest-depth pencil under Stock resolves an analytic reference, so its picker \
         must be visible; painted: {fresh_texts:?}"
    );
}

#[test]
fn a_crease_detector_keeps_its_reference_g_startfrom() {
    let ctx = ctx();
    let mut state = pencil(PencilDetector::Dihedral, StockSource::FromRemainingStock);
    let texts = painted(&ctx, &mut state);
    assert!(
        texts.iter().any(|text| text == "Reference:"),
        "the dihedral arm never reads the machined stock: it always resolves a reference \
         cutter. Hiding its picker under After previous ops hides a live control; \
         painted: {texts:?}"
    );
}

#[test]
fn the_start_from_row_is_the_kit_row_g_startfrom() {
    let ctx = ctx();
    let options = [
        (StockSource::Fresh, "Stock"),
        (StockSource::FromRemainingStock, "After previous ops"),
    ];
    let mut value = StockSource::Fresh;
    let mut target = egui::Pos2::ZERO;
    let mut changed = false;
    let mut painted_labels: Vec<String> = Vec::new();

    // Pass 0-2 settle the layout and name the option's rect. Pass 3 presses
    // on it and pass 4 releases: egui resolves a click from the widget rects
    // the PREVIOUS pass recorded, so a press and a release in one pass is
    // not the shape the real pointer makes.
    for pass in 0..5 {
        let mut events = Vec::new();
        if pass >= 3 {
            events.push(egui::Event::PointerMoved(target));
            events.push(egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: pass == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(PANEL_WIDTH, 400.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            ui.set_max_width(PANEL_WIDTH);
            let response = ui.add(
                ChoiceRow::new("Start from", &mut value, &options).hover("Two ways to start."),
            );
            if response.changed() {
                changed = true;
            }
        });
        if pass == 2 {
            painted_labels = out
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::epaint::Shape::Text(text) => {
                        if text.galley.job.text == "After previous ops" {
                            target = clipped.shape.visual_bounding_rect().center();
                        }
                        Some(text.galley.job.text.clone())
                    }
                    _ => None,
                })
                .collect();
        }
        out.textures_delta.clear();
    }

    for label in ["Start from", "Stock", "After previous ops"] {
        assert!(
            painted_labels.iter().any(|text| text == label),
            "ChoiceRow must paint {label}; painted: {painted_labels:?}"
        );
    }
    assert!(
        changed,
        "a click on the other option must report Response::changed; the caller stamps the \
         toolpath stale on that report alone"
    );
    assert_eq!(
        value,
        StockSource::FromRemainingStock,
        "the click must write the option it named"
    );
}
