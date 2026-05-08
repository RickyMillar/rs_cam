use super::AppEvent;
use crate::state::AppState;
use crate::ui::theme;
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
            let enabled_count = state
                .session
                .toolpath_configs()
                .iter()
                .filter(|tc| tc.enabled)
                .count();
            let computed_count = state
                .session
                .toolpath_configs()
                .iter()
                .filter(|tc| {
                    tc.enabled
                        && state
                            .gui
                            .toolpath_rt
                            .get(&tc.id)
                            .and_then(|rt| rt.result.as_ref())
                            .is_some()
                })
                .count();

            check_card(
                ui,
                if computed_count < enabled_count {
                    CheckStatus::Warning
                } else {
                    CheckStatus::Pass
                },
                "Operations",
                &format!("{computed_count}/{enabled_count} computed"),
                "Toolpaths",
                Some(AppEvent::SwitchWorkspace(
                    crate::state::Workspace::Toolpaths,
                )),
                events,
                &mut still_open,
            );

            // --- Simulation check ---
            let sim_status = if sim.has_results() {
                if sim.is_stale(state.gui.edit_counter) {
                    CheckStatus::Warning
                } else {
                    CheckStatus::Pass
                }
            } else {
                CheckStatus::Warning
            };
            let sim_detail = if sim.has_results() {
                if sim.is_stale(state.gui.edit_counter) {
                    "Stale — parameters changed"
                } else {
                    "Up to date"
                }
            } else {
                "Not run"
            };
            check_card(
                ui,
                sim_status,
                "Simulation",
                sim_detail,
                "Simulation",
                Some(AppEvent::SwitchWorkspace(
                    crate::state::Workspace::Simulation,
                )),
                events,
                &mut still_open,
            );

            // --- Rapid collisions check ---
            let rapid_status = if !sim.has_results() {
                CheckStatus::Warning
            } else if sim.checks.rapid_collisions.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            };
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
                Some(AppEvent::SwitchWorkspace(
                    crate::state::Workspace::Simulation,
                )),
                events,
                &mut still_open,
            );

            // --- Holder clearance check ---
            let holder_status = if sim.checks.holder_collision_count > 0 {
                CheckStatus::Fail
            } else if sim.checks.min_safe_stickout.is_some() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warning
            };
            let holder_detail = if sim.checks.holder_collision_count > 0 {
                format!("{} issues", sim.checks.holder_collision_count)
            } else if sim.checks.min_safe_stickout.is_some() {
                "Clear".to_owned()
            } else {
                "Not checked".to_owned()
            };
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

            // --- Tool load model ---
            // Trace is in viz sim state, not session.simulation.
            let load_report = {
                let sim_trace = state
                    .simulation
                    .results
                    .as_ref()
                    .and_then(|r| r.cut_trace.as_deref());
                rs_cam_core::gcode::project_load_report(&state.session, sim_trace)
            };
            let tool_load_status = if load_report.any_exceeded() {
                CheckStatus::Fail
            } else if load_report.any_unmodeled() || load_report.per_toolpath.is_empty() {
                CheckStatus::Warning
            } else {
                CheckStatus::Pass
            };
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
            let total_time = estimate_total_time(state);
            let m = (total_time / 60.0).floor() as u32;
            let s = (total_time % 60.0) as u32;
            let tool_changes = count_tool_changes(state);
            check_card(
                ui,
                CheckStatus::Pass,
                "Est. cycle time (cutting only)",
                &format!("{}:{:02}  ({} tool changes)", m, s, tool_changes),
                "",
                None,
                events,
                &mut still_open,
            );

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

            let has_failures =
                sim.checks.holder_collision_count > 0 || !sim.checks.rapid_collisions.is_empty();

            if has_failures {
                ui.add_space(4.0);
                let error_color = egui::Color32::from_rgb(220, 80, 80);
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(60, 25, 25))
                    .stroke(egui::Stroke::new(1.5, error_color))
                    .inner_margin(8.0)
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("\u{26A0} Exporting with unresolved collisions")
                                .strong()
                                .color(egui::Color32::from_rgb(220, 100, 80)),
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

            ui.horizontal(|ui| {
                if has_failures {
                    let confirm_id = egui::Id::new("preflight_export_confirm");
                    let confirmed = ui.data(|d| d.get_temp::<bool>(confirm_id).unwrap_or(false));
                    let btn = egui::Button::new(egui::RichText::new("Export Anyway").strong())
                        .fill(if confirmed {
                            egui::Color32::from_rgb(180, 50, 40)
                        } else {
                            egui::Color32::from_rgb(80, 40, 40)
                        });
                    if ui
                        .add_enabled(confirmed && !load_gate_blocks, btn)
                        .clicked()
                    {
                        events.push(AppEvent::ExportGcodeConfirmed);
                        still_open = false;
                    }
                } else {
                    let btn = egui::Button::new("Export G-code");
                    if ui.add_enabled(!load_gate_blocks, btn).clicked() {
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

                if ui.button("Cancel").clicked() {
                    still_open = false;
                }
            });
        });

    still_open
}

#[derive(Clone, Copy)]
enum CheckStatus {
    Pass,
    Fail,
    Warning,
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

fn estimate_total_time(state: &AppState) -> f64 {
    let mut total_secs = 0.0;
    for tc in state.session.toolpath_configs() {
        if tc.enabled
            && let Some(rt) = state.gui.toolpath_rt.get(&tc.id)
            && let Some(result) = &rt.result
        {
            let feed = tc.operation.feed_rate();
            total_secs += (result.stats.cutting_distance / feed) * 60.0;
        }
    }
    total_secs
}

fn count_tool_changes(state: &AppState) -> usize {
    let mut count = 0;
    let mut last_tool: Option<usize> = None;
    for tc in state.session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        if let Some(last) = last_tool
            && tc.tool_id != last
        {
            count += 1;
        }
        last_tool = Some(tc.tool_id);
    }
    count
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
        egui::Color32::from_rgb(60, 25, 25)
    } else {
        egui::Color32::from_rgb(50, 45, 25)
    };
    let stroke_color = if any_exceeded {
        theme::ERROR
    } else {
        theme::WARNING
    };
    egui::Frame::default()
        .fill(frame_color)
        .stroke(egui::Stroke::new(1.5, stroke_color))
        .inner_margin(8.0)
        .rounding(4.0)
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
                events.push(AppEvent::SetToolLoadOverride {
                    accept_unmodeled: overrides.accept_unmodeled,
                    accept_exceeded: overrides.accept_exceeded,
                });
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
            parts.push(format!("chipload: EXCEEDS ({label})"));
        }
        ChiploadVerdict::Unmodeled { reason } => {
            parts.push(format!(
                "chipload: unmodeled ({})",
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
