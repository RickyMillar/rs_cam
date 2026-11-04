use super::AppEvent;
use crate::state::AppState;
use crate::ui::readiness::{self, CheckStatus, CycleTimeBasisExt};
use crate::ui::theme;
use crate::ui_command::UiCommand;
use rs_cam_core::diagnostics::Danger;
use rs_cam_core::gcode::ExportWarning;
use rs_cam_core::tool_load::{ToolLoadReport, ToolpathLoadVerdict};

/// K6 (ruling Q6): the count of the export warnings per [`Danger`], in
/// chip order. One chip per danger with a count above zero; the hover
/// lists `id · toolpath`.
#[must_use]
pub fn danger_counts(warnings: &[ExportWarning]) -> Vec<(Danger, usize)> {
    Danger::ALL
        .iter()
        .map(|&danger| {
            (
                danger,
                warnings.iter().filter(|w| w.danger == danger).count(),
            )
        })
        .collect()
}

/// K6 (operator ruling on the mock, 2026-11-03): the Export button takes
/// the Danger style while a Machine warning stands; one click is the
/// confirm. Nothing else waits or blocks.
#[must_use]
pub fn export_asks_a_click(warnings: &[ExportWarning]) -> bool {
    warnings.iter().any(|w| w.danger == Danger::Machine)
}

/// The chip role of a danger: the theme's role colours, no new colour.
/// The pre-flight chips and the toolpath row (F20, K6) read this one map.
#[must_use]
pub fn danger_role(danger: Danger) -> crate::ui::components::Role {
    use crate::ui::components::Role;
    match danger {
        Danger::Machine => Role::Danger,
        Danger::Tool => Role::Caution,
        Danger::Part => Role::Info,
    }
}

/// K6: the chip row. Colour and count, no sentence; the hover lists the
/// `id · toolpath` rows of the chip.
fn draw_danger_chips(ui: &mut egui::Ui, warnings: &[ExportWarning]) {
    let counts = danger_counts(warnings);
    if counts.iter().all(|(_, n)| *n == 0) {
        return;
    }
    ui.horizontal(|ui| {
        for (danger, count) in counts {
            if count == 0 {
                continue;
            }
            let rows: Vec<String> = warnings
                .iter()
                .filter(|w| w.danger == danger)
                .map(|w| format!("{} \u{00B7} {}", w.id, w.toolpath))
                .collect();
            ui.add(
                crate::ui::components::StatusChip::new(
                    format!("{} {count}", danger.label()),
                    danger_role(danger),
                )
                .hover(rows.join("\n")),
            );
        }
    });
}

/// Draw the pre-flight checklist modal. Returns true if still open.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) -> bool {
    let mut still_open = true;

    egui::Window::new("Export Readiness")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);

            let sim = &state.simulation;
            // K6 (ruling Q6, the agreed mock): the export warnings. The
            // rapid-collision card and the chip row read this one list.
            let warnings = crate::io::export::preflight_warnings(state.session(), &state.gui, sim);

            // --- Operations check ---
            let (ops_status, computed_count, enabled_count) = readiness::operations_check(state);

            check_card(
                ui,
                ops_status,
                "Operations",
                &format!("{computed_count}/{enabled_count} computed"),
                "Toolpaths",
                Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                    crate::state::Workspace::Toolpaths,
                ))),
                events,
                &mut still_open,
            );

            // W2.5 (D10): an op whose stored feeds were modulated from a
            // simulation the session no longer holds. A caution, never a
            // block; the export door reports the same lines.
            for caution in state
                .session()
                .feeds_cautions(0..state.session().toolpath_configs().len())
            {
                check_card(
                    ui,
                    CheckStatus::Warning,
                    "Feeds from an earlier simulation",
                    &caution,
                    "Simulation",
                    Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        crate::state::Workspace::Simulation,
                    ))),
                    events,
                    &mut still_open,
                );
            }

            // --- Simulation check ---
            let sim_status = readiness::simulation_check(state);
            let sim_detail = simulation_detail(state);
            check_card(
                ui,
                sim_status,
                "Simulation",
                &sim_detail,
                "Simulation",
                Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                    crate::state::Workspace::Simulation,
                ))),
                events,
                &mut still_open,
            );

            // --- Rapid collisions check ---
            // Q6: the count of `project.rapid_collision` rows (toolpaths),
            // the unit of the MCP export text and the CLI summary.
            let rapid_status = readiness::rapid_collision_check(state, &warnings);
            let rapid_detail = readiness::rapid_collision_detail(state, &warnings);
            check_card(
                ui,
                rapid_status,
                "Rapid collisions",
                &rapid_detail,
                "Simulation",
                Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                    crate::state::Workspace::Simulation,
                ))),
                events,
                &mut still_open,
            );

            // --- Holder clearance check ---
            let holder_status = readiness::holder_clearance_check(state);
            // F2.12 — one derivation, shared with the Readiness panel.
            let holder_detail = readiness::holder_clearance_detail(state);
            check_card(
                ui,
                holder_status,
                "Holder clearance",
                &holder_detail,
                "Simulation",
                Some(AppEvent::RunCollisionCheck),
                events,
                &mut still_open,
            );

            // --- Tool load model --- (shared thresholds; trace lives in viz
            // sim state, not session.simulation)
            let load_report = readiness::load_report(state);
            let tool_load_status = readiness::tool_load_check(&load_report);
            let tool_load_detail =
                tool_load_summary_detail(&load_report, simulation_kept_no_trace(state));
            check_card(
                ui,
                tool_load_status,
                "Tool load model",
                &tool_load_detail,
                "",
                None,
                events,
                &mut still_open,
            );

            // --- Cycle time (info only) ---
            // The card names its own basis (G-TIMEEST). This is the last
            // number an operator reads before starting a cut, and the old
            // spelling printed a cutting-only figure — measured 7× optimistic
            // on a real job — under the unqualified words "cycle time".
            let cycle = readiness::estimate_total_time(state);
            let tool_changes = readiness::count_tool_changes(state);
            let (title, detail, status) = match cycle.basis {
                // U3: the label names each missing input.
                Some(basis) => (
                    format!("Est. cycle time ({})", cycle.label()),
                    format!(
                        "{}  ({tool_changes} tool changes)",
                        readiness::format_cycle_time(cycle.seconds)
                    ),
                    basis.status(),
                ),
                // No estimate at all — a dash, never 0:00.
                None => (
                    "Est. cycle time".to_owned(),
                    format!("\u{2014}  ({tool_changes} tool changes)"),
                    CheckStatus::Warning,
                ),
            };
            check_card(
                ui,
                status,
                &title,
                &detail,
                "",
                None,
                events,
                &mut still_open,
            );
            if let Some(basis) = cycle.basis
                && basis != readiness::CycleTimeBasis::MachineModel
            {
                // Inline rather than a hover — the caveat has to survive the
                // operator reading this modal once, quickly.
                let caveat = egui::RichText::new(basis.caveat())
                    .small()
                    .color(theme::WARNING);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.add(egui::Label::new(caveat).wrap());
                });
                for remedy in readiness::cycle_time_remedies(&cycle) {
                    let remedy = egui::RichText::new(remedy).small().color(theme::TEXT_MUTED);
                    ui.horizontal(|ui| {
                        ui.add_space(18.0);
                        ui.add(egui::Label::new(remedy).wrap());
                    });
                }
                ui.add_space(2.0);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // K6 (ruling Q6, the agreed mock): one chip row, then the
            // button row. No finding refuses; an op with no current result
            // is in the Part chip (`export.op_left_out`).
            draw_danger_chips(ui, &warnings);
            // The holder check of the GUI (`RunCollisionCheck`) is not in the
            // session's simulation, so its count asks for the click too. A
            // rapid collision is a `project.rapid_collision` row (Q6).
            let asks_a_click =
                export_asks_a_click(&warnings) || sim.checks.holder_collision_count > 0;
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    still_open = false;
                }
                // Exporting past a machine warning is the most destructive
                // thing this product does, so it takes the Danger variant;
                // the one click is the confirm.
                let btn = if asks_a_click {
                    crate::ui::components::Button::danger("Export")
                } else {
                    crate::ui::components::Button::new("Export")
                };
                if ui.add(btn).clicked() {
                    events.push(AppEvent::ExportGcodeConfirmed);
                    still_open = false;
                }
            });
        });

    still_open
}

/// The detail text of the Simulation card: one text per
/// [`crate::state::freshness::SimFreshness`] arm.
///
/// W4: the arm names the reason, so the card can say which of the stale
/// readings it has. The old text called a capture-option change a
/// parameter change.
pub(crate) fn simulation_detail(state: &AppState) -> String {
    use crate::state::freshness::SimFreshness;
    let freshness = state.simulation_freshness();
    match freshness {
        SimFreshness::NoRun => "Not run".to_owned(),
        // A run in flight reads as running whatever the core holds. After
        // an edit dropped the core copy, the view still released its run.
        SimFreshness::Running => {
            crate::state::freshness::running_label(&state.simulation).to_owned()
        }
        // M1: the view released the previous run for a new run. The card
        // names the release, so an empty viewport is not read as "never
        // run".
        SimFreshness::Released { in_flight: true } => {
            "Running — previous result released".to_owned()
        }
        SimFreshness::Released { in_flight: false } => {
            "Stale — previous result released, run again".to_owned()
        }
        SimFreshness::Current => "Up to date".to_owned(),
        // The arm names the edits after the run: a machine import reads
        // "machine settings changed after this run", not "parameters".
        SimFreshness::EditedSince(_) => format!(
            "Stale — {}, run again",
            freshness.stale_reason(state.session()).unwrap_or_default()
        ),
        // G-STALECARDS: a re-run cannot help; the operation needs a
        // regenerate first.
        SimFreshness::Ungenerated(id) => format!(
            "Stale — regenerate {} first",
            state
                .session()
                .find_toolpath_config_by_id(id)
                .map_or("an operation", |(_, tc)| tc.name.as_str())
        ),
    }
}

/// A check card with status icon, label, detail, and optional action link.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn check_card(
    ui: &mut egui::Ui,
    status: CheckStatus,
    label: &str,
    detail: &str,
    action_label: &str,
    action_event: Option<AppEvent>,
    events: &mut Vec<AppEvent>,
    still_open: &mut bool,
) {
    let (icon, color) = match status {
        CheckStatus::Pass => ("\u{2713}", theme::SUCCESS),
        CheckStatus::Fail => ("\u{274C}", theme::ERROR),
        CheckStatus::Warning => ("\u{26A0}\u{FE0F}", theme::WARNING),
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).color(color));
        ui.label(egui::RichText::new(format!("{label}:")).strong());
        ui.label(egui::RichText::new(detail).color(color));

        // Show "Go to X" link for warnings/failures
        if !matches!(status, CheckStatus::Pass)
            && !action_label.is_empty()
            && action_event.is_some()
        {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(action_label)
                    .on_hover_text(format!("Open {action_label} workspace"))
                    .clicked()
                {
                    if let Some(event) = action_event {
                        events.push(event);
                    }
                    *still_open = false;
                }
            });
        }
    });

    ui.add_space(2.0);
}

/// The detail of the Tool load model card. U3: after a run with no cut
/// trace, an unmodelled report names the missing trace
/// ([`unmodeled_reason_label`]), not "simulation has not been run".
fn tool_load_summary_detail(report: &ToolLoadReport, trace_less: bool) -> String {
    let total = report.per_toolpath.len();
    if total == 0 {
        return "No toolpaths to evaluate".to_owned();
    }
    let exceeded: Vec<&ToolpathLoadVerdict> = report
        .per_toolpath
        .iter()
        .filter(|v| v.any_exceeded())
        .collect();
    let unmodeled = report
        .per_toolpath
        .iter()
        .filter(|v| v.any_unmodeled())
        .count();
    if !exceeded.is_empty() {
        format!(
            "{} of {} toolpath(s) EXCEED bounds; {unmodeled} have unmodeled criteria",
            exceeded.len(),
            total
        )
    } else if unmodeled > 0 && trace_less {
        format!(
            "{unmodeled} of {total} toolpath(s) have unmodeled criteria: {}",
            unmodeled_reason_label(
                &rs_cam_core::tool_load::UnmodeledReason::SimulationRequired,
                true
            )
        )
    } else if unmodeled > 0 {
        format!("{unmodeled} of {total} toolpath(s) have unmodeled criteria")
    } else {
        format!("{total} toolpath(s) within all modeled bounds")
    }
}

/// A simulation result exists, and it holds no cut trace.
///
/// Every GUI run captures the trace (operator ruling 2026-10-02, "always
/// capture"), so this is rare. It stays so that a result without a trace
/// never reads as "no simulation" and never reads as a measured zero.
pub(crate) fn simulation_kept_no_trace(state: &AppState) -> bool {
    state
        .simulation
        .results
        .as_ref()
        .is_some_and(|results| results.cut_trace.is_none())
}

/// The reason of an unmodelled criterion on the Tool load model card.
///
/// F080: the clause is core's
/// [`rs_cam_core::tool_load::UnmodeledReason::operator_text`]. One arm keeps
/// its own text: `trace_less` is [`simulation_kept_no_trace`]. Core reports
/// `SimulationRequired` for any missing cut trace, and after a run with no
/// trace "simulation has not been run" names the wrong cause (U3).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "pass-through: every other variant keeps its own value"
)]
pub(crate) fn unmodeled_reason_label(
    reason: &rs_cam_core::tool_load::UnmodeledReason,
    trace_less: bool,
) -> String {
    use rs_cam_core::tool_load::UnmodeledReason;
    match reason {
        UnmodeledReason::SimulationRequired if trace_less => {
            "the simulation result holds no cut trace \u{2014} re-run the simulation".to_owned()
        }
        reason => reason.operator_text(),
    }
}

#[cfg(test)]
mod tests {
    use super::unmodeled_reason_label;
    use rs_cam_core::tool_load::UnmodeledReason;

    /// U3: after a run with no cut trace, the Tool load card names the missing
    /// trace, not "simulation has not been run". It names no capture
    /// control: every run captures the trace (operator ruling 2026-10-02).
    #[test]
    fn a_trace_less_run_names_the_missing_trace() {
        let text = unmodeled_reason_label(&UnmodeledReason::SimulationRequired, true);
        assert!(!text.to_lowercase().contains("simulation first"), "{text}");
        assert!(text.contains("no cut trace"), "{text}");
        assert!(text.contains("re-run the simulation"), "{text}");
    }

    /// With no simulation the card prints core's one clause (F080).
    #[test]
    fn no_simulation_reads_the_core_clause() {
        assert_eq!(
            unmodeled_reason_label(&UnmodeledReason::SimulationRequired, false),
            UnmodeledReason::SimulationRequired.operator_text()
        );
    }
}
