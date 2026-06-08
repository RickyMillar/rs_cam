//! Per-toolpath Optimize modal — U2 of OPTIMIZER_UX_PLAN.md.
//!
//! Replaces the older F&S Suggest modal: shows the current params at
//! the top, the ranked candidate table below, an Apply per row, and
//! a rationale section. Driven by the cached
//! `AppState::optimize_modal` state — the modal does not recompute
//! the outcome on every frame (Optimize is expensive — minutes per
//! toolpath). The controller's `OpenOptimizeModal` handler runs
//! `optimize_toolpath` synchronously and stashes the outcome here.

use rs_cam_core::tool_load::optimize::{
    EntryAdvisory, GateKind, KnobAxis, LimitingGate, OperatorSuggestion, OptimizeCandidate,
    OptimizeOutcome, OutcomeKind, ParamDelta, SearchEnvelopeReached, limiting_gates_for_verdict,
};
use rs_cam_core::tool_load::verdict::{ChipSide, ToolpathLoadVerdict};

use super::components::FreshnessGate;
use super::{AppEvent, theme};
use crate::state::AppState;
use crate::state::toolpath::ToolpathId;
use crate::state::{OptimizeModalState, OptimizeRunStatus};

/// Draw the Optimize modal if `state.optimize_modal` is set.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(modal) = state.optimize_modal.as_ref() else {
        return;
    };
    let toolpath_id = modal.toolpath_id;
    let toolpath_name = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map_or_else(|| format!("toolpath {toolpath_id}"), |tc| tc.name.clone());

    // W0.5/OPT-003 — flag when the baseline behind these numbers came from
    // a sim that's already out of date relative to the current params.
    let baseline_stale = state.simulation.is_stale(state.gui.edit_counter);

    let mut still_open = true;
    egui::Window::new(format!("Optimize — {toolpath_name}"))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(640.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            if baseline_stale {
                FreshnessGate::banner(ui);
                ui.add_space(4.0);
            }
            draw_status(ui, modal, toolpath_id, events);
        });

    if !still_open {
        events.push(AppEvent::CloseOptimizeModal);
    }
}

fn draw_status(
    ui: &mut egui::Ui,
    modal: &OptimizeModalState,
    toolpath_id: usize,
    events: &mut Vec<AppEvent>,
) {
    match &modal.status {
        OptimizeRunStatus::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Optimising — running candidate sims…").small());
            });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "This may take a few minutes. The GUI is responsive — \
                     hit Cancel to stop and keep partial results.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
            // U3 wires Cancel through the worker thread. For U2,
            // closing the modal stops the (non-existent) worker too.
            if ui.button("Cancel").clicked() {
                events.push(AppEvent::CloseOptimizeModal);
            }
        }
        OptimizeRunStatus::Failed(msg) => {
            ui.label(
                egui::RichText::new("Optimize failed")
                    .strong()
                    .color(theme::ERROR),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(msg).small());
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                events.push(AppEvent::CloseOptimizeModal);
            }
        }
        OptimizeRunStatus::Ready(outcome) => {
            draw_outcome(ui, outcome, toolpath_id, events);
        }
    }
}

fn draw_outcome(
    ui: &mut egui::Ui,
    outcome: &OptimizeOutcome,
    toolpath_id: usize,
    events: &mut Vec<AppEvent>,
) {
    let narrative = outcome.narrative.as_ref();
    let attempted = &outcome.candidates;
    match outcome.kind {
        OutcomeKind::Skipped => {
            let reason = outcome
                .reason
                .as_ref()
                .map_or("optimizer refused", |r| r.explanation_for_optimize());
            draw_refusal_section(ui, "Cannot optimise this toolpath", reason, events);
        }
        OutcomeKind::NoSafeImprovement => {
            // G17 A2: render the structured narrative — headline,
            // search envelope, then the attempted table. Per-row
            // limiting-gate readings replace the generic "gate" status.
            ui.label(
                egui::RichText::new("No improvement found")
                    .strong()
                    .color(theme::WARNING),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&narrative.headline).small());
            if let Some(envelope) = format_envelope_summary(&narrative.envelope) {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(envelope)
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            }
            // G17 C2: entry-spike advisories on the closest-to-safe
            // candidate. Informational; not blocking.
            for advisory in &narrative.entry_advisories {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Note: {}", format_entry_advisory(advisory)))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            }
            // G17 A4 / OPT-005: operator-actionable suggestions, if any.
            if !narrative.suggestions.is_empty() {
                ui.add_space(6.0);
                draw_suggestions(ui, &narrative.suggestions, toolpath_id, events);
            }
            ui.add_space(8.0);
            if attempted.len() <= 1 {
                // No non-baseline candidates ran (early refuse). Just
                // close — there's nothing to show.
                if ui.button("Close").clicked() {
                    events.push(AppEvent::CloseOptimizeModal);
                }
            } else {
                ui.separator();
                ui.add_space(4.0);
                draw_attempted(ui, attempted, events);
            }
        }
        OutcomeKind::Ranked => {
            draw_ranked(
                ui,
                &outcome.candidates,
                outcome.first_safe(),
                toolpath_id,
                events,
            );
        }
        OutcomeKind::MarginalSafe => {
            // G16 §11.4 Layer 3: candidates passed every gate but at
            // least one Within reading was admitted only by the
            // tolerance band. G17 A2 swaps the prior generic
            // explanation for narrative.headline (which carries the
            // band-admit overshoot) and renders the search envelope.
            ui.label(
                egui::RichText::new("Verify on a scrap")
                    .strong()
                    .color(theme::WARNING),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&narrative.headline).small());
            if let Some(envelope) = format_envelope_summary(&narrative.envelope) {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(envelope)
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            }
            // G17 C2: entry-spike advisories on the recommended
            // marginal candidate.
            for advisory in &narrative.entry_advisories {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Note: {}", format_entry_advisory(advisory)))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            }
            ui.add_space(8.0);
            draw_ranked(
                ui,
                &outcome.candidates,
                outcome.first_marginal_safe(),
                toolpath_id,
                events,
            );
        }
        OutcomeKind::TradeOff => {
            // Trade-off candidates: faster than baseline AND improve a
            // failing gate, but worsen another. G17 A2 renders the
            // structured narrative.headline ("improves chipload but
            // worsens deflection") instead of the generic copy.
            ui.label(
                egui::RichText::new("Trade-off candidates")
                    .strong()
                    .color(theme::WARNING),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&narrative.headline).small());
            if let Some(envelope) = format_envelope_summary(&narrative.envelope) {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(envelope)
                        .small()
                        .color(theme::TEXT_MUTED),
                );
            }
            ui.add_space(8.0);
            draw_ranked(ui, &outcome.candidates, None, toolpath_id, events);
        }
    }
}

/// Render the attempted-candidates table for `NoSafeImprovement`
/// outcomes. Same shape as Ranked's table but without the ⭐ marker
/// or Apply buttons — none of these candidates is recommended. The
/// goal is purely diagnostic: show what was tried, the cycle delta,
/// and the verdict per row.
fn draw_attempted(ui: &mut egui::Ui, candidates: &[OptimizeCandidate], events: &mut Vec<AppEvent>) {
    let Some((baseline, rest)) = candidates.split_first() else {
        return;
    };
    draw_baseline_card(ui, baseline);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    if rest.is_empty() {
        ui.label(
            egui::RichText::new("No non-baseline candidates were evaluated.")
                .small()
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(8.0);
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeModal);
        }
        return;
    }

    ui.label(
        egui::RichText::new("Candidates evaluated (none beat baseline)")
            .small()
            .strong(),
    );
    ui.add_space(4.0);

    egui::Grid::new("optimize_attempted_grid")
        .num_columns(4)
        .spacing([8.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Δ").small().strong());
            ui.label(egui::RichText::new("cycle").small().strong());
            ui.label(egui::RichText::new("verdict").small().strong());
            ui.label(egui::RichText::new("status").small().strong());
            ui.end_row();

            for candidate in rest {
                draw_attempted_row(ui, candidate, baseline);
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        events.push(AppEvent::CloseOptimizeModal);
    }
}

fn draw_attempted_row(
    ui: &mut egui::Ui,
    candidate: &OptimizeCandidate,
    baseline: &OptimizeCandidate,
) {
    ui.label(egui::RichText::new(format_delta(&candidate.delta)).small());

    let cycle_delta = candidate.cycle_time_s - baseline.cycle_time_s;
    let cycle_color = if cycle_delta < -0.5 {
        theme::SUCCESS
    } else if cycle_delta > 0.5 {
        theme::ERROR
    } else {
        theme::TEXT_MUTED
    };
    ui.label(
        egui::RichText::new(format!(
            "{} ({:+.1}s)",
            format_cycle(candidate.cycle_time_s),
            cycle_delta
        ))
        .small()
        .color(cycle_color),
    );

    draw_verdict_badges(ui, &candidate.verdict);

    // G17 A2: per-row limiting reading. Replaces the generic "gate"
    // status with the specific reading that stopped this candidate —
    // e.g. "chipload 0.071 (+28%)". Slower-than-baseline rows say
    // "slower"; defensively-clean rows say "ok".
    let limiting = limiting_gates_for_verdict(&candidate.verdict);
    if let Some(g) = limiting.first() {
        let color = if g.band_admitted {
            theme::WARNING
        } else {
            theme::ERROR
        };
        ui.label(
            egui::RichText::new(format_limiting_gate(g))
                .small()
                .color(color),
        );
    } else if cycle_delta >= -0.5 {
        ui.label(egui::RichText::new("slower").small().color(theme::WARNING));
    } else {
        ui.label(egui::RichText::new("ok").small().color(theme::TEXT_MUTED));
    }
}

fn draw_refusal_section(
    ui: &mut egui::Ui,
    heading: &str,
    explanation: &str,
    events: &mut Vec<AppEvent>,
) {
    ui.label(egui::RichText::new(heading).strong().color(theme::WARNING));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(explanation).small());
    ui.add_space(8.0);
    if ui.button("Close").clicked() {
        events.push(AppEvent::CloseOptimizeModal);
    }
}

fn draw_ranked(
    ui: &mut egui::Ui,
    candidates: &[OptimizeCandidate],
    recommended: Option<&OptimizeCandidate>,
    toolpath_id: usize,
    events: &mut Vec<AppEvent>,
) {
    // Index 0 is always the baseline. Render it in a pinned card
    // first, then the candidate rows below.
    let Some((baseline, rest)) = candidates.split_first() else {
        return;
    };
    draw_baseline_card(ui, baseline);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    if rest.is_empty() {
        ui.label(
            egui::RichText::new("No non-baseline candidates produced.")
                .small()
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(8.0);
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeModal);
        }
        return;
    }

    ui.label(
        egui::RichText::new("Candidates (ranked by measured cycle time)")
            .small()
            .strong(),
    );
    ui.add_space(4.0);

    let recommended_index =
        recommended.and_then(|r| candidates.iter().position(|c| std::ptr::eq(c, r)));

    egui::Grid::new("optimize_candidates_grid")
        .num_columns(5)
        .spacing([8.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("").small()); // ⭐ column
            ui.label(egui::RichText::new("Δ").small().strong());
            ui.label(egui::RichText::new("cycle").small().strong());
            ui.label(egui::RichText::new("verdict").small().strong());
            ui.label(egui::RichText::new("").small()); // apply column
            ui.end_row();

            for (idx, candidate) in candidates.iter().enumerate().skip(1) {
                let is_recommended = recommended_index == Some(idx);
                draw_candidate_row(
                    ui,
                    idx,
                    candidate,
                    baseline,
                    is_recommended,
                    toolpath_id,
                    events,
                );
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeModal);
        }
    });
}

fn draw_baseline_card(ui: &mut egui::Ui, baseline: &OptimizeCandidate) {
    ui.label(egui::RichText::new("Current").small().strong());
    let cycle_min = format_cycle(baseline.cycle_time_s);
    egui::Grid::new("optimize_baseline_grid")
        .num_columns(2)
        .spacing([12.0, 3.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Cycle:").small());
            ui.label(egui::RichText::new(cycle_min).small());
            ui.end_row();
            ui.label(egui::RichText::new("Feed:").small());
            ui.label(
                egui::RichText::new(format!("{:.0} mm/min", baseline.params.feed_rate())).small(),
            );
            ui.end_row();
            if let Some(rpm) = baseline.params.spindle_rpm() {
                ui.label(egui::RichText::new("RPM:").small());
                ui.label(egui::RichText::new(format!("{rpm}")).small());
                ui.end_row();
            }
            if let Some(stepover) = baseline.params.stepover() {
                ui.label(egui::RichText::new("Stepover:").small());
                ui.label(egui::RichText::new(format!("{stepover:.2} mm")).small());
                ui.end_row();
            }
            if let Some(doc) = baseline.params.depth_per_pass() {
                ui.label(egui::RichText::new("DOC:").small());
                ui.label(egui::RichText::new(format!("{doc:.2} mm")).small());
                ui.end_row();
            }
            ui.label(egui::RichText::new("Verdict:").small());
            draw_verdict_badges(ui, &baseline.verdict);
            ui.end_row();
        });
}

fn draw_candidate_row(
    ui: &mut egui::Ui,
    candidate_index: usize,
    candidate: &OptimizeCandidate,
    baseline: &OptimizeCandidate,
    is_recommended: bool,
    toolpath_id: usize,
    events: &mut Vec<AppEvent>,
) {
    if is_recommended {
        ui.label(egui::RichText::new("⭐").color(theme::SUCCESS));
    } else {
        ui.label("");
    }

    ui.label(egui::RichText::new(format_delta(&candidate.delta)).small());

    let cycle_delta = candidate.cycle_time_s - baseline.cycle_time_s;
    let cycle_color = if cycle_delta < -0.5 {
        theme::SUCCESS
    } else if cycle_delta > 0.5 {
        theme::ERROR
    } else {
        theme::TEXT_MUTED
    };
    ui.label(
        egui::RichText::new(format!(
            "{} ({:+.1}s)",
            format_cycle(candidate.cycle_time_s),
            cycle_delta
        ))
        .small()
        .color(cycle_color),
    );

    draw_verdict_badges(ui, &candidate.verdict);

    // Derived from `criteria()` (Phase 6 task 5) — a fourth gate gates
    // the Apply button automatically.
    let safe = !candidate.verdict.any_exceeded();
    let label = if is_recommended { "Apply ⭐" } else { "Apply" };
    let button = ui.add_enabled(safe, egui::Button::new(label));
    if button.clicked() {
        events.push(AppEvent::ApplyOptimizeCandidate {
            toolpath_id: ToolpathId(toolpath_id),
            candidate_index,
        });
    }
}

fn draw_verdict_badges(ui: &mut egui::Ui, verdict: &ToolpathLoadVerdict) {
    ui.horizontal(|ui| {
        verdict_badge_state(ui, "chipload", verdict.chipload.state());
        verdict_badge_state(ui, "power", verdict.power.state());
        verdict_badge_state(ui, "L/D", verdict.deflection.state());
    });
}

fn verdict_badge_state(
    ui: &mut egui::Ui,
    label: &str,
    state: rs_cam_core::tool_load::verdict::LoadState,
) {
    use rs_cam_core::tool_load::verdict::LoadState;
    let (color, glyph) = match state {
        LoadState::Within => (theme::SUCCESS, "✓"),
        LoadState::Exceeds => (theme::ERROR, "⚠"),
        LoadState::Unmodeled => (theme::TEXT_MUTED, "·"),
    };
    ui.label(
        egui::RichText::new(format!("{glyph} {label}"))
            .small()
            .color(color),
    );
}

/// Render a `ParamDelta` as a compact one-liner: "feed 2100, DOC 2.5".
/// Empty (no changes) returns "—".
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

/// G17 A4 / OPT-005 — render the suggestions as a small inline callout.
/// `CapAxisAt` / `RaiseAxisAbove` carry a concrete axis + value, so each
/// gets an **Apply & re-optimize** button that sets that axis and re-runs
/// the search — an explicit operator click that just removes the manual
/// re-typing step (we still never auto-apply a heuristic). `DataGapHere`
/// has no value to apply, so it stays a plain note.
fn draw_suggestions(
    ui: &mut egui::Ui,
    suggestions: &[OperatorSuggestion],
    toolpath_id: usize,
    events: &mut Vec<AppEvent>,
) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Try this")
                    .small()
                    .strong()
                    .color(theme::TEXT_MUTED),
            );
            for s in suggestions {
                ui.horizontal(|ui| match s {
                    OperatorSuggestion::CapAxisAt { axis, ceiling } => {
                        ui.label(
                            egui::RichText::new(format_suggestion_action(*axis, *ceiling)).small(),
                        );
                        suggestion_apply_button(ui, toolpath_id, *axis, *ceiling, events);
                    }
                    OperatorSuggestion::RaiseAxisAbove { axis, floor } => {
                        ui.label(
                            egui::RichText::new(format_suggestion_action(*axis, *floor)).small(),
                        );
                        suggestion_apply_button(ui, toolpath_id, *axis, *floor, events);
                    }
                    OperatorSuggestion::DataGapHere { reason } => {
                        ui.label(
                            egui::RichText::new(format!("\u{24D8} Data gap: {reason}"))
                                .small()
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });
            }
        });
}

/// OPT-005 — the Apply-&-re-optimize button shared by both actionable
/// suggestion kinds.
fn suggestion_apply_button(
    ui: &mut egui::Ui,
    toolpath_id: usize,
    axis: KnobAxis,
    value: f64,
    events: &mut Vec<AppEvent>,
) {
    if ui
        .small_button("Apply & re-optimize")
        .on_hover_text("Set this axis to the suggested value and re-run the search.")
        .clicked()
    {
        events.push(AppEvent::ReoptimizeWithAxisOverride {
            toolpath_id: ToolpathId(toolpath_id),
            axis,
            value,
        });
    }
}

/// (display label, units, decimal places) for an optimizer search axis.
fn axis_label_units(axis: KnobAxis) -> (&'static str, &'static str, usize) {
    match axis {
        KnobAxis::Feed => ("feed", "mm/min", 0),
        KnobAxis::SpindleRpm => ("RPM", "rpm", 0),
        KnobAxis::Stepover => ("stepover", "mm", 2),
        KnobAxis::DepthPerPass => ("depth-per-pass", "mm", 2),
        KnobAxis::ScallopHeight => ("scallop height", "mm", 3),
    }
}

/// OPT-005 — compact button-row label, e.g. "Cap feed \u{2192} 2961 mm/min".
fn format_suggestion_action(axis: KnobAxis, value: f64) -> String {
    let (label, units, dp) = axis_label_units(axis);
    format!("{label} \u{2192} {value:.dp$} {units}", dp = dp)
}

/// G17 A2 — one-line summary of the search envelope reached, e.g.
/// "tried feed 3150 → 4000 mm/min, stepover 2.0 → 2.2 mm". Returns
/// `None` when no axis moved off baseline (all extents collapsed).
fn format_envelope_summary(env: &SearchEnvelopeReached) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(e) = env.feed_mm_min
        && e.min < e.max
    {
        parts.push(format!("feed {:.0} → {:.0} mm/min", e.min, e.max));
    }
    if let Some(e) = env.spindle_rpm
        && e.min < e.max
    {
        parts.push(format!("rpm {:.0} → {:.0}", e.min, e.max));
    }
    if let Some(e) = env.stepover_mm
        && e.min < e.max
    {
        parts.push(format!("stepover {:.2} → {:.2} mm", e.min, e.max));
    }
    if let Some(e) = env.depth_per_pass_mm
        && e.min < e.max
    {
        parts.push(format!("DOC {:.2} → {:.2} mm", e.min, e.max));
    }
    if let Some(e) = env.scallop_height_mm
        && e.min < e.max
    {
        parts.push(format!("scallop {:.3} → {:.3} mm", e.min, e.max));
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("Tried {}.", parts.join(", ")))
    }
}

/// G17 C2 — operator-friendly one-liner for an entry-sample advisory.
/// "helix entry chipload reached 0.0707 (+29% over LUT max 0.055) —
/// consider gentler entry."
fn format_entry_advisory(a: &EntryAdvisory) -> String {
    let pct = a.overshoot_fraction * 100.0;
    let gate_phrase = match a.gate {
        GateKind::Chipload => match a.side {
            Some(rs_cam_core::tool_load::verdict::ChipSide::High) => format!(
                "chipload reached {:.4} ({pct:+.0}% over LUT max {:.4})",
                a.observed, a.bound,
            ),
            Some(rs_cam_core::tool_load::verdict::ChipSide::Low) => format!(
                "chipload dropped to {:.4} ({pct:+.0}% under LUT min {:.4})",
                a.observed, a.bound,
            ),
            None => format!("chipload {:.4}", a.observed),
        },
        GateKind::Power => format!(
            "power reached {:.2} kW ({pct:+.0}% over available {:.2})",
            a.observed, a.bound,
        ),
        GateKind::Deflection => format!(
            "deflection reached {:.0} µm ({pct:+.0}% over the {:.0} µm threshold)",
            a.observed * 1000.0,
            a.bound * 1000.0,
        ),
    };
    format!("{} {gate_phrase} — consider gentler entry.", a.locality)
}

/// G17 A2 + A3 — compact per-gate reading with optional locality
/// suffix. "chipload 0.0707 (+29%) — slot section" for the wanaka
/// TP 1 case. The locality suffix tells the operator *where* in the
/// cut the limit was hit; sourced from
/// `tool_load::locality::classify_sample_locality`.
fn format_limiting_gate(g: &LimitingGate) -> String {
    let pct = g.overshoot_fraction * 100.0;
    let core = match g.gate {
        GateKind::Chipload => match g.side {
            Some(ChipSide::High) => format!("chipload {:.4} ({pct:+.0}%)", g.observed),
            Some(ChipSide::Low) => format!("chipload {:.4} ({pct:+.0}%)", g.observed),
            None => format!("chipload {:.4}", g.observed),
        },
        GateKind::Power => format!("power {:.2} kW ({pct:+.0}%)", g.observed),
        GateKind::Deflection => format!("defl {:.0} µm ({pct:+.0}%)", g.observed * 1000.0),
    };
    match &g.locality {
        Some(loc) => format!("{core} — {loc}"),
        None => core,
    }
}

/// Format cycle time in mm:ss for cycles ≥ 60s, or as "X.Xs" for
/// shorter runs.
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
    fn format_delta_empty() {
        assert_eq!(format_delta(&ParamDelta::default()), "—");
    }

    #[test]
    fn format_delta_with_feed_and_doc() {
        let delta = ParamDelta {
            feed_mm_min: Some(2100.0),
            depth_per_pass_mm: Some(2.5),
            ..Default::default()
        };
        assert_eq!(format_delta(&delta), "feed 2100, DOC 2.50");
    }

    #[test]
    fn format_cycle_short_seconds() {
        assert_eq!(format_cycle(12.3), "12.3s");
    }

    #[test]
    fn format_cycle_minutes() {
        assert_eq!(format_cycle(125.0), "2:05.0");
    }

    #[test]
    fn format_cycle_handles_inf() {
        assert_eq!(format_cycle(f64::INFINITY), "—");
    }

    #[test]
    fn format_envelope_summary_collapsed_extents_returns_none() {
        // Single candidate at baseline — every axis collapses to one
        // value. Nothing to summarise.
        let env = SearchEnvelopeReached {
            feed_mm_min: Some(rs_cam_core::tool_load::optimize::AxisExtent {
                min: 3000.0,
                max: 3000.0,
            }),
            ..Default::default()
        };
        assert!(format_envelope_summary(&env).is_none());
    }

    #[test]
    fn format_envelope_summary_lists_axes_that_moved() {
        let env = SearchEnvelopeReached {
            feed_mm_min: Some(rs_cam_core::tool_load::optimize::AxisExtent {
                min: 3150.0,
                max: 4000.0,
            }),
            stepover_mm: Some(rs_cam_core::tool_load::optimize::AxisExtent { min: 2.0, max: 2.2 }),
            // DOC collapsed → omitted from the summary.
            depth_per_pass_mm: Some(rs_cam_core::tool_load::optimize::AxisExtent {
                min: 3.0,
                max: 3.0,
            }),
            ..Default::default()
        };
        let s = format_envelope_summary(&env).expect("two axes moved");
        assert!(s.contains("feed 3150 → 4000"));
        assert!(s.contains("stepover 2.00 → 2.20"));
        assert!(!s.contains("DOC"), "collapsed DOC should not render: {s}");
    }

    #[test]
    fn format_limiting_gate_chipload_high() {
        let g = LimitingGate {
            gate: GateKind::Chipload,
            side: Some(ChipSide::High),
            observed: 0.0707,
            bound: 0.055,
            overshoot_fraction: (0.0707 - 0.055) / 0.055,
            band_admitted: false,
            locality: None,
        };
        let s = format_limiting_gate(&g);
        // Wanaka TP 1 case: 0.0707 mm/tooth, 28% over LUT max.
        assert!(s.contains("chipload"));
        assert!(s.contains("0.0707"));
        assert!(
            s.contains("+29") || s.contains("+28"),
            "should mention overshoot % (~28-29%), got: {s}",
        );
    }

    #[test]
    fn format_suggestion_action_feed_rounds_to_int() {
        // OPT-005: compact button-row label, "feed → 2961 mm/min".
        let rendered = format_suggestion_action(KnobAxis::Feed, 2960.7);
        assert_eq!(rendered, "feed \u{2192} 2961 mm/min");
    }

    #[test]
    fn format_suggestion_action_rpm() {
        let rendered = format_suggestion_action(KnobAxis::SpindleRpm, 16000.0);
        assert_eq!(rendered, "RPM \u{2192} 16000 rpm");
    }

    #[test]
    fn format_suggestion_action_doc_uses_two_decimals() {
        let rendered = format_suggestion_action(KnobAxis::DepthPerPass, 3.87);
        assert_eq!(rendered, "depth-per-pass \u{2192} 3.87 mm");
    }

    #[test]
    fn format_entry_advisory_chipload_high_includes_locality_and_pct() {
        let a = EntryAdvisory {
            gate: GateKind::Chipload,
            side: Some(rs_cam_core::tool_load::verdict::ChipSide::High),
            observed: 0.0707,
            bound: 0.055,
            overshoot_fraction: (0.0707 - 0.055) / 0.055,
            locality: "helix entry".to_owned(),
        };
        let s = format_entry_advisory(&a);
        // Wanaka shape: helix entry sample reading + LUT max + pct +
        // operator-friendly suggestion.
        assert!(s.starts_with("helix entry"));
        assert!(s.contains("0.0707"));
        assert!(s.contains("LUT max"));
        assert!(s.contains("+29") || s.contains("+28"));
        assert!(s.ends_with("consider gentler entry."));
    }

    #[test]
    fn format_limiting_gate_appends_locality_suffix() {
        // G17 A3: locality is rendered as " — <label>" after the
        // reading. Wanaka TP 1 case: chipload peak in a full-slot
        // engagement region.
        let g = LimitingGate {
            gate: GateKind::Chipload,
            side: Some(ChipSide::High),
            observed: 0.0707,
            bound: 0.055,
            overshoot_fraction: (0.0707 - 0.055) / 0.055,
            band_admitted: false,
            locality: Some("slot section".to_owned()),
        };
        let s = format_limiting_gate(&g);
        assert!(s.contains("chipload"));
        assert!(
            s.ends_with("— slot section"),
            "locality suffix should append: {s}",
        );
    }

    #[test]
    fn format_limiting_gate_deflection_uses_microns() {
        let g = LimitingGate {
            gate: GateKind::Deflection,
            side: None,
            observed: 0.237,
            bound: 0.200,
            overshoot_fraction: (0.237 - 0.200) / 0.200,
            band_admitted: false,
            locality: None,
        };
        let s = format_limiting_gate(&g);
        // Wanaka refined #1 case: 237 µm peak.
        assert!(
            s.contains("237"),
            "deflection should be rendered in µm: {s}"
        );
    }
}
