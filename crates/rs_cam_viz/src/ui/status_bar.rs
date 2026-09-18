//! The status bar: what the project IS, stated as values.
//!
//! # Rule C, applied to this file
//!
//! `planning/ui_declutter_2026-09-14/PLAN.md` Rule C: a sentence becomes a
//! state plus an action. The bar printed `Models: 1  |  Triangles: 12480` and
//! five lane chips that each carried a full sentence — `TP running · q1 ·
//! Adaptive 3D · 12.3s`. Every state stays. Each one is now a faint label and
//! its value, and the prose half of a lane chip (the job name, the phase, the
//! lane's full name) moved to hover.
//!
//! Safety is the one exception. The collision indicator keeps its colour and
//! keeps its stale-run qualifier, because a collision count is the one thing
//! on this bar that changes what the operator does next.
//!
//! # DC7 — the warnings count lives here
//!
//! The project load warnings used to open a non-modal `egui::Window` that the
//! operator could leave over the workspace tab bar. It covered Setup,
//! Toolpaths and Simulation, so navigation stopped. That is defect F-4.
//!
//! The warnings are a count on this bar instead. [`draw`] returns true when
//! the operator clicks it, and the caller opens the modal. Two things improve
//! at once: the tab bar is never covered, and the count is ALWAYS present, so
//! the warnings stay readable. The window could be dismissed once and never
//! reopened.

use crate::compute::{ComputeLane, LaneSnapshot, LaneState};
use crate::state::AppState;
use crate::state::runtime::ComputeStatus;
use crate::ui::automation;
use crate::ui::components::text;
use crate::ui::tokens;

/// Draw the status bar.
///
/// Returns true when the operator clicks the warnings count. This function
/// reads state and never mutates it, so the caller owns the decision to open
/// the warnings modal.
pub fn draw(
    ui: &mut egui::Ui,
    state: &AppState,
    collision_count: usize,
    lanes: &[LaneSnapshot; 5],
    load_warnings: &[String],
) -> bool {
    let mut open_warnings = false;

    ui.horizontal(|ui| {
        ui.set_min_height(tokens::ROW_DENSE);

        let model_count = state.session.models().len();
        let tri_count: usize = state
            .session
            .models()
            .iter()
            .filter_map(|model| model.mesh.as_ref().map(|mesh| mesh.triangles.len()))
            .sum();

        if model_count > 0 {
            indicator(ui, "Models", model_count.to_string());
            indicator(ui, "Triangles", grouped(tri_count));
        } else {
            ui.label(text::caption("Ready"));
        }

        let tp_done = state
            .gui
            .toolpath_rt
            .values()
            .filter(|rt| matches!(rt.status, ComputeStatus::Done))
            .count();
        let tp_total = state.session.toolpath_configs().len();
        if tp_done > 0 {
            ui.separator();
            indicator(ui, "Toolpaths", format!("{tp_done}/{tp_total}"));
        }

        for lane in lanes {
            if matches!(lane.state, LaneState::Idle) && lane.queue_depth == 0 {
                continue;
            }
            ui.separator();
            // A running lane is real state and the operator needs it. What
            // left the bar is the SENTENCE around it: the lane's full name,
            // the job and the phase are on hover.
            let recorded = lane_indicator_text(lane);
            let response = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = tokens::SPACE_1;
                    ui.label(text::caption(lane_prefix(lane.lane)));
                    ui.label(
                        egui::RichText::new(lane_state_word(lane.state))
                            .size(tokens::SIZE_CAPTION)
                            .color(lane_color(lane.state)),
                    );
                    if lane.queue_depth > 0 {
                        ui.label(text::numeric(format!("q{}", lane.queue_depth)));
                    }
                    if let Some(elapsed) = lane.elapsed() {
                        ui.label(text::numeric(format!("{:.1}s", elapsed.as_secs_f32())));
                    }
                })
                .response
                .on_hover_text(lane_tooltip(lane));
            automation::record(ui, lane_automation_id(lane.lane), &response, &recorded);
        }

        if state.simulation.has_results() {
            ui.separator();
            ui.label(
                egui::RichText::new("SIM")
                    .size(tokens::SIZE_MICRO)
                    .extra_letter_spacing(tokens::MICRO_TRACKING)
                    .color(tokens::OK),
            )
            .on_hover_text("Simulation results available");
        }

        if collision_count > 0 {
            ui.separator();
            // SHE-002 — the value is the shared `total_collision_count()`
            // (holder + rapid), and it matches the workspace bar. Provenance
            // lives on hover, and the value greys when the count comes from a
            // simulation that is now stale, instead of asserting a fresh red
            // count.
            //
            // This is the one indicator on the bar that carries a colour.
            // Safety keeps its voice.
            let holder = state.simulation.checks.holder_collision_count;
            let rapid = state.simulation.checks.rapid_collisions.len();
            let stale = state.simulation_is_stale();
            let color = if stale {
                tokens::TEXT_MUTED
            } else {
                tokens::DANGER
            };
            let stale_suffix = if stale { " (from a stale run)" } else { "" };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = tokens::SPACE_1;
                ui.label(text::caption("Collisions"));
                ui.label(
                    egui::RichText::new(collision_count.to_string())
                        .font(tokens::font_numeric())
                        .color(color),
                );
            })
            .response
            .on_hover_text(format!(
                "{collision_count} collisions — {holder} holder, {rapid} rapid{stale_suffix}"
            ));
        }

        if !load_warnings.is_empty() {
            ui.separator();
            let label = warnings_label(load_warnings.len());
            let response = ui
                .add(
                    egui::Label::new(
                        egui::RichText::new(&label)
                            .size(tokens::SIZE_CAPTION)
                            .color(tokens::CAUTION),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Read the project load warnings");
            if response.clicked() {
                open_warnings = true;
            }
            automation::record(ui, "status_load_warnings", &response, &label);
        }

        if state.gui.dirty {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Modified")
                        .italics()
                        // "Modified" is a state worth reviewing before you
                        // cut, not an error. It takes CAUTION.
                        .color(tokens::CAUTION),
                );
            });
        }
    });

    open_warnings
}

/// The warnings count, as the status bar states it.
///
/// Public so the DC7 sentry reads the same words the operator reads.
#[must_use]
pub fn warnings_label(count: usize) -> String {
    if count == 1 {
        "1 warning".to_owned()
    } else {
        format!("{count} warnings")
    }
}

/// One quiet indicator: a faint label, then its value.
///
/// Rule C. The bar states a value. It does not write a sentence about it.
fn indicator(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = tokens::SPACE_1;
        ui.label(text::caption(label));
        ui.label(text::numeric(value));
    });
}

/// A count with grouped thousands (§3.4).
fn grouped(count: usize) -> String {
    text::group_thousands(i64::try_from(count).unwrap_or(i64::MAX))
}

/// The lane's short label. Three characters at most, so five lanes fit.
fn lane_prefix(lane: ComputeLane) -> &'static str {
    match lane {
        ComputeLane::Toolpath => "TP",
        ComputeLane::Analysis => "AN",
        ComputeLane::Optimize => "OPT",
        ComputeLane::Reach => "RCH",
        ComputeLane::Job => "JOB",
    }
}

/// The lane's full name, for hover.
fn lane_name(lane: ComputeLane) -> &'static str {
    match lane {
        ComputeLane::Toolpath => "Toolpath",
        ComputeLane::Analysis => "Analysis",
        ComputeLane::Optimize => "Optimize",
        ComputeLane::Reach => "Reach map",
        ComputeLane::Job => "Job",
    }
}

fn lane_automation_id(lane: ComputeLane) -> &'static str {
    match lane {
        ComputeLane::Toolpath => "status_lane_toolpath",
        ComputeLane::Analysis => "status_lane_analysis",
        ComputeLane::Optimize => "status_lane_optimize",
        ComputeLane::Reach => "status_lane_reach",
        ComputeLane::Job => "status_lane_job",
    }
}

fn lane_state_word(state: LaneState) -> &'static str {
    match state {
        LaneState::Idle => "idle",
        LaneState::Queued => "queued",
        LaneState::Running => "running",
        LaneState::Cancelling => "cancelling",
    }
}

fn lane_color(state: LaneState) -> egui::Color32 {
    match state {
        LaneState::Idle => tokens::LANE_IDLE,
        LaneState::Queued => tokens::LANE_QUEUED,
        LaneState::Running => tokens::LANE_RUNNING,
        LaneState::Cancelling => tokens::LANE_CANCELLING,
    }
}

/// What the lane indicator says, as one string, for the automation snapshot.
fn lane_indicator_text(lane: &LaneSnapshot) -> String {
    let prefix = lane_prefix(lane.lane);
    let word = lane_state_word(lane.state);
    let mut label = format!("{prefix} {word}");
    if lane.queue_depth > 0 {
        label.push_str(&format!(" q{}", lane.queue_depth));
    }
    if let Some(elapsed) = lane.elapsed() {
        label.push_str(&format!(" {:.1}s", elapsed.as_secs_f32()));
    }
    label
}

/// The sentence that left the bar. The lane's full name, the job it runs and
/// the stage it is in belong on hover, not on a status line.
fn lane_tooltip(lane: &LaneSnapshot) -> String {
    let mut tooltip = format!("{} compute lane", lane_name(lane.lane));
    if let Some(job) = &lane.current_job {
        tooltip.push_str(" — ");
        tooltip.push_str(job);
    }
    if let Some(phase) = &lane.current_phase {
        tooltip.push_str(&format!(" ({phase})"));
    }
    tooltip
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
    fn the_warnings_count_is_a_count_and_not_a_sentence() {
        assert_eq!(warnings_label(11), "11 warnings");
        assert_eq!(warnings_label(1), "1 warning");
        assert_eq!(warnings_label(0), "0 warnings");
    }

    #[test]
    fn a_lane_indicator_carries_no_prose() {
        let lane = LaneSnapshot {
            lane: ComputeLane::Toolpath,
            state: LaneState::Running,
            queue_depth: 2,
            current_job: Some("Adaptive 3D".to_owned()),
            current_phase: Some("Pass 12".to_owned()),
            started_at: None,
            active_toolpath_id: None,
            active_toolpath_index: None,
        };
        let shown = lane_indicator_text(&lane);
        assert_eq!(shown, "TP running q2");
        assert!(
            !shown.contains("Adaptive 3D"),
            "the job name is prose and belongs on hover, got {shown:?}"
        );
        let tooltip = lane_tooltip(&lane);
        assert!(tooltip.contains("Adaptive 3D"), "{tooltip:?}");
        assert!(tooltip.contains("Pass 12"), "{tooltip:?}");
    }
}
