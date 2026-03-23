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
        }
    }
}

impl DexelGrid {
    /// Minimum allowed cell size to avoid division-by-zero and degenerate grids.
    const MIN_CELL_SIZE: f64 = 1e-6;

    /// Create a Z-grid from a bounding box.
    ///
    /// Every ray gets a single segment spanning `[z_min, z_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn z_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size = if cell_size > Self::MIN_CELL_SIZE {
            cell_size
        } else {
            Self::MIN_CELL_SIZE
        };
        let cols = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.z as f32, bbox.max.z as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.x,
            origin_v: bbox.min.y,
            cell_size,
            axis: DexelAxis::Z,
        }
    }

    /// Create an X-grid from a bounding box.
    ///
    /// Rays run along X, indexed by (Y, Z).  `rows` = Z-cells, `cols` = Y-cells.
    /// Every ray gets a single segment spanning `[x_min, x_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn x_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size = if cell_size > Self::MIN_CELL_SIZE {
            cell_size
        } else {
            Self::MIN_CELL_SIZE
        };
        let cols = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.z - bbox.min.z) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.x as f32, bbox.max.x as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.y,
            origin_v: bbox.min.z,
            cell_size,
            axis: DexelAxis::X,
        }
    }

    /// Create a Y-grid from a bounding box.
    ///
    /// Rays run along Y, indexed by (X, Z).  `rows` = Z-cells, `cols` = X-cells.
    /// Every ray gets a single segment spanning `[y_min, y_max]`.
    /// `cell_size` is clamped to a minimum of 1e-6 if zero or negative.
    pub fn y_grid_from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        let cell_size = if cell_size > Self::MIN_CELL_SIZE {
            cell_size
        } else {
            Self::MIN_CELL_SIZE
        };
        let cols = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
        let rows = ((bbox.max.z - bbox.min.z) / cell_size).ceil() as usize + 1;
        let seg = DexelSegment::new(bbox.min.y as f32, bbox.max.y as f32);
        let ray: DexelRay = SmallVec::from_buf([seg]);
        let rays = vec![ray; rows * cols];
        Self {
            rays,
            rows,
            cols,
            origin_u: bbox.min.x,
            origin_v: bbox.min.z,
            cell_size,
            axis: DexelAxis::Y,
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
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
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
