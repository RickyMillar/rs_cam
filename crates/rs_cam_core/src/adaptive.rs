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

use crate::geo::P2;
use crate::polygon::{offset_polygon, Polygon2};
use crate::toolpath::Toolpath;

use std::f64::consts::{PI, TAU};

/// Parameters for adaptive clearing.
pub struct AdaptiveParams {
    pub tool_radius: f64,
    pub stepover: f64,
    pub cut_depth: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
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

    /// Clear a circle of material (mark as CELL_CLEARED).
    pub fn clear_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        let r_sq = radius * radius;
        let col_min = ((cx - radius - self.origin_x) / self.cell_size).floor().max(0.0) as usize;
        let col_max =
            ((cx + radius - self.origin_x) / self.cell_size).ceil() as usize;
        let row_min = ((cy - radius - self.origin_y) / self.cell_size).floor().max(0.0) as usize;
        let row_max =
            ((cy + radius - self.origin_y) / self.cell_size).ceil() as usize;

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

    /// Count the fraction of total material cells that remain uncut. O(1).
    pub fn material_fraction(&self) -> f64 {
        if self.total_solid == 0 {
            return 0.0;
        }
        self.material_count as f64 / self.total_solid as f64
    }

    /// Fast check if a position is inside the machinable region using the cached mask.
    #[inline]
    pub fn is_machinable(&self, mask: &[bool], x: f64, y: f64) -> bool {
        match self.world_to_cell(x, y) {
            Some((r, c)) => mask[r * self.cols + c],
            None => false,
        }
    }

    /// Find the nearest cell with uncut material to the given position.
    /// Returns the world coordinates of the cell center, or None if no material remains.
    pub fn find_nearest_material(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let mut best_dist_sq = f64::INFINITY;
        let mut best = None;

        for row in 0..self.rows {
            let cy = self.origin_y + row as f64 * self.cell_size;
            for col in 0..self.cols {
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

/// Number of sample points around the tool circle for engagement calculation.
const ENGAGEMENT_SAMPLES: usize = 24;

/// Compute engagement fraction at position (cx, cy) with tool of given radius.
///
/// Engagement = fraction of sample points on the tool circle that fall on
/// uncut material cells. Returns a value in [0.0, 1.0].
pub(crate) fn compute_engagement(grid: &MaterialGrid, cx: f64, cy: f64, radius: f64) -> f64 {
    let mut in_material = 0;
    for i in 0..ENGAGEMENT_SAMPLES {
        let angle = TAU * i as f64 / ENGAGEMENT_SAMPLES as f64;
        let px = cx + radius * angle.cos();
        let py = cy + radius * angle.sin();
        if grid.is_material(px, py) {
            in_material += 1;
        }
    }
    in_material as f64 / ENGAGEMENT_SAMPLES as f64
}

/// Compute the target engagement fraction from stepover and tool radius.
///
/// Uses the engagement angle formula: α = arccos(1 - WOC/R)
/// Then converts to fraction of full circle.
pub(crate) fn target_engagement_fraction(stepover: f64, tool_radius: f64) -> f64 {
    let woc = stepover.min(2.0 * tool_radius);
    let alpha = (1.0 - woc / tool_radius).max(-1.0).min(1.0).acos();
    alpha / TAU
}

// ── Direction search ───────────────────────────────────────────────────

/// Search for the best direction to move from (cx, cy) that produces
/// engagement closest to `target_frac`.
///
/// Two-pass search: first try ±90° (forward preference), then full 360°
/// if no good direction found. Returns None only if no direction has
/// any engagement at all.
pub(crate) fn search_direction(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
) -> Option<f64> {
    let tolerance = 0.20; // allow ±20% of target
    let min_frac = (target_frac * (1.0 - tolerance)).max(0.005);
    let max_frac = target_frac * (1.0 + tolerance);

    // Evaluate a sweep and return the best angle
    let evaluate_sweep = |sweep: f64, n_candidates: usize| -> (Option<f64>, Option<f64>) {
        let mut best_good: Option<(f64, f64)> = None; // (score, angle) - within tolerance
        let mut best_any: Option<(f64, f64)> = None; // (score, angle) - any engagement

        for i in 0..n_candidates {
            let t = i as f64 / (n_candidates - 1) as f64;
            let angle = prev_angle - sweep / 2.0 + t * sweep;

            let nx = cx + step_len * angle.cos();
            let ny = cy + step_len * angle.sin();

            if !grid.is_machinable(machinable_mask, nx, ny) {
                continue;
            }

            let engagement = compute_engagement(grid, nx, ny, tool_radius);
            if engagement < 0.005 {
                continue;
            }

            let error = (engagement - target_frac).abs();
            let angle_penalty = angle_diff(angle, prev_angle).abs() / PI; // 0..1
            let score = error + angle_penalty * 0.05;

            if engagement >= min_frac && engagement <= max_frac {
                if best_good.is_none() || score < best_good.unwrap().0 {
                    best_good = Some((score, angle));
                }
            }
            if best_any.is_none() || score < best_any.unwrap().0 {
                best_any = Some((score, angle));
            }
        }

        (
            best_good.map(|(_, a)| a),
            best_any.map(|(_, a)| a),
        )
    };

    // Pass 1: forward ±90°
    let (good, fallback) = evaluate_sweep(PI, 19);
    if let Some(angle) = good {
        return Some(angle);
    }

    // Pass 2: full 360° (allows U-turns)
    let (good2, fallback2) = evaluate_sweep(TAU, 36);
    if let Some(angle) = good2 {
        return Some(angle);
    }

    // Fallback: any direction with engagement
    fallback.or(fallback2)
}

/// Normalize an angle difference to [-π, π].
fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = a - b;
    while d > PI {
        d -= TAU;
    }
    while d < -PI {
        d += TAU;
    }
    d
}

// ── Entry point finding ────────────────────────────────────────────────

/// Find an entry point: a position inside the machinable region
/// that has uncut material nearby.
///
/// Prefers positions with partial engagement (near the edge of uncut material)
/// to avoid plunging into the middle of a large uncut block.
pub(crate) fn find_entry_point(
    grid: &MaterialGrid,
    machinable_mask: &[bool],
    tool_radius: f64,
    last_pos: Option<P2>,
) -> Option<P2> {
    // Find nearest material cell (to last_pos or grid center)
    let search_from = last_pos.unwrap_or_else(|| {
        let cx = grid.origin_x + (grid.cols as f64 * grid.cell_size) / 2.0;
        let cy = grid.origin_y + (grid.rows as f64 * grid.cell_size) / 2.0;
        P2::new(cx, cy)
    });

    let (mx, my) = grid.find_nearest_material(search_from.x, search_from.y)?;

    // If the material cell is inside the machinable region, use it
    if grid.is_machinable(machinable_mask, mx, my) {
        return Some(P2::new(mx, my));
    }

    // Material is outside the machinable region (near polygon boundary).
    // Find the closest machinable cell with engagement near the material.
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

// ── Main adaptive path generation ──────────────────────────────────────

/// A segment of the adaptive path: either a cutting segment or a rapid reposition.
enum AdaptiveSegment {
    /// Cutting moves: a sequence of 2D points.
    Cut(Vec<P2>),
    /// Rapid reposition to a new entry point.
    Rapid(P2),
}

/// Generate the 2D adaptive clearing path segments.
fn adaptive_segments(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    tolerance: f64,
) -> Vec<AdaptiveSegment> {
    // Inset polygon by tool radius to get the machinable region
    let machinable_vec = offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return Vec::new();
    }
    let machinable = &machinable_vec[0];

    // Build material grid from the original polygon (not inset)
    let cell_size = (tool_radius / 6.0).max(tolerance);
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);

    // Cache the machinable region as a boolean mask for fast lookups
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );

    let target_frac = target_engagement_fraction(stepover, tool_radius);
    let step_len = cell_size * 1.5;
    let mut segments = Vec::new();
    let mut last_pos: Option<P2> = None;

    let max_passes = 500; // safety limit
    let mut pass_count = 0;

    while grid.material_fraction() > 0.01 && pass_count < max_passes {
        pass_count += 1;

        // Find entry point
        let entry = match find_entry_point(&grid, &machinable_mask, tool_radius, last_pos) {
            Some(p) => p,
            None => break,
        };

        // Generate rapid to entry
        segments.push(AdaptiveSegment::Rapid(entry));

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

        let max_steps = 5000;
        let mut idle_count = 0;
        for _ in 0..max_steps {
            let before = grid.material_count;

            // Search for next direction
            let angle = match search_direction(
                &grid,
                &machinable_mask,
                cx,
                cy,
                tool_radius,
                step_len,
                target_frac,
                prev_angle,
            ) {
                Some(a) => a,
                None => break,
            };

            // Move in that direction
            cx += step_len * angle.cos();
            cy += step_len * angle.sin();
            path.push(P2::new(cx, cy));

            // Clear material at new position
            grid.clear_circle(cx, cy, tool_radius);

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

        if path.len() >= 2 {
            last_pos = Some(*path.last().unwrap());
            segments.push(AdaptiveSegment::Cut(path));
        } else {
            last_pos = Some(entry);
        }

        // If the pass ended due to idle detection, the remaining material
        // nearby is too small or inaccessible. Force-clear a wider area
        // around the last position to prevent revisiting the same spot.
        if was_idle {
            grid.clear_circle(cx, cy, tool_radius * 2.0);
        }
    }

    segments
}

/// Simplify a path using the Douglas-Peucker algorithm.
fn simplify_path(points: &[P2], tolerance: f64) -> Vec<P2> {
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
        for i in 1..points.len() - 1 {
            let d = ((points[i].x - first.x) * dy - (points[i].y - first.y) * dx).abs() / line_len;
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    } else {
        // Degenerate case: all points are close together
        for i in 1..points.len() - 1 {
            let d = ((points[i].x - first.x).powi(2) + (points[i].y - first.y).powi(2)).sqrt();
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

/// Generate an adaptive clearing toolpath for a 2D polygon region.
///
/// The toolpath maintains approximately constant engagement by dynamically
/// adjusting direction at each step. Returns a Toolpath with rapids,
/// plunges, and feeds at the specified cut_depth.
pub fn adaptive_toolpath(polygon: &Polygon2, params: &AdaptiveParams) -> Toolpath {
    let segments = adaptive_segments(
        polygon,
        params.tool_radius,
        params.stepover,
        params.tolerance,
    );

    let mut tp = Toolpath::new();
    if segments.is_empty() {
        return tp;
    }

    for segment in &segments {
        match segment {
            AdaptiveSegment::Rapid(entry) => {
                // Retract, rapid to entry, plunge
                tp.rapid_to(crate::geo::P3::new(entry.x, entry.y, params.safe_z));
                tp.feed_to(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.plunge_rate,
                );
            }
            AdaptiveSegment::Cut(path) => {
                // Simplify the path
                let simplified = simplify_path(path, params.tolerance);
                for p in simplified.iter().skip(1) {
                    tp.feed_to(
                        crate::geo::P3::new(p.x, p.y, params.cut_depth),
                        params.feed_rate,
                    );
                }
            }
        }
    }

    // Final retract
    if let Some(last) = tp.moves.last() {
        tp.rapid_to(crate::geo::P3::new(last.target.x, last.target.y, params.safe_z));
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
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

        // Machinable = inset by tool radius
        let machinable = offset_polygon(&sq, 3.0);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0], grid.origin_x, grid.origin_y,
            grid.rows, grid.cols, grid.cell_size,
        );

        let target = target_engagement_fraction(1.5, 3.0);
        let angle = search_direction(&grid, &mask, 0.0, 0.0, 3.0, 1.0, target, 0.0);
        assert!(angle.is_some(), "Should find a direction in open material");
    }

    #[test]
    fn test_search_direction_blocked_outside() {
        let sq = square_polygon(10.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

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
            &machinable[0], grid.origin_x, grid.origin_y,
            grid.rows, grid.cols, grid.cell_size,
        );
        let target = target_engagement_fraction(1.0, 2.0);
        let angle = search_direction(&grid, &mask, 0.0, 0.0, 2.0, 0.5, target, 0.0);
        assert!(angle.is_none(), "Should be blocked when no material remains");
    }

    // ── Path simplification tests ──────────────────────────────────────

    #[test]
    fn test_simplify_straight_line() {
        let pts: Vec<P2> = (0..=10)
            .map(|i| P2::new(i as f64, 0.0))
            .collect();
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
        assert!(
            simplified.len() >= 3,
            "L-shape should preserve the corner"
        );
    }

    // ── Full adaptive toolpath tests ───────────────────────────────────

    #[test]
    fn test_adaptive_toolpath_basic() {
        let sq = square_polygon(16.0);
        let params = AdaptiveParams {
            tool_radius: 2.5,
            stepover: 1.2,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.2,
        };

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
        let params = AdaptiveParams {
            tool_radius: 2.5,
            stepover: 1.2,
            cut_depth: -5.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.2,
        };

        let tp = adaptive_toolpath(&sq, &params);

        // All feed moves should be at cut_depth
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { feed_rate } = m.move_type {
                if feed_rate > 500.0 {
                    // cutting move (not plunge)
                    assert!(
                        (m.target.z - (-5.0)).abs() < 1e-10,
                        "Cutting move should be at cut_depth, got z={}",
                        m.target.z
                    );
                }
            }
        }
    }

    #[test]
    fn test_adaptive_too_small_polygon() {
        // Polygon smaller than tool
        let sq = square_polygon(3.0);
        let params = AdaptiveParams {
            tool_radius: 3.0,
            stepover: 1.5,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
        };

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

        let segments = adaptive_segments(&sq, tool_radius, 1.2, 0.2);

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
            remaining < 0.1,
            "Adaptive should clear most material, {:.1}% remaining",
            remaining * 100.0
        );
    }
}
