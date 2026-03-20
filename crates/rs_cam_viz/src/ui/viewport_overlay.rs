use super::AppEvent;
use crate::render::camera::ViewPreset;
use crate::render::toolpath_render::palette_color;
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

    // Simulation timeline scrubber and per-toolpath progress (second+ rows)
    if sim.active && sim.total_moves > 0 {
        // Timeline scrubber
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

        // Current operation info
        if let Some(boundary) = sim.current_boundary() {
            let (within, total) = sim.current_toolpath_progress();
            ui.horizontal(|ui| {
                // Find boundary index for palette color
                let boundary_idx = sim.boundaries.iter().position(|b| b.id == boundary.id).unwrap_or(0);
                let pc = palette_color(boundary_idx);
                let color = egui::Color32::from_rgb(
                    (pc[0] * 255.0) as u8,
                    (pc[1] * 255.0) as u8,
                    (pc[2] * 255.0) as u8,
                );
                ui.label(egui::RichText::new("\u{25CF}").color(color));
                ui.label(format!(
                    "{} ({}) \u{2014} {}/{}",
                    boundary.name, boundary.tool_name, within, total,
                ));
            });
        }

        // Tool position readout
        if let Some(pos) = sim.tool_position {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Tool: X{:.2} Y{:.2} Z{:.2}",
                        pos[0], pos[1], pos[2]
                    ))
                    .color(egui::Color32::from_rgb(180, 180, 100))
                    .small(),
                );
            });
        }

        // Per-toolpath progress segments
        if sim.boundaries.len() > 1 {
            ui.horizontal(|ui| {
                for (i, boundary) in sim.boundaries.iter().enumerate() {
                    let pc = palette_color(i);
                    let progress = if sim.current_move >= boundary.end_move {
                        1.0
                    } else if sim.current_move <= boundary.start_move {
                        0.0
                    } else {
                        (sim.current_move - boundary.start_move) as f32
                            / (boundary.end_move - boundary.start_move).max(1) as f32
                    };
                    let color = egui::Color32::from_rgb(
                        (pc[0] * 255.0) as u8,
                        (pc[1] * 255.0) as u8,
                        (pc[2] * 255.0) as u8,
                    );
                    let dim_color = egui::Color32::from_rgb(
                        (pc[0] * 80.0) as u8,
                        (pc[1] * 80.0) as u8,
                        (pc[2] * 80.0) as u8,
                    );

                    // Draw a small progress bar segment
                    let width = 40.0;
                    let height = 6.0;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, dim_color);
                    let filled_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(width * progress, height),
                    );
                    ui.painter().rect_filled(filled_rect, 2.0, color);
                }
            });
        }
    }
}
