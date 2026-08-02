//! Core tri-dexel data types: segments, rays, and grids.
//!
//! A dexel ray is a sorted list of non-overlapping material segments along one
//! axis.  `SmallVec<[DexelSegment; 1]>` keeps the overwhelmingly common
//! single-segment case (fresh stock, top-only cuts) allocation-free.

use smallvec::SmallVec;

use crate::geo::BoundingBox3;

// ── Segment ─────────────────────────────────────────────────────────────

/// A single contiguous material interval along a ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DexelSegment {
    /// Start of material (inclusive, toward ray origin).
    pub enter: f32,
    /// End of material (inclusive, toward ray tip).
    pub exit: f32,
}

impl DexelSegment {
    #[inline]
    pub fn new(enter: f32, exit: f32) -> Self {
        debug_assert!(enter <= exit, "enter {enter} > exit {exit}");
        Self { enter, exit }
    }

    #[inline]
    pub fn length(&self) -> f32 {
        self.exit - self.enter
    }
}

// ── Ray ─────────────────────────────────────────────────────────────────

/// One ray's segment list.  Segments are always sorted by `enter` and
/// non-overlapping.
pub type DexelRay = SmallVec<[DexelSegment; 1]>;

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Remove all material **above** `z` (i.e. with coordinate > z).
///
/// Used for top-down cuts on a Z-grid: material above the tool surface is air.
pub fn ray_subtract_above(ray: &mut DexelRay, z: f32) {
    // Walk from the end (highest segments first) for efficient removal.
    let mut i = ray.len();
    while i > 0 {
        i -= 1;
        let seg = &ray[i];
        if seg.enter >= z {
            // Entirely above z — remove.
            ray.remove(i);
        } else if seg.exit > z {
            // Straddles z — truncate.
            ray[i].exit = z;
        }
        // else: entirely below z — keep as-is.
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Remove all material **below** `z` (i.e. with coordinate < z).
///
/// Used for bottom-up cuts on a Z-grid.
pub fn ray_subtract_below(ray: &mut DexelRay, z: f32) {
    let mut i = 0;
    while i < ray.len() {
        let seg = &ray[i];
        if seg.exit <= z {
            // Entirely below z — remove.
            ray.remove(i);
            // don't increment i; next element shifted into this slot
        } else if seg.enter < z {
            // Straddles z — truncate.
            ray[i].enter = z;
            i += 1;
        } else {
            // Entirely above z — keep.
            i += 1;
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Partial-blend variant of [`ray_subtract_above`]: shrink the above-`surface`
/// portion of each segment by a fraction `f ∈ [0, 1]` of its height.
///
/// Used by F.a sub-cell stamping (see `DEXEL_Z_ONLY_INVESTIGATION.md` §6.F):
/// when a cell is fractionally covered by the cutter footprint (coverage `f`),
/// the area-weighted view says `f` of the cell sits at the cutter surface and
/// `(1-f)` retains the original top. The resulting cell-averaged top equals
/// the original above-`surface` slice shortened by `f`.
///
/// `f = 1` is equivalent to `ray_subtract_above`; `f = 0` is a no-op. NaN /
/// out-of-range `f` is clamped.
pub fn ray_blend_above(ray: &mut DexelRay, surface: f32, f: f32) {
    if f.is_nan() {
        return;
    }
    let f = f.clamp(0.0, 1.0);
    if f <= 0.0 {
        return;
    }
    if f >= 1.0 {
        ray_subtract_above(ray, surface);
        return;
    }
    let mut i = ray.len();
    while i > 0 {
        i -= 1;
        let seg = ray[i];
        // Lower bound of the above-`surface` portion of this segment.
        let above_lo = seg.enter.max(surface);
        if seg.exit <= above_lo {
            // Segment is entirely at or below `surface` — no above portion.
            continue;
        }
        let above_part = seg.exit - above_lo;
        let new_exit = seg.exit - f * above_part;
        if new_exit <= seg.enter {
            ray.remove(i);
        } else {
            ray[i].exit = new_exit;
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Mirror of [`ray_blend_above`] for bottom-up cuts: shrink the below-`surface`
/// portion of each segment by `f ∈ [0, 1]`.
pub fn ray_blend_below(ray: &mut DexelRay, surface: f32, f: f32) {
    if f.is_nan() {
        return;
    }
    let f = f.clamp(0.0, 1.0);
    if f <= 0.0 {
        return;
    }
    if f >= 1.0 {
        ray_subtract_below(ray, surface);
        return;
    }
    let mut i = 0;
    while i < ray.len() {
        let seg = ray[i];
        let below_hi = seg.exit.min(surface);
        if seg.enter >= below_hi {
            i += 1;
            continue;
        }
        let below_part = below_hi - seg.enter;
        let new_enter = seg.enter + f * below_part;
        if new_enter >= seg.exit {
            ray.remove(i);
        } else {
            ray[i].enter = new_enter;
            i += 1;
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Remove the interval `[a, b]` from the ray (general boolean subtract).
///
/// Any segment fully inside `[a, b]` is deleted.  Segments that partially
/// overlap are trimmed; a segment that straddles `[a, b]` is split in two.
pub fn ray_subtract_interval(ray: &mut DexelRay, a: f32, b: f32) {
    debug_assert!(a <= b);
    let mut i = 0;
    while i < ray.len() {
        let seg = ray[i];
        if seg.exit <= a || seg.enter >= b {
            // No overlap — keep.
            i += 1;
        } else if seg.enter >= a && seg.exit <= b {
            // Entirely inside interval — remove.
            ray.remove(i);
        } else if seg.enter < a && seg.exit > b {
            // Interval is strictly inside segment — split.
            ray[i].exit = a;
            ray.insert(i + 1, DexelSegment::new(b, seg.exit));
            i += 2;
        } else if seg.enter < a {
            // Overlaps on the right — trim exit.
            ray[i].exit = a;
            i += 1;
        } else {
            // seg.exit > b, overlaps on the left — trim enter.
            ray[i].enter = b;
            i += 1;
        }
    }
}

/// Returns `true` if the ray has no material.
#[inline]
pub fn ray_is_empty(ray: &DexelRay) -> bool {
    ray.is_empty()
}

/// Total material length along this ray.
pub fn ray_material_length(ray: &DexelRay) -> f32 {
    ray.iter().map(|s| s.length()).sum()
}

/// Return the highest material coordinate on this ray, or `None` if empty.
#[inline]
pub fn ray_top(ray: &DexelRay) -> Option<f32> {
    ray.last().map(|s| s.exit)
}

/// Return the lowest material coordinate on this ray, or `None` if empty.
#[inline]
pub fn ray_bottom(ray: &DexelRay) -> Option<f32> {
    ray.first().map(|s| s.enter)
}

/// Total material length above a given Z value along this ray.
///
/// Used by simulation metrics to estimate volume removed when a cutter
/// subtracts material above a surface. Each segment contributes
/// `max(0, exit - max(z, enter))`.
pub fn ray_material_length_above(ray: &DexelRay, z: f32) -> f32 {
    let mut sum = 0.0;
    for seg in ray.iter() {
        if seg.exit <= z {
            continue;
        }
        let lower = seg.enter.max(z);
        sum += seg.exit - lower;
    }
    sum
}

// ── Grid axis ───────────────────────────────────────────────────────────

/// Which world axis the rays of a [`DexelGrid`] run along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DexelAxis {
    X,
    Y,
    Z,
}

// ── Grid ────────────────────────────────────────────────────────────────

/// A 2-D grid of dexel rays running along one axis.
///
/// For a Z-grid the grid is indexed by (row=Y, col=X) — matching
/// the heightmap layout — and each ray stores segments
/// along the Z axis.
pub struct DexelGrid {
    pub rays: Vec<DexelRay>,
    pub rows: usize,
    pub cols: usize,
    /// Grid origin in the first planar axis (X for a Z-grid).
    pub origin_u: f64,
    /// Grid origin in the second planar axis (Y for a Z-grid).
    pub origin_v: f64,
    pub cell_size: f64,
    pub axis: DexelAxis,
    /// Maximum fractional cutter coverage seen at each cell across the
    /// project's stamping history (parallel to `rays`).
    ///
    /// Populated by sub-cell stamping (F.a, see `DEXEL_Z_ONLY_INVESTIGATION.md`
    /// §6.F gap 4). Forward-compatible bridge to F.b sub-cell-resolved ray
    /// storage: a value `f ∈ [0, 1]` per cell can later seed `f` of the
    /// sub-cells at the cut surface and `(1-f)` at the bulk material level.
    ///
    /// Not used by any planning / mesh / collision consumer today — purely
    /// observational. Stamping kernels update via running max.
    pub coverage_max: Vec<f32>,

    /// **Sliver-safe upper bound** on where material may still stand anywhere
    /// inside each cell (parallel to `rays`), along the ray axis' high end.
    ///
    /// A/M10. `rays` answers *"how tall is the column AT this sample point"*,
    /// which is the wrong question for a safety ceiling: the cutter's swept
    /// region is stamped with fractional coverage, so a cell only PARTLY
    /// swept is blended down toward the cut surface (`ray_blend_above`) even
    /// though the unswept fraction still stands at its old height. On a
    /// coarse grid an uncut rib narrower than one cell therefore reads as a
    /// half-cut column; on a fine grid it reads at full stock height. That
    /// difference — not any property of the toolpath — is what made the same
    /// generated chain measure 0 / 15 / 20 rapid collisions at 0.5 / 0.25 /
    /// 0.1 mm (TP15 RCA `4f590f3`).
    ///
    /// This channel answers the question a descent ceiling actually asks:
    /// *"how high can material be ANYWHERE in this cell"*. It is lowered
    /// only when a stamp covers the cell **completely**, and then only to an
    /// upper bound of the cutter surface across the whole cell — never to
    /// the cell-centre sample. Two consequences follow, and they are the
    /// entire point:
    ///
    /// * it is a pointwise **over**-estimate of the true material top, so a
    ///   clearance derived from it is safe against any verification grid;
    /// * refining the cell can only LOWER it (a finer cell is covered
    ///   completely more often), so it converges downward to the truth
    ///   instead of jumping around it. Resolution stops changing the answer
    ///   in the direction that matters.
    ///
    /// Maintained for high-side (top-down) removal. Low-side cutting cannot
    /// raise a top, so it needs no mirror; where a mutation cannot be proven
    /// to lower the bound the value is simply left high, which fails safe.
    pub conservative_top: Vec<f32>,
}

impl Clone for DexelGrid {
    fn clone(&self) -> Self {
        Self {
            rays: self.rays.clone(),
            rows: self.rows,
            cols: self.cols,
            origin_u: self.origin_u,
            origin_v: self.origin_v,
            cell_size: self.cell_size,
            axis: self.axis,
            coverage_max: self.coverage_max.clone(),
            conservative_top: self.conservative_top.clone(),
        }
    }
}

impl DexelGrid {
    /// Minimum allowed cell size to avoid division-by-zero and degenerate grids.
    const MIN_CELL_SIZE: f64 = 1e-6;

    /// Maximum total cells per grid (~128 MB at ~8 bytes/ray).
    /// Prevents OOM from pathologically small cell sizes on large stock.
    const MAX_GRID_CELLS: usize = 16_000_000;

    /// Check whether the given cell size would exceed the grid cap for the given extents.
    /// Returns `Some(coarsened_size)` if clamping would occur, `None` if it fits.
    pub fn would_exceed_grid(cell_size: f64, extent_u: f64, extent_v: f64) -> Option<f64> {
        let cs = if cell_size < Self::MIN_CELL_SIZE {
            Self::MIN_CELL_SIZE
        } else {
            cell_size
        };
        let cols = (extent_u / cs).ceil() as usize + 1;
        let rows = (extent_v / cs).ceil() as usize + 1;
        if rows * cols > Self::MAX_GRID_CELLS {
            let area = extent_u * extent_v;
            Some((area / Self::MAX_GRID_CELLS as f64).sqrt())
        } else {
            None
        }
    }

    /// The cell size a grid over this extent will ACTUALLY use: the request
    /// after the minimum-size floor and the grid-cap coarsening.
    ///
    /// Same arithmetic as [`Self::clamp_cell_size`] but silent, so a caller
    /// can record the effective resolution as measurement provenance without
    /// emitting a second coarsening warning (M1: `SimulationResult::
    /// column_grid_cell_mm` — before it, `resolution_clamped` said *that* the
    /// cell changed and no field said *to what*).
    pub fn effective_cell_size(cell_size: f64, extent_u: f64, extent_v: f64) -> f64 {
        Self::would_exceed_grid(cell_size, extent_u, extent_v)
            .unwrap_or_else(|| cell_size.max(Self::MIN_CELL_SIZE))
    }

    /// Adjust cell_size upward if `rows * cols` would exceed [`Self::MAX_GRID_CELLS`].
    fn clamp_cell_size(mut cell_size: f64, extent_u: f64, extent_v: f64) -> f64 {
        if cell_size < Self::MIN_CELL_SIZE {
            cell_size = Self::MIN_CELL_SIZE;
        }
        let cols = (extent_u / cell_size).ceil() as usize + 1;
        let rows = (extent_v / cell_size).ceil() as usize + 1;
        if rows * cols > Self::MAX_GRID_CELLS {
            // Increase cell_size so total cells fit within the cap.
            let area = extent_u * extent_v;
            let new_cs = (area / Self::MAX_GRID_CELLS as f64).sqrt();
            tracing::warn!(
                requested_cell_size = cell_size,
                clamped_cell_size = new_cs,
                "Dexel grid would exceed {}M cells — coarsening resolution",
                Self::MAX_GRID_CELLS / 1_000_000
            );
            new_cs
        } else {
            cell_size
        }
    }

    /// Create a Z-grid from a bounding box.
    ///
    /// Every ray gets a single segment spanning `[z_min, z_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn z_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size =
            Self::clamp_cell_size(cell_size, bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
        let cols = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.z as f32, bbox.max.z as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        let coverage_max = vec![0.0_f32; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.x,
            origin_v: bbox.min.y,
            cell_size,
            axis: DexelAxis::Z,
            coverage_max,
            conservative_top: vec![bbox.max.z as f32; rows * cols],
        }
    }

    /// Create an X-grid from a bounding box.
    ///
    /// Rays run along X, indexed by (Y, Z).  `rows` = Z-cells, `cols` = Y-cells.
    /// Every ray gets a single segment spanning `[x_min, x_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn x_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size =
            Self::clamp_cell_size(cell_size, bbox.max.y - bbox.min.y, bbox.max.z - bbox.min.z);
        let cols = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.z - bbox.min.z) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.x as f32, bbox.max.x as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        let coverage_max = vec![0.0_f32; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.y,
            origin_v: bbox.min.z,
            cell_size,
            axis: DexelAxis::X,
            coverage_max,
            conservative_top: vec![bbox.max.x as f32; rows * cols],
        }
    }

    /// Create a Y-grid from a bounding box.
    ///
    /// Rays run along Y, indexed by (X, Z).  `rows` = Z-cells, `cols` = X-cells.
    /// Every ray gets a single segment spanning `[y_min, y_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn y_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size =
            Self::clamp_cell_size(cell_size, bbox.max.x - bbox.min.x, bbox.max.z - bbox.min.z);
        let cols = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.z - bbox.min.z) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.y as f32, bbox.max.y as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        let coverage_max = vec![0.0_f32; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.x,
            origin_v: bbox.min.z,
            cell_size,
            axis: DexelAxis::Y,
            coverage_max,
            conservative_top: vec![bbox.max.y as f32; rows * cols],
        }
    }

    /// Convert world (u, v) to cell (row, col).  Returns `None` if outside.
    #[inline]
    pub fn world_to_cell(&self, u: f64, v: f64) -> Option<(usize, usize)> {
        let col_f = (u - self.origin_u) / self.cell_size;
        let row_f = (v - self.origin_v) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return None;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return None;
        }
        Some((row as usize, col as usize))
    }

    /// Convert cell (row, col) to world (u, v) center coordinates.
    #[inline]
    pub fn cell_to_world(&self, row: usize, col: usize) -> (f64, f64) {
        (
            self.origin_u + col as f64 * self.cell_size,
            self.origin_v + row as f64 * self.cell_size,
        )
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Borrow the ray at (row, col).
    #[inline]
    pub fn ray(&self, row: usize, col: usize) -> &DexelRay {
        &self.rays[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mutably borrow the ray at (row, col).
    #[inline]
    pub fn ray_mut(&mut self, row: usize, col: usize) -> &mut DexelRay {
        &mut self.rays[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Top material Z at cell. Returns `None` if the ray is empty.
    ///
    /// Equivalent to `Heightmap::get(row, col)` for cells with material.
    #[inline]
    pub fn top_z_at(&self, row: usize, col: usize) -> Option<f32> {
        let ray = &self.rays[row * self.cols + col];
        ray_top(ray)
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Does any material exist above `z_floor` at this cell?
    #[inline]
    pub fn has_material_above(&self, row: usize, col: usize, z_floor: f32) -> bool {
        let ray = &self.rays[row * self.cols + col];
        ray.iter().any(|seg| seg.exit > z_floor)
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Total material length at cell (sum of all segment lengths along the ray).
    #[inline]
    pub fn material_length_at(&self, row: usize, col: usize) -> f32 {
        ray_material_length(&self.rays[row * self.cols + col])
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Maximum fractional cutter coverage seen at the cell across the
    /// project's stamping history. See `DexelGrid::coverage_max` docstring.
    #[inline]
    pub fn coverage_at(&self, row: usize, col: usize) -> f32 {
        self.coverage_max[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Sliver-safe upper bound on material height anywhere inside this cell.
    /// See [`DexelGrid::conservative_top`] — this is the reading a clearance
    /// ceiling must use, not [`Self::top_z_at`].
    #[inline]
    pub fn conservative_top_at(&self, row: usize, col: usize) -> f32 {
        self.conservative_top[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Lower the sliver-safe bound at a flat cell index — monotone, so a
    /// caller that stamps the same ground twice cannot walk the bound back
    /// up. `surface` must already be an upper bound of the removal surface
    /// across the WHOLE cell, not a cell-centre sample.
    #[inline]
    pub fn lower_conservative_top(&mut self, idx: usize, surface: f32) {
        if surface < self.conservative_top[idx] {
            self.conservative_top[idx] = surface;
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    fn seg(a: f32, b: f32) -> DexelSegment {
        DexelSegment::new(a, b)
    }

    fn ray_from(segs: &[(f32, f32)]) -> DexelRay {
        segs.iter().map(|&(a, b)| seg(a, b)).collect()
    }

    // ── DexelSegment ────────────────────────────────────────────────────

    #[test]
    fn segment_length() {
        let s = seg(2.0, 7.5);
        assert!((s.length() - 5.5).abs() < 1e-6);
    }

    // ── subtract_above ──────────────────────────────────────────────────

    #[test]
    fn subtract_above_single_segment_truncates() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_above(&mut r, 7.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 7.0)]);
    }

    #[test]
    fn subtract_above_removes_entirely_above() {
        let mut r = ray_from(&[(0.0, 3.0), (5.0, 8.0), (9.0, 12.0)]);
        ray_subtract_above(&mut r, 6.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(5.0, 6.0)]);
    }

    #[test]
    fn subtract_above_below_all_is_noop() {
        let mut r = ray_from(&[(0.0, 3.0), (5.0, 8.0)]);
        ray_subtract_above(&mut r, 20.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(5.0, 8.0)]);
    }

    #[test]
    fn subtract_above_at_zero_clears() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_above(&mut r, 0.0);
        assert!(r.is_empty());
    }

    // ── subtract_below ──────────────────────────────────────────────────

    #[test]
    fn subtract_below_single_segment_truncates() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_below(&mut r, 3.0);
        assert_eq!(r.as_slice(), &[seg(3.0, 10.0)]);
    }

    #[test]
    fn subtract_below_removes_entirely_below() {
        let mut r = ray_from(&[(0.0, 3.0), (5.0, 8.0), (9.0, 12.0)]);
        ray_subtract_below(&mut r, 6.0);
        assert_eq!(r.as_slice(), &[seg(6.0, 8.0), seg(9.0, 12.0)]);
    }

    #[test]
    fn subtract_below_above_all_is_noop() {
        let mut r = ray_from(&[(5.0, 8.0)]);
        ray_subtract_below(&mut r, 0.0);
        assert_eq!(r.as_slice(), &[seg(5.0, 8.0)]);
    }

    #[test]
    fn subtract_below_at_top_clears() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_below(&mut r, 10.0);
        assert!(r.is_empty());
    }

    // ── blend_above ─────────────────────────────────────────────────────

    #[test]
    fn blend_above_f_zero_is_noop() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_blend_above(&mut r, 5.0, 0.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 10.0)]);
    }

    #[test]
    fn blend_above_f_one_equals_subtract() {
        let mut r = ray_from(&[(0.0, 10.0), (12.0, 15.0)]);
        let mut sub = r.clone();
        ray_blend_above(&mut r, 7.0, 1.0);
        ray_subtract_above(&mut sub, 7.0);
        assert_eq!(r.as_slice(), sub.as_slice());
    }

    #[test]
    fn blend_above_straddle_half_blends_top_half_of_above() {
        // Segment [0, 10], surface 6.0, f=0.5 → above_part = 4, new exit = 10 - 2 = 8.
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_blend_above(&mut r, 6.0, 0.5);
        assert_eq!(r.len(), 1);
        assert!((r[0].enter - 0.0).abs() < 1e-6);
        assert!((r[0].exit - 8.0).abs() < 1e-6);
    }

    #[test]
    fn blend_above_segment_fully_above_partial_shrink() {
        // Segment [6, 10], surface 4.0, f=0.25 → above_part = 4, new exit = 10 - 1 = 9.
        let mut r = ray_from(&[(6.0, 10.0)]);
        ray_blend_above(&mut r, 4.0, 0.25);
        assert!((r[0].exit - 9.0).abs() < 1e-6);
        assert!((r[0].enter - 6.0).abs() < 1e-6);
    }

    #[test]
    fn blend_above_multi_segment() {
        // surface = 2.0, f = 0.5.
        //   [0,1] fully below — untouched.
        //   [3,5] above_lo=3, above_part=2, new_exit = 5 - 1 = 4.
        //   [6,10] above_lo=6, above_part=4, new_exit = 10 - 2 = 8.
        let mut r = ray_from(&[(0.0, 1.0), (3.0, 5.0), (6.0, 10.0)]);
        ray_blend_above(&mut r, 2.0, 0.5);
        assert_eq!(r.len(), 3);
        assert!((r[0].exit - 1.0).abs() < 1e-6);
        assert!((r[1].exit - 4.0).abs() < 1e-6);
        assert!((r[2].enter - 6.0).abs() < 1e-6 && (r[2].exit - 8.0).abs() < 1e-6);
    }

    #[test]
    fn blend_above_clamps_nan_and_overshoot() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_blend_above(&mut r, 5.0, f32::NAN);
        assert_eq!(r.as_slice(), &[seg(0.0, 10.0)]);
        ray_blend_above(&mut r, 5.0, 1.5);
        // Clamped to 1.0 → equivalent to subtract_above.
        assert_eq!(r.as_slice(), &[seg(0.0, 5.0)]);
    }

    #[test]
    fn blend_above_volume_invariant() {
        // Per-stamp volume removed under blend equals f × pre_above_total.
        let mut r = ray_from(&[(0.0, 10.0), (12.0, 16.0)]);
        let surface = 4.0_f32;
        let pre_above = ray_material_length_above(&r, surface) as f64; // 6 + 4 = 10
        let f = 0.4_f32;
        let pre_total = ray_material_length(&r) as f64;
        ray_blend_above(&mut r, surface, f);
        let post_total = ray_material_length(&r) as f64;
        let removed = pre_total - post_total;
        assert!((removed - (f as f64) * pre_above).abs() < 1e-5);
    }

    // ── blend_below ─────────────────────────────────────────────────────

    #[test]
    fn blend_below_f_zero_is_noop() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_blend_below(&mut r, 5.0, 0.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 10.0)]);
    }

    #[test]
    fn blend_below_f_one_equals_subtract() {
        let mut r = ray_from(&[(0.0, 10.0), (12.0, 15.0)]);
        let mut sub = r.clone();
        ray_blend_below(&mut r, 3.0, 1.0);
        ray_subtract_below(&mut sub, 3.0);
        assert_eq!(r.as_slice(), sub.as_slice());
    }

    #[test]
    fn blend_below_straddle_half() {
        // Segment [0, 10], surface 4.0, f=0.5 → below_part = 4, new enter = 0 + 2 = 2.
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_blend_below(&mut r, 4.0, 0.5);
        assert_eq!(r.len(), 1);
        assert!((r[0].enter - 2.0).abs() < 1e-6);
        assert!((r[0].exit - 10.0).abs() < 1e-6);
    }

    // ── subtract_interval ───────────────────────────────────────────────

    #[test]
    fn subtract_interval_splits_segment() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_interval(&mut r, 3.0, 7.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(7.0, 10.0)]);
    }

    #[test]
    fn subtract_interval_removes_middle() {
        let mut r = ray_from(&[(0.0, 3.0), (5.0, 8.0), (9.0, 12.0)]);
        ray_subtract_interval(&mut r, 4.0, 9.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(9.0, 12.0)]);
    }

    #[test]
    fn subtract_interval_trims_both_ends() {
        let mut r = ray_from(&[(0.0, 5.0), (7.0, 12.0)]);
        ray_subtract_interval(&mut r, 3.0, 9.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(9.0, 12.0)]);
    }

    #[test]
    fn subtract_interval_no_overlap_noop() {
        let mut r = ray_from(&[(0.0, 3.0), (7.0, 10.0)]);
        ray_subtract_interval(&mut r, 4.0, 6.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0), seg(7.0, 10.0)]);
    }

    #[test]
    fn subtract_interval_entire_ray() {
        let mut r = ray_from(&[(2.0, 5.0)]);
        ray_subtract_interval(&mut r, 0.0, 10.0);
        assert!(r.is_empty());
    }

    // ── ray helpers ─────────────────────────────────────────────────────

    #[test]
    fn ray_material_length_multi() {
        let r = ray_from(&[(0.0, 3.0), (5.0, 8.0)]);
        assert!((ray_material_length(&r) - 6.0).abs() < 1e-6);
    }

    #[test]
    fn ray_top_bottom() {
        let r = ray_from(&[(2.0, 5.0), (8.0, 11.0)]);
        assert_eq!(ray_top(&r), Some(11.0));
        assert_eq!(ray_bottom(&r), Some(2.0));
    }

    #[test]
    fn ray_top_bottom_empty() {
        let r: DexelRay = SmallVec::new();
        assert_eq!(ray_top(&r), None);
        assert_eq!(ray_bottom(&r), None);
    }

    // ── DexelGrid ───────────────────────────────────────────────────────

    #[test]
    fn z_grid_from_bounds_dimensions() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, -5.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);
        assert_eq!(grid.cols, 11);
        assert_eq!(grid.rows, 11);
        assert_eq!(grid.rays.len(), 121);
        assert_eq!(grid.axis, DexelAxis::Z);
    }

    #[test]
    fn z_grid_initial_segments() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(5.0, 5.0, 10.0),
        };
        let grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);
        for ray in &grid.rays {
            assert_eq!(ray.len(), 1);
            assert!((ray[0].enter - 0.0).abs() < 1e-6);
            assert!((ray[0].exit - 10.0).abs() < 1e-6);
        }
    }

    #[test]
    fn world_cell_roundtrip() {
        let bbox = BoundingBox3 {
            min: P3::new(10.0, 20.0, 0.0),
            max: P3::new(30.0, 40.0, 5.0),
        };
        let grid = DexelGrid::z_grid_from_bounds(&bbox, 0.5);
        let (u, v) = grid.cell_to_world(4, 6);
        let (row, col) = grid
            .world_to_cell(u, v)
            .expect("cell_to_world output should be inside the grid");
        assert_eq!(row, 4);
        assert_eq!(col, 6);
    }

    #[test]
    fn world_to_cell_out_of_bounds() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);
        assert!(grid.world_to_cell(-1.0, 5.0).is_none());
        assert!(grid.world_to_cell(5.0, 11.0).is_none());
    }

    #[test]
    fn grid_ray_access() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(4.0, 4.0, 10.0),
        };
        let mut grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);
        // Cut one ray
        ray_subtract_above(grid.ray_mut(2, 3), 5.0);
        assert_eq!(grid.ray(2, 3)[0].exit, 5.0);
        // Neighbors untouched
        assert_eq!(grid.ray(2, 2)[0].exit, 10.0);
    }

    // ── Compound operations ─────────────────────────────────────────────

    #[test]
    fn top_then_bottom_cut_leaves_middle() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_above(&mut r, 7.0); // remove top 3mm
        ray_subtract_below(&mut r, 2.0); // remove bottom 2mm
        assert_eq!(r.as_slice(), &[seg(2.0, 7.0)]);
    }

    #[test]
    fn through_cut_empties_ray() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_above(&mut r, 7.0);
        ray_subtract_below(&mut r, 2.0);
        // Now remove the middle
        ray_subtract_interval(&mut r, 2.0, 7.0);
        assert!(r.is_empty());
    }

    #[test]
    fn multiple_subtract_above_idempotent() {
        let mut r = ray_from(&[(0.0, 10.0)]);
        ray_subtract_above(&mut r, 5.0);
        ray_subtract_above(&mut r, 5.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 5.0)]);
        // Cutting deeper
        ray_subtract_above(&mut r, 3.0);
        assert_eq!(r.as_slice(), &[seg(0.0, 3.0)]);
    }

    #[test]
    fn interval_subtract_creates_multi_segment() {
        let mut r = ray_from(&[(0.0, 20.0)]);
        ray_subtract_interval(&mut r, 3.0, 7.0);
        ray_subtract_interval(&mut r, 12.0, 15.0);
        assert_eq!(
            r.as_slice(),
            &[seg(0.0, 3.0), seg(7.0, 12.0), seg(15.0, 20.0)]
        );
    }

    // ── X-grid constructor ─────────────────────────────────────────────

    #[test]
    fn x_grid_from_bounds_dimensions() {
        let bbox = BoundingBox3 {
            min: P3::new(-5.0, 0.0, 0.0),
            max: P3::new(15.0, 10.0, 8.0),
        };
        let grid = DexelGrid::x_grid_from_bounds(&bbox, 1.0);
        // cols = Y-cells, rows = Z-cells
        assert_eq!(grid.cols, 11); // (10-0)/1 + 1
        assert_eq!(grid.rows, 9); // (8-0)/1 + 1
        assert_eq!(grid.rays.len(), 9 * 11);
        assert_eq!(grid.axis, DexelAxis::X);
        assert!((grid.origin_u - 0.0).abs() < 1e-10); // y_min
        assert!((grid.origin_v - 0.0).abs() < 1e-10); // z_min
    }

    #[test]
    fn x_grid_initial_segments_span_x() {
        let bbox = BoundingBox3 {
            min: P3::new(-5.0, 0.0, 0.0),
            max: P3::new(15.0, 10.0, 8.0),
        };
        let grid = DexelGrid::x_grid_from_bounds(&bbox, 1.0);
        for ray in &grid.rays {
            assert_eq!(ray.len(), 1);
            assert!((ray[0].enter - (-5.0_f32)).abs() < 1e-6);
            assert!((ray[0].exit - 15.0).abs() < 1e-6);
        }
    }

    // ── Y-grid constructor ─────────────────────────────────────────────

    #[test]
    fn y_grid_from_bounds_dimensions() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, -3.0, 0.0),
            max: P3::new(10.0, 12.0, 6.0),
        };
        let grid = DexelGrid::y_grid_from_bounds(&bbox, 1.0);
        // cols = X-cells, rows = Z-cells
        assert_eq!(grid.cols, 11); // (10-0)/1 + 1
        assert_eq!(grid.rows, 7); // (6-0)/1 + 1
        assert_eq!(grid.rays.len(), 7 * 11);
        assert_eq!(grid.axis, DexelAxis::Y);
        assert!((grid.origin_u - 0.0).abs() < 1e-10); // x_min
        assert!((grid.origin_v - 0.0).abs() < 1e-10); // z_min
    }

    #[test]
    fn y_grid_initial_segments_span_y() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, -3.0, 0.0),
            max: P3::new(10.0, 12.0, 6.0),
        };
        let grid = DexelGrid::y_grid_from_bounds(&bbox, 1.0);
        for ray in &grid.rays {
            assert_eq!(ray.len(), 1);
            assert!((ray[0].enter - (-3.0_f32)).abs() < 1e-6);
            assert!((ray[0].exit - 12.0).abs() < 1e-6);
        }
    }

    #[test]
    fn x_grid_world_cell_roundtrip() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 5.0, 2.0),
            max: P3::new(20.0, 15.0, 10.0),
        };
        let grid = DexelGrid::x_grid_from_bounds(&bbox, 0.5);
        // u=Y, v=Z for X-grid
        let (u, v) = grid.cell_to_world(3, 7);
        let (row, col) = grid
            .world_to_cell(u, v)
            .expect("cell_to_world output should be inside the grid");
        assert_eq!(row, 3);
        assert_eq!(col, 7);
    }

    #[test]
    fn y_grid_world_cell_roundtrip() {
        let bbox = BoundingBox3 {
            min: P3::new(5.0, 0.0, 2.0),
            max: P3::new(15.0, 20.0, 10.0),
        };
        let grid = DexelGrid::y_grid_from_bounds(&bbox, 0.5);
        // u=X, v=Z for Y-grid
        let (u, v) = grid.cell_to_world(3, 7);
        let (row, col) = grid
            .world_to_cell(u, v)
            .expect("cell_to_world output should be inside the grid");
        assert_eq!(row, 3);
        assert_eq!(col, 7);
    }

    #[test]
    fn zero_cell_size_clamped_z_grid() {
        // cell_size=0 should be clamped to MIN_CELL_SIZE, not cause division by zero.
        // Use a tiny bbox so the clamped cell_size doesn't create a huge grid.
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(1e-5, 1e-5, 1e-5),
        };
        let grid = DexelGrid::z_grid_from_bounds(&bbox, 0.0);
        assert!(
            grid.cell_size > 0.0,
            "cell_size should be clamped above zero"
        );
        assert!(grid.cols > 0);
        assert!(grid.rows > 0);
    }

    #[test]
    fn negative_cell_size_clamped_x_grid() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(1e-5, 1e-5, 1e-5),
        };
        let grid = DexelGrid::x_grid_from_bounds(&bbox, -5.0);
        assert!(
            grid.cell_size > 0.0,
            "cell_size should be clamped above zero"
        );
        assert!(grid.cols > 0);
        assert!(grid.rows > 0);
    }

    #[test]
    fn zero_cell_size_clamped_y_grid() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(1e-5, 1e-5, 1e-5),
        };
        let grid = DexelGrid::y_grid_from_bounds(&bbox, 0.0);
        assert!(
            grid.cell_size > 0.0,
            "cell_size should be clamped above zero"
        );
        assert!(grid.cols > 0);
        assert!(grid.rows > 0);
    }

    // ── DexelGrid query helpers ────────────────────────────────────────

    #[test]
    fn test_top_z_at() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(4.0, 4.0, 10.0),
        };
        let mut grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);

        // Uncut cell should return stock top.
        assert_eq!(grid.top_z_at(0, 0), Some(10.0));

        // Cut a cell and verify top lowered.
        ray_subtract_above(grid.ray_mut(2, 3), 5.0);
        assert_eq!(grid.top_z_at(2, 3), Some(5.0));

        // Neighboring cell untouched.
        assert_eq!(grid.top_z_at(2, 2), Some(10.0));

        // Clear a ray entirely and verify None.
        grid.ray_mut(1, 1).clear();
        assert_eq!(grid.top_z_at(1, 1), None);
    }

    #[test]
    fn test_has_material_above() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(2.0, 2.0, 10.0),
        };
        let mut grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);

        // Full stock: material exists above z=5.
        assert!(grid.has_material_above(0, 0, 5.0));

        // Material does NOT exist above the stock top.
        assert!(!grid.has_material_above(0, 0, 10.0));

        // Cut to z=6: material above 5 still exists (exit=6 > 5).
        ray_subtract_above(grid.ray_mut(1, 1), 6.0);
        assert!(grid.has_material_above(1, 1, 5.0));

        // But no material above 6.
        assert!(!grid.has_material_above(1, 1, 6.0));

        // Empty ray has no material above anything.
        grid.ray_mut(0, 1).clear();
        assert!(!grid.has_material_above(0, 1, 0.0));
    }

    #[test]
    fn test_material_length_at() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(2.0, 2.0, 10.0),
        };
        let mut grid = DexelGrid::z_grid_from_bounds(&bbox, 1.0);

        // Full stock: length = 10.
        assert!((grid.material_length_at(0, 0) - 10.0).abs() < 1e-6);

        // Cut a gap: subtract interval [3, 7] leaves [0,3] + [7,10] = 6.
        crate::dexel::ray_subtract_interval(grid.ray_mut(1, 1), 3.0, 7.0);
        assert!((grid.material_length_at(1, 1) - 6.0).abs() < 1e-6);

        // Empty ray: length = 0.
        grid.ray_mut(0, 1).clear();
        assert!((grid.material_length_at(0, 1) - 0.0).abs() < 1e-6);
    }
}
