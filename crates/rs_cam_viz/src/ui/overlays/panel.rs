//! The All viewport options catalogue, and the row renderer that the
//! viewport dock shares with it (viewport redesign, MOCKUPS §10).
//!
//! The catalogue replaces the old Overlays panel. It is a floating window,
//! not a docked column, so it takes no width from the 3D view. It lists
//! every registry row in every state, grouped by dock section, with a search
//! field and a filter. The dock (`crate::ui::viewport_overlay`) is the daily
//! surface; the catalogue is the complete list.
//!
//! It is a **viewer, not an editor** (UX §6.6). Choosing a rest source or
//! setting a boundary offset stays in the properties panel; a disabled row's
//! compute button runs the thing its reason names and nothing else.
//!
//! It **caches nothing**. Every flag is read from [`AppState`] each frame,
//! because the multi-tool planner writes `show_tier_preview` on its own and
//! the workspace switch rewrites a dozen flags behind its back (audit §5,
//! rule 2). A catalogue row and a dock row call the SAME setter,
//! [`registry::set_overlay`], so the catalogue is a second view of one
//! state, not a second writer.

use crate::render::camera::ProjectionMode;
use crate::state::AppState;
use crate::state::overlays::{CatalogueFilter, DockSection, PendingShow};
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::components;
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

use super::live::{self, JobLane, Live, Recovery};
use super::registry::{self, OverlayAction, OverlayRow, OverlaySurface};

/// The catalogue window's widest default width.
const CATALOGUE_WIDTH: f32 = 360.0;

/// The 3D view's floor, in points.
///
/// P6, 2026-09-08: `screenshot_gui` at the window's own 1400 x 900 came back
/// with **no 3D viewport at all** — the operation list and the inspector
/// filled the frame. The old docked Overlays column took a fixed width out
/// of whatever the two side panels had left, and `app.rs` then handed
/// `ui.available_size()` straight to `allocate_exact_size`, so once the side
/// panels had been dragged wide (egui remembers a resizable panel's width
/// across frames and windows, and does not shrink it when the window does)
/// the remainder went to zero and the viewport vanished silently. A viewport
/// that can reach zero width is a screenshot surface that can return a
/// picture of nothing.
///
/// The viewport redesign removed the docked column. The dock and the
/// catalogue float over the 3D view and take no width from it. The floor
/// stays: `app/viewport.rs` floors its own allocation at this width, and
/// the dock and the catalogue fit themselves inside a viewport this narrow.
pub const MIN_VIEWPORT_WIDTH: f32 = 320.0;

// ── the row state ──────────────────────────────────────────────────────────

/// The word the dock shows while a job runs. It matches the inspector's
/// `reach: computing…`.
pub const COMPUTING_WORD: &str = "computing\u{2026}";

/// The word the dock shows after a run failed.
pub const FAILED_WORD: &str = "failed";

/// How a registry row presents now. It is derived each frame from the flag,
/// the precondition and the live analysis state ([`super::live`]), and never
/// stored.
///
/// The plan names seven states, and each one is an arm here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    /// The row can draw, and its flag is off.
    Off,
    /// The row can draw, and its flag is on. This is the FLAG, not the
    /// rendered result; the legend rail reads the rendered result.
    Showing,
    /// The row cannot draw until the named work runs.
    NeedsCompute {
        reason: String,
        action: OverlayAction,
    },
    /// A job for the row's analysis runs now. `switchable` is `true` when
    /// the precondition lets the operator set the flag while it runs (the
    /// reach map: the scheduler follows the selection, not the flag).
    Computing { lane: JobLane, switchable: bool },
    /// The row cannot draw, and no button makes it drawable here.
    Blocked { reason: String },
    /// The row draws a result that is older than its inputs.
    Stale {
        note: String,
        recovery: Option<Recovery>,
    },
    /// The last run of the row's analysis ended in an error.
    Failed { message: String, recovery: Recovery },
}

impl RowState {
    /// Derive the state of `row` now.
    ///
    /// The live analysis state speaks first: a running job is Computing even
    /// when an old result could draw (inventory §2.3, item 1), and a failed
    /// run is Failed rather than a Blocked reason with the wrong words
    /// (item 2). A stale result needs a drawable row, so Stale follows a
    /// `Ready` precondition only.
    pub fn of(state: &AppState, row: &OverlayRow) -> Self {
        let precondition = (row.precondition)(state);
        match live::live(state, row) {
            Live::Computing(lane) => {
                return Self::Computing {
                    lane,
                    switchable: precondition.is_ready(),
                };
            }
            Live::Failed { message, recovery } => return Self::Failed { message, recovery },
            Live::Stale { note, recovery } if precondition.is_ready() => {
                return Self::Stale { note, recovery };
            }
            Live::Stale { .. } | Live::Idle => {}
        }
        match (precondition.reason(), precondition.compute()) {
            (None, _) => {
                if (row.get)(state) {
                    Self::Showing
                } else {
                    Self::Off
                }
            }
            (Some(reason), Some(action)) => Self::NeedsCompute {
                reason: reason.to_owned(),
                action,
            },
            (Some(reason), None) => Self::Blocked {
                reason: reason.to_owned(),
            },
        }
    }

    /// Can the operator switch the row now?
    pub fn is_ready(&self) -> bool {
        match self {
            Self::Off | Self::Showing | Self::Stale { .. } => true,
            Self::Computing { switchable, .. } => *switchable,
            Self::NeedsCompute { .. } | Self::Blocked { .. } | Self::Failed { .. } => false,
        }
    }

    /// Does the row draw its data now, from a current or a stale result?
    pub fn draws(&self) -> bool {
        matches!(self, Self::Showing | Self::Stale { .. })
    }

    /// The text that says why the row does not simply draw, when it does
    /// not.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Off | Self::Showing => None,
            Self::NeedsCompute { reason, .. } | Self::Blocked { reason } => Some(reason.as_str()),
            Self::Computing { .. } => Some(COMPUTING_WORD),
            Self::Stale { note, .. } => Some(note.as_str()),
            Self::Failed { message, .. } => Some(message.as_str()),
        }
    }

    /// The button that makes the row drawable, when one exists.
    pub fn action(&self) -> Option<OverlayAction> {
        match self {
            Self::NeedsCompute { action, .. } => Some(*action),
            Self::Off
            | Self::Showing
            | Self::Computing { .. }
            | Self::Blocked { .. }
            | Self::Stale { .. }
            | Self::Failed { .. } => None,
        }
    }

    /// The glyph that marks the state on a section button and in a hover.
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Off | Self::Showing => "",
            Self::NeedsCompute { .. } | Self::Blocked { .. } => tokens::GLYPH_UNKNOWN,
            Self::Computing { .. } => GLYPH_COMPUTING,
            Self::Stale { .. } => tokens::GLYPH_CAUTION,
            Self::Failed { .. } => tokens::GLYPH_DANGER,
        }
    }

    /// A stable name for the arm, for the sentries.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Showing => "showing",
            Self::NeedsCompute { .. } => "needs_compute",
            Self::Computing { .. } => "computing",
            Self::Blocked { .. } => "blocked",
            Self::Stale { .. } => "stale",
            Self::Failed { .. } => "failed",
        }
    }
}

/// The glyph of a running job (MOCKUPS §5). The row itself draws a spinner;
/// a section button, which holds text only, draws this glyph.
pub const GLYPH_COMPUTING: &str = "\u{25CC}";

// ── the shared row renderer ────────────────────────────────────────────────

/// How a row's control looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowControl {
    /// A checkbox that switches the row on and off.
    Check,
    /// A radio choice. A click switches the row on; per-surface exclusivity
    /// switches its peers off.
    Choice,
}

/// One row with its registry label: the control, then the state lines and
/// the action buttons when it does not simply draw.
pub(crate) fn draw_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
) {
    let control = if row.radio {
        RowControl::Choice
    } else {
        RowControl::Check
    };
    draw_control(ui, state, row, row.label, control);
    draw_reason(ui, state, events, row);
}

/// The control of one row alone, with a label the caller chooses.
///
/// The dock groups related rows under one caption (`Move colour`, `Model
/// colour`), so it shows `Palette` where the catalogue shows the full
/// registry label. The setter is the same.
pub(crate) fn draw_control(
    ui: &mut egui::Ui,
    state: &mut AppState,
    row: &'static OverlayRow,
    label: &str,
    control: RowControl,
) -> egui::Response {
    let row_state = RowState::of(state, row);
    let ready = row_state.is_ready();
    let mut on = (row.get)(state);
    let hover = match row_state.reason() {
        Some(reason) => format!("{}\n\n{} {reason}", row.hover, row_state.glyph()),
        None => row.hover.to_owned(),
    };
    let response = match control {
        RowControl::Choice => {
            let resp = ui.add_enabled(ready, egui::RadioButton::new(on, label));
            if resp.clicked() && !on {
                registry::set_overlay(state, row, true);
            }
            resp
        }
        RowControl::Check => {
            let resp = ui.add_enabled(ready, egui::Checkbox::new(&mut on, label));
            if resp.changed() {
                registry::set_overlay(state, row, on);
            }
            resp
        }
    };
    response.on_hover_text(hover)
}

/// One indented caption line under a row.
fn state_line(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.label(
            egui::RichText::new(text)
                .size(tokens::SIZE_CAPTION)
                .color(color),
        );
    });
}

/// The state lines and the buttons of a row that does not simply draw.
/// Draws nothing for a row in the Off or the Showing state.
pub(crate) fn draw_reason(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
) {
    match RowState::of(state, row) {
        RowState::Off | RowState::Showing => {}
        RowState::Blocked { reason } => {
            state_line(
                ui,
                &format!("{} {reason}", tokens::GLYPH_UNKNOWN),
                tokens::TEXT_MUTED,
            );
        }
        RowState::NeedsCompute { reason, action } => {
            state_line(
                ui,
                &format!("{} {reason}", tokens::GLYPH_UNKNOWN),
                tokens::TEXT_MUTED,
            );
            compute_buttons(ui, state, events, row, action);
        }
        RowState::Computing { lane, .. } => computing_line(ui, events, lane),
        RowState::Stale { note, recovery } => {
            state_line(
                ui,
                &format!("{} stale \u{2014} {note}", tokens::GLYPH_CAUTION),
                tokens::CAUTION,
            );
            if let Some(recovery) = recovery {
                recovery_button(ui, state, events, recovery);
            }
        }
        RowState::Failed { message, recovery } => {
            state_line(
                ui,
                &format!("{} {FAILED_WORD}", tokens::GLYPH_DANGER),
                tokens::DANGER,
            );
            state_line(ui, &message, tokens::TEXT_MUTED);
            recovery_button(ui, state, events, recovery);
        }
    }
}

/// The running-job line: a spinner, the word, the lane on the status bar,
/// and the one cancel that exists (MOCKUPS §5).
///
/// `UiCommand::CancelCompute` cancels EVERY lane, so the label says so.
fn computing_line(ui: &mut egui::Ui, events: &mut Vec<AppEvent>, lane: JobLane) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.add(egui::Spinner::new().size(tokens::SIZE_CAPTION));
        ui.label(
            egui::RichText::new(COMPUTING_WORD)
                .size(tokens::SIZE_CAPTION)
                .color(tokens::TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new(lane.pointer())
                .size(tokens::SIZE_CAPTION)
                .color(tokens::TEXT_MUTED),
        );
        if ui
            .add(components::Button::new("Cancel all jobs"))
            .on_hover_text("Cancels every running and queued compute job, not only this one.")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::CancelCompute(NoArgs)));
        }
    });
}

/// The `Compute` and `Compute & show` buttons of a row that needs compute.
///
/// The two have distinct outcomes. `Compute` runs the work and leaves the
/// flag as it is. `Compute & show` also records a [`PendingShow`] tagged
/// with the target and the preferred mode; [`settle_pending_show`] applies
/// it when the data lands, and only if both still match. A row whose flag
/// is already on shows its data on arrival, so it gets the plain button
/// only.
fn compute_buttons(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
    action: OverlayAction,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(tokens::SPACE_5);
        if action == OverlayAction::EnableRestAnalysis {
            // The ellipsis says that a confirm follows (MOCKUPS §4, B2).
            if ui
                .add(components::Button::new(format!("{}\u{2026}", action.label())))
                .on_hover_text("Says what the work changes before it starts.")
                .clicked()
            {
                run_action(state, events, action, row, ComputeOutcome::ComputeOnly);
            }
            return;
        }
        let plain_hover = if action.is_compute() {
            "Run the work. The row stays as it is."
        } else {
            "Open the dialog that makes this row drawable."
        };
        if ui
            .add(components::Button::new(action.label()))
            .on_hover_text(plain_hover)
            .clicked()
        {
            run_action(state, events, action, row, ComputeOutcome::ComputeOnly);
        }
        if let Some(show_label) = action.show_label()
            && !(row.get)(state)
            && ui
                .add(components::Button::new(show_label))
                .on_hover_text(
                    "Run the work, then show the row when the data lands \u{2014} if the target and the chosen mode have not changed.",
                )
                .clicked()
        {
            run_action(state, events, action, row, ComputeOutcome::ComputeAndShow);
        }
    });
}

/// The button that recovers a stale or a failed row.
fn recovery_button(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    recovery: Recovery,
) {
    let (label, hover) = match recovery {
        Recovery::RetryReach => (
            "Retry".to_owned(),
            live::selected_toolpath(state).map_or_else(
                || "Ask for the reach map again.".to_owned(),
                |id| {
                    format!(
                        "Ask for the reach map of {} again.",
                        live::toolpath_tag(state, id)
                    )
                },
            ),
        ),
        Recovery::Generate(id) => (
            format!("Generate {}", live::toolpath_tag(state, id)),
            "Generate this operation again.".to_owned(),
        ),
        Recovery::RunCollisionCheck => (
            OverlayAction::RunCollisionCheck.label().to_owned(),
            "Run the collision check again on the current project.".to_owned(),
        ),
    };
    ui.horizontal(|ui| {
        ui.add_space(tokens::SPACE_5);
        if ui
            .add(components::Button::new(label))
            .on_hover_text(hover)
            .clicked()
        {
            run_recovery(state, events, recovery);
        }
    });
}

/// Run a recovery. Each arm is an intent: the scheduler or the controller
/// does the work, and the draw loop starts no compute.
pub fn run_recovery(state: &mut AppState, events: &mut Vec<AppEvent>, recovery: Recovery) {
    match recovery {
        // The same move as the stall recovery in
        // `AppController::process_reach_overlay`: forget the key, and the
        // next sweep submits again.
        Recovery::RetryReach => state.gui.reach_overlay.clear(),
        Recovery::Generate(id) => events.push(AppEvent::GenerateToolpath(id)),
        Recovery::RunCollisionCheck => events.push(AppEvent::RunCollisionCheck),
    }
}

/// The two outcomes of a compute button (PLAN, "Compute and Compute & show
/// have distinct outcomes").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeOutcome {
    /// Run the work. The flag stays as it is.
    ComputeOnly,
    /// Run the work, and show the row when its data lands, if the target
    /// and the preferred mode still match the request.
    ComputeAndShow,
}

/// Run the compute action of `row` when the row needs compute. Returns
/// `false`, and does nothing, for a row in any other state.
///
/// `Compute rest` opens its confirm here; the confirm's own buttons start
/// the work through [`compute_rest`].
pub fn request_compute(
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
    outcome: ComputeOutcome,
) -> bool {
    let RowState::NeedsCompute { action, .. } = RowState::of(state, row) else {
        return false;
    };
    run_action(state, events, action, row, outcome);
    true
}

/// Run a row's compute affordance.
///
/// `row` is the row whose precondition offered `action`. With
/// [`ComputeOutcome::ComputeAndShow`], the row is switched on later by
/// [`settle_pending_show`], never here: switching it on at the click would
/// let the result take over the rendering whatever the operator did while
/// the work ran.
fn run_action(
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    action: OverlayAction,
    row: &'static registry::OverlayRow,
    outcome: ComputeOutcome,
) {
    match action {
        OverlayAction::RunCollisionCheck => events.push(AppEvent::RunCollisionCheck),
        OverlayAction::OpenPlanner => {
            events.push(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
        }
        OverlayAction::GenerateAll => events.push(AppEvent::GenerateAll),
        OverlayAction::RecordGeneratorTrace => {
            events.push(AppEvent::SetGeneratorTraceCaptureAll(true));
            events.push(AppEvent::GenerateAll);
        }
        // W2 (G-STARTFROM): switching the heatmap on IS the demand for a
        // rest grid. The viewport redesign puts a confirm before the work,
        // because the regeneration can replace the simulation (MOCKUPS §4,
        // B2). The confirm's buttons call `compute_rest`.
        OverlayAction::EnableRestAnalysis => {
            if let Some(id) = live::selected_toolpath(state) {
                state.overlays.rest_confirm = Some(id);
            }
            return;
        }
    }
    if outcome == ComputeOutcome::ComputeAndShow && action.is_compute() {
        record_pending_show(state, row, live::selected_toolpath(state));
    }
}

/// Start the rest work for toolpath `id`: the outcome of the `Compute rest`
/// confirm.
///
/// The row does not apply the command itself. It calls the one door that
/// owns the stamp, the edited mark and the simulation invalidation, then
/// asks for the generation that fills the grid.
pub fn compute_rest(
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    id: ToolpathId,
    outcome: ComputeOutcome,
) {
    state.overlays.rest_confirm = None;
    crate::ui::properties::apply_auto_enable(state, id);
    events.push(AppEvent::GenerateToolpath(id));
    if outcome == ComputeOutcome::ComputeAndShow
        && let Some(row) = registry::row("rest_heatmap")
    {
        record_pending_show(state, row, Some(id));
    }
}

/// The preferred mode that a `Compute & show` request is tagged with: the
/// row that is on for the surface of `row`, or for a stacking row, the row
/// itself when it is on.
pub fn preferred_mode(state: &AppState, row: &OverlayRow) -> Option<&'static str> {
    if row.surface == OverlaySurface::None {
        return (row.get)(state).then_some(row.id);
    }
    registry::rows()
        .iter()
        .find(|peer| peer.surface == row.surface && (peer.get)(state))
        .map(|peer| peer.id)
}

fn record_pending_show(state: &mut AppState, row: &'static OverlayRow, target: Option<ToolpathId>) {
    state.overlays.pending_show = Some(PendingShow {
        row_id: row.id,
        target,
        workspace: state.workspace,
        preferred: preferred_mode(state, row),
        seen_computing: false,
    });
}

/// What [`settle_pending_show`] did this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowSettle {
    /// No request waits.
    NoRequest,
    /// The request waits for its data.
    Waiting,
    /// The data landed for the same target and mode; the row is on now.
    Applied,
    /// The request ended without a change: the target or the mode changed,
    /// the run failed, or it ended with no data.
    Dropped,
}

/// Apply the waiting `Compute & show` request when its data has landed.
///
/// This is the landing path. It runs once per frame before the dock draws,
/// and it reads live state only. The request applies only when the
/// selected toolpath, the workspace and the preferred mode all still match
/// the tags that the request carries. Otherwise the request ends and the
/// rendering stays as the operator left it: completion of async work never
/// takes over the rendering after the operator changed the selection or the
/// intent.
pub fn settle_pending_show(state: &mut AppState) -> ShowSettle {
    let Some(request) = state.overlays.pending_show.clone() else {
        return ShowSettle::NoRequest;
    };
    let Some(row) = registry::row(request.row_id) else {
        state.overlays.pending_show = None;
        return ShowSettle::Dropped;
    };
    if live::selected_toolpath(state) != request.target
        || state.workspace != request.workspace
        || preferred_mode(state, row) != request.preferred
    {
        state.overlays.pending_show = None;
        return ShowSettle::Dropped;
    }
    match RowState::of(state, row) {
        RowState::Off => {
            registry::set_overlay(state, row, true);
            state.overlays.pending_show = None;
            ShowSettle::Applied
        }
        RowState::Computing { .. } => {
            if let Some(pending) = state.overlays.pending_show.as_mut() {
                pending.seen_computing = true;
            }
            ShowSettle::Waiting
        }
        // A stale result is the OLD data. Wait for the new one.
        RowState::Stale { .. } => ShowSettle::Waiting,
        RowState::NeedsCompute { .. } | RowState::Blocked { .. } => {
            if request.seen_computing {
                // The work ran and left no data.
                state.overlays.pending_show = None;
                ShowSettle::Dropped
            } else {
                ShowSettle::Waiting
            }
        }
        RowState::Showing | RowState::Failed { .. } => {
            state.overlays.pending_show = None;
            ShowSettle::Dropped
        }
    }
}

// ── the Compute rest confirm ───────────────────────────────────────────────

/// The consequence line of the `Compute rest` confirm. It shows only when a
/// simulation exists.
///
/// `Command::AutoEnableRestAnalysis` never clears the simulation: the core
/// drops the toolpath's cached result only, so `Effects::simulation_cleared`
/// is always `false` for it. The generation that follows is a plan
/// (`handle_generate_toolpath`), and the GUI plan ends with
/// `PlanStep::SimulateAll`, which runs the simulation again when the plan
/// generated anything. The plan can also refuse to start (another plan
/// runs) or wait on the resolution question, so the word is "may".
pub const REST_CONFIRM_CONSEQUENCE: &str =
    "The generation then runs the simulation again, so the current simulation may be replaced.";

/// The body of the `Compute rest` confirm.
pub const REST_CONFIRM_BODY: &str =
    "This switches on rest analysis for this operation and regenerates it.";

/// The `Compute rest` confirm (MOCKUPS §4, B2). A no-op unless a row asked
/// for it. No work starts until the operator picks `Compute` or
/// `Compute & show`.
pub fn draw_rest_confirm(ctx: &egui::Context, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let Some(id) = state.overlays.rest_confirm else {
        return;
    };
    let name = state
        .session
        .find_toolpath_config_by_id(id)
        .map(|(_, tc)| tc.name.clone())
        .unwrap_or_default();
    let title = format!("Compute rest for {} {name}", live::toolpath_tag(state, id));
    let simulation_exists = state.simulation.has_results();
    let mut choice: Option<Option<ComputeOutcome>> = None;
    let response = egui::Modal::new(egui::Id::new("compute_rest_confirm")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(title.as_str());
        ui.add_space(tokens::SPACE_3);
        ui.label(REST_CONFIRM_BODY);
        if simulation_exists {
            ui.label(
                egui::RichText::new(format!(
                    "{} {REST_CONFIRM_CONSEQUENCE}",
                    tokens::GLYPH_CAUTION
                ))
                .color(tokens::CAUTION),
            );
        }
        ui.add_space(tokens::SPACE_4);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(components::Button::primary("Compute"))
                .on_hover_text("Compute the rest grid. The Rest colour stays off.")
                .clicked()
            {
                choice = Some(Some(ComputeOutcome::ComputeOnly));
            }
            if ui
                .add(components::Button::new("Compute & show"))
                .on_hover_text(
                    "Compute the rest grid, then show it if this operation is still selected and the model colour has not changed.",
                )
                .clicked()
            {
                choice = Some(Some(ComputeOutcome::ComputeAndShow));
            }
            if ui.add(components::Button::new("Cancel")).clicked() {
                choice = Some(None);
            }
        });
    });
    match choice {
        Some(Some(outcome)) => compute_rest(state, events, id, outcome),
        Some(None) => state.overlays.rest_confirm = None,
        None if response.should_close() => state.overlays.rest_confirm = None,
        None => {}
    }
}

/// The simulated stock's opacity, a slider rather than a registry row —
/// named as the one gap against the audit's KEEP set. The dock draws it
/// under `Simulated stock` in Scene, and the catalogue draws it under the
/// same row.
pub(crate) fn draw_stock_opacity(ui: &mut egui::Ui, state: &mut AppState) {
    let enabled = state.viewport.show_sim_stock && state.simulation.has_results();
    ui.horizontal(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.label(
            egui::RichText::new("Opacity")
                .size(tokens::SIZE_CAPTION)
                .color(tokens::TEXT_MUTED),
        );
        ui.add_enabled(
            enabled,
            egui::Slider::new(&mut state.simulation.stock_opacity, 0.0..=1.0).show_value(true),
        )
        .on_hover_text(
            "Affects the simulated stock only \u{2014} the solid stock block and the height planes are pinned at 0.15.",
        );
    });
}

// ── the catalogue ──────────────────────────────────────────────────────────

/// Does `row` pass the search text and the filter?
fn matches_filter(
    state: &AppState,
    row: &OverlayRow,
    query: &str,
    filter: CatalogueFilter,
) -> bool {
    let query = query.trim().to_lowercase();
    let text_ok =
        query.is_empty() || row.label.to_lowercase().contains(&query) || row.id.contains(&query);
    let filter_ok = match filter {
        CatalogueFilter::All => true,
        CatalogueFilter::Changed => {
            (row.default_for)(state.workspace).is_some_and(|default| (row.get)(state) != default)
        }
        CatalogueFilter::CannotDraw => !(row.precondition)(state).is_ready(),
    };
    text_ok && filter_ok
}

/// The catalogue section header: `VIEW`, `SCENE`, `PATHS: SELECTED`,
/// `INSPECT: REACH`.
fn section_title(state: &AppState, section: DockSection) -> String {
    let base = section.label().to_uppercase();
    match crate::ui::viewport_overlay::section_suffix(state, section) {
        Some(suffix) => format!("{base}: {}", suffix.word.to_uppercase()),
        None => base,
    }
}

/// The All viewport options window. A no-op unless the catalogue is open.
///
/// It opens at the viewport's top-left corner and never wider than the
/// viewport less a gutter on each side, so a 320 pt viewport keeps it
/// inside its own width.
pub fn draw_catalogue(
    ctx: &egui::Context,
    state: &mut AppState,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
    viewport_rect: egui::Rect,
) {
    if !state.overlays.open {
        return;
    }
    let width = CATALOGUE_WIDTH
        .min(viewport_rect.width() - 2.0 * tokens::SPACE_5)
        .max(1.0);
    let list_height = (viewport_rect.height() - 160.0).max(160.0);
    let mut open = true;
    egui::Window::new("All viewport options")
        .id(egui::Id::new("viewport_catalogue"))
        .open(&mut open)
        .default_pos(viewport_rect.min + egui::vec2(tokens::SPACE_5, tokens::SPACE_3))
        .default_width(width)
        .max_width(width)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            catalogue_body(ui, state, projection, events, list_height);
        });
    if !open {
        state.overlays.open = false;
    }
}

fn catalogue_body(
    ui: &mut egui::Ui,
    state: &mut AppState,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
    list_height: f32,
) {
    ui.horizontal(|ui| {
        ui.label("\u{2315}");
        let search = ui.add(
            egui::TextEdit::singleline(&mut state.overlays.catalogue_query)
                .hint_text("filter by name or id\u{2026}")
                .desired_width(ui.available_width()),
        );
        // The first Escape in the field clears the query; egui gives up the
        // focus on the same key, and the next Escape closes the window.
        if search.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.overlays.catalogue_query.clear();
        }
    });
    ui.add(components::ChoiceRow::new(
        "Show",
        &mut state.overlays.catalogue_filter,
        &[
            (CatalogueFilter::All, "All"),
            (CatalogueFilter::Changed, "Changed"),
            (CatalogueFilter::CannotDraw, "Cannot draw"),
        ],
    ));

    let query = state.overlays.catalogue_query.clone();
    let filter = state.overlays.catalogue_filter;
    let show_extras = query.trim().is_empty() && filter == CatalogueFilter::All;
    let mut listed = 0usize;
    egui::ScrollArea::vertical()
        .max_height(list_height)
        .show(ui, |ui| {
            for section in DockSection::ALL {
                let rows: Vec<&'static OverlayRow> = {
                    let view: &AppState = state;
                    registry::rows_in_section(section)
                        .filter(|row| matches_filter(view, row, &query, filter))
                        .collect()
                };
                let extras = show_extras && section == DockSection::View;
                if rows.is_empty() && !extras {
                    continue;
                }
                components::SectionHeader::new(section_title(state, section)).show(ui);
                if extras {
                    crate::ui::viewport_overlay::draw_view_controls(ui, state, projection, events);
                }
                for row in rows {
                    listed += 1;
                    ui.horizontal_wrapped(|ui| {
                        let control = if row.radio {
                            RowControl::Choice
                        } else {
                            RowControl::Check
                        };
                        draw_control(ui, state, row, row.label, control);
                        ui.label(
                            egui::RichText::new(row.id)
                                .monospace()
                                .size(tokens::SIZE_CAPTION)
                                .color(tokens::TEXT_MUTED),
                        );
                    });
                    draw_reason(ui, state, events, row);
                    if row.id == "simulated_stock" {
                        draw_stock_opacity(ui, state);
                    }
                }
            }
            if listed == 0 && !query.trim().is_empty() {
                ui.label(
                    egui::RichText::new(format!("No option matches \"{}\".", query.trim()))
                        .color(tokens::TEXT_MUTED),
                );
            }
        });
    ui.add_space(tokens::SPACE_2);
    ui.label(
        egui::RichText::new(format!(
            "{} options \u{00B7} {} changed from this workspace's defaults",
            registry::rows().len(),
            registry::non_default_count(state),
        ))
        .size(tokens::SIZE_CAPTION)
        .color(tokens::TEXT_MUTED),
    );
}
