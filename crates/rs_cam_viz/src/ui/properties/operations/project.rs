use super::super::pills::PillSuggestions;

use crate::state::job::ModelId;
use crate::state::toolpath::{
    ProjectCurveConfig, ProjectCurveDirection, ProjectCurveProjection, ProjectCurveSide,
};
use rs_cam_core::compute::operation_configs::PROJECT_CURVE_POINT_CAUTION;

use super::super::{advanced, dv, p};
use crate::ui::components::UiExt as _;
use rs_cam_core::compute::catalog::OperationType;

pub(in crate::ui::properties) fn draw_project_curve_params(
    ui: &mut egui::Ui,
    cfg: &mut ProjectCurveConfig,
    models: &[(ModelId, String)],
    _pills: Option<&PillSuggestions>,
) {
    // Project-curve is an engraving op (single-line tracing onto a 3D
    // surface). No stepover/DOC pills; feed/plunge are edited on the
    // Feeds & Speeds tab (W3.2).

    // IC1: every row sits in one parameter grid, so the labels share one
    // column. The two sampling dials are in the Advanced section.
    ui.param_grid("proj_p", |ui| {
        // Surface model selector — lets the user pick a different model for
        // the 3D surface.
        ui.label("Surface:");
        let label = if let Some(surface_id) = cfg.surface_model_id {
            models
                .iter()
                .find(|(id, _)| *id == surface_id)
                .map(|(_, name)| name.as_str())
                .unwrap_or("(missing)")
        } else {
            "(same as input)"
        };
        egui::ComboBox::from_id_salt("proj_surface_model")
            .truncate()
            .selected_text(label)
            .show_ui(ui, |ui| {
                // Option to use same model
                if ui
                    .selectable_label(cfg.surface_model_id.is_none(), "(same as input)")
                    .clicked()
                {
                    cfg.surface_model_id = None;
                }
                // List all available models
                for (id, name) in models {
                    if ui
                        .selectable_label(cfg.surface_model_id == Some(*id), name.as_str())
                        .clicked()
                    {
                        cfg.surface_model_id = Some(*id);
                    }
                }
            });
        ui.end_row();

        // Projection direction
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("proj_direction")
            .truncate()
            .selected_text(cfg.direction.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut cfg.direction,
                    ProjectCurveDirection::FromAbove,
                    "From Above",
                )
                .on_hover_text("Project from above (Z-down). Engraves the top surface.");
                ui.selectable_value(
                    &mut cfg.direction,
                    ProjectCurveDirection::FromBelow,
                    "From Below",
                )
                .on_hover_text("Project from below (Z-up). Engraves the bottom surface.");
            });
        ui.end_row();

        // Projection mode. Point is a deliberate choice: the hover text and
        // the static caution diagnostic carry the same sentence.
        ui.label("Projection:");
        egui::ComboBox::from_id_salt("proj_projection")
            .truncate()
            .selected_text(cfg.projection.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut cfg.projection,
                    ProjectCurveProjection::Cutter,
                    "Cutter",
                )
                .on_hover_text(
                    "Drop the real cutter on the surface. The tool body does not cut the surface.",
                );
                ui.selectable_value(&mut cfg.projection, ProjectCurveProjection::Point, "Point")
                    .on_hover_text(format!(
                        "Tip Z = the surface Z under the curve + the depth. \
                         {PROJECT_CURVE_POINT_CAUTION}"
                    ));
            });
        ui.end_row();

        // Tool radius compensation side (closed rings only)
        ui.label("Side:")
            .on_hover_text("Closed rings only. Open paths always trace on the line.");
        egui::ComboBox::from_id_salt("proj_side")
            .truncate()
            .selected_text(cfg.side.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.side, ProjectCurveSide::Center, "On Line")
                    .on_hover_text("Tool centerline rides exactly on the curve.");
                ui.selectable_value(&mut cfg.side, ProjectCurveSide::Inside, "Inside")
                    .on_hover_text("Offset inward by tool radius. Cut stays inside the line.");
                ui.selectable_value(&mut cfg.side, ProjectCurveSide::Outside, "Outside")
                    .on_hover_text("Offset outward by tool radius. Cut stays outside the line.");
            });
        ui.end_row();

        dv(
            ui,
            p(OperationType::ProjectCurve, "depth", "Depth:"),
            &mut cfg.depth,
            " mm",
            0.1,
        );
    });
    advanced(ui, "proj_adv", |ui| {
        dv(
            ui,
            p(
                OperationType::ProjectCurve,
                "point_spacing",
                "Point Spacing:",
            ),
            &mut cfg.point_spacing,
            " mm",
            0.1,
        );
        // Chaining is OFF at 0.0 and ships that way — the row's hover (the
        // registry help of `chain_distance_mm`) says so.
        dv(
            ui,
            p(
                OperationType::ProjectCurve,
                "chain_distance_mm",
                "Chain Distance:",
            ),
            &mut cfg.chain_distance_mm,
            " mm",
            0.1,
        );
    });
}
