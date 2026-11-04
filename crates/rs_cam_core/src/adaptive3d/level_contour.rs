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
                        shipped_cut_path(path, ctx.cut_tolerance, ctx.min_cutting_radius, &drape, None);
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
    let ceiling = crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        crate::adaptive::pass_engagement_limit(ctx.stepover, ctx.tool_radius),
    );
    let load_cutter = StepLoadCutter {
        lut: ctx.lut,
        chip: ctx.cutter.geometry_hint(),
        diameter: 2.0 * ctx.tool_radius,
        depth_per_pass: ctx.depth_per_pass,
    };
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
        .map(|run| {
            let mut peak = 0.0_f64;
            let mut factor = 1.0_f64;
            for w in run.windows(2) {
                let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
                    continue;
                };
                let load = step_load_on(
                    material_stock,
                    &top_at,
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
        })
        .collect();
    drop(guard);
    tracing::debug!(
        z = z_level,
        runs = runs.len(),
        "C4 contour each level: runs"
    );
    let drape = ctx.stamp_drape();
    let mut left: Vec<(Vec<P3>, Option<f64>)> = runs.into_iter().zip(factors).collect();
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
                Adaptive3dRuntimeEvent::FeedCapStart { factor },
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
    if let Some(band) = ctx.band_edge.borrow_mut().as_mut() {
        band.scan(segments, ctx);
    }
    stats
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
