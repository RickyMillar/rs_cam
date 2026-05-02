use super::AppEvent;
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::runtime::GuiState;
use crate::state::simulation::{
    SimulationAnalyticsTab, SimulationIssueKind, SimulationState, StockVizMode,
};
use crate::ui::theme;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::CutKinematics;
use rs_cam_core::tool_load::{
    Confidence, ExceedsReason, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason, Verdict,
};

pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
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

    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let linked_span = sim.active_debug_span(gui, max_feed);
    let current_boundary_id = sim.current_boundary().map(|boundary| boundary.id);

    draw_reactive_inspector(ui, sim, gui, max_feed, &load_report, events);
    ui.separator();

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
                    if ui.small_button("◀ Prev").clicked()
                        && let Some(target) = sim.focus_issue_delta(gui, max_feed, -1)
                    {
                        events.push(AppEvent::SimJumpToMove(target.move_index));
                    }
                    if ui.small_button("Next ▶").clicked()
                        && let Some(target) = sim.focus_issue_delta(gui, max_feed, 1)
                    {
                        events.push(AppEvent::SimJumpToMove(target.move_index));
                    }
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

/// Render a single short summary line in the project summary card showing how
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
