//! **G10 — the Feeds card names the plunge rule and every entry rule.**
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/G10_PLAN.md` (§3 A8,
//! §3 A10 item 4).
//!
//! # The rule
//!
//! The operator's standing rule (ruling R4, 2026-09-24): no invisible
//! calculation. G10 (2026-09-25) gives the plunge a basis (a G10 claim, a
//! printed fraction of the side feed, or the named repo rule, the material
//! base) and the entry a set of rules (rulings Q6-Q10, Q12). The card has no
//! entry row, so:
//!
//! 1. The plunge basis headline (`PlungeBasis::card_text().0`) is one face
//!    line, with the detail marker; its detail is on the hover.
//! 2. Every entry note of the funnel's `RampFeed` record is one face line
//!    under the ramp record; a caution note carries "⚠".
//! 3. The MCP `get_suggest_rationale` basis (`app::suggest_basis_json`)
//!    carries the same: `plunge.source`, `plunge.re_derived`, `entry` and
//!    `ramp.arm`.
//!
//! Before G10 the card painted a fixed sentence about the material base on
//! the plunge row hover and no face line, the record carried no notes, and
//! the MCP basis had no `plunge` or `entry` key, so each arm fails on the
//! pre-G10 code.
//!
//! The fixtures render the real inspector through a headless
//! `egui::Context`, as `a_claimed_row_states_its_claim_on_the_card_g_claimcard`
//! does.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::ramp::EntryNote;
use rs_cam_core::feeds::suggest::{SuggestContext, SuggestForOperationInput, SuggestWarning};
use rs_cam_core::feeds::{FeedsExplain, PlungeBasis, embedded_vendor_lut};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector rail the Toolpaths workspace draws at.
const PANEL_WIDTH: f32 = 280.0;

fn tool_of(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), kind);
    tool.name = "G10 card fixture".to_owned();
    tool.diameter = diameter;
    tool.flute_count = 2;
    tool.cutting_length = (diameter * 4.0).max(12.0);
    tool.shank_diameter = diameter.max(3.175);
    tool.shaft_diameter = diameter.max(3.175);
    tool.stickout = tool.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        tool.corner_radius = 0.5;
        tool.corner_radius_mm = 0.5;
    }
    tool
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn helix(radius_mm: f64, pitch_mm: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        helix_radius: radius_mm,
        helix_pitch: pitch_mm,
        ..DressupConfig::default()
    }
}

fn ramp(angle_deg: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: angle_deg,
        ..DressupConfig::default()
    }
}

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
        id: rs_cam_core::ToolpathId(0),
        name: "G10 card fixture".to_owned(),
        enabled: true,
        operation,
        dressups,
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
        path: PathBuf::from("g10_card_fixture.svg"),
        name: "G10 card fixture".to_owned(),
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
        .expect("add the G10 card fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::FeedsSpeeds));
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
        embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
}

/// The entry notes of the Suggest run the card makes, with the card's own
/// context (`feeds_speeds.rs`).
fn entry_notes(state: &AppState) -> Vec<EntryNote> {
    let id = state.session.toolpath_configs()[0].id;
    let snapshot = properties::toolpath_panel_snapshot(id, &state.session, &state.gui)
        .expect("the toolpath resolves for the properties panel");
    let session = &state.session;
    let tool = session.tools()[0].clone();
    let suggested = rs_cam_core::feeds::suggest::suggest_for_operation(SuggestForOperationInput {
        operation: &snapshot.entry.operation,
        tool: &tool,
        machine: session.machine(),
        material: &session.stock_config().material,
        lut: embedded_vendor_lut(),
        spindle_strategy: session.post_config().spindle_strategy,
        context: SuggestContext {
            model_bbox: snapshot.model_bbox.as_ref(),
            dressups: Some(&snapshot.entry.dressups),
            ..SuggestContext::default()
        },
    })
    .expect("Suggest serves the fixture");
    suggested
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RampFeed { notes, .. } => Some(notes.as_slice().to_vec()),
            _ => None,
        })
        .expect("the fixture files one RampFeed record")
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
    let ctx = ctx();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("g10_card_properties")
                .default_size(PANEL_WIDTH)
                .max_size(PANEL_WIDTH)
                .resizable(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
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

/// The plunge basis headline is a face line with the detail marker, and its
/// detail is on a hover: the G10 flat claim on a 6 mm flat Pocket, and the
/// named repo rule on a 6 mm bull Adaptive.
#[test]
fn the_plunge_line_names_its_claim_or_rule_g10() {
    for (name, tool, operation, claimed) in [
        (
            "flat 6 mm Pocket",
            tool_of(ToolType::EndMill, 6.0),
            OperationConfig::new_default(OperationType::Pocket),
            true,
        ),
        (
            "bull 6 mm Adaptive",
            tool_of(ToolType::BullNose, 6.0),
            OperationConfig::new_default(OperationType::Adaptive),
            false,
        ),
    ] {
        let mut state = state_for(tool, softwood(), operation, ramp(3.0));
        let plunge = explain(&state).recommended.plunge;
        // Non-vacuity: the fixture is the basis the arm names.
        match (&plunge, claimed) {
            (PlungeBasis::Claimed { .. }, true) | (PlungeBasis::MaterialBase { .. }, false) => {}
            _ => panic!("{name}: unexpected basis {plunge:?}"),
        }
        let (headline, detail) = plunge.card_text();
        if claimed {
            assert!(
                headline.starts_with("Plunge = feed / 2 (G10"),
                "{name}: {headline}"
            );
        } else {
            assert!(
                headline.contains("no source; repo rule"),
                "{name}: {headline}"
            );
        }
        let texts = painted_text(&mut state);
        let marked = format!("{headline} {}", tokens::GLYPH_DETAIL);
        assert!(
            texts.contains(&marked),
            "{name}: the plunge line `{marked}` is not on the card; runs were {texts:#?}"
        );
        assert!(
            texts.iter().any(|t| t.contains(&detail)),
            "{name}: the plunge detail `{detail}` is on no hover; runs {texts:#?}"
        );
    }
}

/// Every entry note is a face line under the ramp record: the helix (r, r /
/// D, the pitch and θ), the centre pip and the clearance on a 6 mm ball
/// with a helix; the ramp angle on a 6 mm flat with a 3° ramp.
#[test]
fn the_entry_lines_name_angle_pitch_pip_and_clearance_g10() {
    for (name, tool, dressups, needles) in [
        (
            "ball 6 mm helix r 1.8",
            tool_of(ToolType::BallNose, 6.0),
            helix(1.8, 1.0),
            &[
                "Helix r 1.80 mm (0.30 x D), pitch 1.00 mm = 5.05°",
                "The helix leaves a centre pip 0.60 mm high",
                "Entry starts 0.50 mm above the material: no source; operator rule 0.5 mm",
            ][..],
        ),
        (
            "flat 6 mm ramp 3°",
            tool_of(ToolType::EndMill, 6.0),
            ramp(3.0),
            &[
                "Ramp angle 3.00°: repo rule (default 3.00°), no wood or router source",
                "Entry starts 0.50 mm above the material: no source; operator rule 0.5 mm",
            ][..],
        ),
    ] {
        let mut state = state_for(
            tool,
            softwood(),
            OperationConfig::new_default(OperationType::Pocket),
            dressups,
        );
        let notes = entry_notes(&state);
        for needle in needles {
            assert!(
                notes.iter().any(|n| n.headline.starts_with(needle)),
                "{name}: no note starts with `{needle}`: {notes:?}"
            );
        }
        let texts = painted_text(&mut state);
        let face: Vec<&String> = texts.iter().filter(|t| !t.contains('\n')).collect();
        for note in &notes {
            let line = if note.caution {
                format!("⚠ {}", note.headline)
            } else {
                note.headline.clone()
            };
            assert!(
                face.iter().any(|t| **t == line),
                "{name}: the entry note `{line}` is not a face line; face runs {face:#?}"
            );
        }
    }
}

/// The MCP basis carries the plunge source, the re-derive record, the entry
/// notes and the ramp arm, from the same profile the card reads.
#[test]
fn the_mcp_basis_carries_plunge_and_entry_g10() {
    let state = state_for(
        tool_of(ToolType::BallNose, 6.0),
        softwood(),
        OperationConfig::new_default(OperationType::Pocket),
        helix(1.8, 1.0),
    );
    let tc = &state.session.toolpath_configs()[0];
    let profile = state
        .session
        .cutter_op_profile(tc)
        .expect("the fixture's tool resolves");
    let feeds = profile.feeds.as_ref().expect("Suggest serves the fixture");
    let basis = rs_cam_viz::app::suggest_basis_json(&profile);

    let (headline, detail) = feeds.plunge.card_text();
    assert_eq!(basis["plunge"]["source"]["headline"], headline.as_str());
    assert_eq!(basis["plunge"]["source"]["detail"], detail.as_str());
    let shipped = profile
        .suggested_operation
        .as_ref()
        .expect("Suggest ships an operation");
    assert_eq!(
        basis["plunge"]["value_mm_min"].as_f64(),
        Some(shipped.plunge_rate())
    );
    let re_derived = &basis["plunge"]["re_derived"];
    assert!(
        re_derived.is_null() || re_derived["binding"].is_string(),
        "{re_derived}"
    );

    let record_notes = profile
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RampFeed { notes, .. } => Some(notes.as_slice().to_vec()),
            _ => None,
        })
        .expect("the fixture files one RampFeed record");
    let entry = basis["entry"].as_array().expect("entry is an array");
    assert!(!entry.is_empty(), "the helix fixture has entry notes");
    assert_eq!(entry.len(), record_notes.len());
    for (json, note) in entry.iter().zip(&record_notes) {
        assert_eq!(json["headline"], note.headline.as_str());
        assert_eq!(json["detail"], note.detail.as_str());
        assert_eq!(json["caution"], note.caution);
    }
    assert!(
        entry.iter().any(|n| n["headline"]
            .as_str()
            .is_some_and(|h| h.contains("centre pip"))),
        "{entry:?}"
    );

    // A ball has no G6 chip: the ramp is the G10 Q2 plunge slope.
    let arm = basis["ramp"]["arm"].as_str().expect("the ramp arm");
    assert!(arm.starts_with("PlungeSlope/"), "{arm}");
    assert_eq!(
        basis["ramp"]["value_mm_min"].as_f64(),
        shipped.ramp_feed_rate()
    );
}
