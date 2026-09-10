use super::AppEvent;
use crate::state::AppState;
use crate::state::Workspace;
use crate::ui::theme;

/// Draw the workspace switcher bar. Sits below the menu bar, always visible.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let current = state.workspace;

        // One list for the bar, the Workspace menu and the sentry
        // (G-WSMENU) — see `Workspace::ALL`. The badge stays a per-tab
        // decision because each reads different state; the aggregate
        // export-readiness chip lives on its named tab (W3.8), not
        // interleaved onto Setup.
        for target in Workspace::ALL {
            let badge = match target {
                Workspace::Setup => None,
                Workspace::Toolpaths => toolpath_badge(state),
                Workspace::Simulation => simulation_badge(state),
                Workspace::Readiness => readiness_badge(state),
            };
            workspace_tab(ui, target.label(), target, current, badge, events);
        }

        // Right-aligned workspace context info
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Show current workspace hint
            ui.label(
                egui::RichText::new(current.hint())
                    .small()
                    .color(theme::TEXT_FAINT),
            );
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
        .corner_radius(egui::CornerRadius {
            nw: 4,
            ne: 4,
            sw: 0,
            se: 0,
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

/// Badge for the Toolpaths tab: stale operations, else pending ones.
///
/// F2.2 (R0.1 §4.4). Stale outranks pending and is reported separately,
/// because the two ask the operator for different things: a pending
/// operation has never produced an answer, while a stale one has produced a
/// WRONG one that every other surface is still drawing and counting. Folding
/// them into one "N pending" would have gone on reading zero on a project
/// where every operation was edited after generation.
pub(crate) fn toolpath_badge(state: &AppState) -> Option<(String, egui::Color32)> {
    let (stale, pending) = crate::ui::readiness::freshness_counts(state);
    if stale > 0 {
        Some((format!("{stale} stale"), theme::WARNING))
    } else if pending > 0 {
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

    // Collisions outrank staleness: a safety error must never hide behind
    // the yellow "stale" warning (SHE-003 — mirror readiness_badge's order).
    let collision_count = sim.checks.total_collision_count();
    if collision_count > 0 {
        return Some((format!(" {collision_count}!"), theme::ERROR));
    }

    if sim.is_stale(state.gui.edit_counter) {
        return Some((" stale".to_owned(), theme::WARNING));
    }

    Some((" \u{2713}".to_owned(), theme::SUCCESS))
}

/// Badge for the Setup tab: aggregate export readiness.
/// Shows issues that would prevent a clean export.
pub(crate) fn readiness_badge(state: &AppState) -> Option<(String, egui::Color32)> {
    let sim = &state.simulation;

    // F2.2 — from the one freshness model. This used to ask
    // `gui.toolpath_rt[..].result.is_none()`, the GUI store, which an edited
    // operation KEEPS so the viewport can still draw it. So the chip counted
    // an operation whose result the core had dropped as computed, and read
    // clean on a project where nothing could be reproduced.
    let (stale_ops, uncomputed) = crate::ui::readiness::freshness_counts(state);

    // Check collisions
    let collisions = sim.checks.total_collision_count();

    // Check simulation staleness
    let stale = sim.has_results() && sim.is_stale(state.gui.edit_counter);

    // Order: collisions, then stale operations, then uncomputed, then a
    // stale simulation. Safety outranks everything (SHE-003), and an edited
    // operation outranks an ungenerated one for the same reason it does on
    // the Toolpaths chip — one of the two is currently showing a wrong
    // answer rather than no answer.
    if collisions > 0 {
        Some((format!("{collisions} collision(s)"), theme::ERROR))
    } else if stale_ops > 0 {
        Some((format!("{stale_ops} stale"), theme::WARNING))
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
