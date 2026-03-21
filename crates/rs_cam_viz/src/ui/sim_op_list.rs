use super::AppEvent;
use crate::render::toolpath_render::palette_color;
use crate::state::job::JobState;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::ToolpathId;

/// Left panel in simulation workspace: operation list with checkboxes, progress bars, and jump buttons.
pub fn draw(ui: &mut egui::Ui, sim: &SimulationState, job: &JobState, events: &mut Vec<AppEvent>) {
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

    // Track if user toggled any checkbox
    let mut toggled_id: Option<ToolpathId> = None;

    for (i, boundary) in sim.boundaries().iter().enumerate() {
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
        });

        if i + 1 < sim.boundaries().len() {
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
        if new_selection.len() == sim.boundaries().len() {
            new_selection.clear();
        }

        if new_selection.is_empty() {
            events.push(AppEvent::RunSimulation);
        } else {
            events.push(AppEvent::RunSimulationWith(new_selection));
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
