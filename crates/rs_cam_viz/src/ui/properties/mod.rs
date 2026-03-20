pub mod stock;
pub mod tool;
pub mod post;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    match state.selection.clone() {
        Selection::None => {
            ui.label(
                egui::RichText::new("Select an item in the project tree")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        Selection::Stock => {
            stock::draw(ui, &mut state.job.stock, events);
        }
        Selection::PostProcessor => {
            post::draw(ui, &mut state.job.post);
        }
        Selection::Model(id) => {
            if let Some(model) = state.job.models.iter().find(|m| m.id == id) {
                ui.heading(&model.name);
                ui.separator();
                ui.label(format!("Type: {:?}", model.kind));
                ui.label(format!("Path: {}", model.path.display()));
                if let Some(mesh) = &model.mesh {
                    ui.add_space(4.0);
                    ui.label(format!("Vertices: {}", mesh.vertices.len()));
                    ui.label(format!("Triangles: {}", mesh.triangles.len()));
                    let bb = &mesh.bbox;
                    ui.label(format!(
                        "Bounds: {:.1} x {:.1} x {:.1} mm",
                        bb.max.x - bb.min.x,
                        bb.max.y - bb.min.y,
                        bb.max.z - bb.min.z
                    ));
                }
            }
        }
        Selection::Tool(id) => {
            if let Some(tool) = state.job.tools.iter_mut().find(|t| t.id == id) {
                tool::draw(ui, tool);
            }
        }
    }
}
