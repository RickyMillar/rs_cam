//! Indexed point-to-boundary distance field for scan-line sampling (G9).
//!
//! Three generators — v-carve, the inlay male plug, and (historically) rest —
//! sampled a polygon's boundary distance once per scan-line sample with a
//! *linear scan over every edge*. The helper was triplicated verbatim in
//! `vcarve.rs` and `inlay.rs`. At the shipped default tolerance (0.05 mm) a
//! 100x100 mm region is ~400k samples; against 2000-edge lettering that is
//! ~8e8 point-segment evaluations, and inlay pays it twice (female via
//! v-carve, male directly).
//!
//! [`EdgeDistanceField`] replaces the scan with a uniform-grid edge index and
//! an expanding-ring query. It is the same trick `chain_segments`' quantized
//! hash index already uses elsewhere in this crate.
//!
//! # Exactness
//!
//! V-carve maps this distance **directly to cut depth**, so an approximate
//! answer is a wrong cut, not a slow one. The query is therefore exact — it
//! returns the bit-identical `f64` the linear scan would have returned, not a
//! close one:
//!
//! - The edge set is built in the same order and with the same closing-edge
//!   rule as the old helpers, so it is the same multiset of segments.
//! - The reduction is `f64::min` over that multiset. `min` ignores NaN and is
//!   order-independent for everything else, so evaluation order cannot change
//!   the result — ties at equal distance are decided by value, not by index.
//! - The ring search only *prunes*: a ring is skipped only once the whole
//!   remaining grid is provably farther than the best distance found so far.
//!   The closest point on any segment lies **on** that segment, and each
//!   segment is stamped into every cell it passes through (plus a one-cell
//!   halo), so the cell holding that closest point is always visited before
//!   the bound can cut the search off.
//!
//! `edge_distance_matches_linear_scan_bitwise` in this module's tests pins
//! that claim against a verbatim copy of the pre-G9 helper over randomised
//! polygons, on-edge and outside-polygon query points, degenerate rings and
//! empty holes.

use crate::geo::{P2, point_to_segment_distance};
use crate::polygon::Polygon2;

/// Maximum cells along either grid axis.
const MAX_DIM: usize = 1024;

/// Maximum total cells, so a pathological aspect ratio cannot blow the index
/// up past a few megabytes.
const MAX_CELLS: usize = 1 << 20;

/// Slack applied to the ring-termination bound, in mm.
///
/// The bound is derived from cell-boundary arithmetic while the distances it
/// is compared against come from `point_to_segment_distance`; the two agree to
/// ~1e-16 relative, and this absolute floor buys about seven orders of
/// magnitude of headroom on mm-scale coordinates. Its only cost when it fires
/// is one extra ring of cells scanned.
const BOUND_SLACK_MM: f64 = 1e-9;

/// A polygon boundary indexed for repeated nearest-distance queries.
///
/// Build once per polygon, query once per sample. See the module docs for the
/// exactness argument.
pub struct EdgeDistanceField {
    /// Every boundary segment, in the order the pre-G9 linear scan walked
    /// them (exterior windows, exterior closing edge, then each hole the same
    /// way).
    edges: Vec<(P2, P2)>,
    /// CSR row starts into [`Self::items`], length `nx * ny + 1`.
    starts: Vec<u32>,
    /// CSR payload: indices into [`Self::edges`].
    items: Vec<u32>,
    min_x: f64,
    min_y: f64,
    cell_x: f64,
    cell_y: f64,
    nx: usize,
    ny: usize,
}

/// Append one ring's edges using the pre-G9 closing rule: `windows(2)` pairs
/// followed by the `(last, first)` wrap-around when the ring is non-empty.
///
/// A one-vertex ring therefore contributes a single degenerate `(p, p)`
/// segment, exactly as the old helper did — `point_to_segment_distance`
/// treats that as a point, and dropping it would change the answer.
#[allow(clippy::indexing_slicing)]
// SAFETY: `windows(2)` yields slices of exactly length 2, so w[0]/w[1] exist.
fn push_ring(edges: &mut Vec<(P2, P2)>, ring: &[P2]) {
    for w in ring.windows(2) {
        edges.push((w[0], w[1]));
    }
    if let (Some(last), Some(first)) = (ring.last(), ring.first()) {
        edges.push((*last, *first));
    }
}

impl EdgeDistanceField {
    /// Index a polygon's exterior plus all its holes.
    #[must_use]
    pub fn from_polygon(polygon: &Polygon2) -> Self {
        Self::from_rings(&polygon.exterior, &polygon.holes)
    }

    /// Index an exterior ring plus hole rings held separately (inlay's male
    /// pass carves relative to the *design* boundary, not the outer stock
    /// rectangle it builds around it, so it never has a `Polygon2` of just
    /// the rings it wants).
    #[must_use]
    pub fn from_rings(exterior: &[P2], holes: &[Vec<P2>]) -> Self {
        let mut edges = Vec::new();
        push_ring(&mut edges, exterior);
        for hole in holes {
            push_ring(&mut edges, hole);
        }
        Self::from_edges(edges)
    }

    /// Number of indexed boundary segments.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    fn empty(edges: Vec<(P2, P2)>) -> Self {
        Self {
            edges,
            starts: vec![0],
            items: Vec::new(),
            min_x: 0.0,
            min_y: 0.0,
            cell_x: 1.0,
            cell_y: 1.0,
            nx: 0,
            ny: 0,
        }
    }

    fn from_edges(edges: Vec<(P2, P2)>) -> Self {
        if edges.is_empty() {
            return Self::empty(edges);
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (a, b) in &edges {
            // NaN coordinates fall through these chains untouched, which
            // matches the linear scan: `f64::min` ignores the NaN distance
            // such an edge produces, so it can never be the answer either way.
            min_x = min_x.min(a.x).min(b.x);
            min_y = min_y.min(a.y).min(b.y);
            max_x = max_x.max(a.x).max(b.x);
            max_y = max_y.max(a.y).max(b.y);
        }
        let w = max_x - min_x;
        let h = max_y - min_y;
        if !(w.is_finite() && h.is_finite() && min_x.is_finite() && min_y.is_finite()) {
            // Infinite/NaN extents: no useful grid exists. Keep the edges and
            // fall back to the linear scan on query.
            return Self::empty(edges);
        }

        let n = edges.len() as f64;
        let area = w * h;
        let (mut nx, mut ny) = if area > 0.0 {
            // Aim for roughly one edge per cell.
            let cell = (area / n).sqrt();
            if cell > 0.0 && cell.is_finite() {
                (
                    ((w / cell).ceil() as usize).clamp(1, MAX_DIM),
                    ((h / cell).ceil() as usize).clamp(1, MAX_DIM),
                )
            } else {
                (1, 1)
            }
        } else {
            // Zero-width or zero-height bbox (a horizontal/vertical run of
            // collinear edges): bucket along whichever axis has extent.
            let m = (n.sqrt().ceil() as usize).clamp(1, MAX_DIM);
            if w > 0.0 {
                (m, 1)
            } else if h > 0.0 {
                (1, m)
            } else {
                (1, 1)
            }
        };
        while nx.saturating_mul(ny) > MAX_CELLS && (nx > 1 || ny > 1) {
            nx = (nx / 2).max(1);
            ny = (ny / 2).max(1);
        }

        let cell_x = if w > 0.0 { w / nx as f64 } else { 1.0 };
        let cell_y = if h > 0.0 { h / ny as f64 } else { 1.0 };

        let mut field = Self {
            edges,
            starts: Vec::new(),
            items: Vec::new(),
            min_x,
            min_y,
            cell_x,
            cell_y,
            nx,
            ny,
        };
        field.build_csr();
        field
    }

    /// Two-pass CSR build: count per cell, prefix-sum, then fill.
    fn build_csr(&mut self) {
        let cells = self.nx * self.ny;
        let mut counts = vec![0u32; cells + 1];
        for (a, b) in &self.edges {
            Self::for_each_cell(
                self.min_x,
                self.min_y,
                self.cell_x,
                self.cell_y,
                self.nx,
                self.ny,
                a,
                b,
                |c| {
                    if let Some(slot) = counts.get_mut(c + 1) {
                        *slot = slot.saturating_add(1);
                    }
                },
            );
        }
        for i in 1..=cells {
            let prev = counts.get(i - 1).copied().unwrap_or(0);
            if let Some(slot) = counts.get_mut(i) {
                *slot = slot.saturating_add(prev);
            }
        }
        let total = counts.get(cells).copied().unwrap_or(0) as usize;
        let mut items = vec![0u32; total];
        let mut cursor = counts.clone();
        for (idx, (a, b)) in self.edges.iter().enumerate() {
            Self::for_each_cell(
                self.min_x,
                self.min_y,
                self.cell_x,
                self.cell_y,
                self.nx,
                self.ny,
                a,
                b,
                |c| {
                    if let Some(pos) = cursor.get_mut(c) {
                        if let Some(slot) = items.get_mut(*pos as usize) {
                            *slot = idx as u32;
                        }
                        *pos += 1;
                    }
                },
            );
        }
        self.starts = counts;
        self.items = items;
    }

    fn ix_of(min: f64, cell: f64, n: usize, v: f64) -> usize {
        let raw = (v - min) / cell;
        if raw.is_nan() {
            return 0;
        }
        let f = raw.floor();
        if f < 0.0 {
            0
        } else if f >= n as f64 {
            n.saturating_sub(1)
        } else {
            f as usize
        }
    }

    /// Visit every cell index a segment could touch.
    ///
    /// Walks the segment column by column, clipping it to each column's x
    /// slab and stamping the cells its y range covers there — so a long
    /// diagonal edge costs O(cells crossed), not O(bbox area). Both the
    /// column range and the per-column row range carry a one-cell halo, which
    /// is what makes the "the closest point's cell is always stamped" claim
    /// robust to boundary rounding.
    #[allow(clippy::too_many_arguments)]
    fn for_each_cell(
        min_x: f64,
        min_y: f64,
        cell_x: f64,
        cell_y: f64,
        nx: usize,
        ny: usize,
        a: &P2,
        b: &P2,
        mut f: impl FnMut(usize),
    ) {
        if nx == 0 || ny == 0 {
            return;
        }
        let x_lo = a.x.min(b.x);
        let x_hi = a.x.max(b.x);
        let ix0 = Self::ix_of(min_x, cell_x, nx, x_lo).saturating_sub(1);
        let ix1 = (Self::ix_of(min_x, cell_x, nx, x_hi) + 1).min(nx - 1);

        let dx = b.x - a.x;
        for ix in ix0..=ix1 {
            let slab_lo = min_x + ix as f64 * cell_x;
            let slab_hi = slab_lo + cell_x;
            let (y_lo, y_hi) = if dx.abs() < 1e-300 {
                (a.y.min(b.y), a.y.max(b.y))
            } else {
                let t0 = ((slab_lo - a.x) / dx).clamp(0.0, 1.0);
                let t1 = ((slab_hi - a.x) / dx).clamp(0.0, 1.0);
                let ya = a.y + t0 * (b.y - a.y);
                let yb = a.y + t1 * (b.y - a.y);
                (ya.min(yb), ya.max(yb))
            };
            let iy0 = Self::ix_of(min_y, cell_y, ny, y_lo).saturating_sub(1);
            let iy1 = (Self::ix_of(min_y, cell_y, ny, y_hi) + 1).min(ny - 1);
            for iy in iy0..=iy1 {
                f(iy * nx + ix);
            }
        }
    }

    /// Exact minimum distance from `p` to any boundary segment.
    ///
    /// Returns `f64::INFINITY` when the boundary has no edges — the same
    /// value the pre-G9 linear scan returned for an empty polygon.
    #[must_use]
    pub fn distance(&self, p: &P2) -> f64 {
        self.query(p).0
    }

    /// [`Self::distance`] plus the number of point-segment evaluations it
    /// took. The count is what `edge_distance_actually_prunes` asserts on —
    /// a deterministic measure of the pruning, rather than a wall clock.
    #[must_use]
    pub fn query(&self, p: &P2) -> (f64, usize) {
        if self.edges.is_empty() {
            return (f64::INFINITY, 0);
        }
        if self.nx == 0 || self.ny == 0 {
            return (self.distance_linear(p), self.edges.len());
        }

        let cx = Self::ix_of(self.min_x, self.cell_x, self.nx, p.x);
        let cy = Self::ix_of(self.min_y, self.cell_y, self.ny, p.y);
        let max_r = cx.max(self.nx - 1 - cx).max(cy).max(self.ny - 1 - cy);

        let mut best = f64::INFINITY;
        let mut visits = 0usize;
        for r in 0..=max_r {
            self.scan_ring(p, cx, cy, r, &mut best, &mut visits);
            if best.is_finite() {
                // Every cell outside the (2r+1)² block around `p`'s cell lies
                // outside the box below, so nothing left to scan can be closer
                // than the gap from `p` to that box's nearest face.
                let bound = self.ring_bound(p, cx, cy, r);
                if bound - BOUND_SLACK_MM > best {
                    break;
                }
            }
        }
        (best, visits)
    }

    /// Distance to the polygon's *nearest* boundary segment computed by a
    /// full linear scan — the pre-G9 behaviour, kept as the reference the
    /// indexed query is pinned against and as the degenerate-grid fallback.
    #[must_use]
    pub fn distance_linear(&self, p: &P2) -> f64 {
        let mut best = f64::INFINITY;
        for (a, b) in &self.edges {
            best = best.min(point_to_segment_distance(p, a, b));
        }
        best
    }

    fn ring_bound(&self, p: &P2, cx: usize, cy: usize, r: usize) -> f64 {
        let r = r as f64;
        let x_lo = self.min_x + (cx as f64 - r) * self.cell_x;
        let x_hi = self.min_x + (cx as f64 + r + 1.0) * self.cell_x;
        let y_lo = self.min_y + (cy as f64 - r) * self.cell_y;
        let y_hi = self.min_y + (cy as f64 + r + 1.0) * self.cell_y;
        (p.x - x_lo).min(x_hi - p.x).min(p.y - y_lo).min(y_hi - p.y)
    }

    fn scan_ring(
        &self,
        p: &P2,
        cx: usize,
        cy: usize,
        r: usize,
        best: &mut f64,
        visits: &mut usize,
    ) {
        let (cx, cy, r) = (cx as isize, cy as isize, r as isize);
        if r == 0 {
            self.scan_cell(p, cx, cy, best, visits);
            return;
        }
        for ix in (cx - r)..=(cx + r) {
            self.scan_cell(p, ix, cy - r, best, visits);
            self.scan_cell(p, ix, cy + r, best, visits);
        }
        for iy in (cy - r + 1)..=(cy + r - 1) {
            self.scan_cell(p, cx - r, iy, best, visits);
            self.scan_cell(p, cx + r, iy, best, visits);
        }
    }

    fn scan_cell(&self, p: &P2, ix: isize, iy: isize, best: &mut f64, visits: &mut usize) {
        if ix < 0 || iy < 0 || ix >= self.nx as isize || iy >= self.ny as isize {
            return;
        }
        let c = iy as usize * self.nx + ix as usize;
        let (Some(&s), Some(&e)) = (self.starts.get(c), self.starts.get(c + 1)) else {
            return;
        };
        let Some(slice) = self.items.get(s as usize..e as usize) else {
            return;
        };
        *visits += slice.len();
        for &idx in slice {
            if let Some((a, b)) = self.edges.get(idx as usize) {
                *best = best.min(point_to_segment_distance(p, a, b));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// **Verbatim copy of the pre-G9 `vcarve::point_to_polygon_distance`.**
    ///
    /// Do not "tidy" this — its whole job is to be the thing the indexed
    /// query is compared against, so it must keep walking every edge in the
    /// original order with the original closing rule.
    fn naive(point: &P2, polygon: &Polygon2) -> f64 {
        let mut min_dist = f64::INFINITY;

        let ext = &polygon.exterior;
        for w in ext.windows(2) {
            min_dist = min_dist.min(point_to_segment_distance(point, &w[0], &w[1]));
        }
        if let (Some(last), Some(first)) = (ext.last(), ext.first()) {
            min_dist = min_dist.min(point_to_segment_distance(point, last, first));
        }

        for hole in &polygon.holes {
            for w in hole.windows(2) {
                min_dist = min_dist.min(point_to_segment_distance(point, &w[0], &w[1]));
            }
            if let (Some(last), Some(first)) = (hole.last(), hole.first()) {
                min_dist = min_dist.min(point_to_segment_distance(point, last, first));
            }
        }

        min_dist
    }

    /// Tiny deterministic PRNG — no dev-dependency, reproducible failures.
    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let x = self.0;
            (x >> 33) ^ x
        }
        fn unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + self.unit() * (hi - lo)
        }
    }

    fn ring(rng: &mut Rng, cx: f64, cy: f64, r: f64, n: usize) -> Vec<P2> {
        (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                let rr = r * rng.range(0.6, 1.0);
                P2::new(cx + rr * t.cos(), cy + rr * t.sin())
            })
            .collect()
    }

    fn assert_bitwise(poly: &Polygon2, pts: &[P2], label: &str) {
        let field = EdgeDistanceField::from_polygon(poly);
        for p in pts {
            let want = naive(p, poly);
            let got = field.distance(p);
            assert_eq!(
                want.to_bits(),
                got.to_bits(),
                "{label}: distance at ({}, {}) diverged — linear {want:?} vs indexed {got:?}",
                p.x,
                p.y
            );
        }
    }

    /// **The G9 exactness pin.** Bit-for-bit, not `==` — `-0.0 == 0.0` would
    /// mask a divergence, and v-carve turns this number straight into a Z.
    #[test]
    fn edge_distance_matches_linear_scan_bitwise() {
        let mut rng = Rng(0x5EED_1234);

        for case in 0..40 {
            // Lettering-ish: a frame with a scatter of holes of varying size.
            let mut poly = Polygon2::rectangle(-30.0, -30.0, 30.0, 30.0);
            let n_holes = case % 7;
            for h in 0..n_holes {
                let cx = rng.range(-22.0, 22.0);
                let cy = rng.range(-22.0, 22.0);
                let verts = 3 + (h * 5 + case) % 40;
                let radius = rng.range(0.5, 6.0);
                poly.holes.push(ring(&mut rng, cx, cy, radius, verts));
            }

            let mut pts: Vec<P2> = Vec::new();
            // Interior scatter.
            for _ in 0..400 {
                pts.push(P2::new(rng.range(-32.0, 32.0), rng.range(-32.0, 32.0)));
            }
            // Far outside the polygon entirely (query cell clamps to the grid
            // edge, so the ring bound starts negative).
            for _ in 0..40 {
                pts.push(P2::new(rng.range(-500.0, -100.0), rng.range(-500.0, 500.0)));
                pts.push(P2::new(rng.range(100.0, 500.0), rng.range(-500.0, 500.0)));
            }
            // Exactly on vertices and on edge midpoints — the tie cases.
            for v in poly.exterior.clone() {
                pts.push(v);
            }
            for hole in poly.holes.clone() {
                for w in hole.windows(2) {
                    pts.push(w[0]);
                    pts.push(P2::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0));
                }
            }
            // Grid-cell boundaries are the one place the bound arithmetic and
            // the floor() bucketing can disagree.
            for i in -60..=60 {
                pts.push(P2::new(i as f64, i as f64 * 0.5));
            }

            assert_bitwise(&poly, &pts, &format!("random case {case}"));
        }
    }

    #[test]
    fn edge_distance_handles_degenerate_boundaries() {
        let probes: Vec<P2> = (-5..=5)
            .flat_map(|i| (-5..=5).map(move |j| P2::new(i as f64 * 2.3, j as f64 * 1.7)))
            .collect();

        // Empty exterior.
        let empty = Polygon2::new(Vec::new());
        let field = EdgeDistanceField::from_polygon(&empty);
        for p in &probes {
            assert_eq!(field.distance(p).to_bits(), naive(p, &empty).to_bits());
            assert!(field.distance(p).is_infinite());
        }

        // Single-vertex exterior: the (last, first) rule makes one degenerate
        // segment, and dropping it would change the answer from 0 to INFINITY.
        let one = Polygon2::new(vec![P2::new(1.0, 2.0)]);
        assert_bitwise(&one, &probes, "single vertex");
        assert_eq!(EdgeDistanceField::from_polygon(&one).edge_count(), 1);

        // Two-vertex exterior (a bare segment, walked twice: forward + wrap).
        let two = Polygon2::new(vec![P2::new(-3.0, 0.0), P2::new(3.0, 0.0)]);
        assert_bitwise(&two, &probes, "two vertices");

        // Collinear exterior: zero-height bbox, so the grid degenerates to a
        // single row.
        let flat = Polygon2::new(vec![
            P2::new(-5.0, 1.0),
            P2::new(0.0, 1.0),
            P2::new(5.0, 1.0),
        ]);
        assert_bitwise(&flat, &probes, "collinear (zero-height bbox)");

        // All vertices coincident: zero-area bbox in both axes.
        let dot = Polygon2::new(vec![P2::new(2.0, 2.0); 4]);
        assert_bitwise(&dot, &probes, "coincident vertices");

        // Empty holes alongside a real exterior.
        let mut with_empty_holes = Polygon2::rectangle(-4.0, -4.0, 4.0, 4.0);
        with_empty_holes.holes.push(Vec::new());
        with_empty_holes.holes.push(vec![P2::new(0.5, 0.5)]);
        assert_bitwise(&with_empty_holes, &probes, "empty + single-vertex holes");

        // Explicitly closed ring (last == first) produces a zero-length wrap
        // edge in both paths.
        let closed = Polygon2::new(vec![
            P2::new(-2.0, -2.0),
            P2::new(2.0, -2.0),
            P2::new(2.0, 2.0),
            P2::new(-2.0, 2.0),
            P2::new(-2.0, -2.0),
        ]);
        assert_bitwise(&closed, &probes, "explicitly closed ring");
    }

    /// A very long thin polygon: the aspect-ratio clamp path, plus queries
    /// whose nearest edge is many rings away along the short axis.
    #[test]
    fn edge_distance_matches_on_extreme_aspect_ratio() {
        let mut rng = Rng(0xABCD_0001);
        let mut poly = Polygon2::rectangle(0.0, 0.0, 4000.0, 2.0);
        for i in 0..200 {
            let cx = 10.0 + i as f64 * 19.7;
            poly.holes.push(ring(&mut rng, cx, 1.0, 0.4, 8));
        }
        let mut pts = Vec::new();
        for _ in 0..2000 {
            pts.push(P2::new(rng.range(-50.0, 4050.0), rng.range(-10.0, 12.0)));
        }
        assert_bitwise(&poly, &pts, "extreme aspect ratio");
    }

    /// `from_rings` (inlay's male pass) must index the same multiset as
    /// `from_polygon` over the same rings.
    #[test]
    fn from_rings_matches_from_polygon() {
        let mut rng = Rng(0x1111_2222);
        let mut poly = Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0);
        poly.holes.push(ring(&mut rng, 0.0, 0.0, 3.0, 17));
        poly.holes.push(ring(&mut rng, 5.0, -4.0, 2.0, 9));

        let a = EdgeDistanceField::from_polygon(&poly);
        let b = EdgeDistanceField::from_rings(&poly.exterior, &poly.holes);
        assert_eq!(a.edge_count(), b.edge_count());
        for i in -30..=30 {
            for j in -30..=30 {
                let p = P2::new(i as f64 * 0.7, j as f64 * 0.7);
                assert_eq!(a.distance(&p).to_bits(), b.distance(&p).to_bits());
            }
        }
    }

    /// The index must not merely agree with the linear scan — it must
    /// actually prune, or G9 bought nothing. Guards against a regression
    /// where a broken bound degenerates the query into a full scan.
    #[test]
    fn edge_distance_actually_prunes() {
        let mut rng = Rng(0x7777_8888);
        let mut poly = Polygon2::rectangle(-30.0, -30.0, 30.0, 30.0);
        for i in 0..40 {
            let cx = -24.0 + (i % 8) as f64 * 6.5;
            let cy = -24.0 + (i / 8) as f64 * 12.0;
            poly.holes.push(ring(&mut rng, cx, cy, 1.5, 48));
        }
        let field = EdgeDistanceField::from_polygon(&poly);
        let n_edges = field.edge_count();
        assert!(
            n_edges > 1900,
            "fixture should be edge-dense, got {n_edges}"
        );

        let mut total_visits = 0usize;
        let samples = 20_000;
        for _ in 0..samples {
            let p = P2::new(rng.range(-29.0, 29.0), rng.range(-29.0, 29.0));
            let (d, visits) = field.query(&p);
            assert!(d.is_finite());
            total_visits += visits;
        }
        let mean = total_visits as f64 / samples as f64;
        let linear = n_edges as f64;
        // Measured 2026-08-20: mean 285.7 of 1924 edges, i.e. 6.7x fewer
        // point-segment evaluations. The bar is set below that with margin —
        // it is a regression guard on the bound, not a tuning target.
        assert!(
            mean * 5.0 < linear,
            "indexed query should evaluate well under a fifth of the {linear} edges the \
             linear scan walks, got mean {mean:.1}"
        );
    }
}
