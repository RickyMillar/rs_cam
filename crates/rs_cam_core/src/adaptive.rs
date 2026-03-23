//! Adaptive clearing with constant engagement.
//!
//! Generates toolpaths that maintain a target engagement angle by making
//! local decisions about direction at each step. Unlike pocket (contour-parallel)
//! or zigzag (scan-line), adaptive dynamically adjusts the path to keep
//! constant tool load.
//!
//! Algorithm overview (Freesteel/Adaptive2d inspired):
//! 1. Build a material grid from the input polygon
//! 2. Find an entry point on the boundary of uncut material
//! 3. At each step, search for a direction producing target engagement
//! 4. When blocked, find the next uncut region and re-enter
//! 5. Repeat until all material is cleared
//!
//! Reference: research/02_algorithms.md §5

pub(crate) use crate::adaptive_shared::{
    angle_diff, average_angles, blend_corners, refine_angle_bracket, target_engagement_fraction,
};
use crate::debug_trace::{HotspotRecord, ToolpathDebugBounds2, ToolpathDebugContext};
use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
use std::time::Instant;

/// Parameters for adaptive clearing.
pub struct AdaptiveParams {
    pub tool_radius: f64,
    pub stepover: f64,
    pub cut_depth: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
    /// Enable slot clearing: cut a center slot before adaptive spiral.
    /// Reduces linking motion at corners for some pockets.
    pub slot_clearing: bool,
    /// Minimum cutting radius: blend sharp inside corners with arcs of at
    /// least this radius. Prevents chatter on sharp corners. 0.0 = disabled.
    pub min_cutting_radius: f64,
    /// Optional prior stock state. When provided, the material grid is
    /// initialized from the tri-dexel stock so that cells already cleared
    /// by earlier operations are not re-cut.
    pub initial_stock: Option<TriDexelStock>,
}

// ── Material grid ──────────────────────────────────────────────────────

/// 2D boolean grid tracking material presence for engagement calculation.
///
/// Cell values: 0 = outside polygon (air), 1 = uncut material, 2 = cleared.
pub(crate) struct MaterialGrid {
    pub cells: Vec<u8>,
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
    /// Number of cells that are CELL_MATERIAL (tracked incrementally).
    material_count: usize,
    /// Total number of non-air cells.
    total_solid: usize,
}

const CELL_AIR: u8 = 0;
const CELL_MATERIAL: u8 = 1;
const CELL_CLEARED: u8 = 2;

impl MaterialGrid {
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Build a material grid from a polygon. Cells inside the polygon (and
    /// not inside holes) are marked as material.
    pub fn from_polygon(polygon: &Polygon2, cell_size: f64) -> Self {
        let (x_min, y_min, x_max, y_max) = polygon_bbox(&polygon.exterior);
        let margin = cell_size;
        let cols = ((x_max - x_min + 2.0 * margin) / cell_size).ceil() as usize + 1;
        let rows = ((y_max - y_min + 2.0 * margin) / cell_size).ceil() as usize + 1;
        let origin_x = x_min - margin;
        let origin_y = y_min - margin;

        let mut cells = vec![CELL_AIR; rows * cols];
        let mut material_count = 0usize;

        for row in 0..rows {
            let y = origin_y + row as f64 * cell_size;
            for col in 0..cols {
                let x = origin_x + col as f64 * cell_size;
                if polygon.contains_point(&P2::new(x, y)) {
                    cells[row * cols + col] = CELL_MATERIAL;
                    material_count += 1;
                }
            }
        }

        let total_solid = material_count;

        Self {
            cells,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
            material_count,
            total_solid,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Build a boolean grid caching which cells are inside the given polygon.
    /// Used to avoid repeated point-in-polygon calls during direction search.
    pub fn build_machinable_mask(
        polygon: &Polygon2,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
    ) -> Vec<bool> {
        let mut mask = vec![false; rows * cols];
        for row in 0..rows {
            let y = origin_y + row as f64 * cell_size;
            for col in 0..cols {
                let x = origin_x + col as f64 * cell_size;
                if polygon.contains_point(&P2::new(x, y)) {
                    mask[row * cols + col] = true;
                }
            }
        }
        mask
    }

    /// Cell index from world coordinates. Returns None if out of bounds.
    #[inline]
    fn world_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < 0.0 || row_f < 0.0 {
            return None;
        }
        let col = col_f as usize;
        let row = row_f as usize;
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some((row, col))
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Get cell value at world coordinates. Returns CELL_AIR for out-of-bounds.
    #[inline]
    pub fn get_at(&self, x: f64, y: f64) -> u8 {
        match self.world_to_cell(x, y) {
            Some((r, c)) => self.cells[r * self.cols + c],
            None => CELL_AIR,
        }
    }

    /// Check if a position has uncut material.
    #[inline]
    pub fn is_material(&self, x: f64, y: f64) -> bool {
        self.get_at(x, y) == CELL_MATERIAL
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Clear a circle of material (mark as CELL_CLEARED).
    pub fn clear_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        let r_sq = radius * radius;
        let col_min = ((cx - radius - self.origin_x) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let col_max = ((cx + radius - self.origin_x) / self.cell_size).ceil() as usize;
        let row_min = ((cy - radius - self.origin_y) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let row_max = ((cy + radius - self.origin_y) / self.cell_size).ceil() as usize;

        let col_max = col_max.min(self.cols - 1);
        let row_max = row_max.min(self.rows - 1);

        for row in row_min..=row_max {
            let cell_y = self.origin_y + row as f64 * self.cell_size;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > r_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = self.origin_x + col as f64 * self.cell_size;
                let dx = cell_x - cx;
                if dx * dx + dy_sq <= r_sq {
                    let idx = row * self.cols + col;
                    if self.cells[idx] == CELL_MATERIAL {
                        self.cells[idx] = CELL_CLEARED;
                        self.material_count -= 1;
                    }
                }
            }
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mark cells as cleared where the tri-dexel stock surface is below
    /// the cutting depth. This lets the adaptive algorithm skip regions
    /// already machined by prior operations.
    pub fn apply_initial_stock(&mut self, stock: &TriDexelStock, cut_depth: f64) {
        let z_grid = &stock.z_grid;
        let cut_z = cut_depth as f32;
        for row in 0..self.rows {
            let world_y = self.origin_y + row as f64 * self.cell_size;
            for col in 0..self.cols {
                let idx = row * self.cols + col;
                if self.cells[idx] != CELL_MATERIAL {
                    continue;
                }
                let world_x = self.origin_x + col as f64 * self.cell_size;

                // Map world coords to dexel grid cell
                let dexel_col_f = (world_x - z_grid.origin_u) / z_grid.cell_size;
                let dexel_row_f = (world_y - z_grid.origin_v) / z_grid.cell_size;
                if dexel_col_f < 0.0 || dexel_row_f < 0.0 {
                    continue;
                }
                let dexel_col = dexel_col_f as usize;
                let dexel_row = dexel_row_f as usize;
                if dexel_col >= z_grid.cols || dexel_row >= z_grid.rows {
                    continue;
                }

                // If the stock top at this cell is at or below the cutting
                // depth, the material was already removed.
                match z_grid.top_z_at(dexel_row, dexel_col) {
                    Some(top_z) if top_z <= cut_z => {
                        self.cells[idx] = CELL_CLEARED;
                        self.material_count -= 1;
                    }
                    None => {
                        // No material at all in this dexel ray — cleared.
                        self.cells[idx] = CELL_CLEARED;
                        self.material_count -= 1;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Count the fraction of total material cells that remain uncut. O(1).
    pub fn material_fraction(&self) -> f64 {
        if self.total_solid == 0 {
            return 0.0;
        }
        self.material_count as f64 / self.total_solid as f64
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Fast check if a position is inside the machinable region using the cached mask.
    #[inline]
    pub fn is_machinable(&self, mask: &[bool], x: f64, y: f64) -> bool {
        match self.world_to_cell(x, y) {
            Some((r, c)) => mask[r * self.cols + c],
            None => false,
        }
    }

    /// Find the nearest cell with uncut material to the given position.
    /// Uses growing-radius search: starts small, doubles until found.
    /// Returns the world coordinates of the cell center, or None if no material remains.
    pub fn find_nearest_material(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let initial_radius = self.cell_size * 8.0;
        let max_radius =
            (self.cols as f64 * self.cell_size).max(self.rows as f64 * self.cell_size) * 1.5;

        let mut radius = initial_radius;
        while radius <= max_radius {
            if let Some(result) = self.find_nearest_material_in_radius(x, y, radius) {
                return Some(result);
            }
            radius *= 2.0;
        }
        // Final full scan as fallback
        self.find_nearest_material_in_radius(x, y, max_radius)
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Search for nearest material within a given radius from (x, y).
    fn find_nearest_material_in_radius(&self, x: f64, y: f64, radius: f64) -> Option<(f64, f64)> {
        let col_min = ((x - radius - self.origin_x) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let col_max = ((x + radius - self.origin_x) / self.cell_size)
            .ceil()
            .min(self.cols.saturating_sub(1) as f64) as usize;
        let row_min = ((y - radius - self.origin_y) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let row_max = ((y + radius - self.origin_y) / self.cell_size)
            .ceil()
            .min(self.rows.saturating_sub(1) as f64) as usize;

        let mut best_dist_sq = f64::INFINITY;
        let mut best = None;

        for row in row_min..=row_max {
            let cy = self.origin_y + row as f64 * self.cell_size;
            for col in col_min..=col_max {
                if self.cells[row * self.cols + col] != CELL_MATERIAL {
                    continue;
                }
                let cx = self.origin_x + col as f64 * self.cell_size;
                let dx = cx - x;
                let dy = cy - y;
                let d_sq = dx * dx + dy * dy;
                if d_sq < best_dist_sq {
                    best_dist_sq = d_sq;
                    best = Some((cx, cy));
                }
            }
        }
        best
    }

    // ── Boundary distance field ───────────────────────────────────────

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Compute distance-to-boundary for every cell using BFS.
    ///
    /// AIR cells have distance 0; material/cleared cells get their
    /// Manhattan-grid distance to the nearest AIR cell (in world units).
    /// Computed once at startup, O(cells).
    pub fn compute_boundary_distances(&self) -> Vec<f64> {
        let n = self.rows * self.cols;
        let mut dist = vec![f64::INFINITY; n];
        let mut queue = VecDeque::new();

        // Seed: all AIR cells have distance 0
        for row in 0..self.rows {
            for col in 0..self.cols {
                let idx = row * self.cols + col;
                if self.cells[idx] == CELL_AIR {
                    dist[idx] = 0.0;
                    queue.push_back((row, col));
                }
            }
        }

        // BFS (4-connected), uniform edge weight = cell_size
        while let Some((row, col)) = queue.pop_front() {
            let curr = dist[row * self.cols + col];
            let next = curr + self.cell_size;
            for &(dr, dc) in &[(-1i32, 0), (1, 0), (0, -1i32), (0, 1)] {
                let nr = row as i32 + dr;
                let nc = col as i32 + dc;
                if nr < 0 || nc < 0 || nr >= self.rows as i32 || nc >= self.cols as i32 {
                    continue;
                }
                let nidx = nr as usize * self.cols + nc as usize;
                if dist[nidx] == f64::INFINITY {
                    dist[nidx] = next;
                    queue.push_back((nr as usize, nc as usize));
                }
            }
        }

        dist
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Look up boundary distance at world coordinates (nearest cell).
    /// Returns 0.0 for out-of-bounds (treated as on-boundary).
    #[inline]
    pub fn boundary_distance_at(&self, distances: &[f64], x: f64, y: f64) -> f64 {
        match self.world_to_cell(x, y) {
            Some((r, c)) => distances[r * self.cols + c],
            None => 0.0,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Compute gradient of the boundary distance field using central differences.
    /// Returns (gx, gy) pointing away from the nearest boundary (toward interior).
    pub fn boundary_gradient(&self, distances: &[f64], x: f64, y: f64) -> (f64, f64) {
        let (row, col) = match self.world_to_cell(x, y) {
            Some(rc) => rc,
            None => return (0.0, 0.0),
        };
        let get = |r: usize, c: usize| -> f64 {
            if r < self.rows && c < self.cols {
                distances[r * self.cols + c]
            } else {
                0.0
            }
        };
        let gx = if col > 0 && col + 1 < self.cols {
            (get(row, col + 1) - get(row, col - 1)) / (2.0 * self.cell_size)
        } else if col + 1 < self.cols {
            (get(row, col + 1) - get(row, col)) / self.cell_size
        } else if col > 0 {
            (get(row, col) - get(row, col - 1)) / self.cell_size
        } else {
            0.0
        };
        let gy = if row > 0 && row + 1 < self.rows {
            (get(row + 1, col) - get(row - 1, col)) / (2.0 * self.cell_size)
        } else if row + 1 < self.rows {
            (get(row + 1, col) - get(row, col)) / self.cell_size
        } else if row > 0 {
            (get(row, col) - get(row - 1, col)) / self.cell_size
        } else {
            0.0
        };
        (gx, gy)
    }
}

fn polygon_bbox(pts: &[P2]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut y_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for p in pts {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }
    (x_min, y_min, x_max, y_max)
}

// ── Engagement computation ─────────────────────────────────────────────

/// Compute engagement fraction at position (cx, cy) with tool of given radius.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Uses disk-area sampling: counts the fraction of grid cells within the
/// tool circle that contain uncut material. This is more precise than
/// circumference-only sampling (which only measures the engagement angle)
/// because it measures the actual cut area fraction.
///
/// Returns a value in [0.0, 1.0].
pub(crate) fn compute_engagement(grid: &MaterialGrid, cx: f64, cy: f64, radius: f64) -> f64 {
    let r_sq = radius * radius;
    let col_min = ((cx - radius - grid.origin_x) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let col_max = ((cx + radius - grid.origin_x) / grid.cell_size).ceil() as usize;
    let row_min = ((cy - radius - grid.origin_y) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let row_max = ((cy + radius - grid.origin_y) / grid.cell_size).ceil() as usize;

    let col_max = col_max.min(grid.cols.saturating_sub(1));
    let row_max = row_max.min(grid.rows.saturating_sub(1));

    let mut material_cells = 0u32;
    let mut total_cells = 0u32;

    for row in row_min..=row_max {
        let cell_y = grid.origin_y + row as f64 * grid.cell_size;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }
        for col in col_min..=col_max {
            let cell_x = grid.origin_x + col as f64 * grid.cell_size;
            let dx = cell_x - cx;
            if dx * dx + dy_sq <= r_sq {
                total_cells += 1;
                if grid.cells[row * grid.cols + col] == CELL_MATERIAL {
                    material_cells += 1;
                }
            }
        }
    }

    if total_cells == 0 {
        return 0.0;
    }
    material_cells as f64 / total_cells as f64
}

#[derive(Debug, Clone, Copy)]
struct SearchDirectionResult {
    angle: f64,
    evaluations: u32,
}

fn path_bounds(path: &[P2]) -> Option<ToolpathDebugBounds2> {
    let points: Vec<(f64, f64)> = path.iter().map(|point| (point.x, point.y)).collect();
    ToolpathDebugBounds2::from_points(points.iter())
}

// ── Direction search ───────────────────────────────────────────────────

/// Search for the best direction to move from (cx, cy) that produces
/// engagement closest to `target_frac`.
///
/// Three-phase search:
/// 1. **Narrow interpolation** (7 candidates near prev_angle + bracket refinement)
/// 2. **Forward sweep** ±90° (19 candidates) — fallback
/// 3. **Full 360°** (36 candidates) — allows U-turns
///
/// Phase 1 uses history-predicted interpolation: tries a narrow spread
/// around the previous angle, finds engagement brackets (one above target,
/// one below), then linearly interpolates to converge in 2 extra evaluations.
/// This produces smoother paths (continuous angle function) and typically
/// needs only ~10 evaluations instead of 55.
///
/// When near a wall (boundary_distance < 2 × tool_radius), a tangential
/// bias steers the tool along the wall instead of into it.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn search_direction(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    boundary_distances: &[f64],
) -> Option<f64> {
    search_direction_with_metrics(
        grid,
        machinable_mask,
        cx,
        cy,
        tool_radius,
        step_len,
        target_frac,
        prev_angle,
        boundary_distances,
    )
    .map(|result| result.angle)
}

#[allow(clippy::too_many_arguments)]
fn search_direction_with_metrics(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    boundary_distances: &[f64],
) -> Option<SearchDirectionResult> {
    let tolerance = 0.20; // allow ±20% of target
    let min_frac = (target_frac * (1.0 - tolerance)).max(0.005);
    let max_frac = target_frac * (1.0 + tolerance);

    let wall_threshold = 2.0 * tool_radius;
    let mut evaluations = 0u32;

    // Helper: evaluate a candidate angle, returns (angle, engagement, score) or None.
    let mut eval_candidate = |angle: f64| -> Option<(f64, f64, f64)> {
        evaluations += 1;
        let nx = cx + step_len * angle.cos();
        let ny = cy + step_len * angle.sin();

        if !grid.is_machinable(machinable_mask, nx, ny) {
            return None;
        }

        let engagement = compute_engagement(grid, nx, ny, tool_radius);
        if engagement < 0.005 {
            return None;
        }

        let error = (engagement - target_frac).abs();
        let angle_penalty = angle_diff(angle, prev_angle).abs() / PI;

        let wall_bias = {
            let bd = grid.boundary_distance_at(boundary_distances, nx, ny);
            if bd < wall_threshold {
                let (gx, gy) = grid.boundary_gradient(boundary_distances, nx, ny);
                let glen = (gx * gx + gy * gy).sqrt();
                if glen > 1e-10 {
                    let tx = -gy / glen;
                    let ty = gx / glen;
                    let alignment = (angle.cos() * tx + angle.sin() * ty).abs();
                    (1.0 - alignment) * 0.15
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        let score = error + angle_penalty * 0.12 + wall_bias;
        Some((angle, engagement, score))
    };

    // ── Phase 1: Narrow interpolation search ──────────────────────────
    // 7 candidates at ±0°, ±15°, ±30°, ±45° from prev_angle
    {
        let offsets = [
            0.0,
            PI / 12.0,
            -PI / 12.0,
            PI / 6.0,
            -PI / 6.0,
            PI / 4.0,
            -PI / 4.0,
        ];
        let mut best_good: Option<(f64, f64)> = None; // (score, angle)
        let mut lo_bracket: Option<(f64, f64, f64)> = None; // (angle, engagement, score)
        let mut hi_bracket: Option<(f64, f64, f64)> = None; // (angle, engagement, score)

        for &offset in &offsets {
            let angle = prev_angle + offset;
            if let Some((angle, eng, score)) = eval_candidate(angle) {
                // Track engagement brackets for interpolation
                if eng < target_frac {
                    if lo_bracket.is_none_or(|b| eng > b.1) {
                        lo_bracket = Some((angle, eng, score));
                    }
                } else if hi_bracket.is_none_or(|b| eng < b.1) {
                    hi_bracket = Some((angle, eng, score));
                }

                if eng >= min_frac && eng <= max_frac && best_good.is_none_or(|b| score < b.0) {
                    best_good = Some((score, angle));
                }
            }
        }

        if let (Some(lo), Some(hi)) = (lo_bracket, hi_bracket)
            && let Some((angle, eng, score)) =
                refine_angle_bracket(lo, hi, target_frac, 2, &mut eval_candidate)
            && eng >= min_frac
            && eng <= max_frac
            && best_good.is_none_or(|b| score < b.0)
        {
            best_good = Some((score, angle));
        }

        if let Some((_, angle)) = best_good {
            return Some(SearchDirectionResult { angle, evaluations });
        }
    }

    // ── Phase 2: Coarse 360° scan + bracket refinement ────────────────
    // 18 candidates at 20° intervals replaces the old Phase 2 (19 @ ±90°)
    // + Phase 3 (36 @ 360°) = 55 evals. Now ~21 evals total.
    {
        let n_coarse = 18;
        let mut best_good: Option<(f64, f64)> = None; // (score, angle)
        let mut best_any: Option<(f64, f64)> = None;
        let mut coarse_lo: Option<(f64, f64, f64)> = None; // (angle, engagement, score)
        let mut coarse_hi: Option<(f64, f64, f64)> = None; // (angle, engagement, score)

        for i in 0..n_coarse {
            let angle = (i as f64 / n_coarse as f64) * TAU;
            if let Some((angle, eng, score)) = eval_candidate(angle) {
                if eng >= min_frac && eng <= max_frac && best_good.is_none_or(|b| score < b.0) {
                    best_good = Some((score, angle));
                }
                if best_any.is_none_or(|b| score < b.0) {
                    best_any = Some((score, angle));
                }
                if eng < target_frac {
                    if coarse_lo
                        .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                    {
                        coarse_lo = Some((angle, eng, score));
                    }
                } else if coarse_hi
                    .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                {
                    coarse_hi = Some((angle, eng, score));
                }
            }
        }

        if let (Some(lo), Some(hi)) = (coarse_lo, coarse_hi)
            && let Some((angle, eng, score)) =
                refine_angle_bracket(lo, hi, target_frac, 2, eval_candidate)
            && eng >= min_frac
            && eng <= max_frac
            && best_good.is_none_or(|b| score < b.0)
        {
            best_good = Some((score, angle));
        }

        if let Some((_, angle)) = best_good {
            return Some(SearchDirectionResult { angle, evaluations });
        }
        best_any.map(|(_, angle)| SearchDirectionResult { angle, evaluations })
    }
}

// ── Entry point finding ────────────────────────────────────────────────

/// Find the nearest material cell that is not near any of the given endpoints.
/// Uses growing-radius search. Falls back to plain nearest material if
/// everything is near an endpoint.
fn find_nearest_material_spread(
    grid: &MaterialGrid,
    x: f64,
    y: f64,
    pass_endpoints: &[P2],
    min_dist_sq: f64,
) -> Option<(f64, f64)> {
    let initial_radius = grid.cell_size * 8.0;
    let max_radius =
        (grid.cols as f64 * grid.cell_size).max(grid.rows as f64 * grid.cell_size) * 1.5;

    let mut radius = initial_radius;
    while radius <= max_radius {
        if let Some(result) =
            find_nearest_material_spread_in_radius(grid, x, y, pass_endpoints, min_dist_sq, radius)
        {
            return Some(result);
        }
        radius *= 2.0;
    }
    find_nearest_material_spread_in_radius(grid, x, y, pass_endpoints, min_dist_sq, max_radius)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn find_nearest_material_spread_in_radius(
    grid: &MaterialGrid,
    x: f64,
    y: f64,
    pass_endpoints: &[P2],
    min_dist_sq: f64,
    radius: f64,
) -> Option<(f64, f64)> {
    let col_min = ((x - radius - grid.origin_x) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let col_max = ((x + radius - grid.origin_x) / grid.cell_size)
        .ceil()
        .min(grid.cols.saturating_sub(1) as f64) as usize;
    let row_min = ((y - radius - grid.origin_y) / grid.cell_size)
        .floor()
        .max(0.0) as usize;
    let row_max = ((y + radius - grid.origin_y) / grid.cell_size)
        .ceil()
        .min(grid.rows.saturating_sub(1) as f64) as usize;

    let mut best_dist_sq = f64::INFINITY;
    let mut best = None;

    for row in row_min..=row_max {
        let cy = grid.origin_y + row as f64 * grid.cell_size;
        for col in col_min..=col_max {
            if grid.cells[row * grid.cols + col] != CELL_MATERIAL {
                continue;
            }
            let cx = grid.origin_x + col as f64 * grid.cell_size;

            let near = pass_endpoints.iter().any(|ep| {
                let dx = cx - ep.x;
                let dy = cy - ep.y;
                dx * dx + dy * dy < min_dist_sq
            });
            if near {
                continue;
            }

            let dx = cx - x;
            let dy = cy - y;
            let d_sq = dx * dx + dy * dy;
            if d_sq < best_dist_sq {
                best_dist_sq = d_sq;
                best = Some((cx, cy));
            }
        }
    }
    best
}

/// Walk the machinable polygon boundary, sampling engagement at regular
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// intervals. Returns the boundary position with the best engagement
/// that isn't too close to a previous endpoint.
///
/// This is more systematic than grid scanning — it checks positions
/// directly on the tool's legal boundary contour, ensuring no regions
/// are missed. Inspired by Freesteel's EngagePoint boundary traversal.
fn walk_boundary_for_entry(
    boundary: &[P2],
    grid: &MaterialGrid,
    tool_radius: f64,
    step: f64,
    pass_endpoints: &[P2],
    min_endpoint_dist_sq: f64,
) -> Option<(P2, f64)> {
    let mut best: Option<(P2, f64)> = None; // (position, engagement)
    let engage_threshold = 0.005;

    for i in 0..boundary.len() {
        let a = boundary[i];
        let b = boundary[(i + 1) % boundary.len()];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-10 {
            continue;
        }

        let n_samples = (len / step).ceil() as usize;
        for j in 0..=n_samples {
            let t = j as f64 / n_samples.max(1) as f64;
            let x = a.x + t * dx;
            let y = a.y + t * dy;

            // Skip if near a previous endpoint
            let near = pass_endpoints.iter().any(|ep| {
                let ex = x - ep.x;
                let ey = y - ep.y;
                ex * ex + ey * ey < min_endpoint_dist_sq
            });
            if near {
                continue;
            }

            let eng = compute_engagement(grid, x, y, tool_radius);
            if eng > engage_threshold && best.is_none_or(|b| eng > b.1) {
                best = Some((P2::new(x, y), eng));
            }
        }
    }

    best
}

/// Find an entry point by walking the machinable boundary contours.
///
/// Uses systematic boundary traversal: walks the machinable polygon
/// exterior and hole contours, checking engagement at each position.
/// This ensures no uncleared regions along walls are missed.
/// Falls back to grid scan for interior material not reachable from boundary.
pub(crate) fn find_entry_point(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    machinable: &Polygon2,
    tool_radius: f64,
    last_pos: Option<P2>,
    pass_endpoints: &[P2],
) -> Option<P2> {
    let min_endpoint_dist_sq = (tool_radius * 3.0) * (tool_radius * 3.0);
    let walk_step = grid.cell_size * 2.0;

    // Phase 1: Walk the machinable boundary contours
    // Check exterior
    let mut best_boundary: Option<(P2, f64)> = walk_boundary_for_entry(
        &machinable.exterior,
        grid,
        tool_radius,
        walk_step,
        pass_endpoints,
        min_endpoint_dist_sq,
    );

    // Check hole boundaries
    for hole in &machinable.holes {
        if let Some((p, eng)) = walk_boundary_for_entry(
            hole,
            grid,
            tool_radius,
            walk_step,
            pass_endpoints,
            min_endpoint_dist_sq,
        ) && best_boundary.is_none_or(|b| eng > b.1)
        {
            best_boundary = Some((p, eng));
        }
    }

    if let Some((p, _)) = best_boundary {
        return Some(p);
    }

    // Phase 2: Fallback to grid scan for interior material
    let search_from = last_pos.unwrap_or_else(|| {
        let cx = grid.origin_x + (grid.cols as f64 * grid.cell_size) / 2.0;
        let cy = grid.origin_y + (grid.rows as f64 * grid.cell_size) / 2.0;
        P2::new(cx, cy)
    });

    let (mx, my) = if !pass_endpoints.is_empty() {
        find_nearest_material_spread(
            grid,
            search_from.x,
            search_from.y,
            pass_endpoints,
            min_endpoint_dist_sq,
        )
        .or_else(|| grid.find_nearest_material(search_from.x, search_from.y))
    } else {
        grid.find_nearest_material(search_from.x, search_from.y)
    }?;

    if grid.is_machinable(machinable_mask, mx, my) {
        return Some(P2::new(mx, my));
    }

    // Search nearby for a machinable cell
    let search_r = tool_radius * 3.0;
    let step = grid.cell_size;
    let mut best_dist_sq = f64::INFINITY;
    let mut best = None;

    let steps = (search_r / step).ceil() as i32;
    for ri in -steps..=steps {
        let y = my + ri as f64 * step;
        for ci in -steps..=steps {
            let x = mx + ci as f64 * step;
            if grid.is_machinable(machinable_mask, x, y) {
                let engagement = compute_engagement(grid, x, y, tool_radius);
                if engagement > 0.005 {
                    let dx = x - mx;
                    let dy = y - my;
                    let d_sq = dx * dx + dy * dy;
                    if d_sq < best_dist_sq {
                        best_dist_sq = d_sq;
                        best = Some(P2::new(x, y));
                    }
                }
            }
        }
    }

    best
}

// ── Link vs retract ────────────────────────────────────────────────────

/// Check if the straight line from `from` to `to` is safe to traverse at
/// cut depth. The entire path must be within the machinable region, and
/// at most 20% of the path may cross uncut material (thin strips are OK —
/// the tool handles light engagement during a link move).
fn is_clear_path(grid: &MaterialGrid, mask: &[bool], from: P2, to: P2, _tool_radius: f64) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return true;
    }

    let n_steps = (len / (grid.cell_size * 2.0)).ceil() as usize;
    let mut material_hits = 0;
    let mut total = 0;

    for i in 0..=n_steps {
        let t = i as f64 / n_steps.max(1) as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        total += 1;

        // Hard fail: outside machinable region
        if !grid.is_machinable(mask, x, y) {
            return false;
        }
        if grid.is_material(x, y) {
            material_hits += 1;
        }
    }

    // Safe if less than 20% of the path crosses material
    total > 0 && (material_hits as f64 / total as f64) <= 0.2
}

// ── Main adaptive path generation ──────────────────────────────────────

/// A segment of the adaptive path: cutting, rapid reposition, or link (tool-down reposition).
#[derive(Debug, Clone, PartialEq)]
pub enum AdaptiveRuntimeEvent {
    SlotClearing {
        line_index: usize,
        line_total: usize,
    },
    PassEntry {
        pass_index: usize,
        entry_x: f64,
        entry_y: f64,
    },
    PassSummary {
        pass_index: usize,
        step_count: usize,
        idle_count: usize,
        search_evaluations: usize,
        exit_reason: String,
    },
    ForcedClear {
        pass_index: usize,
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    BoundaryCleanup {
        contour_index: usize,
        contour_total: usize,
    },
}

impl AdaptiveRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::SlotClearing {
                line_index,
                line_total,
            } => format!("Slot clearing {line_index}/{line_total}"),
            Self::PassEntry {
                pass_index,
                entry_x,
                entry_y,
            } => format!("Pass {pass_index} — entry at ({entry_x:.1}, {entry_y:.1})"),
            Self::PassSummary {
                pass_index,
                step_count,
                idle_count,
                search_evaluations,
                exit_reason,
            } => format!(
                "Pass {pass_index} — {step_count} steps ({exit_reason}, idle {idle_count}, search {search_evaluations})"
            ),
            Self::ForcedClear {
                pass_index,
                center_x,
                center_y,
                radius,
            } => format!(
                "Pass {pass_index} — forced clear at ({center_x:.1}, {center_y:.1}) r {radius:.1}"
            ),
            Self::BoundaryCleanup {
                contour_index,
                contour_total,
            } => format!("Boundary cleanup {contour_index}/{contour_total}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdaptiveRuntimeAnnotation {
    pub move_index: usize,
    pub event: AdaptiveRuntimeEvent,
}

enum AdaptiveSegment {
    /// Cutting moves: a sequence of 2D points.
    Cut(Vec<P2>),
    /// Rapid reposition to a new entry point (retract → rapid → plunge).
    Rapid(P2),
    /// Link move: reposition at cut depth without retracting (cleared path).
    Link(P2),
    /// Structured runtime marker at the current point in the toolpath.
    Marker(AdaptiveRuntimeEvent),
}

/// Generate the 2D adaptive clearing path segments.
#[allow(dead_code)]
fn adaptive_segments(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    tolerance: f64,
    slot_clearing: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let params = AdaptiveParams {
        tool_radius,
        stepover,
        tolerance,
        slot_clearing,
        cut_depth: 0.0,
        feed_rate: 0.0,
        plunge_rate: 0.0,
        safe_z: 0.0,
        min_cutting_radius: 0.0,
        initial_stock: None,
    };
    adaptive_segments_with_debug(polygon, &params, cancel, None)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate 2D adaptive segments and optionally record detailed debug spans.
fn adaptive_segments_with_debug(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let tool_radius = params.tool_radius;
    let stepover = params.stepover;
    let tolerance = params.tolerance;
    let slot_clearing = params.slot_clearing;
    let cut_depth = params.cut_depth;
    // Inset polygon by tool radius to get the machinable region
    let machinable_vec = offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return Ok(Vec::new());
    }
    let machinable = &machinable_vec[0];

    // Build material grid from the original polygon (not inset)
    let cell_size = (tool_radius / 6.0).max(tolerance);
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);

    // If prior stock state is available, mark cells already cleared by
    // earlier operations so the adaptive algorithm does not re-cut them.
    if let Some(ref stock) = params.initial_stock {
        grid.apply_initial_stock(stock, cut_depth);
    }

    // Cache the machinable region as a boolean mask for fast lookups
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );

    // Precompute boundary distance field for wall-tangent bias
    let boundary_distances = grid.compute_boundary_distances();

    let target_frac = target_engagement_fraction(stepover, tool_radius);
    let step_len = cell_size * 1.5;
    let mut segments = Vec::new();
    let mut last_pos: Option<P2> = None;
    let mut pass_endpoints: Vec<P2> = Vec::new();

    // ── Slot clearing (Fusion-style first pass) ───────────────────────
    if slot_clearing {
        let slot_scope = debug.map(|ctx| ctx.start_span("slot_clearing", "Slot clearing"));
        let (x_min, y_min, x_max, y_max) = polygon_bbox(&polygon.exterior);
        let w = x_max - x_min;
        let h = y_max - y_min;
        // Slot along the longest axis
        let slot_angle = if w >= h { 0.0 } else { 90.0 };
        // Use large stepover to get a single center line
        let perp_span = if w >= h { h } else { w };
        let slot_lines = crate::zigzag::zigzag_lines(polygon, tool_radius, perp_span, slot_angle);

        for (line_idx, line) in slot_lines.iter().enumerate() {
            check_cancel(cancel)?;
            segments.push(AdaptiveSegment::Marker(
                AdaptiveRuntimeEvent::SlotClearing {
                    line_index: line_idx + 1,
                    line_total: slot_lines.len(),
                },
            ));
            segments.push(AdaptiveSegment::Rapid(line[0]));

            // Walk along the line and clear material in the grid
            let dx = line[1].x - line[0].x;
            let dy = line[1].y - line[0].y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 0..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = line[0].x + t * dx;
                let y = line[0].y + t * dy;
                grid.clear_circle(x, y, tool_radius);
            }

            segments.push(AdaptiveSegment::Cut(vec![line[0], line[1]]));
            last_pos = Some(line[1]);
        }
        if let Some(scope) = slot_scope.as_ref() {
            scope.set_counter("line_count", slot_lines.len() as f64);
        }
    }

    // ── Adaptive passes ───────────────────────────────────────────────
    let max_passes = 500; // safety limit
    let mut pass_count = 0;

    while grid.material_fraction() > 0.01 && pass_count < max_passes {
        check_cancel(cancel)?;
        pass_count += 1;

        let pass_started = Instant::now();
        let material_before = grid.material_fraction();
        let pass_scope =
            debug.map(|ctx| ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(cut_depth);
            scope.set_counter("material_fraction_before", material_before);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());

        // Find entry point (spread away from previous endpoints)
        let entry_scope = pass_ctx
            .as_ref()
            .map(|ctx| ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let entry = match find_entry_point(
            &grid,
            &machinable_mask,
            machinable,
            tool_radius,
            last_pos,
            &pass_endpoints,
        ) {
            Some(p) => p,
            None => {
                if let Some(scope) = pass_scope.as_ref() {
                    scope.set_exit_reason("no entry");
                    scope.set_counter("pass_index", pass_count as f64);
                }
                break;
            }
        };
        if let Some(scope) = entry_scope.as_ref() {
            scope.set_xy_bbox(ToolpathDebugBounds2 {
                min_x: entry.x,
                max_x: entry.x,
                min_y: entry.y,
                max_y: entry.y,
            });
        }

        // Link or retract to entry point
        let max_link_dist = tool_radius * 6.0; // ~3 tool diameters
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: pass_count,
            entry_x: entry.x,
            entry_y: entry.y,
        }));
        if let Some(last) = last_pos {
            let dx = entry.x - last.x;
            let dy = entry.y - last.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < max_link_dist
                && is_clear_path(&grid, &machinable_mask, last, entry, tool_radius)
            {
                segments.push(AdaptiveSegment::Link(entry));
            } else {
                segments.push(AdaptiveSegment::Rapid(entry));
            }
        } else {
            segments.push(AdaptiveSegment::Rapid(entry));
        }

        // Walk the adaptive path from this entry
        let mut path = vec![entry];
        let mut cx = entry.x;
        let mut cy = entry.y;

        // Initial direction: toward nearest material
        let mut prev_angle = if let Some(pos) = last_pos {
            (entry.y - pos.y).atan2(entry.x - pos.x)
        } else if let Some((mx, my)) = grid.find_nearest_material(cx, cy) {
            (my - cy).atan2(mx - cx)
        } else {
            0.0
        };

        // Clear material at entry position
        grid.clear_circle(cx, cy, tool_radius);

        // Direction smoothing buffer (gyro) — average last N directions
        // for smooth curves instead of jagged steps. Inspired by Freesteel.
        const SMOOTH_BUF_LEN: usize = 3;
        let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

        let max_steps = 5000;
        let mut idle_count = 0;
        let mut search_evaluations = 0u32;
        for _ in 0..max_steps {
            check_cancel(cancel)?;
            let before = grid.material_count;

            // Smoothed direction: average recent angles for prev_angle hint
            let smoothed_angle = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            // Search for next direction
            let search_result = match search_direction_with_metrics(
                &grid,
                &machinable_mask,
                cx,
                cy,
                tool_radius,
                step_len,
                target_frac,
                smoothed_angle,
                &boundary_distances,
            ) {
                Some(result) => result,
                None => break,
            };
            search_evaluations += search_result.evaluations;
            let angle = search_result.angle;

            // Move in that direction
            cx += step_len * angle.cos();
            cy += step_len * angle.sin();
            path.push(P2::new(cx, cy));

            // Clear material at new position
            grid.clear_circle(cx, cy, tool_radius);

            // Update direction smoothing buffer
            if angle_buf.len() >= SMOOTH_BUF_LEN {
                angle_buf.remove(0);
            }
            angle_buf.push(angle);

            // Idle detection: if no material was cleared for many steps, we're
            // going in circles over already-cleared area.
            if grid.material_count == before {
                idle_count += 1;
                if idle_count > 15 {
                    break;
                }
            } else {
                idle_count = 0;
            }

            prev_angle = angle;
        }

        let was_idle = idle_count > 15;
        let exit_reason = if was_idle { "idle" } else { "no direction" };

        let path_len = path.len();
        let path_debug_bounds = path_bounds(&path);

        if path_len >= 2 {
            // SAFETY: path.len() >= 2 checked on line above
            #[allow(clippy::expect_used)]
            let endpoint = *path.last().expect("path is non-empty after loop");
            last_pos = Some(endpoint);
            pass_endpoints.push(endpoint);
            segments.push(AdaptiveSegment::Cut(path));
        } else {
            last_pos = Some(entry);
            pass_endpoints.push(entry);
        }

        // If the pass ended due to idle detection, the remaining material
        // nearby is too small or inaccessible. Force-clear a wider area
        // around the last position to prevent revisiting the same spot.
        if was_idle {
            let forced_clear_scope = pass_ctx
                .as_ref()
                .map(|ctx| ctx.start_span("forced_clear", format!("Forced clear {pass_count}")));
            grid.clear_circle(cx, cy, tool_radius * 2.0);
            segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::ForcedClear {
                pass_index: pass_count,
                center_x: cx,
                center_y: cy,
                radius: tool_radius * 2.0,
            }));
            if let Some(scope) = forced_clear_scope.as_ref() {
                scope.set_xy_bbox(ToolpathDebugBounds2 {
                    min_x: cx - tool_radius * 2.0,
                    max_x: cx + tool_radius * 2.0,
                    min_y: cy - tool_radius * 2.0,
                    max_y: cy + tool_radius * 2.0,
                });
                scope.set_z_level(cut_depth);
            }
        }

        if let Some(scope) = pass_scope.as_ref() {
            scope.set_counter("pass_index", pass_count as f64);
            scope.set_counter("step_count", path_len as f64);
            scope.set_counter("idle_count", idle_count as f64);
            scope.set_counter("search_evaluations", search_evaluations as f64);
            scope.set_counter("material_fraction_after", grid.material_fraction());
            scope.set_exit_reason(exit_reason);
            if let Some(bounds) = path_debug_bounds {
                scope.set_xy_bbox(bounds);
                let (center_x, center_y) = bounds.center();
                if let Some(ctx) = pass_ctx.as_ref() {
                    ctx.record_hotspot(&HotspotRecord {
                        kind: "adaptive_pass".into(),
                        center_x,
                        center_y,
                        z_level: Some(cut_depth),
                        bucket_size_xy: tool_radius * 2.0,
                        bucket_size_z: Some(tolerance.max(step_len)),
                        elapsed_us: pass_started.elapsed().as_micros() as u64,
                        pass_count: 1,
                        step_count: path_len as u64,
                        low_yield_exit_count: 0,
                    });
                }
            }
            scope.set_z_level(cut_depth);
        }
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassSummary {
            pass_index: pass_count,
            step_count: path_len,
            idle_count,
            search_evaluations: search_evaluations as usize,
            exit_reason: exit_reason.to_string(),
        }));
    }

    // ── Boundary cleanup pass ─────────────────────────────────────────
    // Trace ALL machinable boundaries (exterior + hole contours) to sweep
    // any thin strip of material left along the walls. This is the
    // tool-center contour that puts the tool edge right on each wall.
    let mut contours: Vec<&Vec<P2>> = Vec::new();
    if machinable.exterior.len() >= 3 {
        contours.push(&machinable.exterior);
    }
    for hole in &machinable.holes {
        if hole.len() >= 3 {
            contours.push(hole);
        }
    }

    let cleanup_scope = debug.map(|ctx| ctx.start_span("boundary_cleanup", "Boundary cleanup"));
    for (contour_idx, boundary) in contours.iter().enumerate() {
        check_cancel(cancel)?;
        segments.push(AdaptiveSegment::Marker(
            AdaptiveRuntimeEvent::BoundaryCleanup {
                contour_index: contour_idx + 1,
                contour_total: contours.len(),
            },
        ));
        segments.push(AdaptiveSegment::Rapid(boundary[0]));

        let mut cleanup_path = vec![boundary[0]];
        // Walk the contour, clearing material and interpolating between
        // vertices so no cells are missed on long edges.
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 1..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = a.x + t * dx;
                let y = a.y + t * dy;
                grid.clear_circle(x, y, tool_radius);
                cleanup_path.push(P2::new(x, y));
            }
        }
        // Close the loop back to the start
        grid.clear_circle(boundary[0].x, boundary[0].y, tool_radius);
        cleanup_path.push(boundary[0]);
        segments.push(AdaptiveSegment::Cut(cleanup_path));
    }
    if let Some(scope) = cleanup_scope.as_ref() {
        scope.set_counter("contour_count", contours.len() as f64);
        scope.set_z_level(cut_depth);
    }

    Ok(segments)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Simplify a path using the Douglas-Peucker algorithm.
pub(crate) fn simplify_path(points: &[P2], tolerance: f64) -> Vec<P2> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    // Find the point farthest from the line between first and last
    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let line_len = (dx * dx + dy * dy).sqrt();

    let mut max_dist = 0.0;
    let mut max_idx = 0;

    if line_len > 1e-10 {
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let d = ((pt.x - first.x) * dy - (pt.y - first.y) * dx).abs() / line_len;
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    } else {
        // Degenerate case: all points are close together
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let ddx = pt.x - first.x;
            let ddy = pt.y - first.y;
            let d = (ddx * ddx + ddy * ddy).sqrt();
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    }

    if max_dist > tolerance {
        let mut left = simplify_path(&points[..=max_idx], tolerance);
        let right = simplify_path(&points[max_idx..], tolerance);
        left.pop(); // Remove duplicate junction point
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

// ── Public API ─────────────────────────────────────────────────────────

fn segments_to_toolpath(
    segments: &[AdaptiveSegment],
    params: &AdaptiveParams,
) -> (Toolpath, Vec<AdaptiveRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    for segment in segments {
        match segment {
            AdaptiveSegment::Marker(event) => {
                annotations.push(AdaptiveRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            AdaptiveSegment::Rapid(entry) => {
                tp.rapid_to(crate::geo::P3::new(entry.x, entry.y, params.safe_z));
                tp.feed_to(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.plunge_rate,
                );
            }
            AdaptiveSegment::Link(entry) => {
                tp.feed_to(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.feed_rate,
                );
            }
            AdaptiveSegment::Cut(path) => {
                let simplified = simplify_path(path, params.tolerance);
                let final_path = if params.min_cutting_radius > 0.0 {
                    blend_corners(&simplified, params.min_cutting_radius)
                } else {
                    simplified
                };
                for p in final_path.iter().skip(1) {
                    tp.feed_to(
                        crate::geo::P3::new(p.x, p.y, params.cut_depth),
                        params.feed_rate,
                    );
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to(crate::geo::P3::new(
            last.target.x,
            last.target.y,
            params.safe_z,
        ));
    }

    (tp, annotations)
}

fn runtime_annotations_to_labels(
    annotations: &[AdaptiveRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

/// Generate an adaptive clearing toolpath for a 2D polygon region.
///
/// The toolpath maintains approximately constant engagement by dynamically
/// adjusting direction at each step. Returns a Toolpath with rapids,
/// plunges, and feeds at the specified cut_depth.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn adaptive_toolpath(polygon: &Polygon2, params: &AdaptiveParams) -> Toolpath {
    let never_cancel = || false;
    adaptive_toolpath_with_cancel(polygon, params, &never_cancel)
        .expect("non-cancellable adaptive should never be cancelled")
}

pub fn adaptive_toolpath_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    adaptive_toolpath_traced_with_cancel(polygon, params, cancel, None)
}

pub fn adaptive_toolpath_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) =
        adaptive_toolpath_structured_annotated_traced_with_cancel(polygon, params, cancel, debug)?;
    Ok(tp)
}

pub fn adaptive_toolpath_structured_annotated_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<AdaptiveRuntimeAnnotation>), Cancelled> {
    let segments = adaptive_segments_with_debug(polygon, params, cancel, debug)?;
    let (tp, annotations) = segments_to_toolpath(&segments, params);
    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }
    Ok((tp, annotations))
}

pub fn adaptive_toolpath_annotated_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    let (tp, annotations) =
        adaptive_toolpath_structured_annotated_traced_with_cancel(polygon, params, cancel, debug)?;
    Ok((tp, runtime_annotations_to_labels(&annotations)))
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

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params(tool_radius: f64, stepover: f64) -> AdaptiveParams {
        AdaptiveParams {
            tool_radius,
            stepover,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.2,
            slot_clearing: false,
            min_cutting_radius: 0.0,
            initial_stock: None,
        }
    }

    // ── MaterialGrid tests ─────────────────────────────────────────────

    #[test]
    fn test_material_grid_from_square() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 1.0);

        // Center should be material
        assert!(grid.is_material(0.0, 0.0));
        // Outside should be air
        assert!(!grid.is_material(15.0, 0.0));
        assert!(!grid.is_material(0.0, 15.0));
    }

    #[test]
    fn test_material_grid_with_hole() {
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(square_polygon(20.0).exterior, vec![hole]);
        let grid = MaterialGrid::from_polygon(&poly, 0.5);

        // Outside should be air
        assert!(!grid.is_material(15.0, 0.0));
        // Inside hole should be air
        assert!(!grid.is_material(0.0, 0.0));
        // Between hole and exterior should be material
        assert!(grid.is_material(7.0, 0.0));
    }

    #[test]
    fn test_material_grid_clear_circle() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        assert!(grid.is_material(0.0, 0.0));
        grid.clear_circle(0.0, 0.0, 3.0);
        assert!(!grid.is_material(0.0, 0.0));
        assert!(!grid.is_material(2.0, 0.0));

        // Far away should still be material
        assert!(grid.is_material(7.0, 7.0));
    }

    #[test]
    fn test_material_fraction_starts_at_one() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 1.0);
        assert!(grid.material_fraction() > 0.95);
    }

    #[test]
    fn test_material_fraction_decreases_after_clear() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        let before = grid.material_fraction();
        grid.clear_circle(0.0, 0.0, 5.0);
        let after = grid.material_fraction();
        assert!(
            after < before,
            "Material fraction should decrease: {} -> {}",
            before,
            after
        );
    }

    #[test]
    fn test_find_nearest_material() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear center
        grid.clear_circle(0.0, 0.0, 5.0);

        // Nearest material from center should be ~5mm away
        let (mx, my) = grid.find_nearest_material(0.0, 0.0).unwrap();
        let dist = (mx * mx + my * my).sqrt();
        assert!(
            dist > 4.0 && dist < 7.0,
            "Nearest material should be ~5mm away, got {}",
            dist
        );
    }

    // ── Boundary distance tests ───────────────────────────────────────

    #[test]
    fn test_boundary_distance_center_vs_edge() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        let center_dist = grid.boundary_distance_at(&dist, 0.0, 0.0);
        let edge_dist = grid.boundary_distance_at(&dist, 9.0, 0.0);

        assert!(
            center_dist > edge_dist,
            "Center ({:.1}) should be farther from boundary than edge ({:.1})",
            center_dist,
            edge_dist
        );
        // Center of 20x20 square: ~10 cells from boundary at 0.5 cell_size = ~5.0
        assert!(
            center_dist > 4.0,
            "Center distance should be significant, got {:.1}",
            center_dist
        );
        // Near edge (9.0 from center, wall at 10.0): ~1mm from boundary
        assert!(
            edge_dist < 3.0,
            "Edge distance should be small, got {:.1}",
            edge_dist
        );
    }

    #[test]
    fn test_boundary_distance_air_is_zero() {
        let sq = square_polygon(10.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        // Well outside the polygon → AIR → distance 0
        let air_dist = grid.boundary_distance_at(&dist, 20.0, 20.0);
        assert!(
            air_dist < 0.01,
            "AIR cell should have distance 0, got {}",
            air_dist
        );
    }

    #[test]
    fn test_boundary_gradient_points_inward() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        // Near the right wall (x ≈ 9): gradient should point left (negative x)
        let (gx, _gy) = grid.boundary_gradient(&dist, 9.0, 0.0);
        // Gradient points toward increasing distance = away from wall = inward
        // But we're near the right wall, so inward = negative x? Actually no:
        // gradient points in the direction of increasing distance, which is toward
        // the interior. At x=9 (near right wall at x=10), increasing distance is
        // toward the left (negative x direction).
        // Wait - the boundary distance increases as you move AWAY from the wall.
        // So the gradient points away from the wall = toward interior.
        // At x=9 near the right wall: gradient x should be negative (pointing left = inward).
        // Actually let me think again. The wall is air at x>10. Distance increases as you
        // go from x=10 toward x=0 (away from the air boundary). So at x=9, the gradient
        // should point toward x=0, which is negative x.
        assert!(
            gx < -0.1,
            "Near right wall, gradient x should be negative (inward), got {:.2}",
            gx
        );
    }

    // ── Engagement computation tests ───────────────────────────────────

    #[test]
    fn test_engagement_full_material() {
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Center of large square, small tool → should be ~1.0
        let eng = compute_engagement(&grid, 0.0, 0.0, 3.0);
        assert!(
            eng > 0.9,
            "Fully surrounded should have near-1.0 engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_engagement_no_material() {
        let sq = square_polygon(10.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Far outside
        let eng = compute_engagement(&grid, 50.0, 50.0, 3.0);
        assert!(
            eng < 0.01,
            "No material should have 0 engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_engagement_partial() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear a channel through center
        for i in -20..=20 {
            let x = i as f64 * 0.5;
            grid.clear_circle(x, 0.0, 2.0);
        }

        // Engagement at the edge of the channel should be partial
        let eng = compute_engagement(&grid, 0.0, 2.5, 3.0);
        assert!(
            eng > 0.1 && eng < 0.9,
            "Edge of channel should have partial engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_target_engagement_fraction() {
        // 20% stepover on 3.175mm radius tool
        let frac = target_engagement_fraction(1.27, 3.175);
        assert!(
            frac > 0.05 && frac < 0.25,
            "20% stepover should give small engagement fraction, got {}",
            frac
        );

        // Full slot (WOC = diameter) → engagement should be 0.5 (half circle)
        let frac_full = target_engagement_fraction(6.35, 3.175);
        assert!(
            (frac_full - 0.5).abs() < 0.01,
            "Full slot should give 0.5 engagement fraction, got {}",
            frac_full
        );
    }

    // ── Direction search tests ─────────────────────────────────────────

    #[test]
    fn test_search_direction_finds_material() {
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Machinable = inset by tool radius
        let machinable = offset_polygon(&sq, 3.0);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let target = target_engagement_fraction(1.5, 3.0);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            3.0,
            1.0,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find a direction in open material");
    }

    #[test]
    fn test_search_direction_blocked_outside() {
        let sq = square_polygon(10.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Clear everything
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                if grid.cells[row * grid.cols + col] == CELL_MATERIAL {
                    grid.cells[row * grid.cols + col] = CELL_CLEARED;
                }
            }
        }

        let machinable = offset_polygon(&sq, 2.0);
        if machinable.is_empty() {
            return; // polygon too small for tool
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let target = target_engagement_fraction(1.0, 2.0);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            2.0,
            0.5,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(
            angle.is_none(),
            "Should be blocked when no material remains"
        );
    }

    #[test]
    fn test_search_direction_wall_tangent_bias_applied() {
        // Verify that the wall-tangent bias adds a scoring penalty for
        // perpendicular movement near walls. We test the boundary distance
        // and gradient mechanics rather than the full search outcome
        // (which depends on engagement differences too).
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Near the left wall at x=-9 (wall at x=-10): boundary_distance < 2*tool_radius
        let bd = grid.boundary_distance_at(&boundary_dist, -9.0, 0.0);
        assert!(
            bd < 4.0,
            "Near wall, boundary distance should be small, got {:.1}",
            bd
        );

        // Gradient should point away from the wall (positive x = inward)
        let (gx, _gy) = grid.boundary_gradient(&boundary_dist, -9.0, 0.0);
        assert!(
            gx > 0.1,
            "Near left wall, gradient should point right (inward), got gx={:.2}",
            gx
        );

        // Verify search_direction works near a wall (finds a direction)
        let machinable = offset_polygon(&sq, 2.0);
        if machinable.is_empty() {
            return;
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let target = target_engagement_fraction(1.5, 2.0);
        let angle = search_direction(
            &grid,
            &mask,
            -7.0,
            0.0,
            2.0,
            1.0,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find a direction near wall");
    }

    // ── Entry point spreading tests ───────────────────────────────────

    #[test]
    fn test_entry_points_spread() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        if machinable.is_empty() {
            return;
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // First entry: no previous endpoints
        let e1 = find_entry_point(&grid, &mask, &machinable[0], tool_radius, None, &[]);
        assert!(e1.is_some());
        let e1 = e1.unwrap();

        // Second entry: should avoid being close to the first
        let e2 = find_entry_point(&grid, &mask, &machinable[0], tool_radius, Some(e1), &[e1]);
        assert!(e2.is_some());
        let e2 = e2.unwrap();

        let dx = e2.x - e1.x;
        let dy = e2.y - e1.y;
        let dist = (dx * dx + dy * dy).sqrt();
        // The second entry should be at least some distance from the first
        // (not right on top of it, though it may still be nearby if material is concentrated)
        assert!(
            dist > 0.1,
            "Second entry should be spread from first, dist={:.1}",
            dist
        );
    }

    // ── Path simplification tests ──────────────────────────────────────

    #[test]
    fn test_simplify_straight_line() {
        let pts: Vec<P2> = (0..=10).map(|i| P2::new(i as f64, 0.0)).collect();
        let simplified = simplify_path(&pts, 0.01);
        assert_eq!(
            simplified.len(),
            2,
            "Straight line should simplify to 2 points"
        );
    }

    #[test]
    fn test_simplify_preserves_corners() {
        let pts = vec![
            P2::new(0.0, 0.0),
            P2::new(5.0, 0.0),
            P2::new(5.0, 5.0),
            P2::new(10.0, 5.0),
        ];
        let simplified = simplify_path(&pts, 0.1);
        assert!(simplified.len() >= 3, "L-shape should preserve the corner");
    }

    // ── Blend corners tests ────────────────────────────────────────────

    #[test]
    fn test_blend_corners_sharp_turn() {
        // L-shape: 90° turn
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let blended = blend_corners(&path, 2.0);
        // Should add arc points at the corner
        assert!(
            blended.len() > 3,
            "90° corner should get blend points, got {} points",
            blended.len()
        );
        // First and last points should be preserved
        assert!((blended[0].x - 0.0).abs() < 1e-10);
        assert!((blended.last().unwrap().y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_blend_corners_straight_line_unchanged() {
        let path = vec![P2::new(0.0, 0.0), P2::new(5.0, 0.0), P2::new(10.0, 0.0)];
        let blended = blend_corners(&path, 2.0);
        // Nearly straight → no blending, should be 3 points (start, corner, end)
        assert_eq!(blended.len(), 3, "Straight line should not be blended");
    }

    #[test]
    fn test_blend_corners_disabled_when_zero() {
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let blended = blend_corners(&path, 0.0);
        assert_eq!(blended.len(), path.len(), "Zero radius should not blend");
    }

    #[test]
    fn test_blend_corners_radius_too_large() {
        // Very short segments, large radius → setback won't fit
        let path = vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)];
        let blended = blend_corners(&path, 10.0);
        // Radius too large for the segments → corner preserved unblended
        assert_eq!(
            blended.len(),
            3,
            "Too-large radius should not blend short segments"
        );
    }

    // ── Slot clearing tests ────────────────────────────────────────────

    #[test]
    fn test_slot_clearing_reduces_material() {
        let sq = square_polygon(20.0);
        let tool_radius = 2.5;
        let cell_size = 0.5;

        // Without slot clearing
        let grid_no_slot = MaterialGrid::from_polygon(&sq, cell_size);
        let frac_before = grid_no_slot.material_fraction();

        // With slot clearing: run adaptive_segments and check material after slot pass
        let never_cancel = || false;
        let segs = adaptive_segments(&sq, tool_radius, 1.2, 0.2, true, &never_cancel)
            .expect("test helper should not cancel");

        // Verify we got at least one cut segment (the slot)
        let cut_count = segs
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Cut(_)))
            .count();
        assert!(
            cut_count >= 1,
            "Slot clearing should produce at least one cut segment"
        );

        // Replay just the first cut segment to verify it clears material
        let mut grid = MaterialGrid::from_polygon(&sq, cell_size);
        if let Some(AdaptiveSegment::Cut(path)) =
            segs.iter().find(|s| matches!(s, AdaptiveSegment::Cut(_)))
        {
            for p in path {
                grid.clear_circle(p.x, p.y, tool_radius);
            }
        }
        let frac_after_slot = grid.material_fraction();
        assert!(
            frac_after_slot < frac_before,
            "Slot should clear material: {:.1}% → {:.1}%",
            frac_before * 100.0,
            frac_after_slot * 100.0
        );
    }

    // ── Full adaptive toolpath tests ───────────────────────────────────

    #[test]
    fn test_adaptive_toolpath_basic() {
        let sq = square_polygon(16.0);
        let params = default_params(2.5, 1.2);

        let tp = adaptive_toolpath(&sq, &params);

        // Should have moves
        assert!(
            tp.moves.len() > 10,
            "Adaptive should generate moves, got {}",
            tp.moves.len()
        );

        // Should have some cutting distance
        assert!(
            tp.total_cutting_distance() > 20.0,
            "Should have significant cutting, got {}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_toolpath_all_at_cut_depth() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.cut_depth = -5.0;

        let tp = adaptive_toolpath(&sq, &params);

        // All feed moves should be at cut_depth
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { feed_rate } = m.move_type
                && feed_rate > 500.0
            {
                // cutting move (not plunge)
                assert!(
                    (m.target.z - (-5.0)).abs() < 1e-10,
                    "Cutting move should be at cut_depth, got z={}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn test_adaptive_too_small_polygon() {
        // Polygon smaller than tool
        let sq = square_polygon(3.0);
        let params = default_params(3.0, 1.5);

        let tp = adaptive_toolpath(&sq, &params);
        // Should gracefully return empty or minimal toolpath
        assert!(
            tp.moves.len() <= 2,
            "Too-small polygon should produce minimal toolpath"
        );
    }

    #[test]
    fn test_adaptive_clears_most_material() {
        let sq = square_polygon(16.0);
        let cell_size = 0.5;
        let tool_radius = 2.5;

        let never_cancel = || false;
        let segments = adaptive_segments(&sq, tool_radius, 1.2, 0.2, false, &never_cancel)
            .expect("test helper should not cancel");

        // Build a material grid and replay the segments to check coverage
        let mut grid = MaterialGrid::from_polygon(&sq, cell_size);
        for seg in &segments {
            if let AdaptiveSegment::Cut(path) = seg {
                for p in path {
                    grid.clear_circle(p.x, p.y, tool_radius);
                }
            }
        }

        let remaining = grid.material_fraction();
        assert!(
            remaining < 0.15,
            "Adaptive should clear most material, {:.1}% remaining",
            remaining * 100.0
        );
    }

    #[test]
    fn test_adaptive_with_slot_clearing() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.slot_clearing = true;

        let tp = adaptive_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "Adaptive+slot should generate moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 20.0,
            "Should have significant cutting with slot, got {}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_with_min_cutting_radius() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.min_cutting_radius = 1.0;

        let tp = adaptive_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "Adaptive+blend should generate moves, got {}",
            tp.moves.len()
        );
    }

    // ── Link vs retract tests ──────────────────────────────────────────

    #[test]
    fn test_is_clear_path_cleared_area() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // Clear a corridor through the center
        for i in -20..=20 {
            let x = i as f64 * 0.5;
            grid.clear_circle(x, 0.0, tool_radius);
        }

        // Path through the cleared corridor should be safe
        let from = P2::new(-5.0, 0.0);
        let to = P2::new(5.0, 0.0);
        assert!(
            is_clear_path(&grid, &mask, from, to, tool_radius),
            "Path through cleared corridor should be safe"
        );
    }

    #[test]
    fn test_is_clear_path_blocked_by_material() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // Uncleared grid — path through material should be blocked
        let from = P2::new(-5.0, 0.0);
        let to = P2::new(5.0, 0.0);
        assert!(
            !is_clear_path(&grid, &mask, from, to, tool_radius),
            "Path through uncut material should be blocked"
        );
    }

    #[test]
    fn test_link_reduces_rapids() {
        let sq = square_polygon(16.0);
        let params = default_params(2.5, 1.2);

        let tp = adaptive_toolpath(&sq, &params);

        // Count rapid moves (retract + reposition)
        let _rapid_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
            .count();

        // With linking, there should be fewer rapids than passes * 2
        // (each retract+reposition pair = 2 rapids; links eliminate both)
        let never_cancel = || false;
        let segments = adaptive_segments(&sq, 2.5, 1.2, 0.2, false, &never_cancel)
            .expect("test helper should not cancel");
        let total_entries = segments
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)))
            .count();
        let link_count = segments
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Link(_)))
            .count();

        // Should have at least some links (nearby passes in cleared area)
        assert!(
            link_count > 0 || total_entries <= 2,
            "Should produce links between nearby passes, got {} links / {} entries",
            link_count,
            total_entries
        );
    }

    // ── Coarse scan direction search tests ────────────────────────────

    #[test]
    fn test_search_coarse_finds_uturn() {
        // Full material square, tool at center, prev_angle pointing +X.
        // Coarse 360° scan must find a valid direction (since material is everywhere).
        let sq = square_polygon(30.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        let machinable = offset_polygon(&sq, 2.5);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let target = target_engagement_fraction(1.2, 2.5);
        // prev_angle = PI (pointing -X) — narrow search should fail on some configs,
        // coarse scan covers full 360°
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            2.5,
            0.75,
            target,
            PI,
            &boundary_dist,
        );
        assert!(
            angle.is_some(),
            "Coarse scan should find a direction in full material"
        );
    }

    #[test]
    fn test_search_coarse_engagement_result() {
        // Verify that the direction found by the coarse scan actually
        // leads to a position with engagement within the target tolerance.
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        let tool_radius = 3.0;
        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let step_len = grid.cell_size * 1.5;
        let target = target_engagement_fraction(1.5, tool_radius);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            tool_radius,
            step_len,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find direction in open material");

        // Verify engagement at destination
        let a = angle.unwrap();
        let nx = step_len * a.cos();
        let ny = step_len * a.sin();
        let eng = compute_engagement(&grid, nx, ny, tool_radius);
        assert!(
            eng > 0.005,
            "Destination should have non-zero engagement, got {:.4}",
            eng
        );
    }

    // ── Growing-radius entry point tests ──────────────────────────────

    #[test]
    fn test_find_material_radius_finds_cluster() {
        // Material in one corner only, search from far away.
        let sq = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear everything except a 5×5 cluster in the top-right corner
        // by creating a new grid and keeping only the corner
        let mut grid2 = MaterialGrid::from_polygon(&sq, 0.5);
        for r in 0..grid2.rows {
            let y = grid2.origin_y + r as f64 * grid2.cell_size;
            for c in 0..grid2.cols {
                let x = grid2.origin_x + c as f64 * grid2.cell_size;
                if !(x > 33.0 && y > 33.0) && grid2.cells[r * grid2.cols + c] == CELL_MATERIAL {
                    grid2.cells[r * grid2.cols + c] = CELL_CLEARED;
                    grid2.material_count -= 1;
                }
            }
        }

        // Search from (5, 5) — far from the cluster
        let result = grid2.find_nearest_material(5.0, 5.0);
        assert!(
            result.is_some(),
            "Growing-radius search should find distant material"
        );
        let (mx, my) = result.unwrap();
        assert!(
            mx > 30.0 && my > 30.0,
            "Found material should be in the cluster at ({}, {})",
            mx,
            my
        );

        // Verify the original grid still works (regression)
        let result2 = grid.find_nearest_material(5.0, 5.0);
        assert!(result2.is_some(), "Full grid should find nearby material");
        let (mx2, my2) = result2.unwrap();
        let dist = ((mx2 - 5.0).powi(2) + (my2 - 5.0).powi(2)).sqrt();
        assert!(
            dist < 2.0,
            "Nearby material should be very close, got dist={:.1}",
            dist
        );
    }

    #[test]
    fn test_find_material_radius_nearby() {
        // Full material grid — nearest should be found immediately with small radius.
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        let result = grid.find_nearest_material(0.0, 0.0);
        assert!(result.is_some(), "Should find nearby material");
        let (mx, my) = result.unwrap();
        let dist = (mx * mx + my * my).sqrt();
        assert!(
            dist < 1.0,
            "Center of full grid should find material right there, got dist={:.1}",
            dist
        );
    }

    #[test]
    fn traced_adaptive_emits_pass_spans_and_hotspots() {
        let poly = square_polygon(20.0);
        let params = AdaptiveParams {
            slot_clearing: true,
            ..default_params(2.0, 1.5)
        };
        let recorder = crate::debug_trace::ToolpathDebugRecorder::new("Adaptive", "2D Rough");
        let ctx = recorder.root_context();
        let never_cancel = || false;

        let tp = adaptive_toolpath_traced_with_cancel(&poly, &params, &never_cancel, Some(&ctx))
            .expect("debug run should complete");
        let trace = recorder.finish();

        assert!(!tp.moves.is_empty(), "expected a non-empty toolpath");
        assert!(trace.spans.iter().any(|span| span.kind == "slot_clearing"));
        assert!(trace.spans.iter().any(|span| span.kind == "adaptive_pass"));
        assert!(
            trace
                .spans
                .iter()
                .any(|span| span.kind == "boundary_cleanup")
        );
        assert!(
            trace
                .spans
                .iter()
                .filter(|span| span.kind == "adaptive_pass")
                .any(|span| span.exit_reason.is_some()),
            "adaptive pass spans should record exit reasons"
        );
        assert!(
            trace
                .hotspots
                .iter()
                .any(|hotspot| hotspot.kind == "adaptive_pass"),
            "adaptive trace should record at least one hotspot"
        );
    }

    #[test]
    fn initial_stock_reduces_adaptive_moves() {
        use crate::geo::{BoundingBox3, P3};

        let poly = square_polygon(20.0);
        let tool_radius = 2.0;
        let stepover = 1.5;

        // Run without initial stock (full material).
        let params_full = default_params(tool_radius, stepover);
        let tp_full = adaptive_toolpath(&poly, &params_full);
        assert!(!tp_full.moves.is_empty(), "full run should produce moves");

        // Build a stock that covers the polygon, with the left half cleared.
        // Stock: x=-10..10, y=-10..10, z=-10..0
        let bbox = BoundingBox3 {
            min: P3::new(-10.0, -10.0, -10.0),
            max: P3::new(10.0, 10.0, 0.0),
        };
        let cell_size = 0.5;
        let mut stock = TriDexelStock::from_bounds(&bbox, cell_size);

        // Clear the left half (x < 0) by subtracting above z = -10 (removes
        // all material in those cells).
        let grid = &mut stock.z_grid;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let world_x = grid.origin_u + col as f64 * grid.cell_size;
                if world_x < 0.0 {
                    crate::dexel::ray_subtract_above(&mut grid.rays[row * grid.cols + col], -10.0);
                }
            }
        }

        // Run with the half-cleared stock.
        let params_stock = AdaptiveParams {
            initial_stock: Some(stock),
            ..default_params(tool_radius, stepover)
        };
        let tp_stock = adaptive_toolpath(&poly, &params_stock);
        assert!(
            !tp_stock.moves.is_empty(),
            "stock-aware run should still produce moves for remaining material"
        );

        // The stock-aware run should produce fewer moves because half
        // the material is already gone.
        assert!(
            tp_stock.moves.len() < tp_full.moves.len(),
            "stock-aware ({} moves) should be fewer than full ({} moves)",
            tp_stock.moves.len(),
            tp_full.moves.len(),
        );
    }
}
