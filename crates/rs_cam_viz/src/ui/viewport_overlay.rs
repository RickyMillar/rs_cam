use super::AppEvent;
use crate::render::camera::ViewPreset;
use crate::state::simulation::SimulationState;
use crate::state::viewport::{RenderMode, ViewportState};

pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
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

        // Computing indicator
        if let Some(secs) = compute_elapsed {
            ui.label(
                egui::RichText::new(format!("Computing {:.1}s", secs))
                    .color(egui::Color32::from_rgb(200, 180, 80)),
            );
            ui.separator();
        }

        // Simulation controls
        if sim.active {
            let play_label = if sim.playing { "Pause" } else { "Play" };
            if ui.small_button(play_label).clicked() {
                events.push(AppEvent::ToggleSimPlayback);
            }
            if ui.small_button("Reset").clicked() {
                events.push(AppEvent::ResetSimulation);
            }
        } else if ui.small_button("Simulate").clicked() {
            events.push(AppEvent::RunSimulation);
        }
    });

    // Simulation timeline scrubber (second row, only when sim is active)
    if sim.active && sim.total_moves > 0 {
        ui.horizontal(|ui| {
            ui.label("Timeline:");
            let mut pos = sim.current_move as f32;
            let slider = egui::Slider::new(&mut pos, 0.0..=sim.total_moves as f32)
                .show_value(false)
                .step_by(1.0);
            if ui.add(slider).changed() {
                sim.current_move = pos as usize;
                sim.playing = false; // pause when scrubbing
            }
            ui.label(format!(
                "{} / {}  ({:.0}%)",
                sim.current_move,
                sim.total_moves,
                sim.progress() * 100.0,
            ));

            // Speed control
            ui.separator();
            ui.label("Speed:");
            ui.add(
                egui::DragValue::new(&mut sim.speed)
                    .range(10.0..=50000.0)
                    .speed(50.0)
                    .suffix(" mv/s"),
            );
        });
    }
}
