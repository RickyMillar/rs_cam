use super::AppEvent;
use crate::state::AppState;
use crate::state::toolpath::OperationConfig;

/// Draw the pre-flight checklist modal. Returns true if still open.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) -> bool {
    let mut still_open = true;

    egui::Window::new("Pre-Flight Checklist")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);

            let sim = &state.simulation;

            // 1. Simulation run status
            if sim.active {
                let stale = sim.is_stale(state.job.edit_counter);
                if stale {
                    checklist_item(ui, CheckStatus::Warning, "Simulation run", "Stale (params changed since last run)");
                } else {
                    checklist_item(ui, CheckStatus::Pass, "Simulation run", "Up to date");
                }
            } else {
                checklist_item(ui, CheckStatus::Warning, "Simulation run", "Not run");
            }

            // 2. Holder collisions
            if sim.holder_collision_count > 0 {
                checklist_item(ui, CheckStatus::Fail, "Holder collisions",
                    &format!("{} detected", sim.holder_collision_count));
            } else if sim.min_safe_stickout.is_some() {
                checklist_item(ui, CheckStatus::Pass, "Holder collisions", "None");
            } else {
                checklist_item(ui, CheckStatus::Warning, "Holder collisions", "Not checked");
            }

            // 3. Rapid collisions
            if sim.active {
                if sim.rapid_collisions.is_empty() {
                    checklist_item(ui, CheckStatus::Pass, "Rapid collisions", "None");
                } else {
                    checklist_item(ui, CheckStatus::Fail, "Rapid collisions",
                        &format!("{} detected", sim.rapid_collisions.len()));
                }
            } else {
                checklist_item(ui, CheckStatus::Warning, "Rapid collisions", "Not checked");
            }

            // 4. Cycle time
            let total_time = estimate_total_time(state);
            let m = (total_time / 60.0).floor() as u32;
            let s = (total_time % 60.0) as u32;
            checklist_item(ui, CheckStatus::Pass, "Cycle time",
                &format!("{}:{:02}", m, s));

            // 5. Tool changes
            let tool_changes = count_tool_changes(state);
            checklist_item(ui, CheckStatus::Pass, "Tool changes",
                &format!("{}", tool_changes));

            // 6. Enabled operations
            let enabled_count = state.job.toolpaths.iter().filter(|tp| tp.enabled).count();
            let computed_count = state.job.toolpaths.iter()
                .filter(|tp| tp.enabled && tp.result.is_some())
                .count();
            if computed_count < enabled_count {
                checklist_item(ui, CheckStatus::Warning, "Operations",
                    &format!("{}/{} computed", computed_count, enabled_count));
            } else {
                checklist_item(ui, CheckStatus::Pass, "Operations",
                    &format!("{} ready", enabled_count));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let has_failures = sim.holder_collision_count > 0 || !sim.rapid_collisions.is_empty();

            ui.horizontal(|ui| {
                if has_failures {
                    if ui.button("Fix Issues").clicked() {
                        events.push(AppEvent::EnterSimulation);
                        still_open = false;
                    }
                }

                let export_label = if has_failures { "Export Anyway" } else { "Export G-code" };
                let export_btn = egui::Button::new(export_label);
                if ui.add(export_btn).clicked() {
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

fn checklist_item(ui: &mut egui::Ui, status: CheckStatus, label: &str, detail: &str) {
    ui.horizontal(|ui| {
        let (icon, color) = match status {
            CheckStatus::Pass => ("\u{2705}", egui::Color32::from_rgb(100, 200, 100)),
            CheckStatus::Fail => ("\u{274C}", egui::Color32::from_rgb(220, 80, 80)),
            CheckStatus::Warning => ("\u{26A0}\u{FE0F}", egui::Color32::from_rgb(220, 180, 60)),
        };
        ui.label(egui::RichText::new(icon).color(color));
        ui.label(egui::RichText::new(format!("{}:", label)).strong());
        ui.label(egui::RichText::new(detail).color(color));
    });
}

fn estimate_total_time(state: &AppState) -> f64 {
    let mut total_secs = 0.0;
    for tp in &state.job.toolpaths {
        if tp.enabled {
            if let Some(result) = &tp.result {
                let feed = op_feed_rate(&tp.operation);
                total_secs += (result.stats.cutting_distance / feed) * 60.0;
            }
        }
    }
    total_secs
}

fn count_tool_changes(state: &AppState) -> usize {
    let mut seen_tools = Vec::new();
    for tp in &state.job.toolpaths {
        if tp.enabled && !seen_tools.contains(&tp.tool_id) {
            seen_tools.push(tp.tool_id);
        }
    }
    if seen_tools.len() > 1 { seen_tools.len() - 1 } else { 0 }
}

fn op_feed_rate(op: &OperationConfig) -> f64 {
    match op {
        OperationConfig::Face(c) => c.feed_rate,
        OperationConfig::Pocket(c) => c.feed_rate,
        OperationConfig::Profile(c) => c.feed_rate,
        OperationConfig::Adaptive(c) => c.feed_rate,
        OperationConfig::DropCutter(c) => c.feed_rate,
        OperationConfig::Trace(c) => c.feed_rate,
        OperationConfig::Drill(c) => c.feed_rate,
        OperationConfig::Chamfer(c) => c.feed_rate,
        OperationConfig::Zigzag(c) => c.feed_rate,
        OperationConfig::VCarve(c) => c.feed_rate,
        OperationConfig::Rest(c) => c.feed_rate,
        OperationConfig::Inlay(c) => c.feed_rate,
        OperationConfig::Adaptive3d(c) => c.feed_rate,
        OperationConfig::Waterline(c) => c.feed_rate,
        OperationConfig::Pencil(c) => c.feed_rate,
        OperationConfig::Scallop(c) => c.feed_rate,
        OperationConfig::SteepShallow(c) => c.feed_rate,
        OperationConfig::RampFinish(c) => c.feed_rate,
        OperationConfig::SpiralFinish(c) => c.feed_rate,
        OperationConfig::RadialFinish(c) => c.feed_rate,
        OperationConfig::HorizontalFinish(c) => c.feed_rate,
        OperationConfig::ProjectCurve(c) => c.feed_rate,
    }
}
