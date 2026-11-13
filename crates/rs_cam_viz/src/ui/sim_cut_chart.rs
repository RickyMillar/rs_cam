//! The sim width x depth chart (feeds contract P17, change 3; P17b items
//! 1, 2, 3 and 5): the focused toolpath's safe envelope on the chart kit,
//! read-only, with the cut samples on it as a heat map.
//!
//! The Metrics tab draws it after the four cut-metric cards and one
//! hairline (layout B, operator ruling through the lead, 2026-11-06): the
//! cards are the verdict, the chart is the drill-down.
//!
//! # What it draws
//!
//! The plane is the envelope plot of the Passes tab (`ui/feeds/envelope.rs`
//! `draw_plane`): the same axes (the width as a share of D, the depth in
//! mm), frame edges, limit lines, labels and soft fill, through
//! [`EnvelopeScene`] and the public helpers of that module. No candidate,
//! no drag, no apply.
//!
//! The layers, from the ground up:
//!
//! - The iso-force contours (P17b item 3, operator: "faint background
//!   lines"): the level lines of the set's force plane (core
//!   `tool_load::force_field::ForcePlane`, the body cut's law at the
//!   planned advance per tooth, built once per load report) at
//!   [`CONTOUR_SHARES`] of the machine force budget. The three under the
//!   budget are `TEXT_FAINT` [`CONTOUR_WIDTH`]. P29-10 10g (lead ruling
//!   O4): the field draws for every op with a force line; where the budget
//!   is a limit of the body cut the budget line is `CAUTION`
//!   [`BUDGET_CONTOUR_WIDTH`], the colour of the over-budget dots, named
//!   "100 % budget"; where it is not (a finish pass) it is `TEXT_FAINT`
//!   and named "budget, no limit here" (core `BudgetRole::line_name`), and
//!   no dot draws. They paint under every fill (the kit has one slot under
//!   the fills, [`ChartLine::beneath`]). A drill has no field.
//! - P29-10 10g (ruling O5): the planned cuts at the STATED point (the cut
//!   that the samples show, core `SafeEnvelope::marks_at_stated`), each a
//!   hollow square: `TEXT_MUTED`, no tone, because the planned rows
//!   describe the proposal; when the stated point is the proposal they take
//!   the tone of their rows. The hover names the cut.
//! - The heat map (P17b item 1, operator: "brighter where more cuts"): the
//!   samples binned over the DATA range by cutting time, [`GRID_COLUMNS`]
//!   columns and at most [`GRID_ROWS`] rows ([`square_rows`]: the cells
//!   are square on the screen). One hue, [`heat_hue`] (the histogram's
//!   ink family), whose alpha rises with the RANK of the cell's time among
//!   the cells that hold time (histogram equalisation, the `eq_hist`
//!   shading of datashader: no constant, and one heavy cell does not push
//!   every other cell to the faint end), in [`HEAT_STEPS`] steps from
//!   [`FAINTEST_ALPHA`] (a single scattered sample still reads) to
//!   [`DENSEST_ALPHA`] (the densest cell at full strength). No cluster
//!   box, no label: the hover names a cell.
//! - The limit lines, each named ON its line at the line's end in the
//!   line's colour ([`LabelAt::LineEnd`] on [`LabelGround::Ground`], the
//!   P17b kit anchor), as are the contours.
//! - Over-budget dots: each sample whose force is above the force budget
//!   paints a `CAUTION` dot (the budget does not gate an export, so no
//!   `DANGER`). Only under the `ForceBudget` rule. At most [`DOT_CAP`]
//!   dots paint, every k-th by weight; the legend hover names the rest.
//! - The playhead sample: the sample whose move is nearest at or below the
//!   playhead, as the quietest mark (a 1 pt ring at the recommendation
//!   colour), when the playhead is in this toolpath.
//! - The planned point LAST (P17b item 5, the judge's must-fix): the white
//!   dot of the envelope plot over a halo of the panel ground
//!   ([`HALO_WIDTH`]), so no dot under it breaks its outline. The
//!   playhead ring paints between the halo and the dot, so a ring at the
//!   plan still reads as a ring.
//!
//! # The depth range (P17b item 2)
//!
//! The DATA range is `VIEW_HEADROOM` over the deeper of the deepest sample
//! and the planned depth, never past the scene's own top ([`depth_view`]).
//! The heat grid covers it. When a flat depth limit sits above it, the
//! AXIS extends to the lowest such limit, with room above the line for its
//! name, so the limit draws inside the frame and is named on its line; the
//! extension is empty space over the band. A limit more than
//! [`OFF_SCALE_FACTOR`] times the data range does not extend the axis: it
//! keeps one quiet off-scale row in the gutter above the frame
//! ([`PlaneChart::off_scale_top`]), as does every flat limit still above
//! the extended axis (the Rigidity DOC over the Vendor DOC on the rivmap
//! rough: the brief's "lowest such limit" reading, which drops no limit).
//!
//! A click on a dot seeks the playhead to that sample's move; a click on
//! a non-empty cell seeks to the cell's first sample in trace order. The
//! hover names the cell under the pointer. One legend row of painted
//! swatches closes the section (no glyph: the font sentry): a ramp of
//! [`LEGEND_RAMP_SQUARES`] squares for the cut time.
//!
//! # What it does NOT do
//!
//! It reads no simulation state: [`draw_cut_chart`] takes the set's
//! inputs and the playhead, so a headless sentry drives it. It builds no
//! sample and no field: the set is built once per load report
//! (`state::simulation::build_cut_metric_set`). It colours no point by
//! load: the dots stay white and orange (operator, item 3).

use rs_cam_core::feeds::envelope::{CutPoint, SafeEnvelope};
use rs_cam_core::tool_load::cut_force::CutPoint3;

use rs_cam_core::tool_load::force_field::{BudgetRole, ForcePlane};

use super::components::chart::{
    self, Axis, ChartLine, ChartMap, Label, LabelAt, LabelGround, Mark, PlaneChart,
};
use super::components::{histogram, text};
use super::feeds::envelope::{
    self, DEPTH_AXIS_TITLE, EnvelopeScene, GHOST_RADIUS, OUTLINE_WIDTH, PLANNED_RADIUS,
    PLANNED_WIDTH, RECOMMENDATION_WIDTH, VIEW_HEADROOM, WIDTH_AXIS_TITLE, WIDTH_EDGE_TEXT,
    recommendation_color,
};
use super::tokens;

/// The columns of the heat grid across the width range.
pub const GRID_COLUMNS: usize = 48;

/// The most rows of the heat grid up the depth range ([`square_rows`]
/// gives the count the chart draws).
pub const GRID_ROWS: usize = 32;

/// The steps of the heat ramp: one fill per step. Thirty-two steps on the
/// rank ramp read as a gradient (P17b; the sixteen equal buckets over a
/// narrow alpha span read as one level).
pub const HEAT_STEPS: usize = 32;

/// The most over-budget dots the chart paints.
pub const DOT_CAP: usize = 400;

/// The radius of an over-budget dot, in points: under the planned dot, so
/// the plan reads above the samples.
pub const DOT_RADIUS: f32 = 2.5;

/// A click or a hover takes a dot within this distance, in points.
pub const HIT_RADIUS: f32 = 4.0;

/// The alpha of the densest cell: full strength (P17b item 1, "brighter
/// where more cuts").
pub const DENSEST_ALPHA: f32 = 1.0;

/// The alpha of the faintest step: the histogram's out-of-band wash, so a
/// single scattered sample still reads against the safe-region wash.
pub const FAINTEST_ALPHA: f32 = histogram::WASH_OUT;

/// A flat depth limit more than this many times the data range does not
/// extend the axis (the band would squash into a strip, the judge's
/// round-3 finding); it keeps a quiet off-scale row instead.
pub const OFF_SCALE_FACTOR: f64 = 3.0;

/// The iso-force contour levels, as shares of the force budget (core
/// `tool_load::force_field::CONTOUR_SHARES`).
pub use rs_cam_core::tool_load::force_field::CONTOUR_SHARES;

/// The width of a contour under the budget, in points.
pub const CONTOUR_WIDTH: f32 = 1.0;

/// The width of the budget contour, in points: the strongest line of the
/// ground.
pub const BUDGET_CONTOUR_WIDTH: f32 = 1.5;

/// The halo under the planned dot, in points: the envelope plot's outline
/// width, in the panel ground, so a dot under the plan does not break it.
pub const HALO_WIDTH: f32 = OUTLINE_WIDTH;

/// The painted squares of the legend's cut-time ramp, faint to bright.
pub const LEGEND_RAMP_SQUARES: usize = 4;

/// The section title.
pub const TITLE: &str = "Width \u{00D7} depth";

/// The (i) hover of the section.
pub const GUIDE: &str = "Each sample of the cut on the envelope of the operation: its engaged \
                         width as a share of the tool diameter and its engaged depth. The cells \
                         are brighter where more of the cutting time is. The faint lines are the \
                         cut force at the planned feed, as shares of the machine's force budget. \
                         A dot is a sample over the budget. A hollow square is a planned cut at \
                         the stated cut. Click a dot or a cell to go to that move.";

/// The colour of a planned-cut mark with no tone on this chart: the ink of
/// the planned dot (`TEXT_STRONG`). The heat cells are squares in the
/// `TEXT_BODY` ink, so a muted outline (the Passes plot's neutral mark)
/// reads as one more cell here (P29-10 10g, judged on the screenshot).
pub const PLANNED_CUT_COLOR: egui::Color32 = tokens::TEXT_STRONG;

/// The legend captions, in their order.
pub const LEGEND_CUT_TIME: &str = "cut time";
pub const LEGEND_OVER_BUDGET: &str = "over budget";
pub const LEGEND_PLANNED: &str = "planned";
pub const LEGEND_PLAYHEAD: &str = "playhead";

/// The hover of the cut-time ramp.
pub const RAMP_HOVER: &str = "Brighter where more of the cutting time is.";

/// The inputs of the chart: the set's plane, samples and field, and the
/// playhead. No simulation state.
pub struct CutChartInput<'a> {
    /// The operation's safe envelope.
    pub env: &'a SafeEnvelope,
    /// The deflection gate's population on the plane, in trace order.
    pub samples: &'a [CutPoint3],
    /// The tool diameter D (mm) the width shares by.
    pub diameter_mm: f64,
    /// The force budget line (N) of the body cut; `None`: no over-budget
    /// dot (a cut with no force limit).
    pub budget_n: Option<f64>,
    /// The force plane of the body cut (core, built once per load report);
    /// `None`: no contour (a drill, no force line).
    pub force_plane: Option<&'a ForcePlane>,
    /// The playhead as a toolpath-local move, when it is in this toolpath.
    pub playhead: Option<usize>,
    /// The toolpath's move count, for the header caption.
    pub move_count: usize,
}

/// What the chart did this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct CutChartResponse {
    /// The toolpath-local move a click asks the playhead to seek to.
    pub seek: Option<usize>,
    /// The map of the drawn plane, so a sentry aims a click.
    pub map: Option<ChartMap>,
}

/// One cell of the heat grid.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cell {
    /// The cutting time (s) of the samples in the cell.
    pub seconds: f64,
    /// The samples in the cell.
    pub count: usize,
    /// The toolpath-local move of the first sample in the cell, in trace
    /// order.
    pub first_move: Option<usize>,
}

/// The heat grid over the data ranges: `x` is the width share (% of D),
/// `y` the depth (mm). Row 0 is the bottom of the depth range.
#[derive(Debug, Clone, PartialEq)]
pub struct CellGrid {
    cells: Vec<Cell>,
    x: (f64, f64),
    y: (f64, f64),
    /// The rows up the depth range, `1..=GRID_ROWS`.
    rows: usize,
    /// The time of each cell that holds time, ascending: the ranks of
    /// [`Self::alpha`].
    ranked_s: Vec<f64>,
    /// The cutting time of every binned sample.
    pub total_s: f64,
    /// The index of the cell with the most cutting time.
    pub densest: Option<usize>,
}

impl CellGrid {
    /// Bin `points` (`x` share, `y` mm, weight s, toolpath-local move),
    /// in trace order, over the ranges `x` and `y` on [`GRID_ROWS`] rows.
    /// A point past a range lands in the edge cell.
    #[must_use]
    pub fn bin(
        points: impl IntoIterator<Item = (f64, f64, f64, usize)>,
        x: (f64, f64),
        y: (f64, f64),
    ) -> Self {
        Self::bin_rows(points, x, y, GRID_ROWS)
    }

    /// [`Self::bin`] on `rows` rows, held to `1..=GRID_ROWS`.
    #[must_use]
    pub fn bin_rows(
        points: impl IntoIterator<Item = (f64, f64, f64, usize)>,
        x: (f64, f64),
        y: (f64, f64),
        rows: usize,
    ) -> Self {
        let rows = rows.clamp(1, GRID_ROWS);
        let mut grid = Self {
            cells: vec![Cell::default(); GRID_COLUMNS * rows],
            x,
            y,
            rows,
            ranked_s: Vec::new(),
            total_s: 0.0,
            densest: None,
        };
        for (px, py, weight, local_move) in points {
            if !(px.is_finite() && py.is_finite() && weight.is_finite()) {
                continue;
            }
            let i = grid.index_of(px, py);
            if let Some(cell) = grid.cells.get_mut(i) {
                cell.seconds += weight.max(0.0);
                cell.count += 1;
                if cell.first_move.is_none() {
                    cell.first_move = Some(local_move);
                }
                grid.total_s += weight.max(0.0);
            }
        }
        grid.densest = grid
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.seconds > 0.0)
            .max_by(|a, b| a.1.seconds.total_cmp(&b.1.seconds))
            .map(|(i, _)| i);
        grid.ranked_s = grid
            .cells
            .iter()
            .map(|c| c.seconds)
            .filter(|s| *s > 0.0)
            .collect();
        grid.ranked_s.sort_by(f64::total_cmp);
        grid
    }

    /// The rows up the depth range.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// The cells, row-major from the bottom-left cell.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The depth range the grid covers (the data range, not the axis).
    #[must_use]
    pub const fn y_range(&self) -> (f64, f64) {
        self.y
    }

    /// The cell of the point `(x, y)`, held on the grid.
    #[must_use]
    pub fn index_of(&self, x: f64, y: f64) -> usize {
        let col = Self::slot(x, self.x, GRID_COLUMNS);
        let row = Self::slot(y, self.y, self.rows);
        row * GRID_COLUMNS + col
    }

    /// The slot of `v` on `range` divided into `n`, clamped.
    fn slot(v: f64, range: (f64, f64), n: usize) -> usize {
        let span = range.1 - range.0;
        if span <= 0.0 || !span.is_finite() {
            return 0;
        }
        let t = ((v - range.0) / span).clamp(0.0, 1.0);
        ((t * n as f64).floor() as usize).min(n - 1)
    }

    /// The column and the row of cell `i`.
    #[must_use]
    pub const fn col_row(i: usize) -> (usize, usize) {
        (i % GRID_COLUMNS, i / GRID_COLUMNS)
    }

    /// The corners of cell `i` in data units, counter-clockwise from the
    /// bottom-left.
    #[must_use]
    pub fn quad(&self, i: usize) -> [[f64; 2]; 4] {
        let (col, row) = Self::col_row(i);
        let (x0, x1) = Self::edges(col, self.x, GRID_COLUMNS);
        let (y0, y1) = Self::edges(row, self.y, self.rows);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    /// The centre of cell `i` in data units.
    #[must_use]
    pub fn centre(&self, i: usize) -> (f64, f64) {
        let q = self.quad(i);
        ((q[0][0] + q[1][0]) / 2.0, (q[0][1] + q[2][1]) / 2.0)
    }

    /// The two edges of slot `k` of `range` divided into `n`.
    fn edges(k: usize, range: (f64, f64), n: usize) -> (f64, f64) {
        let step = (range.1 - range.0) / n as f64;
        (range.0 + step * k as f64, range.0 + step * (k + 1) as f64)
    }

    /// The heat of cell `i`: the share of the cells that hold time whose
    /// time is at most the cell's own (its rank; histogram equalisation).
    /// 0 for an empty cell; 1 for the densest; a cell with more time is
    /// never fainter than a cell with less.
    #[must_use]
    pub fn alpha(&self, i: usize) -> f64 {
        let s = self.cells.get(i).map_or(0.0, |c| c.seconds);
        let n = self.ranked_s.len();
        if s <= 0.0 || n == 0 {
            return 0.0;
        }
        let at_most = self.ranked_s.partition_point(|v| *v <= s);
        (at_most as f64 / n as f64).clamp(0.0, 1.0)
    }

    /// The ramp step of cell `i`: `None` for an empty cell, else
    /// `0..HEAT_STEPS`, the densest cell in the last step.
    #[must_use]
    pub fn step(&self, i: usize) -> Option<usize> {
        let a = self.alpha(i);
        if a <= 0.0 {
            return None;
        }
        Some(((a * HEAT_STEPS as f64).ceil() as usize).clamp(1, HEAT_STEPS) - 1)
    }
}

/// The rows that make a heat cell square on the screen: the columns times
/// the data range's height over the frame width, in screen points, held
/// to `1..=GRID_ROWS`. `data_height` is the data range's share of the
/// frame height times the frame height.
#[must_use]
pub fn square_rows(data_height: f32, frame_width: f32) -> usize {
    if !(data_height > 0.0 && frame_width > 0.0) {
        return GRID_ROWS;
    }
    let rows = (GRID_COLUMNS as f32 * data_height / frame_width).round();
    (rows.max(1.0) as usize).min(GRID_ROWS)
}

/// The hue of the heat map: the histogram's ink family (its in-band bars
/// are `TEXT_FAINT`; the body rung reads at full strength against the
/// panel and under the `TEXT_STRONG` planned dot on its halo).
#[must_use]
pub const fn heat_hue() -> egui::Color32 {
    tokens::TEXT_BODY
}

/// The alpha of step `s` of [`HEAT_STEPS`]: from [`FAINTEST_ALPHA`] in
/// the first step to [`DENSEST_ALPHA`] in the last, in equal steps.
#[must_use]
pub fn step_alpha(s: usize) -> f32 {
    let t = s.min(HEAT_STEPS - 1) as f32 / (HEAT_STEPS - 1) as f32;
    FAINTEST_ALPHA + (DENSEST_ALPHA - FAINTEST_ALPHA) * t
}

/// The colour of a heat cell in step `s`.
#[must_use]
pub fn step_color(s: usize) -> egui::Color32 {
    heat_hue().gamma_multiply(step_alpha(s))
}

/// The over-budget samples that paint, and how many more there are. The
/// samples above `budget` sort by weight, the heaviest first; every k-th
/// paints so that at most [`DOT_CAP`] do.
#[must_use]
pub fn over_budget_dots(samples: &[CutPoint3], budget: Option<f64>) -> (Vec<CutPoint3>, usize) {
    let Some(budget) = budget else {
        return (Vec::new(), 0);
    };
    // P29-10 10g (site 16): core says which sample is over the line.
    let mut over: Vec<CutPoint3> = samples
        .iter()
        .filter(|s| s.over_budget(Some(budget)))
        .copied()
        .collect();
    over.sort_by(|a, b| b.weight_s.total_cmp(&a.weight_s));
    let n = over.len();
    if n <= DOT_CAP {
        return (over, 0);
    }
    let k = n.div_ceil(DOT_CAP);
    let shown: Vec<CutPoint3> = over.into_iter().step_by(k).collect();
    let hidden = n - shown.len();
    (shown, hidden)
}

/// The sample under the playhead: the one whose move is nearest at or
/// below `playhead`, in trace order (the first such sample).
#[must_use]
pub fn playhead_sample(samples: &[CutPoint3], playhead: Option<usize>) -> Option<CutPoint3> {
    let playhead = playhead?;
    samples
        .iter()
        .filter(|s| s.move_index <= playhead)
        .max_by_key(|s| s.move_index)
        .copied()
}

/// The header caption at the right: the move count.
#[must_use]
pub fn header_caption(move_count: usize) -> String {
    format!(
        "{} moves",
        text::group_thousands(i64::try_from(move_count).unwrap_or(i64::MAX))
    )
}

/// The label of the contour at `share` of the budget: "25 %"; the budget
/// line takes its core name ("100 % budget", or "budget, no limit here"
/// where the budget is no limit of the cut, lead ruling O4).
#[must_use]
pub fn contour_label(share: f64, role: BudgetRole) -> String {
    if (share - 1.0).abs() < 1e-9 {
        role.line_name().to_owned()
    } else {
        histogram::format_share(share * 100.0)
    }
}

/// The colour and the width of the contour at `share`: the budget line is
/// `CAUTION` where the budget is a limit of the cut, else `TEXT_FAINT` as
/// every other level.
#[must_use]
pub fn contour_stroke(share: f64, role: BudgetRole) -> (egui::Color32, f32) {
    let budget_line = (share - 1.0).abs() < 1e-9;
    match (budget_line, role) {
        (true, BudgetRole::Limit) => (tokens::CAUTION, BUDGET_CONTOUR_WIDTH),
        (true, BudgetRole::NoLimitHere) => (tokens::TEXT_FAINT, BUDGET_CONTOUR_WIDTH),
        (false, BudgetRole::Limit | BudgetRole::NoLimitHere) => (tokens::TEXT_FAINT, CONTOUR_WIDTH),
    }
}

/// The contour polylines of the force plane, in `[ae mm, ap mm]`, paired
/// with the share: the level lines core built once per load report (no
/// contour is traced per frame). Empty with no plane.
#[must_use]
pub fn contour_lines(plane: Option<&ForcePlane>) -> Vec<(f64, &[Vec<[f64; 2]>])> {
    plane
        .map(|p| {
            p.levels
                .iter()
                .map(|level| (level.share, level.lines.as_slice()))
                .collect()
        })
        .unwrap_or_default()
}

/// The runs of `points` at or under the depth `top`: a run ends at the
/// crossing of the top (interpolated), so a line reaches the frame edge
/// and stops.
#[must_use]
pub fn clip_below(points: &[[f64; 2]], top: f64) -> Vec<Vec<[f64; 2]>> {
    let mut runs: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut run: Vec<[f64; 2]> = Vec::new();
    let at_top = |a: [f64; 2], b: [f64; 2]| -> [f64; 2] {
        let t = if (b[1] - a[1]).abs() > 0.0 {
            (top - a[1]) / (b[1] - a[1])
        } else {
            0.0
        };
        [a[0] + (b[0] - a[0]) * t, top]
    };
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let (a_in, b_in) = (a[1] <= top, b[1] <= top);
        match (a_in, b_in) {
            (true, true) => {
                if run.is_empty() {
                    run.push(*a);
                }
                run.push(*b);
            }
            (true, false) => {
                if run.is_empty() {
                    run.push(*a);
                }
                run.push(at_top(*a, *b));
                runs.push(std::mem::take(&mut run));
            }
            (false, true) => {
                run.push(at_top(*a, *b));
                run.push(*b);
            }
            (false, false) => {}
        }
    }
    if run.len() >= 2 {
        runs.push(run);
    }
    if points.len() == 1
        && let Some(p) = points.first()
        && p[1] <= top
    {
        runs.push(vec![*p]);
    }
    runs
}

/// The depth range of the chart (P17b item 2): the data range the heat
/// grid covers and the axis range the frame draws.
#[derive(Debug, Clone, PartialEq)]
pub struct DepthView {
    /// The top of the data range: `VIEW_HEADROOM` over the deeper of the
    /// deepest sample and the planned depth, never past the scene top.
    pub data_top: f64,
    /// The top of the axis: the data top, or the lowest flat limit above
    /// it with room for its name, never past the scene top.
    pub ap_top: f64,
    /// The axis reaches the scene's own top (a hard stop may be named).
    pub at_scene_top: bool,
    /// The flat limit the axis extended to, in mm.
    pub extended_to: Option<f64>,
    /// The flat limits still above the axis, lowest first: `(label, mm)`.
    pub off_scale: Vec<(String, f64)>,
}

/// The depth view of `scene` for a cut to `deepest` planned at
/// `planned_ap`. `room_share` is the share of the frame height a limit
/// name needs above its line (one caption row over the frame height).
#[must_use]
pub fn depth_view(
    scene: &EnvelopeScene,
    deepest: f64,
    planned_ap: f64,
    room_share: f64,
) -> DepthView {
    let scene_top = scene.ap_range.1;
    let wanted = VIEW_HEADROOM * deepest.max(planned_ap);
    let data_top = if wanted > 0.0 && wanted < scene_top {
        wanted
    } else {
        scene_top
    };
    let eps = 1e-9;
    let mut flat: Vec<(String, f64)> = scene
        .edges
        .iter()
        .filter_map(|edge| flat_depth(edge).map(|mm| (edge.label.clone(), mm)))
        .filter(|(_, mm)| *mm > data_top + eps)
        .collect();
    flat.sort_by(|a, b| a.1.total_cmp(&b.1));
    let lowest = flat.first().map(|(_, mm)| *mm);
    let (ap_top, extended_to) = match lowest {
        Some(mm) if mm <= OFF_SCALE_FACTOR * data_top => {
            let room = room_share.clamp(0.0, 0.5);
            ((mm / (1.0 - room)).min(scene_top), Some(mm))
        }
        Some(_) | None => (data_top, None),
    };
    let off_scale = flat
        .into_iter()
        .filter(|(_, mm)| *mm > ap_top + eps)
        .collect();
    DepthView {
        data_top,
        ap_top,
        at_scene_top: (ap_top - scene_top).abs() < eps,
        extended_to,
        off_scale,
    }
}

/// The depth of a flat limit `edge` (two points at one depth).
fn flat_depth(edge: &envelope::SceneEdge) -> Option<f64> {
    let first = edge.points.first()?;
    let flat = edge.points.len() == 2 && edge.points.iter().all(|p| (p[1] - first[1]).abs() < 1e-9);
    flat.then_some(first[1])
}

/// The hover of a cell: its width and depth, its share of cutting time
/// and its samples; the click line when the cell has a sample.
/// `diameter_mm` is the D the x share counts by: the envelope's width
/// frame (`EnvelopeScene::ae_range.1`), which `view_ranges` reads from
/// the `Diameter` limit of every plane op.
#[must_use]
pub fn cell_hover(grid: &CellGrid, i: usize, diameter_mm: f64) -> String {
    let (share, ap) = grid.centre(i);
    let ae = share / 100.0 * diameter_mm;
    let cell = grid.cells().get(i).copied().unwrap_or_default();
    let time = if grid.total_s > 0.0 {
        histogram::format_share(cell.seconds / grid.total_s * 100.0)
    } else {
        histogram::format_share(0.0)
    };
    let mut lines = vec![format!(
        "ae {ae:.2} mm ({} D) \u{00b7} ap {ap:.2} mm \u{00b7} {time} of cut time \u{00b7} {} {}",
        histogram::format_share(share),
        cell.count,
        if cell.count == 1 { "sample" } else { "samples" }
    )];
    if cell.first_move.is_some() {
        lines.push("Click to go to the first move here.".to_owned());
    }
    lines.join("\n")
}

/// The hover line of an over-budget dot (P29-10 10g, site 17: core
/// writes it, `CutPoint3::over_budget_words`).
#[must_use]
pub fn dot_hover(sample: &CutPoint3, budget_n: Option<f64>) -> Option<String> {
    sample.over_budget_words(budget_n)
}

/// P29-10 10g: the planned marks of the chart at the stated point, each
/// with its colour: [`PLANNED_CUT_COLOR`] (no tone: the planned rows
/// describe the proposal), and the tone of its rows only when the stated point is the
/// proposal (`PlaneMarks::describes_rows`, ruling O5).
#[must_use]
pub fn stated_marks(
    env: &SafeEnvelope,
) -> Vec<(rs_cam_core::feeds::plane::PlaneMark, egui::Color32)> {
    let Some(marks) = env.marks_at_stated() else {
        return Vec::new();
    };
    let mut out: Vec<_> = marks
        .marks()
        .iter()
        .map(|m| {
            let color = if marks.describes_rows() {
                envelope::mark_tone_color(env.rows(), m.cut)
            } else {
                PLANNED_CUT_COLOR
            };
            (*m, color)
        })
        .collect();
    // A mark in `CAUTION` paints last, on top of a muted one at its point.
    out.sort_by_key(|(_, color)| *color == tokens::CAUTION);
    out
}

/// The hover of a planned mark of the chart: the cut at the stated cut;
/// its rows when they describe this point, else where they are.
#[must_use]
pub fn stated_mark_hover(
    env: &SafeEnvelope,
    mark: &rs_cam_core::feeds::plane::PlaneMark,
) -> String {
    if env.marks_at_stated().is_some_and(|m| m.describes_rows()) {
        return envelope::mark_hover(env, mark);
    }
    format!(
        "{} \u{00B7} ap {:.2} \u{00D7} ae {:.2} mm at the stated cut\nIts limit rows describe \
         the proposal: see the Passes tab.",
        mark.cut.label(),
        mark.at.ap_mm,
        mark.at.ae_mm
    )
}

/// The header row, the chart and the legend. Returns what a click asked.
pub fn draw_cut_chart(ui: &mut egui::Ui, input: &CutChartInput<'_>) -> CutChartResponse {
    // The deepest sample joins the view range: a sample past the
    // envelope's own view would clamp into the top edge cell and paint a
    // false cell there (lead review, round 2).
    let deepest = input
        .samples
        .iter()
        .map(|s| s.ap_mm)
        .filter(|ap| ap.is_finite())
        .fold(0.0_f64, f64::max);
    let scene = EnvelopeScene::with_depth_mark(input.env, None, (deepest > 0.0).then_some(deepest));
    let planned_ap = input.env.stated.map_or(0.0, |p| p.ap_mm);
    let x_range = (0.0, 100.0);
    let sx = |ae: f64| scene.ae_share(ae);
    let pt = |p: [f64; 2]| [sx(p[0]), p[1]];
    let pts = |points: &[[f64; 2]]| points.iter().copied().map(pt).collect::<Vec<_>>();

    // The room a limit name needs above its line, as a share of the frame
    // height: one caption row and a gap, over the frame the kit gives a
    // chart of `PLOT_HEIGHT` with no off-scale row (the row count is not
    // known before the view; the gutter only makes the frame shorter by
    // a row, which the clamp of `depth_view` absorbs).
    let row = ui
        .ctx()
        .fonts_mut(|fonts| fonts.row_height(&chart::caption_font()));
    let probe = PlaneChart::new(
        Axis::new(x_range, WIDTH_AXIS_TITLE),
        Axis::new((0.0, 1.0), DEPTH_AXIS_TITLE),
        envelope::PLOT_HEIGHT,
        egui::Sense::hover(),
    );
    let frame = probe.frame(ui);
    let frame_height = frame.height().max(1.0);
    let room_share = f64::from((row + tokens::SPACE_2) / frame_height);
    let view = depth_view(&scene, deepest, planned_ap, room_share);
    let ap_range = (scene.ap_range.0, view.ap_top);
    let data_range = (scene.ap_range.0, view.data_top);
    let at_mark = |p: CutPoint| {
        let p = scene.on_frame(p);
        [sx(p.ae_mm), p.ap_mm.min(view.ap_top)]
    };

    // The samples on the plane, held on the data range. The grid and the
    // dots are rebuilt per frame over the set's samples (the set itself
    // is built once per load report); a frame of a large toolpath pays a
    // pass over them, logged when it is slow.
    let started = std::time::Instant::now();
    // Square cells on the screen: the rows follow the data range's height
    // in the frame (the off-scale row makes the frame a row shorter; the
    // rounding absorbs it).
    let ap_span = (ap_range.1 - ap_range.0).max(f64::EPSILON);
    let data_share = ((data_range.1 - data_range.0) / ap_span).clamp(0.0, 1.0) as f32;
    let rows = square_rows(frame_height * data_share, frame.width());
    let grid = CellGrid::bin_rows(
        input
            .samples
            .iter()
            .map(|s| (sx(s.ae_mm), s.ap_mm, s.weight_s, s.move_index)),
        x_range,
        data_range,
        rows,
    );
    let (dots, hidden) = over_budget_dots(input.samples, input.budget_n);
    let elapsed = started.elapsed();
    if elapsed > std::time::Duration::from_millis(8) {
        tracing::debug!(
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            samples = input.samples.len(),
            "slow sim chart binning"
        );
    }

    draw_header(ui, input.move_count);

    let mut y_axis = Axis::new(ap_range, DEPTH_AXIS_TITLE);
    if scene.top.is_some() && view.at_scene_top {
        y_axis = y_axis.edge(envelope::trim_decimals(ap_range.1));
    }
    let mut plot = PlaneChart::new(
        Axis::new(x_range, WIDTH_AXIS_TITLE).edge(WIDTH_EDGE_TEXT),
        y_axis,
        envelope::PLOT_HEIGHT,
        egui::Sense::click(),
    );
    // The frame edges (D5), under everything: the width D, and the hard
    // top when the range reaches it.
    let frame_lines: Vec<Vec<[f64; 2]>> = envelope::frame_edges(&scene)
        .into_iter()
        .take(if view.at_scene_top { 2 } else { 1 })
        .map(|line| {
            line.into_iter()
                .map(|p| [p[0], p[1].min(view.ap_top)])
                .collect()
        })
        .collect();
    for edge in &frame_lines {
        plot = plot.line(
            ChartLine::new(
                pts(edge),
                egui::Stroke::new(envelope::FRAME_EDGE_WIDTH, tokens::TEXT_FAINT),
            )
            .beneath(true),
        );
    }
    // The iso-force contours, under the fills, the budget line last (the
    // strongest). Each level's runs under the axis top; the longest run
    // of a level carries its name.
    let contour_set = contour_lines(input.force_plane);
    let role = input
        .force_plane
        .map_or(BudgetRole::NoLimitHere, |p| p.role);
    let mut contour_labels: Vec<Label> = Vec::new();
    for (share, lines) in &contour_set {
        let (color, width) = contour_stroke(*share, role);
        let runs: Vec<Vec<[f64; 2]>> = lines
            .iter()
            .flat_map(|line| clip_below(line, view.ap_top))
            .filter(|run| run.len() >= 2)
            .map(|run| pts(&run))
            .collect();
        for run in &runs {
            plot = plot.line(
                ChartLine::new(run.clone(), egui::Stroke::new(width, color))
                    .beneath(true)
                    .soft(true),
            );
        }
        if let Some(longest) = runs.iter().max_by_key(|run| run.len()) {
            contour_labels.push(
                Label::new(
                    contour_label(*share, role),
                    color,
                    LabelAt::line_end(longest.clone()),
                )
                .ground(LabelGround::Ground),
            );
        }
    }
    // The soft fill of the safe region, then the heat cells, one fill per
    // step, over it.
    plot = plot.fill(
        envelope::safe_region_quads(input.env, &scene)
            .into_iter()
            .map(|q| q.map(pt))
            .collect(),
        tokens::OK.gamma_multiply(histogram::WASH_OK),
    );
    let mut steps: Vec<Vec<[[f64; 2]; 4]>> = vec![Vec::new(); HEAT_STEPS];
    for i in 0..grid.cells().len() {
        if let Some(s) = grid.step(i)
            && let Some(step) = steps.get_mut(s)
        {
            step.push(grid.quad(i));
        }
    }
    for (s, quads) in steps.into_iter().enumerate() {
        if !quads.is_empty() {
            plot = plot.fill(quads, step_color(s));
        }
    }
    // The off-scale rows: one quiet gutter row per flat limit still above
    // the axis, the lower limit first. Decided before the frame: the
    // gutter is part of it.
    for (label, mm) in &view.off_scale {
        plot = plot.off_scale_top(
            format!("{label} {} mm", histogram::format_value(*mm)),
            tokens::TEXT_MUTED,
        );
    }
    let frame = plot.frame(ui);
    // The limit lines on the axis range: a vertical limit stops at the
    // top; a curve keeps the points under the top.
    let mut drawn: Vec<(&envelope::SceneEdge, Vec<[f64; 2]>)> = Vec::new();
    for edge in scene.drawn_edges(frame.size()) {
        let Some(first) = edge.points.first() else {
            continue;
        };
        if flat_depth(edge).is_some_and(|mm| mm > view.ap_top + 1e-9) {
            continue;
        }
        let two = edge.points.len() == 2;
        let vertical = two && edge.points.iter().all(|p| (p[0] - first[0]).abs() < 1e-9);
        let points: Vec<[f64; 2]> = if vertical {
            edge.points
                .iter()
                .map(|p| [p[0], p[1].min(view.ap_top)])
                .collect()
        } else {
            edge.points
                .iter()
                .copied()
                .filter(|p| p[1] <= view.ap_top + 1e-9)
                .collect()
        };
        if points.len() < 2 {
            continue;
        }
        drawn.push((edge, points));
    }
    for (edge, points) in &drawn {
        plot = plot.line(ChartLine::new(
            pts(points),
            egui::Stroke::new(scene.edge_width(edge), scene.edge_color(edge)),
        ));
    }

    // The marks: the planned cuts (P29-10 10g), the over-budget dots, the
    // playhead ring, then the planned point LAST on its halo (item 5).
    // A mark at the planned (stated) point is under its white dot: the dot
    // names that point, so the mark is left out.
    let marks: Vec<_> = stated_marks(input.env)
        .into_iter()
        .filter(|(m, _)| {
            !input.env.stated.is_some_and(|p| {
                (p.ap_mm - m.at.ap_mm).abs() < 1e-6 && (p.ae_mm - m.at.ae_mm).abs() < 1e-6
            })
        })
        .collect();
    for (mark, color) in &marks {
        plot = plot.mark(Mark::square(
            at_mark(CutPoint {
                ap_mm: mark.at.ap_mm,
                ae_mm: mark.at.ae_mm,
            }),
            PLANNED_RADIUS,
            egui::Stroke::new(PLANNED_WIDTH, *color),
        ));
    }
    for s in &dots {
        plot = plot.mark(Mark::dot(
            [sx(s.ae_mm), s.ap_mm],
            DOT_RADIUS,
            tokens::CAUTION,
        ));
    }
    // The halo first, the ring over it (a ring at the planned point says
    // "the playhead is at the plan", round 3), the white dot last.
    let planned_at = input.env.stated.map(at_mark);
    if let Some(at) = planned_at {
        plot = plot.mark(Mark::dot(
            at,
            envelope::DOT_RADIUS + HALO_WIDTH,
            tokens::SURFACE_BASE,
        ));
    }
    let playhead = playhead_sample(input.samples, input.playhead);
    if let Some(s) = playhead {
        plot = plot.mark(Mark::ring(
            [sx(s.ae_mm), s.ap_mm.min(view.ap_top)],
            GHOST_RADIUS,
            egui::Stroke::new(RECOMMENDATION_WIDTH, recommendation_color()),
        ));
    }
    if let Some(at) = planned_at {
        plot = plot.mark(Mark::dot(at, envelope::DOT_RADIUS, tokens::TEXT_STRONG));
    }

    // The labels, in priority: each drawn limit on its line (the binding
    // and crossed lines before the muted ones), the hard top, then the
    // contours (the budget line first).
    drawn.sort_by_key(|(e, _)| scene.edge_width(e) < envelope::LINE_WIDTH);
    // P17b item 2: each label ON its line at the line's end, in the line's
    // colour, on a chip of the chart ground (agent B's kit anchor).
    for (edge, points) in &drawn {
        let at = if points.len() == 3 {
            LabelAt::LineEnd(points.windows(2).map(pts).collect())
        } else {
            LabelAt::line_end(pts(points))
        };
        plot = plot.label(
            Label::new(edge.label.clone(), scene.edge_color(edge), at).ground(LabelGround::Ground),
        );
    }
    if let (true, Some(name), Some(top)) =
        (view.at_scene_top, scene.top_label(), frame_lines.get(1))
    {
        plot = plot.label(
            Label::new(name, tokens::TEXT_MUTED, LabelAt::line_end(pts(top)))
                .ground(LabelGround::Ground),
        );
    }
    for label in contour_labels.into_iter().rev() {
        plot = plot.label(label);
    }

    let chart::ChartResponse { response, map } = plot.show(ui);
    let frame = map.frame;

    // The hit: a dot within `HIT_RADIUS`, else the cell under the pointer
    // (none in the empty extension above the data range).
    let hit_at = |pos: egui::Pos2| -> Hit {
        let dot = dots
            .iter()
            .map(|s| (map.to_screen(sx(s.ae_mm), s.ap_mm).distance(pos), s))
            .filter(|(d, _)| *d <= HIT_RADIUS)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| *s);
        let mark = || {
            marks
                .iter()
                .map(|(m, _)| {
                    let at = at_mark(CutPoint {
                        ap_mm: m.at.ap_mm,
                        ae_mm: m.at.ae_mm,
                    });
                    (map.to_screen(at[0], at[1]).distance(pos), m)
                })
                .filter(|(d, _)| *d <= HIT_RADIUS + PLANNED_RADIUS)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, m)| *m)
        };
        match (dot, mark()) {
            (Some(s), _) => Hit::Dot(s),
            (None, Some(m)) => Hit::Mark(m),
            (None, None) => {
                let (x, y) = map.from_screen(pos);
                if y > grid.y_range().1 {
                    Hit::Air
                } else {
                    Hit::Cell(grid.index_of(x, y))
                }
            }
        }
    };
    let mut seek = None;
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
        && frame.contains(pos)
    {
        // A planned mark is a hover only: a click on it seeks the cell
        // under it, so no cell is lost under a mark.
        let cell_under = || {
            let (x, y) = map.from_screen(pos);
            (y <= grid.y_range().1)
                .then(|| {
                    grid.cells()
                        .get(grid.index_of(x, y))
                        .and_then(|c| c.first_move)
                })
                .flatten()
        };
        seek = match hit_at(pos) {
            Hit::Dot(s) => Some(s.move_index),
            Hit::Cell(i) => grid.cells().get(i).and_then(|c| c.first_move),
            Hit::Mark(_) => cell_under(),
            Hit::Air => None,
        };
    }
    if let Some(pos) = response.hover_pos()
        && frame.contains(pos)
    {
        let text = match hit_at(pos) {
            Hit::Dot(s) => {
                let cell = cell_hover(
                    &grid,
                    grid.index_of(sx(s.ae_mm), s.ap_mm),
                    input.diameter_mm,
                );
                Some(match dot_hover(&s, input.budget_n) {
                    Some(words) => format!("{words}\n{cell}"),
                    None => cell,
                })
            }
            Hit::Mark(m) => Some(stated_mark_hover(input.env, &m)),
            Hit::Cell(i) => Some(cell_hover(&grid, i, input.diameter_mm)),
            Hit::Air => None,
        };
        if let Some(text) = text {
            let _ = response.on_hover_ui_at_pointer(|ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                        .extend(),
                );
            });
        }
    }

    draw_legend(ui, !dots.is_empty(), hidden);
    CutChartResponse {
        seek,
        map: Some(map),
    }
}

/// What sits under the pointer.
enum Hit {
    Dot(CutPoint3),
    /// A planned cut (P29-10 10g).
    Mark(rs_cam_core::feeds::plane::PlaneMark),
    Cell(usize),
    /// The empty extension above the data range.
    Air,
}

/// The header row in the card style: the title, the (i) guide, and the
/// move count at the right.
fn draw_header(ui: &mut egui::Ui, move_count: usize) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(TITLE).color(tokens::TEXT_STRONG)).truncate());
        ui.label(egui::RichText::new(tokens::GLYPH_DETAIL).color(tokens::TEXT_MUTED))
            .on_hover_text(GUIDE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(text::caption(header_caption(move_count))).truncate());
        });
    });
}

/// The width of `caption` in the caption font, in points.
fn text_width(ui: &egui::Ui, caption: &str) -> f32 {
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                caption.to_owned(),
                chart::caption_font(),
                tokens::TEXT_MUTED,
            )
            .size()
            .x
    })
}

/// The side of a legend swatch, in points.
const SWATCH: f32 = 10.0;

/// One legend entry: what the swatch paints, and its caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Swatch {
    Ramp,
    OverBudget,
    Planned,
    Playhead,
}

impl Swatch {
    const fn caption(self) -> &'static str {
        match self {
            Self::Ramp => LEGEND_CUT_TIME,
            Self::OverBudget => LEGEND_OVER_BUDGET,
            Self::Planned => LEGEND_PLANNED,
            Self::Playhead => LEGEND_PLAYHEAD,
        }
    }

    /// The width of the swatch: the ramp is a row of squares.
    const fn width(self) -> f32 {
        match self {
            Self::Ramp => SWATCH * LEGEND_RAMP_SQUARES as f32,
            // P29-10 10g: the planned dot and a planned-cut square.
            Self::Planned => 2.0 * SWATCH,
            Self::OverBudget | Self::Playhead => SWATCH,
        }
    }

    fn paint(self, painter: &egui::Painter, rect: egui::Rect) {
        match self {
            Self::Ramp => {
                for k in 0..LEGEND_RAMP_SQUARES {
                    let s = k * (HEAT_STEPS - 1) / (LEGEND_RAMP_SQUARES - 1).max(1);
                    let square = egui::Rect::from_min_size(
                        rect.min + egui::vec2(SWATCH * k as f32, 0.0),
                        egui::Vec2::splat(SWATCH),
                    );
                    let _ = painter.rect_filled(square, 0.0, step_color(s));
                }
            }
            Self::OverBudget => {
                let _ = painter.circle_filled(rect.center(), DOT_RADIUS, tokens::CAUTION);
            }
            Self::Planned => {
                let dot = rect.left_center() + egui::vec2(SWATCH / 2.0, 0.0);
                let _ = painter.circle_filled(dot, envelope::DOT_RADIUS, tokens::TEXT_STRONG);
                let _ = painter.rect_stroke(
                    egui::Rect::from_center_size(
                        dot + egui::vec2(SWATCH, 0.0),
                        egui::Vec2::splat(2.0 * PLANNED_RADIUS),
                    ),
                    0.0,
                    egui::Stroke::new(PLANNED_WIDTH, PLANNED_CUT_COLOR),
                    egui::StrokeKind::Middle,
                );
            }
            Self::Playhead => {
                let _ = painter.circle_stroke(
                    rect.center(),
                    GHOST_RADIUS,
                    egui::Stroke::new(RECOMMENDATION_WIDTH, recommendation_color()),
                );
            }
        }
    }
}

/// The legend rows of painted swatches and captions, as many rows as the
/// width needs, an entry never split across two. The over-budget entry
/// draws only when a dot painted (round 5); its hover names the dots
/// that did not paint.
fn draw_legend(ui: &mut egui::Ui, has_budget: bool, hidden: usize) {
    let entries: Vec<Swatch> = [
        Swatch::Ramp,
        Swatch::OverBudget,
        Swatch::Planned,
        Swatch::Playhead,
    ]
    .into_iter()
    .filter(|s| has_budget || *s != Swatch::OverBudget)
    .collect();
    let gap = tokens::SPACE_2;
    let entry_width =
        |ui: &egui::Ui, s: Swatch| s.width() + gap + text_width(ui, s.caption()) + tokens::SPACE_3;
    let room = ui.available_width();
    let mut rows: Vec<Vec<Swatch>> = vec![Vec::new()];
    let mut used = 0.0_f32;
    for s in entries {
        let w = entry_width(ui, s);
        if used > 0.0 && used + w > room {
            rows.push(Vec::new());
            used = 0.0;
        }
        used += w;
        if let Some(row) = rows.last_mut() {
            row.push(s);
        }
    }
    for row in rows {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for s in row {
                let (rect, swatch_response) =
                    ui.allocate_exact_size(egui::vec2(s.width(), SWATCH), egui::Sense::hover());
                s.paint(&ui.painter_at(rect), rect);
                if s == Swatch::Ramp {
                    let _ = swatch_response.on_hover_text(RAMP_HOVER);
                }
                let response = ui.add(egui::Label::new(text::caption(s.caption())).truncate());
                if s == Swatch::OverBudget && hidden > 0 {
                    let _ = response.on_hover_text(format!(
                        "{} dots paint; and {} more are over the budget.",
                        DOT_CAP,
                        text::group_thousands(i64::try_from(hidden).unwrap_or(i64::MAX))
                    ));
                }
                ui.add_space(tokens::SPACE_3);
            }
        });
    }
}
