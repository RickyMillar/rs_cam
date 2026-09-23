//! The viewport dock: a floating bar at the bottom centre of the 3D view,
//! with the target line and the legend rail above it (viewport redesign,
//! `planning/viewport_interaction_redesign/MOCKUPS.md` §2–§8).
//!
//! The dock has four text-labelled sections in a fixed order: `View`,
//! `Scene`, `Paths` and `Inspect`. A click on a section opens its upward
//! popover. One popover is open at a time; a click on a second section
//! closes the first. `Escape` or a click outside the dock and the popover
//! closes it. The last button opens the All viewport options catalogue
//! (`crate::ui::overlays::panel`), which lists every registry row.
//!
//! The dock is an `egui::Area`, not a panel, so it takes no width from the
//! 3D view. It takes pointer input only inside its own rects: a drag that
//! starts outside it orbits the camera as before.
//!
//! ## What the redesign removed from this file
//!
//! This file was the strip above the 3D view: `View ▼`, `Persp ▼`, the
//! `Overlays (n)` button, the draw scope, the compute label with `Cancel
//! All`, and the Simulation `Reset`. Each control has a home in the dock
//! now. The P6 retirements stay retired: `Show ▼` became registry rows, and
//! the render-mode menu went because its wireframe arm drew nothing.
//!
//! ## Rules
//!
//! - Every overlay write goes through `registry::set_overlay`, through the
//!   shared row renderer in `panel.rs`, or through a `UiCommand`.
//! - The draw loop writes no core state and starts no compute. The dock
//!   pushes events; the controller does the work.
//! - A row's state is derived each frame from live state
//!   (`panel::RowState`, `overlays::live`). The dock stores no copy of it.
//! - A selection change moves the TARGET. It never changes the preferred
//!   mode. When the preferred mode cannot apply to the new target, the
//!   section suffix carries the `—` glyph and the popover gives the reason;
//!   the dock never shows another mode in its place.
//! - A `Compute & show` result applies only when the target and the mode
//!   still match the request (`panel::settle_pending_show`).

use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::{ProjectionMode, ViewPreset};
use crate::state::overlays::DockSection;
use crate::state::selection::Selection;
use crate::state::{AppState, Workspace};
use crate::ui::automation;
use crate::ui::components;
use crate::ui::overlays::legend_rail::{self, RailLayout};
use crate::ui::overlays::panel::{self, RowControl, RowState};
use crate::ui::overlays::registry::{self, OverlaySurface};
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

/// The egui id of the dock area. It also holds the target line and the
/// legend rail.
pub const DOCK_AREA_ID: &str = "viewport_dock";

/// The egui id of the popover area.
pub const POPOVER_AREA_ID: &str = "viewport_dock_popover";

/// At or above this viewport width, the dock shows the section suffixes
/// (`Paths: Selected`). MOCKUPS §7, rule 1.
pub const FULL_DOCK_MIN_VIEWPORT_WIDTH: f32 = 420.0;

/// At or above this available width, the catalogue button carries the word
/// `All` and the count. Under it, the button is the glyph alone.
/// MOCKUPS §7, rule 3.
pub const LABELLED_CATALOGUE_MIN_WIDTH: f32 = 300.0;

/// The widest popover.
pub const POPOVER_MAX_WIDTH: f32 = 320.0;

/// The dock's side gutter: the space kept free between the dock and each
/// side of the viewport.
pub const DOCK_GUTTER: f32 = tokens::SPACE_5;

/// The highest share of the viewport height that the target line, the
/// legend rail and the dock bar may take together. The phase 4 sentry
/// measures the stack at 320 × 600 pt against this budget.
pub const DOCK_STACK_MAX_HEIGHT_FRACTION: f32 = 0.6;

/// The smallest height a popover's scroll area gets. Under it, the list
/// would show less than one row.
const POPOVER_MIN_LIST_HEIGHT: f32 = 24.0;

/// Which parts of the dock fit at one viewport width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockLayout {
    /// Show `Paths: Selected` rather than `Paths`.
    pub show_suffixes: bool,
    /// Show `All ⌕ (n)` rather than `⌕`.
    pub labelled_catalogue: bool,
}

impl DockLayout {
    /// The width rules of MOCKUPS §7, applied in order. The four section
    /// buttons never hide.
    pub fn for_viewport_width(width: f32) -> Self {
        Self {
            show_suffixes: width >= FULL_DOCK_MIN_VIEWPORT_WIDTH,
            labelled_catalogue: width - 2.0 * DOCK_GUTTER >= LABELLED_CATALOGUE_MIN_WIDTH,
        }
    }
}

/// How the preferred choice of a section stands against its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuffixStatus {
    /// The choice draws, or it is `Off`.
    Ready,
    /// The choice cannot apply to this target. The glyph is `—`.
    Unavailable,
    /// A job for the choice runs now. The glyph is `◌`.
    Computing,
    /// The choice draws an old result. The glyph is `!`.
    Stale,
    /// The last run for the choice failed. The glyph is `✕`.
    Failed,
}

impl SuffixStatus {
    /// The glyph after the section name, or `""` for a ready choice.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Ready => "",
            Self::Unavailable => tokens::GLYPH_UNKNOWN,
            Self::Computing => panel::GLYPH_COMPUTING,
            Self::Stale => tokens::GLYPH_CAUTION,
            Self::Failed => tokens::GLYPH_DANGER,
        }
    }

    fn of(row_state: &RowState) -> Self {
        match row_state {
            RowState::Off | RowState::Showing => Self::Ready,
            RowState::NeedsCompute { .. } | RowState::Blocked { .. } => Self::Unavailable,
            RowState::Computing { .. } => Self::Computing,
            RowState::Stale { .. } => Self::Stale,
            RowState::Failed { .. } => Self::Failed,
        }
    }
}

/// The word after a section name: `Paths: Selected`, `Inspect: Reach`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSuffix {
    pub word: &'static str,
    /// How the preferred choice stands. The dock adds the glyph of a
    /// choice that is not ready; it does not show another choice in its
    /// place.
    pub status: SuffixStatus,
    /// The hover text of the section button when the status is not
    /// `Ready`: the word `unavailable`, `computing…`, `stale` or `failed`,
    /// and the reason.
    pub detail: Option<String>,
}

/// The suffix of one section, or `None` for a section without one.
///
/// Paths names the draw scope. Inspect names the PREFERRED colour source of
/// the workspace's surface: the model surface outside Simulation, the stock
/// surface in Simulation.
pub fn section_suffix(state: &AppState, section: DockSection) -> Option<SectionSuffix> {
    match section {
        DockSection::View | DockSection::Scene => None,
        DockSection::Paths => {
            let show_all = state.viewport.show_all_toolpaths;
            // Inventory §2.3, item 5: with `Paths: Selected` and nothing
            // selected, the moves rows read ticked and nothing draws.
            let nothing_drawn =
                registry::any_generated(state) && !registry::any_toolpath_drawn(state);
            Some(SectionSuffix {
                word: if show_all { "All" } else { "Selected" },
                status: if nothing_drawn {
                    SuffixStatus::Unavailable
                } else {
                    SuffixStatus::Ready
                },
                detail: nothing_drawn.then(|| {
                    if show_all {
                        "Paths: All is unavailable \u{2014} every generated toolpath is hidden."
                            .to_owned()
                    } else {
                        "Paths: Selected is unavailable \u{2014} no generated toolpath is selected, so no move draws. Your choice is kept."
                            .to_owned()
                    }
                }),
            })
        }
        DockSection::Inspect => {
            let simulation = state.workspace == Workspace::Simulation;
            let choices: &[(&str, &'static str)] = if simulation {
                &[
                    ("stock_colour_solid", "Solid"),
                    ("stock_colour_deviation", "Deviation"),
                    ("stock_colour_by_height", "Height"),
                ]
            } else {
                &[("reach_map", "Reach"), ("rest_heatmap", "Rest")]
            };
            for &(id, word) in choices {
                if let Some(row) = registry::row(id)
                    && (row.get)(state)
                {
                    let row_state = RowState::of(state, row);
                    let status = SuffixStatus::of(&row_state);
                    let detail = row_state.reason().map(|reason| {
                        let what = match status {
                            SuffixStatus::Ready | SuffixStatus::Unavailable => "unavailable",
                            SuffixStatus::Computing => panel::COMPUTING_WORD,
                            SuffixStatus::Stale => "stale",
                            SuffixStatus::Failed => panel::FAILED_WORD,
                        };
                        format!("{word} is {what} \u{2014} {reason}. Your choice is kept.")
                    });
                    return Some(SectionSuffix {
                        word,
                        status,
                        detail,
                    });
                }
            }
            (!simulation).then_some(SectionSuffix {
                word: "Off",
                status: SuffixStatus::Ready,
                detail: None,
            })
        }
    }
}

/// The text on one section button.
pub fn section_button_text(state: &AppState, section: DockSection, layout: DockLayout) -> String {
    let label = section.label();
    match section_suffix(state, section) {
        Some(suffix) => {
            let glyph = match suffix.status.glyph() {
                "" => String::new(),
                glyph => format!(" {glyph}"),
            };
            if layout.show_suffixes {
                format!("{label}: {}{glyph}", suffix.word)
            } else {
                format!("{label}{glyph}")
            }
        }
        None => label.to_owned(),
    }
}

/// The target line above the rail (MOCKUPS §2).
pub fn target_text(state: &AppState) -> String {
    let numbered = |id: crate::state::toolpath::ToolpathId| {
        state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
            .map(|(index, tc)| format!("#{} {}", index + 1, tc.name))
    };
    let none = || "Target: none \u{2014} select an operation".to_owned();
    match state.workspace {
        Workspace::Setup => state
            .active_setup_index()
            .map_or_else(none, |index| format!("Target: setup {}", index + 1)),
        Workspace::Simulation => state
            .simulation
            .focused_toolpath()
            .and_then(numbered)
            .map_or_else(none, |text| format!("Target: {text} (playback)")),
        Workspace::Toolpaths | Workspace::Readiness => match state.selection {
            Selection::Toolpath(id) => {
                numbered(id).map_or_else(none, |text| format!("Target: {text}"))
            }
            _ => none(),
        },
    }
}

/// What the dock did with the pointer this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DockOutcome {
    /// A click outside the dock and the popover closed the popover. The
    /// viewport does not also select with that click (MOCKUPS §12, Q1).
    pub closed_popover_on_click: bool,
}

/// Draw the dock, the target line, the legend rail and the open popover
/// over `viewport_rect`.
pub fn draw(
    ui: &mut egui::Ui,
    state: &mut AppState,
    projection: ProjectionMode,
    lanes: &[LaneSnapshot; 5],
    events: &mut Vec<AppEvent>,
    viewport_rect: egui::Rect,
) -> DockOutcome {
    let ctx = ui.ctx().clone();
    // Two settles before anything draws, so the dock, the rail and the
    // viewport callback that the frame builds after the dock all read one
    // state. Both write view state only; neither starts compute.
    registry::settle_exclusive_surfaces(state);
    panel::settle_pending_show(state);
    let outcome = DockOutcome {
        closed_popover_on_click: close_on_outside_click(&ctx, state),
    };
    let layout = DockLayout::for_viewport_width(viewport_rect.width());
    let rail = RailLayout::for_viewport(viewport_rect);
    let max_width = (viewport_rect.width() - 2.0 * DOCK_GUTTER).max(1.0);

    let bar = egui::Area::new(egui::Id::new(DOCK_AREA_ID))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(
            viewport_rect.center().x,
            viewport_rect.max.y - tokens::SPACE_3,
        ))
        .constrain_to(viewport_rect)
        .show(&ctx, |ui| {
            ui.set_max_width(max_width);
            ui.spacing_mut().item_spacing.y = tokens::SPACE_2;
            let target = target_text(state);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(target.as_str())
                        .size(tokens::SIZE_CAPTION)
                        .color(tokens::TEXT_MUTED),
                )
                .truncate(),
            )
            .on_hover_text(target.as_str());
            legend_rail::draw(ui, state, rail);
            dock_bar(ui, state, lanes, events, layout)
        })
        .inner;

    if let Some(section) = state.overlays.open_section
        && let Some(button) = bar.open_button
    {
        let anchor = PopoverAnchor {
            viewport: viewport_rect,
            button,
            bar: bar.frame,
        };
        draw_popover(&ctx, state, projection, events, section, anchor);
    }
    panel::draw_rest_confirm(&ctx, state, events);
    outcome
}

/// Rects the popover anchors to.
struct BarRects {
    /// The dock frame.
    frame: egui::Rect,
    /// The button of the open section, when one is open.
    open_button: Option<egui::Rect>,
}

fn dock_bar(
    ui: &mut egui::Ui,
    state: &mut AppState,
    lanes: &[LaneSnapshot; 5],
    events: &mut Vec<AppEvent>,
    layout: DockLayout,
) -> BarRects {
    let mut open_button = None;
    let frame = egui::Frame::default()
        .fill(tokens::SURFACE_OVERLAY)
        .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
        .corner_radius(tokens::RADIUS_MD)
        .shadow(tokens::SHADOW_OVERLAY)
        .inner_margin(egui::Margin::same(tokens::SPACE_2 as i8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for section in DockSection::ALL {
                    let open = state.overlays.open_section == Some(section);
                    let text = section_button_text(state, section, layout);
                    let detail = section_suffix(state, section).and_then(|suffix| suffix.detail);
                    let mut response = ui.add(egui::Button::selectable(
                        open,
                        egui::RichText::new(text)
                            .size(tokens::SIZE_BODY)
                            .color(if open {
                                tokens::TEXT_STRONG
                            } else {
                                tokens::TEXT_BODY
                            }),
                    ));
                    if let Some(detail) = detail {
                        response = response.on_hover_text(detail);
                    }
                    if response.clicked() {
                        state.overlays.toggle_section(section);
                    }
                    if section == DockSection::Inspect {
                        // The automation harness locates the collision
                        // toggle through this label. The Collisions row
                        // lives in Inspect, so the label moves with it
                        // (UX §8, "two automation labels must survive").
                        automation::record(
                            ui,
                            "overlay_collision_check",
                            &response,
                            "Inspect (collisions)",
                        );
                    }
                    if state.overlays.open_section == Some(section) {
                        open_button = Some(response.rect);
                    }
                }
                ui.separator();
                catalogue_button(ui, state, layout);
                compute_activity(ui, lanes, events);
            });
        });
    BarRects {
        frame: frame.response.rect,
        open_button,
    }
}

/// The button that opens the All viewport options catalogue.
fn catalogue_button(ui: &mut egui::Ui, state: &mut AppState, layout: DockLayout) {
    let count = registry::non_default_count(state);
    let text = match (layout.labelled_catalogue, count) {
        (true, 0) => "All \u{2315}".to_owned(),
        (true, n) => format!("All \u{2315} ({n})"),
        (false, _) => "\u{2315}".to_owned(),
    };
    let hover = if layout.labelled_catalogue || count == 0 {
        "Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O)."
            .to_owned()
    } else {
        format!(
            "Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O). {count} changed from this workspace's defaults."
        )
    };
    let response = ui
        .add(egui::Button::selectable(
            state.overlays.open,
            egui::RichText::new(text).size(tokens::SIZE_BODY),
        ))
        .on_hover_text(hover);
    automation::record(ui, "viewport_catalogue", &response, "All viewport options");
    if response.clicked() {
        state.overlays.open = !state.overlays.open;
        state.overlays.close_section();
    }
}

/// The compute activity and its one cancel. Drawn only while a lane works.
///
/// `UiCommand::CancelCompute` cancels EVERY lane, so the label says so.
fn compute_activity(ui: &mut egui::Ui, lanes: &[LaneSnapshot; 5], events: &mut Vec<AppEvent>) {
    let active: Vec<String> = lanes
        .iter()
        .filter(|lane| lane.is_active())
        .map(|lane| {
            lane.current_job
                .clone()
                .unwrap_or_else(|| "Working".to_owned())
        })
        .collect();
    if active.is_empty() {
        return;
    }
    ui.separator();
    ui.add(egui::Spinner::new().size(tokens::SIZE_BODY))
        .on_hover_text(active.join(" | "));
    let cancel = ui
        .add(components::Button::new("Cancel all jobs"))
        .on_hover_text("Cancels every running and queued compute job, not only this one.");
    automation::record(ui, "overlay_cancel_all", &cancel, "Cancel all jobs");
    if cancel.clicked() {
        events.push(AppEvent::Ui(UiCommand::CancelCompute(NoArgs)));
    }
}

/// Close the popover when a click lands outside the dock and the popover.
///
/// The rects are last frame's, which is the frame the operator saw. The
/// click still reaches whatever is under it, except that the viewport does
/// not select with it (see [`DockOutcome`]).
fn close_on_outside_click(ctx: &egui::Context, state: &mut AppState) -> bool {
    if state.overlays.open_section.is_none() {
        return false;
    }
    let (clicked, pos) = ctx.input(|i| (i.pointer.any_click(), i.pointer.interact_pos()));
    let Some(pos) = pos.filter(|_| clicked) else {
        return false;
    };
    let inside = ctx.memory(|m| {
        [DOCK_AREA_ID, POPOVER_AREA_ID].iter().any(|id| {
            m.area_rect(egui::Id::new(*id))
                .is_some_and(|r| r.contains(pos))
        })
    });
    if inside {
        return false;
    }
    state.overlays.close_section();
    true
}

/// The three rects that place a popover.
#[derive(Debug, Clone, Copy)]
struct PopoverAnchor {
    viewport: egui::Rect,
    /// The button of the open section.
    button: egui::Rect,
    /// The dock frame.
    bar: egui::Rect,
}

/// The popover of the open section, above its button.
fn draw_popover(
    ctx: &egui::Context,
    state: &mut AppState,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
    section: DockSection,
    anchor: PopoverAnchor,
) {
    let PopoverAnchor {
        viewport: viewport_rect,
        button,
        bar,
    } = anchor;
    let width = POPOVER_MAX_WIDTH
        .min(viewport_rect.width() - 2.0 * DOCK_GUTTER)
        .max(1.0);
    let left = button
        .left()
        .min(viewport_rect.right() - DOCK_GUTTER - width)
        .max(viewport_rect.left() + DOCK_GUTTER);
    let bottom = bar.top() - tokens::SPACE_2;
    // The short-viewport rule: the popover never grows above the viewport
    // top. The list scrolls inside the frame instead. The frame margin and
    // its stroke are taken off, so the whole frame, not the list alone,
    // stays inside the viewport.
    let frame_extra = 2.0 * tokens::SPACE_3 + 2.0;
    let max_height =
        (bottom - viewport_rect.top() - tokens::SPACE_3 - frame_extra).max(POPOVER_MIN_LIST_HEIGHT);
    egui::Area::new(egui::Id::new(POPOVER_AREA_ID))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(egui::pos2(left, bottom))
        .constrain_to(viewport_rect)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .shadow(tokens::SHADOW_OVERLAY)
                .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * tokens::SPACE_3);
                    egui::ScrollArea::vertical()
                        .id_salt("viewport_dock_popover_scroll")
                        .max_height(max_height)
                        .show(ui, |ui| match section {
                            DockSection::View => view_popover(ui, state, projection, events),
                            DockSection::Scene => scene_popover(ui, state, events),
                            DockSection::Paths => paths_popover(ui, state, events),
                            DockSection::Inspect => inspect_popover(ui, state, events),
                        });
                });
        });
}

/// A group caption inside a popover (`Move colour`, `Model colour`).
fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_MUTED),
    );
}

/// Draw every row of `section` that `handled` does not name. A new row
/// therefore always shows in its dock section, even before the section
/// gives it a special place.
fn draw_remaining_rows(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    section: DockSection,
    handled: &[&str],
) {
    for row in registry::rows_in_section(section) {
        if !handled.contains(&row.id) {
            panel::draw_row(ui, state, events, row);
        }
    }
}

/// A row by id with a short label, as a choice or a check, plus its reason.
fn draw_row_as(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    id: &str,
    label: &str,
    control: RowControl,
) {
    if let Some(row) = registry::row(id) {
        panel::draw_control(ui, state, row, label, control);
        panel::draw_reason(ui, state, events, row);
    }
}

/// The camera controls that are not registry rows: presets, projection and
/// reset. The catalogue draws the same controls under `VIEW`.
pub(crate) fn draw_view_controls(
    ui: &mut egui::Ui,
    state: &AppState,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal_wrapped(|ui| {
        for (preset, label, hover) in [
            (ViewPreset::Top, "Top", "Shortcut: 1"),
            (ViewPreset::Front, "Front", "Shortcut: 2"),
            (ViewPreset::Right, "Right", "Shortcut: 3"),
            (ViewPreset::Isometric, "Iso", "Shortcut: 4"),
        ] {
            if ui
                .add(components::Button::new(label))
                .on_hover_text(hover)
                .clicked()
            {
                events.push(AppEvent::Ui(UiCommand::SetViewPreset(preset)));
            }
        }
    });
    let mut choice = projection;
    let changed = ui
        .add(components::ChoiceRow::new(
            "Projection",
            &mut choice,
            &[
                (ProjectionMode::Perspective, "Perspective"),
                (ProjectionMode::Orthographic, "Orthographic"),
            ],
        ))
        .changed();
    if changed && choice != projection {
        events.push(AppEvent::Ui(UiCommand::ToggleProjection(NoArgs)));
    }
    ui.horizontal_wrapped(|ui| {
        if ui.add(components::Button::new("Reset view")).clicked() {
            events.push(AppEvent::Ui(UiCommand::ResetView(NoArgs)));
        }
        if state.workspace == Workspace::Simulation
            && ui
                .add(components::Button::new("Reset simulation"))
                .on_hover_text("Reset the simulation to the start.")
                .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));
        }
    });
}

fn view_popover(
    ui: &mut egui::Ui,
    state: &mut AppState,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
) {
    draw_view_controls(ui, state, projection, events);
    draw_remaining_rows(ui, state, events, DockSection::View, &[]);
}

fn scene_popover(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    for row in registry::rows_in_section(DockSection::Scene) {
        panel::draw_row(ui, state, events, row);
        if row.id == "simulated_stock" {
            panel::draw_stock_opacity(ui, state);
        }
    }
}

/// The rows of Paths that the popover places itself.
const PATHS_PLACED: [&str; 13] = [
    "all_toolpaths",
    "cutting_moves",
    "rapids",
    "entry_markers",
    "height_planes",
    "tool_profile_ghost",
    "move_colour_palette",
    "move_colour_engagement",
    "move_colour_advance_per_tooth",
    "span_entry",
    "span_lead_out",
    "span_link_bridge",
    "span_dressup",
];

fn paths_popover(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    // The draw scope writes through `UiCommand::ToggleShowAllToolpaths`,
    // whose handler calls `registry::set_overlay` on `all_toolpaths`.
    if let Some(scope_row) = registry::row("all_toolpaths") {
        let show_all = state.viewport.show_all_toolpaths;
        let ready = panel::RowState::of(state, scope_row).is_ready();
        let mut choice = show_all;
        let changed = ui
            .add_enabled_ui(ready || show_all, |ui| {
                ui.add(
                    components::ChoiceRow::new(
                        "Draw",
                        &mut choice,
                        &[(false, "Selected"), (true, "All")],
                    )
                    .hover(scope_row.hover),
                )
                .changed()
            })
            .inner;
        if changed && choice != show_all {
            events.push(AppEvent::Ui(UiCommand::ToggleShowAllToolpaths(NoArgs)));
        }
        panel::draw_reason(ui, state, events, scope_row);
    }
    for id in [
        "cutting_moves",
        "rapids",
        "entry_markers",
        "height_planes",
        "tool_profile_ghost",
    ] {
        if let Some(row) = registry::row(id) {
            panel::draw_row(ui, state, events, row);
        }
    }
    caption(ui, "Move colour");
    for (id, label) in [
        ("move_colour_palette", "Palette"),
        ("move_colour_engagement", "Engagement"),
        ("move_colour_advance_per_tooth", "Advance / tooth"),
    ] {
        draw_row_as(ui, state, events, id, label, RowControl::Choice);
    }
    caption(ui, "Spans");
    ui.horizontal_wrapped(|ui| {
        for (id, label) in [
            ("span_entry", "Entry"),
            ("span_lead_out", "LeadOut"),
            ("span_link_bridge", "LinkBridge"),
            ("span_dressup", "DressupArtifact"),
        ] {
            if let Some(row) = registry::row(id) {
                panel::draw_control(ui, state, row, label, RowControl::Check);
            }
        }
    });
    // The four span rows share one precondition, so one reason line serves
    // all four.
    if let Some(row) = registry::row("span_entry") {
        panel::draw_reason(ui, state, events, row);
    }
    draw_remaining_rows(ui, state, events, DockSection::Paths, &PATHS_PLACED);
    ui.label(
        egui::RichText::new("Per operation: eye, C and R are on each operation row.")
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_MUTED),
    );
}

/// The rows of Inspect that the popover places itself.
const INSPECT_PLACED: [&str; 11] = [
    "reach_map",
    "rest_heatmap",
    "tier_map",
    "planner_islands",
    "stock_colour_solid",
    "stock_colour_deviation",
    "stock_colour_by_height",
    "collisions",
    "tool_deflection",
    "generator_steps",
    "active_step_highlight",
];

fn inspect_popover(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    caption(ui, "Model colour");
    draw_row_as(ui, state, events, "reach_map", "Reach", RowControl::Choice);
    draw_row_as(
        ui,
        state,
        events,
        "rest_heatmap",
        "Rest",
        RowControl::Choice,
    );
    let model_rows: Vec<_> = registry::rows()
        .iter()
        .filter(|row| row.surface == OverlaySurface::Model)
        .collect();
    let model_off = !model_rows.iter().any(|row| (row.get)(state));
    if ui.radio(model_off, "Off").clicked() && !model_off {
        registry::clear_surface(state, OverlaySurface::Model);
    }
    // The preference stays when it cannot draw. The dock says so rather
    // than show another choice in its place.
    if model_rows
        .iter()
        .any(|row| (row.get)(state) && !RowState::of(state, row).draws())
    {
        caption(ui, "Your choice is kept. The model draws plain.");
    }
    for id in ["tier_map", "planner_islands"] {
        if let Some(row) = registry::row(id) {
            panel::draw_row(ui, state, events, row);
        }
    }
    caption(ui, "Stock colour");
    for (id, label) in [
        ("stock_colour_solid", "Solid"),
        ("stock_colour_deviation", "Deviation"),
        ("stock_colour_by_height", "Height"),
    ] {
        draw_row_as(ui, state, events, id, label, RowControl::Choice);
    }
    for id in [
        "collisions",
        "tool_deflection",
        "generator_steps",
        "active_step_highlight",
    ] {
        if let Some(row) = registry::row(id) {
            panel::draw_row(ui, state, events, row);
        }
    }
    draw_remaining_rows(ui, state, events, DockSection::Inspect, &INSPECT_PLACED);
    ui.horizontal_wrapped(|ui| {
        caption(ui, "Move colour is in Paths.");
        if ui.add(components::Button::new("Open Paths")).clicked() {
            state.overlays.open_section = Some(DockSection::Paths);
        }
    });
}
