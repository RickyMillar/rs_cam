//! 3D Z-level clearing engine for adaptive3d: region detection,
//! per-level contour-parallel and curvature-adaptive clearing,
//! stamping, and waterline cleanup.

use crate::adaptive_shared::average_angles;
use crate::contour_extract::{edt_curvature_field, marching_squares_bool_grid, smooth_grid};
use crate::debug_trace::{HotspotRecord, ToolpathDebugBounds2, ToolpathDebugContext};
use crate::dexel::{ray_subtract_above, ray_top};
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;
use crate::waterline::waterline_contours_with_cancel;
use std::collections::VecDeque;
use std::f64::consts::TAU;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::debug;

use super::path::Adaptive3dSegment;
use super::search::{
    compute_engagement_3d, find_entry_3d, is_clear_path_3d, material_remaining_at_level,
    material_remaining_in_region, search_direction_3d_with_metrics,
};
use super::{
    Adaptive3dRuntimeEvent, ClearingStrategy3d, local_material_sum, path_bounds_3d,
    stock_has_material_above,
};

// ── Region detection ──────────────────────────────────────────────────

/// A connected region of material detected by flood fill on the heightmap.
pub(super) struct MaterialRegion {
    pub(super) row_min: usize,
    pub(super) row_max: usize,
    pub(super) col_min: usize,
    pub(super) col_max: usize,
    /// World-space bounding box (expanded by tool_radius for direction search).
    pub(super) world_x_min: f64,
    pub(super) world_x_max: f64,
    pub(super) world_y_min: f64,
    pub(super) world_y_max: f64,
    pub(super) cell_count: usize,
    pub(super) surface_z_min: f64,
    pub(super) surface_z_max: f64,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Detect connected material regions via 8-connected BFS flood fill.
///
/// A cell "has material" if the top-Z of the dexel ray exceeds
/// `surface_z + stock_to_leave + 0.01`.
/// Regions with fewer than `min_cells` (default 4) are filtered out.
/// Returns regions sorted by cell_count descending (largest first).
pub(super) fn detect_material_regions(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
    tool_radius: f64,
) -> Vec<MaterialRegion> {
    let rows = material_stock.z_grid.rows;
    let cols = material_stock.z_grid.cols;
    let min_cells = 4usize;

    // Label grid: 0 = unlabeled, usize::MAX = no-material
    let mut labels = vec![0usize; rows * cols];

    // Mark cells that have no material
    for row in 0..rows {
        for col in 0..cols {
            let surf_z = surface_hm.surface_z_at(row, col);
            let floor = surf_z + stock_to_leave + 0.01;
            if !stock_has_material_above(material_stock, row, col, floor) {
                labels[row * cols + col] = usize::MAX;
            }
        }
    }

    let mut regions = Vec::new();
    let mut region_id = 1usize;
    let mut queue = VecDeque::new();

    for start_row in 0..rows {
        for start_col in 0..cols {
            let idx = start_row * cols + start_col;
            if labels[idx] != 0 {
                continue; // Already labeled or no material
            }

            // BFS flood fill for this region
            let mut rmin = start_row;
            let mut rmax = start_row;
            let mut cmin = start_col;
            let mut cmax = start_col;
            let mut count = 0usize;
            let mut sz_min = f64::INFINITY;
            let mut sz_max = f64::NEG_INFINITY;

            labels[idx] = region_id;
            queue.push_back((start_row, start_col));

            while let Some((r, c)) = queue.pop_front() {
                count += 1;
                rmin = rmin.min(r);
                rmax = rmax.max(r);
                cmin = cmin.min(c);
                cmax = cmax.max(c);
                let sz = surface_hm.surface_z_at(r, c);
                sz_min = sz_min.min(sz);
                sz_max = sz_max.max(sz);

                // 8-connected neighbors
                for dr in [-1i32, 0, 1] {
                    for dc in [-1i32, 0, 1] {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let nr = r as i32 + dr;
                        let nc = c as i32 + dc;
                        if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                            continue;
                        }
                        let nr = nr as usize;
                        let nc = nc as usize;
                        let ni = nr * cols + nc;
                        if labels[ni] == 0 {
                            labels[ni] = region_id;
                            queue.push_back((nr, nc));
                        }
                    }
                }
            }

            if count >= min_cells {
                let cs = material_stock.z_grid.cell_size;
                regions.push(MaterialRegion {
                    row_min: rmin,
                    row_max: rmax,
                    col_min: cmin,
                    col_max: cmax,
                    world_x_min: material_stock.z_grid.origin_u + cmin as f64 * cs - tool_radius,
                    world_x_max: material_stock.z_grid.origin_u + cmax as f64 * cs + tool_radius,
                    world_y_min: material_stock.z_grid.origin_v + rmin as f64 * cs - tool_radius,
                    world_y_max: material_stock.z_grid.origin_v + rmax as f64 * cs + tool_radius,
                    cell_count: count,
                    surface_z_min: sz_min,
                    surface_z_max: sz_max,
                });
            }

            region_id += 1;
        }
    }

    // Sort largest first
    regions.sort_by(|a, b| b.cell_count.cmp(&a.cell_count));
    regions
}

// ── Z-level clearing helper ──────────────────────────────────────────

/// Parameters for a single Z-level clearing pass, extracted to avoid
/// threading dozens of locals through the helper.
pub(super) struct ClearZLevelContext<'a> {
    pub(super) mesh: &'a TriangleMesh,
    pub(super) index: &'a SpatialIndex,
    pub(super) cutter: &'a dyn MillingCutter,
    pub(super) lut: &'a RadialProfileLUT,
    pub(super) slope_map: &'a SlopeMap,
    pub(super) debug: Option<ToolpathDebugContext>,
    pub(super) tool_radius: f64,
    pub(super) stepover: f64,
    pub(super) stock_to_leave: f64,
    pub(super) depth_per_pass: f64,
    pub(super) tolerance: f64,
    pub(super) target_frac: f64,
    pub(super) step_len: f64,
    pub(super) max_link_dist: f64,
    pub(super) bbox_x_min: f64,
    pub(super) bbox_x_max: f64,
    pub(super) bbox_y_min: f64,
    pub(super) bbox_y_max: f64,
    pub(super) clearing_strategy: ClearingStrategy3d,
    pub(super) z_blend: bool,
}

/// Pre-stamp thin material bands that appear at each Z level on steep walls.
///
/// After cutting at a previous Z level, wall cells retain material_z equal to that
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// level. At the new (lower) Z level, these cells have a thin band of material
/// (material_z - effective_floor) that is technically real but produces unproductive
/// contour passes. This function directly cuts those thin bands at the cell level,
/// leaving waterline cleanup to handle the actual wall boundaries.
///
/// Returns the number of cells pre-stamped.
pub(super) fn pre_stamp_thin_bands(
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    slope_map: &SlopeMap,
    z_level: f64,
    stock_to_leave: f64,
    depth_per_pass: f64,
    region: Option<&MaterialRegion>,
) -> u32 {
    let thin_threshold = depth_per_pass * 0.3;
    // Only pre-stamp on steep walls (>60°). Shallow areas have thin bands
    // that represent real productive material worth clearing with the adaptive spiral.
    let steep_threshold = 60.0_f64.to_radians();
    let mut stamped = 0u32;

    let grid = &material_stock.z_grid;
    let cols = grid.cols;
    let (row_min, row_max, col_min, col_max) = if let Some(r) = region {
        (
            r.row_min,
            r.row_max.min(grid.rows - 1),
            r.col_min,
            r.col_max.min(grid.cols - 1),
        )
    } else {
        (0, grid.rows - 1, 0, grid.cols - 1)
    };

    for row in row_min..=row_max {
        let base = row * cols;
        for col in col_min..=col_max {
            let i = base + col;

            // Skip shallow cells — only pre-stamp steep wall bands
            if slope_map.angles[i] < steep_threshold {
                continue;
            }

            let mat_z = ray_top(material_stock.z_grid.ray(row, col))
                .map(|z| z as f64)
                .unwrap_or(material_stock.stock_bbox.min.z);
            let surf_z = surface_hm.z_values[i];
            let effective_floor = (surf_z + stock_to_leave).max(z_level);
            let thickness = mat_z - effective_floor;
            if thickness > 0.01 && thickness < thin_threshold {
                ray_subtract_above(
                    material_stock.z_grid.ray_mut(row, col),
                    effective_floor as f32,
                );
                stamped += 1;
            }
        }
    }

    stamped
}

// ── Contour-parallel clearing ─────────────────────────────────────────

/// Build a padded boolean grid of material cells at a given Z level.
///
/// A cell is `true` if the stock has material above the effective floor
/// (max of surface_z + stock_to_leave, z_level). The grid is padded with
/// a 1-cell false border so marching squares and EDT detect edge boundaries.
///
/// Returns `(padded_grid, padded_rows, padded_cols, origin_x, origin_y, cell_size)`.
#[allow(clippy::indexing_slicing)] // SAFETY: padded grid indices bounded by loop ranges
fn build_material_bool_grid(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    region: Option<&MaterialRegion>,
) -> (Vec<bool>, usize, usize, f64, f64, f64) {
    let grid = &material_stock.z_grid;
    let rows = grid.rows;
    let cols = grid.cols;
    let cell_size = grid.cell_size;
    let origin_u = grid.origin_u;
    let origin_v = grid.origin_v;

    // Build padded boolean grid (1-cell false border so marching squares
    // detects edge boundaries).
    let padded_rows = rows + 2;
    let padded_cols = cols + 2;
    let mut padded_grid = vec![false; padded_rows * padded_cols];

    for row in 0..rows {
        for col in 0..cols {
            // Skip cells outside the region if one is specified.
            if let Some(r) = region
                && (row < r.row_min || row > r.row_max || col < r.col_min || col > r.col_max)
            {
                continue;
            }

            let surf_z = surface_hm.surface_z_at(row, col);
            let effective_floor = (surf_z + stock_to_leave).max(z_level);

            if stock_has_material_above(material_stock, row, col, effective_floor + 0.01) {
                // +1 offset for the border padding
                padded_grid[(row + 1) * padded_cols + (col + 1)] = true;
            }
        }
    }

    (
        padded_grid,
        padded_rows,
        padded_cols,
        origin_u - cell_size,
        origin_v - cell_size,
        cell_size,
    )
}

/// Stamp dexel stock along a 3D cutting path at `step_len` intervals.
fn stamp_along_path(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    path: &[P3],
    step_len: f64,
) {
    let first = match path.first() {
        Some(p) => *p,
        None => return,
    };
    // Stamp at the first point unconditionally.
    material_stock.stamp_tool_at(
        lut,
        tool_radius,
        first.x,
        first.y,
        first.z,
        StockCutDirection::FromTop,
    );
    let mut travel = 0.0;
    let mut prev = first;
    for pt in path {
        let dx = pt.x - prev.x;
        let dy = pt.y - prev.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        travel += seg_len;
        if travel >= step_len {
            material_stock.stamp_tool_at(
                lut,
                tool_radius,
                pt.x,
                pt.y,
                pt.z,
                StockCutDirection::FromTop,
            );
            travel = 0.0;
        }
        prev = *pt;
    }
    // Stamp at the last point regardless.
    material_stock.stamp_tool_at(
        lut,
        tool_radius,
        prev.x,
        prev.y,
        prev.z,
        StockCutDirection::FromTop,
    );
}

/// Clear a Z level using EDT-based contour-parallel strategy.
///
/// 1. Build a boolean material grid at the given z_level.
/// 2. Compute a Euclidean Distance Transform on the inverted (air) grid,
///    giving distance-to-nearest-air for each material cell.
/// 3. Threshold the EDT at successive stepover intervals to produce
///    concentric contour rings via marching squares.
/// 4. Surface-drape each 2D contour to 3D using the surface heightmap.
/// 5. Stamp dexel stock along each cutting path.
///
/// This replaces the polygon-offset approach which hung on fine tools
/// due to iterative `offset_polygon` on high-vertex polygons.
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level_contour_parallel(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Check material remaining — skip if negligible.
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        debug!(
            z = z_level,
            remaining, "CP: skipping — no material remaining"
        );
        return Ok(());
    }

    // 1. Build boolean material grid (material = true)
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
    );

    let mat_count = material_grid.iter().filter(|&&b| b).count();
    // Check if any material exists
    if mat_count == 0 {
        debug!(z = z_level, "CP: skipping — empty material grid");
        return Ok(());
    }

    // 2. Compute EDT on the INVERTED grid (air = true as source).
    //    This gives distance to nearest air cell for each material cell.
    //    Material cells near the boundary have small distance.
    //    Interior material cells have large distance.
    let air_grid: Vec<bool> = material_grid.iter().map(|&b| !b).collect();
    let edt = crate::contour_extract::distance_transform_2d(&air_grid, rows, cols);

    // 3. Find max distance (determines number of offset levels)
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // 4. Generate contours at each stepover threshold
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let stepover_cells = ctx.stepover / cell_size;

    debug!(
        z = z_level,
        remaining,
        mat_count,
        rows,
        cols,
        max_dist_cells = max_dist,
        tool_radius_cells = tool_radius_cells,
        stepover_cells = stepover_cells,
        "Contour-parallel EDT: generating offset contours"
    );

    // Z-blend: when enabled, outer contours stay flat at z_level and inner
    // contours progressively descend toward the terrain surface.
    let offset_range = max_dist - tool_radius_cells;
    let z_blend_enabled = ctx.z_blend;

    // The starting threshold determines the outermost contour offset. For wide
    // material regions (max_dist >> tool_radius), start at tool_radius_cells so
    // the tool's outer edge just reaches the boundary. For narrower or annular
    // regions, start lower so contours exist even in the narrowest sections where
    // EDT is small. Using min(tool_radius, stepover * 0.5) keeps the first
    // contour close enough to the boundary that even 2-3-cell-wide strips of
    // material have cells above the threshold.
    let mut threshold = tool_radius_cells.min(stepover_cells * 0.5).max(1.0);
    while threshold < max_dist {
        check_cancel(cancel)?;

        // Blend factor: 0.0 at outermost contour, 1.0 at innermost
        // Only active when z_blend is enabled; otherwise all passes cut at z_level.
        let blend = if z_blend_enabled && offset_range > 1e-6 {
            ((threshold - tool_radius_cells) / offset_range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Threshold the EDT: cells with distance > threshold are "inside" the offset
        let mask: Vec<bool> = edt.iter().map(|&d| d > threshold).collect();
        let loops = marching_squares_bool_grid(&mask, rows, cols, origin_x, origin_y, cell_size);

        for loop_pts in &loops {
            if loop_pts.len() < 3 {
                continue;
            }

            // Z-blended surface drape: outer passes cut near z_level (flat),
            // inner passes progressively descend toward the terrain surface.
            // This spreads Z movement across all passes instead of a sudden
            // plunge on the innermost pass.
            let mut path_3d: Vec<P3> = Vec::with_capacity(loop_pts.len());
            for p in loop_pts {
                let surf_z = surface_hm.surface_z_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave // actual terrain — may be below z_level
                };
                // Lerp: blend=0 → z_level (flat), blend=1 → target_z (terrain)
                // Clamp so we never cut below the next Z level's floor.
                let z = (z_level + blend * (target_z - z_level)).max(target_z);
                path_3d.push(P3::new(p.x, p.y, z));
            }

            // Emit entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                // Stay-down link if close to previous position, rapid otherwise
                let link_dist = ctx.tool_radius * 3.0;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                });
                if should_link {
                    segments.push(Adaptive3dSegment::Link(*first));
                } else {
                    segments.push(Adaptive3dSegment::Rapid(*first));
                }
                // Stamp dexel stock along the cutting path before moving path_3d
                stamp_along_path(
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    &path_3d,
                    ctx.step_len,
                );

                *last_pos = path_3d.last().copied();
                segments.push(Adaptive3dSegment::Cut(path_3d));
            }
        }

        threshold += stepover_cells;
    }

    // Cleanup: narrow sections of the material region (annular rings near steep
    // walls) may have EDT below the starting threshold, leaving them without a
    // contour pass. Identify remaining material cells and stamp a raster cleanup.
    let (cleanup_grid, cr, cc, co_x, co_y, c_cs) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
    );
    let cleanup_count = cleanup_grid.iter().filter(|&&b| b).count();
    if cleanup_count > 0 {
        // Raster through remaining material rows.  For each row with material
        // cells, build contiguous runs and emit one cut per run.
        let mut cleanup_pts: Vec<Vec<P3>> = Vec::new();
        for row in 0..cr {
            let mut run_start: Option<usize> = None;
            for col in 0..cc {
                // SAFETY: row*cc+col bounded by grid dimensions
                #[allow(clippy::indexing_slicing)]
                let is_mat = cleanup_grid[row * cc + col];
                if is_mat {
                    if run_start.is_none() {
                        run_start = Some(col);
                    }
                } else if let Some(start) = run_start.take() {
                    let mut path = Vec::new();
                    let mut c = start;
                    while c < col {
                        let wx = co_x + c as f64 * c_cs;
                        let wy = co_y + row as f64 * c_cs;
                        let surf_z = surface_hm.surface_z_at_world(wx, wy);
                        let z = if surf_z == f64::NEG_INFINITY {
                            z_level
                        } else {
                            (surf_z + ctx.stock_to_leave).max(z_level)
                        };
                        path.push(P3::new(wx, wy, z));
                        c += 1;
                    }
                    if path.len() >= 2 {
                        cleanup_pts.push(path);
                    }
                }
            }
            // Close any run that reached the end of the row
            if let Some(start) = run_start {
                let mut path = Vec::new();
                let mut c = start;
                while c < cc {
                    let wx = co_x + c as f64 * c_cs;
                    let wy = co_y + row as f64 * c_cs;
                    let surf_z = surface_hm.surface_z_at_world(wx, wy);
                    let z = if surf_z == f64::NEG_INFINITY {
                        z_level
                    } else {
                        (surf_z + ctx.stock_to_leave).max(z_level)
                    };
                    path.push(P3::new(wx, wy, z));
                    c += 1;
                }
                if path.len() >= 2 {
                    cleanup_pts.push(path);
                }
            }
        }

        for path in &cleanup_pts {
            if let Some(first) = path.first() {
                segments.push(Adaptive3dSegment::Rapid(*first));
                if path.len() >= 2 {
                    stamp_along_path(material_stock, ctx.lut, ctx.tool_radius, path, ctx.step_len);
                    *last_pos = path.last().copied();
                    segments.push(Adaptive3dSegment::Cut(path.clone()));
                } else {
                    // Single-point run: stamp and emit as a tiny cut segment
                    material_stock.stamp_tool_at(
                        ctx.lut,
                        ctx.tool_radius,
                        first.x,
                        first.y,
                        first.z,
                        StockCutDirection::FromTop,
                    );
                    let end = P3::new(first.x + ctx.step_len, first.y, first.z);
                    *last_pos = Some(end);
                    segments.push(Adaptive3dSegment::Cut(vec![*first, end]));
                }
            }
        }
    }

    Ok(())
}

/// Adaptive clearing: variable-offset EDT for constant tool engagement.
///
/// Same structure as `clear_z_level_contour_parallel` but uses a spatially-
/// varying threshold based on EDT level-set curvature.  At convex boundary
/// sections the stepover shrinks (preventing engagement spikes); at concave
/// sections it grows (avoiding wasted light passes).
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level_adaptive(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // ── Material check ─────────────────────────────────────────────────
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        return Ok(());
    }

    // ── 1. Build boolean material grid ─────────────────────────────────
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
    );

    if !material_grid.iter().any(|&b| b) {
        return Ok(());
    }

    // ── 2. EDT on inverted grid (distance to nearest air) ──────────────
    let air_grid: Vec<bool> = material_grid.iter().map(|&b| !b).collect();
    let edt = crate::contour_extract::distance_transform_2d(&air_grid, rows, cols);
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // ── 3. Curvature field from EDT level sets ─────────────────────────
    let mut curvature = edt_curvature_field(&edt, rows, cols);
    // Smooth to suppress finite-difference noise near the medial axis.
    // Scale with tool radius so the kernel covers ~1 tool diameter.
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let smooth_r = (tool_radius_cells as usize).max(3);
    smooth_grid(&mut curvature, rows, cols, smooth_r);

    // ── 4. Precompute per-cell curvature offset ────────────────────────
    // The offset is a CONSTANT shift per cell (does not scale with level N).
    // This keeps contour topology stable across levels while adjusting
    // local spacing based on curvature.
    //   offset = base_step * (−κR / (1 + κR))  clamped for stability
    //   Concave κ < 0: offset > 0 → contour recedes → wider pass
    //   Convex  κ > 0: offset < 0 → contour advances → tighter pass
    let total = rows * cols;
    let alpha = ctx.target_frac * std::f64::consts::TAU;
    let base_step = ctx.tool_radius * (1.0 - alpha.cos());
    let base_step_cells = base_step / cell_size;

    let mut curvature_offset = vec![0.0f64; total];
    for (off, &kappa) in curvature_offset.iter_mut().zip(curvature.iter()) {
        let kr = kappa * tool_radius_cells;
        let denom = (1.0 + kr).clamp(0.5, 2.0);
        // offset = base_step * (1/denom - 1), clamped to ±0.5 * base_step
        *off = (base_step_cells * (1.0 / denom - 1.0))
            .clamp(-0.5 * base_step_cells, 0.5 * base_step_cells);
    }

    // Z-blend setup (identical to contour-parallel)
    let offset_range = max_dist - tool_radius_cells;
    let z_blend_enabled = ctx.z_blend;

    debug!(
        z = z_level,
        max_dist_cells = max_dist,
        tool_radius_cells = tool_radius_cells,
        base_step_cells = base_step_cells,
        "Adaptive EDT: generating curvature-adjusted contours"
    );

    // ── 5. Offset loop: fixed base progression + constant curvature shift
    let mut threshold = tool_radius_cells;

    while threshold < max_dist {
        check_cancel(cancel)?;

        // Blend factor: 0.0 at outermost contour, 1.0 at innermost
        let blend = if z_blend_enabled && offset_range > 1e-6 {
            ((threshold - tool_radius_cells) / offset_range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Variable mask: base threshold + per-cell curvature offset
        let mask: Vec<bool> = edt
            .iter()
            .zip(curvature_offset.iter())
            .map(|(&d, &off)| d > threshold + off)
            .collect();
        let loops = marching_squares_bool_grid(&mask, rows, cols, origin_x, origin_y, cell_size);

        for loop_pts in &loops {
            if loop_pts.len() < 3 {
                continue;
            }

            // Z-blended surface drape (identical to contour-parallel)
            let mut path_3d: Vec<P3> = Vec::with_capacity(loop_pts.len());
            for p in loop_pts {
                let surf_z = surface_hm.surface_z_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave
                };
                let z = (z_level + blend * (target_z - z_level)).max(target_z);
                path_3d.push(P3::new(p.x, p.y, z));
            }

            // Entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                let link_dist = ctx.tool_radius * 3.0;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                });
                if should_link {
                    segments.push(Adaptive3dSegment::Link(*first));
                } else {
                    segments.push(Adaptive3dSegment::Rapid(*first));
                }
                stamp_along_path(
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    &path_3d,
                    ctx.step_len,
                );

                *last_pos = path_3d.last().copied();
                segments.push(Adaptive3dSegment::Cut(path_3d));
            }
        }

        threshold += base_step_cells;
    }

    Ok(())
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Clear material at a single Z level, optionally restricted to a region.
///
/// When `region` is `Some`, entry point search and material-remaining checks
/// are restricted to the region's bounding box, and direction search bbox
/// is clamped to the region's world extent.
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let tool_radius = ctx.tool_radius;

    let scan_bbox = region.map(|r| (r.row_min, r.row_max, r.col_min, r.col_max));

    let dir_x_min = region.map_or(ctx.bbox_x_min, |r| r.world_x_min.max(ctx.bbox_x_min));
    let dir_x_max = region.map_or(ctx.bbox_x_max, |r| r.world_x_max.min(ctx.bbox_x_max));
    let dir_y_min = region.map_or(ctx.bbox_y_min, |r| r.world_y_min.max(ctx.bbox_y_min));
    let dir_y_max = region.map_or(ctx.bbox_y_max, |r| r.world_y_max.min(ctx.bbox_y_max));

    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        return Ok(());
    }

    let level_scope = ctx.debug.as_ref().map(|debug_ctx| {
        let label = if let Some(region) = region {
            format!(
                "Z {:.3} region rows {}..{} cols {}..{}",
                z_level, region.row_min, region.row_max, region.col_min, region.col_max
            )
        } else {
            format!("Z {:.3}", z_level)
        };
        debug_ctx.start_span("z_level", label)
    });
    if let Some(scope) = level_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("remaining_before", remaining);
    }
    let level_ctx = level_scope.as_ref().map(|scope| scope.context());

    // Pre-stamp thin bands on steep walls to avoid unproductive contour re-tracing.
    let pre_stamp_scope = level_ctx
        .as_ref()
        .map(|debug_ctx| debug_ctx.start_span("pre_stamp", format!("Pre-stamp Z {:.3}", z_level)));
    let pre_stamped = pre_stamp_thin_bands(
        material_stock,
        surface_hm,
        ctx.slope_map,
        z_level,
        ctx.stock_to_leave,
        ctx.depth_per_pass,
        region,
    );
    if let Some(scope) = pre_stamp_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("cells", pre_stamped as f64);
    }
    if pre_stamped > 0 {
        debug!(
            cells = pre_stamped,
            z = z_level,
            "Pre-stamped thin wall bands"
        );
        // Re-check remaining after pre-stamp — skip level if negligible
        let remaining_after = if let Some(r) = region {
            material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
        } else {
            material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
        };
        if let Some(scope) = level_scope.as_ref() {
            scope.set_counter("remaining_after_prestamp", remaining_after);
        }
        if remaining_after < 0.005 {
            debug!(
                z = z_level,
                "Skipping Z level — thin bands consumed all remaining material"
            );
            if let Some(scope) = level_scope.as_ref() {
                scope.set_exit_reason("pre-stamp exhausted");
            }
            return Ok(());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let t_level = Instant::now();
    let mut pass_endpoints: Vec<P2> = Vec::new();
    let max_passes = 500;
    let mut pass_count = 0;
    let mut short_pass_streak = 0u32;
    let min_productive_steps = 8;
    let warmup_passes = 50;
    let mut total_steps = 0u64;
    let mut long_passes = 0u32;
    let mut short_passes = 0u32;
    let mut skipped_preflight = 0u32;

    loop {
        check_cancel(cancel)?;
        pass_count += 1;
        if pass_count > max_passes {
            break;
        }
        if pass_count > warmup_passes && short_pass_streak > 8 {
            debug!(
                short_passes = short_pass_streak,
                z = z_level,
                pass = pass_count,
                "Bailing from Z level"
            );
            break;
        }
        if pass_count % 20 == 1 {
            let rem = if let Some(r) = region {
                material_remaining_in_region(
                    material_stock,
                    surface_hm,
                    z_level,
                    ctx.stock_to_leave,
                    r,
                )
            } else {
                material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
            };
            if rem < 0.01 {
                break;
            }
        }

        let last_2d = last_pos.map(|p| P2::new(p.x, p.y));
        let pass_started = Instant::now();
        let pass_scope = level_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(z_level);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());
        let entry_scope = pass_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let Some((entry_xy, entry_z)) = find_entry_3d(
            material_stock,
            surface_hm,
            ctx.mesh,
            ctx.index,
            ctx.cutter,
            z_level,
            ctx.stock_to_leave,
            last_2d,
            &pass_endpoints,
            tool_radius,
            scan_bbox,
        ) else {
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("no entry");
            }
            break;
        };
        if let Some(scope) = entry_scope.as_ref() {
            scope.set_xy_bbox(ToolpathDebugBounds2 {
                min_x: entry_xy.x,
                max_x: entry_xy.x,
                min_y: entry_xy.y,
                max_y: entry_xy.y,
            });
            scope.set_z_level(entry_z);
        }

        let entry_3d = P3::new(entry_xy.x, entry_xy.y, entry_z);

        let preflight_scope = pass_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("preflight", format!("Preflight {pass_count}")));
        let preflight_dir = search_direction_3d_with_metrics(
            material_stock,
            surface_hm,
            entry_xy.x,
            entry_xy.y,
            tool_radius,
            ctx.step_len,
            ctx.target_frac,
            0.0,
            z_level,
            ctx.stock_to_leave,
            dir_x_min,
            dir_x_max,
            dir_y_min,
            dir_y_max,
        );
        if let Some(scope) = preflight_scope.as_ref() {
            scope.set_z_level(z_level);
            scope.set_counter(
                "evaluations",
                preflight_dir
                    .as_ref()
                    .map_or(0.0, |result| result.evaluations as f64),
            );
            if preflight_dir.is_none() {
                scope.set_exit_reason("no viable direction");
            }
        }
        if preflight_dir.is_none() {
            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                entry_xy.x,
                entry_xy.y,
                entry_z,
                StockCutDirection::FromTop,
            );
            for a in 0..8 {
                let angle = (a as f64 / 8.0) * TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let px = entry_xy.x + tool_radius * 0.5 * cos_a;
                let py = entry_xy.y + tool_radius * 0.5 * sin_a;
                material_stock.stamp_tool_at(
                    ctx.lut,
                    ctx.tool_radius,
                    px,
                    py,
                    entry_z,
                    StockCutDirection::FromTop,
                );
            }
            pass_endpoints.push(entry_xy);
            short_pass_streak += 1;
            skipped_preflight += 1;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassPreflightSkip {
                    pass_index: pass_count,
                },
            ));
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("preflight skip");
                scope.set_counter("skipped_preflight", 1.0);
            }
            continue;
        }

        segments.push(Adaptive3dSegment::Marker(
            Adaptive3dRuntimeEvent::PassEntry {
                pass_index: pass_count,
                entry_x: entry_xy.x,
                entry_y: entry_xy.y,
                entry_z,
            },
        ));

        if let Some(last) = *last_pos {
            let dx = entry_3d.x - last.x;
            let dy = entry_3d.y - last.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < ctx.max_link_dist
                && is_clear_path_3d(
                    material_stock,
                    surface_hm,
                    last,
                    entry_3d,
                    z_level,
                    ctx.stock_to_leave,
                )
            {
                segments.push(Adaptive3dSegment::Link(entry_3d));
            } else {
                segments.push(Adaptive3dSegment::Rapid(entry_3d));
            }
        } else {
            segments.push(Adaptive3dSegment::Rapid(entry_3d));
        }

        let mut path = vec![entry_3d];
        let mut cx = entry_xy.x;
        let mut cy = entry_xy.y;
        let mut cz = entry_z;

        let mut prev_angle = if let Some(last) = *last_pos {
            (entry_xy.y - last.y).atan2(entry_xy.x - last.x)
        } else {
            0.0
        };

        material_stock.stamp_tool_at(
            ctx.lut,
            ctx.tool_radius,
            cx,
            cy,
            cz,
            StockCutDirection::FromTop,
        );

        const SMOOTH_BUF_LEN: usize = 3;
        let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

        let max_steps = 5000;
        let mut idle_count = 0;
        let mut step_count = 0u32;
        let mut looped = false;
        let mut pass_removal_sum = 0.0f64;
        let mut search_evaluations = preflight_dir
            .as_ref()
            .map_or(0u32, |result| result.evaluations);

        // Loop detection: after enough steps to form a meaningful loop,
        // check if we've returned near the entry point. The minimum step
        // threshold avoids false positives at the start of a pass.
        let loop_min_steps = (tool_radius * 4.0 / ctx.step_len).ceil() as u32;
        let loop_close_dist_sq = (tool_radius * 1.5) * (tool_radius * 1.5);
        // Track the farthest we've been from entry — only trigger loop
        // detection once we've actually moved away first.
        let mut max_dist_from_entry_sq = 0.0f64;
        let min_excursion_sq = (tool_radius * 4.0) * (tool_radius * 4.0);

        for _ in 0..max_steps {
            check_cancel(cancel)?;
            let local_before = local_material_sum(material_stock, cx, cy, tool_radius);

            let smoothed = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            let Some(search_result) = search_direction_3d_with_metrics(
                material_stock,
                surface_hm,
                cx,
                cy,
                tool_radius,
                ctx.step_len,
                ctx.target_frac,
                smoothed,
                z_level,
                ctx.stock_to_leave,
                dir_x_min,
                dir_x_max,
                dir_y_min,
                dir_y_max,
            ) else {
                break;
            };
            search_evaluations += search_result.evaluations;
            let angle = search_result.angle;
            let z_next = search_result.z_next;

            let (sin_a, cos_a) = angle.sin_cos();
            cx += ctx.step_len * cos_a;
            cy += ctx.step_len * sin_a;
            let max_z_step = ctx.depth_per_pass;
            cz = z_next.max(cz - max_z_step);
            path.push(P3::new(cx, cy, cz));

            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                cx,
                cy,
                cz,
                StockCutDirection::FromTop,
            );

            if angle_buf.len() >= SMOOTH_BUF_LEN {
                angle_buf.remove(0);
            }
            angle_buf.push(angle);
            step_count += 1;

            // Loop detection: have we returned near the entry after travelling far enough?
            let dx_entry = cx - entry_xy.x;
            let dy_entry = cy - entry_xy.y;
            let dist_from_entry_sq = dx_entry * dx_entry + dy_entry * dy_entry;
            if dist_from_entry_sq > max_dist_from_entry_sq {
                max_dist_from_entry_sq = dist_from_entry_sq;
            }
            if step_count > loop_min_steps
                && max_dist_from_entry_sq > min_excursion_sq
                && dist_from_entry_sq < loop_close_dist_sq
            {
                looped = true;
                break;
            }

            let local_after = local_material_sum(material_stock, cx, cy, tool_radius);
            let local_delta = (local_before - local_after).abs();
            pass_removal_sum += local_delta;
            let engagement_here = compute_engagement_3d(
                material_stock,
                surface_hm,
                cx,
                cy,
                tool_radius,
                z_level,
                ctx.stock_to_leave,
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
        // Keep a reference to the path for post-pass widening before moving it
        let should_widen = (looped || pass_steps >= min_productive_steps) && pass_steps >= 4;
        let widen_path: Vec<P3> = if should_widen {
            // Denser sampling to avoid missing tight contours
            let skip = 1.max(path.len() / 500);
            path.iter().step_by(skip).copied().collect()
        } else {
            Vec::new()
        };

        let path_debug_bounds = path_bounds_3d(&path);

        if pass_steps >= 2 {
            // SAFETY: pass_steps >= 2 checked on line above
            #[allow(clippy::expect_used)]
            let endpoint = *path.last().expect("path is non-empty after loop");
            *last_pos = Some(endpoint);
            pass_endpoints.push(P2::new(endpoint.x, endpoint.y));
            segments.push(Adaptive3dSegment::Cut(path));
        } else {
            *last_pos = Some(entry_3d);
            pass_endpoints.push(entry_xy);
        }

        total_steps += pass_steps as u64;
        let exit_reason = if looped {
            "loop closed"
        } else if was_idle {
            "idle"
        } else {
            "no material"
        };

        // Low-yield detection: bail on passes that trace lots of steps but remove
        // negligible material (typical of thin wall contour re-tracing).
        let yield_ratio = if pass_steps > 1 {
            let expected = pass_steps as f64
                * ctx.stepover
                * ctx.depth_per_pass
                * material_stock.z_grid.cell_size;
            if expected > 0.0 {
                pass_removal_sum / expected
            } else {
                1.0
            }
        } else {
            1.0
        };
        let is_low_yield = pass_steps < min_productive_steps || yield_ratio < 0.05;
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_counter("step_count", pass_steps as f64);
            scope.set_counter("idle_count", idle_count as f64);
            scope.set_counter("search_evaluations", search_evaluations as f64);
            scope.set_counter("yield_ratio", yield_ratio);
            scope.set_counter("preflight_skipped", 0.0);
            scope.set_exit_reason(exit_reason);
            if let Some(bounds) = path_debug_bounds {
                scope.set_xy_bbox(bounds);
                let (center_x, center_y) = bounds.center();
                if let Some(debug_ctx) = pass_ctx.as_ref() {
                    debug_ctx.record_hotspot(&HotspotRecord {
                        kind: "adaptive3d_pass".into(),
                        center_x,
                        center_y,
                        z_level: Some(z_level),
                        bucket_size_xy: tool_radius * 2.0,
                        bucket_size_z: Some(ctx.tolerance.max(ctx.depth_per_pass * 0.5)),
                        elapsed_us: pass_started.elapsed().as_micros() as u64,
                        pass_count: 1,
                        step_count: pass_steps as u64,
                        low_yield_exit_count: u32::from(is_low_yield),
                    });
                }
            }
        }

        if is_low_yield {
            short_passes += 1;
            short_pass_streak += 1;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassSummary {
                    pass_index: pass_count,
                    step_count: pass_steps,
                    exit_reason: exit_reason.to_owned(),
                    yield_ratio,
                    short: true,
                },
            ));
        } else {
            long_passes += 1;
            short_pass_streak = 0;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassSummary {
                    pass_index: pass_count,
                    step_count: pass_steps,
                    exit_reason: exit_reason.to_owned(),
                    yield_ratio,
                    short: false,
                },
            ));
        }

        if was_idle {
            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                cx,
                cy,
                cz,
                StockCutDirection::FromTop,
            );
            for a in 0..8 {
                let angle = (a as f64 / 8.0) * TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let px = cx + tool_radius * cos_a;
                let py = cy + tool_radius * sin_a;
                let surf_z = surface_hm.surface_z_at_world(px, py);
                let pz = if surf_z == f64::NEG_INFINITY {
                    cz
                } else {
                    (surf_z + ctx.stock_to_leave).max(z_level)
                };
                material_stock.stamp_tool_at(
                    ctx.lut,
                    ctx.tool_radius,
                    px,
                    py,
                    pz,
                    StockCutDirection::FromTop,
                );
            }
        }

        // Widen the cleared band after loop-close or long contour passes.
        // Stamp perpendicular offsets at 1× and 2× stepover distance (double ring)
        // so adjacent parallel contours are also marked as cleared.
        if !widen_path.is_empty() {
            let widen_scope = pass_ctx
                .as_ref()
                .map(|debug_ctx| debug_ctx.start_span("widen_band", format!("Widen {pass_count}")));
            let widen_offset = ctx.stepover;
            for i in 1..widen_path.len() {
                let prev = &widen_path[i - 1];
                let curr = &widen_path[i];
                let dx = curr.x - prev.x;
                let dy = curr.y - prev.y;
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len < 1e-10 {
                    continue;
                }
                let nx = -dy / seg_len;
                let ny = dx / seg_len;
                for &mult in &[1.0f64, 2.0] {
                    for &sign in &[1.0f64, -1.0] {
                        let px = curr.x + sign * mult * widen_offset * nx;
                        let py = curr.y + sign * mult * widen_offset * ny;
                        let sz = surface_hm.surface_z_at_world(px, py);
                        if sz != f64::NEG_INFINITY {
                            let pz = (sz + ctx.stock_to_leave).max(z_level);
                            material_stock.stamp_tool_at(
                                ctx.lut,
                                ctx.tool_radius,
                                px,
                                py,
                                pz,
                                StockCutDirection::FromTop,
                            );
                        }
                    }
                }
            }
            if let Some(scope) = widen_scope.as_ref() {
                scope.set_z_level(z_level);
                scope.set_counter("sample_points", widen_path.len() as f64);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let level_ms = t_level.elapsed().as_millis() as u64;
    #[cfg(target_arch = "wasm32")]
    let level_ms = 0u64;
    debug!(
        passes = pass_count,
        long = long_passes,
        short = short_passes,
        skipped = skipped_preflight,
        total_steps = total_steps,
        z = z_level,
        elapsed_ms = level_ms,
        "Completed Z level"
    );
    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("passes", pass_count as f64);
        scope.set_counter("long_passes", long_passes as f64);
        scope.set_counter("short_passes", short_passes as f64);
        scope.set_counter("skipped_preflight", skipped_preflight as f64);
        scope.set_counter("total_steps", total_steps as f64);
        scope.set_z_level(z_level);
    }

    Ok(())
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Run waterline boundary cleanup at a given Z level.
///
/// When `slope_map` is provided, only traces contours through steep regions
/// (slope angle > 30°). This avoids re-tracing shallow areas that the
/// adaptive spiral already cleared.
#[allow(clippy::too_many_arguments)]
pub(super) fn waterline_cleanup(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    lut: &RadialProfileLUT,
    slope_map: &SlopeMap,
    material_stock: &mut TriDexelStock,
    z_level: f64,
    tool_radius: f64,
    cell_size: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    #[cfg(not(target_arch = "wasm32"))]
    let t_waterline = Instant::now();
    let waterline_scope = debug_ctx.map(|ctx| {
        ctx.start_span(
            "waterline_cleanup",
            format!("Waterline cleanup Z {:.3}", z_level),
        )
    });
    let sampling = tool_radius.max(cell_size * 4.0);
    let contours = waterline_contours_with_cancel(mesh, index, cutter, z_level, sampling, cancel)?;

    // Threshold for steep-only waterline: only trace contours where slope > 30°.
    // This eliminates redundant shallow-area waterline passes.
    let steep_threshold = 30.0_f64.to_radians();

    let mut traced = 0u32;
    for contour in &contours {
        check_cancel(cancel)?;
        if contour.len() < 3 {
            continue;
        }

        // Check if this contour is predominantly in a steep region.
        // Sample a few points and check the slope. If most are shallow, skip.
        let sample_step = 1.max(contour.len() / 10);
        let steep_samples = contour
            .iter()
            .step_by(sample_step)
            .filter(|p| {
                slope_map
                    .angle_at_world(p.x, p.y)
                    .is_some_and(|a| a >= steep_threshold)
            })
            .count();
        let total_samples = contour.len().div_ceil(sample_step);
        if total_samples > 0 && steep_samples * 3 < total_samples {
            // Less than 1/3 of samples are steep — skip this contour
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
                material_stock.stamp_tool_at(lut, tool_radius, x, y, z, StockCutDirection::FromTop);
                cleanup_path.push(P3::new(x, y, z));
            }
        }
        cleanup_path.push(contour[0]);
        material_stock.stamp_tool_at(
            lut,
            tool_radius,
            contour[0].x,
            contour[0].y,
            contour[0].z,
            StockCutDirection::FromTop,
        );
        segments.push(Adaptive3dSegment::Cut(cleanup_path));
        *last_pos = Some(contour[0]);
        traced += 1;
    }
    if !contours.is_empty() {
        #[cfg(not(target_arch = "wasm32"))]
        let wl_ms = t_waterline.elapsed().as_millis() as u64;
        #[cfg(target_arch = "wasm32")]
        let wl_ms = 0u64;
        debug!(
            total = contours.len(),
            traced = traced,
            z = z_level,
            elapsed_ms = wl_ms,
            "Waterline cleanup (slope-filtered)"
        );
    }
    if let Some(scope) = waterline_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("contours", contours.len() as f64);
        scope.set_counter("traced", traced as f64);
    }

    Ok(())
}
