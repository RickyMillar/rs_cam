use rs_cam_core::feeds::FeedsResult;

use crate::state::toolpath::{ChamferConfig, TraceCompensation, TraceConfig};

use super::super::dv;
use super::draw_feed_params;

pub(in crate::ui::properties) fn draw_trace_params(
    ui: &mut egui::Ui,
    cfg: &mut TraceConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Spec: trace doesn't get stepover/DOC pills (engraving op — LUT
    // axial/radial recommendations don't speak to single-line tracing).
    // Only feed/plunge/RPM pills via draw_feed_params.
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
        });
}

pub(in crate::ui::properties) fn draw_chamfer_params(
    ui: &mut egui::Ui,
    cfg: &mut ChamferConfig,
    feeds_result: Option<&FeedsResult>,
) {
    // Chamfer width/tip offset are geometry-driven, not feeds-driven; only
    // feed/plunge/RPM get pills.
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
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate, feeds_result);
        });
}
