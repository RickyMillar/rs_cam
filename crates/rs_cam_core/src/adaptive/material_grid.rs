//! 2D material grid for adaptive clearing engagement calculation.

use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::polygon::Polygon2;

/// 2D boolean grid tracking material presence for engagement calculation.
///
/// Cell values: 0 = outside polygon (air), 1 = uncut material, 2 = cleared.
#[derive(Clone)]
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
    /// G-ADAPTPASSLOAD round 3: per cell, which of its [`SUB`] x [`SUB`]
    /// sub-points still hold stock, when the grid keeps a fringe
    /// ([`Self::keep_fringe`]). `None`: the historical grid (a cell is cut
    /// when its centre is).
    sub: Option<Vec<u16>>,
}

/// Sub-points per cell side under a fringe: 4 x 4, a quarter-cell pitch
/// (0.125 mm on a 6 mm cutter's 0.5 mm cell). A sliver thinner than the
/// cell that the centre lattice misses is held on these.
pub(super) const SUB: usize = 4;

/// Offset of sub-point `i` (0..SUB) from its cell centre, in cells.
pub(super) fn sub_offset(i: usize) -> f64 {
    (i as f64 + 0.5) / SUB as f64 - 0.5
}

/// One logged cell change: index, state before, sub-points before.
pub(super) type CellChange = (usize, u8, u16);

const CELL_AIR: u8 = 0;
pub(super) const CELL_MATERIAL: u8 = 1;
pub(super) const CELL_CLEARED: u8 = 2;
/// A cell whose centre a cut has reached while some of its sub-points
/// still hold stock (a sliver thinner than the lattice). Not material for
/// residue targets or the material count; the swept width reads its
/// standing sub-points ([`super::search::compute_swept_width`]).
pub(super) const CELL_FRINGE: u8 = 3;

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
            sub: None,
        }
    }

    /// Keep a fringe from now on (the 2D pass-load rule): each cell carries
    /// [`SUB`] x [`SUB`] sub-points, those inside `polygon` holding stock.
    /// A cut clears the sub-points its disc covers; a cell whose centre is
    /// cut becomes [`CELL_FRINGE`] while any sub-point stands and
    /// [`CELL_CLEARED`] when none does. Sub-points outside the part are
    /// never stock, so a cell straddling a wall clears when the cutter
    /// reaches the wall.
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub(super) fn keep_fringe(&mut self, polygon: &Polygon2) {
        let mut sub = vec![0u16; self.cells.len()];
        for row in 0..self.rows {
            let y = self.origin_y + row as f64 * self.cell_size;
            for col in 0..self.cols {
                let idx = row * self.cols + col;
                if self.cells[idx] == CELL_AIR {
                    continue;
                }
                let x = self.origin_x + col as f64 * self.cell_size;
                let mut bits = 0u16;
                for j in 0..SUB {
                    for i in 0..SUB {
                        let p = P2::new(
                            x + sub_offset(i) * self.cell_size,
                            y + sub_offset(j) * self.cell_size,
                        );
                        if polygon.contains_point(&p) {
                            bits |= 1 << (j * SUB + i);
                        }
                    }
                }
                sub[idx] = bits;
            }
        }
        self.sub = Some(sub);
    }

    /// The standing sub-points of the cell at `idx` under a fringe.
    pub(super) fn fringe_bits(&self, idx: usize) -> u16 {
        self.sub
            .as_ref()
            .and_then(|sub| sub.get(idx).copied())
            .unwrap_or(0)
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
    pub(crate) fn get_at(&self, x: f64, y: f64) -> u8 {
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

    /// Clear a circle of material (mark as CELL_CLEARED).
    pub fn clear_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        self.clear_circle_inner(cx, cy, radius, None);
    }

    /// [`Self::clear_circle`], recording every cell it clears in `log` so
    /// [`Self::restore_cleared`] can undo a trial cut.
    pub(super) fn clear_circle_logged(
        &mut self,
        cx: f64,
        cy: f64,
        radius: f64,
        log: &mut Vec<CellChange>,
    ) {
        self.clear_circle_inner(cx, cy, radius, Some(log));
    }

    /// Cut the straight move `a → b` (the capsule the cutter sweeps). Under
    /// a fringe the sweep is read whole, so the stock between two stamps a
    /// step apart is cut as the machine cuts it (a disc per step would
    /// leave scallops a sub-point can hold: 0.094 mm deep between 1.5 mm
    /// steps of a 3 mm radius). The historical grid stamps the disc at `b`.
    pub(super) fn clear_segment(&mut self, a: P2, b: P2, radius: f64) {
        if self.sub.is_some() {
            self.clear_capsule_inner(a, b, radius, None);
        } else {
            self.clear_circle_inner(b.x, b.y, radius, None);
        }
    }

    /// [`Self::clear_segment`], logged for [`Self::restore_cleared`].
    pub(super) fn clear_segment_logged(
        &mut self,
        a: P2,
        b: P2,
        radius: f64,
        log: &mut Vec<CellChange>,
    ) {
        if self.sub.is_some() {
            self.clear_capsule_inner(a, b, radius, Some(log));
        } else {
            self.clear_circle_inner(b.x, b.y, radius, Some(log));
        }
    }

    /// Put back what a logged trial cut changed, newest change first.
    pub(super) fn restore_cleared(&mut self, log: &[CellChange]) {
        for &(idx, was, was_bits) in log.iter().rev() {
            if let Some(cell) = self.cells.get_mut(idx) {
                if was == CELL_MATERIAL && *cell != CELL_MATERIAL {
                    self.material_count += 1;
                }
                *cell = was;
            }
            if let Some(bits) = self.sub.as_mut().and_then(|sub| sub.get_mut(idx)) {
                *bits = was_bits;
            }
        }
    }

    /// The capsule stamp behind [`Self::clear_segment`] (fringe grids).
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    fn clear_capsule_inner(
        &mut self,
        a: P2,
        b: P2,
        radius: f64,
        mut log: Option<&mut Vec<CellChange>>,
    ) {
        let Some(sub) = self.sub.as_mut() else {
            return;
        };
        let r_sq = radius * radius;
        let reach = radius + 0.5 * std::f64::consts::SQRT_2 * self.cell_size;
        let (ux, uy) = (b.x - a.x, b.y - a.y);
        let len_sq = ux * ux + uy * uy;
        let dist_sq = |x: f64, y: f64| {
            let t = if len_sq > 1e-20 {
                (((x - a.x) * ux + (y - a.y) * uy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (dx, dy) = (x - a.x - t * ux, y - a.y - t * uy);
            dx * dx + dy * dy
        };
        let col_min = ((a.x.min(b.x) - reach - self.origin_x) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let col_max = (((a.x.max(b.x) + reach - self.origin_x) / self.cell_size).ceil() as usize)
            .min(self.cols - 1);
        let row_min = ((a.y.min(b.y) - reach - self.origin_y) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let row_max = (((a.y.max(b.y) + reach - self.origin_y) / self.cell_size).ceil() as usize)
            .min(self.rows - 1);
        for row in row_min..=row_max {
            let y = self.origin_y + row as f64 * self.cell_size;
            for col in col_min..=col_max {
                let idx = row * self.cols + col;
                let was = self.cells[idx];
                if was != CELL_MATERIAL && was != CELL_FRINGE {
                    continue;
                }
                let x = self.origin_x + col as f64 * self.cell_size;
                let d_sq = dist_sq(x, y);
                if d_sq > reach * reach {
                    continue;
                }
                let centre_cut = d_sq <= r_sq;
                let was_bits = sub[idx];
                let mut bits = was_bits;
                for j in 0..SUB {
                    let sy = y + sub_offset(j) * self.cell_size;
                    for i in 0..SUB {
                        let sx = x + sub_offset(i) * self.cell_size;
                        if dist_sq(sx, sy) <= r_sq {
                            bits &= !(1 << (j * SUB + i));
                        }
                    }
                }
                let now = if was == CELL_FRINGE || centre_cut {
                    if bits == 0 { CELL_CLEARED } else { CELL_FRINGE }
                } else {
                    CELL_MATERIAL
                };
                if now == was && bits == was_bits {
                    continue;
                }
                sub[idx] = bits;
                self.cells[idx] = now;
                if was == CELL_MATERIAL && now != CELL_MATERIAL {
                    self.material_count -= 1;
                }
                if let Some(log) = log.as_deref_mut() {
                    log.push((idx, was, was_bits));
                }
            }
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    fn clear_circle_inner(
        &mut self,
        cx: f64,
        cy: f64,
        radius: f64,
        mut log: Option<&mut Vec<CellChange>>,
    ) {
        let r_sq = radius * radius;
        // Under a fringe a disc reaches the sub-points of cells whose centre
        // lies up to half a cell diagonal outside it.
        let reach = if self.sub.is_some() {
            radius + 0.5 * std::f64::consts::SQRT_2 * self.cell_size
        } else {
            radius
        };
        let reach_sq = reach * reach;
        let col_min = ((cx - reach - self.origin_x) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let col_max = ((cx + reach - self.origin_x) / self.cell_size).ceil() as usize;
        let row_min = ((cy - reach - self.origin_y) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let row_max = ((cy + reach - self.origin_y) / self.cell_size).ceil() as usize;

        let col_max = col_max.min(self.cols - 1);
        let row_max = row_max.min(self.rows - 1);

        for row in row_min..=row_max {
            let cell_y = self.origin_y + row as f64 * self.cell_size;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > reach_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = self.origin_x + col as f64 * self.cell_size;
                let dx = cell_x - cx;
                let d_sq = dx * dx + dy_sq;
                if d_sq > reach_sq {
                    continue;
                }
                let idx = row * self.cols + col;
                let was = self.cells[idx];
                if was != CELL_MATERIAL && was != CELL_FRINGE {
                    continue;
                }
                let centre_cut = d_sq <= r_sq;
                let Some(sub) = self.sub.as_mut() else {
                    if centre_cut {
                        self.cells[idx] = CELL_CLEARED;
                        self.material_count -= 1;
                        if let Some(log) = log.as_deref_mut() {
                            log.push((idx, was, 0));
                        }
                    }
                    continue;
                };
                let was_bits = sub[idx];
                let mut bits = was_bits;
                for j in 0..SUB {
                    let sy = dy + sub_offset(j) * self.cell_size;
                    for i in 0..SUB {
                        let sx = dx + sub_offset(i) * self.cell_size;
                        if sx * sx + sy * sy <= r_sq {
                            bits &= !(1 << (j * SUB + i));
                        }
                    }
                }
                let now = if was == CELL_FRINGE || centre_cut {
                    if bits == 0 { CELL_CLEARED } else { CELL_FRINGE }
                } else {
                    CELL_MATERIAL
                };
                if now == was && bits == was_bits {
                    continue;
                }
                sub[idx] = bits;
                self.cells[idx] = now;
                if was == CELL_MATERIAL && now != CELL_MATERIAL {
                    self.material_count -= 1;
                }
                if let Some(log) = log.as_deref_mut() {
                    log.push((idx, was, was_bits));
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

    /// The machinable lattice point nearest `(x, y)` within `radius`, if
    /// any: where a cutter of that radius can stand to touch `(x, y)`.
    // SAFETY: rows and columns are clamped to the grid before indexing.
    #[allow(clippy::indexing_slicing)]
    pub(super) fn nearest_machinable_within(
        &self,
        mask: &[bool],
        x: f64,
        y: f64,
        radius: f64,
    ) -> Option<P2> {
        let col_min = ((x - radius - self.origin_x) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let col_max = (((x + radius - self.origin_x) / self.cell_size).ceil() as usize)
            .min(self.cols.saturating_sub(1));
        let row_min = ((y - radius - self.origin_y) / self.cell_size)
            .floor()
            .max(0.0) as usize;
        let row_max = (((y + radius - self.origin_y) / self.cell_size).ceil() as usize)
            .min(self.rows.saturating_sub(1));
        let mut best: Option<(f64, P2)> = None;
        for row in row_min..=row_max {
            let cy = self.origin_y + row as f64 * self.cell_size;
            for col in col_min..=col_max {
                if !mask[row * self.cols + col] {
                    continue;
                }
                let cx = self.origin_x + col as f64 * self.cell_size;
                let d_sq = (cx - x) * (cx - x) + (cy - y) * (cy - y);
                if d_sq <= radius * radius && best.is_none_or(|(b, _)| d_sq < b) {
                    best = Some((d_sq, P2::new(cx, cy)));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Find the nearest cell with uncut material to the given position.
    /// Uses growing-radius search: starts small, doubles until found.
    /// Returns the world coordinates of the cell center, or None if no material remains.
    pub fn find_nearest_material(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        self.find_nearest_material_where(x, y, |_| true)
    }

    /// [`Self::find_nearest_material`] over the material cells whose index
    /// `keep` accepts.
    pub(super) fn find_nearest_material_where(
        &self,
        x: f64,
        y: f64,
        keep: impl Fn(usize) -> bool,
    ) -> Option<(f64, f64)> {
        let initial_radius = self.cell_size * 8.0;
        let max_radius =
            (self.cols as f64 * self.cell_size).max(self.rows as f64 * self.cell_size) * 1.5;

        let mut radius = initial_radius;
        while radius <= max_radius {
            if let Some(result) = self.find_nearest_material_in_radius(x, y, radius, &keep) {
                return Some(result);
            }
            radius *= 2.0;
        }
        // Final full scan as fallback
        self.find_nearest_material_in_radius(x, y, max_radius, &keep)
    }

    /// The cell index of the lattice point at world `(x, y)`, if on the grid.
    pub(super) fn cell_index(&self, x: f64, y: f64) -> Option<usize> {
        self.world_to_cell(x + 1e-9, y + 1e-9)
            .map(|(r, c)| r * self.cols + c)
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Search for nearest material within a given radius from (x, y).
    fn find_nearest_material_in_radius(
        &self,
        x: f64,
        y: f64,
        radius: f64,
        keep: &impl Fn(usize) -> bool,
    ) -> Option<(f64, f64)> {
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
                let idx = row * self.cols + col;
                if self.cells[idx] != CELL_MATERIAL || !keep(idx) {
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

    /// Compute distance-to-boundary for every cell (world units).
    ///
    /// AIR cells have distance 0; material/cleared cells get their true
    /// **Euclidean** distance to the nearest AIR cell, via the shared
    /// Felzenszwalb EDT in `grid_field`. O(cells).
    ///
    /// Pre-Stage-0 this was a 4-connected BFS — a Manhattan metric that
    /// over-read up to ~41% wherever the nearest boundary is diagonal
    /// (the old `is_narrow_machinable` comment documented "~12 mm
    /// Manhattan vs ~8 mm Euclidean" on a donut-ring corner). The wall
    /// bias, the gradient-mode switch and the strip-centerline follower
    /// all consume this field, so they fired late near angled walls
    /// (algorithm review 2026-06-12, F4).
    pub fn compute_boundary_distances(&self) -> Vec<f64> {
        let air: Vec<bool> = self.cells.iter().map(|&c| c == CELL_AIR).collect();
        let mut dist =
            crate::geometry::grid_field::distance_transform_2d(&air, self.rows, self.cols);
        for d in &mut dist {
            *d *= self.cell_size;
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
