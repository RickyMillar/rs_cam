//! **G-WHYLINES — the Why disclosure states facts; its hovers explain them.**
//!
//! # The defect
//!
//! Reported 2026-09-15: the Feeds inspector is "horribly text-heavy". It was.
//! Opened on the Baltic-birch fixture below, `Why is the recommendation
//! here?` painted **44 text runs**, and most of them were prose captions
//! repeating the run above:
//!
//! ```text
//! depth-tier feed derate
//! ×0.750
//! deep cut — slow feed to limit deflection
//! ```
//!
//! Three runs to say one number moved the feed. Six derates did that. Two
//! more blocks printed a heading over a single line, and the result printed
//! its own three-step arithmetic proof under it.
//!
//! The unit below is the PAINTED TEXT RUN, which is what a renderer can
//! count. It over-states the visual line count — a `label value` pair is two
//! runs on one line — and it under-states the height, because a run that
//! wraps takes two lines. Both directions are the same on either side of
//! the change, so the ratio is honest even though neither number is a line.
//!
//! # The rule this pins
//!
//! **A line carries the fact. Its hover carries the explanation.** The same
//! fixture now paints 13 runs, and every explanation that left the page is
//! still reachable — which is the half a count alone cannot check, and the
//! half that separates a declutter from a deletion. Arm 2 checks it.
//!
//! # Why a budget and not an exact count
//!
//! Three of the lines are rationale entries, and the engine decides how many
//! of those there are. An exact count would fail on a legitimate engine
//! change. A budget fails only when the surface goes back to explaining
//! itself in captions, which is the thing being prevented.

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
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector rail the Toolpaths workspace draws at.
const PANEL_WIDTH: f32 = 280.0;

/// The most text runs the disclosure may paint on this fixture.
///
/// Measured 2026-09-15: 44 before, 13 after. The bar leaves room for the
/// engine to emit another rationale entry or another warning, and no room at
/// all for a caption under every row.
const LINE_BUDGET: usize = 16;

/// The fewest lines that still means the disclosure rendered.
///
/// Without a floor, a panel that drew nothing would satisfy the budget and
/// report the surface as decluttered.
const LINE_FLOOR: usize = 8;

/// Where the disclosure's own content starts and ends in paint order.
const DISCLOSURE_HEADER: &str = "Why is the recommendation here?";
const FOLLOWING_SECTION: &str = "Vendor Cutting Data";

/// Explanations that used to hold a line of their own and are now hovers.
/// Each must still be painted somewhere, or the declutter deleted it.
const EXPLANATIONS_THAT_MOVED: [&str; 4] = [
    // the derate captions
    "to limit deflection",
    // the chip-thinning ruling (G-CHIPTHIN-HALFFIX)
    "no radial condition to correct from",
    // the three-step proof of the result
    "combined derate",
    // the provenance row's "Calibrated for"
    "calibrated for",
];

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    // The disclosure is closed by default, which is correct and is not what
    // this sentry is about. Render it open.
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

    // The same shape as the UR1/UP4 fixtures: a deliberately low advance per
    // tooth that trips the rubbing floor while the vendor row stays in play,
    // so provenance, rationale, derates and a warning all render together.
    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Why-lines fixture".to_owned(),
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
        path: PathBuf::from("why_lines_fixture.svg"),
        name: "Why-lines fixture".to_owned(),
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
        .expect("add the Why-lines fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, "feeds".to_owned()));
    state
}

/// Every text run the Feeds tab paints, in paint order.
///
/// The production tab override is one-shot: the first frame consumes it and
/// caches the recipe. The second frame is the steady state the operator sees.
fn painted_text() -> Vec<String> {
    let ctx = ctx();
    let mut state = fixture();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("whylines_properties")
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

/// The runs the disclosure itself paints: from its own header to the start
/// of the next section. Tooltips paint into a later layer, so they fall
/// outside this slice and are counted by arm 2 instead.
fn disclosure_lines(texts: &[String]) -> &[String] {
    let start = texts
        .iter()
        .position(|t| t == DISCLOSURE_HEADER)
        .unwrap_or_else(|| {
            panic!("the Feeds tab painted no `{DISCLOSURE_HEADER}` header; it moved or was renamed")
        });
    let end = texts[start..]
        .iter()
        .position(|t| t == FOLLOWING_SECTION)
        .map(|offset| start + offset)
        .unwrap_or_else(|| {
            panic!("`{FOLLOWING_SECTION}` no longer follows the disclosure; re-derive this slice")
        });
    &texts[start + 1..end]
}

// ── arm 1 — the budget ───────────────────────────────────────────────────

#[test]
fn the_why_disclosure_stays_inside_its_line_budget_g_whylines() {
    let texts = painted_text();
    let lines = disclosure_lines(&texts);
    assert!(
        lines.len() <= LINE_BUDGET,
        "the Why disclosure painted {} lines, over its budget of {LINE_BUDGET}. \
         It painted 44 before the 2026-09-15 declutter, and the way back to 44 \
         is a caption under every row. Put the explanation on the line's hover \
         instead:\n{}",
        lines.len(),
        lines.join("\n"),
    );
}

// ── arm 2 — the explanations moved, they were not deleted ────────────────

/// A line count alone cannot tell a declutter from a deletion. Every phrase
/// below used to hold a line of its own; each must still reach the operator.
#[test]
fn every_explanation_that_left_the_page_is_still_reachable_g_whylines() {
    let texts = painted_text();
    let haystack = texts.join("\n").to_lowercase();
    let missing: Vec<&str> = EXPLANATIONS_THAT_MOVED
        .iter()
        .copied()
        .filter(|needle| !haystack.contains(&needle.to_lowercase()))
        .collect();
    assert!(
        missing.is_empty(),
        "the declutter DELETED these explanations rather than moving them to a \
         hover: {missing:?}. Decluttering removes repetition, never the reason \
         a number is what it is."
    );
}

// ── arm 3 — non-vacuity ──────────────────────────────────────────────────

/// The disclosure rendered, and the hovers it moved the prose into are
/// marked, so the operator can find them.
#[test]
fn the_disclosure_rendered_and_marks_its_hovers_g_whylines() {
    let texts = painted_text();
    let lines = disclosure_lines(&texts);
    assert!(
        lines.len() >= LINE_FLOOR,
        "the Why disclosure painted only {} lines. Arm 1's budget then passes \
         on a surface that drew nothing, which is not the same as a surface \
         that draws less:\n{}",
        lines.len(),
        lines.join("\n"),
    );
    let marked = lines
        .iter()
        .filter(|line| line.contains(tokens::GLYPH_DETAIL))
        .count();
    assert!(
        marked >= lines.len() / 2,
        "only {marked} of {} lines carry `{}`. The mark is the whole \
         affordance: an explanation the operator cannot find is an \
         explanation that was deleted.",
        lines.len(),
        tokens::GLYPH_DETAIL,
    );
}
