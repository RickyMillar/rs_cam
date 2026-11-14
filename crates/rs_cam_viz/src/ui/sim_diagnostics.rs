use super::AppEvent;
use super::components::histogram;
use super::components::{
    ChartFace, ChartFlip, CountPill, DataRow, DataTable, Details, DistributionChart, FreshnessGate,
    NotMeasured, Role, SectionHeader, Sparkline, TabBadge, TabStrip, TraceOpen, role_of_tone, text,
};
use super::readiness::{self, MissingInput, MissingInputExt};
use super::sim_cut_chart::{self, CutChartInput};
use super::sim_debug::{
    debug_span_math_summary, format_json_value, semantic_kind_color, semantic_kind_label,
};
use crate::state::freshness::simulation_freshness;
use crate::state::runtime::GuiState;
use crate::state::simulation::{
    CUT_METRIC_ORDER, CutMetricCard, CutMetricSet, IssueCluster, LimitRowSet, SimInspectorTab,
    SimulationIssueKind, SimulationState,
};
use crate::state::toolpath::ToolpathId;
use crate::ui::theme;
use crate::ui::tokens;
use crate::ui_command::{SimJumpToMoveArgs, UiCommand};
use rs_cam_core::limits::CheckRow;
use rs_cam_core::session::{ProjectSession, ProjectVerdict};
use rs_cam_core::tool_load::cut_force::{RULE_FORCE_BUDGET, RULE_NO_LIMIT};
use rs_cam_core::tool_load::{
    DistributionMetric, DistributionOutcome, MetricDistribution, NotMeasuredReason, ToolLoadReport,
    UnmodeledReason,
};
use rs_cam_core::trace::toolpath_spans::{Span, SpanKind};

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

    let load_report = sim.cached_load_report(session);

    // DC6 — ONE verdict line opens the page. It also carries freshness, so
    // the stale signal is on every tab and above a focused card
    // (INS-001/005).
    draw_status_header(ui, sim, session);

    if sim.results.is_none() {
        ui.label(
            egui::RichText::new("Run simulation to see the cut overview here.")
                .small()
                .italics()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    // A focused hotspot or issue card is a drill-in. It sits above the tab
    // strip, so its Jump, Clear, Prev and Next stay on every tab, and a
    // focus set from the timeline is seen on any tab.
    if draw_focused_hotspot_card(ui, sim, events).is_none() {
        let _ = draw_focused_issue_card(ui, sim, gui, max_feed, events);
    }

    // Refresh the per-span aggregate cache once up front (cheap Arc pointer
    // compare when unchanged). The Navigate tab reads it.
    if let Some(trace_arc) = sim.results.as_ref().and_then(|r| r.cut_trace.as_ref()) {
        let trace_arc = std::sync::Arc::clone(trace_arc);
        sim.debug.span_aggregates.ensure_built(&trace_arc);
    }

    // Inspector clean-up round 3: the sections are tabs. Operator,
    // 2026-10-13: "can we de-clutter by moving the sections into tabs.
    // default to showing cut metric graphs." The tab lives in
    // `SimulationState`, so it lasts while the app runs.
    let badge = warnings_badge_for(sim.cached_simulation_triage(session));
    let mut tab = sim.inspector_tab;
    let _ = TabStrip::new(&mut tab)
        .tab(SimInspectorTab::Metrics, SimInspectorTab::Metrics.label())
        .tab_badge(
            SimInspectorTab::Warnings,
            SimInspectorTab::Warnings.label(),
            badge,
        )
        .tab(SimInspectorTab::Navigate, SimInspectorTab::Navigate.label())
        .tab(SimInspectorTab::Project, SimInspectorTab::Project.label())
        .show(ui);
    sim.inspector_tab = tab;
    ui.add_space(tokens::SPACE_2);

    match tab {
        SimInspectorTab::Metrics => draw_metrics_tab(ui, sim, session, events),
        SimInspectorTab::Warnings => {
            let issues = sim.issues(gui, max_feed);
            draw_warnings_tab(ui, sim, session, &issues, &load_report, events);
        }
        SimInspectorTab::Navigate => {
            draw_now_playing(ui, sim, events);
            let trace_arc = sim
                .results
                .as_ref()
                .and_then(|r| r.cut_trace.as_ref())
                .map(std::sync::Arc::clone);
            if let Some(trace) = trace_arc.as_ref() {
                let issues = sim.issues(gui, max_feed);
                ui.add_space(tokens::SPACE_3);
                draw_span_section(ui, sim, gui, trace, &issues, events);
            }
        }
        SimInspectorTab::Project => draw_project_tab(ui, sim, session, gui),
    }
    // DC6 deletes the "View" section. It was a heading plus one paragraph
    // that told the operator where the display controls live. Pattern C of
    // `planning/ui_declutter_2026-09-14/PLAN.md` rules that nothing which
    // explains where a control lives survives.
}

/// The Warnings badge of `triage` (F189): the count of each class, from one
/// exhaustive match, so a new class does not compile until it says whether
/// the badge counts it.
#[must_use]
pub fn warnings_badge_for(
    triage: &rs_cam_core::stock::sim_triage::SimulationTriage,
) -> Option<TabBadge> {
    use rs_cam_core::stock::sim_triage::TriageClass;
    let (mut safety, mut actions) = (0, 0);
    for finding in triage.findings() {
        match finding.class {
            TriageClass::Safety => safety += 1,
            TriageClass::Action => actions += 1,
            // The badge counts what to act on. An advisory is listed on the
            // tab, not counted on its label.
            TriageClass::Advisory => {}
        }
    }
    warnings_badge(safety, actions)
}

/// The badge of the Warnings tab: the number of safety and action items of
/// the shared triage. The danger role when any item is a safety item, the
/// caution role otherwise, and no badge at 0.
///
/// The count is the triage's, never `issue_count`: that field counts
/// air-cut emission runs, thousands on a healthy cut (DC6).
#[must_use]
pub fn warnings_badge(safety: usize, actions: usize) -> Option<TabBadge> {
    let count = safety + actions;
    if count == 0 {
        return None;
    }
    let role = if safety > 0 {
        Role::Danger
    } else {
        Role::Caution
    };
    Some(TabBadge::Count(count, role))
}

/// The Metrics tab (the default): the cut-metric charts of the operation
/// under the playhead, then the limits they do not chart.
///
/// When the operation has no chart, the tab draws its default-view limit
/// rows and the "Cut groups" drawer, so every row stays on the page.
fn draw_metrics_tab(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    events: &mut Vec<AppEvent>,
) {
    let cut_metrics = cut_metrics_view(sim, session);
    draw_cut_metrics_section(ui, sim, session, &cut_metrics, events);
    if matches!(cut_metrics, CutMetricsView::Cards(..)) {
        return;
    }
    let Some(boundary_id) = sim.current_boundary().map(|boundary| boundary.id) else {
        return;
    };
    let row_set = sim.cached_limit_rows(session);
    let rows = rows_of(&row_set, boundary_id);
    if rows.is_empty() {
        return;
    }
    ui.add_space(tokens::SPACE_2);
    SectionHeader::new("Limits").show(ui);
    DataTable::new("metrics_tab_limit_rows")
        .rows(
            default_view_rows(rows)
                .into_iter()
                .map(|row| limit_data_row(row, None)),
        )
        .show(ui);
    draw_cut_groups(ui, rows, &mut sim.cut_groups_open);
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
            // U2: the hover names the cause and the step that removes it.
            hover: MissingInput::NoCutTrace.remedy().to_owned(),
        };
    }

    let caveat = if not_measured > 0 {
        format!(" \u{00B7} {not_measured} not measured")
    } else {
        String::new()
    };
    let provenance = "From ProjectSession::simulation_triage \u{2014} the same answer the \
                      CLI report, the MCP get_diagnostics block and narration read. \
                      Open the Warnings tab for the detail."
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
/// plus the freshness chip. Everything dense sits below it on a tab
/// (round 3). Drawn before the tab strip so the stale signal covers the
/// focused-card paths too (INS-001/005). No-op when no simulation has run.
///
/// The panel used to hand-roll its own verdict here (collision count, then a
/// 40 % air-cut bar). It reads the shared triage instead, so the GUI cannot
/// rank a finding differently from the CLI and the MCP for one project.
fn draw_status_header(ui: &mut egui::Ui, sim: &mut SimulationState, session: &ProjectSession) {
    if sim.results.is_none() {
        return;
    }
    let trace_present = sim.results.as_ref().is_some_and(|r| r.cut_trace.is_some());
    // Scoped so the triage borrow ends before the freshness read below.
    let verdict = {
        use rs_cam_core::stock::sim_measurability::Measurability;
        let triage = sim.cached_simulation_triage(session);
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
    draw_project_verdict_caption(ui, sim.cached_project_verdict(session));
    // Freshness chip — rendered here (not in the overview body) so it covers
    // the focused-card paths too (INS-005).
    // W4, G-FRESHNESSDISAGREE: this chip asks the core, so it cannot read
    // "live" about a run the core calls stale.
    if let Some(reason) = simulation_freshness(session, sim).stale_reason(session) {
        FreshnessGate::banner(ui, &reason);
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

/// W3 (G-WLAIRLABEL, D69 §3): the value of the air-cut row. `air_pct` is
/// the air-cut share of the total runtime, and `skin_share` the share of
/// that air time that removes material (core
/// `SimulationCutTrace::air_cut_removing_share`). With no skin the row
/// reads as before; with a skin it gives the TRUE air and the skin, on the
/// same row: `14% (+71% skin)`, and [`air_skin_note`] is on the hover. The
/// long form `14% (+71% skin cut under 2% WOC)` elides in the value column
/// of the 304-point inspector (measured), so the WOC words go to the hover.
/// The air-cut figures and the gates do not change.
pub(crate) fn air_cut_value(air_pct: f64, skin_share: f64) -> String {
    if skin_share > 0.0 {
        format!(
            "{:.0}% (+{:.0}% skin)",
            air_pct * (1.0 - skin_share),
            air_pct * skin_share,
        )
    } else {
        format!("{air_pct:.0}% of total runtime")
    }
}

/// W3: the hover line of the skin, or an empty string with no skin.
pub(crate) fn air_skin_note(air_pct: f64, skin_share: f64) -> String {
    if skin_share > 0.0 {
        format!(
            "\nSkin: {:.0}% of the total runtime is flagged air but removes material, \
             cut under {:.0}% WOC by the flank above the tip. The first figure is the \
             air that removes nothing.",
            air_pct * skin_share,
            rs_cam_core::stock::simulation_cut::AIR_CUT_RADIAL_WOC_FRACTION * 100.0
        )
    } else {
        String::new()
    }
}

/// The project verdict caption, on the page (F113).
///
/// # Why the high-air-cutting caution is here
///
/// The declutter of 2026-09-14 deleted it, on the assumption that the
/// triage-driven verdict line above had absorbed it. It had not. Air cut
/// lands in [`ChannelCounts`] — the triage's Class D tallies — and never in
/// `safety`, `actions` or `advisories`, so no verdict is ever formed from
/// it. The page kept the percentage as an informational row and lost the
/// judgement, which is the half an operator acts on.
///
/// Caught by `air_cut_denominators_lh1`, a CORE test that scans this file.
///
/// # Why the band is not here
///
/// The caption reads core's [`ProjectVerdict`], the ladder the CLI summary
/// and the MCP `get_diagnostics` reply read. This file held a copy of the
/// 40 % band, and MCP held another with no holder-collision arm (F113).
/// The match is exhaustive, so a new arm does not compile here until this
/// page says how it shows it.
fn draw_project_verdict_caption(ui: &mut egui::Ui, verdict: Option<ProjectVerdict>) {
    let Some(verdict) = verdict else {
        return;
    };
    match verdict {
        // The triage line above names each collision and each failed check
        // as a safety finding. A second line for the same finding is noise.
        ProjectVerdict::Unknown { .. }
        | ProjectVerdict::Collisions { .. }
        | ProjectVerdict::RapidCollisions { .. }
        | ProjectVerdict::Ok => {}
        ProjectVerdict::HighAirCut {
            pct_of_total_runtime: pct,
        } => {
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
    }
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
                    egui::RichText::new(format!("TP {}", tp_id.raw() + 1))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            });
            // The facts of the hotspot, one row each (inspector clean-up
            // round 2). The move, the waste and the peak are the fields of
            // the canonical summary line (INS-004) that the Top-hotspots and
            // the Span-findings rows print.
            DataTable::new(("hotspot_card", tp_id.raw(), move_start))
                .row(
                    DataRow::value("Moves", format!("{move_start}\u{2013}{move_end}"))
                        .hover(hotspot_summary_line(move_start, wasted, peak_chip)),
                )
                .row(DataRow::measured("Waste", format!("{wasted:.2}"), "s"))
                .row(DataRow::measured(
                    "Peak commanded a/t",
                    format!("{peak_chip:.4}"),
                    "mm/tooth",
                ))
                .row(DataRow::measured(
                    "Peak DOC",
                    format!("{peak_doc:.2}"),
                    "mm",
                ))
                .row(DataRow::value("Samples", sample_count.to_string()))
                .row(
                    DataRow::measured("Avg engagement", format!("{:.0}", avg_eng * 100.0), "%")
                        .hover(ENGAGEMENT_PROVENANCE_HOVER),
                )
                .row(DataRow::value(
                    "Position",
                    format!("X{:.1} Y{:.1} Z{:.2}", pos[0], pos[1], pos[2]),
                ))
                .show(ui);
            ui.horizontal(|ui| {
                if ui.small_button("Jump").clicked() {
                    events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                        move_index: global_start,
                    })));
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
            DataTable::new("issue_card")
                .row(DataRow::value("Move", issue.move_index.to_string()))
                .show(ui);
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
            });
        });
    Some(())
}

/// The Warnings tab: the findings of the run.
///
/// The triage items first (the items the tab badge counts, each a jump),
/// then the rows the stock change did not judge, the count pills, the
/// metrics the cell size withheld, the issue kinds and the hotspot jump
/// list. The tab is the disclosure, so no section inside it is closed.
fn draw_warnings_tab(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    issues: &[crate::state::simulation::SimulationIssue],
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
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

    draw_triage_rows(ui, sim, session, events);

    // S6 (stock changes, "cut as"): the samples of an own material with no
    // force data. The gates did not judge them; name them, one line per
    // toolpath and material, in the core's words.
    DataTable::new("inspector_not_judged")
        .rows(summary.not_judged.iter().map(|entry| {
            DataRow::note(Role::Unknown, entry.label()).hover(
                "A stock change cuts this material as its own material, and the \
                 material has no force data in the library. The load checks did not \
                 judge these samples.",
            )
        }))
        .show(ui);
    ui.add_space(tokens::SPACE_2);
    // Project-wide findings counts — the same `CountPill` grammar + `/T`
    // denominator the verdict HUD uses, reading the same `summary()` producer
    // so the two rollups cannot diverge (W3.3 carry-over, P4-001/002). Load
    // buckets are verdicts (stroked pills); collisions is an observation
    // (fill-only pill).
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
            .hover("Toolpaths exceeding a modeled load limit.");
        ui.add(exceeds_pill.hide_when_zero());
        // UP4: `unmodeled` is an ABSTENTION, not a caution. The gate
        // could not model the toolpath, so it returned no verdict at
        // all. `DESIGN_SPEC.md` §2.6: "not run" is UNKNOWN.
        ui.add(
            CountPill::verdict("\u{26A0} unmodeled", unmodeled)
                .denom(total_tp)
                .color(crate::ui::tokens::UNKNOWN)
                .hide_when_zero()
                .hover("Toolpaths the gate could not model (drill cycles, no vendor data, etc.)."),
        );
        // Zero-count chips self-hide (density pass V4).
        ui.add(
            CountPill::observation("collisions", collision_count)
                .color(theme::ERROR)
                .hide_when_zero()
                .hover("Rapid/holder collisions detected during simulation."),
        );
    });

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
        // Cached on the load report, the session parts beside it
        // and the evidence. A triage build per frame is a full
        // trace pass per toolpath plus a sort, to render one strip.
        let triage = sim.cached_simulation_triage(session);
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
                unmeasured.iter().map(|e| e.toolpath_id.raw()).collect();
            let reason = unmeasured
                .first()
                .and_then(|e| e.measurability.reason())
                .map(|r| r.describe())
                .unwrap_or_default();
            DataTable::new("cut_overview_not_measured_strip")
                .row(
                    DataRow::value(
                        "Not measured",
                        metrics.into_iter().collect::<Vec<_>>().join(", "),
                    )
                    .status(Role::Unknown, format!("{} op(s)", toolpaths.len()))
                    .hover(format!(
                        "{reason}\n\nCollision detection, material removal and \
                                 axial DOC are unaffected and remain valid. Gates reading \
                                 the withheld metrics abstain rather than judge it."
                    )),
                )
                .show(ui);
        }
    }

    // Roadmap C.1 — partition the issue count by SimulationIssueKind into
    // a "must address" cluster (collisions, hotspots) and an
    // informational cluster (low engagement, air cut). The single
    // `issue_count` row hid 24 800 air-cut "issues" alongside 14
    // hotspots, which makes the project look broken when most of the
    // count is emission noise.
    //
    // F189: the cluster is `SimulationIssueKind::cluster`, one exhaustive
    // match, not two hand lists of kinds.
    let mut must_address: Vec<(u8, SimulationIssueKind, usize)> = Vec::new();
    let mut informational: Vec<(SimulationIssueKind, usize)> = Vec::new();
    for issue in issues {
        let kind = issue.kind;
        match kind.cluster() {
            IssueCluster::MustAddress { rank } => {
                match must_address.iter_mut().find(|(_, k, _)| *k == kind) {
                    Some(entry) => entry.2 += 1,
                    None => must_address.push((rank, kind, 1)),
                }
            }
            IssueCluster::Informational => {
                match informational.iter_mut().find(|(k, _)| *k == kind) {
                    Some(entry) => entry.1 += 1,
                    None => informational.push((kind, 1)),
                }
            }
        }
    }
    must_address.sort_by_key(|(rank, _, _)| *rank);
    if !must_address.is_empty() {
        // The severity is the status chip of each row, so the header
        // carries no colour of its own.
        SectionHeader::new("Must address").show(ui);
        DataTable::new("cut_overview_must_address")
            .rows(must_address.iter().map(|(_, kind, count)| {
                DataRow::value(issue_kind_label(*kind), count.to_string())
                    .status(Role::Danger, "act")
            }))
            .show(ui);
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
                // W3 (G-WLAIRLABEL): the share of the air time that removes
                // material, a skin the flank cuts above the tip. The
                // summary is the whole run, so the share is too (`None`).
                trace.air_cut_removing_share(None),
            )
        });
    if let Some((air_pct, low_eng_pct, air_pct_of_cutting, skin_share)) = time_pcts {
        SectionHeader::new("Informational").show(ui);
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
                         which is what the MCP narration reports.{}",
            air_skin_note(air_pct, skin_share)
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
                // R-3 (census §3.5 D2): this said "< 2% of diameter",
                // which is the AIR-CUT trigger, not this one. Low
                // engagement is the band ABOVE it — `0.02 <=
                // radial_woc < 0.10` (`simulation_cut.rs`).
                "Time spent cutting at light radial engagement \
                             (2-10% of diameter). Below 2% counts as air cut, \
                             on the row above.",
                "",
            ),
        ];
        DataTable::new("cut_overview_informational")
            .rows(rows.into_iter().map(|(kind, pct, what, denominator_note)| {
                let value = if kind == SimulationIssueKind::AirCut {
                    air_cut_value(pct, skin_share)
                } else {
                    format!("{pct:.0}% of total runtime")
                };
                DataRow::value(issue_kind_label(kind), value).hover(format!(
                    "{what}\n{} flagged SAMPLES — a per-sample emission \
                                 tally, not a defect count, and not the same \
                                 population as the coalesced issue RUNS the MCP and \
                                 CLI report (on the census fixture the two differed \
                                 by 43x).{denominator_note}",
                    count_for(kind)
                ))
            }))
            .show(ui);
    } else if sim.results.is_some() {
        // U2: a run with no cut trace measured no air cut and no
        // engagement. The rows stay, with the abstention mark, so an
        // absent figure cannot read as a clean 0 %.
        SectionHeader::new("Informational").show(ui);
        let reason = MissingInput::NoCutTrace.remedy();
        DataTable::new("cut_overview_informational_not_measured")
            .rows(
                [
                    SimulationIssueKind::AirCut,
                    SimulationIssueKind::LowEngagement,
                ]
                .into_iter()
                .map(|kind| DataRow::not_measured(issue_kind_label(kind)).hover(reason)),
            )
            .show(ui);
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
        // Round 3: the Warnings tab is the disclosure, so the list
        // is open. It is the hotspot jump list of the run.
        SectionHeader::new("Top hotspots")
            .trailing(hotspot_snapshot.len().to_string())
            .show(ui);
        const TOP_N: usize = 10;
        let shown: Vec<_> = hotspot_snapshot.iter().take(TOP_N).collect();
        let more = hotspot_snapshot.len().saturating_sub(TOP_N);
        let clicked = DataTable::new("cut_overview_top_hotspots")
            .rows(shown.iter().map(|(_, _, move_start, wasted, peak)| {
                DataRow::record(hotspot_summary_line(*move_start, *wasted, *peak))
                    .tone(crate::ui::tokens::CAUTION)
                    .hover("Click to focus and jump to this hotspot.")
                    .clickable()
            }))
            .row_opt((more > 0).then(|| {
                DataRow::record(format!(
                    "\u{2026} +{more} more (the Navigate tab lists them by span)"
                ))
            }))
            .show(ui)
            .clicked;
        if let Some(&&(h_idx, tp_id, move_start, _, _)) = clicked.and_then(|index| shown.get(index))
        {
            let global_start = sim
                .global_move_for_local(tp_id, move_start)
                .unwrap_or(move_start);
            sim.debug.focused_hotspot = Some((tp_id, h_idx));
            events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index: global_start,
            })));
        }
    }
}

/// The role of a Warnings row of `class` (F189). The one exhaustive match:
/// a new class does not compile until this tab says how it shows it.
#[must_use]
pub fn triage_row_role(class: rs_cam_core::stock::sim_triage::TriageClass) -> Role {
    use rs_cam_core::stock::sim_triage::TriageClass;
    match class {
        TriageClass::Safety => Role::Danger,
        TriageClass::Action => Role::Caution,
        TriageClass::Advisory => Role::Info,
    }
}

/// Every triage item, in the contract's own order: safety, then actions,
/// then the capped advisories (F189: the tab read two of three buckets). A
/// row with a toolpath jumps to its worst move. A clear triage reads one
/// "Nothing to act on" row.
fn draw_triage_rows(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    events: &mut Vec<AppEvent>,
) {
    let mut targets: Vec<Option<(ToolpathId, usize)>> = Vec::new();
    let (mut rows, hidden): (Vec<DataRow>, usize) = {
        let triage = sim.cached_simulation_triage(session);
        let rows = triage
            .findings()
            .map(|finding| {
                let role = triage_row_role(finding.class);
                let target = finding
                    .dedup_key
                    .toolpath_id
                    .map(|id| (id, finding.worst.move_index));
                targets.push(target);
                let times = if finding.occurrences > 1 {
                    format!(" \u{00B7} {}\u{00D7}", finding.occurrences)
                } else {
                    String::new()
                };
                let row = DataRow::note(role, finding.diagnostic.message.clone());
                match target {
                    Some((_, local_move)) => row
                        .hover(format!(
                            "Move {local_move} of its operation{times}. Click to jump to it."
                        ))
                        .clickable(),
                    None => row.hover(format!("From the simulation triage{times}.")),
                }
            })
            .collect();
        (rows, triage.advisories.hidden())
    };
    if hidden > 0 {
        // The advisory cap says what it withheld (`Bounded`).
        rows.push(DataRow::record(format!(
            "\u{2026} +{hidden} more advisories (the cap withheld them)"
        )));
        targets.push(None);
    }
    if rows.is_empty() {
        DataTable::new("warnings_triage_clear")
            .row(DataRow::note(Role::Ok, "Nothing to act on"))
            .show(ui);
        return;
    }
    let clicked = DataTable::new("warnings_triage")
        .rows(rows)
        .show(ui)
        .clicked;
    if let Some((toolpath_id, local_move)) =
        clicked.and_then(|index| targets.get(index).copied().flatten())
    {
        let move_index = sim
            .global_move_for_local(toolpath_id, local_move)
            .unwrap_or(local_move);
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index,
        })));
    }
}

/// The Project tab: the run as a whole. The cycle time with its basis, the
/// move and operation counts, and the cut and rapid distances.
fn draw_project_tab(
    ui: &mut egui::Ui,
    sim: &SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) {
    let (total_cutting, total_rapid, cycle) = aggregate_stats(sim, session, gui);
    // No estimate is a dash, never a plausible-looking 0:00. The basis is
    // on the face: this tab sits a panel away from the timeline's readout,
    // and the two must not look like different numbers. U3: the basis
    // names each missing input, and reads "no estimate" when there is none.
    let cycle_str = match cycle.basis {
        Some(_) => readiness::format_cycle_time(cycle.seconds),
        None => "\u{2014}".to_owned(),
    };
    DataTable::new("cut_overview_table")
        .row(
            DataRow::value("Cycle time", format!("{cycle_str} ({})", cycle.label()))
                .hover("The same estimate as the timeline readout."),
        )
        .row(DataRow::value("Moves", sim.total_moves().to_string()))
        .row(DataRow::value(
            "Operations",
            sim.boundaries().len().to_string(),
        ))
        .row(DataRow::measured(
            "Cut distance",
            format!("{total_cutting:.0}"),
            "mm",
        ))
        .row(DataRow::measured(
            "Rapid distance",
            format!("{total_rapid:.0}"),
            "mm",
        ))
        .show(ui);
}

/// The operation under the playhead and its jump (Navigate tab). The limit
/// rows of the operation are on the Metrics tab.
fn draw_now_playing(ui: &mut egui::Ui, sim: &SimulationState, events: &mut Vec<AppEvent>) {
    let now = sim
        .current_boundary()
        .map(|boundary| (boundary.name.clone(), boundary.start_move));
    SectionHeader::new("Now playing").show(ui);
    let Some((name, boundary_start)) = now else {
        DataTable::new("now_playing")
            .row(
                DataRow::value("Operation", "\u{2014}")
                    .hover("Scrub or play to see the active op."),
            )
            .show(ui);
        return;
    };
    DataTable::new("now_playing")
        .row(DataRow::value("Operation", name))
        .show(ui);
    if ui.small_button("Jump to start").clicked() {
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: boundary_start,
        })));
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

/// The rows of the default view of one toolpath (ruling Q5, design 3.7):
/// the rows of the op's own cut group and every other row that exceeds.
/// A `NotApplicable` row (a milling gate on a drill cycle) is only in the
/// "Cut groups" drawer. Core decides each row's place
/// (`CheckRow::in_default_view`); this keeps the core order.
pub(crate) fn default_view_rows(rows: &[CheckRow]) -> Vec<&CheckRow> {
    rows.iter().filter(|row| row.in_default_view()).collect()
}

/// The rows of `toolpath_id` in the memo of the load report, or an empty
/// slice when the report leaves it out.
pub(crate) fn rows_of(set: &LimitRowSet, toolpath_id: ToolpathId) -> &[CheckRow] {
    set.iter()
        .find(|t| t.toolpath_id == toolpath_id)
        .map_or(&[], |t| t.rows.as_slice())
}

/// The hover of a row: the core detail, one clause per line.
pub(crate) fn row_hover(row: &CheckRow) -> String {
    row.detail().replace("; ", "\n")
}

/// **One limit row of a `DataTable`** (P29-10 10e): the core face in the
/// name column, the source toolpath (Readiness) in the value column, the
/// tone as the status chip and the core detail on the hover. A row with
/// no number has no chip; its face and its hover say why (IC3). Nothing here computes a percent, a state or a word: core wrote
/// them (`limits::CheckRow`).
pub(crate) fn limit_data_row(row: &CheckRow, source: Option<&str>) -> DataRow {
    limit_data_row_with_face(row, row.face(), source)
}

/// [`limit_data_row`] with the face the caller reads from the row (P29-10
/// 10f: `CheckRow::face_in_group` under the title of a cut). The one limit
/// row of the viz: the tone, the chip and the hover are the same.
pub(crate) fn limit_data_row_with_face(
    row: &CheckRow,
    face: &str,
    source: Option<&str>,
) -> DataRow {
    let role = role_of_tone(row.tone());
    let hover = match source {
        Some(name) => format!("{name}\n{}", row_hover(row)),
        None => row_hover(row),
    };
    let data = match source {
        Some(name) => DataRow::value(face, name).tone(tokens::TEXT_MUTED),
        None => DataRow::record(face),
    };
    // A row with no number carries no chip: its face already says why
    // ("force no limit here", "gantry push not modelled"), and the face
    // keeps the whole row width (IC3: a value not measured has no chip).
    if role == Role::Unknown {
        return data.hover(hover);
    }
    data.status(role, row.tone().chip_word()).hover(hover)
}

/// **Draw the limit rows of Readiness: one row each, with its source.**
///
/// Readiness hands it a 560-point column. Each row is a core row
/// (`limit_data_row`); the source names the toolpath the row came from
/// when the project holds more than one.
pub(crate) fn draw_limit_rows(ui: &mut egui::Ui, rows: &[(&CheckRow, Option<&str>)]) {
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
    DataTable::new("limit_rows")
        .rows(
            rows.iter()
                .map(|(row, source)| limit_data_row(row, *source)),
        )
        .show(ui);
}

/// Core's reason for an `Unmodeled` verdict, in the words every surface of
/// this panel uses. The limit rows and the cut-metric cards share it.
///
/// F080: the clause is core's [`UnmodeledReason::operator_text`]; this panel
/// adds only its prefix and the menu path of a remedy. Two arms keep their
/// own text, for the reasons below.
///
/// U3: core reports `SimulationRequired` when the gate has no cut trace. The
/// Inspector draws these rows only while a simulation exists, so the text
/// names the missing trace. It does not say "has not been run", which can
/// be false there. The Readiness page also draws these rows with no
/// simulation, and there "run the simulation" in the same sentence is the
/// remedy. Every run captures the cut trace (operator ruling 2026-10-02,
/// "always capture"), so no text names a capture control.
///
/// Roadmap F.8: `NotApplicableForOp` reads "N/A", not "Unmodeled": the gate
/// does not apply to the op type, and the carried detail is the
/// operator-facing explanation.
fn unmodeled_text(reason: Option<&UnmodeledReason>) -> String {
    match reason {
        Some(UnmodeledReason::SimulationRequired) => {
            "Unmodeled: no cut trace \u{2014} run the simulation (Simulation \u{25B8} \
             Run Simulation)"
                .to_owned()
        }
        Some(UnmodeledReason::NotApplicableForOp(detail)) => format!("N/A: {detail}"),
        Some(reason @ UnmodeledReason::ArcEngagementNotCaptured) => format!(
            "Unmodeled: {} (Simulation \u{25B8} Re-run Simulation)",
            reason.operator_text()
        ),
        Some(reason) => format!("Unmodeled: {}", reason.operator_text()),
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
            && let Some(result) = rt.run.result()
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

/// True when the Inspector draws a cut-metric card for the gate of `row`:
/// a headline row of a carded gate paints on its card, not in "Other
/// limits". P17: the depth row has no card, so it paints once in "Other
/// limits".
fn has_cut_metric_card(row: &CheckRow) -> bool {
    row.check()
        .limit
        .criterion()
        .is_some_and(|kind| CUT_METRIC_ORDER.contains(&DistributionMetric::Criterion(kind)))
}

/// The headline row of the gate that a card charts, if the card charts a
/// gate. The stock-population row comes first in core order, before the
/// own-material rows.
fn card_row(metric: DistributionMetric, rows: &[CheckRow]) -> Option<&CheckRow> {
    let kind = match metric {
        DistributionMetric::Criterion(kind) => kind,
        DistributionMetric::Engagement | DistributionMetric::CutForce => return None,
    };
    rows.iter()
        .find(|row| row.is_headline() && row.check().limit.criterion() == Some(kind))
}

/// What a card asks the panel to do after the draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMetricAction {
    /// Flip this metric's card to its other face.
    Flip(DistributionMetric),
    /// Open this metric's trace in the trace modal.
    Open(DistributionMetric),
    /// Seek the playhead to a toolpath-local move.
    Seek { local_move: usize },
}

/// The four guide lines of the (i) hover, in core's words (§3.2).
fn cut_metric_guide(metric: DistributionMetric) -> Option<String> {
    let guide = match metric {
        DistributionMetric::Criterion(kind) => kind.guide()?,
        DistributionMetric::Engagement => rs_cam_core::tool_load::engagement_guide(),
        DistributionMetric::CutForce => rs_cam_core::tool_load::cut_force_guide(),
    };
    Some(format!(
        "{}\n\nToo low: {}\nToo high: {}\nLevers: {}",
        guide.what, guide.too_low, guide.too_high, guide.levers
    ))
}

/// True when the bound is an advisory that does not stop an export.
pub(crate) fn is_advisory(distribution: &MetricDistribution) -> bool {
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
    /// The rows of the default view that exceed (core tone), and the cards
    /// core could not measure.
    fn of(set: &CutMetricSet, rows: &[CheckRow]) -> Self {
        Self {
            exceeded: default_view_rows(rows)
                .iter()
                .filter(|row| row.tone().exceeds())
                .count(),
            not_measured: set
                .cards
                .iter()
                .filter(|card| matches!(card.outcome, DistributionOutcome::NotMeasured(_)))
                .count(),
        }
    }

    /// The word at the right end of the section header.
    fn word(&self) -> String {
        if self.exceeded > 0 {
            format!("{} outside limit", self.exceeded)
        } else if self.not_measured > 0 {
            format!("{} not measured", self.not_measured)
        } else {
            "within limits".to_owned()
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
fn cut_metrics_view(sim: &mut SimulationState, session: &ProjectSession) -> CutMetricsView {
    let has_trace = sim
        .results
        .as_ref()
        .is_some_and(|results| results.cut_trace.is_some());
    if !has_trace {
        // U2: one remedy text for every no-trace surface.
        return CutMetricsView::Empty(MissingInput::NoCutTrace.remedy());
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
    let set = sim.cached_cut_metrics(session, toolpath_id);
    if set.cards.is_empty() {
        return CutMetricsView::Empty("No cut metric applies to this operation.");
    }
    CutMetricsView::Cards(toolpath_id, set)
}

/// The rank of a card: its place in `CUT_METRIC_ORDER`, the order of the
/// gate rows (chip, power, deflection), with the force card after the
/// deflection card (P17: one law).
fn card_rank(metric: DistributionMetric) -> usize {
    CUT_METRIC_ORDER
        .iter()
        .position(|m| *m == metric)
        .unwrap_or(usize::MAX)
}

/// The "Cut metrics" section of the focused toolpath.
///
/// The header states the summary. The charts are always open: they are the
/// part of the page the operator reads (inspector clean-up round 2). The
/// metrics core could not measure, and the metrics the cell size withheld,
/// are one table after the charts.
///
/// Each measured card has a flip button beside its chart (operator request
/// 2026-10-02). It swaps the histogram for the line of the same metric over
/// time. The face is per metric and lives in
/// `SimulationState::cut_metric_over_time`, so it outlasts a change of
/// toolpath while the app runs. The bottom time-series drawer is deleted.
/// A second button, `< >`, opens the one trace large in the trace modal
/// (`ui/sim_trace_modal.rs`).
///
/// Each card carries the core row of its gate (`limits::CheckRow`) in the
/// hover of its status glyph: the face and the detail. The rows with no
/// card, and every row of another cut group that exceeds, follow the cards
/// in "Other limits" (P29-10 10e, ruling Q5). Every row of every gate and
/// cut group is in the closed "Cut groups" drawer under them.
///
/// P17 (layout B): the four cards, the abstention table, one hairline,
/// then the width x depth chart (`sim_cut_chart`) as the drill-down, then
/// "Other limits". The cards and the chart draw from the set alone
/// ([`draw_cut_metric_cards`], [`draw_cut_chart_section`]), so a headless
/// snapshot draws them with no simulation state.
fn draw_cut_metrics_section(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &rs_cam_core::session::ProjectSession,
    view: &CutMetricsView,
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
    let row_set = sim.cached_limit_rows(session);
    let rows = rows_of(&row_set, toolpath_id);
    // "Other limits": the default-view rows that no card paints.
    let other_rows: Vec<&CheckRow> = default_view_rows(rows)
        .into_iter()
        .filter(|row| !(row.is_headline() && has_cut_metric_card(row)))
        .collect();

    // Inspector clean-up round 2: the charts are the default view. The
    // section used to start closed unless a metric exceeded its limit, and
    // its open state was one id for every toolpath, so an operation within
    // its limits showed one header line and no chart.
    let summary = CutMetricsSummary::of(set, rows);
    SectionHeader::new("Cut metrics")
        .trailing(summary.word())
        .show(ui);
    let operation = session
        .find_toolpath_config_by_id(toolpath_id)
        .map_or_else(|| "\u{2014}".to_owned(), |(_, tc)| tc.name.clone());
    DataTable::new("cut_metrics_operation")
        .row(DataRow::value("Operation", operation).hover(
            "The cards show the operation under the playhead. Click an operation \
             in the list at the left to move the playhead to it.",
        ))
        .show(ui);
    // P30-1: the feeds that an export of this operation emits now, the line
    // of the Passes tab (core `EmittedFeeds::line`).
    let emitted_feeds = session
        .find_toolpath_config_by_id(toolpath_id)
        .and_then(|(index, _)| session.emitted_feeds(index));
    crate::ui::properties::draw_emitted_feeds_line(ui, emitted_feeds.as_ref());
    let measurability = measurability_rows(sim, session, toolpath_id);

    // The playhead, as a move of this toolpath, for the line face.
    let playhead = sim
        .current_local_toolpath_move()
        .filter(|(_, id, _)| *id == toolpath_id)
        .map(|(_, _, local_move)| local_move);
    // The global move of the toolpath's first move, so the line names the
    // move the transport bar shows.
    let move_offset = sim.global_move_for_local(toolpath_id, 0).unwrap_or(0);
    let move_count = sim
        .boundary_for_toolpath_id(toolpath_id)
        .map_or(0, |b| b.end_move.saturating_sub(b.start_move));
    // G-STALECARDS: an operation with no core result was not simulated at
    // all. Its cards say to regenerate it, because a re-run cannot help.
    let regenerate_first: Option<String> = session
        .find_toolpath_config_by_id(toolpath_id)
        .filter(|(index, _)| session.result_at(*index).is_none())
        .map(|(_, tc)| tc.name.clone());
    let mut actions = draw_cut_metric_cards(
        ui,
        CutMetricsBody {
            set,
            rows,
            over_time: &sim.cut_metric_over_time,
            playhead,
            move_offset,
            regenerate_first: regenerate_first.as_deref(),
            extra_abstentions: measurability,
        },
    );
    if let Some(local_move) = draw_cut_chart_section(ui, set, playhead, move_count) {
        actions.push(CutMetricAction::Seek { local_move });
    }
    if !other_rows.is_empty() {
        SectionHeader::new("Other limits").show(ui);
        DataTable::new("cut_metrics_other_limits")
            .rows(other_rows.into_iter().map(|row| limit_data_row(row, None)))
            .show(ui);
    }
    draw_cut_groups(ui, rows, &mut sim.cut_groups_open);

    for action in actions {
        match action {
            CutMetricAction::Flip(metric) => sim.flip_cut_metric(metric),
            // `app.rs` sees the new value after this panel draws and calls
            // `AppState::open_cut_metric_trace`, which closes the other
            // modals (modal exclusivity).
            CutMetricAction::Open(metric) => sim.open_trace = Some(metric),
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

/// **The "Cut groups · n" drawer** (P29-10 10e, design 3.7 and item 6):
/// every row of every gate and cut group, the not-judged and no-sample
/// cells included, with their exemption rows in the hover. Closed by
/// default; it draws nothing while closed. `set_ui_view sim_cut_groups`
/// opens it (`open` is `SimulationState::cut_groups_open`).
pub fn draw_cut_groups(ui: &mut egui::Ui, rows: &[CheckRow], open: &mut bool) {
    if rows.is_empty() {
        return;
    }
    ui.add_space(tokens::SPACE_2);
    let _ = Details::new("sim_cut_groups")
        .label("Cut groups")
        .count(rows.len())
        .show_held(ui, open, |ui| {
            DataTable::new("sim_cut_groups_rows")
                .rows(rows.iter().map(|row| limit_data_row(row, None)))
                .show(ui);
        });
}

/// The inputs of the cards of one set, with no simulation state (P17):
/// the section builds it from the state, a headless snapshot by hand.
pub struct CutMetricsBody<'a> {
    /// The set of the focused toolpath.
    pub set: &'a CutMetricSet,
    /// The toolpath's limit rows (`ProjectSession::limit_rows`), the
    /// source of the rows the status hovers carry. Empty: no row, the
    /// cards read their histograms.
    pub rows: &'a [CheckRow],
    /// The metrics whose card shows the line over time.
    pub over_time: &'a [DistributionMetric],
    /// The playhead as a move of this toolpath, when it is in it.
    pub playhead: Option<usize>,
    /// The global move of the toolpath's first move.
    pub move_offset: usize,
    /// The name of the operation to regenerate first (G-STALECARDS).
    pub regenerate_first: Option<&'a str>,
    /// Rows that join the abstention table (the cell-size rows).
    pub extra_abstentions: Vec<DataRow>,
}

/// The cards of one set, split by hairlines, then the abstention table.
/// Measured cards first, then the cards core could not measure; inside
/// each group the cards keep the `CUT_METRIC_ORDER` order. The depth and
/// width readings sit under the force card. Returns what the cards asked.
pub fn draw_cut_metric_cards(ui: &mut egui::Ui, body: CutMetricsBody<'_>) -> Vec<CutMetricAction> {
    let set = body.set;
    let mut cards: Vec<&CutMetricCard> = set.cards.iter().collect();
    cards.sort_by_key(|card| {
        (
            matches!(card.outcome, DistributionOutcome::NotMeasured(_)),
            card_rank(card.metric),
        )
    });
    let mut actions: Vec<CutMetricAction> = Vec::new();
    // A metric core could not measure is one row of the abstention table
    // after the charts, not a card of its own.
    let mut abstentions: Vec<DataRow> = Vec::new();
    let mut charts = 0_usize;
    for card in cards {
        let row = card_row(card.metric, body.rows);
        if let DistributionOutcome::NotMeasured(reason) = &card.outcome {
            abstentions.push(abstention_row(card, reason, row, body.regenerate_first));
            continue;
        }
        if charts > 0 {
            draw_hairline(ui);
        }
        charts += 1;
        let face = if body.over_time.contains(&card.metric) {
            ChartFace::OverTime
        } else {
            ChartFace::Distribution
        };
        let view = CardView {
            face,
            playhead: body.playhead,
            move_offset: body.move_offset,
        };
        draw_cut_metric_card(ui, card, row, &view, &mut actions);
        if card.metric == DistributionMetric::CutForce {
            draw_force_readings(ui, set);
        }
    }
    abstentions.extend(body.extra_abstentions);
    if !abstentions.is_empty() {
        ui.add_space(tokens::SPACE_2);
        DataTable::new("cut_metrics_abstentions")
            .rows(abstentions)
            .show(ui);
    }
    actions
}

/// The two plain readings under the cut-force chart (P17): the depth and
/// the width of the sample set, min to max and the peak. No line, no
/// status, no hover: the depth row of `criteria()` keeps its place in
/// "Other limits". Nothing with no sample.
fn draw_force_readings(ui: &mut egui::Ui, set: &CutMetricSet) {
    let Some((depth, width)) = force_readings(set) else {
        return;
    };
    ui.add(egui::Label::new(text::caption(depth)).wrap());
    ui.add(egui::Label::new(text::caption(width)).wrap());
}

/// The readings lines: "Depth 0.25 to 2.00 mm · peak 2.00 mm" and
/// "Width 12 to 100 % of D · peak 100 %", from the min and max of the
/// sample set's `ap_mm` and `ae_mm / D`. `None` with no sample or no D.
#[must_use]
pub fn force_readings(set: &CutMetricSet) -> Option<(String, String)> {
    if set.samples.is_empty() || set.diameter_mm <= 0.0 || !set.diameter_mm.is_finite() {
        return None;
    }
    let span = |values: &mut dyn Iterator<Item = f64>| {
        values
            .filter(|v| v.is_finite())
            .fold(None, |acc: Option<(f64, f64)>, v| {
                Some(acc.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
            })
    };
    let (ap_lo, ap_hi) = span(&mut set.samples.iter().map(|s| s.ap_mm))?;
    let (ae_lo, ae_hi) = span(
        &mut set
            .samples
            .iter()
            .map(|s| s.ae_mm / set.diameter_mm * 100.0),
    )?;
    Some((
        format!(
            "Depth {} to {} mm \u{00b7} peak {} mm",
            histogram::format_value(ap_lo),
            histogram::format_value(ap_hi),
            histogram::format_value(ap_hi)
        ),
        format!(
            "Width {ae_lo:.0} to {} of D \u{00b7} peak {}",
            histogram::format_share(ae_hi),
            histogram::format_share(ae_hi)
        ),
    ))
}

/// The chart section (P17, layout B): one hairline, then the width x
/// depth chart of the set. Nothing without an envelope. Returns the move
/// a click asked the playhead to seek to.
pub fn draw_cut_chart_section(
    ui: &mut egui::Ui,
    set: &CutMetricSet,
    playhead: Option<usize>,
    move_count: usize,
) -> Option<usize> {
    let env = set.envelope.as_deref()?;
    draw_hairline(ui);
    sim_cut_chart::draw_cut_chart(
        ui,
        &CutChartInput {
            env,
            samples: &set.samples,
            diameter_mm: set.diameter_mm,
            budget_n: set.budget_n,
            force_plane: set.force_plane.as_ref(),
            playhead,
            move_count,
        },
    )
    .seek
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
/// It shows state only. It starts no work and it changes no recording: every
/// run captures the cutting metrics, and the workspace primary is the only
/// run route.
fn draw_cut_metrics_empty(ui: &mut egui::Ui, reason: &str) {
    crate::ui::components::SectionHeader::new("Cut metrics").show(ui);
    ui.horizontal_wrapped(|ui| {
        ui.add(NotMeasured::new().reason(reason));
        ui.label(text::caption(reason));
    });
}

/// How one card draws, beside the card's own data.
struct CardView {
    /// The face the card shows.
    face: ChartFace,
    /// The playhead as a move of this toolpath, when it is in it.
    playhead: Option<usize>,
    /// The global move of the toolpath's first move.
    move_offset: usize,
}

/// One metric: a header row, the chart, and a caption only when some
/// cutting time is out of band.
///
/// The chart is the histogram or the line over time, as `view.face` says.
/// The header row holds the title, the (i) guide, the status glyph, the
/// peak, the flip button and the open button. The two buttons sit at the
/// right end of the row, directly above the chart's right edge. The glyph's
/// hover carries the gate's core row: its face and its detail
/// (`cut_metric_status_hover`).
/// A metric core could not measure draws one muted line with the
/// abstention mark and core's reason.
fn draw_cut_metric_card(
    ui: &mut egui::Ui,
    card: &CutMetricCard,
    row: Option<&CheckRow>,
    view: &CardView,
    actions: &mut Vec<CutMetricAction>,
) {
    let spec = card.metric.text();
    let hover = cut_metric_status_hover(card, row, &spec);
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new(spec.title).color(tokens::TEXT_STRONG)).truncate(),
        );
        if let Some(guide) = cut_metric_guide(card.metric) {
            ui.label(egui::RichText::new(tokens::GLYPH_DETAIL).color(tokens::TEXT_MUTED))
                .on_hover_text(guide);
        }
        if let DistributionOutcome::Measured(distribution) = &card.outcome {
            let (glyph, colour) = status_glyph(distribution, row);
            ui.add(egui::Label::new(egui::RichText::new(glyph).color(colour)).truncate())
                .on_hover_text(hover.clone());
            let peak = peak_text(distribution, row, &spec);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The line face and the modal need samples to draw. A card
                // with none keeps the histogram and offers neither button.
                if !card.series.is_empty() {
                    if ui.add(TraceOpen).clicked() {
                        actions.push(CutMetricAction::Open(card.metric));
                    }
                    if ui.add(ChartFlip::new(view.face)).clicked() {
                        actions.push(CutMetricAction::Flip(card.metric));
                    }
                }
                if let Some(peak) = peak {
                    ui.add(egui::Label::new(text::caption(peak)).truncate());
                }
            });
        }
    });
    match &card.outcome {
        DistributionOutcome::Measured(distribution) => {
            let over_time = view.face == ChartFace::OverTime && !card.series.is_empty();
            let clicked_move = if over_time {
                Sparkline::new(&card.series, &distribution.histogram, spec.unit)
                    .scale(spec.scale)
                    .advisory(is_advisory(distribution))
                    .playhead(view.playhead)
                    .move_offset(view.move_offset)
                    .show(ui)
                    .clicked_move
            } else {
                let chart = DistributionChart::new(&distribution.histogram, spec.unit)
                    .scale(spec.scale)
                    .advisory(is_advisory(distribution))
                    .show(ui);
                chart
                    .clicked_bin
                    .and_then(|bin| distribution.histogram.first_move_per_bin.get(bin))
                    .copied()
                    .flatten()
            };
            if let Some(local_move) = clicked_move {
                actions.push(CutMetricAction::Seek { local_move });
            }
            if let Some(caption) = card_caption(distribution, &spec) {
                ui.add(egui::Label::new(text::caption(caption)).wrap());
            }
        }
        // The section draws a metric core could not measure as one row of
        // its abstention table (`abstention_row`).
        DistributionOutcome::NotMeasured(_) => {}
    }
}

/// One row of the abstention table: the metric, the abstention mark, and
/// the status hover with core's reason after it.
///
/// The hover opens with the gate's core row, the way a measured card's
/// status hover does (G-OWNBOUND).
fn abstention_row(
    card: &CutMetricCard,
    reason: &NotMeasuredReason,
    row: Option<&CheckRow>,
    regenerate_first: Option<&str>,
) -> DataRow {
    let spec = card.metric.text();
    let reason = match (reason, regenerate_first) {
        (NotMeasuredReason::Unmodeled(UnmodeledReason::StaleSimulation), Some(name)) => {
            format!("Unmodeled: '{name}' is not generated — regenerate it first")
        }
        _ => not_measured_text(reason),
    };
    let hover = cut_metric_status_hover(card, row, &spec);
    DataRow::not_measured(spec.title).hover(format!("{hover}\n{reason}"))
}

/// The rows that say the cell size withheld a metric of `toolpath_id`.
///
/// The cards do not read the measurability report: a gate abstains on its
/// own reason. The report names the metrics that the dexel grid cannot
/// resolve at this cell, so a chart the operator expects is either absent
/// or degraded for a reason a card does not state. One row per metric that
/// is `NotMeasurable` or `Degraded`.
///
/// No button sets a finer cell. Core does not publish a per-tool cell for
/// this case (the closed form needs the tip radius, which the trace does
/// not carry), and a number typed here would be invented.
fn measurability_rows(
    sim: &mut SimulationState,
    session: &ProjectSession,
    toolpath_id: ToolpathId,
) -> Vec<DataRow> {
    use rs_cam_core::stock::sim_measurability::{Measurability, MeasurabilityReason, SimMetric};
    let report = &sim.cached_simulation_triage(session).measurability;
    [
        SimMetric::RadialEngagement,
        SimMetric::ChipEngagement,
        SimMetric::AirCut,
    ]
    .into_iter()
    .filter_map(|metric| {
        let (role, word, reason) = match report.for_metric(toolpath_id, metric) {
            Measurability::Measurable => return None,
            Measurability::Degraded(reason) => (Role::Caution, "degraded", reason),
            Measurability::NotMeasurable(reason) => (Role::Unknown, "not measurable", reason),
        };
        let word = match reason {
            MeasurabilityReason::CellTooCoarseForTipContact {
                cell_mm: Some(cell),
                ..
            } => format!("{word} at {cell:.2} mm cell"),
            MeasurabilityReason::BelowFreshMaterialFloor { .. }
            | MeasurabilityReason::CellTooCoarseForTipContact { .. }
            | MeasurabilityReason::KinematicsNotModelled => word.to_owned(),
        };
        let fix = match reason {
            MeasurabilityReason::CellTooCoarseForTipContact { .. } => {
                "\nA finer cell can fix this: Setup & run \u{25B8} Resolution, in the \
                 list at the left."
            }
            MeasurabilityReason::BelowFreshMaterialFloor { .. }
            | MeasurabilityReason::KinematicsNotModelled => "",
        };
        let name = capitalised(metric.label());
        let hover = format!("{}{fix}", reason.describe());
        // Round 3: a metric the cell withheld reads "N/A", and the reason
        // ("not measurable at 0.20 mm cell") is the first hover line. A
        // degraded metric was measured, so its word is the value.
        Some(match role {
            Role::Caution => DataRow::value(name, word).tone(role.text()).hover(hover),
            Role::Ok | Role::Danger | Role::Info | Role::Unknown => {
                DataRow::not_measured(name).status(role, word).hover(hover)
            }
        })
    })
    .collect()
}

/// `text` with its first letter in upper case: core's metric labels are
/// lower case, and a row name starts with a capital.
fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The status glyph of a measured card, and its colour.
///
/// The core row picks the colour (`role_of_tone`, the one map) and the
/// glyph; the in-band share of the histogram never does. Within reads `✓`.
/// A row that exceeds reads `✕` and the out-of-band share. A row with no
/// number reads "no limit" when the histogram has no bound, else the
/// abstention glyph. The force card has no row (the force reading is
/// report-only, `P29-10_DESIGN.md` 13.3): it reads its histogram against
/// the budget line, in the caution colour of an advisory bound.
fn status_glyph(
    distribution: &MetricDistribution,
    row: Option<&CheckRow>,
) -> (String, egui::Color32) {
    let hist = &distribution.histogram;
    let unbounded = hist.floor.is_none() && hist.ceiling.is_none();
    let out_share = |share: f64| {
        let out = (1.0 - share).max(0.0);
        if out > 0.0 {
            format!(
                "{} {}",
                tokens::GLYPH_DANGER,
                histogram::format_share(out * 100.0)
            )
        } else {
            tokens::GLYPH_DANGER.to_owned()
        }
    };
    match row {
        Some(row) => {
            let role = role_of_tone(row.tone());
            if row.tone().exceeds() {
                let text = hist
                    .in_band_share()
                    .map_or_else(|| tokens::GLYPH_DANGER.to_owned(), out_share);
                return (text, role.text());
            }
            if role == Role::Ok {
                return (tokens::GLYPH_OK.to_owned(), role.text());
            }
            if unbounded {
                // Feeds ruling R2: a reading with no bound gets no invented
                // one; it states the absence.
                return ("no limit".to_owned(), tokens::TEXT_MUTED);
            }
            (tokens::GLYPH_UNKNOWN.to_owned(), role.text())
        }
        None => {
            if unbounded {
                return ("no limit".to_owned(), tokens::TEXT_MUTED);
            }
            let Some(share) = hist.in_band_share() else {
                return (tokens::GLYPH_UNKNOWN.to_owned(), tokens::UNKNOWN);
            };
            if hist.above_s > 0.0 || hist.below_s > 0.0 {
                let colour = if is_advisory(distribution) {
                    tokens::CAUTION
                } else {
                    tokens::DANGER
                };
                (out_share(share), colour)
            } else {
                (tokens::GLYPH_OK.to_owned(), tokens::OK)
            }
        }
    }
}

/// The muted peak at the right of the header row: the core margin face of
/// the gate row ("peak 87 % of limit"), or the peak in its unit when the
/// row has no margin. The force card (no row) states its peak against the
/// budget line of its histogram.
fn peak_text(
    distribution: &MetricDistribution,
    row: Option<&CheckRow>,
    spec: &rs_cam_core::tool_load::MetricText,
) -> Option<String> {
    let unit_peak = |peak: f64| {
        format!(
            "peak {} {}",
            histogram::format_value(peak * spec.scale),
            spec.unit
        )
    };
    match row {
        Some(row) => Some(
            match rs_cam_core::limits::margin_face(row.check(), row.compared()) {
                Some(face) => format!("peak {face}"),
                None => unit_peak(row.check().value?),
            },
        ),
        None => {
            // The population maximum is the last histogram edge (the
            // overflow bin's upper edge, core's layout).
            let peak = *distribution.histogram.edges.last()?;
            // P29-10 10g (item 4b): the force card holds no row (the force
            // reading is report-only, ruling F6), so this is its one writer;
            // "peak 387 % of budget" clipped at the rail width, and the card
            // title and its histogram already say the value is the peak.
            Some(match distribution.histogram.ceiling.filter(|b| *b > 0.0) {
                Some(budget) => format!(
                    "{} of budget",
                    histogram::format_share(peak / budget * 100.0)
                ),
                None => unit_peak(peak),
            })
        }
    }
}

/// The hover of a card's status glyph (G-OWNBOUND): the core row of the
/// gate (its face, then its detail), the in-band share of cutting time,
/// and for the force card (no row) its peak against the budget.
fn cut_metric_status_hover(
    card: &CutMetricCard,
    row: Option<&CheckRow>,
    spec: &rs_cam_core::tool_load::MetricText,
) -> String {
    let distribution = match &card.outcome {
        DistributionOutcome::Measured(distribution) => Some(distribution.as_ref()),
        DistributionOutcome::NotMeasured(_) => None,
    };
    let unbounded =
        distribution.is_some_and(|d| d.histogram.floor.is_none() && d.histogram.ceiling.is_none());
    let mut lines: Vec<String> = Vec::new();
    match row {
        Some(row) => lines.push(row.face().to_owned()),
        None => match (card.metric, distribution) {
            // P17: the force reading names its peak and its rule (one law
            // with the deflection gate; report-only).
            (DistributionMetric::CutForce, Some(d)) => {
                let peak = histogram::format_value(
                    d.histogram.edges.last().copied().unwrap_or(0.0) * spec.scale,
                );
                lines.push(match d.histogram.ceiling {
                    Some(budget) => format!(
                        "Cut force: peak {peak} N against the {RULE_FORCE_BUDGET}, {} N",
                        histogram::format_value(budget * spec.scale)
                    ),
                    None => format!("Cut force: peak {peak} N \u{00b7} {RULE_NO_LIMIT}"),
                });
            }
            (DistributionMetric::CutForce, None)
            | (DistributionMetric::Criterion(_) | DistributionMetric::Engagement, _) => {
                lines.push("No gate judges this metric. Use it to compare cuts.".to_owned());
            }
        },
    }
    if let Some(distribution) = distribution {
        let hist = &distribution.histogram;
        if unbounded {
            // The reading has no band share to state.
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
            if is_advisory(distribution) {
                lines.push(
                    "This limit is the machine force budget. It does not stop an export."
                        .to_owned(),
                );
            }
        }
    }
    if let Some(row) = row {
        lines.push(row_hover(row));
    }
    lines.join("\n")
}

/// The caption under the chart: each out-of-band share with its one-word
/// consequence. `None` when no cutting time is out of band.
fn card_caption(
    distribution: &MetricDistribution,
    spec: &rs_cam_core::tool_load::MetricText,
) -> Option<String> {
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
    // P17 (lead ruling 3): a force reading with no line says so.
    if distribution.metric == DistributionMetric::CutForce && hist.ceiling.is_none() {
        parts.push(RULE_NO_LIMIT.to_owned());
    }
    (!parts.is_empty()).then(|| parts.join(" \u{00b7} "))
}

// ── Selected span section ───────────────────────────────────────────────

/// The chip text of one span: core's payload text, else the kind's GUI
/// name and the span id (F074). A chip reads the payload, not the set
/// label, so a depth-pass chip keeps its Z.
fn span_display_label(span: &Span, sid: usize) -> String {
    span.payload_label()
        .unwrap_or_else(|| format!("{} [{sid}]", span.kind.display_label()))
}

/// Find the deepest non-boundary span on `toolpath_id` that contains the
/// playhead's local move. Used to drive the Span section when the user
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
    let result = rt.run.result()?;
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

/// Span scope (pass2 inspector §2.3-C) — "what's in the span I picked?". On
/// the Navigate tab: a "Span" header that names the span, the in-panel lock
/// toggle (INS-008), the span facts, the per-span metrics, the in-span
/// findings as jump rows, then the nested "Generator item" and "Generation
/// trace" drill-downs (§2.4). The tab is the disclosure, so the block is
/// open (inspector clean-up round 3).
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
            .and_then(|rt| rt.run.result())
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
    // Round 3: the Navigate tab is the disclosure, so the span block is open.
    SectionHeader::new("Span")
        .trailing(format!("{header_label}{lock_glyph}"))
        .show(ui);
    // In-panel lock toggle (INS-008) — the panel's own entry point to
    // the span lock, writing the same span_scope field the ribbon does.
    ui.horizontal(|ui| {
        let (lbl, hover) = if locked_span_id.is_some() {
            ("\u{1F512} locked", "Click to follow the playhead again.")
        } else {
            (
                "\u{1F513} follow",
                "Click to lock the Span section to the current span.",
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
}

/// The Span section body: span facts, the per-span metrics grid, and the
/// in-span findings list. Split out of `draw_span_section` so the lock toggle
/// and the nested drill-downs can sit around it.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
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
    let Some(result) = rt.run.result() else {
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

    // Span-level facts and the sample aggregates, one table. The aggregates
    // come from the per-trace cache, which builds on the first frame of a
    // new sim run and is reused on every frame after. `agg` is `None` when
    // no cutting samples landed in this span.
    let span_row = DataRow::value(
        "Span",
        format!(
            "{} · moves {}–{}",
            span.kind.label(),
            span.start_move,
            span.end_move
        ),
    );
    let agg = sim.debug.span_aggregates.get(tp_id, sid).copied();
    match agg {
        Some(agg) if agg.n_cutting > 0 => {
            DataTable::new(("selected_metrics_table", sid))
                .row(span_row)
                .row(DataRow::value(
                    "Samples",
                    format!("{} ({} cutting)", agg.n_samples, agg.n_cutting),
                ))
                // Engagement as percent everywhere (INS-004) + provenance
                // hover (INS-006): comparative signal, not an absolute bar.
                .row(
                    DataRow::value(
                        "Engagement",
                        format!(
                            "avg {:.0}% · peak {:.0}%",
                            agg.avg_engagement() * 100.0,
                            agg.peak_eng * 100.0
                        ),
                    )
                    .hover(ENGAGEMENT_PROVENANCE_HOVER),
                )
                .row(
                    DataRow::value(
                        rs_cam_core::feeds::ACHIEVED_ADVANCE_PER_TOOTH,
                        format!(
                            "avg {:.4} · peak {:.4}",
                            agg.avg_advance_per_tooth(),
                            agg.peak_advance
                        ),
                    )
                    .unit("mm/tooth")
                    .hover(ACHIEVED_ADVANCE_HOVER),
                )
                .row(
                    DataRow::value(
                        rs_cam_core::feeds::ARC_MEAN_CHIP_THICKNESS,
                        format!(
                            "avg {:.4} · peak {:.4}",
                            agg.avg_chip_thickness(),
                            agg.peak_chip
                        ),
                    )
                    .unit("mm")
                    .hover(ARC_MEAN_CHIP_HOVER),
                )
                .row(DataRow::value("Axial DOC", format!("peak {:.2}", agg.peak_doc)).unit("mm"))
                .row(
                    DataRow::value(
                        "MRR",
                        format!("avg {:.0} · peak {:.0}", agg.avg_mrr(), agg.peak_mrr),
                    )
                    .unit("mm³/s"),
                )
                .show(ui);
        }
        _ => {
            DataTable::new(("selected_metrics_table", sid))
                .row(span_row)
                .row(DataRow::not_measured("Samples").hover("No cutting samples in this span."))
                .show(ui);
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
    let hotspots: Vec<_> = in_scope_hotspots.iter().take(MAX_ROWS).collect();
    let more_hotspots = in_scope_hotspots.len().saturating_sub(MAX_ROWS);
    let clicked = DataTable::new(("span_hotspots", sid))
        .rows(hotspots.iter().map(|(_, h)| {
            DataRow::record(format!(
                "Hotspot · {}",
                hotspot_summary_line(
                    h.move_start,
                    h.wasted_runtime_s,
                    h.peak_chipload_mm_per_tooth
                )
            ))
            .tone(crate::ui::tokens::CAUTION)
            .hover("Click to focus and jump to start move.")
            .clickable()
        }))
        .row_opt(
            (more_hotspots > 0)
                .then(|| DataRow::record(format!("\u{2026} +{more_hotspots} more hotspots"))),
        )
        .show(ui)
        .clicked;
    if let Some((h_idx, h)) = clicked.and_then(|index| hotspots.get(index)) {
        let global_start = sim
            .global_move_for_local(tp_id, h.move_start)
            .unwrap_or(h.move_start);
        sim.debug.focused_hotspot = Some((tp_id, *h_idx));
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: global_start,
        })));
    }
    let shown_issues: Vec<_> = in_scope_issues.iter().take(MAX_ROWS).collect();
    let more_issues = in_scope_issues.len().saturating_sub(MAX_ROWS);
    let clicked = DataTable::new(("span_issues", sid))
        .rows(shown_issues.iter().map(|iss| {
            DataRow::record(format!(
                "{} · m{} · {}",
                issue_kind_label(iss.kind),
                iss.move_index,
                iss.label
            ))
            .tone(theme::WARNING_TEXT)
            .hover("Click to jump to issue.")
            .clickable()
        }))
        .row_opt(
            (more_issues > 0)
                .then(|| DataRow::record(format!("\u{2026} +{more_issues} more issues"))),
        )
        .show(ui)
        .clicked;
    if let Some(iss) = clicked.and_then(|index| shown_issues.get(index)) {
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: iss.move_index,
        })));
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
                DataTable::new("sim_selection_details_table")
                    .row(DataRow::value(
                        "Kind",
                        semantic_kind_label(&active.item.kind),
                    ))
                    .row_opt(active.item.xy_bbox.map(|bounds| {
                        DataRow::value(
                            "XY",
                            format!(
                                "{:.2}, {:.2} → {:.2}, {:.2}",
                                bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y
                            ),
                        )
                    }))
                    .row_opt(
                        active
                            .item
                            .z_min
                            .zip(active.item.z_max)
                            .map(|(z_min, z_max)| {
                                DataRow::value("Z", format!("{z_min:.3} → {z_max:.3}"))
                            }),
                    )
                    .rows(
                        active
                            .item
                            .params
                            .values
                            .iter()
                            .take(6)
                            .map(|(key, value)| {
                                DataRow::value(key.clone(), format_json_value(value))
                            }),
                    )
                    .show(ui);
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

            let linked = linked_span
                .as_ref()
                .filter(|(toolpath_id, _)| Some(*toolpath_id) == current_boundary_id)
                .map(|(_, span)| span);
            let mut table = DataTable::new("inspector_generation_trace_table");
            match debug_trace {
                Some(trace) => {
                    table = table
                        .row(DataRow::measured(
                            "Total",
                            format!("{:.1}", trace.summary.total_elapsed_us as f64 / 1000.0),
                            "ms",
                        ))
                        .row_opt(trace.summary.dominant_span_label.as_ref().map(|label| {
                            DataRow::value(
                                "Dominant",
                                format!(
                                    "{label} ({:.1} ms)",
                                    trace.summary.dominant_span_elapsed_us.unwrap_or_default()
                                        as f64
                                        / 1000.0
                                ),
                            )
                        }))
                        .row(DataRow::value("Hotspots", trace.hotspots.len().to_string()))
                        .row_opt(linked.map(|span| {
                            DataRow::value(
                                "Linked span",
                                format!(
                                    "{} ({:.1} ms)",
                                    span.label,
                                    span.elapsed_us as f64 / 1000.0
                                ),
                            )
                            .hover_opt(debug_span_math_summary(&span.kind))
                        }))
                        .row_opt(
                            annotation
                                .as_ref()
                                .map(|label| DataRow::value("Annotation", label.clone())),
                        );
                }
                None => {
                    table = table.row(
                        DataRow::not_measured("Performance trace")
                            .hover("No performance trace available."),
                    );
                }
            }
            table
                .row_opt(semantic_trace.map(|semantic| {
                    DataRow::value(
                        "Semantic items",
                        format!(
                            "{} (move-linked {})",
                            semantic.summary.item_count, semantic.summary.move_linked_item_count
                        ),
                    )
                }))
                .show(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// W3 (G-WLAIRLABEL): the air-cut row names the skin on its own row,
    /// with the WOC words on the hover.
    /// The D69 Corne Waterline split of the W3 core test: 84.8 % air, of
    /// which 83.15 % removes material (279.3 s of 335.9 s). With no skin
    /// the row reads as before. The row fits the 304-point Simulation
    /// inspector (288 points inside) with the label, and does not elide.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
    )]
    fn the_air_row_names_the_skin_w3() {
        let share = 279.3 / 335.9;
        let value = air_cut_value(84.8, share);
        assert_eq!(value, "14% (+71% skin)");
        assert!(air_skin_note(84.8, share).contains("71% of the total runtime"));
        assert!(air_skin_note(84.8, share).contains("under 2% WOC"));
        assert_eq!(air_cut_value(12.4, 0.0), "12% of total runtime");
        assert_eq!(air_skin_note(12.4, 0.0), "");

        let ctx = egui::Context::default();
        crate::ui::tokens::apply(&ctx);
        crate::ui::tokens::apply_fonts(&ctx);
        let mut texts: Vec<(String, bool, usize)> = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(288.0);
                DataTable::new("air_row_fit")
                    .rows([DataRow::value("Air cut", value.clone())])
                    .show(ui);
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some((
                        t.galley.job.text.clone(),
                        t.galley.elided,
                        t.galley.rows.len(),
                    )),
                    _ => None,
                })
                .collect();
            out.textures_delta.clear();
        }
        let row = texts.iter().find(|(t, _, _)| *t == value);
        assert!(
            row.is_some_and(|(_, elided, rows)| !elided && *rows == 1),
            "the value paints on one row, not elided: {texts:?}"
        );
    }

    /// A card that core could not measure for want of an arc names the
    /// re-run, not a control that does not exist (the capture checkbox is
    /// deleted, operator ruling 2026-10-02).
    #[test]
    fn arc_engagement_unmodeled_tooltip_says_re_run() {
        let text = not_measured_text(&NotMeasuredReason::Unmodeled(
            UnmodeledReason::ArcEngagementNotCaptured,
        ));
        assert!(text.contains("Re-run Simulation"), "{text}");
    }

    /// A toolpath whose every criterion says "does not apply" — a direct
    /// `evaluate_toolpath` call leaves `drill_gates` as `None` on a drill op,
    /// so this state is reachable. Its rows are all `NotApplicable`: the
    /// default view takes none, and the Metrics tab then draws no row (a
    /// blank Readiness list states the absence in words).
    #[test]
    fn a_verdict_with_no_applicable_limit_paints_no_row() {
        use rs_cam_core::tool_load::verdict::{
            ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
        };
        let na = || {
            UnmodeledReason::NotApplicableForOp("drill cycle — no continuous engagement".to_owned())
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: rs_cam_core::ToolpathId::from_raw(0),
            groups: rs_cam_core::tool_load::verdict::GroupVerdicts::headline_only_for_tests(
                rs_cam_core::limits::CutGroup::Body,
            ),
            chipload: ChiploadVerdict::Unmodeled { reason: na() },
            power: PowerVerdict::Unmodeled { reason: na() },
            deflection: DeflectionVerdict::Unmodeled { reason: na() },
            depth: DepthVerdict::Unmodeled { reason: na() },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
            material_split: None,
            cut_force: None,
        };
        let rows = rs_cam_core::limits::rows_of_verdict(&verdict);
        let headline: Vec<&CheckRow> = rows.iter().filter(|r| r.is_headline()).collect();
        assert!(!headline.is_empty(), "non-vacuity: the headline rows exist");
        assert!(
            default_view_rows(&rows)
                .iter()
                .all(|r| r.tone() != rs_cam_core::limits::Tone::NotApplicable),
            "a criterion that does not apply is not a limit that was not \
             measured, and it must not take a default-view row"
        );
    }

    /// U3: the `SimulationRequired` row names the missing cut trace. It never
    /// says "run a simulation first" or "has not been run": the Inspector
    /// draws it while a simulation exists. It names no capture control,
    /// because every run captures the trace (operator ruling 2026-10-02).
    #[test]
    fn a_simulation_required_row_names_the_trace_not_a_missing_run() {
        let text = unmodeled_text(Some(&UnmodeledReason::SimulationRequired));
        let lower = text.to_lowercase();
        assert!(!lower.contains("simulation first"), "{text}");
        assert!(!lower.contains("has not been run"), "{text}");
        assert!(text.contains("no cut trace"), "{text}");
        assert!(text.contains("run the simulation"), "{text}");
        assert!(!text.contains("Setup & run"), "{text}");
    }
}
