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
/// What one settled frame painted: the text runs with their rects, and
/// every open line (`Shape::Path`) with its half width.
struct Painted {
    texts: Vec<(String, egui::Rect)>,
    paths: Vec<(Vec<egui::Pos2>, f32, egui::Color32)>,
}

fn paint_at(width: f32, mut state: AppState) -> (Recorded, Painted) {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut texts = Vec::new();
    let mut paths = Vec::new();
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
                match &clipped.shape {
                    egui::epaint::Shape::Text(text) => {
                        texts.push((text.galley.job.text.clone(), text.visual_bounding_rect()));
                    }
                    egui::epaint::Shape::Path(path) if !path.closed && path.points.len() >= 2 => {
                        let color = match &path.stroke.color {
                            egui::epaint::ColorMode::Solid(c) => *c,
                            egui::epaint::ColorMode::UV(_) => egui::Color32::TRANSPARENT,
                        };
                        paths.push((path.points.clone(), path.stroke.width / 2.0, color));
                    }
                    _ => {}
                }
            }
        }
        out.textures_delta.clear();
    }
    let recorded = LAST
        .with(|last| last.borrow().clone())
        .expect("the Feeds tab drew the chart");
    (recorded, Painted { texts, paths })
}

/// The farthest a line label's box may sit from its own line, in points
/// (lead, 2026-10-31: "within ~4 px").
const LINE_LABEL_REACH: f32 = 4.0;

/// The labels that must paint on every fixture at both widths. The others
/// may drop when the plot is short of room or past `MAX_LABELS`.
const MUST_PAINT: [&str; 3] = [
    super::LABEL_MAX_FEED,
    super::LABEL_MAX_RPM,
    super::LABEL_RUBBING,
];

/// The labels that must paint at 340 beyond `MUST_PAINT`. At the 240 rail
/// the plot is 152 points wide; with the spindle minimum wall at 8000 RPM
/// "Recommended" (82 points) has no place beside the ring that crosses no
/// line (it may cross only the faint vendor range lines), so it drops there
/// and the blue ring stands alone.
const MUST_PAINT_AT_340: [&str; 2] = [super::LABEL_RECOMMENDED, super::LABEL_MIN_RPM];

/// The fixtures: (diameter, stickout, the labels it must paint beyond
/// `MUST_PAINT`). On Ø2 at 120 mm the deflection ceiling is on the chart,
/// but its line runs between the rubbing floor and the suggested line with
/// at most 13 points (to the suggested line) and 22 points (to the floor,
/// over the length of the label) of room: "Chip limit" has no place that
/// crosses no line, so it drops (lead, 2026-10-31: "move or drop a label
/// that cannot fit"). The case stays for the line test.
const CASES: [(f64, f64, &[&str]); 2] = [(6.0, 45.0, &[]), (2.0, 120.0, &[])];

/// W3k (operator, 2026-10-31: "check they fit well and are legible"):
/// at the 340-point default and the 240-point rail, every label paints,
/// no two labels overlap, no label crosses a drawn line (lead, 2026-10-31:
/// "Min RPM" painted over the rubbing line), every label is inside the
/// plot rect, at most `MAX_LABELS` paint, and the text is at least the
/// small text size. The lines are the painted `Shape::Path`s of the frame,
/// read from the output, not from the layout's own list.
#[test]
fn the_chart_labels_fit_at_340_and_240_w3k() {
    for (width, (diameter, stickout, extra)) in [340.0, 240.0]
        .into_iter()
        .flat_map(|w| CASES.into_iter().map(move |c| (w, c)))
    {
        let (recorded, painted) = paint_at(width, fixture(diameter, stickout));
        let texts = &painted.texts;
        assert!(
            recorded.labels.len() <= super::MAX_LABELS,
            "at {width} {} labels paint",
            recorded.labels.len()
        );
        // A faint vendor range line may run under a label that has no
        // other place (`DrawnLine::soft`); every other line may not.
        let faint = super::wash(super::chart::BAND, super::BAND_LIMIT_ALPHA);
        let plot_lines: Vec<&(Vec<egui::Pos2>, f32, egui::Color32)> = painted
            .paths
            .iter()
            .filter(|(points, _, _)| {
                points
                    .iter()
                    .all(|p| recorded.frame.expand(2.0).contains(*p))
            })
            .collect();
        assert!(
            plot_lines.iter().filter(|(_, _, c)| *c != faint).count() >= 4,
            "non-vacuity: the plot paints its lines (walls, rubbing, suggested); found {}",
            plot_lines.len()
        );
        for label in &recorded.labels {
            for (points, half, color) in &plot_lines {
                if *color == faint {
                    continue;
                }
                let crosses = points
                    .windows(2)
                    .any(|w| matches!(w, [a, b] if label.quad.hits_segment(*a, *b, *half)));
                assert!(
                    !crosses,
                    "at {width}, Ø{diameter}, `{}` {:?} crosses a drawn line {points:?}",
                    label.text, label.quad
                );
            }
        }
        // Lead, 2026-10-31: a line label touches its line. Its box is
        // within `LINE_LABEL_REACH` of the line it names, and that line is
        // one the frame painted (so the recorded line is not a stand-in).
        for label in &recorded.labels {
            let Some(line) = &label.line else {
                continue;
            };
            let painted_line = painted.paths.iter().any(|(points, _, _)| {
                points.len() == line.len()
                    && points.iter().zip(line).all(|(p, q)| p.distance(*q) < 1.0)
            });
            assert!(
                painted_line,
                "at {width} `{}` names a line the frame did not paint: {line:?}",
                label.text
            );
            let gap = label.quad.distance_to_line(line);
            assert!(
                gap <= LINE_LABEL_REACH,
                "at {width}, Ø{diameter}, `{}` floats {gap:.1} points off its line \
                 (at most {LINE_LABEL_REACH})",
                label.text
            );
        }
        let names: Vec<&str> = recorded.labels.iter().map(|l| l.text).collect();
        let at_340: &[&str] = if width > 300.0 {
            &MUST_PAINT_AT_340
        } else {
            &[]
        };
        for &must in MUST_PAINT.iter().chain(extra.iter()).chain(at_340.iter()) {
            assert!(
                names.contains(&must),
                "at {width}, Ø{diameter}, the label `{must}` has no place; placed \
                 {names:?} in {:?}",
                recorded.frame
            );
        }
        for label in &recorded.labels {
            assert!(
                label.quad.inside(recorded.frame),
                "at {width} `{}` {:?} is outside the plot {:?}",
                label.text,
                label.quad,
                recorded.frame
            );
            let painted = texts
                .iter()
                .find(|(t, _)| t == label.text)
                .unwrap_or_else(|| panic!("at {width} `{}` did not paint", label.text));
            // An upright text sits inside its background. (The
            // `visual_bounding_rect` of a turned text ignores its angle.)
            if label.quad.angle.abs() < f32::EPSILON {
                let background =
                    egui::Rect::from_center_size(label.quad.center, 2.0 * label.quad.half);
                assert!(
                    background.expand(0.5).contains_rect(painted.1),
                    "at {width} the text of `{}` {:?} leaves its background {background:?}",
                    label.text,
                    painted.1
                );
            }
        }
        for (i, a) in recorded.labels.iter().enumerate() {
            for b in recorded.labels.iter().skip(i + 1) {
                assert!(
                    !a.quad.intersects(&b.quad),
                    "at {width} `{}` {:?} overlaps `{}` {:?}",
                    a.text,
                    a.quad,
                    b.text,
                    b.quad
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

/// W3k (lead, 2026-10-31: "a reader must see which line a label names"):
/// every place a line label tries has its near corner `LABEL_GAP` off the
/// line, for a level, an upright and a sloped line.
#[test]
fn a_line_label_sits_next_to_its_line_w3k() {
    let size = egui::vec2(50.0, 15.0);
    for (a, b) in [
        (egui::pos2(0.0, 100.0), egui::pos2(200.0, 100.0)),
        (egui::pos2(100.0, 200.0), egui::pos2(100.0, 0.0)),
        (egui::pos2(0.0, 200.0), egui::pos2(200.0, 150.0)),
        (egui::pos2(0.0, 200.0), egui::pos2(60.0, 0.0)),
    ] {
        let anchor = super::LabelAnchor::Line(vec![a, b]);
        let dir = (b - a).normalized();
        let normal = egui::vec2(dir.y, -dir.x);
        for quad in super::candidates(&anchor, size) {
            // The distance from the line to the nearest corner of the label.
            let near = quad
                .corners()
                .iter()
                .map(|c| (*c - a).dot(normal).abs())
                .fold(f32::INFINITY, f32::min);
            assert!(
                (near - super::LABEL_GAP).abs() < 0.01,
                "a label place {quad:?} sits {near} off the line {a:?}-{b:?}, not {}",
                super::LABEL_GAP
            );
            assert!(
                !quad.hits_segment(a, b, 0.0),
                "a label place {quad:?} crosses its own line"
            );
        }
    }
}
