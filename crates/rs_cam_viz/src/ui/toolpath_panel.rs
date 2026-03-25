use std::sync::Arc;

use super::AppEvent;
use super::sim_debug::draw_trace_badge;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::job::SetupId;
use crate::state::selection::Selection;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::{ComputeStatus, OperationType, ToolpathId};
use crate::ui::theme;

/// Left panel for the Toolpath workspace: operation queue with status chips.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Operations");
    ui.separator();

    // Action bar: generate all
    ui.horizontal(|ui| {
        if ui.button("Generate All").clicked() {
            events.push(AppEvent::GenerateAll);
        }
    });

    ui.add_space(6.0);

    let multi_setup = state.job.setups.len() > 1;
    let mut global_idx = 0usize;

    for setup in &state.job.setups {
        let setup_id = setup.id;

        // Setup header (only if multi-setup)
        if multi_setup {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&setup.name)
                        .strong()
                        .color(theme::TEXT_HEADING),
                );
                // Count ready/total
                let ready = setup
                    .toolpaths
                    .iter()
                    .filter(|tp| matches!(tp.status, ComputeStatus::Done))
                    .count();
                let total = setup.toolpaths.len();
                ui.label(
                    egui::RichText::new(format!("{ready}/{total}"))
                        .small()
                        .color(theme::TEXT_DIM),
                );

                // Per-setup + Add menu
                add_toolpath_menu(ui, setup_id, events);
            });
            ui.separator();
        }

        // Drop zone for this setup
        let drop_frame = egui::Frame::default().inner_margin(2.0);
        let (inner_resp, dropped_payload) = ui.dnd_drop_zone::<ToolpathId, ()>(drop_frame, |ui| {
            if setup.toolpaths.is_empty() {
                ui.label(
                    egui::RichText::new("No toolpaths")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }

            for (local_idx, tp) in setup.toolpaths.iter().enumerate() {
                let i = global_idx;
                global_idx += 1;
                draw_toolpath_card(ui, state, events, tp, i, local_idx);
            }
        });

        // Handle drop
        if let Some(payload) = dropped_payload {
            let dragged_tp_id: ToolpathId = Arc::unwrap_or_clone(payload);
            let source_setup = state.job.setup_of_toolpath(dragged_tp_id);

            // Determine drop index from pointer position
            let drop_idx = compute_drop_index(
                &inner_resp.response,
                ui,
                setup.toolpaths.len(),
                &setup.toolpaths,
            );

            if source_setup == Some(setup_id) {
                // Same setup: reorder
                events.push(AppEvent::ReorderToolpath(dragged_tp_id, drop_idx));
            } else {
                // Different setup: move
                events.push(AppEvent::MoveToolpathToSetup(
                    dragged_tp_id,
                    setup_id,
                    drop_idx,
                ));
            }
        }
    }

    // Single-setup: show "+ Add" below toolpath list
    if !multi_setup && let Some(setup) = state.job.setups.first() {
        ui.add_space(4.0);
        add_toolpath_menu(ui, setup.id, events);
    }

    // Tool library (compact, collapsed by default)
    ui.add_space(12.0);
    egui::CollapsingHeader::new("Tool Library")
        .default_open(false)
        .show(ui, |ui| {
            for tool in &state.job.tools {
                let selected = state.selection == Selection::Tool(tool.id);
                if ui.selectable_label(selected, tool.summary()).clicked() {
                    events.push(AppEvent::Select(Selection::Tool(tool.id)));
                }
            }
            ui.add_space(4.0);
            ui.menu_button("+ Add Tool", |ui| {
                for &tt in crate::state::job::ToolType::ALL {
                    if ui.button(tt.label()).clicked() {
                        events.push(AppEvent::AddTool(tt));
                        ui.close_menu();
                    }
                }
            });
        });
}

/// Draw a single toolpath card, wrapped in a drag source.
fn draw_toolpath_card(
    ui: &mut egui::Ui,
    state: &AppState,
    events: &mut Vec<AppEvent>,
    tp: &crate::state::toolpath::ToolpathEntry,
    global_idx: usize,
    _local_idx: usize,
) {
    let selected = state.selection == Selection::Toolpath(tp.id);
    let dim = !tp.enabled || !tp.visible;

    let pc = palette_color(global_idx);
    let swatch_color = egui::Color32::from_rgb(
        (pc[0] * 255.0) as u8,
        (pc[1] * 255.0) as u8,
        (pc[2] * 255.0) as u8,
    );

    let border_color = if selected {
        swatch_color
    } else {
        egui::Color32::from_rgb(48, 48, 58)
    };

    let drag_id = egui::Id::new("tp_drag").with(tp.id.0);
    let tp_id = tp.id;

    let inner_response = ui.dnd_drag_source(drag_id, tp_id, |ui| {
        egui::Frame::default()
            .fill(if selected {
                theme::CARD_FILL_SELECTED
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::new(1.0, border_color))
            .inner_margin(4.0)
            .rounding(3.0)
            .show(ui, |ui| {
                // Row 1: swatch + status + name
                let response = ui
                    .horizontal(|ui| {
                        // Color swatch
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(6.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, swatch_color);

                        // Status chip
                        let (status_text, status_color) = match &tp.status {
                            ComputeStatus::Pending => ("PEND", theme::TEXT_DIM),
                            ComputeStatus::Computing => ("GEN", theme::WARNING),
                            ComputeStatus::Done => ("OK", theme::SUCCESS_BRIGHT),
                            ComputeStatus::Error(_) => ("ERR", theme::ERROR),
                        };
                        ui.label(
                            egui::RichText::new(status_text)
                                .small()
                                .strong()
                                .color(status_color),
                        );
                        draw_trace_badge(
                            ui,
                            SimulationState::trace_availability_for_toolpath(&state.job, tp_id),
                        );

                        // Name
                        let text_color = if dim {
                            theme::TEXT_FAINT
                        } else {
                            egui::Color32::from_rgb(190, 190, 200)
                        };
                        let resp = ui.selectable_label(
                            selected,
                            egui::RichText::new(&tp.name).color(text_color),
                        );
                        if resp.clicked() {
                            events.push(AppEvent::Select(Selection::Toolpath(tp_id)));
                        }
                        resp
                    })
                    .inner;

                // Row 2: tool info + quick actions
                ui.horizontal(|ui| {
                    // Tool name
                    if let Some(tool) = state.job.tools.iter().find(|t| t.id == tp.tool_id) {
                        ui.label(
                            egui::RichText::new(tool.summary())
                                .small()
                                .color(theme::TEXT_MUTED),
                        );
                    }

                    // Rest dependency badge
                    if let crate::state::toolpath::OperationConfig::Rest(ref rest_cfg) =
                        tp.operation
                    {
                        draw_rest_badge(ui, rest_cfg, state, tp_id);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if tp.result.is_some()
                            && ui
                                .small_button("Sim")
                                .on_hover_text("Inspect in Simulation")
                                .clicked()
                        {
                            events.push(AppEvent::InspectToolpathInSimulation(tp_id));
                        }

                        // Quick generate button
                        if matches!(tp.status, ComputeStatus::Pending)
                            && ui
                                .small_button("\u{25B6}")
                                .on_hover_text("Generate")
                                .clicked()
                        {
                            events.push(AppEvent::GenerateToolpath(tp_id));
                        }
                    });
                });

                // Context menu
                response.context_menu(|ui| {
                    if ui.button("Generate").clicked() {
                        events.push(AppEvent::GenerateToolpath(tp_id));
                        ui.close_menu();
                    }
                    if tp.result.is_some() && ui.button("Inspect in Simulation").clicked() {
                        events.push(AppEvent::InspectToolpathInSimulation(tp_id));
                        ui.close_menu();
                    }
                    let vis_label = if tp.visible { "Hide" } else { "Show" };
                    if ui.button(vis_label).clicked() {
                        events.push(AppEvent::ToggleToolpathVisibility(tp_id));
                        ui.close_menu();
                    }
                    let en_label = if tp.enabled { "Disable" } else { "Enable" };
                    if ui.button(en_label).clicked() {
                        events.push(AppEvent::ToggleToolpathEnabled(tp_id));
                        ui.close_menu();
                    }
                    if ui.button("Duplicate").clicked() {
                        events.push(AppEvent::DuplicateToolpath(tp_id));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Move Up").clicked() {
                        events.push(AppEvent::MoveToolpathUp(tp_id));
                        ui.close_menu();
                    }
                    if ui.button("Move Down").clicked() {
                        events.push(AppEvent::MoveToolpathDown(tp_id));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Delete").clicked() {
                        events.push(AppEvent::RemoveToolpath(tp_id));
                        ui.close_menu();
                    }
                });
            });
    });

    // If this card is being hovered while something is dragged, show insertion indicator
    if egui::DragAndDrop::has_payload_of_type::<ToolpathId>(ui.ctx())
        && inner_response.response.contains_pointer()
    {
        let rect = inner_response.response.rect;
        let painter = ui.painter();
        // Draw a thin line at the bottom to indicate drop position
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
    }
}

/// Compute the drop index based on pointer position relative to the drop zone.
fn compute_drop_index(
    response: &egui::Response,
    ui: &egui::Ui,
    count: usize,
    _toolpaths: &[crate::state::toolpath::ToolpathEntry],
) -> usize {
    if count == 0 {
        return 0;
    }
    // Simple heuristic: use the Y position within the drop zone
    let pointer_y = ui
        .input(|i| i.pointer.hover_pos())
        .unwrap_or(response.rect.center())
        .y;
    let zone_top = response.rect.top();
    let zone_height = response.rect.height().max(1.0);
    let fraction = ((pointer_y - zone_top) / zone_height).clamp(0.0, 1.0);
    let idx = (fraction * count as f32).round() as usize;
    idx.min(count)
}

/// Menu button for adding a toolpath to a specific setup.
/// Emits Select(Setup(id)) first, then AddToolpath, so the handler targets the right setup.
fn add_toolpath_menu(ui: &mut egui::Ui, setup_id: SetupId, events: &mut Vec<AppEvent>) {
    ui.menu_button("+ Add", |ui| {
        ui.label(egui::RichText::new("2.5D (from SVG)").strong());
        for &op in OperationType::ALL_2D {
            if ui.button(op.label()).clicked() {
                events.push(AppEvent::Select(Selection::Setup(setup_id)));
                events.push(AppEvent::AddToolpath(op));
                ui.close_menu();
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("3D (from STL)").strong());
        for &op in OperationType::ALL_3D {
            if ui.button(op.label()).clicked() {
                events.push(AppEvent::Select(Selection::Setup(setup_id)));
                events.push(AppEvent::AddToolpath(op));
                ui.close_menu();
            }
        }
    });
}

/// Show a rest dependency badge for Rest operations.
/// Green "dep" if the dependency is resolved, yellow if stale, red "no dep" if missing.
fn draw_rest_badge(
    ui: &mut egui::Ui,
    rest_cfg: &crate::state::toolpath::RestConfig,
    state: &AppState,
    tp_id: ToolpathId,
) {
    let setup_id = state.job.setup_of_toolpath(tp_id);
    let prev_tool_id = rest_cfg.prev_tool_id;

    let (badge_text, badge_color) = if let Some(prev_id) = prev_tool_id {
        // Check if there's a toolpath in the same setup using this tool
        let same_setup_has_dep = setup_id.is_some_and(|sid| {
            state
                .job
                .setups
                .iter()
                .find(|s| s.id == sid)
                .is_some_and(|setup| {
                    setup
                        .toolpaths
                        .iter()
                        .any(|other| other.id != tp_id && other.tool_id == prev_id)
                })
        });

        if same_setup_has_dep {
            // Check if the dependency toolpath is stale or pending
            let dep_stale = setup_id.is_some_and(|sid| {
                state
                    .job
                    .setups
                    .iter()
                    .find(|s| s.id == sid)
                    .is_some_and(|setup| {
                        setup.toolpaths.iter().any(|other| {
                            other.id != tp_id
                                && other.tool_id == prev_id
                                && (matches!(other.status, ComputeStatus::Pending)
                                    || other.stale_since.is_some())
                        })
                    })
            });

            if dep_stale {
                ("dep", theme::WARNING) // yellow: stale
            } else {
                ("dep", theme::SUCCESS_BRIGHT) // green: resolved
            }
        } else {
            ("no dep", theme::ERROR_MILD) // red: missing
        }
    } else {
        ("no dep", theme::ERROR_MILD) // red: not configured
    };

    ui.label(
        egui::RichText::new(badge_text)
            .small()
            .strong()
            .color(badge_color),
    );
}
