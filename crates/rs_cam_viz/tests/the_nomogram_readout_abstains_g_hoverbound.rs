//! **G-HOVERBOUND — the nomogram readout reports where the pointer IS, or
//! it says nothing.**
//!
//! # The defect
//!
//! Reported by the operator on 2026-09-16 — *"the readout reads where the
//! cursor is, even if the cursor is off the graph"* — and caught in the same
//! capture that was taken to verify the label fix. The readout line under the
//! nomogram read:
//!
//! ```text
//! 37891 RPM · 0 mm/min → commanded advance/tooth 0.0000 mm/tooth ·
//! BURN risk (below band) · past spindle cap
//! ```
//!
//! painted in the DANGER colour, on a chart whose RPM axis stops at 27 600
//! (the 24 000 spindle cap plus the 15 % overrun the plot draws). The pointer
//! was over the inspector panel, not over the chart. There was no operating
//! point at all.
//!
//! `egui_plot::PlotUi::pointer_coordinate` answers for a pointer outside the
//! drawn bounds, so the readout transformed a position that is not on the
//! chart into a feed, an advance per tooth and a burn verdict.
//!
//! # Why this class matters here
//!
//! This is the failure shape this repo has hit repeatedly and names
//! explicitly: **absence rendered as a reading.** An empty triage drawn as an
//! all-clear, a 2D model's zero Z drawn as a blank frame, an empty chart
//! frame that looks like a chart — and now a hazard verdict manufactured from
//! a pointer that is somewhere else entirely. The abstention is the correct
//! output, and it has to be the output.
//!
//! # The bound is the AXIS, not the machine
//!
//! The strip between the spindle cap and the axis maximum is a real place to
//! hover: it is how `· past spindle cap` is read, and that clause is
//! deliberate. So the guard is the plot's own bounds. Arm 3 pins that
//! distinction, because tightening the guard to the machine's cap would
//! silently delete a working feature.

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

/// Room for the whole window, so the plot is drawn unclipped.
const SCREEN: egui::Vec2 = egui::Vec2::new(1400.0, 1000.0);

/// The line the readout shows when there is nothing to report.
const ABSTENTION: &str = "Hover the chart to read an operating point.";

/// A readout of a real operating point contains this phrase.
///
/// It was `" RPM · "`, which is too loose: the legend's `● Now` row reads
/// `17000 RPM · 1291 mm/min` and matched it. That went unnoticed only while
/// the window was short enough to clip that row, so raising the window's
/// default height turned a passing arm red — the test was reading the
/// legend, not the readout, and had been for as long as both existed.
const READING_MARK: &str = "commanded advance/tooth";

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

    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = 1_291.0;
        config.spindle_rpm = Some(17_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Hover bound fixture".to_owned(),
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
        path: PathBuf::from("hover_bound_fixture.svg"),
        name: "Hover bound fixture".to_owned(),
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
        .expect("add the hover bound fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let toolpath_id = state.session.toolpath_configs()[0].id;
    state.feeds_modal = Some(FeedsModalState {
        toolpath_id,
        explore: None,
    });
    state
}

/// Draw the window with the pointer parked at `pointer`, and return every
/// text run it painted.
///
/// The pointer is re-sent every frame: egui only keeps a pointer position
/// while it is being told about one, and the plot needs a settled layout
/// before the coordinate it reports means anything.
fn painted_with_pointer(pointer: egui::Pos2) -> Vec<String> {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let state = fixture();
    let mut texts = Vec::new();
    for pass in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            events: vec![egui::Event::PointerMoved(pointer)],
            ..Default::default()
        };
        let mut events = Vec::new();
        let mut out = ctx.run_ui(input, |ui| {
            feeds::draw(ui.ctx(), &state, &mut events);
        });
        if pass == 3 {
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

/// The window is anchored CENTER_CENTER, so its centre is the screen's. The
/// nomogram occupies the upper half of the body, which puts it under a point
/// a little above centre.
fn a_point_on_the_chart() -> egui::Pos2 {
    egui::pos2(SCREEN.x * 0.5, SCREEN.y * 0.5 - 40.0)
}

// ── arm 1 — the pointer is off the chart ─────────────────────────────────

#[test]
fn the_readout_abstains_when_the_pointer_is_off_the_chart_g_hoverbound() {
    // Top-left of the screen: outside the window entirely, which is where
    // the pointer was when the defect was photographed (over the inspector).
    let texts = painted_with_pointer(egui::pos2(4.0, 4.0));
    let readings: Vec<&String> = texts.iter().filter(|t| t.contains(READING_MARK)).collect();
    assert!(
        readings.is_empty(),
        "the readout reported an operating point for a pointer that is not \
         on the chart: {readings:?}. `pointer_coordinate` answers for a \
         pointer outside the drawn bounds; a verdict built from one is a \
         hazard reading manufactured from an absence."
    );
    assert!(
        texts.iter().any(|t| t == ABSTENTION),
        "the readout line neither reported nor abstained. It must hold its \
         place, or the legend below it jumps as the pointer crosses the plot \
         edge."
    );
}

// ── arm 2 — the pointer is on the chart ──────────────────────────────────

/// Non-vacuity, and the half that stops arm 1 being satisfied by deleting
/// the readout altogether.
#[test]
fn the_readout_reports_when_the_pointer_is_on_the_chart_g_hoverbound() {
    let texts = painted_with_pointer(a_point_on_the_chart());
    assert!(
        texts.iter().any(|t| t.contains(READING_MARK)),
        "the readout said nothing for a pointer ON the chart, so arm 1 passes \
         for the wrong reason — a readout that never reports satisfies it. If \
         the window's layout moved, re-derive `a_point_on_the_chart`. Painted \
         runs: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == ABSTENTION),
        "the readout reported AND abstained in the same frame"
    );
}

// ── arm 3 — the guard is the axis, not the machine cap ───────────────────

/// `· past spindle cap` is a deliberate clause: the plot draws 15 % past the
/// machine's cap so an operator can hover into the forbidden strip and be
/// told. A guard tightened to the machine's cap instead of the axis bound
/// would delete that feature silently, so the source says which it is.
#[test]
fn the_hover_guard_uses_the_axis_bound_g_hoverbound() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds/explore.rs"),
    )
    .expect("read explore source");
    let at = source
        .find("pointer_coordinate()")
        .expect("the hover readout moved or was renamed");
    let guard = &source[at..(at + 400).min(source.len())];
    assert!(
        guard.contains("rpm_axis_max") && guard.contains("feed_axis_max"),
        "the hover guard no longer bounds against the plot's own axes. \
         Bounding against the machine cap instead would delete the \
         `past spindle cap` readout, which is the one thing the 15 % overrun \
         strip exists to show."
    );
    assert!(
        source.contains("past spindle cap"),
        "the `past spindle cap` clause is gone; the overrun strip now reports \
         nothing and the axis bound has nothing left to protect"
    );
}
