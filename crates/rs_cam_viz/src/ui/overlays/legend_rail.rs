//! The viewport legend: one small chip per active colour encoding, in the
//! bottom-right corner of the 3D view (operator ruling 2026-10-02).
//!
//! The legend reads derived state each frame. It writes only its own fold
//! flag, `OverlayPanelState::legend_collapsed`, and it starts no compute.
//!
//! "Active" means the RENDERED result, not the flag. Three kinds of line
//! exist:
//!
//! - [`RailLine::Scale`] — a scalar encoding with a scale: a gradient or a
//!   set of classes, with units where the data has them.
//! - [`RailLine::Categories`] — a categorical encoding: one entry per
//!   category, with its swatch where a render colour source exists.
//! - [`RailLine::Status`] — a preferred encoding that does not draw its
//!   colours now (computing, failed, or no data). The line says why, so a
//!   plain model or a grey move never reads as a result.
//!
//! # The layout
//!
//! - A chip holds a small swatch and the short name with its target (`#n`,
//!   `all drawn`, a count). The reach chip also gives the unreachable share
//!   as an upper bound. A chip with a stale or under-the-floor caveat
//!   carries the caution glyph.
//! - The full legend of a line — the scale, every category and every caveat
//!   line — shows only while the pointer is on its chip
//!   ([`LEGEND_DETAIL_AREA_ID`]). No caveat is deleted; each one moved from
//!   the old rail into this hover detail.
//! - The legend sits beside the dock when the viewport has the width, and
//!   above the dock when it does not ([`LegendPlacement`]). It never covers
//!   the dock bar or the orientation gizmo.
//! - One quiet button folds the legend to one `Legend (n)` chip.
//! - The legend has no motion.

use crate::render::colors;
use crate::state::AppState;
use crate::state::viewport::ToolpathColorMode;
use crate::ui::components::Button;
use crate::ui::tokens;
use crate::ui::viewport_overlay::DOCK_GUTTER;

use super::live;
use super::panel::{COMPUTING_WORD, FAILED_WORD, RowState};
use super::registry::{self, Legend};

// ── the layout ─────────────────────────────────────────────────────────────

/// The egui id of the legend area.
pub const LEGEND_AREA_ID: &str = "viewport_legend";

/// The egui id of the hover detail area. It shows only while the pointer is
/// on a chip.
pub const LEGEND_DETAIL_AREA_ID: &str = "viewport_legend_detail";

/// The widest legend.
pub const LEGEND_MAX_WIDTH: f32 = 300.0;

/// The narrowest legend that sits beside the dock. With less room, the
/// legend goes above the dock at its full width.
pub const LEGEND_MIN_WIDTH_BESIDE: f32 = 200.0;

/// The widest hover detail.
pub const DETAIL_MAX_WIDTH: f32 = 320.0;

/// The highest share of the viewport height that the legend may take. A
/// taller chip list scrolls; it never drops a line.
pub const LEGEND_MAX_HEIGHT_FRACTION: f32 = 0.25;

/// The space kept free under the top edge of the viewport for the
/// orientation gizmo (`app/viewport.rs`): its 50 pt disc, its 10 pt margin
/// and a 10 pt gap.
pub const GIZMO_CLEARANCE: f32 = 70.0;

/// The text on the collapsed legend chip.
pub const COLLAPSED_WORD: &str = "Legend";

/// The glyph of the button that folds the legend.
pub const FOLD_GLYPH: &str = "\u{25BE}";

/// The height of the swatch on a chip.
const CHIP_SWATCH_HEIGHT: f32 = 8.0;

/// The number of colour samples in a gradient.
const RAMP_SAMPLES: usize = 24;

/// Where the legend goes in one viewport, from the dock rect of this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegendPlacement {
    /// The bottom-right corner of the legend.
    pub anchor: egui::Pos2,
    pub max_width: f32,
    pub max_height: f32,
    /// `true` when the legend sits beside the dock, on the viewport bottom.
    /// `false` when it sits above the dock.
    pub beside_dock: bool,
}

impl LegendPlacement {
    /// The legend is right-aligned in the dock gutter. It goes on the
    /// viewport bottom when at least `LEGEND_MIN_WIDTH_BESIDE` clears the
    /// dock on the right, and above the dock when it does not. Its height
    /// stops under the gizmo clearance.
    pub fn new(viewport: egui::Rect, dock: egui::Rect) -> Self {
        let right = viewport.right() - DOCK_GUTTER;
        let beside_room = right - (dock.right() + tokens::SPACE_3);
        let beside_dock = beside_room >= LEGEND_MIN_WIDTH_BESIDE;
        let max_width = if beside_dock {
            LEGEND_MAX_WIDTH.min(beside_room)
        } else {
            LEGEND_MAX_WIDTH
                .min(viewport.width() - 2.0 * DOCK_GUTTER)
                .max(1.0)
        };
        let bottom = if beside_dock {
            viewport.bottom() - tokens::SPACE_3
        } else {
            dock.top() - tokens::SPACE_2
        };
        let room = (bottom - viewport.top() - GIZMO_CLEARANCE).max(0.0);
        Self {
            anchor: egui::pos2(right, bottom),
            max_width,
            max_height: (viewport.height() * LEGEND_MAX_HEIGHT_FRACTION).min(room),
            beside_dock,
        }
    }
}

// ── the lines ──────────────────────────────────────────────────────────────

/// One block of the rail.
#[derive(Debug, Clone, PartialEq)]
pub enum RailLine {
    /// A scalar encoding with a scale legend.
    Scale(Legend),
    /// A categorical encoding with one entry per category.
    Categories(Categories),
    /// A preferred encoding that does not draw its colours now.
    Status(StatusLine),
}

impl RailLine {
    /// Is the line the legend of a colour on screen? A status line is not:
    /// it says why a colour is NOT on screen.
    pub fn is_encoding(&self) -> bool {
        matches!(self, Self::Scale(_) | Self::Categories(_))
    }

    /// A stable key for the encoding: the registry id of a scale legend,
    /// the name of a categorical one, `status` for a status line.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Scale(legend) => legend_row(legend),
            Self::Categories(kind) => kind.name(),
            Self::Status(_) => "status",
        }
    }
}

/// The name that the rail draws for `line`, with its target.
pub fn line_name(state: &AppState, line: &RailLine) -> String {
    match line {
        RailLine::Scale(legend) => scale_name(state, legend),
        RailLine::Categories(kind) => categories_name(state, *kind),
        RailLine::Status(status) => status.name.clone(),
    }
}

/// The categorical encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categories {
    /// The per-toolpath palette colour of the drawn toolpaths.
    ToolpathPalette,
    /// Cut, rapid and span-kind colours in Palette move colour.
    Moves,
    /// The entry-path preview of the selected toolpath.
    EntryMarkers,
    /// The five Z planes of the selected toolpath.
    HeightPlanes,
    /// The collision markers and their density ramp.
    Collisions,
    /// The By Area regions of the selected 3D Rough, one colour each.
    AreaRegions,
}

impl Categories {
    /// The name the rail draws before the target.
    pub fn name(self) -> &'static str {
        match self {
            Self::ToolpathPalette => "Toolpaths",
            Self::Moves => "Moves",
            Self::EntryMarkers => "Entry markers",
            Self::HeightPlanes => "Height planes",
            Self::Collisions => "Collisions",
            Self::AreaRegions => "By Area regions",
        }
    }
}

/// How a status line is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Muted,
    Caution,
    Danger,
}

/// A preferred encoding that does not draw its colours now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    /// The legend name with its target: `Reach · #12`.
    pub name: String,
    pub glyph: &'static str,
    /// What the operator sees instead, and why.
    pub text: String,
    pub tone: StatusTone,
}

/// Is the row switched on AND able to draw now?
fn ready_on(state: &AppState, id: &str) -> bool {
    registry::row(id).is_some_and(|r| (r.get)(state) && (r.precondition)(state).is_ready())
}

/// Is the row's flag on?
fn flag_on(state: &AppState, id: &str) -> bool {
    registry::row(id).is_some_and(|r| (r.get)(state))
}

fn row_state(state: &AppState, id: &str) -> Option<RowState> {
    registry::row(id).map(|row| RowState::of(state, row))
}

/// `#n` for the selected toolpath, or `none`.
fn selected_tag(state: &AppState) -> String {
    live::selected_toolpath(state)
        .map_or_else(|| "none".to_owned(), |id| live::toolpath_tag(state, id))
}

/// The target word of a move legend: `#n` with `Paths: Selected`,
/// `all drawn` with `Paths: All`.
fn moves_target(state: &AppState) -> String {
    if state.viewport.show_all_toolpaths {
        "all drawn".to_owned()
    } else {
        selected_tag(state)
    }
}

/// The number of collision markers the viewport draws.
fn collision_count(state: &AppState) -> usize {
    state
        .simulation
        .checks
        .collision_report
        .as_ref()
        .map_or(0, |report| report.collisions.len())
}

/// A status line for a flagged row that computes or failed.
fn live_status(state: &AppState, id: &str, name: String, instead: &str) -> Option<StatusLine> {
    match row_state(state, id)? {
        RowState::Computing { .. } => Some(StatusLine {
            name,
            glyph: super::panel::GLYPH_COMPUTING,
            text: format!("{COMPUTING_WORD} {instead} until it is done"),
            tone: StatusTone::Muted,
        }),
        RowState::Failed { .. } => Some(StatusLine {
            name,
            glyph: tokens::GLYPH_DANGER,
            text: format!("{FAILED_WORD} \u{2014} {instead}"),
            tone: StatusTone::Danger,
        }),
        _ => None,
    }
}

/// Every rail line, in rail order.
pub fn active_lines(state: &AppState) -> Vec<RailLine> {
    let legends = registry::active_legends(state);
    let has_legend = |wanted: fn(&Legend) -> bool| legends.iter().any(wanted);
    let mut status = Vec::new();

    // The model surface: a preferred colour source that has no colours on
    // screen yet says so (MOCKUPS §5 and §9, G2).
    if flag_on(state, "reach_map") && !has_legend(|l| matches!(l, Legend::Reach(_))) {
        status.extend(live_status(
            state,
            "reach_map",
            format!("Reach \u{00B7} {}", selected_tag(state)),
            "the model draws plain",
        ));
    }
    if flag_on(state, "rest_heatmap") && !has_legend(|l| matches!(l, Legend::RestHeatmap(..))) {
        status.extend(live_status(
            state,
            "rest_heatmap",
            format!("Rest \u{00B7} {}", selected_tag(state)),
            "the model draws plain",
        ));
    }
    // Two modes that paint a substitute colour without their data
    // (inventory §2.3, items 3 and 4). The renderer is not in this file;
    // the rail says what the operator sees.
    if flag_on(state, "stock_colour_deviation")
        && registry::sim_stock_drawn(state)
        && state.simulation.playback.display_deviations.is_none()
    {
        status.push(StatusLine {
            name: "Stock deviation".to_owned(),
            glyph: tokens::GLYPH_UNKNOWN,
            text: "no deviation data \u{2014} the stock draws in a plain colour; run a simulation"
                .to_owned(),
            tone: StatusTone::Muted,
        });
    }
    if flag_on(state, "move_colour_advance_per_tooth")
        && state.viewport.show_cutting
        && registry::any_toolpath_drawn(state)
        && !state.simulation.has_results()
    {
        status.push(StatusLine {
            name: format!(
                "Move colour: Advance / tooth \u{00B7} {}",
                moves_target(state)
            ),
            glyph: tokens::GLYPH_UNKNOWN,
            text: "needs a simulation \u{2014} the moves draw grey".to_owned(),
            tone: StatusTone::Muted,
        });
    }

    let mut out: Vec<RailLine> = legends.into_iter().map(RailLine::Scale).collect();
    let drawn = registry::any_toolpath_drawn(state);
    let palette = matches!(
        state.viewport.toolpath_color_mode,
        ToolpathColorMode::Normal
    );
    if drawn && palette && (ready_on(state, "cutting_moves") || ready_on(state, "rapids")) {
        out.push(RailLine::Categories(Categories::ToolpathPalette));
        out.push(RailLine::Categories(Categories::Moves));
    }
    let selected = live::selected_toolpath(state).is_some();
    if drawn && selected && ready_on(state, "entry_markers") {
        out.push(RailLine::Categories(Categories::EntryMarkers));
    }
    if selected && ready_on(state, "height_planes") {
        out.push(RailLine::Categories(Categories::HeightPlanes));
    }
    if ready_on(state, "collisions") && collision_count(state) > 0 {
        out.push(RailLine::Categories(Categories::Collisions));
    }
    if registry::area_regions_drawn(state) {
        out.push(RailLine::Categories(Categories::AreaRegions));
    }
    out.extend(status.into_iter().map(RailLine::Status));
    out
}

// ── drawing ────────────────────────────────────────────────────────────────

/// The egui id of the chip for the line named `name`. The name is the
/// [`line_name`] of the line, so each chip id is stable from frame to frame.
pub fn chip_id(name: &str) -> egui::Id {
    egui::Id::new(("viewport_legend_chip", name))
}

/// The short text on the chip of `line`: the name with its target, and for
/// the reach map the unreachable share as an upper bound. The grid's bias is
/// non-negative, so the true share is AT OR BELOW the figure; the hover
/// detail gives the full sentence.
pub fn chip_text(state: &AppState, line: &RailLine) -> String {
    let name = line_name(state, line);
    match line {
        RailLine::Scale(Legend::Reach(_)) => match state.gui.reach_overlay.ready_map() {
            Some(map) if map.is_measured() => format!(
                "{name} \u{00B7} \u{2264} {:.1} % unreachable",
                map.unreachable_pct()
            ),
            Some(_) => format!("{name} \u{00B7} not measured"),
            None => name,
        },
        _ => name,
    }
}

/// Every caveat line of `line`, in the order the hover detail draws them.
/// A status line gives its reason here.
pub fn caveat_lines(state: &AppState, line: &RailLine) -> Vec<(String, egui::Color32)> {
    let mut caveats = Caveats::default();
    match line {
        RailLine::Scale(legend) => {
            scale_caveats(state, *legend, &mut caveats);
            live_caveat(state, legend, &mut caveats);
        }
        RailLine::Categories(kind) => categories_caveats(state, *kind, &mut caveats),
        RailLine::Status(status) => caveats.push(
            format!("{} {}", status.glyph, status.text),
            status_colour(status.tone),
        ),
    }
    caveats.lines
}

/// Draw the legend when at least one line is active, beside or above the
/// dock rect `dock` of this frame. Returns the legend rect when it drew.
///
/// While the pointer is on a chip, the hover detail of that line draws
/// above the legend.
pub fn draw(
    ctx: &egui::Context,
    state: &mut AppState,
    viewport: egui::Rect,
    dock: egui::Rect,
) -> Option<egui::Rect> {
    let lines = active_lines(state);
    if lines.is_empty() {
        return None;
    }
    let place = LegendPlacement::new(viewport, dock);
    let mut collapsed = state.overlays.legend_collapsed;
    let mut hovered: Option<usize> = None;
    let shown: &AppState = state;
    let area = egui::Area::new(egui::Id::new(LEGEND_AREA_ID))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(place.anchor)
        .constrain_to(viewport)
        .show(ctx, |ui| {
            // An area gives its content the rect of the frame before as
            // its max rect. A scroll area inside it then grows by the
            // spare space only, one frame at a time, and the legend creeps
            // up the viewport. Set the full budget each frame.
            ui.set_max_width(place.max_width);
            ui.set_max_height(place.max_height);
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .inner_margin(egui::Margin::same(tokens::SPACE_2 as i8))
                .show(ui, |ui| {
                    if collapsed {
                        let text = format!("{COLLAPSED_WORD} ({})", lines.len());
                        if ui
                            .add(Button::quiet(text))
                            .on_hover_text("Show the legend: one chip for each colour on screen")
                            .clicked()
                        {
                            collapsed = false;
                        }
                        return;
                    }
                    // The width comes from the chips of THIS frame, not from
                    // the area size of the frame before, so a wrapped row
                    // cannot change the width that wraps it.
                    let chip_width = place.max_width - 2.0 * tokens::SPACE_2 - 2.0;
                    let chips: Vec<ChipLayout> = lines
                        .iter()
                        .map(|line| chip_layout(ui, shown, line, chip_width))
                        .collect();
                    let gap = tokens::SPACE_2;
                    let fold_width = ui
                        .painter()
                        .layout_no_wrap(
                            FOLD_GLYPH.to_owned(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            tokens::TEXT_BODY,
                        )
                        .size()
                        .x
                        + 2.0 * ui.spacing().button_padding.x;
                    let row_width: f32 =
                        chips.iter().map(|chip| chip.size.x + gap).sum::<f32>() + fold_width;
                    ui.set_width(row_width.min(chip_width));
                    egui::ScrollArea::vertical()
                        .id_salt("viewport_legend_chips")
                        .auto_shrink([false, true])
                        .max_height(
                            // The frame adds its margin and a 1 pt stroke
                            // on each side.
                            (place.max_height - 2.0 * tokens::SPACE_2 - 2.0)
                                .max(tokens::ROW_ACTION),
                        )
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                                for (index, chip) in chips.into_iter().enumerate() {
                                    if paint_chip(ui, chip).hovered() {
                                        hovered = Some(index);
                                    }
                                }
                                if ui
                                    .add(Button::quiet(FOLD_GLYPH))
                                    .on_hover_text("Fold the legend to one chip")
                                    .clicked()
                                {
                                    collapsed = true;
                                }
                            });
                        });
                });
        });
    let rect = area.response.rect;
    if let Some(line) = hovered.and_then(|index| lines.get(index)) {
        draw_detail(ctx, shown, line, viewport, rect);
    }
    state.overlays.legend_collapsed = collapsed;
    Some(rect)
}

/// The width of one single-colour cell in a chip swatch.
const SWATCH_CELL: f32 = 5.0;

/// The width of the gradient part of a chip swatch.
const SWATCH_RAMP: f32 = 18.0;

/// The swatch on a chip: cells side by side, each `(colour, width)`. It
/// shows EVERY colour its overlay draws: each single colour as a cell, and
/// a gradient as a strip of its samples.
enum Swatch {
    None,
    Cells(Vec<(egui::Color32, f32)>),
}

impl Swatch {
    fn width(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Cells(cells) => cells.iter().map(|(_, width)| width).sum(),
        }
    }
}

fn chip_swatch(state: &AppState, line: &RailLine) -> Swatch {
    let Some(key) = line_key(state, line) else {
        return Swatch::None;
    };
    let single = |entries: &[LegendEntry]| -> Vec<(egui::Color32, f32)> {
        let mut cells: Vec<(egui::Color32, f32)> = Vec::new();
        for colour in entries.iter().filter_map(|(_, colour)| *colour) {
            let colour = tokens::from_linear_rgb(colour);
            // A palette that repeats paints one colour once.
            if !cells.iter().any(|(c, _)| *c == colour) {
                cells.push((colour, SWATCH_CELL));
            }
        }
        cells
    };
    let cells = match &key {
        Key::Ramp {
            lead,
            colours,
            tail,
            ..
        } => {
            let step = SWATCH_RAMP / RAMP_SAMPLES as f32;
            single(lead)
                .into_iter()
                .chain(colours.iter().map(|c| (tokens::from_linear_rgb(*c), step)))
                .chain(single(tail))
                .collect()
        }
        Key::Entries(entries) => single(entries),
    };
    if cells.is_empty() {
        Swatch::None
    } else {
        Swatch::Cells(cells)
    }
}

/// The glyph after a chip's text: the status glyph of a status line, the
/// caution glyph when a caveat is in the caution colour.
fn chip_glyph(state: &AppState, line: &RailLine) -> Option<(&'static str, egui::Color32)> {
    match line {
        RailLine::Status(status) => Some((status.glyph, status_colour(status.tone))),
        _ => caveat_lines(state, line)
            .iter()
            .any(|(_, colour)| *colour == tokens::CAUTION)
            .then_some((tokens::GLYPH_CAUTION, tokens::CAUTION)),
    }
}

/// One chip, laid out but not yet placed.
struct ChipLayout {
    name: String,
    swatch: Swatch,
    swatch_width: f32,
    text: std::sync::Arc<egui::Galley>,
    glyph: Option<std::sync::Arc<egui::Galley>>,
    size: egui::Vec2,
}

/// Lay out one chip: a swatch, the short text and an optional glyph, on one
/// row of `ROW_DENSE` height. A long text truncates at `max_width`.
fn chip_layout(ui: &egui::Ui, state: &AppState, line: &RailLine, max_width: f32) -> ChipLayout {
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let pad = tokens::SPACE_2;
    let swatch = chip_swatch(state, line);
    let swatch_width = match &swatch {
        Swatch::None => 0.0,
        Swatch::Cells(_) => swatch.width() + pad,
    };
    let glyph = chip_glyph(state, line).map(|(glyph, colour)| {
        ui.painter()
            .layout_no_wrap(glyph.to_owned(), font.clone(), colour)
    });
    let glyph_width = glyph.as_ref().map_or(0.0, |g| g.size().x + pad);
    let text_width = (max_width - 2.0 * pad - swatch_width - glyph_width).max(tokens::SPACE_7);
    let mut job =
        egui::text::LayoutJob::simple_singleline(chip_text(state, line), font, tokens::TEXT_BODY);
    job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
    let text = ui.painter().layout_job(job);

    let size = egui::vec2(
        2.0 * pad + swatch_width + text.size().x + glyph_width,
        tokens::ROW_DENSE,
    );
    ChipLayout {
        name: line_name(state, line),
        swatch,
        swatch_width,
        text,
        glyph,
        size,
    }
}

/// Place and paint one chip. The chip senses hover only.
fn paint_chip(ui: &mut egui::Ui, chip: ChipLayout) -> egui::Response {
    let ChipLayout {
        name,
        swatch,
        swatch_width,
        text,
        glyph,
        size,
    } = chip;
    let pad = tokens::SPACE_2;
    let (_, rect) = ui.allocate_space(size);
    let response = ui.interact(rect, chip_id(&name), egui::Sense::hover());

    let painter = ui.painter();
    let fill = if response.hovered() {
        tokens::hover_lift(tokens::SURFACE_RAISED)
    } else {
        tokens::SURFACE_RAISED
    };
    painter.rect_filled(rect, tokens::RADIUS_SM, fill);
    let mut x = rect.left() + pad;
    if let Swatch::Cells(cells) = &swatch {
        let top = rect.center().y - CHIP_SWATCH_HEIGHT * 0.5;
        let mut left = x;
        for (colour, width) in cells {
            let cell = egui::Rect::from_min_size(
                egui::pos2(left, top),
                egui::vec2(*width, CHIP_SWATCH_HEIGHT),
            );
            painter.rect_filled(cell, 0.0, *colour);
            left += width;
        }
        x += swatch_width;
    }
    let text_top = rect.center().y - text.size().y * 0.5;
    let text_right = x + text.size().x;
    painter.galley(egui::pos2(x, text_top), text, tokens::TEXT_BODY);
    if let Some(glyph) = glyph {
        let top = rect.center().y - glyph.size().y * 0.5;
        painter.galley(egui::pos2(text_right + pad, top), glyph, tokens::TEXT_BODY);
    }
    response
}

/// The full legend of one line, above the legend rect. The area takes no
/// pointer input, so it can never take the hover from the chip under it.
fn draw_detail(
    ctx: &egui::Context,
    state: &AppState,
    line: &RailLine,
    viewport: egui::Rect,
    legend: egui::Rect,
) {
    let width = DETAIL_MAX_WIDTH
        .min(viewport.width() - 2.0 * DOCK_GUTTER)
        .max(1.0);
    egui::Area::new(egui::Id::new(LEGEND_DETAIL_AREA_ID))
        .order(egui::Order::Foreground)
        .interactable(false)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(egui::pos2(legend.right(), legend.top() - tokens::SPACE_2))
        .constrain_to(viewport)
        .show(ctx, |ui| {
            ui.set_max_width(width);
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .shadow(tokens::SHADOW_OVERLAY)
                .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tokens::SPACE_1;
                    name_label(ui, &line_name(state, line));
                    match line {
                        RailLine::Scale(legend) => draw_key(ui, &scale_key(*legend)),
                        RailLine::Categories(kind) => draw_key(ui, &categories_key(state, *kind)),
                        RailLine::Status(_) => {}
                    }
                    for (text, colour) in caveat_lines(state, line) {
                        ui.label(caption(text, colour));
                    }
                });
        });
}

/// The caveat lines of one legend.
#[derive(Default)]
struct Caveats {
    lines: Vec<(String, egui::Color32)>,
}

impl Caveats {
    fn push(&mut self, text: impl Into<String>, color: egui::Color32) {
        self.lines.push((text.into(), color));
    }
}

fn name_label(ui: &mut egui::Ui, name: &str) {
    ui.label(
        egui::RichText::new(name)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_BODY),
    );
}

fn caption(text: impl Into<String>, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(tokens::SIZE_CAPTION)
        .color(color)
}

fn status_colour(tone: StatusTone) -> egui::Color32 {
    match tone {
        StatusTone::Muted => tokens::TEXT_MUTED,
        StatusTone::Caution => tokens::CAUTION,
        StatusTone::Danger => tokens::DANGER,
    }
}

/// The registry row whose data a scale legend draws.
fn legend_row(legend: &Legend) -> &'static str {
    match legend {
        Legend::RestHeatmap(..) => "rest_heatmap",
        Legend::Reach(_) => "reach_map",
        Legend::TierMap(_) => "tier_map",
        Legend::Deviation => "stock_colour_deviation",
        Legend::ByHeight => "stock_colour_by_height",
        Legend::Engagement => "move_colour_engagement",
        Legend::AdvancePerTooth => "move_colour_advance_per_tooth",
    }
}

/// A scale legend on screen can still draw an old result, or an old result
/// while a new one computes. The legend says so; the one stale cue is the
/// `!` glyph in the caution colour, as on the dock row.
fn live_caveat(state: &AppState, legend: &Legend, caveats: &mut Caveats) {
    match row_state(state, legend_row(legend)) {
        Some(RowState::Stale { note, .. }) => caveats.push(
            format!("{} stale \u{2014} {note}", tokens::GLYPH_CAUTION),
            tokens::CAUTION,
        ),
        Some(RowState::Computing { .. }) => caveats.push(
            format!("{COMPUTING_WORD} this shows the last result until the new one lands"),
            tokens::TEXT_MUTED,
        ),
        _ => {}
    }
}

/// The name of one scale legend, with its target (MOCKUPS §6).
fn scale_name(state: &AppState, legend: &Legend) -> String {
    match legend {
        Legend::RestHeatmap(..) => format!("Rest \u{00B7} {}", selected_tag(state)),
        Legend::Reach(_) => format!("Reach \u{00B7} {}", selected_tag(state)),
        Legend::TierMap(_) => match state.multitool_planner.as_ref() {
            Some(planner) => format!("Tier map \u{00B7} setup {}", planner.setup_index + 1),
            None => "Tier map".to_owned(),
        },
        Legend::Deviation => "Stock deviation".to_owned(),
        Legend::ByHeight => "Stock height".to_owned(),
        Legend::Engagement => format!("Move colour: Engagement \u{00B7} {}", moves_target(state)),
        Legend::AdvancePerTooth => {
            format!(
                "Move colour: Advance / tooth \u{00B7} {}",
                moves_target(state)
            )
        }
    }
}

/// The name of one categorical legend, with its target.
fn categories_name(state: &AppState, kind: Categories) -> String {
    match kind {
        Categories::ToolpathPalette | Categories::Moves => {
            format!("{} \u{00B7} {}", kind.name(), moves_target(state))
        }
        Categories::EntryMarkers | Categories::HeightPlanes | Categories::AreaRegions => {
            format!("{} \u{00B7} {}", kind.name(), selected_tag(state))
        }
        Categories::Collisions => {
            format!("{} \u{00B7} {}", kind.name(), collision_count(state))
        }
    }
}

// ── the keys ───────────────────────────────────────────────────────────────

/// The colour key of one legend.
enum Key {
    /// A gradient with its end labels. `colours` are samples of the
    /// overlay's OWN colour function at `i / (RAMP_SAMPLES - 1)`, so a legend
    /// cannot drift from what is drawn. `lead` and `tail` are the single
    /// colours the same function paints outside the gradient (the reach
    /// green before it, its greys after it), from the same function.
    Ramp {
        lead: Vec<LegendEntry>,
        left: String,
        right: String,
        colours: Vec<[f32; 3]>,
        tail: Vec<LegendEntry>,
    },
    /// A set of classes or categories.
    Entries(Vec<LegendEntry>),
}

fn ramp(
    left: impl Into<String>,
    right: impl Into<String>,
    sample: impl Fn(f32) -> [f32; 3],
) -> Key {
    Key::Ramp {
        lead: Vec::new(),
        left: left.into(),
        right: right.into(),
        colours: (0..RAMP_SAMPLES)
            // Both ends: t runs 0 to 1 inclusive, so the far-end colour
            // (the deepest miss, the clustered red) is in the key.
            .map(|i| sample(i as f32 / (RAMP_SAMPLES - 1) as f32))
            .collect(),
        tail: Vec::new(),
    }
}

/// `key` with single colours before and after its gradient.
fn with_ends(key: Key, before: Vec<LegendEntry>, after: Vec<LegendEntry>) -> Key {
    match key {
        Key::Ramp {
            left,
            right,
            colours,
            ..
        } => Key::Ramp {
            lead: before,
            left,
            right,
            colours,
            tail: after,
        },
        Key::Entries(entries) => Key::Entries(entries),
    }
}

impl Key {
    /// Every colour the key shows, in key order: the lead colours, the
    /// gradient samples, the tail colours, or each entry colour.
    fn colours(&self) -> Vec<[f32; 3]> {
        match self {
            Self::Ramp {
                lead,
                colours,
                tail,
                ..
            } => lead
                .iter()
                .filter_map(|(_, colour)| *colour)
                .chain(colours.iter().copied())
                .chain(tail.iter().filter_map(|(_, colour)| *colour))
                .collect(),
            Self::Entries(entries) => entries.iter().filter_map(|(_, colour)| *colour).collect(),
        }
    }
}

/// The colour key of `line`, or `None` for a status line.
fn line_key(state: &AppState, line: &RailLine) -> Option<Key> {
    match line {
        RailLine::Scale(legend) => Some(scale_key(*legend)),
        RailLine::Categories(kind) => Some(categories_key(state, *kind)),
        RailLine::Status(_) => None,
    }
}

/// Every colour the hover detail of `line` shows, in key order. A status
/// line shows none.
pub fn key_colours(state: &AppState, line: &RailLine) -> Vec<[f32; 3]> {
    line_key(state, line).map_or_else(Vec::new, |key| key.colours())
}

/// Every colour the chip swatch of `line` paints, in order.
pub fn swatch_colours(state: &AppState, line: &RailLine) -> Vec<egui::Color32> {
    match chip_swatch(state, line) {
        Swatch::None => Vec::new(),
        Swatch::Cells(cells) => cells.into_iter().map(|(colour, _)| colour).collect(),
    }
}

fn draw_key(ui: &mut egui::Ui, key: &Key) {
    match key {
        Key::Ramp {
            lead,
            left,
            right,
            colours,
            tail,
        } => {
            if !lead.is_empty() {
                entry_row(ui, lead);
            }
            gradient_strip(ui, left, right, colours);
            if !tail.is_empty() {
                entry_row(ui, tail);
            }
        }
        Key::Entries(entries) => entry_row(ui, entries),
    }
}

/// Paint `colours` as equal segments across `rect`.
fn paint_ramp(painter: &egui::Painter, rect: egui::Rect, colours: &[egui::Color32]) {
    let count = colours.len().max(1) as f32;
    for (i, colour) in colours.iter().enumerate() {
        let t0 = i as f32 / count;
        let t1 = (i + 1) as f32 / count;
        let segment = egui::Rect::from_min_max(
            egui::pos2(rect.left() + t0 * rect.width(), rect.top()),
            egui::pos2(rect.left() + t1 * rect.width(), rect.bottom()),
        );
        painter.rect_filled(segment, 0.0, *colour);
    }
}

/// A horizontal gradient bar plus its end labels. The row wraps at a
/// narrow width; it never pushes the detail past the viewport.
fn gradient_strip(ui: &mut egui::Ui, left: &str, right: &str, colours: &[[f32; 3]]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(caption(left, tokens::TEXT_MUTED));
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(96.0, 10.0), egui::Sense::hover());
        let colours: Vec<egui::Color32> = colours
            .iter()
            .map(|c| tokens::from_linear_rgb(*c))
            .collect();
        paint_ramp(ui.painter(), rect, &colours);
        ui.label(caption(right, tokens::TEXT_MUTED));
    });
}

/// A row of labelled entries, for a legend whose scale is a set of classes
/// or categories. An entry with a colour gets a swatch; an entry without
/// one names its colour in words.
fn entry_row(ui: &mut egui::Ui, entries: &[(String, Option<[f32; 3]>)]) {
    ui.horizontal_wrapped(|ui| {
        for (label, colour) in entries {
            if let Some(colour) = colour {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 0.0, tokens::from_linear_rgb(*colour));
            }
            ui.label(caption(label.as_str(), tokens::TEXT_MUTED));
        }
    });
}

/// The colour key of one scale legend.
fn scale_key(legend: Legend) -> Key {
    match legend {
        Legend::RestHeatmap(threshold, top) => {
            use rs_cam_core::maps::rest_heatmap_mesh::rest_ramp_color;
            let threshold = threshold as f32;
            let top = top.max(threshold + 1e-6);
            let strip = ramp(
                format!("{threshold:.2} mm"),
                format!("p95 {top:.2} mm"),
                |t| rest_ramp_color(threshold + t * (top - threshold), threshold, top),
            );
            with_ends(
                strip,
                vec![(
                    format!("\u{2264} {threshold:.2} mm"),
                    Some(rest_ramp_color(threshold, threshold, top)),
                )],
                Vec::new(),
            )
        }
        Legend::Reach(reach) => {
            use rs_cam_core::maps::reach_map::reach_color;
            // The strip draws the DEPTH band — the bar to the deepest gap,
            // on the ramp's own log scale. Green (reached), grey
            // (unresolved) and the neutral colour (not measured) are single
            // colours, so they are entries before and after the strip.
            // Every colour is a call to `reach_color`, the overlay's own
            // function.
            let bar = reach.bar_mm();
            let top = reach.max_gap_mm.max(bar);
            let ratio = (top / bar).max(1.0);
            let tolerance = reach.tolerance_mm;
            let miss = ramp(
                format!("miss {bar:.3} mm"),
                format!("{top:.2} mm"),
                move |t| {
                    // Inverse of `ReachRamp::depth_t`, nudged past the bar so
                    // t = 0 samples the first MISS colour and not the green.
                    let gap = bar * ratio.powf(f64::from(t)) * 1.001;
                    reach_color(gap as f32, f32::NAN, reach)
                },
            );
            // A gap over the bar and at or under its cell's floor paints the
            // unresolved grey.
            let unresolved_gap = (tolerance * 2.0).max(1e-3) as f32;
            with_ends(
                miss,
                vec![(
                    format!("reached \u{2264} {tolerance:.3} mm"),
                    Some(reach_color((tolerance * 0.5) as f32, f32::NAN, reach)),
                )],
                vec![
                    (
                        "unresolved".to_owned(),
                        Some(reach_color(unresolved_gap, unresolved_gap, reach)),
                    ),
                    (
                        "not measured".to_owned(),
                        Some(reach_color(f32::NAN, f32::NAN, reach)),
                    ),
                ],
            )
        }
        Legend::Deviation => {
            use crate::render::sim_render::deviation_colors;
            ramp("Over-cut \u{2212}1 mm", "+1 mm remaining", |t| {
                let mm = -1.0 + 2.0 * t;
                deviation_colors(&[mm]).first().copied().unwrap_or([0.0; 3])
            })
        }
        Legend::ByHeight => {
            use rs_cam_core::export::ribbon::height_gradient_colors;
            // The function normalises over the vertices it is handed, so one
            // synthetic ramp of Z values reproduces the mesh's own scale.
            let mut vertices = Vec::with_capacity(3 * 25);
            for i in 0..25 {
                vertices.extend_from_slice(&[0.0, 0.0, i as f32 / 24.0]);
            }
            let colors = height_gradient_colors(&vertices);
            ramp("low", "high", move |t| {
                let index = ((t * 24.0).round() as usize).min(24);
                colors.get(index).copied().unwrap_or([0.0; 3])
            })
        }
        Legend::Engagement => {
            use crate::render::toolpath_render::engagement_color;
            // The ramp input is `feed / nominal feed`, and the strip spans
            // 0 to 1.5 of it.
            ramp("0 %", "150 % of nominal feed", |t| {
                engagement_color(f64::from(t) * 1.5, 1.0)
            })
        }
        Legend::AdvancePerTooth => {
            use crate::render::toolpath_render::advance_per_tooth_segment_color;
            use rs_cam_core::feeds::{AdvancePerToothMm, VendorChiploadBand};
            // The band is per matched vendor row, so the legend labels the
            // CLASSES rather than absolute mm/tooth — and it reads them out
            // of the same classifier the lines use. A probe band supplies the
            // class boundaries; the RGB comes from the shipped function.
            let band = VendorChiploadBand::from_advance_range(&(0.05..0.10));
            let at = |mm: f64| {
                advance_per_tooth_segment_color(Some(&band), Some(AdvancePerToothMm::new(mm)))
            };
            Key::Entries(vec![
                ("below".to_owned(), Some(at(0.01))),
                ("within".to_owned(), Some(at(0.075))),
                ("near max".to_owned(), Some(at(0.099))),
                ("above".to_owned(), Some(at(0.2))),
                (
                    "no band".to_owned(),
                    Some(advance_per_tooth_segment_color(
                        None,
                        Some(AdvancePerToothMm::new(0.05)),
                    )),
                ),
            ])
        }
        Legend::TierMap(tier_count) => {
            use rs_cam_core::maps::rest_heatmap_mesh::{tier_fill_color, tier_overlap_color};
            // Tier 0 has no colour: it is the coarse tool's complement and
            // the overlay deliberately draws nothing there.
            let mut entries = Vec::new();
            for tier in 1..=tier_count {
                let Ok(tier) = u8::try_from(tier) else {
                    break;
                };
                if let Some(colour) = tier_fill_color(tier) {
                    entries.push((format!("tier {tier}"), Some(colour)));
                }
            }
            if let Some(overlap) = tier_overlap_color(1) {
                entries.push(("overlap band".to_owned(), Some(overlap)));
            }
            Key::Entries(entries)
        }
    }
}

/// The caveat lines of one scale legend, without the live stale line.
fn scale_caveats(state: &AppState, legend: Legend, caveats: &mut Caveats) {
    match legend {
        Legend::RestHeatmap(..) => caveats.push(
            "grey = at or below the threshold \u{00B7} full red from the 95th percentile up",
            tokens::TEXT_FAINT,
        ),
        Legend::Reach(reach) => {
            caveats.push(
                format!(
                    "green \u{2264} {:.3} mm \u{00B7} grey = unresolved \u{00B7} mid {:.2} mm \u{00B7} log scale",
                    reach.tolerance_mm,
                    reach.mid_stop_mm(),
                ),
                tokens::TEXT_FAINT,
            );
            // P5.3 - the moves are dimmed at DRAW TIME while the shading is
            // on, so the row still reads ON in the dock. Said here, because
            // an operator seeing Cutting moves ticked and faint lines on
            // screen would otherwise be looking at a contradiction.
            if state.viewport.show_cutting || state.viewport.show_rapids {
                caveats.push("moves dimmed while reach map is on", tokens::TEXT_FAINT);
            }
            if let Some(map) = state.gui.reach_overlay.ready_map() {
                let measured = map.is_measured();
                caveats.push(
                    if measured {
                        format!(
                            "unreachable {:.1}% {} \u{00B7} worst gap {:.3} mm",
                            map.unreachable_pct(),
                            map.area_basis_note(),
                            map.max_gap_mm
                        )
                    } else {
                        "reach: not measured".to_owned()
                    },
                    tokens::TEXT_FAINT,
                );
                // The grid, always, and on its own line: two percentages are
                // comparable only on one grid (F5, 2026-09-08). It also
                // carries the "the bar is under the floor" sentence, which is
                // what the wanaka red terrain needed said.
                if measured {
                    let below = map.tolerance_below_floor();
                    caveats.push(
                        map.grid_note(),
                        if below {
                            tokens::CAUTION
                        } else {
                            tokens::TEXT_FAINT
                        },
                    );
                    if below {
                        caveats.push(map.over_statement_note(), tokens::CAUTION);
                    }
                }
            }
        }
        Legend::Deviation => {}
        Legend::ByHeight => caveats.push("scaled to this stock's Z range", tokens::TEXT_FAINT),
        Legend::Engagement => caveats.push("red = heavy load, feed reduced", tokens::TEXT_FAINT),
        Legend::AdvancePerTooth => {
            caveats.push("against the matched vendor band", tokens::TEXT_FAINT);
        }
        Legend::TierMap(_) => caveats.push(
            "each fine tier's overlap band is a lighter tint of its colour",
            tokens::TEXT_FAINT,
        ),
    }
}

// ── the categorical legends ────────────────────────────────────────────────

/// Mirrors of colours that `render/toolpath_render.rs` and
/// `app/gpu_upload.rs` keep as literals inside their builders, with no
/// public colour source. The legend needs a swatch, and those files are
/// outside the dock's scope. The sentry
/// `the_dock_states_read_live_state_g_vpstate` asserts that each render file
/// still holds the literal these mirror, so a change on either side fails a
/// test. The correct fix is to lift each literal into `render/colors.rs`.
pub mod mirrored {
    /// The entry-span colour of Palette move colour.
    pub const SPAN_ENTRY_RGB: [f32; 3] = [0.20, 0.95, 0.95];
    /// The lead-out-span colour of Palette move colour.
    pub const SPAN_LEAD_OUT_RGB: [f32; 3] = [0.95, 0.30, 0.85];
    /// The link-bridge-span colour of Palette move colour.
    pub const SPAN_LINK_BRIDGE_RGB: [f32; 3] = [0.45, 0.45, 0.55];
    /// A rapid is the toolpath colour times this factor.
    pub const RAPID_FACTOR: f32 = 0.35;

    /// A dressup span is the toolpath colour halfway to mid grey.
    pub fn dressup_rgb(base: [f32; 3]) -> [f32; 3] {
        [
            0.5 * (base[0] + 0.5),
            0.5 * (base[1] + 0.5),
            0.5 * (base[2] + 0.5),
        ]
    }

    /// A rapid in Palette move colour.
    pub fn rapid_rgb(base: [f32; 3]) -> [f32; 3] {
        [
            base[0] * RAPID_FACTOR,
            base[1] * RAPID_FACTOR,
            base[2] * RAPID_FACTOR,
        ]
    }

    /// The collision marker colour at local density `t` in `0..=1`:
    /// yellow when isolated, red when clustered.
    pub fn collision_density_rgb(t: f32) -> [f32; 3] {
        [0.95, 0.8 * (1.0 - t) + 0.1, 0.1 * (1.0 - t)]
    }
}

/// The colour of the entry-path preview, read from the preview builder
/// itself on a two-move ramp.
pub fn entry_preview_rgb() -> Option<[f32; 3]> {
    use crate::render::toolpath_render::{EntryPreviewConfig, EntryStyle, entry_preview_vertices};
    use rs_cam_core::geo::P3;
    let mut path = rs_cam_core::toolpath::Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, 0.0), 100.0);
    let config = EntryPreviewConfig {
        entry_style: EntryStyle::Ramp,
        ramp_angle_deg: 3.0,
        helix_radius: 1.0,
        helix_pitch: 1.0,
        lead_in_out: false,
        lead_radius: 0.0,
        feed_z: 5.0,
        top_z: 0.0,
    };
    entry_preview_vertices(&path, &config)
        .first()
        .map(|vertex| vertex.color)
}

/// The palette colour of the toolpath the move legend describes: the
/// selected one when it is drawn, else the first drawn one.
fn legend_base_colour(state: &AppState) -> Option<[f32; 3]> {
    let drawn = registry::drawn_toolpaths(state);
    let selected = live::selected_toolpath(state);
    drawn
        .iter()
        .find(|(_, id)| Some(*id) == selected)
        .or_else(|| drawn.first())
        .map(|(index, _)| colors::palette_color(*index))
}

/// One entry of a categorical legend: the label, and the swatch colour
/// when a render colour source exists.
pub type LegendEntry = (String, Option<[f32; 3]>);

/// The entries of a categorical legend, in drawing order. For the
/// collisions the two entries are the ends of the density ramp.
pub fn category_entries(state: &AppState, kind: Categories) -> Vec<LegendEntry> {
    match kind {
        Categories::ToolpathPalette => {
            let configs = state.session.toolpath_configs();
            registry::drawn_toolpaths(state)
                .into_iter()
                .map(|(index, id)| {
                    let name = configs
                        .get(index)
                        .map_or_else(String::new, |tc| tc.name.clone());
                    (
                        format!("{} {name}", live::toolpath_tag(state, id)),
                        Some(colors::palette_color(index)),
                    )
                })
                .collect()
        }
        Categories::Moves => {
            let base = legend_base_colour(state);
            let mut entries = Vec::new();
            if state.viewport.show_cutting {
                entries.push(("cut = toolpath colour".to_owned(), base));
            }
            if state.viewport.show_rapids {
                entries.push((
                    "rapid = darker toolpath colour".to_owned(),
                    base.map(mirrored::rapid_rgb),
                ));
            }
            let spans = &state.viewport.span_kind_filter;
            if spans.show_entry {
                entries.push(("entry".to_owned(), Some(mirrored::SPAN_ENTRY_RGB)));
            }
            if spans.show_lead_out {
                entries.push(("lead-out".to_owned(), Some(mirrored::SPAN_LEAD_OUT_RGB)));
            }
            if spans.show_link_bridge {
                entries.push((
                    "link bridge".to_owned(),
                    Some(mirrored::SPAN_LINK_BRIDGE_RGB),
                ));
            }
            if spans.show_dressup {
                entries.push(("dressup".to_owned(), base.map(mirrored::dressup_rgb)));
            }
            entries
        }
        Categories::EntryMarkers => vec![(
            "entry path: ramp, helix or lead-in".to_owned(),
            entry_preview_rgb(),
        )],
        Categories::HeightPlanes => vec![
            ("clearance".to_owned(), Some(colors::HEIGHT_CLEARANCE)),
            ("retract".to_owned(), Some(colors::HEIGHT_RETRACT)),
            ("feed".to_owned(), Some(colors::HEIGHT_FEED)),
            ("top".to_owned(), Some(colors::HEIGHT_TOP)),
            ("bottom".to_owned(), Some(colors::HEIGHT_BOTTOM)),
        ],
        Categories::AreaRegions => registry::selected_area_regions(state)
            .map(|map| {
                if map.regions.is_empty() {
                    // A By Area run that found no region is evidence too:
                    // the rail says so rather than draw a name over nothing.
                    return vec![("no region detected".to_owned(), None)];
                }
                map.regions
                    .iter()
                    .map(|r| {
                        (
                            format!("{} \u{00B7} {} cells", r.order, r.cell_count),
                            rs_cam_core::maps::rest_heatmap_mesh::area_region_color(r.order),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Categories::Collisions => vec![
            (
                "isolated".to_owned(),
                Some(mirrored::collision_density_rgb(0.0)),
            ),
            (
                "clustered".to_owned(),
                Some(mirrored::collision_density_rgb(1.0)),
            ),
        ],
    }
}

/// The colour key of one categorical legend. The collisions draw their
/// density ramp, with the two entries as its end labels.
fn categories_key(state: &AppState, kind: Categories) -> Key {
    let entries = category_entries(state, kind);
    match kind {
        Categories::Collisions => {
            let left = entries.first().map_or("", |(label, _)| label.as_str());
            let right = entries.last().map_or("", |(label, _)| label.as_str());
            ramp(left, right, mirrored::collision_density_rgb)
        }
        _ => Key::Entries(entries),
    }
}

/// The caveat lines of one categorical legend.
fn categories_caveats(state: &AppState, kind: Categories, caveats: &mut Caveats) {
    match kind {
        Categories::ToolpathPalette => {
            if state.viewport.show_all_toolpaths && category_entries(state, kind).len() > 1 {
                caveats.push("the selected toolpath draws brighter", tokens::TEXT_FAINT);
            }
        }
        Categories::Moves => caveats.push(
            "a cut is darker lower down \u{00B7} spans draw over the cut colour",
            tokens::TEXT_FAINT,
        ),
        Categories::EntryMarkers => caveats.push(
            "where an entry starts, not how hard it cuts \u{00B7} only an operation with an entry style draws one",
            tokens::TEXT_FAINT,
        ),
        Categories::HeightPlanes => caveats.push(
            "the Heights tab of the inspector gives each Z in mm",
            tokens::TEXT_FAINT,
        ),
        Categories::AreaRegions => {
            if category_entries(state, kind).len() == 1 {
                caveats.push(
                    "one region: By Area cuts the part as Global does",
                    tokens::TEXT_FAINT,
                );
            }
            caveats.push(
                "detected once, before the first level \u{00B7} the box is the cut filter",
                tokens::TEXT_FAINT,
            );
        }
        Categories::Collisions => caveats.push("holder and shank strike points", tokens::TEXT_FAINT),
    }
}
