//! The strip above the 3D view: camera, projection, the Overlays button,
//! the draw scope, the compute indicator and the per-workspace action.
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
    lanes: &[LaneSnapshot; 5],
    events: &mut Vec<AppEvent>,
) {
    let workspace = state.workspace;
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

        // ── Draw scope: selected only / all toolpaths (WP27) ────
        let (scope_label, scope_hover) = if state.viewport.show_all_toolpaths {
            (
                "All toolpaths",
                "The viewport draws every toolpath. Click to draw the selected one only.",
            )
        } else {
            (
                "Selected only",
                "The viewport draws the selected toolpath only. Click to draw them all.",
            )
        };
        if ui
            .small_button(scope_label)
            .on_hover_text(scope_hover)
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::ToggleShowAllToolpaths(NoArgs)));
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
                // Q5, operator ruling 2026-09-14. "Generate All" used to sit
                // here TOO, about 700 points from the identical action in the
                // Operations panel, where it is now the workspace's one
                // Primary. Two copies of one action on one screen is what
                // §4.5's "at most one Primary" exists to stop — the second
                // copy does not add reach, it removes the first one's
                // meaning. The menu keeps its entry, which is conventional
                // and is not on screen at the same time.
                Workspace::Toolpaths => {}
                Workspace::Simulation => {
                    if ui.small_button("Reset").clicked() {
                        events.push(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));
                    }
                }
            },
        );
    });
}
