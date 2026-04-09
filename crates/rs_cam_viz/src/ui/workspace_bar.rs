use super::AppEvent;
use crate::state::AppState;
use crate::state::Workspace;
use crate::ui::theme;

/// Draw the workspace switcher bar. Sits below the menu bar, always visible.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let current = state.workspace;

        workspace_tab(
            ui,
            "Setup",
            Workspace::Setup,
            current,
            readiness_badge(state),
            events,
        );
        workspace_tab(
            ui,
            "Toolpaths",
            Workspace::Toolpaths,
            current,
            toolpath_badge(state),
            events,
        );
        workspace_tab(
            ui,
            "Simulation",
            Workspace::Simulation,
            current,
            simulation_badge(state),
            events,
        );

        // Right-aligned workspace context info
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Show current workspace hint
            let hint = match current {
                Workspace::Setup => "Stock, orientation, workholding",
                Workspace::Toolpaths => "Operations, tools, generation",
                Workspace::Simulation => "Verify, animate, export",
            };
            ui.label(egui::RichText::new(hint).small().color(theme::TEXT_FAINT));
        });
    });
}

fn workspace_tab(
    ui: &mut egui::Ui,
    label: &str,
    target: Workspace,
    current: Workspace,
    badge: Option<(String, egui::Color32)>,
    events: &mut Vec<AppEvent>,
) {
    let is_active = current == target;

    let (bg, text_color) = if is_active {
        (
            egui::Color32::from_rgb(65, 72, 95),
            egui::Color32::from_rgb(220, 225, 240),
        )
    } else {
        (egui::Color32::TRANSPARENT, theme::TEXT_MUTED)
    };

    let button = egui::Button::new(egui::RichText::new(label).color(text_color).strong())
        .fill(bg)
        .rounding(egui::Rounding {
            nw: 4.0,
            ne: 4.0,
            sw: 0.0,
            se: 0.0,
        })
        .min_size(egui::vec2(90.0, 28.0));

    let response = ui.add(button);
    if response.clicked() && !is_active {
        events.push(AppEvent::SwitchWorkspace(target));
    }

    // Draw active indicator line under the tab
    if is_active {
        let rect = response.rect;
        let painter = ui.painter();
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 2.0, rect.max.y),
                egui::pos2(rect.max.x - 2.0, rect.max.y),
            ],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
    }

    // Badge (drawn after the tab button)
    if let Some((badge_text, badge_color)) = badge {
        let badge_label = egui::RichText::new(badge_text).small().color(badge_color);
        ui.label(badge_label);
    }

    ui.add_space(2.0);
}

/// Badge for the Toolpaths tab: count of pending operations.
fn toolpath_badge(state: &AppState) -> Option<(String, egui::Color32)> {
    use crate::state::runtime::ComputeStatus;
    let pending = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| {
            let rt = state.gui.toolpath_rt.get(&tc.id);
            let status = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
            tc.enabled && matches!(status, ComputeStatus::Pending | ComputeStatus::Computing)
        })
        .count();
    if pending > 0 {
        Some((format!("{pending} pending"), theme::WARNING))
    } else {
        None
    }
}

/// Badge for the Simulation tab: stale, collisions, or empty.
fn simulation_badge(state: &AppState) -> Option<(String, egui::Color32)> {
    let sim = &state.simulation;

    if !sim.has_results() {
        return None;
    }

    if sim.is_stale(state.gui.edit_counter) {
        return Some((" stale".to_owned(), theme::WARNING));
    }

    let collision_count = sim.checks.holder_collision_count + sim.checks.rapid_collisions.len();
    if collision_count > 0 {
        return Some((format!(" {collision_count}!"), theme::ERROR));
    }

    Some((" \u{2713}".to_owned(), theme::SUCCESS))
}

/// Badge for the Setup tab: aggregate export readiness.
/// Shows issues that would prevent a clean export.
fn readiness_badge(state: &AppState) -> Option<(String, egui::Color32)> {
    let sim = &state.simulation;

    // Count uncomputed enabled operations
    let uncomputed = state
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
                    .is_none()
        })
        .count();

    // Check collisions
    let collisions = sim.checks.holder_collision_count + sim.checks.rapid_collisions.len();

    // Check simulation staleness
    let stale = sim.has_results() && sim.is_stale(state.gui.edit_counter);

    if collisions > 0 {
        Some((format!("{collisions} collision(s)"), theme::ERROR))
    } else if uncomputed > 0 {
        Some((format!("{uncomputed} uncomputed"), theme::WARNING))
    } else if stale {
        Some(("sim stale".to_owned(), theme::WARNING))
    } else if !sim.has_results() && state.session.toolpath_configs().iter().any(|tc| tc.enabled) {
        Some(("not simulated".to_owned(), theme::TEXT_DIM))
    } else {
        None
    }
}
