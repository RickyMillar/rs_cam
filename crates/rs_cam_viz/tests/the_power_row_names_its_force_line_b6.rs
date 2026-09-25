//! **B6 — the power row names its force line.**
//!
//! Ruling B6 (2026-09-24) gives each material one typed force line
//! (`Material::force_line`). The Feeds card paints it under the power row
//! (plan `planning/extrapolation_2026-09-24/B6_PLAN.md` §6):
//!
//! - a material with a line paints the line's card text, for example
//!   "Force line: Curti 2021 density law, ρ 676 kg/m³ (FPL SG 0.60), upper
//!   envelope";
//! - a material with no line paints the refusal headline, for example "No
//!   force line: plywood (ruling B6)";
//! - only when the mean chip of the cut is outside the printed range, a
//!   second line states the extrapolation.
//!
//! The expected texts come from the core doors (`Material::force_line` and
//! `feeds::operating_point::force_at_operating_point` on the cut `⚡ Apply
//! all` writes), not from literals, so this sentry pins the wiring. The
//! frame is the real inspector through a headless `egui::Context`, as
//! `the_chipload_verdict_is_one_row_g_chipverdict` paints it.

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
use rs_cam_core::feeds::{self, FeedsField};
use rs_cam_core::material::force_line::{ChipRegime, ForceAtPoint};
use rs_cam_core::material::{Material, PlywoodGrade, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector rail the Simulation workspace draws at.
const PANEL_WIDTH: f32 = 240.0;

/// The start of the extrapolation line (plan §6).
const EXTRAPOLATION_PREFIX: &str = "Mean chip ";

// ── fixtures ───────────────────────────────────────────────────────────────

struct Fixture {
    diameter_mm: f64,
    stickout_mm: f64,
    material: Material,
    operation: OperationConfig,
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// A Ø6 flat 2F pocket at 3000 mm/min and 18 000 rpm.
fn pocket(material: Material) -> Fixture {
    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    Fixture {
        diameter_mm: 6.0,
        stickout_mm: 18.0,
        material,
        operation,
    }
}

/// A Ø3.175 flat 2F profile in generic hardwood on a 12 mm stickout: the
/// in-band cell of `the_chipload_verdict_is_one_row_g_chipverdict`. Its
/// advance per tooth is about 0.023 mm, and the mean chip
/// `fz · (1 − cos ψ) / ψ` is at most 0.73 fz, so it sits below the Curti
/// range of 0.04-0.10 mm. The arm asserts that through the door before it
/// reads the card, so a moved recipe fails loudly rather than vacuously.
fn thin_profile() -> Fixture {
    let mut operation = OperationConfig::Profile(Default::default());
    if let OperationConfig::Profile(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    Fixture {
        diameter_mm: 3.175,
        stickout_mm: 12.0,
        material: hardwood(),
        operation,
    }
}

fn state_for(fixture: &Fixture) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = fixture.diameter_mm;
    tool.flute_count = 2;
    tool.stickout = fixture.stickout_mm;

    let stock = StockConfig {
        material: fixture.material.clone(),
        ..Default::default()
    };
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Force line fixture".to_owned(),
        enabled: true,
        operation: fixture.operation.clone(),
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
        path: PathBuf::from("force_line_fixture.svg"),
        name: "Force line fixture".to_owned(),
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
        .expect("add the force line fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::FeedsSpeeds));
    state
}

// ── harness ────────────────────────────────────────────────────────────────

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips too, which is where the detail lives.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

/// Every text run the Feeds tab paints on its steady-state frame.
fn painted_text(fixture: &Fixture) -> Vec<String> {
    let mut state = state_for(fixture);
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("force_line_properties")
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

/// The face runs: a hover run always holds a newline.
fn face_runs(texts: &[String]) -> Vec<&String> {
    texts.iter().filter(|t| !t.contains('\n')).collect()
}

/// The force point of the cut `⚡ Apply all` writes, through the door the
/// card reads. The applied cut is built the way
/// `the_chipload_verdict_is_one_row_g_chipverdict::door_figure` builds it.
fn door_point(fixture: &Fixture) -> ForceAtPoint {
    let state = state_for(fixture);
    let tc = &state.session.toolpath_configs()[0];
    let tool = &state.session.tools()[0];
    let stock = state.session.stock_config();
    let preview = feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    );
    let recommended = preview.recommended();
    let model_bbox = state.session.model_bbox(tc.model_id);
    let previews = feeds::suggest::preview_field_applies(
        &tc.operation,
        recommended,
        tool,
        state.session.machine(),
        &stock.material,
        tc.operation.feeds_style().1,
        feeds::suggest::SuggestContext {
            model_bbox: model_bbox.as_ref(),
            ..feeds::suggest::SuggestContext::default()
        },
    );
    let mut applied = tc.operation.clone();
    let mut scratch = feeds::FeedsProvenance::default();
    for field in [
        FeedsField::FeedRate,
        FeedsField::PlungeRate,
        FeedsField::SpindleRpm,
        FeedsField::Stepover,
        FeedsField::DepthPerPass,
    ] {
        if let Some(p) = previews.get(field) {
            p.write_to(&mut applied, &mut scratch);
        }
    }
    let value = |field: FeedsField, raw: f64| previews.get(field).map_or(raw, |p| p.value);
    feeds::operating_point::force_at_operating_point(
        &applied,
        tool,
        &stock.material,
        Some(feeds::suggest::CalculatorOperatingPoint {
            radial_width_mm: value(FeedsField::Stepover, recommended.radial_width_mm),
            axial_depth_mm: value(FeedsField::DepthPerPass, recommended.axial_depth_mm),
            feed_rate_mm_min: value(FeedsField::FeedRate, recommended.feed_rate_mm_min),
            rpm: value(FeedsField::SpindleRpm, recommended.rpm),
        }),
    )
    .unwrap_or_else(|reason| panic!("the fixture resolves no force point: {reason:?}"))
}

// ── the arms ───────────────────────────────────────────────────────────────

/// Hardwood paints the Curti line, its hover holds the line's detail, and
/// the extrapolation line paints exactly when the chip is outside the range.
#[test]
fn hardwood_paints_the_curti_line_b6() {
    let fixture = pocket(hardwood());
    let line = fixture
        .material
        .force_line()
        .expect("generic hardwood has a force line");
    let card = line.card_text();
    assert!(
        card.starts_with("Force line: Curti 2021 density law"),
        "the core card text moved: {card:?}"
    );
    let texts = painted_text(&fixture);
    let faces = face_runs(&texts);
    assert!(
        faces.iter().any(|run| **run == card),
        "the power row must paint {card:?}; face runs were {faces:?}"
    );
    let detail = line.detail_text();
    assert!(
        texts.iter().any(|run| run.contains(&detail)),
        "the force line hover must hold the line's detail {detail:?}"
    );
    let point = door_point(&fixture);
    let extrapolation_painted = faces
        .iter()
        .any(|run| run.starts_with(EXTRAPOLATION_PREFIX));
    assert_eq!(
        extrapolation_painted,
        !point.chip_regime.is_measured(),
        "the extrapolation line must paint only when the mean chip {:.4} mm is \
         outside the range; faces {faces:?}",
        point.mean_chip_mm
    );
}

/// Plywood paints the refusal headline and no line of its own.
#[test]
fn plywood_paints_the_refusal_headline_b6() {
    let fixture = pocket(Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    });
    let refusal = fixture
        .material
        .force_line()
        .expect_err("plywood has no force line");
    let headline = refusal.headline();
    assert_eq!(headline, "No force line: plywood (ruling B6)");
    let texts = painted_text(&fixture);
    let faces = face_runs(&texts);
    assert!(
        faces.iter().any(|run| **run == headline),
        "the power row must paint {headline:?}; face runs were {faces:?}"
    );
    assert!(
        !faces
            .iter()
            .any(|run| run.starts_with("Force line:") || run.starts_with(EXTRAPOLATION_PREFIX)),
        "a refused material paints no line and no extrapolation; faces {faces:?}"
    );
    assert!(
        texts.iter().any(|run| run.contains(refusal.detail())),
        "the refusal hover must hold the refusal detail"
    );
}

/// A cell with a mean chip below 0.04 mm paints the extrapolation line.
#[test]
fn a_thin_chip_paints_the_extrapolation_line_b6() {
    let fixture = thin_profile();
    let point = door_point(&fixture);
    assert!(
        matches!(point.chip_regime, ChipRegime::BelowMeasured { .. }),
        "non-vacuity: the fixture must sit below the measured chip range; the \
         door gives {:.4} mm ({:?})",
        point.mean_chip_mm,
        point.chip_regime
    );
    let expected = point
        .extrapolation_text()
        .expect("a chip below the range has an extrapolation line");
    assert!(expected.starts_with(EXTRAPOLATION_PREFIX) && expected.contains("below"));
    let texts = painted_text(&fixture);
    let faces = face_runs(&texts);
    assert!(
        faces.iter().any(|run| **run == expected),
        "the power row must paint {expected:?}; face runs were {faces:?}"
    );
    assert!(
        faces.iter().any(|run| **run == point.line.card_text()),
        "the line itself paints above the extrapolation; faces {faces:?}"
    );
}
