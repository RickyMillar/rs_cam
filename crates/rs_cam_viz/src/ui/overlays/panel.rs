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
use crate::state::overlays::{CatalogueFilter, DockSection};
use crate::ui::AppEvent;
use crate::ui::components;
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

use super::registry::{self, OverlayAction, OverlayRow};

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

/// How a registry row presents now. It is derived each frame from the flag
/// and the precondition, and never stored.
///
/// The plan names seven states. Today's registry gives four of them; the
/// other three (computing, stale, failed) need state that phase 3 adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    /// The row can draw, and its flag is off.
    Off,
    /// The row can draw, and its flag is on. This is the FLAG, not the
    /// rendered result.
    Showing,
    /// The row cannot draw until the named work runs.
    NeedsCompute {
        reason: String,
        action: OverlayAction,
    },
    /// The row cannot draw, and no button makes it drawable here.
    Blocked { reason: String },
}

impl RowState {
    /// Derive the state of `row` now.
    pub fn of(state: &AppState, row: &OverlayRow) -> Self {
        let precondition = (row.precondition)(state);
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

    /// Can the operator switch the row on now?
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Off | Self::Showing)
    }

    /// Why the row cannot draw, when it cannot.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Off | Self::Showing => None,
            Self::NeedsCompute { reason, .. } | Self::Blocked { reason } => Some(reason.as_str()),
        }
    }

    /// The button that makes the row drawable, when one exists.
    pub fn action(&self) -> Option<OverlayAction> {
        match self {
            Self::NeedsCompute { action, .. } => Some(*action),
            Self::Off | Self::Showing | Self::Blocked { .. } => None,
        }
    }
}

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

/// One row with its registry label: the control, then the reason line and
/// the action button when it cannot draw.
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
        Some(reason) => format!("{}\n\n{} {reason}", row.hover, tokens::GLYPH_UNKNOWN),
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

/// The reason line and the action button of a row that cannot draw. Draws
/// nothing for a row that can.
pub(crate) fn draw_reason(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
) {
    let row_state = RowState::of(state, row);
    let Some(reason) = row_state.reason() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.label(
            egui::RichText::new(format!("{} {reason}", tokens::GLYPH_UNKNOWN))
                .size(tokens::SIZE_CAPTION)
                .color(tokens::TEXT_MUTED),
        );
    });
    if let Some(action) = row_state.action() {
        ui.horizontal(|ui| {
            ui.add_space(tokens::SPACE_5);
            if ui.add(components::Button::new(action.label())).clicked() {
                run_action(state, events, action, row);
            }
        });
    }
}

/// Run a disabled row's compute affordance.
///
/// `row` is the row whose precondition offered `action`, so an action may
/// switch its OWN row on through the registry's one setter.
fn run_action(
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    action: OverlayAction,
    row: &'static registry::OverlayRow,
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
        // rest grid. The row does not apply the command itself; it calls
        // the one door that owns the stamp, the edited mark and the
        // simulation invalidation, then asks for the generation that fills
        // the grid. The row goes on here too, or the operator would have to
        // come back and click the checkbox a second time.
        OverlayAction::EnableRestAnalysis => {
            if let crate::state::selection::Selection::Toolpath(id) = state.selection {
                crate::ui::properties::apply_auto_enable(state, id);
                registry::set_overlay(state, row, true);
                events.push(AppEvent::GenerateToolpath(id));
            }
        }
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
