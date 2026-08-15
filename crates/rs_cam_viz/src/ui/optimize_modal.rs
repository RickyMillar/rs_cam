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
    BaselineTraceAssumptions, EntryAdvisory, GateKind, KinematicsSource, KnobAxis, LimitingGate,
    LutQueryStamp, MachineSnapshot, OperatorSuggestion, OptimizeCandidate, OptimizeOutcome,
    OutcomeKind, OutcomeNarrative, ParamDelta, SearchEnvelopeReached, SimAssumptionStamp,
    limiting_gates_for_verdict,
};
use rs_cam_core::tool_load::verdict::{ChipSide, ToolpathLoadVerdict};

use super::components::{FreshnessGate, UiExt};
use super::{AppEvent, theme};
use crate::state::AppState;
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
    toolpath_id: rs_cam_core::ToolpathId,
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
    toolpath_id: rs_cam_core::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    let narrative = outcome.narrative.as_ref();
    let attempted = &outcome.candidates;
    // Set by the two shapes that have nothing to tabulate; the Close button
    // they own is drawn after the provenance drawer (see below).
    let mut trailing_close = false;
    match outcome.kind {
        OutcomeKind::Skipped => {
            let reason = outcome
                .reason
                .as_ref()
                .map_or("optimizer refused", |r| r.explanation_for_optimize());
            draw_refusal_section(ui, "Cannot optimise this toolpath", reason);
            trailing_close = true;
        }
        OutcomeKind::NoSafeImprovement => {
            // G17 A2: render the structured narrative — headline,
            // search envelope, then the attempted table. Per-row
            // limiting-gate readings replace the generic "gate" status.
            //
            // G-EXPL-HIDDEN (2026-08-16): the prose is `headline` AND
            // `explanation`, not headline alone. This branch used to
            // drop `explanation` on the floor, which hid F2.3's
            // `DeflectionSetupLocked` prescription entirely — that
            // refusal ships an EMPTY headline and puts the whole
            // prescription on `explanation`, so the card printed a
            // blank line where the operator's instructions belonged.
            ui.label(
                egui::RichText::new("No improvement found")
                    .strong()
                    .color(theme::WARNING),
            );
            for (i, line) in narrative_prose(narrative).into_iter().enumerate() {
                ui.add_space(4.0);
                let text = egui::RichText::new(line).small();
                ui.label(if i == 0 {
                    text
                } else {
                    text.color(theme::TEXT_MUTED)
                });
            }
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
                // No non-baseline candidates ran (early refuse). Nothing to
                // tabulate; the Close button is drawn below the provenance.
                trailing_close = true;
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
            //
            // G-EXPL-HIDDEN (2026-08-16): "swaps" turned out to mean
            // "drops" — `build_outcome` still sets an `explanation` on
            // this tier ("verify on a scrap before applying; the strict
            // LUT bound was exceeded by less than the configured
            // breakage / burn tolerance"), and it was never rendered.
            // Same omission as the `NoSafeImprovement` branch, same fix,
            // found while fixing that one. `TradeOff` is left alone: its
            // narrative contract populates no `explanation` at all.
            ui.label(
                egui::RichText::new("Verify on a scrap")
                    .strong()
                    .color(theme::WARNING),
            );
            for (i, line) in narrative_prose(narrative).into_iter().enumerate() {
                ui.add_space(4.0);
                let text = egui::RichText::new(line).small();
                ui.label(if i == 0 {
                    text
                } else {
                    text.color(theme::TEXT_MUTED)
                });
            }
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

    // Checkpoint P (4): every tier, refusals included. A `Skipped` outcome
    // never simulated, but it still decided something about the world at an
    // operating point — that is why `optimize_toolpath` stamps both blocks
    // outside the inner search, and why they are rendered here rather than
    // inside one of the arms above.
    ui.add_space(10.0);
    draw_run_provenance(ui, outcome);

    if trailing_close {
        ui.add_space(8.0);
        if ui.button("Close").clicked() {
            events.push(AppEvent::CloseOptimizeModal);
        }
    }
}

/// **Checkpoint P (4), 2026-08-14 — the run's own provenance, rendered.**
///
/// `OptimizeOutcome::machine_snapshot` has been written and never read since
/// F4.3; `assumptions` (A-8's `SimAssumptionStamp`) reached MCP agents over
/// the wire but no human surface. The operator's 2026-08-07 review asked for
/// the candidate-model isolation to be made *"explicit in every optimizer
/// result"*, and a JSON field an agent can read is half of that.
///
/// **Read-only.** Nothing here is editable and nothing locks a field
/// (feedback: no background field locking). It is a footer disclosure in the
/// panel's existing `CollapsingHeader` + `UiExt::param_grid` idiom, open by
/// default — see the comment at the header for why this one differs from its
/// siblings in `optimize_project.rs`.
///
/// **It renders absence as absence.** `assumptions: None` means *not
/// stamped* — never "the defaults" — and `baseline.resolution_mm: None` means
/// the trace does not record a dexel cell, which is a different statement
/// from "the same cell as the candidates". Substituting a guess for either is
/// precisely the defect the stamp exists to close.
fn draw_run_provenance(ui: &mut egui::Ui, outcome: &OptimizeOutcome) {
    ui.separator();
    // `default_open(true)`, unlike the sibling drawers in
    // `optimize_project.rs`. The review's repair was worded as "make it
    // explicit in every optimizer result", and a drawer the operator has to
    // find and open is not explicit. The modal is resizable and this is the
    // last block on the card, so the cost is scroll, not obstruction.
    egui::CollapsingHeader::new(
        egui::RichText::new("How these numbers were taken")
            .small()
            .strong(),
    )
    .id_salt("optimize_run_provenance")
    .default_open(true)
    .show(ui, |ui| {
        match outcome.machine_snapshot.as_ref() {
            Some(m) => draw_machine_snapshot(ui, m),
            None => draw_not_stamped(
                ui,
                "Machine the search was bounded by",
                "not stamped — this result predates the machine snapshot, or \
                 was built by a constructor that never ran a search",
            ),
        }
        ui.add_space(6.0);
        match outcome.assumptions.as_ref() {
            Some(a) => draw_assumptions(ui, a),
            None => draw_not_stamped(
                ui,
                "Simulation assumptions",
                "not stamped — which is not the same thing as \"the defaults\". \
                 This outcome was not passed through optimize_toolpath, or was \
                 loaded from a record written before the stamp existed.",
            ),
        }
    });
}

fn draw_not_stamped(ui: &mut egui::Ui, title: &str, why: &str) {
    ui.named_section(title, |ui| {
        ui.label(
            egui::RichText::new(why)
                .small()
                .italics()
                .color(theme::TEXT_MUTED),
        );
    });
}

fn prov_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.label(egui::RichText::new(label).small().color(theme::TEXT_MUTED));
    ui.label(egui::RichText::new(value.into()).small());
    ui.end_row();
}

fn draw_machine_snapshot(ui: &mut egui::Ui, m: &MachineSnapshot) {
    ui.named_section("Machine the search was bounded by", |ui| {
        ui.param_grid("optimize_prov_machine", |ui| {
            prov_row(ui, "Profile", m.name.clone());
            prov_row(
                ui,
                "Cutting feed ceiling",
                format!("{:.0} mm/min", m.cutting_feed_ceiling_mm_min),
            );
            prov_row(
                ui,
                "Travel rate",
                format!("{:.0} mm/min", m.max_feed_mm_min),
            );
            prov_row(
                ui,
                "Spindle range",
                format!("{:.0} – {:.0} rpm", m.rpm_min, m.rpm_max),
            );
        });
        ui.label(
            egui::RichText::new(
                "Snapshot taken when the search ran — the session's machine \
                 may have changed since.",
            )
            .small()
            .color(theme::TEXT_MUTED),
        );
    });
}

fn draw_assumptions(ui: &mut egui::Ui, a: &SimAssumptionStamp) {
    let c = &a.candidates;
    ui.named_section("Candidate scoring sims", |ui| {
        ui.param_grid("optimize_prov_candidates", |ui| {
            prov_row(
                ui,
                "Dexel cell (rank / report)",
                format!(
                    "{:.2} mm / {:.2} mm",
                    c.coarse_resolution_mm, c.refined_resolution_mm
                ),
            );
            prov_row(
                ui,
                "Feed modulation",
                if c.adaptive_feed_modulation {
                    format!(
                        "on ({:?}, {:.2})",
                        c.modulation_strategy, c.modulation_aggressiveness
                    )
                } else {
                    "off".to_owned()
                },
            );
            prov_row(
                ui,
                "Predicted feed in gates",
                if c.use_predicted_feed_in_gates {
                    "on"
                } else {
                    "off"
                },
            );
        });
    });

    // The disclosure the review actually asked for — say it in words, not
    // just as a flag the operator has to interpret.
    if c.diverges_from_library_default_modulation() {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "Candidates were scored with feed modulation OFF while normal \
                 simulation runs it ON. \"Safe\" and \"faster\" here mean safe \
                 and faster in the unmodulated commanded-feed model.",
            )
            .small()
            .color(theme::WARNING),
        );
    }

    ui.add_space(6.0);
    draw_baseline_assumptions(ui, &a.baseline);

    ui.add_space(6.0);
    ui.named_section("Model provenance", |ui| {
        ui.param_grid("optimize_prov_model", |ui| {
            prov_row(
                ui,
                "Kinematics",
                match a.kinematics {
                    KinematicsSource::ProfileDeclared => "declared by the machine profile",
                    KinematicsSource::GenericWoodRouterFallback => {
                        "generic wood-router fallback (profile declares none)"
                    }
                },
            );
            match a.lut_query.as_ref() {
                Some(q) => prov_row(ui, "Vendor LUT query", format_lut_query(q)),
                None => prov_row(
                    ui,
                    "Vendor LUT query",
                    "not measured — no evaluation context could be built",
                ),
            }
            prov_row(
                ui,
                "Boundary epsilon",
                format!("{:.3e} relative", a.boundary_epsilon_rel),
            );
        });
    });
}

fn draw_baseline_assumptions(ui: &mut egui::Ui, b: &BaselineTraceAssumptions) {
    ui.named_section("Baseline (your on-screen sim)", |ui| {
        ui.param_grid("optimize_prov_baseline", |ui| {
            prov_row(
                ui,
                "Dexel cell",
                match b.resolution_mm {
                    Some(mm) => format!("{mm:.2} mm"),
                    // Not "same as the candidates" — the trace does not
                    // record a cell at all.
                    None => "not recorded by the trace".to_owned(),
                },
            );
            prov_row(
                ui,
                "Feed modulation",
                match b.adaptive_feed_modulation {
                    Some(true) => "on (observed in the trace)",
                    Some(false) | None => "not measured",
                },
            );
            prov_row(ui, "Sampling step", format!("{:.3} mm", b.sample_step_mm));
        });
        ui.label(
            egui::RichText::new(
                "The baseline row is scored against this trace directly — it \
                 is not re-simulated at the candidate operating point.",
            )
            .small()
            .color(theme::TEXT_MUTED),
        );
    });
}

fn format_lut_query(q: &LutQueryStamp) -> String {
    match q {
        LutQueryStamp::Routed {
            declared_family,
            declared_pass_role,
            queried_family,
            queried_pass_role,
        } => {
            if q.is_rerouted() {
                format!(
                    "{declared_family:?}/{declared_pass_role:?} → \
                     {queried_family:?}/{queried_pass_role:?} (rerouted)"
                )
            } else {
                format!("{queried_family:?}/{queried_pass_role:?}")
            }
        }
        LutQueryStamp::Refused {
            declared_family,
            declared_pass_role,
            tool_family,
        } => format!(
            "refused — no rows for {tool_family:?} on \
             {declared_family:?}/{declared_pass_role:?}"
        ),
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

/// The refusal headline + explanation. The trailing Close button used to live
/// here; Checkpoint P (4) moved it to the end of [`draw_outcome`] so the
/// provenance drawer sits **above** it rather than below the last control —
/// a refusal is the case where naming the operating point matters most. The
/// button appears for exactly the same two outcome shapes as before.
fn draw_refusal_section(ui: &mut egui::Ui, heading: &str, explanation: &str) {
    ui.label(egui::RichText::new(heading).strong().color(theme::WARNING));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(explanation).small());
}

/// **G-EXPL-HIDDEN, 2026-08-16.** The prose an outcome carries —
/// `headline` then `explanation` — in render order, with empties
/// dropped.
///
/// One construction site for the two surfaces that show a refusal:
/// this modal renders the list as stacked labels, the project rollup
/// ([`super::optimize_project`]) joins it for its collapsed row. Before
/// this wave each surface rendered exactly ONE field and silently
/// dropped the other — the modal kept `headline`, the rollup kept
/// `explanation` — so between them every `NoSafeImprovement` shape was
/// half-reported somewhere. What each drop cost, checked against the
/// construction sites rather than assumed:
///
/// - **The modal, on four paths, showed nothing at all.** A pre-flight
///   refusal (`DeflectionSetupLocked`, `BipolarEngagement`), a cancel
///   before any candidate, a stage-2 evaluation failure and a cancelled
///   partial all build from `OutcomeNarrative::default()` plus an
///   `explanation`, so `headline` is **empty** and the branch printed a
///   blank label. F2.3's stickout prescription is one of these.
/// - **The modal, on the ordinary path, dropped the reason.** A search
///   that ran and lost populates BOTH: `headline_no_safe` names the
///   limiting gate readings, and `build_outcome` sets an `explanation`
///   naming which of the two ways it lost ("every candidate hit a gate
///   limit" vs "no candidate beat the baseline cycle time by more than
///   N s"). Only the first reached the card.
/// - **The rollup dropped the limiting-gate readings**, the mirror
///   image: it never read `headline`, so on the ordinary path the
///   operator got the generic reason with no gate numbers behind it.
///
/// Both fields are operator prose about the same outcome, so both are
/// rendered. Identical strings collapse to one — nothing here rewords,
/// truncates or invents text.
pub(super) fn narrative_prose(narrative: &OutcomeNarrative) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    for line in [narrative.headline.as_str(), narrative.explanation.as_str()] {
        if !line.trim().is_empty() && !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

fn draw_ranked(
    ui: &mut egui::Ui,
    candidates: &[OptimizeCandidate],
    recommended: Option<&OptimizeCandidate>,
    toolpath_id: rs_cam_core::ToolpathId,
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
    toolpath_id: rs_cam_core::ToolpathId,
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
            toolpath_id,
            candidate_index,
        });
    }
}

fn draw_verdict_badges(ui: &mut egui::Ui, verdict: &ToolpathLoadVerdict) {
    ui.horizontal(|ui| {
        verdict_badge_state(ui, "advance/tooth", verdict.chipload.state());
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
    toolpath_id: rs_cam_core::ToolpathId,
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
    toolpath_id: rs_cam_core::ToolpathId,
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
            toolpath_id,
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
                "advance/tooth reached {:.4} mm/tooth ({pct:+.0}% over vendor band max {:.4})",
                a.observed, a.bound,
            ),
            Some(rs_cam_core::tool_load::verdict::ChipSide::Low) => format!(
                "advance/tooth dropped to {:.4} mm/tooth ({pct:+.0}% under vendor band min {:.4})",
                a.observed, a.bound,
            ),
            None => format!("advance/tooth {:.4} mm/tooth", a.observed),
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
            Some(ChipSide::High) => format!("advance/tooth {:.4} ({pct:+.0}%)", g.observed),
            Some(ChipSide::Low) => format!("advance/tooth {:.4} ({pct:+.0}%)", g.observed),
            None => format!("advance/tooth {:.4}", g.observed),
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

    /// The narrative a pre-flight `DeflectionSetupLocked` refusal builds:
    /// `OutcomeNarrative::default()` plus an `explanation`, so the
    /// headline is EMPTY. Shape pinned core-side by
    /// `deflection_exceeds_with_unreachable_corner_refuses_setup_locked`
    /// (`tool_load/optimize/mod.rs`).
    fn deflection_setup_locked_narrative() -> OutcomeNarrative {
        OutcomeNarrative {
            explanation: "Deflection is setup-locked: predicted peak 622 µm exceeds the \
                          200 µm safety bound even at the lowest-force corner the search \
                          can reach. Reduce stickout to 31.4 mm or fit a stiffer tool."
                .to_owned(),
            ..OutcomeNarrative::default()
        }
    }

    /// The narrative `attach_retarget_refusals` builds for a
    /// `ChiploadBandNarrowerThanHeadroom` refusal at zero non-baseline
    /// attempts: headline replaced (the stock one falsely claims an
    /// empty search space), explanation carrying the band and the
    /// unreachable target. Shape pinned core-side by
    /// `a_refused_retarget_names_itself_on_the_outcome`.
    fn narrow_band_refusal_narrative() -> OutcomeNarrative {
        OutcomeNarrative {
            headline: "No candidate proposed — the chipload retarget was refused: the vendor \
                       band the gate judges by is narrower than the retarget headroom."
                .to_owned(),
            explanation: "chipload retarget refused: the band the gate judges by \
                          [0.0900, 0.1000] mm/tooth is narrower than the 1.20× high-side \
                          headroom, so its target 0.0833 mm/tooth falls outside that band"
                .to_owned(),
            ..OutcomeNarrative::default()
        }
    }

    /// G-EXPL-HIDDEN. The prescription is the ONLY prose this refusal
    /// carries — before the fix the branch rendered `headline` alone and
    /// the operator got a blank line where the instruction belonged.
    #[test]
    fn narrative_prose_renders_the_deflection_prescription() {
        let narrative = deflection_setup_locked_narrative();
        let prose = narrative_prose(&narrative);
        assert_eq!(
            prose,
            vec![narrative.explanation.as_str()],
            "the deflection prescription must reach the card verbatim"
        );
        assert!(prose[0].contains("stickout"), "got: {:?}", prose[0]);
        assert!(prose[0].contains("µm"), "got: {:?}", prose[0]);
    }

    /// G-EXPL-HIDDEN. QN-c's typed refusal populates both fields; both
    /// render, explanation second, and it is the explanation that
    /// carries the band and the unreachable target.
    #[test]
    fn narrative_prose_renders_both_lines_for_a_narrow_band_refusal() {
        let narrative = narrow_band_refusal_narrative();
        let prose = narrative_prose(&narrative);
        assert_eq!(prose.len(), 2, "got: {prose:?}");
        assert_eq!(prose[0], narrative.headline.as_str());
        assert_eq!(prose[1], narrative.explanation.as_str());
        assert!(prose[1].contains("0.0900"), "got: {:?}", prose[1]);
        assert!(prose[1].contains("0.0833"), "got: {:?}", prose[1]);
        // The unwind of QN-c's headline-only workaround: the band is
        // stated once, on the explanation, not twice in one card.
        assert!(
            !prose[0].contains("mm/tooth"),
            "the headline should no longer duplicate the band, got: {:?}",
            prose[0]
        );
    }

    /// The control. A narrative carrying only a headline yields exactly
    /// one line — the helper never emits a blank second label. Not a
    /// shipped `NoSafeImprovement` shape (every construction site sets
    /// an explanation); it pins the empties-dropped half of the contract
    /// so the fix cannot regress into two labels, one blank.
    #[test]
    fn narrative_prose_keeps_a_headline_only_narrative() {
        let narrative = OutcomeNarrative {
            headline: "Tried 6 candidates; none were faster than baseline by enough to \
                       recommend."
                .to_owned(),
            ..OutcomeNarrative::default()
        };
        assert_eq!(
            narrative_prose(&narrative),
            vec![narrative.headline.as_str()]
        );
    }

    /// G-EXPL-HIDDEN, second surface. `build_outcome` sets this exact
    /// explanation on every `MarginalSafe` outcome (`outcome.rs`), and
    /// the modal's branch rendered `headline` alone — so the one
    /// sentence telling the operator to cut a scrap first never
    /// appeared on the card that recommends the candidate.
    #[test]
    fn narrative_prose_renders_the_marginal_safe_scrap_warning() {
        let narrative = OutcomeNarrative {
            headline: "Best candidate is 0.1840 mm/tooth against a 0.1600 LUT max, \
                       admitted by the tolerance band."
                .to_owned(),
            explanation: "Best candidate is admitted only by the layer-1 tolerance band — \
                          verify on a scrap before applying. The strict LUT bound was \
                          exceeded by less than the configured breakage / burn tolerance."
                .to_owned(),
            ..OutcomeNarrative::default()
        };
        let prose = narrative_prose(&narrative);
        assert_eq!(prose.len(), 2, "got: {prose:?}");
        assert!(
            prose[1].contains("verify on a scrap"),
            "got: {:?}",
            prose[1]
        );
    }

    /// Identical strings collapse — the helper never prints one sentence
    /// twice, and never invents or truncates text.
    #[test]
    fn narrative_prose_collapses_a_duplicated_line_and_drops_empties() {
        let narrative = OutcomeNarrative {
            headline: "same sentence".to_owned(),
            explanation: "same sentence".to_owned(),
            ..OutcomeNarrative::default()
        };
        assert_eq!(narrative_prose(&narrative), vec!["same sentence"]);
        assert!(narrative_prose(&OutcomeNarrative::default()).is_empty());
    }

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
        assert!(s.contains("advance/tooth"));
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
        // Wanaka shape: helix entry sample reading + vendor band max +
        // pct + operator-friendly suggestion. "LUT max" became "vendor
        // band max" at Checkpoint H2 — the word "chipload" survives only
        // where a vendor band is named, so the band gets named.
        assert!(s.starts_with("helix entry"));
        assert!(s.contains("0.0707"));
        assert!(s.contains("advance/tooth"));
        assert!(s.contains("vendor band max"));
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
        assert!(s.contains("advance/tooth"));
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
