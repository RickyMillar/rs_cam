//! The sim width x depth chart (feeds contract P17, change 3): the focused
//! toolpath's safe envelope on the chart kit, read-only, with the cut
//! samples on it.
//!
//! The Metrics tab draws it after the four cut-metric cards and one
//! hairline (layout B, operator ruling through the lead, 2026-11-06): the
//! cards are the verdict, the chart is the drill-down.
//!
//! # What it draws
//!
//! The plane is the envelope plot of the Passes tab (`ui/feeds/envelope.rs`
//! `draw_plane`): the same axes (the width as a share of D, the depth in
//! mm), the same ranges, frame edges, limit lines, labels and soft fill,
//! through [`EnvelopeScene`] and the public helpers of that module. No
//! candidate, no drag, no apply.
//!
//! The samples layer, between the fill and the limit lines:
//!
//! - Density cells: the samples binned on a [`GRID_COLUMNS`] x
//!   [`GRID_ROWS`] grid over the chart ranges by cutting time. A cell's
//!   alpha is `sqrt(cell time / densest cell time)`, painted in
//!   [`ALPHA_BUCKETS`] buckets of `TEXT_BODY`. A sample past a range
//!   clamps to the edge cell. The depth range follows the samples (round
//!   3): `VIEW_HEADROOM` over the deeper of the deepest sample and the
//!   planned depth, never past the hard top; the scene is built with the
//!   deepest sample as one more mark ([`EnvelopeScene::with_depth_mark`]),
//!   so every limit under that range is in it. A flat depth limit above
//!   the range is an off-scale row in the gutter above the frame
//!   ([`PlaneChart::off_scale_top`]: the arrow beside the name and the
//!   value, outside the data area); a curve or a vertical limit draws as
//!   far as the frame.
//! - The steady cluster block: a 1 pt `TEXT_MUTED` outline about the nine
//!   cells; the cluster label keeps clear of every cell at [`CLEAR_SHARE`]
//!   of the densest or more and points at the block with a leader.
//! - Over-budget dots: each sample whose force is above the force budget
//!   paints a `CAUTION` dot (the budget does not gate an export, so no
//!   `DANGER`). Only under the `ForceBudget` rule (lead ruling 3). At most
//!   [`DOT_CAP`] dots paint, every k-th by weight; the legend hover names
//!   the rest.
//! - The playhead sample: the sample whose move is nearest at or below the
//!   playhead, as the quietest mark (a 1 pt ring at the recommendation
//!   colour, under the planned dot), when the playhead is in this toolpath.
//! - The planned point: the white dot of the envelope plot, on top.
//! - The steady cluster: the densest cell and its eight neighbours; their
//!   share of cutting time is one label at the cell centre, through the
//!   label kit (a leader when the label moves).
//!
//! A click on a dot seeks the playhead to that sample's move; a click on
//! a non-empty cell seeks to the cell's first sample in trace order. The
//! hover names the cell under the pointer. One legend row of painted
//! swatches closes the section (no glyph: the font sentry).
//!
//! # What it does NOT do
//!
//! It reads no simulation state: [`draw_cut_chart`] takes the set's
//! inputs and the playhead, so a headless sentry drives it. It builds no
//! sample: the set is built once per load report
//! (`state::simulation::build_cut_metric_set`).

use rs_cam_core::feeds::envelope::{CutPoint, SafeEnvelope};
use rs_cam_core::tool_load::cut_force::CutPoint3;

use super::components::chart::{
    self, Axis, ChartLine, ChartMap, Label, LabelAt, LabelGround, Mark, PlaneChart,
};
use super::components::{histogram, text};
use super::feeds::envelope::{
    self, DEPTH_AXIS_TITLE, EnvelopeScene, GHOST_RADIUS, RECOMMENDATION_WIDTH, VIEW_HEADROOM,
    WIDTH_AXIS_TITLE, WIDTH_EDGE_TEXT, recommendation_color,
};
use super::tokens;

/// The columns of the density grid across the width range.
pub const GRID_COLUMNS: usize = 48;

/// The rows of the density grid up the depth range.
pub const GRID_ROWS: usize = 32;

/// The alpha buckets of the density cells: one fill per bucket (round 3:
/// sixteen, so the shade reads as a gradient).
pub const ALPHA_BUCKETS: usize = 16;

/// The most over-budget dots the chart paints.
pub const DOT_CAP: usize = 400;

/// The cluster label keeps clear of every cell whose alpha is this share
/// of the densest cell's or more (round 5).
pub const CLEAR_SHARE: f64 = 0.25;

/// The radius of an over-budget dot, in points: under the planned dot, so
/// the plan reads above the samples.
pub const DOT_RADIUS: f32 = 2.5;

/// A click or a hover takes a dot within this distance, in points.
pub const HIT_RADIUS: f32 = 4.0;

/// The alpha of the densest cell: the histogram's out-of-band wash, five
/// times, so the cluster reads against the safe-region wash under it and
/// stays under the white planned dot (judged by eye in the P17 snapshots).
pub const DENSEST_ALPHA: f32 = histogram::WASH_OUT * 5.0;

/// The alpha of the lightest bucket: the out-of-band wash itself. A cell
/// with a few samples still reads; the square root alone left it unseen
/// in the first P17 snapshot.
pub const LIGHTEST_ALPHA: f32 = histogram::WASH_OUT;

/// The section title.
pub const TITLE: &str = "Width \u{00D7} depth";

/// The (i) hover of the section.
pub const GUIDE: &str = "Each sample of the cut on the envelope of the operation: its engaged \
                         width as a share of the tool diameter and its engaged depth. The shade \
                         is cutting time. A dot is a sample over the force budget. Click a dot \
                         or a cell to go to that move.";

/// The legend captions, in their order.
pub const LEGEND_CUT_TIME: &str = "cut time";
pub const LEGEND_OVER_BUDGET: &str = "over budget";
pub const LEGEND_PLANNED: &str = "planned";
pub const LEGEND_PLAYHEAD: &str = "playhead";

/// The inputs of the chart: the set's plane and samples, and the
/// playhead. No simulation state.
pub struct CutChartInput<'a> {
    /// The operation's safe envelope.
    pub env: &'a SafeEnvelope,
    /// The deflection gate's population on the plane, in trace order.
    pub samples: &'a [CutPoint3],
    /// The tool diameter D (mm) the width shares by.
    pub diameter_mm: f64,
    /// The force budget line (N); `None`: no over-budget dot.
    pub budget_n: Option<f64>,
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

/// One cell of the density grid.
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

/// The density grid over the chart ranges: `x` is the width share (% of
/// D), `y` the depth (mm). Row 0 is the bottom of the depth range.
#[derive(Debug, Clone, PartialEq)]
pub struct CellGrid {
    cells: Vec<Cell>,
    x: (f64, f64),
    y: (f64, f64),
    /// The cutting time of every binned sample.
    pub total_s: f64,
    /// The index of the cell with the most cutting time.
    pub densest: Option<usize>,
}

impl CellGrid {
    /// Bin `points` (`x` share, `y` mm, weight s, toolpath-local move),
    /// in trace order, over the ranges `x` and `y`. A point past a range
    /// lands in the edge cell.
    #[must_use]
    pub fn bin(
        points: impl IntoIterator<Item = (f64, f64, f64, usize)>,
        x: (f64, f64),
        y: (f64, f64),
    ) -> Self {
        let mut grid = Self {
            cells: vec![Cell::default(); GRID_COLUMNS * GRID_ROWS],
            x,
            y,
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
        grid
    }

    /// The cells, row-major from the bottom-left cell.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cell of the point `(x, y)`, held on the grid.
    #[must_use]
    pub fn index_of(&self, x: f64, y: f64) -> usize {
        let col = Self::slot(x, self.x, GRID_COLUMNS);
        let row = Self::slot(y, self.y, GRID_ROWS);
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
        let (y0, y1) = Self::edges(row, self.y, GRID_ROWS);
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

    /// The alpha of cell `i`: the square root of its share of the densest
    /// cell's time. 0 for an empty cell; 1 for the densest.
    #[must_use]
    pub fn alpha(&self, i: usize) -> f64 {
        let densest = self
            .densest
            .and_then(|d| self.cells.get(d))
            .map_or(0.0, |c| c.seconds);
        let s = self.cells.get(i).map_or(0.0, |c| c.seconds);
        if densest > 0.0 && s > 0.0 {
            (s / densest).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The bucket of cell `i`: `None` for an empty cell, else `0..ALPHA_BUCKETS`,
    /// the densest cell in the last bucket.
    #[must_use]
    pub fn bucket(&self, i: usize) -> Option<usize> {
        let a = self.alpha(i);
        if a <= 0.0 {
            return None;
        }
        Some(((a * ALPHA_BUCKETS as f64).ceil() as usize).clamp(1, ALPHA_BUCKETS) - 1)
    }

    /// The cells within one cell of `i`: `i` and its eight neighbours
    /// inside the grid.
    #[must_use]
    pub fn neighbourhood(i: usize) -> Vec<usize> {
        let (col, row) = Self::col_row(i);
        let mut out = Vec::with_capacity(9);
        for dr in -1_i64..=1 {
            for dc in -1_i64..=1 {
                let c = col as i64 + dc;
                let r = row as i64 + dr;
                if (0..GRID_COLUMNS as i64).contains(&c) && (0..GRID_ROWS as i64).contains(&r) {
                    out.push(r as usize * GRID_COLUMNS + c as usize);
                }
            }
        }
        out
    }

    /// The steady cluster: the densest cell and the share of cutting time
    /// in it and its eight neighbours, `0..=1`. `None` with no time.
    #[must_use]
    pub fn cluster(&self) -> Option<(usize, f64)> {
        let densest = self.densest?;
        if self.total_s <= 0.0 {
            return None;
        }
        let seconds: f64 = Self::neighbourhood(densest)
            .into_iter()
            .filter_map(|j| self.cells.get(j))
            .map(|c| c.seconds)
            .sum();
        Some((densest, (seconds / self.total_s).clamp(0.0, 1.0)))
    }
}

/// The alpha of bucket `b` of [`ALPHA_BUCKETS`]: from [`LIGHTEST_ALPHA`]
/// in the first bucket to [`DENSEST_ALPHA`] in the last, in equal steps.
#[must_use]
pub fn bucket_alpha(b: usize) -> f32 {
    let t = b.min(ALPHA_BUCKETS - 1) as f32 / (ALPHA_BUCKETS - 1) as f32;
    LIGHTEST_ALPHA + (DENSEST_ALPHA - LIGHTEST_ALPHA) * t
}

/// The colour of a density cell in bucket `b`.
#[must_use]
pub fn bucket_color(b: usize) -> egui::Color32 {
    tokens::TEXT_BODY.gamma_multiply(bucket_alpha(b))
}

/// The over-budget samples that paint, and how many more there are. The
/// samples above `budget` sort by weight, the heaviest first; every k-th
/// paints so that at most [`DOT_CAP`] do.
#[must_use]
pub fn over_budget_dots(samples: &[CutPoint3], budget: Option<f64>) -> (Vec<CutPoint3>, usize) {
    let Some(budget) = budget else {
        return (Vec::new(), 0);
    };
    let mut over: Vec<CutPoint3> = samples
        .iter()
        .filter(|s| s.force_n.is_some_and(|f| f.is_finite() && f > budget))
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

/// The header caption at the right: the move count and the steady share.
/// `short`: the steady share alone, for a rail too narrow for both.
#[must_use]
pub fn header_caption(move_count: usize, steady_share: Option<f64>, short: bool) -> String {
    let moves = format!(
        "{} moves",
        text::group_thousands(i64::try_from(move_count).unwrap_or(i64::MAX))
    );
    match steady_share {
        Some(share) if short => format!("{} steady", histogram::format_share(share * 100.0)),
        Some(share) => format!(
            "{moves} \u{00b7} {} steady",
            histogram::format_share(share * 100.0)
        ),
        None => moves,
    }
}

/// The cluster label: "steady passes · 84 %".
#[must_use]
pub fn cluster_label(share: f64) -> String {
    format!(
        "steady passes \u{00b7} {}",
        histogram::format_share(share * 100.0)
    )
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

/// The hover line of an over-budget dot.
#[must_use]
pub fn dot_hover(force_n: f64, budget_n: f64) -> String {
    format!(
        "force {} N \u{00b7} over the budget {} N",
        histogram::format_value(force_n),
        histogram::format_value(budget_n)
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
    // Round 3: the range follows the samples, so a cut under 2 mm is not
    // a 15 px band under a 9 mm limit. The scene's range (the limits in
    // view, never past the hard top) caps it.
    let planned_ap = input.env.stated.map_or(0.0, |p| p.ap_mm);
    let wanted = VIEW_HEADROOM * deepest.max(planned_ap);
    let ap_top = if wanted > 0.0 && wanted < scene.ap_range.1 {
        wanted
    } else {
        scene.ap_range.1
    };
    let ap_range = (scene.ap_range.0, ap_top);
    let at_scene_top = (ap_top - scene.ap_range.1).abs() < 1e-9;
    let x_range = (0.0, 100.0);
    let sx = |ae: f64| scene.ae_share(ae);
    let pt = |p: [f64; 2]| [sx(p[0]), p[1]];
    let pts = |points: &[[f64; 2]]| points.iter().copied().map(pt).collect::<Vec<_>>();
    let at_mark = |p: CutPoint| {
        let p = scene.on_frame(p);
        [sx(p.ae_mm), p.ap_mm]
    };

    // The samples on the plane, held on the frame. The grid and the dots
    // are rebuilt per frame over the set's samples (the set itself is
    // built once per load report); a frame of a large toolpath pays a
    // pass over them, logged when it is slow.
    let started = std::time::Instant::now();
    let grid = CellGrid::bin(
        input
            .samples
            .iter()
            .map(|s| (sx(s.ae_mm), s.ap_mm, s.weight_s, s.move_index)),
        x_range,
        ap_range,
    );
    let cluster = grid.cluster();
    let (dots, hidden) = over_budget_dots(input.samples, input.budget_n);
    let elapsed = started.elapsed();
    if elapsed > std::time::Duration::from_millis(8) {
        tracing::debug!(
            elapsed_ms = elapsed.as_secs_f64() * 1000.0,
            samples = input.samples.len(),
            "slow sim chart binning"
        );
    }

    draw_header(ui, input.move_count, cluster.map(|(_, s)| s));

    let mut y_axis = Axis::new(ap_range, DEPTH_AXIS_TITLE);
    if scene.top.is_some() && at_scene_top {
        y_axis = y_axis.edge(envelope::trim_decimals(ap_range.1));
    }
    let mut plot = PlaneChart::new(
        Axis::new(x_range, WIDTH_AXIS_TITLE).edge(WIDTH_EDGE_TEXT),
        y_axis,
        envelope::PLOT_HEIGHT,
        egui::Sense::click(),
    )
    .fill(
        envelope::safe_region_quads(input.env, &scene)
            .into_iter()
            .map(|q| q.map(pt))
            .collect(),
        tokens::OK.gamma_multiply(histogram::WASH_OK),
    );
    // The density cells, one fill per bucket, over the wash.
    let mut buckets: Vec<Vec<[[f64; 2]; 4]>> = vec![Vec::new(); ALPHA_BUCKETS];
    for i in 0..grid.cells().len() {
        if let Some(b) = grid.bucket(i)
            && let Some(bucket) = buckets.get_mut(b)
        {
            bucket.push(grid.quad(i));
        }
    }
    for (b, quads) in buckets.into_iter().enumerate() {
        if !quads.is_empty() {
            plot = plot.fill(quads, bucket_color(b));
        }
    }
    // The frame edges (D5), under the limit lines: the width D, and the
    // hard top when the range reaches it.
    let frame_lines: Vec<Vec<[f64; 2]>> = envelope::frame_edges(&scene)
        .into_iter()
        .take(if at_scene_top { 2 } else { 1 })
        .map(|line| line.into_iter().map(|p| [p[0], p[1].min(ap_top)]).collect())
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
    // The off-scale markers: the name and the value of each flat limit
    // above the range, one gutter row each above the frame (the kit's
    // idiom), the lower limit first. Decided before the frame: the gutter
    // is part of it.
    let flat_above = |edge: &envelope::SceneEdge| -> Option<f64> {
        let first = edge.points.first()?;
        let flat =
            edge.points.len() == 2 && edge.points.iter().all(|p| (p[1] - first[1]).abs() < 1e-9);
        (flat && first[1] > ap_top + 1e-9).then_some(first[1])
    };
    let mut off_scale: Vec<(&envelope::SceneEdge, f64)> = scene
        .edges
        .iter()
        .filter_map(|edge| flat_above(edge).map(|mm| (edge, mm)))
        .collect();
    off_scale.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (edge, mm) in &off_scale {
        plot = plot.off_scale_top(
            format!("{} {} mm", edge.label, histogram::format_value(*mm)),
            scene.edge_color(edge),
        );
    }
    let frame = plot.frame(ui);
    // The limit lines on the chart range: a vertical limit stops at the
    // top; a curve keeps the points under the top.
    let mut drawn: Vec<(&envelope::SceneEdge, Vec<[f64; 2]>)> = Vec::new();
    for edge in scene.drawn_edges(frame.size()) {
        let Some(first) = edge.points.first() else {
            continue;
        };
        if flat_above(edge).is_some() {
            continue;
        }
        let two = edge.points.len() == 2;
        let vertical = two && edge.points.iter().all(|p| (p[0] - first[0]).abs() < 1e-9);
        let points: Vec<[f64; 2]> = if vertical {
            edge.points
                .iter()
                .map(|p| [p[0], p[1].min(ap_top)])
                .collect()
        } else {
            edge.points
                .iter()
                .copied()
                .filter(|p| p[1] <= ap_top + 1e-9)
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

    // The marks: the over-budget dots, the planned point, the playhead.
    for s in &dots {
        plot = plot.mark(Mark::dot(
            [sx(s.ae_mm), s.ap_mm],
            DOT_RADIUS,
            tokens::CAUTION,
        ));
    }
    // The playhead ring is the quietest mark and paints under the planned
    // dot (round 3), so the plan reads when the two coincide.
    let playhead = playhead_sample(input.samples, input.playhead);
    if let Some(s) = playhead {
        plot = plot.mark(Mark::ring(
            [sx(s.ae_mm), s.ap_mm.min(ap_top)],
            GHOST_RADIUS,
            egui::Stroke::new(RECOMMENDATION_WIDTH, recommendation_color()),
        ));
    }
    if let Some(p) = input.env.stated {
        let p = CutPoint {
            ap_mm: p.ap_mm.min(ap_top),
            ae_mm: p.ae_mm,
        };
        plot = plot.mark(Mark::dot(
            at_mark(p),
            envelope::DOT_RADIUS,
            tokens::TEXT_STRONG,
        ));
    }

    // The labels: the cluster first, then each drawn limit on its line,
    // the binding and crossed lines before the muted ones, the hard top
    // last (the envelope plot's order).
    if let Some((densest, share)) = cluster {
        // Round 5: the cluster is visible as a 1 pt outline about its cell
        // block (the nine cells, merged to their bounding rect); the label
        // keeps clear of every cell at a quarter of the densest or more
        // and points at the block with a leader always drawn.
        let block = CellGrid::neighbourhood(densest)
            .into_iter()
            .map(|i| grid.quad(i))
            .fold(None, |acc: Option<[f64; 4]>, q| {
                let (x0, x1, y0, y1) = (q[0][0], q[1][0], q[0][1], q[2][1]);
                Some(acc.map_or([x0, x1, y0, y1], |[a, b, c, d]| {
                    [a.min(x0), b.max(x1), c.min(y0), d.max(y1)]
                }))
            });
        if let Some([x0, x1, y0, y1]) = block {
            plot = plot.line(ChartLine::new(
                vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]],
                egui::Stroke::new(1.0, tokens::TEXT_MUTED),
            ));
            let dense: Vec<[[f64; 2]; 4]> = (0..grid.cells().len())
                .filter(|i| grid.alpha(*i) >= CLEAR_SHARE)
                .map(|i| grid.quad(i))
                .collect();
            plot = plot.keep_clear(dense);
            let a = map_size(frame, x_range, ap_range, x1 - x0, y1 - y0);
            plot = plot.label(
                Label::new(
                    cluster_label(share),
                    tokens::TEXT_BODY,
                    LabelAt::Mark {
                        at: [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
                        radius: a.x.hypot(a.y) / 2.0,
                    },
                )
                .ground(LabelGround::None)
                .leader(),
            );
        }
    }
    drawn.sort_by_key(|(e, _)| scene.edge_width(e) < envelope::LINE_WIDTH);
    for (edge, points) in &drawn {
        let at = if points.len() == 3 {
            LabelAt::Lines(points.windows(2).map(pts).collect())
        } else {
            LabelAt::Line(pts(points))
        };
        plot = plot.label(
            Label::new(edge.label.clone(), scene.edge_color(edge), at).ground(LabelGround::None),
        );
    }
    if let (true, Some(name), Some(top)) = (at_scene_top, scene.top_label(), frame_lines.get(1)) {
        plot = plot.label(
            Label::new(name, tokens::TEXT_MUTED, LabelAt::Line(pts(top))).ground(LabelGround::None),
        );
    }

    let chart::ChartResponse { response, map } = plot.show(ui);
    let frame = map.frame;

    // The hit: a dot within `HIT_RADIUS`, else the cell under the pointer.
    let hit_at = |pos: egui::Pos2| -> Hit {
        let dot = dots
            .iter()
            .map(|s| (map.to_screen(sx(s.ae_mm), s.ap_mm).distance(pos), s))
            .filter(|(d, _)| *d <= HIT_RADIUS)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| *s);
        match dot {
            Some(s) => Hit::Dot(s),
            None => {
                let (x, y) = map.from_screen(pos);
                Hit::Cell(grid.index_of(x, y))
            }
        }
    };
    let mut seek = None;
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
        && frame.contains(pos)
    {
        seek = match hit_at(pos) {
            Hit::Dot(s) => Some(s.move_index),
            Hit::Cell(i) => grid.cells().get(i).and_then(|c| c.first_move),
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
                match (s.force_n, input.budget_n) {
                    (Some(f), Some(b)) => format!("{}\n{cell}", dot_hover(f, b)),
                    (None, _) | (_, None) => cell,
                }
            }
            Hit::Cell(i) => cell_hover(&grid, i, input.diameter_mm),
        };
        let _ = response.on_hover_ui_at_pointer(|ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }

    draw_legend(ui, !dots.is_empty(), hidden);
    CutChartResponse {
        seek,
        map: Some(map),
    }
}

/// The size in points of a span of `dx` by `dy` data units on `frame`.
fn map_size(frame: egui::Rect, x: (f64, f64), y: (f64, f64), dx: f64, dy: f64) -> egui::Vec2 {
    let sx = if x.1 > x.0 { dx / (x.1 - x.0) } else { 0.0 };
    let sy = if y.1 > y.0 { dy / (y.1 - y.0) } else { 0.0 };
    egui::vec2(sx as f32 * frame.width(), sy as f32 * frame.height())
}

/// What sits under the pointer.
enum Hit {
    Dot(CutPoint3),
    Cell(usize),
}

/// The header row in the card style: the title, the (i) guide, and the
/// caption at the right. The caption drops the move count when the two
/// do not fit beside the title (the 240 point rail).
fn draw_header(ui: &mut egui::Ui, move_count: usize, steady_share: Option<f64>) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(TITLE).color(tokens::TEXT_STRONG)).truncate());
        ui.label(egui::RichText::new(tokens::GLYPH_DETAIL).color(tokens::TEXT_MUTED))
            .on_hover_text(GUIDE);
        let room = ui.available_width() - tokens::SPACE_3;
        let fits = |caption: &str| text_width(ui, caption) <= room;
        let full = header_caption(move_count, steady_share, false);
        let caption = if fits(&full) {
            full
        } else {
            header_caption(move_count, steady_share, true)
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(text::caption(caption)).truncate());
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
    Wash,
    OverBudget,
    Planned,
    Playhead,
}

impl Swatch {
    const fn caption(self) -> &'static str {
        match self {
            Self::Wash => LEGEND_CUT_TIME,
            Self::OverBudget => LEGEND_OVER_BUDGET,
            Self::Planned => LEGEND_PLANNED,
            Self::Playhead => LEGEND_PLAYHEAD,
        }
    }

    fn paint(self, painter: &egui::Painter, rect: egui::Rect) {
        match self {
            Self::Wash => {
                let _ = painter.rect_filled(rect, 0.0, bucket_color(ALPHA_BUCKETS - 1));
            }
            Self::OverBudget => {
                let _ = painter.circle_filled(rect.center(), DOT_RADIUS, tokens::CAUTION);
            }
            Self::Planned => {
                let _ =
                    painter.circle_filled(rect.center(), envelope::DOT_RADIUS, tokens::TEXT_STRONG);
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
        Swatch::Wash,
        Swatch::OverBudget,
        Swatch::Planned,
        Swatch::Playhead,
    ]
    .into_iter()
    .filter(|s| has_budget || *s != Swatch::OverBudget)
    .collect();
    let gap = tokens::SPACE_2;
    let entry_width =
        |ui: &egui::Ui, s: Swatch| SWATCH + gap + text_width(ui, s.caption()) + tokens::SPACE_3;
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
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(SWATCH), egui::Sense::hover());
                s.paint(&ui.painter_at(rect), rect);
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
