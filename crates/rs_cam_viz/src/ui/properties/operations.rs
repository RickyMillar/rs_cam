use crate::state::job::{JobState, ToolType};
use crate::state::toolpath::*;

use super::dv;

/// Draw the standard "Feed Rate" + "Plunge Rate" parameter pair used by
/// most cutting operations.
fn draw_feed_params(ui: &mut egui::Ui, feed_rate: &mut f64, plunge_rate: &mut f64) {
    dv(ui, "Feed Rate:", feed_rate, " mm/min", 10.0, 1.0..=50000.0);
    dv(
        ui,
        "Plunge Rate:",
        plunge_rate,
        " mm/min",
        10.0,
        1.0..=10000.0,
    );
}

pub(super) fn draw_pocket_params(ui: &mut egui::Ui, cfg: &mut PocketConfig) {
    egui::Grid::new("pocket_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Pattern:");
            egui::ComboBox::from_id_salt("pocket_pat")
                .selected_text(match cfg.pattern {
                    PocketPattern::Contour => "Contour",
                    PocketPattern::Zigzag => "Zigzag",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
                    ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
                });
            ui.end_row();
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            ui.label("Climb:");
            ui.checkbox(&mut cfg.climb, "");
            ui.end_row();
            if cfg.pattern == PocketPattern::Zigzag {
                dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
            }
            ui.label("Finishing Passes:");
            let mut fp = cfg.finishing_passes as i32;
            if ui
                .add(egui::DragValue::new(&mut fp).range(0..=10))
                .on_hover_text("Spring passes at final depth for dimensional accuracy")
                .changed()
            {
                cfg.finishing_passes = fp.max(0) as usize;
            }
            ui.end_row();
        });
}

pub(super) fn draw_profile_params(ui: &mut egui::Ui, cfg: &mut ProfileConfig) {
    egui::Grid::new("profile_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Side:");
            egui::ComboBox::from_id_salt("prof_side")
                .selected_text(match cfg.side {
                    ProfileSide::Outside => "Outside",
                    ProfileSide::Inside => "Inside",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
                    ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
                });
            ui.end_row();
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            ui.label("Climb:");
            ui.checkbox(&mut cfg.climb, "");
            ui.end_row();
            ui.label("Compensation:");
            // Only "In Computer" is implemented. G41/G42 ("In Control") is not
            // yet wired to the G-code emitter, so the option is hidden to avoid
            // misleading users. Restore when controller compensation is implemented.
            ui.label("In Computer");
            ui.end_row();
        });
    ui.add_space(8.0);
    ui.collapsing("Tabs", |ui| {
        egui::Grid::new("tab_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Count:");
                let mut count = cfg.tab_count as i32;
                if ui
                    .add(egui::DragValue::new(&mut count).range(0..=20))
                    .changed()
                {
                    cfg.tab_count = count.max(0) as usize;
                }
                ui.end_row();
                if cfg.tab_count > 0 {
                    dv(ui, "Width:", &mut cfg.tab_width, " mm", 0.5, 1.0..=50.0);
                    dv(ui, "Height:", &mut cfg.tab_height, " mm", 0.5, 0.5..=20.0);
                }
            });
        if cfg.tab_count > 0 {
            ui.add_space(4.0);
            draw_tab_diagram(ui, cfg.tab_count, cfg.tab_width, cfg.tab_height);
        }
    });
    egui::Grid::new("prof_finish")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Finishing Passes:");
            let mut fp = cfg.finishing_passes as i32;
            if ui
                .add(egui::DragValue::new(&mut fp).range(0..=10))
                .on_hover_text("Spring passes at final depth for dimensional accuracy")
                .changed()
            {
                cfg.finishing_passes = fp.max(0) as usize;
            }
            ui.end_row();
        });
}

pub(super) fn draw_adaptive_params(ui: &mut egui::Ui, cfg: &mut AdaptiveConfig) {
    egui::Grid::new("adapt_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            dv(
                ui,
                "Tolerance:",
                &mut cfg.tolerance,
                " mm",
                0.01,
                0.01..=1.0,
            );
            ui.label("Slot Clearing:");
            ui.checkbox(&mut cfg.slot_clearing, "");
            ui.end_row();
            dv(
                ui,
                "Min Cut Radius:",
                &mut cfg.min_cutting_radius,
                " mm",
                0.1,
                0.0..=50.0,
            );
        });
}

pub(super) fn draw_vcarve_params(ui: &mut egui::Ui, cfg: &mut VCarveConfig) {
    ui.label(
        egui::RichText::new("Requires V-Bit tool")
            .italics()
            .color(egui::Color32::from_rgb(150, 140, 110)),
    );
    egui::Grid::new("vcarve_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Max Depth:", &mut cfg.max_depth, " mm", 0.1, 0.1..=50.0);
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.05, 0.01..=10.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_rest_params(
    ui: &mut egui::Ui,
    cfg: &mut RestConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
) {
    egui::Grid::new("rest_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Previous Tool:");
            let prev_label = cfg
                .prev_tool_id
                .and_then(|pid| tools.iter().find(|(id, _, _)| *id == pid))
                .map(|(_, s, _)| s.as_str())
                .unwrap_or("(select)");
            egui::ComboBox::from_id_salt("rest_prev_tool")
                .selected_text(prev_label)
                .show_ui(ui, |ui| {
                    for (id, name, _) in tools {
                        let selected = cfg.prev_tool_id == Some(*id);
                        if ui.selectable_label(selected, name.as_str()).clicked() {
                            cfg.prev_tool_id = Some(*id);
                        }
                    }
                });
            ui.end_row();
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        });
}

pub(super) fn draw_inlay_params(ui: &mut egui::Ui, cfg: &mut InlayConfig) {
    ui.label(
        egui::RichText::new("Requires V-Bit tool")
            .italics()
            .color(egui::Color32::from_rgb(150, 140, 110)),
    );
    egui::Grid::new("inlay_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Pocket Depth:",
                &mut cfg.pocket_depth,
                " mm",
                0.1,
                0.1..=50.0,
            );
            dv(ui, "Glue Gap:", &mut cfg.glue_gap, " mm", 0.01, 0.0..=2.0);
            dv(
                ui,
                "Flat Depth:",
                &mut cfg.flat_depth,
                " mm",
                0.1,
                0.0..=20.0,
            );
            dv(
                ui,
                "Boundary Offset:",
                &mut cfg.boundary_offset,
                " mm",
                0.05,
                0.0..=10.0,
            );
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(
                ui,
                "Flat Tool Radius:",
                &mut cfg.flat_tool_radius,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_zigzag_params(ui: &mut egui::Ui, cfg: &mut ZigzagConfig) {
    egui::Grid::new("zigzag_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        });
}

// ── 3D operation parameters ──────────────────────────────────────────────

pub(super) fn draw_dropcutter_params(ui: &mut egui::Ui, cfg: &mut DropCutterConfig) {
    egui::Grid::new("dc_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_adaptive3d_params(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig) {
    egui::Grid::new("a3d_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
            );
            dv(
                ui,
                "Stock to Leave:",
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
                "Stock Top Z:",
                &mut cfg.stock_top_z,
                " mm",
                0.5,
                -100.0..=200.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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
                    EntryStyle::Plunge => "Plunge",
                    EntryStyle::Helix => "Helix",
                    EntryStyle::Ramp => "Ramp",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.entry_style, EntryStyle::Plunge, "Plunge");
                    ui.selectable_value(&mut cfg.entry_style, EntryStyle::Helix, "Helix");
                    ui.selectable_value(&mut cfg.entry_style, EntryStyle::Ramp, "Ramp");
                });
            ui.end_row();
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
                });
            ui.end_row();
            ui.label("Z Blend:");
            ui.checkbox(&mut cfg.z_blend, "");
            ui.end_row();
        });
}

pub(super) fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
}

pub(super) fn draw_pencil_params(ui: &mut egui::Ui, cfg: &mut PencilConfig) {
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_scallop_params(ui: &mut egui::Ui, cfg: &mut ScallopConfig) {
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_steep_shallow_params(ui: &mut egui::Ui, cfg: &mut SteepShallowConfig) {
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
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_ramp_finish_params(ui: &mut egui::Ui, cfg: &mut RampFinishConfig) {
    egui::Grid::new("rf_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Max Stepdown:",
                &mut cfg.max_stepdown,
                " mm",
                0.1,
                0.05..=10.0,
            );
            dv(
                ui,
                "Slope From:",
                &mut cfg.slope_from,
                " deg",
                1.0,
                0.0..=90.0,
            );
            dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
            ui.label("Direction:");
            egui::ComboBox::from_id_salt("rf_dir")
                .selected_text(match cfg.direction {
                    CutDirection::Climb => "Climb",
                    CutDirection::Conventional => "Conventional",
                    CutDirection::BothWays => "Both Ways",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.direction, CutDirection::Climb, "Climb");
                    ui.selectable_value(
                        &mut cfg.direction,
                        CutDirection::Conventional,
                        "Conventional",
                    );
                    ui.selectable_value(&mut cfg.direction, CutDirection::BothWays, "Both Ways");
                });
            ui.end_row();
            ui.label("Bottom Up:");
            ui.checkbox(&mut cfg.order_bottom_up, "");
            ui.end_row();
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

// ── Heights panel ────────────────────────────────────────────────────────

/// F360-style height row: [offset value] [from Reference ▾]
/// Auto mode auto-converts to FromReference with sensible defaults.
fn draw_height_row(
    ui: &mut egui::Ui,
    label: &str,
    mode: &mut HeightMode,
    default_ref: HeightReference,
    default_offset: f64,
    ctx: &HeightContext,
    id_salt: &str,
) {
    // Auto-promote to FromReference with the pre-computed sensible default
    if mode.is_auto() {
        *mode = HeightMode::FromReference(ReferenceOffset {
            reference: default_ref,
            offset: default_offset,
        });
    }

    // If Manual, convert to FromReference from the nearest reference
    if let HeightMode::Manual(abs_z) = *mode {
        let best_ref = find_nearest_reference(abs_z, ctx);
        let base_z = best_ref.resolve_z(ctx);
        *mode = HeightMode::FromReference(ReferenceOffset {
            reference: best_ref,
            offset: abs_z - base_z,
        });
    }

    ui.label(label);

    if let HeightMode::FromReference(ref_offset) = mode {
        ui.add(
            egui::DragValue::new(&mut ref_offset.offset)
                .suffix(" mm")
                .speed(0.5)
                .range(-500.0..=500.0),
        );

        egui::ComboBox::from_id_salt(format!("hr_{id_salt}"))
            .width(105.0)
            .selected_text(ref_label(ref_offset.reference, ref_offset.offset))
            .show_ui(ui, |ui| {
                for &href in HeightReference::ALL {
                    ui.selectable_value(
                        &mut ref_offset.reference,
                        href,
                        href.label(),
                    );
                }
            });

        // Show resolved absolute Z as a dim hint
        let resolved = ref_offset.reference.resolve_z(ctx) + ref_offset.offset;
        ui.label(
            egui::RichText::new(format!("= {resolved:.1}"))
                .small()
                .color(egui::Color32::from_rgb(100, 100, 115)),
        );
    }

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

pub(super) fn draw_heights_params(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
) {
    // Sensible default offsets (from the auto-resolve logic)
    let safe_offset = ctx.safe_z - ctx.stock_top_z; // safe_z relative to stock top

    egui::Grid::new("heights_p")
        .num_columns(4)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            draw_height_row(
                ui, "Clearance:", &mut heights.clearance_z,
                HeightReference::StockTop, safe_offset + 10.0, ctx, "h_clear",
            );
            draw_height_row(
                ui, "Retract:", &mut heights.retract_z,
                HeightReference::StockTop, safe_offset, ctx, "h_retract",
            );
            draw_height_row(
                ui, "Feed:", &mut heights.feed_z,
                HeightReference::StockTop, safe_offset - 2.0, ctx, "h_feed",
            );
            draw_height_row(
                ui, "Top:", &mut heights.top_z,
                HeightReference::StockTop, 0.0, ctx, "h_top",
            );
            draw_height_row(
                ui, "Bottom:", &mut heights.bottom_z,
                HeightReference::StockTop, -ctx.op_depth.abs(), ctx, "h_bottom",
            );
        });
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
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    // Workpiece rectangle (80% of canvas, centered)
    let margin = 14.0;
    let wp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin),
    );
    painter.rect_stroke(
        wp,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 95)),
    );

    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let path_stroke = egui::Stroke::new(1.2, path_color);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

    match pattern {
        StepoverPattern::Zigzag { stepover, angle } => {
            let angle_rad = (*angle as f32).to_radians();
            let cos_a = angle_rad.cos();
            let sin_a = angle_rad.sin();

            // Determine how many lines fit
            let perp_span = wp.width() * sin_a.abs() + wp.height() * cos_a.abs();
            let step_px = (*stepover as f32 / perp_span.max(1.0) * perp_span).max(6.0).min(perp_span / 2.0);
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
                let p0 = egui::pos2(x0.clamp(wp.left(), wp.right()), y0.clamp(wp.top(), wp.bottom()));
                let p1 = egui::pos2(x1.clamp(wp.left(), wp.right()), y1.clamp(wp.top(), wp.bottom()));

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
                        1.2,
                        egui::Color32::from_rgba_premultiplied(
                            (path_color.r() as f32 * alpha) as u8,
                            (path_color.g() as f32 * alpha) as u8,
                            (path_color.b() as f32 * alpha) as u8,
                            (255.0 * alpha) as u8,
                        ),
                    ),
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let cx = rect.center().x;
    let cy = rect.center().y + 5.0;
    let path_color = egui::Color32::from_rgb(120, 120, 140);
    let overcut_color = egui::Color32::from_rgb(220, 160, 50);
    let tool_color = egui::Color32::from_rgb(160, 170, 190);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);
    let mat_color = egui::Color32::from_rgb(40, 40, 52);

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
    painter.line_segment([a, corner], egui::Stroke::new(2.0, path_color));
    painter.line_segment([corner, c], egui::Stroke::new(2.0, path_color));

    // Direction arrows on edges
    let arrow_color = egui::Color32::from_rgb(80, 140, 80);
    let mid_a = egui::pos2((a.x + cx) / 2.0, cy);
    painter.circle_filled(egui::pos2(mid_a.x + 4.0, mid_a.y), 2.0, arrow_color);
    let mid_c = egui::pos2((cx + c.x) / 2.0, (cy + c.y) / 2.0);
    painter.circle_filled(mid_c, 2.0, arrow_color);

    // Tool circle at corner (where tool is when it reaches corner B)
    painter.circle_stroke(corner, tool_r, egui::Stroke::new(1.0, tool_color));

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
        egui::Stroke::new(1.5, overcut_color),
    );
    painter.circle_filled(overcut_pt, 3.0, overcut_color);

    // Ghost tool at overcut position
    painter.circle_stroke(
        overcut_pt,
        tool_r,
        egui::Stroke::new(0.8, egui::Color32::from_rgba_premultiplied(220, 160, 50, 80)),
    );

    // Corner angle arc
    let arc_r = 16.0_f32;
    let arc_start = 0.0_f32; // edge A forward direction
    let arc_end = u2_angle; // edge B forward direction
    let mut arc_pts = Vec::with_capacity(12);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let a_angle = arc_start + (arc_end - arc_start) * t;
        arc_pts.push(egui::pos2(cx + arc_r * a_angle.cos(), cy + arc_r * a_angle.sin()));
    }
    painter.add(egui::Shape::line(
        arc_pts,
        egui::Stroke::new(0.8, dim_color),
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let cy = rect.center().y;
    let path_color = egui::Color32::from_rgb(80, 80, 95);
    let lead_color = egui::Color32::from_rgb(50, 200, 230);
    let dim_color = egui::Color32::from_rgb(140, 140, 155);

    // Scale radius to fit
    let r_px = (radius as f32 * 3.0).clamp(15.0, 35.0);

    // Cut path: horizontal line across the middle
    let cut_left = rect.left() + 30.0;
    let cut_right = rect.right() - 30.0;
    painter.line_segment(
        [egui::pos2(cut_left, cy), egui::pos2(cut_right, cy)],
        egui::Stroke::new(2.0, path_color),
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
        egui::Stroke::new(2.0, lead_color),
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
        egui::Stroke::new(2.0, lead_color),
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
pub(super) fn draw_tab_diagram(ui: &mut egui::Ui, tab_count: usize, tab_width: f64, tab_height: f64) {
    if tab_count == 0 {
        return;
    }

    let desired_size = egui::vec2(ui.available_width().min(260.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    // Perimeter rectangle
    let margin = 20.0;
    let pr = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin),
    );
    painter.rect_stroke(
        pr,
        2.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 95)),
    );

    let tab_color = egui::Color32::from_rgb(220, 160, 50);
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
            egui::Stroke::new(3.0, tab_color),
        );
        // Small dot at the base
        painter.circle_filled(egui::pos2(px, py), 2.5, tab_color);
    }

    // Label
    let dim_color = egui::Color32::from_rgb(140, 140, 155);
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let margin = 20.0;
    let wp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + 16.0),
        egui::pos2(rect.right() - margin, rect.bottom() - 16.0),
    );

    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

    // Main path outline
    painter.rect_stroke(wp, 2.0, egui::Stroke::new(2.0, path_color));

    // Direction arrows (clockwise around the perimeter)
    let arrow_r = 3.0;
    let positions = [
        (wp.center_top() + egui::vec2(15.0, 0.0), true),   // top, going right
        (wp.right_center() + egui::vec2(0.0, 10.0), true),  // right, going down
        (wp.center_bottom() + egui::vec2(-15.0, 0.0), true), // bottom, going left
        (wp.left_center() + egui::vec2(0.0, -10.0), true),  // left, going up
    ];
    for (pos, _) in &positions {
        painter.circle_filled(*pos, arrow_r, path_color);
    }

    // Offset indicator (if applicable)
    if let Some(side) = offset_side {
        let offset_dist = 6.0;
        let offset_color = egui::Color32::from_rgba_premultiplied(50, 200, 180, 100);
        let inset = if side == "Inside" { offset_dist } else { -offset_dist };
        let offset_rect = egui::Rect::from_min_max(
            egui::pos2(wp.left() + inset, wp.top() + inset),
            egui::pos2(wp.right() - inset, wp.bottom() - inset),
        );
        painter.rect_stroke(offset_rect, 1.0, egui::Stroke::new(1.0, offset_color));
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let cx = rect.center().x;
    let cy = rect.center().y;
    let max_r = rect.width().min(rect.height()) * 0.4;
    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

    // Workpiece boundary
    painter.rect_stroke(
        egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(max_r * 2.1, max_r * 2.1)),
        2.0,
        egui::Stroke::new(0.5, egui::Color32::from_rgb(50, 50, 60)),
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
    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.2, path_color)));

    // Center dot
    painter.circle_filled(egui::pos2(cx, cy), 2.5, path_color);

    // Labels
    let dir_label = if outward { "Inside \u{2192} Out" } else { "Outside \u{2192} In" };
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let cx = rect.center().x;
    let cy = rect.center().y;
    let max_r = rect.width().min(rect.height()) * 0.4;
    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

    let step_rad = (angular_step as f32).to_radians();
    let num_spokes = (std::f32::consts::TAU / step_rad.max(0.01)).ceil() as usize;
    let num_spokes = num_spokes.min(72); // cap for very small angular_step

    for i in 0..num_spokes {
        let angle = step_rad * i as f32;
        let end = egui::pos2(cx + max_r * angle.cos(), cy + max_r * angle.sin());
        painter.line_segment(
            [egui::pos2(cx, cy), end],
            egui::Stroke::new(1.0, path_color),
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

    // Scattered drill points (representative pattern)
    let positions = [
        (0.25, 0.35), (0.45, 0.55), (0.7, 0.3), (0.55, 0.7),
        (0.3, 0.65), (0.75, 0.6), (0.5, 0.4),
    ];
    for &(fx, fy) in &positions {
        let x = rect.left() + 20.0 + fx as f32 * (rect.width() - 40.0);
        let y = rect.top() + 14.0 + fy as f32 * (rect.height() - 28.0);
        // Crosshair at each point
        let s = 4.0;
        painter.line_segment(
            [egui::pos2(x - s, y), egui::pos2(x + s, y)],
            egui::Stroke::new(1.0, path_color),
        );
        painter.line_segment(
            [egui::pos2(x, y - s), egui::pos2(x, y + s)],
            egui::Stroke::new(1.0, path_color),
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let offset_color = egui::Color32::from_rgb(50, 160, 200);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);
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
        egui::Stroke::new(2.0, path_color),
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
                    1.0,
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let dim_color = egui::Color32::from_rgb(100, 100, 115);
    let steep_color = egui::Color32::from_rgb(80, 160, 220);
    let shallow_color = egui::Color32::from_rgb(50, 200, 180);

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
    painter.rect_filled(steep_rect, 0.0, egui::Color32::from_rgba_premultiplied(80, 160, 220, 20));
    for i in 1..=3 {
        let inset = i as f32 * 6.0;
        if steep_rect.width() > inset * 2.0 + 4.0 && steep_rect.height() > inset * 2.0 + 4.0 {
            painter.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(steep_rect.left() + inset, steep_rect.top() + inset),
                    egui::pos2(steep_rect.right() - inset, steep_rect.bottom() - inset),
                ),
                1.0,
                egui::Stroke::new(0.8, steep_color),
            );
        }
    }

    // Shallow zone (right): raster lines
    let shallow_rect = egui::Rect::from_min_max(egui::pos2(div_x, wp.min.y), wp.max);
    painter.rect_filled(
        shallow_rect,
        0.0,
        egui::Color32::from_rgba_premultiplied(50, 200, 180, 20),
    );
    let line_step = 7.0;
    let mut y = shallow_rect.top() + line_step;
    while y < shallow_rect.bottom() - 2.0 {
        painter.line_segment(
            [egui::pos2(shallow_rect.left() + 2.0, y), egui::pos2(shallow_rect.right() - 2.0, y)],
            egui::Stroke::new(0.8, shallow_color),
        );
        y += line_step;
    }

    // Divider line
    painter.line_segment(
        [egui::pos2(div_x, wp.top()), egui::pos2(div_x, wp.bottom())],
        egui::Stroke::new(1.5, dim_color),
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
pub(super) fn draw_inlay_diagram(ui: &mut egui::Ui, pocket_depth: f64, glue_gap: f64, flat_depth: f64) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let dim_color = egui::Color32::from_rgb(100, 100, 115);
    let female_color = egui::Color32::from_rgb(80, 160, 220);
    let male_color = egui::Color32::from_rgb(50, 200, 180);
    let mat_color = egui::Color32::from_rgb(50, 50, 65);

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
        [egui::pos2(rect.left() + 8.0, surface_y), egui::pos2(rect.right() - 8.0, surface_y)],
        egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 85)),
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
        egui::Color32::from_rgb(20, 20, 26),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::line(pocket_pts, egui::Stroke::new(1.5, female_color)));

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
        egui::Color32::from_rgba_premultiplied(50, 200, 180, 40),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::line(
        vec![plug_pts[0], plug_pts[1], plug_pts[2], plug_pts[3], plug_pts[0]],
        egui::Stroke::new(1.5, male_color),
    ));

    // Drop arrow (shows the plug goes down into the pocket)
    let arrow_x = cx + half_w + 12.0;
    let arrow_top = plug_top;
    let arrow_bottom = surface_y + 4.0;
    painter.line_segment(
        [egui::pos2(arrow_x, arrow_top), egui::pos2(arrow_x, arrow_bottom)],
        egui::Stroke::new(1.0, dim_color),
    );
    painter.add(egui::Shape::line(
        vec![
            egui::pos2(arrow_x - 3.0, arrow_bottom - 6.0),
            egui::pos2(arrow_x, arrow_bottom),
            egui::pos2(arrow_x + 3.0, arrow_bottom - 6.0),
        ],
        egui::Stroke::new(1.0, dim_color),
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

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let path_color = egui::Color32::from_rgb(50, 200, 180);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);

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
            egui::Stroke::new(1.5, path_color),
        );
        // Ramp down to next level
        if i < num_levels - 1 {
            let next_y = y + level_step;
            painter.line_segment(
                [egui::pos2(x_end, y), egui::pos2(x_start, next_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(50, 200, 180, 120)),
            );
        }
    }

    // Stepdown dimension
    painter.line_segment(
        [egui::pos2(x_end + 8.0, y_top), egui::pos2(x_end + 8.0, y_top + level_step)],
        egui::Stroke::new(1.0, dim_color),
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
    label: &'static str,
    /// Which field index (0..5) for drag targeting.
    index: usize,
}

/// Draw an interactive 2D side-view diagram showing stock, model, and height planes.
pub(super) fn draw_height_diagram(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
) {
    let resolved = heights.resolve(ctx);

    // Build line definitions (ordered top to bottom for rendering)
    let lines = [
        DiagramLine {
            z: resolved.clearance_z,
            color: egui::Color32::from_rgb(77, 128, 230),
            label: "CZ",
            index: 0,
        },
        DiagramLine {
            z: resolved.retract_z,
            color: egui::Color32::from_rgb(77, 204, 204),
            label: "RZ",
            index: 1,
        },
        DiagramLine {
            z: resolved.feed_z,
            color: egui::Color32::from_rgb(77, 204, 77),
            label: "FZ",
            index: 2,
        },
        DiagramLine {
            z: resolved.top_z,
            color: egui::Color32::from_rgb(230, 204, 51),
            label: "TZ",
            index: 3,
        },
        DiagramLine {
            z: resolved.bottom_z,
            color: egui::Color32::from_rgb(230, 77, 51),
            label: "BZ",
            index: 4,
        },
    ];

    // Compute Z range with margin
    let all_z_values = [
        resolved.clearance_z,
        resolved.retract_z,
        resolved.feed_z,
        resolved.top_z,
        resolved.bottom_z,
        ctx.stock_top_z,
        ctx.stock_bottom_z,
    ];
    let z_min_raw = all_z_values
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);
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
    let desired_size = egui::vec2(ui.available_width().min(260.0), 180.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    // Coordinate mapping: Z → screen Y (higher Z = higher on screen = lower Y)
    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        rect.bottom() - (frac as f32) * rect.height()
    };

    // Stock rectangle (centered, ~55% width)
    let stock_hw = rect.width() * 0.275;
    let stock_left = rect.center().x - stock_hw;
    let stock_right = rect.center().x + stock_hw;
    let stock_top_y = z_to_y(ctx.stock_top_z);
    let stock_bottom_y = z_to_y(ctx.stock_bottom_z);
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(stock_left, stock_top_y),
            egui::pos2(stock_right, stock_bottom_y),
        ),
        2.0,
        egui::Color32::from_rgb(45, 45, 55),
    );
    painter.rect_stroke(
        egui::Rect::from_min_max(
            egui::pos2(stock_left, stock_top_y),
            egui::pos2(stock_right, stock_bottom_y),
        ),
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 95)),
    );

    // Model rectangle (narrower, different color)
    if let (Some(mt), Some(mb)) = (ctx.model_top_z, ctx.model_bottom_z) {
        let model_hw = rect.width() * 0.2;
        let model_left = rect.center().x - model_hw;
        let model_right = rect.center().x + model_hw;
        let model_top_y = z_to_y(mt);
        let model_bottom_y = z_to_y(mb);
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(model_left, model_top_y),
                egui::pos2(model_right, model_bottom_y),
            ),
            1.0,
            egui::Color32::from_rgb(55, 55, 75),
        );
        painter.rect_stroke(
            egui::Rect::from_min_max(
                egui::pos2(model_left, model_top_y),
                egui::pos2(model_right, model_bottom_y),
            ),
            1.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 90, 120)),
        );
    }

    // Height lines + labels
    let label_x = rect.right() - 42.0;
    let hit_threshold = 6.0_f32;

    // Check pointer proximity for hover cursor
    let pointer_y = response.hover_pos().map(|p| p.y);
    let mut nearest_line: Option<(usize, f32)> = None;
    for line in &lines {
        let line_y = z_to_y(line.z);
        if let Some(py) = pointer_y {
            let dist = (py - line_y).abs();
            if dist < hit_threshold {
                if nearest_line.map_or(true, |(_, d)| dist < d) {
                    nearest_line = Some((line.index, dist));
                }
            }
        }
    }
    if nearest_line.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    for line in &lines {
        let y = z_to_y(line.z);
        let is_hovered = nearest_line.map_or(false, |(idx, _)| idx == line.index);

        // Line
        let stroke_width = if is_hovered { 2.5 } else { 1.5 };
        painter.line_segment(
            [egui::pos2(rect.left() + 2.0, y), egui::pos2(rect.right() - 44.0, y)],
            egui::Stroke::new(stroke_width, line.color),
        );

        // Label
        let label_text = format!("{} {:.1}", line.label, line.z);
        painter.text(
            egui::pos2(label_x, y),
            egui::Align2::LEFT_CENTER,
            &label_text,
            egui::FontId::proportional(9.0),
            line.color,
        );
    }

    // Drag interaction
    let drag_id = ui.id().with("height_drag_idx");
    let dragging_idx: Option<usize> = ui.memory(|mem| mem.data.get_temp(drag_id));

    if response.drag_started() {
        if let Some((idx, _)) = nearest_line {
            ui.memory_mut(|mem| mem.data.insert_temp(drag_id, idx));
        }
    }

    if response.dragged() {
        if let Some(idx) = ui.memory(|mem| mem.data.get_temp::<usize>(drag_id)) {
            let dy = response.drag_delta().y;
            // Convert screen delta to Z delta (screen Y is inverted relative to Z)
            let z_per_pixel = (z_max - z_min) / rect.height() as f64;
            let dz = -(dy as f64) * z_per_pixel;

            let field = match idx {
                0 => &mut heights.clearance_z,
                1 => &mut heights.retract_z,
                2 => &mut heights.feed_z,
                3 => &mut heights.top_z,
                // SAFETY: idx is always 0..5 from DiagramLine definitions above
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
    }

    if response.drag_stopped() {
        ui.memory_mut(|mem| mem.data.remove::<usize>(drag_id));
    }

    // Legend at bottom: "Stock" and "Model" labels
    let legend_y = rect.bottom() - 8.0;
    painter.text(
        egui::pos2(stock_left + 2.0, legend_y),
        egui::Align2::LEFT_CENTER,
        "Stock",
        egui::FontId::proportional(8.0),
        egui::Color32::from_rgb(80, 80, 95),
    );
    if ctx.model_top_z.is_some() {
        painter.text(
            egui::pos2(rect.center().x, legend_y),
            egui::Align2::CENTER_CENTER,
            "Model",
            egui::FontId::proportional(8.0),
            egui::Color32::from_rgb(90, 90, 120),
        );
    }

    // Drop the unused variable hint
    let _ = dragging_idx;
}

// ── New operation parameters ─────────────────────────────────────────────

pub(super) fn draw_face_params(ui: &mut egui::Ui, cfg: &mut FaceConfig) {
    ui.label(
        egui::RichText::new("Levels stock top surface")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    egui::Grid::new("face_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Direction:");
            egui::ComboBox::from_id_salt("face_dir")
                .selected_text(match cfg.direction {
                    FaceDirection::OneWay => "One Way",
                    FaceDirection::Zigzag => "Zigzag",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.direction, FaceDirection::OneWay, "One Way");
                    ui.selectable_value(&mut cfg.direction, FaceDirection::Zigzag, "Zigzag");
                });
            ui.end_row();
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.5, 0.5..=100.0);
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.0..=50.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=20.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
            dv(
                ui,
                "Stock Offset:",
                &mut cfg.stock_offset,
                " mm",
                0.5,
                0.0..=50.0,
            );
        });
}

pub(super) fn draw_trace_params(ui: &mut egui::Ui, cfg: &mut TraceConfig) {
    ui.label(
        egui::RichText::new("Follows path exactly")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    egui::Grid::new("trace_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Compensation:");
            egui::ComboBox::from_id_salt("trace_comp")
                .selected_text(match cfg.compensation {
                    TraceCompensation::None => "None",
                    TraceCompensation::Left => "Left",
                    TraceCompensation::Right => "Right",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.compensation, TraceCompensation::None, "None");
                    ui.selectable_value(&mut cfg.compensation, TraceCompensation::Left, "Left");
                    ui.selectable_value(&mut cfg.compensation, TraceCompensation::Right, "Right");
                });
            ui.end_row();
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=50.0);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=20.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
}

pub(super) fn draw_drill_params(ui: &mut egui::Ui, cfg: &mut DrillConfig) {
    ui.label(
        egui::RichText::new("Hole positions from SVG circles")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    egui::Grid::new("drill_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Cycle:");
            egui::ComboBox::from_id_salt("drill_cycle")
                .selected_text(match cfg.cycle {
                    DrillCycleType::Simple => "Simple (G81)",
                    DrillCycleType::Dwell => "Dwell (G82)",
                    DrillCycleType::Peck => "Peck (G83)",
                    DrillCycleType::ChipBreak => "Chip Break (G73)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Simple, "Simple (G81)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Dwell, "Dwell (G82)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Peck, "Peck (G83)");
                    ui.selectable_value(
                        &mut cfg.cycle,
                        DrillCycleType::ChipBreak,
                        "Chip Break (G73)",
                    );
                });
            ui.end_row();
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.5, 0.5..=100.0);
            dv(
                ui,
                "Feed Rate:",
                &mut cfg.feed_rate,
                " mm/min",
                10.0,
                1.0..=5000.0,
            );
            dv(ui, "Retract Z:", &mut cfg.retract_z, " mm", 0.5, 0.5..=50.0);
            if matches!(cfg.cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
                dv(
                    ui,
                    "Peck Depth:",
                    &mut cfg.peck_depth,
                    " mm",
                    0.5,
                    0.5..=50.0,
                );
            }
            if cfg.cycle == DrillCycleType::Dwell {
                dv(
                    ui,
                    "Dwell Time:",
                    &mut cfg.dwell_time,
                    " s",
                    0.1,
                    0.1..=10.0,
                );
            }
            if cfg.cycle == DrillCycleType::ChipBreak {
                dv(
                    ui,
                    "Retract Amt:",
                    &mut cfg.retract_amount,
                    " mm",
                    0.1,
                    0.1..=5.0,
                );
            }
        });
}

pub(super) fn draw_alignment_pin_drill_params(
    ui: &mut egui::Ui,
    cfg: &mut AlignmentPinDrillConfig,
) {
    ui.label(
        egui::RichText::new("Drills alignment pin holes through stock into spoilboard")
            .italics()
            .color(egui::Color32::from_rgb(140, 180, 140)),
    );
    ui.label(format!("{} hole(s)", cfg.holes.len()));
    egui::Grid::new("pin_drill_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Spoilboard:",
                &mut cfg.spoilboard_penetration,
                " mm",
                0.5,
                0.5..=20.0,
            );
            ui.label("Cycle:");
            egui::ComboBox::from_id_salt("pin_drill_cycle")
                .selected_text(match cfg.cycle {
                    DrillCycleType::Simple => "Simple (G81)",
                    DrillCycleType::Dwell => "Dwell (G82)",
                    DrillCycleType::Peck => "Peck (G83)",
                    DrillCycleType::ChipBreak => "Chip Break (G73)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Simple, "Simple (G81)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Dwell, "Dwell (G82)");
                    ui.selectable_value(&mut cfg.cycle, DrillCycleType::Peck, "Peck (G83)");
                    ui.selectable_value(
                        &mut cfg.cycle,
                        DrillCycleType::ChipBreak,
                        "Chip Break (G73)",
                    );
                });
            ui.end_row();
            dv(
                ui,
                "Feed Rate:",
                &mut cfg.feed_rate,
                " mm/min",
                10.0,
                1.0..=5000.0,
            );
            dv(ui, "Retract Z:", &mut cfg.retract_z, " mm", 0.5, 0.5..=50.0);
            if matches!(cfg.cycle, DrillCycleType::Peck | DrillCycleType::ChipBreak) {
                dv(
                    ui,
                    "Peck Depth:",
                    &mut cfg.peck_depth,
                    " mm",
                    0.5,
                    0.5..=50.0,
                );
            }
        });
}

pub(super) fn draw_chamfer_params(ui: &mut egui::Ui, cfg: &mut ChamferConfig) {
    ui.label(
        egui::RichText::new("Requires V-Bit tool")
            .italics()
            .color(egui::Color32::from_rgb(150, 140, 110)),
    );
    egui::Grid::new("chamfer_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Chamfer Width:",
                &mut cfg.chamfer_width,
                " mm",
                0.1,
                0.1..=10.0,
            );
            dv(
                ui,
                "Tip Offset:",
                &mut cfg.tip_offset,
                " mm",
                0.01,
                0.0..=2.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
}

pub(super) fn draw_spiral_finish_params(ui: &mut egui::Ui, cfg: &mut SpiralFinishConfig) {
    egui::Grid::new("spiral_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=20.0);
            ui.label("Direction:");
            egui::ComboBox::from_id_salt("spiral_dir")
                .selected_text(match cfg.direction {
                    SpiralDirection::InsideOut => "Inside Out",
                    SpiralDirection::OutsideIn => "Outside In",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.direction,
                        SpiralDirection::InsideOut,
                        "Inside Out",
                    );
                    ui.selectable_value(
                        &mut cfg.direction,
                        SpiralDirection::OutsideIn,
                        "Outside In",
                    );
                });
            ui.end_row();
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_radial_finish_params(ui: &mut egui::Ui, cfg: &mut RadialFinishConfig) {
    egui::Grid::new("radial_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Angular Step:",
                &mut cfg.angular_step,
                " deg",
                1.0,
                1.0..=90.0,
            );
            dv(
                ui,
                "Point Spacing:",
                &mut cfg.point_spacing,
                " mm",
                0.1,
                0.1..=5.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_horizontal_finish_params(ui: &mut egui::Ui, cfg: &mut HorizontalFinishConfig) {
    ui.label(
        egui::RichText::new("Machines only flat areas")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    egui::Grid::new("horiz_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(
                ui,
                "Angle Threshold:",
                &mut cfg.angle_threshold,
                " deg",
                1.0,
                1.0..=30.0,
            );
            dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=20.0);
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
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

pub(super) fn draw_project_curve_params(ui: &mut egui::Ui, cfg: &mut ProjectCurveConfig) {
    ui.label(
        egui::RichText::new("Projects 2D curves onto 3D mesh")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    egui::Grid::new("proj_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=20.0);
            dv(
                ui,
                "Point Spacing:",
                &mut cfg.point_spacing,
                " mm",
                0.1,
                0.1..=5.0,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
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
}

struct ValidationSetup {
    toolpaths: Vec<ValidationToolpath>,
}

struct ValidationToolpath {
    id: ToolpathId,
    tool_id: crate::state::job::ToolId,
    model_id: crate::state::job::ModelId,
    enabled: bool,
}

impl ToolpathValidationContext {
    pub fn from_job(job: &JobState) -> Self {
        Self {
            tools: job
                .tools
                .iter()
                .map(|tool| ValidationTool {
                    id: tool.id,
                    tool_type: tool.tool_type,
                    diameter: tool.diameter,
                })
                .collect(),
            models: job
                .models
                .iter()
                .map(|model| ValidationModel {
                    id: model.id,
                    has_polygons: model.polygons.is_some(),
                    has_mesh: model.mesh.is_some(),
                    has_enriched_mesh: model.enriched_mesh.is_some(),
                })
                .collect(),
            setups: job
                .setups
                .iter()
                .map(|setup| ValidationSetup {
                    toolpaths: setup
                        .toolpaths
                        .iter()
                        .map(|toolpath| ValidationToolpath {
                            id: toolpath.id,
                            tool_id: toolpath.tool_id,
                            model_id: toolpath.model_id,
                            enabled: toolpath.enabled,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
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
                if !has_prior_rest_source(ctx, entry, prev) {
                    errs.push(
                        "Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same model"
                            .into(),
                    );
                }
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
        if !has_polygons || !has_mesh {
            errs.push("Selected model must provide both 2D geometry and a 3D mesh".into());
        }
    } else if entry.operation.is_3d() {
        if !has_mesh {
            errs.push("Selected model has no 3D mesh".into());
        }
    } else if !has_polygons {
        errs.push("Selected model has no 2D geometry".into());
    }
}

fn has_prior_rest_source(
    ctx: &ToolpathValidationContext,
    entry: &ToolpathEntry,
    prev_tool_id: crate::state::job::ToolId,
) -> bool {
    let Some(setup) = ctx.setups.iter().find(|setup| {
        setup
            .toolpaths
            .iter()
            .any(|toolpath| toolpath.id == entry.id)
    }) else {
        return false;
    };

    let Some(current_idx) = setup
        .toolpaths
        .iter()
        .position(|toolpath| toolpath.id == entry.id)
    else {
        return false;
    };

    // SAFETY: current_idx from position() within setup.toolpaths
    #[allow(clippy::indexing_slicing)]
    setup.toolpaths[..current_idx].iter().any(|toolpath| {
        toolpath.enabled && toolpath.tool_id == prev_tool_id && toolpath.model_id == entry.model_id
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rs_cam_core::mesh::make_test_flat;
    use rs_cam_core::polygon::Polygon2;

    use super::*;
    use crate::state::job::{
        JobState, LoadedModel, ModelId, ModelKind, ModelUnits, ToolConfig, ToolId,
    };

    fn polygon_model(id: ModelId) -> LoadedModel {
        LoadedModel {
            id,
            path: PathBuf::from("demo.svg"),
            name: "2D".to_string(),
            kind: ModelKind::Svg,
            mesh: None,
            polygons: Some(Arc::new(vec![Polygon2::rectangle(
                -10.0, -10.0, 10.0, 10.0,
            )])),
            enriched_mesh: None,
            units: ModelUnits::Millimeters,
            winding_report: None,
            load_error: None,
        }
    }

    fn mesh_model(id: ModelId) -> LoadedModel {
        LoadedModel {
            id,
            path: PathBuf::from("demo.stl"),
            name: "3D".to_string(),
            kind: ModelKind::Stl,
            mesh: Some(Arc::new(make_test_flat(20.0))),
            polygons: None,
            enriched_mesh: None,
            units: ModelUnits::Millimeters,
            winding_report: None,
            load_error: None,
        }
    }

    fn sample_tool(id: ToolId, tool_type: ToolType, diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(id, tool_type);
        tool.diameter = diameter;
        tool
    }

    #[test]
    fn validate_toolpath_rejects_wrong_geometry_type() {
        let mut job = JobState::new();
        job.tools
            .push(sample_tool(ToolId(1), ToolType::EndMill, 6.0));
        job.models.push(mesh_model(ModelId(2)));

        let entry = ToolpathEntry::for_operation(
            ToolpathId(3),
            "Pocket".to_string(),
            ToolId(1),
            ModelId(2),
            OperationType::Pocket,
        );

        let errs = validate_toolpath(&entry, &ToolpathValidationContext::from_job(&job));
        assert!(
            errs.iter().any(|err| err.contains("2D geometry")),
            "expected 2D geometry validation error, got {errs:?}"
        );
    }

    #[test]
    fn validate_rest_requires_earlier_matching_operation() {
        let mut job = JobState::new();
        job.tools
            .push(sample_tool(ToolId(1), ToolType::EndMill, 10.0));
        job.tools
            .push(sample_tool(ToolId(2), ToolType::EndMill, 6.0));
        job.models.push(polygon_model(ModelId(4)));

        let mut rest = ToolpathEntry::for_operation(
            ToolpathId(6),
            "Rest".to_string(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        job.push_toolpath(rest);

        let errs = validate_toolpath(
            job.find_toolpath(ToolpathId(6)).unwrap(),
            &ToolpathValidationContext::from_job(&job),
        );
        assert!(
            errs.iter()
                .any(|err| err.contains("earlier enabled operation")),
            "expected earlier-operation validation error, got {errs:?}"
        );
    }

    #[test]
    fn validate_rest_accepts_earlier_matching_operation() {
        let mut job = JobState::new();
        job.tools
            .push(sample_tool(ToolId(1), ToolType::EndMill, 10.0));
        job.tools
            .push(sample_tool(ToolId(2), ToolType::EndMill, 6.0));
        job.models.push(polygon_model(ModelId(4)));

        let roughing = ToolpathEntry::for_operation(
            ToolpathId(5),
            "Pocket".to_string(),
            ToolId(1),
            ModelId(4),
            OperationType::Pocket,
        );
        let mut rest = ToolpathEntry::for_operation(
            ToolpathId(6),
            "Rest".to_string(),
            ToolId(2),
            ModelId(4),
            OperationType::Rest,
        );
        if let OperationConfig::Rest(cfg) = &mut rest.operation {
            cfg.prev_tool_id = Some(ToolId(1));
        }
        job.push_toolpath(roughing);
        job.push_toolpath(rest);

        let errs = validate_toolpath(
            job.find_toolpath(ToolpathId(6)).unwrap(),
            &ToolpathValidationContext::from_job(&job),
        );
        assert!(
            !errs
                .iter()
                .any(|err| err.contains("earlier enabled operation")),
            "did not expect rest-ordering error, got {errs:?}"
        );
    }
}
