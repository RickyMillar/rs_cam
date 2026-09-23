//! `DistributionChart` — a time-weighted histogram with its limit lines.
//!
//! Package C of `planning/sim_cut_metrics_2026-09-23/PLAN.md` (§3.1). The
//! chart draws one `rs_cam_core::tool_load::Histogram` in the width the
//! panel gives it. The painter draws it directly. It does not use
//! `egui_plot`, because a plot keeps its bounds per id from frame to frame,
//! and the rail needs no axes and no zoom.
//!
//! # What the chart shows
//!
//! - One bar per bin. The bar height is the bin's share of cutting time,
//!   against the tallest bin.
//! - A bar in the band takes a neutral token. A bar below the floor takes
//!   `CAUTION`. A bar above the ceiling takes `DANGER`.
//! - The band is a ZONE: a faint `OK` wash behind the bars across the
//!   in-band x range, a faint `CAUTION` wash below the floor and a faint
//!   `DANGER` wash above the ceiling. The operator ruled on 2026-09-24 that
//!   shading is correct for these cards. The feeds charts keep their own
//!   "no shading" rule.
//! - A limit marker at each bound the gate has. It is dashed, in a lower
//!   contrast than the bars, and it starts above the bar area under a
//!   small filled cap. An advisory bound has a longer gap in its dash.
//!   These properties keep a marker from reading as a bar.
//! - The x axis follows the DATA. A bound near the data range stays to
//!   scale. A bound far outside it is an off-scale marker at the chart
//!   edge: an arrow and the bound's value with its unit. The wash then
//!   runs to that edge.
//! - Labels under the bars: the floor, the ceiling and the two range ends,
//!   in the metric's unit. The bounds are placed first. A range-end label
//!   that collides moves toward the middle, and is left out when it still
//!   collides.
//!
//! The first and the last bins are overflow bins (see the core type). The
//! chart draws each one as a fixed OUTLINED stub, so a spike is never
//! hidden, never stretches the axis, and never reads as a limit.
//!
//! # What the chart does NOT do
//!
//! It does not seek the playhead. [`DistributionChart::show`] returns the
//! clicked bin, and the caller emits the seek command.

use rs_cam_core::tool_load::Histogram;

use crate::ui::tokens;

/// The height of the bar area, in points.
pub const BAR_HEIGHT: f32 = 38.0;

/// The width of each overflow stub, in points.
pub const STUB_WIDTH: f32 = tokens::SPACE_3;

/// The gap between two bars, in points.
const BAR_GAP: f32 = 1.0;

/// The height of the cap over a limit marker, in points. The cap sits in
/// its own strip above the bar area.
const CAP_HEIGHT: f32 = tokens::SPACE_2;

/// The half width of a limit cap and of an off-scale arrow, in points.
const CAP_HALF_WIDTH: f32 = 3.0;

/// A bound this far outside the data range, as a share of the range, stays
/// to scale. A bound farther out is drawn off scale at the edge.
pub const NEAR_BOUND_SHARE: f64 = 0.15;

/// The alpha factor of the in-band wash.
const WASH_OK: f32 = 0.07;

/// The alpha factor of the out-of-band washes.
const WASH_OUT: f32 = 0.12;

/// The contrast factor of a limit marker against its token.
const MARKER_TONE: f32 = 0.7;

/// A histogram chart for a narrow panel.
pub struct DistributionChart<'a> {
    histogram: &'a Histogram,
    unit: &'a str,
    scale: f64,
    advisory: bool,
}

/// What the operator did to the chart this frame.
pub struct DistributionChartResponse {
    pub response: egui::Response,
    /// The bin the operator clicked, if any.
    pub clicked_bin: Option<usize>,
}

/// Where a bin sits against the bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinSide {
    Below,
    InBand,
    Above,
}

/// Where a bound sits on the chart's x axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundPlace {
    /// Inside or near the data range: drawn to scale.
    ToScale,
    /// Far below the data range: drawn at the left edge.
    OffLeft,
    /// Far above the data range: drawn at the right edge.
    OffRight,
}

impl<'a> DistributionChart<'a> {
    /// A chart of `histogram`. `unit` is the display unit of the values.
    #[must_use]
    pub fn new(histogram: &'a Histogram, unit: &'a str) -> Self {
        Self {
            histogram,
            unit,
            scale: 1.0,
            advisory: false,
        }
    }

    /// Multiply every edge and bound by `scale` for display, for example
    /// 1000 for millimetres shown as micrometres. The core values stay as
    /// they are.
    #[must_use]
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    /// Draw the bound markers with a longer dash gap: the bound is a rule
    /// of thumb that does not gate an export.
    #[must_use]
    pub fn advisory(mut self, advisory: bool) -> Self {
        self.advisory = advisory;
        self
    }

    /// Draw the chart in the full available width.
    pub fn show(self, ui: &mut egui::Ui) -> DistributionChartResponse {
        let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
        let label_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        let width = ui.available_width().max(STUB_WIDTH * 4.0);
        let top_strip = CAP_HEIGHT + tokens::SPACE_1;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(
                width,
                top_strip + BAR_HEIGHT + tokens::SPACE_1 + label_height,
            ),
            egui::Sense::click(),
        );
        let hist = self.histogram;
        let bins = hist.weights_s.len();
        let bar_area = egui::Rect::from_min_size(
            rect.min + egui::vec2(0.0, top_strip),
            egui::vec2(width, BAR_HEIGHT),
        );
        let layout = ChartLayout::new(hist, bar_area);

        let hovered_bin = response
            .hover_pos()
            .and_then(|pos| layout.bin_at(pos.x, bins));
        let clicked_bin = if response.clicked() {
            response
                .interact_pointer_pos()
                .and_then(|pos| layout.bin_at(pos.x, bins))
        } else {
            None
        };

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            self.paint_zones(&painter, &layout, bar_area);
            painter.line_segment(
                [bar_area.left_bottom(), bar_area.right_bottom()],
                egui::Stroke::new(1.0, tokens::HAIRLINE),
            );
            paint_bars(&painter, hist, &layout, bar_area, hovered_bin);

            let label_y = bar_area.bottom() + tokens::SPACE_1;
            let mut placed: Vec<egui::Rect> = Vec::new();
            for (bound, colour) in [
                (hist.floor, tokens::CAUTION),
                (hist.ceiling, tokens::DANGER),
            ] {
                let Some(value) = bound.filter(|v| v.is_finite()) else {
                    continue;
                };
                let tone = colour.gamma_multiply(MARKER_TONE);
                match layout.place(value) {
                    BoundPlace::ToScale => {
                        let x = layout.value_x(value);
                        self.paint_marker(&painter, x, rect.top(), label_y, tone);
                        place_label(
                            &painter,
                            &mut placed,
                            rect,
                            egui::pos2(x, label_y),
                            egui::Align2::CENTER_TOP,
                            &format_value(value * self.scale),
                            &font,
                            colour,
                            Nudge::None,
                        );
                    }
                    place => {
                        let text = format!("{} {}", format_value(value * self.scale), self.unit);
                        paint_off_scale(
                            &painter,
                            &mut placed,
                            rect,
                            label_y,
                            label_height,
                            place,
                            &text,
                            &font,
                            colour,
                        );
                    }
                }
            }
            if let (Some(first), Some(last)) = (hist.edges.first(), hist.edges.last()) {
                place_label(
                    &painter,
                    &mut placed,
                    rect,
                    egui::pos2(rect.left(), label_y),
                    egui::Align2::LEFT_TOP,
                    &format_value(first * self.scale),
                    &font,
                    tokens::TEXT_MUTED,
                    Nudge::Right,
                );
                place_label(
                    &painter,
                    &mut placed,
                    rect,
                    egui::pos2(rect.right(), label_y),
                    egui::Align2::RIGHT_TOP,
                    &format!("{} {}", format_value(last * self.scale), self.unit),
                    &font,
                    tokens::TEXT_MUTED,
                    Nudge::Left,
                );
            }
        }

        let response = match hovered_bin {
            Some(bin) => {
                let text = bin_hover_text(hist, bin, self.scale, self.unit);
                response.on_hover_text_at_pointer(text)
            }
            None => response,
        };
        if hovered_bin.is_some_and(|bin| {
            hist.first_move_per_bin
                .get(bin)
                .is_some_and(Option::is_some)
        }) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        DistributionChartResponse {
            response,
            clicked_bin,
        }
    }

    /// Paint the band zones behind the bars. A chart with no bound paints
    /// no zone: there is no band to show.
    fn paint_zones(&self, painter: &egui::Painter, layout: &ChartLayout, area: egui::Rect) {
        let hist = self.histogram;
        let floor = hist.floor.filter(|v| v.is_finite());
        let ceiling = hist.ceiling.filter(|v| v.is_finite());
        if floor.is_none() && ceiling.is_none() {
            return;
        }
        let floor_x = floor.map_or(area.left(), |v| layout.zone_x(v));
        let ceiling_x = ceiling.map_or(area.right(), |v| layout.zone_x(v));
        let zone = |left: f32, right: f32, colour: egui::Color32| {
            if right > left {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(left, area.top()),
                        egui::pos2(right, area.bottom()),
                    ),
                    0.0,
                    colour,
                );
            }
        };
        // `gamma_multiply` scales all four channels, so the result stays
        // premultiplied (the UP4 note in `sim_timeline.rs`).
        if floor.is_some() {
            zone(
                area.left(),
                floor_x,
                tokens::CAUTION.gamma_multiply(WASH_OUT),
            );
        }
        zone(floor_x, ceiling_x, tokens::OK.gamma_multiply(WASH_OK));
        if ceiling.is_some() {
            zone(
                ceiling_x.max(floor_x),
                area.right(),
                tokens::DANGER.gamma_multiply(WASH_OUT),
            );
        }
    }

    /// Paint one to-scale limit marker: a cap at the top, then a dashed
    /// line from the cap down to the label row.
    fn paint_marker(
        &self,
        painter: &egui::Painter,
        x: f32,
        top: f32,
        bottom: f32,
        colour: egui::Color32,
    ) {
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x - CAP_HALF_WIDTH, top),
                egui::pos2(x + CAP_HALF_WIDTH, top),
                egui::pos2(x, top + CAP_HEIGHT),
            ],
            colour,
            egui::Stroke::NONE,
        ));
        let (dash, gap) = if self.advisory {
            (2.0, 4.0)
        } else {
            (3.0, 2.0)
        };
        painter.extend(egui::Shape::dashed_line(
            &[egui::pos2(x, top + CAP_HEIGHT), egui::pos2(x, bottom)],
            egui::Stroke::new(1.0, colour),
            dash,
            gap,
        ));
    }
}

/// Paint the bars. An inner bar is filled. An overflow stub is outlined,
/// so it does not read as a bar at a limit.
fn paint_bars(
    painter: &egui::Painter,
    hist: &Histogram,
    layout: &ChartLayout,
    bar_area: egui::Rect,
    hovered_bin: Option<usize>,
) {
    let bins = hist.weights_s.len();
    let tallest = hist.weights_s.iter().copied().fold(0.0_f64, f64::max);
    for bin in 0..bins {
        let weight = hist.weights_s.get(bin).copied().unwrap_or(0.0);
        if weight <= 0.0 || tallest <= 0.0 {
            continue;
        }
        let Some((left, right)) = layout.bin_x(bin, bins) else {
            continue;
        };
        let height = ((weight / tallest) as f32 * BAR_HEIGHT).max(1.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(left, bar_area.bottom() - height),
            egui::pos2((right - BAR_GAP).max(left + 1.0), bar_area.bottom()),
        );
        let colour = match bin_side(hist, bin) {
            BinSide::Below => tokens::CAUTION,
            BinSide::Above => tokens::DANGER,
            BinSide::InBand if hovered_bin == Some(bin) => tokens::TEXT_MUTED,
            BinSide::InBand => tokens::TEXT_FAINT,
        };
        if is_overflow_bin(bin, bins) {
            painter.rect_stroke(
                bar,
                0.0,
                egui::Stroke::new(1.0, colour),
                egui::StrokeKind::Inside,
            );
        } else {
            painter.rect_filled(bar, 0.0, colour);
        }
    }
}

/// Paint an off-scale bound at the chart edge: an arrow that points out of
/// the chart, and the value with its unit beside it.
// SAFETY: a private painter helper; the nine inputs are one marker's
// place, text and style, and a struct for them would be used once.
#[allow(clippy::too_many_arguments)]
fn paint_off_scale(
    painter: &egui::Painter,
    placed: &mut Vec<egui::Rect>,
    bounds: egui::Rect,
    label_y: f32,
    label_height: f32,
    place: BoundPlace,
    text: &str,
    font: &egui::FontId,
    colour: egui::Color32,
) {
    let mid_y = label_y + label_height * 0.5;
    let arrow_w = CAP_HALF_WIDTH * 1.5;
    let (points, anchor, align) = if place == BoundPlace::OffLeft {
        let tip = bounds.left();
        (
            vec![
                egui::pos2(tip, mid_y),
                egui::pos2(tip + arrow_w, mid_y - CAP_HALF_WIDTH),
                egui::pos2(tip + arrow_w, mid_y + CAP_HALF_WIDTH),
            ],
            egui::pos2(tip + arrow_w + tokens::SPACE_1, label_y),
            egui::Align2::LEFT_TOP,
        )
    } else {
        // Read left to right as "▸ 0.84 kW", flush with the right edge.
        let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), colour);
        let text_left = bounds.right() - galley.size().x;
        let base = text_left - tokens::SPACE_1 - arrow_w;
        (
            vec![
                egui::pos2(base, mid_y - CAP_HALF_WIDTH),
                egui::pos2(base + arrow_w, mid_y),
                egui::pos2(base, mid_y + CAP_HALF_WIDTH),
            ],
            egui::pos2(bounds.right(), label_y),
            egui::Align2::RIGHT_TOP,
        )
    };
    // The label goes first, so the arrow beside it cannot push it out.
    // The arrow then joins the placed set, so no range label covers it.
    place_label(
        painter,
        placed,
        bounds,
        anchor,
        align,
        text,
        font,
        colour,
        Nudge::None,
    );
    placed.push(egui::Rect::from_points(&points));
    painter.add(egui::Shape::convex_polygon(
        points,
        colour,
        egui::Stroke::NONE,
    ));
}

/// True when `bin` is the underflow or the overflow bin.
#[must_use]
pub fn is_overflow_bin(bin: usize, bins: usize) -> bool {
    bin == 0 || bin + 1 == bins
}

/// The x geometry of one chart: a stub at each end, and the inner bins
/// between them, mapped by value over the display range.
///
/// The display range is the inner range of the data, widened to include a
/// bound that lies within [`NEAR_BOUND_SHARE`] of it. A bound farther out
/// is off scale and does not widen it.
struct ChartLayout {
    left: f32,
    right: f32,
    inner_left: f32,
    inner_right: f32,
    inner_lo: f64,
    inner_hi: f64,
    display_lo: f64,
    display_hi: f64,
    edges: Vec<f64>,
}

/// The inner range of the data, and the display range: the inner range
/// widened to include each bound within [`NEAR_BOUND_SHARE`] of it.
fn ranges(hist: &Histogram) -> ((f64, f64), (f64, f64)) {
    let n = hist.edges.len();
    let inner_lo = hist.edges.get(1).copied().unwrap_or(0.0);
    let inner_hi = n
        .checked_sub(2)
        .and_then(|i| hist.edges.get(i))
        .copied()
        .unwrap_or(inner_lo + 1.0);
    let margin = (inner_hi - inner_lo).max(0.0) * NEAR_BOUND_SHARE;
    let (mut display_lo, mut display_hi) = (inner_lo, inner_hi);
    for bound in [hist.floor, hist.ceiling].into_iter().flatten() {
        if bound.is_finite() && bound >= inner_lo - margin && bound <= inner_hi + margin {
            display_lo = display_lo.min(bound);
            display_hi = display_hi.max(bound);
        }
    }
    ((inner_lo, inner_hi), (display_lo, display_hi))
}

/// Where the chart draws `value`: to scale, or off scale at one edge.
#[must_use]
pub fn bound_place(hist: &Histogram, value: f64) -> BoundPlace {
    let (_, (display_lo, display_hi)) = ranges(hist);
    place_in(value, display_lo, display_hi)
}

fn place_in(value: f64, display_lo: f64, display_hi: f64) -> BoundPlace {
    if value < display_lo {
        BoundPlace::OffLeft
    } else if value > display_hi {
        BoundPlace::OffRight
    } else {
        BoundPlace::ToScale
    }
}

impl ChartLayout {
    fn new(hist: &Histogram, area: egui::Rect) -> Self {
        let ((inner_lo, inner_hi), (display_lo, display_hi)) = ranges(hist);
        Self {
            left: area.left(),
            right: area.right(),
            inner_left: area.left() + STUB_WIDTH,
            inner_right: area.right() - STUB_WIDTH,
            inner_lo,
            inner_hi,
            display_lo,
            display_hi,
            edges: hist.edges.clone(),
        }
    }

    /// Where `value` sits: to scale, or off scale at one edge.
    fn place(&self, value: f64) -> BoundPlace {
        place_in(value, self.display_lo, self.display_hi)
    }

    /// The x of a value inside the display range. A value outside it is
    /// clamped to the stub edge.
    fn value_x(&self, value: f64) -> f32 {
        let span = self.display_hi - self.display_lo;
        if span <= 0.0 || !value.is_finite() {
            return self.inner_left;
        }
        let t = ((value - self.display_lo) / span).clamp(0.0, 1.0) as f32;
        self.inner_left + t * (self.inner_right - self.inner_left)
    }

    /// The x where a zone changes at `value`: the value's x when it is to
    /// scale, and the chart edge when it is off scale.
    fn zone_x(&self, value: f64) -> f32 {
        match self.place(value) {
            BoundPlace::ToScale => self.value_x(value),
            BoundPlace::OffLeft => self.left,
            BoundPlace::OffRight => self.right,
        }
    }

    /// The left and right x of bin `bin` of `bins`.
    fn bin_x(&self, bin: usize, bins: usize) -> Option<(f32, f32)> {
        if bins < 3 || bin >= bins {
            return None;
        }
        if bin == 0 {
            return Some((self.left, self.inner_left));
        }
        if bin == bins - 1 {
            return Some((self.inner_right, self.right));
        }
        let lo = self.edges.get(bin).copied().unwrap_or(self.inner_lo);
        let hi = self.edges.get(bin + 1).copied().unwrap_or(self.inner_hi);
        Some((self.value_x(lo), self.value_x(hi)))
    }

    /// The bin under x.
    fn bin_at(&self, x: f32, bins: usize) -> Option<usize> {
        (0..bins).find(|&bin| {
            self.bin_x(bin, bins)
                .is_some_and(|(left, right)| x >= left && x < right.max(left + 1.0))
        })
    }
}

/// Where bin `bin` sits against the bounds, read at the bin's midpoint. The
/// caption shares come from core's exact sums, not from this.
#[must_use]
pub fn bin_side(hist: &Histogram, bin: usize) -> BinSide {
    let (Some(lo), Some(hi)) = (hist.edges.get(bin), hist.edges.get(bin + 1)) else {
        return BinSide::InBand;
    };
    let mid = 0.5 * (lo + hi);
    if hist.floor.is_some_and(|floor| mid < floor) {
        BinSide::Below
    } else if hist.ceiling.is_some_and(|ceiling| mid > ceiling) {
        BinSide::Above
    } else {
        BinSide::InBand
    }
}

/// The hover line of one bin:
/// `0.031–0.036 mm/tooth · 14 % of cut time · 312 samples`.
#[must_use]
pub fn bin_hover_text(hist: &Histogram, bin: usize, scale: f64, unit: &str) -> String {
    let lo = hist.edges.get(bin).copied().unwrap_or(0.0) * scale;
    let hi = hist.edges.get(bin + 1).copied().unwrap_or(0.0) * scale;
    let weight = hist.weights_s.get(bin).copied().unwrap_or(0.0);
    let count = hist.counts.get(bin).copied().unwrap_or(0);
    let share = if hist.total_s > 0.0 {
        weight / hist.total_s * 100.0
    } else {
        0.0
    };
    let mut text = format!(
        "{}\u{2013}{} {unit} \u{00b7} {} of cut time \u{00b7} {count} samples",
        format_value(lo),
        format_value(hi),
        format_share(share),
    );
    let bins = hist.weights_s.len();
    if bin == 0 && bins >= 3 {
        text.push_str(
            "\nUnderflow bin: the values below the chart range. Its width is not to scale.",
        );
    } else if bin + 1 == bins && bins >= 3 {
        text.push_str(
            "\nOverflow bin: the values above the chart range. Its width is not to scale.",
        );
    }
    if hist
        .first_move_per_bin
        .get(bin)
        .is_some_and(Option::is_some)
    {
        text.push_str("\nClick to go to the first sample in this bin.");
    }
    text
}

/// A percentage for a caption: `14 %`, or `<1 %` for a share that is not
/// zero but rounds to it.
#[must_use]
pub fn format_share(percent: f64) -> String {
    if percent > 0.0 && percent < 0.5 {
        "<1 %".to_owned()
    } else {
        format!("{percent:.0} %")
    }
}

/// A value with the digits its size needs.
#[must_use]
pub fn format_value(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude < 0.1 {
        format!("{value:.3}")
    } else if magnitude < 10.0 {
        format!("{value:.2}")
    } else if magnitude < 100.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    }
}

/// Which way a colliding label may move before it is left out.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nudge {
    None,
    Left,
    Right,
}

/// Paint one axis label when it does not collide with a label already
/// placed, and keep it inside `bounds`. A label with a `Nudge` moves clear
/// of the label it collides with, in that direction, and is left out when
/// it still collides or leaves `bounds`.
// SAFETY: a private painter helper; the nine inputs are one label's
// position, text, style and nudge, and a struct for them would be used once.
#[allow(clippy::too_many_arguments)]
fn place_label(
    painter: &egui::Painter,
    placed: &mut Vec<egui::Rect>,
    bounds: egui::Rect,
    anchor: egui::Pos2,
    align: egui::Align2,
    text: &str,
    font: &egui::FontId,
    colour: egui::Color32,
    nudge: Nudge,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), colour);
    let mut label = align.anchor_size(anchor, galley.size());
    if label.left() < bounds.left() {
        label = label.translate(egui::vec2(bounds.left() - label.left(), 0.0));
    }
    if label.right() > bounds.right() {
        label = label.translate(egui::vec2(bounds.right() - label.right(), 0.0));
    }
    let pad = egui::vec2(tokens::SPACE_1, 0.0);
    // At most one move per placed label, so the loop ends.
    for _ in 0..=placed.len() {
        let padded = label.expand2(pad);
        let Some(other) = placed.iter().find(|other| other.intersects(padded)) else {
            placed.push(label);
            painter.galley(label.min, galley, colour);
            return;
        };
        let shift = match nudge {
            Nudge::None => return,
            Nudge::Left => other.left() - tokens::SPACE_3 - label.right(),
            Nudge::Right => other.right() + tokens::SPACE_3 - label.left(),
        };
        label = label.translate(egui::vec2(shift, 0.0));
        if label.left() < bounds.left() || label.right() > bounds.right() {
            return;
        }
    }
}
