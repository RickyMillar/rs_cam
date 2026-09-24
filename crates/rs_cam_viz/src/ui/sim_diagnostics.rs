use super::AppEvent;
use super::components::histogram;
use super::components::{CountPill, DistributionChart, FreshnessGate, NotMeasured, text};
use super::readiness;
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::freshness::simulation_freshness;
use crate::state::runtime::GuiState;
use crate::state::simulation::{
    CUT_METRIC_ORDER, CutMetricCard, CutMetricSet, SimulationIssueKind, SimulationState,
};
use crate::state::toolpath::ToolpathId;
use crate::ui::components::UiExt as _;
use crate::ui::theme;
use crate::ui::tokens;
use crate::ui_command::{SimJumpToMoveArgs, UiCommand};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::verdict::{
    BoundSource, ChipSide, ChiploadVerdict, CriterionKind, CriterionStatus, LoadState,
};
use rs_cam_core::tool_load::{
    Confidence, DistributionMetric, DistributionOutcome, MetricDistribution, NotMeasuredReason,
    ToolLoadReport, ToolpathLoadVerdict, UnmodeledReason,
};
use rs_cam_core::trace::toolpath_spans::{Span, SpanKind, SpanPayload};

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

    // DC6 — the page is summary-first. ONE verdict line here, and every dense
    // section below it is closed until the operator opens it. The line also
    // carries freshness, so the stale signal is reachable even when a focused
    // card replaces the scope sections (INS-001/005).
    draw_status_header(ui, sim, session, gui);

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
        // The limit rows live in ONE place. When the focused toolpath has
        // cut-metric cards, each card draws its own row and the section
        // draws the rows with no card after them, so the Inspector lists
        // every row once, in `criteria()` order. Otherwise "Now playing"
        // draws the rows, as before.
        let cut_metrics = cut_metrics_view(sim, session, gui);
        let cards_shown = matches!(cut_metrics, CutMetricsView::Cards(..));
        draw_toolpath_section(ui, sim, &load_report, cards_shown, events);
        ui.add_space(6.0);
        draw_cut_metrics_section(ui, sim, session, &cut_metrics, &load_report, events);

        let trace_arc = sim
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .map(std::sync::Arc::clone);
        if let Some(trace) = trace_arc.as_ref() {
            ui.add_space(6.0);
            draw_span_section(ui, sim, gui, trace, &issues, events);
        }
    }
    // DC6 deletes the "View" section. It was a heading plus one paragraph
    // that told the operator where the display controls live: the Overlays
    // panel (shortcut O) and each toolpath row's C / R glyphs. Pattern C of
    // `planning/ui_declutter_2026-09-14/PLAN.md` rules that nothing which
    // explains where a control lives survives. Every control the paragraph
    // named is already reachable, so the paragraph goes and no control moves.
}

/// What the verdict line draws: the words, the voice, and the caveat on hover.
struct VerdictLine {
    text: String,
    color: egui::Color32,
    hover: String,
}

/// Build the verdict line from the triage answer (DC6).
///
/// The order is the triage contract's own — `safety`, then `actions`, then
/// `advisories`. Nothing here reads `SimulationCutSummary::issue_count`: that
/// tally counts air-cut emission runs, not defects, and a project reads
/// thousands of them on a healthy cut.
///
/// `not_measured` is the count of metrics that ABSTAINED. An abstention never
/// reads as a clean pass, so a clear triage with an abstaining metric takes
/// the UNKNOWN voice and says so.
fn verdict_line(
    triage: &rs_cam_core::stock::sim_triage::SimulationTriage,
    not_measured: usize,
    trace_present: bool,
) -> VerdictLine {
    // No cut trace means no measurement at all, and the default triage is
    // empty. An empty triage is not an all-clear, so the line abstains.
    if !trace_present {
        return VerdictLine {
            text: format!(
                "{} Not measured \u{2014} this run captured no cutting metrics",
                tokens::GLYPH_UNKNOWN
            ),
            color: tokens::UNKNOWN,
            hover: "Turn on cutting-metric capture in the timeline panel and run \
                    the simulation again."
                .to_owned(),
        };
    }

    let caveat = if not_measured > 0 {
        format!(" \u{00B7} {not_measured} not measured")
    } else {
        String::new()
    };
    let provenance = "From ProjectSession::simulation_triage \u{2014} the same answer the \
                      CLI report, the MCP get_diagnostics block and narration read. \
                      Open the sections below for the detail."
        .to_owned();

    if let Some(first) = triage.safety.first() {
        let n = triage.safety.len();
        return VerdictLine {
            text: format!(
                "{} {n} safety \u{2014} {}{caveat}",
                tokens::GLYPH_DANGER,
                first.diagnostic.message
            ),
            color: tokens::DANGER,
            hover: provenance,
        };
    }
    if let Some(first) = triage.actions.first() {
        let n = triage.actions.len();
        return VerdictLine {
            text: format!(
                "{} {n} to act on \u{2014} {}{caveat}",
                tokens::GLYPH_CAUTION,
                first.diagnostic.message
            ),
            color: tokens::CAUTION,
            hover: provenance,
        };
    }
    if let Some(first) = triage.advisories.items.first() {
        let n = triage.advisories.total_matching;
        return VerdictLine {
            text: format!(
                "Advisories: {n} \u{2014} {}{caveat}",
                first.diagnostic.message
            ),
            color: tokens::INFO,
            hover: provenance,
        };
    }
    if not_measured > 0 {
        return VerdictLine {
            text: format!(
                "{} Nothing to act on, and {not_measured} metric(s) abstained",
                tokens::GLYPH_UNKNOWN
            ),
            color: tokens::UNKNOWN,
            hover: "A metric that could not be measured returns no verdict. \
                    Collision detection is never disabled."
                .to_owned(),
        };
    }
    VerdictLine {
        text: format!("{} Nothing to act on", tokens::GLYPH_OK),
        color: tokens::OK,
        hover: provenance,
    }
}

/// The verdict line (DC6 / Rule A) — the one summary the page opens with,
/// plus the freshness chip. Everything dense sits below it behind a
/// disclosure. Drawn before the card dispatch so the stale signal covers the
/// focused-card paths too (INS-001/005). No-op when no simulation has run.
///
/// The panel used to hand-roll its own verdict here (collision count, then a
/// 40 % air-cut bar). It reads the shared triage instead, so the GUI cannot
/// rank a finding differently from the CLI and the MCP for one project.
fn draw_status_header(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) {
    if sim.results.is_none() {
        return;
    }
    let trace_present = sim.results.as_ref().is_some_and(|r| r.cut_trace.is_some());
    // Scoped so the triage borrow ends before the freshness read below.
    let verdict = {
        use rs_cam_core::stock::sim_measurability::Measurability;
        let triage = sim.cached_simulation_triage(session, gui.edit_counter);
        let not_measured = triage
            .measurability
            .entries
            .iter()
            .filter(|e| matches!(e.measurability, Measurability::NotMeasurable(_)))
            .count();
        verdict_line(triage, not_measured, trace_present)
    };
    ui.label(
        egui::RichText::new(verdict.text)
            .color(verdict.color)
            .strong(),
    )
    .on_hover_text(verdict.hover);
    draw_air_cut_caution(ui, sim);
    // Freshness chip — rendered here (not in the overview body) so it covers
    // the focused-card paths too (INS-005).
    // W4, G-FRESHNESSDISAGREE: this chip and the Optimize window's banner
    // are the two halves of the ledger row. Both ask the core now, so the
    // pair cannot read "live" and "stale" about one run again.
    if simulation_freshness(session, sim).is_stale() {
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

/// The high-air-cutting caution, on the page.
///
/// # Why this is back
///
/// The declutter of 2026-09-14 deleted it, on the assumption that the
/// triage-driven verdict line above had absorbed it. It had not. Air cut
/// lands in [`ChannelCounts`] — the triage's Class D tallies — and never in
/// `safety`, `actions` or `advisories`, so no verdict is ever formed from
/// it. The page kept the percentage as an informational row and lost the
/// judgement, which is the half an operator acts on.
///
/// Caught by `air_cut_denominators_lh1`, a CORE test that scans this file.
/// It went unseen for a day because the declutter ran `cargo test -p
/// rs_cam_viz` and never the core gate.
///
/// # Why the threshold is not recomputed here
///
/// The share comes from [`AirCutRatios::air_cut_pct_of_total_runtime`], the
/// measure the shipped thresholds are tuned against. The old banner divided
/// by hand in the view, which is the pattern `rs_cam_viz/CLAUDE.md` bans.
const HIGH_AIR_CUT_PCT: f64 = 40.0;

fn draw_air_cut_caution(ui: &mut egui::Ui, sim: &SimulationState) {
    use rs_cam_core::stock::simulation_cut::AirCutRatios;
    let Some(trace) = sim.results.as_ref().and_then(|r| r.cut_trace.as_ref()) else {
        return;
    };
    let pct = trace.summary.air_cut_pct_of_total_runtime();
    if pct <= HIGH_AIR_CUT_PCT {
        return;
    }
    ui.label(
        egui::RichText::new(format!(
            "\u{26A0} High air cutting ({pct:.0}% of total runtime)"
        ))
        .small()
        .color(theme::WARNING_MILD),
    )
    .on_hover_text(
        "More than two fifths of the run is spent moving at cutting feed \
         without removing material. The toolpath may be crossing cleared \
         ground; check the linking strategy and the stock model.",
    );
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
    let toolpath_id = tp_id;
    let global_start = sim
        .global_move_for_local(toolpath_id, move_start)
        .unwrap_or(move_start);

    // Distinct shape (pass2 inspector §2.3 / INS-003): the hotspot card is
    // FILLED with a solid orange border and a filled ◍ glyph (a "time-waste"
    // focus), structurally unlike the issue card's hollow outline.
    egui::Frame::default()
        .fill(crate::ui::tokens::TINT_CAUTION)
        .stroke(egui::Stroke::new(1.5_f32, crate::ui::tokens::CAUTION))
        .inner_margin(6.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{25CD} Hotspot")
                        .strong()
                        .color(crate::ui::tokens::CAUTION),
                );
                ui.label(
                    egui::RichText::new(format!("TP {}", tp_id.0 + 1))
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
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(
                        SimJumpToMoveArgs { move_index: global_start },
                    )));
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
        .stroke(egui::Stroke::new(1.5_f32, crate::ui::tokens::CAUTION))
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
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: target.move_index,
                    })));
                }
                if ui.small_button("Next ▶").clicked()
                    && let Some(target) = sim.focus_issue_delta(gui, max_feed, 1)
                {
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: target.move_index,
                    })));
                }
                if ui.small_button("Jump").clicked() {
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: issue.move_index,
                    })));
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
    let (total_cutting, total_rapid, cycle) = aggregate_stats(sim, session, gui);
    let cycle_str = match cycle.basis {
        Some(_) => readiness::format_cycle_time(cycle.seconds),
        // No estimate is a dash, never a plausible-looking 0:00.
        None => "\u{2014}".to_owned(),
    };

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

    // Summary-first header line: cycle + the within/exceeding glance. The
    // cycle carries its basis — this header sits a panel away from the
    // timeline's readout, and the two must not look like different numbers.
    let basis_tag = match cycle.basis {
        Some(basis) => format!(" ({})", basis.qualifier()),
        None => " (no estimate)".to_owned(),
    };
    let header =
        format!("Project — {cycle_str}{basis_tag} · \u{2713}{ok} within · \u{2715}{bad} exceeding");
    // DC6 / Rule A — the section header IS the summary, so the body starts
    // closed. The verdict line above the sections is the page's one always-on
    // answer; this grid, the findings pills and the hotspot list are the
    // dig-deeper.
    egui::CollapsingHeader::new(header)
        .id_salt("inspector_project")
        .default_open(false)
        .show(ui, |ui| {
            // ─── Global stats ─── (cycle now lives in the header line)
            ui.label(
                egui::RichText::new("Global")
                    .strong()
                    .color(theme::TEXT_HEADING),
            );

            ui.param_grid("cut_overview_grid", |ui| {
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
            // buckets are verdicts (stroked pills); collisions is an observation
            // (fill-only pill). The exceeding pill is actionable (`… →` button) when
            // any TP exceeds — it jumps straight to the project-level Optimize,
            // replacing the separate ⚡ Optimize-all button (FINAL_DESIGN §5.1).
            crate::ui::components::SectionHeader::new("Findings").show(ui);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    CountPill::verdict("\u{2713} within", ok)
                        .denom(total_tp)
                        .color(crate::ui::tokens::OK)
                        .hover("Toolpaths within modeled load limits (of total modeled)."),
                );
                let exceeds_pill = CountPill::verdict("\u{2715} exceeding", bad)
            .denom(total_tp)
            .color(crate::ui::tokens::DANGER)
            .hover("Toolpaths exceeding a modeled load limit. Click to optimize all exceeding.");
                if bad > 0 {
                    if ui.add(exceeds_pill.actionable()).clicked() {
                        events.push(AppEvent::OpenOptimizeProject);
                    }
                } else {
                    ui.add(exceeds_pill.hide_when_zero());
                }
                // UP4: `unmodeled` is an ABSTENTION, not a caution. The gate
                // could not model the toolpath, so it returned no verdict at
                // all — and it wore the same amber as a measurement that came
                // back and needs review. `DESIGN_SPEC.md` §2.6: "not run" is
                // UNKNOWN.
                ui.add(
            CountPill::verdict("\u{26A0} unmodeled", unmodeled)
                .denom(total_tp)
                .color(crate::ui::tokens::UNKNOWN)
                .hide_when_zero()
                .hover("Toolpaths the gate could not model (drill cycles, no vendor data, etc.)."),
        );
                // Zero-count chips self-hide (density pass V4); the header's
                // within/exceeding line carries the all-clear.
                ui.add(
                    CountPill::observation("collisions", collision_count)
                        .color(theme::ERROR)
                        .hide_when_zero()
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

            // Checkpoint D Q2 / census §4: the measurability strip, ABOVE
            // everything else, because it qualifies everything else. Read
            // from the shared `ProjectSession::simulation_triage` contract —
            // the same object the MCP `get_diagnostics` response, the CLI
            // report and narration consume, so the four surfaces cannot
            // disagree about what was measured.
            //
            // Rendered only when something is NOT measurable: a strip that
            // says "everything was measured" on every project is a strip
            // nobody reads by the time it matters.
            {
                use rs_cam_core::stock::sim_measurability::Measurability;
                // Cached by (trace pointer, edit counter) — the same
                // staleness rule as the load report and chipload envelopes
                // beside it. Rebuilding the triage per frame is a full
                // trace pass per toolpath plus a sort, to render one strip.
                let triage = sim.cached_simulation_triage(session, gui.edit_counter);
                let unmeasured: Vec<_> = triage
                    .measurability
                    .entries
                    .iter()
                    .filter(|e| matches!(e.measurability, Measurability::NotMeasurable(_)))
                    .collect();
                if !unmeasured.is_empty() {
                    let metrics: std::collections::BTreeSet<&str> =
                        unmeasured.iter().map(|e| e.metric.label()).collect();
                    let toolpaths: std::collections::BTreeSet<usize> =
                        unmeasured.iter().map(|e| e.toolpath_id.0).collect();
                    let reason = unmeasured
                        .first()
                        .and_then(|e| e.measurability.reason())
                        .map(|r| r.describe())
                        .unwrap_or_default();
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "NOT MEASURED: {} for {} operation(s)",
                            metrics.into_iter().collect::<Vec<_>>().join(", "),
                            toolpaths.len()
                        ))
                        .small()
                        .strong()
                        .color(theme::TEXT_MUTED),
                    )
                    .on_hover_text(format!(
                        "{reason}\n\nCollision detection, material removal and axial \
                         DOC are unaffected and remain valid. Gates reading the \
                         withheld metrics abstain rather than judge it."
                    ));
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
                SimulationIssueKind::Annotation,
            ];
            // R-4 (census §3.5 D3): `Annotation` was in NEITHER list, so
            // generation-time debug annotations reached this panel and were
            // silently dropped — the one issue kind with no home. It is a
            // curated, generator-authored note, so it belongs with the
            // must-address cluster's neighbours rather than beside the
            // per-run tallies; it is listed last there so it cannot outrank
            // a collision.
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
                ui.param_grid("cut_overview_must_address", |ui| {
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
            // Density pass V5 — informational rows report % of runtime from
            // the trace summary's time-weighted tallies, not raw per-sample
            // counts ("Air cut 12031" is an expert numerator with no
            // denominator; "Air cut 12% of runtime" is a judgment a standard
            // user can act on). Raw sample counts stay reachable on hover.
            //
            // LH-1: "% of runtime" is ambiguous - these are shares of TOTAL
            // runtime (cutting + rapids), the same measure the banner and the
            // per-operation thresholds use. The air-cut row also carries the
            // cutting-time reading on hover, because that is the number the
            // MCP narration reports for the same seconds.
            let time_pcts = sim
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref())
                .map(|trace| {
                    use rs_cam_core::stock::simulation_cut::AirCutRatios;
                    let s = &trace.summary;
                    // LH-1: the air-cut share comes from the trait, not from
                    // a division written here. `air_cut_pct_of_total_runtime`
                    // is the measure every shipped threshold is tuned
                    // against, and a hand-rolled copy is how two surfaces
                    // come to quote different numbers for the same seconds.
                    //
                    // The declutter of 2026-09-14 replaced that call with a
                    // local `pct_of_total` closure. LH-1's negative arm greps
                    // for the field name followed by a divide, which a
                    // closure applied to the field does not contain — so the
                    // guard missed it. (This comment must not spell that
                    // pattern out either: the sentry scans raw source,
                    // comments included, and an explanation of the trap that
                    // contains the trap fails the test.) The closure survives
                    // for low engagement, which the trait publishes no ratio
                    // for.
                    let low_engagement_pct_of_total = if s.total_runtime_s > 0.0 {
                        s.low_engagement_time_s / s.total_runtime_s * 100.0
                    } else {
                        0.0
                    };
                    (
                        s.air_cut_pct_of_total_runtime(),
                        low_engagement_pct_of_total,
                        s.air_cut_pct_of_cutting_time(),
                    )
                });
            if let Some((air_pct, low_eng_pct, air_pct_of_cutting)) = time_pcts {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Informational")
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                ui.param_grid("cut_overview_informational", |ui| {
                    let count_for = |kind: SimulationIssueKind| {
                        informational
                            .iter()
                            .find(|(k, _)| *k == kind)
                            .map(|(_, c)| *c)
                            .unwrap_or(0)
                    };
                    let air_denominator_note = format!(
                        "\nDenominator: TOTAL runtime (cutting + rapids) - the measure \
                             the banner and the per-operation thresholds use. Over CUTTING \
                             time alone the same seconds read {air_pct_of_cutting:.0}%, \
                             which is what the MCP narration reports."
                    );
                    let rows = [
                        (
                            SimulationIssueKind::AirCut,
                            air_pct,
                            "Time the tool spends moving at cutting feed without \
                                 removing material.",
                            air_denominator_note.as_str(),
                        ),
                        (
                            SimulationIssueKind::LowEngagement,
                            low_eng_pct,
                            // R-3 (census §3.5 D2): this said "< 2% of
                            // diameter", which is the AIR-CUT trigger,
                            // not this one. Low engagement is the band
                            // ABOVE it — `0.02 <= radial_woc < 0.10`
                            // (`simulation_cut.rs`). As written, the two
                            // informational rows described the same
                            // threshold and neither described this row.
                            "Time spent cutting at light radial engagement \
                                 (2-10% of diameter). Below 2% counts as air cut, \
                                 on the row above.",
                            "",
                        ),
                    ];
                    for (kind, pct, what, denominator_note) in rows {
                        ui.label(
                            egui::RichText::new(issue_kind_label(kind))
                                .small()
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(format!("{pct:.0}% of total runtime")).small(),
                        )
                        .on_hover_text(format!(
                            "{what}\n{} flagged SAMPLES — a per-sample emission \
                                 tally, not a defect count, and not the same \
                                 population as the coalesced issue RUNS the MCP and \
                                 CLI report (on the census fixture the two differed \
                                 by 43x).{denominator_note}",
                            count_for(kind)
                        ));
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
            let hotspot_snapshot: Vec<(usize, rs_cam_core::ToolpathId, usize, f64, f64)> = sim
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref())
                .map(|trace| {
                    let mut v: Vec<(usize, rs_cam_core::ToolpathId, usize, f64, f64)> = trace
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
                            let tp_id = *tp_id_raw;
                            let global_start = sim
                                .global_move_for_local(tp_id, *move_start)
                                .unwrap_or(*move_start);
                            let label = hotspot_summary_line(*move_start, *wasted, *peak);
                            let resp = ui
                                .selectable_label(
                                    false,
                                    egui::RichText::new(label)
                                        .small()
                                        .color(crate::ui::tokens::CAUTION),
                                )
                                .on_hover_text("Click to focus and jump to this hotspot.");
                            if resp.clicked() {
                                sim.debug.focused_hotspot = Some((tp_id, *h_idx));
                                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(
                                    SimJumpToMoveArgs {
                                        move_index: global_start,
                                    },
                                )));
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
    load_report: &ToolLoadReport,
    cards_shown: bool,
    events: &mut Vec<AppEvent>,
) {
    let now = sim
        .current_boundary()
        .map(|boundary| (boundary.id, boundary.name.clone(), boundary.start_move));
    let title = match &now {
        Some((_, name, _)) => format!("Now playing: {name}"),
        None => "Now playing: \u{2014}".to_owned(),
    };
    // DC6 / Rule A — the title names the op under the playhead, which is the
    // summary. The badges and the two buttons are the dig-deeper, so the body
    // starts closed. It used to open itself on every frame a boundary played.
    egui::CollapsingHeader::new(title)
        .id_salt("inspector_toolpath")
        .default_open(false)
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
                .find(|tp| tp.toolpath_id == boundary_id)
            {
                // V2 (2026-09-18) — the three caps this function used to
                // build are gone. Two of them were GUI-owned: the power cap
                // as `max_power_kw × safety_factor`, and the deflection cap
                // as an L over D ratio of 4.0 beside a gate that judges
                // MILLIMETRES. Every row now reads
                // `CriterionStatus::bound`, which is the value the gate
                // itself judged against (S4).
                //
                // Package C (2026-09-23): when the "Cut metrics" section
                // shows cards, it draws every row of this toolpath, so the
                // rows are not drawn twice.
                if !cards_shown {
                    draw_limit_rows(ui, &limit_rows(tp));
                }
            }
            ui.horizontal(|ui| {
                if ui.small_button("Optimize this op").clicked() {
                    events.push(AppEvent::OpenOptimizeModal(boundary_id));
                }
                if ui.small_button("Jump to start").clicked() {
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: boundary_start,
                    })));
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
fn hotspot_summary_line(move_start: usize, wasted_runtime_s: f64, peak_advance: f64) -> String {
    // The value is `SimulationCutSummary::peak_chipload_mm_per_tooth`, a
    // per-sample peak of the **commanded** advance per tooth — not a chip
    // thickness, which is what "peak chip … mm" read as (A-1 census row
    // V5). Name and unit corrected 2026-08-08; the number is unchanged.
    format!(
        "m{move_start} · waste {wasted_runtime_s:.2}s · peak commanded a/t {peak_advance:.4} mm/tooth"
    )
}

/// Quantity caveat on the Cut-Metrics advance/tooth row — the same
/// expression the tool-load gate observes, so the row and the badge
/// cannot disagree (Checkpoint H1, 2026-08-08).
const ACHIEVED_ADVANCE_HOVER: &str = "Achieved advance per tooth = effective feed \u{00f7} (RPM \u{00d7} flutes), where \
     effective feed is the machine's predicted feed for the move (F-035). This is the \
     quantity vendor chipload bands are published in and the quantity the tool-load gate \
     observes.";

/// Quantity caveat on the Cut-Metrics chip-thickness row. Deliberately
/// states that there is no band for it.
const ARC_MEAN_CHIP_HOVER: &str = "Arc-mean chip thickness measured by the dexel simulator. An engagement/force \
     signal, NOT the unit any vendor chipload band is published in \u{2014} do not compare \
     it to one. It falls with engagement arc even when feed, RPM and flutes are unchanged.";

/// Provenance caveat shown as an `on_hover_text` on every engagement readout
/// (INS-006) — engagement is comparative, not an absolute under-engagement bar.
const ENGAGEMENT_PROVENANCE_HOVER: &str = "Engagement = cylinder-side radial width-of-cut fraction. Reads ~10× below the \
     algorithmic target; use it to compare variants, not as an absolute under-engagement bar.";

/// **One limit row on the tool-load surface.**
///
/// The criterion the gate produced, plus the two facts a renderer needs and
/// [`CriterionStatus`] does not carry: whether a chipload reading sits BELOW
/// its band rather than above it, and which toolpath the reading came from
/// when a surface folds several toolpaths into one column.
pub(crate) struct LimitRow<'a> {
    pub status: CriterionStatus<'a>,
    /// True for a chipload verdict that exceeded on the LOW side. The peak is
    /// then under the band floor, not over the ceiling, so the row states a
    /// burn risk instead of a percent of a bound it never passed.
    pub burn_risk: bool,
    /// The toolpath this reading was measured on. `None` when the surface
    /// already names one toolpath, as the Simulation Inspector does.
    pub source: Option<String>,
}

/// **The rows one toolpath's verdict paints, in `criteria()` order.**
///
/// V2 (2026-09-18). This iterates the criterion tier instead of naming three
/// gates, so the depth-of-cut row (S3) and the gantry-push row (S1) reach the
/// screen the day core publishes them, and a sixth gate needs no edit here.
///
/// A row whose reason is `NotApplicableForOp` is left out: the gate has no
/// meaning for the operation, which is a different statement from a limit
/// that was not measured. That is what turns a drill cycle's eight rows into
/// its three drill rows, and it leaves a milling toolpath's five untouched.
pub(crate) fn limit_rows(verdict: &ToolpathLoadVerdict) -> Vec<LimitRow<'_>> {
    let burn_risk = matches!(
        verdict.chipload,
        ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            ..
        }
    );
    verdict
        .criteria()
        .into_iter()
        .filter(|status| {
            !matches!(
                status.unmodeled_reason,
                Some(UnmodeledReason::NotApplicableForOp(_))
            )
        })
        .map(|status| LimitRow {
            burn_risk: burn_risk && status.kind == CriterionKind::Chipload,
            status,
            source: None,
        })
        .collect()
}

/// **Draw the limit rows: one face and one caption each.**
///
/// The face is `<label> <reading>` on a common 0-to-limit scale. The caption
/// under it names the SETTING that set the bound (`BoundSource::setting`) and
/// the population the reading was drawn from. The provenance and the bound's
/// own digits live in the hover, where the operator's rule 2 puts them.
///
/// The label is [`CriterionKind::label`], the crate's own word.
/// `SURVEY_UI.md` §4 counted the same three verdicts rendering in six
/// vocabularies — `advance/tooth` / `advance/t` / `Chip`, `L/D` / `defl` /
/// `deflection` — and a shared scale has to pick one. Picking core's means
/// the GUI, the CLI and the MCP name one row one way.
///
/// Both the Simulation Inspector and the Readiness page call this. Readiness
/// hands it a 560-point column and the Inspector a 240-point rail; neither
/// gets a second renderer.
pub(crate) fn draw_limit_rows(ui: &mut egui::Ui, rows: &[LimitRow<'_>]) {
    ui.label(
        egui::RichText::new("Tool load")
            .small()
            .color(theme::TEXT_STRONG),
    );
    if rows.is_empty() {
        // Every row said "does not apply". Nothing was measured and nothing
        // is claimed — a blank here would read as a clean cut.
        ui.label(
            egui::RichText::new("No limit applies to this operation.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    for row in rows {
        verdict_badge(ui, row);
    }
}

/// The vendor band floor a burn reading sits below, when the band publishes
/// one.
///
/// `BoundSource::VendorChipBand::floor_mm_per_tooth` is an `Option` because
/// [`rs_cam_core::tool_load::verdict::ChipBounds::min_mm_per_tooth`] is one:
/// some LUT rows ship only an upper bound. `None` means the row states the
/// absence; it never states a zero.
fn burn_floor(status: &CriterionStatus<'_>) -> Option<f64> {
    match status.bound_source.as_ref()? {
        BoundSource::VendorChipBand {
            floor_mm_per_tooth, ..
        } => *floor_mm_per_tooth,
        _ => None,
    }
}

/// Format the peak as a percentage of the bound the gate judged it against.
/// Returns `None` when there is no bound, or the bound is not positive —
/// the caller then paints a non-numeric badge rather than a manufactured
/// figure.
fn pct_of_bound(peak: f64, bound: Option<f64>) -> Option<i32> {
    let b = bound.filter(|b| *b > 0.0)?;
    let pct = (peak / b * 100.0).round();
    if !pct.is_finite() {
        return None;
    }
    Some(pct as i32)
}

/// The caption under a row's face: the setting that moves the bound, the
/// population the reading was drawn from, and the toolpath it came from.
///
/// Every figure is formatted from the value it describes. The population's
/// unit is [`PopulationUnit::plural`], core's own word.
fn row_caption(row: &LimitRow<'_>) -> String {
    let status = &row.status;
    let mut parts: Vec<String> = Vec::new();
    match status.bound_source.as_ref() {
        // W1 — the provenance rides in the caption the row already has, so
        // the geometry is identical for a measured bound and a rule of thumb.
        Some(source) => parts.push(source.setting().to_owned()),
        // An unmodelled row has no setting to name. The reasons that carry
        // their own clause state it here, in core's words; the rest state it
        // in the hover.
        None => match status.unmodeled_reason {
            Some(UnmodeledReason::NotImplemented(why))
            | Some(UnmodeledReason::NotApplicableForOp(why))
            | Some(UnmodeledReason::CutterModeUnsupported(why)) => parts.push(why.clone()),
            _ => {}
        },
    }
    // W4 — every row states its population, so a three-sample verdict cannot
    // read like a measured run. The vacuous case keeps its own clause, which
    // the badge already paints.
    if let Some(population) = status.population
        && !population.is_vacuous()
    {
        parts.push(format!(
            "{} of {} {}",
            population.contributing,
            population.offered,
            population.unit.plural()
        ));
    }
    if let Some(source) = &row.source {
        parts.push(source.clone());
    }
    parts.join(" \u{00b7} ")
}

/// The face of one limit row: its colour and its one-line text, for
/// example `power 1%`. `verdict_badge` paints it; a cut-metric card puts it
/// on the first line of its status hover.
fn verdict_face(row: &LimitRow<'_>) -> (egui::Color32, String) {
    let status = &row.status;
    let label = status.kind.label();
    let peak = status.display_peak.unwrap_or(0.0);
    let (color, status_text) = if status.is_vacuous() {
        // X-VAC (2026-08-14) — a gate handed an empty population returns
        // `Within` and used to paint theme::SUCCESS with a "0%" or "OK"
        // badge: indistinguishable from a measured clean cut. The VERDICT is
        // unchanged (report-tier); the badge stops claiming it measured
        // something. See `planning/review_2026-08-08/XVAC_CENSUS.md`.
        (theme::TEXT_DIM, "\u{2205}".to_owned())
    } else {
        // V1 (2026-09-18) — the colour follows `state` alone. The
        // `Approximate` arms used to take WARNING_MILD and append a `≈`, so a
        // row INSIDE its bound read as a warning. The operator's ruling:
        // every limit reads as a plain 0-to-limit figure, and the confidence
        // tier moves into the hover. The behaviour still carries the meaning
        // — a weak bound cannot refuse an export
        // (`BoundSource::gates_export`) — so the face does not have to.
        match status.state {
            LoadState::Within => match pct_of_bound(peak, status.bound) {
                Some(pct) => (theme::SUCCESS, format!("{pct}%")),
                None => (theme::SUCCESS, "OK".to_owned()),
            },
            // For a burn the peak is BELOW the floor, not above the ceiling,
            // and `status.bound` is the ceiling the gate reports. A percent
            // of it would read as "you are under the breakage limit", which
            // is true and is not the finding.
            LoadState::Exceeds if row.burn_risk => (theme::ERROR, "BURN".to_owned()),
            LoadState::Exceeds => match pct_of_bound(peak, status.bound) {
                Some(pct) => (theme::ERROR, format!("{pct}%")),
                None => (theme::ERROR, "FAIL".to_owned()),
            },
            LoadState::Unmodeled => (theme::TEXT_DIM, "\u{2014}".to_owned()),
        }
    };
    (color, format!("{label} {status_text}"))
}

/// Paint one limit row: the face, then the caption under it.
fn verdict_badge(ui: &mut egui::Ui, row: &LimitRow<'_>) {
    let hover = verdict_tooltip(row);
    let (color, face) = verdict_face(row);
    ui.label(egui::RichText::new(face).small().color(color))
        .on_hover_text(hover.clone());
    let caption = row_caption(row);
    if !caption.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(tokens::SPACE_2);
            ui.label(egui::RichText::new(caption).small().color(theme::TEXT_DIM))
                .on_hover_text(hover);
        });
    }
}

fn verdict_tooltip(row: &LimitRow<'_>) -> String {
    let status = &row.status;
    // X-VAC: the clause comes from core so GUI, CLI, MCP and the
    // diagnostics list cannot word it differently.
    let vacuity = status.vacuity_clause();
    if !vacuity.is_empty() {
        return format!(
            "{} reports {:?} but{vacuity}",
            status.kind.label(),
            status.state
        );
    }
    // For burn-risk chipload, the bound the reading sits against is the LUT
    // FLOOR — render as "peak / floor" so the relationship reads correctly
    // (peak < floor, not peak > cap). (Roadmap C.3)
    let (bound_label, bound) = if row.burn_risk {
        ("floor", burn_floor(status))
    } else {
        ("limit", status.bound)
    };
    let format_peak = |peak: f64| -> String {
        match bound.filter(|b| *b > 0.0) {
            Some(b) => {
                let pct = (peak / b * 100.0).round() as i32;
                format!("peak {peak:.4} / {bound_label} {b:.4} ({pct}%)")
            }
            // A burn against a band that publishes no floor. Stating the
            // absence is the whole point: a fabricated 0.0 floor would make
            // every reading look infinitely far above it.
            None if row.burn_risk => {
                format!("peak {peak:.4}; the vendor row publishes no floor")
            }
            None => format!("peak {peak:.4}"),
        }
    };
    let peak = status.display_peak.unwrap_or(0.0);
    let head = match status.state {
        LoadState::Within => match status.confidence {
            Some(Confidence::Validated) | None => {
                format!("Within bounds ({}) — validated", format_peak(peak))
            }
            // V1: the reason stays here, and only here. It names WHICH input
            // is approximate.
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
            // R-4 (2026-08-04): the remedy now comes from the verdict
            // that tripped, not from a match on `kind` here. A drill
            // remedy depends on the CYCLE — "reduce peck depth" is
            // wrong for a `Simple` hole, "switch to a peck cycle" is
            // wrong for an op already pecking — and the cycle is known
            // in core and was never carried this far. The fallback is
            // the chipload side split, which is the one distinction
            // this site legitimately makes on its own.
            let reason_str = status.exceeded.as_ref().map(|e| e.remedy).unwrap_or(
                match (status.kind, row.burn_risk) {
                    (CriterionKind::Chipload, true) => {
                        "achieved advance/tooth below the vendor band minimum — \
                         rubbing/burning risk. At low advance per tooth the tool edge \
                         rubs instead of cutting; friction generates heat that glazes \
                         and burns the wood. Increase feed rate or reduce RPM."
                    }
                    _ => "load criterion exceeded",
                },
            );
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
        LoadState::Unmodeled => unmodeled_text(status.unmodeled_reason),
    };
    // W1 / rule 3 — the bound's own digits and the setting that moves them.
    // `bound_clause` is core's wording, formatted from the values the source
    // carries; no number in it is typed here.
    let clause = status.bound_clause();
    if clause.is_empty() {
        return head;
    }
    match status.bound_source.as_ref() {
        Some(source) => format!("{head}\n{clause}.\nThe {} sets it.", source.setting()),
        None => format!("{head}\n{clause}."),
    }
}

/// Core's reason for an `Unmodeled` verdict, in the words every surface of
/// this panel uses. The limit rows and the cut-metric cards share it.
fn unmodeled_text(reason: Option<&UnmodeledReason>) -> String {
    match reason {
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
    }
}

/// Aggregate cutting distance, rapid distance, and cycle time across all
/// boundaries.
///
/// G-TIMEEST: the time used to be a local `cutting_distance / feed_rate()`,
/// which put this panel's header in direct disagreement with the timeline
/// sitting beside it. Both now fold the one shared
/// [`readiness::toolpath_cycle_time`] decision over the same simulated
/// population.
fn aggregate_stats(
    sim: &SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) -> (f64, f64, readiness::CycleTime) {
    let trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_ref());
    let mut total_cutting = 0.0;
    let mut total_rapid = 0.0;
    let mut cycle = readiness::CycleTime::NONE;

    for boundary in sim.boundaries() {
        if let Some(rt) = gui.toolpath_rt.get(&boundary.id)
            && let Some(result) = &rt.result
            && let Some((_, tc)) = session.find_toolpath_config_by_id(boundary.id)
        {
            total_cutting += result.stats.cutting_distance;
            total_rapid += result.stats.rapid_distance;
            cycle.fold(readiness::toolpath_cycle_time(
                session,
                trace,
                boundary.id,
                result.stats.cutting_distance,
                tc.operation.feed_rate(),
            ));
        }
    }

    (total_cutting, total_rapid, cycle)
}

// ── Cut metrics section ─────────────────────────────────────────────────
//
// Package C of `planning/sim_cut_metrics_2026-09-23/PLAN.md` (§3.1, §3.2).
// One compact row group per metric of the focused toolpath: a header row,
// a time-weighted histogram of the gate's own population with the gate's
// band as a zone, and a status glyph that the GATE verdict colours. The
// in-band share never colours it (follow-up 2026-09-24, PLAN §8).

/// True when the Inspector draws a cut-metric card for `kind`. Those kinds
/// leave the text limit rows of "Now playing"; Readiness keeps the rows.
fn has_cut_metric_card(kind: CriterionKind) -> bool {
    CUT_METRIC_ORDER.contains(&DistributionMetric::Criterion(kind))
}

/// What a card asks the panel to do after the draw.
enum CutMetricAction {
    /// Open or close the time-series drawer.
    ToggleSeries,
    /// Open the drawer and scroll it to this metric's track.
    ShowSeries(DistributionMetric),
    /// Seek the playhead to a toolpath-local move.
    Seek { local_move: usize },
}

/// How a card names and scales its metric. The time-series drawer reads
/// the same spec, so a card and its track show one unit.
pub(crate) struct CutMetricSpec {
    pub title: &'static str,
    /// The display unit. It differs from core's unit only for deflection.
    pub unit: &'static str,
    /// Display value = core value × `scale`.
    pub scale: f64,
    /// The one-word consequence of time above the ceiling.
    pub above: &'static str,
}

impl CutMetricSpec {
    pub(crate) fn of(metric: DistributionMetric) -> Self {
        match metric {
            DistributionMetric::Criterion(CriterionKind::Chipload) => Self {
                title: "Chipload",
                unit: "mm/tooth",
                scale: 1.0,
                above: "overload",
            },
            DistributionMetric::Criterion(CriterionKind::DepthOfCut) => Self {
                title: "Depth of cut",
                unit: "mm",
                scale: 1.0,
                above: "flex",
            },
            // Core bins deflection in millimetres; the card reads
            // micrometres, and so do its bounds.
            DistributionMetric::Criterion(CriterionKind::Deflection) => Self {
                title: "Tool deflection",
                unit: "\u{00b5}m",
                scale: 1000.0,
                above: "chatter",
            },
            DistributionMetric::Criterion(CriterionKind::Power) => Self {
                title: "Spindle power",
                unit: "kW",
                scale: 1.0,
                above: "stall",
            },
            DistributionMetric::Criterion(kind) => Self {
                title: kind.label(),
                unit: kind.unit(),
                scale: 1.0,
                above: "over limit",
            },
            DistributionMetric::Engagement => Self {
                title: "Engagement",
                unit: "\u{00b0}",
                scale: 1.0,
                above: "over limit",
            },
        }
    }
}

/// The four guide lines of the (i) hover, in core's words (§3.2).
fn cut_metric_guide(metric: DistributionMetric) -> Option<String> {
    let guide = match metric {
        DistributionMetric::Criterion(kind) => kind.guide()?,
        DistributionMetric::Engagement => rs_cam_core::tool_load::engagement_guide(),
    };
    Some(format!(
        "{}\n\nToo low: {}\nToo high: {}\nLevers: {}",
        guide.what, guide.too_low, guide.too_high, guide.levers
    ))
}

/// True when the bound is a rule of thumb that does not gate an export.
fn is_advisory(distribution: &MetricDistribution) -> bool {
    distribution
        .bound_source
        .as_ref()
        .is_some_and(|source| !source.gates_export())
}

/// Core's reason for a missing histogram, in words.
fn not_measured_text(reason: &NotMeasuredReason) -> String {
    match reason {
        NotMeasuredReason::Unmodeled(reason) => unmodeled_text(Some(reason)),
        NotMeasuredReason::Vacuous(population) => {
            format!("Not measured{}", population.vacuity_clause())
        }
    }
}

/// The section's summary: the header states it, so the body can start closed.
struct CutMetricsSummary {
    exceeded: usize,
    not_measured: usize,
}

impl CutMetricsSummary {
    fn of(set: &CutMetricSet) -> Self {
        let mut summary = Self {
            exceeded: 0,
            not_measured: 0,
        };
        for card in &set.cards {
            match &card.outcome {
                DistributionOutcome::Measured(d) if d.state == Some(LoadState::Exceeds) => {
                    summary.exceeded += 1;
                }
                DistributionOutcome::Measured(_) => {}
                DistributionOutcome::NotMeasured(_) => summary.not_measured += 1,
            }
        }
        summary
    }

    fn title(&self) -> String {
        if self.exceeded > 0 {
            format!("Cut metrics \u{00b7} {} outside limit", self.exceeded)
        } else if self.not_measured > 0 {
            format!("Cut metrics \u{00b7} {} not measured", self.not_measured)
        } else {
            "Cut metrics \u{00b7} within limits".to_owned()
        }
    }
}

/// What the "Cut metrics" section shows for the focused toolpath.
enum CutMetricsView {
    /// Nothing is measured, for this reason.
    Empty(&'static str),
    /// No toolpath is in focus.
    NoFocus,
    /// The cards of this toolpath.
    Cards(ToolpathId, std::sync::Arc<CutMetricSet>),
}

/// Decide what the section shows. The decision is made once per frame,
/// before "Now playing" draws, because that section draws the limit rows
/// only when no card will.
fn cut_metrics_view(
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) -> CutMetricsView {
    let has_trace = sim
        .results
        .as_ref()
        .is_some_and(|results| results.cut_trace.is_some());
    if !has_trace {
        return CutMetricsView::Empty(
            "The accepted simulation result contains no cut trace. Turn on \
             \"Capture cutting metrics\" and run the simulation again.",
        );
    }
    if simulation_freshness(session, sim).is_stale() {
        return CutMetricsView::Empty(
            "The cut trace is from an earlier version of the project. Run the \
             simulation again to measure the cut.",
        );
    }
    let Some(toolpath_id) = sim.focused_toolpath() else {
        return CutMetricsView::NoFocus;
    };
    let set = sim.cached_cut_metrics(session, gui.edit_counter, toolpath_id);
    if set.cards.is_empty() {
        return CutMetricsView::Empty("No cut metric applies to this operation.");
    }
    CutMetricsView::Cards(toolpath_id, set)
}

/// The rank of a card: the position of its criterion in `criteria()`, so
/// the cards and the rows keep the one order the CLI and the MCP use.
/// Engagement has no criterion and goes last.
fn card_rank(metric: DistributionMetric, rows: &[LimitRow<'_>]) -> usize {
    match metric {
        DistributionMetric::Criterion(kind) => rows
            .iter()
            .position(|row| row.status.kind == kind)
            .unwrap_or(usize::MAX - 1),
        DistributionMetric::Engagement => usize::MAX,
    }
}

/// The "Cut metrics" section of the focused toolpath.
///
/// DC6: the header is the summary, and the body opens by default only when
/// a criterion exceeds. The drawer toggle sits under the header, so it is
/// reachable with the body closed. It is the one toggle for the drawer
/// (§7 Q3); a card title only opens the drawer at its track. The toggle
/// state stays one state.
///
/// Each card carries its criterion's limit row in the hover of its status
/// glyph: the face `verdict_badge` paints for Readiness, the setting, the
/// population, the bound clause and the confidence reason. The rows with
/// no card follow the cards through `verdict_badge`.
fn draw_cut_metrics_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &rs_cam_core::session::ProjectSession,
    view: &CutMetricsView,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    let (toolpath_id, set) = match view {
        CutMetricsView::Empty(reason) => {
            draw_cut_metrics_empty(ui, reason);
            return;
        }
        CutMetricsView::NoFocus => {
            crate::ui::components::SectionHeader::new("Cut metrics").show(ui);
            ui.label(text::caption("Play or select a toolpath."));
            return;
        }
        CutMetricsView::Cards(toolpath_id, set) => (*toolpath_id, set),
    };
    let rows: Vec<LimitRow<'_>> = load_report
        .per_toolpath
        .iter()
        .find(|verdict| verdict.toolpath_id == toolpath_id)
        .map(limit_rows)
        .unwrap_or_default();
    let mut cards: Vec<&CutMetricCard> = set.cards.iter().collect();
    // Measured cards first, then the cards core could not measure. Inside
    // each group the cards keep the `criteria()` order.
    cards.sort_by_key(|card| {
        (
            matches!(card.outcome, DistributionOutcome::NotMeasured(_)),
            card_rank(card.metric, &rows),
        )
    });
    let other_rows: Vec<&LimitRow<'_>> = rows
        .iter()
        .filter(|row| !has_cut_metric_card(row.status.kind))
        .collect();

    let summary = CutMetricsSummary::of(set);
    let id = ui.make_persistent_id("inspector_cut_metrics");
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        summary.exceeded > 0,
    );
    let header = ui.horizontal(|ui| {
        state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        let title = ui.add(
            egui::Label::new(summary.title())
                .sense(egui::Sense::click())
                .truncate(),
        );
        if title.clicked() {
            state.toggle(ui);
        }
    });

    let mut actions: Vec<CutMetricAction> = Vec::new();
    let toggle_label = if sim.time_series_open {
        "Hide time series \u{25be}"
    } else {
        "See time series \u{25b8}"
    };
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().indent);
        if ui.link(toggle_label).clicked() {
            actions.push(CutMetricAction::ToggleSeries);
        }
    });
    // G-STALECARDS: an operation with no core result was not simulated at
    // all. Its cards say to regenerate it, because a re-run cannot help.
    let regenerate_first: Option<String> = session
        .find_toolpath_config_by_id(toolpath_id)
        .filter(|(index, _)| session.get_result(*index).is_none())
        .map(|(_, tc)| tc.name.clone());
    state.show_body_indented(&header.response, ui, |ui| {
        for (index, card) in cards.into_iter().enumerate() {
            let row = match card.metric {
                DistributionMetric::Criterion(kind) => {
                    rows.iter().find(|row| row.status.kind == kind)
                }
                DistributionMetric::Engagement => None,
            };
            if index > 0 {
                draw_hairline(ui);
            }
            draw_cut_metric_card(ui, card, row, regenerate_first.as_deref(), &mut actions);
        }
        ui.add_space(tokens::SPACE_2);
        if !other_rows.is_empty() {
            crate::ui::components::SectionHeader::new("Other limits").show(ui);
            for row in other_rows {
                verdict_badge(ui, row);
            }
        }
    });

    for action in actions {
        match action {
            CutMetricAction::ToggleSeries => {
                sim.time_series_open = !sim.time_series_open;
                sim.time_series_scroll_to = None;
            }
            CutMetricAction::ShowSeries(metric) => {
                sim.time_series_open = true;
                sim.time_series_scroll_to = Some(metric);
            }
            CutMetricAction::Seek { local_move } => {
                if let Some(move_index) = sim.global_move_for_local(toolpath_id, local_move) {
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index,
                    })));
                }
            }
        }
    }
}

/// A hairline between two metrics, with a small space on each side.
fn draw_hairline(ui: &mut egui::Ui) {
    ui.add_space(tokens::SPACE_2);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, tokens::HAIRLINE),
    );
    ui.add_space(tokens::SPACE_2);
}

/// The section when nothing is measured: the header and the abstention
/// mark with its reason.
///
/// This is the empty state the bottom panel used to draw (DC6, package A).
/// It shows state only. It starts no work and it changes no recording: the
/// one capture control is "Capture cutting metrics" in `sim_op_list.rs`, and
/// the workspace primary is the only run route.
fn draw_cut_metrics_empty(ui: &mut egui::Ui, reason: &str) {
    crate::ui::components::SectionHeader::new("Cut metrics").show(ui);
    ui.horizontal_wrapped(|ui| {
        ui.add(NotMeasured::new().reason(reason));
        ui.label(text::caption(reason));
    });
}

/// One metric: a header row, the histogram, and a caption only when some
/// cutting time is out of band.
///
/// The header row holds the title (a click opens this metric's track in the
/// drawer), the (i) guide, the status glyph and the peak. The glyph's hover
/// carries the criterion's limit row: the face, the setting, the population,
/// the bound clause and the confidence reason (`cut_metric_status_hover`).
/// A metric core could not measure draws one muted line with the
/// abstention mark and core's reason.
fn draw_cut_metric_card(
    ui: &mut egui::Ui,
    card: &CutMetricCard,
    row: Option<&LimitRow<'_>>,
    regenerate_first: Option<&str>,
    actions: &mut Vec<CutMetricAction>,
) {
    let spec = CutMetricSpec::of(card.metric);
    let hover = cut_metric_status_hover(card, row, &spec);
    ui.horizontal(|ui| {
        let title = ui
            .add(
                egui::Label::new(egui::RichText::new(spec.title).color(tokens::TEXT_STRONG))
                    .sense(egui::Sense::click())
                    .truncate(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Show this metric's time series");
        if title.clicked() {
            actions.push(CutMetricAction::ShowSeries(card.metric));
        }
        if let Some(guide) = cut_metric_guide(card.metric) {
            ui.label(egui::RichText::new(tokens::GLYPH_DETAIL).color(tokens::TEXT_MUTED))
                .on_hover_text(guide);
        }
        if let DistributionOutcome::Measured(distribution) = &card.outcome {
            let (glyph, colour) = status_glyph(distribution);
            ui.add(egui::Label::new(egui::RichText::new(glyph).color(colour)).truncate())
                .on_hover_text(hover.clone());
            if let Some(peak) = peak_text(distribution, row, &spec) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(text::caption(peak)).truncate());
                });
            }
        }
    });
    match &card.outcome {
        DistributionOutcome::Measured(distribution) => {
            let chart = DistributionChart::new(&distribution.histogram, spec.unit)
                .scale(spec.scale)
                .advisory(is_advisory(distribution))
                .show(ui);
            if let Some(bin) = chart.clicked_bin
                && let Some(Some(local_move)) = distribution.histogram.first_move_per_bin.get(bin)
            {
                actions.push(CutMetricAction::Seek {
                    local_move: *local_move,
                });
            }
            if let Some(caption) = card_caption(distribution, &spec) {
                ui.add(egui::Label::new(text::caption(caption)).wrap());
            }
        }
        DistributionOutcome::NotMeasured(reason) => {
            let reason = match (reason, regenerate_first) {
                (NotMeasuredReason::Unmodeled(UnmodeledReason::StaleSimulation), Some(name)) => {
                    format!("Unmodeled: '{name}' is not generated — regenerate it first")
                }
                _ => not_measured_text(reason),
            };
            ui.horizontal(|ui| {
                ui.add(NotMeasured::new().reason(hover));
                ui.add(egui::Label::new(text::caption(reason)).truncate());
            });
        }
    }
}

/// The status glyph of a measured card, and its colour.
///
/// The GATE verdict picks the glyph and the colour; the in-band share never
/// does. Within reads `✓`. Exceeds reads `✕` and the out-of-band share, in
/// `DANGER`, or in `CAUTION` when the bound is advisory. No cut time reads
/// the abstention glyph. A metric with no bound reads "no limit".
fn status_glyph(distribution: &MetricDistribution) -> (String, egui::Color32) {
    let hist = &distribution.histogram;
    if hist.floor.is_none() && hist.ceiling.is_none() {
        // Two cases have no bound, and neither gets an invented one:
        // engagement has no gate, and a gate can report a reading it does
        // not judge (feeds ruling R2: a finishing pass has no depth cap).
        return ("no limit".to_owned(), tokens::TEXT_MUTED);
    }
    let Some(share) = hist.in_band_share() else {
        return (tokens::GLYPH_UNKNOWN.to_owned(), tokens::UNKNOWN);
    };
    match distribution.state {
        Some(LoadState::Within) => (tokens::GLYPH_OK.to_owned(), tokens::OK),
        Some(LoadState::Exceeds) => {
            let colour = if is_advisory(distribution) {
                tokens::CAUTION
            } else {
                tokens::DANGER
            };
            let out = (1.0 - share).max(0.0);
            let text = if out > 0.0 {
                format!(
                    "{} {}",
                    tokens::GLYPH_DANGER,
                    histogram::format_share(out * 100.0)
                )
            } else {
                tokens::GLYPH_DANGER.to_owned()
            };
            (text, colour)
        }
        Some(LoadState::Unmodeled) => (tokens::GLYPH_UNKNOWN.to_owned(), tokens::UNKNOWN),
        None => (histogram::format_share(share * 100.0), tokens::TEXT_MUTED),
    }
}

/// The muted peak at the right of the header row: the peak as a percent of
/// the bound the gate judged it against, or the peak in its unit when there
/// is no bound. Every figure comes from the criterion row or the histogram.
fn peak_text(
    distribution: &MetricDistribution,
    row: Option<&LimitRow<'_>>,
    spec: &CutMetricSpec,
) -> Option<String> {
    let unit_peak = |peak: f64| {
        format!(
            "peak {} {}",
            histogram::format_value(peak * spec.scale),
            spec.unit
        )
    };
    let Some(row) = row else {
        // No criterion (engagement): the largest value in the histogram.
        return distribution
            .histogram
            .edges
            .last()
            .map(|max| unit_peak(*max));
    };
    let status = &row.status;
    let peak = status.display_peak?;
    if row.burn_risk {
        return Some(match pct_of_bound(peak, burn_floor(status)) {
            Some(pct) => format!("peak {pct} % of floor"),
            None => unit_peak(peak),
        });
    }
    Some(match pct_of_bound(peak, status.bound) {
        Some(pct) => format!("peak {pct} % of limit"),
        None => unit_peak(peak),
    })
}

/// The hover of a card's status glyph: the criterion's limit row, in the
/// words `verdict_badge` paints (G-OWNBOUND). No figure is typed here.
///
/// 1. The face (`verdict_face`), for example `power 1%`. A row with no
///    bound states the reading and the absence instead of "OK".
/// 2. The caption (`row_caption`): the setting, the population, the source.
/// 3. The in-band share of cutting time.
/// 4. The tooltip (`verdict_tooltip`): the peak against the bound, the
///    confidence reason, and the bound clause.
/// 5. The advisory line, when the bound does not stop an export.
fn cut_metric_status_hover(
    card: &CutMetricCard,
    row: Option<&LimitRow<'_>>,
    spec: &CutMetricSpec,
) -> String {
    let distribution = match &card.outcome {
        DistributionOutcome::Measured(distribution) => Some(distribution.as_ref()),
        DistributionOutcome::NotMeasured(_) => None,
    };
    let unbounded =
        distribution.is_some_and(|d| d.histogram.floor.is_none() && d.histogram.ceiling.is_none());
    let mut lines: Vec<String> = Vec::new();
    match row {
        Some(row) => {
            if unbounded {
                let peak = row.status.display_peak.map_or_else(String::new, |peak| {
                    format!(
                        " {} {}",
                        histogram::format_value(peak * spec.scale),
                        spec.unit
                    )
                });
                lines.push(format!(
                    "{} peak{peak} \u{00b7} no limit",
                    row.status.kind.label()
                ));
            } else {
                lines.push(verdict_face(row).1);
            }
            let caption = row_caption(row);
            if !caption.is_empty() {
                lines.push(caption);
            }
        }
        None => lines.push("No gate judges this metric. Use it to compare cuts.".to_owned()),
    }
    if let Some(distribution) = distribution {
        let hist = &distribution.histogram;
        if unbounded {
            if matches!(
                distribution.metric,
                DistributionMetric::Criterion(CriterionKind::DepthOfCut)
            ) {
                lines.push("Depth is reported; the deflection limit decides.".to_owned());
            }
        } else if let Some(share) = hist.in_band_share() {
            lines.push(format!(
                "{} of cut time in band.",
                histogram::format_share(share * 100.0)
            ));
        } else {
            lines.push("No cut time was measured.".to_owned());
        }
        if row.is_none() {
            let population = distribution.population;
            lines.push(format!(
                "The chart holds {} of {} {}.",
                population.contributing,
                population.offered,
                population.unit.plural()
            ));
        }
    }
    if let Some(row) = row {
        lines.push(verdict_tooltip(row));
    }
    if distribution.is_some_and(is_advisory) {
        lines.push("This limit is a rule of thumb. It does not stop an export.".to_owned());
    }
    lines.join("\n")
}

/// The caption under the chart: each out-of-band share with its one-word
/// consequence. `None` when no cutting time is out of band.
fn card_caption(distribution: &MetricDistribution, spec: &CutMetricSpec) -> Option<String> {
    let hist = &distribution.histogram;
    if hist.total_s <= 0.0 {
        return None;
    }
    let share = |seconds: f64| histogram::format_share(seconds / hist.total_s * 100.0);
    let mut parts: Vec<String> = Vec::new();
    if hist.floor.is_some() && hist.below_s > 0.0 {
        parts.push(format!("{} below floor (rubbing)", share(hist.below_s)));
    }
    if hist.ceiling.is_some() && hist.above_s > 0.0 {
        parts.push(format!(
            "{} above ceiling ({})",
            share(hist.above_s),
            spec.above
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" \u{00b7} "))
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
        Some(SpanPayload::Region { region_id, .. }) => format!("Region {region_id}"),
        _ => format!("Region [{sid}]"),
    }
}

fn span_display_label(span: &Span, sid: usize) -> String {
    match span.kind {
        SpanKind::DepthPass => depth_pass_chip_label(span, sid),
        SpanKind::Region => region_chip_label(span, sid),
        kind => format!("{} [{sid}]", kind.label()),
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
    let rt = gui.toolpath_rt.get(&toolpath_id)?;
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
fn draw_span_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    trace: &rs_cam_core::stock::simulation_cut::SimulationCutTrace,
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
            .get(&tp)
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
        .default_open(locked_span_id.is_some())
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
            draw_generator_item_disclosure(ui, sim, gui);
            draw_generation_trace_disclosure(ui, sim, gui);
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
    trace: &rs_cam_core::stock::simulation_cut::SimulationCutTrace,
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

    let Some(rt) = gui.toolpath_rt.get(&tp_id) else {
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
            span.kind.label(),
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
            ui.param_grid(("selected_metrics_grid", sid), |ui| {
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
                    rs_cam_core::feeds::ACHIEVED_ADVANCE_PER_TOOTH,
                    format!(
                        "avg {:.4} · peak {:.4} mm/tooth",
                        agg.avg_advance_per_tooth(),
                        agg.peak_advance
                    ),
                )
                .on_hover_text(ACHIEVED_ADVANCE_HOVER);
                ui.end_row();
                row(
                    ui,
                    rs_cam_core::feeds::ARC_MEAN_CHIP_THICKNESS,
                    format!(
                        "avg {:.4} · peak {:.4} mm",
                        agg.avg_chip_thickness(),
                        agg.peak_chip
                    ),
                )
                .on_hover_text(ARC_MEAN_CHIP_HOVER);
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
    let mut in_scope_hotspots: Vec<(
        usize,
        &rs_cam_core::stock::simulation_cut::SimulationCutHotspot,
    )> = trace
        .hotspots
        .iter()
        .enumerate()
        .filter(|(_, h)| h.toolpath_id == tp_id && h.span_path.iter().any(|s| s.0 == sid))
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

    crate::ui::components::SectionHeader::new("Findings in this span").show(ui);

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
                .color(crate::ui::tokens::CAUTION),
            )
            .on_hover_text("Click to focus and jump to start move.");
        if resp.clicked() {
            sim.debug.focused_hotspot = Some((tp_id, *h_idx));
            events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index: global_start,
            })));
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
            events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index: move_idx,
            })));
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
fn draw_generator_item_disclosure(ui: &mut egui::Ui, sim: &mut SimulationState, gui: &GuiState) {
    let active_semantic = sim.active_semantic_item(gui);
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
                    ui.param_grid("sim_selection_details_grid", |ui| {
                        for (idx, (key, value)) in active.item.params.values.iter().enumerate() {
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
fn draw_generation_trace_disclosure(ui: &mut egui::Ui, sim: &mut SimulationState, gui: &GuiState) {
    let linked_span = sim.active_debug_span(gui);
    let current_boundary_id = sim.current_boundary().map(|b| b.id);
    let annotation = sim.current_debug_annotation(gui).map(|(_, a)| a.label);
    egui::CollapsingHeader::new("Generation trace")
        .id_salt("inspector_generation_trace")
        .default_open(false)
        .show(ui, |ui| {
            let rt = current_boundary_id.and_then(|tp| gui.toolpath_rt.get(&tp));
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
            // X-VAC: an `Unmodeled` gate has no population to state, and
            // `None` here means exactly that — not an empty one.
            population: None,
            display_peak: None,
            unit: "mm/tooth",
            // S4: an `Unmodeled` gate judged nothing, so it states no
            // bound and no source. V2 is the step that makes the badge
            // read these instead of the caps built above.
            bound: None,
            bound_source: None,
            exceeded: None,
        };
        let tooltip = verdict_tooltip(&LimitRow {
            status,
            burn_risk: false,
            source: None,
        });
        assert!(tooltip.contains("Cut Metrics"));
    }

    /// A toolpath whose every criterion says "does not apply" — the optimizer
    /// path leaves `drill_gates` as `None` on a drill op, so this state is
    /// reachable. `limit_rows` drops every row, and `draw_limit_rows` must
    /// then state the absence: a blank there reads as a clean cut, which is
    /// defect class 3.
    #[test]
    fn a_verdict_with_no_applicable_limit_paints_no_row() {
        use rs_cam_core::tool_load::verdict::{
            DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
        };
        let na = || {
            UnmodeledReason::NotApplicableForOp("drill cycle — no continuous engagement".to_owned())
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: rs_cam_core::ToolpathId(0),
            chipload: ChiploadVerdict::Unmodeled { reason: na() },
            power: PowerVerdict::Unmodeled { reason: na() },
            deflection: DeflectionVerdict::Unmodeled { reason: na() },
            depth: DepthVerdict::Unmodeled { reason: na() },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
        };
        assert!(
            limit_rows(&verdict).is_empty(),
            "a criterion that does not apply is not a limit that was not \
             measured, and it must not take a row"
        );
    }
}
