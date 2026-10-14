use super::super::pills::PillSuggestions;

use crate::state::toolpath::{ChamferConfig, ProfileSide, TraceCompensation, TraceConfig};

use super::super::{depth_caution_row, dv, help_for, p};
use super::DepthBeyondStock;
use crate::ui::components::UiExt as _;
use rs_cam_core::compute::catalog::OperationType;

pub(in crate::ui::properties) fn draw_trace_params(
    ui: &mut egui::Ui,
    cfg: &mut TraceConfig,
    _pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
) {
    // Spec: trace doesn't get stepover/DOC pills (engraving op — LUT
    // axial/radial recommendations don't speak to single-line tracing).
    // Feed/plunge are edited on the Feeds & Speeds tab (W3.2).
    ui.param_grid("trace_p", |ui| {
        ui.label("Compensation:");
        egui::ComboBox::from_id_salt("trace_comp")
            .truncate()
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
        dv(
            ui,
            p(OperationType::Trace, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
            0.1..=50.0,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::Trace, "depth_per_pass", "Depth/Pass:"),
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
    ui.param_grid("chamfer_p", |ui| {
        // W1.3b: the side says where the material is; the bevel goes into it
        // at every ring of the outline, holes included.
        let side_help = help_for(OperationType::Chamfer, "side");
        let label = ui.label("Side:");
        if let Some(help) = side_help {
            label.on_hover_text(help);
        }
        egui::ComboBox::from_id_salt("chamfer_side")
            .truncate()
            .selected_text(match cfg.side {
                ProfileSide::Outside => "Outside",
                ProfileSide::Inside => "Inside",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
                ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
            });
        ui.end_row();
        dv(
            ui,
            p(OperationType::Chamfer, "chamfer_width", "Chamfer Width:"),
            &mut cfg.chamfer_width,
            " mm",
            0.1,
            0.1..=10.0,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::Chamfer, "tip_offset", "Tip Offset:"),
            &mut cfg.tip_offset,
            " mm",
            0.01,
            0.0..=2.0,
        );
    });
}
