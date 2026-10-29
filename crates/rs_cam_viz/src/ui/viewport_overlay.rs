//! The viewport dock: a floating bar at the bottom centre of the 3D view,
//! with the target line above it (viewport redesign,
//! `planning/viewport_interaction_redesign/MOCKUPS.md` §2–§8). The legend
//! is a separate area in the bottom-right corner
//! (`crate::ui::overlays::legend_rail`); it never covers the dock.
//!
//! DOCK_DECLUTTER (operator approval, 2026-10-29): the bar holds the
//! one-click controls, in this order: the display mode, Grid, Stock (Off ·
//! Box · Solid), Cutting moves, Rapids, Collisions, then `More`. `More`
//! opens one upward popover with every other row, grouped under the
//! section titles `View`, `Scene` and `Paths` (R6: Scene holds one
//! captioned group per scene object: Model, Stock, Tool, Obstacles, Pins);
//! its last line
//! opens the All viewport options catalogue
//! (`crate::ui::overlays::panel`), which lists every registry row. The four
//! section popovers went (a declutter removes a control; it does not hide
//! a duplicate route). A row has ONE home: on the bar ([`BAR_ROWS`]) or in
//! `More`. `Escape` or a click outside the dock and the popover closes
//! `More`.
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
//!   section title in `More` carries the `—` glyph and the row gives the
//!   reason; the dock never shows another mode in its place.
//! - A `Compute & show` result applies only when the target and the mode
//!   still match the request (`panel::settle_pending_show`).

use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::state::overlays::DockSection;
use crate::state::selection::Selection;
use crate::state::viewport::ToolpathColorMode;
use crate::state::{AppState, Workspace};
use crate::ui::automation;
use crate::ui::components;
use crate::ui::overlays::panel::{self, RowControl, RowState};
use crate::ui::overlays::registry::{self, DockGroup, OverlayRow, OverlaySurface};
use crate::ui::overlays::{context_rail, legend_rail};
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

/// The egui id of the dock area. It also holds the target line.
pub const DOCK_AREA_ID: &str = "viewport_dock";

/// The egui id of the popover area.
pub const POPOVER_AREA_ID: &str = "viewport_dock_popover";

/// The rows the bar owns (DOCK_DECLUTTER). Each has its home on the bar
/// and is not listed in `More`; the catalogue still lists it.
pub const BAR_ROWS: [&str; 6] = [
    "grid",
    "stock_box",
    "stock_solid",
    "cutting_moves",
    "rapids",
    "collisions",
];

/// The widest popover.
pub const POPOVER_MAX_WIDTH: f32 = 320.0;

/// The dock's side gutter: the space kept free between the dock and each
/// side of the viewport.
pub const DOCK_GUTTER: f32 = tokens::SPACE_5;

/// The highest share of the viewport height that the target line and the
/// dock bar may take together. The phase 4 sentry
/// measures the stack at 320 × 600 pt against this budget.
pub const DOCK_STACK_MAX_HEIGHT_FRACTION: f32 = 0.6;

/// The smallest height a popover's scroll area gets. Under it, the list
/// would show less than one row.
const POPOVER_MIN_LIST_HEIGHT: f32 = 24.0;

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
}

/// The word after a section name: `Paths: Selected`.
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
/// Paths names the draw scope. R6 removed the Inspect section and its
/// colour suffix: the colour choice of an object is in its group, with the
/// "Your choice is kept" line under it.
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
    }
}

/// The target line above the dock bar (MOCKUPS §2).
pub fn target_text(state: &AppState) -> String {
    let numbered = |id: crate::state::toolpath::ToolpathId| {
        state
            .session()
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
        Workspace::Toolpaths | Workspace::Readiness => match *state.focus() {
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
    /// A click outside the dock and the popover closed `More`. The
    /// viewport does not also select with that click (MOCKUPS §12, Q1).
    pub closed_popover_on_click: bool,
}

/// Draw the dock, the target line, the legend and the `More` popover over
/// `viewport_rect`.
pub fn draw(
    ui: &mut egui::Ui,
    state: &mut AppState,
    lanes: &[LaneSnapshot; 4],
    events: &mut Vec<AppEvent>,
    viewport_rect: egui::Rect,
) -> DockOutcome {
    let ctx = ui.ctx().clone();
    // Two settles before anything draws, so the dock, the legend and the
    // viewport callback that the frame builds after the dock all read one
    // state. Both write view state only; neither starts compute.
    registry::settle_exclusive_surfaces(state);
    panel::settle_pending_show(state);
    let outcome = DockOutcome {
        closed_popover_on_click: close_on_outside_click(&ctx, state),
    };
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
            dock_bar(ui, state, lanes, events)
        });
    // The legend reads the dock rect of THIS frame, so it can never sit on
    // the dock bar.
    let dock_rect = bar.response.rect;
    let bar = bar.inner;
    let legend_rect = legend_rail::draw(&ctx, state, viewport_rect, dock_rect);
    // R6 (ruling Q5): the context rail stands on the legend, so it keeps
    // the legend's clearance of the dock and the view cube corner.
    context_rail::draw(&ctx, state, viewport_rect, dock_rect, legend_rect);

    if state.overlays.more_open {
        let anchor = PopoverAnchor {
            viewport: viewport_rect,
            button: bar.more_button,
            bar: bar.frame,
        };
        draw_more(&ctx, state, events, anchor);
    }
    panel::draw_rest_confirm(&ctx, state, events);
    outcome
}

/// Rects the popover anchors to.
struct BarRects {
    /// The dock frame.
    frame: egui::Rect,
    /// The `More` button.
    more_button: egui::Rect,
}

/// The bar: the display mode, Grid, Stock, Cutting moves, Rapids,
/// Collisions, then `More` (DOCK_DECLUTTER). Every toggle is the row's own
/// control (`panel::draw_control`): its precondition, its reason on hover
/// and its workspace default stay the row's.
fn dock_bar(
    ui: &mut egui::Ui,
    state: &mut AppState,
    lanes: &[LaneSnapshot; 4],
    events: &mut Vec<AppEvent>,
) -> BarRects {
    let mut more_button = egui::Rect::NOTHING;
    let frame = egui::Frame::default()
        .fill(tokens::SURFACE_OVERLAY)
        .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
        .corner_radius(tokens::RADIUS_MD)
        .shadow(tokens::SHADOW_OVERLAY)
        .inner_margin(egui::Margin::same(tokens::SPACE_2 as i8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                panel::draw_display_mode(ui, state);
                ui.separator();
                bar_toggle(ui, state, "grid");
                panel::draw_stock_display(ui, state);
                bar_toggle(ui, state, "cutting_moves");
                bar_toggle(ui, state, "rapids");
                if let Some(response) = bar_toggle(ui, state, "collisions") {
                    // The automation harness locates the collision toggle
                    // through this label (UX §8, "two automation labels
                    // must survive").
                    automation::record(ui, "overlay_collision_check", &response, "Collisions");
                }
                ui.separator();
                let open = state.overlays.more_open;
                let response = ui
                    .add(egui::Button::selectable(
                        open,
                        egui::RichText::new("More \u{25B4}")
                            .size(tokens::SIZE_BODY)
                            .color(if open {
                                tokens::TEXT_STRONG
                            } else {
                                tokens::TEXT_BODY
                            }),
                    ))
                    .on_hover_text(
                        "Every other viewport option, by section, and All viewport options.",
                    );
                if response.clicked() {
                    state.overlays.toggle_more();
                }
                more_button = response.rect;
                compute_activity(ui, lanes, events);
            });
        });
    BarRects {
        frame: frame.response.rect,
        more_button,
    }
}

/// One bar toggle: the row's own checkbox with its registry label.
fn bar_toggle(ui: &mut egui::Ui, state: &mut AppState, id: &str) -> Option<egui::Response> {
    let row = registry::row(id)?;
    Some(panel::draw_control(
        ui,
        state,
        row,
        row.label,
        RowControl::Check,
    ))
}

/// The last line of `More`: the All viewport options catalogue.
fn catalogue_line(ui: &mut egui::Ui, state: &mut AppState) {
    let count = registry::non_default_count(state);
    let text = match count {
        0 => "All viewport options \u{2315}".to_owned(),
        n => format!("All viewport options \u{2315} ({n})"),
    };
    let response = ui
        .add(egui::Button::selectable(
            state.overlays.open,
            egui::RichText::new(text).size(tokens::SIZE_BODY),
        ))
        .on_hover_text(
            "Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O). The count is the options changed from this workspace's defaults.",
        );
    automation::record(ui, "viewport_catalogue", &response, "All viewport options");
    if response.clicked() {
        state.overlays.open = !state.overlays.open;
        state.overlays.close_more();
    }
}

/// The compute activity and its one cancel. Drawn only while a lane works.
///
/// `UiCommand::CancelCompute` cancels EVERY lane, so the label says so.
fn compute_activity(ui: &mut egui::Ui, lanes: &[LaneSnapshot; 4], events: &mut Vec<AppEvent>) {
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
    if !state.overlays.more_open {
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
    state.overlays.close_more();
    true
}

/// The three rects that place a popover.
#[derive(Debug, Clone, Copy)]
struct PopoverAnchor {
    viewport: egui::Rect,
    /// The `More` button.
    button: egui::Rect,
    /// The dock frame.
    bar: egui::Rect,
}

/// The `More` popover, above its button: every row that is not on the bar,
/// under its section title, then the catalogue line.
fn draw_more(
    ctx: &egui::Context,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
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
    let left = (button.right() - width)
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
                        .show(ui, |ui| {
                            for section in DockSection::ALL {
                                // R5c: a section with no row that can apply
                                // in this workspace shows no title.
                                if more_rows(state.workspace, section) == 0 {
                                    continue;
                                }
                                components::SectionHeader::new(panel::section_title(
                                    state, section,
                                ))
                                .show(ui);
                                match section {
                                    DockSection::View => view_group(ui, state, events),
                                    DockSection::Scene => scene_group(ui, state, events),
                                    DockSection::Paths => paths_group(ui, state, events),
                                }
                            }
                            ui.separator();
                            catalogue_line(ui, state);
                        });
                });
        });
}

/// How many rows `More` lists under `section` in `ws` (R5c): the rows of
/// the section that the bar does not own and that can apply in `ws`.
#[must_use]
pub fn more_rows(ws: Workspace, section: DockSection) -> usize {
    registry::rows_in_section(section)
        .filter(|row| !BAR_ROWS.contains(&row.id) && row.listed_in(ws))
        .count()
}

/// A group caption inside a popover (`Move colour`, `Model colour`).
fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_MUTED),
    );
}

/// True when `More` lists `row` in `ws`: the bar does not own it and it
/// can apply there (R5c).
fn in_more(row: &OverlayRow, ws: Workspace) -> bool {
    !BAR_ROWS.contains(&row.id) && row.listed_in(ws)
}

/// Draw every row of `section` that `handled` does not name and the bar
/// does not own ([`BAR_ROWS`]). A new row therefore always shows in `More`,
/// even before its group gives it a special place.
fn draw_remaining_rows(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    section: DockSection,
    handled: &[&str],
) {
    for row in registry::rows_in_section(section) {
        if !handled.contains(&row.id) && in_more(row, state.workspace) {
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
    if let Some(row) = registry::row(id).filter(|r| r.listed_in(state.workspace)) {
        panel::draw_control(ui, state, row, label, control);
        panel::draw_reason(ui, state, events, row);
    }
}

/// Draw `add` one indent step in (a child row of the row above it).
fn indented(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.vertical(add);
    });
}

/// R5b: the view cube in the viewport corner turns the view (a face click
/// gives its preset, a drag orbits) and the toggle under it sets the
/// projection; the F key fits and the View menu keeps Reset View and the
/// presets. So the View section holds the rows of its home only: the view
/// cube row.
fn view_group(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    draw_remaining_rows(ui, state, events, DockSection::View, &[]);
}

/// R6 (ruling Q1): the Scene section is one captioned group per scene
/// object, in [`DockGroup::ALL`] order, then the single rows. The groups
/// are registry data (`OverlayRow::group`); this function only draws them.
fn scene_group(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    for group in DockGroup::ALL {
        if group.section() == DockSection::Scene {
            draw_dock_group(ui, state, events, group);
        }
    }
}

/// How many rows `More` lists in `group` in `ws`. A group with none shows
/// no caption (R6).
#[must_use]
pub fn more_rows_in_group(ws: Workspace, group: DockGroup) -> usize {
    registry::rows_in_group(group)
        .filter(|row| in_more(row, ws))
        .count()
}

/// The label of a row inside its group: the part after the colon of a
/// colour choice (`Model colour: Reach` is `Reach`); `Visible` for the row
/// that has the name of its group (the `Model` row under `Model`).
fn label_in_group(row: &OverlayRow) -> &'static str {
    if row.group.caption() == Some(row.label) {
        return "Visible";
    }
    row.label
        .rsplit_once(": ")
        .map_or(row.label, |(_, short)| short)
}

/// One group of `More`: its caption, then its rows in list order. The
/// colour choices of one surface draw as one block at the place of the
/// first of them: a `Plain` choice first where the surface has a plain
/// state (the model), then the workspace default, then the others.
/// A group of one row that has the group's name draws that row alone.
fn draw_dock_group(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    group: DockGroup,
) {
    let ws = state.workspace;
    let rows: Vec<&'static OverlayRow> = registry::rows_in_group(group)
        .filter(|row| in_more(row, ws))
        .collect();
    let Some(first) = rows.first() else {
        return;
    };
    let alone = rows.len() == 1 && group.caption() == Some(first.label);
    if let Some(text) = group.caption().filter(|_| !alone) {
        caption(ui, text);
    }
    let mut drawn_surfaces: Vec<OverlaySurface> = Vec::new();
    for row in &rows {
        if row.surface == OverlaySurface::None {
            let label = if alone {
                row.label
            } else {
                label_in_group(row)
            };
            panel::draw_control(ui, state, row, label, RowControl::Check);
            panel::draw_reason(ui, state, events, row);
            if row.id == "simulated_stock" {
                panel::draw_stock_opacity(ui, state);
            }
        } else if !drawn_surfaces.contains(&row.surface) {
            drawn_surfaces.push(row.surface);
            colour_block(ui, state, events, &rows, row.surface);
        }
    }
}

/// The colour choices of `surface` among `rows`, as one radio block.
fn colour_block(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    rows: &[&'static OverlayRow],
    surface: OverlaySurface,
) {
    let ws = state.workspace;
    let mut members: Vec<&'static OverlayRow> = rows
        .iter()
        .copied()
        .filter(|row| row.surface == surface)
        .collect();
    // The workspace default first; the sort is stable, so the others keep
    // their list order.
    members.sort_by_key(|row| registry::effective_default(state, row, ws) != Some(true));
    let has_plain = !members.iter().any(|row| row.radio);
    if has_plain {
        // ruling Q5: `Plain` replaces `Off`. It clears every colour source
        // of the surface, so the model draws plain.
        let plain = !members.iter().any(|row| (row.get)(state));
        if ui
            .radio(plain, "Plain")
            .on_hover_text("No colour source: the plain surface.")
            .clicked()
            && !plain
        {
            registry::clear_surface(state, surface);
        }
    }
    for row in &members {
        panel::draw_control(ui, state, row, label_in_group(row), RowControl::Choice);
        panel::draw_reason(ui, state, events, row);
    }
    // The preference stays when it cannot draw. The dock says so rather
    // than show another choice in its place.
    if has_plain
        && members
            .iter()
            .any(|row| (row.get)(state) && !RowState::of(state, row).draws())
    {
        caption(ui, "Your choice is kept. The surface draws plain.");
    }
}

/// The rows of Paths that the group places itself (or the bar owns).
const PATHS_PLACED: [&str; 15] = [
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
    "generator_steps",
    "active_step_highlight",
];

fn paths_group(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let ws = state.workspace;
    // The draw scope writes through `UiCommand::ToggleShowAllToolpaths`,
    // whose handler calls `registry::set_overlay` on `all_toolpaths`.
    if let Some(scope_row) = registry::row("all_toolpaths").filter(|r| r.listed_in(ws)) {
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
    // Cutting moves and Rapids are on the bar.
    for id in ["entry_markers", "height_planes", "tool_profile_ghost"] {
        if let Some(row) = registry::row(id).filter(|r| r.listed_in(ws)) {
            panel::draw_row(ui, state, events, row);
        }
    }
    if registry::row("move_colour_palette").is_some_and(|r| r.listed_in(ws)) {
        caption(ui, "Move colour");
        draw_row_as(
            ui,
            state,
            events,
            "move_colour_palette",
            "Palette",
            RowControl::Choice,
        );
        // Ruling Q9: the span filter applies in Palette only, so its four
        // checks show under Palette, one step in, while Palette is the
        // move colour.
        if matches!(
            state.viewport.toolpath_color_mode,
            ToolpathColorMode::Normal
        ) {
            indented(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (id, label) in [
                        ("span_entry", "Entry"),
                        ("span_lead_out", "Lead-out"),
                        ("span_link_bridge", "Link bridge"),
                        ("span_dressup", "Dressup"),
                    ] {
                        if let Some(row) = registry::row(id) {
                            panel::draw_control(ui, state, row, label, RowControl::Check);
                        }
                    }
                });
                // The four span rows share one precondition, so one reason
                // line serves all four.
                if let Some(row) = registry::row("span_entry") {
                    panel::draw_reason(ui, state, events, row);
                }
            });
        }
        for (id, label) in [
            ("move_colour_engagement", "Engagement"),
            ("move_colour_advance_per_tooth", "Advance / tooth"),
        ] {
            draw_row_as(ui, state, events, id, label, RowControl::Choice);
        }
    }
    // Ruling Q2: Highlight active step is a child of Generator steps. It
    // shows one step in, only while Generator steps is on.
    if let Some(steps) = registry::row("generator_steps").filter(|r| r.listed_in(ws)) {
        panel::draw_row(ui, state, events, steps);
        if (steps.get)(state)
            && let Some(child) = registry::row("active_step_highlight").filter(|r| r.listed_in(ws))
        {
            indented(ui, |ui| panel::draw_row(ui, state, events, child));
        }
    }
    draw_remaining_rows(ui, state, events, DockSection::Paths, &PATHS_PLACED);
    ui.label(
        egui::RichText::new("Per operation: eye, C and R are on each operation row.")
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_MUTED),
    );
}
