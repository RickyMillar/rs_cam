use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::{ProjectionMode, ViewPreset};
use crate::state::Workspace;
use crate::state::viewport::{RenderMode, ToolpathColorMode, ViewportState};
use crate::ui::theme;

// SAFETY: viewport overlay needs the full UI context (workspace, sim flag,
// projection, isolation label, mutable viewport state, lane snapshots, and
// event sink). Bundling them into a struct would just rename the same data
// for one call site.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    workspace: Workspace,
    _sim_active: bool,
    projection: ProjectionMode,
    isolated_name: Option<&str>,
    viewport: &mut ViewportState,
    lanes: &[LaneSnapshot; 3],
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal_wrapped(|ui| {
        // ── View dropdown: presets + reset ──────────────────────
        ui.menu_button("View ▼", |ui| {
            if ui.button("Top").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Top));
                ui.close_menu();
            }
            if ui.button("Front").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Front));
                ui.close_menu();
            }
            if ui.button("Right").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Right));
                ui.close_menu();
            }
            if ui.button("Iso").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Isometric));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Reset view").clicked() {
                events.push(AppEvent::ResetView);
                ui.close_menu();
            }
        });

        // ── Render mode: Shaded / Wire ──────────────────────────
        let shade_label = match viewport.render_mode {
            RenderMode::Shaded => "Shaded ▼",
            RenderMode::Wireframe => "Wire ▼",
        };
        ui.menu_button(shade_label, |ui| {
            if ui
                .selectable_label(viewport.render_mode == RenderMode::Shaded, "Shaded")
                .clicked()
            {
                viewport.render_mode = RenderMode::Shaded;
                ui.close_menu();
            }
            if ui
                .selectable_label(viewport.render_mode == RenderMode::Wireframe, "Wireframe")
                .clicked()
            {
                viewport.render_mode = RenderMode::Wireframe;
                ui.close_menu();
            }
        });

        // ── Projection: Persp / Ortho ───────────────────────────
        let proj_label = match projection {
            ProjectionMode::Perspective => "Persp ▼",
            ProjectionMode::Orthographic => "Ortho ▼",
        };
        ui.menu_button(proj_label, |ui| {
            if ui
                .selectable_label(
                    matches!(projection, ProjectionMode::Perspective),
                    "Perspective",
                )
                .clicked()
            {
                if !matches!(projection, ProjectionMode::Perspective) {
                    events.push(AppEvent::ToggleProjection);
                }
                ui.close_menu();
            }
            if ui
                .selectable_label(
                    matches!(projection, ProjectionMode::Orthographic),
                    "Orthographic",
                )
                .clicked()
            {
                if !matches!(projection, ProjectionMode::Orthographic) {
                    events.push(AppEvent::ToggleProjection);
                }
                ui.close_menu();
            }
        });

        // ── Show dropdown: all visibility toggles in one popover ─
        ui.menu_button("Show ▼", |ui| {
            ui.set_min_width(180.0);
            ui.checkbox(&mut viewport.show_grid, "Grid");
            ui.checkbox(&mut viewport.show_stock, "Stock");
            ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
            ui.checkbox(&mut viewport.show_polygons, "Curves (DXF/SVG)");
            ui.separator();
            ui.checkbox(&mut viewport.show_cutting, "Paths (cutting)");
            ui.checkbox(&mut viewport.show_rapids, "Rapids");
            ui.checkbox(&mut viewport.show_collisions, "Collisions");
            ui.separator();
            ui.checkbox(&mut viewport.show_tool_profile_preview, "Tool-profile ghost");
            ui.horizontal(|ui| {
                ui.label("Toolpath color:");
                egui::ComboBox::from_id_salt("toolpath_color_mode")
                    .selected_text(match viewport.toolpath_color_mode {
                        ToolpathColorMode::Normal => "Palette",
                        ToolpathColorMode::Engagement => "Engagement",
                        ToolpathColorMode::Chipload => "Chipload",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Normal,
                            "Palette",
                        )
                        .on_hover_text("Per-toolpath palette color with Z-depth blending");
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Engagement,
                            "Engagement",
                        )
                        .on_hover_text(
                            "Color cutting moves by feed rate: green→yellow→red for light→heavy load",
                        );
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Chipload,
                            "Chipload",
                        )
                        .on_hover_text(
                            "Color each segment by per-sample chipload vs the matched vendor row's window",
                        );
                    });
            });
        });

        // ── Isolate button ──────────────────────────────────────
        if let Some(name) = isolated_name {
            // Active state: show the isolated toolpath name + clear button
            ui.label(
                egui::RichText::new(format!("\u{25CE} {}", name))
                    .color(theme::WARNING)
                    .strong(),
            )
            .on_hover_text("Currently showing only this toolpath. Click ✕ to clear.");
            if ui
                .small_button("✕")
                .on_hover_text("Clear isolation (show all toolpaths)")
                .clicked()
            {
                events.push(AppEvent::ClearIsolation);
            }
        } else if ui
            .small_button("Isolate")
            .on_hover_text("Show only the selected toolpath (shortcut: I)")
            .clicked()
        {
            events.push(AppEvent::ToggleIsolateToolpath);
        }

        // ── Compute activity indicator (right side) ─────────────
        let active_lanes: Vec<_> = lanes.iter().filter(|lane| lane.is_active()).collect();
        if !active_lanes.is_empty() {
            ui.separator();
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
            if ui.small_button("Cancel All").clicked() {
                events.push(AppEvent::CancelCompute);
            }
        }

        // ── Workspace-specific actions (right-aligned) ──────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match workspace {
            Workspace::Setup => {}
            Workspace::Toolpaths => {
                if ui.small_button("Generate All").clicked() {
                    events.push(AppEvent::GenerateAll);
                }
            }
            Workspace::Simulation => {
                if ui.small_button("Reset").clicked() {
                    events.push(AppEvent::ResetSimulation);
                }
                if ui.small_button("Re-run").clicked() {
                    events.push(AppEvent::RunSimulation);
                }
            }
        });
    });
}
