//! **G-FSCONTROL — the operator can set a feed, a plunge and an RPM.**
//!
//! # The defect
//!
//! Reported 2026-09-15: *"there is no direct f/s control?"* There was none.
//!
//! The controls had been deliberately concentrated. W3.2 moved feed and
//! plunge OFF the per-operation Geometry panel into the Feeds & Speeds tab's
//! `SPEED` section; W3.1 moved the spindle override there too, as a
//! `PrecedenceField`. `ui/properties/operations/mod.rs` still carries the
//! comment recording both moves.
//!
//! Commit `540715c1` ("UR4 — the feeds modal is Explore only") then deleted
//! that section on its way past. Its stated scope was the MODAL; the SPEED
//! section was inspector furniture the same commit removed, and nothing
//! replaced it. `PrecedenceField` was left with no call site in the crate.
//!
//! The product shipped for several hours with **no way to set a feed rate**.
//! The only write was `⚡ Apply all`, which overwrites RPM, feed, plunge, DOC
//! and WOC together — so an operator who wanted a slower feed had to accept
//! a different cut as well.
//!
//! # Why this sentry exists in this shape
//!
//! Nothing failed when the section was deleted. Every test still passed: the
//! panel rendered, the rail held its width, the apply contract was intact.
//! **The absence of a control is invisible to a suite that only checks what
//! is present.**
//!
//! So this arm drives the real inspector and asserts the controls are THERE,
//! and a second arm asserts they WRITE. A control that renders and writes
//! nothing is the same defect wearing a coat.

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

const PANEL_WIDTH: f32 = 280.0;

/// The three things an operator must be able to set by hand.
const CONTROLS: [&str; 3] = ["Feed:", "Plunge:", "Spindle:"];

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
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

    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Speed control fixture".to_owned(),
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
        path: PathBuf::from("speed_control_fixture.svg"),
        name: "Speed control fixture".to_owned(),
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
        .expect("add the speed control fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, "feeds".to_owned()));
    state
}

fn painted_text() -> Vec<String> {
    let ctx = ctx();
    let mut state = fixture();
    let mut texts = Vec::new();
    for pass in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::right("fscontrol_properties")
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

fn properties_src() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties/mod.rs"))
        .expect("read properties source")
}

// ── arm 1 — the controls are on screen ───────────────────────────────────

#[test]
fn the_feeds_tab_offers_feed_plunge_and_spindle_g_fscontrol() {
    let texts = painted_text();
    let missing: Vec<&str> = CONTROLS
        .iter()
        .copied()
        .filter(|label| !texts.iter().any(|t| t == label))
        .collect();
    assert!(
        missing.is_empty(),
        "the Feeds & Speeds tab paints no {missing:?} control. UR4 deleted the \
         SPEED section and left the product unable to set a feed rate; only \
         `⚡ Apply all` could write, and it overwrites the cut as well."
    );
}

// ── arm 2 — the controls write ───────────────────────────────────────────

/// A rendered control that writes nothing is the same defect in a coat.
///
/// This is source-level on purpose. The write path is a `DragValue` drag,
/// which a headless frame cannot perform without synthesising pointer
/// events against a widget id this test would have to guess. What it CAN
/// check without guessing is that the draw site calls the operation's
/// setters and stamps staleness — the panel idiom whose write-back turns
/// them into a `Command` (WP6).
#[test]
fn the_speed_controls_write_through_the_panel_idiom_g_fscontrol() {
    let source = properties_src();
    let start = source
        .find("fn draw_speed_controls(")
        .expect("draw_speed_controls moved or was renamed");
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset)
        .expect("draw_speed_controls has no closing brace");
    let body = &source[start..end];

    for setter in ["set_feed_rate(", "set_plunge_rate(", "set_spindle_rpm("] {
        assert!(
            body.contains(setter),
            "draw_speed_controls never calls `{setter}`, so the control it \
             draws cannot change the operation"
        );
    }
    assert!(
        body.matches("stale_since = Some").count() >= 3,
        "a speed edit must stamp `stale_since`, or the panel's write-back \
         never sends a Command and the cards stay green over a changed \
         parameter set (G-FRESHSTATE)"
    );
    // The recommendation must not leak into the manual controls. They set
    // what the operation runs; `⚡ Apply all` is the one route from a
    // recommendation into an operation (Checkpoint I).
    assert!(
        !body.contains("FeedsResult") && !body.contains("recommended"),
        "draw_speed_controls reads the calculator. It sets what the operation \
         runs; the recommendation reaches the operation only through the \
         apply funnel."
    );
}

// ── arm 3 — the component the spindle row needs is still wired ───────────

/// `PrecedenceField` exists so the project default stays VISIBLE beside the
/// override. The widget it replaced hid the real default behind a hardcoded
/// 18 000, so the operator could not see which value would actually run
/// (P1-005 / P2-006). Between UR4 and this fix the component had no call
/// site at all, which is how it became deletable-looking.
#[test]
fn the_spindle_override_shows_the_project_default_g_fscontrol() {
    let source = properties_src();
    assert!(
        source.contains("PrecedenceField::new(\"Spindle:\""),
        "the spindle row no longer uses PrecedenceField, so the project \
         default it overrides is not on screen"
    );
    let texts = painted_text();
    assert!(
        texts.iter().any(|t| t.contains("override")),
        "the spindle row paints no override control"
    );
}
