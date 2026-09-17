//! The small per-operation shape diagrams.
//!
//! Each function draws one minimap that explains one parameter: the stepover
//! pattern, the dogbone corner, the lead-in arc, the tabs, and so on. The
//! parent re-exports every name, so the inspector imports do not change.

use crate::state::toolpath::{OperationConfig, PocketPattern};

/// Whether an operation has a displayable stepover pattern.
pub(in crate::ui::properties) enum StepoverPattern {
    Zigzag { stepover: f64, angle: f64 },
    Contour { stepover: f64 },
}

impl StepoverPattern {
    /// Extract pattern info from the current operation config (if applicable).
    pub fn from_operation(op: &OperationConfig) -> Option<Self> {
        match op {
            OperationConfig::Pocket(cfg) => Some(match cfg.pattern {
                PocketPattern::Zigzag => Self::Zigzag {
                    stepover: cfg.stepover,
                    angle: cfg.angle,
                },
                PocketPattern::Contour => Self::Contour {
                    stepover: cfg.stepover,
                },
            }),
            OperationConfig::Face(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: 0.0,
            }),
            OperationConfig::Zigzag(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: cfg.angle,
            }),
            OperationConfig::VCarve(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: 0.0,
            }),
            OperationConfig::Rest(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: cfg.angle,
            }),
            OperationConfig::HorizontalFinish(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: 0.0,
            }),
            OperationConfig::DropCutter(cfg) => Some(Self::Zigzag {
                stepover: cfg.stepover,
                angle: 0.0,
            }),
            OperationConfig::Waterline(cfg) => Some(Self::Contour {
                stepover: cfg.z_step,
            }),
            OperationConfig::Scallop(cfg) => Some(Self::Contour {
                stepover: cfg.scallop_height * 5.0,
            }),
            _ => None,
        }
    }
}

/// Draw a top-down minimap showing stepover pass pattern.
pub(in crate::ui::properties) fn draw_stepover_diagram(
    ui: &mut egui::Ui,
    pattern: &StepoverPattern,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    // Workpiece rectangle (80% of canvas, centered)
    let margin = 14.0;
    let wp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin),
    );
    painter.rect_stroke(
        wp,
        2.0,
        egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_MATERIAL),
        egui::StrokeKind::Middle,
    );

    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let path_stroke = egui::Stroke::new(1.2_f32, path_color);
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;

    match pattern {
        StepoverPattern::Zigzag { stepover, angle } => {
            let angle_rad = (*angle as f32).to_radians();
            let cos_a = angle_rad.cos();
            let sin_a = angle_rad.sin();

            // Determine how many lines fit
            let perp_span = wp.width() * sin_a.abs() + wp.height() * cos_a.abs();
            let step_px = (*stepover as f32 / perp_span.max(1.0) * perp_span)
                .max(6.0)
                .min(perp_span / 2.0);
            let num_lines = (perp_span / step_px).ceil() as usize;
            let num_lines = num_lines.min(20);

            let cx = wp.center().x;
            let cy = wp.center().y;

            for i in 0..=num_lines {
                let t = if num_lines > 0 {
                    i as f32 / num_lines as f32
                } else {
                    0.5
                };
                let offset = (t - 0.5) * perp_span;

                // Line center point offset perpendicular to angle
                let lx = cx + offset * sin_a;
                let ly = cy + offset * (-cos_a);

                // Line extends along the angle direction
                let half_diag = (wp.width() + wp.height()) * 0.7;
                let x0 = lx - half_diag * cos_a;
                let y0 = ly - half_diag * sin_a;
                let x1 = lx + half_diag * cos_a;
                let y1 = ly + half_diag * sin_a;

                // Clip to workpiece (simple rect clip)
                let p0 = egui::pos2(
                    x0.clamp(wp.left(), wp.right()),
                    y0.clamp(wp.top(), wp.bottom()),
                );
                let p1 = egui::pos2(
                    x1.clamp(wp.left(), wp.right()),
                    y1.clamp(wp.top(), wp.bottom()),
                );

                if (p0.x - p1.x).abs() > 1.0 || (p0.y - p1.y).abs() > 1.0 {
                    painter.line_segment([p0, p1], path_stroke);

                    // Direction arrow on middle lines
                    if i > 0 && i < num_lines && i % 2 == 0 {
                        let mid = egui::pos2((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
                        let dir = if i % 4 == 0 { 1.0 } else { -1.0 };
                        let ax = mid.x + dir * cos_a * 5.0;
                        let ay = mid.y + dir * sin_a * 5.0;
                        painter.circle_filled(egui::pos2(ax, ay), 2.0, path_color);
                    }
                }
            }

            // Angle label
            if angle.abs() > 0.1 {
                painter.text(
                    egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
                    egui::Align2::LEFT_TOP,
                    format!("Zigzag {angle:.0}\u{00B0}"),
                    egui::FontId::proportional(9.0),
                    dim_color,
                );
            } else {
                painter.text(
                    egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
                    egui::Align2::LEFT_TOP,
                    "Zigzag",
                    egui::FontId::proportional(9.0),
                    dim_color,
                );
            }

            // Stepover dimension
            painter.text(
                egui::pos2(rect.right() - 6.0, rect.bottom() - 6.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("step {stepover:.2} mm"),
                egui::FontId::proportional(8.0),
                dim_color,
            );
        }

        StepoverPattern::Contour { stepover } => {
            // Concentric inset rectangles
            let step_frac = (*stepover as f32 / 50.0).clamp(0.05, 0.3);
            let max_insets = 8_usize;
            let mut inset = 0.0_f32;
            let min_dim = wp.width().min(wp.height());

            for i in 0..max_insets {
                let r = egui::Rect::from_min_max(
                    egui::pos2(wp.left() + inset, wp.top() + inset),
                    egui::pos2(wp.right() - inset, wp.bottom() - inset),
                );
                if r.width() < 4.0 || r.height() < 4.0 {
                    break;
                }
                let alpha = if i == 0 { 1.0 } else { 0.6 };
                painter.rect_stroke(
                    r,
                    1.0,
                    egui::Stroke::new(
                        1.2_f32,
                        egui::Color32::from_rgba_premultiplied(
                            (path_color.r() as f32 * alpha) as u8,
                            (path_color.g() as f32 * alpha) as u8,
                            (path_color.b() as f32 * alpha) as u8,
                            (255.0 * alpha) as u8,
                        ),
                    ),
                    egui::StrokeKind::Middle,
                );
                inset += step_frac * min_dim;
            }

            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
                egui::Align2::LEFT_TOP,
                "Contour",
                egui::FontId::proportional(9.0),
                dim_color,
            );
            painter.text(
                egui::pos2(rect.right() - 6.0, rect.bottom() - 6.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("step {stepover:.2} mm"),
                egui::FontId::proportional(8.0),
                dim_color,
            );
        }
    }
}

// ── Dogbone Diagram ─────────────────────────────────────────────────────

/// Draw a corner showing the dogbone overcut geometry.
/// Matches the actual dogbone algorithm from dressup.rs: the overcut goes
/// along the opposite bisector of the forward vectors into the material.
pub(in crate::ui::properties) fn draw_dogbone_diagram(ui: &mut egui::Ui, max_angle: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let cx = rect.center().x;
    let cy = rect.center().y + 5.0;
    let path_color = crate::ui::tokens::TEXT_FAINT;
    let overcut_color = crate::ui::tokens::CAUTION;
    let tool_color = crate::ui::tokens::TEXT_STRONG;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;
    let mat_color = crate::ui::tokens::SURFACE_OVERLAY;

    let corner = egui::pos2(cx, cy);
    let arm_len = 35.0;
    let tool_r = 8.0;

    // The max_angle parameter is the threshold: corners sharper than
    // (180° - max_angle) get dogbones. We show a corner AT max_angle.
    // Edge A arrives from the left (forward dir u1 = right = 0°)
    // Edge B departs at angle such that the turning angle = π - max_angle_rad
    let max_angle_rad = (max_angle as f32).to_radians();
    // Turning angle between forward vectors
    let turn_angle = std::f32::consts::PI - max_angle_rad;
    // Edge B forward direction: u1 rotated by (π - turn_angle) = max_angle_rad
    // Since u1 points right (0°), u2 direction angle = -(π - turn_angle) to turn left
    let u2_angle = -turn_angle; // negative = clockwise = inside corner going up-right

    // Points: A → B is the incoming edge, B → C is the outgoing edge
    let a = egui::pos2(cx - arm_len, cy);
    let c = egui::pos2(cx + arm_len * u2_angle.cos(), cy + arm_len * u2_angle.sin());

    // Material: fill the inside of the corner (the region the tool can't reach)
    // The inside is on the right side of the path (clockwise turn)
    let mat_far = egui::pos2(cx + arm_len * 0.8, cy - arm_len * 0.5);
    let mat_pts = vec![corner, egui::pos2(cx + arm_len, cy), mat_far, c];
    painter.add(egui::Shape::convex_polygon(
        mat_pts,
        mat_color,
        egui::Stroke::NONE,
    ));

    // Edges
    painter.line_segment([a, corner], egui::Stroke::new(2.0_f32, path_color));
    painter.line_segment([corner, c], egui::Stroke::new(2.0_f32, path_color));

    // Direction arrows on edges
    let arrow_color = crate::ui::tokens::OK;
    let mid_a = egui::pos2((a.x + cx) / 2.0, cy);
    painter.circle_filled(egui::pos2(mid_a.x + 4.0, mid_a.y), 2.0, arrow_color);
    let mid_c = egui::pos2((cx + c.x) / 2.0, (cy + c.y) / 2.0);
    painter.circle_filled(mid_c, 2.0, arrow_color);

    // Tool circle at corner (where tool is when it reaches corner B)
    painter.circle_stroke(corner, tool_r, egui::Stroke::new(1.0_f32, tool_color));

    // Compute dogbone overcut direction (from the actual algorithm):
    // u1 = forward of edge A = (1, 0) (rightward)
    // u2 = forward of edge B = (cos(u2_angle), sin(u2_angle))
    // bisector of forwards = (-u1 + u2)
    // dogbone dir = -(bisector), normalized
    let u1x: f32 = 1.0;
    let u1y: f32 = 0.0;
    let u2x = u2_angle.cos();
    let u2y = u2_angle.sin();
    let bx = -u1x + u2x;
    let by = -u1y + u2y;
    let blen = (bx * bx + by * by).sqrt().max(0.001);
    let dx = -(bx / blen);
    let dy = -(by / blen);

    let overcut_pt = egui::pos2(cx + dx * tool_r, cy + dy * tool_r);

    // Overcut line and point
    painter.line_segment(
        [corner, overcut_pt],
        egui::Stroke::new(1.5_f32, overcut_color),
    );
    painter.circle_filled(overcut_pt, 3.0, overcut_color);

    // Ghost tool at overcut position
    painter.circle_stroke(
        overcut_pt,
        tool_r,
        egui::Stroke::new(
            0.8_f32,
            // The same tone as the overcut it marks, at its original alpha.
            egui::Color32::from_rgba_premultiplied(
                overcut_color.r(),
                overcut_color.g(),
                overcut_color.b(),
                80,
            ),
        ),
    );

    // Corner angle arc
    let arc_r = 16.0_f32;
    let arc_start = 0.0_f32; // edge A forward direction
    let arc_end = u2_angle; // edge B forward direction
    let mut arc_pts = Vec::with_capacity(12);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let a_angle = arc_start + (arc_end - arc_start) * t;
        arc_pts.push(egui::pos2(
            cx + arc_r * a_angle.cos(),
            cy + arc_r * a_angle.sin(),
        ));
    }
    painter.add(egui::Shape::line(
        arc_pts,
        egui::Stroke::new(0.8_f32, dim_color),
    ));

    // Labels
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
        egui::Align2::LEFT_TOP,
        "Dogbone Overcut",
        egui::FontId::proportional(9.0),
        overcut_color,
    );
    painter.text(
        egui::pos2(rect.right() - 6.0, rect.bottom() - 6.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("corners \u{2264} {max_angle:.0}\u{00B0}"),
        egui::FontId::proportional(8.0),
        dim_color,
    );
}

// ── Lead-in / Lead-out Diagram ──────────────────────────────────────────

/// Draw a top-down view showing lead-in and lead-out quarter-circle arcs.
pub(in crate::ui::properties) fn draw_lead_in_out_diagram(ui: &mut egui::Ui, radius: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let cy = rect.center().y;
    let path_color = crate::ui::tokens::DIAGRAM_MATERIAL;
    let lead_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::TEXT_MUTED;

    // Scale radius to fit
    let r_px = (radius as f32 * 3.0).clamp(15.0, 35.0);

    // Cut path: horizontal line across the middle
    let cut_left = rect.left() + 30.0;
    let cut_right = rect.right() - 30.0;
    painter.line_segment(
        [egui::pos2(cut_left, cy), egui::pos2(cut_right, cy)],
        egui::Stroke::new(2.0_f32, path_color),
    );

    // Lead-in arc (left side): quarter-circle approaching from below-left
    let in_center = egui::pos2(cut_left, cy - r_px); // arc center above entry point
    let mut in_pts = Vec::with_capacity(10);
    for i in 0..=8 {
        let t = i as f32 / 8.0;
        let a = std::f32::consts::FRAC_PI_2 * (1.0 - t); // 90° → 0°
        in_pts.push(egui::pos2(
            in_center.x - r_px * a.cos(),
            in_center.y + r_px * a.sin(),
        ));
    }
    painter.add(egui::Shape::line(
        in_pts,
        egui::Stroke::new(2.0_f32, lead_color),
    ));

    // Lead-out arc (right side): quarter-circle departing upward-right
    let out_center = egui::pos2(cut_right, cy - r_px);
    let mut out_pts = Vec::with_capacity(10);
    for i in 0..=8 {
        let t = i as f32 / 8.0;
        let a = std::f32::consts::FRAC_PI_2 * t; // 0° → 90°
        out_pts.push(egui::pos2(
            out_center.x + r_px * a.cos(),
            out_center.y + r_px * a.sin(),
        ));
    }
    painter.add(egui::Shape::line(
        out_pts,
        egui::Stroke::new(2.0_f32, lead_color),
    ));

    // Entry/exit markers
    painter.circle_filled(egui::pos2(cut_left, cy), 3.0, lead_color);
    painter.circle_filled(egui::pos2(cut_right, cy), 3.0, lead_color);

    // Labels
    painter.text(
        egui::pos2(cut_left - 6.0, cy + r_px * 0.3),
        egui::Align2::RIGHT_CENTER,
        "In",
        egui::FontId::proportional(9.0),
        lead_color,
    );
    painter.text(
        egui::pos2(cut_right + 6.0, cy - r_px * 0.3),
        egui::Align2::LEFT_CENTER,
        "Out",
        egui::FontId::proportional(9.0),
        lead_color,
    );

    // Radius annotation
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 6.0),
        egui::Align2::CENTER_BOTTOM,
        format!("r = {radius:.1} mm"),
        egui::FontId::proportional(8.0),
        dim_color,
    );
}

// ── Tab Placement Diagram ───────────────────────────────────────────────

/// Draw a simplified top-down perimeter with tab markers at even spacing.
pub(in crate::ui::properties) fn draw_tab_diagram(
    ui: &mut egui::Ui,
    tab_count: usize,
    tab_width: f64,
    tab_height: f64,
) {
    if tab_count == 0 {
        return;
    }

    let desired_size = egui::vec2(ui.available_width().min(260.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    // Perimeter rectangle
    let margin = 20.0;
    let pr = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin),
    );
    painter.rect_stroke(
        pr,
        2.0,
        egui::Stroke::new(1.5_f32, crate::ui::tokens::DIAGRAM_MATERIAL),
        egui::StrokeKind::Middle,
    );

    let tab_color = crate::ui::tokens::CAUTION;
    let perimeter = 2.0 * (pr.width() + pr.height());

    // Place tabs at even intervals around the perimeter
    let tab_marker_len = (tab_width as f32 / 50.0 * perimeter * 0.1).clamp(4.0, 20.0);
    for i in 0..tab_count {
        let t = i as f32 / tab_count as f32;
        let dist = t * perimeter;

        // Walk along the rectangle perimeter
        let (px, py, nx, ny) = if dist < pr.width() {
            // Top edge (left to right)
            (pr.left() + dist, pr.top(), 0.0, -1.0)
        } else if dist < pr.width() + pr.height() {
            // Right edge (top to bottom)
            let d = dist - pr.width();
            (pr.right(), pr.top() + d, 1.0, 0.0)
        } else if dist < 2.0 * pr.width() + pr.height() {
            // Bottom edge (right to left)
            let d = dist - pr.width() - pr.height();
            (pr.right() - d, pr.bottom(), 0.0, 1.0)
        } else {
            // Left edge (bottom to top)
            let d = dist - 2.0 * pr.width() - pr.height();
            (pr.left(), pr.bottom() - d, -1.0, 0.0)
        };

        // Draw tab marker (small line perpendicular to edge)
        painter.line_segment(
            [
                egui::pos2(px, py),
                egui::pos2(px + nx * tab_marker_len, py + ny * tab_marker_len),
            ],
            egui::Stroke::new(3.0_f32, tab_color),
        );
        // Small dot at the base
        painter.circle_filled(egui::pos2(px, py), 2.5, tab_color);
    }

    // Label
    let dim_color = crate::ui::tokens::TEXT_MUTED;
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        format!(
            "{tab_count} tab{} \u{00D7} {tab_width:.1}mm \u{00D7} {tab_height:.1}mm",
            if tab_count == 1 { "" } else { "s" }
        ),
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Outline Path Diagram (Profile, Chamfer, Trace, ProjectCurve) ────────

/// Draw a single perimeter outline with optional offset and direction arrows.
pub(in crate::ui::properties) fn draw_outline_diagram(
    ui: &mut egui::Ui,
    label: &str,
    offset_side: Option<&str>,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let margin = 20.0;
    let wp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + 16.0),
        egui::pos2(rect.right() - margin, rect.bottom() - 16.0),
    );

    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;

    // Main path outline
    painter.rect_stroke(
        wp,
        2.0,
        egui::Stroke::new(2.0_f32, path_color),
        egui::StrokeKind::Middle,
    );

    // Direction arrows (clockwise around the perimeter)
    let arrow_r = 3.0;
    let positions = [
        (wp.center_top() + egui::vec2(15.0, 0.0), true), // top, going right
        (wp.right_center() + egui::vec2(0.0, 10.0), true), // right, going down
        (wp.center_bottom() + egui::vec2(-15.0, 0.0), true), // bottom, going left
        (wp.left_center() + egui::vec2(0.0, -10.0), true), // left, going up
    ];
    for (pos, _) in &positions {
        painter.circle_filled(*pos, arrow_r, path_color);
    }

    // Offset indicator (if applicable)
    if let Some(side) = offset_side {
        let offset_dist = 6.0;
        // The offset ring is a second KIND of line beside the path, so it
        // takes a data-scale step rather than a hue of its own (§2.9).
        let [.., span_offset, _] = crate::ui::tokens::SPAN_SCALE;
        let offset_color = egui::Color32::from_rgba_unmultiplied(
            span_offset.r(),
            span_offset.g(),
            span_offset.b(),
            100,
        );
        let inset = if side == "Inside" {
            offset_dist
        } else {
            -offset_dist
        };
        let offset_rect = egui::Rect::from_min_max(
            egui::pos2(wp.left() + inset, wp.top() + inset),
            egui::pos2(wp.right() - inset, wp.bottom() - inset),
        );
        painter.rect_stroke(
            offset_rect,
            1.0,
            egui::Stroke::new(1.0_f32, offset_color),
            egui::StrokeKind::Middle,
        );
    }

    // Label
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Spiral Diagram (Adaptive, Adaptive3D, SpiralFinish) ─────────────────

/// Draw an Archimedean spiral pattern.
pub(in crate::ui::properties) fn draw_spiral_diagram(
    ui: &mut egui::Ui,
    stepover: f64,
    outward: bool,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 110.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let cx = rect.center().x;
    let cy = rect.center().y;
    let max_r = rect.width().min(rect.height()) * 0.4;
    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;

    // Workpiece boundary
    painter.rect_stroke(
        egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(max_r * 2.1, max_r * 2.1)),
        2.0,
        egui::Stroke::new(0.5_f32, crate::ui::tokens::HAIRLINE),
        egui::StrokeKind::Middle,
    );

    // Spiral: r = max_r * t, θ = turns * 2π * t
    let turns = 4.0_f32;
    let steps = (turns * 48.0) as usize;
    let mut pts = Vec::with_capacity(steps + 1);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let t_dir = if outward { t } else { 1.0 - t };
        let r = max_r * t_dir;
        let theta = turns * std::f32::consts::TAU * t;
        pts.push(egui::pos2(cx + r * theta.cos(), cy + r * theta.sin()));
    }
    painter.add(egui::Shape::line(
        pts,
        egui::Stroke::new(1.2_f32, path_color),
    ));

    // Center dot
    painter.circle_filled(egui::pos2(cx, cy), 2.5, path_color);

    // Labels
    let dir_label = if outward {
        "Inside \u{2192} Out"
    } else {
        "Outside \u{2192} In"
    };
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("Spiral ({dir_label})"),
        egui::FontId::proportional(9.0),
        dim_color,
    );
    painter.text(
        egui::pos2(rect.right() - 6.0, rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("step {stepover:.2} mm"),
        egui::FontId::proportional(8.0),
        dim_color,
    );
}

// ── Radial Spokes Diagram ───────────────────────────────────────────────

/// Draw radial lines from center at angular_step intervals.
pub(in crate::ui::properties) fn draw_radial_diagram(ui: &mut egui::Ui, angular_step: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 110.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let cx = rect.center().x;
    let cy = rect.center().y;
    let max_r = rect.width().min(rect.height()) * 0.4;
    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;

    let step_rad = (angular_step as f32).to_radians();
    let num_spokes = (std::f32::consts::TAU / step_rad.max(0.01)).ceil() as usize;
    let num_spokes = num_spokes.min(72); // cap for very small angular_step

    for i in 0..num_spokes {
        let angle = step_rad * i as f32;
        let end = egui::pos2(cx + max_r * angle.cos(), cy + max_r * angle.sin());
        painter.line_segment(
            [egui::pos2(cx, cy), end],
            egui::Stroke::new(1.0_f32, path_color),
        );
        // Alternating direction dots
        if i % 2 == 0 {
            let mid_r = max_r * 0.6;
            painter.circle_filled(
                egui::pos2(cx + mid_r * angle.cos(), cy + mid_r * angle.sin()),
                1.5,
                path_color,
            );
        }
    }

    painter.circle_filled(egui::pos2(cx, cy), 2.5, path_color);

    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("Radial ({angular_step:.0}\u{00B0} step)"),
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Point Set Diagram (Drill, AlignmentPinDrill) ────────────────────────

/// Draw scattered drill points.
pub(in crate::ui::properties) fn draw_point_set_diagram(ui: &mut egui::Ui, label: &str) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 70.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;

    // Scattered drill points (representative pattern)
    let positions = [
        (0.25, 0.35),
        (0.45, 0.55),
        (0.7, 0.3),
        (0.55, 0.7),
        (0.3, 0.65),
        (0.75, 0.6),
        (0.5, 0.4),
    ];
    for &(fx, fy) in &positions {
        let x = rect.left() + 20.0 + fx as f32 * (rect.width() - 40.0);
        let y = rect.top() + 14.0 + fy as f32 * (rect.height() - 28.0);
        // Crosshair at each point
        let s = 4.0;
        painter.line_segment(
            [egui::pos2(x - s, y), egui::pos2(x + s, y)],
            egui::Stroke::new(1.0_f32, path_color),
        );
        painter.line_segment(
            [egui::pos2(x, y - s), egui::pos2(x, y + s)],
            egui::Stroke::new(1.0_f32, path_color),
        );
        painter.circle_filled(egui::pos2(x, y), 2.0, path_color);
    }

    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Pencil Diagram ──────────────────────────────────────────────────────

/// Draw edge traces with parallel offset passes.
pub(in crate::ui::properties) fn draw_pencil_diagram(
    ui: &mut egui::Ui,
    num_offsets: usize,
    offset_step: f64,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let offset_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;
    let cy = rect.center().y;

    // Center crease line (wavy to represent edge detection)
    let mut center_pts = Vec::with_capacity(30);
    let x_start = rect.left() + 20.0;
    let x_end = rect.right() - 20.0;
    for i in 0..=24 {
        let t = i as f32 / 24.0;
        let x = x_start + t * (x_end - x_start);
        let wave = (t * 3.0 * std::f32::consts::TAU).sin() * 8.0;
        center_pts.push(egui::pos2(x, cy + wave));
    }
    painter.add(egui::Shape::line(
        center_pts.clone(),
        egui::Stroke::new(2.0_f32, path_color),
    ));

    // Offset passes
    let step_px = (offset_step as f32 * 2.0).clamp(4.0, 12.0);
    for pass in 1..=num_offsets.min(3) {
        let off = pass as f32 * step_px;
        for sign in [-1.0_f32, 1.0] {
            let offset_pts: Vec<_> = center_pts
                .iter()
                .map(|p| egui::pos2(p.x, p.y + sign * off))
                .collect();
            let alpha = (200 - pass * 40) as u8;
            painter.add(egui::Shape::line(
                offset_pts,
                egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgba_premultiplied(
                        offset_color.r(),
                        offset_color.g(),
                        offset_color.b(),
                        alpha,
                    ),
                ),
            ));
        }
    }

    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("Pencil ({num_offsets} offset passes)"),
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Steep/Shallow Diagram ───────────────────────────────────────────────

/// Draw a split-zone diagram showing steep (contour) vs shallow (raster) regions.
pub(in crate::ui::properties) fn draw_steep_shallow_diagram(ui: &mut egui::Ui, threshold: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let dim_color = crate::ui::tokens::DIAGRAM_DIM;
    // Steep and shallow are two CATEGORIES, so they take two steps of the
    // span scale (§2.9). They had collapsed onto one ink, which left the two
    // zone washes below as the only thing separating them.
    let [steep_color, .., shallow_color, _] = crate::ui::tokens::SPAN_SCALE;

    let margin = 14.0;
    let wp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + 16.0),
        egui::pos2(rect.right() - margin, rect.bottom() - 8.0),
    );

    // Diagonal divider based on threshold angle
    let frac = (threshold as f32 / 90.0).clamp(0.2, 0.8);
    let div_x = wp.left() + frac * wp.width();

    // Steep zone (left): contour rings
    let steep_rect = egui::Rect::from_min_max(wp.min, egui::pos2(div_x, wp.max.y));
    painter.rect_filled(
        steep_rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(
            steep_color.r(),
            steep_color.g(),
            steep_color.b(),
            20,
        ),
    );
    for i in 1..=3 {
        let inset = i as f32 * 6.0;
        if steep_rect.width() > inset * 2.0 + 4.0 && steep_rect.height() > inset * 2.0 + 4.0 {
            painter.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(steep_rect.left() + inset, steep_rect.top() + inset),
                    egui::pos2(steep_rect.right() - inset, steep_rect.bottom() - inset),
                ),
                1.0,
                egui::Stroke::new(0.8_f32, steep_color),
                egui::StrokeKind::Middle,
            );
        }
    }

    // Shallow zone (right): raster lines
    let shallow_rect = egui::Rect::from_min_max(egui::pos2(div_x, wp.min.y), wp.max);
    painter.rect_filled(
        shallow_rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(
            shallow_color.r(),
            shallow_color.g(),
            shallow_color.b(),
            20,
        ),
    );
    let line_step = 7.0;
    let mut y = shallow_rect.top() + line_step;
    while y < shallow_rect.bottom() - 2.0 {
        painter.line_segment(
            [
                egui::pos2(shallow_rect.left() + 2.0, y),
                egui::pos2(shallow_rect.right() - 2.0, y),
            ],
            egui::Stroke::new(0.8_f32, shallow_color),
        );
        y += line_step;
    }

    // Divider line
    painter.line_segment(
        [egui::pos2(div_x, wp.top()), egui::pos2(div_x, wp.bottom())],
        egui::Stroke::new(1.5_f32, dim_color),
    );

    // Labels
    painter.text(
        egui::pos2(steep_rect.center().x, wp.top() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "Steep",
        egui::FontId::proportional(8.0),
        steep_color,
    );
    painter.text(
        egui::pos2(shallow_rect.center().x, wp.top() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "Shallow",
        egui::FontId::proportional(8.0),
        shallow_color,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 3.0),
        egui::Align2::CENTER_TOP,
        format!("Threshold {threshold:.0}\u{00B0}"),
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Inlay Cross-Section Diagram ─────────────────────────────────────────

/// Draw a cross-section showing male/female inlay pocket mating.
/// Draw an assembly cross-section: female pocket in material with male plug
/// hovering above, about to drop in (flipped). Shows how the V-angles match.
pub(in crate::ui::properties) fn draw_inlay_diagram(
    ui: &mut egui::Ui,
    pocket_depth: f64,
    glue_gap: f64,
    flat_depth: f64,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let dim_color = crate::ui::tokens::DIAGRAM_DIM;
    // The female pocket and the male plug are two CATEGORIES. They had
    // collapsed onto one ink, so the plug fill below was the only thing
    // telling them apart. Two steps of the span scale (§2.9) instead.
    let female_color = crate::ui::tokens::DIAGRAM_INK;
    let [.., male_color, _] = crate::ui::tokens::SPAN_SCALE;
    let mat_color = crate::ui::tokens::DIAGRAM_DIM;

    let cx = rect.center().x;
    let total_depth = pocket_depth.max(flat_depth).max(1.0);
    let scale = (rect.height() * 0.3) / total_depth as f32;
    let half_w = 40.0;

    // Surface line divides upper (air + plug) from lower (material + pocket)
    let surface_y = rect.center().y + 4.0;
    let pocket_d = pocket_depth as f32 * scale;
    let flat_d = flat_depth as f32 * scale;
    let gap_px = (glue_gap as f32 * scale).max(2.0);

    // Material block (below surface)
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left() + 8.0, surface_y),
            egui::pos2(rect.right() - 8.0, rect.bottom() - 6.0),
        ),
        0.0,
        mat_color,
    );
    // Surface line
    painter.line_segment(
        [
            egui::pos2(rect.left() + 8.0, surface_y),
            egui::pos2(rect.right() - 8.0, surface_y),
        ],
        egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_MATERIAL),
    );

    // Female pocket (V cavity cut into material)
    let pocket_pts = vec![
        egui::pos2(cx - half_w, surface_y),
        egui::pos2(cx, surface_y + pocket_d),
        egui::pos2(cx + half_w, surface_y),
    ];
    // Clear the pocket area
    painter.add(egui::Shape::convex_polygon(
        pocket_pts.clone(),
        crate::ui::tokens::DIAGRAM_CANVAS,
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::line(
        pocket_pts,
        egui::Stroke::new(1.5_f32, female_color),
    ));

    // Male plug (flipped V, hovering above the pocket, about to drop in)
    // The plug is the same V shape but inverted, with a flat bottom cut off
    let plug_bottom = surface_y - gap_px; // hover just above the surface
    let plug_top = plug_bottom - flat_d;
    // V shape going up: wide at bottom, narrow at top (but with flat top)
    let flat_hw = half_w * (1.0 - flat_d / pocket_d.max(0.1)).max(0.1);
    let plug_pts = vec![
        egui::pos2(cx - half_w, plug_bottom),
        egui::pos2(cx - flat_hw, plug_top),
        egui::pos2(cx + flat_hw, plug_top),
        egui::pos2(cx + half_w, plug_bottom),
    ];
    // Fill the plug
    painter.add(egui::Shape::convex_polygon(
        plug_pts.clone(),
        egui::Color32::from_rgba_unmultiplied(male_color.r(), male_color.g(), male_color.b(), 40),
        egui::Stroke::NONE,
    ));
    // SAFETY: plug_pts always has exactly 4 elements, constructed on the lines above.
    #[allow(clippy::indexing_slicing)]
    let plug_outline = vec![
        plug_pts[0],
        plug_pts[1],
        plug_pts[2],
        plug_pts[3],
        plug_pts[0],
    ];
    painter.add(egui::Shape::line(
        plug_outline,
        egui::Stroke::new(1.5_f32, male_color),
    ));

    // Drop arrow (shows the plug goes down into the pocket)
    let arrow_x = cx + half_w + 12.0;
    let arrow_top = plug_top;
    let arrow_bottom = surface_y + 4.0;
    painter.line_segment(
        [
            egui::pos2(arrow_x, arrow_top),
            egui::pos2(arrow_x, arrow_bottom),
        ],
        egui::Stroke::new(1.0_f32, dim_color),
    );
    painter.add(egui::Shape::line(
        vec![
            egui::pos2(arrow_x - 3.0, arrow_bottom - 6.0),
            egui::pos2(arrow_x, arrow_bottom),
            egui::pos2(arrow_x + 3.0, arrow_bottom - 6.0),
        ],
        egui::Stroke::new(1.0_f32, dim_color),
    ));

    // Glue gap annotation
    painter.text(
        egui::pos2(cx - half_w - 4.0, (plug_bottom + surface_y) / 2.0),
        egui::Align2::RIGHT_CENTER,
        format!("gap {glue_gap:.2}"),
        egui::FontId::proportional(7.0),
        dim_color,
    );

    // Depth annotations on right
    let dim_x = rect.right() - 30.0;
    painter.text(
        egui::pos2(dim_x, surface_y + pocket_d * 0.5),
        egui::Align2::LEFT_CENTER,
        format!("{pocket_depth:.1} deep"),
        egui::FontId::proportional(7.0),
        female_color,
    );

    // Labels
    painter.text(
        egui::pos2(cx, plug_top - 4.0),
        egui::Align2::CENTER_BOTTOM,
        "Plug (flipped)",
        egui::FontId::proportional(8.0),
        male_color,
    );
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        "Inlay Assembly",
        egui::FontId::proportional(9.0),
        dim_color,
    );
}

// ── Ramp Finish Diagram ─────────────────────────────────────────────────

/// Side-view showing contour levels connected by helical ramps.
pub(in crate::ui::properties) fn draw_ramp_finish_diagram(ui: &mut egui::Ui, max_stepdown: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, crate::ui::tokens::DIAGRAM_CANVAS);

    let path_color = crate::ui::tokens::DIAGRAM_INK;
    let dim_color = crate::ui::tokens::DIAGRAM_DIM;
    // The ramp leg is a second KIND of move beside the contour, so it takes a
    // span-scale step rather than a hue of its own (§2.9).
    let [.., ramp_color, _] = crate::ui::tokens::SPAN_SCALE;

    let num_levels = 4;
    let x_start = rect.left() + 20.0;
    let x_end = rect.right() - 20.0;
    let y_top = rect.top() + 16.0;
    let y_bottom = rect.bottom() - 12.0;
    let level_step = (y_bottom - y_top) / (num_levels - 1) as f32;

    // Contour levels (horizontal lines) connected by diagonal ramps
    for i in 0..num_levels {
        let y = y_top + i as f32 * level_step;
        // Contour at this Z level
        painter.line_segment(
            [egui::pos2(x_start, y), egui::pos2(x_end, y)],
            egui::Stroke::new(1.5_f32, path_color),
        );
        // Ramp down to next level
        if i < num_levels - 1 {
            let next_y = y + level_step;
            painter.line_segment(
                [egui::pos2(x_end, y), egui::pos2(x_start, next_y)],
                egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgba_unmultiplied(
                        ramp_color.r(),
                        ramp_color.g(),
                        ramp_color.b(),
                        120,
                    ),
                ),
            );
        }
    }

    // Stepdown dimension
    painter.line_segment(
        [
            egui::pos2(x_end + 8.0, y_top),
            egui::pos2(x_end + 8.0, y_top + level_step),
        ],
        egui::Stroke::new(1.0_f32, dim_color),
    );
    painter.text(
        egui::pos2(x_end + 10.0, y_top + level_step / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{max_stepdown:.1}"),
        egui::FontId::proportional(8.0),
        dim_color,
    );

    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        "Ramp Finish (side view)",
        egui::FontId::proportional(9.0),
        dim_color,
    );
}
