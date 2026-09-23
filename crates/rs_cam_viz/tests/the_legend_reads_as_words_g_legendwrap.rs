//! **G-LEGENDWRAP — a legend line does not break inside a word.**
//!
//! # The defect
//!
//! The operator's words for the Explore window on 2026-09-15 were "super
//! messed up format... it's all cooked there". This was the largest part of
//! it. The legend rendered `vendor band` as:
//!
//! ```text
//! ven
//! dor
//! ban
//! d
//! ```
//!
//! One letter per line, for every entry, in all three legends. Capture:
//! `planning/feeds_rework_2026-09-15/evidence_modal_bottom.png`.
//!
//! # The mechanism, which is the part worth keeping
//!
//! Each legend was an `egui::Grid`, and a `Grid` cell has no known width on
//! its first layout pass. **Both of the obvious wrap modes are wrong there:**
//!
//! - `TextWrapMode::Extend`, the Grid cell default, asks for INFINITE width.
//!   That is the UR1 defect this crate bans, where content widens its own
//!   container until the panel clips it.
//! - `.wrap()` — the obvious correction, and what the code had — has no
//!   width to wrap against, so it collapses to the narrowest legal break.
//!   For a long token that is one character.
//!
//! There is no third wrap mode that fixes it, because the problem is the
//! column negotiation and not the label. A legend is a list of lines, so it
//! is laid out as a list of lines.
//!
//! # Why this is rendered and not scanned
//!
//! The defect is a layout OUTCOME. A source scan for `Grid` would pass the
//! day someone reintroduces the shape by another route, and would fail on
//! the many grids in this crate that are perfectly fine.

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
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::{AppState, FeedsModalState};
use rs_cam_viz::ui::{feeds, tokens};

/// A viewport with room for the whole window, so nothing is clipped and
/// every legend line is laid out at its real width.
const SCREEN: egui::Vec2 = egui::Vec2::new(1400.0, 1000.0);

/// A galley row shorter than this many characters, inside a longer line, is
/// the symptom: a word broken across rows rather than a short final row.
const MIN_BROKEN_ROW_CHARS: usize = 4;

fn fixture() -> AppState {
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
        name: "Legend fixture".to_owned(),
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
        path: PathBuf::from("legend_fixture.svg"),
        name: "Legend fixture".to_owned(),
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
        .expect("add the legend fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let toolpath_id = state.session.toolpath_configs()[0].id;
    state.feeds_modal = Some(FeedsModalState {
        toolpath_id,
        explore: None,
    });
    state
}

/// Every galley the Explore window paints, as its laid-out ROWS.
///
/// `Galley::rows` is what the renderer actually broke the text into, which
/// is the only place the defect is visible — the source string is intact
/// either way.
fn painted_rows() -> Vec<(String, Vec<String>)> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let state = fixture();
    let mut collected = Vec::new();
    for pass in 0..3 {
        let mut events = Vec::new();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
                ..Default::default()
            },
            |ui| {
                feeds::draw(ui.ctx(), &state, &mut events);
            },
        );
        if pass == 2 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let rows: Vec<String> = text.galley.rows.iter().map(|row| row.text()).collect();
                    collected.push((text.galley.job.text.clone(), rows));
                }
            }
        }
        out.textures_delta.clear();
    }
    collected
}

// ── arm 1 — no legend line breaks inside a word ──────────────────────────

#[test]
fn no_explore_label_breaks_inside_a_word_g_legendwrap() {
    let painted = painted_rows();
    let mut offenders = Vec::new();
    for (whole, rows) in &painted {
        if rows.len() < 2 {
            continue;
        }
        // A short FINAL row is ordinary wrapping. A short row with more rows
        // after it means the renderer broke a word to fit.
        for row in &rows[..rows.len() - 1] {
            let trimmed = row.trim();
            if !trimmed.is_empty() && trimmed.chars().count() < MIN_BROKEN_ROW_CHARS {
                offenders.push(format!("{whole:?} broke as {rows:?}"));
                break;
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the Explore window broke {} label(s) mid-word. A `Grid` cell has no \
         width to wrap against, so `.wrap()` there collapses to one character \
         per row — lay the legend out as a list of rows instead:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

// ── arm 2 — non-vacuity ──────────────────────────────────────────────────

/// The window really did render, and really did paint the legend this arm
/// exists to check. Without this, arm 1 passes on an empty frame.
#[test]
fn the_explore_window_painted_its_legend_g_legendwrap() {
    let painted = painted_rows();
    assert!(
        painted
            .iter()
            .any(|(whole, _)| whole == "What the colours mean"),
        "the Explore window painted no legend heading, so the wrap arm read \
         nothing. It found {} text runs.",
        painted.len()
    );
    // G-CHARTLINES (2026-09-23): the legend prints `Vendor range` only when
    // the matched row publishes both limits. A row that publishes one value
    // prints `Vendor value` instead. Either row is the long vendor entry this
    // arm needs, so the arm accepts both and does not depend on which kind
    // of row the fixture matches.
    assert!(
        painted.iter().any(|(whole, _)| {
            let lower = whole.to_lowercase();
            lower.contains("vendor range") || lower.contains("vendor value")
        }),
        "the legend painted no `Vendor range` or `Vendor value` entry — the \
         row that rendered as `ven / dor / ban / d` when it was still called \
         `Vendor band`"
    );
}
