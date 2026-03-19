//! 3D adaptive clearing with constant engagement on mesh surfaces.
//!
//! Maintains constant tool engagement while following an STL mesh surface.
//! Uses heightmap-based material tracking (not boolean grid), drop-cutter
//! queries for Z following, and precomputed surface heightmap for fast
//! engagement computation.
//!
//! Key differences from 2D adaptive:
//! - Material state: f64 heightmap (not boolean grid)
//! - Z at each step: from point_drop_cutter (not constant)
//! - Engagement: "material above surface" not "material vs cleared"
//! - Multi-level: Z levels from stock_top down to mesh surface
//! - Boundary cleanup: waterline contours (not polygon offset contours)

use crate::adaptive::{
    angle_diff, average_angles, blend_corners, target_engagement_fraction,
};
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::simulation::{stamp_tool_at, Heightmap};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::waterline::waterline_contours;

use rayon::prelude::*;
use std::f64::consts::{PI, TAU};
use tracing::{info, debug};

/// Entry strategy for 3D adaptive (replaces vertical plunge).
#[derive(Debug, Clone, Copy, Default)]
pub enum EntryStyle3d {
    /// Vertical plunge (default prior behavior).
    #[default]
    Plunge,
    /// Helical entry: spiral down with given radius and pitch (mm/rev).
    Helix { radius: f64, pitch: f64 },
    /// Ramp entry: descend at a shallow angle along the next cutting direction.
    Ramp { max_angle_deg: f64 },
}

/// Parameters for 3D adaptive clearing.
pub struct Adaptive3dParams {
    pub tool_radius: f64,
    pub stepover: f64,
    pub depth_per_pass: f64,
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
    pub min_cutting_radius: f64,
    pub stock_top_z: f64,
    /// Entry strategy (default: Plunge for backward compat).
    pub entry_style: EntryStyle3d,
    /// Fine stepdown: when set, insert intermediate Z levels at this interval.
    pub fine_stepdown: Option<f64>,
    /// Detect flat areas in the mesh and insert Z levels at shelf heights.
    pub detect_flat_areas: bool,
    /// Maximum distance to stay down between passes (default: tool_radius * 6).
    pub max_stay_down_dist: Option<f64>,
}

// ── Surface heightmap ─────────────────────────────────────────────────

/// Precomputed mesh surface Z heights at grid resolution.
/// One parallel batch of drop-cutter queries at init, then O(1) lookups.
struct SurfaceHeightmap {
    z_values: Vec<f64>,
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell_size: f64,
}

impl SurfaceHeightmap {
    /// Build via rayon-parallelized drop-cutter queries at each grid cell.
    fn from_mesh(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
        min_z: f64,
    ) -> Self {
        let total = rows * cols;
        let z_values: Vec<f64> = (0..total)
            .into_par_iter()
            .map(|i| {
                let row = i / cols;
                let col = i % cols;
                let x = origin_x + col as f64 * cell_size;
                let y = origin_y + row as f64 * cell_size;
                let cl = point_drop_cutter(x, y, mesh, index, cutter);
                cl.z.max(min_z)
            })
            .collect();

        Self {
            z_values,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        }
    }

    /// O(1) surface Z lookup by cell indices.
    #[inline]
    fn surface_z_at(&self, row: usize, col: usize) -> f64 {
        self.z_values[row * self.cols + col]
    }

    /// Surface Z at world coordinates. Returns min representable for out-of-bounds.
    fn surface_z_at_world(&self, x: f64, y: f64) -> f64 {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return f64::NEG_INFINITY;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return f64::NEG_INFINITY;
        }
        self.z_values[row as usize * self.cols + col as usize]
    }

    /// Minimum surface Z across all cells (bottom of the mesh surface).
    fn min_z(&self) -> f64 {
        self.z_values
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
    }
}

/// Sum Z values in a local area around (cx, cy) within radius.
/// Used for cheap O(local_cells) idle detection instead of summing entire grid.
#[inline]
fn local_material_sum(hm: &Heightmap, cx: f64, cy: f64, radius: f64) -> f64 {
    let cs = hm.cell_size;
    let r = radius * 1.5; // Slightly wider to catch changes from the stamp
    let col_min = ((cx - r - hm.origin_x) / cs).floor().max(0.0) as usize;
    let col_max = ((cx + r - hm.origin_x) / cs).ceil().min((hm.cols - 1) as f64) as usize;
    let row_min = ((cy - r - hm.origin_y) / cs).floor().max(0.0) as usize;
    let row_max = ((cy + r - hm.origin_y) / cs).ceil().min((hm.rows - 1) as f64) as usize;

    let mut sum = 0.0;
    for row in row_min..=row_max {
        let base = row * hm.cols;
        for col in col_min..=col_max {
            sum += hm.cells[base + col];
        }
    }
    sum
}

// ── 3D engagement ─────────────────────────────────────────────────────

/// Compute 3D engagement at position (cx, cy) for a given z_level.
///
/// For each cell in the tool circle, material exists if:
///   material_heightmap_z > max(surface_z + stock_to_leave, z_level) + epsilon
///
/// Returns fraction of cells with material in [0.0, 1.0].
fn compute_engagement_3d(
    material_hm: &Heightmap,
    surface_hm: &SurfaceHeightmap,
    cx: f64,
    cy: f64,
    tool_radius: f64,
    z_level: f64,
    stock_to_leave: f64,
) -> f64 {
    let r_sq = tool_radius * tool_radius;
    let cs = material_hm.cell_size;

    let col_min = ((cx - tool_radius - material_hm.origin_x) / cs)
        .floor()
        .max(0.0) as usize;
    let col_max = ((cx + tool_radius - material_hm.origin_x) / cs).ceil() as usize;
    let row_min = ((cy - tool_radius - material_hm.origin_y) / cs)
        .floor()
        .max(0.0) as usize;
    let row_max = ((cy + tool_radius - material_hm.origin_y) / cs).ceil() as usize;

    let col_max = col_max.min(material_hm.cols.saturating_sub(1));
    let row_max = row_max.min(material_hm.rows.saturating_sub(1));

    let mut material_cells = 0u32;
    let mut total_cells = 0u32;

    for row in row_min..=row_max {
        let cell_y = material_hm.origin_y + row as f64 * cs;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }

        for col in col_min..=col_max {
            let cell_x = material_hm.origin_x + col as f64 * cs;
            let dx = cell_x - cx;
            if dx * dx + dy_sq > r_sq {
                continue;
            }

            total_cells += 1;
            let mat_z = material_hm.get(row, col);
            let surf_z = surface_hm.surface_z_at(row, col);
            let effective_floor = (surf_z + stock_to_leave).max(z_level);

            if mat_z > effective_floor + 0.01 {
                material_cells += 1;
            }
        }
    }

    if total_cells == 0 {
        0.0
    } else {
        material_cells as f64 / total_cells as f64
    }
}

/// Fraction of grid cells where material remains above the effective floor
/// at a given z_level. Used to decide when a level is done.
fn material_remaining_at_level(
    material_hm: &Heightmap,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
) -> f64 {
    let mut above = 0u64;
    let mut total = 0u64;
    for i in 0..material_hm.cells.len() {
        let surf_z = surface_hm.z_values[i];
        let floor = (surf_z + stock_to_leave).max(z_level);
        // Only count cells where the surface is actually below the current level
        // (cells where surface is above z_level were handled at higher levels)
        if surf_z + stock_to_leave <= z_level + 0.01 {
            total += 1;
            if material_hm.cells[i] > floor + 0.01 {
                above += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        above as f64 / total as f64
    }
}

// ── 3D direction search ───────────────────────────────────────────────

/// Search for the best direction to move from (cx, cy) that achieves
/// target engagement. Returns (angle, z_at_next_position).
///
/// Three-phase search (same as 2D adaptive):
/// 1. Narrow interpolation (7 candidates near prev_angle + bracket refinement)
/// 2. Forward sweep +/-90 (19 candidates)
/// 3. Full 360 (36 candidates)
fn search_direction_3d(
    material_hm: &Heightmap,
    surface_hm: &SurfaceHeightmap,
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    z_level: f64,
    stock_to_leave: f64,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
) -> Option<(f64, f64)> {
    // Evaluate a candidate direction: returns (score, z_at_next) or None if invalid.
    // Uses precomputed surface heightmap for Z lookups (O(1)) instead of drop-cutter queries.
    let evaluate = |angle: f64| -> Option<(f64, f64)> {
        let nx = cx + step_len * angle.cos();
        let ny = cy + step_len * angle.sin();

        // Bounds check
        if nx < bbox_x_min || nx > bbox_x_max || ny < bbox_y_min || ny > bbox_y_max {
            return None;
        }

        // Z from precomputed surface heightmap (O(1) vs O(k) for drop-cutter)
        let surf_z = surface_hm.surface_z_at_world(nx, ny);
        if surf_z == f64::NEG_INFINITY {
            return None;
        }
        let z = (surf_z + stock_to_leave).max(z_level);

        // Engagement at candidate
        let eng =
            compute_engagement_3d(material_hm, surface_hm, nx, ny, tool_radius, z_level, stock_to_leave);

        if eng < 0.001 {
            return None;
        }

        let error = (eng - target_frac).abs();
        let ad = angle_diff(angle, prev_angle).abs() / PI;
        let score = error + ad * 0.12;

        Some((score, z))
    };

    // Phase 1: Narrow interpolation — 7 candidates near prev_angle
    let narrow_offsets = [0.0, 15.0_f64, -15.0, 30.0, -30.0, 45.0, -45.0];
    let mut best: Option<(f64, f64, f64)> = None; // (score, angle, z)

    // Track brackets for interpolation
    let mut bracket_lo: Option<(f64, f64, f64)> = None; // (angle, eng, z) where eng < target
    let mut bracket_hi: Option<(f64, f64, f64)> = None; // (angle, eng, z) where eng > target

    for &offset_deg in &narrow_offsets {
        let angle = prev_angle + offset_deg.to_radians();
        let nx = cx + step_len * angle.cos();
        let ny = cy + step_len * angle.sin();

        if nx < bbox_x_min || nx > bbox_x_max || ny < bbox_y_min || ny > bbox_y_max {
            continue;
        }

        let surf_z = surface_hm.surface_z_at_world(nx, ny);
        if surf_z == f64::NEG_INFINITY {
            continue;
        }
        let z = (surf_z + stock_to_leave).max(z_level);
        let eng =
            compute_engagement_3d(material_hm, surface_hm, nx, ny, tool_radius, z_level, stock_to_leave);

        if eng < 0.001 {
            continue;
        }

        let error = (eng - target_frac).abs();
        let ad = angle_diff(angle, prev_angle).abs() / PI;
        let score = error + ad * 0.12;

        if best.is_none() || score < best.unwrap().0 {
            best = Some((score, angle, z));
        }

        // Track brackets for interpolation
        if eng < target_frac {
            if bracket_lo.is_none()
                || (eng - target_frac).abs() < (bracket_lo.unwrap().1 - target_frac).abs()
            {
                bracket_lo = Some((angle, eng, z));
            }
        } else if bracket_hi.is_none()
            || (eng - target_frac).abs() < (bracket_hi.unwrap().1 - target_frac).abs()
        {
            bracket_hi = Some((angle, eng, z));
        }
    }

    // Try bracket interpolation if we have both sides
    if let (Some(lo), Some(hi)) = (bracket_lo, bracket_hi)
        && (hi.1 - lo.1).abs() > 0.001
    {
        let t = (target_frac - lo.1) / (hi.1 - lo.1);
        let diff = angle_diff(hi.0, lo.0);
        let interp_angle = lo.0 + t * diff;

        if let Some((score, z)) = evaluate(interp_angle)
            && (best.is_none() || score < best.unwrap().0)
        {
            best = Some((score, interp_angle, z));
        }
    }


    // If narrow search found a good result, return it
    if let Some((score, angle, z)) = best
        && score < 0.15
    {
        return Some((angle, z));
    }

    // Phase 2: Forward sweep +/-90 degrees (19 candidates)
    let mut fallback: Option<(f64, f64, f64)> = best;
    for i in 0..19 {
        let angle = prev_angle - PI / 2.0 + (i as f64 / 18.0) * PI;
        if let Some((score, z)) = evaluate(angle)
            && (fallback.is_none() || score < fallback.unwrap().0)
        {
            fallback = Some((score, angle, z));
        }
    }

    if let Some((score, angle, z)) = fallback
        && score < 0.3
    {
        return Some((angle, z));
    }

    // Phase 3: Full 360 (36 candidates, allows U-turns)
    let mut fallback2: Option<(f64, f64, f64)> = fallback;
    for i in 0..36 {
        let angle = (i as f64 / 36.0) * TAU;
        if let Some((score, z)) = evaluate(angle)
            && (fallback2.is_none() || score < fallback2.unwrap().0)
        {
            fallback2 = Some((score, angle, z));
        }
    }

    fallback2.map(|(_, angle, z)| (angle, z))
}

// ── 3D entry point finding ────────────────────────────────────────────

/// Find the next entry point: a cell where material remains above the
/// effective floor at z_level.
fn find_entry_3d(
    material_hm: &Heightmap,
    surface_hm: &SurfaceHeightmap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z_level: f64,
    stock_to_leave: f64,
    last_pos: Option<P2>,
    pass_endpoints: &[P2],
    tool_radius: f64,
) -> Option<(P2, f64)> {
    let min_endpoint_dist_sq = (tool_radius * 3.0) * (tool_radius * 3.0);

    // Reference position for nearest search
    let ref_pos = last_pos.unwrap_or_else(|| {
        let cx = material_hm.origin_x
            + (material_hm.cols as f64 / 2.0) * material_hm.cell_size;
        let cy = material_hm.origin_y
            + (material_hm.rows as f64 / 2.0) * material_hm.cell_size;
        P2::new(cx, cy)
    });

    let mut best: Option<(f64, usize, usize)> = None; // (dist_sq, row, col)

    for row in 0..material_hm.rows {
        for col in 0..material_hm.cols {
            let mat_z = material_hm.get(row, col);
            let surf_z = surface_hm.surface_z_at(row, col);
            let floor = (surf_z + stock_to_leave).max(z_level);

            if mat_z <= floor + 0.01 {
                continue; // No material here
            }

            let (x, y) = material_hm.cell_to_world(row, col);

            // Skip cells near previous endpoints (entry spreading)
            let too_close = pass_endpoints.iter().any(|ep| {
                let dx = x - ep.x;
                let dy = y - ep.y;
                dx * dx + dy * dy < min_endpoint_dist_sq
            });
            if too_close && pass_endpoints.len() < 50 {
                // Allow after many passes to avoid deadlock
                continue;
            }

            let dx = x - ref_pos.x;
            let dy = y - ref_pos.y;
            let dist_sq = dx * dx + dy * dy;

            if best.is_none() || dist_sq < best.unwrap().0 {
                best = Some((dist_sq, row, col));
            }
        }
    }

    // If spreading excluded everything, retry without spreading
    if best.is_none() {
        for row in 0..material_hm.rows {
            for col in 0..material_hm.cols {
                let mat_z = material_hm.get(row, col);
                let surf_z = surface_hm.surface_z_at(row, col);
                let floor = (surf_z + stock_to_leave).max(z_level);

                if mat_z <= floor + 0.01 {
                    continue;
                }

                let (x, y) = material_hm.cell_to_world(row, col);
                let dx = x - ref_pos.x;
                let dy = y - ref_pos.y;
                let dist_sq = dx * dx + dy * dy;

                if best.is_none() || dist_sq < best.unwrap().0 {
                    best = Some((dist_sq, row, col));
                }
            }
        }
    }

    best.map(|(_, row, col)| {
        let (x, y) = material_hm.cell_to_world(row, col);
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        let z = (cl.z + stock_to_leave).max(z_level);
        (P2::new(x, y), z)
    })
}

// ── Link vs retract ───────────────────────────────────────────────────

/// Check if the tool can safely feed from `from` to `to` without hitting
/// excessive material above the cutting plane.
fn is_clear_path_3d(
    material_hm: &Heightmap,
    _surface_hm: &SurfaceHeightmap,
    from: P3,
    to: P3,
    _z_level: f64,
    _stock_to_leave: f64,
) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return true;
    }

    let n_samples = (len / (material_hm.cell_size * 2.0)).ceil() as usize;
    let n_samples = n_samples.max(2);
    let mut blocked = 0u32;

    for i in 0..=n_samples {
        let t = i as f64 / n_samples as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        let z = from.z + t * (to.z - from.z);

        if let Some((row, col)) = material_hm.world_to_cell(x, y) {
            let mat_z = material_hm.get(row, col);
            // Material significantly above our travel Z means collision
            if mat_z > z + 1.0 {
                blocked += 1;
            }
        }
    }

    let blocked_frac = blocked as f64 / (n_samples + 1) as f64;
    blocked_frac < 0.2
}

// ── 3D path simplification ───────────────────────────────────────────

/// Simplify a 3D path using Douglas-Peucker with 3D perpendicular distance.
fn simplify_path_3d(points: &[P3], tolerance: f64) -> Vec<P3> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let dz = last.z - first.z;
    let seg_len = (dx * dx + dy * dy + dz * dz).sqrt();

    if seg_len < 1e-10 {
        return vec![first, last];
    }

    // Find point farthest from the line first→last
    let mut max_dist = 0.0;
    let mut max_idx = 0;

    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        // Vector from first to p
        let vx = p.x - first.x;
        let vy = p.y - first.y;
        let vz = p.z - first.z;
        // Cross product magnitude: |v x d| / |d|
        let cx = vy * dz - vz * dy;
        let cy = vz * dx - vx * dz;
        let cz = vx * dy - vy * dx;
        let dist = (cx * cx + cy * cy + cz * cz).sqrt() / seg_len;
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist <= tolerance {
        return vec![first, last];
    }

    let mut left = simplify_path_3d(&points[..=max_idx], tolerance);
    let right = simplify_path_3d(&points[max_idx..], tolerance);
    left.pop(); // Remove duplicate at split point
    left.extend(right);
    left
}

/// Blend corners on a 3D path. Projects to 2D for geometry, interpolates Z.
fn blend_corners_3d(path: &[P3], min_radius: f64) -> Vec<P3> {
    if min_radius <= 0.0 || path.len() < 3 {
        return path.to_vec();
    }

    // Project to 2D, blend, then re-attach Z by parameter interpolation
    let path_2d: Vec<P2> = path.iter().map(|p| P2::new(p.x, p.y)).collect();
    let blended_2d = blend_corners(&path_2d, min_radius);

    if blended_2d.len() == path_2d.len() {
        // No blending happened, return original
        return path.to_vec();
    }

    // Re-attach Z: for each blended 2D point, find nearest original point's Z
    // and interpolate. Walk the original path to find the closest segment.
    blended_2d
        .iter()
        .map(|bp| {
            let z = interpolate_z_from_path(path, bp.x, bp.y);
            P3::new(bp.x, bp.y, z)
        })
        .collect()
}

/// Find Z at (x, y) by finding the nearest segment on the original 3D path
/// and interpolating linearly.
fn interpolate_z_from_path(path: &[P3], x: f64, y: f64) -> f64 {
    if path.is_empty() {
        return 0.0;
    }
    if path.len() == 1 {
        return path[0].z;
    }

    let mut best_dist_sq = f64::INFINITY;
    let mut best_z = path[0].z;

    for i in 0..path.len() - 1 {
        let a = &path[i];
        let b = &path[i + 1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let seg_len_sq = dx * dx + dy * dy;

        let t = if seg_len_sq < 1e-20 {
            0.0
        } else {
            ((x - a.x) * dx + (y - a.y) * dy) / seg_len_sq
        };
        let t = t.clamp(0.0, 1.0);

        let px = a.x + t * dx;
        let py = a.y + t * dy;
        let dist_sq = (x - px) * (x - px) + (y - py) * (y - py);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_z = a.z + t * (b.z - a.z);
        }
    }

    best_z
}

// ── Segment types ─────────────────────────────────────────────────────

enum Adaptive3dSegment {
    /// 3D cutting path with variable Z
    Cut(Vec<P3>),
    /// Retract to safe_z, rapid XY, plunge to entry
    Rapid(P3),
    /// Feed directly at cutting depth (no retract)
    Link(P3),
}

// ── Main loop ─────────────────────────────────────────────────────────

fn adaptive_3d_segments(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> Vec<Adaptive3dSegment> {
    let tool_radius = params.tool_radius;
    let r = cutter.radius();

    // Grid geometry: expand mesh bbox by cutter radius
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - r;
    let origin_y = bbox.min.y - r;
    let extent_x = bbox.max.x + r;
    let extent_y = bbox.max.y + r;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);

    // Initialize material heightmap at stock top
    let mut material_hm =
        Heightmap::from_stock(origin_x, origin_y, extent_x, extent_y, params.stock_top_z, cell_size);

    // Precompute surface heightmap (rayon parallel drop-cutter)
    debug!(cols = material_hm.cols, rows = material_hm.rows, "Precomputing surface heightmap");
    let surface_hm = SurfaceHeightmap::from_mesh(
        mesh,
        index,
        cutter,
        material_hm.origin_x,
        material_hm.origin_y,
        material_hm.rows,
        material_hm.cols,
        material_hm.cell_size,
        bbox.min.z,
    );

    // Compute Z levels: stock_top down to surface bottom + stock_to_leave
    let surface_bottom = surface_hm.min_z();
    let z_bottom = surface_bottom + params.stock_to_leave;
    let mut z_levels = Vec::new();
    let mut z = params.stock_top_z - params.depth_per_pass;
    while z > z_bottom {
        z_levels.push(z);
        z -= params.depth_per_pass;
    }
    z_levels.push(z_bottom); // Always include final level at the surface

    // Fix 5: Flat area detection — histogram surface Z, insert levels at shelves
    if params.detect_flat_areas {
        let total_cells = surface_hm.z_values.len();
        if total_cells > 0 {
            // Build histogram of surface Z values binned at tolerance resolution
            let bin_size = params.tolerance.max(0.05);
            let z_min_surf = surface_bottom;
            let z_max_surf = params.stock_top_z;
            let n_bins = ((z_max_surf - z_min_surf) / bin_size).ceil() as usize + 1;
            let mut histogram = vec![0u32; n_bins];
            for &sz in &surface_hm.z_values {
                let bin = ((sz - z_min_surf) / bin_size).floor() as usize;
                if bin < n_bins {
                    histogram[bin] += 1;
                }
            }
            // Bins with >2% of total cells represent flat features
            let threshold = (total_cells as f64 * 0.02) as u32;
            let mut flat_levels = Vec::new();
            for (i, &count) in histogram.iter().enumerate() {
                if count > threshold {
                    let flat_z = z_min_surf + (i as f64 + 0.5) * bin_size + params.stock_to_leave;
                    // Only insert if within the working range and not too close to existing levels
                    if flat_z > z_bottom + bin_size && flat_z < params.stock_top_z - bin_size {
                        let too_close = z_levels.iter().any(|&zl| (zl - flat_z).abs() < bin_size);
                        if !too_close {
                            flat_levels.push(flat_z);
                        }
                    }
                }
            }
            if !flat_levels.is_empty() {
                debug!(count = flat_levels.len(), "Detected flat area Z levels");
                z_levels.extend(flat_levels);
                z_levels.sort_by(|a, b| b.partial_cmp(a).unwrap()); // Top-down order
                z_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            }
        }
    }

    // Fix 4: Fine stepdown — insert intermediate Z levels between major levels
    if let Some(fine_step) = params.fine_stepdown
        && fine_step > 0.0 && fine_step < params.depth_per_pass
    {
        let major_levels = z_levels.clone();
        let mut all_levels = Vec::new();
        // Insert intermediates between stock_top and first level
        let first_start = params.stock_top_z;
        for window in std::iter::once(&first_start)
            .chain(major_levels.iter())
            .collect::<Vec<_>>()
            .windows(2)
        {
            let z_top = *window[0];
            let z_bot = *window[1];
            let mut iz = z_top - fine_step;
            while iz > z_bot + fine_step * 0.5 {
                all_levels.push(iz);
                iz -= fine_step;
            }
            all_levels.push(z_bot); // Always include the major level
        }
        all_levels.sort_by(|a, b| b.partial_cmp(a).unwrap());
        all_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        debug!(from = z_levels.len(), to = all_levels.len(), fine_step = fine_step, "Fine stepdown expanded Z levels");
        z_levels = all_levels;
    }

    info!(count = z_levels.len(), z_top = z_levels.first().copied().unwrap_or(0.0), z_bottom = z_levels.last().copied().unwrap_or(0.0), depth_per_pass = params.depth_per_pass, "Z levels computed");

    let target_frac = target_engagement_fraction(params.stepover, tool_radius);
    let step_len = cell_size * 1.5;
    let max_link_dist = params.max_stay_down_dist.unwrap_or(tool_radius * 6.0);

    // Expanded bbox for bounds checking in direction search
    let bbox_x_min = origin_x + tool_radius;
    let bbox_x_max = extent_x - tool_radius;
    let bbox_y_min = origin_y + tool_radius;
    let bbox_y_max = extent_y - tool_radius;

    let mut segments = Vec::new();
    let mut last_pos: Option<P3> = None;

    for (level_idx, &z_level) in z_levels.iter().enumerate() {
        let remaining = material_remaining_at_level(&material_hm, &surface_hm, z_level, params.stock_to_leave);
        if remaining < 0.005 {
            debug!(level = level_idx, z = z_level, "Skipping level, no material");
            continue;
        }
        debug!(level = level_idx, z = z_level, remaining_pct = remaining * 100.0, "Starting Z level");

        let mut pass_endpoints: Vec<P2> = Vec::new();
        let max_passes = 500;
        let mut pass_count = 0;
        let mut short_pass_streak = 0u32; // Consecutive short passes
        let min_productive_steps = 8; // Pass shorter than this is "unproductive"
        let warmup_passes = 50; // Don't bail during warmup

        loop {
            pass_count += 1;
            if pass_count > max_passes {
                break;
            }
            // After warmup, bail if too many consecutive short passes
            // (tool is circling narrow steep features unproductively)
            if pass_count > warmup_passes && short_pass_streak > 15 {
                debug!(short_passes = short_pass_streak, z = z_level, pass = pass_count, "Bailing from Z level");
                break;
            }
            // Check global material remaining periodically (expensive O(cells) scan)
            if pass_count % 20 == 1 {
                let rem = material_remaining_at_level(&material_hm, &surface_hm, z_level, params.stock_to_leave);
                if rem < 0.01 {
                    break;
                }
            }

            // Find entry point
            let last_2d = last_pos.map(|p| P2::new(p.x, p.y));
            let (entry_xy, entry_z) = match find_entry_3d(
                &material_hm,
                &surface_hm,
                mesh,
                index,
                cutter,
                z_level,
                params.stock_to_leave,
                last_2d,
                &pass_endpoints,
                tool_radius,
            ) {
                Some(e) => e,
                None => break,
            };

            let entry_3d = P3::new(entry_xy.x, entry_xy.y, entry_z);

            // Pre-flight check: test if there's any viable cutting direction
            // from this entry. Entries with material but no viable direction
            // (thin slivers, isolated cells) produce 1-step passes that waste time.
            let preflight_dir = search_direction_3d(
                &material_hm, &surface_hm,
                entry_xy.x, entry_xy.y, tool_radius, step_len,
                target_frac, 0.0, z_level, params.stock_to_leave,
                bbox_x_min, bbox_x_max, bbox_y_min, bbox_y_max,
            );
            if preflight_dir.is_none() {
                // No viable direction — force-clear this spot and move on
                stamp_tool_at(&mut material_hm, cutter, entry_xy.x, entry_xy.y, entry_z);
                for a in 0..8 {
                    let angle = (a as f64 / 8.0) * TAU;
                    let px = entry_xy.x + tool_radius * 0.5 * angle.cos();
                    let py = entry_xy.y + tool_radius * 0.5 * angle.sin();
                    stamp_tool_at(&mut material_hm, cutter, px, py, entry_z);
                }
                pass_endpoints.push(entry_xy);
                short_pass_streak += 1;
                continue;
            }

            // Link or retract to entry
            if let Some(last) = last_pos {
                let dx = entry_3d.x - last.x;
                let dy = entry_3d.y - last.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < max_link_dist
                    && is_clear_path_3d(
                        &material_hm,
                        &surface_hm,
                        last,
                        entry_3d,
                        z_level,
                        params.stock_to_leave,
                    )
                {
                    segments.push(Adaptive3dSegment::Link(entry_3d));
                } else {
                    segments.push(Adaptive3dSegment::Rapid(entry_3d));
                }
            } else {
                segments.push(Adaptive3dSegment::Rapid(entry_3d));
            }

            // Walk the adaptive path
            let mut path = vec![entry_3d];
            let mut cx = entry_xy.x;
            let mut cy = entry_xy.y;
            let mut cz = entry_z;

            // Initial direction
            let mut prev_angle = if let Some(last) = last_pos {
                (entry_xy.y - last.y).atan2(entry_xy.x - last.x)
            } else {
                0.0
            };

            // Stamp at entry
            stamp_tool_at(&mut material_hm, cutter, cx, cy, cz);

            // Direction smoothing buffer
            const SMOOTH_BUF_LEN: usize = 3;
            let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

            let max_steps = 5000;
            let mut idle_count = 0;

            for _ in 0..max_steps {
                // Snapshot local cells around tool for cheap idle detection
                let local_before = local_material_sum(&material_hm, cx, cy, tool_radius);

                // Smoothed direction
                let smoothed = if angle_buf.len() >= 2 {
                    average_angles(&angle_buf)
                } else {
                    prev_angle
                };

                // Search for direction
                let (angle, z_next) = match search_direction_3d(
                    &material_hm,
                    &surface_hm,
                    cx,
                    cy,
                    tool_radius,
                    step_len,
                    target_frac,
                    smoothed,
                    z_level,
                    params.stock_to_leave,
                    bbox_x_min,
                    bbox_x_max,
                    bbox_y_min,
                    bbox_y_max,
                ) {
                    Some(r) => r,
                    None => break,
                };

                // Move
                cx += step_len * angle.cos();
                cy += step_len * angle.sin();
                // Z-rate clamping: limit max descent per step to depth_per_pass.
                // Prevents tool from plunging freely on steep slopes.
                // Upward movement is always unrestricted (safe).
                let max_z_step = params.depth_per_pass;
                cz = z_next.max(cz - max_z_step);
                path.push(P3::new(cx, cy, cz));

                // Stamp
                stamp_tool_at(&mut material_hm, cutter, cx, cy, cz);

                // Update direction buffer
                if angle_buf.len() >= SMOOTH_BUF_LEN {
                    angle_buf.remove(0);
                }
                angle_buf.push(angle);

                // Idle detection: require BOTH no local material change AND
                // low engagement. The engagement threshold must be high enough
                // that the tool doesn't idle-break at the boundary of a cleared
                // ring when there's still material just inward (the direction
                // search will steer inward if given the chance).
                let local_after = local_material_sum(&material_hm, cx, cy, tool_radius);
                let local_delta = (local_before - local_after).abs();
                let engagement_here = compute_engagement_3d(
                    &material_hm, &surface_hm, cx, cy, tool_radius,
                    z_level, params.stock_to_leave,
                );
                if local_delta < 0.001 && engagement_here < 0.05 {
                    idle_count += 1;
                    if idle_count > 20 {
                        break;
                    }
                } else {
                    idle_count = 0;
                }

                prev_angle = angle;
            }

            let was_idle = idle_count > 20;

            let pass_steps = path.len();
            if pass_steps >= 2 {
                let endpoint = *path.last().unwrap();
                last_pos = Some(endpoint);
                pass_endpoints.push(P2::new(endpoint.x, endpoint.y));
                segments.push(Adaptive3dSegment::Cut(path));
            } else {
                last_pos = Some(entry_3d);
                pass_endpoints.push(entry_xy);
            }

            // Track pass productivity
            if pass_steps < min_productive_steps {
                short_pass_streak += 1;
            } else {
                short_pass_streak = 0;
            }

            // Force-clear around endpoint if idle to prevent revisiting
            if was_idle {
                stamp_tool_at(&mut material_hm, cutter, cx, cy, cz);
                // Clear a wider area by stamping in a small circle
                for a in 0..8 {
                    let angle = (a as f64 / 8.0) * TAU;
                    let px = cx + tool_radius * angle.cos();
                    let py = cy + tool_radius * angle.sin();
                    let surf_z = surface_hm.surface_z_at_world(px, py);
                    let pz = if surf_z == f64::NEG_INFINITY { cz } else { (surf_z + params.stock_to_leave).max(z_level) };
                    stamp_tool_at(&mut material_hm, cutter, px, py, pz);
                }
            }
        }

        debug!(passes = pass_count, z = z_level, "Completed Z level");

        // Boundary cleanup: waterline contours at this z_level
        let sampling = cell_size * 2.0;
        let contours = waterline_contours(mesh, index, cutter, z_level, sampling);
        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }
            segments.push(Adaptive3dSegment::Rapid(contour[0]));

            let mut cleanup_path = vec![contour[0]];
            for i in 0..contour.len() {
                let a = contour[i];
                let b = contour[(i + 1) % contour.len()];
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let len = (dx * dx + dy * dy).sqrt();
                let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
                for j in 1..=n_steps {
                    let t = j as f64 / n_steps.max(1) as f64;
                    let x = a.x + t * dx;
                    let y = a.y + t * dy;
                    let z = a.z + t * (b.z - a.z);
                    stamp_tool_at(&mut material_hm, cutter, x, y, z);
                    cleanup_path.push(P3::new(x, y, z));
                }
            }
            // Close loop
            cleanup_path.push(contour[0]);
            stamp_tool_at(&mut material_hm, cutter, contour[0].x, contour[0].y, contour[0].z);
            segments.push(Adaptive3dSegment::Cut(cleanup_path));
            last_pos = Some(contour[0]);
        }
    }

    segments
}

// ── Public API ────────────────────────────────────────────────────────

/// Generate a 3D adaptive clearing toolpath for roughing a mesh surface.
///
/// Starting from flat stock at `stock_top_z`, roughs out material following
/// the STL mesh surface with constant engagement control. Multi-level
/// passes from top to bottom, waterline boundary cleanup at each level.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(tool_radius = params.tool_radius, stepover = params.stepover))]
pub fn adaptive_3d_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> Toolpath {
    let segments = adaptive_3d_segments(mesh, index, cutter, params);

    let mut tp = Toolpath::new();
    if segments.is_empty() {
        return tp;
    }

    for segment in &segments {
        match segment {
            Adaptive3dSegment::Rapid(entry) => {
                match params.entry_style {
                    EntryStyle3d::Plunge => {
                        // Original: retract → rapid XY → plunge
                        tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                        tp.feed_to(*entry, params.plunge_rate);
                    }
                    EntryStyle3d::Helix { radius, pitch } => {
                        // Rapid to 2mm above entry, then helix down
                        let clearance = 2.0;
                        let helix_start_z = entry.z + clearance;
                        tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                        tp.rapid_to(P3::new(entry.x, entry.y, helix_start_z));

                        let dz = (helix_start_z - entry.z).abs();
                        if dz < 0.01 || pitch < 0.01 {
                            tp.feed_to(*entry, params.plunge_rate);
                        } else {
                            let revolutions = dz / pitch;
                            let steps_per_rev = 36;
                            let total_steps = (revolutions * steps_per_rev as f64).ceil() as usize;
                            if total_steps == 0 {
                                tp.feed_to(*entry, params.plunge_rate);
                            } else {
                                let total_angle = revolutions * TAU;
                                for i in 1..=total_steps {
                                    let t = i as f64 / total_steps as f64;
                                    let angle = total_angle * t;
                                    let z = helix_start_z - dz * t;
                                    let x = entry.x + radius * angle.cos();
                                    let y = entry.y + radius * angle.sin();
                                    tp.feed_to(P3::new(x, y, z), params.plunge_rate);
                                }
                                // Return to entry center at final Z
                                tp.feed_to(*entry, params.plunge_rate);
                            }
                        }
                    }
                    EntryStyle3d::Ramp { max_angle_deg } => {
                        // Rapid to 2mm above entry, then ramp down along a direction
                        let clearance = 2.0;
                        let ramp_start_z = entry.z + clearance;
                        tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                        tp.rapid_to(P3::new(entry.x, entry.y, ramp_start_z));

                        let dz = (ramp_start_z - entry.z).abs().max(0.1);
                        let max_angle_rad = max_angle_deg.to_radians();
                        let ramp_xy_len = dz / max_angle_rad.tan();
                        let half_len = ramp_xy_len / 2.0;

                        // Ramp along X direction (arbitrary), zigzag down
                        let mid_z = (ramp_start_z + entry.z) / 2.0;
                        tp.feed_to(
                            P3::new(entry.x + half_len, entry.y, mid_z),
                            params.plunge_rate,
                        );
                        tp.feed_to(*entry, params.plunge_rate);
                    }
                }
            }
            Adaptive3dSegment::Link(target) => {
                tp.feed_to(*target, params.feed_rate);
            }
            Adaptive3dSegment::Cut(path) => {
                if path.len() < 2 {
                    continue;
                }
                // Simplify in 3D, then blend corners
                let simplified = simplify_path_3d(path, params.tolerance);
                let blended = blend_corners_3d(&simplified, params.min_cutting_radius);

                for pt in blended.iter().skip(1) {
                    tp.feed_to(*pt, params.feed_rate);
                }
            }
        }
    }

    // Final retract
    if let Some(last) = tp.moves.last() {
        tp.rapid_to(P3::new(last.target.x, last.target.y, params.safe_z));
    }

    info!(moves = tp.moves.len(), cutting_mm = tp.total_cutting_distance(), rapid_mm = tp.total_rapid_distance(), "3D adaptive toolpath complete");

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::FlatEndmill;

    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn make_hemisphere_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn flat_cutter() -> FlatEndmill {
        FlatEndmill::new(6.35, 25.0)
    }

    fn default_params() -> Adaptive3dParams {
        Adaptive3dParams {
            tool_radius: 3.175,
            stepover: 2.0,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            tolerance: 0.1,
            min_cutting_radius: 0.0,
            stock_top_z: 25.0,
            entry_style: EntryStyle3d::Plunge,
            fine_stepdown: None,
            detect_flat_areas: false,
            max_stay_down_dist: None,
        }
    }

    // ── Surface heightmap tests ──────────────────────────────────────

    #[test]
    fn test_surface_heightmap_flat() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        // Grid within mesh footprint (mesh is 50x50, centered at origin)
        let shm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, -20.0, -20.0, 8, 8, 5.0, -10.0,
        );
        // Interior cells should have surface Z near 0 (flat mesh at z=0)
        // Edge cells might get min_z if outside mesh footprint
        let mut interior_count = 0;
        for row in 1..shm.rows - 1 {
            for col in 1..shm.cols - 1 {
                let z = shm.surface_z_at(row, col);
                assert!(
                    (-1.0..=1.0).contains(&z),
                    "Interior flat mesh Z should be near 0, got {:.2} at ({}, {})",
                    z, row, col
                );
                interior_count += 1;
            }
        }
        assert!(interior_count > 10, "Should have checked interior cells");
    }

    #[test]
    fn test_surface_heightmap_hemisphere() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let bbox = &mesh.bbox;
        let shm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            bbox.min.x - 5.0,
            bbox.min.y - 5.0,
            20,
            20,
            3.0,
            bbox.min.z,
        );

        // Center should be higher than edges
        let center_row = shm.rows / 2;
        let center_col = shm.cols / 2;
        let center_z = shm.surface_z_at(center_row, center_col);
        let edge_z = shm.surface_z_at(0, 0);
        assert!(
            center_z > edge_z,
            "Hemisphere center ({:.1}) should be higher than edge ({:.1})",
            center_z,
            edge_z
        );
    }

    // ── Engagement tests ──────────────────────────────────────────────

    #[test]
    fn test_engagement_3d_full_material() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        let material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_hm.origin_x,
            material_hm.origin_y,
            material_hm.rows,
            material_hm.cols,
            cell_size,
            -10.0,
        );

        // Stock at 20, surface near 0, z_level=10: everything above 10
        let eng = compute_engagement_3d(&material_hm, &surface_hm, 0.0, 0.0, 3.175, 10.0, 0.5);
        assert!(
            eng > 0.9,
            "Full material should give high engagement, got {:.2}",
            eng
        );
    }

    #[test]
    fn test_engagement_3d_cleared() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        // Stock already at surface level
        let material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 0.5, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_hm.origin_x,
            material_hm.origin_y,
            material_hm.rows,
            material_hm.cols,
            cell_size,
            -10.0,
        );

        let eng = compute_engagement_3d(&material_hm, &surface_hm, 0.0, 0.0, 3.175, 0.0, 0.5);
        assert!(
            eng < 0.1,
            "Cleared material should give low engagement, got {:.2}",
            eng
        );
    }

    #[test]
    fn test_engagement_3d_partial() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        let mut material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_hm.origin_x,
            material_hm.origin_y,
            material_hm.rows,
            material_hm.cols,
            cell_size,
            -10.0,
        );

        // Stamp tool at (3, 0) to clear half the area near (0, 0)
        stamp_tool_at(&mut material_hm, &cutter, 3.0, 0.0, 10.0);

        let eng = compute_engagement_3d(&material_hm, &surface_hm, 0.0, 0.0, 3.175, 10.0, 0.5);
        assert!(
            eng > 0.2 && eng < 0.9,
            "Partial material should give mid engagement, got {:.2}",
            eng
        );
    }

    // ── Direction search tests ─────────────────────────────────────────

    #[test]
    fn test_direction_search_finds_material() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        let material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter,
            material_hm.origin_x, material_hm.origin_y,
            material_hm.rows, material_hm.cols, cell_size, -10.0,
        );

        let target = target_engagement_fraction(2.0, 3.175);
        let result = search_direction_3d(
            &material_hm, &surface_hm,
            0.0, 0.0, 3.175, 1.5, target, 0.0, 10.0, 0.5,
            -25.0, 25.0, -25.0, 25.0,
        );
        assert!(result.is_some(), "Should find a direction with full material");
    }

    #[test]
    fn test_direction_search_no_material() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        // Stock at surface level — nothing to cut
        let material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 0.5, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter,
            material_hm.origin_x, material_hm.origin_y,
            material_hm.rows, material_hm.cols, cell_size, -10.0,
        );

        let target = target_engagement_fraction(2.0, 3.175);
        let result = search_direction_3d(
            &material_hm, &surface_hm,
            0.0, 0.0, 3.175, 1.5, target, 0.0, 0.0, 0.5,
            -25.0, 25.0, -25.0, 25.0,
        );
        assert!(result.is_none(), "Should find no direction when all cleared");
    }

    // ── Entry point tests ───────────────────────────────────────────────

    #[test]
    fn test_entry_3d_finds_remaining() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        let material_hm =
            Heightmap::from_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter,
            material_hm.origin_x, material_hm.origin_y,
            material_hm.rows, material_hm.cols, cell_size, -10.0,
        );

        let result = find_entry_3d(
            &material_hm, &surface_hm, &mesh, &si, &cutter,
            10.0, 0.5, None, &[], 3.175,
        );
        assert!(result.is_some(), "Should find entry with full material");
    }

    // ── Z level computation ─────────────────────────────────────────────

    #[test]
    fn test_z_level_computation() {
        let stock_top = 20.0;
        let depth_per_pass = 5.0;
        let surface_bottom = 0.0;
        let stock_to_leave = 0.5;
        let z_bottom = surface_bottom + stock_to_leave;

        let mut z_levels: Vec<f64> = Vec::new();
        let mut z = stock_top - depth_per_pass;
        while z > z_bottom {
            z_levels.push(z);
            z -= depth_per_pass;
        }
        z_levels.push(z_bottom);

        assert_eq!(z_levels.len(), 4, "Should have 4 levels: [15, 10, 5, 0.5]");
        assert!((z_levels[0] - 15.0_f64).abs() < 0.01);
        assert!((z_levels[1] - 10.0_f64).abs() < 0.01);
        assert!((z_levels[2] - 5.0_f64).abs() < 0.01);
        assert!((z_levels[3] - 0.5_f64).abs() < 0.01);
    }

    // ── Path simplification ─────────────────────────────────────────────

    #[test]
    fn test_simplify_path_3d() {
        // Collinear 3D points should simplify
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 1.0),
            P3::new(2.0, 0.0, 2.0),
            P3::new(3.0, 0.0, 3.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 2, "Collinear 3D points should reduce to 2");

        // Non-collinear should be preserved
        let path2 = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 5.0, 1.0),
            P3::new(2.0, 0.0, 2.0),
        ];
        let simplified2 = simplify_path_3d(&path2, 0.01);
        assert_eq!(simplified2.len(), 3, "Non-collinear should be preserved");
    }

    // ── Integration tests ───────────────────────────────────────────────

    #[test]
    fn test_adaptive_3d_flat_produces_toolpath() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0, // 5mm above flat mesh at z=0
            depth_per_pass: 5.0, // Single level
            stock_to_leave: 0.0,
            tolerance: 0.5, // Coarse for speed
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Should produce a non-trivial toolpath, got {} moves",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}mm",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_3d_hemisphere_multi_level() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 25.0, // Above hemisphere peak (~20)
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5, // Coarse for speed
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 20,
            "Hemisphere should produce multi-level passes, got {} moves",
            tp.moves.len()
        );

        // Z values should span from near stock_top down to near surface
        let min_z = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_z < 15.0,
            "Should cut down to lower Z levels, min feed Z = {:.1}",
            min_z
        );
    }

    #[test]
    fn test_adaptive_3d_z_follows_surface() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);

        // All cutting moves should be at or above stock_to_leave
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.target.z < params.safe_z - 1.0
            {
                assert!(
                    m.target.z >= params.stock_to_leave - 1.0,
                    "Cut Z ({:.2}) should be >= stock_to_leave ({:.1}) - tolerance",
                    m.target.z,
                    params.stock_to_leave
                );
            }
        }
    }

    // ── Fix 1: Z-rate clamping test ────────────────────────────────────

    #[test]
    fn test_z_rate_clamp_limits_descent() {
        // Verify that Z-rate clamping works in the internal stepping loop.
        // We test by calling adaptive_3d_segments directly and inspecting Cut paths
        // before simplification/blending.
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let depth_per_pass = 3.0;
        let params = Adaptive3dParams {
            stock_top_z: 25.0,
            depth_per_pass,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            ..default_params()
        };

        let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params);

        // Check raw Cut segments: consecutive points should not drop > depth_per_pass
        let mut checked = 0;
        for seg in &segments {
            if let Adaptive3dSegment::Cut(path) = seg {
                for window in path.windows(2) {
                    let z_drop = window[0].z - window[1].z;
                    if z_drop > 0.0 {
                        assert!(
                            z_drop <= depth_per_pass + 0.1,
                            "Raw path Z drop {:.2} exceeds depth_per_pass {:.1}",
                            z_drop, depth_per_pass,
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "Should have checked some downward Z moves");
    }

    // ── Fix 2: Helix entry test ────────────────────────────────────────

    #[test]
    fn test_helix_entry_no_vertical_plunge() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.0,
            tolerance: 0.5,
            entry_style: EntryStyle3d::Helix {
                radius: cutter.radius() * 0.8,
                pitch: 1.0,
            },
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(tp.moves.len() > 5, "Should produce a toolpath");

        // With helix entry, there should be feed moves that descend while
        // moving in XY (helix spiral). Individual helix steps are small,
        // so check for any downward-feed with XY motion.
        let mut has_helix_moves = false;
        for window in tp.moves.windows(2) {
            if let crate::toolpath::MoveType::Linear { .. } = window[1].move_type {
                let dx = (window[1].target.x - window[0].target.x).abs();
                let dy = (window[1].target.y - window[0].target.y).abs();
                let dz = window[0].target.z - window[1].target.z;
                // A helix step descends while moving in XY
                if dz > 0.005 && (dx > 0.01 || dy > 0.01) {
                    has_helix_moves = true;
                    break;
                }
            }
        }
        assert!(has_helix_moves, "Helix entry should produce moves with simultaneous XY+Z motion");
    }

    // ── Fix 4: Fine stepdown test ──────────────────────────────────────

    #[test]
    fn test_fine_stepdown_inserts_levels() {
        // Verify that fine_stepdown produces more Z levels
        let stock_top: f64 = 20.0;
        let depth_per_pass: f64 = 5.0;
        let fine_step: f64 = 1.0;
        let surface_bottom: f64 = 0.0;
        let stock_to_leave: f64 = 0.5;
        let z_bottom = surface_bottom + stock_to_leave;

        // Major levels only
        let mut major_levels = Vec::new();
        let mut z = stock_top - depth_per_pass;
        while z > z_bottom {
            major_levels.push(z);
            z -= depth_per_pass;
        }
        major_levels.push(z_bottom);
        let n_major = major_levels.len(); // Should be 4: [15, 10, 5, 0.5]

        // Fine stepdown levels
        let mut all_levels = Vec::new();
        let first_start = stock_top;
        for window in std::iter::once(&first_start)
            .chain(major_levels.iter())
            .collect::<Vec<_>>()
            .windows(2)
        {
            let z_top = *window[0];
            let z_bot = *window[1];
            let mut iz = z_top - fine_step;
            while iz > z_bot + fine_step * 0.5 {
                all_levels.push(iz);
                iz -= fine_step;
            }
            all_levels.push(z_bot);
        }
        all_levels.sort_by(|a, b| b.partial_cmp(a).unwrap());
        all_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);

        assert!(
            all_levels.len() > n_major * 3,
            "Fine stepdown should produce significantly more levels: {} vs {}",
            all_levels.len(),
            n_major
        );
        // With fine_step=1 and depth_per_pass=5, each major interval gets ~4 intermediates
        // Total should be around 19-20 levels
        assert!(
            all_levels.len() >= 15,
            "Expected at least 15 fine levels, got {}",
            all_levels.len()
        );
    }

    // ── Fix 5: Flat area detection test ────────────────────────────────

    #[test]
    fn test_flat_area_detection_finds_shelf() {
        // Build a surface heightmap where many cells sit at z=10 (a shelf)
        // and the rest sit at z=0 (floor)
        let cell_size = 1.0;
        let rows = 20;
        let cols = 20;
        let mut z_values = vec![0.0; rows * cols];
        // Create a shelf: rows 5..15, cols 5..15 at z=10
        for row in 5..15 {
            for col in 5..15 {
                z_values[row * cols + col] = 10.0;
            }
        }

        let shm = SurfaceHeightmap {
            z_values,
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };

        // Histogram detection logic (same as in adaptive_3d_segments)
        let tolerance: f64 = 0.1;
        let stock_to_leave: f64 = 0.5;
        let stock_top: f64 = 25.0;
        let total_cells = shm.z_values.len();
        let bin_size = tolerance.max(0.05);
        let z_min_surf = 0.0;
        let z_max_surf = stock_top;
        let n_bins = ((z_max_surf - z_min_surf) / bin_size).ceil() as usize + 1;
        let mut histogram = vec![0u32; n_bins];
        for &sz in &shm.z_values {
            let bin = ((sz - z_min_surf) / bin_size).floor() as usize;
            if bin < n_bins {
                histogram[bin] += 1;
            }
        }
        let threshold = (total_cells as f64 * 0.02) as u32;
        let mut flat_levels = Vec::new();
        let z_bottom = 0.0 + stock_to_leave;
        for (i, &count) in histogram.iter().enumerate() {
            if count > threshold {
                let flat_z = z_min_surf + (i as f64 + 0.5) * bin_size + stock_to_leave;
                if flat_z > z_bottom + bin_size && flat_z < stock_top - bin_size {
                    flat_levels.push(flat_z);
                }
            }
        }

        // Should detect the shelf at z≈10 (+stock_to_leave=0.5 → 10.5)
        let found_shelf = flat_levels.iter().any(|&z| (z - 10.5).abs() < 1.0);
        assert!(
            found_shelf,
            "Should detect shelf near z=10.5, found levels: {:?}",
            flat_levels
        );
    }
}
