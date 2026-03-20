use crate::state::AppState;
use crate::state::toolpath::ComputeStatus;

pub fn draw(ui: &mut egui::Ui, state: &AppState, collision_count: usize) {
    ui.horizontal(|ui| {
        let model_count = state.job.models.len();
        let tri_count: usize = state
            .job
            .models
            .iter()
            .filter_map(|m| m.mesh.as_ref().map(|mesh| mesh.triangles.len()))
            .sum();

        let computing = state
            .job
            .toolpaths
            .iter()
            .any(|tp| matches!(tp.status, ComputeStatus::Computing(_)));

        if computing {
            ui.label("Computing...");
            ui.separator();
        }

        if model_count > 0 {
            ui.label(format!("Models: {}  |  Triangles: {}", model_count, tri_count));
        } else {
            ui.label("Ready");
        }

        let tp_done = state
            .job
            .toolpaths
            .iter()
            .filter(|tp| matches!(tp.status, ComputeStatus::Done))
            .count();
        if tp_done > 0 {
            ui.separator();
            ui.label(format!("Toolpaths: {}/{}", tp_done, state.job.toolpaths.len()));
        }

        if state.simulation.active {
            ui.separator();
            ui.label(
                egui::RichText::new("SIM")
                    .color(egui::Color32::from_rgb(100, 180, 100)),
            );
        }

        if collision_count > 0 {
            ui.separator();
            ui.label(
                egui::RichText::new(format!("{} collisions", collision_count))
                    .color(egui::Color32::from_rgb(220, 80, 80)),
            );
        }

        if state.job.dirty {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Modified")
                        .italics()
                        .color(egui::Color32::from_rgb(140, 140, 100)),
                );
            });
        }
    });
}
