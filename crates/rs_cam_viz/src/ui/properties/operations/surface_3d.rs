use std::f64::consts::TAU;

use rs_cam_core::adaptive_shared::{
    radial_woc_fraction_from_leading_arc, target_engagement_fraction,
};
use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, DropCutterConfig, PencilConfig,
    RegionOrdering, ScallopConfig, ScallopDirection, SteepShallowConfig, WaterlineConfig,
};

use super::super::{dv, dv_pill};

/// Trochoid-cap endpoints the "Nibble" slider interpolates between. More
/// nibble → tighter cap (`NIBBLE_CAP_TIGHT`) → more trochoidal relief
/// loops, flattest load, most travel. Less nibble → relaxed cap
/// (`NIBBLE_CAP_RELAXED`) → fewer loops, least travel. The stored value is
/// `Adaptive3dConfig::trochoid_cap_mult`; the slider is the inverse map so
/// "more nibble" reads left→right as the user expects. See the cap sweep
/// in `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`.
const NIBBLE_CAP_TIGHT: f64 = 1.0;
const NIBBLE_CAP_RELAXED: f64 = 3.0;

/// Fallback tool radius (1/8" endmill) when the active tool's radius is
/// unavailable or non-physical — keeps the load↔stepover bridge finite.
const FALLBACK_TOOL_RADIUS: f64 = 3.175;

pub(in crate::ui::properties) fn draw_dropcutter_params(
    ui: &mut egui::Ui,
    cfg: &mut DropCutterConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("dc_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(ui, "Min Z:", &mut cfg.min_z, " mm", 0.5, -500.0..=0.0);
            dv(
                ui,
                "Slope From:",
                &mut cfg.slope_from,
                " deg",
                1.0,
                0.0..=90.0,
            );
            dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        });
}

pub(in crate::ui::properties) fn draw_adaptive3d_params(
    ui: &mut egui::Ui,
    cfg: &mut Adaptive3dConfig,
    tool_radius: f64,
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: pill stepover + depth_per_pass; leave fine_stepdown alone
    // (finishing-pass param the LUT doesn't speak to).
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    // The ContourSpiral strategy holds engagement flat by construction, so
    // its primary knobs are the friendly "Optimal load" + "Nibble" pair
    // (load derives the stepover; nibble drives the trochoid cap). The raw
    // stepover pill is retired for that strategy and shown derived instead.
    let spiral = matches!(cfg.clearing_strategy, ClearingStrategy::ContourSpiral);
    egui::Grid::new("a3d_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            if spiral {
                draw_spiral_load_controls(ui, cfg, tool_radius);
            } else {
                dv_pill(
                    ui,
                    "Stepover:",
                    &mut cfg.stepover,
                    " mm",
                    0.1,
                    0.05..=50.0,
                    stepover_sugg,
                );
            }
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            dv(
                ui,
                "Floor Stock:",
                &mut cfg.stock_to_leave_axial,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(
                ui,
                "Wall Stock:",
                &mut cfg.stock_to_leave_radial,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
            dv(
                ui,
                "Min Cut Radius:",
                &mut cfg.min_cutting_radius,
                " mm",
                0.1,
                0.0..=50.0,
            );
            ui.label("Entry Style:");
            egui::ComboBox::from_id_salt("a3d_entry")
                .selected_text(match cfg.entry_style {
                    Adaptive3dEntryStyle::Plunge => "Plunge",
                    Adaptive3dEntryStyle::Helix => "Helix",
                    Adaptive3dEntryStyle::Ramp => "Ramp",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.entry_style,
                        Adaptive3dEntryStyle::Plunge,
                        "Plunge",
                    );
                    ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Helix, "Helix");
                    ui.selectable_value(&mut cfg.entry_style, Adaptive3dEntryStyle::Ramp, "Ramp");
                });
            ui.end_row();
            // Entry-style-specific parameters — only shown for the style in use.
            match cfg.entry_style {
                Adaptive3dEntryStyle::Plunge => {}
                Adaptive3dEntryStyle::Ramp => {
                    dv(
                        ui,
                        "Ramp Angle:",
                        &mut cfg.ramp_angle_deg,
                        " deg",
                        0.5,
                        0.5..=45.0,
                    );
                }
                Adaptive3dEntryStyle::Helix => {
                    dv(
                        ui,
                        "Helix Radius:",
                        &mut cfg.helix_radius_factor,
                        " × D",
                        0.05,
                        0.05..=0.5,
                    );
                    dv(
                        ui,
                        "Helix Pitch:",
                        &mut cfg.helix_pitch,
                        " mm",
                        0.1,
                        0.1..=10.0,
                    );
                }
            }
            dv(
                ui,
                "Fine Stepdown:",
                &mut cfg.fine_stepdown,
                " mm",
                0.1,
                0.0..=10.0,
            );
            ui.label("Detect Flat:");
            ui.checkbox(&mut cfg.detect_flat_areas, "");
            ui.end_row();
            ui.label("Ordering:");
            egui::ComboBox::from_id_salt("a3d_ord")
                .selected_text(match cfg.region_ordering {
                    RegionOrdering::Global => "Global",
                    RegionOrdering::ByArea => "By Area",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::Global, "Global");
                    ui.selectable_value(
                        &mut cfg.region_ordering,
                        RegionOrdering::ByArea,
                        "By Area",
                    );
                });
            ui.end_row();
            ui.label("Strategy:");
            egui::ComboBox::from_id_salt("a3d_strat")
                .selected_text(match cfg.clearing_strategy {
                    ClearingStrategy::ContourParallel => "Contour Parallel",
                    ClearingStrategy::Adaptive => "Adaptive",
                    ClearingStrategy::AgentSearch => "Agent Search",
                    ClearingStrategy::ContourSpiral => "Contour Spiral",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::ContourParallel,
                        "Contour Parallel",
                    );
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::Adaptive,
                        "Adaptive",
                    );
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::AgentSearch,
                        "Agent Search",
                    )
                    .on_hover_text(
                        "Per-step direction search with preflight skip and widen-band \
                         recovery. Slow to generate — use when Contour Parallel or \
                         Adaptive leave uncut bands on difficult geometry.",
                    );
                    ui.selectable_value(
                        &mut cfg.clearing_strategy,
                        ClearingStrategy::ContourSpiral,
                        "Contour Spiral",
                    )
                    .on_hover_text(
                        "Constructive inside-out spiral per slice: one continuous \
                         stay-down pass per region with engagement bounded by the \
                         stepover. Experimental (Stage 1, algorithm review 2026-06-12).",
                    );
                });
            ui.end_row();
            ui.label("Z Blend:");
            ui.checkbox(&mut cfg.z_blend, "");
            ui.end_row();
            ui.label("Mill Shallow:").on_hover_text(
                "Insert fine sub-passes on low-slope cells within each \
                     DPP descent. Steep walls keep the normal DPP cadence; \
                     shallow areas come off the rough nearly smooth. \
                     Works with any clearing strategy.",
            );
            ui.checkbox(&mut cfg.mill_shallow_areas, "");
            ui.end_row();
            if cfg.mill_shallow_areas {
                let mut angle = cfg.shallow_angle_deg.unwrap_or(30.0);
                ui.label("Shallow Angle:");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut angle)
                                .speed(1.0)
                                .range(5.0..=60.0)
                                .suffix("°"),
                        )
                        .changed()
                    {
                        cfg.shallow_angle_deg = Some(angle);
                    }
                });
                ui.end_row();
                let mut step = cfg.shallow_stepdown.unwrap_or(cfg.depth_per_pass * 0.5);
                ui.label("Shallow Step:");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut step)
                                .speed(0.05)
                                .range(0.05..=cfg.depth_per_pass.max(0.1))
                                .suffix(" mm"),
                        )
                        .changed()
                    {
                        cfg.shallow_stepdown = Some(step);
                    }
                });
                ui.end_row();
            }
        });
    // The load/nibble paint widget spans the full panel width, so it lives
    // below the grid. Only meaningful for the spiral strategy.
    if spiral {
        let r = sane_tool_radius(tool_radius);
        let load = target_engagement_fraction(cfg.stepover, r).clamp(0.05, 0.45);
        let nibble = nibble_from_cap(cfg.trochoid_cap_mult);
        ui.add_space(4.0);
        draw_load_nibble_diagram(ui, load, nibble);
    }
}

/// Clamp the active tool's radius to a finite, positive value for the
/// load↔stepover bridge.
fn sane_tool_radius(tool_radius: f64) -> f64 {
    if tool_radius.is_finite() && tool_radius > 0.0 {
        tool_radius
    } else {
        FALLBACK_TOOL_RADIUS
    }
}

/// Slider position (0 = relaxed, 1 = max nibble) for a stored trochoid cap.
fn nibble_from_cap(cap: f64) -> f64 {
    let cap = if cap.is_finite() && cap > 0.0 {
        cap
    } else {
        NIBBLE_CAP_RELAXED - (NIBBLE_CAP_RELAXED - NIBBLE_CAP_TIGHT) * 0.7
    };
    ((NIBBLE_CAP_RELAXED - cap) / (NIBBLE_CAP_RELAXED - NIBBLE_CAP_TIGHT)).clamp(0.0, 1.0)
}

/// "Optimal load" + "Nibble" rows for the ContourSpiral strategy. The load
/// slider derives `cfg.stepover` from a leading-arc engagement fraction via
/// the [`target_engagement_fraction`] bridge; the nibble slider drives
/// `cfg.trochoid_cap_mult` through the inverse of [`nibble_from_cap`].
fn draw_spiral_load_controls(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig, tool_radius: f64) {
    let r = sane_tool_radius(tool_radius);

    // ── Optimal load ───────────────────────────────────────────────
    // Derived live from the raw stepover so existing projects (and the
    // feeds suggestion that wrote `stepover`) round-trip through the knob.
    let mut load_pct = (target_engagement_fraction(cfg.stepover, r) * 100.0).clamp(5.0, 45.0);
    ui.label("Optimal load:").on_hover_text(
        "Cutter engagement the spiral holds on every steady wrap \
         (leading-arc fraction of the tool). Sets the stepover for you — \
         more load = wider step = fewer passes but a heavier cut.",
    );
    if ui
        .add(egui::Slider::new(&mut load_pct, 5.0..=45.0).suffix("%"))
        .changed()
    {
        cfg.stepover = 2.0 * r * radial_woc_fraction_from_leading_arc(load_pct / 100.0);
    }
    ui.end_row();
    ui.label("");
    ui.label(
        egui::RichText::new(format!("→ stepover ≈ {:.2} mm", cfg.stepover))
            .small()
            .color(egui::Color32::from_rgb(140, 140, 150)),
    );
    ui.end_row();

    // ── Nibble ─────────────────────────────────────────────────────
    let mut nibble_pct = nibble_from_cap(cfg.trochoid_cap_mult) * 100.0;
    ui.label("Nibble:").on_hover_text(
        "How hard the spiral works to keep the load flat. More nibble = \
         more trochoidal relief loops = gentler, flatter cut but more \
         travel (slower). Less = relaxed, fewer loops, faster.",
    );
    if ui
        .add(egui::Slider::new(&mut nibble_pct, 0.0..=100.0).suffix("%"))
        .changed()
    {
        let nibble = nibble_pct / 100.0;
        cfg.trochoid_cap_mult =
            NIBBLE_CAP_RELAXED - nibble * (NIBBLE_CAP_RELAXED - NIBBLE_CAP_TIGHT);
    }
    ui.end_row();
    ui.label("");
    ui.label(
        egui::RichText::new(format!("trochoid cap ×{:.2}", cfg.trochoid_cap_mult))
            .small()
            .color(egui::Color32::from_rgb(140, 140, 150)),
    );
    ui.end_row();
}

/// Communicative cartoon for the load/nibble pair: a cutter circle with an
/// engaged wedge (its angle = the leading-arc load fraction) biting into a
/// material band, and a looped frontier whose loop count grows with the
/// nibble setting (more relief loops = flatter load). Mirrors the
/// stepover-diagram idiom in `properties/tool.rs`.
fn draw_load_nibble_diagram(ui: &mut egui::Ui, load: f64, nibble: f64) {
    let w = ui.available_width().min(240.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 96.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 5.0, egui::Color32::from_rgb(24, 24, 30));

    let center = egui::pos2(rect.left() + 54.0, rect.center().y);
    let radius = 32.0_f32;
    let frontier_x = center.x + radius * 0.55;

    // Uncut material band to the right of the frontier.
    let mat = egui::Rect::from_min_max(
        egui::pos2(frontier_x, rect.top() + 6.0),
        egui::pos2(rect.right() - 6.0, rect.bottom() - 6.0),
    );
    p.rect_filled(mat, 2.0, egui::Color32::from_rgb(40, 44, 36));

    // Engaged wedge — sector of angle = TAU*load, centred on +x (cut dir).
    #[allow(clippy::cast_possible_truncation)]
    let span = (TAU * load).clamp(0.0, TAU) as f32;
    let steps = 24;
    let mut pts = Vec::with_capacity(steps + 2);
    pts.push(center);
    for i in 0..=steps {
        #[allow(clippy::cast_precision_loss)]
        let a = -span / 2.0 + span * (i as f32 / steps as f32);
        pts.push(egui::pos2(
            center.x + radius * a.cos(),
            center.y + radius * a.sin(),
        ));
    }
    p.add(egui::Shape::convex_polygon(
        pts,
        egui::Color32::from_rgba_unmultiplied(90, 170, 255, 70),
        egui::Stroke::new(1.5, egui::Color32::from_rgb(110, 190, 255)),
    ));

    // Cutter outline.
    p.circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(200, 200, 210)),
    );

    // Looped frontier — trochoidal relief loops; count grows with nibble.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let loops = 2 + (nibble * 6.0).round() as i32;
    let top = rect.top() + 14.0;
    let bot = rect.bottom() - 14.0;
    let loop_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 180, 90));
    for i in 0..loops {
        #[allow(clippy::cast_precision_loss)]
        let t = if loops > 1 {
            i as f32 / (loops - 1) as f32
        } else {
            0.5
        };
        let y = top + (bot - top) * t;
        p.circle_stroke(egui::pos2(frontier_x, y), 6.0, loop_stroke);
    }

    // Tiny legends.
    p.text(
        egui::pos2(center.x, rect.bottom() - 8.0),
        egui::Align2::CENTER_BOTTOM,
        "load",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(110, 190, 255),
    );
    p.text(
        egui::pos2(frontier_x + 12.0, rect.top() + 8.0),
        egui::Align2::LEFT_TOP,
        "nibble",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(255, 180, 90),
    );
}

pub(in crate::ui::properties) fn draw_waterline_params(
    ui: &mut egui::Ui,
    cfg: &mut WaterlineConfig,
    _feeds_result: Option<&FeedsResult>,
) {
    // Waterline: z_step is the axial pass spacing, but Step 3 LUT mapping
    // (axial_depth_mm) is calibrated for clearing DOC, not contour Z-step.
    // Leave Z Step alone; feed/plunge live on the Feeds tab (W3.2).
    egui::Grid::new("wl_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            ui.label("Continuous:");
            ui.checkbox(&mut cfg.continuous, "");
            ui.end_row();
            // Z range now comes from the Heights tab (top_z / bottom_z)
        });
}

pub(in crate::ui::properties) fn draw_pencil_params(
    ui: &mut egui::Ui,
    cfg: &mut PencilConfig,
    _feeds_result: Option<&FeedsResult>,
) {
    // Pencil's offset_stepover is a parallel-pass spacing, not the same
    // shape as a clearing radial WOC; leave it alone. Feed/plunge live on
    // the Feeds tab (W3.2).
    egui::Grid::new("pen_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Bitangency Angle:",
                &mut cfg.bitangency_angle,
                " deg",
                1.0,
                90.0..=180.0,
            );
            dv(
                ui,
                "Min Cut Length:",
                &mut cfg.min_cut_length,
                " mm",
                0.5,
                0.5..=50.0,
            );
            dv(
                ui,
                "Hookup Distance:",
                &mut cfg.hookup_distance,
                " mm",
                0.5,
                0.5..=50.0,
            );
            ui.label("Offset Passes:");
            let mut n = cfg.num_offset_passes as i32;
            if ui.add(egui::DragValue::new(&mut n).range(0..=10)).changed() {
                cfg.num_offset_passes = n.max(0) as usize;
            }
            ui.end_row();
            dv(
                ui,
                "Offset Stepover:",
                &mut cfg.offset_stepover,
                " mm",
                0.1,
                0.05..=10.0,
            );
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
        });
}

pub(in crate::ui::properties) fn draw_scallop_params(
    ui: &mut egui::Ui,
    cfg: &mut ScallopConfig,
    _feeds_result: Option<&FeedsResult>,
) {
    // Scallop's stepover is computed from scallop_height + tool radius, not
    // an editable field, so no stepover pill. Feed/plunge live on the Feeds
    // tab (W3.2).
    egui::Grid::new("sc_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Scallop Height:",
                &mut cfg.scallop_height,
                " mm",
                0.01,
                0.01..=2.0,
            );
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
            ui.label("Direction:");
            egui::ComboBox::from_id_salt("sc_dir")
                .selected_text(match cfg.direction {
                    ScallopDirection::OutsideIn => "Outside In",
                    ScallopDirection::InsideOut => "Inside Out",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.direction,
                        ScallopDirection::OutsideIn,
                        "Outside In",
                    );
                    ui.selectable_value(
                        &mut cfg.direction,
                        ScallopDirection::InsideOut,
                        "Inside Out",
                    );
                });
            ui.end_row();
            ui.label("Continuous:");
            ui.checkbox(&mut cfg.continuous, "");
            ui.end_row();
            dv(
                ui,
                "Slope From:",
                &mut cfg.slope_from,
                " deg",
                1.0,
                0.0..=90.0,
            );
            dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
        });
}

pub(in crate::ui::properties) fn draw_steep_shallow_params(
    ui: &mut egui::Ui,
    cfg: &mut SteepShallowConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("ss_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Threshold Angle:",
                &mut cfg.threshold_angle,
                " deg",
                1.0,
                10.0..=80.0,
            );
            dv(
                ui,
                "Overlap:",
                &mut cfg.overlap_distance,
                " mm",
                0.1,
                0.0..=10.0,
            );
            dv(
                ui,
                "Wall Clearance:",
                &mut cfg.wall_clearance,
                " mm",
                0.1,
                0.0..=10.0,
            );
            ui.label("Steep First:");
            ui.checkbox(&mut cfg.steep_first, "");
            ui.end_row();
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            dv(
                ui,
                "Stock to Leave:",
                &mut cfg.stock_to_leave,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
        });
}
