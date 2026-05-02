use super::AppEvent;
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::runtime::GuiState;
use crate::state::selection::Selection;
use crate::state::simulation::{
    SimulationAnalyticsTab, SimulationIssueKind, SimulationState, StockVizMode,
};
use crate::state::toolpath::ToolpathId;
use crate::ui::theme;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::{CutKinematics, SimulationCutSample, SimulationCutTrace};
use rs_cam_core::tool_load::{
    Confidence, ExceedsReason, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason, Verdict,
};

/// Right panel in simulation workspace: current state, warnings, and summary stats.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    selection: &Selection,
    events: &mut Vec<AppEvent>,
) {
    let max_feed = session.machine().max_feed_mm_min;
    ui.heading("Inspector");
    ui.separator();
    draw_analytics_tabs(ui, &mut sim.analytics_tab);
    ui.add_space(4.0);

    let active_tab = sim.analytics_tab;

    let load_report = {
        let sim_trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        rs_cam_core::gcode::project_load_report(session, sim_trace)
    };

    // ── Summary card (go/no-go at a glance) ─────────────────
    if sim.results.is_some() {
        let op_count = sim.boundaries().len();
        let (total_cutting, _total_rapid, total_time_min) = aggregate_stats(sim, session, gui);
        let collision_count = sim.checks.rapid_collisions.len() + sim.checks.holder_collision_count;

        let time_str = if total_time_min >= 1.0 {
            format!("{:.0} min", total_time_min)
        } else {
            format!("{:.0} s", total_time_min * 60.0)
        };

        egui::Frame::default()
            .fill(egui::Color32::from_rgb(36, 38, 48))
            .inner_margin(6.0)
            .rounding(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{op_count} ops"))
                            .strong()
                            .color(theme::TEXT_STRONG),
                    );
                    ui.label(
                        egui::RichText::new(format!("\u{00B7} {time_str}"))
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "\u{00B7} {:.1} m cut",
                            total_cutting / 1000.0
                        ))
                        .color(theme::TEXT_MUTED),
                    );
                    if collision_count > 0 {
                        ui.label(
                            egui::RichText::new(format!("\u{00B7} {collision_count} collisions"))
                                .strong()
                                .color(theme::ERROR),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("\u{00B7} no collisions").color(theme::SUCCESS),
                        );
                    }
                });
                draw_tool_load_summary_line(ui, &load_report);
            });
        ui.add_space(4.0);
    }

    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let linked_span = sim.active_debug_span(gui, max_feed);
    let current_boundary_id = sim.current_boundary().map(|boundary| boundary.id);

    draw_reactive_inspector(ui, sim, gui, max_feed, &load_report, events);
    ui.separator();

    if matches!(active_tab, SimulationAnalyticsTab::RunStatus) {
        draw_run_status_snapshot(ui, sim, gui, &load_report);
        ui.add_space(4.0);
    }

    if sim.debug.enabled {
        ui.horizontal_wrapped(|ui| {
            if sim.debug.pinned_semantic_item.is_some() && ui.button("Clear Pin").clicked() {
                sim.clear_pinned_semantic_item();
            }
            if ui.small_button("Prev Issue").clicked()
                && let Some(target) = sim.focus_issue_delta(gui, max_feed, -1)
            {
                events.push(AppEvent::SimJumpToMove(target.move_index));
            }
            if ui.small_button("Next Issue").clicked()
                && let Some(target) = sim.focus_issue_delta(gui, max_feed, 1)
            {
                events.push(AppEvent::SimJumpToMove(target.move_index));
            }
            if let Some(issue) = sim.current_issue(gui, max_feed) {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        issue_kind_label(issue.kind),
                        issue.label
                    ))
                    .small()
                    .color(theme::WARNING_TEXT),
                );
                if let Some(toolpath_id) = issue.toolpath_id {
                    ui.label(
                        egui::RichText::new(format!("TP {}", toolpath_id.0 + 1))
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }
            }
        });
        ui.add_space(4.0);
    }

    // --- Stock Display ---
    if matches!(active_tab, SimulationAnalyticsTab::RunStatus) {
        egui::CollapsingHeader::new("Stock Display")
        .default_open(true)
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
                        )
                        .on_hover_text("Color by surface deviation: blue = material remaining, green = on target, red = over-cut");
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
            if matches!(sim.stock_viz_mode, StockVizMode::Deviation)
                && sim.playback.display_deviations.is_none()
            {
                ui.label(
                    egui::RichText::new("No deviation data \u{2014} re-run simulation to compute")
                        .small()
                        .color(theme::WARNING),
                );
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
                            .color(theme::WARNING),
                    );
                }
            });
            // Warn when the current resolution would exceed the dexel grid cap
            // or when the resulting mesh would be too large for the GPU.
            {
                let sx = session.stock_config().x;
                let sy = session.stock_config().y;
                let res = sim.resolution;
                if !sim.auto_resolution
                    && rs_cam_core::dexel::DexelGrid::would_exceed_grid(res, sx, sy).is_some()
                {
                    ui.label(
                        egui::RichText::new("Grid too large — resolution will be coarsened")
                            .small()
                            .color(theme::WARNING),
                    );
                }
                // (Large meshes are now auto-chunked for GPU upload — no blank-screen warning needed.)
            }
        });
    }

    ui.add_space(4.0);

    // --- Semantic Context ---
    if matches!(active_tab, SimulationAnalyticsTab::DebugTrace) {
        egui::CollapsingHeader::new("Semantic Context")
            .default_open(false)
            .show(ui, |ui| {
                if let Some(active) = active_semantic.as_ref() {
                    let color = semantic_kind_color(&active.item.kind);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(&active.item.label)
                                .strong()
                                .color(color),
                        );
                        if sim.debug.pinned_semantic_item
                            == Some((active.toolpath_id, active.item.id))
                        {
                            ui.label(
                                egui::RichText::new("Pinned")
                                    .small()
                                    .color(theme::WARNING_TEXT),
                            );
                        }
                        if ui.small_button("Start").clicked()
                            && let Some(target) = sim.trace_target_for_item(
                                gui,
                                max_feed,
                                active.toolpath_id,
                                active.item.id,
                                false,
                            )
                        {
                            events.push(AppEvent::SimJumpToMove(target.move_index));
                        }
                        if ui.small_button("End").clicked()
                            && let Some(target) = sim.trace_target_for_item(
                                gui,
                                max_feed,
                                active.toolpath_id,
                                active.item.id,
                                true,
                            )
                        {
                            events.push(AppEvent::SimJumpToMove(target.move_index));
                        }
                    });
                    ui.label(
                        egui::RichText::new(semantic_kind_label(&active.item.kind))
                            .small()
                            .color(theme::TEXT_MUTED),
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
                    if let Some(metrics) = sim.semantic_runtime_metrics(
                        gui,
                        max_feed,
                        active.toolpath_id,
                        active.item.id,
                    ) {
                        ui.label(format!(
                            "Runtime: {:.2}s total | {:.2}s cutting | {:.2}s rapid",
                            metrics.total_seconds, metrics.cutting_seconds, metrics.rapid_seconds
                        ));
                    }
                    if !active.item.params.values.is_empty() {
                        ui.add_space(4.0);
                        egui::Grid::new("sim_semantic_context_grid")
                            .num_columns(2)
                            .spacing([8.0, 2.0])
                            .show(ui, |ui| {
                                for (idx, (key, value)) in
                                    active.item.params.values.iter().enumerate()
                                {
                                    if idx >= 6 {
                                        break;
                                    }
                                    ui.label(
                                        egui::RichText::new(key).small().color(theme::TEXT_MUTED),
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
                            .color(theme::TEXT_DIM),
                    );
                }
            });
    }

    ui.add_space(4.0);

    // --- Generation Metrics ---
    if matches!(active_tab, SimulationAnalyticsTab::DebugTrace) {
        egui::CollapsingHeader::new("Generation Metrics")
            .default_open(true)
            .show(ui, |ui| {
                let debug_trace = current_boundary_id
                    .and_then(|toolpath_id| gui.toolpath_rt.get(&toolpath_id.0))
                    .and_then(|rt| rt.debug_trace.as_ref());

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
                            .color(theme::INFO),
                        );
                    }
                    ui.label(format!("Hotspots: {}", trace.hotspots.len()));
                    if let Some((toolpath_id, span)) = linked_span.as_ref()
                        && Some(*toolpath_id) == current_boundary_id
                    {
                        ui.label(
                            egui::RichText::new(format!(
                                "Linked span: {} ({:.1} ms)",
                                span.label,
                                span.elapsed_us as f64 / 1000.0
                            ))
                            .small()
                            .color(theme::INFO),
                        );
                        if let Some(summary) = debug_span_math_summary(&span.kind) {
                            ui.label(
                                egui::RichText::new(summary)
                                    .small()
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    }
                    if let Some((_, annotation)) = sim.current_debug_annotation(gui) {
                        ui.label(
                            egui::RichText::new(format!("Annotation: {}", annotation.label))
                                .small()
                                .color(theme::WARNING_TEXT),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("No performance trace available")
                            .small()
                            .italics()
                            .color(theme::TEXT_DIM),
                    );
                }
            });
    }

    if matches!(active_tab, SimulationAnalyticsTab::DebugTrace) {
        draw_trace_provenance(ui, sim, gui, current_boundary_id);
        ui.add_space(4.0);
    }

    ui.add_space(4.0);

    // --- Cutting Metrics ---
    if matches!(active_tab, SimulationAnalyticsTab::CutQuality) {
        egui::CollapsingHeader::new("Cutting Metrics")
        .default_open(true)
        .show(ui, |ui| {
            let current_boundary = sim.current_boundary().map(|boundary| boundary.id);
            let cut_trace = sim
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref());

            if let Some(toolpath_id) = current_boundary
                && let Some(summary) = sim.toolpath_cut_summary(toolpath_id)
            {
                ui.label(format!("Runtime: {:.2}s", summary.total_runtime_s));
                ui.label(format!(
                    "Cut {:.2}s | rapid {:.2}s",
                    summary.cutting_runtime_s, summary.rapid_runtime_s
                ));
                ui.label(format!(
                    "Air {:.2}s | low engage {:.2}s",
                    summary.air_cut_time_s, summary.low_engagement_time_s
                ));
                ui.label(format!(
                    "Avg engagement {:.1}% | avg MRR {:.1} mm^3/s",
                    summary.average_engagement * 100.0,
                    summary.average_mrr_mm3_s
                ));
                if let Some(active_sample) = sim.current_cut_sample()
                    && active_sample.toolpath_id == toolpath_id
                {
                    ui.label(
                        egui::RichText::new(format!(
                            "Sample: move {} | {} | arc {} | {:.1}% engage | {:.3} mm DOC | {:.4} chipload | MRR {:.1}",
                            active_sample.sample.move_index,
                            cut_kinematics_label(active_sample.sample.cut_kinematics),
                            format_arc_engagement(active_sample.sample.arc_engagement_radians),
                            active_sample.sample.radial_engagement * 100.0,
                            active_sample.sample.axial_doc_mm,
                            active_sample.sample.chipload_mm_per_tooth,
                            active_sample.sample.mrr_mm3_s
                        ))
                        .small()
                        .color(theme::SUCCESS),
                    );
                }
                let hotspot_count = sim.cut_hotspots(toolpath_id, 5).len();
                ui.label(format!("Runtime hotspots: {}", hotspot_count));
                draw_cut_quality_findings(ui, sim, toolpath_id, events);
                if let Some(verdict) = load_report
                    .per_toolpath
                    .iter()
                    .find(|v| v.toolpath_id == toolpath_id.0)
                {
                    draw_tool_load_badges(ui, verdict);
                }
                if let Some(active) = active_semantic.as_ref()
                    && active.toolpath_id == toolpath_id
                    && let Some(item_summary) =
                        sim.semantic_cut_summary(toolpath_id, active.item.id)
                {
                    ui.label(
                        egui::RichText::new(format!(
                            "Item: {} | wasted {:.2}s | avg engage {:.1}% | avg MRR {:.1}",
                            active.item.label,
                            item_summary.wasted_runtime_s,
                            item_summary.average_engagement * 100.0,
                            item_summary.average_mrr_mm3_s
                        ))
                        .small()
                        .color(theme::SUCCESS),
                    );
                }
            } else if cut_trace.is_none() {
                ui.label(
                    egui::RichText::new("Enable Cut Metrics and re-run simulation")
                        .small()
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            } else {
                ui.label(
                    egui::RichText::new("No cutting metrics for the current toolpath")
                        .small()
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
        });

        ui.add_space(4.0);

        // --- Cut signals over time ---
        egui::CollapsingHeader::new("Cut signals over time")
            .default_open(false)
            .show(ui, |ui| {
                draw_timeseries_panel(ui, sim, session, selection);
            });
    }

    ui.add_space(4.0);

    // --- Current State ---
    if matches!(active_tab, SimulationAnalyticsTab::RunStatus) {
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
                            .color(theme::TEXT_DIM),
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
                let move_info = current_move_info(sim, session, gui);
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
                                "Linear" => theme::SUCCESS,
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
    }

    ui.add_space(4.0);

    // --- Warnings & Flags ---
    if matches!(active_tab, SimulationAnalyticsTab::Safety) {
        egui::CollapsingHeader::new("Warnings & Flags")
            .default_open(true)
            .show(ui, |ui| {
                // Holder clearance (first path only)
                if sim.checks.holder_collision_count == 0 && sim.checks.min_safe_stickout.is_some()
                {
                    ui.label(
                        egui::RichText::new("\u{2705} Holder clearance: Clear")
                            .color(theme::SUCCESS),
                    );
                } else if sim.checks.holder_collision_count > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "\u{274C} Holder clearance: {} issues",
                            sim.checks.holder_collision_count
                        ))
                        .color(theme::ERROR),
                    );
                    if let Some(stickout) = sim.checks.min_safe_stickout {
                        ui.label(
                            egui::RichText::new(format!(
                                "   Min safe stickout: {:.1} mm",
                                stickout
                            ))
                            .small()
                            .color(theme::WARNING),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("\u{26A0} Holder clearance: Not checked")
                            .color(theme::WARNING),
                    );
                }

                // Rapid collisions
                if sim.checks.rapid_collisions.is_empty() {
                    ui.label(
                        egui::RichText::new("\u{2705} Rapid collisions: None")
                            .color(theme::SUCCESS),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!(
                            "\u{274C} Rapid collisions: {}",
                            sim.checks.rapid_collisions.len()
                        ))
                        .color(theme::ERROR),
                    );
                    if ui.small_button("Jump to first rapid collision").clicked() {
                        let target = sim
                            .checks
                            .rapid_collision_move_indices
                            .first()
                            .copied()
                            .or_else(|| {
                                sim.checks
                                    .rapid_collisions
                                    .first()
                                    .map(|collision| collision.move_index)
                            });
                        if let Some(move_index) = target {
                            events.push(AppEvent::SimJumpToMove(move_index));
                        }
                    }
                }

                // Stale results warning
                if sim.is_stale(gui.edit_counter) {
                    ui.label(
                        egui::RichText::new("\u{26A0} Results stale (params changed)")
                            .color(theme::WARNING),
                    );
                }

                // Run collision check button
                if sim.checks.holder_collision_count == 0
                    && sim.checks.min_safe_stickout.is_none()
                    && ui.small_button("Check Holder Clearance").clicked()
                {
                    events.push(AppEvent::RunCollisionCheck);
                }

                ui.separator();
                draw_tool_load_table(ui, sim, &load_report, events);
            });
    }

    ui.add_space(4.0);

    // --- Summary Stats (detailed breakdown; key info in summary card above) ---
    if matches!(active_tab, SimulationAnalyticsTab::RunStatus) {
        egui::CollapsingHeader::new("Summary Stats")
            .default_open(false)
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
                let (total_cutting, total_rapid, total_time_min) =
                    aggregate_stats(sim, session, gui);

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
                        egui::RichText::new(format!("{}:{:02} min", total_min, total_sec))
                            .strong()
                            .color(theme::INFO),
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
                                if let Some(rt) = gui.toolpath_rt.get(&boundary.id.0)
                                    && let Some(result) = &rt.result
                                    && let Some((_, tc)) =
                                        session.find_toolpath_config_by_id(boundary.id.0)
                                {
                                    let feed = tc.operation.feed_rate();
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
                                    ui.label(
                                        egui::RichText::new(format!("{}:{:02}", m, s)).small(),
                                    );
                                    ui.end_row();
                                }
                            }
                        });
                }
            });
    }
}

fn draw_cut_quality_findings(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    let Some(boundary) = sim
        .boundaries()
        .iter()
        .find(|boundary| boundary.id == toolpath_id)
    else {
        return;
    };

    let cut_hotspots = sim.cut_hotspots(toolpath_id, 3);
    if !cut_hotspots.is_empty() {
        ui.separator();
        ui.label(
            egui::RichText::new("Worst runtime hotspots")
                .small()
                .strong(),
        );
        for hotspot in cut_hotspots {
            ui.horizontal(|ui| {
                if ui.small_button("Jump").clicked() {
                    events.push(AppEvent::SimJumpToMove(
                        boundary.start_move + hotspot.move_start,
                    ));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "wasted {:.2}s | air {:.2}s | low {:.2}s | MRR {:.1}",
                        hotspot.wasted_runtime_s,
                        hotspot.air_cut_time_s,
                        hotspot.low_engagement_time_s,
                        hotspot.average_mrr_mm3_s
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            });
        }
    }

    let Some(trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_ref())
    else {
        return;
    };
    let issues: Vec<_> = trace
        .issues
        .iter()
        .filter(|issue| issue.toolpath_id == toolpath_id.0)
        .take(4)
        .cloned()
        .collect();
    if !issues.is_empty() {
        ui.separator();
        ui.label(
            egui::RichText::new("Cutting issue segments")
                .small()
                .strong(),
        );
        for issue in issues {
            ui.horizontal(|ui| {
                if ui.small_button("Jump").clicked()
                    && let Some(target) = sim.trace_target_for_cut_issue(&issue)
                {
                    events.push(AppEvent::SimJumpToMove(target.move_index));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} {:.2}s | moves {}–{} | min engage {:.1}%",
                        issue.label,
                        issue.duration_s,
                        issue.move_index,
                        issue.end_move_index,
                        issue.min_radial_engagement * 100.0
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            });
        }
    }
}

fn draw_run_status_snapshot(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    gui: &GuiState,
    report: &ToolLoadReport,
) {
    egui::CollapsingHeader::new("Run Status")
        .default_open(true)
        .show(ui, |ui| {
            if sim.results.is_some() {
                let stale = sim.is_stale(gui.edit_counter);
                ui.label(
                    egui::RichText::new(if stale {
                        "⚠ Results stale"
                    } else {
                        "✅ Results fresh"
                    })
                    .color(if stale {
                        theme::WARNING
                    } else {
                        theme::SUCCESS
                    }),
                );
                ui.label(format!("Resolution: {:.3} mm", sim.resolution));

                if let Some(trace) = sim
                    .results
                    .as_ref()
                    .and_then(|results| results.cut_trace.as_ref())
                {
                    let captured_arc = trace
                        .provenance
                        .as_ref()
                        .map(|provenance| provenance.captured_arc_engagement)
                        .unwrap_or_else(|| {
                            trace
                                .samples
                                .iter()
                                .any(|sample| sample.arc_engagement_radians.is_some())
                        });
                    ui.label(egui::RichText::new("✅ Cut metrics captured").color(theme::SUCCESS));
                    ui.label(
                        egui::RichText::new(if captured_arc {
                            "✅ Arc engagement captured"
                        } else {
                            "⚠ Arc engagement not captured — power may be unmodeled"
                        })
                        .color(if captured_arc {
                            theme::SUCCESS
                        } else {
                            theme::WARNING
                        }),
                    );
                    if let Some(provenance) = &trace.provenance {
                        ui.label(
                            egui::RichText::new(format!(
                                "Schema v{} | {} toolpath hashes",
                                provenance.trace_schema_version,
                                provenance.toolpath_hashes.len()
                            ))
                            .small()
                            .color(theme::TEXT_MUTED),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("⚠ Cut metrics not captured").color(theme::WARNING),
                    );
                }

                ui.separator();
                ui.label(egui::RichText::new("Top findings").small().strong());
                let mut has_findings = false;
                if !sim.checks.rapid_collisions.is_empty() {
                    has_findings = true;
                    ui.label(
                        egui::RichText::new(format!(
                            "❌ {} rapid collisions",
                            sim.checks.rapid_collisions.len()
                        ))
                        .color(theme::ERROR),
                    );
                }
                if sim.checks.holder_collision_count > 0 {
                    has_findings = true;
                    ui.label(
                        egui::RichText::new(format!(
                            "❌ {} holder clearance issues",
                            sim.checks.holder_collision_count
                        ))
                        .color(theme::ERROR),
                    );
                }
                if report.any_exceeded() {
                    has_findings = true;
                    ui.label(
                        egui::RichText::new("❌ Tool-load criterion exceeded").color(theme::ERROR),
                    );
                }
                if report.any_unmodeled() {
                    has_findings = true;
                    ui.label(
                        egui::RichText::new("⚠ Some tool-load criteria are unmodeled")
                            .color(theme::WARNING),
                    );
                }
                if let Some(summary) = sim
                    .results
                    .as_ref()
                    .and_then(|results| results.cut_trace.as_ref())
                    .map(|trace| &trace.summary)
                {
                    if summary.air_cut_time_s > 0.0 {
                        has_findings = true;
                        ui.label(
                            egui::RichText::new(format!(
                                "⚠ {:.2}s air cutting",
                                summary.air_cut_time_s
                            ))
                            .color(theme::WARNING_MILD),
                        );
                    }
                    if summary.low_engagement_time_s > 0.0 {
                        has_findings = true;
                        ui.label(
                            egui::RichText::new(format!(
                                "⚠ {:.2}s low engagement",
                                summary.low_engagement_time_s
                            ))
                            .color(theme::WARNING_MILD),
                        );
                    }
                }
                if !has_findings {
                    ui.label(egui::RichText::new("No major findings.").color(theme::SUCCESS));
                }
            } else {
                ui.label(
                    egui::RichText::new("Run simulation to populate analytics.")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
        });
}

fn draw_reactive_inspector(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Inspector")
            .small()
            .strong()
            .color(theme::TEXT_STRONG),
    );
    if sim.results.is_none() {
        ui.label(
            egui::RichText::new(
                "Run simulation, then select a timeline point, issue, hotspot, or toolpath.",
            )
            .small()
            .italics()
            .color(theme::TEXT_DIM),
        );
        return;
    }

    if let Some(issue) = sim.current_issue(gui, max_feed) {
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(42, 36, 28))
            .inner_margin(6.0)
            .rounding(4.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        issue_kind_label(issue.kind),
                        issue.label
                    ))
                    .strong()
                    .color(theme::WARNING_TEXT),
                );
                ui.label(format!("Move {}", issue.move_index));
                ui.horizontal(|ui| {
                    if ui.small_button("Jump").clicked() {
                        events.push(AppEvent::SimJumpToMove(issue.move_index));
                    }
                    if let Some(toolpath_id) = issue.toolpath_id
                        && ui.small_button("Suggest").clicked()
                    {
                        events.push(AppEvent::OpenSuggestModal(toolpath_id));
                    }
                });
            });
        ui.add_space(4.0);
    }

    if let Some(active) = sim.current_cut_sample() {
        ui.label(egui::RichText::new("Sample").small().strong());
        egui::Grid::new("reactive_sample_grid")
            .num_columns(2)
            .spacing([8.0, 2.0])
            .show(ui, |ui| {
                ui.label("toolpath");
                ui.label(format!("{}", active.toolpath_id.0 + 1));
                ui.end_row();
                ui.label("time");
                ui.label(format!("{:.2}s", active.sample.cumulative_time_s));
                ui.end_row();
                ui.label("feed");
                ui.label(format!("{:.0} mm/min", active.sample.feed_rate_mm_min));
                ui.end_row();
                ui.label("chip thickness");
                ui.label(
                    active
                        .sample
                        .effective_chip_thickness_mm
                        .map(|v| format!("{v:.4} mm"))
                        .unwrap_or_else(|| "unmodeled".to_owned()),
                );
                ui.end_row();
                ui.label("arc engagement");
                ui.label(format_arc_engagement(active.sample.arc_engagement_radians));
                ui.end_row();
                ui.label("axial DOC");
                ui.label(format!("{:.3} mm", active.sample.axial_doc_mm));
                ui.end_row();
                ui.label("MRR");
                ui.label(format!("{:.1} mm³/s", active.sample.mrr_mm3_s));
                ui.end_row();
            });
        ui.add_space(4.0);
    }

    if let Some(boundary) = sim.current_boundary() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&boundary.name).strong());
            if ui.small_button("Suggest").clicked() {
                events.push(AppEvent::OpenSuggestModal(boundary.id));
            }
            if ui.small_button("Jump start").clicked() {
                events.push(AppEvent::SimJumpToMove(boundary.start_move));
            }
        });
        if let Some(tp) = load_report
            .per_toolpath
            .iter()
            .find(|tp| tp.toolpath_id == boundary.id.0)
        {
            draw_tool_load_badges(ui, tp);
            ui.label(verdict_label(&tp.chipload));
            ui.label(verdict_label(&tp.power));
            ui.label(verdict_label(&tp.deflection));
        }
        if let Some(summary) = sim.toolpath_cut_summary(boundary.id) {
            ui.label(
                egui::RichText::new(format!(
                    "cut {:.1}s · air {:.1}s · low-engage {:.1}s · peak chip {:.4}",
                    summary.cutting_runtime_s,
                    summary.air_cut_time_s,
                    summary.low_engagement_time_s,
                    summary.peak_chipload_mm_per_tooth
                ))
                .small()
                .color(theme::TEXT_MUTED),
            );
        }
    } else {
        ui.label(
            egui::RichText::new("Select a toolpath segment, signal peak, issue, or hotspot.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
    }
}

fn draw_analytics_tabs(ui: &mut egui::Ui, active_tab: &mut SimulationAnalyticsTab) {
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(active_tab, SimulationAnalyticsTab::RunStatus, "Run Status")
            .on_hover_text("Freshness, capture status, runtime, and top findings.");
        ui.selectable_value(
            active_tab,
            SimulationAnalyticsTab::Safety,
            "Safety / Tool Load",
        )
        .on_hover_text("Collisions, holder clearance, chipload, power, and deflection.");
        ui.selectable_value(
            active_tab,
            SimulationAnalyticsTab::CutQuality,
            "Cut Quality",
        )
        .on_hover_text("Air cutting, engagement, MRR, issues, hotspots, and active sample.");
        ui.selectable_value(
            active_tab,
            SimulationAnalyticsTab::DebugTrace,
            "Debug / Trace",
        )
        .on_hover_text("Generation spans, semantic/debug internals, artifacts, and provenance.");
    });
}

fn draw_trace_provenance(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    gui: &GuiState,
    current_boundary_id: Option<crate::state::toolpath::ToolpathId>,
) {
    egui::CollapsingHeader::new("Trace Artifacts & Provenance")
        .default_open(true)
        .show(ui, |ui| {
            if let Some(toolpath_id) = current_boundary_id
                && let Some(rt) = gui.toolpath_rt.get(&toolpath_id.0)
            {
                if let Some(path) = &rt.debug_trace_path {
                    ui.label("Generation trace:");
                    ui.label(
                        egui::RichText::new(path.display().to_string())
                            .small()
                            .monospace()
                            .color(theme::INFO),
                    );
                }
                if let Some(trace) = &rt.semantic_trace {
                    ui.label(format!(
                        "Semantic items: {} (move-linked {})",
                        trace.summary.item_count, trace.summary.move_linked_item_count
                    ));
                }
                if let Some(trace) = &rt.debug_trace {
                    ui.label(format!(
                        "Generation spans: {} | annotations: {}",
                        trace.spans.len(),
                        trace.annotations.len()
                    ));
                }
            }

            if let Some(results) = sim.results.as_ref() {
                if let Some(path) = &results.cut_trace_path {
                    ui.label("Cut trace:");
                    ui.label(
                        egui::RichText::new(path.display().to_string())
                            .small()
                            .monospace()
                            .color(theme::SUCCESS),
                    );
                }
                if let Some(trace) = results.cut_trace.as_ref() {
                    ui.label(format!(
                        "Cut samples: {} | issues: {} | hotspots: {}",
                        trace.samples.len(),
                        trace.issues.len(),
                        trace.hotspots.len()
                    ));
                    if let Some(provenance) = &trace.provenance {
                        ui.label(format!(
                            "Schema v{} | stock hash {:016x} | machine hash {:016x}",
                            provenance.trace_schema_version,
                            provenance.stock_hash,
                            provenance.machine_hash
                        ));
                    }
                }
            }
        });
}

fn draw_tool_load_table(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    ui.label(egui::RichText::new("Tool-load guardrails").small().strong());
    if report.per_toolpath.is_empty() {
        ui.label(
            egui::RichText::new("No tool-load data available.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    egui::Grid::new("simulation_tool_load_table")
        .num_columns(6)
        .spacing([8.0, 3.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Operation").small().strong());
            ui.label(egui::RichText::new("Chipload").small().strong());
            ui.label(egui::RichText::new("Power").small().strong());
            ui.label(egui::RichText::new("Deflection").small().strong());
            ui.label(egui::RichText::new("Jump").small().strong());
            ui.label(egui::RichText::new("Suggest").small().strong());
            ui.end_row();

            for verdict in &report.per_toolpath {
                let name = sim
                    .boundaries()
                    .iter()
                    .find(|boundary| boundary.id.0 == verdict.toolpath_id)
                    .map(|boundary| boundary.name.as_str())
                    .unwrap_or("Toolpath");
                ui.label(egui::RichText::new(name).small());
                compact_verdict_cell(ui, &verdict.chipload);
                compact_verdict_cell(ui, &verdict.power);
                compact_verdict_cell(ui, &verdict.deflection);
                if let Some(move_index) = first_exceeded_move(sim, verdict) {
                    if ui.small_button("Jump").clicked() {
                        events.push(AppEvent::SimJumpToMove(move_index));
                    }
                } else {
                    ui.label(egui::RichText::new("—").small().color(theme::TEXT_DIM));
                }
                if ui.small_button("Suggest").clicked() {
                    events.push(AppEvent::OpenSuggestModal(
                        crate::state::toolpath::ToolpathId(verdict.toolpath_id),
                    ));
                }
                ui.end_row();
            }
        });
}

fn first_exceeded_move(sim: &SimulationState, verdict: &ToolpathLoadVerdict) -> Option<usize> {
    let sample_index = [&verdict.chipload, &verdict.power, &verdict.deflection]
        .iter()
        .find_map(|criterion| match criterion {
            Verdict::Exceeds { sample_range, .. } => Some(sample_range.start),
            Verdict::Within { .. } | Verdict::Unmodeled { .. } => None,
        })?;
    let sample = sim
        .results
        .as_ref()?
        .cut_trace
        .as_ref()?
        .samples
        .get(sample_index)?;
    let boundary_start = sim
        .boundaries()
        .iter()
        .find(|boundary| boundary.id.0 == sample.toolpath_id)
        .map(|boundary| boundary.start_move)
        .unwrap_or_default();
    Some(boundary_start + sample.move_index)
}

fn compact_verdict_cell(ui: &mut egui::Ui, verdict: &Verdict) {
    let (text, color) = match verdict {
        Verdict::Within {
            confidence: Confidence::Validated,
            ..
        } => ("✅ Within", theme::SUCCESS),
        Verdict::Within {
            confidence: Confidence::Approximate(_),
            ..
        } => ("⚠ Within≈", theme::WARNING_MILD),
        Verdict::Exceeds { .. } => ("❌ Exceeds", theme::ERROR),
        Verdict::Unmodeled { .. } => ("⚠ Unmodeled", theme::WARNING),
    };
    ui.label(egui::RichText::new(text).small().color(color))
        .on_hover_text(verdict_tooltip(verdict));
}

fn cut_kinematics_label(kind: CutKinematics) -> &'static str {
    match kind {
        CutKinematics::Linear => "linear",
        CutKinematics::Plunge => "plunge",
        CutKinematics::Helix => "helix",
        CutKinematics::Arc => "arc",
        CutKinematics::Rapid => "rapid",
    }
}

fn format_arc_engagement(arc: Option<f64>) -> String {
    match arc {
        Some(radians) => {
            let degrees = radians.to_degrees();
            if (degrees - 180.0).abs() <= 5.0 {
                "180° slot".to_owned()
            } else {
                format!("{degrees:.0}°")
            }
        }
        None => "not captured".to_owned(),
    }
}

fn issue_kind_label(kind: SimulationIssueKind) -> &'static str {
    match kind {
        SimulationIssueKind::Hotspot => "Hotspot",
        SimulationIssueKind::Annotation => "Annotation",
        SimulationIssueKind::AirCut => "Air cut",
        SimulationIssueKind::LowEngagement => "Low engagement",
        SimulationIssueKind::RapidCollision => "Rapid collision",
        SimulationIssueKind::HolderCollision => "Holder collision",
    }
}

/// Determine the move type and feed rate at the current move index.
// SAFETY: local_idx bounds-checked against moves.len() before indexing
#[allow(clippy::indexing_slicing)]
fn current_move_info(
    sim: &SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) -> Option<(String, Option<f64>)> {
    let current = sim.playback.current_move;
    let mut cumulative = 0;
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        if let Some(rt) = gui.toolpath_rt.get(&tc.id)
            && let Some(result) = &rt.result
        {
            let tp_moves = result.toolpath.moves.len();
            if current <= cumulative + tp_moves {
                let local_idx = current.saturating_sub(cumulative);
                if local_idx < result.toolpath.moves.len() {
                    let mv = &result.toolpath.moves[local_idx];
                    return Some(match mv.move_type {
                        rs_cam_core::toolpath::MoveType::Rapid => ("Rapid".to_owned(), None),
                        rs_cam_core::toolpath::MoveType::Linear { feed_rate } => {
                            ("Linear".to_owned(), Some(feed_rate))
                        }
                        rs_cam_core::toolpath::MoveType::ArcCW { feed_rate, .. } => {
                            ("Arc CW".to_owned(), Some(feed_rate))
                        }
                        rs_cam_core::toolpath::MoveType::ArcCCW { feed_rate, .. } => {
                            ("Arc CCW".to_owned(), Some(feed_rate))
                        }
                    });
                }
            }
            cumulative += tp_moves;
        }
    }
    None
}

/// Render a single short summary line in the project summary card showing how
/// many toolpaths have each guardrail criterion modeled, plus a flag if any
/// criterion is `Exceeds`. No scalar load %; deliberately keeps the three
/// criteria visible separately.
fn draw_tool_load_summary_line(ui: &mut egui::Ui, report: &ToolLoadReport) {
    let total = report.per_toolpath.len();
    if total == 0 {
        return;
    }
    let chip_modeled = report
        .per_toolpath
        .iter()
        .filter(|v| !v.chipload.is_unmodeled())
        .count();
    let power_modeled = report
        .per_toolpath
        .iter()
        .filter(|v| !v.power.is_unmodeled())
        .count();
    let defl_modeled = report
        .per_toolpath
        .iter()
        .filter(|v| !v.deflection.is_unmodeled())
        .count();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Load:")
                .strong()
                .color(theme::TEXT_STRONG),
        );
        ui.label(
            egui::RichText::new(format!("chipload {chip_modeled}/{total}"))
                .color(theme::TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new(format!("\u{00B7} power {power_modeled}/{total}"))
                .color(theme::TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new(format!("\u{00B7} deflection {defl_modeled}/{total}"))
                .color(theme::TEXT_MUTED),
        );
        if report.any_exceeded() {
            ui.label(
                egui::RichText::new("\u{00B7} EXCEEDED")
                    .strong()
                    .color(theme::ERROR),
            );
        }
    });
}

/// Render three independent badges (chipload | power | deflection) for the
/// active toolpath. **Never** combine into a single load %.
fn draw_tool_load_badges(ui: &mut egui::Ui, verdict: &ToolpathLoadVerdict) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tool load:")
                .small()
                .color(theme::TEXT_STRONG),
        );
        verdict_badge(ui, "chipload", &verdict.chipload);
        verdict_badge(ui, "power", &verdict.power);
        verdict_badge(ui, "deflection", &verdict.deflection);
    });
}

fn verdict_label(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Within { peak, .. } => format!("within · peak {peak:.4}"),
        Verdict::Exceeds { peak, reason, .. } => format!("exceeds · {reason:?} · peak {peak:.4}"),
        Verdict::Unmodeled { reason } => format!("unmodeled · {reason:?}"),
    }
}

fn verdict_badge(ui: &mut egui::Ui, label: &str, verdict: &Verdict) {
    let (color, status) = match verdict {
        Verdict::Within {
            confidence: Confidence::Validated,
            ..
        } => (theme::SUCCESS, "OK"),
        Verdict::Within {
            confidence: Confidence::Approximate(_),
            ..
        } => (theme::WARNING_MILD, "OK\u{2248}"),
        Verdict::Exceeds { .. } => (theme::ERROR, "FAIL"),
        Verdict::Unmodeled { .. } => (theme::TEXT_DIM, "—"),
    };
    let text = format!("{label} {status}");
    ui.label(egui::RichText::new(text).small().color(color))
        .on_hover_text(verdict_tooltip(verdict));
}

fn verdict_tooltip(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Within { peak, confidence } => match confidence {
            Confidence::Validated => format!("Within bounds (peak {peak:.3}) — validated"),
            Confidence::Approximate(why) => {
                format!("Within bounds (peak {peak:.3}) — approximate: {why}")
            }
        },
        Verdict::Exceeds {
            peak,
            reason,
            confidence,
            ..
        } => {
            let reason_str = match reason {
                ExceedsReason::ChiploadBurnRisk => {
                    "chipload below vendor min — rubbing/burning risk"
                }
                ExceedsReason::ChiploadBreakageRisk => "chipload above vendor max — breakage risk",
                ExceedsReason::LongToolStiffnessUnsafe => "L/D > 6 — tool stickout too long",
                ExceedsReason::SpindlePowerExceeded => {
                    "predicted spindle power exceeds machine limit"
                }
            };
            let conf = match confidence {
                Confidence::Validated => "validated".to_owned(),
                Confidence::Approximate(why) => format!("approximate: {why}"),
            };
            format!("EXCEEDS: {reason_str} (peak {peak:.3}, {conf})")
        }
        Verdict::Unmodeled { reason } => match reason {
            UnmodeledReason::SimulationRequired => {
                "Unmodeled: simulation has not been run".to_owned()
            }
            UnmodeledReason::StaleSimulation => {
                "Unmodeled: simulation trace is stale — re-run simulation".to_owned()
            }
            UnmodeledReason::ArcEngagementNotCaptured => {
                "Unmodeled: arc-engagement metric not captured — enable Cut Metrics and re-run"
                    .to_owned()
            }
            UnmodeledReason::NoVendorData => {
                "Unmodeled: no vendor LUT row for this tool/material combination".to_owned()
            }
            UnmodeledReason::SteadyStateSamplesNotPresent => {
                "Unmodeled: no steady-state cutting samples — toolpath runs entirely on transient (plunge/ramp) feeds".to_owned()
            }
            UnmodeledReason::MaterialUnvalidated => {
                "Unmodeled: material is Custom without a validated Kc value".to_owned()
            }
            UnmodeledReason::CutterModeUnsupported(why) => {
                format!("Unmodeled: cutter mode unsupported — {why}")
            }
            UnmodeledReason::NotImplemented(phase) => {
                format!("Unmodeled: not implemented yet — {phase}")
            }
        },
    }
}

/// Aggregate cutting distance, rapid distance, and estimated time across all boundaries.
fn aggregate_stats(
    sim: &SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) -> (f64, f64, f64) {
    let mut total_cutting = 0.0;
    let mut total_rapid = 0.0;
    let mut total_time_min = 0.0;

    for boundary in sim.boundaries() {
        if let Some(rt) = gui.toolpath_rt.get(&boundary.id.0)
            && let Some(result) = &rt.result
            && let Some((_, tc)) = session.find_toolpath_config_by_id(boundary.id.0)
        {
            total_cutting += result.stats.cutting_distance;
            total_rapid += result.stats.rapid_distance;
            let feed = tc.operation.feed_rate();
            total_time_min += result.stats.cutting_distance / feed;
        }
    }

    (total_cutting, total_rapid, total_time_min)
}

/// Resolve the toolpath whose signals to plot. Prefers an explicit
/// toolpath selection in the project tree, falls back to the playback
/// head's current toolpath. Returns `None` when neither is available.
fn active_timeseries_toolpath(sim: &SimulationState, selection: &Selection) -> Option<ToolpathId> {
    if let Selection::Toolpath(id) = selection {
        return Some(*id);
    }
    sim.current_boundary().map(|b| b.id)
}

/// Maximum points kept per series before stride-skipping. Keeps the
/// plot interactive on traces with tens of thousands of samples.
const TIMESERIES_MAX_POINTS: usize = 2000;

fn collect_series<F>(samples: &[&SimulationCutSample], extract: F) -> Vec<[f64; 2]>
where
    F: Fn(&SimulationCutSample) -> Option<f64>,
{
    let mut points: Vec<[f64; 2]> = samples
        .iter()
        .filter_map(|s| extract(s).map(|y| [s.cumulative_time_s, y]))
        .collect();
    if points.len() > TIMESERIES_MAX_POINTS {
        let stride = points.len().div_ceil(TIMESERIES_MAX_POINTS);
        points = points
            .into_iter()
            .enumerate()
            .filter_map(|(i, p)| if i % stride == 0 { Some(p) } else { None })
            .collect();
    }
    points
}

fn draw_timeseries_panel(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    session: &ProjectSession,
    selection: &Selection,
) {
    let Some(toolpath_id) = active_timeseries_toolpath(sim, selection) else {
        ui.label(
            egui::RichText::new("Select a toolpath to see signals over time")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    };

    let Some(trace) = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref()) else {
        ui.label(
            egui::RichText::new("Run a simulation with Cut Metrics enabled")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    };

    let samples: Vec<&SimulationCutSample> = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id.0 && s.is_cutting)
        .collect();
    if samples.is_empty() {
        ui.label(
            egui::RichText::new("No cutting samples for this toolpath")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    let chipload_pts = collect_series(&samples, |s| s.effective_chip_thickness_mm);
    let arc_pts = collect_series(&samples, |s| s.arc_engagement_radians);
    let doc_pts = collect_series(&samples, |s| {
        if s.axial_doc_mm > 0.0 {
            Some(s.axial_doc_mm)
        } else {
            None
        }
    });

    // Pull chipload envelope from the suggest module when available.
    let envelope = chipload_envelope_for_toolpath(session, Some(trace), toolpath_id);

    Plot::new("cut_signals_timeseries")
        .legend(Legend::default())
        .height(200.0)
        .allow_zoom([true, false])
        .allow_drag([true, false])
        .show(ui, |plot_ui| {
            if !chipload_pts.is_empty() {
                plot_ui.line(
                    Line::new(PlotPoints::from(chipload_pts))
                        .name("chipload (mm/tooth)")
                        .color(egui::Color32::from_rgb(230, 200, 60)),
                );
            }
            if !arc_pts.is_empty() {
                plot_ui.line(
                    Line::new(PlotPoints::from(arc_pts))
                        .name("arc engagement (rad)")
                        .color(egui::Color32::from_rgb(80, 200, 120)),
                );
            }
            if !doc_pts.is_empty() {
                plot_ui.line(
                    Line::new(PlotPoints::from(doc_pts))
                        .name("axial DOC (mm)")
                        .color(egui::Color32::from_rgb(110, 150, 230)),
                );
            }
            if let Some((cl_min, cl_max)) = envelope {
                let t0 = samples.first().map(|s| s.cumulative_time_s).unwrap_or(0.0);
                let t1 = samples.last().map(|s| s.cumulative_time_s).unwrap_or(t0);
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[t0, cl_min], [t1, cl_min]]))
                        .name("cl_min")
                        .color(egui::Color32::from_rgb(200, 80, 80))
                        .style(egui_plot::LineStyle::dashed_loose()),
                );
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[t0, cl_max], [t1, cl_max]]))
                        .name("cl_max")
                        .color(egui::Color32::from_rgb(200, 80, 80))
                        .style(egui_plot::LineStyle::dashed_loose()),
                );
            }
        });

    if envelope.is_none() {
        ui.label(
            egui::RichText::new("No vendor envelope for this toolpath — bounds not shown")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
    }
}

/// Look up the matched LUT row's chipload window for one toolpath via
/// the suggest module. Returns `None` when the toolpath has no model-able
/// envelope (custom material, no vendor data, etc.).
fn chipload_envelope_for_toolpath(
    session: &ProjectSession,
    sim_trace: Option<&SimulationCutTrace>,
    toolpath_id: ToolpathId,
) -> Option<(f64, f64)> {
    let suggestions = rs_cam_core::tool_load::suggest::project_suggestions(session, sim_trace);
    let s = suggestions
        .into_iter()
        .find(|s| s.toolpath_id == toolpath_id.0)?;
    let suggested = s.suggested.ok()?;
    Some((
        suggested.chipload_envelope.start,
        suggested.chipload_envelope.end,
    ))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn formats_arc_engagement_for_readability() {
        assert_eq!(format_arc_engagement(None), "not captured");
        assert_eq!(
            format_arc_engagement(Some(std::f64::consts::FRAC_PI_2)),
            "90°"
        );
        assert_eq!(
            format_arc_engagement(Some(std::f64::consts::PI)),
            "180° slot"
        );
    }

    #[test]
    fn arc_engagement_unmodeled_tooltip_points_to_cut_metrics() {
        let tooltip = verdict_tooltip(&Verdict::Unmodeled {
            reason: UnmodeledReason::ArcEngagementNotCaptured,
        });
        assert!(tooltip.contains("Cut Metrics"));
    }
}
