//! 2D material grid for adaptive clearing engagement calculation.

use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::polygon::Polygon2;
use std::collections::VecDeque;

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
    pub(super) material_count: usize,
    /// Total number of non-air cells.
    total_solid: usize,
}

const CELL_AIR: u8 = 0;
pub(super) const CELL_MATERIAL: u8 = 1;
pub(super) const CELL_CLEARED: u8 = 2;

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
        let Some((row, col)) = self.world_to_cell(x, y) else {
            return (0.0, 0.0);
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

pub(super) fn polygon_bbox(pts: &[P2]) -> (f64, f64, f64, f64) {
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
