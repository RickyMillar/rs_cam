//! F&S suggestion modal — Phase 3 layer 3.
//!
//! Shows the current vs suggested feed/RPM for a single toolpath, the
//! matched LUT row + the alternates considered, the rationale, and an
//! Apply / Cancel pair. Triggered by the per-toolpath "Suggest" button
//! in the tool-load table on the Simulation workspace's diagnostics
//! panel.

use rs_cam_core::tool_load::suggest::{
    FeedSuggestion, RefuseReason, RowEvaluation, SuggestedFeeds, project_suggestions,
};

use super::{AppEvent, theme};
use crate::state::AppState;
use crate::state::toolpath::ToolpathId;

/// Draw the modal if `state.suggest_modal_for` is set. Recomputes the
/// suggestion fresh each frame from the current session + sim trace —
/// the modal stays accurate even if the user changes a tool param
/// behind it.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(toolpath_id) = state.suggest_modal_for else {
        return;
    };

    // Re-derive the suggestion this frame so the modal reacts to live
    // edits (e.g. user changes the tool's flute count, the modal updates).
    let sim_trace = state
        .simulation
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_deref());
    let all = project_suggestions(&state.session, sim_trace);
    let Some(suggestion) = all.into_iter().find(|s| s.toolpath_id == toolpath_id) else {
        // Toolpath disappeared (e.g. removed mid-frame). Close the modal.
        events.push(AppEvent::CloseSuggestModal);
        return;
    };

    let toolpath_name = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map_or_else(|| format!("toolpath {toolpath_id}"), |tc| tc.name.clone());

    let mut still_open = true;
    egui::Window::new(format!("F&S suggest — {toolpath_name}"))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(560.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_current_state(ui, &suggestion);
            ui.separator();
            match &suggestion.suggested {
                Ok(suggested) => draw_recommendation(ui, &suggestion, suggested, events),
                Err(reason) => draw_refusal(ui, &suggestion, reason),
            }

            if !suggestion.considered_rows.is_empty() {
                ui.separator();
                ui.collapsing(
                    egui::RichText::new(format!(
                        "Considered rows ({})",
                        suggestion.considered_rows.len()
                    ))
                    .small(),
                    |ui| draw_considered_rows(ui, &suggestion.considered_rows),
                );
            }

            if !suggestion.rationale.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Rationale").small().strong());
                for line in &suggestion.rationale {
                    ui.label(egui::RichText::new(format!("• {line}")).small());
                }
            }
        });

    if !still_open {
        events.push(AppEvent::CloseSuggestModal);
    }
}

fn draw_current_state(ui: &mut egui::Ui, s: &FeedSuggestion) {
    ui.label(egui::RichText::new("Current").small().strong());
    egui::Grid::new("suggest_current_state")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Feed").small());
            ui.label(format_mm_min(s.current_feed_mm_min));
            ui.end_row();
            ui.label(egui::RichText::new("RPM").small());
            ui.label(format_rpm(s.current_rpm));
            ui.end_row();
            ui.label(egui::RichText::new("Peak chipload").small());
            ui.label(format_chipload(s.current_peak_chipload_mm));
            ui.end_row();
        });
}

fn draw_recommendation(
    ui: &mut egui::Ui,
    s: &FeedSuggestion,
    suggested: &SuggestedFeeds,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Suggested")
            .small()
            .strong()
            .color(theme::SUCCESS),
    );
    egui::Grid::new("suggest_recommendation")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Feed").small());
            let delta = suggested.feed_mm_min - s.current_feed_mm_min;
            let delta_color = if delta.abs() < 1.0 {
                theme::TEXT_DIM
            } else if delta > 0.0 {
                theme::SUCCESS
            } else {
                theme::WARNING
            };
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format_mm_min(suggested.feed_mm_min))
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(format!("({delta:+.0})"))
                        .small()
                        .color(delta_color),
                );
            });
            ui.end_row();

            ui.label(egui::RichText::new("RPM").small());
            ui.label(egui::RichText::new(format!("{:.0}", suggested.rpm)).strong());
            ui.end_row();

            ui.label(egui::RichText::new("Chipload window").small());
            ui.label(format!(
                "{:.4} – {:.4} mm/tooth",
                suggested.chipload_envelope.start, suggested.chipload_envelope.end
            ));
            ui.end_row();

            ui.label(egui::RichText::new("Matched row").small());
            ui.label(egui::RichText::new(&suggested.matched_row_id).code().small());
            ui.end_row();

            if let Some(p) = suggested.power_cap_kw {
                ui.label(egui::RichText::new("Power cap").small());
                ui.label(format!("{p:.2} kW"));
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Apply").strong(),
            ))
            .clicked()
        {
            events.push(AppEvent::ApplySuggestedFeed {
                toolpath_id: ToolpathId(s.toolpath_id),
                feed_mm_min: suggested.feed_mm_min,
            });
        }
        if ui.button("Cancel").clicked() {
            events.push(AppEvent::CloseSuggestModal);
        }
    });
}

fn draw_refusal(ui: &mut egui::Ui, _s: &FeedSuggestion, reason: &RefuseReason) {
    ui.label(
        egui::RichText::new("Cannot suggest")
            .small()
            .strong()
            .color(theme::WARNING),
    );
    ui.label(refusal_description(reason));
}

fn refusal_description(reason: &RefuseReason) -> &'static str {
    match reason {
        RefuseReason::SimulationRequired => "Run a simulation first.",
        RefuseReason::ArcEngagementNotCaptured => {
            "Simulation lacks per-sample arc engagement. Re-run with arc capture."
        }
        RefuseReason::MaterialUnvalidated => {
            "Custom material without a validated Kc. Set the material to a known wood/plywood/MDF first."
        }
        RefuseReason::NoVendorData => {
            "No vendor LUT row matches this tool / material / op tuple. \
             Coverage gap — see source_manifest.json."
        }
        RefuseReason::SteadyStateSamplesNotPresent => {
            "No samples ran at the operation's commanded feed (e.g. drill cycle). \
             The chipload envelope is calibrated for steady-state cutting."
        }
        RefuseReason::BipolarEngagement => {
            "Some samples below cl_min, others above cl_max. No single feed fixes both — \
             reduce stepover variation rather than feed."
        }
        RefuseReason::NoFeasibleRow => {
            "Every compatible LUT row was rejected by feasibility (chipload bounds × RPM \
             outside the machine's feed range or below the row's cl_min)."
        }
        RefuseReason::RpmBracketEmpty => {
            "Every compatible LUT row's RPM bracket falls outside the machine spindle \
             bracket. Consider a different cutter."
        }
    }
}

fn draw_considered_rows(ui: &mut egui::Ui, rows: &[RowEvaluation]) {
    egui::Grid::new("suggest_considered_rows")
        .num_columns(4)
        .spacing([6.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Row").small().strong());
            ui.label(egui::RichText::new("Diam fit").small().strong());
            ui.label(egui::RichText::new("Feed range").small().strong());
            ui.label(egui::RichText::new("Status").small().strong());
            ui.end_row();
            for r in rows {
                ui.label(egui::RichText::new(&r.observation_id).code().small());
                ui.label(
                    egui::RichText::new(format!("{}/200", r.diameter_match_score)).small(),
                );
                ui.label(
                    egui::RichText::new(match &r.feasible_feed_range {
                        Some(rng) => format!("{:.0}-{:.0} mm/min", rng.start, rng.end),
                        None => "—".to_owned(),
                    })
                    .small(),
                );
                if let Some(why) = &r.rejected {
                    ui.label(
                        egui::RichText::new(why)
                            .small()
                            .color(theme::WARNING_MILD),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("feasible")
                            .small()
                            .color(theme::SUCCESS),
                    );
                }
                ui.end_row();
            }
        });
}

fn format_mm_min(v: f64) -> String {
    format!("{v:.0} mm/min")
}

fn format_rpm(v: Option<f64>) -> String {
    match v {
        Some(rpm) => format!("{rpm:.0}"),
        None => "—".to_owned(),
    }
}

fn format_chipload(v: Option<f64>) -> String {
    match v {
        Some(cl) => format!("{cl:.4} mm/tooth"),
        None => "—".to_owned(),
    }
}
