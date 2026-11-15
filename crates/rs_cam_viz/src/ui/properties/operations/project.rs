use super::super::pills::PillSuggestions;

use crate::state::job::ModelId;
use crate::state::toolpath::{
    ProjectCurveConfig, ProjectCurveDirection, ProjectCurveProjection, ProjectCurveSide,
};
use rs_cam_core::compute::operation_configs::PROJECT_CURVE_POINT_CAUTION;

use super::super::linking_dressup::{Fields, combo_in_row, combo_param, plain_row};
use super::super::{advanced, dv, dv_opt, p};
use rs_cam_core::compute::catalog::OperationType;

pub(in crate::ui::properties) fn draw_project_curve_params(
    ui: &mut egui::Ui,
    cfg: &mut ProjectCurveConfig,
    models: &[(ModelId, String)],
    _pills: Option<&PillSuggestions>,
    fields: Fields<'_>,
) {
    // Project-curve is an engraving op (single-line tracing onto a 3D
    // surface). No stepover/DOC pills; feed/plunge are edited on the
    // Feeds & Speeds tab (W3.2).

    // IC1: every row is a `PropRow`, so the labels share one column. The
    // two sampling dials are in the Advanced section.
    // Surface model selector — lets the user pick a different model for
    // the 3D surface.
    let label = if let Some(surface_id) = cfg.surface_model_id {
        models
            .iter()
            .find(|(id, _)| *id == surface_id)
            .map(|(_, name)| name.as_str())
            .unwrap_or("(missing)")
    } else {
        "(same as input)"
    };
    combo_in_row(
        ui,
        plain_row(
            "Surface",
            Some("The model the curve projects onto. Empty: the input model."),
        ),
        "proj_surface_model",
        label,
        |ui| {
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
        },
    );

    // Projection direction
    combo_in_row(
        ui,
        plain_row(
            "Direction",
            Some("From above engraves the top surface. From below engraves the bottom surface."),
        ),
        "proj_direction",
        cfg.direction.label(),
        |ui| {
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
        },
    );

    // Projection mode. Point is a deliberate choice: the hover text and
    // the static caution diagnostic carry the same sentence.
    combo_param(
        ui,
        p(OperationType::ProjectCurve, "projection", "Projection"),
        fields,
        "proj_projection",
        cfg.projection.label(),
        |ui| {
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
        },
    );

    // Tool radius compensation side (closed rings only)
    combo_in_row(
        ui,
        plain_row(
            "Side",
            Some("Closed rings only. Open paths always trace on the line."),
        ),
        "proj_side",
        cfg.side.label(),
        |ui| {
            ui.selectable_value(&mut cfg.side, ProjectCurveSide::Center, "On Line")
                .on_hover_text("Tool centerline rides exactly on the curve.");
            ui.selectable_value(&mut cfg.side, ProjectCurveSide::Inside, "Inside")
                .on_hover_text("Offset inward by tool radius. Cut stays inside the line.");
            ui.selectable_value(&mut cfg.side, ProjectCurveSide::Outside, "Outside")
                .on_hover_text("Offset outward by tool radius. Cut stays outside the line.");
        },
    );

    dv_opt(
        ui,
        p(OperationType::ProjectCurve, "depth", "Depth"),
        &mut cfg.depth,
        " mm",
        0.1,
        None,
        fields,
    );
    // P33: an optional key. Empty cuts the full depth in one pass.
    dv_opt(
        ui,
        p(
            OperationType::ProjectCurve,
            "depth_per_pass",
            "Depth per pass",
        ),
        &mut cfg.depth_per_pass,
        " mm",
        0.1,
        Some("one pass"),
        fields,
    );
    advanced(ui, "proj_adv", |ui| {
        // P35 F: an optional key. Empty cuts on the curve.
        dv_opt(
            ui,
            p(
                OperationType::ProjectCurve,
                "lateral_offset",
                "Lateral offset",
            ),
            &mut cfg.lateral_offset,
            " mm",
            0.1,
            Some("on the curve"),
            fields,
        );
        dv(
            ui,
            p(
                OperationType::ProjectCurve,
                "point_spacing",
                "Point spacing",
            ),
            &mut cfg.point_spacing,
            " mm",
            0.1,
            fields,
        );
        // Chaining is OFF at 0.0 and ships that way — the row's hover (the
        // registry help of `chain_distance_mm`) says so.
        dv(
            ui,
            p(
                OperationType::ProjectCurve,
                "chain_distance_mm",
                "Chain distance",
            ),
            &mut cfg.chain_distance_mm,
            " mm",
            0.1,
            fields,
        );
    });
}
