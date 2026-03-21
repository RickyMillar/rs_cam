use super::AppEvent;
use crate::render::toolpath_render::palette_color;
use crate::state::job::JobState;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::OperationConfig;

/// Bottom panel in simulation workspace: transport controls, timeline scrubber, speed control.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    events: &mut Vec<AppEvent>,
) {
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
    }
}
