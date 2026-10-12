//! Project-level Optimize rollup — U3 of OPTIMIZER_UX_PLAN.md.
//!
//! Surfaces the result of `optimize_project` as one rollup window
//! anchored at the screen centre. Header shows baseline vs optimized
//! cycle time (with a `+N not estimated` note for skipped rows and a
//! baseline-provenance line). W3.5/OPT-004 re-groups the grid by what
//! the operator can do: **Apply now** (Ranked + safe candidate —
//! checkbox-only column) and a collapsed **Not optimized** disclosure
//! (no-safe / skipped — full narrative behind a per-row expander, no
//! inline truncation).

use rs_cam_core::tool_load::optimize::{
    OptimizeCandidate, OptimizeOutcome, OutcomeKind, ProjectOptimizeReport, candidate_is_safe,
};
use rs_cam_core::tool_load::verdict::LoadState;

use super::components::FreshnessGate;
use super::components::format::{
    format_cycle, format_delta, format_full_cut_readings, format_full_cut_verdict, truncate,
};
use super::optimize_modal::{full_cut_color, narrative_prose};
use super::{AppEvent, theme};
use crate::state::{AppState, OptimizeProjectState, OptimizeProjectStatus};
use crate::ui_command::{NoArgs, UiCommand};

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
        events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
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
    if let Some(reason) = state.simulation_stale_reason() {
        FreshnessGate::banner(ui, &reason);
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
            "The search runs the links lever, then the depth and width lever, on \
             every enabled toolpath. Each candidate is one simulation. The GUI is responsive — click Cancel to stop \
             and discard the partial run.",
        )
        .small()
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(8.0);
    if ui.button("Cancel").clicked() {
        events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
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
        events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
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
            events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
        }
        return;
    }

    // OPT-004: bucket rows by what the operator can actually do, so
    // "apply this" and "nothing to apply" never share one unlabeled
    // column. `row_idx` is preserved so the
    // Apply-now checkbox still indexes `row_selected` correctly.
    let mut apply_now: Vec<(usize, usize, &OptimizeOutcome)> = Vec::new();
    let mut not_optimized: Vec<(usize, &OptimizeOutcome)> = Vec::new();
    for (row_idx, (tp_index, outcome)) in report.per_toolpath.iter().enumerate() {
        match outcome.kind {
            OutcomeKind::Ranked if outcome.first_safe().is_some() => {
                apply_now.push((row_idx, *tp_index, outcome));
            }
            _ => not_optimized.push((*tp_index, outcome)),
        }
    }

    egui::ScrollArea::vertical()
        .max_height(420.0)
        .show(ui, |ui| {
            if !apply_now.is_empty() {
                draw_section_heading(ui, "APPLY NOW");
                // UI-02: 5 columns and striped, so `param_grid` does not fit.
                egui::Grid::new("optimize_apply_now_grid")
                    .num_columns(5)
                    .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                    .min_row_height(crate::ui::tokens::ROW_DENSE)
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
            events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
        }
    });
}

/// Section heading for the role-grouped rollup tables.
///
/// UI-03: the kit's header element, not a fourth hand-rolled rung.
fn draw_section_heading(ui: &mut egui::Ui, text: &str) {
    crate::ui::components::SectionHeader::new(text).show(ui);
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
        // Index 0 is the baseline of every outcome that scored one; a
        // skipped outcome has no candidates.
        .and_then(|(_, outcome)| outcome.candidates.first().map(|c| c.cycle_time_s))
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
        events.push(AppEvent::Ui(UiCommand::ToggleOptimizeProjectRow(row_idx)));
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
        // A Ranked outcome with no safe candidate is bucketed as not
        // optimized.
        OutcomeKind::Ranked => (
            "·",
            theme::TEXT_MUTED,
            "no improvement found".to_owned(),
            "The optimizer found no candidate faster than the baseline.".to_owned(),
        ),
    }
}

/// The verdict cell of a rollup row: one glyph for the gates, then the
/// full-cut verdict. The hover gives the depth row and the three
/// full-cut readings.
///
/// The glyph reads the core predicate `candidate_is_safe`. A depth row
/// that exceeds its bound is the warning colour, never the refuse
/// colour, because the depth row never refuses an export (S3).
fn draw_compact_verdict(ui: &mut egui::Ui, candidate: &OptimizeCandidate) {
    let depth_exceeds = candidate.verdict.depth.state() == LoadState::Exceeds;
    let (glyph, color) = match (candidate_is_safe(candidate), depth_exceeds) {
        (false, _) => ("⚠", theme::ERROR),
        (true, true) => ("⚠", theme::WARNING),
        (true, false) => ("✓", theme::SUCCESS),
    };
    let verdict = format_full_cut_verdict(&candidate.full_cut.verdict);
    let mut hover = format!(
        "full cut {verdict}\n{}",
        format_full_cut_readings(&candidate.full_cut)
    );
    if depth_exceeds {
        hover.push_str(
            "\nThe depth of cut is above its bound. The depth row reports; it does not refuse.",
        );
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(glyph).color(color));
        ui.label(
            egui::RichText::new(format!("full cut {}", truncate(&verdict, 32)))
                .small()
                .color(full_cut_color(&candidate.full_cut.verdict)),
        );
    })
    .response
    .on_hover_text(hover);
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
        events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
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
        events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
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
            // UI-02: a run-time column count and striped, so `param_grid` does not fit.
            egui::Grid::new("optimize_project_readonly_grid")
                .num_columns(cols)
                .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
                .min_row_height(crate::ui::tokens::ROW_DENSE)
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
        // Zero candidates, so `tried` is 0 and the disclosure body is
        // the prose alone.
        OptimizeOutcome::no_safe_improvement(
            Vec::new(),
            RefuseReason::NoImprovementFound,
            narrative,
        )
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

    /// The Phase 1 outcome: the search has no levers, so the headline
    /// and the explanation both reach the row.
    #[test]
    fn not_optimized_row_carries_both_narrative_fields() {
        let outcome = no_safe_row(OutcomeNarrative {
            headline: "No candidate was searched.".to_owned(),
            explanation: "This operation has no lever: it holds no depth per pass with a \
                          total depth, and its links are already on or do not apply to it."
                .to_owned(),
            ..OutcomeNarrative::default()
        });
        let (_, _, phrase, detail) = not_optimized_row_text(&outcome);
        assert!(
            phrase.starts_with("no safe gain — No candidate was searched."),
            "got: {phrase}"
        );
        assert!(
            detail.contains("No candidate was searched."),
            "got: {detail}"
        );
        assert!(detail.contains("has no lever"), "got: {detail}");
    }
}
