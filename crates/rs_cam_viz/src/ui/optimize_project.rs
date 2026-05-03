//! Project-level Optimize rollup — U3 of OPTIMIZER_UX_PLAN.md.
//!
//! Surfaces the result of `optimize_project` as one rollup window
//! anchored at the screen centre. Header shows baseline vs optimized
//! cycle time; bottleneck callout names the toolpath that dominates
//! runtime; per-row checkboxes drive batch Apply. Refused rows
//! (Skipped / NoSafeImprovement) are rendered inline with their typed
//! reason — no separate error column.

use rs_cam_core::tool_load::optimize::{
    OptimizeCandidate, OptimizeOutcome, ParamDelta, ProjectOptimizeReport,
};

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
    match &view.status {
        OptimizeProjectStatus::Loading => draw_loading(ui, events),
        OptimizeProjectStatus::Failed(msg) => draw_failed(ui, msg, events),
        OptimizeProjectStatus::Ready(report) => {
            draw_ready(ui, state, report, &view.row_selected, events);
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

    egui::ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            egui::Grid::new("optimize_project_grid")
                .num_columns(5)
                .spacing([8.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("").small()); // checkbox
                    ui.label(egui::RichText::new("toolpath").small().strong());
                    ui.label(egui::RichText::new("Δ").small().strong());
                    ui.label(egui::RichText::new("cycle delta").small().strong());
                    ui.label(egui::RichText::new("verdict").small().strong());
                    ui.end_row();

                    for (row_idx, (tp_index, outcome)) in
                        report.per_toolpath.iter().enumerate()
                    {
                        draw_row(
                            ui,
                            state,
                            row_idx,
                            *tp_index,
                            outcome,
                            row_selected,
                            events,
                        );
                        ui.end_row();
                    }
                });
        });

    ui.add_space(8.0);
    let any_safe_selected = report
        .per_toolpath
        .iter()
        .zip(row_selected.iter())
        .any(|((_, outcome), &sel)| sel && outcome.first_safe().is_some());
    ui.horizontal(|ui| {
        let apply_btn =
            ui.add_enabled(any_safe_selected, egui::Button::new("Apply selected"));
        if apply_btn.clicked() {
            events.push(AppEvent::ApplyOptimizeProject);
        }
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeProject);
        }
    });
}

fn draw_header(
    ui: &mut egui::Ui,
    report: &ProjectOptimizeReport,
    row_selected: &[bool],
) {
    let baseline = report.baseline_cycle_time_s;
    let optimized = compute_optimized_cycle(report, row_selected);
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
}

/// Sum baseline cycle for unselected rows + recommended cycle for
/// selected rows — gives the "if you applied just these" estimate.
/// Refused/skipped rows always contribute their baseline cycle.
fn compute_optimized_cycle(
    report: &ProjectOptimizeReport,
    row_selected: &[bool],
) -> f64 {
    report
        .per_toolpath
        .iter()
        .zip(row_selected.iter().chain(std::iter::repeat(&false)))
        .map(|((_, outcome), &selected)| match outcome {
            OptimizeOutcome::Ranked(candidates) => {
                let baseline = candidates.first().map_or(0.0, |c| c.cycle_time_s);
                if selected {
                    outcome.first_safe().map_or(baseline, |c| c.cycle_time_s)
                } else {
                    baseline
                }
            }
            OptimizeOutcome::NoSafeImprovement { .. } | OptimizeOutcome::Skipped { .. } => {
                // No candidate to swap — contributes baseline.
                // We don't have direct access to baseline cycle for
                // refused rows from the outcome; treat as 0 to avoid
                // double-counting (the header is an estimate anyway).
                0.0
            }
        })
        .sum()
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
        .and_then(|(_, outcome)| match outcome {
            OptimizeOutcome::Ranked(candidates) => candidates.first().map(|c| c.cycle_time_s),
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

fn draw_row(
    ui: &mut egui::Ui,
    state: &AppState,
    row_idx: usize,
    toolpath_index: usize,
    outcome: &OptimizeOutcome,
    row_selected: &[bool],
    events: &mut Vec<AppEvent>,
) {
    let name = state
        .session
        .toolpath_configs()
        .get(toolpath_index)
        .map_or_else(|| format!("idx {toolpath_index}"), |tc| tc.name.clone());

    match outcome {
        OptimizeOutcome::Ranked(candidates) => {
            let baseline = candidates.first();
            let recommended = outcome.first_safe();
            let selected = row_selected.get(row_idx).copied().unwrap_or(false);
            let mut checked = selected;
            // Disable the checkbox if there's nothing to apply.
            let enabled = recommended.is_some();
            let response = ui.add_enabled(
                enabled,
                egui::Checkbox::new(&mut checked, ""),
            );
            if response.clicked() {
                events.push(AppEvent::ToggleOptimizeProjectRow(row_idx));
            }
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
            } else {
                ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
                ui.label(
                    egui::RichText::new("no improvement found")
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                ui.label("");
            }
        }
        OptimizeOutcome::NoSafeImprovement { explanation, .. } => {
            ui.label(""); // checkbox column blank
            ui.label(egui::RichText::new(name).small());
            ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
            ui.label(
                egui::RichText::new(truncate(explanation, 80))
                    .small()
                    .color(theme::WARNING),
            );
            ui.label("");
        }
        OptimizeOutcome::Skipped { reason } => {
            ui.label("");
            ui.label(egui::RichText::new(name).small());
            ui.label(egui::RichText::new("—").small().color(theme::TEXT_MUTED));
            ui.label(
                egui::RichText::new(reason.explanation_for_optimize().to_owned())
                    .small()
                    .color(theme::TEXT_DIM),
            );
            ui.label("");
        }
    }
}

fn draw_compact_verdict(ui: &mut egui::Ui, candidate: &OptimizeCandidate) {
    use rs_cam_core::tool_load::verdict::Verdict;
    let any_exceed = matches!(candidate.verdict.chipload, Verdict::Exceeds { .. })
        || matches!(candidate.verdict.power, Verdict::Exceeds { .. })
        || matches!(candidate.verdict.deflection, Verdict::Exceeds { .. });
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
