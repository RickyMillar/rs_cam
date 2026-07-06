//! Steep and Shallow finishing strategy.
//!
//! Automatically splits the surface into steep and shallow regions based on a
//! threshold angle, then applies waterline (contour) passes to steep areas and
//! parallel (raster) passes to shallow areas.
//!
//! From Fusion 360 docs: "machines steep areas using Contour passes and shallow
//! areas using Parallel or Scallop passes."
//!
//! Key features:
//! - Threshold angle classification (default 40° from horizontal)
//! - Overlap distance: both strategies extend into each other's regions
//! - Wall clearance: shallow passes stay clear of steep walls
//! - Steep-first ordering for safer tool conditions
//! - Scallop height support for variable stepover in shallow regions

use crate::dropcutter::batch_drop_cutter;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::region_set::RegionSet;
use crate::slope::{SlopeMap, classify_steep_shallow};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::waterline::waterline_contours;

use tracing::{debug, info};

/// Parameters for steep-and-shallow finishing.
pub struct SteepShallowParams {
    /// Threshold angle (degrees from horizontal). Areas steeper than this get
    /// waterline passes; shallower areas get parallel passes. Default: 40.
    pub threshold_angle: f64,
    /// Overlap distance: both strategies extend into each other's region (mm).
    /// Eliminates visible transition marks. Default: 2× stepover.
    pub overlap_distance: f64,
    /// Wall clearance: shallow passes stay this far from steep walls (mm).
    /// Prevents tool rubbing on steep walls during parallel passes.
    pub wall_clearance: f64,
    /// Machine steep regions first for safer tool conditions.
    pub steep_first: bool,
    /// Stepover for parallel passes in shallow regions (mm).
    pub stepover: f64,
    /// Z step for waterline passes in steep regions (mm).
    pub z_step: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Fiber sampling spacing for waterline contour generation.
    pub sampling: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
}

impl Default for SteepShallowParams {
    fn default() -> Self {
        Self {
            threshold_angle: 45.0,
            overlap_distance: 2.0,
            wall_clearance: 1.0,
            steep_first: true,
            stepover: 1.0,
            z_step: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            sampling: 1.0,
            stock_to_leave: 0.0,
            tolerance: 0.05,
        }
    }
}

/// Dilate a boolean grid by `radius_cells`, setting additional cells to `true`.
/// 8-connected expansion.
#[allow(clippy::indexing_slicing)] // bounded indexing in grid morphology
pub fn dilate_grid(grid: &[bool], rows: usize, cols: usize, radius_cells: usize) -> Vec<bool> {
    if radius_cells == 0 {
        return grid.to_vec();
    }
    let mut result = grid.to_vec();
    for _ in 0..radius_cells {
        let prev = result.clone();
        for row in 0..rows {
            for col in 0..cols {
                if prev[row * cols + col] {
                    continue; // Already true
                }
                // Check 8-connected neighbors
                let has_neighbor = (-1i32..=1).any(|dr| {
                    (-1i32..=1).any(|dc| {
                        if dr == 0 && dc == 0 {
                            return false;
                        }
                        let nr = row as i32 + dr;
                        let nc = col as i32 + dc;
                        if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                            return false;
                        }
                        prev[nr as usize * cols + nc as usize]
                    })
                });
                if has_neighbor {
                    result[row * cols + col] = true;
                }
            }
        }
    }
    result
}

/// Erode a boolean grid by `radius_cells`, clearing cells near the boundary.
fn erode_grid(grid: &[bool], rows: usize, cols: usize, radius_cells: usize) -> Vec<bool> {
    if radius_cells == 0 {
        return grid.to_vec();
    }
    // Erode = dilate the inverse, then invert back
    let inv: Vec<bool> = grid.iter().map(|&v| !v).collect();
    let dilated_inv = dilate_grid(&inv, rows, cols, radius_cells);
    dilated_inv.iter().map(|&v| !v).collect()
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(
    clippy::indexing_slicing,
    clippy::too_many_arguments,
    clippy::expect_used
)]
// Production goes through the _with_cancel variant; this never-cancel
// convenience wrapper is exercised by the unit tests below.
#[cfg_attr(not(test), allow(dead_code))]
/// Generate steep (waterline) passes filtered to steep+overlap region.
fn generate_steep_passes(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &SlopeMap,
    steep_expanded: &[bool],
    z_top: f64,
    z_bottom: f64,
    z_step: f64,
    sampling: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let never_cancel = || false;
    generate_steep_passes_with_cancel(
        mesh,
        index,
        cutter,
        slope_map,
        steep_expanded,
        z_top,
        z_bottom,
        z_step,
        sampling,
        stock_to_leave,
        feed_rate,
        plunge_rate,
        safe_z,
        None,
        &never_cancel,
    )
    .expect("non-cancellable steep-pass generation should never be cancelled")
}

/// Cancellable variant of [`generate_steep_passes`]. Polls `cancel` once per
/// Z level (the outer waterline-contour loop).
///
/// `boundary_regions` (P2.3): folded into the existing steep-grid keep
/// predicate below — a contour point survives only when it's both in the
/// expanded steep grid AND inside a machining-boundary region (when one is
/// given). `None` reproduces today's steep-grid-only filtering byte-for-byte.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn generate_steep_passes_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &SlopeMap,
    steep_expanded: &[bool],
    z_top: f64,
    z_bottom: f64,
    z_step: f64,
    sampling: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut tp = Toolpath::new();

    let steep_threshold = 30.0_f64.to_radians(); // Filter out contours that are mostly shallow

    // `snap_to_bottom = false`: matches the prior inline loop exactly — the
    // ladder is not guaranteed to land exactly on `z_bottom` when the range
    // isn't a whole multiple of `z_step` (see finish_setup::z_ladder docs).
    let z_levels = crate::finish_setup::z_ladder(
        z_top,
        z_bottom,
        z_step,
        crate::finish_setup::Z_LADDER_DEFAULT_EPSILON,
        false,
    );
    for z in z_levels {
        check_cancel(cancel)?;
        let contours = waterline_contours(mesh, index, cutter, z, sampling);

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }

            // Filter: keep contours that are predominantly in the steep+overlap region.
            // Sample points and check the expanded steep grid.
            let sample_step = 1.max(contour.len() / 10);
            let in_steep = contour
                .iter()
                .step_by(sample_step)
                .filter(|p| {
                    slope_map
                        .angle_at_world(p.x, p.y)
                        .is_some_and(|a| a >= steep_threshold)
                })
                .count();
            let total = contour.len().div_ceil(sample_step);
            if total > 0 && in_steep * 3 < total {
                continue; // Mostly shallow, skip
            }

            // Further filter: mark which points fall within the expanded
            // steep grid AND (when given) a machining-boundary region,
            // preserving contour ordering so we can tell which survivors
            // are contiguous.
            let keep: Vec<bool> = contour
                .iter()
                .map(|p| {
                    let in_steep_grid = slope_map
                        .world_to_cell(p.x, p.y)
                        .is_some_and(|(row, col)| steep_expanded[row * slope_map.cols + col]);
                    let in_region =
                        boundary_regions.is_none_or(|regions| regions.contains(&P2::new(p.x, p.y)));
                    in_steep_grid && in_region
                })
                .collect();

            let kept_count = keep.iter().filter(|&&k| k).count();
            if kept_count < 3 {
                continue;
            }

            use crate::toolpath::MoveIntent;
            let z_adjusted = z + stock_to_leave;
            // Only a contour where every point survives is a genuinely
            // closed loop — safe to close back to its start. Partial
            // survivors are non-contiguous with the excluded shallow
            // region: chording straight across that gap at cutting feed
            // would gouge material that sits above this Z level. Split
            // into contiguous runs and emit each as its own open pass with
            // its own rapid/plunge/retract envelope.
            let whole_contour_kept = kept_count == contour.len();
            let runs = crate::point_runs::split_runs(
                contour,
                |i, _p| keep.get(i).copied().unwrap_or(false),
                crate::point_runs::RunTopology::Closed,
                2,
            );

            for run in &runs {
                if run.len() < 2 {
                    continue;
                }
                let path: Vec<P3> = run
                    .iter()
                    .map(|pt| P3::new(pt.x, pt.y, z_adjusted))
                    .collect();

                // Whole-contour survivors are a genuine closed loop — use
                // the shared closed-contour emitter (rapid/plunge/feed/
                // close/retract) instead of hand-closing the point list.
                // Partial survivors are an open run: emit as a plain path
                // segment with its own rapid/plunge/retract envelope.
                if whole_contour_kept {
                    tp.emit_closed_contour_with_intent(
                        &path,
                        safe_z,
                        feed_rate,
                        plunge_rate,
                        MoveIntent::FinishingCut,
                    );
                } else {
                    tp.emit_path_segment_with_intent(
                        &path,
                        safe_z,
                        feed_rate,
                        plunge_rate,
                        MoveIntent::FinishingCut,
                    );
                }
            }
        }
    }

    Ok(tp)
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(
    clippy::indexing_slicing,
    clippy::too_many_arguments,
    clippy::expect_used
)]
// Production goes through the _with_cancel variant; this never-cancel
// convenience wrapper is exercised by the unit tests below.
#[cfg_attr(not(test), allow(dead_code))]
/// Generate shallow (parallel raster) passes filtered to shallow+overlap region.
fn generate_shallow_passes(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    shallow_eroded: &[bool],
    slope_map: &SlopeMap,
    stepover: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let never_cancel = || false;
    generate_shallow_passes_with_cancel(
        mesh,
        index,
        cutter,
        shallow_eroded,
        slope_map,
        stepover,
        stock_to_leave,
        feed_rate,
        plunge_rate,
        safe_z,
        None,
        &never_cancel,
    )
    .expect("non-cancellable shallow-pass generation should never be cancelled")
}

/// Cancellable variant of [`generate_shallow_passes`]. Polls `cancel` once
/// per raster row.
///
/// `boundary_regions` (P2.3): folded into the existing `is_shallow`
/// per-point keep check below (the drop-cutter batch already ran before
/// this loop, so there's no query left to pre-skip — the region check is
/// still cheap since it only gates which precomputed grid points get
/// emitted). `None` reproduces today's shallow-grid-only filtering
/// byte-for-byte.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn generate_shallow_passes_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    shallow_eroded: &[bool],
    slope_map: &SlopeMap,
    stepover: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut tp = Toolpath::new();

    // Generate drop-cutter raster grid
    let grid = batch_drop_cutter(mesh, index, cutter, stepover, 0.0, mesh.bbox.min.z);
    check_cancel(cancel)?;

    // Walk rows in zigzag pattern, clipping to the shallow region
    for row in 0..grid.rows {
        check_cancel(cancel)?;
        let reverse = row % 2 == 1;
        let mut in_region = false;
        let mut run: Vec<P3> = Vec::new();

        let cols: Box<dyn Iterator<Item = usize>> = if reverse {
            Box::new((0..grid.cols).rev())
        } else {
            Box::new(0..grid.cols)
        };

        for col in cols {
            let cl = grid.get(row, col);
            // Read world coordinates straight off the CL point rather than
            // reconstructing them from the grid origin/step — the grid's
            // `u_start`/`v_start` are rotated-frame minima that only read as
            // world X/Y when `direction_deg == 0.0` (which the call above
            // hardcodes, but `cl.x`/`cl.y` are always world-frame regardless
            // of sampling direction, so this removes that trap entirely).
            let x = cl.x;
            let y = cl.y;

            // Check if this point is in the shallow region and (when given)
            // inside a machining-boundary region.
            let is_shallow = slope_map.world_to_cell(x, y).is_some_and(|(_r, _c)| {
                let idx = _r * slope_map.cols + _c;
                idx < shallow_eroded.len() && shallow_eroded[idx]
            }) && boundary_regions
                .is_none_or(|regions| regions.contains(&P2::new(x, y)));

            if is_shallow {
                let z = cl.z + stock_to_leave;
                run.push(P3::new(x, y, z));
                in_region = true;
            } else if in_region {
                use crate::toolpath::MoveIntent;
                // Exiting region — emit the run
                if run.len() >= 2 {
                    tp.rapid_to_with_intent(
                        P3::new(run[0].x, run[0].y, safe_z),
                        MoveIntent::Linking,
                    );
                    tp.feed_to_with_intent(run[0], plunge_rate, MoveIntent::EntryPlunge);
                    for pt in &run[1..] {
                        tp.feed_to_with_intent(*pt, feed_rate, MoveIntent::FinishingCut);
                    }
                    if let Some(&last) = run.last() {
                        tp.rapid_to_with_intent(
                            P3::new(last.x, last.y, safe_z),
                            MoveIntent::Retract,
                        );
                    }
                }
                run.clear();
                in_region = false;
            }
        }

        // Flush remaining run at end of row
        if run.len() >= 2 {
            use crate::toolpath::MoveIntent;
            tp.rapid_to_with_intent(P3::new(run[0].x, run[0].y, safe_z), MoveIntent::Linking);
            tp.feed_to_with_intent(run[0], plunge_rate, MoveIntent::EntryPlunge);
            for pt in &run[1..] {
                tp.feed_to_with_intent(*pt, feed_rate, MoveIntent::FinishingCut);
            }
            if let Some(&last) = run.last() {
                tp.rapid_to_with_intent(P3::new(last.x, last.y, safe_z), MoveIntent::Retract);
            }
        }
    }

    Ok(tp)
}

/// Generate a steep-and-shallow finishing toolpath.
///
/// Splits the surface into steep and shallow regions based on slope angle,
/// then generates waterline passes for steep areas and parallel raster passes
/// for shallow areas, with configurable overlap and wall clearance.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
#[tracing::instrument(skip(mesh, index, cutter, params), fields(threshold = params.threshold_angle))]
pub fn steep_shallow_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SteepShallowParams,
) -> Toolpath {
    let never_cancel = || false;
    steep_shallow_toolpath_with_cancel(mesh, index, cutter, params, None, &never_cancel)
        .expect("non-cancellable steep/shallow toolpath should never be cancelled")
}

/// Cancellable variant of [`steep_shallow_toolpath`]. Propagates `cancel`
/// into heightmap construction and both pass generators (per-Z-level for
/// steep, per-raster-row for shallow).
///
/// `boundary_regions` (P2.3): forwarded to both pass generators, which fold
/// it into their existing per-point keep checks (steep grid / shallow grid
/// respectively). `None` reproduces today's output byte-for-byte.
#[tracing::instrument(skip(mesh, index, cutter, params, cancel), fields(threshold = params.threshold_angle))]
pub fn steep_shallow_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SteepShallowParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs)
    let surface = crate::finish_setup::build_finish_surface_with_cancel(
        mesh,
        index,
        cutter,
        params.tolerance,
        cancel,
    )?;
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;
    let cell_size = surface_hm.cell_size;
    let rows = surface_hm.rows;
    let cols = surface_hm.cols;

    // Classify steep vs shallow
    let steep_grid = classify_steep_shallow(&slope_map, params.threshold_angle);

    let steep_count = steep_grid.iter().filter(|&&s| s).count();
    let shallow_count = steep_grid.iter().filter(|&&s| !s).count();
    info!(
        steep = steep_count,
        shallow = shallow_count,
        threshold = params.threshold_angle,
        "Steep/shallow classification"
    );

    // Expand steep region by overlap_distance for waterline pass extension
    let overlap_cells = (params.overlap_distance / cell_size).ceil() as usize;
    let steep_expanded = dilate_grid(&steep_grid, rows, cols, overlap_cells);

    // Create shallow region: invert steep, then erode by wall_clearance
    let shallow_grid: Vec<bool> = steep_grid.iter().map(|&s| !s).collect();
    let clearance_cells = (params.wall_clearance / cell_size).ceil() as usize;
    let shallow_eroded = erode_grid(&shallow_grid, rows, cols, clearance_cells);

    // Expand shallow region by overlap_distance for raster pass extension
    let shallow_expanded = dilate_grid(&shallow_eroded, rows, cols, overlap_cells);

    // Z range
    let z_top = bbox.max.z;
    let z_bottom = surface_hm.min_z();

    debug!(
        z_top = format!("{:.1}", z_top),
        z_bottom = format!("{:.1}", z_bottom),
        overlap_cells = overlap_cells,
        clearance_cells = clearance_cells,
        "Generating steep and shallow passes"
    );

    // Generate steep (waterline) passes
    let steep_tp = generate_steep_passes_with_cancel(
        mesh,
        index,
        cutter,
        &slope_map,
        &steep_expanded,
        z_top,
        z_bottom,
        params.z_step,
        params.sampling,
        params.stock_to_leave,
        params.feed_rate,
        params.plunge_rate,
        params.safe_z,
        boundary_regions,
        cancel,
    )?;

    // Generate shallow (parallel) passes
    let shallow_tp = generate_shallow_passes_with_cancel(
        mesh,
        index,
        cutter,
        &shallow_expanded,
        &slope_map,
        params.stepover,
        params.stock_to_leave,
        params.feed_rate,
        params.plunge_rate,
        params.safe_z,
        boundary_regions,
        cancel,
    )?;

    info!(
        steep_moves = steep_tp.moves.len(),
        shallow_moves = shallow_tp.moves.len(),
        steep_cut_mm = format!("{:.0}", steep_tp.total_cutting_distance()),
        shallow_cut_mm = format!("{:.0}", shallow_tp.total_cutting_distance()),
        "Steep and shallow passes generated"
    );

    // Merge: steep_first means steep toolpath comes first
    let mut tp = Toolpath::new();
    if params.steep_first {
        tp.moves.extend(steep_tp.moves);
        tp.moves.extend(shallow_tp.moves);
    } else {
        tp.moves.extend(shallow_tp.moves);
        tp.moves.extend(steep_tp.moves);
    }

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Steep and shallow toolpath complete"
    );

    Ok(tp)
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
    use crate::mesh::SpatialIndex;
    use crate::slope::SurfaceHeightmap;
    use crate::tool::BallEndmill;

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    // ── Grid morphology tests ───────────────────────────────────────

    #[test]
    fn test_dilate_grid_single_cell() {
        // Single true cell in center, dilate by 1 → 3×3 block
        let mut grid = vec![false; 25]; // 5×5
        grid[2 * 5 + 2] = true; // Center
        let dilated = dilate_grid(&grid, 5, 5, 1);

        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                let r = (2 + dr) as usize;
                let c = (2 + dc) as usize;
                assert!(
                    dilated[r * 5 + c],
                    "Cell ({},{}) should be true after dilation",
                    r,
                    c
                );
            }
        }
        // Corner should still be false
        assert!(!dilated[0], "Corner should remain false");
    }

    #[test]
    fn test_erode_grid() {
        // 7×7 grid: true in center 5×5 block, false border → erode by 1 shrinks to 3×3
        let rows = 7;
        let cols = 7;
        let mut grid = vec![false; rows * cols];
        for r in 1..6 {
            for c in 1..6 {
                grid[r * cols + c] = true;
            }
        }
        let eroded = erode_grid(&grid, rows, cols, 1);

        // Border of the original block (row/col 1 and 5) should be eroded
        assert!(!eroded[cols + 1], "Edge of block should be eroded");
        assert!(!eroded[5 * cols + 5], "Edge of block should be eroded");
        // Interior (rows 2-4, cols 2-4) should survive
        assert!(eroded[3 * cols + 3], "Center should survive erosion");
        assert!(eroded[2 * cols + 2], "Inner cell should survive erosion");
    }

    #[test]
    fn test_dilate_erode_identity() {
        // Dilate then erode by same amount should approximately preserve shape
        // (not exact due to morphological properties, but close)
        let rows = 10;
        let cols = 10;
        let mut grid = vec![false; rows * cols];
        // Create a 4×4 block in center
        for r in 3..7 {
            for c in 3..7 {
                grid[r * cols + c] = true;
            }
        }
        let dilated = dilate_grid(&grid, rows, cols, 1);
        let restored = erode_grid(&dilated, rows, cols, 1);

        // Original block should be present
        for r in 3..7 {
            for c in 3..7 {
                assert!(
                    restored[r * cols + c],
                    "Original block cell ({},{}) should survive dilate+erode",
                    r,
                    c
                );
            }
        }
    }

    // ── Classification + region tests ───────────────────────────────

    #[test]
    fn test_steep_shallow_hemisphere_split() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let cell_size = 1.0;
        let bbox = &mesh.bbox;

        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();
        let steep = classify_steep_shallow(&slope_map, 40.0);

        let steep_count = steep.iter().filter(|&&s| s).count();
        let shallow_count = steep.iter().filter(|&&s| !s).count();

        assert!(steep_count > 0, "Hemisphere should have steep cells, got 0");
        assert!(
            shallow_count > 0,
            "Hemisphere should have shallow cells, got 0"
        );
        // Outer ring should be steep, inner area shallow
        assert!(
            shallow_count > steep_count / 2,
            "Should have a significant shallow region: {} steep, {} shallow",
            steep_count,
            shallow_count
        );
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_steep_shallow_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };

        let tp = steep_shallow_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Should produce non-trivial toolpath, got {} moves",
            tp.moves.len()
        );
    }

    #[test]
    fn test_steep_has_waterline_passes() {
        // Steep passes should be at constant Z levels
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let cell_size = 1.0;
        let bbox = &mesh.bbox;

        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();
        let steep_grid = classify_steep_shallow(&slope_map, 40.0);
        let steep_expanded = dilate_grid(&steep_grid, rows, cols, 2);

        let steep_tp = generate_steep_passes(
            &mesh,
            &si,
            &cutter,
            &slope_map,
            &steep_expanded,
            bbox.max.z,
            surface_hm.min_z(),
            2.0,
            3.0,
            0.0,
            1000.0,
            500.0,
            30.0,
        );

        // Steep waterline passes should have constant Z within each contour
        // (between rapids). Collect Z values of feed moves between rapids.
        let mut contour_z_values: Vec<f64> = Vec::new();
        let mut in_contour = false;
        for m in &steep_tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if in_contour && contour_z_values.len() >= 2 {
                        let z_min = contour_z_values
                            .iter()
                            .copied()
                            .fold(f64::INFINITY, f64::min);
                        let z_max = contour_z_values
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max);
                        assert!(
                            (z_max - z_min) < 0.1,
                            "Steep contour Z should be constant, got range {:.3}",
                            z_max - z_min
                        );
                    }
                    contour_z_values.clear();
                    in_contour = false;
                }
                crate::toolpath::MoveType::Linear { .. } => {
                    if m.target.z < 29.0 {
                        // Below safe_z → cutting
                        contour_z_values.push(m.target.z);
                        in_contour = true;
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_shallow_has_raster_passes() {
        // Shallow passes should have varying Z (following the surface)
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let cell_size = 1.0;
        let bbox = &mesh.bbox;

        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();
        let steep_grid = classify_steep_shallow(&slope_map, 40.0);
        let shallow_grid: Vec<bool> = steep_grid.iter().map(|&s| !s).collect();
        let shallow_expanded = dilate_grid(&shallow_grid, rows, cols, 2);

        let shallow_tp = generate_shallow_passes(
            &mesh,
            &si,
            &cutter,
            &shallow_expanded,
            &slope_map,
            2.0,
            0.0,
            1000.0,
            500.0,
            30.0,
        );

        // Shallow raster should have variable Z across the surface
        let cutting_z: Vec<f64> = shallow_tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }) && m.target.z < 29.0
            })
            .map(|m| m.target.z)
            .collect();

        if cutting_z.len() > 2 {
            let z_min = cutting_z.iter().copied().fold(f64::INFINITY, f64::min);
            let z_max = cutting_z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                z_max - z_min > 1.0,
                "Shallow raster Z should vary (following surface), range was only {:.2}",
                z_max - z_min
            );
        }
    }

    #[test]
    fn test_steep_first_ordering() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            steep_first: true,
            ..SteepShallowParams::default()
        };

        let tp = steep_shallow_toolpath(&mesh, &si, &cutter, &params);
        // With steep_first, waterline passes (constant Z) should come before
        // raster passes (variable Z). We can't perfectly distinguish in the
        // merged toolpath, but at minimum it should produce output.
        assert!(
            tp.moves.len() > 5,
            "Steep-first should produce moves, got {}",
            tp.moves.len()
        );
    }

    #[test]
    fn test_overlap_increases_coverage() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();

        // Without overlap
        let params_no_overlap = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            overlap_distance: 0.0,
            ..SteepShallowParams::default()
        };
        let tp_no = steep_shallow_toolpath(&mesh, &si, &cutter, &params_no_overlap);

        // With overlap
        let params_overlap = SteepShallowParams {
            overlap_distance: 5.0,
            ..params_no_overlap
        };
        let tp_yes = steep_shallow_toolpath(&mesh, &si, &cutter, &params_overlap);

        // Overlap should produce more or equal cutting distance
        assert!(
            tp_yes.total_cutting_distance() >= tp_no.total_cutting_distance() - 1.0,
            "Overlap should increase coverage: with={:.0} without={:.0}",
            tp_yes.total_cutting_distance(),
            tp_no.total_cutting_distance()
        );
    }

    // ── Regression: steep passes must not chord across excluded regions ──

    /// Regression test for the gouge bug: `generate_steep_passes` used to
    /// filter waterline contour points by the steep mask, then feed ALL
    /// survivors sequentially and unconditionally close the loop back to
    /// the first survivor. When survivors are non-contiguous (a shallow
    /// gap sits between them), that closing/connecting move chords
    /// straight across the excluded region at cutting feed, into material
    /// that sits above the pass's Z level.
    ///
    /// Uses a real hemisphere waterline contour (a genuine closed circular
    /// loop) with a hand-built steep mask (steep iff world X < 0) that has
    /// nothing to do with the hemisphere's real slope — it exists purely to
    /// force a deterministic half-kept / half-excluded split so the test
    /// doesn't depend on the real classifier's grid resolution.
    #[test]
    fn test_steep_passes_no_chord_across_excluded_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let cell_size = 1.0;
        let bbox = &mesh.bbox;

        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();

        // Hand-built mask: steep iff the cell's world X coordinate is
        // negative — a hard half-plane split unrelated to real slope.
        let mut steep_mask = vec![false; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = origin_x + col as f64 * cell_size;
                if x < 0.0
                    && let Some(slot) = steep_mask.get_mut(row * cols + col)
                {
                    *slot = true;
                }
            }
        }

        let z_test = 8.0;
        let sampling = 1.0;

        // Capture the raw contour to establish the expected point spacing.
        let raw_contours = waterline_contours(&mesh, &si, &cutter, z_test, sampling);
        let contour = raw_contours
            .iter()
            .max_by_key(|c| c.len())
            .expect("hemisphere should produce a contour at z=8");
        assert!(
            contour.len() >= 8,
            "need a non-trivial contour to exercise the split, got {}",
            contour.len()
        );

        let mut max_adjacent_spacing = 0.0_f64;
        for i in 0..contour.len() {
            let a = contour[i];
            let b = contour[(i + 1) % contour.len()];
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
            max_adjacent_spacing = max_adjacent_spacing.max(d);
        }

        let steep_tp = generate_steep_passes(
            &mesh,
            &si,
            &cutter,
            &slope_map,
            &steep_mask,
            z_test,
            z_test - 0.5, // z_bottom: keeps the loop to a single Z level
            100.0,        // z_step larger than the range: exactly one pass
            sampling,
            0.0,
            1000.0,
            500.0,
            30.0,
        );

        // No consecutive pair of cutting (FinishingCut) moves may be farther
        // apart than a small multiple of the contour's own point spacing —
        // that would mean a cutting move chorded across the excluded half.
        let max_allowed = (max_adjacent_spacing * 3.0).max(3.0);
        let mut prev_cut: Option<P3> = None;
        let mut saw_any_cut = false;
        for m in &steep_tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_any_cut = true;
                if let Some(prev) = prev_cut {
                    let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                    assert!(
                        d <= max_allowed,
                        "cutting move chorded across the excluded region: {:.2}mm \
                         (allowed {:.2}mm) from ({:.2},{:.2}) to ({:.2},{:.2})",
                        d,
                        max_allowed,
                        prev.x,
                        prev.y,
                        m.target.x,
                        m.target.y
                    );
                }
                prev_cut = Some(m.target);
            } else {
                // A rapid or plunge breaks the run — the next cutting move
                // starts a fresh pass and shouldn't be distance-checked
                // against whatever preceded the break.
                prev_cut = None;
            }
        }
        assert!(saw_any_cut, "expected at least one cutting move");
    }

    /// Companion to the gouge regression above: when every contour point
    /// survives the steep filter (no excluded gap at all), each pass must
    /// still close back to its own start — the closed-loop behavior must
    /// be preserved for genuinely closed contours.
    #[test]
    fn test_steep_passes_still_close_loop_when_fully_steep() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let cell_size = 1.0;
        let bbox = &mesh.bbox;

        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();

        // Every cell steep: the whole contour should survive the filter.
        let all_steep = vec![true; rows * cols];

        let steep_tp = generate_steep_passes(
            &mesh, &si, &cutter, &slope_map, &all_steep, 8.0, 7.5, 100.0, 1.0, 0.0, 1000.0, 500.0,
            30.0,
        );

        // Reconstruct each pass (a run of plunge+cut moves between rapids)
        // and assert every pass returns to its own starting XY.
        let mut passes: Vec<Vec<P3>> = Vec::new();
        let mut current: Vec<P3> = Vec::new();
        for m in &steep_tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if current.len() >= 2 {
                        passes.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                crate::toolpath::MoveType::Linear { .. }
                    if m.intent == crate::toolpath::MoveIntent::EntryPlunge
                        || m.intent == crate::toolpath::MoveIntent::FinishingCut =>
                {
                    current.push(m.target);
                }
                _ => {}
            }
        }
        if current.len() >= 2 {
            passes.push(current);
        }

        assert!(!passes.is_empty(), "expected at least one steep pass");
        for pass in &passes {
            let first = pass.first().copied();
            let last = pass.last().copied();
            if let (Some(first), Some(last)) = (first, last) {
                let d = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
                assert!(
                    d < 1e-6,
                    "fully-steep contour pass should close its loop, gap was {d:.4}mm"
                );
            }
        }
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn steep_shallow_boundary_regions_none_matches_call_without_param() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        let never_cancel = || false;

        let tp_default = steep_shallow_toolpath(&mesh, &si, &cutter, &params);
        let tp_none =
            steep_shallow_toolpath_with_cancel(&mesh, &si, &cutter, &params, None, &never_cancel)
                .unwrap();

        assert_eq!(tp_default.moves.len(), tp_none.moves.len());
        for (a, b) in tp_default.moves.iter().zip(tp_none.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-9);
            assert!((a.target.y - b.target.y).abs() < 1e-9);
            assert!((a.target.z - b.target.z).abs() < 1e-9);
        }
    }

    #[test]
    fn steep_shallow_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        let never_cancel = || false;

        // Left half of the hemisphere's XY footprint (bbox roughly [-20,20]).
        let bbox = &mesh.bbox;
        let left_half = crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(bbox.min.x, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.max.y),
            crate::geo::P2::new(bbox.min.x, bbox.max.y),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let tp = steep_shallow_toolpath_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "cutting move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one cutting move");
    }
}
