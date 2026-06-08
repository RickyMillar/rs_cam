use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{
    CutDirection, HorizontalFinishConfig, RadialFinishConfig, RampFinishConfig, SpiralDirection,
    SpiralFinishConfig,
};

use super::super::{dv, dv_pill};
use super::draw_feed_params;

pub(in crate::ui::properties) fn draw_ramp_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut RampFinishConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Ramp finish: max_stepdown is a Z-step (geometry-driven, not LUT
    // axial DOC). No stepover field. Only feed/plunge/RPM get pills.
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
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

pub(in crate::ui::properties) fn draw_spiral_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut SpiralFinishConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
    egui::Grid::new("spiral_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=20.0,
                stepover_sugg,
            );
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
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

pub(in crate::ui::properties) fn draw_radial_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut RadialFinishConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Radial finish uses angular_step + point_spacing — neither maps to a
    // clearing-style WOC/DOC. Only feed/plunge/RPM get pills.
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
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

pub(in crate::ui::properties) fn draw_horizontal_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut HorizontalFinishConfig,
    feeds_result: Option<&FeedsResult>,
) {
    let stepover_sugg = feeds_result.map(|r| (r.radial_width_mm, &r.chipload_source));
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
            dv_pill(
                ui,
                "Stepover:",
                &mut cfg.stepover,
                " mm",
                0.1,
                0.05..=20.0,
                stepover_sugg,
            );
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
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
