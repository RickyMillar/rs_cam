use super::AppEvent;
use crate::render::camera::ViewPreset;
use crate::state::AppMode;
use crate::state::viewport::{RenderMode, ViewportState};

pub fn draw(
    ui: &mut egui::Ui,
    mode: AppMode,
    sim_active: bool,
    viewport: &mut ViewportState,
    compute_elapsed: Option<f32>,
    events: &mut Vec<AppEvent>,
) {
    // Top row: view presets + render mode + visibility + sim controls
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

        // Render mode
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

        // Visibility toggles
        ui.checkbox(&mut viewport.show_cutting, "Cut");
        ui.checkbox(&mut viewport.show_rapids, "Rapid");
        ui.checkbox(&mut viewport.show_collisions, "Col");

        ui.separator();

        // Computing indicator with cancel button
        if let Some(secs) = compute_elapsed {
            ui.label(
                egui::RichText::new(format!("Computing {:.1}s", secs))
                    .color(egui::Color32::from_rgb(200, 180, 80)),
            );
            if ui.small_button("Cancel").clicked() {
                events.push(AppEvent::CancelCompute);
            }
            ui.separator();
        }

        // Simulation controls — only shown in Editor mode (sim workspace has its own)
        if mode == AppMode::Editor {
            if sim_active {
                if ui.small_button("Open Sim").clicked() {
                    events.push(AppEvent::EnterSimulation);
                }
                if ui.small_button("Reset").clicked() {
                    events.push(AppEvent::ResetSimulation);
                }
            } else if ui.small_button("Simulate").clicked() {
                events.push(AppEvent::RunSimulation);
            }
        }
    });
}
