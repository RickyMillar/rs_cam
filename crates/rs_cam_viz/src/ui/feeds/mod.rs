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

use self::shared::toolpath_name;
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
const DEFAULT_HEIGHT: f32 = 560.0;

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
