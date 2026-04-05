use super::AppEvent;
use crate::state::AppState;
use crate::ui::theme;

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
            let enabled_count = state.job.all_toolpaths().filter(|tp| tp.enabled).count();
            let computed_count = state
                .job
                .all_toolpaths()
                .filter(|tp| tp.enabled && tp.result.is_some())
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
                if sim.is_stale(state.job.edit_counter) {
                    CheckStatus::Warning
                } else {
                    CheckStatus::Pass
                }
            } else {
                CheckStatus::Warning
            };
            let sim_detail = if sim.has_results() {
                if sim.is_stale(state.job.edit_counter) {
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
                    if ui.add_enabled(confirmed, btn).clicked() {
                        events.push(AppEvent::ExportGcodeConfirmed);
                        still_open = false;
                    }
                } else if ui.button("Export G-code").clicked() {
                    events.push(AppEvent::ExportGcodeConfirmed);
                    still_open = false;
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
    for tp in state.job.all_toolpaths() {
        if tp.enabled
            && let Some(result) = &tp.result
        {
            let feed = tp.operation.feed_rate();
            total_secs += (result.stats.cutting_distance / feed) * 60.0;
        }
    }
    total_secs
}

fn count_tool_changes(state: &AppState) -> usize {
    let mut count = 0;
    let mut last_tool = None;
    for tp in state.job.all_toolpaths() {
        if !tp.enabled {
            continue;
        }
        if let Some(last) = last_tool
            && tp.tool_id != last
        {
            count += 1;
        }
        last_tool = Some(tp.tool_id);
    }
    count
}
