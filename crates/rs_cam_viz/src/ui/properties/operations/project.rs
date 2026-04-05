use crate::state::toolpath::*;

use super::super::dv;
use super::draw_feed_params;

pub(in crate::ui::properties) fn draw_project_curve_params(
    ui: &mut egui::Ui,
    cfg: &mut ProjectCurveConfig,
) {
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
