//! The trace modal: one cut metric's trace for the focused toolpath, large.
//!
//! Operator request 2026-10-02: "a `< >` like 'open' to open a modal that
//! just shows that one trace if you want to dig in". The `< >` button on a
//! cut-metric card sets `SimulationState::open_trace`; `app.rs` then calls
//! `AppState::open_cut_metric_trace`, which closes the other modals first
//! (modal exclusivity).
//!
//! The modal copies the load-warnings modal in `app.rs`: an `egui::Modal`
//! on the `SCRIM` backdrop, one primary "Close" button, and
//! `ModalResponse::should_close` for the Escape key and a click on the
//! backdrop.
//!
//! It draws the shared `Sparkline` in its detail face, so the colours, the
//! split at the bounds, the y scale (the whole run plus a margin, with every
//! limit line) and the min/max decimation per pixel column are the card's
//! own. The modal adds the zoom and the pan:
//!
//! - The scroll wheel (or a pinch) zooms the move axis about the pointer.
//! - A drag pans it.
//! - A double-click, or "Show the whole run", shows every move again.
//! - A click seeks the playhead through `SimJumpToMove`, the card's route.
//!
//! The modal closes itself when its trace stops being a measured one: no
//! cut trace, a stale simulation, no focused toolpath, or a metric that has
//! no measured card.

use super::AppEvent;
use super::components::{Button, Sparkline, sparkline, text};
use super::sim_diagnostics::{CutMetricSpec, is_advisory};
use crate::state::freshness::simulation_freshness;
use crate::state::runtime::GuiState;
use crate::state::simulation::SimulationState;
use crate::ui::tokens;
use crate::ui_command::{SimJumpToMoveArgs, UiCommand};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool_load::DistributionOutcome;

/// The egui id of the modal.
const MODAL_ID: &str = "cut_metric_trace_modal";

/// The narrowest and the widest the modal is, in points.
const MODAL_MIN_WIDTH: f32 = 480.0;
const MODAL_MAX_WIDTH: f32 = 1100.0;

/// The share of the window width the modal takes between those limits.
const MODAL_WIDTH_SHARE: f32 = 0.8;

/// The height of the trace, in points.
const TRACE_HEIGHT: f32 = 320.0;

/// The zoom per point of scroll. A scroll of 100 points zooms by e^0.2.
const ZOOM_PER_POINT: f64 = 0.002;

/// Draw the modal when a trace is open. A trace that no longer has a
/// measured card closes the modal.
pub fn draw(
    ctx: &egui::Context,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    events: &mut Vec<AppEvent>,
) {
    let Some(metric) = sim.open_trace else {
        return;
    };
    let has_trace = sim
        .results
        .as_ref()
        .is_some_and(|results| results.cut_trace.is_some());
    let fresh = has_trace && !simulation_freshness(session, sim).is_stale();
    let Some(toolpath_id) = sim.focused_toolpath().filter(|_| fresh) else {
        sim.open_trace = None;
        return;
    };
    let set = sim.cached_cut_metrics(session, gui.edit_counter, toolpath_id);
    let measured = set.cards.iter().find_map(|card| match &card.outcome {
        DistributionOutcome::Measured(distribution) if card.metric == metric => {
            Some((card, distribution))
        }
        _ => None,
    });
    let Some((card, distribution)) = measured else {
        sim.open_trace = None;
        return;
    };
    let Some(extent) = sparkline::move_extent(&card.series) else {
        sim.open_trace = None;
        return;
    };

    let spec = CutMetricSpec::of(metric);
    let name = sim
        .boundaries()
        .iter()
        .find(|boundary| boundary.id == toolpath_id)
        .map_or_else(
            || format!("TP {}", toolpath_id.0 + 1),
            |boundary| boundary.name.clone(),
        );
    let title = format!("{} \u{00b7} {name} ({})", spec.title, spec.unit);
    let playhead = sim
        .current_local_toolpath_move()
        .filter(|(_, id, _)| *id == toolpath_id)
        .map(|(_, _, local_move)| local_move);
    let move_offset = sim.global_move_for_local(toolpath_id, 0).unwrap_or(0);

    // The zoom window lives in `SimulationState::trace_window`, not in egui
    // memory (UI-09). `None` is the whole run. A window from another
    // toolpath is moved into this one's move range.
    let full = (extent.0 as f64, extent.1 as f64);
    let window = sim
        .trace_window
        .map_or(full, |window| sparkline::pan_window(window, full, 0.0));
    let shown_window = {
        let lo = window.0.floor().max(0.0) as usize;
        let hi = (window.1.ceil().max(0.0) as usize).max(lo);
        (lo, hi)
    };

    let width =
        (ctx.content_rect().width() * MODAL_WIDTH_SHARE).clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH);
    let modal = egui::Modal::new(egui::Id::new(MODAL_ID))
        .backdrop_color(tokens::SCRIM)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.label(text::display(title.as_str()));
            ui.label(text::caption(
                "Scroll to zoom, drag to pan, double-click to show the whole run. \
                 Click to go to a move.",
            ));
            ui.add_space(tokens::SPACE_3);
            let line = Sparkline::new(&card.series, &distribution.histogram, spec.unit)
                .scale(spec.scale)
                .advisory(is_advisory(distribution))
                .playhead(playhead)
                .height(TRACE_HEIGHT)
                .window(Some(shown_window))
                .move_offset(move_offset)
                .detail(true)
                .show(ui);
            let mut next = window;
            if line.response.double_clicked() {
                next = full;
            } else if line.response.dragged() {
                let delta = -f64::from(line.response.drag_delta().x) * line.moves_per_point;
                next = sparkline::pan_window(next, full, delta);
            } else if let Some(anchor) = line.pointer_move {
                let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                let factor = (-f64::from(scroll) * ZOOM_PER_POINT).exp() / f64::from(pinch);
                if (factor - 1.0).abs() > 1e-6 {
                    next = sparkline::zoom_window(next, full, anchor, factor);
                }
            }
            ui.add_space(tokens::SPACE_3);
            let close = ui
                .horizontal(|ui| {
                    let close = ui.add(Button::primary("Close")).clicked();
                    if ui.add(Button::quiet("Show the whole run")).clicked() {
                        next = full;
                    }
                    close
                })
                .inner;
            (next, line.clicked_move, close)
        });

    let (next, clicked_move, close_clicked) = modal.inner;
    if close_clicked || modal.should_close() {
        sim.open_trace = None;
        sim.trace_window = None;
        return;
    }
    sim.trace_window = Some(next);
    if let Some(local_move) = clicked_move
        && let Some(move_index) = sim.global_move_for_local(toolpath_id, local_move)
    {
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index,
        })));
    }
}
