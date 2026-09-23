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
//!   `CAUTION`. A bar above the ceiling takes `DANGER`. There is no
//!   background shading (§7 Q1).
//! - A thin vertical line at each bound the gate has. The line is dashed
//!   when the bound is advisory.
//! - Labels under the bars: the floor, the ceiling and the two range ends,
//!   in the metric's unit. A label that collides with one already placed is
//!   left out; the bounds are placed first.
//!
//! The first and the last bins are overflow bins (see the core type). The
//! chart draws each one as a fixed stub, so a spike is never hidden and
//! never stretches the axis.
//!
//! # What the chart does NOT do
//!
//! It does not seek the playhead. [`DistributionChart::show`] returns the
//! clicked bin, and the caller emits the seek command.

use rs_cam_core::tool_load::Histogram;

use crate::ui::tokens;

/// The height of the bar area, in points.
pub const BAR_HEIGHT: f32 = 56.0;

/// The width of each overflow stub, in points.
pub const STUB_WIDTH: f32 = tokens::SPACE_3;

/// The gap between two bars, in points.
const BAR_GAP: f32 = 1.0;

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

    /// Draw the bound lines dashed: the bound is a rule of thumb that does
    /// not gate an export.
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
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, BAR_HEIGHT + tokens::SPACE_1 + label_height),
            egui::Sense::click(),
        );
        let hist = self.histogram;
        let bins = hist.weights_s.len();
        let bar_area = egui::Rect::from_min_size(rect.min, egui::vec2(width, BAR_HEIGHT));
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
            painter.line_segment(
                [bar_area.left_bottom(), bar_area.right_bottom()],
                egui::Stroke::new(1.0, tokens::HAIRLINE),
            );
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
                painter.rect_filled(bar, 0.0, colour);
            }

            let mut placed: Vec<egui::Rect> = Vec::new();
            let label_y = bar_area.bottom() + tokens::SPACE_1;
            for (bound, colour) in [
                (hist.floor, tokens::CAUTION),
                (hist.ceiling, tokens::DANGER),
            ] {
                let Some(value) = bound else {
                    continue;
                };
                let x = layout.value_x(value);
                let top = bar_area.top();
                let bottom = bar_area.bottom();
                let stroke = egui::Stroke::new(1.0, colour);
                if self.advisory {
                    painter.extend(egui::Shape::dashed_line(
                        &[egui::pos2(x, top), egui::pos2(x, bottom)],
                        stroke,
                        3.0,
                        2.0,
                    ));
                } else {
                    painter.line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], stroke);
                }
                place_label(
                    &painter,
                    &mut placed,
                    rect,
                    egui::pos2(x, label_y),
                    egui::Align2::CENTER_TOP,
                    &format_value(value * self.scale),
                    &font,
                    colour,
                );
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
}

/// The x geometry of one chart: a stub at each end, and the inner bins in
/// equal widths between them.
struct ChartLayout {
    left: f32,
    right: f32,
    inner_left: f32,
    inner_right: f32,
    inner_lo: f64,
    inner_hi: f64,
}

impl ChartLayout {
    fn new(hist: &Histogram, area: egui::Rect) -> Self {
        let n = hist.edges.len();
        let inner_lo = hist.edges.get(1).copied().unwrap_or(0.0);
        let inner_hi = n
            .checked_sub(2)
            .and_then(|i| hist.edges.get(i))
            .copied()
            .unwrap_or(inner_lo + 1.0);
        Self {
            left: area.left(),
            right: area.right(),
            inner_left: area.left() + STUB_WIDTH,
            inner_right: area.right() - STUB_WIDTH,
            inner_lo,
            inner_hi,
        }
    }

    /// The x of a value inside the inner range. A value outside it is
    /// clamped to the stub edge.
    fn value_x(&self, value: f64) -> f32 {
        let span = self.inner_hi - self.inner_lo;
        if span <= 0.0 || !value.is_finite() {
            return self.inner_left;
        }
        let t = ((value - self.inner_lo) / span).clamp(0.0, 1.0) as f32;
        self.inner_left + t * (self.inner_right - self.inner_left)
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
        let inner = (bins - 2) as f32;
        let step = (self.inner_right - self.inner_left) / inner;
        let left = self.inner_left + step * (bin - 1) as f32;
        Some((left, left + step))
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

/// Paint one axis label when it does not collide with a label already
/// placed, and keep it inside `bounds`.
// SAFETY: a private painter helper; the eight inputs are one label's
// position, text and style, and a struct for them would be used once.
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
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), colour);
    let mut label = align.anchor_size(anchor, galley.size());
    if label.left() < bounds.left() {
        label = label.translate(egui::vec2(bounds.left() - label.left(), 0.0));
    }
    if label.right() > bounds.right() {
        label = label.translate(egui::vec2(bounds.right() - label.right(), 0.0));
    }
    let padded = label.expand2(egui::vec2(tokens::SPACE_1, 0.0));
    if placed.iter().any(|other| other.intersects(padded)) {
        return;
    }
    placed.push(label);
    painter.galley(label.min, galley, colour);
}
