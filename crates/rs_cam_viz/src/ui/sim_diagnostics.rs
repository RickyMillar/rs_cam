use super::AppEvent;
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::runtime::GuiState;
use crate::state::simulation::{SimulationIssueKind, SimulationState, StockVizMode};
use crate::state::toolpath::ToolpathId;
use crate::ui::theme;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::{
    Confidence, ExceedsReason, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason, Verdict,
};

pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    viewport: &mut crate::state::viewport::ViewportState,
    events: &mut Vec<AppEvent>,
) {
    let max_feed = session.machine().max_feed_mm_min;
    ui.heading("Inspector");
    ui.separator();

    let load_report = {
        let sim_trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        rs_cam_core::gcode::project_load_report(session, sim_trace)
    };

    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let linked_span = sim.active_debug_span(gui, max_feed);
    let current_boundary_id = sim.current_boundary().map(|boundary| boundary.id);

    draw_reactive_inspector(ui, sim, session, gui, max_feed, &load_report, events);
    ui.separator();

    // --- View ---
    // Display-only settings: how things look in the workspace. Capture
    // toggles live in the left-panel "Setup & run" section instead, next
    // to the simulation Run button.
    let any_traces_recorded = gui
        .toolpath_rt
        .values()
        .any(|rt| rt.debug_trace.is_some() || rt.semantic_trace.is_some());

    egui::CollapsingHeader::new("View")
        .default_open(true)
        .show(ui, |ui| {
            // Stock visibility — basic show/hide and opacity. Color modes
            // (Deviation, By Height) live under "Analysis" below.
            ui.label(
                egui::RichText::new("Stock")
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            ui.checkbox(&mut viewport.show_stock, "Show stock")
                .on_hover_text("Show the simulated stock mesh in the 3D viewport.");
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                ui.add(egui::Slider::new(&mut sim.stock_opacity, 0.0..=1.0).show_value(true));
            });

            ui.add_space(8.0);

            // Toolpath visibility — project-wide show/hide for cutting and
            // rapid moves. Per-toolpath overrides remain on each row's
            // toolpath_row_controls below.
            ui.label(
                egui::RichText::new("Toolpaths")
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            ui.checkbox(&mut viewport.show_cutting, "Show cutting moves")
                .on_hover_text("Show green cutting-feed lines in the 3D viewport.");
            ui.checkbox(&mut viewport.show_rapids, "Show rapid moves")
                .on_hover_text("Show orange rapid-traverse lines in the 3D viewport.");

            ui.add_space(8.0);

            // Analysis — coloring and overlays that surface analysis data
            // on top of the basic visibility above. Stock color modes
            // (Deviation, By Height) live here, plus the generator-step
            // overlay when traces are recorded.
            ui.label(
                egui::RichText::new("Analysis")
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            let prev_mode = sim.stock_viz_mode;
            ui.horizontal(|ui| {
                ui.label("Stock color:");
                egui::ComboBox::from_id_salt("stock_viz_mode")
                    .selected_text(match sim.stock_viz_mode {
                        StockVizMode::Solid => "Solid",
                        StockVizMode::Deviation => "Deviation",
                        StockVizMode::ByOperation => "Solid", // placeholder: treated as Solid
                        StockVizMode::ByHeight => "By Height",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sim.stock_viz_mode, StockVizMode::Solid, "Solid")
                            .on_hover_text("Default wood-tone gradient. No analysis coloring.");
                        ui.selectable_value(
                            &mut sim.stock_viz_mode,
                            StockVizMode::Deviation,
                            "Deviation",
                        )
                        .on_hover_text(
                            "Color by surface deviation: blue = material remaining, green = on target, red = over-cut.",
                        );
                        ui.selectable_value(
                            &mut sim.stock_viz_mode,
                            StockVizMode::ByHeight,
                            "By Height",
                        )
                        .on_hover_text("Color by Z height: low = blue, high = red.");
                    });
            });
            if sim.stock_viz_mode != prev_mode {
                events.push(AppEvent::SimVizModeChanged);
            }
            if matches!(sim.stock_viz_mode, StockVizMode::Deviation)
                && sim.playback.display_deviations.is_none()
            {
                ui.label(
                    egui::RichText::new("No deviation data — re-run simulation to compute")
                        .small()
                        .color(theme::WARNING),
                );
            }

            // Generator overlay — only meaningful when traces are recorded.
            // Capture from the left-panel "Setup & run" section and re-
            // generate to populate.
            if any_traces_recorded {
                ui.checkbox(&mut sim.debug.enabled, "Show generator steps")
                    .on_hover_text(
                        "Add a semantic timeline band on the boundary timeline and a per-toolpath outline of generator steps.",
                    );
                if sim.debug.enabled {
                    ui.checkbox(
                        &mut sim.debug.highlight_active_item,
                        "Highlight active step",
                    )
                    .on_hover_text(
                        "When playback is inside a generator step, highlight that step's geometry in the 3D viewport.",
                    );
                }
            }
        });

    ui.add_space(4.0);

    // --- Semantic Context ---
    {
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
    {
        egui::CollapsingHeader::new("Generation Metrics")
            .default_open(false)
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

    {
        draw_trace_provenance(ui, sim, gui, current_boundary_id);
        ui.add_space(4.0);
    }
}

fn draw_reactive_inspector(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    if sim.results.is_none() {
        ui.label(
            egui::RichText::new("Run simulation to see the cut overview here.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    // Priority order for what to display:
    // 1. Focused hotspot card (user clicked a 3D-viewport pin or graph dot).
    // 2. Issue card (an air-cut / low-engagement issue at the current move).
    // 3. Project overview (the default — cut totals + counts).
    //
    // We deliberately don't stack these; showing the overview *and* a
    // hotspot card together is what the user called out as too much info.

    if let Some(()) = draw_focused_hotspot_card(ui, sim, events) {
        return;
    }
    if let Some(()) = draw_focused_issue_card(ui, sim, gui, max_feed, events) {
        return;
    }

    draw_project_overview(ui, sim, session, gui, max_feed, load_report, events);
}

/// Hotspot card. Returns `Some(())` when drawn so the caller can early-return.
fn draw_focused_hotspot_card(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    events: &mut Vec<AppEvent>,
) -> Option<()> {
    let h = sim.focused_hotspot_data()?;
    let tp_id = h.toolpath_id;
    let move_start = h.move_start;
    let move_end = h.move_end;
    let sample_count = h.sample_index_end - h.sample_index_start;
    let wasted = h.wasted_runtime_s;
    let peak_chip = h.peak_chipload_mm_per_tooth;
    let peak_doc = h.peak_axial_doc_mm;
    let avg_eng = h.average_engagement;
    let pos = h.representative_position;
    let toolpath_id = ToolpathId(tp_id);
    let global_start = sim
        .global_move_for_local(toolpath_id, move_start)
        .unwrap_or(move_start);

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(50, 38, 28))
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Hotspot")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 170, 90)),
                );
                ui.label(
                    egui::RichText::new(format!("TP {}", tp_id + 1))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            });
            ui.label(
                egui::RichText::new(format!(
                    "Moves {move_start}–{move_end} · {sample_count} samples"
                ))
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new(format!(
                    "wasted {wasted:.2}s · peak chip {peak_chip:.4} · peak DOC {peak_doc:.2} · avg engage {:.0}%",
                    avg_eng * 100.0
                ))
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new(format!("X{:.1} Y{:.1} Z{:.2}", pos[0], pos[1], pos[2]))
                    .small()
                    .monospace()
                    .color(theme::TEXT_DIM),
            );
            ui.horizontal(|ui| {
                if ui.small_button("Jump").clicked() {
                    events.push(AppEvent::SimJumpToMove(global_start));
                }
                if ui.small_button("Optimize").clicked() {
                    events.push(AppEvent::OpenOptimizeModal(toolpath_id));
                }
                if ui.small_button("Clear").clicked() {
                    sim.debug.focused_hotspot = None;
                }
            });
        });
    Some(())
}

/// Issue card. Returns `Some(())` when drawn so the caller can early-return.
fn draw_focused_issue_card(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    events: &mut Vec<AppEvent>,
) -> Option<()> {
    let issue = sim.current_issue(gui, max_feed)?;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(42, 36, 28))
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{}: {}", issue_kind_label(issue.kind), issue.label))
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
                    && ui.small_button("Optimize").clicked()
                {
                    events.push(AppEvent::OpenOptimizeModal(toolpath_id));
                }
            });
        });
    Some(())
}

/// Default state: a single-screen overview of the whole cut. Cycle time
/// + total moves/ops at the top, issue/safety counts in a key-value grid
///   below, then a slim "Now playing: TP X" strip with verdict badges when
///   playback is inside a toolpath. Replaces the previous Cutting Metrics,
///   Warnings & Flags, and Summary Stats sections in the right panel.
#[allow(clippy::too_many_arguments)]
fn draw_project_overview(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    let (total_cutting, total_rapid, total_time_min) = aggregate_stats(sim, session, gui);
    let total_min = total_time_min.floor() as u32;
    let total_sec = ((total_time_min - total_min as f64) * 60.0) as u32;

    let (ok, _warn, bad, unmodeled) = verdict_counts_local(load_report);
    let collision_count = sim.checks.rapid_collisions.len() + sim.checks.holder_collision_count;
    let issue_count = sim.issues(gui, max_feed).len();
    let hotspot_count = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(|t| t.hotspots.len())
        .unwrap_or(0);

    // Big cycle-time line.
    ui.label(
        egui::RichText::new(format!("Cycle: {}:{:02} min", total_min, total_sec))
            .strong()
            .color(theme::INFO),
    );

    egui::Grid::new("cut_overview_grid")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Moves")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{}", sim.total_moves())).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Operations")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{}", sim.boundaries().len())).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Cut distance")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{:.0} mm", total_cutting)).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Rapid distance")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{:.0} mm", total_rapid)).small());
            ui.end_row();
        });

    ui.add_space(4.0);
    ui.separator();

    // Issue counts row — colored to match the bottom-panel HUD pills, but
    // here as a single horizontal summary.
    ui.label(egui::RichText::new("Findings").small().strong());
    egui::Grid::new("cut_overview_findings")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Within bounds")
                    .small()
                    .color(egui::Color32::from_rgb(120, 200, 130)),
            );
            ui.label(egui::RichText::new(format!("{ok}")).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Exceedances")
                    .small()
                    .color(egui::Color32::from_rgb(220, 90, 90)),
            );
            ui.label(egui::RichText::new(format!("{bad}")).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Unmodeled")
                    .small()
                    .color(egui::Color32::from_rgb(210, 170, 80)),
            );
            ui.label(egui::RichText::new(format!("{unmodeled}")).small());
            ui.end_row();
            let collision_color = if collision_count == 0 {
                theme::SUCCESS
            } else {
                theme::ERROR
            };
            ui.label(
                egui::RichText::new("Collisions")
                    .small()
                    .color(collision_color),
            );
            ui.label(egui::RichText::new(format!("{collision_count}")).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Issues")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{issue_count}")).small());
            ui.end_row();
            ui.label(
                egui::RichText::new("Hotspots")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(format!("{hotspot_count}")).small());
            ui.end_row();
        });

    if sim.is_stale(gui.edit_counter) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("⚠ Results stale (params changed) — re-run sim")
                .small()
                .color(theme::WARNING),
        );
    }

    // Slim "Now playing: TP X" strip when playback is inside a TP.
    if let Some(boundary) = sim.current_boundary() {
        let boundary_id = boundary.id;
        let boundary_name = boundary.name.clone();
        let boundary_start = boundary.start_move;
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Now playing:")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(&boundary_name).small().strong());
        });
        if let Some(tp) = load_report
            .per_toolpath
            .iter()
            .find(|tp| tp.toolpath_id == boundary_id.0)
        {
            // Cap context for "% of cap" readout: chipload from the
            // matched LUT row, power from machine cap × safety factor,
            // deflection from the safe L/D threshold (4.0). Each is
            // optional — the badge falls back to a non-numeric display
            // when the cap is missing.
            let sim_trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref());
            let chipload_envelopes =
                rs_cam_core::tool_load::chipload_envelopes_for_session(session, sim_trace);
            let chipload_cap = chipload_envelopes
                .get(&boundary_id.0)
                .map(|range| range.end);
            let machine = session.machine();
            let max_power_kw = match machine.power {
                rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => power_kw,
                rs_cam_core::machine::PowerModel::VfdConstantTorque { rated_power_kw, .. } => {
                    rated_power_kw
                }
            };
            let power_cap_kw = (max_power_kw * machine.safety_factor > 0.0)
                .then_some(max_power_kw * machine.safety_factor);
            let deflection_cap = Some(DEFLECTION_SAFE_LD_RATIO);
            draw_tool_load_badges(ui, tp, chipload_cap, power_cap_kw, deflection_cap);
        }
        ui.horizontal(|ui| {
            if ui.small_button("Optimize").clicked() {
                events.push(AppEvent::OpenOptimizeModal(boundary_id));
            }
            if ui.small_button("Jump to start").clicked() {
                events.push(AppEvent::SimJumpToMove(boundary_start));
            }
        });
    }
}

fn verdict_counts_local(report: &ToolLoadReport) -> (usize, usize, usize, usize) {
    let mut ok = 0;
    let mut warn = 0;
    let mut bad = 0;
    let mut unmodeled = 0;
    for tp in &report.per_toolpath {
        for verdict in [&tp.chipload, &tp.power, &tp.deflection] {
            match verdict {
                Verdict::Within { .. } => ok += 1,
                Verdict::Unmodeled { .. } => unmodeled += 1,
                Verdict::Exceeds { .. } => bad += 1,
            }
            if matches!(
                verdict,
                Verdict::Within {
                    confidence: Confidence::Approximate(_),
                    ..
                } | Verdict::Exceeds {
                    confidence: Confidence::Approximate(_),
                    ..
                }
            ) {
                warn += 1;
            }
        }
    }
    (ok, warn, bad, unmodeled)
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
                if let Some(result) = rt.result.as_ref() {
                    use rs_cam_core::toolpath_spans::SpanKind;
                    let spans = result.spans();
                    let mut counts: std::collections::BTreeMap<&'static str, usize> =
                        std::collections::BTreeMap::new();
                    for s in spans {
                        let key = match s.kind {
                            SpanKind::Operation => "Operation",
                            SpanKind::DepthPass => "DepthPass",
                            SpanKind::Region => "Region",
                            SpanKind::Entry => "Entry",
                            SpanKind::LeadOut => "LeadOut",
                            SpanKind::LinkBridge => "LinkBridge",
                            SpanKind::DressupArtifact => "DressupArtifact",
                            SpanKind::RapidOrderBarrier => "RapidOrderBarrier",
                        };
                        *counts.entry(key).or_insert(0) += 1;
                    }
                    let breakdown = if counts.is_empty() {
                        "none".to_owned()
                    } else {
                        counts
                            .iter()
                            .map(|(k, n)| format!("{k}×{n}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let validity = if result.spans_valid() {
                        "valid"
                    } else {
                        "INVALID"
                    };
                    ui.label(format!(
                        "Structural spans: {} ({validity}) — {breakdown}",
                        spans.len()
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

/// Render a single short summary line in the project summary card showing how
/// Render three independent badges (chipload | power | deflection) for the
/// active toolpath. **Never** combine into a single load %.
fn draw_tool_load_badges(
    ui: &mut egui::Ui,
    verdict: &ToolpathLoadVerdict,
    chipload_cap: Option<f64>,
    power_cap_kw: Option<f64>,
    deflection_cap: Option<f64>,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tool load:")
                .small()
                .color(theme::TEXT_STRONG),
        );
        verdict_badge(ui, "chipload", &verdict.chipload, chipload_cap);
        verdict_badge(ui, "power", &verdict.power, power_cap_kw);
        verdict_badge(ui, "L/D", &verdict.deflection, deflection_cap);
    });
}

/// Default L/D safe threshold for the deflection gate's `% of cap`
/// readout. Above this is the "long tool" warning band; the gate
/// flags `LongToolStiffnessUnsafe` further past it. Matches the
/// constant used by `feeds::calculate` for the L/D feed derate.
const DEFLECTION_SAFE_LD_RATIO: f64 = 4.0;

/// Format the peak as a percentage of the cap. Returns `None` when
/// the cap is unavailable or non-positive — caller falls back to a
/// non-numeric badge.
fn pct_of_cap(peak: f64, cap: Option<f64>) -> Option<i32> {
    let c = cap.filter(|c| *c > 0.0)?;
    let pct = (peak / c * 100.0).round();
    if !pct.is_finite() {
        return None;
    }
    Some(pct as i32)
}

fn verdict_badge(ui: &mut egui::Ui, label: &str, verdict: &Verdict, cap: Option<f64>) {
    // For chipload BurnRisk the peak is *below* the floor, not above the
    // cap — `% of cap` is misleading there. Skip the % branch and fall
    // back to a non-numeric badge.
    let burn_risk = matches!(
        verdict,
        Verdict::Exceeds {
            reason: ExceedsReason::ChiploadBurnRisk,
            ..
        }
    );
    let (color, status) = match verdict {
        Verdict::Within {
            peak,
            confidence: Confidence::Validated,
        } => match pct_of_cap(*peak, cap) {
            Some(pct) => (theme::SUCCESS, format!("{pct}%")),
            None => (theme::SUCCESS, "OK".to_owned()),
        },
        Verdict::Within {
            peak,
            confidence: Confidence::Approximate(_),
        } => match pct_of_cap(*peak, cap) {
            Some(pct) => (theme::WARNING_MILD, format!("{pct}%\u{2248}")),
            None => (theme::WARNING_MILD, "OK\u{2248}".to_owned()),
        },
        Verdict::Exceeds { peak, .. } if !burn_risk => match pct_of_cap(*peak, cap) {
            Some(pct) => (theme::ERROR, format!("{pct}%")),
            None => (theme::ERROR, "FAIL".to_owned()),
        },
        Verdict::Exceeds { .. } => (theme::ERROR, "BURN".to_owned()),
        Verdict::Unmodeled { .. } => (theme::TEXT_DIM, "—".to_owned()),
    };
    let text = format!("{label} {status}");
    ui.label(egui::RichText::new(text).small().color(color))
        .on_hover_text(verdict_tooltip(verdict, cap));
}

fn verdict_tooltip(verdict: &Verdict, cap: Option<f64>) -> String {
    let format_peak = |peak: f64| -> String {
        match cap.filter(|c| *c > 0.0) {
            Some(c) => {
                let pct = (peak / c * 100.0).round() as i32;
                format!("peak {peak:.4} / cap {c:.4} ({pct}%)")
            }
            None => format!("peak {peak:.4}"),
        }
    };
    match verdict {
        Verdict::Within { peak, confidence } => match confidence {
            Confidence::Validated => format!("Within bounds ({}) — validated", format_peak(*peak)),
            Confidence::Approximate(why) => {
                format!("Within bounds ({}) — approximate: {why}", format_peak(*peak))
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
            format!("EXCEEDS: {reason_str} ({}, {conf})", format_peak(*peak))
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
    fn arc_engagement_unmodeled_tooltip_points_to_cut_metrics() {
        let tooltip = verdict_tooltip(
            &Verdict::Unmodeled {
                reason: UnmodeledReason::ArcEngagementNotCaptured,
            },
            None,
        );
        assert!(tooltip.contains("Cut Metrics"));
    }
}
