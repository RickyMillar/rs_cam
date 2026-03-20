use crate::state::AppState;

pub fn draw(ui: &mut egui::Ui, state: &AppState) {
    ui.horizontal(|ui| {
        let model_count = state.job.models.len();
        let tri_count: usize = state
            .job
            .models
            .iter()
            .filter_map(|m| m.mesh.as_ref().map(|mesh| mesh.triangles.len()))
            .sum();

        if model_count > 0 {
            ui.label(format!(
                "Models: {}  |  Triangles: {}",
                model_count, tri_count
            ));
        } else {
            ui.label("Ready  |  File > Import STL to get started");
        }
    });
}
