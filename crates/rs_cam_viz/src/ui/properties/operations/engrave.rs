use super::super::pills::PillSuggestions;

use crate::state::toolpath::{ChamferConfig, TraceCompensation, TraceConfig};

use super::super::{depth_caution_row, dv};
use super::DepthBeyondStock;

pub(in crate::ui::properties) fn draw_trace_params(
    ui: &mut egui::Ui,
    cfg: &mut TraceConfig,
    _pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: trace doesn't get stepover/DOC pills (engraving op — LUT
    // axial/radial recommendations don't speak to single-line tracing).
    // Feed/plunge are edited on the Feeds & Speeds tab (W3.2).
    egui::Grid::new("trace_p")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
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
            depth_caution_row(ui, depth_caution);
            dv(
                ui,
                "Depth/Pass:",
                &mut cfg.depth_per_pass,
                " mm",
                0.1,
                0.1..=20.0,
            );
        });
}

pub(in crate::ui::properties) fn draw_chamfer_params(
    ui: &mut egui::Ui,
    cfg: &mut ChamferConfig,
    _pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Chamfer width/tip offset are geometry-driven, not feeds-driven; feed/
    // plunge are edited on the Feeds & Speeds tab (W3.2).
    egui::Grid::new("chamfer_p")
        .num_columns(2)
        .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
        .min_row_height(crate::ui::tokens::ROW_DENSE)
        .show(ui, |ui| {
            dv(
                ui,
                "Chamfer Width:",
                &mut cfg.chamfer_width,
                " mm",
                0.1,
                0.1..=10.0,
            );
            depth_caution_row(ui, depth_caution);
            dv(
                ui,
                "Tip Offset:",
                &mut cfg.tip_offset,
                " mm",
                0.01,
                0.0..=2.0,
            );
        });
}
