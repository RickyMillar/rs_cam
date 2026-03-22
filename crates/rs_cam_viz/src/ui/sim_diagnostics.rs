use super::AppEvent;
use super::sim_debug::{format_json_value, semantic_kind_color, semantic_kind_label};
use crate::state::job::JobState;
use crate::state::simulation::{SimulationState, StockVizMode};
use crate::state::toolpath::OperationConfig;

/// Right panel in simulation workspace: current state, warnings, and summary stats.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    job: &JobState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Diagnostics");
    ui.separator();

    let active_semantic = sim.active_semantic_item(job);
    let current_boundary_id = sim.current_boundary().map(|boundary| boundary.id);

    // --- Stock Display ---
    egui::CollapsingHeader::new("Stock Display")
        .default_open(false)
        .show(ui, |ui| {
            let prev_mode = sim.stock_viz_mode;
            ui.horizontal(|ui| {
                ui.label("Color mode:");
                egui::ComboBox::from_id_salt("stock_viz_mode")
                    .selected_text(match sim.stock_viz_mode {
                        StockVizMode::Solid => "Solid",
                        StockVizMode::Deviation => "Deviation",
                        StockVizMode::ByOperation => "Solid", // placeholder: treated as Solid
                        StockVizMode::ByHeight => "By Height",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sim.stock_viz_mode, StockVizMode::Solid, "Solid");
                        ui.selectable_value(
                            &mut sim.stock_viz_mode,
                            StockVizMode::Deviation,
                            "Deviation",
                        );
                        ui.selectable_value(
                            &mut sim.stock_viz_mode,
                            StockVizMode::ByHeight,
                            "By Height",
                        );
                    });
            });
            if sim.stock_viz_mode != prev_mode {
                events.push(AppEvent::SimVizModeChanged);
            }

            ui.horizontal(|ui| {
                ui.label("Opacity:");
                ui.add(egui::Slider::new(&mut sim.stock_opacity, 0.0..=1.0).show_value(true));
            });

            ui.horizontal(|ui| {
                ui.label("Resolution:");
                if sim.auto_resolution {
                    ui.label(format!("{:.3} mm (auto)", sim.resolution));
                } else {
                    ui.add(
                        egui::Slider::new(&mut sim.resolution, 0.02..=1.0)
                            .suffix(" mm")
                            .logarithmic(true)
                            .show_value(true),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut sim.auto_resolution, "Auto from tool size");
                if !sim.auto_resolution {
                    ui.label(
                        egui::RichText::new("(re-run to apply)")
                            .small()
                            .color(egui::Color32::from_rgb(180, 160, 80)),
                    );
                }
            });
        });

    ui.add_space(4.0);

    // --- Semantic Context ---
    egui::CollapsingHeader::new("Semantic Context")
        .default_open(true)
        .show(ui, |ui| {
            if let Some(active) = active_semantic.as_ref() {
                let color = semantic_kind_color(&active.item.kind);
                ui.label(
                    egui::RichText::new(&active.item.label)
                        .strong()
                        .color(color),
                );
                ui.label(
                    egui::RichText::new(semantic_kind_label(&active.item.kind))
                        .small()
                        .color(egui::Color32::from_rgb(140, 140, 155)),
                );
                if let (Some(move_start), Some(move_end)) =
                    (active.item.move_start, active.item.move_end)
                {
                    ui.label(format!("Moves: {move_start}..{move_end}"));
                }
                if let Some(bounds) = active.item.xy_bbox {
                    ui.label(format!(
                        "XY: {:.2}, {:.2} → {:.2}, {:.2}",
                        bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y
                    ));
                }
                if let (Some(z_min), Some(z_max)) = (active.item.z_min, active.item.z_max) {
                    ui.label(format!("Z: {:.3} → {:.3}", z_min, z_max));
                }
                if !active.item.params.values.is_empty() {
                    ui.add_space(4.0);
                    egui::Grid::new("sim_semantic_context_grid")
                        .num_columns(2)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            for (idx, (key, value)) in active.item.params.values.iter().enumerate()
                            {
                                if idx >= 6 {
                                    break;
                                }
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
            } else {
                ui.label(
                    egui::RichText::new("No semantic item at the current move")
                        .small()
                        .italics()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
            }
        });

    ui.add_space(4.0);

    // --- Performance Trace ---
    egui::CollapsingHeader::new("Performance Trace")
        .default_open(true)
        .show(ui, |ui| {
            let debug_trace = current_boundary_id
                .and_then(|toolpath_id| job.find_toolpath(toolpath_id))
                .and_then(|toolpath| toolpath.debug_trace.as_ref());

            if let Some(trace) = debug_trace {
                ui.label(format!(
                    "Total: {:.1} ms",
                    trace.summary.total_elapsed_us as f64 / 1000.0
                ));
                if let Some(label) = &trace.summary.dominant_span_label {
                    ui.label(
                        egui::RichText::new(format!(
                            "Dominant: {} ({:.1} ms)",
                            label,
                            trace.summary.dominant_span_elapsed_us.unwrap_or_default() as f64
                                / 1000.0
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(140, 190, 230)),
                    );
                }
                ui.label(format!("Hotspots: {}", trace.hotspots.len()));
                if let Some((_, annotation)) = sim.current_debug_annotation(job) {
                    ui.label(
                        egui::RichText::new(format!("Annotation: {}", annotation.label))
                            .small()
                            .color(egui::Color32::from_rgb(255, 210, 120)),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("No performance trace available")
                        .small()
                        .italics()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
            }
        });

    ui.add_space(4.0);

    // --- Current State ---
    egui::CollapsingHeader::new("Current State")
        .default_open(true)
        .show(ui, |ui| {
            if let Some(pos) = sim.playback.tool_position {
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.label(
                        egui::RichText::new(format!(
                            "X{:.2} Y{:.2} Z{:.2}",
                            pos[0], pos[1], pos[2]
                        ))
                        .color(egui::Color32::from_rgb(180, 180, 100))
                        .monospace(),
                    );
                });
            } else {
                ui.label(
                    egui::RichText::new("No tool position")
                        .italics()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
            }

            if let Some(boundary) = sim.current_boundary() {
                ui.horizontal(|ui| {
                    ui.label("Operation:");
                    ui.label(egui::RichText::new(&boundary.name).strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Tool:");
                    ui.label(&boundary.tool_name);
                });
            }

            // Move type from current move index
            let move_info = current_move_info(sim, job);
            ui.horizontal(|ui| {
                ui.label("Move:");
                ui.label(format!(
                    "{} / {}",
                    sim.playback.current_move,
                    sim.total_moves()
                ));
                if let Some((mt, _feed)) = &move_info {
                    ui.label(egui::RichText::new(format!("({})", mt)).small().color(
                        match mt.as_str() {
                            "Rapid" => egui::Color32::from_rgb(200, 200, 80),
                            "Linear" => egui::Color32::from_rgb(100, 180, 100),
                            "Arc CW" | "Arc CCW" => egui::Color32::from_rgb(100, 140, 200),
                            _ => egui::Color32::from_rgb(150, 150, 150),
                        },
                    ));
                }
            });
            if let Some((_, Some(feed))) = &move_info {
                ui.horizontal(|ui| {
                    ui.label("Feed rate:");
                    ui.label(format!("{:.0} mm/min", feed));
                });
            }

            if !sim.playback.tool_type_label.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Tool type:");
                    ui.label(&sim.playback.tool_type_label);
                });
            }
        });

    ui.add_space(4.0);

    // --- Warnings & Flags ---
    egui::CollapsingHeader::new("Warnings & Flags")
        .default_open(true)
        .show(ui, |ui| {
            // Holder clearance (first path only)
            if sim.checks.holder_collision_count == 0 && sim.checks.min_safe_stickout.is_some() {
                ui.label(
                    egui::RichText::new("\u{2705} Holder clearance: Clear")
                        .color(egui::Color32::from_rgb(100, 180, 100)),
                );
            } else if sim.checks.holder_collision_count > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "\u{274C} Holder clearance: {} issues",
                        sim.checks.holder_collision_count
                    ))
                    .color(egui::Color32::from_rgb(220, 80, 80)),
                );
                if let Some(stickout) = sim.checks.min_safe_stickout {
                    ui.label(
                        egui::RichText::new(format!("   Min safe stickout: {:.1} mm", stickout))
                            .small()
                            .color(egui::Color32::from_rgb(180, 140, 80)),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("\u{26A0} Holder clearance: Not checked")
                        .color(egui::Color32::from_rgb(220, 180, 60)),
                );
            }

            // Rapid collisions
            if sim.checks.rapid_collisions.is_empty() {
                ui.label(
                    egui::RichText::new("\u{2705} Rapid collisions: None")
                        .color(egui::Color32::from_rgb(100, 180, 100)),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "\u{274C} Rapid collisions: {}",
                        sim.checks.rapid_collisions.len()
                    ))
                    .color(egui::Color32::from_rgb(220, 80, 80)),
                );
            }

            // Stale results warning
            if sim.is_stale(job.edit_counter) {
                ui.label(
                    egui::RichText::new("\u{26A0} Results stale (params changed)")
                        .color(egui::Color32::from_rgb(220, 180, 60)),
                );
            }

            // Run collision check button
            if sim.checks.holder_collision_count == 0
                && sim.checks.min_safe_stickout.is_none()
                && ui.small_button("Check Holder Clearance").clicked()
            {
                events.push(AppEvent::RunCollisionCheck);
            }
        });

    ui.add_space(4.0);

    // --- Summary Stats ---
    egui::CollapsingHeader::new("Summary Stats")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Total moves:");
                ui.label(format!("{}", sim.total_moves()));
            });
            ui.horizontal(|ui| {
                ui.label("Operations:");
                ui.label(format!("{}", sim.boundaries().len()));
            });

            // Aggregate stats from job toolpaths
            let (total_cutting, total_rapid, total_time_min) = aggregate_stats(sim, job);

            ui.horizontal(|ui| {
                ui.label("Cutting dist:");
                ui.label(format!("{:.0} mm", total_cutting));
            });
            ui.horizontal(|ui| {
                ui.label("Rapid dist:");
                ui.label(format!("{:.0} mm", total_rapid));
            });

            let total_min = total_time_min.floor() as u32;
            let total_sec = ((total_time_min - total_min as f64) * 60.0) as u32;
            ui.horizontal(|ui| {
                ui.label("Est. cycle time:");
                ui.label(
                    egui::RichText::new(format!("{}:{:02}", total_min, total_sec))
                        .strong()
                        .color(egui::Color32::from_rgb(100, 180, 220)),
                );
            });

            // Per-op breakdown table
            if sim.boundaries().len() > 1 {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Per-operation:").small().strong());

                egui::Grid::new("op_stats_grid")
                    .num_columns(3)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Name").small().strong());
                        ui.label(egui::RichText::new("Cut").small().strong());
                        ui.label(egui::RichText::new("Time").small().strong());
                        ui.end_row();

                        for boundary in sim.boundaries() {
                            if let Some(tp) = job.find_toolpath(boundary.id)
                                && let Some(result) = &tp.result
                            {
                                let feed = op_feed_rate(&tp.operation);
                                let time_min = result.stats.cutting_distance / feed;
                                let m = time_min.floor() as u32;
                                let s = ((time_min - m as f64) * 60.0) as u32;

                                ui.label(egui::RichText::new(&boundary.name).small());
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.0}",
                                        result.stats.cutting_distance
                                    ))
                                    .small(),
                                );
                                ui.label(egui::RichText::new(format!("{}:{:02}", m, s)).small());
                                ui.end_row();
                            }
                        }
                    });
            }
        });
}

/// Determine the move type and feed rate at the current move index.
fn current_move_info(sim: &SimulationState, job: &JobState) -> Option<(String, Option<f64>)> {
    let current = sim.playback.current_move;
    let mut cumulative = 0;
    for tp in job.all_toolpaths() {
        if !tp.enabled {
            continue;
        }
        if let Some(result) = &tp.result {
            let tp_moves = result.toolpath.moves.len();
            if current <= cumulative + tp_moves {
                let local_idx = current.saturating_sub(cumulative);
                if local_idx < result.toolpath.moves.len() {
                    let mv = &result.toolpath.moves[local_idx];
                    return Some(match mv.move_type {
                        rs_cam_core::toolpath::MoveType::Rapid => ("Rapid".to_string(), None),
                        rs_cam_core::toolpath::MoveType::Linear { feed_rate } => {
                            ("Linear".to_string(), Some(feed_rate))
                        }
                        rs_cam_core::toolpath::MoveType::ArcCW { feed_rate, .. } => {
                            ("Arc CW".to_string(), Some(feed_rate))
                        }
                        rs_cam_core::toolpath::MoveType::ArcCCW { feed_rate, .. } => {
                            ("Arc CCW".to_string(), Some(feed_rate))
                        }
                    });
                }
            }
            cumulative += tp_moves;
        }
    }
    None
}

/// Aggregate cutting distance, rapid distance, and estimated time across all boundaries.
fn aggregate_stats(sim: &SimulationState, job: &JobState) -> (f64, f64, f64) {
    let mut total_cutting = 0.0;
    let mut total_rapid = 0.0;
    let mut total_time_min = 0.0;

    for boundary in sim.boundaries() {
        if let Some(tp) = job.find_toolpath(boundary.id)
            && let Some(result) = &tp.result
        {
            total_cutting += result.stats.cutting_distance;
            total_rapid += result.stats.rapid_distance;
            let feed = op_feed_rate(&tp.operation);
            total_time_min += result.stats.cutting_distance / feed;
        }
    }

    (total_cutting, total_rapid, total_time_min)
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
