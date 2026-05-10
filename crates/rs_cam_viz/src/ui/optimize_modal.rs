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
    GateKind, LimitingGate, OptimizeCandidate, OptimizeOutcome, ParamDelta,
    SearchEnvelopeReached, limiting_gates_for_verdict,
};
use rs_cam_core::tool_load::verdict::{ChipSide, ToolpathLoadVerdict};

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

    let mut still_open = true;
    egui::Window::new(format!("Optimize — {toolpath_name}"))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(640.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
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
    match outcome {
        OptimizeOutcome::Skipped { reason } => draw_refusal_section(
            ui,
            "Cannot optimise this toolpath",
            reason.explanation_for_optimize(),
            events,
        ),
        OptimizeOutcome::NoSafeImprovement {
            attempted,
            narrative,
            ..
        } => {
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
        OptimizeOutcome::Ranked(candidates) => {
            draw_ranked(ui, candidates, outcome.first_safe(), toolpath_id, events);
        }
        OptimizeOutcome::MarginalSafe {
            candidates,
            narrative,
            ..
        } => {
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
            ui.add_space(8.0);
            draw_ranked(
                ui,
                candidates,
                outcome.first_marginal_safe(),
                toolpath_id,
                events,
            );
        }
        OptimizeOutcome::TradeOff {
            candidates,
            narrative,
        } => {
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
            draw_ranked(ui, candidates, None, toolpath_id, events);
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
        ui.label(
            egui::RichText::new("slower")
                .small()
                .color(theme::WARNING),
        );
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

    let safe = !candidate.verdict.chipload.is_exceeded()
        && !candidate.verdict.power.is_exceeded()
        && !candidate.verdict.deflection.is_exceeded();
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
            stepover_mm: Some(rs_cam_core::tool_load::optimize::AxisExtent {
                min: 2.0,
                max: 2.2,
            }),
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
        assert!(s.contains("237"), "deflection should be rendered in µm: {s}");
    }
}
