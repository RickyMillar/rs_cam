//! The Explore window: the frame, its size policy and its one body call.
//!
//! This file holds ONLY the container. The body it draws lives in
//! [`super::explore`]; the inspector surfaces in [`super::compare`] and
//! [`super::why`] are not reached from here at all.

use super::explore;
use super::shared::toolpath_name;
use crate::state::AppState;
use crate::ui::AppEvent;
use crate::ui_command::{NoArgs, UiCommand};

/// The window's own identity, independent of its title.
///
/// `egui::Window` derives its `Area` id from the title text, and this title
/// carries the toolpath name. Every re-selection therefore produced a NEW
/// area, so the window forgot the size and position the operator had given
/// it. A fixed id is what egui's own documentation prescribes for a changing
/// title.
const WINDOW_ID: &str = "feeds_explore_window";

/// Air left between the window frame and each screen edge.
const SCREEN_MARGIN: f32 = 48.0;

/// The smallest window this surface is still worth drawing in.
const MIN_WIDTH: f32 = 320.0;
const MIN_HEIGHT: f32 = 240.0;

/// The size the window takes when it has never been sized by hand.
///
/// This is what the body actually wants — the spindle policy row, the
/// nomogram, its legend and its readout — so a display with room shows the
/// whole surface and never draws a scroll bar. A display without room caps
/// it instead, and then the scroll bar earns its place.
///
/// The height dropped from 700 when the two mini charts were deleted
/// (`planning/feeds_rework_2026-09-15/PLAN.md` W4). Re-measure it if the
/// body gains a section; a default taller than the content leaves dead space
/// under the legend.
const DEFAULT_WIDTH: f32 = 640.0;
/// Measured against the rendered window on 2026-09-16: at 560 the legend
/// clipped after `Under spindle min`, hiding the three rows that name the
/// marks — `Now`, `Vendor target`, `Recommended`. Those are the rows a
/// reader who does not know the chart needs most, so the body has to hold
/// them without scrolling.
const DEFAULT_HEIGHT: f32 = 720.0;

/// Top-level draw entry. Short-circuits when no modal is open.
///
/// DC5a deleted the mode flip and the two-button header that drove it. The
/// window now holds ONE scope — this operation — so the title is one
/// expression and the body is one call. The project rollup answers at the
/// project's scope and lives in the Readiness workspace.
///
/// # The window may not outgrow the screen
///
/// Reported 2026-09-15: the window opened taller than the display, and the
/// title bar — which carries the only close button — sat ABOVE the top edge.
/// The operator could not shut the window they had opened.
///
/// The body is inherently tall: a 280-point nomogram over two 180-point mini
/// charts, plus their captions. A `Window` sizes itself to its content and
/// then centres that size, so a body taller than the screen pushes half the
/// overflow off each edge. Two builders hold it:
///
/// - `max_height` / `max_width` cap the OUTER size, title bar included.
/// - `vscroll` gives the body somewhere to go once it meets the cap, so the
///   cap hides nothing.
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

    // `content_rect` is the viewport minus anything the OS may cover —
    // a status bar or a display notch. The title bar must stay clear of
    // those too, so the cap is derived from it, not from the raw viewport.
    let screen = ctx.content_rect();
    let max_width = (screen.width() - SCREEN_MARGIN).max(MIN_WIDTH);
    let max_height = (screen.height() - SCREEN_MARGIN).max(MIN_HEIGHT);

    let mut still_open = true;
    egui::Window::new(format!("Explore feed vs RPM — {name}"))
        .id(egui::Id::new(WINDOW_ID))
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(DEFAULT_WIDTH.min(max_width))
        .default_height(DEFAULT_HEIGHT.min(max_height))
        .max_width(max_width)
        .max_height(max_height)
        .vscroll(true)
        .open(&mut still_open)
        .show(ctx, |ui| {
            explore::draw_modal_body(ui, state, toolpath_id, modal, events);
        });

    if !still_open {
        events.push(AppEvent::Ui(UiCommand::CloseFeedsModal(NoArgs)));
    }
}
