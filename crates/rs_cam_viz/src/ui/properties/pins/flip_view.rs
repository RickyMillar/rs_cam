//! The flip diagram of the pin wizard (round 2, 2026-10-12).
//!
//! A small view from above shows the flip that the setups select: the
//! stock with the part footprint, the flip axis and the pins. The part
//! turns over about the flip axis in a loop, like a card. The holes in the
//! part move to their images, and each lands on a dowel when the set
//! seats.
//!
//! The motion comes from the judged setup's map (`PinSetReport::map`,
//! `PinSetupMap`), not from a special case. An in-plane flip is a mirror
//! about the flip axis (`PinSetupMap::axis_segment`). The card turn scales
//! the distance across the axis by cos(pi s). A rest of the map that is
//! not the mirror (a turn about the stock centre) turns in step, so the
//! frame at s = 1 is the map itself ([`FlipMotion::at`]).
//!
//! A hover stops the loop and shows the part turned over. A pure function
//! gives the frame at a time ([`cycle_progress`], [`FlipMotion::at`]).

use rs_cam_core::compute::alignment_pins::{AlignmentPin, PinObstacleKind, PinSetupMap};
use rs_cam_core::session::PinSetReport;

use super::plan_view::{PlanFrame, fill_polygon, legend, view_height};
use crate::state::pin_view::pin_status_colour;
use crate::ui::tokens;

/// The time of one loop, in seconds: the turn over, a rest, the turn back
/// and a rest.
const CYCLE_S: f64 = 3.0;

/// The part of the loop at rest at each end, as a fraction of the loop.
const REST_FRACTION: f64 = 0.15;

/// The largest height of the diagram, in points.
const MAX_HEIGHT_PX: f32 = 150.0;

/// A pin smaller than this on the screen still shows, in points.
const MIN_PIN_PX: f32 = 2.5;

/// The progress of the turn at the time `t` seconds: 0 is the part as it
/// is cut, 1 is the part turned over. The loop rests at each end, and each
/// turn eases in and out.
#[must_use]
pub fn cycle_progress(t: f64) -> f64 {
    let phase = t.rem_euclid(CYCLE_S) / CYCLE_S;
    let ease = |u: f64| {
        let u = u.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    let turn = 0.5 - REST_FRACTION;
    if phase < REST_FRACTION {
        0.0
    } else if phase < 0.5 {
        ease((phase - REST_FRACTION) / turn)
    } else if phase < 0.5 + REST_FRACTION {
        1.0
    } else {
        1.0 - ease((phase - 0.5 - REST_FRACTION) / turn)
    }
}

/// The motion of a point of the part during the turn.
#[derive(Clone)]
pub struct FlipMotion {
    /// The map of the judged setup.
    map: PinSetupMap,
    /// A point on the flip axis (its midpoint, the stock centre).
    centre: (f64, f64),
    /// The unit direction of the axis and the unit normal to it.
    u: (f64, f64),
    n: (f64, f64),
    /// The turn of the map about the centre that is not the mirror, in
    /// radians. Zero for every in-plane flip of today's maps.
    turn_rad: f64,
}

impl FlipMotion {
    /// The motion of `map`, or `None` when the map has no flip axis.
    #[must_use]
    pub fn of(map: &PinSetupMap) -> Option<Self> {
        let [a, b] = map.axis_segment()?;
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        if len <= 0.0 {
            return None;
        }
        let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let n = (-u.1, u.0);
        let centre = (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1));
        let mut motion = Self {
            map: map.clone(),
            centre,
            u,
            n,
            turn_rad: 0.0,
        };
        // The rest of the map: the angle from the mirror image of a test
        // vector to the map's image of it.
        let probe = (centre.0 + u.0 + n.0, centre.1 + u.1 + n.1);
        let mirrored = motion.card(probe.0, probe.1, 1.0);
        let mapped = map.map_xy(probe.0, probe.1);
        let c_mapped = map.map_xy(centre.0, centre.1);
        let v_r = (mirrored.0 - centre.0, mirrored.1 - centre.1);
        let v_m = (mapped.0 - c_mapped.0, mapped.1 - c_mapped.1);
        motion.turn_rad = (v_r.0 * v_m.1 - v_r.1 * v_m.0).atan2(v_r.0 * v_m.0 + v_r.1 * v_m.1);
        Some(motion)
    }

    /// The card turn alone: the distance across the axis scaled by
    /// cos(pi s).
    fn card(&self, x: f64, y: f64, s: f64) -> (f64, f64) {
        let d = (x - self.centre.0, y - self.centre.1);
        let along = d.0 * self.u.0 + d.1 * self.u.1;
        let across = d.0 * self.n.0 + d.1 * self.n.1;
        let across = across * (std::f64::consts::PI * s).cos();
        (
            self.centre.0 + along * self.u.0 + across * self.n.0,
            self.centre.1 + along * self.u.1 + across * self.n.1,
        )
    }

    /// Where the part point `(x, y)` is at the progress `s` (0 to 1). At
    /// `s = 0` it is the point; at `s = 1` it is the map's image of it.
    #[must_use]
    pub fn at(&self, x: f64, y: f64, s: f64) -> (f64, f64) {
        let (cx, cy) = self.card(x, y, s);
        let shift = {
            let c = self.map.map_xy(self.centre.0, self.centre.1);
            (s * (c.0 - self.centre.0), s * (c.1 - self.centre.1))
        };
        let (sin, cos) = (self.turn_rad * s).sin_cos();
        let d = (cx - self.centre.0, cy - self.centre.1);
        (
            self.centre.0 + d.0 * cos - d.1 * sin + shift.0,
            self.centre.1 + d.0 * sin + d.1 * cos + shift.1,
        )
    }

    /// `true` when the progress `s` shows the underside of the stock.
    #[must_use]
    pub fn underside(s: f64) -> bool {
        s > 0.5
    }
}

/// Draw the flip diagram. The caller checks that the map has a flip axis;
/// with none, the diagram draws the still stock.
pub fn show(
    ui: &mut egui::Ui,
    report: &PinSetReport,
    pins: &[AlignmentPin],
    stock_w: f64,
    stock_d: f64,
) -> egui::Response {
    let width = ui.available_width().clamp(120.0, 360.0);
    let height = view_height(width, stock_w, stock_d, MAX_HEIGHT_PX);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let response = response.on_hover_text("Hover: the part turned over.");
    let motion = FlipMotion::of(&report.map);
    let s = if response.hovered() {
        1.0
    } else {
        if motion.is_some() {
            ui.ctx().request_repaint();
        }
        cycle_progress(ui.input(|i| i.time))
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, tokens::RADIUS_SM, tokens::DIAGRAM_CANVAS);
    let Some(frame) = PlanFrame::fit(rect, stock_w, stock_d) else {
        return response;
    };
    let place = |x: f64, y: f64| {
        let (x, y) = motion.as_ref().map_or((x, y), |m| m.at(x, y, s));
        frame.to_screen(x, y)
    };
    let scale_r = |d: f64| {
        // SAFETY: a pin diameter in mm is far inside the f32 range.
        #[allow(clippy::cast_possible_truncation)]
        let r = 0.5 * d as f32 * frame.scale;
        r.max(MIN_PIN_PX)
    };

    // The dowels in the board stay where the holes were drilled.
    for pin in pins {
        painter.circle_stroke(
            frame.to_screen(pin.at.x(), pin.at.y()),
            scale_r(pin.diameter) + 1.5,
            egui::Stroke::new(1.0, tokens::TEXT_MUTED),
        );
    }

    // The stock, its face, and the part footprints, as they turn.
    let corners: Vec<egui::Pos2> = [
        (0.0, 0.0),
        (stock_w, 0.0),
        (stock_w, stock_d),
        (0.0, stock_d),
    ]
    .into_iter()
    .map(|(x, y)| place(x, y))
    .collect();
    let face = if FlipMotion::underside(s) {
        tokens::DIAGRAM_MATERIAL_UNDERSIDE
    } else {
        tokens::DIAGRAM_MATERIAL
    };
    fill_polygon(&painter, &corners, face);
    painter.add(egui::Shape::closed_line(
        corners,
        egui::Stroke::new(1.0, tokens::BORDER),
    ));
    for obstacle in &report.obstacles_local {
        if obstacle.kind == PinObstacleKind::Part && obstacle.closed {
            let points: Vec<egui::Pos2> =
                obstacle.points.iter().map(|&(x, y)| place(x, y)).collect();
            fill_polygon(&painter, &points, tokens::DIAGRAM_PART);
        }
    }

    // The flip axis does not move.
    if let Some([a, b]) = report.map.axis_segment() {
        painter.extend(egui::Shape::dashed_line(
            &[frame.to_screen(a.0, a.1), frame.to_screen(b.0, b.1)],
            egui::Stroke::new(1.0, tokens::DIAGRAM_INK),
            6.0,
            4.0,
        ));
    }

    // The holes in the part move with it.
    for (i, pin) in pins.iter().enumerate() {
        painter.circle_filled(
            place(pin.at.x(), pin.at.y()),
            scale_r(pin.diameter),
            pin_status_colour(report.pin_status(i)),
        );
    }
    legend(
        &painter,
        &egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Proportional),
    );
    response
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).hypot(a.1 - b.1) < 1e-9
    }

    /// The frame at s = 0 is the part as cut; at s = 1 it is the setup's
    /// own map, for every in-plane flip (the diagonal of a quarter turn on
    /// a square stock too). At s = 0.5 the part is on edge: every point is
    /// on the axis line.
    #[test]
    fn the_turn_ends_on_the_map_of_the_setup() {
        let cases = [
            (ZRotation::Deg0, 80.0, 50.0),
            (ZRotation::Deg180, 80.0, 50.0),
            (ZRotation::Deg90, 65.0, 65.0),
            (ZRotation::Deg270, 65.0, 65.0),
        ];
        for (turn, w, d) in cases {
            let map = PinSetupMap::new(FaceUp::Bottom, turn, w, d);
            let motion = FlipMotion::of(&map).unwrap();
            for p in [(0.0, 0.0), (w, 0.0), (12.0, 7.5), (w * 0.3, d * 0.9)] {
                assert!(close(motion.at(p.0, p.1, 0.0), p), "{turn:?} {p:?}");
                assert!(
                    close(motion.at(p.0, p.1, 1.0), map.map_xy(p.0, p.1)),
                    "{turn:?} {p:?}: {:?} vs {:?}",
                    motion.at(p.0, p.1, 1.0),
                    map.map_xy(p.0, p.1)
                );
                let mid = motion.at(p.0, p.1, 0.5);
                let [a, b] = map.axis_segment().unwrap();
                let cross = (b.0 - a.0) * (mid.1 - a.1) - (b.1 - a.1) * (mid.0 - a.0);
                assert!(cross.abs() < 1e-6, "{turn:?} {p:?}: off the axis at s 0.5");
            }
        }
        let top = PinSetupMap::new(FaceUp::Top, ZRotation::Deg0, 65.0, 65.0);
        assert!(FlipMotion::of(&top).is_none());
    }

    /// The loop rests at each end and turns in between.
    #[test]
    fn the_loop_rests_at_each_end() {
        assert_eq!(cycle_progress(0.0), 0.0);
        assert_eq!(cycle_progress(CYCLE_S * 0.55), 1.0);
        let mid = cycle_progress(CYCLE_S * (REST_FRACTION + 0.5) * 0.5);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(cycle_progress(CYCLE_S * 1.05), 0.0, "the loop repeats");
        assert!(!FlipMotion::underside(0.4) && FlipMotion::underside(0.6));
    }
}
