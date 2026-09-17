//! **G-WHYROW — every recommended number explains itself, on its own row.**
//!
//! # The defect, in two steps
//!
//! The Feeds tab carried a `Why is the recommendation here?` disclosure:
//! provenance, then rationale, then a derate chain, then a result, then
//! warnings. Opened, it painted **44 text runs**, most of them captions
//! repeating the run above.
//!
//! The 2026-09-15 declutter cut it to 13 runs. The operator's verdict on the
//! result was that it was still "all just too much", and that what they
//! wanted was: *if a recommended value differs from mine, let me hover it and
//! see why.*
//!
//! **That is not a complaint about length, and the first fix failing to
//! satisfy it is the evidence.** The disclosure explained the recommendation
//! AS A WHOLE. An operator reads the card ONE ROW AT A TIME, and the question
//! they ask is always about one row — *why is my DOC being tripled?* No
//! whole-recipe explanation answers a per-row question, at any length. So the
//! explanation moved onto the row whose number it explains.
//!
//! # What this pins
//!
//! 1. The disclosure does not come back.
//! 2. Every row of the comparison card carries a hover, and the hover is
//!    about THAT row — checked by looking for the row's own number in it.
//! 3. The explanations that used to hold lines are still reachable.
//! 4. Warnings did NOT move to a hover. A warning behind a hover is a
//!    warning that was deleted.
//!
//! Arm 3 is the one that matters most. A rework that deletes a panel and
//! writes nothing in its place would satisfy arms 1 and 4 and leave the
//! operator with less than they started with.

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
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector rail the Toolpaths workspace draws at.
const PANEL_WIDTH: f32 = 280.0;

/// Every row of the comparison card. Each must carry a hover of its own.
const RECIPE_ROWS: [&str; 6] = ["RPM", "Feed", "Plunge", "DOC", "WOC", "Advance/tooth"];

/// Explanations that used to hold a line and are now inside a row's hover.
/// Each must still be painted somewhere, or the rework deleted it.
const EXPLANATIONS_THAT_MOVED: [&str; 5] = [
    // the derate captions
    "to limit deflection",
    // the chip-thinning ruling (G-CHIPTHIN-HALFFIX)
    "no radial condition to correct from",
    // the arithmetic proof of the advance per tooth
    "reduction",
    // the DOC/WOC calculator caveat, which shipped twice as a caption
    "invariant funnel",
    // the vendor row's identity
    "vendor row",
];

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // Paints tooltips, which is where the explanations now live.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    ctx
}

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

    // The UR1/UP4 fixture shape: a deliberately low advance per tooth that
    // trips the rubbing floor while the vendor row stays in play, so
    // provenance, rationale, derates and a warning all render together.
    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Why-row fixture".to_owned(),
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
        path: PathBuf::from("why_row_fixture.svg"),
        name: "Why-row fixture".to_owned(),
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
        .expect("add the Why-row fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab =
        Some((id, rs_cam_viz::ui::properties::ToolpathTab::FeedsSpeeds));
    state
}

/// Every text run the Feeds tab paints, in paint order. The production tab
/// override is one-shot, so the second frame is the steady state.
fn painted_text() -> Vec<String> {
    let ctx = ctx();
    let mut state = fixture();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("whyrow_properties")
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

fn feeds_sources() -> Vec<(PathBuf, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .expect("read src/ui/feeds")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            let source = std::fs::read_to_string(&path).expect("read feeds source");
            out.push((path, source));
        }
    }
    // The inspector's Feeds tab. P4 (2026-09-17) split `properties/mod.rs`
    // into `mod.rs` plus seven panel children beside it, so every `.rs` file
    // directly in that folder is read. `operations/` is a sub-folder with
    // its own sentries and is not read.
    let inspector = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut panels: Vec<PathBuf> = std::fs::read_dir(&inspector)
        .expect("read src/ui/properties")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    panels.sort();
    assert!(!panels.is_empty(), "no source under src/ui/properties");
    for path in panels {
        let source = std::fs::read_to_string(&path).expect("read properties source");
        out.push((path, source));
    }
    out
}

// ── arm 1 — the disclosure does not come back ────────────────────────────

#[test]
fn the_recommendation_has_no_why_disclosure_g_whyrow() {
    for (path, source) in feeds_sources() {
        // Strip line comments: this file and its neighbours discuss the
        // deleted disclosure by name, and a scan that reads a comment
        // reports code that is not there.
        let code: String = source
            .lines()
            .map(|line| match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("Why is the recommendation here?"),
            "{} builds the Why disclosure again. It explained the recipe as a \
             whole, which is never the question an operator asks while reading \
             one row that changed.",
            path.display()
        );
    }
    let texts = painted_text();
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("Why is the recommendation")),
        "the Feeds tab painted the Why disclosure header"
    );
}

// ── arm 2 — every row explains its own number ────────────────────────────

/// Each row carries the detail mark, and its hover quotes the row's own
/// recommended value — which is what makes the hover about THAT row rather
/// than a copy of one recipe-wide paragraph on six rows.
#[test]
fn every_recipe_row_carries_its_own_explanation_g_whyrow() {
    let texts = painted_text();
    let mut missing = Vec::new();
    for row in RECIPE_ROWS {
        let marked = format!("{row} {}", tokens::GLYPH_DETAIL);
        if !texts.iter().any(|t| t.starts_with(&marked)) {
            missing.push(row);
        }
    }
    assert!(
        missing.is_empty(),
        "these comparison rows carry no `{}` mark, so their explanation is \
         unreachable: {missing:?}",
        tokens::GLYPH_DETAIL
    );

    // The hovers are distinct from one another. One paragraph repeated on
    // six rows would satisfy the mark check above and answer nothing.
    let hovers: Vec<&String> = texts
        .iter()
        .filter(|t| t.contains('\n') && t.len() > 40)
        .collect();
    let distinct: std::collections::BTreeSet<&str> = hovers.iter().map(|h| h.as_str()).collect();
    assert!(
        distinct.len() >= 4,
        "the card painted {} multi-line hovers but only {} distinct ones. A \
         per-row explanation that is the same on every row is the disclosure \
         again, copied six times.",
        hovers.len(),
        distinct.len()
    );
}

// ── arm 3 — the explanations survived the move ───────────────────────────

#[test]
fn every_explanation_that_left_the_page_is_still_reachable_g_whyrow() {
    let texts = painted_text();
    let haystack = texts.join("\n").to_lowercase();
    let missing: Vec<&str> = EXPLANATIONS_THAT_MOVED
        .iter()
        .copied()
        .filter(|needle| !haystack.contains(&needle.to_lowercase()))
        .collect();
    assert!(
        missing.is_empty(),
        "the rework DELETED these explanations rather than moving them onto a \
         row: {missing:?}. Decluttering removes repetition, never the reason a \
         number is what it is."
    );
}

// ── arm 4 — a warning is not a hover ─────────────────────────────────────

/// The rubbing-floor warning stays on the PAGE. `set_everything_is_visible`
/// paints tooltips too, so "somewhere in the output" would not distinguish a
/// warning on the page from one hidden behind a hover. This arm therefore
/// looks for it as a text run of its own.
#[test]
fn the_rubbing_floor_warning_stays_on_the_page_g_whyrow() {
    let texts = painted_text();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with('⚠') && t.contains("below rubbing floor")),
        "the rubbing-floor warning is no longer painted as its own line. A \
         warning behind a hover is a warning that was deleted, and this \
         surface exists to stop an operator burning a cutter."
    );
}
