use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::ViewPreset;
use crate::state::AppMode;
use crate::state::viewport::{RenderMode, ViewportState};
use crate::ui::automation;

pub fn draw(
    ui: &mut egui::Ui,
    mode: AppMode,
    sim_active: bool,
    viewport: &mut ViewportState,
    lanes: &[LaneSnapshot; 2],
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
        ui.checkbox(&mut viewport.show_fixtures, "Fix");

        ui.separator();

        let active_lanes: Vec<_> = lanes.iter().filter(|lane| lane.is_active()).collect();
        if !active_lanes.is_empty() {
            let label = active_lanes
                .iter()
                .map(|lane| {
                    lane.current_job
                        .clone()
                        .unwrap_or_else(|| "Working".to_string())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            ui.label(egui::RichText::new(label).color(egui::Color32::from_rgb(200, 180, 80)));
            let cancel = ui.small_button("Cancel All");
            automation::record(ui, "overlay_cancel_all", &cancel, "Cancel All");
            if cancel.clicked() {
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
            } else {
                let simulate = ui.small_button("Simulate");
                automation::record(ui, "overlay_simulate", &simulate, "Simulate");
                if simulate.clicked() {
                    events.push(AppEvent::RunSimulation);
                }
            }

            let collision = ui.small_button("Collision");
            automation::record(ui, "overlay_collision_check", &collision, "Collision");
            if collision.clicked() {
                events.push(AppEvent::RunCollisionCheck);
            }
        }
    });
}
