use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{
    AdaptiveConfig, CompensationType, FaceConfig, FaceDirection, InlayConfig, PocketConfig,
    PocketPattern, ProfileConfig, ProfileSide, RestConfig, VCarveConfig, ZigzagConfig,
};

use super::super::{dv, dv_pill};
use super::draw_feed_params;
use super::draw_tab_diagram;

pub(in crate::ui::properties) fn draw_face_params(
    ui: &mut egui::Ui,
    cfg: &mut FaceConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.5,
                0.5..=100.0,
                stepover_sugg,
            );
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.0..=50.0);
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=20.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
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

pub(in crate::ui::properties) fn draw_pocket_params(
    ui: &mut egui::Ui,
    cfg: &mut PocketConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
            ui.label("Climb:");
            ui.checkbox(&mut cfg.climb, "");
            ui.end_row();
            if cfg.pattern == PocketPattern::Zigzag {
                dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
            }
            ui.label("Finishing Passes:");
            let mut fp = cfg.finishing_passes as i32;
            if ui
                .add(egui::DragValue::new(&mut fp).range(0..=10).speed(0.1))
                .on_hover_text("Spring passes at final depth for dimensional accuracy")
                .changed()
            {
                cfg.finishing_passes = fp.max(0) as usize;
            }
            ui.end_row();
        });
}

pub(in crate::ui::properties) fn draw_profile_params(
    ui: &mut egui::Ui,
    cfg: &mut ProfileConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
            ui.label("Climb:");
            ui.checkbox(&mut cfg.climb, "");
            ui.end_row();
            ui.label("Compensation:");
            egui::ComboBox::from_id_salt("prof_comp")
                .selected_text(match cfg.compensation {
                    CompensationType::InComputer => "In Computer",
                    CompensationType::InControl => "In Control",
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
                        "In Control",
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
                .add(egui::DragValue::new(&mut fp).range(0..=10).speed(0.1))
                .on_hover_text("Spring passes at final depth for dimensional accuracy")
                .changed()
            {
                cfg.finishing_passes = fp.max(0) as usize;
            }
            ui.end_row();
        });
}

pub(in crate::ui::properties) fn draw_adaptive_params(
    ui: &mut egui::Ui,
    cfg: &mut AdaptiveConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    egui::Grid::new("adapt_p")
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
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
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

pub(in crate::ui::properties) fn draw_vcarve_params(
    ui: &mut egui::Ui,
    cfg: &mut VCarveConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // V-carve uses `max_depth` as the axial limit and `stepover` as the
    // lateral step. Map LUT axial → max_depth, radial → stepover.
    let max_depth_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("vcarve_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv_pill(
                ui,
                "Max Depth:",
                &mut cfg.max_depth,
                " mm",
                0.1,
                0.1..=50.0,
                max_depth_sugg,
            );
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.05,
                0.01..=10.0,
                stepover_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
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

pub(in crate::ui::properties) fn draw_rest_params(
    ui: &mut egui::Ui,
    cfg: &mut RestConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        });
}

pub(in crate::ui::properties) fn draw_inlay_params(
    ui: &mut egui::Ui,
    cfg: &mut InlayConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: only the stepover field maps to LUT radial_width. The other
    // inlay fields (pocket_depth, glue_gap, flat_depth, boundary_offset,
    // flat_tool_radius) are geometry-driven, not feeds-driven.
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=50.0,
                stepover_sugg,
            );
            dv(
                ui,
                "Flat Tool Radius:",
                &mut cfg.flat_tool_radius,
                " mm",
                0.1,
                0.1..=50.0,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
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

pub(in crate::ui::properties) fn draw_zigzag_params(
    ui: &mut egui::Ui,
    cfg: &mut ZigzagConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    let dpp_sugg = feeds_result.map(|r| (r.axial_depth_mm, &r.chipload_source));
    egui::Grid::new("zigzag_p")
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
            dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
            dv_pill(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=50.0,
                dpp_sugg,
            );
            draw_feed_params(
                ui,
                &mut cfg.feed_rate,
                &mut cfg.plunge_rate,
                &mut cfg.spindle_rpm,
                feeds_result,
            );
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        });
}
