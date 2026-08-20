//! Shared greedy nearest-neighbour selection with deletion (PERF_REVIEW G5).
//!
//! Five generators hand-rolled the same loop: keep a `visited` flag per
//! candidate, and at every step scan **all** candidates for the nearest
//! unvisited one. That is Θ(n²) distance evaluations for a tour, uncapped —
//! `tsp`'s O(N³) 2-opt refinement is capped at `MAX_2OPT_SEGMENTS = 500`, its
//! quadratic seed was not, and `surface_link` documents 12,780 fragments on
//! the wanaka workload (≈163 M evaluations, twice).
//!
//! The five are `tsp::run_tsp`'s seed, `surface_link`'s fragment reorder,
//! `pencil::order_paths_nearest`,
//! `adaptive3d::clearing::nearest_neighbor_order` and
//! `ramp_finish::match_contours`. The last is **not** a tour — it is a greedy
//! bipartite match whose nearest candidate may be rejected by a downstream
//! test and must stay available for the next query — which is why
//! [`NearestPicker::nearest`] and [`NearestPicker::remove`] are separate
//! operations rather than one "take the nearest" call.
//!
//! `unified_finish::route_greedy` looks like a sixth but is not one: its
//! per-candidate cost is a mesh-drape link *time* from `compute_cycle_time`,
//! not monotone in XY distance, so no geometric structure can order it
//! without changing the route. See `DELTA_gen_w3b.md`.
//!
//! # The contract this type is built around
//!
//! A different tour is a different toolpath. The scan being replaced picks
//! **the first strict minimum in candidate-index order**, i.e. the
//! lexicographic minimum of `(distance, index)` — so wherever two candidates
//! are exactly equidistant the *lower index* wins. On machined geometry
//! (rasters, lattices, regular grids) exact ties are the normal case, not the
//! pathological one, and a spatial structure that visits candidates in
//! bucket order would silently pick the other one and diverge for the rest of
//! the tour.
//!
//! [`NearestPicker::nearest`] therefore returns the lexicographic minimum of
//! `(value, owner)` over the live candidates, **exactly** — the grid is a
//! pruning device only, never a tie-break. Three properties make that hold:
//!
//! 1. The early-out is **strict** (`lower_bound > best`). Stopping on `>=`
//!    would be sound for distance alone but not for the tie-break: an
//!    unscanned candidate at exactly the incumbent distance with a lower
//!    index still wins, and equal-distance candidates genuinely can sit in
//!    different rings. Under `>` no unscanned candidate can even tie.
//! 2. The per-candidate value is computed with the same expression the
//!    hand-rolled loops used (`dx * dx + dy * dy`, optionally `sqrt`),
//!    selected by [`Metric`]. `sqrt` matters: it is monotone but not
//!    injective, so two different squared distances can compare *equal*
//!    after it — and then the index tie-break fires where a squared-distance
//!    comparison would have picked a strict winner.
//! 3. NaN is ordered after every real value and never prunes, reproducing
//!    `d < best` being false for NaN. A caller that only accepts values below
//!    its own sentinel (`f64::INFINITY`, `f64::MAX`) keeps its own fallback
//!    behaviour by testing the returned value, which is why `nearest` reports
//!    the value rather than applying a threshold itself.
//!
//! `nn_order::tests` pins all of this against verbatim copies of all five
//! replaced loops, asserting the **identical permutation** rather than an
//! equal tour length.
//!
//! # Cost
//!
//! Build is O(n) (counting sort into CSR cells — no `Vec<Vec<_>>`). A query
//! scans expanding Chebyshev rings from the query cell and stops at the first
//! ring whose geometric lower bound beats the incumbent, which is O(1)
//! expected on a roughly uniform cloud. Deletion marks the owner dead; the
//! grid is rebuilt from the survivors whenever the live count halves, so the
//! endgame cannot degenerate into scanning a mostly-dead grid and the total
//! rebuild cost telescopes to O(n).

/// Which distance expression the caller's original loop compared.
///
/// Not cosmetic: see property 2 in the module docs — `sqrt` can collapse two
/// distinct squared distances onto one value and hand the decision to the
/// index tie-break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Metric {
    /// `(dx * dx + dy * dy).sqrt()` — `tsp::xy_distance`, `surface_link::xy_gap`.
    Euclid,
    /// `dx * dx + dy * dy` — `pencil`, `adaptive3d::clearing`.
    EuclidSq,
}

impl Metric {
    #[inline]
    fn value(self, dx: f64, dy: f64) -> f64 {
        let sq = dx * dx + dy * dy;
        match self {
            Self::Euclid => sq.sqrt(),
            Self::EuclidSq => sq,
        }
    }

    /// Carry a true-Euclidean lower bound through the metric. Both variants
    /// are monotone non-decreasing in Euclidean distance, so a bound stays a
    /// bound.
    #[inline]
    fn bound(self, euclid: f64) -> f64 {
        match self {
            Self::Euclid => euclid,
            Self::EuclidSq => euclid * euclid,
        }
    }
}

/// Below this many live candidates the grid is not worth its own indirection
/// and the picker scans linearly — which is also what keeps the tail of a
/// tour cheap.
const LINEAR_SCAN_MAX: usize = 32;

/// Upper bound on grid cells, so a pathological aspect ratio or a single
/// far-flung outlier cannot allocate an enormous index. Exceeding it coarsens
/// the cell size (still correct, just less selective).
const MAX_CELLS: usize = 1 << 22;

/// Shrink applied to the ring lower bound, as a fraction of the cell size.
/// The bound is exact in real arithmetic; this covers the handful of ULPs
/// that `(q - x0) / cs` and `x0 + k * cs` disagree by. Four orders of
/// magnitude larger than the error it absorbs, and it can only ever cause an
/// extra ring to be scanned.
const BOUND_SLACK_FRACTION: f64 = 1e-9;

/// A candidate point belonging to an owner.
struct Pt {
    x: f64,
    y: f64,
    owner: u32,
}

/// Greedy nearest-neighbour selection over 2D candidate points, with
/// per-owner deletion.
///
/// An owner may contribute more than one point (`pencil` measures a chain by
/// both of its endpoints); its value is the minimum over its own points,
/// matching `d_start.min(d_end)`, and deleting the owner removes all of them.
pub(crate) struct NearestPicker {
    metric: Metric,
    pts: Vec<Pt>,
    /// Per-owner liveness. Indexed by owner id.
    alive: Vec<bool>,
    live_owners: usize,
    /// Point slots participating in the current grid (all live at build time).
    /// Also the linear-scan population.
    members: Vec<u32>,
    /// Slots whose coordinates are not both finite — they cannot be placed in
    /// a grid, so they are scanned unconditionally. Normally empty.
    strays: Vec<u32>,
    /// Live-owner count at the last (re)build, for the halving trigger.
    built_live: usize,
    // ── grid ────────────────────────────────────────────────────────────
    x0: f64,
    y0: f64,
    cs: f64,
    nx: usize,
    ny: usize,
    /// CSR: `cell_start[c]..cell_start[c + 1]` indexes `cell_items`.
    cell_start: Vec<u32>,
    cell_items: Vec<u32>,
}

impl NearestPicker {
    /// Empty picker for `owners` owners. Feed it with [`Self::push`], then
    /// [`Self::build`].
    pub(crate) fn new(metric: Metric, owners: usize) -> Self {
        Self {
            metric,
            pts: Vec::new(),
            alive: vec![true; owners],
            live_owners: owners,
            members: Vec::new(),
            strays: Vec::new(),
            built_live: owners,
            x0: 0.0,
            y0: 0.0,
            cs: 1.0,
            nx: 0,
            ny: 0,
            cell_start: Vec::new(),
            cell_items: Vec::new(),
        }
    }

    /// Register one candidate point for `owner`. Owners that register no
    /// point are never returned — matching the `continue` the hand-rolled
    /// loops used for empty chains.
    pub(crate) fn push(&mut self, owner: usize, x: f64, y: f64) {
        self.pts.push(Pt {
            x,
            y,
            owner: owner as u32,
        });
    }

    /// Index the registered points. Call once, after all [`Self::push`]es.
    pub(crate) fn build(&mut self) {
        self.rebuild();
    }

    /// Drop `owner` from consideration. Idempotent — the replaced loops can
    /// re-select an already-visited index in their degenerate "nothing was
    /// closer than the sentinel" fallback, and that must stay a no-op.
    pub(crate) fn remove(&mut self, owner: usize) {
        let Some(slot) = self.alive.get_mut(owner) else {
            return;
        };
        if !*slot {
            return;
        }
        *slot = false;
        self.live_owners -= 1;
        if self.live_owners * 2 <= self.built_live && self.live_owners > LINEAR_SCAN_MAX {
            self.rebuild();
        }
    }

    /// The live candidate minimising `(value, owner)` lexicographically, with
    /// NaN values ordered last. `None` only when no live owner has a point.
    ///
    /// The value is returned rather than thresholded: each call site keeps its
    /// own acceptance sentinel (`f64::INFINITY` in `tsp` / `surface_link` /
    /// `clearing`, `f64::MAX` in `pencil`) and its own fallback when nothing
    /// clears it.
    pub(crate) fn nearest(&self, qx: f64, qy: f64) -> Option<(usize, f64)> {
        let mut best: Option<(f64, u32)> = None;
        for &s in &self.strays {
            self.consider(s, qx, qy, &mut best);
        }

        if self.members.len() <= LINEAR_SCAN_MAX
            || self.nx == 0
            || self.ny == 0
            || !qx.is_finite()
            || !qy.is_finite()
        {
            for &s in &self.members {
                self.consider(s, qx, qy, &mut best);
            }
            return best.map(|(v, o)| (o as usize, v));
        }

        // SAFETY (clamp): `nx`/`ny` are >= 1 here, and the clamp only matters
        // for a query outside the point cloud — for which the ring bound
        // below already accounts by omitting the sides that have no cells.
        let cx = (((qx - self.x0) / self.cs).floor()).clamp(0.0, (self.nx - 1) as f64) as usize;
        let cy = (((qy - self.y0) / self.cs).floor()).clamp(0.0, (self.ny - 1) as f64) as usize;

        let max_r = self.nx + self.ny;
        for r in 0..=max_r {
            self.scan_ring(cx, cy, r, qx, qy, &mut best);

            // Lower bound on the distance from `q` to anything NOT yet
            // scanned. Only sides that actually have cells beyond them count;
            // omitting the others is what keeps an out-of-cloud query from
            // degenerating into a full scan.
            let lo_x = self.x0 + (cx as f64 - r as f64) * self.cs;
            let hi_x = self.x0 + (cx as f64 + r as f64 + 1.0) * self.cs;
            let lo_y = self.y0 + (cy as f64 - r as f64) * self.cs;
            let hi_y = self.y0 + (cy as f64 + r as f64 + 1.0) * self.cs;
            let mut safe = f64::INFINITY;
            if cx > r {
                // i.e. column `cx - r` is not the leftmost: cells exist beyond it.
                safe = safe.min(qx - lo_x);
            }
            if cx + r + 1 < self.nx {
                safe = safe.min(hi_x - qx);
            }
            if cy > r {
                safe = safe.min(qy - lo_y);
            }
            if cy + r + 1 < self.ny {
                safe = safe.min(hi_y - qy);
            }
            if safe.is_infinite() {
                // The scanned square covers the whole grid.
                break;
            }
            let safe = (safe - self.cs * BOUND_SLACK_FRACTION).max(0.0);
            let lb = self.metric.bound(safe);
            // Strict: see property 1 in the module docs.
            if let Some((bv, _)) = best
                && !bv.is_nan()
                && lb > bv
            {
                break;
            }
        }

        best.map(|(v, o)| (o as usize, v))
    }

    // ── internals ───────────────────────────────────────────────────────

    #[inline]
    fn consider(&self, slot: u32, qx: f64, qy: f64, best: &mut Option<(f64, u32)>) {
        let Some(p) = self.pts.get(slot as usize) else {
            return;
        };
        if !self.alive.get(p.owner as usize).copied().unwrap_or(false) {
            return;
        }
        let v = self.metric.value(p.x - qx, p.y - qy);
        if beats(v, p.owner, *best) {
            *best = Some((v, p.owner));
        }
    }

    fn scan_ring(
        &self,
        cx: usize,
        cy: usize,
        r: usize,
        qx: f64,
        qy: f64,
        best: &mut Option<(f64, u32)>,
    ) {
        let (cx, cy, r) = (cx as isize, cy as isize, r as isize);
        let (nx, ny) = (self.nx as isize, self.ny as isize);
        for j in (cy - r).max(0)..=(cy + r).min(ny - 1) {
            let edge_row = j == cy - r || j == cy + r;
            let mut i = (cx - r).max(0);
            let i_hi = (cx + r).min(nx - 1);
            while i <= i_hi {
                self.scan_cell(i as usize, j as usize, qx, qy, best);
                if edge_row || r == 0 {
                    i += 1;
                } else if i < cx + r {
                    // Interior row: only the two side columns belong to the
                    // ring. Jump straight to the right one.
                    i = cx + r;
                } else {
                    break;
                }
            }
        }
    }

    #[inline]
    fn scan_cell(&self, i: usize, j: usize, qx: f64, qy: f64, best: &mut Option<(f64, u32)>) {
        let c = j * self.nx + i;
        let (Some(&a), Some(&b)) = (self.cell_start.get(c), self.cell_start.get(c + 1)) else {
            return;
        };
        let Some(items) = self.cell_items.get(a as usize..b as usize) else {
            return;
        };
        for &slot in items {
            self.consider(slot, qx, qy, best);
        }
    }

    fn rebuild(&mut self) {
        self.members.clear();
        self.strays.clear();
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (slot, p) in self.pts.iter().enumerate() {
            if !self.alive.get(p.owner as usize).copied().unwrap_or(false) {
                continue;
            }
            if !p.x.is_finite() || !p.y.is_finite() {
                self.strays.push(slot as u32);
                continue;
            }
            self.members.push(slot as u32);
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        self.built_live = self.live_owners;

        let n = self.members.len();
        if n <= LINEAR_SCAN_MAX {
            self.nx = 0;
            self.ny = 0;
            self.cell_start.clear();
            self.cell_items.clear();
            return;
        }

        let w = (max_x - min_x).max(0.0);
        let h = (max_y - min_y).max(0.0);
        let mut cs = if w > 0.0 && h > 0.0 {
            (w * h / n as f64).sqrt()
        } else {
            (w.max(h) / n as f64).max(f64::MIN_POSITIVE)
        };
        if !cs.is_finite() || cs <= 0.0 {
            cs = 1.0;
        }
        let mut nx = ((w / cs).ceil() as usize).saturating_add(1).max(1);
        let mut ny = ((h / cs).ceil() as usize).saturating_add(1).max(1);
        if nx.saturating_mul(ny) > MAX_CELLS {
            // Coarsen until the index fits. Correctness is unaffected — a
            // bigger cell just prunes less.
            let scale = ((nx as f64 * ny as f64) / MAX_CELLS as f64).sqrt();
            cs *= scale.max(1.0);
            nx = ((w / cs).ceil() as usize).saturating_add(1).max(1);
            ny = ((h / cs).ceil() as usize).saturating_add(1).max(1);
        }
        self.x0 = min_x;
        self.y0 = min_y;
        self.cs = cs;
        self.nx = nx;
        self.ny = ny;

        // Counting sort into CSR — no per-cell Vec.
        let cells = nx * ny;
        self.cell_start.clear();
        self.cell_start.resize(cells + 1, 0);
        let cell_of = |p: &Pt| -> usize {
            let i = (((p.x - min_x) / cs).floor()).clamp(0.0, (nx - 1) as f64) as usize;
            let j = (((p.y - min_y) / cs).floor()).clamp(0.0, (ny - 1) as f64) as usize;
            j * nx + i
        };
        for &slot in &self.members {
            let Some(p) = self.pts.get(slot as usize) else {
                continue;
            };
            if let Some(c) = self.cell_start.get_mut(cell_of(p) + 1) {
                *c += 1;
            }
        }
        for c in 1..=cells {
            let prev = self.cell_start.get(c - 1).copied().unwrap_or(0);
            if let Some(v) = self.cell_start.get_mut(c) {
                *v += prev;
            }
        }
        self.cell_items.clear();
        self.cell_items.resize(self.members.len(), 0);
        let mut cursor = self.cell_start.clone();
        for &slot in &self.members {
            let Some(p) = self.pts.get(slot as usize) else {
                continue;
            };
            let c = cell_of(p);
            let Some(at) = cursor.get_mut(c) else {
                continue;
            };
            let pos = *at as usize;
            *at += 1;
            if let Some(dst) = self.cell_items.get_mut(pos) {
                *dst = slot;
            }
        }
    }
}

/// `(v, owner)` beats `best` lexicographically, with NaN ordered after every
/// real value. Reproduces `if d < best_dist` scanning in ascending index
/// order — which keeps the LOWEST index on an exact tie.
#[inline]
fn beats(v: f64, owner: u32, best: Option<(f64, u32)>) -> bool {
    let Some((bv, bo)) = best else { return true };
    if v < bv {
        true
    } else if v > bv {
        false
    } else if v == bv {
        owner < bo
    } else if bv.is_nan() {
        // At least one NaN; a real value always wins, NaN-vs-NaN by index.
        !v.is_nan() || owner < bo
    } else {
        false
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    // ── verbatim copies of the five replaced loops ──────────────────────
    //
    // These are the specification. They are copied from the pre-G5 bodies of
    // `tsp::run_tsp`, `surface_link`'s reorder block,
    // `pencil::order_paths_nearest`,
    // `adaptive3d::clearing::nearest_neighbor_order` and
    // `ramp_finish::match_contours`, with only the
    // surrounding plumbing (segment/fragment/path types) reduced to raw
    // coordinates. Because they live here rather than being re-derived from
    // the shipped code, they cannot rot into agreement.

    /// Pre-G5 `tsp::run_tsp` seed: query = previous segment's END, candidates
    /// = every segment's START, metric = `sqrt`, sentinel `f64::INFINITY`,
    /// fallback index 0.
    fn ref_tsp(starts: &[(f64, f64)], ends: &[(f64, f64)]) -> Vec<usize> {
        let n = starts.len();
        let mut visited = vec![false; n];
        let mut order = Vec::with_capacity(n);
        if n == 0 {
            return order;
        }
        order.push(0);
        visited[0] = true;
        for _ in 1..n {
            let current = order[order.len() - 1];
            let (ex, ey) = ends[current];
            let mut best_idx = 0;
            let mut best_dist = f64::INFINITY;
            for j in 0..n {
                if visited[j] {
                    continue;
                }
                let dx = ex - starts[j].0;
                let dy = ey - starts[j].1;
                let d = (dx * dx + dy * dy).sqrt();
                if d < best_dist {
                    best_dist = d;
                    best_idx = j;
                }
            }
            visited[best_idx] = true;
            order.push(best_idx);
        }
        order
    }

    /// Pre-G6/G5 `surface_link` reorder: query = previous fragment's EXIT,
    /// candidates = every fragment's ENTRY, metric = `sqrt`, sentinel
    /// `f64::INFINITY`, fallback `usize::MAX` → break.
    fn ref_surface_link(entries: &[(f64, f64)], exits: &[(f64, f64)]) -> Vec<usize> {
        let n = entries.len();
        let mut visited = vec![false; n];
        let mut order = Vec::with_capacity(n);
        if n == 0 {
            return order;
        }
        order.push(0);
        visited[0] = true;
        let mut here = exits[0];
        for _ in 1..n {
            let mut best = usize::MAX;
            let mut best_d = f64::INFINITY;
            for (j, e) in entries.iter().enumerate() {
                if visited[j] {
                    continue;
                }
                let dx = e.0 - here.0;
                let dy = e.1 - here.1;
                let d = (dx * dx + dy * dy).sqrt();
                if d < best_d {
                    best_d = d;
                    best = j;
                }
            }
            if best == usize::MAX {
                break;
            }
            visited[best] = true;
            here = exits[best];
            order.push(best);
        }
        order
    }

    /// Pre-G5 `pencil::order_paths_nearest`: two candidate points per path,
    /// value = `min(d_start, d_end)` SQUARED, sentinel `f64::MAX`, fallback
    /// index 0; empty paths are skipped entirely. Returns the visit order and
    /// the per-step reverse decision.
    fn ref_pencil(paths: &[Vec<(f64, f64)>]) -> (Vec<usize>, Vec<bool>) {
        let n = paths.len();
        let mut order = Vec::with_capacity(n);
        let mut reversed = vec![false; n];
        if n <= 1 {
            return ((0..n).collect(), reversed);
        }
        let mut used = vec![false; n];
        // Local mutable copy so the reverse decision sees prior reversals,
        // exactly as the shipped in-place version does.
        let mut pts: Vec<Vec<(f64, f64)>> = paths.to_vec();
        order.push(0);
        used[0] = true;
        for _ in 1..n {
            let last_path = &pts[*order.last().unwrap()];
            let Some(&last_pt) = last_path.last() else {
                continue;
            };
            let mut best_idx = 0;
            let mut best_dist = f64::MAX;
            for (i, path) in pts.iter().enumerate() {
                if used[i] || path.is_empty() {
                    continue;
                }
                let p = path[0];
                let d_start = {
                    let dx = p.0 - last_pt.0;
                    let dy = p.1 - last_pt.1;
                    dx * dx + dy * dy
                };
                let q = *path.last().unwrap();
                let d_end = {
                    let dx = q.0 - last_pt.0;
                    let dy = q.1 - last_pt.1;
                    dx * dx + dy * dy
                };
                let d = d_start.min(d_end);
                if d < best_dist {
                    best_dist = d;
                    best_idx = i;
                }
            }
            if !pts[best_idx].is_empty() {
                let s = pts[best_idx][0];
                let e = *pts[best_idx].last().unwrap();
                let d_start = (s.0 - last_pt.0).powi(2) + (s.1 - last_pt.1).powi(2);
                let d_end = (e.0 - last_pt.0).powi(2) + (e.1 - last_pt.1).powi(2);
                if d_end < d_start {
                    pts[best_idx].reverse();
                    reversed[best_idx] = !reversed[best_idx];
                }
            }
            used[best_idx] = true;
            order.push(best_idx);
        }
        (order, reversed)
    }

    /// Pre-G5 `adaptive3d::clearing::nearest_neighbor_order`: free start
    /// point, squared metric, sentinel `f64::INFINITY`, and no fallback at
    /// all — a step that finds nothing simply emits nothing.
    fn ref_clearing(anchors: &[(f64, f64)], start: (f64, f64)) -> Vec<usize> {
        let mut visited = vec![false; anchors.len()];
        let mut order: Vec<usize> = Vec::with_capacity(anchors.len());
        let mut cur = start;
        for _ in 0..anchors.len() {
            let mut best: Option<usize> = None;
            let mut best_d = f64::INFINITY;
            for (i, a) in anchors.iter().enumerate() {
                if visited[i] {
                    continue;
                }
                let d = (a.0 - cur.0).powi(2) + (a.1 - cur.1).powi(2);
                if d < best_d {
                    best_d = d;
                    best = Some(i);
                }
            }
            if let Some(i) = best {
                visited[i] = true;
                order.push(i);
                cur = anchors[i];
            }
        }
        order
    }

    /// Pre-G5 `ramp_finish::match_contours`: NOT a tour — a greedy bipartite
    /// match in which the nearest candidate can be REJECTED by the extent
    /// test and stay available for the next query. `extent[li]` stands in for
    /// `lower[li].total_length`.
    fn ref_ramp_match(
        upper: &[(f64, f64)],
        upper_len: &[f64],
        lower: &[(f64, f64)],
        lower_len: &[f64],
    ) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        let mut lower_used = vec![false; lower.len()];
        for (ui, uc) in upper.iter().enumerate() {
            let mut best_dist = f64::INFINITY;
            let mut best_li = None;
            for (li, lc) in lower.iter().enumerate() {
                if lower_used[li] {
                    continue;
                }
                let dx = uc.0 - lc.0;
                let dy = uc.1 - lc.1;
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_li = Some(li);
                }
            }
            if let Some(li) = best_li {
                let max_extent = upper_len[ui].max(lower_len[li]) * 0.5;
                if best_dist.sqrt() < max_extent {
                    matches.push((ui, li));
                    lower_used[li] = true;
                }
            }
        }
        matches
    }

    // ── the picker driving each of the five shapes ──────────────────────

    fn picked_tsp(starts: &[(f64, f64)], ends: &[(f64, f64)]) -> Vec<usize> {
        let n = starts.len();
        let mut order = Vec::with_capacity(n);
        if n == 0 {
            return order;
        }
        let mut picker = NearestPicker::new(Metric::Euclid, n);
        for (i, s) in starts.iter().enumerate() {
            picker.push(i, s.0, s.1);
        }
        picker.build();
        picker.remove(0);
        order.push(0);
        for _ in 1..n {
            let (ex, ey) = ends[order[order.len() - 1]];
            let best_idx = match picker.nearest(ex, ey) {
                Some((i, d)) if d < f64::INFINITY => i,
                _ => 0,
            };
            picker.remove(best_idx);
            order.push(best_idx);
        }
        order
    }

    fn picked_surface_link(entries: &[(f64, f64)], exits: &[(f64, f64)]) -> Vec<usize> {
        let n = entries.len();
        let mut order = Vec::with_capacity(n);
        if n == 0 {
            return order;
        }
        let mut picker = NearestPicker::new(Metric::Euclid, n);
        for (i, e) in entries.iter().enumerate() {
            picker.push(i, e.0, e.1);
        }
        picker.build();
        picker.remove(0);
        order.push(0);
        let mut here = exits[0];
        for _ in 1..n {
            let best = match picker.nearest(here.0, here.1) {
                Some((i, d)) if d < f64::INFINITY => i,
                _ => usize::MAX,
            };
            if best == usize::MAX {
                break;
            }
            picker.remove(best);
            here = exits[best];
            order.push(best);
        }
        order
    }

    fn picked_pencil(paths: &[Vec<(f64, f64)>]) -> (Vec<usize>, Vec<bool>) {
        let n = paths.len();
        let mut reversed = vec![false; n];
        if n <= 1 {
            return ((0..n).collect(), reversed);
        }
        let mut pts: Vec<Vec<(f64, f64)>> = paths.to_vec();
        let mut picker = NearestPicker::new(Metric::EuclidSq, n);
        for (i, p) in pts.iter().enumerate() {
            if p.is_empty() {
                continue;
            }
            picker.push(i, p[0].0, p[0].1);
            let last = *p.last().unwrap();
            picker.push(i, last.0, last.1);
        }
        picker.build();
        picker.remove(0);
        let mut order = Vec::with_capacity(n);
        order.push(0);
        for _ in 1..n {
            let last_path = &pts[*order.last().unwrap()];
            let Some(&last_pt) = last_path.last() else {
                continue;
            };
            let best_idx = match picker.nearest(last_pt.0, last_pt.1) {
                Some((i, d)) if d < f64::MAX => i,
                _ => 0,
            };
            if !pts[best_idx].is_empty() {
                let s = pts[best_idx][0];
                let e = *pts[best_idx].last().unwrap();
                let d_start = (s.0 - last_pt.0).powi(2) + (s.1 - last_pt.1).powi(2);
                let d_end = (e.0 - last_pt.0).powi(2) + (e.1 - last_pt.1).powi(2);
                if d_end < d_start {
                    pts[best_idx].reverse();
                    reversed[best_idx] = !reversed[best_idx];
                }
            }
            picker.remove(best_idx);
            order.push(best_idx);
        }
        (order, reversed)
    }

    fn picked_clearing(anchors: &[(f64, f64)], start: (f64, f64)) -> Vec<usize> {
        let mut order: Vec<usize> = Vec::with_capacity(anchors.len());
        let mut picker = NearestPicker::new(Metric::EuclidSq, anchors.len());
        for (i, a) in anchors.iter().enumerate() {
            picker.push(i, a.0, a.1);
        }
        picker.build();
        let mut cur = start;
        for _ in 0..anchors.len() {
            match picker.nearest(cur.0, cur.1) {
                Some((i, d)) if d < f64::INFINITY => {
                    picker.remove(i);
                    order.push(i);
                    cur = anchors[i];
                }
                _ => {}
            }
        }
        order
    }

    fn picked_ramp_match(
        upper: &[(f64, f64)],
        upper_len: &[f64],
        lower: &[(f64, f64)],
        lower_len: &[f64],
    ) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        let mut picker = NearestPicker::new(Metric::EuclidSq, lower.len());
        for (li, lc) in lower.iter().enumerate() {
            picker.push(li, lc.0, lc.1);
        }
        picker.build();
        for (ui, uc) in upper.iter().enumerate() {
            let (li, best_dist) = match picker.nearest(uc.0, uc.1) {
                Some((li, d)) if d < f64::INFINITY => (li, d),
                _ => continue,
            };
            let max_extent = upper_len[ui].max(lower_len[li]) * 0.5;
            if best_dist.sqrt() < max_extent {
                matches.push((ui, li));
                picker.remove(li);
            }
        }
        matches
    }

    // ── adversarial fixtures ────────────────────────────────────────────

    /// Deterministic, no RNG dependency: a xorshift so the "random" cases are
    /// reproducible across machines and runs.
    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// A regular lattice — the case the module docs call out: exact ties
    /// everywhere, because every neighbour is at the same distance.
    fn lattice(nx: usize, ny: usize, pitch: f64) -> Vec<(f64, f64)> {
        (0..ny)
            .flat_map(|j| (0..nx).map(move |i| (i as f64 * pitch, j as f64 * pitch)))
            .collect()
    }

    fn cases() -> Vec<(&'static str, Vec<(f64, f64)>)> {
        let mut v: Vec<(&'static str, Vec<(f64, f64)>)> = vec![
            ("empty", vec![]),
            ("single", vec![(3.0, -4.0)]),
            ("two_identical", vec![(1.0, 1.0), (1.0, 1.0)]),
            (
                "coincident_cluster",
                vec![(2.0, 2.0); 40].into_iter().collect(),
            ),
            (
                "collinear",
                (0..80).map(|i| (i as f64 * 0.5, 7.0)).collect(),
            ),
            (
                "collinear_reversed",
                (0..80).map(|i| (-(i as f64) * 0.5, 7.0)).collect(),
            ),
            ("lattice_9x9", lattice(9, 9, 1.0)),
            ("lattice_20x20", lattice(20, 20, 2.5)),
            ("lattice_1x400", lattice(1, 400, 0.25)),
            (
                "signed_zero",
                vec![
                    (0.0, 0.0),
                    (-0.0, -0.0),
                    (0.0, -0.0),
                    (-0.0, 0.0),
                    (1.0, 0.0),
                ],
            ),
            (
                "two_rings_equidistant",
                (0..64)
                    .map(|i| {
                        let t = std::f64::consts::TAU * i as f64 / 64.0;
                        (10.0 * t.cos(), 10.0 * t.sin())
                    })
                    .collect(),
            ),
            ("far_outlier", {
                let mut p = lattice(12, 12, 1.0);
                p.push((1.0e7, -1.0e7));
                p
            }),
            (
                "nan_and_inf",
                vec![
                    (0.0, 0.0),
                    (f64::NAN, 1.0),
                    (2.0, f64::NAN),
                    (f64::INFINITY, 0.0),
                    (1.0, 1.0),
                    (-3.0, 2.0),
                ],
            ),
        ];
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        // A dense cloud on a coarse quantisation grid: random enough to
        // exercise the ring search, quantised enough that exact ties recur.
        let quantised: Vec<(f64, f64)> = (0..600)
            .map(|_| {
                (
                    (rng.unit() * 40.0).round() * 0.5,
                    (rng.unit() * 40.0).round() * 0.5,
                )
            })
            .collect();
        v.push(("quantised_cloud_600", quantised));
        let scattered: Vec<(f64, f64)> = (0..1200)
            .map(|_| (rng.unit() * 250.0, rng.unit() * 190.0))
            .collect();
        v.push(("scattered_1200", scattered));
        // The bench fixture's own scatter, which is what the G5 numbers are
        // measured on.
        let bench: Vec<(f64, f64)> = (0..1500)
            .map(|i| {
                let t = i as f64;
                (
                    200.0 * (t * 0.9137).sin().abs(),
                    200.0 * (t * 0.4271).cos().abs(),
                )
            })
            .collect();
        v.push(("bench_scatter_1500", bench));
        v
    }

    #[test]
    fn tsp_seed_permutation_is_identical() {
        for (name, pts) in cases() {
            // Segment ends offset from starts, so query points are NOT the
            // candidate set — the asymmetry the real `tsp` has.
            let ends: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 + 1.5, p.1 + 0.8)).collect();
            assert_eq!(ref_tsp(&pts, &ends), picked_tsp(&pts, &ends), "tsp/{name}");
            // And with ends == starts (zero-length segments), which puts a
            // candidate exactly on the query point.
            assert_eq!(
                ref_tsp(&pts, &pts),
                picked_tsp(&pts, &pts),
                "tsp/{name}/degenerate"
            );
        }
    }

    #[test]
    fn surface_link_order_is_identical() {
        for (name, pts) in cases() {
            let exits: Vec<(f64, f64)> = pts.iter().map(|p| (p.1 * 0.5, p.0 * 0.5)).collect();
            assert_eq!(
                ref_surface_link(&pts, &exits),
                picked_surface_link(&pts, &exits),
                "surface_link/{name}"
            );
        }
    }

    #[test]
    fn pencil_order_and_reversals_are_identical() {
        for (name, pts) in cases() {
            // Each "path" is a short polyline: start at the case point, end a
            // deterministic offset away, so both endpoints matter.
            let paths: Vec<Vec<(f64, f64)>> = pts
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if i % 17 == 5 {
                        Vec::new() // empty chains: skipped by the reference
                    } else {
                        let sgn = if i % 2 == 0 { 1.0 } else { -1.0 };
                        vec![*p, (p.0 + 0.7, p.1), (p.0 + sgn * 1.4, p.1 + sgn * 0.9)]
                    }
                })
                .collect();
            let (ro, rr) = ref_pencil(&paths);
            let (po, pr) = picked_pencil(&paths);
            assert_eq!(ro, po, "pencil order/{name}");
            assert_eq!(rr, pr, "pencil reversals/{name}");
        }
    }

    #[test]
    fn clearing_order_is_identical() {
        for (name, pts) in cases() {
            for start in [(0.0, 0.0), (-50.0, -50.0), (1.0e9, 0.0), (5.5, 5.5)] {
                assert_eq!(
                    ref_clearing(&pts, start),
                    picked_clearing(&pts, start),
                    "clearing/{name}/start{start:?}"
                );
            }
        }
    }

    /// The bipartite-match shape, whose deletion is driven by ACCEPTANCE and
    /// not by selection — a rejected candidate must still be there for the
    /// next query. Extents are swept from "reject almost everything" to
    /// "accept everything" so both sides of the branch are exercised.
    #[test]
    fn ramp_contour_match_is_identical() {
        for (name, pts) in cases() {
            if pts.is_empty() {
                continue;
            }
            let half = pts.len().div_ceil(2);
            let (upper, lower) = pts.split_at(half);
            for extent in [0.0_f64, 0.5, 1.0, 3.0, 25.0, 1.0e6] {
                let ul: Vec<f64> = (0..upper.len())
                    .map(|i| extent * (1.0 + (i % 3) as f64))
                    .collect();
                let ll: Vec<f64> = (0..lower.len())
                    .map(|i| extent * (1.0 + (i % 5) as f64))
                    .collect();
                assert_eq!(
                    ref_ramp_match(upper, &ul, lower, &ll),
                    picked_ramp_match(upper, &ul, lower, &ll),
                    "ramp/{name}/extent{extent}"
                );
            }
        }
    }

    /// Coordinates compared as raw bits, not `==`: `-0.0 == 0.0` would mask a
    /// divergence, and so would a last-ULP difference in the value the picker
    /// reports back to the call site (which is what the acceptance sentinel
    /// is tested against).
    #[test]
    fn reported_value_matches_the_hand_rolled_expression_bit_for_bit() {
        for (name, pts) in cases() {
            if pts.is_empty() {
                continue;
            }
            for metric in [Metric::Euclid, Metric::EuclidSq] {
                let mut picker = NearestPicker::new(metric, pts.len());
                for (i, p) in pts.iter().enumerate() {
                    picker.push(i, p.0, p.1);
                }
                picker.build();
                for q in [(0.0, 0.0), (3.25, -7.5), (1.0e6, 1.0e6), pts[0]] {
                    let Some((idx, v)) = picker.nearest(q.0, q.1) else {
                        continue;
                    };
                    let dx = pts[idx].0 - q.0;
                    let dy = pts[idx].1 - q.1;
                    let sq = dx * dx + dy * dy;
                    let want = if metric == Metric::Euclid {
                        sq.sqrt()
                    } else {
                        sq
                    };
                    assert_eq!(
                        v.to_bits(),
                        want.to_bits(),
                        "{name}/{metric:?}/q{q:?}: reported {v} vs recomputed {want}"
                    );
                }
            }
        }
    }

    /// The pruning must not change the answer relative to an exhaustive scan
    /// of the SAME picker — this isolates the grid from the tie-break rule.
    #[test]
    fn ring_search_agrees_with_exhaustive_scan_under_deletion() {
        let pts = cases()
            .into_iter()
            .find(|(n, _)| *n == "scattered_1200")
            .map(|(_, p)| p)
            .unwrap();
        let mut rng = Rng(0xDEAD_BEEF_1234_5678);
        for metric in [Metric::Euclid, Metric::EuclidSq] {
            let mut picker = NearestPicker::new(metric, pts.len());
            for (i, p) in pts.iter().enumerate() {
                picker.push(i, p.0, p.1);
            }
            picker.build();
            let mut alive: Vec<bool> = vec![true; pts.len()];
            for step in 0..pts.len() {
                let q = (rng.unit() * 300.0 - 25.0, rng.unit() * 240.0 - 25.0);
                // Exhaustive reference over the same live set.
                let mut want: Option<(f64, u32)> = None;
                for (i, p) in pts.iter().enumerate() {
                    if !alive[i] {
                        continue;
                    }
                    let v = metric.value(p.0 - q.0, p.1 - q.1);
                    if beats(v, i as u32, want) {
                        want = Some((v, i as u32));
                    }
                }
                let got = picker.nearest(q.0, q.1);
                assert_eq!(
                    got.map(|(i, v)| (i, v.to_bits())),
                    want.map(|(v, i)| (i as usize, v.to_bits())),
                    "{metric:?} step {step}"
                );
                if let Some((i, _)) = got {
                    picker.remove(i);
                    alive[i] = false;
                }
            }
        }
    }

    #[test]
    fn remove_is_idempotent_and_out_of_range_is_a_no_op() {
        let mut picker = NearestPicker::new(Metric::Euclid, 3);
        for i in 0..3 {
            picker.push(i, i as f64, 0.0);
        }
        picker.build();
        picker.remove(0);
        picker.remove(0);
        picker.remove(99);
        assert_eq!(picker.nearest(0.0, 0.0).map(|(i, _)| i), Some(1));
        picker.remove(1);
        picker.remove(2);
        assert_eq!(picker.nearest(0.0, 0.0), None);
    }
}
