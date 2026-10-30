use super::super::pills::PillSuggestions;

use crate::state::toolpath::{ChamferConfig, ChamferSide, TraceCompensation, TraceConfig};

use super::super::linking_dressup::{Fields, combo_in_row, combo_param, plain_row};
use super::super::{dv, dv_opt, p};
use super::DepthBeyondStock;
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
    fields: Fields<'_>,
) {
    // G-DEPTHSTOCK: the caution is the outline of the Depth row.
    let fields = fields.with_depth_caution("depth", depth_caution);
    // Spec: trace doesn't get stepover/DOC pills (engraving op — LUT
    // axial/radial recommendations don't speak to single-line tracing).
    // Feed/plunge are edited on the Feeds & Speeds tab (W3.2).
    // P6.7 (O-1): Left and Right are relative to the drawn direction
    // of each line, closed loops included.
    combo_in_row(
        ui,
        plain_row(
            "Compensation",
            Some(
                "Left and Right are relative to the drawn direction of each line, closed loops included.",
            ),
        ),
        "trace_comp",
        trace_compensation_label(cfg.compensation),
        |ui| {
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
        },
    );
    dv_opt(
        ui,
        p(OperationType::Trace, "depth", "Depth"),
        &mut cfg.depth,
        " mm",
        0.1,
        None,
        fields,
    );
    dv(
        ui,
        p(OperationType::Trace, "depth_per_pass", "Depth/pass"),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        fields,
    );
    super::boundary_2d::region_ordering_row(ui, "trace_ord", &mut cfg.region_ordering);
}

pub(in crate::ui::properties) fn draw_chamfer_params(
    ui: &mut egui::Ui,
    cfg: &mut ChamferConfig,
    _pills: Option<&PillSuggestions>,
    depth_caution: Option<&DepthBeyondStock>,
    fields: Fields<'_>,
) {
    // G-DEPTHSTOCK: the chamfer depth comes from the width, so the caution
    // is the outline of the Chamfer width row.
    let fields = fields.with_depth_caution("chamfer_width", depth_caution);
    // Chamfer width/tip offset are geometry-driven, not feeds-driven; feed/
    // plunge are edited on the Feeds & Speeds tab (W3.2).
    // W1.3b: the side says where the material is; the bevel goes into it
    // at every ring of the outline, holes included.
    combo_param(
        ui,
        p(OperationType::Chamfer, "side", "Side"),
        fields,
        "chamfer_side",
        match cfg.side {
            ChamferSide::Outside => "Outside",
            ChamferSide::Inside => "Inside",
        },
        |ui| {
            ui.selectable_value(&mut cfg.side, ChamferSide::Outside, "Outside");
            ui.selectable_value(&mut cfg.side, ChamferSide::Inside, "Inside");
        },
    );
    dv(
        ui,
        p(OperationType::Chamfer, "chamfer_width", "Chamfer width"),
        &mut cfg.chamfer_width,
        " mm",
        0.1,
        fields,
    );
    dv(
        ui,
        p(OperationType::Chamfer, "tip_offset", "Tip offset"),
        &mut cfg.tip_offset,
        " mm",
        0.01,
        fields,
    );
}
