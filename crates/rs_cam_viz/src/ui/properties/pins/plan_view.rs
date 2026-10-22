//! The overhead plan view of the pins (R3; round 2, 2026-10-12).
//!
//! The view looks down on the stock: the stock rectangle with X0 Y0 at its
//! min corner and the X and Y axes, the part footprints, the keep-out zones
//! and the fixtures, the flip axis, each pin at true scale with its number
//! and its diameter, and the wrong-seating ghost when the set does not
//! key. Each pin carries its thinnest wall. The stock and the part have two
//! different fills, with a legend (round 2).
//!
//! Every number comes from the ONE core report
//! (`ProjectSession::pin_set_report_for`): the obstacles
//! (`PinSetReport::obstacles_local`), the flip axis, the state colours,
//! the ghost and the walls. The view computes no pin number. It only maps
//! millimetres to points.
//!
//! A drag on the view moves the pin SET as one, ALONG the flip axis only
//! (round 2): the view projects the drag onto the axis, so the set stays
//! symmetric. The draft moves every frame, and the edit commits once, on
//! release (`PanelEdit::drag`). The step is 0.1 mm. A click marks the pin
//! under the pointer.

use rs_cam_core::compute::alignment_pins::{AlignmentPin, PIN_WALL_MM, PinObstacleKind};
use rs_cam_core::session::PinSetReport;

use super::edit;
use crate::state::pin_view::{PIN_GHOST_COLOUR, ghost_ring_radius, pin_status_colour};
use crate::ui::properties::PanelEdit;
use crate::ui::tokens;

/// The drag step of the set, in steps per millimetre: 0.1 mm.
const DRAG_STEPS_PER_MM: f64 = 10.0;

/// The margin round the stock rectangle, in points: room for the axis
/// labels, the legend and a pin on the stock edge.
const MARGIN_PX: f32 = 18.0;

/// A pin smaller than this on the screen also gets a dot, in points.
const MIN_PIN_PX: f32 = 2.5;

/// How far from a pin centre a click still marks it, in points.
const PICK_PX: f32 = 10.0;

/// The height of one fill row of a polygon, in points.
const FILL_ROW_PX: f32 = 1.0;

/// What the view did this frame.
#[derive(Debug, Default)]
pub struct PlanViewOut {
    /// A click marked a pin (`Some(Some(i))`) or cleared the mark
    /// (`Some(None)`).
    pub selected_pin: Option<Option<usize>>,
}

/// The millimetre-to-point map of one frame.
#[derive(Debug, Clone, Copy)]
pub struct PlanFrame {
    /// The stock rectangle on the screen.
    pub stock_rect: egui::Rect,
    /// Points per millimetre.
    pub scale: f32,
}

impl PlanFrame {
    /// The frame for a `w` x `d` mm stock in `outer`.
    #[must_use]
    pub fn fit(outer: egui::Rect, w: f64, d: f64) -> Option<Self> {
        if !(w > 0.0 && d > 0.0) {
            return None;
        }
        let inner = outer.shrink(MARGIN_PX);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a stock dimension in mm is far inside the f32 range."
        )]
        let (w32, d32) = (w as f32, d as f32);
        let scale = (inner.width() / w32).min(inner.height() / d32);
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        let size = egui::vec2(w32 * scale, d32 * scale);
        let stock_rect = egui::Rect::from_center_size(inner.center(), size);
        Some(Self { stock_rect, scale })
    }

    /// The screen point of the stock-local point `(x, y)` mm. Y points up.
    #[must_use]
    pub fn to_screen(&self, x: f64, y: f64) -> egui::Pos2 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a stock-local coordinate in mm is far inside the f32 range."
        )]
        let (x, y) = (x as f32, y as f32);
        egui::pos2(
            self.stock_rect.left() + x * self.scale,
            self.stock_rect.bottom() - y * self.scale,
        )
    }

    /// The millimetre move of a screen move `delta`.
    #[must_use]
    pub fn to_mm(&self, delta: egui::Vec2) -> (f64, f64) {
        (
            f64::from(delta.x / self.scale),
            f64::from(-delta.y / self.scale),
        )
    }
}

/// The height a view takes for a `w` x `d` stock at `width`, at most
/// `max_height`.
pub(super) fn view_height(width: f32, w: f64, d: f64, max_height: f32) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the aspect ratio of a stock is far inside the f32 range."
    )]
    let aspect = if w > 0.0 { (d / w) as f32 } else { 1.0 };
    let inner = (width - 2.0 * MARGIN_PX).max(1.0);
    (inner * aspect + 2.0 * MARGIN_PX).clamp(4.0 * MARGIN_PX, max_height.max(4.0 * MARGIN_PX))
}

/// The pin under `pos`, the nearest within [`PICK_PX`] of its edge.
fn pin_at(frame: &PlanFrame, pins: &[AlignmentPin], pos: egui::Pos2) -> Option<usize> {
    pins.iter()
        .enumerate()
        .map(|(i, p)| {
            let centre = frame.to_screen(p.at.x(), p.at.y());
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a pin diameter in mm is far inside the f32 range."
            )]
            let radius = 0.5 * p.diameter as f32 * frame.scale;
            (i, centre.distance(pos) - radius)
        })
        .filter(|(_, gap)| *gap <= PICK_PX)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// What the plan view reads besides the draft pins.
#[derive(Clone, Copy)]
pub struct PlanInputs<'a> {
    /// The stock X extent, in mm.
    pub stock_w: f64,
    /// The stock Y extent, in mm.
    pub stock_d: f64,
    /// The one report of the draft pins.
    pub report: &'a PinSetReport,
    /// The marked pin.
    pub selected: Option<usize>,
    /// The largest height of the view, in points.
    pub max_height: f32,
}

/// Draw the plan view, and move the draft `pins` along the flip axis by a
/// drag.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    pins: &mut Vec<AlignmentPin>,
    inputs: PlanInputs<'_>,
    edit_out: &mut PanelEdit,
) -> PlanViewOut {
    let PlanInputs {
        stock_w,
        stock_d,
        report,
        selected,
        max_height,
    } = inputs;
    let mut out = PlanViewOut::default();
    let width = ui.available_width().max(4.0 * MARGIN_PX);
    let height = view_height(width, stock_w, stock_d, max_height);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
    let response = response.on_hover_text(
        "Drag: move the pins along the flip axis (0.1 mm steps). Click: mark a pin.",
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, tokens::RADIUS_SM, tokens::DIAGRAM_CANVAS);
    let Some(frame) = PlanFrame::fit(rect, stock_w, stock_d) else {
        return out;
    };

    // The drag moves the set along the flip axis. The remainder under one
    // step stays in egui memory as drag state (a drag index, not panel
    // content).
    let remainder_id = ui.id().with((id_salt, "pin_plan_drag_remainder"));
    if response.dragged()
        && !pins.is_empty()
        && let Some((ux, uy)) = edit::axis_unit(&report.map)
    {
        let (dx, dy) = frame.to_mm(response.drag_delta());
        let remainder = ui
            .ctx()
            .data(|d| d.get_temp::<f64>(remainder_id))
            .unwrap_or(0.0);
        let along = remainder + dx * ux + dy * uy;
        let step = (along * DRAG_STEPS_PER_MM).round() / DRAG_STEPS_PER_MM;
        if step != 0.0 {
            *pins = edit::move_along_axis(pins, &report.map, step);
            edit_out.changed = true;
        }
        ui.ctx()
            .data_mut(|d| d.insert_temp(remainder_id, along - step));
    }
    if response.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<f64>(remainder_id));
    }
    edit_out.committed |= response.drag_stopped();
    edit_out.in_flight |= response.dragged();
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let hit = pin_at(&frame, pins, pos);
        out.selected_pin = Some(if hit == selected { None } else { hit });
    }

    paint(&painter, &frame, pins, report, selected);
    out
}

/// Paint the plan.
fn paint(
    painter: &egui::Painter,
    frame: &PlanFrame,
    pins: &[AlignmentPin],
    report: &PinSetReport,
    selected: Option<usize>,
) {
    let micro = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Proportional);
    let mono = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Monospace);
    let stock = frame.stock_rect;

    // The stock, and X0 Y0 at its min corner with the two axes.
    painter.rect_filled(stock, tokens::RADIUS_SM, tokens::DIAGRAM_MATERIAL);
    painter.rect_stroke(
        stock,
        tokens::RADIUS_SM,
        egui::Stroke::new(1.0, tokens::BORDER),
        egui::StrokeKind::Middle,
    );
    // The part footprints, filled.
    for obstacle in &report.obstacles_local {
        if obstacle.kind == PinObstacleKind::Part && obstacle.closed {
            let points: Vec<egui::Pos2> = obstacle
                .points
                .iter()
                .map(|&(x, y)| frame.to_screen(x, y))
                .collect();
            fill_polygon(painter, &points, tokens::DIAGRAM_PART);
        }
    }
    let origin = stock.left_bottom();
    let arm = (stock.width().min(stock.height()) * 0.2).clamp(8.0, 28.0);
    painter.arrow(
        origin,
        egui::vec2(arm, 0.0),
        egui::Stroke::new(1.5, tokens::AXIS_X),
    );
    painter.arrow(
        origin,
        egui::vec2(0.0, -arm),
        egui::Stroke::new(1.5, tokens::AXIS_Y),
    );
    painter.text(
        origin + egui::vec2(arm + 2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "X",
        micro.clone(),
        tokens::AXIS_X,
    );
    painter.text(
        origin + egui::vec2(0.0, -arm - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "Y",
        micro.clone(),
        tokens::AXIS_Y,
    );
    painter.text(
        origin + egui::vec2(-2.0, 2.0),
        egui::Align2::RIGHT_TOP,
        "X0 Y0",
        micro.clone(),
        tokens::TEXT_MUTED,
    );
    legend(painter, &micro);

    // The part outlines and the obstacles.
    for obstacle in &report.obstacles_local {
        let colour = match obstacle.kind {
            PinObstacleKind::Part => tokens::DIAGRAM_TOOL,
            PinObstacleKind::Obstacle => tokens::DANGER,
        };
        let points: Vec<egui::Pos2> = obstacle
            .points
            .iter()
            .map(|&(x, y)| frame.to_screen(x, y))
            .collect();
        let stroke = egui::Stroke::new(1.0, colour);
        if obstacle.closed {
            painter.add(egui::Shape::closed_line(points, stroke));
        } else {
            painter.add(egui::Shape::line(points, stroke));
        }
    }

    // The flip axis.
    if let Some([a, b]) = report.map.axis_segment() {
        let (pa, pb) = (frame.to_screen(a.0, a.1), frame.to_screen(b.0, b.1));
        painter.extend(egui::Shape::dashed_line(
            &[pa, pb],
            egui::Stroke::new(1.0, tokens::DIAGRAM_INK),
            6.0,
            4.0,
        ));
        if let Some(label) = report.map.axis_label() {
            painter.text(
                pb + egui::vec2(-2.0, -2.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("flip axis {label}"),
                micro,
                tokens::DIAGRAM_INK,
            );
        }
    }

    // The wrong-seating ghost: where the holes land when the part goes on
    // the dowels turned.
    if !report.flip.keyed() {
        for (i, &(x, y)) in report.wrong_seating_images.iter().enumerate() {
            let d = pins.get(i).map_or(0.0, |p| p.diameter);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a ring radius in mm is far inside the f32 range."
            )]
            let radius = ghost_ring_radius(d) as f32 * frame.scale;
            let centre = frame.to_screen(x, y);
            let ring: Vec<egui::Pos2> = (0..=32_u8)
                .map(|k| {
                    let a = f32::from(k) / 32.0 * std::f32::consts::TAU;
                    centre + radius.max(MIN_PIN_PX + 2.0) * egui::vec2(a.cos(), a.sin())
                })
                .collect();
            painter.extend(egui::Shape::dashed_line(
                &ring,
                egui::Stroke::new(1.0, PIN_GHOST_COLOUR),
                3.0,
                3.0,
            ));
        }
    }

    // The pins at true scale, with the number, the diameter and the
    // thinnest wall of each.
    for (i, pin) in pins.iter().enumerate() {
        let centre = frame.to_screen(pin.at.x(), pin.at.y());
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a pin diameter in mm is far inside the f32 range."
        )]
        let radius = 0.5 * pin.diameter as f32 * frame.scale;
        let colour = pin_status_colour(report.pin_status(i));
        painter.circle_filled(centre, radius.max(MIN_PIN_PX), colour);
        if selected == Some(i) {
            painter.circle_stroke(
                centre,
                radius.max(MIN_PIN_PX) + 3.0,
                egui::Stroke::new(1.5, tokens::TEXT_STRONG),
            );
        }
        // A pin in the right half of the view takes its labels on its left,
        // so they stay inside the view.
        let right_half = centre.x > painter.clip_rect().center().x;
        let side = if right_half { -1.0 } else { 1.0 };
        let label_at = centre + egui::vec2(side * (radius.max(MIN_PIN_PX) + 2.0), 0.0);
        let (above, below) = if right_half {
            (egui::Align2::RIGHT_BOTTOM, egui::Align2::RIGHT_TOP)
        } else {
            (egui::Align2::LEFT_BOTTOM, egui::Align2::LEFT_TOP)
        };
        painter.text(
            label_at,
            above,
            format!("{} \u{00D8}{:.2}", i + 1, pin.diameter),
            mono.clone(),
            tokens::TEXT_STRONG,
        );
        if let Some(walls) = report.walls.get(i) {
            let wall = walls.min_mm();
            let wall_colour = if wall < 0.0 {
                tokens::DANGER
            } else if wall < PIN_WALL_MM {
                tokens::CAUTION
            } else {
                tokens::TEXT_MUTED
            };
            painter.text(
                label_at,
                below,
                format!("wall {wall:.1}"),
                mono.clone(),
                wall_colour,
            );
        }
    }
}

/// The legend in the top margin: a swatch and a word for the stock and
/// for the part.
pub(super) fn legend(painter: &egui::Painter, font: &egui::FontId) {
    let mut at = painter.clip_rect().left_top() + egui::vec2(4.0, 4.0);
    for (colour, word) in [
        (tokens::DIAGRAM_MATERIAL, "Stock"),
        (tokens::DIAGRAM_PART, "Part"),
    ] {
        let swatch = egui::Rect::from_min_size(at, egui::vec2(8.0, 8.0));
        painter.rect_filled(swatch, 1.0, colour);
        painter.rect_stroke(
            swatch,
            1.0,
            egui::Stroke::new(1.0, tokens::BORDER),
            egui::StrokeKind::Middle,
        );
        let text = painter.text(
            swatch.right_center() + egui::vec2(3.0, 0.0),
            egui::Align2::LEFT_CENTER,
            word,
            font.clone(),
            tokens::TEXT_MUTED,
        );
        at.x = text.right() + 8.0;
    }
}

/// The horizontal spans that fill the closed polygon `points`, one row of
/// [`FILL_ROW_PX`] at a time, by the even-odd rule: `(top, left, right)`.
/// The fill is correct on a concave outline too; `egui`'s path fill is
/// correct on a convex outline only.
#[must_use]
pub fn scanline_spans(points: &[egui::Pos2]) -> Vec<(f32, f32, f32)> {
    let mut out = Vec::new();
    if points.len() < 3 {
        return out;
    }
    let top = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let bottom = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    if !(top.is_finite() && bottom.is_finite()) {
        return out;
    }
    let edges: Vec<(egui::Pos2, egui::Pos2)> = points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .collect();
    let mut y = top;
    let mut xs: Vec<f32> = Vec::new();
    while y < bottom {
        let centre = y + 0.5 * FILL_ROW_PX;
        xs.clear();
        for (a, b) in &edges {
            if (a.y <= centre) != (b.y <= centre) {
                xs.push(a.x + (centre - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
        xs.sort_by(f32::total_cmp);
        for [left, right] in xs.as_chunks::<2>().0 {
            out.push((y, *left, *right));
        }
        y += FILL_ROW_PX;
    }
    out
}

/// Fill the closed polygon `points` with `colour` (a concave one too).
pub(super) fn fill_polygon(painter: &egui::Painter, points: &[egui::Pos2], colour: egui::Color32) {
    let mut mesh = egui::Mesh::default();
    for (top, left, right) in scanline_spans(points) {
        mesh.add_colored_rect(
            egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, top + FILL_ROW_PX)),
            colour,
        );
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_keeps_the_stock_aspect_and_puts_y_up() {
        let outer = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(236.0, 136.0));
        let frame = PlanFrame::fit(outer, 200.0, 100.0).unwrap();
        assert!((frame.stock_rect.width() / frame.stock_rect.height() - 2.0).abs() < 1e-3);
        let origin = frame.to_screen(0.0, 0.0);
        let up = frame.to_screen(0.0, 10.0);
        assert!(up.y < origin.y, "Y points up on the screen");
        let (dx, dy) = frame.to_mm(egui::vec2(frame.scale * 5.0, -frame.scale * 2.0));
        assert!(
            (dx - 5.0).abs() < 1e-4 && (dy - 2.0).abs() < 1e-4,
            "{dx} {dy}"
        );
    }

    /// The fill covers a concave L shape and leaves its notch empty.
    #[test]
    fn the_fill_spans_a_concave_outline() {
        let l = [
            egui::pos2(0.0, 0.0),
            egui::pos2(20.0, 0.0),
            egui::pos2(20.0, 10.0),
            egui::pos2(10.0, 10.0),
            egui::pos2(10.0, 20.0),
            egui::pos2(0.0, 20.0),
        ];
        let spans = scanline_spans(&l);
        assert_eq!(spans.len(), 20, "one span per row");
        let area: f32 = spans.iter().map(|(_, a, b)| b - a).sum();
        assert!((area - 300.0).abs() < 1e-3, "the L area is 300: {area}");
        assert!(spans.iter().all(|(y, _, b)| *y < 10.0 || *b <= 10.0));
    }
}
