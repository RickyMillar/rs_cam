//! The legend rail: one compact block per active colour encoding, drawn above
//! the viewport dock (viewport redesign, MOCKUPS §2 and §6).
//!
//! The rail shows while the dock popovers and the catalogue are closed, so
//! the operator can always read what a colour means. It reads derived state
//! each frame. It writes no state and starts no compute.
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
//! Every line names its target (`#n`, `all drawn`, a count). The rail has no
//! `+N` overflow: a long line wraps, and a rail taller than its budget
//! scrolls. On a short viewport the caveat lines fold behind one `ⓘ` per
//! legend; the name, the scale and the units never fold.

use crate::render::colors;
use crate::state::AppState;
use crate::state::viewport::ToolpathColorMode;
use crate::ui::tokens;

use super::live;
use super::panel::{COMPUTING_WORD, FAILED_WORD, RowState};
use super::registry::{self, Legend};

// ── the layout ─────────────────────────────────────────────────────────────

/// How much room the rail has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailLayout {
    /// Fold each legend's caveat lines behind one `ⓘ`.
    pub compact: bool,
    /// The rail's height budget. A taller rail scrolls; it never drops a
    /// legend.
    pub max_height: f32,
}

impl RailLayout {
    /// Under this viewport height the caveat lines fold (MOCKUPS §7, the
    /// height caution).
    pub const COMPACT_BELOW_HEIGHT: f32 = 480.0;
    /// The rail's share of the viewport height.
    pub const MAX_HEIGHT_FRACTION: f32 = 0.3;
    /// The rail's smallest height budget: about two legend lines.
    pub const MIN_HEIGHT: f32 = 48.0;

    /// The layout for a viewport.
    pub fn for_viewport(viewport: egui::Rect) -> Self {
        Self {
            compact: viewport.height() < Self::COMPACT_BELOW_HEIGHT,
            max_height: (viewport.height() * Self::MAX_HEIGHT_FRACTION).max(Self::MIN_HEIGHT),
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

/// Draw the rail when at least one line is active. Returns `true` when it
/// drew something.
pub fn draw(ui: &mut egui::Ui, state: &AppState, layout: RailLayout) -> bool {
    let lines = active_lines(state);
    if lines.is_empty() {
        return false;
    }
    egui::Frame::default()
        .fill(tokens::SURFACE_RAISED)
        .corner_radius(tokens::RADIUS_MD)
        .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("viewport_legend_rail")
                .max_height(layout.max_height)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tokens::SPACE_1;
                    for line in &lines {
                        let mut caveats = Caveats::default();
                        match line {
                            RailLine::Scale(legend) => {
                                name_label(ui, &scale_name(state, legend));
                                draw_legend(ui, state, *legend, &mut caveats);
                                live_caveat(state, legend, &mut caveats);
                            }
                            RailLine::Categories(kind) => {
                                name_label(ui, &categories_name(state, *kind));
                                draw_categories(ui, state, *kind, &mut caveats);
                            }
                            RailLine::Status(status) => draw_status(ui, status),
                        }
                        caveats.show(ui, layout.compact);
                    }
                });
        });
    true
}

/// The caveat lines of one legend. They draw after the scale, or fold
/// behind one `ⓘ` on a short viewport.
#[derive(Default)]
struct Caveats {
    lines: Vec<(String, egui::Color32)>,
}

impl Caveats {
    fn push(&mut self, text: impl Into<String>, color: egui::Color32) {
        self.lines.push((text.into(), color));
    }

    fn show(self, ui: &mut egui::Ui, compact: bool) {
        if self.lines.is_empty() {
            return;
        }
        if compact {
            let text = self
                .lines
                .iter()
                .map(|(line, _)| line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            ui.label(caption(
                format!("{} details", tokens::GLYPH_DETAIL),
                tokens::TEXT_MUTED,
            ))
            .on_hover_text(text);
        } else {
            for (line, color) in self.lines {
                ui.label(caption(line, color));
            }
        }
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

fn draw_status(ui: &mut egui::Ui, status: &StatusLine) {
    let color = match status.tone {
        StatusTone::Muted => tokens::TEXT_MUTED,
        StatusTone::Caution => tokens::CAUTION,
        StatusTone::Danger => tokens::DANGER,
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(caption(status.name.as_str(), tokens::TEXT_BODY));
        ui.label(caption(format!("{} {}", status.glyph, status.text), color));
    });
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

// ── the scale legends ──────────────────────────────────────────────────────

/// A horizontal gradient bar plus its end labels.
///
/// `sample` maps `0..=1` along the bar to the colour the 3D overlay paints.
/// Every caller passes the overlay's OWN colour function, so a legend cannot
/// drift from what is drawn. The row wraps at a narrow width; it never
/// pushes the rail past the viewport.
fn gradient_strip(ui: &mut egui::Ui, left: &str, right: &str, sample: impl Fn(f32) -> [f32; 3]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(caption(left, tokens::TEXT_MUTED));
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(96.0, 10.0), egui::Sense::hover());
        let painter = ui.painter();
        const SEGMENTS: u32 = 24;
        for i in 0..SEGMENTS {
            let t0 = i as f32 / SEGMENTS as f32;
            let t1 = (i + 1) as f32 / SEGMENTS as f32;
            let [r, g, b] = sample(t0);
            let seg = egui::Rect::from_min_max(
                egui::pos2(rect.left() + t0 * rect.width(), rect.top()),
                egui::pos2(rect.left() + t1 * rect.width(), rect.bottom()),
            );
            painter.rect_filled(seg, 0.0, rgb(r, g, b));
        }
        ui.label(caption(right, tokens::TEXT_MUTED));
    });
}

fn rgb(r: f32, g: f32, b: f32) -> egui::Color32 {
    tokens::from_linear_rgb([r, g, b])
}

/// A row of labelled entries, for a legend whose scale is a set of classes
/// or categories. An entry with a colour gets a swatch; an entry without
/// one names its colour in words.
fn entry_row(ui: &mut egui::Ui, entries: &[(String, Option<[f32; 3]>)]) {
    ui.horizontal_wrapped(|ui| {
        for (label, colour) in entries {
            if let Some([r, g, b]) = colour {
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, rgb(*r, *g, *b));
                resp.on_hover_text(label.as_str());
            }
            ui.label(caption(label.as_str(), tokens::TEXT_MUTED));
        }
    });
}

fn draw_legend(ui: &mut egui::Ui, state: &AppState, legend: Legend, caveats: &mut Caveats) {
    match legend {
        Legend::RestHeatmap(threshold, top) => {
            use rs_cam_core::maps::rest_heatmap_mesh::rest_ramp_color;
            let threshold = threshold as f32;
            let top = top.max(threshold + 1e-6);
            gradient_strip(
                ui,
                &format!("{threshold:.2} mm"),
                &format!("p95 {top:.2} mm"),
                |t| rest_ramp_color(threshold + t * (top - threshold), threshold, top),
            );
            caveats.push(
                "grey = at or below the threshold \u{00B7} full red from the 95th percentile up",
                tokens::TEXT_FAINT,
            );
        }
        Legend::Reach(ramp) => {
            use rs_cam_core::maps::reach_map::reach_color;
            // The strip draws the DEPTH band only — the bar to the deepest
            // gap, on the ramp's own log scale — because that is the part
            // with structure in it. Green and grey are single colours and
            // are named on the line under it rather than given ramp width.
            let bar = ramp.bar_mm();
            let top = ramp.max_gap_mm.max(bar);
            let ratio = (top / bar).max(1.0);
            gradient_strip(
                ui,
                &format!("miss {bar:.3} mm"),
                &format!("{top:.2} mm"),
                move |t| {
                    // Inverse of `ReachRamp::depth_t`, nudged past the bar so
                    // t = 0 samples the first MISS colour and not the green.
                    let gap = bar * ratio.powf(f64::from(t)) * 1.001;
                    reach_color(gap as f32, f32::NAN, ramp)
                },
            );
            caveats.push(
                format!(
                    "green \u{2264} {:.3} mm \u{00B7} grey = unresolved \u{00B7} mid {:.2} mm \u{00B7} log scale",
                    ramp.tolerance_mm,
                    ramp.mid_stop_mm(),
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
        Legend::Deviation => {
            use crate::render::sim_render::deviation_colors;
            gradient_strip(ui, "Over-cut \u{2212}1 mm", "+1 mm remaining", |t| {
                let mm = -1.0 + 2.0 * t;
                deviation_colors(&[mm]).first().copied().unwrap_or([0.0; 3])
            });
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
            gradient_strip(ui, "low", "high", move |t| {
                let index = ((t * 24.0).round() as usize).min(24);
                colors.get(index).copied().unwrap_or([0.0; 3])
            });
            caveats.push("scaled to this stock's Z range", tokens::TEXT_FAINT);
        }
        Legend::Engagement => {
            use crate::render::toolpath_render::engagement_color;
            // The ramp input is `feed / nominal feed`, and the strip spans
            // 0 to 1.5 of it.
            gradient_strip(ui, "0 %", "150 % of nominal feed", |t| {
                engagement_color(f64::from(t) * 1.5, 1.0)
            });
            caveats.push("red = heavy load, feed reduced", tokens::TEXT_FAINT);
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
            entry_row(
                ui,
                &[
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
                ],
            );
            caveats.push("against the matched vendor band", tokens::TEXT_FAINT);
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
            entry_row(ui, &entries);
            caveats.push(
                "each fine tier's overlap band is a lighter tint of its colour",
                tokens::TEXT_FAINT,
            );
        }
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

fn draw_categories(ui: &mut egui::Ui, state: &AppState, kind: Categories, caveats: &mut Caveats) {
    let entries = category_entries(state, kind);
    match kind {
        Categories::ToolpathPalette => {
            entry_row(ui, &entries);
            if state.viewport.show_all_toolpaths && entries.len() > 1 {
                caveats.push("the selected toolpath draws brighter", tokens::TEXT_FAINT);
            }
        }
        Categories::Moves => {
            entry_row(ui, &entries);
            caveats.push(
                "a cut is darker lower down \u{00B7} spans draw over the cut colour",
                tokens::TEXT_FAINT,
            );
        }
        Categories::EntryMarkers => {
            entry_row(ui, &entries);
            caveats.push(
                "where an entry starts, not how hard it cuts \u{00B7} only an operation with an entry style draws one",
                tokens::TEXT_FAINT,
            );
        }
        Categories::HeightPlanes => {
            entry_row(ui, &entries);
            caveats.push(
                "the Heights tab of the inspector gives each Z in mm",
                tokens::TEXT_FAINT,
            );
        }
        Categories::AreaRegions => {
            entry_row(ui, &entries);
            if entries.len() == 1 {
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
        Categories::Collisions => {
            let left = entries.first().map_or("", |(label, _)| label.as_str());
            let right = entries.last().map_or("", |(label, _)| label.as_str());
            gradient_strip(ui, left, right, mirrored::collision_density_rgb);
            caveats.push("holder and shank strike points", tokens::TEXT_FAINT);
        }
    }
}
