//! The alignment pins in the viewport (P7): the circles in their state
//! colour, the wrong-orientation ghost, the flip axis and the pin numbers.
//!
//! Every position comes from `state::pin_view::pin_display_point` in the
//! frame of the active display setup, the frame the pick reads (C8). Every
//! state comes from the one core report,
//! `ProjectSession::pin_set_report`.

use crate::render::LineVertex;
use crate::state::AppState;
use crate::state::job::SetupFrame;
use crate::state::pin_view::{
    PIN_GHOST_COLOUR, ghost_ring_radius, line_rgb, pin_display_point, pin_display_points,
    pin_status_colour,
};

/// The segments of a pin circle.
const CIRCLE_SEGMENTS: usize = 32;

/// The radius of the second ring round the selected pin, as a multiple of
/// the pin radius.
const SELECTED_RING_SCALE: f32 = 1.35;

/// Push the pin overlays of `state` in `frame` into the line buffer: the
/// circles, the selected ring, the ghost and the flip axis. Each reads its
/// own registry flag.
pub(super) fn push_pin_lines(state: &AppState, frame: &SetupFrame, out: &mut Vec<LineVertex>) {
    let stock = state.session.stock_config();
    let pins = &stock.alignment_pins;
    let show_pins = state.viewport.show_alignment_pins && !pins.is_empty();
    let show_ghost = state.viewport.show_pin_ghost && !pins.is_empty();
    if show_pins || show_ghost {
        let report = state.session.pin_set_report();
        let centres = pin_display_points(frame, stock);
        if show_pins {
            for (index, (pin, c)) in pins.iter().zip(&centres).enumerate() {
                let radius = (pin.diameter * 0.5) as f32;
                let rgb = line_rgb(pin_status_colour(report.pin_status(index)));
                let (cx, cy, cz) = (c.x as f32, c.y as f32, c.z as f32);
                super::push_circle_vertices(out, cx, cy, cz, radius, rgb, CIRCLE_SEGMENTS);
                if state.panels.selected_pin == Some(index) {
                    let ring = radius * SELECTED_RING_SCALE;
                    let accent = line_rgb(crate::ui::tokens::ACCENT);
                    super::push_circle_vertices(out, cx, cy, cz, ring, accent, CIRCLE_SEGMENTS);
                }
            }
        }
        // The ghost shows WHY the set does not key: the holes after the
        // closest wrong seating. A ghost on a real dowel is a seating.
        if show_ghost && !report.flip.keyed() {
            let rgb = line_rgb(PIN_GHOST_COLOUR);
            for (pin, &(gx, gy)) in pins.iter().zip(&report.wrong_seating_images) {
                let c = pin_display_point(frame, stock, gx, gy);
                let radius = ghost_ring_radius(pin.diameter) as f32;
                push_dashed_circle(out, [c.x as f32, c.y as f32, c.z as f32], radius, rgb);
            }
        }
    }

    // The flip axis, from the setups by the one core selector (P3).
    let axis = state
        .viewport
        .show_flip_axis
        .then(|| {
            stock.pin_flip_map(
                state
                    .session
                    .list_setups()
                    .iter()
                    .map(|sd| (sd.face_up, sd.z_rotation)),
            )
        })
        .flatten()
        .and_then(|map| map.axis_segment());
    if let Some([a, b]) = axis {
        let s = pin_display_point(frame, stock, a.0, a.1);
        let e = pin_display_point(frame, stock, b.0, b.1);
        crate::render::toolpath_render::push_dashed_line(
            out,
            [s.x as f32, s.y as f32, s.z as f32],
            [e.x as f32, e.y as f32, e.z as f32],
            line_rgb(crate::ui::tokens::CAUTION),
            5.0,
            3.0,
        );
    }
}

/// A dashed circle: every second segment of a [`CIRCLE_SEGMENTS`] circle.
fn push_dashed_circle(out: &mut Vec<LineVertex>, c: [f32; 3], radius: f32, color: [f32; 3]) {
    let step = std::f32::consts::TAU / CIRCLE_SEGMENTS as f32;
    for i in (0..CIRCLE_SEGMENTS).step_by(2) {
        let a0 = i as f32 * step;
        let a1 = a0 + step;
        out.push(LineVertex {
            position: [c[0] + radius * a0.cos(), c[1] + radius * a0.sin(), c[2]],
            color,
        });
        out.push(LineVertex {
            position: [c[0] + radius * a1.cos(), c[1] + radius * a1.sin(), c[2]],
            color,
        });
    }
}

impl super::RsCamApp {
    /// Paint the number of each pin beside its circle, in the colour of its
    /// state. The numbers count from 1, in stored order, as the stock
    /// panel's table and the findings count them.
    pub(super) fn draw_pin_numbers(&self, ui: &egui::Ui, viewport_rect: egui::Rect) {
        let state = self.controller.state();
        let stock = state.session.stock_config();
        if !state.viewport.show_alignment_pins || stock.alignment_pins.is_empty() {
            return;
        }
        let Some((frame, _)) =
            crate::state::pin_view::active_display_setup(&state.session, &state.selection)
        else {
            return;
        };
        let report = state.session.pin_set_report();
        let width = viewport_rect.width().max(1.0);
        let height = viewport_rect.height().max(1.0);
        let aspect = width / height;
        let painter = ui.painter_at(viewport_rect);
        let font = egui::FontId::proportional(13.0);
        for (index, pin) in stock.alignment_pins.iter().enumerate() {
            // The label sits beside the circle, one radius out along +X and
            // +Y of the display frame, so it never covers the circle.
            let c = pin_display_point(&frame, stock, pin.x, pin.y);
            let r = pin.diameter * 0.5;
            let anchor = [(c.x + r) as f32, (c.y + r) as f32, c.z as f32];
            let Some(p) = self.camera.project_to_screen(anchor, aspect, width, height) else {
                continue;
            };
            let at = egui::pos2(viewport_rect.min.x + p[0], viewport_rect.min.y + p[1]);
            let colour = pin_status_colour(report.pin_status(index));
            let galley = painter.layout_no_wrap((index + 1).to_string(), font.clone(), colour);
            let pill = egui::Rect::from_min_size(at, galley.size() + egui::vec2(8.0, 4.0));
            painter.rect_filled(pill, 4.0, crate::ui::tokens::SURFACE_RAISED);
            let stroke_width = if state.panels.selected_pin == Some(index) {
                2.5_f32
            } else {
                1.0_f32
            };
            painter.rect_stroke(
                pill,
                4.0,
                egui::Stroke::new(stroke_width, colour),
                egui::StrokeKind::Inside,
            );
            painter.galley(pill.center() - galley.size() * 0.5, galley, colour);
        }
    }
}
