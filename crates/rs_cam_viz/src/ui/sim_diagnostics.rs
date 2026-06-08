use super::AppEvent;
use super::components::{CountPill, FreshnessGate};
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::runtime::GuiState;
use crate::state::simulation::{SimulationIssueKind, SimulationState, StockVizMode};
use crate::state::toolpath::ToolpathId;
use crate::ui::theme;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::drill_gates::{DrillGateOutcome, DrillGateSeverity};
use rs_cam_core::tool_load::verdict::{
    ChipSide, ChiploadVerdict, CriterionKind, CriterionStatus, LoadState,
};
use rs_cam_core::tool_load::{Confidence, ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason};
use rs_cam_core::toolpath_spans::{Span, SpanKind, SpanPayload};

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

    let load_report = sim.cached_load_report(session, gui.edit_counter);

    // Fixed status header (pass2 inspector §2.1) — verdict + freshness, drawn
    // before the card dispatch so the stale signal is reachable even when a
    // focused card replaces the scope sections (INS-001/005).
    draw_status_header(ui, sim, gui);

    // Scope-tiered body (§2.2/2.3). A focused hotspot/issue card is a
    // mutually-exclusive drill-in overlay (early-return); otherwise the three
    // concern-scoped sections — Project / Toolpath / Span — render in fixed
    // order, each summary-first behind its own disclosure.
    if sim.results.is_none() {
        ui.label(
            egui::RichText::new("Run simulation to see the cut overview here.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
    } else if draw_focused_hotspot_card(ui, sim, events).is_some() {
        // Focused hotspot card shown — scope sections suppressed.
    } else if draw_focused_issue_card(ui, sim, gui, max_feed, events).is_some() {
        // Focused issue card shown — scope sections suppressed.
    } else {
        // Refresh the per-span aggregate cache once up front (cheap Arc
        // pointer compare when unchanged) and compute the issue list once —
        // both the Project and Span sections read it.
        if let Some(trace_arc) = sim.results.as_ref().and_then(|r| r.cut_trace.as_ref()) {
            let trace_arc = std::sync::Arc::clone(trace_arc);
            sim.debug.span_aggregates.ensure_built(&trace_arc);
        }
        let issues = sim.issues(gui, max_feed);

        draw_project_section(ui, sim, session, gui, &issues, &load_report, events);
        ui.add_space(6.0);
        draw_toolpath_section(ui, sim, session, gui, &load_report, events);

        let trace_arc = sim
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .map(std::sync::Arc::clone);
        if let Some(trace) = trace_arc.as_ref() {
            ui.add_space(6.0);
            draw_span_section(ui, sim, gui, max_feed, trace, &issues, events);
        }
    }
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
            // Stock appearance — opacity only. The show/hide *toggle* lives in
            // the viewport "Show ▼" menu (W4.3: one home for visibility); this
            // panel keeps stock *appearance* (opacity + the colour modes under
            // "Analysis" below). Same backing field, two labels, was P4-005.
            ui.label(
                egui::RichText::new("Stock")
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                ui.add(egui::Slider::new(&mut sim.stock_opacity, 0.0..=1.0).show_value(true));
            });

            ui.add_space(8.0);

            // Toolpath visibility moved entirely to the viewport "Show ▼" menu
            // (global) and each row's C / R buttons (per-toolpath). Duplicating
            // the global checkboxes here under a second set of labels was the
            // P4-005 confusable; the pointer keeps them discoverable.
            ui.label(
                egui::RichText::new("Toolpaths")
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            );
            ui.label(
                egui::RichText::new(
                    "Cutting / rapid visibility: viewport \u{201C}Show \u{25BE}\u{201D} menu. \
                     Per-toolpath: each row\u{2019}s C / R.",
                )
                .small()
                .color(theme::TEXT_FAINT),
            );

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
                // P5-004: the mode selector that needs deviation data is right
                // here, so the re-run affordance is too — no off-surface hunt
                // for the Run button.
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("No deviation data \u{2014}")
                            .small()
                            .color(theme::WARNING),
                    );
                    if ui
                        .small_button("Re-run simulation")
                        .on_hover_text("Re-run the simulation to compute surface deviation.")
                        .clicked()
                    {
                        events.push(AppEvent::RunSimulation);
                    }
                });
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
}

/// Fixed status header (pass2 inspector §2.1) — the always-visible glance: a
/// one-line verdict banner (collision / air-cut / OK) plus a freshness chip.
/// Drawn before the card dispatch so the stale signal covers the focused-card
/// paths too (INS-001/005). No-op when no simulation has run.
fn draw_status_header(ui: &mut egui::Ui, sim: &SimulationState, gui: &GuiState) {
    if sim.results.is_none() {
        return;
    }
    let collision_count = sim.checks.total_collision_count();
    // Roadmap C.6 — verdict mirrors the rule the MCP `run_simulation` response
    // uses: collisions → ERROR; air cut > 20% → WARNING; otherwise SUCCESS.
    let air_cut_pct = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(|ct| {
            let s = &ct.summary;
            if s.total_runtime_s > 0.0 {
                s.air_cut_time_s / s.total_runtime_s * 100.0
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);
    let (banner_text, banner_color) = if collision_count > 0 {
        (
            format!(
                "⚠ {collision_count} collision{} — review before export",
                if collision_count == 1 { "" } else { "s" }
            ),
            theme::ERROR,
        )
    } else if air_cut_pct > 20.0 {
        (
            format!(
                "⚠ High air cutting ({air_cut_pct:.0}%) — toolpath may be sweeping over uncut stock"
            ),
            theme::WARNING,
        )
    } else {
        (
            "✓ No collisions, air cutting under threshold".to_owned(),
            theme::SUCCESS,
        )
    };
    ui.label(
        egui::RichText::new(banner_text)
            .color(banner_color)
            .strong(),
    );
    // Freshness chip — rendered here (not in the overview body) so it covers
    // the focused-card paths too (INS-005).
    if sim.is_stale(gui.edit_counter) {
        FreshnessGate::banner(ui);
    } else {
        ui.label(
            egui::RichText::new("\u{2713} live")
                .small()
                .color(theme::TEXT_DIM),
        );
    }
    ui.add_space(4.0);
    ui.separator();
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

    // Distinct shape (pass2 inspector §2.3 / INS-003): the hotspot card is
    // FILLED with a solid orange border and a filled ◍ glyph (a "time-waste"
    // focus), structurally unlike the issue card's hollow outline.
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(50, 38, 28))
        .stroke(egui::Stroke::new(
            1.5,
            egui::Color32::from_rgb(255, 170, 90),
        ))
        .inner_margin(6.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{25CD} Hotspot")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 170, 90)),
                );
                ui.label(
                    egui::RichText::new(format!("TP {}", tp_id + 1))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            });
            // Lead line: the canonical hotspot summary (INS-004), identical to
            // the Top-hotspots and Span-findings rows. Detail lines follow.
            ui.label(
                egui::RichText::new(hotspot_summary_line(move_start, wasted, peak_chip))
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new(format!(
                    "moves {move_start}–{move_end} · {sample_count} samples · peak DOC {peak_doc:.2}"
                ))
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new(format!("avg engage {:.0}%", avg_eng * 100.0))
                    .small()
                    .color(theme::TEXT_MUTED),
            )
            .on_hover_text(ENGAGEMENT_PROVENANCE_HOVER);
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
                if ui.small_button("Optimize this op").clicked() {
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
    // Distinct shape (pass2 inspector §2.3 / INS-003): the issue card is a
    // HOLLOW outline (no fill) with an outline △ glyph and Prev/Next nav —
    // visually unlike the filled hotspot card even though they share the slot.
    egui::Frame::default()
        .stroke(egui::Stroke::new(
            1.5,
            egui::Color32::from_rgb(210, 170, 80),
        ))
        .inner_margin(6.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "\u{25B3} {}: {}",
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
                    && ui.small_button("Optimize this op").clicked()
                {
                    events.push(AppEvent::OpenOptimizeModal(toolpath_id));
                }
            });
        });
    Some(())
}

/// Project scope (pass2 inspector §2.3-A) — "is the whole run good?". A
/// summary-first CollapsingHeader: the header line carries cycle time + the
/// within/exceeding glance; the body holds the Global grid, the CountPill
/// Findings rollup, the issue-kind partition, and the nested Top-hotspots
/// triage list. The verdict banner + freshness moved up to the fixed status
/// header (§2.1); the now-playing strip and Selected span are their own scope
/// sections now.
#[allow(clippy::too_many_arguments)]
fn draw_project_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    issues: &[crate::state::simulation::SimulationIssue],
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    let (total_cutting, total_rapid, total_time_min) = aggregate_stats(sim, session, gui);
    let total_min = total_time_min.floor() as u32;
    let total_sec = ((total_time_min - total_min as f64) * 60.0) as u32;

    // Roadmap C.2/F.11 — ToolLoadReport::summary() gives toolpath-counted
    // denominators (and operator-readable exceed labels), the same producer
    // the verdict HUD reads so the two rollups cannot diverge.
    let summary = load_report.summary(|id| {
        session
            .toolpath_configs()
            .iter()
            .find(|tc| tc.id == id)
            .map(|tc| tc.name.clone())
    });
    let (ok, bad, unmodeled, total_tp) = (
        summary.within,
        summary.exceeds,
        summary.fully_unmodeled,
        summary.total_toolpaths,
    );
    let collision_count = sim.checks.total_collision_count();

    // Summary-first header line: cycle + the within/exceeding glance.
    let header = format!(
        "Project — {total_min}:{total_sec:02} · \u{2713}{ok} within · \u{2715}{bad} exceeding"
    );
    egui::CollapsingHeader::new(header)
        .id_salt("inspector_project")
        .default_open(true)
        .show(ui, |ui| {
            // ─── Global stats ─── (cycle now lives in the header line)
            ui.label(
                egui::RichText::new("Global")
                    .strong()
                    .color(theme::TEXT_HEADING),
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

            // Project-wide findings counts — the same `CountPill` grammar + `/T`
            // denominator the verdict HUD uses, reading the same `summary()` producer
            // so the two rollups cannot diverge (W3.3 carry-over, P4-001/002). Load
            // buckets are verdicts (`[ … ]`); collisions is an observation (`{ … }`).
            // The exceeding pill is actionable (`( … → )`) when any TP exceeds — it
            // jumps straight to the project-level Optimize, replacing the separate
            // ⚡ Optimize-all button (FINAL_DESIGN §5.1).
            ui.label(egui::RichText::new("Findings").small().strong());
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    CountPill::verdict("\u{2713} within", ok)
                        .denom(total_tp)
                        .color(egui::Color32::from_rgb(120, 200, 130))
                        .hover("Toolpaths within modeled load limits (of total modeled)."),
                );
                let exceeds_pill = CountPill::verdict("\u{2715} exceeding", bad)
            .denom(total_tp)
            .color(egui::Color32::from_rgb(220, 90, 90))
            .hover("Toolpaths exceeding a modeled load limit. Click to optimize all exceeding.");
                if bad > 0 {
                    if ui.add(exceeds_pill.actionable()).clicked() {
                        events.push(AppEvent::OpenOptimizeProject);
                    }
                } else {
                    ui.add(exceeds_pill);
                }
                ui.add(
            CountPill::verdict("\u{26A0} unmodeled", unmodeled)
                .denom(total_tp)
                .color(egui::Color32::from_rgb(210, 170, 80))
                .hover("Toolpaths the gate could not model (drill cycles, no vendor data, etc.)."),
        );
                let collision_color = if collision_count == 0 {
                    theme::SUCCESS
                } else {
                    theme::ERROR
                };
                ui.add(
                    CountPill::observation("collisions", collision_count)
                        .color(collision_color)
                        .hover("Rapid/holder collisions detected during simulation."),
                );
            });

            // OPT-006 — discoverable in-context Optimize entry. When any
            // toolpath exceeds, the actionable exceeding-pill above is the
            // entry (W3.3). When nothing is over-budget the pill is inert,
            // yet the optimizer can still cut cycle time on within-gate
            // toolpaths — so surface a muted always-available entry here so
            // the obscure Toolpath-menu item is no longer the only path.
            if bad == 0 {
                ui.add_space(2.0);
                if ui
                    .button(
                        egui::RichText::new("\u{26A1} Optimize project for cycle time")
                            .small()
                            .color(theme::TEXT_MUTED),
                    )
                    .on_hover_text(
                        "Search every enabled toolpath for a faster, still-safe set of \
                         feeds & speeds.",
                    )
                    .clicked()
                {
                    events.push(AppEvent::OpenOptimizeProject);
                }
            }

            // Roadmap C.1 — partition the issue count by SimulationIssueKind into
            // a "must address" cluster (collisions, hotspots) and an
            // informational cluster (low engagement, air cut). The single
            // `issue_count` row hid 24 800 air-cut "issues" alongside 14
            // hotspots, which makes the project look broken when most of the
            // count is emission noise.
            let mut must_address: Vec<(SimulationIssueKind, usize)> = Vec::new();
            let mut informational: Vec<(SimulationIssueKind, usize)> = Vec::new();
            let kinds_must = [
                SimulationIssueKind::RapidCollision,
                SimulationIssueKind::HolderCollision,
                SimulationIssueKind::Hotspot,
            ];
            let kinds_info = [
                SimulationIssueKind::LowEngagement,
                SimulationIssueKind::AirCut,
            ];
            for kind in kinds_must {
                let count = issues.iter().filter(|i| i.kind == kind).count();
                if count > 0 {
                    must_address.push((kind, count));
                }
            }
            for kind in kinds_info {
                let count = issues.iter().filter(|i| i.kind == kind).count();
                informational.push((kind, count));
            }
            if !must_address.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Must address")
                        .small()
                        .strong()
                        .color(theme::ERROR),
                );
                egui::Grid::new("cut_overview_must_address")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        for (kind, count) in &must_address {
                            ui.label(
                                egui::RichText::new(issue_kind_label(*kind))
                                    .small()
                                    .color(theme::ERROR),
                            );
                            ui.label(egui::RichText::new(format!("{count}")).small());
                            ui.end_row();
                        }
                    });
            }
            if informational.iter().any(|(_, c)| *c > 0) {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Informational")
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                egui::Grid::new("cut_overview_informational")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        for (kind, count) in &informational {
                            ui.label(
                                egui::RichText::new(issue_kind_label(*kind))
                                    .small()
                                    .color(theme::TEXT_MUTED),
                            );
                            ui.label(egui::RichText::new(format!("{count}")).small());
                            ui.end_row();
                        }
                    });
            }
            // Roadmap C.4 — project-wide hotspot triage list. Source:
            // `cut_trace.hotspots`, sorted by `wasted_runtime_s` desc. The
            // single-card `draw_focused_hotspot_card` only shows one hotspot at
            // a time; without this list a user has no glanceable triage of
            // "where is the tool wasting time?" at the project level.
            //
            // Snapshot the (idx, toolpath_id, move_start, wasted, peak) tuples
            // so we can drop the trace borrow before re-borrowing sim mutably
            // inside the click handler.
            let hotspot_snapshot: Vec<(usize, usize, usize, f64, f64)> = sim
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref())
                .map(|trace| {
                    let mut v: Vec<(usize, usize, usize, f64, f64)> = trace
                        .hotspots
                        .iter()
                        .enumerate()
                        .map(|(idx, h)| {
                            (
                                idx,
                                h.toolpath_id,
                                h.move_start,
                                h.wasted_runtime_s,
                                h.peak_chipload_mm_per_tooth,
                            )
                        })
                        .collect();
                    v.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
                    v
                })
                .unwrap_or_default();
            if !hotspot_snapshot.is_empty() {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(format!("Top hotspots ({})", hotspot_snapshot.len()))
                    .default_open(false)
                    .show(ui, |ui| {
                        const TOP_N: usize = 10;
                        for (h_idx, tp_id_raw, move_start, wasted, peak) in
                            hotspot_snapshot.iter().take(TOP_N)
                        {
                            let tp_id = ToolpathId(*tp_id_raw);
                            let global_start = sim
                                .global_move_for_local(tp_id, *move_start)
                                .unwrap_or(*move_start);
                            let label = hotspot_summary_line(*move_start, *wasted, *peak);
                            let resp = ui
                                .selectable_label(
                                    false,
                                    egui::RichText::new(label)
                                        .small()
                                        .color(egui::Color32::from_rgb(255, 170, 90)),
                                )
                                .on_hover_text("Click to focus and jump to this hotspot.");
                            if resp.clicked() {
                                sim.debug.focused_hotspot = Some((tp_id, *h_idx));
                                events.push(AppEvent::SimJumpToMove(global_start));
                            }
                        }
                        if hotspot_snapshot.len() > TOP_N {
                            ui.label(
                                egui::RichText::new(format!(
                                    "… +{} more (open Selected for span-scoped list)",
                                    hotspot_snapshot.len() - TOP_N
                                ))
                                .small()
                                .color(theme::TEXT_DIM),
                            );
                        }
                    });
            }
        });
}

/// Toolpath scope (pass2 inspector §2.3-B) — "what is the op under the
/// playhead doing?". The header names the playing op (or "—" when idle); the
/// body carries the tool-load badges + the per-op Optimize / Jump buttons.
fn draw_toolpath_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    let now = sim
        .current_boundary()
        .map(|boundary| (boundary.id, boundary.name.clone(), boundary.start_move));
    let title = match &now {
        Some((_, name, _)) => format!("Now playing: {name}"),
        None => "Now playing: \u{2014}".to_owned(),
    };
    egui::CollapsingHeader::new(title)
        .id_salt("inspector_toolpath")
        .default_open(now.is_some())
        .show(ui, |ui| {
            let Some((boundary_id, _, boundary_start)) = now else {
                ui.label(
                    egui::RichText::new("Scrub or play to see the active op.")
                        .small()
                        .italics()
                        .color(theme::TEXT_DIM),
                );
                return;
            };
            if let Some(tp) = load_report
                .per_toolpath
                .iter()
                .find(|tp| tp.toolpath_id == boundary_id.0)
            {
                let chipload_envelopes = sim.cached_chipload_envelopes(session, gui.edit_counter);
                let chipload_cap = chipload_envelopes
                    .get(&boundary_id.0)
                    .map(|range| range.end);
                let machine = session.machine();
                let max_power_kw = match machine.power {
                    rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => power_kw,
                    rs_cam_core::machine::PowerModel::VfdConstantTorque {
                        rated_power_kw, ..
                    } => rated_power_kw,
                };
                let power_cap_kw = (max_power_kw * machine.safety_factor > 0.0)
                    .then_some(max_power_kw * machine.safety_factor);
                let deflection_cap = Some(DEFLECTION_SAFE_LD_RATIO);
                draw_tool_load_badges(ui, tp, chipload_cap, power_cap_kw, deflection_cap);
            }
            ui.horizontal(|ui| {
                if ui.small_button("Optimize this op").clicked() {
                    events.push(AppEvent::OpenOptimizeModal(boundary_id));
                }
                if ui.small_button("Jump to start").clicked() {
                    events.push(AppEvent::SimJumpToMove(boundary_start));
                }
            });
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

/// Canonical one-line hotspot summary (INS-004). The same datum used to print
/// three different ways (focused card, Top-hotspots list, Span findings row);
/// every site now leads with this identical line + units so the focused card's
/// first line matches the list rows exactly.
fn hotspot_summary_line(move_start: usize, wasted_runtime_s: f64, peak_chip: f64) -> String {
    format!("m{move_start} · waste {wasted_runtime_s:.2}s · peak chip {peak_chip:.4} mm")
}

/// Provenance caveat shown as an `on_hover_text` on every engagement readout
/// (INS-006) — engagement is comparative, not an absolute under-engagement bar.
const ENGAGEMENT_PROVENANCE_HOVER: &str = "Engagement = cylinder-side radial width-of-cut fraction. Reads ~10× below the \
     algorithmic target; use it to compare variants, not as an absolute under-engagement bar.";

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
    if let Some(drill_gates) = &verdict.drill_gates {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Drill gates:")
                    .small()
                    .color(theme::TEXT_STRONG),
            );
            drill_gate_badge(ui, "chip weld", &drill_gates.chip_welding);
            drill_gate_badge(ui, "peck", &drill_gates.peck_adequacy);
            drill_gate_badge(ui, "plunge", &drill_gates.plunge_feed);
        });
        return;
    }

    let burn_risk = matches!(
        verdict.chipload,
        ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            ..
        }
    );
    // Roadmap C.3 — for BurnRisk (chipload-low), the relevant bound is
    // the LUT *floor*, not the cap. Without this, the BURN tooltip
    // talked about "peak / cap" which made the user think they were
    // *exceeding* the high bound when they were actually under-feeding.
    let chipload_bound = if burn_risk
        && let ChiploadVerdict::Exceeds {
            triggering: ref m, ..
        } = verdict.chipload
    {
        m.bounds.min_mm_per_tooth.or(chipload_cap)
    } else {
        chipload_cap
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tool load:")
                .small()
                .color(theme::TEXT_STRONG),
        );
        verdict_badge(
            ui,
            "chipload",
            &verdict.chipload.as_criterion_status(),
            chipload_bound,
            burn_risk,
        );
        verdict_badge(
            ui,
            "power",
            &verdict.power.as_criterion_status(),
            power_cap_kw,
            false,
        );
        verdict_badge(
            ui,
            "L/D",
            &verdict.deflection.as_criterion_status(),
            deflection_cap,
            false,
        );
    });
}

fn drill_gate_badge(ui: &mut egui::Ui, label: &str, outcome: &DrillGateOutcome) {
    let (color, text, hover) = match outcome {
        DrillGateOutcome::Within {
            observed,
            threshold,
        } => (
            theme::SUCCESS,
            "OK".to_owned(),
            format!("Within drill gate: observed {observed:.3}, threshold {threshold:.3}"),
        ),
        DrillGateOutcome::Exceeds {
            observed,
            threshold,
            severity,
        } => {
            let sev = match severity {
                DrillGateSeverity::Elevated => "elevated",
                DrillGateSeverity::Critical => "critical",
            };
            (
                theme::ERROR,
                sev.to_owned(),
                format!("Drill gate exceeds: observed {observed:.3}, threshold {threshold:.3}"),
            )
        }
    };
    ui.label(
        egui::RichText::new(format!("{label} {text}"))
            .small()
            .color(color),
    )
    .on_hover_text(hover);
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

fn verdict_badge(
    ui: &mut egui::Ui,
    label: &str,
    status: &CriterionStatus<'_>,
    cap: Option<f64>,
    burn_risk: bool,
) {
    // For chipload BurnRisk the peak is *below* the floor, not above the
    // cap — `% of cap` is misleading there. Skip the % branch and fall
    // back to a non-numeric badge.
    let peak = status.display_peak.unwrap_or(0.0);
    let (color, status_text) = match (status.state, status.confidence) {
        (LoadState::Within, Some(Confidence::Validated)) | (LoadState::Within, None) => {
            match pct_of_cap(peak, cap) {
                Some(pct) => (theme::SUCCESS, format!("{pct}%")),
                None => (theme::SUCCESS, "OK".to_owned()),
            }
        }
        (LoadState::Within, Some(Confidence::Approximate(_))) => match pct_of_cap(peak, cap) {
            Some(pct) => (theme::WARNING_MILD, format!("{pct}%\u{2248}")),
            None => (theme::WARNING_MILD, "OK\u{2248}".to_owned()),
        },
        (LoadState::Exceeds, _) if burn_risk => (theme::ERROR, "BURN".to_owned()),
        (LoadState::Exceeds, _) => match pct_of_cap(peak, cap) {
            Some(pct) => (theme::ERROR, format!("{pct}%")),
            None => (theme::ERROR, "FAIL".to_owned()),
        },
        (LoadState::Unmodeled, _) => (theme::TEXT_DIM, "—".to_owned()),
    };
    let text = format!("{label} {status_text}");
    ui.label(egui::RichText::new(text).small().color(color))
        .on_hover_text(verdict_tooltip(status, cap, burn_risk));
}

fn verdict_tooltip(status: &CriterionStatus<'_>, cap: Option<f64>, burn_risk: bool) -> String {
    // For burn-risk chipload, the bound is the LUT *floor* — render as
    // "peak / floor" instead of "peak / cap" so the relationship reads
    // correctly (peak < floor, not peak > cap). (Roadmap C.3)
    let bound_label = if burn_risk { "floor" } else { "cap" };
    let format_peak = |peak: f64| -> String {
        match cap.filter(|c| *c > 0.0) {
            Some(c) => {
                let pct = (peak / c * 100.0).round() as i32;
                format!("peak {peak:.4} / {bound_label} {c:.4} ({pct}%)")
            }
            None => format!("peak {peak:.4}"),
        }
    };
    let peak = status.display_peak.unwrap_or(0.0);
    match status.state {
        LoadState::Within => match status.confidence {
            Some(Confidence::Validated) | None => {
                format!("Within bounds ({}) — validated", format_peak(peak))
            }
            Some(Confidence::Approximate(why)) => {
                format!("Within bounds ({}) — approximate: {why}", format_peak(peak))
            }
        },
        LoadState::Exceeds => {
            // UX dial-in A9: when the LUT row backing the verdict is
            // extrapolated past the calibration band, the "rubbing risk"
            // claim is advisory rather than authoritative. Prefix with
            // an ADVISORY tag so users (and agents) don't anchor on the
            // hard-fail framing for tools where the LUT row is
            // significantly stretched.
            let is_extrapolated = matches!(status.confidence, Some(Confidence::Approximate(_)));
            let reason_str = match (status.kind, burn_risk) {
                (CriterionKind::Chipload, true) => {
                    "chipload below vendor min — rubbing/burning risk. \
                     At low chipload the tool edge rubs instead of cutting; \
                     friction generates heat that glazes and burns the wood. \
                     Increase feed rate or reduce RPM."
                }
                (CriterionKind::Chipload, false) => {
                    "chipload above vendor max — breakage risk. \
                     Reduce feed rate or increase RPM."
                }
                (CriterionKind::Power, _) => "predicted spindle power exceeds machine limit",
                (CriterionKind::Deflection, _) => {
                    "tip deflection exceeds 200 µm — finish/breakage risk"
                }
            };
            let conf = match status.confidence {
                Some(Confidence::Validated) | None => "validated".to_owned(),
                Some(Confidence::Approximate(why)) => format!("approximate: {why}"),
            };
            let prefix = if is_extrapolated && status.kind == CriterionKind::Chipload {
                "ADVISORY (extrapolated LUT row)"
            } else {
                "EXCEEDS"
            };
            format!("{prefix}: {reason_str} ({}, {conf})", format_peak(peak))
        }
        LoadState::Unmodeled => match status.unmodeled_reason {
            Some(UnmodeledReason::SimulationRequired) => {
                "Unmodeled: simulation has not been run".to_owned()
            }
            Some(UnmodeledReason::StaleSimulation) => {
                "Unmodeled: simulation trace is stale — re-run simulation".to_owned()
            }
            Some(UnmodeledReason::ArcEngagementNotCaptured) => {
                "Unmodeled: arc-engagement metric not captured — enable Cut Metrics and re-run"
                    .to_owned()
            }
            Some(UnmodeledReason::NoVendorData) => {
                "Unmodeled: no vendor LUT row for this tool/material combination".to_owned()
            }
            Some(UnmodeledReason::SteadyStateSamplesNotPresent) => {
                "Unmodeled: no steady-state cutting samples — toolpath runs entirely on transient (plunge/ramp) feeds".to_owned()
            }
            Some(UnmodeledReason::MaterialUnvalidated) => {
                "Unmodeled: material is Custom without a validated Kc value".to_owned()
            }
            Some(UnmodeledReason::CutterModeUnsupported(why)) => {
                format!("Unmodeled: cutter mode unsupported — {why}")
            }
            Some(UnmodeledReason::NotImplemented(phase)) => {
                format!("Unmodeled: not implemented yet — {phase}")
            }
            // Roadmap F.8 — the gate isn't applicable to this op type
            // (drill / alignment-pin-drill). The carried `String` is
            // the operator-facing explanation; surface it verbatim so
            // users read "doesn't apply" rather than the misleading
            // "couldn't measure" framing.
            Some(UnmodeledReason::NotApplicableForOp(detail)) => {
                format!("N/A: {detail}")
            }
            // UX dial-in A10 — sim ran but toolpath never contacted the
            // stock. The finding is the no-contact, not "missing data".
            Some(UnmodeledReason::AllSamplesAirCutOrRapid) => {
                "Unmodeled: toolpath made no contact with material — every sample was a rapid or air-cut. Check depth / direction / stock position.".to_owned()
            }
            None => "Unmodeled".to_owned(),
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

// ── Selected span section ───────────────────────────────────────────────

fn depth_pass_chip_label(span: &Span, sid: usize) -> String {
    match &span.payload {
        Some(SpanPayload::DepthPass {
            pass_index,
            z_level,
        }) => format!("DepthPass {pass_index} · z={z_level:.2}"),
        _ => format!("DepthPass [{sid}]"),
    }
}

fn region_chip_label(span: &Span, sid: usize) -> String {
    match &span.payload {
        Some(SpanPayload::Region { region_id }) => format!("Region {region_id}"),
        _ => format!("Region [{sid}]"),
    }
}

fn span_kind_label(kind: SpanKind) -> &'static str {
    match kind {
        SpanKind::Operation => "Operation",
        SpanKind::DepthPass => "DepthPass",
        SpanKind::Region => "Region",
        SpanKind::Entry => "Entry",
        SpanKind::LeadOut => "LeadOut",
        SpanKind::LinkBridge => "LinkBridge",
        SpanKind::DressupArtifact => "DressupArtifact",
        SpanKind::WaterlineCleanup => "WaterlineCleanup",
        SpanKind::RapidOrderBarrier => "RapidOrderBarrier",
    }
}

fn span_display_label(span: &Span, sid: usize) -> String {
    match span.kind {
        SpanKind::DepthPass => depth_pass_chip_label(span, sid),
        SpanKind::Region => region_chip_label(span, sid),
        kind => format!("{} [{sid}]", span_kind_label(kind)),
    }
}

/// Find the deepest non-boundary span on `toolpath_id` that contains the
/// playhead's local move. Used to drive the Selected section when the user
/// hasn't locked it to a ribbon click. Prefers the span with the smallest
/// move range, which corresponds to the innermost structural level
/// (Region, then DepthPass, then Operation).
fn playhead_span_id(sim: &SimulationState, gui: &GuiState, toolpath_id: ToolpathId) -> Option<u32> {
    let boundary = sim.boundaries().iter().find(|b| b.id == toolpath_id)?;
    let global = sim.playback.current_move;
    if global < boundary.start_move || global >= boundary.end_move {
        return None;
    }
    let local = global - boundary.start_move;
    let rt = gui.toolpath_rt.get(&toolpath_id.0)?;
    let result = rt.result.as_ref()?;
    if !result.spans_valid() {
        return None;
    }
    result
        .spans()
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            !s.is_boundary() && !matches!(s.kind, SpanKind::RapidOrderBarrier) && s.contains(local)
        })
        .min_by_key(|(_, s)| s.move_count())
        .map(|(i, _)| i as u32)
}

/// Span scope (pass2 inspector §2.3-C) — "what's in the span I picked?". A
/// CollapsingHeader titled "Selected: <span>" with the in-panel lock toggle
/// (INS-008); the body holds the span facts, the per-span metrics grid, and
/// the in-span findings list, then the nested "Generator item" and "Generation
/// trace" drill-downs (§2.4), so the inspector has a single "what's selected"
/// home with two clearly-labelled sub-tiers.
#[allow(clippy::too_many_arguments)]
fn draw_span_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    issues: &[crate::state::simulation::SimulationIssue],
    events: &mut Vec<AppEvent>,
) {
    let toolpath_id = sim
        .focused_toolpath()
        .or_else(|| sim.current_boundary().map(|b| b.id));
    let locked_span_id = sim.debug.span_scope.span_id;
    let effective =
        toolpath_id.and_then(|tp| locked_span_id.or_else(|| playhead_span_id(sim, gui, tp)));

    // Title carries the span label + lock glyph (§2.3-C).
    let header_label = match (toolpath_id, effective) {
        (Some(tp), Some(sid)) => gui
            .toolpath_rt
            .get(&tp.0)
            .and_then(|rt| rt.result.as_ref())
            .and_then(|r| {
                r.spans()
                    .get(sid as usize)
                    .map(|s| span_display_label(s, sid as usize))
            })
            .unwrap_or_else(|| "\u{2014}".to_owned()),
        _ => "\u{2014}".to_owned(),
    };
    let lock_glyph = if locked_span_id.is_some() {
        " \u{1F512}"
    } else {
        ""
    };
    let title = format!("Selected: {header_label}{lock_glyph}");

    egui::CollapsingHeader::new(title)
        .id_salt("inspector_span")
        .default_open(true)
        .show(ui, |ui| {
            // In-panel lock toggle (INS-008) — the panel's own entry point to
            // the span lock, writing the same span_scope field the ribbon does.
            ui.horizontal(|ui| {
                let (lbl, hover) = if locked_span_id.is_some() {
                    ("\u{1F512} locked", "Click to follow the playhead again.")
                } else {
                    (
                        "\u{1F513} follow",
                        "Click to lock the Selected section to the current span.",
                    )
                };
                if ui.small_button(lbl).on_hover_text(hover).clicked() {
                    if locked_span_id.is_some() {
                        sim.debug.span_scope.span_id = None;
                        sim.debug.span_scope.toolpath_id = None;
                    } else if let (Some(tp), Some(sid)) = (toolpath_id, effective) {
                        sim.debug.span_scope.span_id = Some(sid);
                        sim.debug.span_scope.toolpath_id = Some(tp);
                    }
                }
            });

            draw_span_body(ui, sim, gui, trace, issues, toolpath_id, effective, events);

            // Nested drill-downs (§2.4): the generator item that produced the
            // span, and the generation-phase trace — folded here so the top
            // level no longer carries two competing "selection" collapsers.
            ui.add_space(4.0);
            draw_generator_item_disclosure(ui, sim, gui, max_feed);
            draw_generation_trace_disclosure(ui, sim, gui, max_feed);
        });
}

/// The Span section body: span facts, the per-span metrics grid, and the
/// in-span findings list. Split out of `draw_span_section` so the lock toggle
/// and the nested drill-downs can sit around it.
#[allow(clippy::too_many_arguments)]
fn draw_span_body(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    issues: &[crate::state::simulation::SimulationIssue],
    toolpath_id: Option<ToolpathId>,
    effective: Option<u32>,
    events: &mut Vec<AppEvent>,
) {
    let Some(tp_id) = toolpath_id else {
        ui.label(
            egui::RichText::new("Scrub or play to see span details.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    };

    let Some(rt) = gui.toolpath_rt.get(&tp_id.0) else {
        return;
    };
    let Some(result) = rt.result.as_ref() else {
        return;
    };
    let spans = result.spans();

    let Some(sid) = effective else {
        ui.label(
            egui::RichText::new("(playhead outside any span)")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    };
    let Some(span) = spans.get(sid as usize) else {
        return;
    };

    // Span-level facts: kind label, move range, payload extras.
    ui.label(
        egui::RichText::new(format!(
            "{} · moves {}–{}",
            span_kind_label(span.kind),
            span.start_move,
            span.end_move
        ))
        .small()
        .color(theme::TEXT_MUTED),
    );

    // Sample aggregates for this span — pulled from the per-trace cache,
    // which builds on the first frame of a new sim run and is reused on
    // every frame after. `agg` is `None` when no cutting samples landed
    // in this span.
    let agg = sim.debug.span_aggregates.get(tp_id, sid).copied();
    match agg {
        Some(agg) if agg.n_cutting > 0 => {
            ui.add_space(2.0);
            egui::Grid::new(("selected_metrics_grid", sid))
                .num_columns(2)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, label: &str, value: String| -> egui::Response {
                        ui.label(egui::RichText::new(label).small().color(theme::TEXT_MUTED));
                        ui.label(egui::RichText::new(value).small().monospace())
                    };
                    row(
                        ui,
                        "Samples",
                        format!("{} ({} cutting)", agg.n_samples, agg.n_cutting),
                    );
                    ui.end_row();
                    // Engagement as percent everywhere (INS-004) + provenance
                    // hover (INS-006): comparative signal, not an absolute bar.
                    row(
                        ui,
                        "Engagement",
                        format!(
                            "avg {:.0}% · peak {:.0}%",
                            agg.avg_engagement() * 100.0,
                            agg.peak_eng * 100.0
                        ),
                    )
                    .on_hover_text(ENGAGEMENT_PROVENANCE_HOVER);
                    ui.end_row();
                    row(
                        ui,
                        "Chipload",
                        format!(
                            "avg {:.4} · peak {:.4} mm",
                            agg.avg_chipload(),
                            agg.peak_chip
                        ),
                    );
                    ui.end_row();
                    row(ui, "Axial DOC", format!("peak {:.2} mm", agg.peak_doc));
                    ui.end_row();
                    row(
                        ui,
                        "MRR",
                        format!("avg {:.0} · peak {:.0} mm³/s", agg.avg_mrr(), agg.peak_mrr),
                    );
                    ui.end_row();
                });
        }
        _ => {
            ui.label(
                egui::RichText::new("No cutting samples in this span.")
                    .small()
                    .italics()
                    .color(theme::TEXT_DIM),
            );
        }
    }

    // In-scope hotspots and issues.
    // Roadmap C.4 — sort by wasted_runtime_s desc so the most expensive
    // hotspots show first (rather than insertion order).
    let mut in_scope_hotspots: Vec<(usize, &rs_cam_core::simulation_cut::SimulationCutHotspot)> =
        trace
            .hotspots
            .iter()
            .enumerate()
            .filter(|(_, h)| h.toolpath_id == tp_id.0 && h.span_path.iter().any(|s| s.0 == sid))
            .collect();
    in_scope_hotspots.sort_by(|a, b| {
        b.1.wasted_runtime_s
            .partial_cmp(&a.1.wasted_runtime_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let boundary_start = sim
        .boundaries()
        .iter()
        .find(|b| b.id == tp_id)
        .map(|b| b.start_move)
        .unwrap_or(0);
    let in_scope_issues: Vec<&_> = issues
        .iter()
        .filter(|iss| {
            iss.toolpath_id.is_some_and(|tp| tp == tp_id)
                && iss
                    .move_index
                    .checked_sub(boundary_start)
                    .is_some_and(|local| span.contains(local))
        })
        .collect();

    if in_scope_hotspots.is_empty() && in_scope_issues.is_empty() {
        return;
    }

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Findings in this span")
            .small()
            .strong()
            .color(theme::TEXT_HEADING),
    );

    const MAX_ROWS: usize = 8;
    for (h_idx, h) in in_scope_hotspots.iter().take(MAX_ROWS) {
        let global_start = sim
            .global_move_for_local(tp_id, h.move_start)
            .unwrap_or(h.move_start);
        let resp = ui
            .selectable_label(
                false,
                egui::RichText::new(format!(
                    "Hotspot · {}",
                    hotspot_summary_line(
                        h.move_start,
                        h.wasted_runtime_s,
                        h.peak_chipload_mm_per_tooth
                    )
                ))
                .small()
                .color(egui::Color32::from_rgb(255, 170, 90)),
            )
            .on_hover_text("Click to focus and jump to start move.");
        if resp.clicked() {
            sim.debug.focused_hotspot = Some((tp_id, *h_idx));
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

/// "Generator item" drill-down (pass2 inspector §2.4) — the semantic generator
/// item under the playhead/pin (label, kind, XY/Z bbox, first few params).
/// Nested under the Span section instead of floating as a top-level peer
/// "Selection details" collapser (INS-007).
fn draw_generator_item_disclosure(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
) {
    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let pinned = sim.debug.pinned_semantic_item;
    egui::CollapsingHeader::new("Generator item")
        .id_salt("inspector_generator_item")
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
                    if pinned == Some((active.toolpath_id, active.item.id)) {
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
                            for (idx, (key, value)) in active.item.params.values.iter().enumerate()
                            {
                                if idx >= 6 {
                                    break;
                                }
                                ui.label(egui::RichText::new(key).small().color(theme::TEXT_MUTED));
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

/// "Generation trace" drill-down (pass2 inspector §2.4) — generator-internal
/// phase timings and the semantic-trace item count. About *how* the toolpath
/// was built, orthogonal to the structural span tree; nested under the Span
/// section as a sibling of "Generator item" (INS-007).
fn draw_generation_trace_disclosure(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
) {
    let linked_span = sim.active_debug_span(gui, max_feed);
    let current_boundary_id = sim.current_boundary().map(|b| b.id);
    let annotation = sim.current_debug_annotation(gui).map(|(_, a)| a.label);
    egui::CollapsingHeader::new("Generation trace")
        .id_salt("inspector_generation_trace")
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
                if let Some(label) = &annotation {
                    ui.label(
                        egui::RichText::new(format!("Annotation: {label}"))
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
        let reason = UnmodeledReason::ArcEngagementNotCaptured;
        let status = CriterionStatus {
            kind: CriterionKind::Chipload,
            state: LoadState::Unmodeled,
            confidence: None,
            unmodeled_reason: Some(&reason),
            sample_range: None,
            display_peak: None,
            unit: "mm/tooth",
            exceeded: None,
        };
        let tooltip = verdict_tooltip(&status, None, false);
        assert!(tooltip.contains("Cut Metrics"));
    }
}
