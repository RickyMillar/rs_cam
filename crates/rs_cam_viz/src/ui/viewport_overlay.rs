use super::AppEvent;
use crate::render::camera::ViewPreset;
use crate::state::simulation::SimulationState;

pub fn draw(ui: &mut egui::Ui, sim: &SimulationState, events: &mut Vec<AppEvent>) {
    // View preset buttons (top-left)
    ui.horizontal(|ui| {
        let btn = |ui: &mut egui::Ui, label: &str, preset: ViewPreset, events: &mut Vec<AppEvent>| {
            if ui.small_button(label).clicked() {
                events.push(AppEvent::SetViewPreset(preset));
            }
        };
        btn(ui, "Top", ViewPreset::Top, events);
        btn(ui, "Front", ViewPreset::Front, events);
        btn(ui, "Right", ViewPreset::Right, events);
        btn(ui, "Iso", ViewPreset::Isometric, events);

        ui.separator();

        // Simulation controls
        if sim.active {
            if ui.small_button("Reset Sim").clicked() {
                events.push(AppEvent::ResetSimulation);
            }
            ui.label(
                egui::RichText::new(format!("Sim: {:.0}%", sim.progress() * 100.0))
                    .color(egui::Color32::from_rgb(100, 180, 100)),
            );
        } else {
            if ui.small_button("Simulate").clicked() {
                events.push(AppEvent::RunSimulation);
            }
        }
    });
}
