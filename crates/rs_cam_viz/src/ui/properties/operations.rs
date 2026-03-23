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
            egui::ComboBox::from_id_salt("prof_comp")
                .selected_text(match cfg.compensation {
                    CompensationType::InComputer => "In Computer",
                    CompensationType::InControl => "In Control (G41/G42)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut cfg.compensation,
                        CompensationType::InComputer,
                        "In Computer",
                    );
                    ui.selectable_value(
                        &mut cfg.compensation,
                        CompensationType::InControl,
                        "In Control (G41/G42)",
                    );
                });
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
        });
}

pub(super) fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
    egui::Grid::new("wl_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            dv(ui, "Start Z:", &mut cfg.start_z, " mm", 0.5, -200.0..=200.0);
            dv(ui, "Final Z:", &mut cfg.final_z, " mm", 0.5, -200.0..=200.0);
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
                &mut cfg.stock_to_leave_axial,
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
                &mut cfg.stock_to_leave_axial,
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
                &mut cfg.stock_to_leave_axial,
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
                &mut cfg.stock_to_leave_axial,
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

/// Current mode discriminant for the mode combo box.
#[derive(Clone, Copy, PartialEq)]
enum HeightModeKind {
    Auto,
    Manual,
    FromReference,
}

impl HeightModeKind {
    fn of(mode: &HeightMode) -> Self {
        match mode {
            HeightMode::Auto => Self::Auto,
            HeightMode::Manual(_) => Self::Manual,
            HeightMode::FromReference(_) => Self::FromReference,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Manual => "Manual",
            Self::FromReference => "From Ref",
        }
    }
}

fn draw_height_row(
    ui: &mut egui::Ui,
    label: &str,
    mode: &mut HeightMode,
    auto_value: f64,
    ctx: &HeightContext,
    id_salt: &str,
) {
    // Column 1: label
    ui.label(label);

    // Column 2: mode selector
    let current_kind = HeightModeKind::of(mode);
    let mut new_kind = current_kind;
    egui::ComboBox::from_id_salt(format!("hm_{id_salt}"))
        .width(65.0)
        .selected_text(current_kind.label())
        .show_ui(ui, |ui| {
            for kind in [
                HeightModeKind::Auto,
                HeightModeKind::Manual,
                HeightModeKind::FromReference,
            ] {
                ui.selectable_value(&mut new_kind, kind, kind.label());
            }
        });

    // Handle mode transitions
    if new_kind != current_kind {
        match new_kind {
            HeightModeKind::Auto => *mode = HeightMode::Auto,
            HeightModeKind::Manual => {
                // Pre-fill with the current resolved value
                let resolved = mode.resolve_value(auto_value, ctx);
                *mode = HeightMode::Manual(resolved);
            }
            HeightModeKind::FromReference => {
                *mode = HeightMode::FromReference(ReferenceOffset {
                    reference: HeightReference::StockTop,
                    offset: 0.0,
                });
            }
        }
    }

    // Column 3: value editor (varies by mode)
    match mode {
        HeightMode::Auto => {
            ui.label(
                egui::RichText::new(format!("{auto_value:.1} mm"))
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        HeightMode::Manual(val) => {
            ui.add(
                egui::DragValue::new(val)
                    .suffix(" mm")
                    .speed(0.5)
                    .range(-500.0..=500.0),
            );
        }
        HeightMode::FromReference(ref_offset) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                egui::ComboBox::from_id_salt(format!("hr_{id_salt}"))
                    .width(80.0)
                    .selected_text(ref_offset.reference.label())
                    .show_ui(ui, |ui| {
                        for &href in HeightReference::ALL {
                            ui.selectable_value(
                                &mut ref_offset.reference,
                                href,
                                href.label(),
                            );
                        }
                    });
                let sign_prefix = if ref_offset.offset >= 0.0 { "+" } else { "" };
                ui.add(
                    egui::DragValue::new(&mut ref_offset.offset)
                        .prefix(sign_prefix)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(-500.0..=500.0),
                );
            });
        }
    }

    ui.end_row();
}

pub(super) fn draw_heights_params(
    ui: &mut egui::Ui,
    heights: &mut HeightsConfig,
    ctx: &HeightContext,
) {
    ui.label(
        egui::RichText::new("Set heights relative to stock/model geometry")
            .small()
            .italics()
            .color(egui::Color32::from_rgb(130, 130, 140)),
    );

    // Compute auto defaults for display (same logic as resolve())
    let retract_auto = ctx.safe_z;
    let clearance_auto = retract_auto + 10.0;
    let feed_auto = retract_auto - 2.0;
    let top_auto = 0.0;
    let bottom_auto = -ctx.op_depth.abs();

    egui::Grid::new("heights_p")
        .num_columns(3)
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            draw_height_row(
                ui, "Clearance Z:", &mut heights.clearance_z, clearance_auto, ctx, "h_clear",
            );
            draw_height_row(
                ui, "Retract Z:", &mut heights.retract_z, retract_auto, ctx, "h_retract",
            );
            draw_height_row(
                ui, "Feed Z:", &mut heights.feed_z, feed_auto, ctx, "h_feed",
            );
            draw_height_row(
                ui, "Top Z:", &mut heights.top_z, top_auto, ctx, "h_top",
            );
            draw_height_row(
                ui, "Bottom Z:", &mut heights.bottom_z, bottom_auto, ctx, "h_bottom",
            );
        });
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
                &mut cfg.stock_to_leave_axial,
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
                &mut cfg.stock_to_leave_axial,
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
                &mut cfg.stock_to_leave_axial,
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
