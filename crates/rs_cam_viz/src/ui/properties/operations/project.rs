use crate::state::job::ModelId;
use crate::state::toolpath::ProjectCurveConfig;

use super::super::dv;
use super::draw_feed_params;

pub(in crate::ui::properties) fn draw_project_curve_params(
    ui: &mut egui::Ui,
    cfg: &mut ProjectCurveConfig,
    models: &[(ModelId, String)],
) {
    ui.label(
        egui::RichText::new("Projects 2D curves onto 3D mesh")
            .italics()
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );

    // Surface model selector — lets the user pick a different model for the 3D surface.
    ui.horizontal(|ui| {
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
    });

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
