//! W3k: the chart labels fit the plot at the default inspector width
//! and at the narrow rail.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::Material;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};

use super::PlacedLabel;
use crate::state::AppState;
use crate::ui::{properties, tokens};

/// The last label layout of the chart on this thread.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub(crate) frame: egui::Rect,
    pub(crate) labels: Vec<PlacedLabel>,
    pub(crate) font_size: f32,
}

thread_local! {
    static LAST: RefCell<Option<Recorded>> = const { RefCell::new(None) };
}

/// Keep the layout of this frame for the test to read.
pub(crate) fn record(frame: egui::Rect, labels: &[PlacedLabel], font_size: f32) {
    LAST.with(|last| {
        *last.borrow_mut() = Some(Recorded {
            frame,
            labels: labels.to_vec(),
            font_size,
        });
    });
}

/// A two-flute flat Pocket of `diameter` at `stickout` in generic
/// softwood at 1291 mm/min and 17000 RPM, selected, on the Feeds tab
/// (the corridor sentry's fixture). Ø6 at 45 mm puts the deflection
/// ceiling off the chart; Ø2 at 120 mm puts it on the chart.
fn fixture(diameter: f64, stickout: f64) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = diameter;
    tool.stickout = stickout;
    tool.flute_count = 2;
    let stock = StockConfig {
        material: Material::default(),
        ..Default::default()
    };
    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = Some(1_291.0);
        config.spindle_rpm = Some(17_000);
    }
    let config = ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Label fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("label_fixture.svg"),
        name: "Label fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the label fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let toolpath_id = state.session().toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(
        toolpath_id,
        crate::state::runtime::ToolpathRuntime::new(true),
    );
    let _ = state.set_focus(crate::state::selection::Selection::Toolpath(toolpath_id));
    state.gui.pending_toolpath_tab = Some((toolpath_id, properties::ToolpathTab::FeedsSpeeds));
    state
}

/// Draw the inspector at `width` until the layout settles. Returns the
/// recorded layout and every text run of the last frame with its rect.
fn paint_at(width: f32, mut state: AppState) -> (Recorded, Vec<(String, egui::Rect)>) {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut texts = Vec::new();
    LAST.with(|last| *last.borrow_mut() = None);
    for pass in 0..6 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::Panel::right("w3k_labels")
                .default_size(width)
                .min_size(width)
                .max_size(width)
                .resizable(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, &mut state, &mut events);
                    });
                });
        });
        if pass == 5 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push((text.galley.job.text.clone(), text.visual_bounding_rect()));
                }
            }
        }
        out.textures_delta.clear();
    }
    let recorded = LAST
        .with(|last| last.borrow().clone())
        .expect("the Feeds tab drew the chart");
    (recorded, texts)
}

/// The labels that must paint on every fixture at both widths. "Now"
/// and "Before derate" may drop when the plot is short of room.
const MUST_PAINT: [&str; 5] = [
    super::LABEL_RECOMMENDED,
    super::LABEL_MAX_FEED,
    super::LABEL_MAX_RPM,
    super::LABEL_RUBBING,
    super::LABEL_MIN_RPM,
];

/// The fixtures: (diameter, stickout, the labels it must paint beyond
/// `MUST_PAINT`).
const CASES: [(f64, f64, &[&str]); 2] =
    [(6.0, 45.0, &[]), (2.0, 120.0, &[super::LABEL_CHIP_LIMIT])];

/// W3k (operator, 2026-10-31: "check they fit well and are legible"):
/// at the 340-point default and the 240-point rail, every label paints,
/// no two labels overlap, every label is inside the plot rect, and the
/// text is at least the small text size.
#[test]
fn the_chart_labels_fit_at_340_and_240_w3k() {
    for (width, (diameter, stickout, extra)) in [340.0, 240.0]
        .into_iter()
        .flat_map(|w| CASES.into_iter().map(move |c| (w, c)))
    {
        let (recorded, texts) = paint_at(width, fixture(diameter, stickout));
        let names: Vec<&str> = recorded.labels.iter().map(|l| l.text).collect();
        for &must in MUST_PAINT.iter().chain(extra.iter()) {
            assert!(
                names.contains(&must),
                "at {width}, Ø{diameter}, the label `{must}` has no place; placed \
                 {names:?} in {:?}",
                recorded.frame
            );
        }
        for label in &recorded.labels {
            assert!(
                recorded.frame.contains_rect(label.rect),
                "at {width} `{}` {:?} is outside the plot {:?}",
                label.text,
                label.rect,
                recorded.frame
            );
            // The text paints, inside its background.
            let painted = texts
                .iter()
                .find(|(t, _)| t == label.text)
                .unwrap_or_else(|| panic!("at {width} `{}` did not paint", label.text));
            assert!(
                label.rect.expand(0.5).contains_rect(painted.1),
                "at {width} the text of `{}` {:?} leaves its background {:?}",
                label.text,
                painted.1,
                label.rect
            );
        }
        for (i, a) in recorded.labels.iter().enumerate() {
            for b in recorded.labels.iter().skip(i + 1) {
                assert!(
                    !a.rect.intersects(b.rect),
                    "at {width} `{}` {:?} overlaps `{}` {:?}",
                    a.text,
                    a.rect,
                    b.text,
                    b.rect
                );
            }
        }
        assert!(
            recorded.font_size >= tokens::SIZE_CAPTION,
            "the labels paint at {} pt, under the small text size {}",
            recorded.font_size,
            tokens::SIZE_CAPTION
        );
    }
}
