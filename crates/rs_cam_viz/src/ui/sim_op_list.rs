use super::AppEvent;
use super::sim_debug::{draw_trace_badge, semantic_kind_color, semantic_kind_label};
use crate::render::toolpath_render::palette_color;
use crate::state::job::{JobState, SetupId};
use crate::state::simulation::SimulationState;
use crate::state::toolpath::ToolpathId;

/// Left panel in simulation workspace: operation list with checkboxes, progress bars, and jump buttons.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Verification");
    ui.separator();

    // Empty state: no results yet
    if sim.boundaries().is_empty() {
        let has_computed = job
            .all_toolpaths()
            .any(|tp| tp.enabled && tp.result.is_some());

        egui::Frame::default()
            .fill(egui::Color32::from_rgb(36, 36, 44))
            .inner_margin(12.0)
            .rounding(4.0)
            .show(ui, |ui| {
                if has_computed {
                    ui.label(
                        egui::RichText::new("Ready to simulate")
                            .strong()
                            .color(egui::Color32::from_rgb(180, 180, 195)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Run simulation to verify toolpaths, check collisions, and review stock removal.",
                        )
                        .small()
                        .color(egui::Color32::from_rgb(140, 140, 155)),
                    );
                    ui.add_space(8.0);
                    if ui.button("Run Simulation").clicked() {
                        events.push(AppEvent::RunSimulation);
                    }
                } else {
                    ui.label(
                        egui::RichText::new("No toolpaths computed")
                            .color(egui::Color32::from_rgb(180, 140, 80)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Switch to Toolpaths workspace to add and generate operations first.",
                        )
                        .small()
                        .color(egui::Color32::from_rgb(140, 140, 155)),
                    );
                    ui.add_space(8.0);
                    if ui.button("Go to Toolpaths").clicked() {
                        events.push(AppEvent::SwitchWorkspace(
                            crate::state::Workspace::Toolpaths,
                        ));
                    }
                }
            });
        return;
    }

    // Staleness warning
    if sim.is_stale(job.edit_counter) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("\u{26A0} Results may be stale")
                    .color(egui::Color32::from_rgb(220, 180, 60)),
            );
        });
        if ui.small_button("Re-run").clicked() {
            events.push(AppEvent::RunSimulation);
        }
        ui.separator();
    }

    // Collect selected toolpath IDs for checkbox state
    let all_selected = sim.selected_toolpaths().is_none();
    let selected_set: Vec<ToolpathId> = sim.selected_toolpaths().cloned().unwrap_or_default();
    let boundaries = sim.boundaries().to_vec();
    let setup_boundaries = sim.setup_boundaries().to_vec();
    sim.sync_debug_state(job);
    let active_item = sim.active_semantic_item(job);
    let active_item_id = active_item
        .as_ref()
        .map(|item| (item.toolpath_id, item.item.id));

    // Track if user toggled any checkbox
    let mut toggled_id: Option<ToolpathId> = None;
    let mut current_setup_id: Option<SetupId> = None;

    for (i, boundary) in boundaries.iter().enumerate() {
        // Insert setup transition divider when the setup changes
        let this_setup = setup_boundaries
            .iter()
            .rev()
            .find(|sb| sb.start_move <= boundary.start_move);
        if let Some(sb) = this_setup
            && current_setup_id != Some(sb.setup_id)
        {
            current_setup_id = Some(sb.setup_id);
            if i > 0 {
                ui.add_space(4.0);
                ui.separator();
            }
            ui.label(
                egui::RichText::new(&sb.setup_name)
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 200)),
            );
            ui.add_space(2.0);
        }
        let is_current = sim.current_boundary().map(|b| b.id) == Some(boundary.id);
        let pc = palette_color(i);
        let color = egui::Color32::from_rgb(
            (pc[0] * 255.0) as u8,
            (pc[1] * 255.0) as u8,
            (pc[2] * 255.0) as u8,
        );

        // Frame current operation with accent border
        let frame = if is_current {
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(38, 42, 55))
                .stroke(egui::Stroke::new(1.0, color))
                .inner_margin(4.0)
                .rounding(3.0)
        } else {
            egui::Frame::default().inner_margin(4.0)
        };

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Checkbox for including in simulation
                let mut checked = all_selected || selected_set.contains(&boundary.id);
                if ui.checkbox(&mut checked, "").changed() {
                    toggled_id = Some(boundary.id);
                }

                // Palette color swatch
                let (swatch_rect, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch_rect, 2.0, color);

                // Operation name (bold if current)
                let name_text = egui::RichText::new(&boundary.name).small();
                let name_text = if is_current {
                    name_text.strong()
                } else {
                    name_text
                };
                ui.label(name_text);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    draw_trace_badge(
                        ui,
                        SimulationState::trace_availability_for_toolpath(job, boundary.id),
                    );
                });
            });

            // Tool name + time estimate on same row
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&boundary.tool_name)
                        .small()
                        .color(egui::Color32::from_rgb(140, 140, 150)),
                );

                // Estimated time from job toolpaths
                if let Some(est) = estimate_op_time(job, boundary.id) {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(est)
                                .small()
                                .color(egui::Color32::from_rgb(100, 140, 100)),
                        );
                    });
                }
            });

            // Progress bar
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let progress = if op_moves == 0 {
                0.0
            } else if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f32 / op_moves as f32
            };

            let bar_height = 4.0;
            let bar_width = ui.available_width();
            let (bar_rect, _) =
                ui.allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::hover());
            let dim_color = egui::Color32::from_rgb(
                (pc[0] * 60.0) as u8,
                (pc[1] * 60.0) as u8,
                (pc[2] * 60.0) as u8,
            );
            ui.painter().rect_filled(bar_rect, 2.0, dim_color);
            let filled = egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(bar_width * progress, bar_height),
            );
            ui.painter().rect_filled(filled, 2.0, color);

            // Jump buttons + move info
            ui.horizontal(|ui| {
                if ui
                    .small_button("|<")
                    .on_hover_text("Jump to op start")
                    .clicked()
                {
                    events.push(AppEvent::SimJumpToOpStart(i));
                }
                let move_info = format!(
                    "{} / {}",
                    sim.playback
                        .current_move
                        .saturating_sub(boundary.start_move)
                        .min(op_moves),
                    op_moves,
                );
                ui.label(
                    egui::RichText::new(move_info)
                        .small()
                        .color(egui::Color32::from_rgb(140, 140, 150)),
                );
                if ui
                    .small_button(">|")
                    .on_hover_text("Jump to op end")
                    .clicked()
                {
                    events.push(AppEvent::SimJumpToOpEnd(i));
                }
            });

            if sim.debug.enabled {
                let has_semantic = job
                    .find_toolpath(boundary.id)
                    .and_then(|toolpath| toolpath.semantic_trace.as_ref())
                    .is_some();
                if is_current && has_semantic {
                    sim.debug.set_toolpath_expanded(boundary.id, true);
                }

                if has_semantic {
                    ui.add_space(4.0);
                    let expanded = sim.debug.is_toolpath_expanded(boundary.id);
                    let toggle_label = if expanded {
                        "Hide semantics"
                    } else {
                        "Show semantics"
                    };
                    if ui
                        .small_button(toggle_label)
                        .on_hover_text("Expand semantic trace for this toolpath")
                        .clicked()
                    {
                        sim.debug.toggle_toolpath_expanded(boundary.id);
                    }

                    if sim.debug.is_toolpath_expanded(boundary.id) {
                        draw_semantic_outline(ui, sim, job, boundary, active_item_id, events);
                    }
                }
            }
        });

        if i + 1 < boundaries.len() {
            ui.add_space(2.0);
        }
    }

    // If a checkbox was toggled, re-run sim with new selection
    if let Some(id) = toggled_id {
        let mut new_selection: Vec<ToolpathId> = if all_selected {
            // Was "all" — now exclude the toggled one
            sim.boundaries()
                .iter()
                .map(|b| b.id)
                .filter(|bid| *bid != id)
                .collect()
        } else {
            let mut s = selected_set;
            if s.contains(&id) {
                s.retain(|x| *x != id);
            } else {
                s.push(id);
            }
            s
        };

        // If all are selected again, use None (meaning "all")
        if new_selection.len() == boundaries.len() {
            new_selection.clear();
        }

        if new_selection.is_empty() {
            events.push(AppEvent::RunSimulation);
        } else {
            events.push(AppEvent::RunSimulationWith(new_selection));
        }
    }

    fn draw_semantic_outline(
        ui: &mut egui::Ui,
        sim: &mut SimulationState,
        job: &JobState,
        boundary: &crate::state::simulation::ToolpathBoundary,
        active_item_id: Option<(ToolpathId, u64)>,
        events: &mut Vec<AppEvent>,
    ) {
        let Some(toolpath) = job.find_toolpath(boundary.id) else {
            return;
        };
        let Some(trace) = toolpath.semantic_trace.as_ref() else {
            return;
        };
        let Some(index) = sim.debug.semantic_indexes.get(&boundary.id).cloned() else {
            return;
        };
        let root_items = index
            .child_indices_by_parent
            .get(&None)
            .cloned()
            .unwrap_or_default();
        if root_items.is_empty() {
            ui.label(
                egui::RichText::new("No move-linked semantics")
                    .small()
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
            return;
        }

        ui.add_space(2.0);
        for item_index in root_items {
            draw_semantic_item_row(
                ui,
                trace,
                &index,
                sim,
                boundary,
                item_index,
                0,
                active_item_id,
                events,
            );
        }
    }

    // SAFETY: item_index from recursive traversal of trace.items children
    #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
    fn draw_semantic_item_row(
        ui: &mut egui::Ui,
        trace: &rs_cam_core::semantic_trace::ToolpathSemanticTrace,
        index: &crate::state::simulation::SimulationSemanticIndex,
        sim: &mut SimulationState,
        boundary: &crate::state::simulation::ToolpathBoundary,
        item_index: usize,
        depth: usize,
        active_item_id: Option<(ToolpathId, u64)>,
        events: &mut Vec<AppEvent>,
    ) {
        let item = &trace.items[item_index];
        let color = semantic_kind_color(&item.kind);
        let is_active = active_item_id == Some((boundary.id, item.id));

        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            ui.label(
                egui::RichText::new(semantic_kind_label(&item.kind))
                    .small()
                    .color(color),
            );

            let text = if is_active {
                egui::RichText::new(&item.label).small().strong()
            } else {
                egui::RichText::new(&item.label).small()
            };
            let response = ui.selectable_label(is_active, text);
            if response.clicked() {
                sim.pin_semantic_item(boundary.id, item.id);
                if let Some(move_start) = item.move_start {
                    events.push(AppEvent::SimJumpToMove(boundary.start_move + move_start));
                }
            }

            if let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) {
                if ui
                    .small_button(">|")
                    .on_hover_text("Jump to semantic item end")
                    .clicked()
                {
                    events.push(AppEvent::SimJumpToMove(boundary.start_move + move_end));
                }
                ui.label(
                    egui::RichText::new(format!("{move_start}-{move_end}"))
                        .small()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
            }
        });

        if let Some(children) = index.child_indices_by_parent.get(&Some(item.id)) {
            for child_index in children {
                draw_semantic_item_row(
                    ui,
                    trace,
                    index,
                    sim,
                    boundary,
                    *child_index,
                    depth + 1,
                    active_item_id,
                    events,
                );
            }
        }
    }
}

/// Estimate operation time as a formatted string.
fn estimate_op_time(job: &JobState, tp_id: ToolpathId) -> Option<String> {
    let tp = job.find_toolpath(tp_id)?;
    let result = tp.result.as_ref()?;
    let feed = match &tp.operation {
        crate::state::toolpath::OperationConfig::Face(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Pocket(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Profile(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Adaptive(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::DropCutter(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Trace(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Drill(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Chamfer(c) => c.feed_rate,
        crate::state::toolpath::OperationConfig::Zigzag(c) => c.feed_rate,
        _ => 1000.0,
    };
    let est_secs = (result.stats.cutting_distance / feed) * 60.0;
    let est_min = (est_secs / 60.0).floor() as u32;
    let est_sec = (est_secs % 60.0) as u32;
    Some(format!("~{}:{:02}", est_min, est_sec))
}
