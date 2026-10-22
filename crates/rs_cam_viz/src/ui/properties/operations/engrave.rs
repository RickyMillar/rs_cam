use super::super::pills::PillSuggestions;

use crate::state::toolpath::{ChamferConfig, ChamferSide, TraceCompensation, TraceConfig};

use super::super::{depth_caution_row, dv, help_for, p};
use super::DepthBeyondStock;
use crate::ui::components::UiExt as _;
use rs_cam_core::compute::catalog::OperationType;

/// The label of a Trace compensation (P6.7, O-1): the tool on the line,
/// or on one side of the drawn direction.
pub(super) fn trace_compensation_label(compensation: TraceCompensation) -> &'static str {
    match compensation {
        TraceCompensation::None => "On the line",
        TraceCompensation::Left => "Left of the drawn direction",
        TraceCompensation::Right => "Right of the drawn direction",
    }
}

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
        // P6.7 (O-1): Left and Right are relative to the drawn direction
        // of each line, closed loops included.
        ui.label("Compensation:");
        egui::ComboBox::from_id_salt("trace_comp")
            .truncate()
            .selected_text(trace_compensation_label(cfg.compensation))
            .show_ui(ui, |ui| {
                for compensation in [
                    TraceCompensation::None,
                    TraceCompensation::Left,
                    TraceCompensation::Right,
                ] {
                    ui.selectable_value(
                        &mut cfg.compensation,
                        compensation,
                        trace_compensation_label(compensation),
                    );
                }
            });
        ui.end_row();
        dv(
            ui,
            p(OperationType::Trace, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::Trace, "depth_per_pass", "Depth/Pass:"),
            &mut cfg.depth_per_pass,
            " mm",
            0.1,
        );
        super::boundary_2d::region_ordering_row(ui, "trace_ord", &mut cfg.region_ordering);
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
                ChamferSide::Outside => "Outside",
                ChamferSide::Inside => "Inside",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.side, ChamferSide::Outside, "Outside");
                ui.selectable_value(&mut cfg.side, ChamferSide::Inside, "Inside");
            });
        ui.end_row();
        dv(
            ui,
            p(OperationType::Chamfer, "chamfer_width", "Chamfer Width:"),
            &mut cfg.chamfer_width,
            " mm",
            0.1,
        );
        depth_caution_row(ui, depth_caution);
        dv(
            ui,
            p(OperationType::Chamfer, "tip_offset", "Tip Offset:"),
            &mut cfg.tip_offset,
            " mm",
            0.01,
        );
    });
}
