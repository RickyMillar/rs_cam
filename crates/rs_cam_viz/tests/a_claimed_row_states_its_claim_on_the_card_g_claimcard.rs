//! **G-CLAIMCARD — an off-size vendor row states its claim on the Feeds card.**
//!
//! # The rule (operator, extrapolation programme 2026-09-24)
//!
//! No invisible calculation. When no vendor row prints the tool's size, the
//! G1 size claim (`feeds::extrapolation::Claim`) carries the band from the
//! anchor row to the tool. The card must say so. Before P1 step 4 the card
//! painted one `approx ×0.87` token, and only when the raw ratio was more
//! than 40 % off: the rule, the range and the spread were nowhere.
//!
//! # What this pins
//!
//! 1. The claim headline (`Claim::card_text().0`: the scale and the anchor
//!    row) is a visible line on the card.
//! 2. The claim detail (the rule, the range and the spread) is on that
//!    line's hover, with the hardness scale.
//! 3. A `Derived` row shows its printed material label as a visible line.
//!    The fixture's row is an A4 row: "Wood, MDF, Sign-Foam (one printed
//!    row); hardwood is not printed apart".
//! 4. The old `approx ×` token does not also paint for a claimed row.
//!
//! # The fixture
//!
//! A 1.2 mm tapered ball tip, 2 flutes, on a 3D Finish (DropCutter, which
//! routes to Parallel/Finish) in generic hardwood. After ruling A1 the lookup
//! key is the 1.2 mm tip. No row prints 1.2 mm. The anchor is
//! `amana-tapered-hardwood-parallel-1000-2f-zrn-v8` (derived, grade b): its
//! diameter term is `(1 - ln 1.2 / ln 2) · 200 ≈ 147`, above the 1.5875 mm
//! row (≈ 119) and the SpeTool 1.5 mm row (≈ 136), and it ties the SpeTool
//! 1.0 mm row, which sorts after it by id. The claim is form A, between the
//! chart's printed 1.0 mm and 1.5875 mm 2-flute rows. The test reads the
//! claim from the explain payload and pins only its shape, so a scale change
//! in `extrapolation/size.rs` does not break it.

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
use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::vendor_lut::ObservationKind;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The Simulation workspace's right rail, the narrowest the Feeds tab holds.
const PANEL_WIDTH: f32 = 240.0;

/// The anchor row (see the module docs).
const ANCHOR_ID: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";

/// The A4 card sentence the anchor row carries (ruling A4).
const A4_LABEL: &str = "Wood, MDF, Sign-Foam (one printed row); hardwood is not printed apart";

fn tapered_ball(tip_diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    tool.name = "Claim card tapered ball".to_owned();
    tool.diameter = tip_diameter;
    tool.taper_half_angle = 5.7;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.cutting_length = 20.0;
    tool.flute_count = 2;
    tool
}

fn state() -> AppState {
    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Claim card fixture".to_owned(),
        enabled: true,
        operation: OperationConfig::DropCutter(Default::default()),
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
        path: PathBuf::from("claim_card_fixture.svg"),
        name: "Claim card fixture".to_owned(),
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
        .tool(tapered_ball(1.2))
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the claim card fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab =
        Some((id, rs_cam_viz::ui::properties::ToolpathTab::FeedsSpeeds));
    state
}

/// The explain payload the Feeds tab reads for the fixture's operation.
fn explain(state: &AppState) -> FeedsExplain {
    let session = &state.session;
    let tc = &session.toolpath_configs()[0];
    let tool = session.tools()[0].clone();
    rs_cam_core::feeds::suggest::feeds_explain_for_operation(
        &tc.operation,
        &tool,
        &session.stock_config().material,
        session.machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips too, which is where the claim detail lives.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Every text run the Feeds tab paints, in paint order. The production tab
/// override is one-shot, so the second frame is the steady state.
fn painted_text(mut state: AppState) -> Vec<String> {
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("claimcard_properties")
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

#[test]
fn a_claimed_row_states_its_claim_and_its_a4_label_g_claimcard() {
    let state = state();
    let explain = explain(&state);
    let row = explain
        .matched_row
        .as_ref()
        .expect("a vendor row answers the 1.2 mm tapered finish");

    // Non-vacuity: the fixture IS a claimed, derived A4 cell.
    assert_eq!(row.observation_id, ANCHOR_ID, "{row:?}");
    assert_eq!(row.row_kind, ObservationKind::Derived, "{row:?}");
    assert_eq!(row.material_label, A4_LABEL, "{row:?}");
    let claim = row
        .size_basis
        .claim()
        .unwrap_or_else(|| panic!("a 1.2 mm tip on a 1.0 mm row carries a claim: {row:?}"));
    let (headline, detail) = claim.card_text();
    assert!(
        headline.starts_with("extrapolated (G1 size): x") && headline.contains("1.0 mm row"),
        "the claim headline changed shape: {headline}"
    );

    let texts = painted_text(state);

    // 1. The headline is a visible, single-line run with the detail marker.
    let marked = format!("{headline} {}", tokens::GLYPH_DETAIL);
    assert!(
        texts.contains(&marked),
        "the claim headline `{marked}` is not on the card; runs were {texts:#?}"
    );

    // 2. The hover holds the detail (the rule, the range, the spread) and the
    //    hardness scale.
    let hover = texts
        .iter()
        .find(|t| t.contains(&detail))
        .unwrap_or_else(|| panic!("the claim detail `{detail}` is on no hover; runs {texts:#?}"));
    assert!(
        hover.contains("hardness scale x"),
        "the claim hover does not state the hardness scale: {hover}"
    );

    // 3. The A4 label is a visible line.
    let label_line = format!("Row material: {A4_LABEL} {}", tokens::GLYPH_DETAIL);
    assert!(
        texts.contains(&label_line),
        "the A4 label `{label_line}` is not on the card; runs were {texts:#?}"
    );

    // 4. The claim replaces the bare combined-scale token.
    assert!(
        !texts.iter().any(|t| t.starts_with("approx ×")),
        "a claimed row still paints the bare `approx ×` token; runs were {texts:#?}"
    );
}
