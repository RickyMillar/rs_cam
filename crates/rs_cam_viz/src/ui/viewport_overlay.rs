use super::AppEvent;
use crate::render::camera::ViewPreset;
use crate::state::simulation::SimulationState;
use crate::state::viewport::{RenderMode, ViewportState};

pub fn draw(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    viewport: &mut ViewportState,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        // View presets
        for (label, preset) in [
            ("Top", ViewPreset::Top),
            ("Front", ViewPreset::Front),
            ("Right", ViewPreset::Right),
            ("Iso", ViewPreset::Isometric),
        ] {
            if ui.small_button(label).clicked() {
                events.push(AppEvent::SetViewPreset(preset));
            }
        }

        ui.separator();

        // Render mode toggle
        let mode_label = match viewport.render_mode {
            RenderMode::Shaded => "Shaded",
            RenderMode::Wireframe => "Wire",
        };
        if ui.small_button(mode_label).clicked() {
            viewport.render_mode = match viewport.render_mode {
                RenderMode::Shaded => RenderMode::Wireframe,
                RenderMode::Wireframe => RenderMode::Shaded,
            };
        }

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
        } else if ui.small_button("Simulate").clicked() {
            events.push(AppEvent::RunSimulation);
        }
    });
}
