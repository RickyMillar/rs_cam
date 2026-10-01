//! `Sparkline`, `ChartFlip` and `TraceOpen` — the "over time" face of a
//! cut-metric card, and the button that opens the trace large.
//!
//! Operator request 2026-10-02: each cut-metric histogram has a flip button
//! beside it. The button swaps the histogram for a small line of the same
//! metric over the run of the toolpath. The bottom time-series drawer is
//! deleted. A second button opens the one trace in a modal
//! (`ui/sim_trace_modal.rs`), which draws the same component large, with
//! zoom and pan. The painter draws the line directly, for the reason
//! `histogram.rs` gives: a plot keeps its bounds per id from frame to frame.
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
//! - The line joins consecutive samples at their true x, across pixel
//!   columns with no sample. The gate population is sparse in move order
//!   (it leaves out acceleration ramps and transit samples), and the tool
//!   cut through those moves. Round 3 broke the line at every
//!   empty column; zoomed in, that drew isolated dots (2026-10-02).
//! - A gap breaks the line only at a non-finite sample. The card series
//!   holds one wherever the tool is in air: an air sample, or air between
//!   two samples (`state::simulation::series_with_breaks`, round 5). The
//!   line is drawn across cutting samples only; it never drops to zero in
//!   air. The hover over an air column says so.
//!
//! # The y scale
//!
//! Operator ruling 2026-10-02: "show the whole thing plus a little, so you
//! can see the limit bars too". [`line_range`] gives the rule:
//!
//! 1. Start from the minimum and the maximum of every finite sample of the
//!    series. There is no percentile cut and no clamp: every sample is in
//!    the range.
//! 2. Extend the range to include each bound the gate has, floor and
//!    ceiling, so every limit line is on the chart.
//! 3. Add [`LINE_MARGIN`] of the span above and below. This is a display
//!    margin only, so the line and the limit lines do not touch the edge.
//!
//! The range covers the whole series, not the shown window, so the y axis
//! stays still while the modal zooms and pans. A bound far from the data
//! compresses the line; the operator accepted that to keep the limits in
//! view. The histogram keeps its own x-axis rule (`histogram.rs`).
//!
//! # Decimation: a median line over a faint min-to-max band
//!
//! [`columns`] puts the samples into pixel columns. Each column keeps the
//! MEDIAN of its finite samples, and its minimum and maximum.
//!
//! - The line joins the column medians. It is the trend. Round 1 drew the
//!   full min-to-max range of each column as the line; on a toolpath of
//!   40 000 short moves a column held about 140 moves, and the line became
//!   a solid zigzag band with no visible trend (operator, 2026-10-02). The
//!   data does vary from move to move (acceleration ramps, feed
//!   modulation), so the band is true; it is not the trend.
//! - The median, not a mean: one spike in a column does not move it. The
//!   card series holds `(move, value)` only, with no time weight, so a
//!   time-weighted mean is not available here. An even count takes the
//!   mean of the two middle values.
//! - The min-to-max range is a faint band behind the line, in the band
//!   colours, split at each bound, so a spike above the ceiling inside a
//!   column with an in-band median still shows red. A column of
//!   [`BAND_MIN_SAMPLES`] samples or fewer draws no band: zoomed in that
//!   far, the line is the per-move signal.
//! - A max-only decimation would hide every dip below the chipload floor;
//!   the band keeps both ends.

use rs_cam_core::tool_load::Histogram;

use crate::ui::components::histogram::{self, BinSide};
use crate::ui::tokens;

/// The width of an icon stroke, in points.
pub const LINE_WIDTH: f32 = 1.5;

/// The width of the median line, in points.
pub const MEDIAN_WIDTH: f32 = 1.75;

/// The alpha factor of the min-to-max band behind the line.
pub const BAND_ALPHA: f32 = 0.3;

/// A column of this many samples or fewer draws no band.
pub const BAND_MIN_SAMPLES: usize = 2;

/// When the line has at least this much width per drawn column, in points,
/// each column median also gets a dot, so the true sample positions show.
pub const MARKER_SPACING: f32 = tokens::SPACE_2;

/// The side of the square icon buttons (flip and open), in points.
pub const FLIP_SIZE: f32 = tokens::SPACE_5;

/// The display margin above and below the y range, as a share of the span.
/// It is not a bound and not data: it keeps the line off the edge.
pub const LINE_MARGIN: f64 = 0.05;

/// The fewest moves a zoomed window shows.
pub const MIN_WINDOW_MOVES: f64 = 8.0;

/// The space above and below the data area, in points.
const EDGE_PAD: f32 = tokens::SPACE_1;

/// One pixel column of the line, in core's unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub min: f64,
    pub max: f64,
    /// The median of the column's finite samples: the line's value.
    pub median: f64,
    /// The number of finite samples in the column.
    pub count: usize,
    /// The toolpath-local move of the first finite sample.
    pub first_move: usize,
    /// True when the line joins the previous drawn column to this one: an
    /// earlier column holds the previous finite sample, and no non-finite
    /// sample lies between them. Empty columns between them do not break
    /// the join.
    pub joined: bool,
}

/// The columns of one series, and the move range they cover.
#[derive(Clone, Debug, PartialEq)]
pub struct Columns {
    /// One entry per pixel column. `None` is a gap: no finite sample.
    pub columns: Vec<Option<Column>>,
    /// One flag per pixel column: the column holds a non-finite sample,
    /// which the card series uses for "the tool is in air".
    pub air: Vec<bool>,
    pub first_move: usize,
    pub last_move: usize,
}

impl Columns {
    /// The number of moves the columns cover.
    #[must_use]
    pub fn move_span(&self) -> usize {
        self.last_move - self.first_move + 1
    }

    /// The first move of column `index`.
    #[must_use]
    pub fn move_of(&self, index: usize) -> usize {
        let count = self.columns.len().max(1);
        self.first_move + index * self.move_span() / count
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

/// The first and the last move of `series`, or `None` when it is empty.
#[must_use]
pub fn move_extent(series: &[(usize, f64)]) -> Option<(usize, usize)> {
    let first = series.iter().map(|(m, _)| *m).min()?;
    let last = series.iter().map(|(m, _)| *m).max()?;
    Some((first, last))
}

/// Put `series` into at most `max_columns` pixel columns.
///
/// `series` is `(toolpath-local move, value)` in trace order. The column
/// count is never more than the number of moves in the range, so a short
/// toolpath does not leave an empty column between two moves. A value that
/// is not finite is a gap: it is not drawn, and the line does not join
/// across it. An empty column does not break the line. Returns `None` when
/// no value is finite.
#[must_use]
pub fn columns(series: &[(usize, f64)], max_columns: usize) -> Option<Columns> {
    let window = move_extent(series)?;
    columns_in(series, max_columns, window)
}

/// [`columns`] over the inclusive move window `window` only. A sample
/// outside the window is not drawn.
#[must_use]
pub fn columns_in(
    series: &[(usize, f64)],
    max_columns: usize,
    window: (usize, usize),
) -> Option<Columns> {
    if !series.iter().any(|(_, value)| value.is_finite()) {
        return None;
    }
    let (first_move, last_move) = (window.0.min(window.1), window.0.max(window.1));
    let span = last_move - first_move + 1;
    let count = max_columns.max(1).min(span);
    let mut out = Columns {
        columns: vec![None; count],
        air: vec![false; count],
        first_move,
        last_move,
    };
    let mut values: Vec<Vec<f64>> = vec![Vec::new(); count];
    let mut after_gap = true;
    let mut last_column: Option<usize> = None;
    for &(local_move, value) in series {
        if !value.is_finite() {
            after_gap = true;
            if let Some(flag) = out
                .column_of(local_move)
                .and_then(|index| out.air.get_mut(index))
            {
                *flag = true;
            }
            continue;
        }
        let Some(index) = out.column_of(local_move) else {
            continue;
        };
        // Join to the previous finite sample in ANY earlier column. An empty
        // column between them is a move with no population sample (an
        // acceleration ramp, a transit sample), not a gap: the tool cut
        // through it. Only a non-finite sample (air) breaks the line.
        let joined = !after_gap && last_column.is_some_and(|last| last < index);
        if let Some(slot) = out.columns.get_mut(index) {
            match slot {
                Some(column) => {
                    column.min = column.min.min(value);
                    column.max = column.max.max(value);
                }
                None => {
                    *slot = Some(Column {
                        min: value,
                        max: value,
                        median: value,
                        count: 0,
                        first_move: local_move,
                        joined,
                    });
                }
            }
        }
        if let Some(bucket) = values.get_mut(index) {
            bucket.push(value);
        }
        after_gap = false;
        last_column = Some(index);
    }
    for (slot, mut bucket) in out.columns.iter_mut().zip(values) {
        if let Some(column) = slot {
            column.count = bucket.len();
            if let Some(median) = median(&mut bucket) {
                column.median = median;
            }
        }
    }
    Some(out)
}

/// The median of `values`, which must be finite. An even count takes the
/// mean of the two middle values. The order of `values` changes. Returns
/// `None` when `values` is empty.
#[must_use]
pub fn median(values: &mut [f64]) -> Option<f64> {
    let n = values.len();
    if n == 0 {
        return None;
    }
    let mid = n / 2;
    let (below, upper, _) = values.select_nth_unstable_by(mid, f64::total_cmp);
    let upper = *upper;
    if n % 2 == 1 {
        return Some(upper);
    }
    let lower = below.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(0.5 * (lower + upper))
}

/// The y range of a line, in core's unit. See "The y scale" above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineRange {
    /// The smallest finite sample.
    pub data_min: f64,
    /// The largest finite sample.
    pub data_max: f64,
    /// The bottom of the chart: the data and the bounds, less the margin.
    pub lo: f64,
    /// The top of the chart: the data and the bounds, plus the margin.
    pub hi: f64,
}

/// The y range of `series` with the bounds `floor` and `ceiling`: the full
/// data range, extended to each finite bound, plus [`LINE_MARGIN`] of the
/// span on each side. Returns `None` when no sample is finite.
#[must_use]
pub fn line_range(
    series: &[(usize, f64)],
    floor: Option<f64>,
    ceiling: Option<f64>,
) -> Option<LineRange> {
    let finite = || series.iter().map(|(_, v)| *v).filter(|v| v.is_finite());
    let data_min = finite().fold(f64::INFINITY, f64::min);
    let data_max = finite().fold(f64::NEG_INFINITY, f64::max);
    if !data_min.is_finite() || !data_max.is_finite() {
        return None;
    }
    let (mut lo, mut hi) = (data_min, data_max);
    for bound in [floor, ceiling].into_iter().flatten() {
        if bound.is_finite() {
            lo = lo.min(bound);
            hi = hi.max(bound);
        }
    }
    if hi <= lo {
        // Every value is equal and no bound is apart from it. Give the
        // range a width, as core's histogram does. This is display only.
        let pad = if lo.abs() > 0.0 {
            lo.abs() * LINE_MARGIN
        } else {
            1e-6
        };
        lo -= pad;
        hi += pad;
    }
    let margin = (hi - lo) * LINE_MARGIN;
    Some(LineRange {
        data_min,
        data_max,
        lo: lo - margin,
        hi: hi + margin,
    })
}

/// Zoom `window` by `factor` about the move `anchor`. A factor below 1
/// zooms in. The result keeps at least [`MIN_WINDOW_MOVES`] and stays
/// inside `extent`.
#[must_use]
pub fn zoom_window(window: (f64, f64), extent: (f64, f64), anchor: f64, factor: f64) -> (f64, f64) {
    let full = (extent.1 - extent.0).max(0.0);
    let span = (window.1 - window.0).max(f64::EPSILON);
    let new_span = (span * factor).clamp(MIN_WINDOW_MOVES.min(full), full);
    let anchor = anchor.clamp(window.0, window.1.max(window.0));
    let share = (anchor - window.0) / span;
    let lo = anchor - share * new_span;
    clamp_window((lo, lo + new_span), extent)
}

/// Move `window` by `delta` moves, inside `extent`.
#[must_use]
pub fn pan_window(window: (f64, f64), extent: (f64, f64), delta: f64) -> (f64, f64) {
    clamp_window((window.0 + delta, window.1 + delta), extent)
}

/// Shift `window` into `extent` and keep its span.
fn clamp_window(window: (f64, f64), extent: (f64, f64)) -> (f64, f64) {
    let span = (window.1 - window.0).min(extent.1 - extent.0).max(0.0);
    let lo = window.0.clamp(extent.0, (extent.1 - span).max(extent.0));
    (lo, lo + span)
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

/// The worst side of the bounds a column's band reaches: above when its
/// maximum is over the ceiling, else below when its minimum is under the
/// floor. The line takes the side of the median instead.
#[must_use]
pub fn column_side(column: &Column, floor: Option<f64>, ceiling: Option<f64>) -> BinSide {
    if histogram::value_side(floor, ceiling, column.max) == BinSide::Above {
        BinSide::Above
    } else if histogram::value_side(floor, ceiling, column.min) == BinSide::Below {
        BinSide::Below
    } else {
        BinSide::InBand
    }
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
    /// The y of `value` in the data area.
    fn y(&self, value: f64) -> f32 {
        let span = self.hi - self.lo;
        if span <= 0.0 || !value.is_finite() {
            return 0.5 * (self.top + self.bottom);
        }
        let t = ((value - self.lo) / span).clamp(0.0, 1.0) as f32;
        self.bottom - t * (self.bottom - self.top)
    }
}

/// The "over time" face of a cut-metric card, and the large trace of the
/// trace modal.
pub struct Sparkline<'a> {
    series: &'a [(usize, f64)],
    histogram: &'a Histogram,
    unit: &'a str,
    scale: f64,
    advisory: bool,
    playhead: Option<usize>,
    height: Option<f32>,
    window: Option<(usize, usize)>,
    move_offset: usize,
    detail: bool,
}

/// What the operator did to the line this frame.
pub struct SparklineResponse {
    pub response: egui::Response,
    /// The toolpath-local move of the first sample in the clicked column.
    pub clicked_move: Option<usize>,
    /// The toolpath-local move under the pointer, as a fraction, when the
    /// pointer is over the data area. The modal zooms about it.
    pub pointer_move: Option<f64>,
    /// How many moves one point of width covers. The modal pans with it.
    pub moves_per_point: f64,
}

impl<'a> Sparkline<'a> {
    /// A line of `series`, `(toolpath-local move, value in core's unit)` in
    /// trace order. `histogram` is the card's histogram of the same
    /// population: the line reads its bounds.
    #[must_use]
    pub fn new(series: &'a [(usize, f64)], histogram: &'a Histogram, unit: &'a str) -> Self {
        Self {
            series,
            histogram,
            unit,
            scale: 1.0,
            advisory: false,
            playhead: None,
            height: None,
            window: None,
            move_offset: 0,
            detail: false,
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

    /// The height of the line, in points. The default is the histogram's
    /// height, so a card does not change height when it flips.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Show only the inclusive move window `window`. The default is the
    /// whole series.
    #[must_use]
    pub fn window(mut self, window: Option<(usize, usize)>) -> Self {
        self.window = window;
        self
    }

    /// Add `offset` to every move number the line shows, so it reads the
    /// global move of the transport bar. The default is 0.
    #[must_use]
    pub fn move_offset(mut self, offset: usize) -> Self {
        self.move_offset = offset;
        self
    }

    /// The modal face: named limit labels, a move label row under the
    /// line, and drag senses for pan.
    #[must_use]
    pub fn detail(mut self, detail: bool) -> Self {
        self.detail = detail;
        self
    }

    /// Draw the line in the full available width.
    pub fn show(self, ui: &mut egui::Ui) -> SparklineResponse {
        let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
        let label_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        let width = ui.available_width().max(histogram::STUB_WIDTH * 4.0);
        let height = self.height.unwrap_or_else(|| histogram::chart_height(ui));
        let sense = if self.detail {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
        let hist = self.histogram;
        let floor = hist.floor.filter(|v| v.is_finite());
        let ceiling = hist.ceiling.filter(|v| v.is_finite());
        let range = line_range(self.series, floor, ceiling);

        // The gutter labels: each bound first, then the data maximum and
        // minimum, each at its own y. A label that collides with one
        // already placed is left out.
        let mut labels: Vec<(f64, egui::Color32, &str)> = Vec::new();
        for (bound, colour, name) in [
            (ceiling, tokens::DANGER, "ceiling"),
            (floor, tokens::CAUTION, "floor"),
        ] {
            if let Some(value) = bound {
                labels.push((value, colour, name));
            }
        }
        if let Some(range) = range {
            labels.push((range.data_max, tokens::TEXT_MUTED, "max"));
            labels.push((range.data_min, tokens::TEXT_MUTED, "min"));
        }
        let painter = ui.painter_at(rect);
        let galleys: Vec<(f64, std::sync::Arc<egui::Galley>, egui::Color32)> = labels
            .into_iter()
            .map(|(value, colour, name)| {
                let number = histogram::format_value(value * self.scale);
                let text = if self.detail {
                    format!("{name} {number}")
                } else {
                    number
                };
                (
                    value,
                    painter.layout_no_wrap(text, font.clone(), colour),
                    colour,
                )
            })
            .collect();
        let gutter = galleys
            .iter()
            .map(|(_, galley, _)| galley.size().x)
            .fold(0.0_f32, f32::max)
            + tokens::SPACE_2;
        let foot = if self.detail {
            label_height + tokens::SPACE_1
        } else {
            0.0
        };
        let area = egui::Rect::from_min_max(
            egui::pos2(rect.left() + gutter, rect.top() + EDGE_PAD),
            egui::pos2(rect.right(), rect.bottom() - EDGE_PAD - foot),
        );
        let y_scale = YScale {
            top: area.top(),
            bottom: area.bottom(),
            lo: range.map_or(0.0, |r| r.lo),
            hi: range.map_or(1.0, |r| r.hi),
        };
        let max_columns = area.width().max(1.0) as usize;
        let columns = match self.window.or_else(|| move_extent(self.series)) {
            Some(window) => columns_in(self.series, max_columns, window),
            None => None,
        };
        let column_count = columns.as_ref().map_or(1, |c| c.columns.len().max(1));
        let (first_move, move_span) = columns
            .as_ref()
            .map_or((0, 1), |c| (c.first_move, c.move_span()));
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
        let moves_per_point = move_span as f64 / f64::from(area.width().max(1.0));
        let pointer_move = response
            .hover_pos()
            .filter(|pos| pos.x >= area.left() && pos.x <= area.right())
            .map(|pos| first_move as f64 + f64::from(pos.x - area.left()) * moves_per_point);

        if ui.is_rect_visible(rect) {
            paint_zones(&painter, area, &y_scale, floor, ceiling);
            let (dash, gap) = if self.advisory {
                (2.0, 4.0)
            } else {
                (3.0, 2.0)
            };
            for (bound, colour) in [(floor, tokens::CAUTION), (ceiling, tokens::DANGER)] {
                if let Some(value) = bound {
                    let y = y_scale.y(value);
                    painter.extend(egui::Shape::dashed_line(
                        &[egui::pos2(area.left(), y), egui::pos2(area.right(), y)],
                        egui::Stroke::new(1.0, colour.gamma_multiply(histogram::MARKER_TONE)),
                        dash,
                        gap,
                    ));
                }
            }
            let mut placed: Vec<egui::Rect> = Vec::new();
            for (value, galley, colour) in galleys {
                let size = galley.size();
                let top = (y_scale.y(value) - 0.5 * size.y)
                    .clamp(rect.top(), (area.bottom() - size.y).max(rect.top()));
                let label = egui::Rect::from_min_size(egui::pos2(rect.left(), top), size);
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
                let drawn = columns.columns.iter().flatten().count().max(1);
                let geometry = LineGeometry {
                    column_width: area.width() / column_count as f32,
                    markers: area.width() / drawn as f32 >= MARKER_SPACING,
                };
                paint_line(
                    &painter, columns, &y_scale, floor, ceiling, &column_x, geometry,
                );
                if let Some(index) = hovered_index
                    && hovered_column.is_some()
                {
                    let x = column_x(index);
                    painter.line_segment(
                        [egui::pos2(x, area.top()), egui::pos2(x, area.bottom())],
                        egui::Stroke::new(1.0, tokens::HAIRLINE),
                    );
                }
                if self.detail {
                    let label_y = area.bottom() + EDGE_PAD + tokens::SPACE_1;
                    let ends = [
                        (area.left(), egui::Align2::LEFT_TOP, columns.first_move),
                        (area.right(), egui::Align2::RIGHT_TOP, columns.last_move),
                    ];
                    for (x, align, local_move) in ends {
                        painter.text(
                            egui::pos2(x, label_y),
                            align,
                            format!("move {}", local_move + self.move_offset),
                            font.clone(),
                            tokens::TEXT_MUTED,
                        );
                    }
                }
            }
        }

        let hover_text = hovered_index.map(|_| match hovered_column {
            Some(column) => column_hover_text(
                &column,
                column.first_move + self.move_offset,
                floor,
                ceiling,
                self.scale,
                self.unit,
            ),
            None => {
                let air = hovered_index.and_then(|index| {
                    let columns = columns.as_ref()?;
                    columns
                        .air
                        .get(index)
                        .copied()
                        .filter(|air| *air)
                        .map(|_| columns.move_of(index))
                });
                match air {
                    Some(local_move) => {
                        format!("In air (no cut) at move {}", local_move + self.move_offset)
                    }
                    None => "No gate sample at this move.".to_owned(),
                }
            }
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
            pointer_move,
            moves_per_point,
        }
    }
}

/// Paint the band zones behind the line, as the histogram does. A line
/// with no bound paints no zone. Every bound is inside the y range, so
/// each zone starts at its bound.
fn paint_zones(
    painter: &egui::Painter,
    area: egui::Rect,
    y_scale: &YScale,
    floor: Option<f64>,
    ceiling: Option<f64>,
) {
    if floor.is_none() && ceiling.is_none() {
        return;
    }
    let floor_y = floor.map_or(area.bottom(), |v| y_scale.y(v));
    let ceiling_y = ceiling.map_or(area.top(), |v| y_scale.y(v));
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

/// The horizontal geometry `paint_line` needs beside the columns.
#[derive(Clone, Copy, Debug)]
struct LineGeometry {
    /// The width of one pixel column, in points.
    column_width: f32,
    /// Draw a dot at each column median: the columns are far apart.
    markers: bool,
}

/// Paint the faint min-to-max band of each column, then the median line
/// that joins the columns. Both take the band colours and split at each
/// bound. A column of [`BAND_MIN_SAMPLES`] samples or fewer draws no band.
///
/// The line joins each drawn column to the previous drawn column when the
/// column is `joined`, across any empty columns between them. When there are
/// fewer samples than columns (a zoomed modal), the line is points joined at
/// their true x, with a dot at each point.
fn paint_line(
    painter: &egui::Painter,
    columns: &Columns,
    y_scale: &YScale,
    floor: Option<f64>,
    ceiling: Option<f64>,
    column_x: &dyn Fn(usize) -> f32,
    geometry: LineGeometry,
) {
    let half = (0.5 * geometry.column_width).max(0.5);
    for (index, slot) in columns.columns.iter().enumerate() {
        let Some(column) = slot else {
            continue;
        };
        if column.count <= BAND_MIN_SAMPLES || column.max <= column.min {
            continue;
        }
        let x = column_x(index);
        for (t0, t1, side) in split_at_bounds(column.min, column.max, floor, ceiling) {
            let value = |t: f64| column.min + (column.max - column.min) * t;
            let (y0, y1) = (y_scale.y(value(t0)), y_scale.y(value(t1)));
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x - half, y0.min(y1)),
                    egui::pos2(x + half, y0.max(y1).max(y0.min(y1) + 1.0)),
                ),
                0.0,
                histogram::side_colour(side).gamma_multiply(BAND_ALPHA),
            );
        }
    }

    let piece = |x0: f32, v0: f64, x1: f32, v1: f64| {
        for (t0, t1, side) in split_at_bounds(v0, v1, floor, ceiling) {
            let at = |t: f64| {
                let x = x0 + (x1 - x0) * t as f32;
                egui::pos2(x, y_scale.y(v0 + (v1 - v0) * t))
            };
            painter.line_segment(
                [at(t0), at(t1)],
                egui::Stroke::new(MEDIAN_WIDTH, histogram::side_colour(side)),
            );
        }
    };
    let drawn: Vec<(usize, Column)> = columns
        .columns
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| slot.map(|column| (index, column)))
        .collect();
    for (k, &(index, column)) in drawn.iter().enumerate() {
        let x = column_x(index);
        let before = k.checked_sub(1).and_then(|p| drawn.get(p));
        let joined_in = column.joined && before.is_some();
        let joined_out = drawn.get(k + 1).is_some_and(|(_, next)| next.joined);
        if joined_in && let Some(&(before_index, before)) = before {
            piece(column_x(before_index), before.median, x, column.median);
        }
        if !joined_in && !joined_out {
            // A point with no joined neighbour: a short flat piece, so it
            // stays visible next to a gap.
            piece(x - half, column.median, x + half, column.median);
        }
        if geometry.markers {
            let side = histogram::value_side(floor, ceiling, column.median);
            painter.circle_filled(
                egui::pos2(x, y_scale.y(column.median)),
                MEDIAN_WIDTH,
                histogram::side_colour(side),
            );
        }
    }
}

/// The words for a side of the bounds.
#[must_use]
pub fn side_words(side: BinSide) -> &'static str {
    match side {
        BinSide::Below => "below the floor",
        BinSide::InBand => "within the limits",
        BinSide::Above => "above the ceiling",
    }
}

/// The hover line of one column.
///
/// - One sample: `Move 1234 · 0.031 mm/tooth · within the limits`.
/// - More samples: `Move 1234 · median 0.031 mm/tooth (min 0.020, max
///   0.090) · within the limits`. The side is the median's. When the band
///   reaches a worse side, a second clause names it.
#[must_use]
pub fn column_hover_text(
    column: &Column,
    shown_move: usize,
    floor: Option<f64>,
    ceiling: Option<f64>,
    scale: f64,
    unit: &str,
) -> String {
    let number = |value: f64| histogram::format_value(value * scale);
    let side = histogram::value_side(floor, ceiling, column.median);
    let mut text = if column.count > 1 {
        format!(
            "Move {shown_move} \u{00b7} median {} {unit} (min {}, max {}) \u{00b7} {}",
            number(column.median),
            number(column.min),
            number(column.max),
            side_words(side),
        )
    } else {
        format!(
            "Move {shown_move} \u{00b7} {} {unit} \u{00b7} {}",
            number(column.median),
            side_words(side),
        )
    };
    let worst = column_side(column, floor, ceiling);
    if worst != side {
        text.push_str(&format!("; a sample is {}", side_words(worst)));
    }
    text.push_str("\nClick to go to this move.");
    text
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

/// The hover text of the open button.
pub const OPEN_HOVER: &str = "Open this trace";

/// The icon an icon button paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Icon {
    /// A small line: "show over time".
    Line,
    /// Three bars: "show distribution".
    Bars,
    /// `< >`: "open this trace".
    Open,
}

/// Allocate one square icon button and paint `icon` in it. The flip and
/// the open buttons share this, so they are one family. It paints no glyph
/// from a font, so it does not depend on font cover.
fn icon_button(ui: &mut egui::Ui, icon: Icon, hover: &str) -> egui::Response {
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
        let frame = rect.shrink(tokens::SPACE_1 + 1.0);
        let at = |x: f32, y: f32| {
            egui::pos2(
                frame.left() + x * frame.width(),
                frame.top() + y * frame.height(),
            )
        };
        let stroke = egui::Stroke::new(LINE_WIDTH, colour);
        let polyline = |points: &[egui::Pos2]| {
            for pair in points.windows(2) {
                if let (Some(&a), Some(&b)) = (pair.first(), pair.get(1)) {
                    painter.line_segment([a, b], stroke);
                }
            }
        };
        match icon {
            Icon::Line => polyline(&[
                at(0.0, 0.75),
                at(0.3, 0.3),
                at(0.55, 0.8),
                at(0.8, 0.15),
                at(1.0, 0.4),
            ]),
            Icon::Bars => {
                for (left, height) in [(0.0, 0.5), (0.36, 1.0), (0.72, 0.7)] {
                    painter.rect_filled(
                        egui::Rect::from_min_max(at(left, 1.0 - height), at(left + 0.28, 1.0)),
                        0.0,
                        colour,
                    );
                }
            }
            Icon::Open => {
                polyline(&[at(0.35, 0.1), at(0.0, 0.5), at(0.35, 0.9)]);
                polyline(&[at(0.65, 0.1), at(1.0, 0.5), at(0.65, 0.9)]);
            }
        }
    }
    response.on_hover_text(hover)
}

/// The flip button of a cut-metric card: a small painted icon of the face
/// a click shows. A line icon on the histogram face, a bar icon on the
/// line face.
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
        let icon = match self.face {
            ChartFace::Distribution => Icon::Line,
            ChartFace::OverTime => Icon::Bars,
        };
        icon_button(ui, icon, self.face.flip_hover())
    }
}

/// The open button of a cut-metric card: `< >`. A click opens the one
/// trace large in the trace modal (`ui/sim_trace_modal.rs`).
pub struct TraceOpen;

impl egui::Widget for TraceOpen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        icon_button(ui, Icon::Open, OPEN_HOVER)
    }
}
