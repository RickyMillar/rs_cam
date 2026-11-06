//! `PlaneChart` — a painter-drawn x/y chart in the visual language of the
//! histogram (`histogram.rs`).
//!
//! Feeds contract P17 (operator 2026-11-06: the histogram is the style
//! bar). The chart draws one x/y plane in the width the panel gives it. The
//! painter draws it directly. It does not use `egui_plot`, because a plot
//! keeps its bounds per id from frame to frame, paints its own grid and
//! text, and its look does not match the histogram under it.
//!
//! # What the chart shows
//!
//! - A frame with one `HAIRLINE` baseline at the bottom and one at the
//!   left; tick marks `SPACE_1` long outward; a faint `HAIRLINE` reference
//!   line at each named y step and none at x. There is no full grid.
//! - Tick labels in the caption font, `TEXT_MUTED`, on 1-2-5 steps
//!   ([`nice_steps`]): the finest step whose labels keep their spacing. A
//!   named edge ([`Axis::edge`]) names the frame edge itself and drops a
//!   tick label within one label spacing of it (the P5d D5 rule). The edge
//!   label is anchored to the edge, as the histogram anchors its range-end
//!   label, so it is never clipped.
//! - Axis titles in `TEXT_FAINT` with the letter spacing of a section
//!   header (the caller writes them in upper case, a unit in its own
//!   case); the y title turns -90 degrees at the left and stays inside
//!   the allocated rect.
//! - The layers, in paint order: soft fills (one `Mesh` of quads, for
//!   example the histogram's `WASH_OK` on `OK`), the reference lines, the
//!   lines (solid or dashed), the marks (a filled dot, a hollow ring, an
//!   outlined diamond), the off-scale rows in the gutter above the frame
//!   ([`PlaneChart::off_scale_top`], a value above the y range), the
//!   labels on the lines (the label kit of [`labels`], clear of the marks;
//!   plain text or on a chip, the caller's [`LabelGround`]), then the
//!   tick labels and titles. A line marked [`ChartLine::beneath`] paints
//!   under the fills.
//! - The room: the left gutter is the widest y tick label plus `SPACE_2`
//!   plus the turned title row; the bottom gutter is one caption row of x
//!   ticks and one of the x title; [`EDGE_ROOM`] at the top and the right,
//!   so a mark on the frame edge paints whole.
//!
//! [`ChartResponse`] gives back the response and the [`ChartMap`] (the
//! frame and the two ranges), so a caller aims a hover or a click in data
//! units.
//!
//! # What the chart does NOT do
//!
//! It does not pick a colour: every line, mark, fill and label takes the
//! colour its caller gives it, from `ui/tokens.rs`. It keeps no state: the
//! caller sets the ranges on every frame. It paints no hover; the caller
//! reads [`ChartResponse::response`].

pub mod labels;

pub use labels::LabelGround;
use labels::{ChartLabel, DrawnLine, LabelAnchor, Nudge, paint_labels, place_label, place_labels};

use super::histogram::CAP_HALF_WIDTH;
use crate::ui::tokens;

/// The room past the frame edge at the top and at the right, in points:
/// the half width of the widest mark (a diamond of radius 6) and its
/// outline, so a mark on the edge paints whole.
pub const EDGE_ROOM: f32 = 7.0;

/// The length of a tick mark, outward from the frame, in points.
const TICK_LENGTH: f32 = tokens::SPACE_1;

/// The least spacing of two x tick labels, in points (P5c: 45 points keeps
/// the quarter ticks of a 100 % axis at 340 points and drops them at 240).
const X_TICK_SPACING: f32 = 45.0;

/// The least spacing of two y tick labels, in points (the egui_plot y
/// default the envelope plot read before P17).
const Y_TICK_SPACING: f32 = 20.0;

/// The room a label keeps around a mark, in points.
const MARK_CLEARANCE: f32 = 4.0;

/// The colour of a tick label.
pub const TICK_TEXT: egui::Color32 = tokens::TEXT_MUTED;

/// The colour of an axis title.
pub const TITLE_TEXT: egui::Color32 = tokens::TEXT_FAINT;

/// The letter spacing of an axis title: the section header's small caps.
pub const TITLE_LETTER_SPACING: f32 = tokens::SIZE_CAPTION * 0.07;

/// The caption font of every chart text.
#[must_use]
pub fn caption_font() -> egui::FontId {
    egui::FontId::proportional(tokens::SIZE_CAPTION)
}

/// The steps of a chart axis over `lo..hi`: a 1-2-5 step near a third of
/// the range for the named ticks, half and a tenth of it for the finer
/// ones: `[fine, half, large]`. A range that is not positive reads as 1.
#[must_use]
pub fn nice_steps(lo: f64, hi: f64) -> [f64; 3] {
    let range = (hi - lo).abs();
    let raw = if range.is_finite() && range > 0.0 {
        range / 3.0
    } else {
        1.0
    };
    let power = 10_f64.powf(raw.log10().floor());
    let large = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * power);
    [large / 10.0, large / 2.0, large]
}

/// The text of a tick: the fewest decimals (up to three) that print the
/// value exactly: "2.5", "0.25", "25".
#[must_use]
pub fn tick_text(v: f64) -> String {
    for decimals in 0..=3 {
        let text = format!("{v:.decimals$}");
        if text
            .parse::<f64>()
            .is_ok_and(|back| (back - v).abs() < 1e-9)
        {
            return text;
        }
    }
    format!("{v:.3}")
}

/// One axis: its range, its title and an optional name for the frame edge
/// at the top of the range.
#[derive(Debug, Clone)]
pub struct Axis<'a> {
    range: (f64, f64),
    title: &'a str,
    edge: Option<String>,
}

impl<'a> Axis<'a> {
    /// An axis over `range` with `title`: upper case, a unit in its own
    /// case ("DEPTH (mm)").
    #[must_use]
    pub const fn new(range: (f64, f64), title: &'a str) -> Self {
        Self {
            range,
            title,
            edge: None,
        }
    }

    /// Name the frame edge at the top of the range (D5): the edge names
    /// itself, and a tick label within one label spacing of it goes.
    #[must_use]
    pub fn edge(mut self, text: impl Into<String>) -> Self {
        self.edge = Some(text.into());
        self
    }
}

/// One line of the chart, in data units.
#[derive(Debug, Clone)]
pub struct ChartLine {
    points: Vec<[f64; 2]>,
    stroke: egui::Stroke,
    dash: Option<(f32, f32)>,
    soft: bool,
    beneath: bool,
}

impl ChartLine {
    /// A solid line through `points`.
    #[must_use]
    pub const fn new(points: Vec<[f64; 2]>, stroke: egui::Stroke) -> Self {
        Self {
            points,
            stroke,
            dash: None,
            soft: false,
            beneath: false,
        }
    }

    /// Paint the line under the fills (a frame edge under the density
    /// cells, so a sample on the edge is not cut by it; P17 round 5).
    #[must_use]
    pub const fn beneath(mut self, beneath: bool) -> Self {
        self.beneath = beneath;
        self
    }

    /// Dash the line: `dash` points on, `gap` points off.
    #[must_use]
    pub const fn dashed(mut self, dash: f32, gap: f32) -> Self {
        self.dash = Some((dash, gap));
        self
    }

    /// A faint line: a label may cross it when it has no other place.
    #[must_use]
    pub const fn soft(mut self, soft: bool) -> Self {
        self.soft = soft;
        self
    }
}

/// The shape of a mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// A filled dot.
    Dot,
    /// A hollow ring.
    Ring,
    /// An outlined diamond.
    Diamond,
}

/// One mark of the chart, in data units.
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    at: [f64; 2],
    kind: MarkKind,
    radius: f32,
    color: egui::Color32,
    width: f32,
}

impl Mark {
    /// A filled dot of `radius` at `at`.
    #[must_use]
    pub const fn dot(at: [f64; 2], radius: f32, color: egui::Color32) -> Self {
        Self {
            at,
            kind: MarkKind::Dot,
            radius,
            color,
            width: 0.0,
        }
    }

    /// A hollow ring of `radius` at `at`, outlined `width` points.
    #[must_use]
    pub const fn ring(at: [f64; 2], radius: f32, stroke: egui::Stroke) -> Self {
        Self {
            at,
            kind: MarkKind::Ring,
            radius,
            color: stroke.color,
            width: stroke.width,
        }
    }

    /// An outlined diamond of `radius` at `at`.
    #[must_use]
    pub const fn diamond(at: [f64; 2], radius: f32, stroke: egui::Stroke) -> Self {
        Self {
            at,
            kind: MarkKind::Diamond,
            radius,
            color: stroke.color,
            width: stroke.width,
        }
    }
}

/// What a label names, in data units.
#[derive(Debug, Clone)]
pub enum LabelAt {
    /// A line: the label sits beside it.
    Line(Vec<[f64; 2]>),
    /// Any one of several lines (the two segments of a step).
    Lines(Vec<Vec<[f64; 2]>>),
    /// A mark of `radius` points at `at`.
    Mark { at: [f64; 2], radius: f32 },
}

/// The most off-scale rows the top gutter holds ([`PlaneChart::off_scale_top`]).
pub const OFF_SCALE_ROWS: usize = 3;

/// One label to place, in priority order: the most important first.
#[derive(Debug, Clone)]
pub struct Label {
    text: String,
    color: egui::Color32,
    at: LabelAt,
    ground: LabelGround,
    leader: bool,
}

impl Label {
    /// A label `text` in `color` beside `at`, on a chip of the overlay
    /// surface.
    #[must_use]
    pub fn new(text: impl Into<String>, color: egui::Color32, at: LabelAt) -> Self {
        Self {
            text: text.into(),
            color,
            at,
            ground: LabelGround::Overlay,
            leader: false,
        }
    }

    /// The ground of the label: `LabelGround::None` paints the text alone,
    /// as the histogram does.
    #[must_use]
    pub const fn ground(mut self, ground: LabelGround) -> Self {
        self.ground = ground;
        self
    }

    /// A mark label with its leader always drawn (a 1 pt `TEXT_MUTED` line
    /// from the mark's edge to the label), even when the label sits beside
    /// the mark: the label points at what it names (P17 round 5).
    #[must_use]
    pub const fn leader(mut self) -> Self {
        self.leader = true;
        self
    }
}

/// A soft fill: convex quads in data units, one colour.
#[derive(Debug, Clone)]
struct Fill {
    quads: Vec<[[f64; 2]; 4]>,
    color: egui::Color32,
}

/// The map between the data plane and the screen: the frame rect and the
/// two ranges. It replaces `egui_plot::PlotTransform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartMap {
    /// The frame: the x range runs along its width, the y range up its
    /// height.
    pub frame: egui::Rect,
    /// The x range, left to right.
    pub x: (f64, f64),
    /// The y range, bottom to top.
    pub y: (f64, f64),
}

impl ChartMap {
    /// The screen position of the data point `(x, y)`; a point past a
    /// range lands past the frame.
    #[must_use]
    pub fn to_screen(&self, x: f64, y: f64) -> egui::Pos2 {
        let f = self.frame;
        let tx = share(x, self.x);
        let ty = share(y, self.y);
        egui::pos2(f.left() + tx * f.width(), f.bottom() - ty * f.height())
    }

    /// The data point under `pos`, held on the ranges: a pointer past the
    /// frame reads as the frame edge.
    #[must_use]
    pub fn from_screen(&self, pos: egui::Pos2) -> (f64, f64) {
        let f = self.frame;
        let tx = ((pos.x - f.left()) / f.width().max(f32::EPSILON)).clamp(0.0, 1.0);
        let ty = ((f.bottom() - pos.y) / f.height().max(f32::EPSILON)).clamp(0.0, 1.0);
        (
            (self.x.0 + f64::from(tx) * (self.x.1 - self.x.0)).clamp(self.x.0, self.x.1),
            (self.y.0 + f64::from(ty) * (self.y.1 - self.y.0)).clamp(self.y.0, self.y.1),
        )
    }
}

/// `v` as a share of `range`, not clamped.
fn share(v: f64, range: (f64, f64)) -> f32 {
    let span = range.1 - range.0;
    if span.abs() <= f64::EPSILON || !v.is_finite() {
        0.0
    } else {
        ((v - range.0) / span) as f32
    }
}

/// The geometry of one chart: the map, the named ticks and the font.
struct Geometry {
    map: ChartMap,
    x_ticks: Vec<f64>,
    y_ticks: Vec<f64>,
    font: egui::FontId,
    row: f32,
}

/// What the operator did to the chart this frame, and where it is.
pub struct ChartResponse {
    /// The response of the allocated rect, with the chart's sense.
    pub response: egui::Response,
    /// The frame and the ranges.
    pub map: ChartMap,
}

/// An x/y chart for a narrow panel.
pub struct PlaneChart<'a> {
    x: Axis<'a>,
    y: Axis<'a>,
    height: f32,
    sense: egui::Sense,
    fills: Vec<Fill>,
    lines: Vec<ChartLine>,
    marks: Vec<Mark>,
    labels: Vec<Label>,
    avoid: Vec<[[f64; 2]; 4]>,
    off_scale: Vec<(String, egui::Color32)>,
}

impl<'a> PlaneChart<'a> {
    /// A chart of `height` points over the two axes, with `sense`.
    #[must_use]
    pub const fn new(x: Axis<'a>, y: Axis<'a>, height: f32, sense: egui::Sense) -> Self {
        Self {
            x,
            y,
            height,
            sense,
            fills: Vec::new(),
            lines: Vec::new(),
            marks: Vec::new(),
            labels: Vec::new(),
            avoid: Vec::new(),
            off_scale: Vec::new(),
        }
    }

    /// An off-scale value above the y range (P17 round 5): one caption row
    /// in a gutter ABOVE the frame, right-aligned to the frame's right
    /// edge, a small up-arrow painted at the text's left, in `color` (the
    /// histogram's off-scale idiom: the arrow beside the text, outside the
    /// data area). The chart allocates the gutter only when it holds a
    /// row; at most [`OFF_SCALE_ROWS`] rows paint.
    #[must_use]
    pub fn off_scale_top(mut self, text: impl Into<String>, color: egui::Color32) -> Self {
        self.off_scale.push((text.into(), color));
        self
    }

    /// The height of the off-scale gutter above the frame, in points.
    fn top_gutter(&self, row: f32) -> f32 {
        self.off_scale.len().min(OFF_SCALE_ROWS) as f32 * (row + tokens::SPACE_1)
    }

    /// The height the chart allocates: its height and the off-scale gutter.
    fn total_height(&self, ui: &egui::Ui) -> f32 {
        let row = ui
            .ctx()
            .fonts_mut(|fonts| fonts.row_height(&caption_font()));
        self.height + self.top_gutter(row)
    }

    /// Quads in data units the labels keep clear of, as they keep clear
    /// of the marks (P17 round 4: the sim chart's densest row of cells, so
    /// the cluster label sits beside the band, not on it).
    #[must_use]
    pub fn keep_clear(mut self, quads: Vec<[[f64; 2]; 4]>) -> Self {
        self.avoid.extend(quads);
        self
    }

    /// A soft fill of convex `quads`, behind every line.
    #[must_use]
    pub fn fill(mut self, quads: Vec<[[f64; 2]; 4]>, color: egui::Color32) -> Self {
        self.fills.push(Fill { quads, color });
        self
    }

    /// A line, over the fills; the first line paints first.
    #[must_use]
    pub fn line(mut self, line: ChartLine) -> Self {
        self.lines.push(line);
        self
    }

    /// A mark, over the lines; the labels keep clear of it.
    #[must_use]
    pub fn mark(mut self, mark: Mark) -> Self {
        self.marks.push(mark);
        self
    }

    /// A label, in priority order: the first label takes its place first.
    #[must_use]
    pub fn label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// The frame the chart takes in the full available width, before it is
    /// drawn: the frame of [`Self::show`] on this `ui`. A caller that
    /// decides what to draw by the frame size (a line that hugs the frame)
    /// reads it here first.
    #[must_use]
    pub fn frame(&self, ui: &egui::Ui) -> egui::Rect {
        let rect = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(self.width(ui), self.total_height(ui)),
        );
        self.geometry(ui, rect).map.frame
    }

    /// The width the chart allocates on `ui`.
    fn width(&self, ui: &egui::Ui) -> f32 {
        ui.available_width().max(4.0 * EDGE_ROOM)
    }

    /// The geometry of the chart in `rect`: the frame, the map and the
    /// named ticks.
    fn geometry(&self, ui: &egui::Ui, rect: egui::Rect) -> Geometry {
        let font = caption_font();
        let row = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        // The bottom gutter: the tick row and the title row. The frame
        // height follows, so the y ticks are known before the left gutter.
        let bottom_gutter = TICK_LENGTH + tokens::SPACE_1 + row + tokens::SPACE_1 + row;
        let frame_top = rect.top() + self.top_gutter(row) + EDGE_ROOM;
        let frame_bottom = (rect.bottom() - bottom_gutter).max(frame_top + 1.0);
        let frame_height = frame_bottom - frame_top;
        let y_ticks = ticks(
            self.y.range,
            frame_height,
            Y_TICK_SPACING,
            self.y.edge.is_some(),
        );
        let measure = |text: &str| {
            ui.painter()
                .layout_no_wrap(text.to_owned(), font.clone(), TICK_TEXT)
                .size()
                .x
        };
        let widest_y = y_ticks
            .iter()
            .map(|v| measure(&tick_text(*v)))
            .chain(self.y.edge.as_deref().map(measure))
            .fold(0.0_f32, f32::max);
        let left_gutter = row + tokens::SPACE_2 + widest_y + tokens::SPACE_2 + TICK_LENGTH;
        let frame_left = rect.left() + left_gutter;
        let frame_right = (rect.right() - EDGE_ROOM).max(frame_left + 1.0);
        let frame = egui::Rect::from_min_max(
            egui::pos2(frame_left, frame_top),
            egui::pos2(frame_right, frame_bottom),
        );
        let x_ticks = ticks(
            self.x.range,
            frame.width(),
            X_TICK_SPACING,
            self.x.edge.is_some(),
        );
        Geometry {
            map: ChartMap {
                frame,
                x: self.x.range,
                y: self.y.range,
            },
            x_ticks,
            y_ticks,
            font,
            row,
        }
    }

    /// Draw the chart in the full available width.
    pub fn show(self, ui: &mut egui::Ui) -> ChartResponse {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(self.width(ui), self.total_height(ui)),
            self.sense,
        );
        let Geometry {
            map,
            x_ticks,
            y_ticks,
            font,
            row,
        } = self.geometry(ui, rect);
        let frame = map.frame;

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            // The lines and fills stop at the frame; a mark or a label on
            // the edge paints whole in the room past it.
            let clipped = ui.painter_at(frame.expand(1.0));
            let hairline = egui::Stroke::new(1.0, tokens::HAIRLINE);

            let mut drawn: Vec<DrawnLine> = Vec::with_capacity(self.lines.len());
            let paint_line = |line: &ChartLine, drawn: &mut Vec<DrawnLine>| {
                let points: Vec<egui::Pos2> = line
                    .points
                    .iter()
                    .map(|p| map.to_screen(p[0], p[1]))
                    .collect();
                if points.len() < 2 {
                    return;
                }
                match line.dash {
                    Some((dash, gap)) => {
                        clipped.extend(egui::Shape::dashed_line(&points, line.stroke, dash, gap));
                    }
                    None => {
                        let _ = clipped.add(egui::Shape::line(points.clone(), line.stroke));
                    }
                }
                drawn.push(DrawnLine {
                    points,
                    half_width: line.stroke.width / 2.0,
                    soft: line.soft,
                });
            };
            // The lines under the fills (a frame edge under the cells).
            for line in self.lines.iter().filter(|l| l.beneath) {
                paint_line(line, &mut drawn);
            }
            for fill in &self.fills {
                paint_fill(&clipped, &map, fill);
            }
            for v in &y_ticks {
                if *v > self.y.range.0 {
                    let y = map.to_screen(self.x.range.0, *v).y;
                    let _ = painter.line_segment(
                        [egui::pos2(frame.left(), y), egui::pos2(frame.right(), y)],
                        hairline,
                    );
                }
            }
            let _ = painter.line_segment([frame.left_bottom(), frame.right_bottom()], hairline);
            let _ = painter.line_segment([frame.left_top(), frame.left_bottom()], hairline);

            for line in self.lines.iter().filter(|l| !l.beneath) {
                paint_line(line, &mut drawn);
            }

            let mut keep_clear: Vec<egui::Rect> = Vec::with_capacity(self.marks.len());
            for mark in &self.marks {
                let c = map.to_screen(mark.at[0], mark.at[1]);
                keep_clear.push(egui::Rect::from_center_size(
                    c,
                    egui::Vec2::splat(2.0 * mark.radius + MARK_CLEARANCE),
                ));
                match mark.kind {
                    MarkKind::Dot => {
                        let _ = painter.circle_filled(c, mark.radius, mark.color);
                    }
                    MarkKind::Ring => {
                        let _ = painter.circle_stroke(
                            c,
                            mark.radius,
                            egui::Stroke::new(mark.width, mark.color),
                        );
                    }
                    MarkKind::Diamond => {
                        let r = mark.radius;
                        let _ = painter.add(egui::Shape::closed_line(
                            vec![
                                c + egui::vec2(0.0, -r),
                                c + egui::vec2(r, 0.0),
                                c + egui::vec2(0.0, r),
                                c + egui::vec2(-r, 0.0),
                            ],
                            egui::Stroke::new(mark.width, mark.color),
                        ));
                    }
                }
            }

            for quad in &self.avoid {
                let corners: Vec<egui::Pos2> =
                    quad.iter().map(|p| map.to_screen(p[0], p[1])).collect();
                keep_clear.push(egui::Rect::from_points(&corners));
            }

            // The off-scale rows in the gutter above the frame: the text
            // right-aligned to the frame's right edge, the arrow at its
            // left (the histogram's `paint_off_scale`, pointing up).
            for (k, (text, color)) in self.off_scale.iter().take(OFF_SCALE_ROWS).enumerate() {
                let top = rect.top() + k as f32 * (row + tokens::SPACE_1);
                let galley = painter.layout_no_wrap(text.clone(), font.clone(), *color);
                let text_left = frame.right() - galley.size().x;
                painter.galley(egui::pos2(text_left, top), galley, *color);
                let mid_y = top + row / 2.0;
                let arrow_h = CAP_HALF_WIDTH * 1.5;
                let tip_x = text_left - tokens::SPACE_1 - CAP_HALF_WIDTH;
                let _ = painter.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(tip_x, mid_y - arrow_h / 2.0),
                        egui::pos2(tip_x + CAP_HALF_WIDTH, mid_y + arrow_h / 2.0),
                        egui::pos2(tip_x - CAP_HALF_WIDTH, mid_y + arrow_h / 2.0),
                    ],
                    *color,
                    egui::Stroke::NONE,
                ));
            }

            let specs: Vec<ChartLabel> = self
                .labels
                .iter()
                .map(|l| ChartLabel {
                    text: l.text.clone(),
                    color: l.color,
                    ground: l.ground,
                    anchor: match &l.at {
                        LabelAt::Line(points) => LabelAnchor::Line(
                            points.iter().map(|p| map.to_screen(p[0], p[1])).collect(),
                        ),
                        LabelAt::Lines(lines) => LabelAnchor::Lines(
                            lines
                                .iter()
                                .map(|line| {
                                    line.iter().map(|p| map.to_screen(p[0], p[1])).collect()
                                })
                                .collect(),
                        ),
                        LabelAt::Mark { at, radius } => LabelAnchor::Mark {
                            center: map.to_screen(at[0], at[1]),
                            radius: *radius,
                        },
                    },
                })
                .collect();
            let placed = place_labels(ui, frame, &keep_clear, &drawn, specs);
            // The leaders a caller asked for on every placement: from the
            // mark's edge to the label's near point, under the text.
            for p in &placed {
                let wants = self
                    .labels
                    .iter()
                    .any(|l| l.leader && l.text == p.text && l.color == p.color);
                if let (true, Some((center, radius))) = (wants, p.mark) {
                    let near = egui::Rect::from_center_size(p.quad.center, 2.0 * p.quad.half)
                        .clamp(center);
                    let off = near - center;
                    if off.length() > 0.0 {
                        let start = center + off.normalized() * radius.min(off.length());
                        let _ = painter.line_segment(
                            [start, near],
                            egui::Stroke::new(1.0, tokens::TEXT_MUTED),
                        );
                    }
                }
            }
            paint_labels(ui, frame, &placed);

            paint_axes(
                &painter, &map, rect, &self.x, &self.y, &x_ticks, &y_ticks, &font, row,
            );
        }

        ChartResponse { response, map }
    }
}

/// The named ticks of an axis over `range` on `px` points: the finest 1-2-5
/// step whose labels are at least `spacing` points apart. With a named
/// edge, a tick at the edge or within `spacing` points of it goes: the
/// edge names itself (D5).
fn ticks(range: (f64, f64), px: f32, spacing: f32, named_edge: bool) -> Vec<f64> {
    let span = range.1 - range.0;
    if !(span.is_finite() && span > 0.0) || px <= 0.0 {
        return Vec::new();
    }
    let per_unit = f64::from(px) / span;
    let steps = nice_steps(range.0, range.1);
    let step = steps
        .iter()
        .copied()
        .find(|s| s * per_unit >= f64::from(spacing))
        .unwrap_or(steps[2]);
    let eps = step * 1e-6;
    let first = ((range.0 - eps) / step).ceil() as i64;
    let last = ((range.1 + eps) / step).floor() as i64;
    (first..=last)
        .map(|k| k as f64 * step)
        .filter(|v| {
            !named_edge
                || (range.1 - v).abs() > eps && (range.1 - v) * per_unit >= f64::from(spacing)
        })
        .collect()
}

/// Paint one fill as one mesh of quads.
fn paint_fill(painter: &egui::Painter, map: &ChartMap, fill: &Fill) {
    if fill.quads.is_empty() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for quad in &fill.quads {
        let i = mesh.vertices.len() as u32;
        for p in quad {
            mesh.colored_vertex(map.to_screen(p[0], p[1]), fill.color);
        }
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    let _ = painter.add(egui::Shape::mesh(mesh));
}

/// The tick marks, the tick labels and the two titles.
#[expect(
    clippy::too_many_arguments,
    reason = "a private painter helper; the nine inputs are one frame's geometry, axes, ticks and font, and a struct for them would be used once."
)]
fn paint_axes(
    painter: &egui::Painter,
    map: &ChartMap,
    rect: egui::Rect,
    x: &Axis<'_>,
    y: &Axis<'_>,
    x_ticks: &[f64],
    y_ticks: &[f64],
    font: &egui::FontId,
    row: f32,
) {
    let frame = map.frame;
    let tick = egui::Stroke::new(1.0, tokens::HAIRLINE);
    let mut placed: Vec<egui::Rect> = Vec::new();
    let label_y = frame.bottom() + TICK_LENGTH + tokens::SPACE_1;
    // `SPACE_2` of air left of the y ticks: the y "0" then sits clear of
    // the x "0" under the corner, and both paint.
    let label_right = frame.left() - TICK_LENGTH - tokens::SPACE_2;

    // The edge labels first: the edge is the frame, and it names itself.
    if let Some(text) = &x.edge {
        let _ = painter.line_segment(
            [
                frame.right_bottom(),
                frame.right_bottom() + egui::vec2(0.0, TICK_LENGTH),
            ],
            tick,
        );
        place_label(
            painter,
            &mut placed,
            rect,
            egui::pos2(frame.right(), label_y),
            egui::Align2::RIGHT_TOP,
            text,
            font,
            TICK_TEXT,
            Nudge::None,
        );
    }
    if let Some(text) = &y.edge {
        let _ = painter.line_segment(
            [
                frame.left_top(),
                frame.left_top() - egui::vec2(TICK_LENGTH, 0.0),
            ],
            tick,
        );
        // Anchored to the top edge, held inside the rect.
        let top = (frame.top() - row / 2.0).max(rect.top());
        place_label(
            painter,
            &mut placed,
            rect,
            egui::pos2(label_right, top),
            egui::Align2::RIGHT_TOP,
            text,
            font,
            TICK_TEXT,
            Nudge::None,
        );
    }
    for v in y_ticks {
        let py = map.to_screen(x.range.0, *v).y;
        let _ = painter.line_segment(
            [
                egui::pos2(frame.left(), py),
                egui::pos2(frame.left() - TICK_LENGTH, py),
            ],
            tick,
        );
        place_label(
            painter,
            &mut placed,
            rect,
            egui::pos2(label_right, py),
            egui::Align2::RIGHT_CENTER,
            &tick_text(*v),
            font,
            TICK_TEXT,
            Nudge::None,
        );
    }

    for v in x_ticks {
        let px = map.to_screen(*v, y.range.0).x;
        let _ = painter.line_segment(
            [
                egui::pos2(px, frame.bottom()),
                egui::pos2(px, frame.bottom() + TICK_LENGTH),
            ],
            tick,
        );
        place_label(
            painter,
            &mut placed,
            rect,
            egui::pos2(px, label_y),
            egui::Align2::CENTER_TOP,
            &tick_text(*v),
            font,
            TICK_TEXT,
            Nudge::None,
        );
    }
    // The titles: the x title under the tick row, the y title turned -90
    // degrees at the left, inside the rect.
    let x_title = title_galley(painter, x.title);
    painter.galley(
        egui::Align2::CENTER_TOP
            .anchor_size(
                egui::pos2(frame.center().x, rect.bottom() - row),
                x_title.size(),
            )
            .min,
        x_title,
        TITLE_TEXT,
    );
    let y_title = title_galley(painter, y.title);
    let len = y_title.size().x;
    let bottom = (frame.center().y + len / 2.0)
        .min(rect.bottom())
        .max(rect.top() + len);
    let _ = painter.add(
        egui::epaint::TextShape::new(egui::pos2(rect.left(), bottom), y_title, TITLE_TEXT)
            .with_angle(-std::f32::consts::FRAC_PI_2),
    );
}

/// An axis title: caption size, letter spaced. The caller writes the
/// title in upper case and keeps the case of a unit ("DEPTH (mm)").
fn title_galley(painter: &egui::Painter, title: &str) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        title,
        0.0,
        egui::TextFormat {
            font_id: caption_font(),
            color: TITLE_TEXT,
            extra_letter_spacing: TITLE_LETTER_SPACING,
            ..Default::default()
        },
    );
    painter.layout_job(job)
}
