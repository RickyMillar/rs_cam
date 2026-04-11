//! Engagement computation, direction search, and entry-point finding.
//!
//! Shared by the adaptive main loop in `path.rs` (via `pub(super)` free
//! functions) and by `mod.rs` tests.

use super::material_grid::CELL_MATERIAL;
use super::{MaterialGrid, angle_diff, refine_angle_bracket};
use crate::debug_trace::ToolpathDebugBounds2;
use crate::geo::P2;
use crate::polygon::Polygon2;

use std::f64::consts::{PI, TAU};

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
pub(super) struct SearchDirectionResult {
    pub(super) angle: f64,
    pub(super) evaluations: u32,
}

pub(super) fn path_bounds(path: &[P2]) -> Option<ToolpathDebugBounds2> {
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
#[cfg_attr(not(test), allow(dead_code))]
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
pub(super) fn search_direction_with_metrics(
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
    let tolerance = 0.05; // allow ±5% of target (matches libactp reference)
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

        let score = error + angle_penalty * 0.03 + wall_bias;
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
                refine_angle_bracket(lo, hi, target_frac, 8, &mut eval_candidate)
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
        let n_coarse = 36;
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
                refine_angle_bracket(lo, hi, target_frac, 8, eval_candidate)
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
