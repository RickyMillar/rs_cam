//! The live "Getting started" list of the empty inspector (W5, P13).
//!
//! The list was seven static labels. Each step now reads the project: a
//! done step shows a tick and is muted, the first step that is not done is
//! the next step and is highlighted. Each step is a button that does the
//! step or goes to the place of the step, through a route that already
//! exists (the File menu import, Setup › Stock, the tool library, the add
//! menu of the rail, Generate All, the Simulation workspace, the export
//! wizard).

use crate::state::selection::Selection;
use crate::state::{AppState, Workspace};
use crate::ui::AppEvent;
use crate::ui::components::Button;
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

/// The steps, in order. Simulation comes before export (G-WSMENU,
/// 2026-09-10): simulation is where collisions, air cutting and the
/// tool-load gates are measured, so it is a step of its own.
pub(crate) const STEPS: [&str; 7] = [
    "Import a model",
    "Configure the stock",
    "Add a cutting tool",
    "Create a toolpath",
    "Generate toolpaths",
    "Simulate and review",
    "Export G-code",
];

/// The step that the operator can always do again: the export has no
/// record in the project, so it never shows a tick.
const EXPORT: usize = 6;

/// Which steps the project has done. `done[EXPORT]` is always false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Progress {
    pub done: [bool; 7],
}

impl Progress {
    /// Read the steps from the project.
    pub(crate) fn of(state: &AppState) -> Self {
        let session = state.session();
        let stock = session.stock_config();
        let enabled: Vec<_> = session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.id)
            .collect();
        let generated = !enabled.is_empty()
            && enabled.iter().all(|&id| {
                matches!(
                    rs_cam_core::session::toolpath_run_state(session, id, false),
                    Some(rs_cam_core::session::RunState::Done)
                )
            });
        Self {
            done: [
                !session.models().is_empty(),
                session.stock_thickness_refusal().is_none() && stock.x > 0.0 && stock.y > 0.0,
                !session.tools().is_empty(),
                !session.toolpath_configs().is_empty(),
                generated,
                state.simulation.has_results(),
                false,
            ],
        }
    }

    /// The first step that is not done.
    pub(crate) fn next(&self) -> usize {
        self.done.iter().position(|d| !d).unwrap_or(EXPORT)
    }

    /// True while a step before the export is not done: the empty
    /// inspector then shows the list.
    pub(crate) fn shows_list(&self) -> bool {
        self.next() < EXPORT
    }
}

/// Draw the list. The caller draws it only while nothing is selected and
/// [`Progress::shows_list`] is true.
pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &AppState,
    progress: Progress,
    events: &mut Vec<AppEvent>,
) {
    crate::ui::components::SectionHeader::new("Getting started")
        .small_caps()
        .show(ui);
    ui.add_space(tokens::SPACE_2);
    let next = progress.next();
    let has_toolpath = progress.done[3];
    let has_setup = !state.session().list_setups().is_empty();
    for (index, (text, &done)) in STEPS.iter().zip(progress.done.iter()).enumerate() {
        ui.horizontal(|ui| {
            let (marker, colour) = if done {
                ("\u{2713}".to_owned(), tokens::OK)
            } else if index == next {
                ("\u{203A}".to_owned(), tokens::ACCENT)
            } else {
                (format!("{}.", index + 1), tokens::TEXT_FAINT)
            };
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(tokens::ROW_DENSE, tokens::ROW_ACTION),
                egui::Sense::hover(),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                marker,
                egui::TextStyle::Body.resolve(ui.style()),
                colour,
            );
            // A done step is muted; the next step is the raised button.
            let button = if index == next {
                Button::new(*text)
            } else {
                Button::quiet(*text)
            };
            let button = match index {
                3 if !has_setup => button.enabled(false).disabled_reason("Add a setup first."),
                4 if !has_toolpath => button
                    .enabled(false)
                    .disabled_reason("Create a toolpath first."),
                _ => button,
            };
            let response = ui.add(button);
            step_action(index, &response, state, events);
        });
    }
}

/// The route of each step. Step 4 opens the add menu of the rail on the
/// first setup; the others push one existing event.
fn step_action(
    index: usize,
    response: &egui::Response,
    state: &AppState,
    events: &mut Vec<AppEvent>,
) {
    if index == 3 {
        if let Some(setup_id) = state.session().list_setups().first().map(|s| s.id) {
            egui::Popup::menu(response).show(|ui| {
                crate::ui::toolpath_panel::add_menu_items(ui, setup_id, state, events);
            });
        }
        return;
    }
    if !response.clicked() {
        return;
    }
    match index {
        0 => crate::ui::setup_panel::import_model_from_disk(events),
        1 => {
            events.push(AppEvent::Ui(UiCommand::SwitchWorkspace(Workspace::Setup)));
            events.push(AppEvent::Ui(UiCommand::Select(Selection::Stock)));
        }
        2 => events.push(AppEvent::Ui(UiCommand::OpenToolLibrary(NoArgs))),
        4 => events.push(AppEvent::GenerateAll),
        5 => events.push(AppEvent::Ui(UiCommand::SwitchWorkspace(
            Workspace::Simulation,
        ))),
        _ => events.push(AppEvent::Ui(UiCommand::OpenExportWizard(NoArgs))),
    }
}

#[cfg(test)]
mod tests {
    use super::{Progress, STEPS};
    use crate::state::AppState;

    /// P13: a new project has done nothing; the next step is the import
    /// and the list shows. The default stock has no thickness (S11), so
    /// the stock step does not tick before the operator sets one.
    #[test]
    fn a_new_project_starts_at_the_import_p13() {
        let state = AppState::new();
        let progress = Progress::of(&state);
        assert_eq!(progress.done, [false; 7]);
        assert_eq!(progress.next(), 0);
        assert!(progress.shows_list());
        assert_eq!(STEPS.len(), progress.done.len());
    }

    /// P13: the ticks follow the project. A stock with a thickness and a
    /// tool tick steps 2 and 3; the import stays the next step.
    #[test]
    fn the_ticks_follow_the_project_p13() {
        use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
        let stock = rs_cam_core::compute::stock_config::StockConfig {
            z: Some(18.0),
            ..Default::default()
        };
        let mut state = AppState::new();
        state.replace_session(
            rs_cam_core::session::ProjectSessionBuilder::new()
                .stock(stock)
                .tool(ToolConfig::test_default(
                    ToolId::from_raw(1),
                    ToolType::EndMill,
                ))
                .build(),
        );
        let progress = Progress::of(&state);
        assert_eq!(
            progress.done,
            [false, true, true, false, false, false, false]
        );
        assert_eq!(progress.next(), 0);
    }
}
