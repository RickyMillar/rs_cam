//! The shared confirmation for removing a setup.

use crate::state::AppState;
use crate::ui::AppEvent;

/// Draw the confirmation requested by either setup surface.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(confirm) = state.panels.pending_setup_removal.as_ref() else {
        return;
    };
    let setup_id = confirm.setup_id;
    let response = egui::Modal::new(egui::Id::new("remove_setup_confirm")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading("Remove setup?");
        ui.add_space(8.0);
        ui.label(format!(
            "Remove '{}' and all toolpaths it contains?",
            confirm.setup_name
        ));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                events.push(AppEvent::CancelRemoveSetup);
            }
            if ui
                .button(egui::RichText::new("Remove Setup").color(crate::ui::tokens::DANGER))
                .clicked()
            {
                events.push(AppEvent::RemoveSetup(setup_id));
            }
        });
    });
    if response.should_close() {
        events.push(AppEvent::CancelRemoveSetup);
    }
}
