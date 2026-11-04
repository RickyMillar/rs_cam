use super::super::pills::PillSuggestions;

use crate::state::toolpath::{
    CutDirection, HorizontalFinishConfig, RadialFinishConfig, RampFinishConfig, SpiralDirection,
    SpiralFinishConfig,
};

use super::super::linking_dressup::{Fields, check_in_row, combo_in_row, plain_row};
use super::super::{advanced, dv, dv_opt, dv_pill_opt, p};
use rs_cam_core::compute::catalog::OperationType;

pub(in crate::ui::properties) fn draw_ramp_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut RampFinishConfig,
    _pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    // Ramp finish: max_stepdown is a Z-step (geometry-driven, not LUT
    // axial DOC). No stepover field; feed/plunge live on the Feeds tab.
    dv_opt(
        ui,
        p(OperationType::RampFinish, "max_stepdown", "Max stepdown"),
        &mut cfg.max_stepdown,
        " mm",
        0.1,
        None,
        fields,
    );
    dv(
        ui,
        p(OperationType::RampFinish, "slope_from", "Slope from"),
        &mut cfg.slope_from,
        " deg",
        1.0,
        fields,
    );
    dv(
        ui,
        p(OperationType::RampFinish, "slope_to", "Slope to"),
        &mut cfg.slope_to,
        " deg",
        1.0,
        fields,
    );
    combo_in_row(
        ui,
        plain_row("Direction", Some("The cut direction of the ramp passes.")),
        "rf_dir",
        match cfg.direction {
            CutDirection::Climb => "Climb",
            CutDirection::Conventional => "Conventional",
            CutDirection::BothWays => "Both Ways",
        },
        |ui| {
            ui.selectable_value(&mut cfg.direction, CutDirection::Climb, "Climb");
            ui.selectable_value(
                &mut cfg.direction,
                CutDirection::Conventional,
                "Conventional",
            );
            ui.selectable_value(&mut cfg.direction, CutDirection::BothWays, "Both Ways");
        },
    );
    dv(
        ui,
        p(
            OperationType::RampFinish,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
    advanced(ui, "rf_adv", |ui| {
        check_in_row(
            ui,
            plain_row("Bottom up", Some("Cut the levels from the bottom up.")),
            &mut cfg.order_bottom_up,
        );
        dv(
            ui,
            p(OperationType::RampFinish, "sampling", "Sampling"),
            &mut cfg.sampling,
            " mm",
            0.1,
            fields,
        );
        dv(
            ui,
            p(OperationType::RampFinish, "tolerance", "Tolerance"),
            &mut cfg.tolerance,
            " mm",
            0.01,
            fields,
        );
    });
}

pub(in crate::ui::properties) fn draw_spiral_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut SpiralFinishConfig,
    pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv_pill_opt(
        ui,
        p(OperationType::SpiralFinish, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    combo_in_row(
        ui,
        plain_row(
            "Direction",
            Some("Inside out starts at the centre. Outside in starts at the edge."),
        ),
        "spiral_dir",
        match cfg.direction {
            SpiralDirection::InsideOut => "Inside Out",
            SpiralDirection::OutsideIn => "Outside In",
        },
        |ui| {
            ui.selectable_value(&mut cfg.direction, SpiralDirection::InsideOut, "Inside Out");
            ui.selectable_value(&mut cfg.direction, SpiralDirection::OutsideIn, "Outside In");
        },
    );
    dv(
        ui,
        p(
            OperationType::SpiralFinish,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
}

pub(in crate::ui::properties) fn draw_radial_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut RadialFinishConfig,
    _pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    // Radial finish uses angular_step + point_spacing — neither maps to a
    // clearing-style WOC/DOC. Feed/plunge live on the Feeds tab.
    dv(
        ui,
        p(OperationType::RadialFinish, "angular_step", "Angular step"),
        &mut cfg.angular_step,
        " deg",
        1.0,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::RadialFinish,
            "point_spacing",
            "Point spacing",
        ),
        &mut cfg.point_spacing,
        " mm",
        0.1,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::RadialFinish,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
}

pub(in crate::ui::properties) fn draw_horizontal_finish_params(
    ui: &mut egui::Ui,
    cfg: &mut HorizontalFinishConfig,
    pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    let stepover_sugg = pills.map(PillSuggestions::stepover);
    dv(
        ui,
        p(
            OperationType::HorizontalFinish,
            "angle_threshold",
            "Angle threshold",
        ),
        &mut cfg.angle_threshold,
        " deg",
        1.0,
        fields,
    );
    dv_pill_opt(
        ui,
        p(OperationType::HorizontalFinish, "stepover", "Stepover"),
        &mut cfg.stepover,
        " mm",
        0.1,
        stepover_sugg,
        fields,
    );
    dv(
        ui,
        p(
            OperationType::HorizontalFinish,
            "stock_to_leave",
            "Stock to leave",
        ),
        &mut cfg.stock_to_leave,
        " mm",
        0.05,
        fields,
    );
}
