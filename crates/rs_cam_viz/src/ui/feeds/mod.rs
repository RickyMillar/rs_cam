//! The Feeds and Speeds surfaces.
//!
//! DC5a measured `ui/feeds_modal.rs` at 3 450 lines and found that its draw
//! functions divide into four jobs that share nothing but a window, at TWO
//! different scopes:
//!
//! | File | Job | Scope |
//! |---|---|---|
//! | [`compare`] | compare and apply | this operation |
//! | [`why`] | the detail behind the recommendation | this operation |
//! | [`explore`] | the nomogram and the mini charts | this operation |
//! | [`shared`] | what more than one job needs | — |
//!
//! There was a FOURTH job, and it is the finding: a rollup over every
//! toolpath, at THE WHOLE PROJECT's scope, held in this window behind a
//! `FeedsModalMode` flip. A container holds one scope, so the rollup left.
//! It lives in `ui/readiness_panel.rs`, on the workspace that already
//! answers project-wide questions, and it took its own state with it
//! (`AppState::project_feeds`). The mode flip is deleted.
//!
//! This split moved every function VERBATIM. Only the visibility and the
//! `use` paths changed, so the split and the behaviour change can be reviewed
//! apart.

pub(crate) mod compare;
pub(crate) mod explore;
pub(crate) mod shared;
pub(crate) mod why;

use self::compare::{draw_spindle_strategy_row, draw_toolpath_view};
use self::shared::toolpath_name;
use crate::state::AppState;
use crate::ui::AppEvent;
use crate::ui_command::{NoArgs, UiCommand};

/// Top-level draw entry. Short-circuits when no modal is open.
///
/// DC5a deleted the mode flip and the two-button header that drove it. The
/// window now holds ONE scope — this operation — so the title is one
/// expression and the body is one call. The project rollup answers at the
/// project's scope and lives in the Readiness workspace.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(modal) = state.feeds_modal.as_ref() else {
        return;
    };
    let toolpath_id = modal.toolpath_id;
    let Some(name) = toolpath_name(state, toolpath_id) else {
        // The toolpath is gone (project changed under an open modal).
        // Pre-fix this rendered a broken shell titled "toolpath N" — caught
        // by the 2026-06-11 capture sweep.
        events.push(AppEvent::Ui(UiCommand::CloseFeedsModal(NoArgs)));
        return;
    };

    let mut still_open = true;
    // DC5a re-sized the window. 960 x 620 was cut for the project table,
    // which held a checkbox column, six numeric columns and a scatter plot
    // for every toolpath in the project. What remains is a 380-point
    // comparison column beside the charts, so the width falls to 760.
    egui::Window::new(format!("Feeds & Speeds — {name}"))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(760.0)
        .default_height(560.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_spindle_strategy_row(
                ui,
                state.session.post_config().spindle_strategy,
                state.session.machine(),
                events,
            );
            ui.separator();
            draw_toolpath_view(ui, state, toolpath_id, modal, events);
        });

    if !still_open {
        events.push(AppEvent::Ui(UiCommand::CloseFeedsModal(NoArgs)));
    }
}
