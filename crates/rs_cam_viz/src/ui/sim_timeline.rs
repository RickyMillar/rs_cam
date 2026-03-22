use super::AppEvent;
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::render::toolpath_render::palette_color;
use crate::state::job::JobState;
use crate::state::simulation::{ActiveSemanticItem, SimulationDebugTab, SimulationState};
use crate::state::toolpath::OperationConfig;
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};

/// Bottom panel in simulation workspace: transport controls, timeline scrubber, speed control.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    events: &mut Vec<AppEvent>,
) {
    sim.sync_debug_state(job);
    let active_semantic = sim.active_semantic_item(job);
    let current_boundary = sim.current_boundary().cloned();

    // Row 1: Transport + scrubber + time display
    ui.horizontal(|ui| {
        // Transport buttons
        if ui
            .button("|◄")
            .on_hover_text("Jump to start (Home)")
            .clicked()
        {
            events.push(AppEvent::SimJumpToStart);
        }
        if ui.button("◄").on_hover_text("Step back (Left)").clicked() {
            events.push(AppEvent::SimStepBackward);
        }
        let play_label = if sim.playback.playing {
            "❚❚"
        } else {
            "▶"
        };
        let play_tip = if sim.playback.playing {
            "Pause (Space)"
        } else {
            "Play (Space)"
        };
        if ui.button(play_label).on_hover_text(play_tip).clicked() {
            events.push(AppEvent::ToggleSimPlayback);
        }
        if ui
            .button("►")
            .on_hover_text("Step forward (Right)")
            .clicked()
        {
            events.push(AppEvent::SimStepForward);
        }
        if ui.button("►|").on_hover_text("Jump to end (End)").clicked() {
            events.push(AppEvent::SimJumpToEnd);
        }

        ui.separator();

        // Timeline scrubber
        if sim.total_moves() > 0 {
            let mut pos = sim.playback.current_move as f32;
            let slider = egui::Slider::new(&mut pos, 0.0..=sim.total_moves() as f32)
                .show_value(false)
                .step_by(1.0);
            let available = (ui.available_width() - 160.0).max(80.0);
            let slider_response = ui.add_sized(egui::vec2(available, 18.0), slider);
            if slider_response.changed() {
                sim.playback.current_move = pos as usize;
                sim.playback.playing = false;
            }

            ui.separator();

            // Time display: MM:SS / MM:SS
            let (elapsed_time, total_time) = estimate_times(sim, job);
            let elapsed_str = format_time(elapsed_time);
            let total_str = format_time(total_time);
            ui.label(
                egui::RichText::new(format!("{} / {}", elapsed_str, total_str))
                    .monospace()
                    .color(egui::Color32::from_rgb(160, 200, 240)),
            );
        }
    });

    if sim.setup_boundaries().len() > 1 {
        // Snapshot to avoid borrow conflict with playback mutation.
        let setups: Vec<_> = sim
            .setup_boundaries()
            .iter()
            .map(|s| (s.setup_name.clone(), s.start_move))
            .collect();
        let total = sim.total_moves();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Setups:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            for (name, start_move) in &setups {
                let is_current = sim.playback.current_move >= *start_move;
                let color = if is_current {
                    egui::Color32::from_rgb(160, 170, 200)
                } else {
                    egui::Color32::from_rgb(110, 110, 120)
                };
                let pct = if total > 0 {
                    *start_move as f32 / total as f32 * 100.0
                } else {
                    0.0
                };
                let button = egui::Button::new(
                    egui::RichText::new(format!("{} {:.0}%", name, pct))
                        .small()
                        .color(color),
                )
                .selected(is_current);
                if ui.add(button).clicked() {
                    sim.playback.current_move = *start_move;
                    sim.playback.playing = false;
                }
            }
        });
    }

    // Row 2: Custom-painted per-op timeline with collision markers
    if sim.total_moves() > 0 && !sim.boundaries().is_empty() {
        let total_width = ui.available_width();
        let height = 12.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::click());

        let painter = ui.painter_at(rect);
        let total_moves = sim.total_moves().max(1) as f32;

        // Draw per-op colored segments
        for (i, boundary) in sim.boundaries().iter().enumerate() {
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let x_start = rect.min.x + (boundary.start_move as f32 / total_moves) * total_width;
            let x_end = rect.min.x + (boundary.end_move as f32 / total_moves) * total_width;

            let pc = palette_color(i);
            let dim_color = egui::Color32::from_rgb(
                (pc[0] * 50.0) as u8,
                (pc[1] * 50.0) as u8,
                (pc[2] * 50.0) as u8,
            );
            let color = egui::Color32::from_rgb(
                (pc[0] * 255.0) as u8,
                (pc[1] * 255.0) as u8,
                (pc[2] * 255.0) as u8,
            );

            // Background (dim)
            let seg_rect = egui::Rect::from_min_max(
                egui::pos2(x_start, rect.min.y),
                egui::pos2(x_end, rect.max.y),
            );
            painter.rect_filled(seg_rect, 1.0, dim_color);

            // Filled progress
            let progress = if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f32 / op_moves.max(1) as f32
            };
            let fill_width = (x_end - x_start) * progress;
            let fill_rect = egui::Rect::from_min_size(
                egui::pos2(x_start, rect.min.y),
                egui::vec2(fill_width, height),
            );
            painter.rect_filled(fill_rect, 1.0, color);
        }

        // Red tick marks at holder collision move indices
        if let Some(ref report) = sim.checks.collision_report {
            let holder_color = egui::Color32::from_rgb(255, 50, 50);
            for col in &report.collisions {
                let x = rect.min.x + (col.move_idx as f32 / total_moves) * total_width;
                painter.line_segment(
                    [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                    egui::Stroke::new(2.0, holder_color),
                );
            }
        }

        // Orange tick marks at rapid collision move indices
        let rapid_color = egui::Color32::from_rgb(255, 160, 40);
        for &idx in &sim.checks.rapid_collision_move_indices {
            let x = rect.min.x + (idx as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.5, rapid_color),
            );
        }

        // Position indicator (white vertical line)
        let pos_x = rect.min.x + (sim.playback.current_move as f32 / total_moves) * total_width;
        painter.line_segment(
            [
                egui::pos2(pos_x, rect.min.y - 1.0),
                egui::pos2(pos_x, rect.max.y + 1.0),
            ],
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );

        // Click-to-jump
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let frac = ((pos.x - rect.min.x) / total_width).clamp(0.0, 1.0);
            sim.playback.current_move = (frac * total_moves) as usize;
            sim.playback.playing = false;
        }
    }

    if sim.debug.enabled
        && let Some(boundary) = current_boundary.as_ref()
    {
        draw_semantic_band(ui, sim, job, boundary, active_semantic.as_ref(), events);
    }

    // Row 3: Speed control
    ui.horizontal(|ui| {
        ui.label("Speed:");

        // Speed preset buttons
        for &(label, speed) in &[
            ("100", 100.0),
            ("500", 500.0),
            ("1k", 1000.0),
            ("5k", 5000.0),
            ("10k", 10000.0),
            ("Max", 50000.0),
        ] {
            let is_selected = (sim.playback.speed - speed).abs() < 1.0;
            let btn = egui::Button::new(egui::RichText::new(label).small()).selected(is_selected);
            if ui.add(btn).clicked() {
                sim.playback.speed = speed;
            }
        }

        ui.separator();
        ui.add(
            egui::DragValue::new(&mut sim.playback.speed)
                .range(10.0..=50000.0)
                .speed(50.0)
                .suffix(" mv/s"),
        );

        ui.separator();
        ui.label(
            egui::RichText::new("[ ] speed  ← → step  Home/End jump  Space play")
                .small()
                .color(egui::Color32::from_rgb(90, 90, 100)),
        );
    });

    if sim.debug.enabled {
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            let toggle = if sim.debug.drawer_open {
                "Hide Debug"
            } else {
                "Show Debug"
            };
            if ui.button(toggle).clicked() {
                sim.debug.drawer_open = !sim.debug.drawer_open;
            }

            if sim.debug.drawer_open {
                ui.selectable_value(
                    &mut sim.debug.active_tab,
                    SimulationDebugTab::Semantic,
                    "Semantic",
                );
                ui.selectable_value(
                    &mut sim.debug.active_tab,
                    SimulationDebugTab::Generation,
                    "Generation",
                );
                ui.selectable_value(
                    &mut sim.debug.active_tab,
                    SimulationDebugTab::Cutting,
                    "Cutting",
                );
                ui.selectable_value(
                    &mut sim.debug.active_tab,
                    SimulationDebugTab::Trace,
                    "Trace",
                );
            }
        });

        if sim.debug.drawer_open {
            ui.add_space(4.0);
            match sim.debug.active_tab {
                SimulationDebugTab::Semantic => {
                    draw_semantic_drawer(
                        ui,
                        sim,
                        job,
                        current_boundary.as_ref(),
                        active_semantic.as_ref(),
                        events,
                    );
                }
                SimulationDebugTab::Generation => {
                    draw_generation_drawer(
                        ui,
                        sim,
                        job,
                        current_boundary.as_ref(),
                        active_semantic.as_ref(),
                        events,
                    );
                }
                SimulationDebugTab::Cutting => {
                    draw_cutting_drawer(
                        ui,
                        sim,
                        job,
                        current_boundary.as_ref(),
                        active_semantic.as_ref(),
                        events,
                    );
                }
                SimulationDebugTab::Trace => {
                    draw_trace_drawer(ui, sim, job, current_boundary.as_ref());
                }
            }
        }
    }
}

/// Estimate elapsed and total time (in seconds) based on feed rates.
fn estimate_times(sim: &SimulationState, job: &JobState) -> (f64, f64) {
    let mut total_secs = 0.0;
    let mut elapsed_secs = 0.0;

    for boundary in sim.boundaries() {
        if let Some(tp) = job.find_toolpath(boundary.id)
            && let Some(result) = &tp.result
        {
            let feed = op_feed_rate(&tp.operation);
            let op_time = (result.stats.cutting_distance / feed) * 60.0;
            total_secs += op_time;

            // Estimate elapsed time for this op
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let progress = if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f64 / op_moves.max(1) as f64
            };
            elapsed_secs += op_time * progress;
        }
    }

    (elapsed_secs, total_secs)
}

fn format_time(secs: f64) -> String {
    let m = (secs / 60.0).floor() as u32;
    let s = (secs % 60.0) as u32;
    format!("{}:{:02}", m, s)
}

fn op_feed_rate(op: &OperationConfig) -> f64 {
    match op {
        OperationConfig::Face(c) => c.feed_rate,
        OperationConfig::Pocket(c) => c.feed_rate,
        OperationConfig::Profile(c) => c.feed_rate,
        OperationConfig::Adaptive(c) => c.feed_rate,
        OperationConfig::DropCutter(c) => c.feed_rate,
        OperationConfig::Trace(c) => c.feed_rate,
        OperationConfig::Drill(c) => c.feed_rate,
        OperationConfig::Chamfer(c) => c.feed_rate,
        OperationConfig::Zigzag(c) => c.feed_rate,
        OperationConfig::VCarve(c) => c.feed_rate,
        OperationConfig::Rest(c) => c.feed_rate,
        OperationConfig::Inlay(c) => c.feed_rate,
        OperationConfig::Adaptive3d(c) => c.feed_rate,
        OperationConfig::Waterline(c) => c.feed_rate,
        OperationConfig::Pencil(c) => c.feed_rate,
        OperationConfig::Scallop(c) => c.feed_rate,
        OperationConfig::SteepShallow(c) => c.feed_rate,
        OperationConfig::RampFinish(c) => c.feed_rate,
        OperationConfig::SpiralFinish(c) => c.feed_rate,
        OperationConfig::RadialFinish(c) => c.feed_rate,
        OperationConfig::HorizontalFinish(c) => c.feed_rate,
        OperationConfig::ProjectCurve(c) => c.feed_rate,
        OperationConfig::AlignmentPinDrill(c) => c.feed_rate,
    }
}

fn draw_semantic_band(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    boundary: &crate::state::simulation::ToolpathBoundary,
    active_semantic: Option<&ActiveSemanticItem>,
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

    let local_total = boundary.end_move.saturating_sub(boundary.start_move).max(1);
    let total_width = ui.available_width();
    let height = 10.0;
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Semantic timeline")
            .small()
            .color(egui::Color32::from_rgb(140, 140, 155)),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(30, 30, 40));

    let mut segments = index.move_item_indices.clone();
    segments.sort_by_key(|item_index| index.depths[*item_index]);
    for item_index in segments {
        let item = &trace.items[item_index];
        let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
            continue;
        };
        let x_start = rect.min.x + (move_start as f32 / local_total as f32) * total_width;
        let x_end = rect.min.x + ((move_end + 1) as f32 / local_total as f32) * total_width;
        let seg_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, rect.min.y),
            egui::pos2(x_end.max(x_start + 1.0), rect.max.y),
        );
        let color = semantic_kind_color(&item.kind);
        painter.rect_filled(seg_rect, 1.0, color.linear_multiply(0.85));
        if active_semantic
            .is_some_and(|active| active.toolpath_id == boundary.id && active.item.id == item.id)
        {
            painter.rect_stroke(seg_rect, 1.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
        }
    }

    if let Some(debug_trace) = toolpath.debug_trace.as_ref() {
        for annotation in &debug_trace.annotations {
            let x = rect.min.x + (annotation.move_index as f32 / local_total as f32) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 210, 120)),
            );
        }
    }

    if let Some(cut_trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_ref())
    {
        for issue in cut_trace
            .issues
            .iter()
            .filter(|issue| issue.toolpath_id == boundary.id.0)
        {
            let x = rect.min.x + (issue.move_index as f32 / local_total as f32) * total_width;
            let color = match issue.kind {
                rs_cam_core::simulation_cut::SimulationCutIssueKind::AirCut => {
                    egui::Color32::from_rgb(255, 120, 80)
                }
                rs_cam_core::simulation_cut::SimulationCutIssueKind::LowEngagement => {
                    egui::Color32::from_rgb(250, 200, 100)
                }
            };
            painter.circle_filled(egui::pos2(x, rect.center().y), 2.5, color);
        }
    }

    let local_move = sim
        .playback
        .current_move
        .saturating_sub(boundary.start_move)
        .min(local_total);
    let pos_x = rect.min.x + (local_move as f32 / local_total as f32) * total_width;
    painter.line_segment(
        [
            egui::pos2(pos_x, rect.min.y - 1.0),
            egui::pos2(pos_x, rect.max.y + 1.0),
        ],
        egui::Stroke::new(1.5, egui::Color32::WHITE),
    );

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        if let Some(debug_trace) = toolpath.debug_trace.as_ref() {
            let nearest_annotation = debug_trace
                .annotations
                .iter()
                .enumerate()
                .map(|(index, annotation)| {
                    let x = rect.min.x
                        + (annotation.move_index as f32 / local_total as f32) * total_width;
                    (index, annotation, (pointer.x - x).abs())
                })
                .filter(|(_, _, distance)| *distance <= 5.0)
                .min_by(|left, right| left.2.total_cmp(&right.2));
            if let Some((annotation_index, annotation, _)) = nearest_annotation
                && let Some(target) = sim.trace_target_for_annotation(boundary.id, annotation)
            {
                sim.debug.focused_issue_index = None;
                sim.debug.focused_hotspot = None;
                sim.clear_pinned_semantic_item();
                let _ = annotation_index;
                events.push(AppEvent::SimJumpToMove(target.move_index));
                return;
            }
        }

        if let Some(cut_trace) = sim
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        {
            let nearest_issue = cut_trace
                .issues
                .iter()
                .filter(|issue| issue.toolpath_id == boundary.id.0)
                .map(|issue| {
                    let x =
                        rect.min.x + (issue.move_index as f32 / local_total as f32) * total_width;
                    (issue.clone(), (pointer.x - x).abs())
                })
                .filter(|(_, distance)| *distance <= 6.0)
                .min_by(|left, right| left.1.total_cmp(&right.1));
            if let Some((issue, _)) = nearest_issue
                && let Some(target) = sim.trace_target_for_cut_issue(job, &issue)
            {
                if let Some(item_id) = target.semantic_item_id {
                    sim.pin_semantic_item(boundary.id, item_id);
                }
                sim.debug.focused_issue_index = None;
                sim.debug.focused_hotspot = None;
                events.push(AppEvent::SimJumpToMove(target.move_index));
                return;
            }
        }

        let semantic_hit = index
            .move_item_indices
            .iter()
            .copied()
            .filter_map(|item_index| {
                let item = trace.items.get(item_index)?;
                let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
                    return None;
                };
                let x_start = rect.min.x + (move_start as f32 / local_total as f32) * total_width;
                let x_end = rect.min.x + ((move_end + 1) as f32 / local_total as f32) * total_width;
                (pointer.x >= x_start && pointer.x <= x_end).then_some((
                    item_index,
                    index.depths[item_index],
                    move_end.saturating_sub(move_start),
                ))
            })
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.2.cmp(&left.2)))
            .map(|(item_index, _, _)| item_index);

        if let Some(item_index) = semantic_hit {
            let item = &trace.items[item_index];
            sim.pin_semantic_item(boundary.id, item.id);
            sim.debug.focused_issue_index = None;
            sim.debug.focused_hotspot = None;
            if let Some(target) = sim.trace_target_for_item(job, boundary.id, item.id, false) {
                events.push(AppEvent::SimJumpToMove(target.move_index));
            }
            return;
        }

        let frac = ((pointer.x - rect.min.x) / total_width).clamp(0.0, 1.0);
        let local_move = (frac * local_total as f32) as usize;
        sim.clear_pinned_semantic_item();
        sim.debug.focused_issue_index = None;
        sim.debug.focused_hotspot = None;
        events.push(AppEvent::SimJumpToMove(boundary.start_move + local_move));
    }
}

fn draw_semantic_drawer(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    current_boundary: Option<&crate::state::simulation::ToolpathBoundary>,
    active_semantic: Option<&ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    let Some(boundary) = current_boundary else {
        ui.label("No active toolpath.");
        return;
    };
    let Some(toolpath) = job.find_toolpath(boundary.id) else {
        ui.label("Toolpath missing.");
        return;
    };
    let Some(trace) = toolpath.semantic_trace.as_ref() else {
        ui.label("No semantic trace available.");
        return;
    };
    let Some(index) = sim.debug.semantic_indexes.get(&boundary.id).cloned() else {
        ui.label("Semantic index unavailable.");
        return;
    };

    ui.label(
        egui::RichText::new(format!("{} semantic items", trace.summary.item_count))
            .small()
            .color(egui::Color32::from_rgb(140, 140, 155)),
    );

    let root_items = index
        .child_indices_by_parent
        .get(&None)
        .cloned()
        .unwrap_or_default();
    egui::ScrollArea::vertical()
        .max_height(180.0)
        .show(ui, |ui| {
            for item_index in root_items {
                draw_semantic_drawer_item(
                    ui,
                    trace,
                    &index,
                    sim,
                    boundary,
                    item_index,
                    0,
                    active_semantic,
                    events,
                );
            }
        });

    if let Some(active) = active_semantic
        && active.toolpath_id == boundary.id
    {
        ui.separator();
        ui.label(
            egui::RichText::new(&active.item.label)
                .strong()
                .color(semantic_kind_color(&active.item.kind)),
        );
        ui.label(
            egui::RichText::new(semantic_kind_label(&active.item.kind))
                .small()
                .color(egui::Color32::from_rgb(150, 150, 165)),
        );
        if !active.ancestry.is_empty() {
            ui.label(
                egui::RichText::new(
                    active
                        .ancestry
                        .iter()
                        .map(|item| item.label.as_str())
                        .collect::<Vec<_>>()
                        .join(" / "),
                )
                .small()
                .color(egui::Color32::from_rgb(130, 130, 145)),
            );
        }
        if let (Some(move_start), Some(move_end)) = (active.item.move_start, active.item.move_end) {
            ui.label(format!("Moves: {move_start}..{move_end}"));
        }
        if let Some(bounds) = active.item.xy_bbox {
            ui.label(format!(
                "XY: {:.2},{:.2} → {:.2},{:.2}",
                bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y
            ));
        }
        if let (Some(z_min), Some(z_max)) = (active.item.z_min, active.item.z_max) {
            ui.label(format!("Z: {:.3} → {:.3}", z_min, z_max));
        }
        if !active.item.params.values.is_empty() {
            ui.add_space(4.0);
            egui::Grid::new("semantic_params_grid")
                .num_columns(2)
                .spacing([10.0, 2.0])
                .show(ui, |ui| {
                    for (key, value) in &active.item.params.values {
                        ui.label(
                            egui::RichText::new(key)
                                .small()
                                .color(egui::Color32::from_rgb(140, 140, 155)),
                        );
                        ui.label(egui::RichText::new(format_json_value(value)).small());
                        ui.end_row();
                    }
                });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_semantic_drawer_item(
    ui: &mut egui::Ui,
    trace: &rs_cam_core::semantic_trace::ToolpathSemanticTrace,
    index: &crate::state::simulation::SimulationSemanticIndex,
    sim: &mut SimulationState,
    boundary: &crate::state::simulation::ToolpathBoundary,
    item_index: usize,
    depth: usize,
    active_semantic: Option<&ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    let item = &trace.items[item_index];
    let color = semantic_kind_color(&item.kind);
    let is_active = active_semantic
        .is_some_and(|active| active.toolpath_id == boundary.id && active.item.id == item.id);
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 12.0);
        ui.label(
            egui::RichText::new(semantic_kind_label(&item.kind))
                .small()
                .color(color),
        );
        let response = ui.selectable_label(
            is_active,
            if is_active {
                egui::RichText::new(&item.label).small().strong()
            } else {
                egui::RichText::new(&item.label).small()
            },
        );
        if response.clicked() {
            sim.pin_semantic_item(boundary.id, item.id);
            sim.debug.focused_issue_index = None;
            sim.debug.focused_hotspot = None;
            if let Some(move_start) = item.move_start {
                events.push(AppEvent::SimJumpToMove(boundary.start_move + move_start));
            }
        }
    });
    if let Some(children) = index.child_indices_by_parent.get(&Some(item.id)) {
        for child_index in children {
            draw_semantic_drawer_item(
                ui,
                trace,
                index,
                sim,
                boundary,
                *child_index,
                depth + 1,
                active_semantic,
                events,
            );
        }
    }
}

fn draw_generation_drawer(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    current_boundary: Option<&crate::state::simulation::ToolpathBoundary>,
    active_semantic: Option<&ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    let Some(boundary) = current_boundary else {
        ui.label("No active toolpath.");
        return;
    };
    let Some(toolpath) = job.find_toolpath(boundary.id) else {
        ui.label("Toolpath missing.");
        return;
    };
    let Some(trace) = toolpath.debug_trace.as_ref() else {
        ui.label("No performance trace available.");
        return;
    };

    let summary = &trace.summary;
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Total: {:.1} ms",
            summary.total_elapsed_us as f64 / 1000.0
        ));
        ui.separator();
        ui.label(format!("Spans: {}", summary.span_count));
        ui.separator();
        ui.label(format!("Hotspots: {}", summary.hotspot_count));
    });

    if let Some(annotation) = sim.current_debug_annotation(job)
        && annotation.0 == boundary.id
    {
        ui.label(
            egui::RichText::new(format!("Annotation: {}", annotation.1.label))
                .small()
                .color(egui::Color32::from_rgb(255, 210, 120)),
        );
    }

    if let Some((_, span)) = sim.active_debug_span(job)
        && boundary.id == current_boundary.map(|b| b.id).unwrap_or(boundary.id)
    {
        ui.label(
            egui::RichText::new(format!(
                "Linked span: {} ({:.1} ms)",
                span.label,
                span.elapsed_us as f64 / 1000.0
            ))
            .small()
            .color(egui::Color32::from_rgb(140, 190, 230)),
        );
        if let Some(summary) = debug_span_math_summary(&span.kind) {
            ui.label(
                egui::RichText::new(summary)
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 145)),
            );
        }
    }

    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(160.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Dominant spans").small().strong());
            let mut spans: Vec<_> = trace.spans.iter().collect();
            spans.sort_by_key(|span| std::cmp::Reverse(span.elapsed_us));
            for span in spans.into_iter().take(8) {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&span.kind)
                            .small()
                            .color(egui::Color32::from_rgb(130, 170, 220)),
                    );
                    let button = egui::Button::new(egui::RichText::new(&span.label).small())
                        .selected(active_semantic.is_some_and(|active| {
                            active.toolpath_id == boundary.id
                                && active
                                    .ancestry
                                    .iter()
                                    .rev()
                                    .find_map(|item| item.debug_span_id)
                                    == Some(span.id)
                        }));
                    let response = ui.add(button);
                    if let Some(summary) = debug_span_math_summary(&span.kind) {
                        response.clone().on_hover_text(summary);
                    }
                    if response.clicked()
                        && let Some(target) =
                            sim.trace_target_for_span(job, boundary.id, span.id, false)
                    {
                        if let Some(item_id) = target.semantic_item_id {
                            sim.pin_semantic_item(boundary.id, item_id);
                        }
                        sim.debug.focused_issue_index = None;
                        sim.debug.focused_hotspot = None;
                        events.push(AppEvent::SimJumpToMove(target.move_index));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.1} ms",
                                span.elapsed_us as f64 / 1000.0
                            ))
                            .small()
                            .color(egui::Color32::from_rgb(150, 150, 165)),
                        );
                    });
                });
            }
        });

    if !trace.hotspots.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Hotspots").small().strong());
        for (index, hotspot) in trace.hotspots.iter().take(5).enumerate() {
            let is_focused = sim.debug.focused_hotspot == Some((boundary.id, index));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("#{}", index + 1))
                        .small()
                        .strong()
                        .color(if is_focused {
                            egui::Color32::from_rgb(255, 210, 120)
                        } else {
                            egui::Color32::from_rgb(180, 180, 195)
                        }),
                );
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{} {:.1} ms",
                            hotspot.kind,
                            hotspot.total_elapsed_us as f64 / 1000.0
                        ))
                        .small(),
                    )
                    .selected(is_focused),
                );
                if response.clicked()
                    && let Some(target) = sim.trace_target_for_hotspot(job, boundary.id, index)
                {
                    sim.debug.focused_hotspot = Some((boundary.id, index));
                    if let Some(item_id) = target.semantic_item_id {
                        sim.pin_semantic_item(boundary.id, item_id);
                    }
                    sim.debug.focused_issue_index = None;
                    events.push(AppEvent::SimJumpToMove(target.move_index));
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "@ {:.2}, {:.2}",
                            hotspot.center_x, hotspot.center_y
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(130, 130, 145)),
                    );
                    if let Some(span_id) = hotspot.representative_span_id
                        && let Some(span) = trace.spans.iter().find(|span| span.id == span_id)
                    {
                        ui.label(
                            egui::RichText::new(&span.label)
                                .small()
                                .color(egui::Color32::from_rgb(140, 190, 230)),
                        );
                        if let Some(summary) = debug_span_math_summary(&span.kind) {
                            ui.label(
                                egui::RichText::new(summary)
                                    .small()
                                    .color(egui::Color32::from_rgb(120, 120, 135)),
                            );
                        }
                    }
                });
            });
        }
    }
}

fn draw_cutting_drawer(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    current_boundary: Option<&crate::state::simulation::ToolpathBoundary>,
    active_semantic: Option<&ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    let Some(boundary) = current_boundary else {
        ui.label("No active toolpath.");
        return;
    };
    let Some(trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.clone())
    else {
        ui.label("Enable Capture Metrics and re-run simulation to collect cutting metrics.");
        return;
    };

    let samples: Vec<_> = trace
        .samples
        .iter()
        .filter(|sample| sample.toolpath_id == boundary.id.0)
        .cloned()
        .collect();
    if samples.is_empty() {
        ui.label("No cutting samples for the active toolpath.");
        return;
    }

    if let Some(summary) = sim.toolpath_cut_summary(boundary.id) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Samples: {}", summary.sample_count));
            ui.separator();
            ui.label(format!("Runtime: {:.2}s", summary.total_runtime_s));
            ui.separator();
            ui.label(format!("Cut: {:.2}s", summary.cutting_runtime_s));
            ui.separator();
            ui.label(format!("Rapid: {:.2}s", summary.rapid_runtime_s));
            ui.separator();
            ui.label(format!("Air: {:.2}s", summary.air_cut_time_s));
            ui.separator();
            ui.label(format!(
                "Low engagement: {:.2}s",
                summary.low_engagement_time_s
            ));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Avg engagement: {:.1}%",
                summary.average_engagement * 100.0
            ));
            ui.separator();
            ui.label(format!(
                "Peak chipload: {:.4} mm/tooth",
                summary.peak_chipload_mm_per_tooth
            ));
            ui.separator();
            ui.label(format!(
                "Peak axial DOC: {:.3} mm",
                summary.peak_axial_doc_mm
            ));
            ui.separator();
            ui.label(format!(
                "Removed volume: {:.1} mm^3",
                summary.total_removed_volume_est_mm3
            ));
            ui.separator();
            ui.label(format!("Avg MRR: {:.1} mm^3/s", summary.average_mrr_mm3_s));
        });
    }

    if let Some(active_sample) = sim.current_cut_sample()
        && active_sample.toolpath_id == boundary.id
    {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Active sample").small().strong());
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Move {} sample {}",
                active_sample.sample.move_index, active_sample.sample.sample_index
            ));
            ui.separator();
            ui.label(format!(
                "t = {:.2}s",
                active_sample.sample.cumulative_time_s
            ));
            ui.separator();
            ui.label(if active_sample.sample.is_cutting {
                "Cutting"
            } else {
                "Rapid"
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Engagement {:.1}%",
                active_sample.sample.radial_engagement * 100.0
            ));
            ui.separator();
            ui.label(format!(
                "Axial DOC {:.3} mm",
                active_sample.sample.axial_doc_mm
            ));
            ui.separator();
            ui.label(format!(
                "Chipload {:.4} mm/tooth",
                active_sample.sample.chipload_mm_per_tooth
            ));
            ui.separator();
            ui.label(format!("MRR {:.1} mm^3/s", active_sample.sample.mrr_mm3_s));
        });
    }

    if let Some(active_semantic) = active_semantic
        && active_semantic.toolpath_id == boundary.id
        && let Some(summary) = sim.semantic_cut_summary(boundary.id, active_semantic.item.id)
    {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Active semantic item").small().strong());
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} ({})",
                active_semantic.item.label,
                semantic_kind_label(&active_semantic.item.kind)
            ));
            ui.separator();
            ui.label(format!("moves {}-{}", summary.move_start, summary.move_end));
            ui.separator();
            ui.label(format!("wasted {:.2}s", summary.wasted_runtime_s));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Avg engage {:.1}% | peak {:.1}%",
                summary.average_engagement * 100.0,
                summary.peak_engagement * 100.0
            ));
            ui.separator();
            ui.label(format!(
                "Avg MRR {:.1} | peak MRR {:.1}",
                summary.average_mrr_mm3_s, summary.peak_mrr_mm3_s
            ));
            ui.separator();
            ui.label(format!(
                "Air {:.2}s / {} | low {:.2}s / {}",
                summary.air_cut_time_s,
                summary.air_cut_issue_count,
                summary.low_engagement_time_s,
                summary.low_engagement_issue_count
            ));
        });
    }

    let worst_items = sim.cut_worst_items(boundary.id, 6);
    if !worst_items.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Worst items").small().strong());
        for item in worst_items {
            let is_active = active_semantic.is_some_and(|active| {
                active.toolpath_id == boundary.id && active.item.id == item.semantic_item_id
            });
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{} | wasted {:.2}s | MRR {:.1}",
                            item.label, item.wasted_runtime_s, item.average_mrr_mm3_s
                        ))
                        .small(),
                    )
                    .selected(is_active),
                );
                if response.clicked()
                    && let Some(target) =
                        sim.trace_target_for_item(job, boundary.id, item.semantic_item_id, false)
                {
                    sim.pin_semantic_item(boundary.id, item.semantic_item_id);
                    sim.debug.focused_issue_index = None;
                    sim.debug.focused_hotspot = None;
                    events.push(AppEvent::SimJumpToMove(target.move_index));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{:.1}% engage | air {:.2}s | low {:.2}s",
                        item.average_engagement * 100.0,
                        item.air_cut_time_s,
                        item.low_engagement_time_s
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 145)),
                );
            });
        }
    }

    let toolpath_issues: Vec<_> = trace
        .issues
        .iter()
        .filter(|issue| issue.toolpath_id == boundary.id.0)
        .cloned()
        .collect();
    if !toolpath_issues.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Cutting issues").small().strong());
        for issue in toolpath_issues.iter().take(6) {
            let response = ui.add(
                egui::Button::new(
                    egui::RichText::new(format!(
                        "{} @ {:.2}s ({:.1}%)",
                        issue.label,
                        issue.cumulative_time_s,
                        issue.radial_engagement * 100.0
                    ))
                    .small(),
                )
                .selected(sim.current_issue(job).as_ref().is_some_and(|current| {
                    current.toolpath_id == Some(boundary.id)
                        && current.move_index == boundary.start_move + issue.move_index
                        && current.kind == match issue.kind {
                            rs_cam_core::simulation_cut::SimulationCutIssueKind::AirCut => {
                                crate::state::simulation::SimulationIssueKind::AirCut
                            }
                            rs_cam_core::simulation_cut::SimulationCutIssueKind::LowEngagement => {
                                crate::state::simulation::SimulationIssueKind::LowEngagement
                            }
                        }
                })),
            );
            if response.clicked()
                && let Some(target) = sim.trace_target_for_cut_issue(job, issue)
            {
                if let Some(item_id) = target.semantic_item_id {
                    sim.pin_semantic_item(boundary.id, item_id);
                }
                sim.debug.focused_hotspot = None;
                sim.debug.focused_issue_index = None;
                events.push(AppEvent::SimJumpToMove(target.move_index));
            }
        }
    }

    let cut_hotspots = sim.cut_hotspots(boundary.id, 5);
    if !cut_hotspots.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Runtime hotspots").small().strong());
        for hotspot in cut_hotspots {
            let is_active =
                hotspot
                    .semantic_item_id
                    .zip(active_semantic)
                    .is_some_and(|(item_id, active)| {
                        active.toolpath_id == boundary.id && active.item.id == item_id
                    });
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{} | wasted {:.2}s | total {:.2}s",
                            cut_hotspot_label(job, boundary.id, &hotspot),
                            hotspot.wasted_runtime_s,
                            hotspot.total_runtime_s
                        ))
                        .small(),
                    )
                    .selected(is_active),
                );
                if response.clicked() {
                    if let Some(item_id) = hotspot.semantic_item_id {
                        sim.pin_semantic_item(boundary.id, item_id);
                    }
                    sim.debug.focused_issue_index = None;
                    sim.debug.focused_hotspot = None;
                    events.push(AppEvent::SimJumpToMove(
                        boundary.start_move + hotspot.move_start,
                    ));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "air {:.2}s | low {:.2}s | MRR {:.1}",
                        hotspot.air_cut_time_s,
                        hotspot.low_engagement_time_s,
                        hotspot.average_mrr_mm3_s
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 145)),
                );
            });
        }
    }

    ui.separator();
    ui.label(egui::RichText::new("Time series").small().strong());
    let engagement_points =
        decimate_plot_points(&samples, 300, |sample| sample.radial_engagement * 100.0);
    let chipload_points =
        decimate_plot_points(&samples, 300, |sample| sample.chipload_mm_per_tooth);
    let axial_doc_points = decimate_plot_points(&samples, 300, |sample| sample.axial_doc_mm);
    let mrr_points = decimate_plot_points(&samples, 300, |sample| sample.mrr_mm3_s);
    let bars = runtime_breakdown_bars(sim, boundary.id);

    Plot::new("cutting_metrics_plot")
        .height(210.0)
        .legend(Legend::default())
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(engagement_points).name("Engagement %"));
            plot_ui.line(Line::new(chipload_points).name("Chipload"));
            plot_ui.line(Line::new(axial_doc_points).name("Axial DOC"));
            plot_ui.line(Line::new(mrr_points).name("MRR"));
            plot_ui.bar_chart(BarChart::new(bars).name("Runtime"));
        });
}

fn draw_trace_drawer(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    job: &JobState,
    current_boundary: Option<&crate::state::simulation::ToolpathBoundary>,
) {
    let Some(boundary) = current_boundary else {
        ui.label("No active toolpath.");
        return;
    };
    let Some(toolpath) = job.find_toolpath(boundary.id) else {
        ui.label("Toolpath missing.");
        return;
    };

    let availability = SimulationState::trace_availability_for_toolpath(job, boundary.id);
    ui.label(format!("Trace status: {:?}", availability));
    ui.label(format!("Toolpath: {}", toolpath.name));
    if let Some(path) = &toolpath.debug_trace_path {
        ui.label(
            egui::RichText::new(path.display().to_string())
                .small()
                .monospace()
                .color(egui::Color32::from_rgb(140, 170, 230)),
        );
    } else {
        ui.label(
            egui::RichText::new("No trace artifact path")
                .small()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    }

    if let Some(semantic_trace) = &toolpath.semantic_trace {
        ui.label(format!(
            "Semantic items: {} (move-linked {})",
            semantic_trace.summary.item_count, semantic_trace.summary.move_linked_item_count
        ));
    }
    if let Some(debug_trace) = &toolpath.debug_trace {
        ui.label(format!(
            "Perf spans: {} | annotations: {}",
            debug_trace.spans.len(),
            debug_trace.annotations.len()
        ));
    }
    if let Some(results) = sim.results.as_ref() {
        if let Some(path) = &results.cut_trace_path {
            ui.label("Cut trace artifact:");
            ui.label(
                egui::RichText::new(path.display().to_string())
                    .small()
                    .monospace()
                    .color(egui::Color32::from_rgb(120, 210, 150)),
            );
        }
        if let Some(cut_trace) = results.cut_trace.as_ref() {
            ui.label(format!(
                "Cut samples: {} | cut issues: {} | cut hotspots: {}",
                cut_trace.samples.len(),
                cut_trace.issues.len(),
                cut_trace.hotspots.len()
            ));
        }
    }

    if let Some(target) = sim.debug.pending_inspect_toolpath {
        ui.label(
            egui::RichText::new(format!("Pending inspect target: {}", target.0))
                .small()
                .color(egui::Color32::from_rgb(220, 180, 90)),
        );
    }
}

fn cut_hotspot_label(
    job: &JobState,
    toolpath_id: crate::state::toolpath::ToolpathId,
    hotspot: &rs_cam_core::simulation_cut::SimulationCutHotspot,
) -> String {
    job.find_toolpath(toolpath_id)
        .and_then(|toolpath| toolpath.semantic_trace.as_ref())
        .and_then(|trace| {
            hotspot.semantic_item_id.and_then(|item_id| {
                trace
                    .items
                    .iter()
                    .find(|item| item.id == item_id)
                    .map(|item| item.label.clone())
            })
        })
        .unwrap_or_else(|| format!("Moves {}..{}", hotspot.move_start, hotspot.move_end))
}

fn decimate_plot_points(
    samples: &[rs_cam_core::simulation_cut::SimulationCutSample],
    max_points: usize,
    value_fn: impl Fn(&rs_cam_core::simulation_cut::SimulationCutSample) -> f64,
) -> PlotPoints {
    if samples.is_empty() {
        return PlotPoints::from(Vec::<[f64; 2]>::new());
    }
    let stride = ((samples.len() as f64 / max_points.max(1) as f64).ceil() as usize).max(1);
    let mut points = Vec::with_capacity(samples.len().div_ceil(stride));
    for sample in samples.iter().step_by(stride) {
        points.push([sample.cumulative_time_s, value_fn(sample)]);
    }
    if let Some(last) = samples.last() {
        let last_point = [last.cumulative_time_s, value_fn(last)];
        if points.last().copied() != Some(last_point) {
            points.push(last_point);
        }
    }
    PlotPoints::from(points)
}

fn runtime_breakdown_bars(
    sim: &SimulationState,
    toolpath_id: crate::state::toolpath::ToolpathId,
) -> Vec<Bar> {
    let Some(summary) = sim.toolpath_cut_summary(toolpath_id) else {
        return Vec::new();
    };
    vec![
        Bar::new(0.0, summary.cutting_runtime_s).name("Cutting"),
        Bar::new(1.0, summary.rapid_runtime_s).name("Rapid"),
        Bar::new(2.0, summary.air_cut_time_s).name("Air"),
        Bar::new(3.0, summary.low_engagement_time_s).name("Low engage"),
    ]
}
