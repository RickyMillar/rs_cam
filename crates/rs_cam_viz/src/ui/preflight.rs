use super::AppEvent;
use crate::state::AppState;
use crate::ui::readiness::{self, CheckStatus, CycleTimeBasisExt};
use crate::ui::theme;
use crate::ui_command::{SetToolLoadOverrideArgs, UiCommand};
use rs_cam_core::tool_load::{ToolLoadReport, ToolpathLoadVerdict};

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

            // --- Operations that are not current (G-EXPORTSKIP,
            // G-STALEXPORT) ---
            // One row per ENABLED op whose result is not the answer for the
            // configuration on screen, printing the same sentence the
            // export refusal prints — one text builder
            // (`io::export::blocking_toolpath_message`), three callers
            // (here, the export, MCP `export_gcode`). Pre-fix the export
            // silently skipped an ungenerated op and silently emitted an
            // EDITED op's previous geometry, while this modal said
            // "3/4 computed".
            let blocking = crate::io::export::blocking_toolpaths(
                &state.session,
                &state.gui,
                0..state.session.toolpath_configs().len(),
                state.gui.stale_export,
            );
            for blocker in &blocking {
                let (status, label) = if blocker.waived_by_operator {
                    (CheckStatus::Warning, "Accepted — exports previous geometry")
                } else {
                    (CheckStatus::Fail, "Blocks export")
                };
                check_card(
                    ui,
                    status,
                    label,
                    &blocker.message,
                    "Toolpaths",
                    Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        crate::state::Workspace::Toolpaths,
                    ))),
                    events,
                    &mut still_open,
                );
            }
            let ungenerated_blocks = blocking.iter().any(|b| !b.waived_by_operator);
            let any_edited_since = blocking
                .iter()
                .any(|b| b.freshness == crate::state::freshness::FreshnessState::EditedSince);

            // --- Simulation check ---
            let sim_status = readiness::simulation_check(state);
            // W4: the arm names the reason, so the card can say which of the
            // two stale readings it has. The old text called a capture-option
            // change a parameter change.
            let sim_detail = match state.simulation_freshness() {
                crate::state::freshness::SimFreshness::NoRun => "Not run",
                crate::state::freshness::SimFreshness::Running => "Running",
                crate::state::freshness::SimFreshness::Current => "Up to date",
                crate::state::freshness::SimFreshness::EditedSince => "Stale — parameters changed",
                crate::state::freshness::SimFreshness::CaptureOptionsChanged => {
                    "Stale — capture options changed"
                }
            };
            check_card(
                ui,
                sim_status,
                "Simulation",
                sim_detail,
                "Simulation",
                Some(AppEvent::Ui(UiCommand::SwitchWorkspace(
                    crate::state::Workspace::Simulation,
                ))),
                events,
                &mut still_open,
            );

            // --- Rapid collisions check ---
            let rapid_status = readiness::rapid_collision_check(state);
            let rapid_detail = if !sim.has_results() {
                "Run simulation first".to_owned()
            } else if sim.checks.rapid_collisions.is_empty() {
                "None detected".to_owned()
            } else {
                format!("{} detected", sim.checks.rapid_collisions.len())
            };
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
            let tool_load_detail = tool_load_summary_detail(&load_report);
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
                Some(basis) => (
                    format!("Est. cycle time ({})", basis.qualifier()),
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
                if let Some(remedy) = basis.remedy() {
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

            // --- Tool-load override checkboxes (two distinct flags) ───────
            let load_blocked_unmodeled = load_report.any_unmodeled();
            let load_blocked_exceeded = load_report.any_exceeded();
            if load_blocked_unmodeled || load_blocked_exceeded {
                draw_tool_load_overrides(ui, state, &load_report, events);
                ui.add_space(4.0);
            }

            // --- Edited-since acceptance (G-STALEXPORT) ---
            if any_edited_since {
                draw_stale_export_acceptance(ui, state, events);
                ui.add_space(4.0);
            }

            let has_failures =
                sim.checks.holder_collision_count > 0 || !sim.checks.rapid_collisions.is_empty();

            if has_failures {
                ui.add_space(4.0);
                let error_color = crate::ui::tokens::DANGER;
                egui::Frame::default()
                    .fill(crate::ui::tokens::TINT_DANGER)
                    .stroke(egui::Stroke::new(1.5_f32, error_color))
                    .inner_margin(8.0)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("\u{26A0} Exporting with unresolved collisions")
                                .strong()
                                .color(crate::ui::tokens::DANGER),
                        );
                        ui.add_space(4.0);
                        // Use egui memory for checkbox state
                        let confirm_id = egui::Id::new("preflight_export_confirm");
                        let mut confirmed =
                            ui.data(|d| d.get_temp::<bool>(confirm_id).unwrap_or(false));
                        ui.checkbox(&mut confirmed, "I understand the risks");
                        ui.data_mut(|d| d.insert_temp(confirm_id, confirmed));
                    });
                ui.add_space(4.0);
            }

            // The export gate would refuse with the current overrides:
            // (Exceeds without `accept_exceeded`) or (Unmodeled without `accept_unmodeled`).
            let overrides = state.gui.tool_load_overrides;
            let load_gate_blocks = (load_blocked_exceeded && !overrides.accept_exceeded)
                || (load_blocked_unmodeled && !overrides.accept_unmodeled);
            // The export itself refuses on an ungenerated enabled op
            // (G-EXPORTSKIP), so the button says so here instead of
            // letting the click fail in the file dialog.
            let export_blocked = load_gate_blocks || ungenerated_blocks;

            ui.horizontal(|ui| {
                if has_failures {
                    let confirm_id = egui::Id::new("preflight_export_confirm");
                    let confirmed = ui.data(|d| d.get_temp::<bool>(confirm_id).unwrap_or(false));
                    // Exporting past a failed check is the most destructive
                    // thing this product does, so it takes the Danger
                    // variant. It carried two hand-mixed reds,
                    // (180, 50, 40) and (80, 40, 40), which were the fourth
                    // and fifth reds in a product whose palette has one.
                    // The armed and unarmed states now differ by ENABLEMENT,
                    // which is the channel that already gates the click,
                    // rather than by a private shade.
                    let btn = crate::ui::components::Button::danger("Export Anyway")
                        .enabled(confirmed && !export_blocked);
                    if ui.add(btn).clicked() {
                        events.push(AppEvent::ExportGcodeConfirmed);
                        still_open = false;
                    }
                } else {
                    let btn = egui::Button::new("Export G-code");
                    if ui.add_enabled(!export_blocked, btn).clicked() {
                        events.push(AppEvent::ExportGcodeConfirmed);
                        still_open = false;
                    }
                }

                if load_gate_blocks {
                    ui.label(
                        egui::RichText::new("Tool-load gate blocks export")
                            .small()
                            .color(theme::ERROR),
                    );
                }
                if ungenerated_blocks {
                    ui.label(
                        egui::RichText::new("Operations without a current result block export")
                            .small()
                            .color(theme::ERROR),
                    );
                }

                if ui.button("Cancel").clicked() {
                    still_open = false;
                }
            });
        });

    still_open
}

/// A check card with status icon, label, detail, and optional action link.
#[allow(clippy::too_many_arguments)]
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

fn tool_load_summary_detail(report: &ToolLoadReport) -> String {
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
    } else if unmodeled > 0 {
        format!("{unmodeled} of {total} toolpath(s) have unmodeled criteria")
    } else {
        format!("{total} toolpath(s) within all modeled bounds")
    }
}

/// Render the acceptance for operations edited since they were generated
/// (G-STALEXPORT).
///
/// Separate from the tool-load overrides and never folded into them: the
/// tool-load flags accept a predicted CONSEQUENCE of cutting the emitted
/// geometry, and this one accepts emitting DIFFERENT geometry from the
/// one the panel shows. The checkbox says that in those words rather than
/// "export anyway", because that is the decision being made.
fn draw_stale_export_acceptance(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    use crate::state::runtime::StaleResultPolicy;

    egui::Frame::default()
        .fill(crate::ui::tokens::TINT_CAUTION)
        .stroke(egui::Stroke::new(1.5_f32, theme::WARNING))
        .inner_margin(8.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Edited since generation")
                    .strong()
                    .color(theme::WARNING),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(
                    "The stored result for the operations above is the geometry from \
                     before the edit. Regenerating them is the fix.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(4.0);
            let mut accepted = state.gui.stale_export.accepts_previous_geometry();
            if ui
                .checkbox(
                    &mut accepted,
                    "Cut the PREVIOUS geometry — the file will not match the parameters on screen",
                )
                .changed()
            {
                events.push(AppEvent::Ui(UiCommand::SetStaleExportPolicy(if accepted {
                    StaleResultPolicy::AcceptPreviousGeometry
                } else {
                    StaleResultPolicy::Refuse
                })));
            }
        });
}

/// Render the tool-load override panel: per-criterion summary and the two
/// distinct override checkboxes. Each checkbox bypasses a single class of
/// refusal — they are deliberately not collapsed into a single "I accept the
/// risk" toggle, because `Unmodeled` (we don't know) and `Exceeds` (we know
/// it's bad) are different classes of acceptance.
fn draw_tool_load_overrides(
    ui: &mut egui::Ui,
    state: &AppState,
    report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    let any_exceeded = report.any_exceeded();
    let any_unmodeled = report.any_unmodeled();
    let frame_color = if any_exceeded {
        crate::ui::tokens::TINT_DANGER
    } else {
        crate::ui::tokens::TINT_CAUTION
    };
    let stroke_color = if any_exceeded {
        theme::ERROR
    } else {
        theme::WARNING
    };
    egui::Frame::default()
        .fill(frame_color)
        .stroke(egui::Stroke::new(1.5_f32, stroke_color))
        .inner_margin(8.0)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Safety / Tool Load gate")
                    .strong()
                    .color(stroke_color),
            );
            ui.add_space(2.0);

            for verdict in &report.per_toolpath {
                if !verdict.any_exceeded() && !verdict.any_unmodeled() {
                    continue;
                }
                let line = format_verdict_line(verdict);
                ui.label(egui::RichText::new(line).small().color(theme::TEXT_MUTED));
            }
            ui.add_space(4.0);

            let mut overrides = state.gui.tool_load_overrides;
            let mut changed = false;

            if any_unmodeled {
                let resp = ui.checkbox(
                    &mut overrides.accept_unmodeled,
                    "Accept unmodeled criteria (criterion could not be evaluated)",
                );
                if resp.changed() {
                    changed = true;
                }
            }
            if any_exceeded {
                let resp = ui.checkbox(
                    &mut overrides.accept_exceeded,
                    "Accept EXCEEDED criteria (predicted to break tool / exceed power)",
                );
                if resp.changed() {
                    changed = true;
                }
            }
            if changed {
                events.push(AppEvent::Ui(UiCommand::SetToolLoadOverride(
                    SetToolLoadOverrideArgs {
                        accept_unmodeled: overrides.accept_unmodeled,
                        accept_exceeded: overrides.accept_exceeded,
                    },
                )));
            }
        });
}

fn unmodeled_reason_label(reason: &rs_cam_core::tool_load::UnmodeledReason) -> &'static str {
    use rs_cam_core::tool_load::UnmodeledReason;
    match reason {
        UnmodeledReason::SimulationRequired => "run simulation first",
        UnmodeledReason::StaleSimulation => "re-run stale simulation",
        UnmodeledReason::ArcEngagementNotCaptured => "enable Cut Metrics and re-run",
        UnmodeledReason::NoVendorData => "no vendor data",
        UnmodeledReason::SteadyStateSamplesNotPresent => "no steady-state cutting samples",
        UnmodeledReason::MaterialUnvalidated => "material not validated",
        UnmodeledReason::CutterModeUnsupported(_) => "cutter mode unsupported",
        UnmodeledReason::NotImplemented(_) => "not implemented",
        // Roadmap F.8 — gate genuinely doesn't apply (drill cycles).
        // No operator action; this surface doesn't render the carried
        // detail string, just labels the bucket.
        UnmodeledReason::NotApplicableForOp(_) => "not applicable for op type",
        UnmodeledReason::AllSamplesAirCutOrRapid => "toolpath made no material contact",
    }
}

fn format_verdict_line(verdict: &ToolpathLoadVerdict) -> String {
    use rs_cam_core::tool_load::verdict::{
        ChipSide, ChiploadVerdict, DeflectionVerdict, PowerVerdict,
    };
    let mut parts: Vec<String> = Vec::new();
    match &verdict.chipload {
        ChiploadVerdict::Exceeds { side, .. } => {
            let label = match side {
                ChipSide::Low => "ChiploadBurnRisk",
                ChipSide::High => "ChiploadBreakageRisk",
            };
            parts.push(format!("advance/tooth: EXCEEDS ({label})"));
        }
        ChiploadVerdict::Unmodeled { reason } => {
            parts.push(format!(
                "advance/tooth: unmodeled ({})",
                unmodeled_reason_label(reason)
            ));
        }
        ChiploadVerdict::Within { .. } => {}
    }
    match &verdict.power {
        PowerVerdict::Exceeds { .. } => {
            parts.push("power: EXCEEDS (SpindlePowerExceeded)".to_owned());
        }
        PowerVerdict::Unmodeled { reason } => {
            parts.push(format!(
                "power: unmodeled ({})",
                unmodeled_reason_label(reason)
            ));
        }
        PowerVerdict::Within { .. } => {}
    }
    match &verdict.deflection {
        DeflectionVerdict::Exceeds { .. } => {
            parts.push("deflection: EXCEEDS (LongToolStiffnessUnsafe)".to_owned());
        }
        DeflectionVerdict::Unmodeled { reason } => {
            parts.push(format!(
                "deflection: unmodeled ({})",
                unmodeled_reason_label(reason)
            ));
        }
        DeflectionVerdict::Within { .. } => {}
    }
    format!("TP {}: {}", verdict.toolpath_id, parts.join(" \u{00B7} "))
}
