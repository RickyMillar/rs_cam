use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::ViewPreset;
use crate::state::Workspace;
use crate::state::viewport::{RenderMode, ToolpathColorMode, ViewportState};
use crate::ui::automation;
use crate::ui::theme;

pub fn draw(
    ui: &mut egui::Ui,
    workspace: Workspace,
    sim_active: bool,
    viewport: &mut ViewportState,
    lanes: &[LaneSnapshot; 2],
    events: &mut Vec<AppEvent>,
) {
    // Top row: view presets + render mode + visibility + sim controls
    ui.horizontal_wrapped(|ui| {
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
        ui.checkbox(&mut viewport.show_cutting, "Paths")
            .on_hover_text("Show cutting moves");
        ui.checkbox(&mut viewport.show_rapids, "Rapids")
            .on_hover_text("Show rapid moves");
        ui.checkbox(&mut viewport.show_collisions, "Collisions")
            .on_hover_text("Show collisions");
        ui.checkbox(&mut viewport.show_fixtures, "Fixtures")
            .on_hover_text("Show fixtures");
        ui.checkbox(&mut viewport.show_polygons, "Curves")
            .on_hover_text("Show DXF/SVG polygon outlines");

        // Toolpath color mode
        let engagement_on = viewport.toolpath_color_mode == ToolpathColorMode::Engagement;
        let mut engagement_checked = engagement_on;
        ui.checkbox(&mut engagement_checked, "Engagement")
            .on_hover_text("Color toolpath lines by feed rate: green = light cut, yellow = moderate, red = heavy engagement");
        if engagement_checked != engagement_on {
            viewport.toolpath_color_mode = if engagement_checked {
                ToolpathColorMode::Engagement
            } else {
                ToolpathColorMode::Normal
            };
        }

        ui.separator();

        let active_lanes: Vec<_> = lanes.iter().filter(|lane| lane.is_active()).collect();
        if !active_lanes.is_empty() {
            let label = active_lanes
                .iter()
                .map(|lane| {
                    lane.current_job
                        .clone()
                        .unwrap_or_else(|| "Working".to_owned())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            ui.label(egui::RichText::new(label).color(theme::WARNING));
            let cancel = ui.small_button("Cancel All");
            automation::record(ui, "overlay_cancel_all", &cancel, "Cancel All");
            if cancel.clicked() {
                events.push(AppEvent::CancelCompute);
            }
            ui.separator();
        }

        // Simulation controls — only shown outside Simulation workspace (sim has its own)
        if matches!(workspace, Workspace::Setup | Workspace::Toolpaths) {
            if sim_active {
                if ui.small_button("Open Simulation").clicked() {
                    events.push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                }
                if ui.small_button("Reset").clicked() {
                    events.push(AppEvent::ResetSimulation);
                }
            } else {
                let simulate = ui.small_button("Run Simulation");
                automation::record(ui, "overlay_simulate", &simulate, "Run Simulation");
                if simulate.clicked() {
                    events.push(AppEvent::RunSimulation);
                }
            }

            let collision = ui.small_button("Check Holder");
            automation::record(ui, "overlay_collision_check", &collision, "Check Holder");
            if collision.clicked() {
                events.push(AppEvent::RunCollisionCheck);
            }
        }
    });
}
