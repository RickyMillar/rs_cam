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
use rs_cam_core::toolpath_spans::{Span, SpanKind, SpanPayload};

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

    // Default the scope's toolpath to whatever's playing, so the chip row
    // and tree have something useful to show when the user first opens the
    // pane. Explicit user picks (any non-default value) are preserved.
    if sim.debug.span_scope.toolpath_id.is_none()
        && sim.debug.span_scope.span_id.is_none()
        && let Some(boundary_id) = current_boundary_id
    {
        sim.debug.span_scope.toolpath_id = Some(boundary_id);
    }

    draw_scope_chips(ui, sim, gui);
    ui.add_space(2.0);

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

    // --- Selection details ---
    // Slim view of the active semantic item: only data the span tree and
    // chip filter cannot provide — the params grid and geometric bbox.
    // Runtime numbers and start/end jump buttons were removed; runtime is
    // visible in the project overview (scoped via the chip row), and span-
    // navigation lives on the timeline ribbon.
    {
        egui::CollapsingHeader::new("Selection details")
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
                    });
                    ui.label(
                        egui::RichText::new(semantic_kind_label(&active.item.kind))
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
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
                        egui::Grid::new("sim_selection_details_grid")
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
    // Generator-internal phases (preflight / widen_band / agent_search /…)
    // and the semantic-trace item count. Orthogonal to the structural span
    // tree — this is about *how* the toolpath was built, not what's in it.
    {
        egui::CollapsingHeader::new("Generation Metrics")
            .default_open(false)
            .show(ui, |ui| {
                let rt = current_boundary_id.and_then(|tp| gui.toolpath_rt.get(&tp.0));
                let debug_trace = rt.and_then(|r| r.debug_trace.as_ref());
                let semantic_trace = rt.and_then(|r| r.semantic_trace.as_ref());

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
                if let Some(semantic) = semantic_trace {
                    ui.label(
                        egui::RichText::new(format!(
                            "Semantic items: {} (move-linked {})",
                            semantic.summary.item_count, semantic.summary.move_linked_item_count
                        ))
                        .small()
                        .color(theme::TEXT_MUTED),
                    );
                }
            });
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

    let scope = sim.debug.span_scope;
    let scope_active = scope.is_active();

    let (ok, _warn, bad, unmodeled) = verdict_counts_local(load_report);
    let collision_count = sim.checks.rapid_collisions.len() + sim.checks.holder_collision_count;
    let issue_count = sim
        .issues(gui, max_feed)
        .iter()
        .filter(|iss| {
            !scope_active
                || iss
                    .toolpath_id
                    .is_some_and(|tp| Some(tp) == scope.toolpath_id)
        })
        .count();
    let hotspot_count = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(|t| {
            t.hotspots
                .iter()
                .filter(|h| scope.matches(h.toolpath_id, &h.span_path))
                .count()
        })
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
    // here as a single horizontal summary. When the scope chip-row is
    // active, "Issues" and "Hotspots" honor the scope; verdicts and
    // collisions remain project-wide because they're not span-indexed.
    let findings_label = if scope_active {
        "Findings (scoped)"
    } else {
        "Findings"
    };
    ui.label(egui::RichText::new(findings_label).small().strong());
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

    // Tool-load badges + jump buttons for the currently-playing toolpath
    // (project-wide concern, not span-scoped).
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

    // Span tree — shows Operation → DepthPass → Region rows for the
    // toolpath in scope (or the currently-playing one if scope is empty).
    // Per-row metrics are computed from cut samples whose span_path
    // contains that row's SpanId. Clicking a row sets the chip-row scope.
    //
    // Clone the Arc<SimulationCutTrace> so we can pass `&mut sim` into the
    // tree without aliasing the read borrow.
    let tree_toolpath_id = scope
        .toolpath_id
        .or_else(|| sim.current_boundary().map(|b| b.id));
    let trace_arc = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(std::sync::Arc::clone);
    if let (Some(tp_id), Some(trace)) = (tree_toolpath_id, trace_arc.as_ref()) {
        draw_span_tree(ui, sim, gui, tp_id, trace);
    }

    // Scoped findings list — only when a span scope is active, since the
    // unscoped lists are noisy enough that the project-overview just
    // shows the count.
    if scope_active && let Some(trace) = trace_arc.as_ref() {
        draw_scoped_findings(ui, sim, gui, max_feed, trace, events);
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

// ── Span scope: chip filter row ─────────────────────────────────────────

/// Compact label for a Toolpath chip (truncates long names).
fn toolpath_chip_label(name: &str, idx: usize) -> String {
    let trimmed: String = name.chars().take(18).collect();
    if trimmed.chars().count() < name.chars().count() {
        format!("TP{idx}: {trimmed}…")
    } else {
        format!("TP{idx}: {trimmed}")
    }
}

/// Render the scope chip row at the top of the inspector. Reads/writes
/// `sim.debug.span_scope`. The row mirrors the `inspect_spans` MCP filter
/// (toolpath + optional span_id) so an agent and the human see the same
/// scope when they pick the same chips.
fn draw_scope_chips(ui: &mut egui::Ui, sim: &mut SimulationState, gui: &GuiState) {
    let toolpaths: Vec<(ToolpathId, String)> = sim
        .boundaries()
        .iter()
        .map(|b| (b.id, b.name.clone()))
        .collect();
    if toolpaths.is_empty() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Scope:")
                .small()
                .color(theme::TEXT_MUTED),
        );

        // Toolpath chip — required to scope by span_id (span ids are
        // toolpath-relative).
        let current_tp = sim.debug.span_scope.toolpath_id;
        let current_label = current_tp
            .and_then(|tp| {
                toolpaths
                    .iter()
                    .enumerate()
                    .find(|(_, (id, _))| *id == tp)
                    .map(|(idx, (_, name))| toolpath_chip_label(name, idx + 1))
            })
            .unwrap_or_else(|| "All toolpaths".to_owned());
        let mut new_tp = current_tp;
        egui::ComboBox::from_id_salt("scope_chip_toolpath")
            .selected_text(egui::RichText::new(current_label).small())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_tp, None, "— All toolpaths —");
                for (idx, (id, name)) in toolpaths.iter().enumerate() {
                    ui.selectable_value(&mut new_tp, Some(*id), toolpath_chip_label(name, idx + 1));
                }
            });
        if new_tp != current_tp {
            sim.debug.span_scope.toolpath_id = new_tp;
            sim.debug.span_scope.span_id = None;
        }

        // DepthPass + Region chips: only render when the chosen toolpath
        // exposes structural spans. Otherwise we'd be showing two empty
        // dropdowns.
        let spans = sim
            .debug
            .span_scope
            .toolpath_id
            .and_then(|tp| gui.toolpath_rt.get(&tp.0))
            .and_then(|rt| rt.result.as_ref())
            .filter(|r| r.spans_valid())
            .map(|r| r.spans())
            .filter(|s| !s.is_empty());

        if let Some(spans) = spans {
            // Determine the currently-selected DepthPass id from the scope's
            // span_id by walking up to the containing DepthPass span.
            let selected_dp = current_depth_pass_id(spans, sim.debug.span_scope.span_id);
            let dp_label = selected_dp
                .and_then(|sid| {
                    spans
                        .get(sid as usize)
                        .map(|s| depth_pass_chip_label(s, sid as usize))
                })
                .unwrap_or_else(|| "All passes".to_owned());

            let depth_passes: Vec<(u32, &Span)> = spans
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s.kind, SpanKind::DepthPass))
                .map(|(idx, s)| (idx as u32, s))
                .collect();

            if !depth_passes.is_empty() {
                let mut new_dp = selected_dp;
                egui::ComboBox::from_id_salt("scope_chip_depth_pass")
                    .selected_text(egui::RichText::new(dp_label).small())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut new_dp, None, "— All passes —");
                        for (sid, span) in &depth_passes {
                            ui.selectable_value(
                                &mut new_dp,
                                Some(*sid),
                                depth_pass_chip_label(span, *sid as usize),
                            );
                        }
                    });
                if new_dp != selected_dp {
                    sim.debug.span_scope.span_id = new_dp;
                }
            }

            // Region chip — only meaningful when a DepthPass is selected
            // (regions sit inside passes). When no pass is selected we'd
            // show every region across the toolpath, which is not what the
            // chip semantics promise.
            if let Some(dp_sid) = selected_dp
                && let Some(dp_span) = spans.get(dp_sid as usize)
            {
                let regions: Vec<(u32, &Span)> = spans
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        matches!(s.kind, SpanKind::Region)
                            && s.start_move >= dp_span.start_move
                            && s.end_move <= dp_span.end_move
                    })
                    .map(|(idx, s)| (idx as u32, s))
                    .collect();
                if !regions.is_empty() {
                    let region_label = if sim.debug.span_scope.span_id == Some(dp_sid) {
                        "All regions".to_owned()
                    } else {
                        sim.debug
                            .span_scope
                            .span_id
                            .and_then(|sid| {
                                spans
                                    .get(sid as usize)
                                    .filter(|s| matches!(s.kind, SpanKind::Region))
                                    .map(|s| region_chip_label(s, sid as usize))
                            })
                            .unwrap_or_else(|| "All regions".to_owned())
                    };
                    let mut new_region = if sim.debug.span_scope.span_id == Some(dp_sid) {
                        None
                    } else {
                        sim.debug.span_scope.span_id
                    };
                    egui::ComboBox::from_id_salt("scope_chip_region")
                        .selected_text(egui::RichText::new(region_label).small())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut new_region, None, "— All regions —");
                            for (sid, span) in &regions {
                                ui.selectable_value(
                                    &mut new_region,
                                    Some(*sid),
                                    region_chip_label(span, *sid as usize),
                                );
                            }
                        });
                    let target = new_region.or(Some(dp_sid));
                    if target != sim.debug.span_scope.span_id {
                        sim.debug.span_scope.span_id = target;
                    }
                }
            }
        }

        if sim.debug.span_scope.is_active()
            && ui
                .small_button("✕")
                .on_hover_text("Clear scope filter")
                .clicked()
        {
            sim.debug.span_scope.clear();
        }
    });
}

/// Walk up from a span id to find the DepthPass span that contains it (or
/// is it). Returns `None` when the input is `None`, falls outside the
/// vec, or when no DepthPass ancestor exists.
fn current_depth_pass_id(spans: &[Span], span_id: Option<u32>) -> Option<u32> {
    let sid = span_id?;
    let target = spans.get(sid as usize)?;
    if matches!(target.kind, SpanKind::DepthPass) {
        return Some(sid);
    }
    spans
        .iter()
        .enumerate()
        .find(|(_, s)| {
            matches!(s.kind, SpanKind::DepthPass)
                && s.start_move <= target.start_move
                && s.end_move >= target.end_move
        })
        .map(|(idx, _)| idx as u32)
}

fn depth_pass_chip_label(span: &Span, sid: usize) -> String {
    match &span.payload {
        Some(SpanPayload::DepthPass {
            pass_index,
            z_level,
        }) => format!("DP{pass_index}: z={z_level:.2}"),
        _ => format!("DepthPass[{sid}]"),
    }
}

fn region_chip_label(span: &Span, sid: usize) -> String {
    match &span.payload {
        Some(SpanPayload::Region { region_id }) => format!("Region {region_id}"),
        _ => format!("Region[{sid}]"),
    }
}

// ── Span tree (replaces per-SpanKind breakdown table) ───────────────────

/// Bucket of cutting samples for one span. Updated incrementally while
/// scanning the trace's samples.
#[derive(Default, Clone, Copy)]
struct SpanBucket {
    n: usize,
    sum_eng: f64,
    sum_chip: f64,
    peak_chip: f64,
}

impl SpanBucket {
    fn ingest(&mut self, eng: f64, chip: f64) {
        self.n += 1;
        self.sum_eng += eng;
        self.sum_chip += chip;
        if chip > self.peak_chip {
            self.peak_chip = chip;
        }
    }
}

/// Render the Operation → DepthPass → Region tree for `toolpath_id`.
/// Each row shows n samples, avg engagement, and avg/peak chipload from
/// cutting samples whose `span_path` contains that row's SpanId. Clicking
/// a row sets `sim.debug.span_scope.span_id` so the chip row + findings
/// list update together.
fn draw_span_tree(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    toolpath_id: ToolpathId,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
) {
    let Some(rt) = gui.toolpath_rt.get(&toolpath_id.0) else {
        return;
    };
    let Some(result) = rt.result.as_ref() else {
        return;
    };
    if !result.spans_valid() || result.spans().is_empty() {
        return;
    }
    let spans = result.spans();

    // Pre-aggregate per-SpanId. One scan over samples; every span the
    // sample's path contains gets the increment.
    let mut buckets: Vec<SpanBucket> = vec![SpanBucket::default(); spans.len()];
    for s in &trace.samples {
        if s.toolpath_id != toolpath_id.0 || !s.is_cutting {
            continue;
        }
        let chip = s
            .effective_chip_thickness_mm
            .unwrap_or(s.chipload_mm_per_tooth);
        for sid in &s.span_path {
            if let Some(b) = buckets.get_mut(sid.0 as usize) {
                b.ingest(s.radial_engagement, chip);
            }
        }
    }

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Span breakdown")
            .small()
            .color(theme::TEXT_MUTED),
    )
    .on_hover_text(
        "Operation → DepthPass → Region. Click a row to scope the chip filter to that span.",
    );

    let scope_span_id = sim.debug.span_scope.span_id;
    let mut click_target: Option<Option<u32>> = None;

    for (op_id, op_span) in spans
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s.kind, SpanKind::Operation))
    {
        let op_id_u32 = op_id as u32;
        // SAFETY: `buckets` is sized exactly `spans.len()` and `op_id`
        // came from enumerating `spans`, so the index is in-bounds.
        let op_bucket = buckets.get(op_id).copied().unwrap_or_default();
        if span_tree_row(
            ui,
            0,
            "Operation",
            op_span,
            op_id,
            op_bucket,
            scope_span_id == Some(op_id_u32),
        )
        .clicked()
        {
            click_target = Some(if scope_span_id == Some(op_id_u32) {
                None
            } else {
                Some(op_id_u32)
            });
        }

        // DepthPass children (one tier in)
        for (dp_id, dp_span) in spans.iter().enumerate().filter(|(_, s)| {
            matches!(s.kind, SpanKind::DepthPass)
                && s.start_move >= op_span.start_move
                && s.end_move <= op_span.end_move
        }) {
            let dp_id_u32 = dp_id as u32;
            let dp_bucket = buckets.get(dp_id).copied().unwrap_or_default();
            let dp_label = depth_pass_chip_label(dp_span, dp_id);
            if span_tree_row(
                ui,
                1,
                &dp_label,
                dp_span,
                dp_id,
                dp_bucket,
                scope_span_id == Some(dp_id_u32),
            )
            .clicked()
            {
                click_target = Some(if scope_span_id == Some(dp_id_u32) {
                    None
                } else {
                    Some(dp_id_u32)
                });
            }

            // Region grandchildren (only when this DepthPass is selected — keeps
            // the tree compact; otherwise a 100-region op buries the whole pane).
            if scope_span_id == Some(dp_id_u32) {
                for (r_id, r_span) in spans.iter().enumerate().filter(|(_, s)| {
                    matches!(
                        s.kind,
                        SpanKind::Region | SpanKind::Entry | SpanKind::LeadOut
                    ) && s.start_move >= dp_span.start_move
                        && s.end_move <= dp_span.end_move
                }) {
                    let r_id_u32 = r_id as u32;
                    let r_bucket = buckets.get(r_id).copied().unwrap_or_default();
                    let r_label = match r_span.kind {
                        SpanKind::Region => region_chip_label(r_span, r_id),
                        SpanKind::Entry => format!("Entry[{r_id}]"),
                        SpanKind::LeadOut => format!("LeadOut[{r_id}]"),
                        _ => format!("[{r_id}]"),
                    };
                    if span_tree_row(
                        ui,
                        2,
                        &r_label,
                        r_span,
                        r_id,
                        r_bucket,
                        scope_span_id == Some(r_id_u32),
                    )
                    .clicked()
                    {
                        click_target = Some(if scope_span_id == Some(r_id_u32) {
                            // Toggling a region off goes back to its parent
                            // DepthPass, not the whole toolpath.
                            Some(dp_id_u32)
                        } else {
                            Some(r_id_u32)
                        });
                    }
                }
            }
        }
    }

    if let Some(target) = click_target {
        sim.debug.span_scope.toolpath_id = Some(toolpath_id);
        sim.debug.span_scope.span_id = target;
    }
}

/// Render one indented tree row. Returns the row's Response so the caller
/// can detect clicks. Keeps text small/monospace; selected row gets the
/// info accent so it stands out without a separate header style.
fn span_tree_row(
    ui: &mut egui::Ui,
    depth: u8,
    label: &str,
    span: &Span,
    sid: usize,
    bucket: SpanBucket,
    selected: bool,
) -> egui::Response {
    let indent = depth as f32 * 12.0;
    let row_color = if selected {
        theme::INFO
    } else {
        theme::TEXT_HEADING
    };
    let metric_color = if selected {
        theme::INFO
    } else {
        theme::TEXT_MUTED
    };
    let metrics = if bucket.n > 0 {
        let avg_eng = bucket.sum_eng / bucket.n as f64;
        let avg_chip = bucket.sum_chip / bucket.n as f64;
        format!(
            "n={:<5} eng={avg_eng:.2}  chip={avg_chip:.4}/{:.4}",
            bucket.n, bucket.peak_chip
        )
    } else {
        format!("n=0    moves {}–{}", span.start_move, span.end_move)
    };
    let label_with_id = format!("{label} [{sid}]");
    ui.horizontal(|ui| {
        ui.add_space(indent);
        let label_resp = ui.add(
            egui::Label::new(egui::RichText::new(label_with_id).small().color(row_color))
                .sense(egui::Sense::click()),
        );
        ui.add(egui::Label::new(
            egui::RichText::new(metrics)
                .small()
                .monospace()
                .color(metric_color),
        ));
        label_resp
    })
    .inner
}

// ── Scoped findings list ────────────────────────────────────────────────

/// Inline list of in-scope hotspots and issues, shown beneath the project
/// overview when the chip-row scope is active. Click a row to focus
/// (sets `focused_hotspot` / `focused_issue_index`) and jump to its move.
fn draw_scoped_findings(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    events: &mut Vec<AppEvent>,
) {
    let scope = sim.debug.span_scope;
    let in_scope_hotspots: Vec<(usize, &rs_cam_core::simulation_cut::SimulationCutHotspot)> = trace
        .hotspots
        .iter()
        .enumerate()
        .filter(|(_, h)| scope.matches(h.toolpath_id, &h.span_path))
        .collect();
    let issues = sim.issues(gui, max_feed);
    let in_scope_issues: Vec<&_> = issues
        .iter()
        .filter(|iss| {
            iss.toolpath_id
                .is_some_and(|tp| Some(tp) == scope.toolpath_id)
        })
        .collect();

    if in_scope_hotspots.is_empty() && in_scope_issues.is_empty() {
        return;
    }

    ui.add_space(6.0);
    ui.separator();
    ui.label(
        egui::RichText::new("In-scope findings")
            .small()
            .strong()
            .color(theme::TEXT_HEADING),
    );

    const MAX_ROWS: usize = 8;
    for (h_idx, h) in in_scope_hotspots.iter().take(MAX_ROWS) {
        let toolpath_id = ToolpathId(h.toolpath_id);
        let global_start = sim
            .global_move_for_local(toolpath_id, h.move_start)
            .unwrap_or(h.move_start);
        let resp = ui
            .selectable_label(
                false,
                egui::RichText::new(format!(
                    "Hotspot · m{} · waste {:.2}s · peak chip {:.4}",
                    h.move_start, h.wasted_runtime_s, h.peak_chipload_mm_per_tooth
                ))
                .small()
                .color(egui::Color32::from_rgb(255, 170, 90)),
            )
            .on_hover_text("Click to focus and jump to start move.");
        if resp.clicked() {
            sim.debug.focused_hotspot = Some((toolpath_id, *h_idx));
            events.push(AppEvent::SimJumpToMove(global_start));
        }
    }
    if in_scope_hotspots.len() > MAX_ROWS {
        ui.label(
            egui::RichText::new(format!(
                "… +{} more hotspots",
                in_scope_hotspots.len() - MAX_ROWS
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
    }

    for iss in in_scope_issues.iter().take(MAX_ROWS) {
        let move_idx = iss.move_index;
        let label = format!(
            "{} · m{} · {}",
            issue_kind_label(iss.kind),
            move_idx,
            iss.label
        );
        let resp = ui
            .selectable_label(
                false,
                egui::RichText::new(label)
                    .small()
                    .color(theme::WARNING_TEXT),
            )
            .on_hover_text("Click to jump to issue.");
        if resp.clicked() {
            events.push(AppEvent::SimJumpToMove(move_idx));
        }
    }
    if in_scope_issues.len() > MAX_ROWS {
        ui.label(
            egui::RichText::new(format!(
                "… +{} more issues",
                in_scope_issues.len() - MAX_ROWS
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
    }
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
