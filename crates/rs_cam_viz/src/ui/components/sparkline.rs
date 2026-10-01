//! `Sparkline` and `ChartFlip` — the "over time" face of a cut-metric card.
//!
//! Operator request 2026-10-02: each cut-metric histogram has a flip button
//! beside it. The button swaps the histogram for a small line of the same
//! metric over the run of the toolpath. The bottom time-series drawer is
//! deleted. The painter draws the line directly, for the reason
//! `histogram.rs` gives: a plot keeps its bounds per id from frame to frame,
//! and the rail needs no axes and no zoom.
//!
//! # What the line shows
//!
//! - x is the toolpath-local move index of each sample, in trace order. A
//!   click seeks the playhead to the first sample under the pointer.
//! - Each piece of the line takes the band colour of its value, from
//!   [`histogram::side_colour`]: the bars of the histogram read the same
//!   function, so one value has one colour on both faces. A piece that
//!   crosses a bound is split at the crossing.
//! - The band is the same faint zone wash the histogram paints, turned on
//!   its side, with a dashed limit line at each bound.
//! - A gap breaks the line: a sample with no finite value, or a pixel
//!   column with no sample. A gap is "not measured", never zero.
//!
//! # The y scale
//!
//! The y range is [`histogram::display_range`], the same range the
//! histogram's x axis uses. Core sets it to a percentile range of the gate
//! population (`RANGE_PERCENTILE_LOW` to `RANGE_PERCENTILE_HIGH`), and the
//! chart widens it to include each bound within [`histogram::NEAR_BOUND_SHARE`]
//! of it. The rule has three parts:
//!
//! 1. The range follows the data, so one spike cannot flatten the line.
//! 2. A value outside the range is clamped to the edge, and the column gets
//!    a small triangle at that edge in the value's band colour.
//! 3. A bound far outside the range does not widen it. It is an off-scale
//!    marker at the edge: a triangle and the bound's value with its unit.
//!
//! Part 3 is a deliberate difference from "always include the limit line".
//! The deleted drawer always included the bounds, and a spindle-power limit
//! forty times the peak then pressed the whole line flat against the
//! bottom. The histogram met the same defect on 2026-09-24 and took this
//! rule; the line takes the same rule, so the two faces agree.
//!
//! # Decimation
//!
//! [`columns`] keeps the minimum, the maximum, the first and the last value
//! of each pixel column. The line draws the min-to-max range of each column
//! as a vertical piece, so a one-sample spike or dip is never lost. A
//! max-only decimation would hide every dip below the chipload floor.

use rs_cam_core::tool_load::Histogram;

use crate::ui::components::histogram::{self, BinSide, BoundPlace};
use crate::ui::tokens;

/// The width of the line, in points.
pub const LINE_WIDTH: f32 = 1.5;

/// The side of the square flip button, in points.
pub const FLIP_SIZE: f32 = tokens::SPACE_5;

/// The space above and below the data area, in points. The outlier
/// triangles sit in it.
const EDGE_PAD: f32 = histogram::CAP_HALF_WIDTH + 1.0;

/// One pixel column of the line, in core's unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub min: f64,
    pub max: f64,
    /// The first finite value in trace order.
    pub first: f64,
    /// The last finite value in trace order.
    pub last: f64,
    /// The toolpath-local move of the first finite sample.
    pub first_move: usize,
    /// True when the line joins the previous column to this one: the
    /// previous column holds the previous finite sample, and no gap lies
    /// between them.
    pub joined: bool,
}

/// The columns of one series, and the move range they cover.
#[derive(Clone, Debug, PartialEq)]
pub struct Columns {
    /// One entry per pixel column. `None` is a gap: no finite sample.
    pub columns: Vec<Option<Column>>,
    pub first_move: usize,
    pub last_move: usize,
}

impl Columns {
    /// The number of moves the columns cover.
    #[must_use]
    pub fn move_span(&self) -> usize {
        self.last_move - self.first_move + 1
    }

    /// The column that holds `local_move`, or `None` outside the range.
    #[must_use]
    pub fn column_of(&self, local_move: usize) -> Option<usize> {
        if local_move < self.first_move || local_move > self.last_move {
            return None;
        }
        let count = self.columns.len().max(1);
        Some(((local_move - self.first_move) * count / self.move_span()).min(count - 1))
    }
}

/// Put `series` into at most `max_columns` pixel columns.
///
/// `series` is `(toolpath-local move, value)` in trace order. The column
/// count is never more than the number of moves in the range, so a short
/// toolpath does not leave an empty column between two moves. A value that
/// is not finite is a gap: it is not drawn, and the line does not join
/// across it. Returns `None` when no value is finite.
#[must_use]
pub fn columns(series: &[(usize, f64)], max_columns: usize) -> Option<Columns> {
    if !series.iter().any(|(_, value)| value.is_finite()) {
        return None;
    }
    let first_move = series.iter().map(|(m, _)| *m).min()?;
    let last_move = series.iter().map(|(m, _)| *m).max()?;
    let span = last_move - first_move + 1;
    let count = max_columns.max(1).min(span);
    let mut out = Columns {
        columns: vec![None; count],
        first_move,
        last_move,
    };
    let mut after_gap = true;
    let mut last_column: Option<usize> = None;
    for &(local_move, value) in series {
        if !value.is_finite() {
            after_gap = true;
            continue;
        }
        let Some(index) = out.column_of(local_move) else {
            continue;
        };
        let joined = !after_gap && index > 0 && last_column == Some(index - 1);
        if let Some(slot) = out.columns.get_mut(index) {
            match slot {
                Some(column) => {
                    column.min = column.min.min(value);
                    column.max = column.max.max(value);
                    column.last = value;
                }
                None => {
                    *slot = Some(Column {
                        min: value,
                        max: value,
                        first: value,
                        last: value,
                        first_move: local_move,
                        joined,
                    });
                }
            }
        }
        after_gap = false;
        last_column = Some(index);
    }
    Some(out)
}

/// Split the piece from `v0` to `v1` at each bound it crosses.
///
/// Returns `(t_start, t_end, side)` pieces in order, with `t` from 0 at
/// `v0` to 1 at `v1`. Each piece takes the side of its midpoint value, so
/// it takes one band colour.
#[must_use]
pub fn split_at_bounds(
    v0: f64,
    v1: f64,
    floor: Option<f64>,
    ceiling: Option<f64>,
) -> Vec<(f64, f64, BinSide)> {
    let mut cuts = vec![0.0, 1.0];
    for bound in [floor, ceiling].into_iter().flatten() {
        if bound.is_finite() && (v0 - bound) * (v1 - bound) < 0.0 {
            cuts.push((bound - v0) / (v1 - v0));
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.windows(2)
        .filter_map(|pair| {
            let (&t0, &t1) = (pair.first()?, pair.get(1)?);
            (t1 > t0).then(|| {
                let mid = v0 + (v1 - v0) * 0.5 * (t0 + t1);
                (t0, t1, histogram::value_side(floor, ceiling, mid))
            })
        })
        .collect()
}

/// Where a gutter label sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LabelAt {
    /// At the y of its value: a to-scale bound.
    Value,
    /// At the top edge: the top of the range.
    Top,
    /// At the bottom edge: the bottom of the range.
    Bottom,
}

/// The vertical geometry of one line: the data area and its value range.
#[derive(Clone, Copy, Debug)]
struct YScale {
    top: f32,
    bottom: f32,
    lo: f64,
    hi: f64,
}

impl YScale {
    /// The y of `value`, clamped to the data area.
    fn y(&self, value: f64) -> f32 {
        let span = self.hi - self.lo;
        if span <= 0.0 || !value.is_finite() {
            return 0.5 * (self.top + self.bottom);
        }
        let t = ((value - self.lo) / span).clamp(0.0, 1.0) as f32;
        self.bottom - t * (self.bottom - self.top)
    }

    /// The y where a zone changes at `bound`: the bound's y to scale, and
    /// the area edge off scale.
    fn zone_y(&self, place: BoundPlace, bound: f64) -> f32 {
        match place {
            BoundPlace::ToScale => self.y(bound),
            BoundPlace::OffLeft => self.bottom,
            BoundPlace::OffRight => self.top,
        }
    }
}

/// The "over time" face of a cut-metric card.
pub struct Sparkline<'a> {
    series: &'a [(usize, f64)],
    histogram: &'a Histogram,
    unit: &'a str,
    scale: f64,
    advisory: bool,
    playhead: Option<usize>,
}

/// What the operator did to the line this frame.
pub struct SparklineResponse {
    pub response: egui::Response,
    /// The toolpath-local move of the first sample in the clicked column.
    pub clicked_move: Option<usize>,
}

impl<'a> Sparkline<'a> {
    /// A line of `series`, `(toolpath-local move, value in core's unit)` in
    /// trace order. `histogram` is the card's histogram of the same
    /// population: the line reads its bounds and its display range.
    #[must_use]
    pub fn new(series: &'a [(usize, f64)], histogram: &'a Histogram, unit: &'a str) -> Self {
        Self {
            series,
            histogram,
            unit,
            scale: 1.0,
            advisory: false,
            playhead: None,
        }
    }

    /// Multiply every label by `scale` for display. The core values stay
    /// as they are. See [`histogram::DistributionChart::scale`].
    #[must_use]
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    /// Draw the limit lines with a longer dash gap: the bound is a rule of
    /// thumb that does not gate an export.
    #[must_use]
    pub fn advisory(mut self, advisory: bool) -> Self {
        self.advisory = advisory;
        self
    }

    /// Mark the playhead at this toolpath-local move.
    #[must_use]
    pub fn playhead(mut self, local_move: Option<usize>) -> Self {
        self.playhead = local_move;
        self
    }

    /// Draw the line in the full available width, at the histogram's
    /// height.
    pub fn show(self, ui: &mut egui::Ui) -> SparklineResponse {
        let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
        let width = ui.available_width().max(histogram::STUB_WIDTH * 4.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, histogram::chart_height(ui)),
            egui::Sense::click(),
        );
        let hist = self.histogram;
        let (lo, hi) = histogram::display_range(hist);
        let floor = hist.floor.filter(|v| v.is_finite());
        let ceiling = hist.ceiling.filter(|v| v.is_finite());

        // The gutter labels: each to-scale bound first, then the two range
        // ends. A label that collides with one already placed is left out.
        let mut labels: Vec<(f64, egui::Color32, LabelAt)> = Vec::new();
        for (bound, colour) in [(floor, tokens::CAUTION), (ceiling, tokens::DANGER)] {
            if let Some(value) = bound
                && histogram::bound_place(hist, value) == BoundPlace::ToScale
            {
                labels.push((value, colour, LabelAt::Value));
            }
        }
        labels.push((hi, tokens::TEXT_MUTED, LabelAt::Top));
        labels.push((lo, tokens::TEXT_MUTED, LabelAt::Bottom));
        let painter = ui.painter_at(rect);
        let galleys: Vec<(f64, std::sync::Arc<egui::Galley>, egui::Color32, LabelAt)> = labels
            .into_iter()
            .map(|(value, colour, at)| {
                let text = histogram::format_value(value * self.scale);
                let galley = painter.layout_no_wrap(text, font.clone(), colour);
                (value, galley, colour, at)
            })
            .collect();
        let gutter = galleys
            .iter()
            .map(|(_, galley, _, _)| galley.size().x)
            .fold(0.0_f32, f32::max)
            + tokens::SPACE_2;
        let area = egui::Rect::from_min_max(
            egui::pos2(rect.left() + gutter, rect.top() + EDGE_PAD),
            egui::pos2(rect.right(), rect.bottom() - EDGE_PAD),
        );
        let y_scale = YScale {
            top: area.top(),
            bottom: area.bottom(),
            lo,
            hi,
        };
        let max_columns = area.width().max(1.0) as usize;
        let columns = columns(self.series, max_columns);
        let column_count = columns.as_ref().map_or(1, |c| c.columns.len().max(1));
        let column_x = |index: usize| -> f32 {
            area.left() + (index as f32 + 0.5) * area.width() / column_count as f32
        };
        let column_at = |x: f32| -> Option<usize> {
            if x < area.left() || x > area.right() || area.width() <= 0.0 {
                return None;
            }
            let t = (x - area.left()) / area.width();
            Some(((t * column_count as f32) as usize).min(column_count - 1))
        };

        let column_data = |index: usize| -> Option<Column> {
            columns.as_ref()?.columns.get(index).copied().flatten()
        };
        let hovered_index = response.hover_pos().and_then(|pos| column_at(pos.x));
        let hovered_column = hovered_index.and_then(column_data);
        let clicked_move = if response.clicked() {
            response
                .interact_pointer_pos()
                .and_then(|pos| column_at(pos.x))
                .and_then(column_data)
                .map(|column| column.first_move)
        } else {
            None
        };

        if ui.is_rect_visible(rect) {
            self.paint_zones(&painter, area, &y_scale, floor, ceiling);
            self.paint_limits(&painter, area, &y_scale, floor, ceiling, &font);
            let mut placed: Vec<egui::Rect> = Vec::new();
            for (value, galley, colour, at) in galleys {
                let size = galley.size();
                let y = match at {
                    LabelAt::Top => rect.top(),
                    LabelAt::Bottom => rect.bottom() - size.y,
                    LabelAt::Value => y_scale.y(value) - 0.5 * size.y,
                };
                let label = egui::Rect::from_min_size(egui::pos2(rect.left(), y), size);
                let padded = label.expand2(egui::vec2(0.0, tokens::SPACE_1));
                if placed.iter().any(|other| other.intersects(padded)) {
                    continue;
                }
                placed.push(label);
                painter.galley(label.min, galley, colour);
            }
            if let Some(columns) = &columns {
                if let Some(local_move) = self.playhead
                    && let Some(index) = columns.column_of(local_move)
                {
                    let x = column_x(index);
                    painter.line_segment(
                        [egui::pos2(x, area.top()), egui::pos2(x, area.bottom())],
                        egui::Stroke::new(1.0, tokens::TEXT_MUTED),
                    );
                }
                paint_line(&painter, columns, &y_scale, floor, ceiling, &column_x);
                if let Some(index) = hovered_index
                    && hovered_column.is_some()
                {
                    let x = column_x(index);
                    painter.line_segment(
                        [egui::pos2(x, area.top()), egui::pos2(x, area.bottom())],
                        egui::Stroke::new(1.0, tokens::HAIRLINE),
                    );
                }
            }
        }

        let hover_text = hovered_index.map(|_| match hovered_column {
            Some(column) => column_hover_text(&column, self.scale, self.unit),
            None => "Not measured here.".to_owned(),
        });
        let response = match hover_text {
            Some(text) => response.on_hover_text_at_pointer(text),
            None => response,
        };
        if hovered_column.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        SparklineResponse {
            response,
            clicked_move,
        }
    }

    /// Paint the band zones behind the line, as the histogram does. A line
    /// with no bound paints no zone.
    fn paint_zones(
        &self,
        painter: &egui::Painter,
        area: egui::Rect,
        y_scale: &YScale,
        floor: Option<f64>,
        ceiling: Option<f64>,
    ) {
        if floor.is_none() && ceiling.is_none() {
            return;
        }
        let hist = self.histogram;
        let floor_y = floor.map_or(area.bottom(), |v| {
            y_scale.zone_y(histogram::bound_place(hist, v), v)
        });
        let ceiling_y = ceiling.map_or(area.top(), |v| {
            y_scale.zone_y(histogram::bound_place(hist, v), v)
        });
        // `top` is above `bottom` on screen: a smaller y.
        let zone = |top: f32, bottom: f32, colour: egui::Color32| {
            if bottom > top {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(area.left(), top),
                        egui::pos2(area.right(), bottom),
                    ),
                    0.0,
                    colour,
                );
            }
        };
        if floor.is_some() {
            zone(
                floor_y,
                area.bottom(),
                tokens::CAUTION.gamma_multiply(histogram::WASH_OUT),
            );
        }
        zone(
            ceiling_y,
            floor_y,
            tokens::OK.gamma_multiply(histogram::WASH_OK),
        );
        if ceiling.is_some() {
            zone(
                area.top(),
                ceiling_y.min(floor_y),
                tokens::DANGER.gamma_multiply(histogram::WASH_OUT),
            );
        }
    }

    /// Paint a dashed limit line at each to-scale bound, and an off-scale
    /// marker at the edge for each bound outside the range.
    fn paint_limits(
        &self,
        painter: &egui::Painter,
        area: egui::Rect,
        y_scale: &YScale,
        floor: Option<f64>,
        ceiling: Option<f64>,
        font: &egui::FontId,
    ) {
        let (dash, gap) = if self.advisory {
            (2.0, 4.0)
        } else {
            (3.0, 2.0)
        };
        for (bound, colour) in [(floor, tokens::CAUTION), (ceiling, tokens::DANGER)] {
            let Some(value) = bound else {
                continue;
            };
            let tone = colour.gamma_multiply(histogram::MARKER_TONE);
            match histogram::bound_place(self.histogram, value) {
                BoundPlace::ToScale => {
                    let y = y_scale.y(value);
                    painter.extend(egui::Shape::dashed_line(
                        &[egui::pos2(area.left(), y), egui::pos2(area.right(), y)],
                        egui::Stroke::new(1.0, tone),
                        dash,
                        gap,
                    ));
                }
                place => {
                    let text = format!(
                        "{} {}",
                        histogram::format_value(value * self.scale),
                        self.unit
                    );
                    let galley = painter.layout_no_wrap(text, font.clone(), colour);
                    let size = galley.size();
                    let half = histogram::CAP_HALF_WIDTH;
                    let tip_x = area.right() - half;
                    // A triangle that points out of the area, at the edge
                    // the bound lies beyond, with the value beside it.
                    let (points, text_y) = if place == BoundPlace::OffRight {
                        let base = area.top() + half * 1.5;
                        (
                            vec![
                                egui::pos2(tip_x, area.top()),
                                egui::pos2(tip_x + half, base),
                                egui::pos2(tip_x - half, base),
                            ],
                            area.top(),
                        )
                    } else {
                        let base = area.bottom() - half * 1.5;
                        (
                            vec![
                                egui::pos2(tip_x - half, base),
                                egui::pos2(tip_x + half, base),
                                egui::pos2(tip_x, area.bottom()),
                            ],
                            area.bottom() - size.y,
                        )
                    };
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        colour,
                        egui::Stroke::NONE,
                    ));
                    let text_x = tip_x - half - tokens::SPACE_1 - size.x;
                    painter.galley(egui::pos2(text_x, text_y), galley, colour);
                }
            }
        }
    }
}

/// Paint the line: the joins between columns, the min-to-max piece of
/// each column, and a triangle at the edge for each clamped value.
fn paint_line(
    painter: &egui::Painter,
    columns: &Columns,
    y_scale: &YScale,
    floor: Option<f64>,
    ceiling: Option<f64>,
    column_x: &dyn Fn(usize) -> f32,
) {
    let piece = |x0: f32, v0: f64, x1: f32, v1: f64| {
        for (t0, t1, side) in split_at_bounds(v0, v1, floor, ceiling) {
            let at = |t: f64| {
                let x = x0 + (x1 - x0) * t as f32;
                egui::pos2(x, y_scale.y(v0 + (v1 - v0) * t))
            };
            painter.line_segment(
                [at(t0), at(t1)],
                egui::Stroke::new(LINE_WIDTH, histogram::side_colour(side)),
            );
        }
    };
    let mut previous: Option<Column> = None;
    for (index, slot) in columns.columns.iter().enumerate() {
        let Some(column) = slot else {
            previous = None;
            continue;
        };
        let x = column_x(index);
        if column.joined
            && let Some(before) = previous
        {
            piece(
                column_x(index.saturating_sub(1)),
                before.last,
                x,
                column.first,
            );
        }
        if column.max > column.min {
            piece(x, column.min, x, column.max);
        } else {
            // One value: a short flat piece, so a single sample with no
            // neighbour stays visible.
            piece(x - LINE_WIDTH, column.min, x + LINE_WIDTH, column.max);
        }
        let half = histogram::CAP_HALF_WIDTH;
        if column.max > y_scale.hi {
            let colour = histogram::side_colour(histogram::value_side(floor, ceiling, column.max));
            let top = y_scale.top - EDGE_PAD;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x, top),
                    egui::pos2(x + half, top + half),
                    egui::pos2(x - half, top + half),
                ],
                colour,
                egui::Stroke::NONE,
            ));
        }
        if column.min < y_scale.lo {
            let colour = histogram::side_colour(histogram::value_side(floor, ceiling, column.min));
            let bottom = y_scale.bottom + EDGE_PAD;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x - half, bottom - half),
                    egui::pos2(x + half, bottom - half),
                    egui::pos2(x, bottom),
                ],
                colour,
                egui::Stroke::NONE,
            ));
        }
        previous = Some(*column);
    }
}

/// The hover line of one column: `0.031 mm/tooth`, or
/// `0.028–0.036 mm/tooth` when the column holds a range.
#[must_use]
pub fn column_hover_text(column: &Column, scale: f64, unit: &str) -> String {
    let value = if column.max > column.min {
        format!(
            "{}\u{2013}{} {unit}",
            histogram::format_value(column.min * scale),
            histogram::format_value(column.max * scale),
        )
    } else {
        format!("{} {unit}", histogram::format_value(column.min * scale))
    };
    format!("{value}\nClick to go to this point.")
}

/// The two faces of a cut-metric card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartFace {
    /// The histogram of the run.
    Distribution,
    /// The line of the run over time.
    OverTime,
}

impl ChartFace {
    /// The other face.
    #[must_use]
    pub fn flipped(self) -> Self {
        match self {
            Self::Distribution => Self::OverTime,
            Self::OverTime => Self::Distribution,
        }
    }

    /// The hover text of the flip button on this face: what a click shows.
    #[must_use]
    pub fn flip_hover(self) -> &'static str {
        match self {
            Self::Distribution => "Show over time",
            Self::OverTime => "Show distribution",
        }
    }
}

/// The flip button of a cut-metric card: a small painted icon of the face
/// a click shows. A line icon on the histogram face, a bar icon on the
/// line face. It paints no glyph from a font, so it does not depend on
/// font cover.
pub struct ChartFlip {
    face: ChartFace,
}

impl ChartFlip {
    /// The flip button of a card that shows `face`.
    #[must_use]
    pub fn new(face: ChartFace) -> Self {
        Self { face }
    }
}

impl egui::Widget for ChartFlip {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(FLIP_SIZE, FLIP_SIZE), egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let colour = if response.hovered() {
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::from(tokens::RADIUS_SM),
                    tokens::SURFACE_RAISED,
                );
                tokens::TEXT_STRONG
            } else {
                tokens::TEXT_MUTED
            };
            let icon = rect.shrink(tokens::SPACE_1 + 1.0);
            let at = |x: f32, y: f32| {
                egui::pos2(
                    icon.left() + x * icon.width(),
                    icon.top() + y * icon.height(),
                )
            };
            match self.face {
                ChartFace::Distribution => {
                    let stroke = egui::Stroke::new(LINE_WIDTH, colour);
                    let points = [
                        at(0.0, 0.75),
                        at(0.3, 0.3),
                        at(0.55, 0.8),
                        at(0.8, 0.15),
                        at(1.0, 0.4),
                    ];
                    for pair in points.windows(2) {
                        if let (Some(&a), Some(&b)) = (pair.first(), pair.get(1)) {
                            painter.line_segment([a, b], stroke);
                        }
                    }
                }
                ChartFace::OverTime => {
                    for (left, height) in [(0.0, 0.5), (0.36, 1.0), (0.72, 0.7)] {
                        painter.rect_filled(
                            egui::Rect::from_min_max(at(left, 1.0 - height), at(left + 0.28, 1.0)),
                            0.0,
                            colour,
                        );
                    }
                }
            }
        }
        response.on_hover_text(self.face.flip_hover())
    }
}
