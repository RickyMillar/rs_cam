//! C4 (G-RESTDOC; operator ruling D69, 2026-10-28): the contour on each
//! level (`Adaptive3dConfig::contour_each_level`, D68 option 2 made
//! product).
//!
//! A 3D Rough on remaining stock clips its planner stock to the centre
//! containment (the boundary clear), so the level regions hold no centre
//! cell over the stock its tool meets beyond the containment: the reach
//! band. Without this pass the band is cut at the lowest level by the edge
//! of the disc, deeper than one Depth/Pass. With it, after each level one
//! contour runs along the containment where the band still stands, never
//! more than one Depth/Pass under any stock in reach.
//!
//! The band state is the band's column tops, read before the boundary clear
//! ([`super::path::PriorStockTops`]), lowered under every emitted cut and
//! link. The cuts go out through the planner's one segment door
//! (`push_segment_with_stamp`): each entry gets its floor and keep-down
//! proof from `plan_entry`, and each cut is stamped in the planner stock.

use crate::dexel_stock::TriDexelStock;
use crate::geo::{P2, P3};
use crate::machine::kinematics::{LinkKinematics, RetractDescent, retract_saves_time};
use crate::polygon::FlattenPolicy;
use crate::surface::slope::SurfaceHeightmap;

use super::centre_clip::CentreClip;
use super::clearing::{
    ClearZLevelContext, PlannerCursor, StepLoadCutter, capped_step_factor, polygon_rings,
    push_segment_with_stamp, shipped_cut_path, step_load_on,
};
use super::path::{Adaptive3dSegment, PriorStockTops};
use super::{Adaptive3dRuntimeEvent, stock_has_material_above, stock_top_z_at};

/// The margin above a cell's floor under which the cell holds no material:
/// the margin of the level anchor (`path::level_anchor_z`).
pub(super) const MATERIAL_MARGIN_MM: f64 = 0.01;

/// The reach band of a rest rough and the line its contour runs on.
pub(super) struct BandEdge {
    /// Row-major band-cell tops on the planner grid; `None` off the band.
    tops: Vec<Option<f64>>,
    rows: usize,
    cols: usize,
    origin_u: f64,
    origin_v: f64,
    cell: f64,
    /// The cell reach of the tool disc: R + cs/2, the reach of
    /// [`PriorStockTops`] (a cell whose square the disc touches).
    reach: f64,
    /// The pass line: each containment ring, one point per cell.
    rings: Vec<Vec<P2>>,
    /// Segments already lowered into the band state.
    scanned: usize,
    /// The machine's link timing for the merge rule; `None`: no merge.
    link_kinematics: Option<LinkKinematics>,
}

impl BandEdge {
    /// The band of a rough on prior stock: the cells in the tool's reach
    /// ([`PriorStockTops`]) whose centre lies OUTSIDE the centre containment
    /// and that hold material. `None` when no band cell holds material.
    pub(super) fn new(
        stock: &TriDexelStock,
        prior: &PriorStockTops,
        clip: &CentreClip,
        tool_radius: f64,
        chord_tolerance: f64,
        link_kinematics: Option<LinkKinematics>,
    ) -> Option<Self> {
        let grid = &stock.z_grid;
        let polys = clip.polygons();
        let cols = prior.cols.max(1);
        let mut tops = vec![None; grid.rows * grid.cols];
        let mut band_cells = 0usize;
        for (i, top) in prior.tops.iter().enumerate() {
            let Some(t) = top else { continue };
            let (x, y) = grid.cell_to_world(i / cols, i % cols);
            let p = P2::new(x, y);
            if !polys.iter().any(|q| q.contains_point(&p))
                && let Some(slot) = tops.get_mut(i)
            {
                *slot = Some(*t);
                band_cells += 1;
            }
        }
        if band_cells == 0 {
            return None;
        }
        // The pass line: the containment moved the offset library's own
        // bound inside (`FlattenPolicy::UNTOLERANCED_MM`), so the centre
        // clip, which reads the same polygons, keeps every point of it.
        let step = grid.cell_size;
        let mut rings = Vec::new();
        for poly in polys {
            for inner in crate::polygon::offset_polygon(poly, FlattenPolicy::UNTOLERANCED_MM) {
                for ring in polygon_rings(&inner) {
                    // The line the emitter will ship: the ring simplified at
                    // the op's chord tolerance, as the emitter's RDP does.
                    // The band is read and lowered along that one line, so
                    // no cell the read counts is one the cut passes by.
                    let ring = simplified_ring(ring, chord_tolerance);
                    let pts = resample_ring(&ring, step);
                    if pts.len() >= 3 {
                        rings.push(pts);
                    }
                }
            }
        }
        tracing::debug!(
            band_cells,
            rings = rings.len(),
            "C4 contour each level: the reach band"
        );
        Some(Self {
            tops,
            rows: grid.rows,
            cols: grid.cols,
            origin_u: grid.origin_u,
            origin_v: grid.origin_v,
            cell: grid.cell_size,
            reach: tool_radius + grid.cell_size * 0.5,
            rings,
            scanned: 0,
            link_kinematics,
        })
    }

    /// The `(row, col)` of every grid cell whose centre is within the read
    /// reach of `(x, y)`.
    fn cells_in_reach(&self, x: f64, y: f64) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.cells_within(x, y, self.reach)
    }

    /// The `(row, col)` of every grid cell whose centre is within `r` of
    /// `(x, y)`.
    fn cells_within(&self, x: f64, y: f64, r: f64) -> impl Iterator<Item = (usize, usize)> + '_ {
        let to_index = |v: f64| -> usize {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "floored at 0 and clamped to the grid size below"
            )]
            let i = v.max(0.0) as usize;
            i
        };
        let c0 = to_index(((x - r - self.origin_u) / self.cell).floor());
        let c1 =
            to_index(((x + r - self.origin_u) / self.cell).ceil()).min(self.cols.saturating_sub(1));
        let r0 = to_index(((y - r - self.origin_v) / self.cell).floor());
        let r1 =
            to_index(((y + r - self.origin_v) / self.cell).ceil()).min(self.rows.saturating_sub(1));
        (r0..=r1)
            .flat_map(move |row| (c0..=c1).map(move |col| (row, col)))
            .filter(move |&(row, col)| {
                let cx = self.origin_u + col as f64 * self.cell;
                let cy = self.origin_v + row as f64 * self.cell;
                (cx - x).powi(2) + (cy - y).powi(2) <= r * r
            })
    }

    /// The highest band top within reach of `(x, y)`.
    fn band_top_in_reach(&self, x: f64, y: f64) -> f64 {
        self.cells_in_reach(x, y)
            .filter_map(|(row, col)| self.tops.get(row * self.cols + col).copied().flatten())
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Lower the band under the path `path` (each point and every cell
    /// along it).
    fn lower_along(&mut self, path: &[P3]) {
        let mut points = Vec::new();
        for w in path.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else {
                continue;
            };
            let len = (b.x - a.x).hypot(b.y - a.y);
            let n = crate::util::steps::steps(len, self.cell).unwrap_or(1);
            for j in 0..=n {
                let t = j as f64 / n.max(1) as f64;
                points.push(P3::new(
                    a.x + t * (b.x - a.x),
                    a.y + t * (b.y - a.y),
                    a.z + t * (b.z - a.z),
                ));
            }
        }
        if let [only] = path {
            points.push(*only);
        }
        for p in points {
            let cells: Vec<(usize, usize)> = self.cells_in_reach(p.x, p.y).collect();
            for (row, col) in cells {
                if let Some(Some(top)) = self.tops.get_mut(row * self.cols + col) {
                    *top = top.min(p.z);
                }
            }
        }
    }

    /// Lower the band under every cut and link emitted since the last scan.
    /// A cut lowers along the path the emitter ships
    /// ([`shipped_cut_path`]), as the planner stamp does: the simplified
    /// chord can pass a cell the raw points pass by (found on the C4
    /// sentry: a 60 mm chord 0.05 mm off the ring left the band 0.94 mm
    /// higher than the state read).
    fn scan(&mut self, segments: &[Adaptive3dSegment], ctx: &ClearZLevelContext<'_>) {
        let from = self.scanned.min(segments.len());
        let drape = ctx.stamp_drape();
        for seg in segments.get(from..).unwrap_or(&[]) {
            match seg {
                Adaptive3dSegment::Cut(path) => {
                    let shipped =
                        // The band reads no tool position; with a tool that
                        // does not cut at its centre the first descent of a
                        // cut is read from `path[0]` here (K5, P7 C).
                        // P36 B: no fed chain either (an empty lead).
                        shipped_cut_path(
                            path,
                            ctx.cut_tolerance,
                            ctx.min_cutting_radius,
                            &drape,
                            None,
                            &mut crate::tool::body::FedLead::default(),
                        );
                    self.lower_along(&shipped);
                }
                Adaptive3dSegment::Link(p) => self.lower_along(&[*p]),
                Adaptive3dSegment::Rapid(_)
                | Adaptive3dSegment::RapidWithFloor { .. }
                | Adaptive3dSegment::Marker(_) => {}
            }
        }
        self.scanned = segments.len();
    }
}

/// The closed `ring` simplified by the emitter's RDP
/// (`toolpath::simplify_path_3d`) at `tolerance`, as a closed ring with its
/// first point not repeated.
///
/// RDP on a closed ring depends on where the ring starts, and the polygon
/// library does not fix a ring's first vertex from run to run. So the ring
/// first starts at its vertex of least X, then least Y: a property of the
/// geometry, so the line is too.
fn simplified_ring(ring: &[P2], tolerance: f64) -> Vec<P2> {
    let start = ring
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)))
        .map_or(0, |(i, _)| i);
    let mut closed: Vec<P3> = ring
        .iter()
        .cycle()
        .skip(start)
        .take(ring.len())
        .map(|p| P3::new(p.x, p.y, 0.0))
        .collect();
    if let Some(first) = closed.first().copied() {
        closed.push(first);
    }
    let mut out: Vec<P2> = crate::toolpath::simplify_path_3d(&closed, tolerance)
        .iter()
        .map(|p| P2::new(p.x, p.y))
        .collect();
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// `ring` (closed) with one point at most `step` apart, from its first
/// vertex; the closing point is not repeated.
fn resample_ring(ring: &[P2], step: f64) -> Vec<P2> {
    let mut pts = Vec::new();
    for k in 0..ring.len() {
        let (Some(a), Some(b)) = (ring.get(k), ring.get((k + 1) % ring.len())) else {
            continue;
        };
        let n = crate::util::steps::steps((b.x - a.x).hypot(b.y - a.y), step)
            .unwrap_or(1)
            .max(1);
        for j in 0..n {
            let t = j as f64 / n as f64;
            pts.push(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
        }
    }
    pts
}

/// The highest planner-stock top within the cell reach of `(x, y)` that
/// stands above its own floor (surface plus stock to leave): the stock
/// inside the containment the contour must not cut more than one
/// Depth/Pass under. Cells over the part read their floor, so a wall does
/// not lift the contour.
fn planner_top_in_reach(
    band: &BandEdge,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
    x: f64,
    y: f64,
) -> f64 {
    band.cells_in_reach(x, y)
        .filter(|&(row, col)| row < stock.z_grid.rows && col < stock.z_grid.cols)
        .filter_map(|(row, col)| {
            let floor = surface_hm.z_or_bbox_floor_at(row, col) + stock_to_leave;
            stock_has_material_above(stock, row, col, floor + MATERIAL_MARGIN_MM)
                .then(|| stock_top_z_at(stock, row, col))
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// The gaps of a ring's `standing` flags that the one feed-or-retract rule
/// says to feed through: each gap between two standing runs (cyclic) whose
/// feed, from the last point of the run before it to the first point of the
/// run after it at the gap's own Z, is not slower than a retract, a rapid
/// and a plunge re-entry (`retract_saves_time`, W6.1). The returned flags
/// are the input with those gaps set. No kinematics: no gap is fed.
pub(super) fn merge_short_gaps(
    points: &[P3],
    standing: &[bool],
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    kinematics: Option<&LinkKinematics>,
) -> Vec<bool> {
    let mut out = standing.to_vec();
    let n = points.len().min(standing.len());
    let Some(timing) = kinematics else {
        return out;
    };
    let is_standing = |i: usize| standing.get(i % n).copied().unwrap_or(false);
    let Some(s0) = (0..n).find(|&i| is_standing(i)) else {
        return out;
    };
    if (0..n).all(is_standing) {
        return out;
    }
    let mut k = 1;
    while k <= n {
        if is_standing(s0 + k) {
            k += 1;
            continue;
        }
        // A gap: from the standing point before it to the next standing
        // point after it.
        let gap_start = k;
        while k < n && !is_standing(s0 + k) {
            k += 1;
        }
        let (Some(&from), Some(&to)) = (
            points.get((s0 + gap_start - 1) % n),
            points.get((s0 + k) % n),
        ) else {
            break;
        };
        let mut path: Vec<P3> = (gap_start..k)
            .filter_map(|j| points.get((s0 + j) % n).copied())
            .collect();
        path.push(to);
        if !retract_saves_time(
            from,
            &path,
            feed_rate,
            safe_z,
            RetractDescent::Plunge {
                rate_mm_min: plunge_rate,
            },
            timing,
        ) {
            for j in gap_start..k {
                if let Some(f) = out.get_mut((s0 + j) % n) {
                    *f = true;
                }
            }
        }
        k += 1;
    }
    out
}

/// The runs of `points` whose flag is set, cyclic: each run in ring order.
/// A ring flagged everywhere gives one closed run (its first point
/// repeated at the end).
fn standing_runs(points: &[P3], flags: &[bool]) -> Vec<Vec<P3>> {
    let n = points.len().min(flags.len());
    let flag = |i: usize| flags.get(i % n).copied().unwrap_or(false);
    let Some(start) = (0..n).find(|&i| !flag(i)) else {
        let mut closed: Vec<P3> = points.get(..n).unwrap_or(&[]).to_vec();
        if let Some(first) = closed.first().copied() {
            closed.push(first);
        }
        return if closed.len() >= 2 {
            vec![closed]
        } else {
            Vec::new()
        };
    };
    let mut runs = Vec::new();
    let mut cur: Vec<P3> = Vec::new();
    for k in 1..=n {
        let i = (start + k) % n;
        if flag(i) {
            if let Some(p) = points.get(i) {
                cur.push(*p);
            }
        } else if !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

/// What one level's contour did, for the level's debug span.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LevelContourStats {
    /// The cuts emitted.
    pub(super) runs: usize,
    /// The highest chip-area load of a cut move, before the cap
    /// (`tool_load::chip_load_fraction`, read on the planner stock and the
    /// band).
    pub(super) peak_load: f64,
    /// The lowest feed factor a cap set (1: no cap).
    pub(super) min_factor: f64,
}

/// The contour of this level along the containment where the reach band
/// stands. Each point's Z is `max(floor, top − Depth/Pass − tolerance)`,
/// where `floor` is the level draped on the surface plus stock to leave
/// and `top` the highest of the band state and the planner stock in reach;
/// a point "stands" when the band top in reach is more than one planner
/// cell above that Z. Runs on one ring join by [`merge_short_gaps`]. The
/// runs go out nearest end first.
///
/// The load (the operator's addition to C4): a contour at the edge of the
/// disc is close to a half slot. Each run's moves are read by the one load
/// (`step_load_on`, `chip_load_fraction`) on the planner stock and the band;
/// a run whose peak passes the pass ceiling (the radial fraction of
/// `adaptive::pass_engagement_limit`, the ceiling of the Geodesic step cap)
/// is cut between `FeedCapStart` and `FeedCapEnd` at the lowest
/// `capped_step_factor` of its moves, so it carries `Move::feed_cap`.
pub(super) fn level_contour_pass(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
) -> LevelContourStats {
    let mut stats = LevelContourStats {
        min_factor: 1.0,
        ..LevelContourStats::default()
    };
    let mut guard = ctx.band_edge.borrow_mut();
    let Some(band) = guard.as_mut() else {
        return stats;
    };
    band.scan(segments, ctx);
    let cell = band.cell;
    let mut runs: Vec<Vec<P3>> = Vec::new();
    for ring in &band.rings {
        let mut points = Vec::with_capacity(ring.len());
        let mut standing = Vec::with_capacity(ring.len());
        for p in ring {
            let surface = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
            let floor = if surface == f64::NEG_INFINITY {
                z_level
            } else {
                (surface + ctx.stock_to_leave).max(z_level)
            };
            let band_top = band.band_top_in_reach(p.x, p.y);
            let top = band_top.max(planner_top_in_reach(
                band,
                material_stock,
                surface_hm,
                ctx.stock_to_leave,
                p.x,
                p.y,
            ));
            let z = floor.max(top - ctx.depth_per_pass - ctx.tolerance);
            standing.push(band_top > z + cell);
            points.push(P3::new(p.x, p.y, z));
        }
        let flags = merge_short_gaps(
            &points,
            &standing,
            ctx.feed_rate,
            ctx.plunge_rate,
            ctx.safe_z,
            band.link_kinematics.as_ref(),
        );
        runs.extend(standing_runs(&points, &flags));
    }
    // The feed factor of each run, read before any run is cut (the band
    // state is lowered after the level, by `scan`).
    let band_ref: &BandEdge = band;
    let top_at = |row: usize, col: usize| -> f64 {
        let band_top = band_ref
            .tops
            .get(row * band_ref.cols + col)
            .copied()
            .flatten()
            .unwrap_or(f64::NEG_INFINITY);
        stock_top_z_at(material_stock, row, col).max(band_top)
    };
    let factors: Vec<Option<f64>> = runs
        .iter()
        .map(|run| run_feed_cap(ctx, material_stock, &top_at, run, &mut stats))
        .collect();
    drop(guard);
    tracing::debug!(
        z = z_level,
        runs = runs.len(),
        "C4 contour each level: runs"
    );
    emit_runs(
        ctx,
        material_stock,
        runs.into_iter().zip(factors).collect(),
        cell,
        segments,
        cursor,
        &mut stats,
        None,
    );
    if let Some(band) = ctx.band_edge.borrow_mut().as_mut() {
        band.scan(segments, ctx);
    }
    stats
}

/// The feed cap of one contour run (the operator's addition to C4): each
/// move of the run is read by the one load (`step_load_on`,
/// `chip_load_fraction`) with the column tops of `top_at`. A run whose peak
/// passes the pass ceiling (the radial fraction of
/// `adaptive::pass_engagement_limit`, the ceiling of the Geodesic step cap)
/// gets the lowest `capped_step_factor` of its moves; else `None`. The peak
/// joins `stats`.
fn run_feed_cap(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &TriDexelStock,
    top_at: &dyn Fn(usize, usize) -> f64,
    run: &[P3],
    stats: &mut LevelContourStats,
) -> Option<f64> {
    let ceiling = crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        crate::adaptive::pass_engagement_limit(ctx.stepover, ctx.tool_radius),
    );
    let load_cutter = StepLoadCutter {
        lut: ctx.lut,
        diameter: 2.0 * ctx.tool_radius,
        depth_per_pass: ctx.depth_per_pass,
    };
    let mut peak = 0.0_f64;
    let mut factor = 1.0_f64;
    for w in run.windows(2) {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
            continue;
        };
        let load = step_load_on(
            material_stock,
            top_at,
            a,
            b,
            ctx.tool_radius,
            ctx.stay_down_clearance_mm,
            &load_cutter,
        );
        peak = peak.max(load);
        if load > ceiling {
            factor = factor.min(capped_step_factor(
                ceiling,
                load,
                a,
                b,
                ctx.feed_rate,
                ctx.plunge_rate,
                None,
            ));
        }
    }
    stats.peak_load = stats.peak_load.max(peak);
    (peak > ceiling).then_some(factor)
}

/// Emit the contour runs `left` (each with its feed cap), nearest end
/// first, through the planner's one segment door: an entry, then the cut
/// between `FeedCapStart` and `FeedCapEnd` when it has a cap. The C4
/// contour and the P42 wall pass both call it, so a run of either is
/// planned, stamped and capped by one code path.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn emit_runs(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    mut left: Vec<(Vec<P3>, Option<f64>)>,
    cell: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
    stats: &mut LevelContourStats,
    hold: Option<StripHold<'_>>,
) {
    let drape = ctx.stamp_drape();
    while !left.is_empty() {
        let here = cursor.last_pos.map_or((0.0, 0.0), |p| (p.x, p.y));
        let mut best = (0usize, false, f64::INFINITY);
        for (k, (run, _)) in left.iter().enumerate() {
            for (end, reversed) in [(run.first(), false), (run.last(), true)] {
                if let Some(e) = end {
                    let d = (e.x - here.0).powi(2) + (e.y - here.1).powi(2);
                    if d < best.2 {
                        best = (k, reversed, d);
                    }
                }
            }
        }
        let (mut path, cap) = left.swap_remove(best.0);
        if best.1 {
            path.reverse();
        }
        // A closed ring starts at its point nearest the tool. Its first
        // vertex is the containment polygon's, which the polygon library
        // does not fix from run to run; the nearest point (ties by X, then
        // Y) is a property of the geometry, so the output is too.
        if path.len() > 2 && path.first() == path.last() {
            path.pop();
            let key = |p: &P3| ((p.x - here.0).powi(2) + (p.y - here.1).powi(2), p.x, p.y);
            if let Some((start, _)) = path.iter().enumerate().min_by(|(_, a), (_, b)| {
                let (ka, kb) = (key(a), key(b));
                ka.0.total_cmp(&kb.0)
                    .then(ka.1.total_cmp(&kb.1))
                    .then(ka.2.total_cmp(&kb.2))
            }) {
                path.rotate_left(start);
            }
            if let Some(first) = path.first().copied() {
                path.push(first);
            }
        }
        let Some(first) = path.first().copied() else {
            continue;
        };
        if path.len() == 1 {
            // A single standing point: one cell along the ring is the
            // shortest cut that is a cut.
            path.push(P3::new(first.x + cell, first.y, first.z));
        }
        // P45: the strip pass holds the one-pass Z along the chords that
        // ship, in the order and from the start point that ship.
        if let Some(hold) = hold {
            path = hold_shipped_chords(ctx, material_stock, hold, path, cell);
        }
        push_segment_with_stamp(
            segments,
            material_stock,
            ctx.lut,
            ctx.tool_radius,
            cursor,
            Adaptive3dSegment::Rapid(first),
            ctx.safe_z,
            ctx.cut_tolerance,
            ctx.min_cutting_radius,
            &drape,
        );
        if let Some(factor) = cap {
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::FeedCapStart {
                    factor,
                    source: super::FeedCapSource::LevelContour,
                },
            ));
            stats.min_factor = stats.min_factor.min(factor);
        }
        push_segment_with_stamp(
            segments,
            material_stock,
            ctx.lut,
            ctx.tool_radius,
            cursor,
            Adaptive3dSegment::Cut(path),
            ctx.safe_z,
            ctx.cut_tolerance,
            ctx.min_cutting_radius,
            &drape,
        );
        if cap.is_some() {
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::FeedCapEnd,
            ));
        }
        stats.runs += 1;
    }
}

/// P45: what [`hold_shipped_chords`] reads.
#[derive(Clone, Copy)]
struct StripHold<'a> {
    state: &'a StripTops,
    surface_hm: &'a SurfaceHeightmap,
    z_level: f64,
}

/// P45: raise the Z of a run of the strip pass until each chord that ships
/// holds the one-pass Z of [`wall_read`] at each point along it.
///
/// The Z of a run is read at the ring points. The emitter ships the
/// simplified chords (`simplify_path_3d` at the cut tolerance, in
/// `path::emitted_cut_path`), which leave a ring point by up to that
/// tolerance, in XY and in Z. A chord that drops a corner point of the line
/// holds cell centres that no ring point holds (on the Wanaka back a chord
/// of round 1 left the corner point (230.200, 59.800) by 0.09 mm and held 6
/// centres of the column X 233.2 up to 0.061 mm inside its edge, which the
/// ring points have on their edge).
///
/// So this reads the chords that ship: the same simplification on the same
/// points, a read at one cell along each chord, and each point of the run
/// under a chord with a higher read is raised to that read. It repeats on
/// the raised run, because the raise changes the chords. Each repeat that
/// changes the run raises a point to one of the reads of the run, so the
/// count of repeats is at most the count of points.
fn hold_shipped_chords(
    ctx: &ClearZLevelContext<'_>,
    stock: &TriDexelStock,
    hold: StripHold<'_>,
    mut run: Vec<P3>,
    cell: f64,
) -> Vec<P3> {
    if cell <= 0.0 {
        return run;
    }
    for _ in 0..run.len() {
        let keep = crate::toolpath::simplify_path_3d_keep_mask(&run, ctx.cut_tolerance);
        let kept: Vec<usize> = keep
            .iter()
            .enumerate()
            .filter_map(|(i, &k)| k.then_some(i))
            .collect();
        let mut changed = false;
        for pair in kept.windows(2) {
            let (&[ia, ib], Some(a), Some(b)) = (
                pair,
                pair.first().and_then(|&i| run.get(i)).copied(),
                pair.get(1).and_then(|&i| run.get(i)).copied(),
            ) else {
                continue;
            };
            let n = crate::util::steps::steps((b.x - a.x).hypot(b.y - a.y), cell)
                .unwrap_or(1)
                .max(1);
            let mut need = f64::NEG_INFINITY;
            for j in 0..=n {
                let t = j as f64 / n as f64;
                let p = P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
                if ctx.centre_clip.is_some_and(|clip| !clip.contains(p.x, p.y)) {
                    continue;
                }
                let floor = super::clearing::lift_to_level(ctx, hold.surface_hm, hold.z_level, p).z;
                let read = wall_read(
                    ctx,
                    stock,
                    hold.surface_hm,
                    (hold.z_level, WallRule::RemainingStock),
                    p,
                    floor,
                    Some(hold.state),
                );
                if read.z > a.z + t * (b.z - a.z) {
                    need = need.max(read.z);
                }
            }
            if need.is_finite() {
                for p in run.get_mut(ia..=ib).unwrap_or(&mut []) {
                    if p.z < need {
                        p.z = need;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    run
}

/// P42: the wall pass of a level ([`region_wall_pass`]) is on. The session
/// sets it with the one-pass rule (`Adaptive3dLinking::one_pass_hold`):
/// `OnePassHold::RemainingStock` where the planner stock is the simulated
/// stock on the simulation cell, `OnePassHold::FreshStock` (P43) on a fresh
/// block ([`WallRule`]).
#[derive(Debug, Clone)]
pub(super) struct WallPass {
    /// The machine's link timing for the merge rule of two runs on one
    /// ring (`merge_short_gaps`); `None`: no merge.
    pub(super) link_kinematics: Option<LinkKinematics>,
    /// Which line the pass runs on and what a point reads.
    pub(super) rule: WallRule,
}

/// The two forms of the wall pass (`OnePassHold`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WallRule {
    /// P42: the planner stock is the simulated stock on the simulation
    /// cell. The line is one planner cell plus the cut tolerance inside
    /// the raw outline of each planned region; a point reads the strip in
    /// its disc ([`wall_read`]) and repeats while a round lowers it.
    RemainingStock,
    /// P43: a fresh block on the coarse planner cell (`tool radius / 6`).
    /// The 2D plan of a level does not always cut to the limit of its
    /// tool-centre region, and no read of a coarse cell finds a strip
    /// narrower than a cell. So the line is nearer to the wall than that
    /// limit: one planner cell less [`fresh_line_shift`] inside the raw
    /// outline of each planned region, the same line at each level of one
    /// wall. A point ships by
    /// the wall alone (no test that stock stands in front of it), once per
    /// level.
    FreshStock,
}

/// P43: how much nearer to the wall (mm) the wall line of a fresh rough
/// stands than the least margin of a plan, which is one planner cell from
/// the raw region boundary (`clearing::grown_outlines` refuses a region
/// under it). The line is `cell - shift` inside the raw outline.
///
/// Why not 0: the plan line of a level and the wall line of the level above
/// ship with the emission tolerances, and the edge of a disc that passes
/// the line of the level above cuts a sliver at the full height of the wall
/// (measured: nodes 5 to 12 micrometres inside the disc, 14.5 mm tall). The
/// stack is three times the cut tolerance `t`: the chord tolerance of the
/// plan cut, the simplification of the wall line and the chord tolerance of
/// the wall cut.
///
/// The bound: the raw boundary is within half a cell of the true
/// tool-centre limit, and the nearest-cell read of the lift is inside it by
/// less than half a cell (research note `P3_C22_3d_turns.md` 11.8). So the
/// line keeps half a cell after its own two tolerances: the shift is at
/// most `cell / 2 - 2 t`, and never under 0.
///
/// Measured on the Wanaka back (18 mm tool, cell 1.5 mm, tolerance 0.1 mm:
/// the line is 1.2 mm inside the raw outline; lead ruling D-1, 2026-11-20):
/// over 4.77 m of wall line the tool is never nearer than 0.934 mm to the
/// model wall at its own height, and no sample along a wall is lifted onto
/// the wall top. The one planner cell of `grown_outlines` (D19) stays the
/// rule of the PLAN margin.
///
/// The line reads the RAW outline, not the grown outline of the plan: the
/// smoothing of the plan insets each region by its own shortfall
/// (`grown_outlines`), so the limit of the plan moves from level to level
/// (measured: 0.022 mm between two levels of one wall, and the lower wall
/// line then cut that sliver 9.0 mm tall). The raw outline at a wall is the
/// same cells at each level.
pub(super) fn fresh_line_shift(cell: f64, cut_tolerance: f64) -> f64 {
    let t = cut_tolerance.max(0.0);
    (3.0 * t).min(cell / 2.0 - 2.0 * t).max(0.0)
}

#[cfg(test)]
thread_local! {
    /// Test control only: the one-pass rule with no wall pass (the tree
    /// before P42), so that a test can show that its claim needs the pass.
    pub(super) static WALL_PASS_OFF_FOR_CONTROL: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// One point of a wall-pass line and its last read.
struct WallPoint {
    xy: P2,
    /// The strip top of the round that last cut this point; `+inf` before
    /// its first cut. `WallRule::FreshStock`: the Z of its one cut.
    cut_from: f64,
}

/// The read of one wall-pass point on the planner stock.
struct WallRead {
    /// The Z of the cut: the level floor, raised so that no cell whose
    /// centre is inside the disc stands more than one Depth/Pass above the
    /// cutter surface over it.
    z: f64,
    /// The highest stock above the cutter surface, as a top less the
    /// profile height, over the cells that the disc covers in full and
    /// where no centre of the level has material to plan. `-inf`: none.
    strip_top: f64,
    /// A material cell within one planner cell of the disc stands more
    /// than the one-pass limit above the cutter: stock that a plan holds
    /// (the one-pass rule, the cut-length dial, another By Area job).
    /// The plan keeps one planner cell off such stock (the margin that the
    /// one-pass rule counts on); so does the wall pass.
    beside_held_stock: bool,
    /// The model wall top over the strip: the highest tool-centre surface
    /// plus the stock to leave over the strip cells (a centre on a strip
    /// cell stands on the wall). `-inf`: no strip.
    wall_top: f64,
    /// P44: the strip read without the material test: the highest stock
    /// over the cells that the disc covers in full and that stand more than
    /// one planner cell above the cutter, a material cell of the level or
    /// not. `-inf`: none. The region contour reads it
    /// ([`ContourRule::Region`]). `WallRule::FreshStock`: `-inf` (a coarse
    /// planner cell cannot read it, and no region contour runs).
    disc_top: f64,
}

/// [`WallRead`] at `p` with the level floor `floor`.
///
/// `WallRule::FreshStock` (P43) changes three reads, each for the coarse
/// planner cell:
/// - the Z is the floor, and is raised to one Depth/Pass under the stock
///   inside the disc only where that stock is over the one-pass limit (the
///   promise itself; no margin of a cell);
/// - the strip top is the highest stock over the cells that the disc
///   TOUCHES and that are not material cells of the level. A strip narrower
///   than a cell is not a cell; the wall cells behind it carry the height
///   that the levels above left;
/// - the wall top is the highest tool-centre surface plus the stock to
///   leave within two planner cells of the point: the point is at most one
///   cell inside the raw region boundary, and the centre of a cell outside
///   it is at most one cell beyond.
fn wall_read(
    ctx: &ClearZLevelContext<'_>,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    (z_level, rule): (f64, WallRule),
    p: P2,
    floor: f64,
    strip: Option<&StripTops>,
) -> WallRead {
    let grid = &stock.z_grid;
    let cs = grid.cell_size;
    let r = ctx.tool_radius;
    let r_sq = r * r;
    let index = |v: f64, origin: f64, n: usize, up: bool| -> usize {
        let i = (v - origin) / cs;
        let i = if up { i.ceil() } else { i.floor() };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..n before the cast"
        )]
        {
            i.clamp(0.0, n.saturating_sub(1) as f64) as usize
        }
    };
    // The box of the cells within one planner cell of the disc.
    let far = r + cs;
    let (row_lo, row_hi) = (
        index(p.y - far, grid.origin_v, grid.rows, false),
        index(p.y + far, grid.origin_v, grid.rows, true),
    );
    let (col_lo, col_hi) = (
        index(p.x - far, grid.origin_u, grid.cols, false),
        index(p.x + far, grid.origin_u, grid.cols, true),
    );
    // Pass 1: the highest stock, less the profile, over the cells whose
    // centre is inside the disc. It sets the Z of the cut.
    let mut over_top = f64::NEG_INFINITY;
    let mut inside: Vec<(usize, usize, f64, f64, f64)> = Vec::new();
    for row in row_lo..=row_hi {
        let dv = grid.origin_v + row as f64 * cs - p.y;
        for col in col_lo..=col_hi {
            let du = grid.origin_u + col as f64 * cs - p.x;
            let d_sq = du * du + dv * dv;
            if d_sq > r_sq {
                continue;
            }
            // P45: the strip pass counts a centre that the disc holds with
            // room to spare, the same test as the lowering of its state
            // (`StripTops::lower_under`). A centre on the disc edge is in
            // neither: no cut takes stock there.
            if strip.is_some() && !TriDexelStock::disc_holds_cell_core(du, dv, r_sq, cs) {
                continue;
            }
            let profile = ctx
                .lut
                .height_at_dist_sq(d_sq.min(ctx.lut.radius_sq()))
                .unwrap_or(0.0);
            // P45: the strip pass reads the bound of the stock at the cell
            // centre (`StripTops`), not the ray top: the ray top of a cell
            // at the edge of earlier discs is a blend.
            let top = strip.map_or_else(|| stock_top_z_at(stock, row, col), |s| s.at(row, col));
            let over = top - profile;
            if over > floor {
                over_top = over_top.max(over);
                inside.push((row, col, du, dv, over));
            }
        }
    }
    let z = match rule {
        WallRule::RemainingStock => floor.max(over_top - ctx.depth_per_pass),
        WallRule::FreshStock if ctx.one_pass.over_limit(over_top - floor) => {
            over_top - ctx.depth_per_pass
        }
        WallRule::FreshStock => floor,
    };
    let mut strip_top = f64::NEG_INFINITY;
    let mut wall_top = f64::NEG_INFINITY;
    if rule == WallRule::FreshStock {
        // P43, the strip top: the cells that the disc touches.
        for row in row_lo..=row_hi {
            let dv = grid.origin_v + row as f64 * cs - p.y;
            for col in col_lo..=col_hi {
                let du = grid.origin_u + col as f64 * cs - p.x;
                if TriDexelStock::disc_cell_coverage(du, dv, r_sq, cs) <= 0.0
                    || super::clearing::cell_has_material(
                        stock,
                        surface_hm,
                        row,
                        col,
                        z_level,
                        ctx.stock_to_leave,
                    )
                {
                    continue;
                }
                let profile = ctx
                    .lut
                    .height_at_dist_sq((du * du + dv * dv).min(ctx.lut.radius_sq()))
                    .unwrap_or(0.0);
                strip_top = strip_top.max(stock_top_z_at(stock, row, col) - profile);
            }
        }
        // P43, the wall top: the tool-centre surface within two cells.
        let near = 2.0 * cs;
        for row in index(p.y - near, grid.origin_v, grid.rows, false)
            ..=index(p.y + near, grid.origin_v, grid.rows, true)
        {
            let dv = grid.origin_v + row as f64 * cs - p.y;
            for col in index(p.x - near, grid.origin_u, grid.cols, false)
                ..=index(p.x + near, grid.origin_u, grid.cols, true)
            {
                let du = grid.origin_u + col as f64 * cs - p.x;
                if du * du + dv * dv <= near * near {
                    wall_top =
                        wall_top.max(surface_hm.z_or_bbox_floor_at(row, col) + ctx.stock_to_leave);
                }
            }
        }
        inside.clear();
    }
    // Pass 2 (`WallRule::RemainingStock`): the strip. A cell counts when
    // the disc covers it in full (the coverage of the stamp), it stands
    // more than one planner cell above the cutter, and it is not a material
    // cell of the level.
    let mut disc_top = f64::NEG_INFINITY;
    for (row, col, du, dv, over) in inside {
        if over <= z + cs {
            continue;
        }
        // P45: the strip pass counts each cell whose centre the disc holds:
        // its cut lowers that centre, so the next round reads the cut
        // (`StripTops`). The other contours read the ray top, which a part
        // cover lowers by a fraction only; they count the cells that the
        // disc covers in full.
        if strip.is_none() && TriDexelStock::disc_cell_coverage(du, dv, r_sq, cs) < 1.0 {
            continue;
        }
        disc_top = disc_top.max(over);
        if strip.is_some() {
            continue;
        }
        if super::clearing::cell_has_material(
            stock,
            surface_hm,
            row,
            col,
            z_level,
            ctx.stock_to_leave,
        ) {
            continue;
        }
        strip_top = strip_top.max(over);
        wall_top = wall_top.max(surface_hm.z_or_bbox_floor_at(row, col) + ctx.stock_to_leave);
    }
    // Pass 3: held stock beside the disc. The cells whose centre is inside
    // the disc stand at most one Depth/Pass above the cutter (pass 1), so
    // only the ring of one cell around the disc can hold such a cell.
    let mut beside_held_stock = false;
    'held: for row in row_lo..=row_hi {
        let dv = grid.origin_v + row as f64 * cs - p.y;
        for col in col_lo..=col_hi {
            let du = grid.origin_u + col as f64 * cs - p.x;
            let d_sq = du * du + dv * dv;
            // The ring starts where pass 1 stops (P45: the strip read holds
            // a centre with room to spare, so a centre on the disc edge is
            // in this ring).
            let in_pass_1 = if strip.is_some() {
                d_sq <= r_sq && TriDexelStock::disc_holds_cell_core(du, dv, r_sq, cs)
            } else {
                d_sq <= r_sq
            };
            if in_pass_1 || d_sq > far * far {
                continue;
            }
            // The profile height at the cutter edge, as the stamp reads a
            // cell past the edge.
            let profile = ctx
                .lut
                .height_at_dist_sq(ctx.lut.radius_sq())
                .unwrap_or(0.0);
            if ctx
                .one_pass
                .over_limit(stock_top_z_at(stock, row, col) - profile - z)
                && super::clearing::cell_has_material(
                    stock,
                    surface_hm,
                    row,
                    col,
                    z_level,
                    ctx.stock_to_leave,
                )
            {
                beside_held_stock = true;
                break 'held;
            }
        }
    }
    WallRead {
        z,
        strip_top,
        beside_held_stock,
        wall_top,
        disc_top,
    }
}

/// The level of a wall pass: its Z and the Z of the next level of the plan
/// (`None` at the last level).
#[derive(Debug, Clone, Copy)]
pub(super) struct WallLevel {
    pub(super) z_level: f64,
    pub(super) next_level_z: Option<f64>,
}

/// The heights of one wall-pass point (Z, mm).
#[derive(Debug, Clone, Copy)]
pub(super) struct WallHeights {
    /// The strip top ([`WallRead::strip_top`]).
    pub(super) strip_top: f64,
    /// The model wall top over the strip ([`WallRead::wall_top`]).
    pub(super) wall_top: f64,
    /// The floor of the point at this level.
    pub(super) floor: f64,
    /// The floor of the point at the next level of the plan; at the last
    /// level, `floor`.
    pub(super) next_floor: f64,
}

/// P42 (lead rulings D-1, rounds 2 and 3): whether a point of the wall line
/// ships. The pass exists to stop stock at a wall from standing more than
/// the one-pass limit above the cutter of a lower level. A point ships when
/// either
/// 1. without it the next level meets the strip more than the limit above
///    its cutter here, or
/// 2. the model wall at the point stands more than the limit above the floor
///    of this level: the wall is deeper than one pass, so the level cuts its
///    strip as each level above did (else the margin strip of the last level
///    stays at the foot of the wall).
///
/// Both are the height test of the one-pass rule (`OnePass::over_limit`).
/// A stair tread of a draped slope fails both: its next floor is its own
/// floor, and the surface rises less than the limit inside the disc.
pub(super) fn wall_point_ships(one_pass: &super::one_pass::OnePass, h: WallHeights) -> bool {
    one_pass.over_limit(h.strip_top - h.next_floor) || one_pass.over_limit(h.wall_top - h.floor)
}

/// P42: the wall pass of a level. After the plan of the level, one contour
/// runs along the boundary of each planned region where stock stands in
/// the tool disc on cells where no centre of the level has material to
/// plan: the strip that the margin of the plan leaves at a wall
/// (`smoothed_region`, `PERIMETER_INSET_MARGIN_MM`). So the strip goes down
/// one pass per level, and the one-pass rule of the next level has nothing
/// to hold at that wall.
///
/// - The line: each ring of each region of `regions` (the raw outline of
///   the level grid) moved inside by one planner cell plus the cut
///   tolerance, then simplified at the cut tolerance and sampled at one
///   cell, as the C4 band line. One planner cell is the least distance of
///   a centre from the raw boundary (`grown_outlines`, D19), and the
///   margin that the one-pass rule counts on.
/// - Only at a wall deeper than one pass ([`wall_point_ships`]): the next
///   level would meet the strip over the one-pass limit, or the model wall
///   stands more than the limit above the floor of this level. A wall
///   deeper than one pass gets the pass at each level that reaches it; the
///   tread of a draped slope gets none.
/// - The Z and the standing test of a point: [`wall_read`]. A point that
///   the boundary clip drops, that the one-pass rule blocks
///   (`OnePass::blocks`), or that stands within one planner cell of held
///   stock more than one pass tall (`WallRead::beside_held_stock`), is not
///   cut.
/// - The rounds: the read repeats on the stock after each round. A point is
///   cut again only when the last round lowered its strip by more than one
///   planner cell, so stock taller than one Depth/Pass goes down in passes
///   and the loop ends when no point makes progress.
/// - The runs, the merge of short gaps, the load cap and the emission are
///   those of the C4 contour (`merge_short_gaps`, `run_feed_cap`,
///   `emit_runs`): the planner stamps the path that the emitter ships.
///
/// The runs follow a `WaterlineCleanup` marker: they are the wall cleanup
/// of the level. Off (`ctx.wall_pass` is `None`): nothing.
///
/// P43, `WallRule::FreshStock` (a fresh block on the coarse planner cell):
/// the same pass with another line and other reads. The line is one planner
/// cell less [`fresh_line_shift`] inside the raw outline of each planned
/// region (a region on the perimeter sweep too): nearer to the wall than
/// the plan of any level, and the same line at each level of one wall, so
/// the edge of no later disc passes it. A point ships by
/// [`wall_point_ships`] on the reads of the wall ([`wall_read`]), with no
/// test that stock stands in front of the wall, and once per level. The
/// one-pass guard does not act (`OnePass::on` is false).
pub(super) fn region_wall_pass(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    level: WallLevel,
    regions: &[&crate::polygon::Polygon2],
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
) -> LevelContourStats {
    let Some(wall) = ctx.wall_pass.as_ref() else {
        return LevelContourStats {
            min_factor: 1.0,
            ..LevelContourStats::default()
        };
    };
    #[cfg(test)]
    if WALL_PASS_OFF_FOR_CONTROL.with(std::cell::Cell::get) {
        return LevelContourStats {
            min_factor: 1.0,
            ..LevelContourStats::default()
        };
    }
    contour_rounds(
        ctx,
        material_stock,
        surface_hm,
        level,
        regions,
        ContourRule::Wall(wall.rule),
        segments,
        cursor,
        &mut Vec::new(),
        None,
    )
}

/// P44: what a contour of [`contour_rounds`] is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContourRule {
    /// The wall pass of a planned region ([`region_wall_pass`]): only the
    /// strip at a wall deeper than one pass ([`wall_point_ships`]).
    Wall(WallRule),
    /// P44: the contour of a region that no plan cuts
    /// ([`region_contour_pass`]). A point ships where stock stands in the
    /// disc more than one planner cell above the cutter
    /// (`WallRead::disc_top`), a material cell of the level or not. No test
    /// of the wall height: the contour is the only cut of the region. A run
    /// shorter than
    /// `min_region_cut_length_mm` is not cut (the operator dial of F-038)
    /// and is recorded (`LeftWhy::ShortForecast`).
    Region,
}

/// P44: the distance (mm) of a contour line inside the raw outline of a
/// region of the level grid: one planner cell plus the cut tolerance. One
/// planner cell is the least distance of a centre from the raw boundary
/// (`clearing::grown_outlines`, D19: nearer, the lift reads the rim); the
/// simplification of the line can move it back out by the cut tolerance.
/// The wall pass on remaining stock, the region contour and the F-038
/// forecast read this ONE value.
pub(super) fn contour_line_inset(cell: f64, cut_tolerance: f64) -> f64 {
    cell + cut_tolerance.max(0.0)
}

/// P44: the region contour of a level. A region of the level whose plan
/// ships no cut (the geodesic field has no room in it: a slot region
/// narrower than the starter pocket) gets one pass along its contour line
/// ([`contour_line_inset`]), with the reads, the rounds, the one-pass guard,
/// the load cap and the emission of the wall pass ([`contour_rounds`],
/// [`ContourRule::Region`]). `clear_z_level_slice` calls it after the plan of
/// the level, before the wall pass; such a region is not a planned region,
/// so the wall pass does not trace it again. The second value says, for
/// each region of `regions`, whether a run of its line shipped: such a
/// region is cut, so it is not a left piece (P44b, lead ruling D-15 c).
///
/// On with the guard of the one-pass rule (`OnePass::on`:
/// `OnePassHold::RemainingStock`). On a fresh block a region with no room
/// for the field stays, and the record names it (`LeftWhy::NoField`).
pub(super) fn region_contour_pass(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    level: WallLevel,
    regions: &[&crate::polygon::Polygon2],
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
) -> (LevelContourStats, Vec<bool>) {
    // The region contour is part of the one-pass rule on remaining stock
    // (ruling D-4 of P42: no dial of its own). It reads the stock in the
    // disc on the simulation cell. On a fresh block the planner cell is
    // coarse (a sixth of the tool radius): it cannot read a sliver that the
    // levels above left beside the line, and a contour there cut 10.3 mm
    // deep at 4.166 mm per pass (P44, build 1, the Wanaka back bulk rough).
    let none = || {
        (
            LevelContourStats {
                min_factor: 1.0,
                ..LevelContourStats::default()
            },
            vec![false; regions.len()],
        )
    };
    if !ctx.one_pass.on {
        return none();
    }
    #[cfg(test)]
    if REGION_CONTOUR_OFF_FOR_CONTROL.with(std::cell::Cell::get) {
        return none();
    }
    let mut shipped: Vec<P2> = Vec::new();
    let stats = contour_rounds(
        ctx,
        material_stock,
        surface_hm,
        level,
        regions,
        ContourRule::Region,
        segments,
        cursor,
        &mut shipped,
        None,
    );
    // A region is cut when a run of its line shipped (the line stands
    // inside the outline of its region).
    let cut = regions
        .iter()
        .map(|region| shipped.iter().any(|p| region.contains_point(p)))
        .collect();
    (stats, cut)
}

#[cfg(test)]
thread_local! {
    /// Test control only: no region contour, so that a test can show that
    /// its claim needs it.
    pub(super) static REGION_CONTOUR_OFF_FOR_CONTROL: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// P44: the length (mm) of the contour line of `region` (each ring of its
/// outline moved inside by [`contour_line_inset`]): the least length that
/// the plan of the region cuts. 0: the region has no room for the line.
pub(super) fn contour_line_length(
    region: &crate::polygon::Polygon2,
    cell: f64,
    cut_tolerance: f64,
) -> f64 {
    crate::polygon::offset_polygon(region, contour_line_inset(cell, cut_tolerance))
        .iter()
        .flat_map(polygon_rings)
        .map(|ring| {
            ring.iter()
                .zip(ring.iter().cycle().skip(1))
                .map(|(a, b)| (b.x - a.x).hypot(b.y - a.y))
                .sum::<f64>()
        })
        .sum()
}

#[cfg(test)]
thread_local! {
    /// Test control only: no strip pass (the tree before P45), so that a
    /// test can show that its claim needs the pass.
    pub(super) static STRIP_PASS_OFF_FOR_CONTROL: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// P45: the stock that the strip pass reads at each cell centre.
///
/// The planner stamp lowers a cell that a disc covers in part by the
/// covered fraction of its stock (`ray_blend_above`). So the ray top of a
/// cell at the edge of earlier discs is an average: it stands under the
/// stock at the cell centre when no earlier disc held the centre. A contour
/// on a new line holds such a centre inside its disc and takes that stock
/// whole (P44 build 2, traced: 11 centres of the Wanaka back stood 3.2 to
/// 7.8 mm above their ray top, each 0.000 to 0.162 mm outside the disc edge
/// of the level cuts, and the strip pass cut them 6.61 to 8.50 mm deep at
/// 6 mm per pass).
///
/// The state is an upper bound of the stock at the centre of each cell:
/// - at the start, the bound that the stamp keeps for the whole cell
///   (`DexelGrid::conservative_top`: lowered only by a stamp that covers the
///   cell in full), where the cell holds stock;
/// - then lowered under each cut that the strip pass ships, at each cell
///   whose centre the swept disc holds with room to spare
///   (`TriDexelStock::disc_holds_cell_core`), to the cutter surface over the
///   centre ([`StripTops::scan`], along [`shipped_cut_path`], as the C4 band
///   state is).
///
/// So a centre that the pass cut reads the Z of that cut in the next round,
/// and the rounds go down one Depth/Pass at a time. The ray top alone does
/// not: a centre that the disc edge holds stays part-covered at each round.
pub(super) struct StripTops {
    tops: Vec<f32>,
    rows: usize,
    cols: usize,
    origin_u: f64,
    origin_v: f64,
    cell: f64,
    /// Segments already lowered into the state.
    scanned: usize,
}

impl StripTops {
    /// The state on `stock` before the pass; `segments_now` is the count of
    /// segments that are already in the stock.
    fn new(stock: &TriDexelStock, segments_now: usize) -> Self {
        let grid = &stock.z_grid;
        let tops = (0..grid.rows * grid.cols)
            .map(|i| {
                let (row, col) = (i / grid.cols.max(1), i % grid.cols.max(1));
                match crate::stock::dexel::ray_top(grid.ray(row, col)) {
                    // The bound of the whole cell; never under the ray top.
                    Some(top) => top.max(grid.conservative_top_at(row, col)),
                    // No stock in the cell: the read of an empty ray.
                    #[expect(clippy::cast_possible_truncation, reason = "a stock height in mm")]
                    None => stock_top_z_at(stock, row, col) as f32,
                }
            })
            .collect();
        Self {
            tops,
            rows: grid.rows,
            cols: grid.cols,
            origin_u: grid.origin_u,
            origin_v: grid.origin_v,
            cell: grid.cell_size,
            scanned: segments_now,
        }
    }

    /// The bound at a cell (`-inf` off the grid).
    fn at(&self, row: usize, col: usize) -> f64 {
        if row >= self.rows || col >= self.cols {
            return f64::NEG_INFINITY;
        }
        self.tops
            .get(row * self.cols + col)
            .map_or(f64::NEG_INFINITY, |&t| f64::from(t))
    }

    /// Lower the state under the fed chord `a` to `b`: each cell whose
    /// centre the swept disc holds (`TriDexelStock::disc_holds_cell_core`)
    /// reads at most the cutter surface
    /// over it where the chord passes nearest (the tool stands there with
    /// the centre under it, so the stock at the centre is at or under that
    /// surface).
    fn lower_under(&mut self, lut: &crate::stock::radial_profile::RadialProfileLUT, a: P3, b: P3) {
        let r_sq = lut.radius_sq();
        let r = r_sq.sqrt();
        if self.cell <= 0.0 || self.rows == 0 || self.cols == 0 {
            return;
        }
        let index = |v: f64, origin: f64, n: usize, up: bool| -> usize {
            let i = (v - origin) / self.cell;
            let i = if up { i.ceil() } else { i.floor() };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to 0..n before the cast"
            )]
            {
                i.clamp(0.0, n.saturating_sub(1) as f64) as usize
            }
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        for row in index(a.y.min(b.y) - r, self.origin_v, self.rows, false)
            ..=index(a.y.max(b.y) + r, self.origin_v, self.rows, true)
        {
            let y = self.origin_v + row as f64 * self.cell;
            for col in index(a.x.min(b.x) - r, self.origin_u, self.cols, false)
                ..=index(a.x.max(b.x) + r, self.origin_u, self.cols, true)
            {
                let x = self.origin_u + col as f64 * self.cell;
                let t = if len_sq > 0.0 {
                    (((x - a.x) * dx + (y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (du, dv) = (x - (a.x + t * dx), y - (a.y + t * dy));
                let d_sq = du * du + dv * dv;
                // The disc holds the centre with room to spare: a centre on
                // the disc edge is not cut (the last bit of the arithmetic
                // and the rounding of the G-code decide it).
                if !TriDexelStock::disc_holds_cell_core(du, dv, r_sq, self.cell) {
                    continue;
                }
                let Some(profile) = lut.height_at_dist_sq(d_sq) else {
                    continue;
                };
                let surface = a.z + t * (b.z - a.z) + profile;
                if let Some(top) = self.tops.get_mut(row * self.cols + col) {
                    #[expect(clippy::cast_possible_truncation, reason = "a stock height in mm")]
                    {
                        *top = top.min(surface as f32);
                    }
                }
            }
        }
    }

    /// Lower the state under every cut shipped since the last scan, along
    /// the path that the emitter ships ([`shipped_cut_path`]: the simplified
    /// chord, not the ring points), as [`BandEdge::scan`] does. The strip
    /// pass ships cuts and entries only; an entry stands on the first point
    /// of its cut.
    fn scan(&mut self, segments: &[Adaptive3dSegment], ctx: &ClearZLevelContext<'_>) {
        let from = self.scanned.min(segments.len());
        let drape = ctx.stamp_drape();
        for seg in segments.get(from..).unwrap_or(&[]) {
            match seg {
                Adaptive3dSegment::Cut(path) => {
                    let shipped = shipped_cut_path(
                        path,
                        ctx.cut_tolerance,
                        ctx.min_cutting_radius,
                        &drape,
                        None,
                        &mut crate::tool::body::FedLead::default(),
                    );
                    for pair in shipped.windows(2) {
                        if let [a, b] = pair {
                            self.lower_under(ctx.lut, *a, *b);
                        }
                    }
                }
                Adaptive3dSegment::Link(_)
                | Adaptive3dSegment::Rapid(_)
                | Adaptive3dSegment::RapidWithFloor { .. }
                | Adaptive3dSegment::Marker(_) => {}
            }
        }
        self.scanned = segments.len();
    }
}

/// P45: the strip pass of an op on remaining stock, after its last level.
///
/// The level grid reads the stock at the tool centre only, so a strip
/// beside a wall that is narrower than the tool radius makes no region at
/// any level (the corner and the wall margin that a larger tool left). The
/// wall pass cuts such a strip only along a region that the level plans.
/// This pass cuts the rest:
///
/// - The cells: [`super::standing::standing_cells`] at the bottom level
///   with the strip read (a material cell that the plan left, or a strip
///   cell).
/// - The bands: the connected cells, grown on their open side so that the
///   contour line fits ([`grow_open_side`]).
/// - The cut: the region contour along each band ([`ContourRule::Region`]),
///   in rounds, one Depth/Pass per round (`wall_read`), under the one-pass
///   guard. A band whose line is shorter than `min_region_cut_length_mm`,
///   or that has no room for the line, is not cut and is recorded.
///
/// On with the guard of the one-pass rule (`OnePass::on`:
/// `OnePassHold::RemainingStock`, where the planner cell is the simulation
/// cell): a coarse cell cannot read a strip. No dial of its own (ruling D-4
/// of P42: the wall passes are part of the one-pass rule).
pub(super) fn strip_pass(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
) -> LevelContourStats {
    let none = LevelContourStats {
        min_factor: 1.0,
        ..LevelContourStats::default()
    };
    if !ctx.one_pass.on {
        return none;
    }
    #[cfg(test)]
    if REGION_CONTOUR_OFF_FOR_CONTROL.with(std::cell::Cell::get)
        || STRIP_PASS_OFF_FOR_CONTROL.with(std::cell::Cell::get)
    {
        return none;
    }
    let cell = material_stock.z_grid.cell_size;
    let mut standing =
        super::standing::standing_cells(ctx, material_stock, surface_hm, z_level, true);
    if standing.count == 0 || cell <= 0.0 {
        return none;
    }
    let outlines = |standing: &super::standing::Standing, stock: &TriDexelStock| {
        crate::polygon::detect_containment(
            crate::geometry::contour_extract::marching_squares_bool_grid(
                &standing.padded,
                standing.padded_rows,
                standing.padded_cols,
                stock.z_grid.origin_u - cell,
                stock.z_grid.origin_v - cell,
                cell,
            )
            .into_iter()
            .filter(|ring| ring.len() >= 3)
            .map(crate::polygon::Polygon2::new)
            .collect(),
        )
    };
    // The operator dial, read on each band before it grows (the ONE rule of
    // the F-038 forecast): a band whose cut is under the least cut length is
    // not worth its entry. The cut of a band is its contour line; a band
    // too narrow for the line (a strip of one to three cells) is cut there
    // and back along its own length, which is its outline.
    let mut dropped = 0usize;
    if ctx.min_region_cut_length_mm > 0.0 {
        let grid = &material_stock.z_grid;
        for band in outlines(&standing, material_stock) {
            let line = contour_line_length(&band, cell, ctx.cut_tolerance);
            let length = if line > 0.0 {
                line
            } else {
                polygon_rings(&band)
                    .map(|ring| {
                        ring.iter()
                            .zip(ring.iter().cycle().skip(1))
                            .map(|(a, b)| (b.x - a.x).hypot(b.y - a.y))
                            .sum::<f64>()
                    })
                    .sum()
            };
            if length >= ctx.min_region_cut_length_mm {
                continue;
            }
            dropped += 1;
            // The cells of the band leave the grid.
            let (mut lo, mut hi) = (
                P2::new(f64::INFINITY, f64::INFINITY),
                P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            );
            for p in &band.exterior {
                lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            let index = |v: f64, origin: f64, n: usize| -> usize {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "clamped to 0..n before the cast"
                )]
                {
                    ((v - origin) / cell)
                        .round()
                        .clamp(0.0, n.saturating_sub(1) as f64) as usize
                }
            };
            for row in index(lo.y, grid.origin_v, grid.rows)..=index(hi.y, grid.origin_v, grid.rows)
            {
                for col in
                    index(lo.x, grid.origin_u, grid.cols)..=index(hi.x, grid.origin_u, grid.cols)
                {
                    let (x, y) = grid.cell_to_world(row, col);
                    if band.contains_point(&P2::new(x, y))
                        && let Some(c) = standing
                            .padded
                            .get_mut((row + 1) * standing.padded_cols + (col + 1))
                    {
                        *c = false;
                    }
                }
            }
            ctx.unplanned
                .borrow_mut()
                .push(super::left_stock::Unplanned {
                    z_level,
                    outline: band,
                    why: super::LeftWhy::ShortForecast,
                });
        }
    }
    grow_open_side(ctx, material_stock, surface_hm, z_level, &mut standing);
    let bands = outlines(&standing, material_stock);
    // A grown band with no room for the line is recorded (none is expected:
    // the growth gives each band the room).
    let mut kept: Vec<&crate::polygon::Polygon2> = Vec::new();
    for band in &bands {
        if contour_line_length(band, cell, ctx.cut_tolerance) > 0.0 {
            kept.push(band);
        } else {
            ctx.unplanned
                .borrow_mut()
                .push(super::left_stock::Unplanned {
                    z_level,
                    outline: band.clone(),
                    why: super::LeftWhy::NoLine,
                });
        }
    }
    let mut state = StripTops::new(material_stock, segments.len());
    let stats = contour_rounds(
        ctx,
        material_stock,
        surface_hm,
        WallLevel {
            z_level,
            next_level_z: None,
        },
        &kept,
        ContourRule::Region,
        segments,
        cursor,
        &mut Vec::new(),
        Some(&mut state),
    );
    tracing::debug!(
        z = z_level,
        bands = bands.len(),
        cut = kept.len(),
        under_the_dial = dropped,
        runs = stats.runs,
        peak_load = stats.peak_load,
        "P45 strip pass"
    );
    stats
}

/// P44: grow the cells of `standing` on their open side, so that the
/// contour line ([`contour_line_inset`]) has room in a band that is one cell
/// wide. A band of strip cells lies against the tool-centre limit of a
/// wall. The line must stand one planner cell plus the cut tolerance inside
/// that limit, so the band needs that margin twice plus one cell.
///
/// A neighbour joins when its level floor is at most one planner cell above
/// the floor of the cell that it joins from: the ground beside the band,
/// never the wall (the tool-centre floor steps up at the wall). The grown
/// cells stay inside the boundary of the op.
#[expect(
    clippy::indexing_slicing,
    reason = "each index is inside the padded grid: row < rows + 2 and col < cols + 2"
)]
fn grow_open_side(
    ctx: &ClearZLevelContext<'_>,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    standing: &mut super::standing::Standing,
) {
    let grid = &stock.z_grid;
    let (rows, cols, cs) = (grid.rows, grid.cols, grid.cell_size);
    let pc = standing.padded_cols;
    let floor = |row: usize, col: usize| {
        super::intended_floor::level_floor_z(
            surface_hm.z_or_bbox_floor_at(row, col),
            ctx.stock_to_leave,
            z_level,
        )
    };
    // The cells to grow: the line margin twice plus one cell, less the one
    // cell that the band has.
    let steps = crate::util::steps::steps(2.0 * contour_line_inset(cs, ctx.cut_tolerance), cs)
        .unwrap_or(1)
        .max(1);
    let mut frontier: Vec<(usize, usize)> = (0..rows)
        .flat_map(|row| (0..cols).map(move |col| (row, col)))
        .filter(|&(row, col)| standing.padded[(row + 1) * pc + (col + 1)])
        .collect();
    for _ in 0..steps {
        let mut next = Vec::new();
        for &(row, col) in &frontier {
            let from = floor(row, col);
            for (dr, dc) in [
                (-1isize, -1isize),
                (-1, 0),
                (-1, 1),
                (0, -1),
                (0, 1),
                (1, -1),
                (1, 0),
                (1, 1),
            ] {
                let (Some(r2), Some(c2)) = (
                    row.checked_add_signed(dr).filter(|&v| v < rows),
                    col.checked_add_signed(dc).filter(|&v| v < cols),
                ) else {
                    continue;
                };
                let i = (r2 + 1) * pc + (c2 + 1);
                if standing.padded[i] || floor(r2, c2) > from + cs {
                    continue;
                }
                let (x, y) = grid.cell_to_world(r2, c2);
                if ctx.centre_clip.is_some_and(|clip| !clip.contains(x, y)) {
                    continue;
                }
                standing.padded[i] = true;
                next.push((r2, c2));
            }
        }
        frontier = next;
    }
    if let Some(clip) = ctx.region_clip.as_ref() {
        clip.restrict_regions(&mut standing.padded, standing.padded_rows, pc);
    }
}

/// The rounds of a contour along `regions` ([`region_wall_pass`] has the
/// rules of the wall pass; [`ContourRule`] says what differs for a region
/// contour). `shipped` gets the first point of each run that ships.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn contour_rounds(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    level: WallLevel,
    regions: &[&crate::polygon::Polygon2],
    contour: ContourRule,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
    shipped: &mut Vec<P2>,
    mut strip: Option<&mut StripTops>,
) -> LevelContourStats {
    let WallLevel {
        z_level,
        next_level_z,
    } = level;
    let mut stats = LevelContourStats {
        min_factor: 1.0,
        ..LevelContourStats::default()
    };
    let cell = material_stock.z_grid.cell_size;
    if cell <= 0.0 || regions.is_empty() {
        return stats;
    }
    // The reads of a point: the rule of the wall pass of the op.
    let rule = match contour {
        ContourRule::Wall(rule) => rule,
        // A region contour runs only on remaining stock (`OnePass::on`).
        ContourRule::Region => WallRule::RemainingStock,
    };
    let link_kinematics = ctx
        .wall_pass
        .as_ref()
        .and_then(|wall| wall.link_kinematics.as_ref());
    // The polygons whose rings are the wall line.
    let lines: Vec<crate::polygon::Polygon2> = match (contour, rule) {
        // One planner cell inside the raw outline after the simplification
        // (which can move the line back out by its tolerance): the least
        // distance of a centre from the raw boundary (`grown_outlines`,
        // D19: nearer, the lift reads the rim).
        (ContourRule::Region, _) | (ContourRule::Wall(_), WallRule::RemainingStock) => regions
            .iter()
            .flat_map(|region| {
                crate::polygon::offset_polygon(region, contour_line_inset(cell, ctx.cut_tolerance))
            })
            .collect(),
        // P43: one planner cell less `fresh_line_shift` inside the raw
        // outline: nearer to the wall than each plan line, and the same
        // line at each level of one wall. A region that keeps the
        // perimeter sweep gets the line too: its plan stands a tool radius
        // inside the region, and the level below can open on its grown
        // outline and take that band in one cut (measured on the Wanaka
        // front: 5.73 mm at 4.166 mm per pass). The band is under one tool
        // diameter wide; the load cap of the run holds its feed.
        (ContourRule::Wall(_), WallRule::FreshStock) => {
            let inset = cell - fresh_line_shift(cell, ctx.cut_tolerance);
            regions
                .iter()
                .flat_map(|region| crate::polygon::offset_polygon(region, inset))
                .collect()
        }
    };
    let mut rings: Vec<Vec<WallPoint>> = Vec::new();
    for ring in lines.iter().flat_map(polygon_rings) {
        let ring = simplified_ring(ring, ctx.cut_tolerance);
        let pts = resample_ring(&ring, cell);
        if pts.len() >= 3 {
            rings.push(
                pts.into_iter()
                    .map(|xy| WallPoint {
                        xy,
                        cut_from: f64::INFINITY,
                    })
                    .collect(),
            );
        }
    }
    let mut marked = false;
    loop {
        let mut runs: Vec<Vec<P3>> = Vec::new();
        for ring in &mut rings {
            let mut points = Vec::with_capacity(ring.len());
            let mut standing = Vec::with_capacity(ring.len());
            let mut refused = Vec::with_capacity(ring.len());
            for wp in ring.iter() {
                let p = wp.xy;
                let floor = super::clearing::lift_to_level(ctx, surface_hm, z_level, p).z;
                let inside_clip = ctx.centre_clip.is_none_or(|clip| clip.contains(p.x, p.y));
                let read = if inside_clip {
                    wall_read(
                        ctx,
                        material_stock,
                        surface_hm,
                        (z_level, rule),
                        p,
                        floor,
                        strip.as_deref(),
                    )
                } else {
                    WallRead {
                        z: floor,
                        strip_top: f64::NEG_INFINITY,
                        beside_held_stock: false,
                        wall_top: f64::NEG_INFINITY,
                        disc_top: f64::NEG_INFINITY,
                    }
                };
                let point = P3::new(p.x, p.y, read.z);
                // Progress: the first cut of a point, or a strip that the
                // last round lowered by more than one cell.
                // The floor of the point at the next level (the last level
                // has no next: its own floor).
                let next_floor = next_level_z.map_or(floor, |z| {
                    super::clearing::lift_to_level(ctx, surface_hm, z, p).z
                });
                // P44: the stock that a region contour reads at the point.
                let region_top = read.disc_top;
                let stands = match (contour, rule) {
                    (ContourRule::Region, _) => {
                        region_top > read.z + cell && region_top < wp.cut_from - cell
                    }
                    (ContourRule::Wall(_), WallRule::RemainingStock) => {
                        read.strip_top > read.z + cell
                            && read.strip_top < wp.cut_from - cell
                            && wall_point_ships(
                                &ctx.one_pass,
                                WallHeights {
                                    strip_top: read.strip_top,
                                    wall_top: read.wall_top,
                                    floor,
                                    next_floor,
                                },
                            )
                    }
                    // P43: once per level, by the wall alone. Rule 1 of
                    // `wall_point_ships` is read only where the next level
                    // cuts under this floor at the point: on the tread of a
                    // draped slope the two floors are the same, and the
                    // cells uphill under the disc are the part, not a strip.
                    (ContourRule::Wall(_), WallRule::FreshStock) => {
                        wp.cut_from.is_infinite()
                            && wall_point_ships(
                                &ctx.one_pass,
                                WallHeights {
                                    strip_top: if next_floor < floor - MATERIAL_MARGIN_MM {
                                        read.strip_top
                                    } else {
                                        f64::NEG_INFINITY
                                    },
                                    wall_top: read.wall_top,
                                    floor,
                                    next_floor,
                                },
                            )
                    }
                };
                let blocked = !inside_clip
                    || read.beside_held_stock
                    || (stands
                        && ctx
                            .one_pass
                            .blocks(material_stock, ctx.one_pass_tool(), point));
                standing.push(stands && !blocked);
                refused.push(blocked);
                points.push((
                    point,
                    match contour {
                        ContourRule::Region => region_top,
                        ContourRule::Wall(_) => read.strip_top,
                    },
                ));
            }
            // P44: a point of a region contour takes the highest Z of its
            // own read and of its two neighbours on the ring. The Z of a
            // point holds one Depth/Pass under the stock in ITS disc; the
            // move to a neighbour with a higher Z would pass under the
            // stock of that neighbour at the lower Z (measured: 9.7 mm deep
            // at the disc edge, at 6 mm per pass).
            if contour == ContourRule::Region && points.len() >= 3 {
                let n = points.len();
                let own: Vec<f64> = points.iter().map(|(p, _)| p.z).collect();
                for (i, (point, _)) in points.iter_mut().enumerate() {
                    let before = own.get((i + n - 1) % n).copied().unwrap_or(point.z);
                    let after = own.get((i + 1) % n).copied().unwrap_or(point.z);
                    point.z = point.z.max(before).max(after);
                }
            }
            let path: Vec<P3> = points.iter().map(|(p, _)| *p).collect();
            let mut flags = merge_short_gaps(
                &path,
                &standing,
                ctx.feed_rate,
                ctx.plunge_rate,
                ctx.safe_z,
                link_kinematics,
            );
            // No merge feeds through a point that the clip or the one-pass
            // rule refuses.
            for (flag, &no) in flags.iter_mut().zip(&refused) {
                *flag = *flag && !no;
            }
            for ((wp, &cut), (point, strip_top)) in ring.iter_mut().zip(&standing).zip(&points) {
                if cut {
                    wp.cut_from = match (contour, rule) {
                        (ContourRule::Region, _)
                        | (ContourRule::Wall(_), WallRule::RemainingStock) => *strip_top,
                        (ContourRule::Wall(_), WallRule::FreshStock) => point.z,
                    };
                }
            }
            for run in standing_runs(&path, &flags) {
                // P44: the operator dial of F-038 on a region contour: a run
                // under the least cut length is not worth its entry.
                if contour == ContourRule::Region
                    && crate::geo::polyline_length(&run) < ctx.min_region_cut_length_mm
                {
                    if let Some(outline) = run_box(&run, cell) {
                        ctx.unplanned
                            .borrow_mut()
                            .push(super::left_stock::Unplanned {
                                z_level,
                                outline,
                                why: super::LeftWhy::ShortForecast,
                            });
                    }
                    continue;
                }
                shipped.extend(run.first().map(|p| P2::new(p.x, p.y)));
                runs.push(run);
            }
        }
        if runs.is_empty() {
            break;
        }
        let stock_ref: &TriDexelStock = material_stock;
        let top_at = |row: usize, col: usize| stock_top_z_at(stock_ref, row, col);
        let capped: Vec<(Vec<P3>, Option<f64>)> = runs
            .into_iter()
            .map(|run| {
                let cap = run_feed_cap(ctx, stock_ref, &top_at, &run, &mut stats);
                (run, cap)
            })
            .collect();
        if !marked {
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::WaterlineCleanup,
            ));
            marked = true;
        }
        emit_runs(
            ctx,
            material_stock,
            capped,
            cell,
            segments,
            cursor,
            &mut stats,
            strip.as_deref().map(|state| StripHold {
                state,
                surface_hm,
                z_level,
            }),
        );
        // P45: the strip state follows the cuts of the round.
        if let Some(state) = strip.as_deref_mut() {
            state.scan(segments, ctx);
        }
    }
    tracing::debug!(
        z = z_level,
        runs = stats.runs,
        peak_load = stats.peak_load,
        contour = ?contour,
        "P42 wall pass of the level"
    );
    stats
}

/// The XY box of a run, one planner cell larger on each side, as a polygon:
/// the outline of the record of a run that the contour did not cut.
fn run_box(run: &[P3], cell: f64) -> Option<crate::polygon::Polygon2> {
    let first = run.first()?;
    let (mut lo, mut hi) = (P2::new(first.x, first.y), P2::new(first.x, first.y));
    for p in run {
        lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some(crate::polygon::Polygon2::new(vec![
        P2::new(lo.x - cell, lo.y - cell),
        P2::new(hi.x + cell, lo.y - cell),
        P2::new(hi.x + cell, hi.y + cell),
        P2::new(lo.x - cell, hi.y + cell),
    ]))
}

/// C4b (the F9 fix): whether a wall-cleanup point `p` at `z_level` of the
/// region `mask` may be cut now. It may when no planner cell whose centre
/// is within `reach` of `p` (R + cs/2, the reach of [`BandEdge`] and
/// [`PriorStockTops`]) holds MATERIAL above `z_level` outside the mask:
/// stock above both the level and the cell's own floor (surface plus stock
/// to leave), by `MATERIAL_MARGIN_MM`, the test of
/// [`planner_top_in_reach`]. The planner stock holds the part too, so a
/// wall cell below its own floor is the part, not material. Stock of the
/// region itself is the material the cleanup cuts; another region blocks
/// only while its material stands above the level, so a cut never reaches
/// a region that is not roughed yet.
pub(super) fn keeps_to_region(
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
    mask: &super::clearing::AreaMask,
    p: P3,
    z_level: f64,
    reach: f64,
) -> bool {
    let grid = &stock.z_grid;
    let cs = grid.cell_size;
    if cs <= 0.0 {
        return true;
    }
    let to_index = |v: f64| -> usize {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "floored at 0 and clamped to the grid size below"
        )]
        let i = v.max(0.0) as usize;
        i
    };
    let c0 = to_index(((p.x - reach - grid.origin_u) / cs).floor());
    let c1 = to_index(((p.x + reach - grid.origin_u) / cs).ceil()).min(grid.cols.saturating_sub(1));
    let r0 = to_index(((p.y - reach - grid.origin_v) / cs).floor());
    let r1 = to_index(((p.y + reach - grid.origin_v) / cs).ceil()).min(grid.rows.saturating_sub(1));
    for row in r0..=r1 {
        for col in c0..=c1 {
            let (cx, cy) = grid.cell_to_world(row, col);
            if (cx - p.x).powi(2) + (cy - p.y).powi(2) > reach * reach {
                continue;
            }
            let floor = (surface_hm.z_or_bbox_floor_at(row, col) + stock_to_leave).max(z_level);
            if !mask.owns(row, col)
                && stock_has_material_above(stock, row, col, floor + MATERIAL_MARGIN_MM)
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::expect_used,
    reason = "test code: the indices are inside the ring the test builds, and an expect is how a test reports a failure"
)]
mod tests {
    use super::merge_short_gaps;
    use crate::geo::P3;

    /// A ring of radius 50 mm at Z 0, one point every 0.5 mm.
    fn ring() -> Vec<P3> {
        let n = 628;
        (0..n)
            .map(|i| {
                let t = i as f64 * std::f64::consts::TAU / n as f64;
                P3::new(50.0 * t.cos(), 50.0 * t.sin(), 0.0)
            })
            .collect()
    }

    /// C4 sentry 4: on one ring a short gap between two standing runs is
    /// fed through (the one feed-or-retract rule says a retract does not
    /// save time over 1.5 mm), and the long gap (about 214 mm) stays a
    /// retract. With no machine kinematics no gap is fed.
    #[test]
    fn the_band_pass_merges_short_gaps_c4() {
        let points = ring();
        let standing: Vec<bool> = (0..points.len())
            .map(|i| i < 100 || (103..200).contains(&i))
            .collect();
        let timing = crate::machine::MachineProfile::generic_wood_router().link_kinematics();
        let merged = merge_short_gaps(&points, &standing, 1000.0, 300.0, 20.0, Some(&timing));
        assert!(
            (100..103).all(|i| merged[i]),
            "the 1.5 mm gap must be fed through"
        );
        assert!(
            (200..points.len()).all(|i| !merged[i]),
            "the long gap must stay a retract"
        );
        let unmerged = merge_short_gaps(&points, &standing, 1000.0, 300.0, 20.0, None);
        assert_eq!(unmerged, standing, "no kinematics: no gap is fed");
    }

    /// P45: the strip state lowers a cell only when the swept disc holds its
    /// centre with room to spare (`TriDexelStock::disc_holds_cell_core`). A
    /// centre exactly on the disc edge is not cut: the last bit of the
    /// arithmetic and the rounding of the G-code decide which side it is
    /// on. On the Wanaka back the state counted such a column as cut at Z
    /// 19.2 (line X 230.2, radius 3.0, column X 233.2); the replay did not;
    /// the next round then took 7.3 to 11.9 mm from it at 6 mm per pass.
    /// Stock 10 mm tall on 0.2 mm cells, a chord along Y 5.0 at Z 4, tool
    /// radius 3.0: the row Y 8.0 is on the edge, the row Y 7.8 is inside.
    #[test]
    fn the_strip_state_does_not_count_a_centre_on_the_disc_edge_p45() {
        use crate::dexel_stock::TriDexelStock;
        let stock = TriDexelStock::from_stock(0.0, 0.0, 14.0, 14.0, -1.0, 10.0, 0.2);
        let cutter = crate::tool::FlatEndmill::new(6.0, 25.0);
        let lut = crate::stock::radial_profile::RadialProfileLUT::from_cutter(
            &cutter,
            crate::stock::radial_profile::LUT_SAMPLES,
        );
        let mut state = super::StripTops::new(&stock, 0);
        state.lower_under(&lut, P3::new(5.0, 5.0, 4.0), P3::new(9.0, 5.0, 4.0));
        let col = 35; // X 7.0
        let (edge_row, inside_row) = (40, 39); // Y 8.0 and Y 7.8
        assert!(
            (stock.z_grid.cell_to_world(edge_row, col).1 - 8.0).abs() < 1e-12,
            "the fixture: the row is on the disc edge"
        );
        assert!(
            (state.at(inside_row, col) - 4.0).abs() < 1e-6,
            "the centre 0.2 mm inside the edge reads the cut: {}",
            state.at(inside_row, col)
        );
        assert!(
            (state.at(edge_row, col) - 10.0).abs() < 1e-6,
            "the centre on the disc edge keeps its stock: {}",
            state.at(edge_row, col)
        );
    }

    /// C4b sentry 3: the mask rule. A 20 × 20 mm stock (top 10, 1 mm
    /// cells) on a flat surface at 0; the mask owns the left half.
    #[test]
    fn the_wall_mask_keeps_off_unroughed_regions_c4b() {
        use crate::dexel_stock::TriDexelStock;
        use crate::surface::slope::SurfaceHeightmap;
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -10.0, 10.0, 1.0);
        let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
        let surface = SurfaceHeightmap::from_parts(
            vec![0.0; rows * cols],
            vec![true; rows * cols],
            rows,
            cols,
            stock.z_grid.origin_u,
            stock.z_grid.origin_v,
            1.0,
        );
        let half_x = 10.0;
        let owned: Vec<bool> = (0..rows * cols)
            .map(|i| stock.z_grid.cell_to_world(i / cols, i % cols).0 < half_x)
            .collect();
        let mask = super::super::clearing::AreaMask::from_owned(0, owned, cols)
            .expect("the mask owns cells");
        let (z, reach) = (5.0, 3.5);
        let near_edge = P3::new(half_x - 1.0, 10.0, z);
        let inside = P3::new(3.0, 10.0, z);
        // The other half stands at 10, above the level: rejected.
        assert!(!super::keeps_to_region(
            &stock, &surface, 0.0, &mask, near_edge, z, reach
        ));
        // The own half stands at 10 too: that is the cleanup's material.
        assert!(super::keeps_to_region(
            &stock, &surface, 0.0, &mask, inside, z, reach
        ));
        // The other half cut to the level: accepted.
        for i in 0..rows * cols {
            let (row, col) = (i / cols, i % cols);
            if stock.z_grid.cell_to_world(row, col).0 >= half_x {
                crate::stock::dexel::ray_subtract_above(stock.z_grid.ray_mut(row, col), 5.0);
            }
        }
        assert!(super::keeps_to_region(
            &stock, &surface, 0.0, &mask, near_edge, z, reach
        ));
    }
}
