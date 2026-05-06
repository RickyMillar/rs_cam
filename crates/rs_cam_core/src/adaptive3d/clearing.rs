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
use super::search::{
    blend_corners_3d, is_clear_path_3d, material_remaining_at_level, material_remaining_in_region,
};
use super::{
    Adaptive3dRuntimeEvent, ClearingStrategy3d, ZLevelPlanMetrics, stock_has_material_above,
    stock_top_z_at,
};
use crate::toolpath::simplify_path_3d;

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
    /// Safe-Z used by `segments_to_toolpath` for retracts and the start
    /// of peck-plunges in `Adaptive3dSegment::Rapid`. Plumbed into the
    /// clearing layer so the planner can mirror those plunge stamps in
    /// its internal dexel state (parity with the simulator replay).
    pub(super) safe_z: f64,
    pub(super) bbox_x_min: f64,
    pub(super) bbox_x_max: f64,
    pub(super) bbox_y_min: f64,
    pub(super) bbox_y_max: f64,
    pub(super) clearing_strategy: ClearingStrategy3d,
    pub(super) z_blend: bool,
    /// Minimum corner radius for `blend_corners_3d` — needed inside
    /// `stamp_emitted_segment` so the planner stamps the SAME path the
    /// simulator will replay (segments_to_toolpath blends Cut paths
    /// before emitting feeds).
    pub(super) min_cutting_radius: f64,
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

/// Stamp dexel stock along a 3D cutting path with **swept** segment
/// stamps between consecutive points.
///
/// Mirrors how `simulate_toolpath_with_metrics_with_cancel` replays a
/// `Cut` — each consecutive pair becomes a `stamp_linear_segment` (a
/// stadium-swept stamp). Earlier this function point-stamped at each
/// path point with `stamp_tool_at`, which over-removes on sloped Cut
/// segments where the deeper-Z cylinder dominates the cylinder union
/// at midpoints (Bug 1 in the planner-↔-simulator parity work).
fn stamp_along_path(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    path: &[P3],
) {
    if path.len() < 2 {
        if let Some(p) = path.first() {
            material_stock.stamp_tool_at(
                lut,
                tool_radius,
                p.x,
                p.y,
                p.z,
                StockCutDirection::FromTop,
            );
        }
        return;
    }
    for pair in path.windows(2) {
        if let [a, b] = pair {
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                *a,
                *b,
                StockCutDirection::FromTop,
            );
        }
    }
}

/// Mirror in the planner's `material_stock` the swept-tube stamps that
/// the simulator will produce when it replays the toolpath emitted by
/// `segments_to_toolpath` for `segment`.
///
/// Call this AFTER updating `last_pos` for the previous segment but
/// BEFORE updating it for `segment` (we need the pre-segment XY/Z to
/// know where a `Link` feed starts from).
///
/// `safe_z` / `tolerance` / `min_cutting_radius` match
/// `Adaptive3dParams`; tolerance and min_cutting_radius are needed
/// because `segments_to_toolpath` runs `simplify_path_3d` and
/// `blend_corners_3d` on `Cut` paths before emitting feeds, so the
/// simulator stamps the SIMPLIFIED+BLENDED path. To stay in lockstep
/// the planner must do the same transformation here.
#[allow(clippy::too_many_arguments)]
fn stamp_emitted_segment(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    last_pos: &Option<P3>,
    segment: &Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
) {
    match segment {
        Adaptive3dSegment::Cut(path) => {
            // Mirror segments_to_toolpath's path transformation —
            // simplify_path_3d (RDP) and blend_corners_3d — so the
            // planner's swept stamps cover the SAME tubes the
            // simulator will stamp from the emitted feeds.
            if path.len() >= 2 {
                let simplified = simplify_path_3d(path, tolerance);
                let blended = blend_corners_3d(&simplified, min_cutting_radius);
                stamp_along_path(material_stock, lut, tool_radius, &blended);
            } else {
                stamp_along_path(material_stock, lut, tool_radius, path);
            }
        }
        Adaptive3dSegment::Rapid(entry) => {
            // Toolpath: rapid lift to safe_z, rapid XY at safe_z,
            // peck-plunge from safe_z down to entry.z. Only the
            // peck-plunge feeds get stamped — and the net swept-tube
            // of the interleaved peck/retract feeds is just the full
            // vertical descent from safe_z to entry.
            let start = P3::new(entry.x, entry.y, safe_z);
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                start,
                *entry,
                StockCutDirection::FromTop,
            );
        }
        Adaptive3dSegment::RapidWithFloor {
            entry,
            rapid_floor_z,
        } => {
            // Toolpath: rapid descent from safe_z down to ~rapid_floor_z
            // (cleared air, no stamp), then peck-plunge from there to
            // entry. Mirror segments_to_toolpath's `descent_floor` calc
            // exactly so we don't stamp BELOW entry.z (which would
            // happen if rapid_floor_z < entry.z, e.g. previous pass
            // already cut DEEPER than this entry — clearing function
            // sampled the post-stamp top).
            const RAPID_DESCENT_BUFFER_MM: f64 = 0.5;
            let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                .min(safe_z)
                .max(entry.z);
            let start = P3::new(entry.x, entry.y, descent_floor);
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                start,
                *entry,
                StockCutDirection::FromTop,
            );
        }
        Adaptive3dSegment::Link(target) => {
            // Toolpath: feed at constant Z from last_pos to target.
            if let Some(prev) = last_pos {
                material_stock.stamp_linear_segment(
                    lut,
                    tool_radius,
                    *prev,
                    *target,
                    StockCutDirection::FromTop,
                );
            }
        }
        Adaptive3dSegment::Marker(_) => {}
    }
}

/// Helper to push a segment and stamp its simulator-equivalent swept
/// material removal in one call.
#[allow(clippy::too_many_arguments)]
fn push_segment_with_stamp(
    segments: &mut Vec<Adaptive3dSegment>,
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    last_pos: &mut Option<P3>,
    segment: Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
) {
    stamp_emitted_segment(
        material_stock,
        lut,
        tool_radius,
        last_pos,
        &segment,
        safe_z,
        tolerance,
        min_cutting_radius,
    );
    // Update last_pos based on segment's terminal XYZ before pushing.
    match &segment {
        Adaptive3dSegment::Cut(path) => {
            if let Some(p) = path.last() {
                *last_pos = Some(*p);
            }
        }
        Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
            *last_pos = Some(*entry);
        }
        Adaptive3dSegment::Link(target) => {
            *last_pos = Some(*target);
        }
        Adaptive3dSegment::Marker(_) => {}
    }
    segments.push(segment);
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
                let entry_seg = if should_link {
                    Adaptive3dSegment::Link(*first)
                } else {
                    Adaptive3dSegment::Rapid(*first)
                };
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    entry_seg,
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                );
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Cut(path_3d),
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                );
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
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Rapid(*first),
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                );
                if path.len() >= 2 {
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path.clone()),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
                } else {
                    // Single-point run: emit as a tiny cut segment.
                    let end = P3::new(first.x + ctx.step_len, first.y, first.z);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(vec![*first, end]),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
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
                let entry_seg = if should_link {
                    Adaptive3dSegment::Link(*first)
                } else {
                    Adaptive3dSegment::Rapid(*first)
                };
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    entry_seg,
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                );
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Cut(path_3d),
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                );
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
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
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

        push_segment_with_stamp(
            segments,
            material_stock,
            lut,
            tool_radius,
            last_pos,
            Adaptive3dSegment::Rapid(contour[0]),
            safe_z,
            tolerance,
            min_cutting_radius,
        );

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
                cleanup_path.push(P3::new(x, y, z));
            }
        }
        cleanup_path.push(contour[0]);
        push_segment_with_stamp(
            segments,
            material_stock,
            lut,
            tool_radius,
            last_pos,
            Adaptive3dSegment::Cut(cleanup_path),
            safe_z,
            tolerance,
            min_cutting_radius,
        );
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
    level_marker: Option<Adaptive3dRuntimeEvent>,
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
    let mut region_areas_mm2: Vec<f64> = regions.iter().map(|r| r.area().abs()).collect();
    region_areas_mm2.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    region_areas_mm2.truncate(10);

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

    let mut level_metrics = ZLevelPlanMetrics {
        available: true,
        marching_squares_regions: region_count_before,
        region_areas_mm2,
        dropped_micro_region_count: dropped_micro,
        perimeter_sweep_length_mm: 0.0,
        agent_walk_cut_length_mm: 0.0,
        residual_cleanup_cell_count: 0,
    };
    let level_marker_index = level_marker.map(|event| {
        segments.push(Adaptive3dSegment::Marker(event));
        segments.len().saturating_sub(1)
    });

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
                if let Some(first) = path_3d.first().copied() {
                    // Sample the stock top BEFORE we stamp the rapid /
                    // cut for this loop, so the rapid-floor reflects
                    // material state from prior passes only.
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(first),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
                }
                if path_3d.len() >= 2 {
                    level_metrics.perimeter_sweep_length_mm += polyline_length_3d(&path_3d);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path_3d),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
                    cut_count += 1;
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
                if let Some(first) = path_3d.first().copied() {
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(first),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
                }
                if path_3d.len() >= 2 {
                    level_metrics.perimeter_sweep_length_mm += polyline_length_3d(&path_3d);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path_3d),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
                    cut_count += 1;
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
                    level_metrics.agent_walk_cut_length_mm += polyline_length_3d(&path_3d);
                    // Per-point classification (BEFORE any stamping):
                    // cutter is "engaged" if the current dexel ray top at
                    // this XY is above the cutter Z. "Air" points get
                    // demoted to rapids — and crucially we DON'T stamp
                    // them, so the dexel state stays consistent with what
                    // the actual machine would produce (rapids don't cut).
                    // Without this, generation-time dexel diverges from
                    // simulator-time dexel: the planner thinks it cleared
                    // cells the actual G-code never cuts, leaving
                    // visible streaks of uncut material in the sim.
                    const AIR_THRESHOLD_MM: f64 = 0.2;
                    let mut engaged: Vec<bool> = path_3d
                        .iter()
                        .map(|p| match sample_stock_top_at(material_stock, p.x, p.y) {
                            Some(top) => top > p.z + AIR_THRESHOLD_MM,
                            None => false,
                        })
                        .collect();
                    // Re-promote short air runs back to engaged: rapid-mode
                    // is only faster than feed-mode for transit > ~70mm
                    // because each rapid carries retract+plunge overhead
                    // (~0.5s). For a 6mm tool at 3150mm/min feed and
                    // 5000mm/min rapid, the crossover is around 70mm.
                    // Below that, feeding through air is cheaper than
                    // demoting to rapid. Threshold is XY toolpath
                    // distance, so we walk forward summing segment
                    // lengths until we exceed it or change classification.
                    const MIN_AIR_RUN_MM: f64 = 70.0;
                    if !engaged.is_empty() {
                        let mut i = 0;
                        while i < engaged.len() {
                            if engaged[i] {
                                i += 1;
                                continue;
                            }
                            // Find end of this air run.
                            let run_start = i;
                            let mut run_end = i;
                            let mut run_len_mm = 0.0_f64;
                            while run_end + 1 < engaged.len() && !engaged[run_end + 1] {
                                let dx = path_3d[run_end + 1].x - path_3d[run_end].x;
                                let dy = path_3d[run_end + 1].y - path_3d[run_end].y;
                                run_len_mm += (dx * dx + dy * dy).sqrt();
                                run_end += 1;
                            }
                            // Add the entry transition length too (from
                            // last engaged point to first air point) so a
                            // tiny air "wedge" between two engaged points
                            // also counts.
                            if run_start > 0 {
                                let dx = path_3d[run_start].x - path_3d[run_start - 1].x;
                                let dy = path_3d[run_start].y - path_3d[run_start - 1].y;
                                run_len_mm += (dx * dx + dy * dy).sqrt();
                            }
                            if run_len_mm < MIN_AIR_RUN_MM {
                                for k in run_start..=run_end {
                                    engaged[k] = true;
                                }
                            }
                            i = run_end + 1;
                        }
                    }
                    if let Some(last) = path_3d.last().copied() {
                        *last_pos = Some(last);
                    }
                    // Split the lifted path at:
                    //   - large Z transitions (existing safety: peak→valley
                    //     bridges that drag the cutter diagonally through
                    //     uncut stock, see PLUNGE_SLOPE_LIMIT below);
                    //   - engagement transitions (NEW: air→engaged or vice
                    //     versa demarcates a real cut from a transit run).
                    const PLUNGE_SLOPE_LIMIT: f64 = 0.3;
                    let z_drop_threshold = ctx.depth_per_pass * 1.1;
                    let mut sub_start = 0usize;
                    let mut sub_engaged = engaged.first().copied().unwrap_or(true);
                    for i in 1..path_3d.len() {
                        let prev = path_3d[i - 1];
                        let curr = path_3d[i];
                        let dz = curr.z - prev.z;
                        let dx = curr.x - prev.x;
                        let dy = curr.y - prev.y;
                        let xy_dist = (dx * dx + dy * dy).sqrt();
                        let abs_slope = if xy_dist > 1e-6 {
                            dz.abs() / xy_dist
                        } else if dz.abs() > 1e-6 {
                            f64::INFINITY
                        } else {
                            0.0
                        };
                        let large_z = dz.abs() > z_drop_threshold || abs_slope > PLUNGE_SLOPE_LIMIT;
                        let engagement_change = engaged[i] != sub_engaged;
                        if large_z || engagement_change {
                            // Flush the current run.
                            let run_len = i - sub_start;
                            if run_len >= 1 {
                                if sub_engaged {
                                    if run_len >= 2 {
                                        push_segment_with_stamp(
                                            segments,
                                            material_stock,
                                            ctx.lut,
                                            ctx.tool_radius,
                                            last_pos,
                                            Adaptive3dSegment::Cut(path_3d[sub_start..i].to_vec()),
                                            ctx.safe_z,
                                            ctx.tolerance,
                                            ctx.min_cutting_radius,
                                        );
                                        cut_count += 1;
                                    }
                                } else {
                                    // Air run → single RapidWithFloor to
                                    // the end of the run (the cutter just
                                    // traverses cleared territory). No
                                    // stamping (rapids don't cut).
                                    let end_pt = path_3d[i.saturating_sub(1)];
                                    let rapid_floor =
                                        sample_stock_top_at(material_stock, end_pt.x, end_pt.y);
                                    let entry_seg = match rapid_floor {
                                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                            entry: end_pt,
                                            rapid_floor_z: top_z,
                                        },
                                        None => Adaptive3dSegment::Rapid(end_pt),
                                    };
                                    push_segment_with_stamp(
                                        segments,
                                        material_stock,
                                        ctx.lut,
                                        ctx.tool_radius,
                                        last_pos,
                                        entry_seg,
                                        ctx.safe_z,
                                        ctx.tolerance,
                                        ctx.min_cutting_radius,
                                    );
                                }
                            }
                            // After a large_z split, also rapid-position
                            // to the new point so the cutter retracts
                            // before continuing.
                            if large_z {
                                let p3 = path_3d[i];
                                let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                                let entry_seg = match rapid_floor {
                                    Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                        entry: p3,
                                        rapid_floor_z: top_z,
                                    },
                                    None => Adaptive3dSegment::Rapid(p3),
                                };
                                push_segment_with_stamp(
                                    segments,
                                    material_stock,
                                    ctx.lut,
                                    ctx.tool_radius,
                                    last_pos,
                                    entry_seg,
                                    ctx.safe_z,
                                    ctx.tolerance,
                                    ctx.min_cutting_radius,
                                );
                            }
                            sub_start = i;
                            sub_engaged = engaged[i];
                        }
                    }
                    // Flush final run.
                    let run_len = path_3d.len() - sub_start;
                    if run_len >= 1 {
                        if sub_engaged {
                            if run_len >= 2 {
                                push_segment_with_stamp(
                                    segments,
                                    material_stock,
                                    ctx.lut,
                                    ctx.tool_radius,
                                    last_pos,
                                    Adaptive3dSegment::Cut(path_3d[sub_start..].to_vec()),
                                    ctx.safe_z,
                                    ctx.tolerance,
                                    ctx.min_cutting_radius,
                                );
                                cut_count += 1;
                            }
                        } else if let Some(end_pt) = path_3d.last().copied() {
                            let rapid_floor =
                                sample_stock_top_at(material_stock, end_pt.x, end_pt.y);
                            let entry_seg = match rapid_floor {
                                Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                    entry: end_pt,
                                    rapid_floor_z: top_z,
                                },
                                None => Adaptive3dSegment::Rapid(end_pt),
                            };
                            push_segment_with_stamp(
                                segments,
                                material_stock,
                                ctx.lut,
                                ctx.tool_radius,
                                last_pos,
                                entry_seg,
                                ctx.safe_z,
                                ctx.tolerance,
                                ctx.min_cutting_radius,
                            );
                        }
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
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(p3),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
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
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(p3),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                    );
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
        scope.set_counter(
            "perimeter_sweep_length_mm",
            level_metrics.perimeter_sweep_length_mm,
        );
        scope.set_counter(
            "agent_walk_cut_length_mm",
            level_metrics.agent_walk_cut_length_mm,
        );
    }
    update_level_marker_metrics(segments, level_marker_index, level_metrics);

    Ok(())
}

fn update_level_marker_metrics(
    segments: &mut [Adaptive3dSegment],
    marker_index: Option<usize>,
    metrics: ZLevelPlanMetrics,
) {
    let Some(index) = marker_index else {
        return;
    };
    if let Some(Adaptive3dSegment::Marker(event)) = segments.get_mut(index) {
        event.set_z_level_metrics(metrics);
    }
}

fn polyline_length_3d(path: &[P3]) -> f64 {
    path.windows(2)
        .map(|pair| {
            let Some(a) = pair.first() else {
                return 0.0;
            };
            let Some(b) = pair.get(1) else {
                return 0.0;
            };
            (*b - *a).norm()
        })
        .sum()
}
