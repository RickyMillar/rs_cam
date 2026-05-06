//! 3D Z-level clearing engine for adaptive3d: region detection,
//! per-level contour-parallel and curvature-adaptive clearing,
//! stamping, and waterline cleanup.

use crate::contour_extract::{edt_curvature_field, marching_squares_bool_grid, smooth_grid};
use crate::debug_trace::ToolpathDebugContext;
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;
use crate::waterline::waterline_contours_with_cancel;
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::debug;

use super::path::Adaptive3dSegment;
use super::search::{is_clear_path_3d, material_remaining_at_level, material_remaining_in_region};
use super::{ClearingStrategy3d, stock_has_material_above, stock_top_z_at};

// ── Region detection ──────────────────────────────────────────────────

/// A connected region of material detected by flood fill on the heightmap.
#[allow(dead_code)] // Some fields are strategy-specific and only read by some strategies.
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
#[allow(dead_code)] // Some fields are strategy-specific (ContourParallel, Adaptive, AgentSearch-2d).
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

            // Pick the entry point that requires the shallowest plunge
            // through fresh material. The natural starting point of the
            // marching-squares contour is wherever the algorithm's pixel
            // walk happened to begin, which can land on full-height stock
            // for a deeper Z-level pass — leading to a deep vertical
            // plunge through material from safe_z down to z_level.
            //
            // Rotating the closed loop so the point with the *lowest
            // current stock_top* is first means the plunge passes through
            // mostly already-cleared air, then bites only the depth-of-cut
            // worth of material at the bottom. Same total cut area, same
            // contour shape — just a kinder entry XY for closed loops.
            //
            // Only applies to closed contour loops (>= 3 pts); open
            // single-segment cleanup paths are left alone.
            if path_3d.len() >= 3 {
                let stock_top_at_path_idx = |idx: usize| -> f64 {
                    #[allow(clippy::indexing_slicing)] // idx < path_3d.len() by construction
                    let p = &path_3d[idx];
                    match material_stock.z_grid.world_to_cell(p.x, p.y) {
                        Some((row, col)) => stock_top_z_at(material_stock, row, col),
                        None => f64::INFINITY,
                    }
                };
                let best = (0..path_3d.len()).min_by(|&a, &b| {
                    let ta = stock_top_at_path_idx(a);
                    let tb = stock_top_at_path_idx(b);
                    ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(idx) = best
                    && idx > 0
                {
                    path_3d.rotate_left(idx);
                }
            }

            // Emit entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                // Stay-down link if close to previous position AND the link
                // path is clear of material. Without the is_clear_path_3d
                // gate, z_blend=true produced Link moves that crossed
                // uncut terrain between rings at different Z heights
                // (F-5 in planning/adaptive_review_2026-04.md). The gate
                // matches the one in clear_z_level (the AgentSearch path).
                let link_dist = ctx.max_link_dist;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                        && is_clear_path_3d(
                            material_stock,
                            surface_hm,
                            lp,
                            *first,
                            z_level,
                            ctx.stock_to_leave,
                        )
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

            // Entry (link or rapid) + cut segment. Matches the gate in
            // clear_z_level_contour_parallel — see F-5 rationale there.
            if let Some(first) = path_3d.first() {
                let link_dist = ctx.max_link_dist;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                        && is_clear_path_3d(
                            material_stock,
                            surface_hm,
                            lp,
                            *first,
                            z_level,
                            ctx.stock_to_leave,
                        )
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

/// Sample the current dexel stock top at world XY. Returns `None`
/// when the XY falls outside the grid (caller should fall back to
/// the conservative full-safe_z plunge). Returns `f64::NEG_INFINITY`
/// for the "everything cleared" case is collapsed to `None` too —
/// the caller can't usefully rapid down to negative infinity.
fn sample_stock_top_at(material_stock: &TriDexelStock, x: f64, y: f64) -> Option<f64> {
    let (row, col) = material_stock.z_grid.world_to_cell(x, y)?;
    let top_z = stock_top_z_at(material_stock, row, col);
    top_z.is_finite().then_some(top_z)
}

// ── 2.5D slice adaptive (AgentSearch strategy) ─────────────────────────

/// Signed polygon area via the shoelace formula. Positive = CCW.
fn polygon_signed_area(points: &[P2]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut acc = 0.0;
    for i in 0..n {
        #[allow(clippy::indexing_slicing)] // SAFETY: i < n, (i+1) % n < n
        let a = points[i];
        #[allow(clippy::indexing_slicing)]
        let b = points[(i + 1) % n];
        acc += a.x * b.y - b.x * a.y;
    }
    0.5 * acc
}

/// AgentSearch via 2.5D slices: at each Z-level, extract the 2D
/// material polygon via marching squares, then run the proven 2D
/// `adaptive_segments_with_debug` on it. Lift the resulting 2D path
/// back to 3D (Z clamped to `z_level` or surface+stock_to_leave) and
/// stamp the dexel stock along it.
///
/// This replaces the ~700-line 3D agent-based search that struggled
/// with surface-following, axial engagement, and boundary walking by
/// delegating to the working 2D adaptive implementation. The trade-off
/// is that the tool stays at a fixed Z within each slab (no per-step
/// terrain follow) — acceptable for roughing; finish passes handle
/// the staircase.
#[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
pub(super) fn clear_z_level_agent_2d_slice(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        return Ok(());
    }

    let level_scope = ctx.debug.as_ref().map(|debug_ctx| {
        let label = if let Some(r) = region {
            format!(
                "Z {:.3} region rows {}..{} cols {}..{}",
                z_level, r.row_min, r.row_max, r.col_min, r.col_max
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

    // 1. Material boolean grid at this Z-level (includes 1-cell air padding).
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

    // 2. Marching squares → polygon contours.
    let contours = crate::contour_extract::marching_squares_bool_grid(
        &material_grid,
        rows,
        cols,
        origin_x,
        origin_y,
        cell_size,
    );
    if contours.is_empty() {
        return Ok(());
    }

    // 3. Group contours into disjoint regions with their contained holes.
    //
    //    Marching squares emits one contour per material/air boundary, both
    //    outer boundaries (CCW, positive signed area) and hole boundaries
    //    (CW, negative). For multi-region slices — e.g. terrain hills
    //    emerging as separate islands at shallow Z — there are multiple
    //    outer boundaries that must each be cleared independently.
    //
    //    The previous implementation flattened signed area to absolute
    //    value, treated the largest contour as the only outer, and pushed
    //    every other contour into that one polygon's `holes` list. Disjoint
    //    islands got misclassified as holes — the 2D adaptive then treated
    //    them as already-cleared interior pockets, walking around them and
    //    plunging into them at "safe" XYs. Visible symptom: drilled holes
    //    through fresh stock, low engagement, high air-cut on terrain-shaped
    //    geometry.
    //
    //    `polygon::detect_containment` does the right thing: builds each
    //    contour as a single-loop polygon, runs containment tests, and
    //    returns N outer regions each with their nested holes (CW-flipped)
    //    attached.
    let single_loops: Vec<crate::polygon::Polygon2> = contours
        .into_iter()
        .filter(|pts| polygon_signed_area(pts).abs() > cell_size * cell_size)
        .map(|pts| crate::polygon::Polygon2 {
            exterior: pts,
            holes: Vec::new(),
            closed: true,
        })
        .collect();
    if single_loops.is_empty() {
        return Ok(());
    }
    let mut regions = crate::polygon::detect_containment(single_loops);

    // Fusion-style "ignore stock smaller than X" filter. Heightmap-style
    // models (e.g. terrain.stl) emit many micro-peaks at top Z levels
    // — each becomes its own region with its own perimeter sweep + 2D
    // adaptive entry/exit. The cutter spends most of its in-cut time
    // travelling between them, technically at feed_rate but barely
    // engaging material. On wanaka this drove 81% air-cut at every Z.
    //
    // Threshold = (2 × tool_diameter)² ≈ "the tool footprint plus an
    // offset ring fits". Anything smaller is sub-tool noise the cutter
    // can't address efficiently anyway. Bottom-Z waterline cleanup
    // catches surviving material; the finishing pass mops up the rest.
    let tool_diameter = ctx.tool_radius * 2.0;
    let min_region_area_mm2 = (tool_diameter * 2.0).powi(2);
    let region_count_before = regions.len();
    regions.retain(|r| r.area().abs() >= min_region_area_mm2);
    let dropped_micro = region_count_before - regions.len();
    if dropped_micro > 0 {
        debug!(
            z = z_level,
            dropped = dropped_micro,
            kept = regions.len(),
            threshold_mm2 = min_region_area_mm2,
            "Dropped sub-tool regions (Fusion-style ignore-features filter)"
        );
    }
    if regions.is_empty() {
        return Ok(());
    }

    // 4. Build 2D adaptive params from 3D context.
    let params_2d = crate::adaptive::AdaptiveParams {
        tool_radius: ctx.tool_radius,
        stepover: ctx.stepover,
        // cut_depth / feed_rate / plunge_rate / safe_z are unused by
        // `adaptive_segments_with_debug`; only the final `segments_to_toolpath`
        // consumes them. We're using segments directly here.
        cut_depth: 0.0,
        feed_rate: 0.0,
        plunge_rate: 0.0,
        safe_z: 0.0,
        tolerance: ctx.tolerance,
        slot_clearing: false,
        min_cutting_radius: 0.0,
        initial_stock: None,
    };

    // 5. Lift 2D points to 3D, respecting terrain peaks above z_level.
    let lift = |p: P2| -> P3 {
        let surf_z = surface_hm.surface_z_at_world(p.x, p.y);
        let z = if surf_z == f64::NEG_INFINITY {
            z_level
        } else {
            (surf_z + ctx.stock_to_leave).max(z_level)
        };
        P3::new(p.x, p.y, z)
    };

    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("region_count", regions.len() as f64);
    }

    // 6. Run 2D adaptive on each region independently, lift segments,
    //    stamp dexel stock. Each region's first segment from the 2D adaptive
    //    is a Rapid (= entry into that region), which becomes a 3D Rapid
    //    (retract to safe_z, traverse XY, plunge). Between disjoint regions
    //    that retract is correct; the 2D adaptive can't link across regions
    //    because each call sees only its own region's polygon.
    let mut cut_count = 0u32;
    let region_total = regions.len();
    for (region_idx, region_polygon) in regions.iter().enumerate() {
        let region_scope = level_ctx.as_ref().map(|debug_ctx| {
            debug_ctx.start_span(
                "agent2d_region",
                format!(
                    "region {}/{} @ Z {:.3}",
                    region_idx + 1,
                    region_total,
                    z_level
                ),
            )
        });

        // Perimeter sweep: trace the polygon boundary inset by tool_radius.
        //
        // The 2D adaptive insets the polygon by `tool_radius` to compute
        // its machinable region, then walks an "agent search" path inside.
        // That walk doesn't reliably sweep the full machinable boundary —
        // its spiral starts at an entry and works inward, leaving cells
        // in the outermost band (within tool_radius of polygon boundary)
        // touched only sporadically. Successive z-levels miss the same
        // cells, leaving stock that gets cleared at the deepest pass with
        // a single sample's axial DOC equal to the full uncleared depth
        // (~18mm on a 25mm stock).
        //
        // Pre-emit a Cut segment that follows the inset boundary for each
        // outer loop (and each hole). Cutter walks this contour at z_level,
        // stamping cells within tool_radius of the contour — fully covering
        // the polygon's outer band before the 2D adaptive starts. Same
        // dexel update strategy as the 2D adaptive's Cut segments.
        // Inset by tool_radius + a small margin so the perimeter sweep
        // sits SAFELY INSIDE the effective_boundary (silhouette inset by
        // tool_radius for containment=Inside). Without the margin, the
        // perimeter sits ON the effective_boundary edge and the post-
        // generation `clip_toolpath_to_boundary` flips its `contains_point`
        // result on edge-case points, converting these cuts to rapids and
        // leaving the boundary band unstamped — which is exactly the
        // wanaka 18mm-DOC failure mode (O5b/O5c).
        const PERIMETER_INSET_MARGIN_MM: f64 = 0.25;
        let inset_polygons = crate::polygon::offset_polygon(
            region_polygon,
            ctx.tool_radius + PERIMETER_INSET_MARGIN_MM,
        );
        for inset in &inset_polygons {
            if inset.exterior.len() >= 3 {
                // simplify_path_3d (RDP) early-returns on zero-length
                // baselines (first == last), which collapses any closed
                // loop to two coincident points → no-op feed. cavalier-
                // contours' parallel_offset output INCLUDES the closing
                // duplicate vertex, so we must drop it explicitly to
                // keep the path open for RDP. Without this drop, the
                // entire perimeter sweep silently became a no-op.
                let mut path_2d = inset.exterior.clone();
                if path_2d.len() >= 2
                    && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                    && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
                {
                    path_2d.pop();
                }
                let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                for p in &path_3d {
                    material_stock.stamp_tool_at(
                        ctx.lut,
                        ctx.tool_radius,
                        p.x,
                        p.y,
                        p.z,
                        StockCutDirection::FromTop,
                    );
                }
                if let Some(first) = path_3d.first().copied() {
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    match rapid_floor {
                        Some(top_z) => segments.push(Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        }),
                        None => segments.push(Adaptive3dSegment::Rapid(first)),
                    }
                }
                if path_3d.len() >= 2 {
                    segments.push(Adaptive3dSegment::Cut(path_3d.clone()));
                    cut_count += 1;
                }
                if let Some(last) = path_3d.last().copied() {
                    *last_pos = Some(last);
                }
            }
            // Sweep each hole's boundary too — cutter must clear material
            // around each hole's perimeter, not just the outer.
            for hole in &inset.holes {
                if hole.len() < 3 {
                    continue;
                }
                // Same closing-duplicate handling as the exterior sweep:
                // drop it if present to keep simplify_path_3d's RDP working.
                let mut path_2d = hole.clone();
                if path_2d.len() >= 2
                    && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                    && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
                {
                    path_2d.pop();
                }
                let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                for p in &path_3d {
                    material_stock.stamp_tool_at(
                        ctx.lut,
                        ctx.tool_radius,
                        p.x,
                        p.y,
                        p.z,
                        StockCutDirection::FromTop,
                    );
                }
                if let Some(first) = path_3d.first().copied() {
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    match rapid_floor {
                        Some(top_z) => segments.push(Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        }),
                        None => segments.push(Adaptive3dSegment::Rapid(first)),
                    }
                }
                if path_3d.len() >= 2 {
                    segments.push(Adaptive3dSegment::Cut(path_3d.clone()));
                    cut_count += 1;
                }
                if let Some(last) = path_3d.last().copied() {
                    *last_pos = Some(last);
                }
            }
        }

        let segs_2d = crate::adaptive::adaptive_segments_with_debug(
            region_polygon,
            &params_2d,
            cancel,
            region_scope.as_ref().map(|s| s.context()).as_ref(),
        )?;

        for seg in segs_2d {
            match seg {
                crate::adaptive::AdaptiveSegment::Cut(path_2d) => {
                    if path_2d.is_empty() {
                        continue;
                    }
                    let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                    // Stamp along the full path so the next Z-level sees
                    // every XY the cutter will reach (regardless of how
                    // we partition the path into Cut + RapidWithFloor
                    // sub-segments below).
                    for p in &path_3d {
                        material_stock.stamp_tool_at(
                            ctx.lut,
                            ctx.tool_radius,
                            p.x,
                            p.y,
                            p.z,
                            StockCutDirection::FromTop,
                        );
                    }
                    if let Some(last) = path_3d.last().copied() {
                        *last_pos = Some(last);
                    }
                    // Split the lifted path at large Z transitions.
                    //
                    // The lift function clamps path Z to either `z_level`
                    // (over valleys) or `terrain + stock_to_leave` (over
                    // peaks). Within one offset-ring path that crosses
                    // both, consecutive points can have Z deltas spanning
                    // the full mesh height. The path emitter consumes
                    // consecutive points via linear `feed_to`, so a
                    // peak→valley transition becomes a diagonal feed move
                    // through fresh stock at intermediate XYs (the
                    // dexel above the linearly-interpolated Z is uncut
                    // material from the previous Z-level). The simulator
                    // measures arc engagement reaching π → effective
                    // chipload jumps to nominal (slot mode) → gate flags
                    // chipload_breakage_risk.
                    //
                    // Fix: split where the cutter would descend faster
                    // than a configurable plunge slope. Pure-|dz|
                    // thresholds miss the real cause: a 1.1mm dz over
                    // a 0.8mm XY step is a 54° plunge that drags the
                    // cutter through stock at lateral feed even when
                    // dz alone looks "small". We use slope (descent
                    // per XY) AND absolute |dz| as belt-and-braces:
                    //
                    //   - slope > 0.3 (≈17°) — cutter is plunging more
                    //     than skimming. Split.
                    //   - |dz| > one pass depth × 1.1 — pathological
                    //     bridge from a previous z-level lift gap.
                    //     Split regardless of XY length.
                    //
                    // After splitting, retract + rapid + plunge to the
                    // next point (RapidWithFloor uses dexel-sampled
                    // stock_top so descent through cleared air is at
                    // rapid speed). Lost cutting work between split
                    // points is small (one offset step) and gets
                    // handled by deeper Z-level passes or waterline
                    // cleanup.
                    const PLUNGE_SLOPE_LIMIT: f64 = 0.3;
                    let z_drop_threshold = ctx.depth_per_pass * 1.1;
                    let mut sub_start = 0usize;
                    for i in 1..path_3d.len() {
                        let prev = path_3d[i - 1];
                        let curr = path_3d[i];
                        let dz = curr.z - prev.z;
                        let dx = curr.x - prev.x;
                        let dy = curr.y - prev.y;
                        let xy_dist = (dx * dx + dy * dy).sqrt();
                        // Symmetric slope check. Originally only descending
                        // dz was treated as risky ("rising = leaving air"),
                        // but the simulator's axial DOC at a steep RISING
                        // sample is equally large because ray_top - cutter_z
                        // is huge at the move's low-Z endpoint regardless
                        // of motion direction. Both signs split.
                        let abs_slope = if xy_dist > 1e-6 {
                            dz.abs() / xy_dist
                        } else if dz.abs() > 1e-6 {
                            f64::INFINITY // pure-vertical in a Cut path always splits
                        } else {
                            0.0
                        };
                        if dz.abs() > z_drop_threshold || abs_slope > PLUNGE_SLOPE_LIMIT {
                            if i - sub_start >= 2 {
                                segments
                                    .push(Adaptive3dSegment::Cut(path_3d[sub_start..i].to_vec()));
                                cut_count += 1;
                            }
                            let p3 = path_3d[i];
                            let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                            match rapid_floor {
                                Some(top_z) => segments.push(Adaptive3dSegment::RapidWithFloor {
                                    entry: p3,
                                    rapid_floor_z: top_z,
                                }),
                                None => segments.push(Adaptive3dSegment::Rapid(p3)),
                            }
                            sub_start = i;
                        }
                    }
                    if path_3d.len() - sub_start >= 2 {
                        segments.push(Adaptive3dSegment::Cut(path_3d[sub_start..].to_vec()));
                        cut_count += 1;
                    }
                }
                crate::adaptive::AdaptiveSegment::Rapid(p) => {
                    let p3 = lift(p);
                    // Sample current stock_top at the entry XY. When the
                    // previous Z-level cleared above this XY, the dexel
                    // column has a low top and the peck-plunge from
                    // safe_z down would burn time feeding through air.
                    // Pass the sampled top to the path emitter so it
                    // can rapid through the air gap before pecking.
                    let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                    *last_pos = Some(p3);
                    match rapid_floor {
                        Some(top_z) => segments.push(Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        }),
                        None => segments.push(Adaptive3dSegment::Rapid(p3)),
                    }
                }
                crate::adaptive::AdaptiveSegment::Link(p) => {
                    // 2D Link = feed at cut depth assuming the path is
                    // clear in the 2D material grid. In 3D, the linear path
                    // between two link endpoints can collide with terrain
                    // peaks that rise between them, even though both
                    // endpoints are at safe Z. Treat as Rapid (retract to
                    // safe_z) to guarantee no material collision. Same
                    // rapid-floor optimisation as the Rapid case.
                    let p3 = lift(p);
                    let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                    *last_pos = Some(p3);
                    match rapid_floor {
                        Some(top_z) => segments.push(Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        }),
                        None => segments.push(Adaptive3dSegment::Rapid(p3)),
                    }
                }
                crate::adaptive::AdaptiveSegment::Marker(_) => {
                    // 2D runtime events don't translate cleanly to 3D; swallow.
                    // The debug trace captured them already under agent2d_region.
                }
            }
        }
    }

    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("cut_segments", cut_count as f64);
    }

    Ok(())
}
