//! The strip above the 3D view: camera, projection, the Overlays button,
//! isolation, the compute indicator and the per-workspace action.
//!
//! ## What P6 removed from this file
//!
//! `Show ▼` was a flat popover of twelve unrelated things — the grid, the
//! stock, fixtures, curves, paths, rapids, collisions, a span-kind submenu,
//! the rest heatmap, the tier preview, the tool ghost and a colour combo. Its
//! entries are registry rows now (`crate::ui::overlays::registry`), and the
//! `Overlays` button opens the panel that lists them all — including the
//! nineteen overlays no control named at all.
//!
//! `Shaded ▼` went with it. Its `Wireframe` arm drew NOTHING: the crate has
//! no wireframe pipeline, so the mode hid an STL model and left a STEP model
//! untouched (audit §3.1). The model is a plain visibility row now.
//!
//! The rest heatmap legend moved into the panel's legend block, beside
//! legends for the five colour-carrying overlays that had none.

use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::{ProjectionMode, ViewPreset};
use crate::state::{AppState, Workspace};
use crate::ui::automation;
use crate::ui::overlays::panel;
use crate::ui::theme;
use crate::ui_command::{NoArgs, UiCommand};

pub fn draw(
    ui: &mut egui::Ui,
    state: &mut AppState,
    projection: ProjectionMode,
    lanes: &[LaneSnapshot; 4],
    events: &mut Vec<AppEvent>,
) {
    let workspace = state.workspace;
    let isolated_name = state.viewport.isolate_toolpath.and_then(|tp_id| {
        state
            .session
            .toolpath_configs()
            .iter()
            .find(|tc| tc.id == tp_id)
            .map(|tc| tc.name.clone())
    });

    ui.horizontal_wrapped(|ui| {
        // ── View dropdown: presets + reset ──────────────────────
        ui.menu_button("View ▼", |ui| {
            if ui.button("Top").clicked() {
                events.push(AppEvent::Ui(UiCommand::SetViewPreset(ViewPreset::Top)));
                ui.close();
            }
            if ui.button("Front").clicked() {
                events.push(AppEvent::Ui(UiCommand::SetViewPreset(ViewPreset::Front)));
                ui.close();
            }
            if ui.button("Right").clicked() {
                events.push(AppEvent::Ui(UiCommand::SetViewPreset(ViewPreset::Right)));
                ui.close();
            }
            if ui.button("Iso").clicked() {
                events.push(AppEvent::Ui(UiCommand::SetViewPreset(
                    ViewPreset::Isometric,
                )));
                ui.close();
            }
            ui.separator();
            if ui.button("Reset view").clicked() {
                events.push(AppEvent::Ui(UiCommand::ResetView(NoArgs)));
                ui.close();
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
                    events.push(AppEvent::Ui(UiCommand::ToggleProjection(NoArgs)));
                }
                ui.close();
            }
            if ui
                .selectable_label(
                    matches!(projection, ProjectionMode::Orthographic),
                    "Orthographic",
                )
                .clicked()
            {
                if !matches!(projection, ProjectionMode::Orthographic) {
                    events.push(AppEvent::Ui(UiCommand::ToggleProjection(NoArgs)));
                }
                ui.close();
            }
        });

        // ── Overlays panel button ───────────────────────────────
        let overlays_button = panel::toolbar_button(ui, state);
        // The automation harness locates the collision toggle through this
        // label. It was registered on `Show ▼`; the Collisions row lives
        // behind this button now, so the label moves with it rather than
        // being deleted (UX §8, "two automation labels must survive").
        automation::record(
            ui,
            "overlay_collision_check",
            &overlays_button,
            "Overlays (collisions)",
        );

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
                events.push(AppEvent::Ui(UiCommand::ClearIsolation(NoArgs)));
            }
        } else if ui
            .small_button("Isolate")
            .on_hover_text("Show only the selected toolpath (shortcut: I)")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::ToggleIsolateToolpath(NoArgs)));
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
            let cancel_resp = ui.small_button("Cancel All");
            automation::record(ui, "overlay_cancel_all", &cancel_resp, "Cancel All");
            if cancel_resp.clicked() {
                events.push(AppEvent::Ui(UiCommand::CancelCompute(NoArgs)));
            }
        }

        // ── Workspace-specific actions (right-aligned) ──────────
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| match workspace {
                // Readiness has no viewport, so this overlay never renders there.
                Workspace::Setup | Workspace::Readiness => {}
                Workspace::Toolpaths => {
                    if ui.small_button("Generate All").clicked() {
                        events.push(AppEvent::GenerateAll);
                    }
                }
                Workspace::Simulation => {
                    if ui.small_button("Reset").clicked() {
                        events.push(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));
                    }
                    if ui.small_button("Re-run").clicked() {
                        events.push(AppEvent::RunSimulation);
                    }
                }
            },
        );
    });
}
