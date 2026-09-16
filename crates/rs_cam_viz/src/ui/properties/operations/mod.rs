mod boundary_2d;
mod drill;
mod engrave;
mod finishing;
mod project;
mod surface_3d;

// Re-export all draw_*_params functions so callers don't need to change imports.
pub(super) use boundary_2d::{
    draw_adaptive_params, draw_face_params, draw_inlay_params, draw_pocket_params,
    draw_profile_params, draw_rest_params, draw_vcarve_params, draw_zigzag_params,
};
pub(super) use drill::{draw_alignment_pin_drill_params, draw_drill_params};
pub(super) use engrave::{draw_chamfer_params, draw_trace_params};
pub(super) use finishing::{
    draw_horizontal_finish_params, draw_radial_finish_params, draw_ramp_finish_params,
    draw_spiral_finish_params,
};
pub(super) use project::draw_project_curve_params;
pub(super) use surface_3d::{
    draw_adaptive3d_params, draw_dropcutter_params, draw_pencil_params, draw_scallop_params,
    draw_steep_shallow_params, draw_unified_finish_params, draw_waterline_params,
};

// F1.18: `DepthSemantics` left with the GUI's copy of the
// depth-beyond-stock rule. The applicability test lives in core now.
use rs_cam_core::diagnostics::adapters::from_static_checks as core_static_checks;

use crate::state::job::ToolType;
use crate::state::rest_dependency::{RestCandidate, rest_predecessors};
use crate::state::toolpath::{
    HeightContext, HeightMode, HeightReference, HeightsConfig, OperationConfig, OperationType,
    PocketPattern, ProfileConfig, ReferenceOffset, ToolpathEntry, ToolpathId,
};

// Feed / plunge editing moved off the per-op Geometry panel into the
// Feeds & Speeds tab's SPEED section (W3.2) — the `draw_feed_params` helper
// (and its `FeedsResult` / `dv_pill` deps) was retired with the recharter.
//
// The per-operation spindle RPM override moved to the Feeds-card SPEED section
// (W3.1) as a `PrecedenceField`, where the project default RPM is reachable and
// the override-vs-default precedence renders honestly. The old in-place
// `draw_spindle_rpm_row` (hardcoded 18 000 default) was retired.

// ── Heights panel ────────────────────────────────────────────────────────

/// How one height row is *rendered*: every mode is shown as an offset from a
/// named reference, including `Auto` and `Manual`.
///
/// This projection is a **pure read** — the signature takes `&HeightMode` so it
/// cannot write, and that is the whole point.
///
/// G-HEIGHTSTAB (2026-08-23): the row used to promote `Auto` →
/// `FromReference(default)` and `Manual` → `FromReference(nearest)` at the top
/// of its *draw* function, so merely rendering the Heights tab rewrote the
/// stored config. The panel's write-back (`write_entry_config_to_session`) then
/// saw `heights` change, stamped `stale_since`, and auto-regeneration took the
/// op apart. On any operation whose `default_depth_for_heights()` is 0 — the
/// entire 3D family, which reports `DepthSemantics::None` — the promoted bottom
/// row became `FromReference { StockTop, -0.0 }`: bottom == stock top, and the
/// promotion also flipped `ResolvedHeights::bottom_pinned` from false to true,
/// so the operation honoured a floor at the stock top and regenerated to **zero
/// moves**. Opening a read-only tab destroyed a healthy 7 672-move toolpath.
///
/// Nothing here may write to `mode`; see [`commit_height_row`] for the one
/// place that does, and what gates it.
fn height_row_display(
    mode: &HeightMode,
    default_ref: HeightReference,
    default_z: f64,
    ctx: &HeightContext,
) -> ReferenceOffset {
    match *mode {
        HeightMode::FromReference(ref_offset) => ref_offset,
        // `default_z` is the absolute Z the CORE resolver produces for this
        // row, so the displayed value can never disagree with what generation
        // actually does.
        HeightMode::Auto => ReferenceOffset {
            reference: default_ref,
            offset: default_z - default_ref.resolve_z(ctx),
        },
        HeightMode::Manual(abs_z) => {
            let best_ref = find_nearest_reference(abs_z, ctx);
            ReferenceOffset {
                reference: best_ref,
                offset: abs_z - best_ref.resolve_z(ctx),
            }
        }
    }
}

/// Write an edited row back to the stored mode.
///
/// `edited` must be true only when the user moved a widget **this frame**
/// (a DragValue change or a reference pick). Rendering alone never sets it, so
/// viewing the Heights tab leaves `Auto` as `Auto` — which is what keeps
/// `bottom_pinned` false and the operation's own depth semantics in charge.
fn commit_height_row(mode: &mut HeightMode, display: ReferenceOffset, edited: bool) {
    if edited {
        *mode = HeightMode::FromReference(display);
    }
}

/// F360-style height row: [offset value] [from Reference ▾]
///
/// `default_z` is the absolute Z the core resolver produces for this row (see
/// [`height_row_display`]). An `Auto` row displays that value and stays `Auto`
/// until the user actually edits the row.
#[allow(clippy::too_many_arguments)]
fn draw_height_row(
    ui: &mut egui::Ui,
    label: &str,
    tooltip: &str,
    mode: &mut HeightMode,
    default_ref: HeightReference,
    default_z: f64,
    ctx: &HeightContext,
    id_salt: &str,
) {
    let was_auto = mode.is_auto();
    let mut display = height_row_display(mode, default_ref, default_z, ctx);

    ui.label(label).on_hover_text(tooltip);

    let mut edited = ui
        .add(
            egui::DragValue::new(&mut display.offset)
                .suffix(" mm")
                .speed(0.5)
                .range(-500.0..=500.0),
        )
        .changed();

    let combo = egui::ComboBox::from_id_salt(format!("hr_{id_salt}"))
        .width(105.0)
        .selected_text(ref_label(display.reference, display.offset))
        .show_ui(ui, |ui| {
            let mut picked = false;
            for &href in HeightReference::ALL {
                // `.clicked()` rather than `.changed()`: re-picking the
                // reference an Auto row is merely *displaying* is still an
                // explicit "pin it here" from the operator.
                picked |= ui
                    .selectable_value(&mut display.reference, href, href.label())
                    .clicked();
            }
            picked
        });
    edited |= combo.inner.unwrap_or(false);

    commit_height_row(mode, display, edited);

    // Resolved absolute Z as a dim hint. "(auto)" marks a row that is still
    // deferring to the resolver rather than carrying a pinned value.
    let resolved = display.reference.resolve_z(ctx) + display.offset;
    let hint = if was_auto && !edited {
        format!("= {resolved:.1} (auto)")
    } else {
        format!("= {resolved:.1}")
    };
    ui.label(
        egui::RichText::new(hint)
            .small()
            .color(crate::ui::tokens::DIAGRAM_DIM),
    );

    ui.end_row();
}

/// Descriptive label for the reference dropdown: "above/below Stock Top" etc.
fn ref_label(reference: HeightReference, offset: f64) -> String {
    let dir = if offset >= 0.0 { "above" } else { "below" };
    format!("{dir} {}", reference.label())
}

/// Find the nearest reference point to an absolute Z value.
fn find_nearest_reference(z: f64, ctx: &HeightContext) -> HeightReference {
    let mut best = HeightReference::StockTop;
    let mut best_dist = f64::INFINITY;
    for &href in HeightReference::ALL {
        let ref_z = href.resolve_z(ctx);
        let dist = (z - ref_z).abs();
        if dist < best_dist {
            best_dist = dist;
            best = href;
        }
    }
    best
}

/// F1.19 / G-BOTTOMPIN: the sentence the Heights tab prints beside the Bottom
/// row when the operation does not read a pinned Bottom Z.
///
/// `None` means the pin DOES drive the cut, so the row is offered plain.
/// `Some(note)` names the dial that really sets the floor.
///
/// The core half measured the fact and declared it as
/// [`rs_cam_core::compute::catalog::OperationType::honors_pinned_bottom_z`]:
/// three of the twenty-four operations read `heights.bottom_z`, and on the
/// other twenty-one a pinned bottom reaches no emitted motion. This match
/// carries its own arm per operation because each family names a DIFFERENT
/// dial, and the sentry
/// (`crates/rs_cam_viz/tests/bottom_z_pin_note_g_bottompin.rs`) ties the two
/// enumerations together so they cannot drift.
///
/// The panel ANNOTATES rather than disables. A disabled row cannot be set
/// back to Auto, and a legacy project can carry a pin that puts the resolved
/// bottom above the top — which the Heights badge and the core
/// `geom.bottom_above_top_z` check both report. The operator must keep the
/// one control that clears it.
pub fn bottom_z_pin_note(op_type: OperationType) -> Option<&'static str> {
    use crate::state::toolpath::OperationType as Op;

    const DEPTH: &str = "Not used by this operation. Its Depth field sets the floor.";
    const MAX_DEPTH: &str = "Not used by this operation. Its Max Depth sets the floor.";
    const POCKET_DEPTH: &str = "Not used by this operation. Its Pocket Depth sets the floor.";
    const WIDTH: &str = "Not used by this operation. Its Chamfer Width sets the floor.";
    const PIN_DRILL: &str = "Not used by this operation. Its Depth and Spoilboard set the floor.";
    const CURVE: &str = "Not used by this operation. The projected surface sets the floor.";
    const SURFACE: &str = "Not used by this operation. The model surface sets the floor.";

    // One arm per operation. The `match` is exhaustive, so a new operation
    // cannot be added to the catalog without an answer here, and the sentry
    // ties every answer to `honors_pinned_bottom_z()`.
    match op_type {
        // The three that read `heights.bottom_z`. No note.
        Op::Adaptive3d | Op::UnifiedFinish | Op::Waterline => None,
        Op::Face => Some(DEPTH),
        Op::Pocket => Some(DEPTH),
        Op::Profile => Some(DEPTH),
        Op::Adaptive => Some(DEPTH),
        Op::Rest => Some(DEPTH),
        Op::Zigzag => Some(DEPTH),
        Op::Trace => Some(DEPTH),
        Op::Drill => Some(DEPTH),
        Op::VCarve => Some(MAX_DEPTH),
        Op::Inlay => Some(POCKET_DEPTH),
        Op::Chamfer => Some(WIDTH),
        Op::AlignmentPinDrill => Some(PIN_DRILL),
        Op::ProjectCurve => Some(CURVE),
        Op::DropCutter => Some(SURFACE),
        Op::Scallop => Some(SURFACE),
        Op::Pencil => Some(SURFACE),
        Op::HorizontalFinish => Some(SURFACE),
        Op::SteepShallow => Some(SURFACE),
        Op::RampFinish => Some(SURFACE),
        Op::SpiralFinish => Some(SURFACE),
        Op::RadialFinish => Some(SURFACE),
    }
}

/// The Bottom row tooltip for an operation that DOES read the pin.
const BOTTOM_Z_TOOLTIP: &str = "Deepest cut depth. The tool stops at this Z.";

pub(super) fn draw_heights_params(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
    op_type: OperationType,
) {
    // What an `Auto` row shows is the value the CORE resolver produces for
    // THIS config — not a second set of defaults maintained here. The panel
    // used to carry its own (feed_z defaulted to `stock_top + 2` while the
    // resolver's is `retract - 2`), and because the panel then wrote its
    // defaults into the config on render, the divergence was a silent edit
    // rather than a visible disagreement. Rows other than `Auto` ignore this
    // value and display their own stored offset / absolute Z.
    let auto = heights.resolve(ctx);

    // F1.19 / G-BOTTOMPIN: the Bottom row's own tooltip used to say that
    // pinning the row overrides the operation's floor. That is true on three
    // operations and false on twenty-one. The note replaces the claim on the
    // twenty-one and names the dial that does set the floor.
    let pin_note = bottom_z_pin_note(op_type);
    let bottom_tooltip = match pin_note {
        Some(note) => format!("{note} The row still shows the stored value."),
        None => BOTTOM_Z_TOOLTIP.to_owned(),
    };

    egui::Grid::new("heights_p")
        .num_columns(4)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            draw_height_row(
                ui,
                "Clearance:",
                "Highest safe height. Rapid moves between separate operations travel at this Z.",
                &mut heights.clearance_z,
                HeightReference::StockTop,
                auto.clearance_z,
                ctx,
                "h_clear",
            );
            draw_height_row(
                ui,
                "Retract:",
                "Rapid travel height within an operation. Tool retracts here between cutting passes.",
                &mut heights.retract_z,
                HeightReference::StockTop,
                auto.retract_z,
                ctx,
                "h_retract",
            );
            draw_height_row(
                ui,
                "Feed:",
                "Approach height. Tool switches from rapid to feed rate here before plunging into material.",
                &mut heights.feed_z,
                HeightReference::StockTop,
                auto.feed_z,
                ctx,
                "h_feed",
            );
            draw_height_row(
                ui,
                "Top:",
                "Top of material. Cutting starts at this Z. Usually the stock top surface.",
                &mut heights.top_z,
                HeightReference::StockTop,
                auto.top_z,
                ctx,
                "h_top",
            );
            draw_height_row(
                ui,
                "Bottom:",
                &bottom_tooltip,
                &mut heights.bottom_z,
                HeightReference::StockTop,
                auto.bottom_z,
                ctx,
                "h_bottom",
            );
        });

    // F1.19 / G-BOTTOMPIN: the note is drawn, not hidden behind a hover. The
    // row above stays editable, so an operator can still read a stored pin
    // and set it back to auto.
    if let Some(note) = pin_note {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(format!("Bottom: {note}"))
                .small()
                .italics()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    }
}

// ── Stepover Pattern Diagram ─────────────────────────────────────────────

/// Whether an operation has a displayable stepover pattern.
pub(super) enum StepoverPattern {
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
pub(super) fn draw_stepover_diagram(ui: &mut egui::Ui, pattern: &StepoverPattern) {
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
pub(super) fn draw_dogbone_diagram(ui: &mut egui::Ui, max_angle: f64) {
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
pub(super) fn draw_lead_in_out_diagram(ui: &mut egui::Ui, radius: f64) {
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
pub(super) fn draw_tab_diagram(
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
pub(super) fn draw_outline_diagram(ui: &mut egui::Ui, label: &str, offset_side: Option<&str>) {
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
pub(super) fn draw_spiral_diagram(ui: &mut egui::Ui, stepover: f64, outward: bool) {
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
pub(super) fn draw_radial_diagram(ui: &mut egui::Ui, angular_step: f64) {
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
pub(super) fn draw_point_set_diagram(ui: &mut egui::Ui, label: &str) {
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
pub(super) fn draw_pencil_diagram(ui: &mut egui::Ui, num_offsets: usize, offset_step: f64) {
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
pub(super) fn draw_steep_shallow_diagram(ui: &mut egui::Ui, threshold: f64) {
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
pub(super) fn draw_inlay_diagram(
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
pub(super) fn draw_ramp_finish_diagram(ui: &mut egui::Ui, max_stepdown: f64) {
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

// ── 2D Height Diagram ───────────────────────────────────────────────────

/// Height line definition for diagram rendering and interaction.
struct DiagramLine {
    z: f64,
    color: egui::Color32,
    /// The text drawn beside the line. It carries the value, except on a
    /// Bottom the operation does not read: there it names the state, and the
    /// value stays in the Bottom field above (ruling R33).
    text: String,
    /// Which field index (0..5) for drag targeting.
    index: usize,
}

/// The `DiagramLine::index` of the Bottom row. The drag handler and the
/// R33 hover both name it.
const BOTTOM_LINE_INDEX: usize = 4;

/// What the height diagram can honestly say about the model's Z profile.
///
/// # The defect this names (F-1, 2026-09-14)
///
/// The diagram drew the model as a rectangle between `ctx.model_top_z` and
/// `ctx.model_bottom_z`, and drew nothing at all when either was `None`.
/// Both branches were wrong on the wanaka project, and in different ways.
///
/// **A 2D model carries no Z.** `rs_cam_core::session::polygons_bbox`
/// reports an SVG or DXF bbox at
/// `z = 0.0 ..= 0.0`, so an operation bound to a drawing got a model
/// rectangle of ZERO height. epaint collapses such a rectangle to a
/// one-point line in the STROKE colour (`Tessellator::tessellate_rect`
/// turns a rect thinner than the feathering into a line segment), and that
/// stroke was `DIAGRAM_MATERIAL` — a dark grey on the dark canvas. The
/// operator therefore read "no model" on operations 2 to 5 of setup 1,
/// which are bound to the drawings, while operation 1, which is bound to
/// `terrain.stl` and has a real Z extent, drew correctly.
///
/// **A missing profile is a different thing.** It is NOT MEASURED, and this
/// repo never renders an absent value as a clean result. The two cases must
/// not share one blank frame, so they are two variants here and the drawing
/// states which one it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModelProfile {
    /// The model carries a Z extent. The diagram draws a box.
    Solid,
    /// The model is measured and its Z extent is zero. A 2D drawing sits on
    /// one plane, so the honest profile is a rule, not a box.
    Flat,
    /// No model profile reached the diagram. NOT MEASURED.
    Absent,
}

/// A model Z extent at or below this reads as flat. A 2D drawing reports
/// exactly zero, so the value only has to exclude arithmetic noise from the
/// setup transform.
const MODEL_FLAT_EPS_MM: f64 = 1.0e-6;

/// The canvas height in points. The width follows the panel.
const DIAGRAM_HEIGHT: f32 = 180.0;
/// The widest canvas. A wider panel gives the diagram no more to say.
const DIAGRAM_MAX_WIDTH: f32 = 260.0;
/// The narrowest canvas that still holds its own labels. The inspector is
/// 280 points wide (`app.rs`, `default_size`), so this floor never binds
/// there. It keeps the layout total rather than letting a label escape.
const DIAGRAM_MIN_WIDTH: f32 = 160.0;
/// The label column never takes more than this share of the canvas, so a
/// long value cannot push the height lines out of sight.
const LABEL_COLUMN_MAX_FRACTION: f32 = 0.45;
/// The legend swatch length in points.
const LEGEND_SWATCH_W: f32 = 10.0;
/// Stroke widths for a height line, a hovered height line, a legend swatch
/// and a flat model profile.
const LINE_STROKE: f32 = 1.5;
const LINE_STROKE_HOVER: f32 = 2.5;
const SWATCH_STROKE: f32 = 2.0;
const FLAT_PROFILE_STROKE: f32 = 2.0;
/// Half-widths of the stock and model boxes, as a share of the body.
const STOCK_HALF_WIDTH_FRACTION: f32 = 0.275;
const MODEL_HALF_WIDTH_FRACTION: f32 = 0.2;

/// Classify the model profile a [`HeightContext`] carries.
///
/// This is a pure read of the context, kept apart from the drawing, so the
/// sentry asserts the DECISION rather than a pixel.
#[must_use]
pub fn model_profile(ctx: &HeightContext) -> ModelProfile {
    match (ctx.model_top_z, ctx.model_bottom_z) {
        (Some(top), Some(bottom)) => {
            if (top - bottom).abs() <= MODEL_FLAT_EPS_MM {
                ModelProfile::Flat
            } else {
                ModelProfile::Solid
            }
        }
        _ => ModelProfile::Absent,
    }
}

/// Draw an interactive 2D side-view diagram showing stock, model, and height planes.
///
/// # F-2 (2026-09-14): the canvas holds its content
///
/// Every label used to sit at a hardcoded offset — the value column at
/// `rect.right() - 42.0` and the legend at `rect.bottom() - 8.0` — and
/// `ui.painter_at(rect)` then cut off whatever did not fit. The operator saw
/// `Stock` and `Model` sliced at the container edge. Rule F of
/// `planning/ui_declutter_2026-09-14/PLAN.md` says a container sizes to hold
/// its content, so this function now LAYS OUT every text run first, takes
/// the width and the height those runs need, and clamps each run inside the
/// allocated rect. `painter_at` stays as the backstop, and nothing should
/// reach it. The sentry is
/// `crates/rs_cam_viz/tests/the_heights_diagram_fits_f2.rs`.
///
/// # F-6 / ruling R33 (2026-09-14): the diagram agrees with the panel
///
/// The diagram drew a `DANGER` red floor at `resolved.bottom_z` on every
/// operation. `OperationType::honors_pinned_bottom_z` says three of the
/// twenty-four read that dial, so on the other twenty-one the panel printed
/// "Bottom: Not used by this operation. …" and the diagram under it drew a
/// red floor. Two surfaces gave opposite answers about where the cut stops,
/// and red is the strongest mark on this canvas.
///
/// The line STAYS. The operator set that value, and a diagram that drops it
/// invites the reading "the pin did not take". What changes is the mark: an
/// inert Bottom reads `UNKNOWN`, its text names the state, and the reason
/// arrives on hover in `bottom_z_pin_note`'s own words, so the two surfaces
/// cannot fork their vocabulary.
///
/// ## Why the inert run carries no value
///
/// Do not "fix" `BZ (not used)` back into `BZ 26.0 (not used)`.
///
/// This diagram is a picture of CUT LIMITS. Every other run on it names a Z
/// that bounds the cut. On an operation that does not read
/// `heights.bottom_z`, the pin is not a cut limit: the operation floors at
/// `top_z` minus its own depth dial. A number printed beside four real
/// limits reads as operative, whatever follows it in brackets, so it would
/// re-assert the one thing this ruling exists to deny. The line's POSITION
/// shows that the pin exists and where it sits. The value belongs to the
/// Bottom field above, which is the surface about the SETTING rather than
/// about the cut. Those are two questions, and they now have two homes.
///
/// A second mechanism agrees. `LABEL_COLUMN_MAX_FRACTION` feeds
/// `painter.layout` as a WRAP width, not a truncation cap, so
/// `BZ 26.0 (not used)` would not clip. It would wrap to two rows in a
/// five-row diagram, and then meet the label-collision push below.
pub fn draw_height_diagram(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
    op_type: OperationType,
) {
    use crate::ui::tokens;

    let resolved = heights.resolve(ctx);
    let profile = model_profile(ctx);

    // R33. One fact — `honors_pinned_bottom_z` — drives the colour and the
    // text, and the same fact drives `bottom_z_pin_note` in the panel above.
    // The inert run names the state and carries NO value: this diagram
    // pictures cut limits, and an inert pin is not one. See the ruling in
    // this function's doc comment before changing the text.
    let bottom_drives_the_cut = op_type.honors_pinned_bottom_z();
    let (bottom_color, bottom_text) = if bottom_drives_the_cut {
        (tokens::DANGER, format!("BZ {:.1}", resolved.bottom_z))
    } else {
        (tokens::UNKNOWN, "BZ (not used)".to_owned())
    };

    // Build line definitions (ordered top to bottom for rendering)
    let lines = [
        DiagramLine {
            z: resolved.clearance_z,
            color: tokens::DIAGRAM_INK,
            text: format!("CZ {:.1}", resolved.clearance_z),
            index: 0,
        },
        DiagramLine {
            z: resolved.retract_z,
            color: tokens::DIAGRAM_INK,
            text: format!("RZ {:.1}", resolved.retract_z),
            index: 1,
        },
        DiagramLine {
            z: resolved.feed_z,
            color: tokens::OK,
            text: format!("FZ {:.1}", resolved.feed_z),
            index: 2,
        },
        DiagramLine {
            z: resolved.top_z,
            color: tokens::CAUTION,
            text: format!("TZ {:.1}", resolved.top_z),
            index: 3,
        },
        DiagramLine {
            z: resolved.bottom_z,
            color: bottom_color,
            text: bottom_text,
            index: BOTTOM_LINE_INDEX,
        },
    ];

    // Compute Z range with margin.
    //
    // The model extents belong in the range. The diagram DRAWS the model,
    // so a model above or below every height would map outside the plot and
    // paint over the canvas edge. With every drawn Z inside the range, and a
    // margin on each side, `z_to_y` is total: it cannot leave the plot.
    let mut all_z_values = vec![
        resolved.clearance_z,
        resolved.retract_z,
        resolved.feed_z,
        resolved.top_z,
        resolved.bottom_z,
        ctx.stock_top_z,
        ctx.stock_bottom_z,
    ];
    all_z_values.extend(ctx.model_top_z);
    all_z_values.extend(ctx.model_bottom_z);
    let z_min_raw = all_z_values.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let z_max_raw = all_z_values
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(10.0);
    let z_span = (z_max_raw - z_min_raw).max(1.0);
    let margin = z_span * 0.12;
    let z_min = z_min_raw - margin;
    let z_max = z_max_raw + margin;

    // Canvas
    let avail = ui.available_width();
    let width = avail.clamp(DIAGRAM_MIN_WIDTH, DIAGRAM_MAX_WIDTH);
    let desired_size = egui::vec2(width, DIAGRAM_HEIGHT);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, tokens::RADIUS_SM, tokens::DIAGRAM_CANVAS);

    let inner = rect.shrink(tokens::SPACE_2);

    // ── The legend, measured before the plot takes its height ───────────
    //
    // The legend names the two boxes, and on a flat profile it says so. It
    // is laid out first because the band it needs comes off the plot. The
    // canvas therefore holds the legend instead of drawing it over the
    // stock or off the bottom edge.
    let legend_font = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Proportional);
    let legend_wrap = (inner.width() - LEGEND_SWATCH_W - tokens::SPACE_1).max(1.0);
    let legend_text = |text: &str| {
        painter.layout(
            text.to_owned(),
            legend_font.clone(),
            tokens::TEXT_MUTED,
            legend_wrap,
        )
    };
    let mut legend: Vec<(egui::Color32, std::sync::Arc<egui::Galley>)> = Vec::new();
    legend.push((tokens::DIAGRAM_MATERIAL, legend_text("Stock")));
    match profile {
        ModelProfile::Solid => legend.push((tokens::DIAGRAM_DIM, legend_text("Model"))),
        ModelProfile::Flat => legend.push((tokens::DIAGRAM_DIM, legend_text("Model (flat)"))),
        // An absent profile gets no swatch, because there is no shape to
        // key. It is stated under the canvas instead, as NOT MEASURED.
        ModelProfile::Absent => {}
    }

    let mut legend_row_h = 0.0_f32;
    let mut legend_items_w = 0.0_f32;
    for (_, galley) in &legend {
        legend_row_h = legend_row_h.max(galley.size().y);
        legend_items_w += LEGEND_SWATCH_W + tokens::SPACE_1 + galley.size().x;
    }
    let legend_gaps = legend.len().saturating_sub(1) as f32;
    let legend_one_row_w = legend_items_w + tokens::SPACE_3 * legend_gaps;
    let legend_stacked = legend_one_row_w > inner.width();
    let legend_row_count = if legend_stacked { legend.len() } else { 1 };
    let legend_rows = legend_row_count as f32;
    let legend_h = legend_row_h * legend_rows + tokens::SPACE_1 * (legend_rows - 1.0);

    // ── The plot area ───────────────────────────────────────────────────
    let plot_bottom = inner.bottom() - legend_h - tokens::SPACE_2;
    let plot_bottom = plot_bottom.max(inner.top() + 1.0);
    let plot = egui::Rect::from_min_max(inner.min, egui::pos2(inner.right(), plot_bottom));

    // Coordinate mapping: Z → screen Y (higher Z = higher on screen = lower Y)
    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        plot.bottom() - (frac as f32) * plot.height()
    };

    // ── The value labels, measured before the lines are drawn ───────────
    //
    // The column takes the width the widest label needs, capped so the
    // lines keep the majority of the canvas. Every label is laid out with a
    // wrap width, so no run can be wider than the column it sits in.
    let value_font = egui::FontId::new(tokens::SIZE_MICRO, egui::FontFamily::Monospace);
    let label_wrap = (plot.width() * LABEL_COLUMN_MAX_FRACTION).max(1.0);
    let mut labels: Vec<std::sync::Arc<egui::Galley>> = Vec::new();
    let mut label_w = 0.0_f32;
    for line in &lines {
        let galley = painter.layout(
            line.text.clone(),
            value_font.clone(),
            line.color,
            label_wrap,
        );
        label_w = label_w.max(galley.size().x);
        labels.push(galley);
    }
    label_w = label_w.min(label_wrap);
    let label_x = (plot.right() - label_w).max(plot.left());
    let line_right = (label_x - tokens::SPACE_1).max(plot.left());

    // The body is the part of the plot the boxes may use: everything left
    // of the label column.
    let body = egui::Rect::from_min_max(plot.min, egui::pos2(line_right, plot.bottom()));

    // Stock rectangle (centered in the body)
    let stock_hw = body.width() * STOCK_HALF_WIDTH_FRACTION;
    let stock_left = body.center().x - stock_hw;
    let stock_right = body.center().x + stock_hw;
    let stock_rect = egui::Rect::from_min_max(
        egui::pos2(stock_left, z_to_y(ctx.stock_top_z)),
        egui::pos2(stock_right, z_to_y(ctx.stock_bottom_z)),
    );
    painter.rect_filled(stock_rect, tokens::RADIUS_SM, tokens::SURFACE_OVERLAY);
    painter.rect_stroke(
        stock_rect,
        tokens::RADIUS_SM,
        egui::Stroke::new(1.0_f32, tokens::DIAGRAM_MATERIAL),
        egui::StrokeKind::Middle,
    );

    // The model profile. A solid model is a box. A flat model is a rule at
    // its one plane, drawn in the model's own fill colour so it reads as
    // clearly as a box. An absent model draws nothing here and states
    // itself under the canvas.
    let model_hw = body.width() * MODEL_HALF_WIDTH_FRACTION;
    let model_left = body.center().x - model_hw;
    let model_right = body.center().x + model_hw;
    match (profile, ctx.model_top_z, ctx.model_bottom_z) {
        (ModelProfile::Solid, Some(model_top), Some(model_bottom)) => {
            let model_rect = egui::Rect::from_min_max(
                egui::pos2(model_left, z_to_y(model_top)),
                egui::pos2(model_right, z_to_y(model_bottom)),
            );
            painter.rect_filled(model_rect, tokens::RADIUS_SM, tokens::DIAGRAM_DIM);
            painter.rect_stroke(
                model_rect,
                tokens::RADIUS_SM,
                egui::Stroke::new(1.0_f32, tokens::DIAGRAM_MATERIAL),
                egui::StrokeKind::Middle,
            );
        }
        (ModelProfile::Flat, Some(model_top), _) => {
            let y = z_to_y(model_top);
            painter.line_segment(
                [egui::pos2(model_left, y), egui::pos2(model_right, y)],
                egui::Stroke::new(FLAT_PROFILE_STROKE, tokens::DIAGRAM_DIM),
            );
        }
        _ => {}
    }

    // Height lines + labels
    let hit_threshold = 12.0_f32;

    // Check pointer proximity for hover cursor
    let pointer_y = response.hover_pos().map(|p| p.y);
    let mut nearest_line: Option<(usize, f32)> = None;
    for line in &lines {
        let line_y = z_to_y(line.z);
        if let Some(py) = pointer_y {
            let dist = (py - line_y).abs();
            if dist < hit_threshold && nearest_line.is_none_or(|(_, d)| dist < d) {
                nearest_line = Some((line.index, dist));
            }
        }
    }
    if nearest_line.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    // R33. The reason for an inert Bottom is the panel's own sentence, not a
    // second wording. `nearest_line` is `Some` only while the pointer is
    // within the hit threshold of that line, so the tooltip belongs to the
    // Bottom line rather than to the whole canvas.
    if !bottom_drives_the_cut
        && nearest_line.is_some_and(|(idx, _)| idx == BOTTOM_LINE_INDEX)
        && let Some(note) = bottom_z_pin_note(op_type)
    {
        response = response.on_hover_text(note);
    }

    // Labels are pushed apart in the order they are drawn, so two heights a
    // millimetre apart do not print on top of each other. The capture
    // `planning/ui_premium_2026-09-13/review_heights.png` shows `TZ 27.0`
    // and `BZ 26.0` superimposed into one unreadable run. The push is
    // clamped inside the plot, so it can never move a label out of the
    // canvas; at worst two labels share the last row, which is what they
    // did before.
    let mut next_free_y = plot.top();

    for (line, galley) in lines.iter().zip(labels) {
        let y = z_to_y(line.z);
        let is_hovered = nearest_line.is_some_and(|(idx, _)| idx == line.index);

        // Line
        let stroke_width = if is_hovered {
            LINE_STROKE_HOVER
        } else {
            LINE_STROKE
        };
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(line_right, y)],
            egui::Stroke::new(stroke_width, line.color),
        );

        // Label, clamped into the plot on both axes.
        let size = galley.size();
        let text_x = label_x.min(plot.right() - size.x).max(plot.left());
        let text_y_max = (plot.bottom() - size.y).max(plot.top());
        let wanted_y = (y - size.y * 0.5).max(next_free_y);
        let text_y = wanted_y.clamp(plot.top(), text_y_max);
        next_free_y = text_y + size.y;
        painter.galley(egui::pos2(text_x, text_y), galley, line.color);
    }

    // Drag interaction
    let drag_id = ui.id().with("height_drag_idx");

    if response.drag_started()
        && let Some((idx, _)) = nearest_line
    {
        ui.memory_mut(|mem| mem.data.insert_temp(drag_id, idx));
    }

    if response.dragged()
        && let Some(idx) = ui.memory(|mem| mem.data.get_temp::<usize>(drag_id))
    {
        let dy = response.drag_delta().y;
        // Convert screen delta to Z delta (screen Y is inverted relative to Z)
        let z_per_pixel = (z_max - z_min) / plot.height() as f64;
        let dz = -(dy as f64) * z_per_pixel;

        let field = match idx {
            0 => &mut heights.clearance_z,
            1 => &mut heights.retract_z,
            2 => &mut heights.feed_z,
            3 => &mut heights.top_z,
            // SAFETY: idx is always 0..5 from DiagramLine definitions above,
            // so the last arm is `BOTTOM_LINE_INDEX`.
            _ => &mut heights.bottom_z,
        };

        // Get current resolved value and apply delta
        let current_z = match idx {
            0 => resolved.clearance_z,
            1 => resolved.retract_z,
            2 => resolved.feed_z,
            3 => resolved.top_z,
            _ => resolved.bottom_z,
        };
        *field = HeightMode::Manual(current_z + dz);
    }

    if response.drag_stopped() {
        ui.memory_mut(|mem| mem.data.remove::<usize>(drag_id));
    }

    // ── The legend, drawn into the band reserved for it ──────────────────
    let mut legend_x = inner.left();
    let mut legend_y = inner.bottom() - legend_h;
    for (swatch, galley) in legend {
        let size = galley.size();
        let centre_y = legend_y + legend_row_h * 0.5;
        painter.line_segment(
            [
                egui::pos2(legend_x, centre_y),
                egui::pos2(legend_x + LEGEND_SWATCH_W, centre_y),
            ],
            egui::Stroke::new(SWATCH_STROKE, swatch),
        );
        let text_x = legend_x + LEGEND_SWATCH_W + tokens::SPACE_1;
        let text_y = legend_y + (legend_row_h - size.y) * 0.5;
        painter.galley(egui::pos2(text_x, text_y), galley, tokens::TEXT_MUTED);
        if legend_stacked {
            legend_y += legend_row_h + tokens::SPACE_1;
        } else {
            legend_x = text_x + size.x + tokens::SPACE_3;
        }
    }

    // ── What the canvas cannot show, said in words ──────────────────────
    //
    // An empty frame must never stand for a measurement. A flat profile IS
    // a measurement, and it says what it measured. An absent profile
    // abstains with the `NotMeasured` treatment and names the reason.
    match profile {
        ModelProfile::Solid => {}
        ModelProfile::Flat => {
            ui.add_space(tokens::SPACE_1);
            ui.add(
                egui::Label::new(crate::ui::components::text::caption(
                    "Model: the drawing is flat. A 2D model has no Z extent, \
                     so its profile is one plane.",
                ))
                .wrap_mode(egui::TextWrapMode::Wrap),
            );
        }
        ModelProfile::Absent => {
            ui.add_space(tokens::SPACE_1);
            ui.horizontal_wrapped(|ui| {
                ui.add(crate::ui::components::NotMeasured::new().reason(
                    "No model geometry reached this operation, so the diagram \
                     has no model profile to draw.",
                ));
                ui.add(
                    egui::Label::new(crate::ui::components::text::caption(
                        "Model profile: not measured.",
                    ))
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
        }
    };
}

// ── Validation ───────────────────────────────────────────────────────────

pub struct ToolpathValidationContext {
    tools: Vec<ValidationTool>,
    models: Vec<ValidationModel>,
    setups: Vec<ValidationSetup>,
}

struct ValidationTool {
    id: crate::state::job::ToolId,
    tool_type: ToolType,
    diameter: f64,
}

struct ValidationModel {
    id: crate::state::job::ModelId,
    has_polygons: bool,
    has_mesh: bool,
    has_enriched_mesh: bool,
    /// Pickable drill targets (DXF points and circle/arc centres) the model
    /// exposes. The `Drill` arm reads this, never `has_polygons`
    /// (G-DRILLCENTROID).
    drill_target_count: usize,
}

struct ValidationSetup {
    /// The setup's toolpaths in plan order, as the Rest predecessor rule
    /// reads them (`crate::state::rest_dependency`).
    toolpaths: Vec<RestCandidate>,
}

impl ToolpathValidationContext {
    /// Build from a `ProjectSession`.
    pub fn from_session(session: &rs_cam_core::session::ProjectSession) -> Self {
        Self {
            tools: session
                .tools()
                .iter()
                .map(|tool| ValidationTool {
                    id: tool.id,
                    tool_type: tool.tool_type,
                    diameter: tool.diameter,
                })
                .collect(),
            models: session
                .models()
                .iter()
                .map(|model| ValidationModel {
                    id: crate::state::job::ModelId(model.id),
                    has_polygons: model.polygons.is_some(),
                    has_mesh: model.mesh.is_some(),
                    has_enriched_mesh: model.enriched_mesh.is_some(),
                    drill_target_count: model.drill_targets.len(),
                })
                .collect(),
            setups: session
                .list_setups()
                .iter()
                .map(|setup| ValidationSetup {
                    toolpaths: setup
                        .toolpath_indices
                        .iter()
                        .filter_map(|&tp_idx| session.toolpath_configs().get(tp_idx))
                        .map(RestCandidate::from_config)
                        .collect(),
                })
                .collect(),
        }
    }
}

/// Validate a session `ToolpathConfig` using the same logic as `validate_toolpath`.
pub fn validate_toolpath_config(
    tc: &rs_cam_core::session::ToolpathConfig,
    ctx: &ToolpathValidationContext,
) -> Vec<String> {
    let mut errs = Vec::new();

    let tool_id = crate::state::job::ToolId(tc.tool_id);
    let model_id = crate::state::job::ModelId(tc.model_id);
    let tp_id = tc.id;

    let Some(tool) = ctx.tools.iter().find(|t| t.id == tool_id) else {
        errs.push("No tool selected".into());
        return errs;
    };
    let tool_diameter = tool.diameter;

    // Geometry validation (inline, operating on tc fields)
    if !tc.operation.is_stock_based() {
        let model = ctx.models.iter().find(|m| m.id == model_id);
        if let Some(model) = model {
            let has_face_polygons = model.has_enriched_mesh
                && tc.face_selection.as_ref().is_some_and(|f| !f.is_empty());
            let has_polygons = model.has_polygons || has_face_polygons;
            let has_mesh = model.has_mesh;

            if tc.operation.needs_both() {
                let effective_has_mesh = if let OperationConfig::ProjectCurve(ref cfg) =
                    tc.operation
                    && let Some(surface_id) = cfg.surface_model_id
                {
                    ctx.models
                        .iter()
                        .find(|m| m.id == surface_id)
                        .is_some_and(|m| m.has_mesh)
                } else {
                    has_mesh
                };
                if !has_polygons || !effective_has_mesh {
                    errs.push("Selected model must provide both 2D geometry and a 3D mesh".into());
                }
            } else if tc.operation.is_3d() {
                if !has_mesh {
                    errs.push("Selected model has no 3D mesh".into());
                }
            } else if !has_polygons {
                errs.push("Selected model has no 2D geometry".into());
            }
        } else {
            errs.push("Selected model is missing".into());
        }
    }

    match &tc.operation {
        OperationConfig::Pocket(c) => {
            if c.stepover >= tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::Adaptive(c) => {
            if c.stepover >= tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::VCarve(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("VCarve requires a V-Bit tool".into());
            }
        }
        OperationConfig::Inlay(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("Inlay requires a V-Bit tool".into());
            }
        }
        OperationConfig::Chamfer(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("Chamfer requires a V-Bit tool".into());
            }
        }
        OperationConfig::Rest(c) => {
            if c.prev_tool_id.is_none() {
                errs.push("Previous tool not selected".into());
            } else if let Some(prev) = c.prev_tool_id {
                let prev_d = ctx.tools.iter().find(|t| t.id == prev).map(|t| t.diameter);
                if let Some(pd) = prev_d
                    && pd <= tool_diameter
                {
                    errs.push("Previous tool must be larger than current tool".into());
                }
                if !has_prior_rest_source(ctx, tp_id, model_id, prev) {
                    errs.push(
                        "Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same model"
                            .into(),
                    );
                }
            }
        }
        OperationConfig::Drill(c) => {
            if let Some(msg) = drill_targets_refusal(ctx, model_id, c) {
                errs.push(msg.to_owned());
            }
        }
        _ => {}
    }

    errs
}

/// G-DRILLCENTROID (UX-R03-004): the same predicate the generator refuses
/// with, read against the target model's drill-target count, so Generate is
/// disabled with the generator's own sentence instead of a hole appearing
/// at a polygon centroid.
fn drill_targets_refusal(
    ctx: &ToolpathValidationContext,
    model_id: crate::state::job::ModelId,
    cfg: &crate::state::toolpath::DrillConfig,
) -> Option<&'static str> {
    let targets = ctx
        .models
        .iter()
        .find(|m| m.id == model_id)
        .map_or(0, |m| m.drill_target_count);
    rs_cam_core::compute::execute::drill_targets_refusal(cfg, targets)
}

pub fn validate_toolpath(entry: &ToolpathEntry, ctx: &ToolpathValidationContext) -> Vec<String> {
    let mut errs = Vec::new();

    let Some(tool) = ctx.tools.iter().find(|tool| tool.id == entry.tool_id) else {
        errs.push("No tool selected".into());
        return errs;
    };
    let tool_diameter = tool.diameter;

    validate_geometry_selection(entry, ctx, &mut errs);

    match &entry.operation {
        OperationConfig::Pocket(c) => {
            if c.stepover >= tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::Adaptive(c) => {
            if c.stepover >= tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::VCarve(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("VCarve requires a V-Bit tool".into());
            }
        }
        OperationConfig::Inlay(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("Inlay requires a V-Bit tool".into());
            }
        }
        OperationConfig::Chamfer(_) => {
            if tool.tool_type != ToolType::VBit {
                errs.push("Chamfer requires a V-Bit tool".into());
            }
        }
        OperationConfig::Rest(c) => {
            if c.prev_tool_id.is_none() {
                errs.push("Previous tool not selected".into());
            } else if let Some(prev) = c.prev_tool_id {
                let prev_d = ctx
                    .tools
                    .iter()
                    .find(|tool| tool.id == prev)
                    .map(|tool| tool.diameter);
                if let Some(pd) = prev_d
                    && pd <= tool_diameter
                {
                    errs.push("Previous tool must be larger than current tool".into());
                }
                if !has_prior_rest_source(ctx, entry.id, entry.model_id, prev) {
                    errs.push(
                        "Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same model"
                            .into(),
                    );
                }
            }
        }
        OperationConfig::Drill(c) => {
            if let Some(msg) = drill_targets_refusal(ctx, entry.model_id, c) {
                errs.push(msg.to_owned());
            }
        }
        _ => {}
    }

    errs
}

fn validate_geometry_selection(
    entry: &ToolpathEntry,
    ctx: &ToolpathValidationContext,
    errs: &mut Vec<String>,
) {
    if entry.operation.is_stock_based() {
        return;
    }

    let model = ctx.models.iter().find(|model| model.id == entry.model_id);
    let Some(model) = model else {
        errs.push("Selected model is missing".into());
        return;
    };

    // STEP models with face selection derive polygons at compute time
    let has_face_polygons =
        model.has_enriched_mesh && entry.face_selection.as_ref().is_some_and(|f| !f.is_empty());
    let has_polygons = model.has_polygons || has_face_polygons;
    let has_mesh = model.has_mesh;

    if entry.operation.needs_both() {
        // ProjectCurve can use a separate surface model for the 3D mesh.
        let effective_has_mesh =
            if let crate::state::toolpath::OperationConfig::ProjectCurve(ref cfg) = entry.operation
                && let Some(surface_id) = cfg.surface_model_id
            {
                ctx.models
                    .iter()
                    .find(|m| m.id == surface_id)
                    .is_some_and(|m| m.has_mesh)
            } else {
                has_mesh
            };
        if !has_polygons || !effective_has_mesh {
            errs.push("Selected model must provide both 2D geometry and a 3D mesh (use Surface selector for separate mesh)".into());
        }
    } else if entry.operation.is_3d() {
        if !has_mesh {
            errs.push("Selected model has no 3D mesh".into());
        }
    } else if !has_polygons {
        errs.push("Selected model has no 2D geometry".into());
    }
}

/// Does the Rest op `rest_id` have a predecessor under the one rule in
/// [`crate::state::rest_dependency`]? The Operations card badge
/// (`ui::toolpath_panel::rest_badge`) reads the same rule (G-RESTBADGE).
fn has_prior_rest_source(
    ctx: &ToolpathValidationContext,
    rest_id: ToolpathId,
    rest_model_id: crate::state::job::ModelId,
    prev_tool_id: crate::state::job::ToolId,
) -> bool {
    ctx.setups.iter().any(|setup| {
        !rest_predecessors(&setup.toolpaths, rest_id, rest_model_id, prev_tool_id).is_empty()
    })
}

// ── Depth vs stock thickness (G-DEPTHSTOCK / G-DEPTHSTOCKCORE) ──────────

/// The depth-beyond-stock finding, re-exported from core.
///
/// F1.6 shipped this rule GUI-side. F1.18 moved the predicate to
/// [`core_static_checks::depth_beyond_stock`] and F1.18's GUI half deleted
/// the copy, so the Operations card row, the Safety header and MCP
/// `get_toolpath_diagnostics` all answer from ONE predicate. The name stays
/// here so the per-operation forms need no edit; the fields are core's, and
/// the old `bottom_z` field is now `cut_floor_z`.
pub use rs_cam_core::diagnostics::adapters::from_static_checks::DepthBeyondStock;

/// The equality epsilon the through-cut line shares with the depth rule.
/// One definition, in core: excesses at or under this are arithmetic, not a
/// cut into the bed, and a through cut EXACTLY at the stock thickness must
/// not trigger the caution (UX-R03-006 owns that case).
const DEPTH_BEYOND_STOCK_EPSILON_MM: f64 = core_static_checks::DEPTH_BEYOND_STOCK_EPS_MM;

/// The resolved-heights snapshot every GUI diagnostic surface hands to core.
///
/// One line, because the derivation belongs to core. N4 (2026-09-10) hoisted
/// the body into `ResolvedHeights::from_heights`, and the session route —
/// which MCP `get_toolpath_diagnostics` calls — now uses the same
/// constructor. Before that this GUI copy was the only caller that read the
/// entry's OWN `HeightsConfig`; the session route called
/// `ResolvedHeights::from_context`, which projects the stock top and the safe
/// Z into the five slots and drops every pin.
///
/// This matters for the depth rule: the generators cut `top_z - depth`, so a
/// Top Z pinned below the stock top deepens the emitted floor.
/// [`profile_through_cut`] in the same form already reads that pin
/// (`a_top_z_pinned_below_the_stock_top_counts_towards_the_through_cut_g_throughcut`),
/// and the two lines must not disagree at the boundary they share.
///
/// The stock span is `Some` because the context always carries it. `None`
/// there means NOT MEASURED and makes core's rule abstain.
fn diagnostics_heights(
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> core_static_checks::ResolvedHeights {
    core_static_checks::ResolvedHeights::from_heights(heights, ctx)
}

/// UX-R03-007 / G-DEPTHSTOCK: does this cut go below the stock bottom?
///
/// **This function carries no rule.** It is an input adapter: it resolves the
/// entry's heights and hands them to [`core_static_checks::depth_beyond_stock`].
/// Which operations the rule answers for, the comparison, the epsilon and the
/// sentence all live in core.
///
/// Two things a reader must not infer wrongly.
///
/// - The pinned BOTTOM Z is deliberately not read. F1.19 measured that a
///   pinned bottom reaches no emitted motion on any operation this rule
///   answers for, so cautioning on it described a cut the machine does not
///   make. The Heights tab says that beside the field instead
///   ([`bottom_z_pin_note`]).
/// - `None` covers three states and none of them is "clean on a measured
///   value": the rule does not answer for this operation, the caller supplied
///   no stock span, or the floor is at or above the stock bottom.
pub fn depth_beyond_stock(
    operation: &OperationConfig,
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> Option<DepthBeyondStock> {
    core_static_checks::depth_beyond_stock(operation, &diagnostics_heights(heights, ctx))
}

// ── Profile through cut (G-THROUGHCUT, UX-R03-006) ──────────────────────

/// A Profile whose cut bottom reaches the bottom of the board.
///
/// Informational, never a caution: the last pass frees the part, and the
/// operator decides how it is held. Zero tabs is a valid answer (vacuum,
/// double-sided tape), so the line names the holding and does not judge it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThroughCut {
    /// The depth the rule compared, measured from the STOCK top, in mm. Equal
    /// to the Depth field when Top Z is `Auto`; deeper when Top Z is pinned
    /// below the stock top.
    pub depth_mm: f64,
    /// The stock thickness of the setup, in mm.
    pub stock_thickness_mm: f64,
    /// The Profile's tab count at the time the rule was read.
    pub tab_count: usize,
}

impl ThroughCut {
    /// The one line the Profile form prints beside Depth.
    pub fn message(&self) -> String {
        through_cut_message(self.stock_thickness_mm, self.tab_count)
    }
}

/// The board thickness as an operator writes it: `12 mm`, `18.5 mm`. One
/// decimal only when the thickness has one.
fn board_thickness_text(stock_thickness_mm: f64) -> String {
    if (stock_thickness_mm - stock_thickness_mm.round()).abs() < 0.05 {
        format!("{stock_thickness_mm:.0}")
    } else {
        format!("{stock_thickness_mm:.1}")
    }
}

fn through_cut_message(stock_thickness_mm: f64, tab_count: usize) -> String {
    let board = board_thickness_text(stock_thickness_mm);
    let holding = match tab_count {
        0 => "no tabs configured".to_owned(),
        1 => "1 tab".to_owned(),
        k => format!("{k} tabs"),
    };
    format!("Through cut of a {board} mm board · Holding: {holding}")
}

/// UX-R03-006 / G-THROUGHCUT: the through-cut line for a Profile.
///
/// `None` when `depth_mm` is under `stock_thickness_mm` (a partial-depth
/// profile). Otherwise the line names the board and the holding:
///
/// - `Through cut of a 12 mm board · Holding: no tabs configured`
/// - `Through cut of a 12 mm board · Holding: 4 tabs`
///
/// The comparison is `depth >= thickness` within
/// [`DEPTH_BEYOND_STOCK_EPSILON_MM`], so a depth EXACTLY at the thickness is
/// a through cut (the case G-DEPTHSTOCK deliberately leaves alone), and a
/// depth beyond it shows this line AND the depth caution. A non-finite or
/// non-positive thickness gives `None`: there is no board to cut through.
pub fn profile_through_cut_line(
    depth_mm: f64,
    stock_thickness_mm: f64,
    tab_count: usize,
) -> Option<String> {
    if !depth_mm.is_finite()
        || !stock_thickness_mm.is_finite()
        || stock_thickness_mm <= 0.0
        || depth_mm + DEPTH_BEYOND_STOCK_EPSILON_MM < stock_thickness_mm
    {
        return None;
    }
    Some(through_cut_message(stock_thickness_mm, tab_count))
}

/// The form of [`profile_through_cut_line`] the inspector reads: the depth is
/// the Profile's cut bottom measured from the stock top, so a Top Z pinned
/// below the stock top counts towards the through cut the same way it does
/// in [`depth_beyond_stock`]. The generator cuts `top_z - cfg.depth`
/// (`OperationConfig::cutting_levels`), so that is the bottom read here; a
/// pinned Bottom Z does not reach the profile generator and is not read.
pub fn profile_through_cut(
    cfg: &ProfileConfig,
    heights: &HeightsConfig,
    ctx: &HeightContext,
) -> Option<ThroughCut> {
    let resolved = heights.resolve(ctx);
    let bottom_z = resolved.top_z - cfg.depth;
    let depth_mm = ctx.stock_top_z - bottom_z;
    let stock_thickness_mm = ctx.stock_top_z - ctx.stock_bottom_z;
    profile_through_cut_line(depth_mm, stock_thickness_mm, cfg.tab_count).map(|_| ThroughCut {
        depth_mm,
        stock_thickness_mm,
        tab_count: cfg.tab_count,
    })
}

// ── Contextual diagnostics ──────────────────────────────────────────────

/// Collect the unified [`rs_cam_core::diagnostics::Diagnostic`]
/// list for a single toolpath via the core orchestrator. All GUI
/// surfaces that need per-toolpath diagnostics route through this
/// function — it delegates to
/// [`rs_cam_core::diagnostics::diagnose_toolpath_inputs`] so the
/// params panel and MCP `get_toolpath_diagnostics` produce identical
/// findings for the same project state. Since F1.18 that includes
/// `geom.depth_beyond_stock`: the GUI carries no rule of its own any
/// more (see [`depth_beyond_stock`]).
///
/// The heights snapshot is [`diagnostics_heights`], built from the
/// entry's own `HeightsConfig`. Since N4 (2026-09-10) the session
/// route builds its snapshot with the same core constructor, so a
/// pinned Top Z reaches both surfaces. The caller supplies the required
/// precondition and model-reference contexts from the same owned panel
/// snapshot as the editable entry, so these static checks cannot be omitted
/// from this GUI path.
///
/// Load-gate (chipload / power / deflection / drill) diagnostics are
/// included when a `load_verdict` is supplied — they render in the
/// same ribbon as the other findings.
pub fn collect_diagnostics(
    entry: &ToolpathEntry,
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
    stale_defaults: &[rs_cam_core::compute::validate::StaleDefault],
    height_ctx: Option<&HeightContext>,
    preconditions: &rs_cam_core::diagnostics::diagnose::PreconditionContext,
    model_refs: &rs_cam_core::diagnostics::diagnose::ModelRefContext,
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    let Some(tool) = tool else {
        return Vec::new();
    };
    let heights = height_ctx.map(|ctx| diagnostics_heights(&entry.heights, ctx));
    let inputs = rs_cam_core::diagnostics::ToolpathDiagnoseInputs {
        toolpath_id: entry.id,
        operation: &entry.operation,
        tool,
        heights: heights.as_ref(),
        feeds_result: entry.feeds_result.as_ref(),
        load_verdict,
        stale_defaults,
        preconditions: Some(preconditions),
        model_refs: Some(model_refs),
        // A/M9: generation-time findings (standing material) ride on the
        // entry's own result. Passing `None` here was why the GUI's
        // diagnostics ribbon — the surface a router operator actually
        // reads — stayed silent about a raised island the core diagnostic
        // pipeline already knew about. `None` before generation is honest:
        // nothing has been measured yet.
        stats: entry.result.as_ref().map(|result| &result.stats),
    };
    // G-DEPTHSTOCKCORE (F1.18): the depth-beyond-stock caution used to be
    // appended here from a second, GUI-side copy of the rule. Both copies
    // fired, both stamped `geom.depth_beyond_stock`, and the operator read
    // the identical sentence twice. The copy is deleted; the caution now
    // arrives inside this list, from the one core predicate, on the snapshot
    // `diagnostics_heights` built above.
    rs_cam_core::diagnostics::diagnose_toolpath_inputs(&inputs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rs_cam_core::mesh::make_test_flat;
    use rs_cam_core::polygon::Polygon2;
    use rs_cam_core::session::ProjectSessionBuilder;

    use super::*;
    use crate::state::job::{ModelId, ModelKind, ModelUnits, ToolConfig, ToolId, ToolType};
    use crate::state::toolpath::OperationType;

    // ── G-HEIGHTSTAB: the Heights tab is a pure read ──────────────────
    //
    // These pin the two halves of the row contract: the display projection
    // cannot write (it takes `&HeightMode`), and the commit is gated on a
    // real user edit. See `height_row_display`'s doc comment for the defect.

    /// Stock 0..25, and an operation with NO depth semantics — the whole 3D
    /// family (`DepthSemantics::None` → `default_depth_for_heights() == 0`).
    /// This is the exact shape that took the adaptive3d rough to zero moves.
    fn zero_depth_3d_ctx() -> HeightContext {
        HeightContext {
            safe_z: 30.0,
            op_depth: 0.0,
            stock_top_z: 25.0,
            stock_bottom_z: 0.0,
            model_top_z: Some(24.0),
            model_bottom_z: Some(2.0),
        }
    }

    #[test]
    fn heights_display_leaves_auto_rows_unpinned_g_heightstab() {
        let heights = HeightsConfig::default();
        let ctx = zero_depth_3d_ctx();
        let auto = heights.resolve(&ctx);

        // Pre-condition: an all-Auto config leaves the floor to the operation.
        assert!(!auto.bottom_pinned);
        assert!(!auto.top_pinned);
        // …and on a zero-depth op the resolver's Auto bottom sits AT the stock
        // top, which is exactly why committing it clipped the op to nothing.
        assert!((auto.bottom_z - ctx.stock_top_z).abs() < 1e-9);

        // Rendering projects each Auto row for display only.
        for (mode, default_z) in [
            (&heights.clearance_z, auto.clearance_z),
            (&heights.retract_z, auto.retract_z),
            (&heights.feed_z, auto.feed_z),
            (&heights.top_z, auto.top_z),
            (&heights.bottom_z, auto.bottom_z),
        ] {
            let display = height_row_display(mode, HeightReference::StockTop, default_z, &ctx);
            assert_eq!(display.reference, HeightReference::StockTop);
            // The displayed "= z" hint must equal what the core resolver
            // produces — the panel must not carry its own defaults.
            let shown = display.reference.resolve_z(&ctx) + display.offset;
            assert!(
                (shown - default_z).abs() < 1e-9,
                "displayed {shown} != resolved {default_z}"
            );
        }

        // The stored config is untouched, so nothing goes stale and the
        // operation keeps deciding its own floor.
        assert!(heights.clearance_z.is_auto());
        assert!(heights.retract_z.is_auto());
        assert!(heights.feed_z.is_auto());
        assert!(heights.top_z.is_auto());
        assert!(heights.bottom_z.is_auto());
        assert!(!heights.resolve(&ctx).bottom_pinned);
    }

    #[test]
    fn height_row_commits_only_on_a_user_edit_g_heightstab() {
        let ctx = zero_depth_3d_ctx();
        let mut heights = HeightsConfig::default();
        let auto = heights.resolve(&ctx);
        let display = height_row_display(
            &heights.bottom_z,
            HeightReference::StockTop,
            auto.bottom_z,
            &ctx,
        );

        // Viewing the tab: no widget moved.
        commit_height_row(&mut heights.bottom_z, display, false);
        assert!(heights.bottom_z.is_auto());
        assert!(!heights.resolve(&ctx).bottom_pinned);

        // The user drags the row: now it pins, and only now.
        let edited = ReferenceOffset {
            reference: HeightReference::StockTop,
            offset: -6.0,
        };
        commit_height_row(&mut heights.bottom_z, edited, true);
        assert!(matches!(
            heights.bottom_z,
            HeightMode::FromReference(r) if (r.offset + 6.0).abs() < 1e-9
        ));
        let resolved = heights.resolve(&ctx);
        assert!(resolved.bottom_pinned);
        assert!((resolved.bottom_z - 19.0).abs() < 1e-9);
    }

    #[test]
    fn manual_height_rows_display_against_the_nearest_reference() {
        let ctx = zero_depth_3d_ctx();
        // 2.4 is nearest ModelBottom (2.0), not StockBottom (0.0).
        let mode = HeightMode::Manual(2.4);
        let display = height_row_display(&mode, HeightReference::StockTop, 0.0, &ctx);
        assert_eq!(display.reference, HeightReference::ModelBottom);
        assert!((display.offset - 0.4).abs() < 1e-9);
        // Still Manual — projecting it for display does not rewrite it.
        assert!(matches!(mode, HeightMode::Manual(v) if (v - 2.4).abs() < 1e-9));
    }

    fn session_polygon_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id,
            path: PathBuf::from("demo.svg"),
            name: "2D".to_owned(),
            kind: Some(ModelKind::Svg),
            mesh: None,
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -10.0, -10.0, 10.0, 10.0,
            )])),
            drill_targets: std::sync::Arc::new(Vec::new()),
            layers: std::sync::Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        }
    }

    fn session_mesh_model(id: usize) -> rs_cam_core::session::LoadedModel {
        rs_cam_core::session::LoadedModel {
            id,
            path: PathBuf::from("demo.stl"),
            name: "3D".to_owned(),
            kind: Some(ModelKind::Stl),
            mesh: Some(Arc::new(make_test_flat(20.0))),
            polygons: None,
            drill_targets: std::sync::Arc::new(Vec::new()),
            layers: std::sync::Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        }
    }

    fn sample_tool(id: ToolId, tool_type: ToolType, diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(id, tool_type);
        tool.diameter = diameter;
        tool
    }

    fn make_session_toolpath_config(
        name: &str,
        tool_id: usize,
        model_id: usize,
        op: OperationConfig,
    ) -> rs_cam_core::session::ToolpathConfig {
        rs_cam_core::session::ToolpathConfig {
            id: rs_cam_core::ToolpathId(0), // assigned by session.add_toolpath
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: Default::default(),
            heights: Default::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            stock_source: Default::default(),
            coolant: Default::default(),
            face_selection: None,
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            rest_analysis: Default::default(),
            planner_origin: None,
        }
    }

    #[test]
    fn validate_toolpath_rejects_wrong_geometry_type() {
        // The builder keeps the ids this fixture chose. `add_tool` and
        // `add_model` overwrite them.
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 6.0))
            .model(session_mesh_model(2))
            .build();

        let entry = ToolpathEntry::for_operation(
            ToolpathId(3),
            "Pocket".to_owned(),
            ToolId(1),
            ModelId(2),
            OperationType::Pocket,
        );

        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter().any(|err| err.contains("2D geometry")),
            "expected 2D geometry validation error, got {errs:?}"
        );
    }

    /// A drawing with one circle-centre target — what a DXF with a `CIRCLE`
    /// entity imports to.
    fn session_target_model(id: usize) -> rs_cam_core::session::LoadedModel {
        let mut model = session_polygon_model(id);
        model.drill_targets = Arc::new(vec![rs_cam_core::dxf_input::DrillTarget {
            x: 0.0,
            y: 0.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::dxf_input::DrillTargetKind::CircleCenter { diameter: 6.0 },
        }]);
        model
    }

    fn drill_entry(model: usize) -> ToolpathEntry {
        ToolpathEntry::for_operation(
            ToolpathId(3),
            "Drill".to_owned(),
            ToolId(1),
            ModelId(model),
            OperationType::Drill,
        )
    }

    /// G-DRILLCENTROID (UX-R03-004): a Drill op on a drawing with closed
    /// shapes but no circles or points is blocked with the generator's own
    /// sentence. Pre-fix the validator was silent and Generate drilled the
    /// polygon centroid.
    #[test]
    fn validate_drill_blocks_when_model_exposes_no_targets() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_polygon_model(2))
            .build();

        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            errs.iter()
                .any(|e| e == rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG),
            "expected {:?}, got {errs:?}",
            rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG
        );

        // The session-config door prints the same sentence.
        let tc =
            make_session_toolpath_config("Drill", 1, 2, OperationConfig::Drill(Default::default()));
        let errs =
            validate_toolpath_config(&tc, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter()
                .any(|e| e == rs_cam_core::compute::execute::NO_DRILL_TARGETS_MSG),
            "config door: got {errs:?}"
        );
    }

    #[test]
    fn validate_drill_passes_when_model_exposes_a_target() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_target_model(2))
            .build();

        let errs = validate_toolpath(
            &drill_entry(2),
            &ToolpathValidationContext::from_session(&session),
        );
        assert!(
            !errs.iter().any(|e| e.contains("drill targets")),
            "a target-bearing model must not be blocked: {errs:?}"
        );
    }

    #[test]
    fn validate_drill_passes_on_an_explicit_pick_without_model_targets() {
        let session = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 3.0))
            .model(session_polygon_model(2))
            .build();

        let mut entry = drill_entry(2);
        if let OperationConfig::Drill(cfg) = &mut entry.operation {
            cfg.selected_holes = Some(vec![[1.0, 2.0]]);
        }
        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_session(&session));
        assert!(
            !errs.iter().any(|e| e.contains("drill targets")),
            "a pick is a hole source: {errs:?}"
        );
    }

    #[test]
    fn validate_rest_requires_earlier_matching_operation() {
        let mut builder = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 10.0))
            .tool(sample_tool(ToolId(2), ToolType::EndMill, 6.0))
            .model(session_polygon_model(4));

        // Add the rest toolpath config to the session (no prior roughing)
        let mut rest_op = OperationConfig::Rest(Default::default());
        if let OperationConfig::Rest(cfg) = &mut rest_op {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        let rest_config = make_session_toolpath_config("Rest", 2, 4, rest_op);
        let rest_idx = builder
            .add_toolpath(0, rest_config)
            .expect("add_toolpath reports the new toolpath index");
        let session = builder.build();

        // Build entry with the session-assigned ID
        // SAFETY: rest_idx bounded by add_toolpath return
        #[allow(clippy::indexing_slicing)]
        let rest_id = session.toolpath_configs()[rest_idx].id;
        let mut rest = ToolpathEntry::for_operation(
            rest_id,
            "Rest".to_owned(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }

        let errs = validate_toolpath(&rest, &ToolpathValidationContext::from_session(&session));
        assert!(
            errs.iter()
                .any(|err| err.contains("earlier enabled operation")),
            "expected earlier-operation validation error, got {errs:?}"
        );
    }

    #[test]
    fn validate_rest_accepts_earlier_matching_operation() {
        let mut builder = ProjectSessionBuilder::new()
            .tool(sample_tool(ToolId(1), ToolType::EndMill, 10.0))
            .tool(sample_tool(ToolId(2), ToolType::EndMill, 6.0))
            .model(session_polygon_model(4));

        // Add roughing toolpath first
        let roughing_config = make_session_toolpath_config(
            "Pocket",
            1,
            4,
            OperationConfig::Pocket(Default::default()),
        );
        let _ = builder.add_toolpath(0, roughing_config).unwrap();

        // Add rest toolpath — session assigns the ID
        let mut rest_op = OperationConfig::Rest(Default::default());
        if let OperationConfig::Rest(cfg) = &mut rest_op {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        let rest_config = make_session_toolpath_config("Rest", 2, 4, rest_op);
        let rest_idx = builder
            .add_toolpath(0, rest_config)
            .expect("add_toolpath reports the new toolpath index");
        let session = builder.build();

        // Build entry with the session-assigned ID so validation can locate it
        // SAFETY: rest_idx bounded by add_toolpath return
        #[allow(clippy::indexing_slicing)]
        let rest_id = session.toolpath_configs()[rest_idx].id;
        let mut rest = ToolpathEntry::for_operation(
            rest_id,
            "Rest".to_owned(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }

        let errs = validate_toolpath(&rest, &ToolpathValidationContext::from_session(&session));
        assert!(
            !errs
                .iter()
                .any(|err| err.contains("earlier enabled operation")),
            "did not expect rest-ordering error, got {errs:?}"
        );
    }
}
