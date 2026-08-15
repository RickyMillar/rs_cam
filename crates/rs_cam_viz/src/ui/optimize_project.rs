//! Project-level Optimize rollup — U3 of OPTIMIZER_UX_PLAN.md.
//!
//! Surfaces the result of `optimize_project` as one rollup window
//! anchored at the screen centre. Header shows baseline vs optimized
//! cycle time (with a `+N not estimated` note for skipped rows and a
//! baseline-provenance line). W3.5/OPT-004 re-groups the grid by what
//! the operator can do: **Apply now** (Ranked + safe candidate —
//! checkbox-only column), **Needs your call** (TradeOff / MarginalSafe
//! — role chip + a Review button that opens the per-toolpath modal,
//! OPT-002), and a collapsed **Not optimized** disclosure (no-safe /
//! skipped — full narrative behind a per-row expander, no inline
//! truncation).

use rs_cam_core::tool_load::optimize::{
    OptimizeCandidate, OptimizeOutcome, OutcomeKind, ParamDelta, ProjectOptimizeReport,
};

use super::components::FreshnessGate;
use super::optimize_modal::narrative_prose;
use super::{AppEvent, theme};
use crate::state::{AppState, OptimizeProjectState, OptimizeProjectStatus};

/// Draw the Optimize-project rollup if `state.optimize_project` is set.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(view) = state.optimize_project.as_ref() else {
        return;
    };
    let mut still_open = true;
    egui::Window::new("Optimize project")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(720.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_view(ui, state, view, events);
        });

    if !still_open {
        events.push(AppEvent::CloseOptimizeProject);
    }
}

fn draw_view(
    ui: &mut egui::Ui,
    state: &AppState,
    view: &OptimizeProjectState,
    events: &mut Vec<AppEvent>,
) {
    // W0.5/OPT-003 — the baseline comes from the sim snapshot taken at
    // launch; if that sim was already stale the optimized-vs-baseline
    // numbers are computed from out-of-date stock. Flag it instead of
    // presenting the figures as current.
    if state.simulation.is_stale(state.gui.edit_counter) {
        FreshnessGate::banner(ui);
        ui.add_space(4.0);
    }
    match &view.status {
        OptimizeProjectStatus::Loading => draw_loading(ui, events),
        OptimizeProjectStatus::Failed(msg) => draw_failed(ui, msg, events),
        OptimizeProjectStatus::Ready(report) => {
            draw_ready(ui, state, report, &view.row_selected, events);
        }
        OptimizeProjectStatus::Reconciling(report) => {
            draw_reconciling(ui, state, report, events);
        }
        OptimizeProjectStatus::Reconciled(report) => {
            draw_reconciled(ui, state, report, events);
        }
    }
}

fn draw_loading(ui: &mut egui::Ui, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(egui::RichText::new("Optimising project…").strong());
    });
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            "Stage 0/1/2 search across every enabled toolpath. Expect 3–10 minutes \
             on a wanaka-sized job. The GUI is responsive — click Cancel to stop \
             and discard the partial run.",
        )
        .small()
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(8.0);
    if ui.button("Cancel").clicked() {
        events.push(AppEvent::CloseOptimizeProject);
    }
}

fn draw_failed(ui: &mut egui::Ui, msg: &str, events: &mut Vec<AppEvent>) {
    ui.label(
        egui::RichText::new("Optimize failed")
            .strong()
            .color(theme::ERROR),
    );
    ui.add_space(4.0);
    ui.label(egui::RichText::new(msg).small());
    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        events.push(AppEvent::CloseOptimizeProject);
    }
}

fn draw_ready(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ProjectOptimizeReport,
    row_selected: &[bool],
    events: &mut Vec<AppEvent>,
) {
    draw_header(ui, report, row_selected);
    ui.add_space(6.0);
    if let Some(bottleneck_idx) = report.bottleneck_index {
        draw_bottleneck_callout(ui, state, report, bottleneck_idx);
        ui.add_space(6.0);
    }
    ui.separator();
    ui.add_space(4.0);

    if report.per_toolpath.is_empty() {
        ui.label(
            egui::RichText::new("No enabled toolpaths to optimise.")
                .small()
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(8.0);
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeProject);
        }
        return;
    }

    // OPT-004: bucket rows by what the operator can actually do, so
    // "apply this", "decide on this", and "nothing to apply" never
    // share one unlabeled column. `row_idx` is preserved so the
    // Apply-now checkbox still indexes `row_selected` correctly.
    let mut apply_now: Vec<(usize, usize, &OptimizeOutcome)> = Vec::new();
    let mut needs_call: Vec<(usize, &OptimizeOutcome)> = Vec::new();
    let mut not_optimized: Vec<(usize, &OptimizeOutcome)> = Vec::new();
    for (row_idx, (tp_index, outcome)) in report.per_toolpath.iter().enumerate() {
        match outcome.kind {
            OutcomeKind::Ranked if outcome.first_safe().is_some() => {
                apply_now.push((row_idx, *tp_index, outcome));
            }
            OutcomeKind::TradeOff | OutcomeKind::MarginalSafe => {
                needs_call.push((*tp_index, outcome));
            }
            _ => not_optimized.push((*tp_index, outcome)),
        }
    }

    egui::ScrollArea::vertical()
        .max_height(420.0)
        .show(ui, |ui| {
            if !apply_now.is_empty() {
                draw_section_heading(ui, "APPLY NOW");
                egui::Grid::new("optimize_apply_now_grid")
                    .num_columns(5)
                    .spacing([8.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Apply").small().strong());
                        ui.label(egui::RichText::new("toolpath").small().strong());
                        ui.label(egui::RichText::new("change").small().strong());
                        ui.label(egui::RichText::new("−cycle").small().strong());
                        ui.label(egui::RichText::new("verdict").small().strong());
                        ui.end_row();
                        for (row_idx, tp_index, outcome) in &apply_now {
                            draw_apply_now_row(
                                ui,
                                state,
                                *row_idx,
                                *tp_index,
                                outcome,
                                row_selected,
                                events,
                            );
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
            }

            if !needs_call.is_empty() {
                draw_section_heading(ui, "NEEDS YOUR CALL");
                egui::Grid::new("optimize_needs_call_grid")
                    .num_columns(4)
                    .spacing([8.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("role").small().strong());
                        ui.label(egui::RichText::new("toolpath").small().strong());
                        ui.label(egui::RichText::new("note").small().strong());
                        ui.label(egui::RichText::new("action").small().strong());
                        ui.end_row();
                        for (tp_index, outcome) in &needs_call {
                            draw_needs_call_row(ui, state, *tp_index, outcome, events);
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
            }

            if !not_optimized.is_empty() {
                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("Not optimized ({})", not_optimized.len())).small(),
                )
                .id_salt("optimize_not_optimized")
                .default_open(false)
                .show(ui, |ui| {
                    for (tp_index, outcome) in &not_optimized {
                        draw_not_optimized_row(ui, state, *tp_index, outcome);
                    }
                });
            }
        });

    ui.add_space(8.0);
    let any_safe_selected = report
        .per_toolpath
        .iter()
        .zip(row_selected.iter())
        .any(|((_, outcome), &sel)| sel && outcome.first_safe().is_some());
    ui.horizontal(|ui| {
        let apply_btn = ui.add_enabled(any_safe_selected, egui::Button::new("Apply selected"));
        if apply_btn.clicked() {
            events.push(AppEvent::ApplyOptimizeProject);
        }
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeProject);
        }
    });
}

/// Small caps-style section heading for the role-grouped rollup tables.
fn draw_section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .strong()
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(2.0);
}

fn draw_header(ui: &mut egui::Ui, report: &ProjectOptimizeReport, row_selected: &[bool]) {
    let baseline = report.baseline_cycle_time_s;
    let optimized = report.optimized_cycle_time_s(row_selected);
    let saving = baseline - optimized;
    let pct = if baseline > 0.0 {
        100.0 * saving / baseline
    } else {
        0.0
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Current").small());
        ui.label(egui::RichText::new(format_cycle(baseline)).strong());
        ui.add_space(16.0);
        ui.label(egui::RichText::new("Optimized").small());
        ui.label(egui::RichText::new(format_cycle(optimized)).strong());
        if saving > 0.5 {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("(-{}, -{:.0}%)", format_cycle(saving), pct))
                    .small()
                    .color(theme::SUCCESS),
            );
        }
    });

    // OPT-001: Skipped rows carry no candidate baseline and are excluded
    // from both the baseline and optimized sums (see
    // `optimized_cycle_time_s`). Surface them explicitly so the headline
    // never silently drops their time.
    let not_estimated = report
        .per_toolpath
        .iter()
        .filter(|(_, o)| matches!(o.kind, OutcomeKind::Skipped))
        .count();
    if not_estimated > 0 {
        let plural = if not_estimated == 1 { "" } else { "s" };
        ui.label(
            egui::RichText::new(format!(
                "+ {not_estimated} toolpath{plural} not estimated (skipped)"
            ))
            .small()
            .color(theme::TEXT_MUTED),
        );
    }

    // OPT-003: baseline provenance. Staleness (params changed since the
    // sim these numbers came from) is flagged by the FreshnessGate banner
    // at the top of the view; this line states the source when fresh.
    ui.label(
        egui::RichText::new("baseline: current simulation")
            .small()
            .color(theme::TEXT_DIM),
    );
}

fn draw_bottleneck_callout(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ProjectOptimizeReport,
    bottleneck_index: usize,
) {
    let name = state
        .session
        .toolpath_configs()
        .get(bottleneck_index)
        .map_or_else(|| format!("idx {bottleneck_index}"), |tc| tc.name.clone());
    // Find the bottleneck row's baseline cycle for the percentage.
    let baseline = report.baseline_cycle_time_s;
    let bottleneck_cycle = report
        .per_toolpath
        .iter()
        .find(|(idx, _)| *idx == bottleneck_index)
        .and_then(|(_, outcome)| match outcome.kind {
            OutcomeKind::Ranked | OutcomeKind::MarginalSafe => {
                outcome.candidates.first().map(|c| c.cycle_time_s)
            }
            _ => None,
        })
        .unwrap_or(0.0);
    let pct = if baseline > 0.0 {
        100.0 * bottleneck_cycle / baseline
    } else {
        0.0
    };
    ui.label(
        egui::RichText::new(format!("Bottleneck: {name}  ({pct:.0}% of runtime)"))
            .strong()
            .color(theme::WARNING),
    );
}

fn toolpath_name(state: &AppState, toolpath_index: usize) -> String {
    state
        .session
        .toolpath_configs()
        .get(toolpath_index)
        .map_or_else(|| format!("idx {toolpath_index}"), |tc| tc.name.clone())
}

/// OPT-004 "Apply now" row: a Ranked outcome with a safe recommended
/// candidate. The checkbox is always live (rows without a safe
/// candidate never reach this section), and the verdict column holds a
/// single glyph — no overloaded meaning.
fn draw_apply_now_row(
    ui: &mut egui::Ui,
    state: &AppState,
    row_idx: usize,
    toolpath_index: usize,
    outcome: &OptimizeOutcome,
    row_selected: &[bool],
    events: &mut Vec<AppEvent>,
) {
    let name = toolpath_name(state, toolpath_index);
    let mut checked = row_selected.get(row_idx).copied().unwrap_or(false);
    let response = ui.add(egui::Checkbox::new(&mut checked, ""));
    if response.clicked() {
        events.push(AppEvent::ToggleOptimizeProjectRow(row_idx));
    }
    ui.label(egui::RichText::new(name).small());
    if let (Some(b), Some(rec)) = (outcome.candidates.first(), outcome.first_safe()) {
        ui.label(egui::RichText::new(format_delta(&rec.delta)).small());
        let saving = b.cycle_time_s - rec.cycle_time_s;
        ui.label(
            egui::RichText::new(format!("-{}", format_cycle(saving)))
                .small()
                .color(theme::SUCCESS),
        );
        draw_compact_verdict(ui, rec);
    } else {
        // Defensive: bucketing guarantees a safe candidate here.
        ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
        ui.label("");
    }
}

/// OPT-002 / OPT-004 "Needs your call" row: TradeOff / MarginalSafe.
/// A role chip (not a fake checkbox) names *why* it needs a decision,
/// and the Review button opens the per-toolpath modal for that exact
/// row — closing the old dead-end where the user had to hunt for it.
fn draw_needs_call_row(
    ui: &mut egui::Ui,
    state: &AppState,
    toolpath_index: usize,
    outcome: &OptimizeOutcome,
    events: &mut Vec<AppEvent>,
) {
    let name = toolpath_name(state, toolpath_index);
    let tried = outcome.candidates.len().saturating_sub(1);
    let (chip, note) = match outcome.kind {
        OutcomeKind::TradeOff => ("trade-off", format!("{tried} faster, gate regression")),
        OutcomeKind::MarginalSafe => ("verify scrap", format!("{tried} inside tolerance band")),
        _ => ("review", String::new()),
    };
    ui.label(egui::RichText::new(chip).small().color(theme::WARNING));
    ui.label(egui::RichText::new(name).small());
    ui.label(egui::RichText::new(note).small().color(theme::WARNING));

    let tp_id = state
        .session
        .toolpath_configs()
        .get(toolpath_index)
        .map(|tc| tc.id);
    if let Some(id) = tp_id {
        if ui
            .small_button("Review ▸")
            .on_hover_text("Open the per-toolpath optimize detail to inspect and apply.")
            .clicked()
        {
            events.push(AppEvent::OpenOptimizeModal(id));
        }
    } else {
        ui.label("");
    }
}

/// OPT-004 "Not optimized" row: NoSafeImprovement / Skipped / a Ranked
/// outcome with no safe candidate. Rendered as a per-row disclosure —
/// the header is a glyph + one phrase, the full narrative and the
/// candidates-tried count live in the body, so there is no 70-char
/// inline truncation in the table.
fn draw_not_optimized_row(
    ui: &mut egui::Ui,
    state: &AppState,
    toolpath_index: usize,
    outcome: &OptimizeOutcome,
) {
    let name = toolpath_name(state, toolpath_index);
    let (glyph, color, phrase, detail) = not_optimized_row_text(outcome);
    egui::CollapsingHeader::new(
        egui::RichText::new(format!("{glyph}  {name} — {phrase}"))
            .small()
            .color(color),
    )
    .id_salt(("optimize_not_optimized_row", toolpath_index))
    .default_open(false)
    .show(ui, |ui| {
        ui.label(egui::RichText::new(detail).small().color(theme::TEXT_MUTED));
    });
}

/// The four strings/colour [`draw_not_optimized_row`] renders: header
/// glyph, header colour, header phrase, disclosure body. Pure so the
/// row's TEXT is testable — this crate has no egui render harness, so
/// splitting the derivation out is how a test can witness what the row
/// says rather than only that it drew something (G-EXPL-HIDDEN).
fn not_optimized_row_text(
    outcome: &OptimizeOutcome,
) -> (&'static str, egui::Color32, String, String) {
    let tried = outcome.candidates.len().saturating_sub(1);
    match outcome.kind {
        OutcomeKind::Skipped => {
            let reason = outcome
                .reason
                .as_ref()
                .map_or("optimizer refused", |r| r.explanation_for_optimize())
                .to_owned();
            ("·", theme::TEXT_DIM, format!("skipped: {reason}"), reason)
        }
        OutcomeKind::NoSafeImprovement => {
            // G-EXPL-HIDDEN (2026-08-16): this row used to read
            // `narrative.explanation` alone, which is the mirror of the
            // modal's omission — a search that ran and lost puts its
            // limiting-gate readings on `headline` and only the generic
            // reason on `explanation`, so the row named the reason with
            // no gate numbers behind it. `narrative_prose` is the shared
            // render order for both surfaces.
            let prose = narrative_prose(&outcome.narrative).join(" ");
            let detail = if tried > 0 {
                format!("{prose}\n\nTried {tried} candidate(s); none beat the baseline safely.")
            } else {
                prose.clone()
            };
            (
                "⚠",
                theme::WARNING,
                format!("no safe gain — {}", truncate(&prose, 60)),
                detail,
            )
        }
        // Ranked-without-safe lands here too (bucketed as not-optimized).
        _ => (
            "·",
            theme::TEXT_MUTED,
            "no improvement found".to_owned(),
            "The optimizer found no candidate faster than the baseline.".to_owned(),
        ),
    }
}

fn draw_compact_verdict(ui: &mut egui::Ui, candidate: &OptimizeCandidate) {
    // Derived from `criteria()` (Phase 6 task 5) — a fourth gate joins
    // this badge automatically.
    let any_exceed = candidate.verdict.any_exceeded();
    let (glyph, color) = if any_exceed {
        ("⚠", theme::ERROR)
    } else {
        ("✓", theme::SUCCESS)
    };
    ui.label(egui::RichText::new(glyph).color(color));
}

fn format_delta(delta: &ParamDelta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(f) = delta.feed_mm_min {
        parts.push(format!("feed {f:.0}"));
    }
    if let Some(rpm) = delta.spindle_rpm {
        parts.push(format!("rpm {rpm}"));
    }
    if let Some(s) = delta.stepover_mm {
        parts.push(format!("stepover {s:.2}"));
    }
    if let Some(d) = delta.depth_per_pass_mm {
        parts.push(format!("DOC {d:.2}"));
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(", ")
    }
}

fn format_cycle(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "—".to_owned();
    }
    if seconds >= 60.0 {
        let minutes = (seconds / 60.0).floor();
        let secs = seconds - 60.0 * minutes;
        format!("{minutes:.0}:{secs:04.1}")
    } else {
        format!("{seconds:.1}s")
    }
}

/// Reconciling: rollup is visible but dimmed; spinner indicates the
/// post-Apply project sim is in flight. Cancel is a no-op for the
/// sim (the analysis lane runs to completion); Close drops the view.
fn draw_reconciling(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ProjectOptimizeReport,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(
            egui::RichText::new("Applying selected candidates and running reconciliation sim…")
                .strong(),
        );
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);

    // Show the report dimmed so the user has context while the sim runs.
    let prev_visuals = ui.visuals().clone();
    let mut dim = prev_visuals.clone();
    dim.override_text_color = Some(theme::TEXT_MUTED);
    ui.ctx().set_visuals(dim);
    draw_report_table_readonly(ui, state, report, false /* show_reconciled */);
    ui.ctx().set_visuals(prev_visuals);

    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        events.push(AppEvent::CloseOptimizeProject);
    }
}

/// Reconciled: post-Apply project sim has finished. Show the report
/// with both candidate-isolated and reconciled cycle times per row;
/// flag rows where the reconciled verdict disagrees with the
/// candidate verdict (cross-TP interaction).
fn draw_reconciled(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ProjectOptimizeReport,
    events: &mut Vec<AppEvent>,
) {
    ui.label(egui::RichText::new("Optimize applied — reconciled").strong());
    ui.add_space(6.0);
    // UI-wins: the prose paragraph is replaced by the amber `xtp` note on
    // mismatched reconciled cells (see draw_readonly_row) — the cell that
    // disagrees with candidate-isolated *is* the cross-toolpath signal.
    ui.separator();
    ui.add_space(6.0);

    draw_report_table_readonly(ui, state, report, true /* show_reconciled */);

    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        events.push(AppEvent::CloseOptimizeProject);
    }
}

/// Read-only table view of a report — no checkboxes, no Apply
/// buttons. When `show_reconciled` is true, an extra column shows the
/// reconciled cycle time and a delta vs candidate-isolated.
fn draw_report_table_readonly(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ProjectOptimizeReport,
    show_reconciled: bool,
) {
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            let cols = if show_reconciled { 5 } else { 4 };
            egui::Grid::new("optimize_project_readonly_grid")
                .num_columns(cols)
                .spacing([8.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("toolpath").small().strong());
                    ui.label(egui::RichText::new("Δ").small().strong());
                    ui.label(egui::RichText::new("cycle saving").small().strong());
                    ui.label(egui::RichText::new("verdict").small().strong());
                    if show_reconciled {
                        ui.label(egui::RichText::new("reconciled").small().strong());
                    }
                    ui.end_row();

                    for (tp_index, outcome) in &report.per_toolpath {
                        draw_readonly_row(ui, state, *tp_index, outcome, show_reconciled);
                        ui.end_row();
                    }
                });
        });
}

fn draw_readonly_row(
    ui: &mut egui::Ui,
    state: &AppState,
    toolpath_index: usize,
    outcome: &OptimizeOutcome,
    show_reconciled: bool,
) {
    let name = state
        .session
        .toolpath_configs()
        .get(toolpath_index)
        .map_or_else(|| format!("idx {toolpath_index}"), |tc| tc.name.clone());
    match outcome.kind {
        OutcomeKind::Ranked => {
            let baseline = outcome.candidates.first();
            let recommended = outcome.first_safe();
            ui.label(egui::RichText::new(name).small());
            if let (Some(b), Some(rec)) = (baseline, recommended) {
                ui.label(egui::RichText::new(format_delta(&rec.delta)).small());
                let saving = b.cycle_time_s - rec.cycle_time_s;
                ui.label(
                    egui::RichText::new(format!("-{}", format_cycle(saving)))
                        .small()
                        .color(theme::SUCCESS),
                );
                draw_compact_verdict(ui, rec);
                if show_reconciled {
                    let reconciled = rec.reconciled_cycle_time_s;
                    let label = match reconciled {
                        Some(c) => {
                            let candidate_cycle = rec.cycle_time_s;
                            let delta = c - candidate_cycle;
                            let mismatch = delta.abs() > 1.0;
                            let (color, suffix) = if mismatch {
                                (theme::WARNING, "  xtp")
                            } else {
                                (theme::TEXT_MUTED, "")
                            };
                            egui::RichText::new(format!(
                                "{} ({:+.1}s){suffix}",
                                format_cycle(c),
                                delta
                            ))
                            .small()
                            .color(color)
                        }
                        None => egui::RichText::new("—").small().color(theme::TEXT_DIM),
                    };
                    ui.label(label).on_hover_text(
                        "Reconciled cycle from a project end-to-end sim. `xtp` marks a \
                         cross-toolpath interaction — the reconciled value disagrees with \
                         the candidate-isolated estimate.",
                    );
                }
            } else {
                ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
                ui.label(
                    egui::RichText::new("no improvement found")
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                ui.label("");
                if show_reconciled {
                    ui.label("");
                }
            }
        }
        OutcomeKind::NoSafeImprovement => {
            // G-EXPL-HIDDEN (2026-08-16): same omission as
            // `draw_not_optimized_row`, same fix — the prose is both
            // narrative fields, in `narrative_prose`'s order.
            let prose = narrative_prose(&outcome.narrative).join(" ");
            ui.label(egui::RichText::new(name).small());
            ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
            let tried = outcome.candidates.len().saturating_sub(1);
            let suffix = if tried > 0 {
                format!(" — tried {tried}")
            } else {
                String::new()
            };
            ui.label(
                egui::RichText::new(format!("{}{}", truncate(&prose, 50), suffix))
                    .small()
                    .color(theme::WARNING),
            );
            ui.label("");
            if show_reconciled {
                ui.label("");
            }
        }
        OutcomeKind::Skipped => {
            let reason_text = outcome
                .reason
                .as_ref()
                .map_or("optimizer refused", |r| r.explanation_for_optimize());
            ui.label(egui::RichText::new(name).small());
            ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
            ui.label(
                egui::RichText::new(reason_text.to_owned())
                    .small()
                    .color(theme::TEXT_DIM),
            );
            ui.label("");
            if show_reconciled {
                ui.label("");
            }
        }
        OutcomeKind::TradeOff => {
            ui.label(egui::RichText::new(name).small());
            ui.label(
                egui::RichText::new("trade-off")
                    .small()
                    .color(theme::WARNING),
            );
            let tried = outcome.candidates.len().saturating_sub(1);
            ui.label(
                egui::RichText::new(format!("{tried} faster candidate(s) — needs review"))
                    .small()
                    .color(theme::WARNING),
            );
            ui.label("");
            if show_reconciled {
                ui.label("");
            }
        }
        OutcomeKind::MarginalSafe => {
            ui.label(egui::RichText::new(name).small());
            ui.label(
                egui::RichText::new("verify on scrap")
                    .small()
                    .color(theme::WARNING),
            );
            let tried = outcome.candidates.len().saturating_sub(1);
            ui.label(
                egui::RichText::new(format!("{tried} candidate(s) inside tolerance band"))
                    .small()
                    .color(theme::WARNING),
            );
            ui.label("");
            if show_reconciled {
                ui.label("");
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
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

    use rs_cam_core::tool_load::RefuseReason;
    use rs_cam_core::tool_load::optimize::OutcomeNarrative;

    fn no_safe_row(narrative: OutcomeNarrative) -> OptimizeOutcome {
        // Zero candidates — a pre-flight refusal never gets to burn a
        // sim, so `tried` is 0 and the disclosure body is the prose
        // alone.
        OptimizeOutcome::no_safe_improvement(
            Vec::new(),
            RefuseReason::DeflectionSetupLocked,
            narrative,
        )
    }

    /// G-EXPL-HIDDEN. The rollup row already read `explanation`, so the
    /// deflection prescription reached it — this pins that the shared
    /// helper did not lose it.
    #[test]
    fn not_optimized_row_carries_the_deflection_prescription() {
        let outcome = no_safe_row(OutcomeNarrative {
            explanation: "Deflection is setup-locked: predicted peak 622 µm exceeds the \
                          200 µm bound. Reduce stickout to 31.4 mm."
                .to_owned(),
            ..OutcomeNarrative::default()
        });
        let (_, _, phrase, detail) = not_optimized_row_text(&outcome);
        assert!(detail.contains("stickout"), "got: {detail}");
        assert!(detail.contains("622 µm"), "got: {detail}");
        assert!(
            phrase.starts_with("no safe gain — Deflection"),
            "got: {phrase}"
        );
    }

    /// G-EXPL-HIDDEN, the rollup's half of the defect. A search that ran
    /// and lost populates BOTH fields — `headline_no_safe` names the
    /// limiting gate, `build_outcome` names which of the two ways it
    /// lost — and this row read only the second, so the operator got the
    /// generic reason with no gate numbers behind it.
    #[test]
    fn not_optimized_row_no_longer_drops_the_limiting_gate_readings() {
        let outcome = no_safe_row(OutcomeNarrative {
            headline: "Tried 6 candidates; chipload 0.2100 mm/tooth (+31% over LUT max \
                       0.1600) is the limiting gate."
                .to_owned(),
            explanation: "no candidate was both faster and safe: every candidate hit a \
                          gate limit (chipload, power, or deflection)"
                .to_owned(),
            ..OutcomeNarrative::default()
        });
        let (_, _, phrase, detail) = not_optimized_row_text(&outcome);
        assert!(detail.contains("limiting gate"), "got: {detail}");
        assert!(detail.contains("0.2100"), "the gate reading, got: {detail}");
        assert!(detail.contains("every candidate hit"), "got: {detail}");
        assert!(phrase.contains("Tried 6 candidates"), "got: {phrase}");
    }

    /// Both fields populated (QN-c's typed chipload refusal) — both
    /// reach the row, explanation second.
    #[test]
    fn not_optimized_row_carries_both_narrative_fields() {
        let outcome = no_safe_row(OutcomeNarrative {
            headline: "No candidate proposed — the chipload retarget was refused.".to_owned(),
            explanation: "chipload retarget refused: the band [0.0900, 0.1000] mm/tooth is \
                          narrower than the 1.20× high-side headroom"
                .to_owned(),
            ..OutcomeNarrative::default()
        });
        let (_, _, _, detail) = not_optimized_row_text(&outcome);
        assert!(detail.contains("No candidate proposed"), "got: {detail}");
        assert!(detail.contains("0.0900"), "got: {detail}");
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn truncate_long_appends_ellipsis() {
        let out = truncate("the quick brown fox", 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn format_cycle_minutes_format() {
        assert_eq!(format_cycle(125.0), "2:05.0");
    }

    #[test]
    fn format_cycle_short_seconds() {
        assert_eq!(format_cycle(12.3), "12.3s");
    }

    #[test]
    fn format_cycle_handles_inf() {
        assert_eq!(format_cycle(f64::INFINITY), "—");
    }

    #[test]
    fn format_delta_no_changes() {
        assert_eq!(format_delta(&ParamDelta::default()), "—");
    }

    #[test]
    fn format_delta_feed_and_doc() {
        let delta = ParamDelta {
            feed_mm_min: Some(2100.0),
            depth_per_pass_mm: Some(2.5),
            ..Default::default()
        };
        assert_eq!(format_delta(&delta), "feed 2100, DOC 2.50");
    }
}
