//! The one question a generation plan asks before it starts (R1).
//!
//! The Simulation panel holds a pinned cell size coarser than the rest the
//! plan machines needs. A plan that ran at the panel's value would carve the
//! rest operations against a snapshot that cannot see what the coarse tool
//! left, so the operator decides: take the finer cell, or cancel.
//!
//! The state lives on the controller, at
//! [`crate::controller::PlanResolutionConfirm`]. This file draws it and
//! answers nothing else; the sentry tests the state, not the pixels.

use crate::controller::PlanResolutionConfirm;

/// What the operator chose, or `None` while the question stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanResolutionChoice {
    /// Write the finer cell size to the panel and start the plan.
    UseRequired,
    /// Submit nothing. The panel is untouched.
    Cancel,
}

/// Draw the question. Modal: nothing behind it is clickable.
pub fn draw(ctx: &egui::Context, confirm: &PlanResolutionConfirm) -> Option<PlanResolutionChoice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("generation_resolution_confirm")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading("Simulation resolution");
        ui.add_space(8.0);
        ui.label(&confirm.message);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(&confirm.accept_label).clicked() {
                choice = Some(PlanResolutionChoice::UseRequired);
            }
            if ui.button("Cancel").clicked() {
                choice = Some(PlanResolutionChoice::Cancel);
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(PlanResolutionChoice::Cancel);
    }
    choice
}
