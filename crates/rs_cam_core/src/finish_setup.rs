//! Shared setup helpers for mesh-finishing operations that sample a surface
//! onto a regular XY grid: heightmap + slope-map construction, the
//! slope-window filter sentinel, and the Z-ladder used to walk from a top Z
//! down to a bottom Z in fixed steps.
//!
//! Extracted 2026-07 (`planning/finishing_stack_review_2026-07.md` P1.2) from
//! near-verbatim setup blocks in `scallop.rs`, `ramp_finish.rs`, and
//! `steep_shallow.rs` (plus test-module copies in the first two). `waterline.rs`
//! and `execute.rs` have their own copies of parts of this (the Z-ladder and
//! the slope-window sentinel respectively) that are out of scope for this
//! pass — noted for a follow-up.

use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;

// ── Heightmap + slope map setup ─────────────────────────────────────────

/// A surface sampled onto a regular XY grid, ready for finish-op planning:
/// the raw heightmap (Z values + coverage mask) and its derived slope map
/// (angles, normals, curvature).
pub struct FinishSurface {
    pub heightmap: SurfaceHeightmap,
    pub slope_map: SlopeMap,
}

impl FinishSurface {
    /// Grid cell size (mm), shared by `heightmap` and `slope_map`.
    pub fn cell_size(&self) -> f64 {
        self.heightmap.cell_size
    }

    pub fn rows(&self) -> usize {
        self.heightmap.rows
    }

    pub fn cols(&self) -> usize {
        self.heightmap.cols
    }
}

/// Build a [`FinishSurface`] over `mesh`'s own bounding box, expanded by one
/// cutter radius on every side (so the cutter's full extent has heightmap
/// coverage right up to the model boundary), at an explicit grid resolution
/// of `cell_size`.
///
/// This is the resolution-explicit entry point. Production call sites derive
/// `cell_size` from tool radius and tolerance instead — see
/// [`build_finish_surface_with_cancel`]. Some pre-existing tests pin a fixed
/// `cell_size` directly (e.g. `1.0`, independent of tool radius/tolerance);
/// those keep calling this variant so migrating them onto the shared helper
/// doesn't silently change their sampling resolution.
pub fn build_finish_surface_with_cell_size_and_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    let tool_radius = cutter.radius();
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;
    let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
    let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

    let heightmap = SurfaceHeightmap::from_mesh_with_cancel(
        mesh, index, cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z, cancel,
    )?;
    let slope_map = heightmap.slope_map();
    Ok(FinishSurface {
        heightmap,
        slope_map,
    })
}

/// Build a [`FinishSurface`] using the standard finish-op cell-size formula:
/// `(tool_radius / 4).max(tolerance)`.
///
/// As of 2026-07 this formula is shared verbatim by the production call
/// sites in `scallop.rs`, `ramp_finish.rs`, and `steep_shallow.rs` (each
/// passes its own `params.tolerance`). If a future op needs a different
/// formula, add a new entry point rather than bending this one — don't
/// silently change another op's resolution to fit a new caller.
pub fn build_finish_surface_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    let cell_size = (cutter.radius() / 4.0).max(tolerance);
    build_finish_surface_with_cell_size_and_cancel(mesh, index, cutter, cell_size, cancel)
}

/// Diameter of the bare-surface probe used by
/// [`build_classification_surface_with_cancel`] — small enough that the
/// probe's own offset is negligible at finish cell sizes, mirroring
/// `rest_field`'s "tiny bare-surface probe" reference pattern.
pub const CLASSIFICATION_PROBE_DIAMETER_MM: f64 = 0.05;

/// Build the CLASSIFICATION surface: the true model surface sampled with a
/// tiny bare-surface probe, NOT `cutter`'s tool-center offset surface.
///
/// Slope-band decomposition (`crate::finish_planner`) must read real surface
/// slopes: a ball tool's offset surface geometrically hides steepness at
/// feature scales at or below the ball radius — the ball bridges the feature
/// and its center glides over a smoothed blanket. Measured on the wanaka
/// relief (6 mm features, Ø6 ball): 38.5% of true surface area is ≥45°, but
/// only 0.1% of the offset surface reads that steep, with a max of 52° vs a
/// true 89°. The pre-existing `steep_shallow` op classifies on the offset
/// surface and shares this blind spot.
///
/// Grid origin, extent, and resolution mirror [`build_finish_surface_with_cancel`]
/// for the same `cutter`, so classification cells align 1:1 with the
/// generation surface's cells. Cells beyond the mesh footprint are simply
/// uncovered (the probe contacts nothing there), which also guarantees the
/// non-contact margin ring the mask→polygon extractor needs.
pub fn build_classification_surface_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    let tool_radius = cutter.radius();
    let cell_size = (tool_radius / 4.0).max(tolerance);
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;
    let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
    let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

    let probe = crate::tool::BallEndmill::new(CLASSIFICATION_PROBE_DIAMETER_MM, 1.0);
    let heightmap = SurfaceHeightmap::from_mesh_with_cancel(
        mesh, index, &probe, origin_x, origin_y, rows, cols, cell_size, bbox.min.z, cancel,
    )?;
    // Max-of-one-sided-gradients stencil: central differences smear a
    // single-cell cliff (e.g. the wanaka lake coastline, ~90° step walls)
    // to `atan(h / (2·cell))` — invisible to the steep threshold. The
    // classification surface exists to read TRUE steepness, so it also
    // gets the sharp stencil. Generation surfaces keep `slope_map()`.
    let slope_map = heightmap.slope_map_max_gradient();
    Ok(FinishSurface {
        heightmap,
        slope_map,
    })
}

// ── Slope-window filter ──────────────────────────────────────────────────

/// Lower sentinel (degrees) for the slope-confinement window: at or below
/// this `slope_from`, the window's bottom edge is treated as "no floor".
pub const SLOPE_FILTER_MIN_DEG: f64 = 0.01;

/// Upper sentinel (degrees) for the slope-confinement window: at or above
/// this `slope_to`, the window's top edge is treated as "no ceiling".
pub const SLOPE_FILTER_MAX_DEG: f64 = 89.99;

/// True when `slope_from`/`slope_to` narrow the window below the full
/// `[0, 90]` degree range — i.e. the slope-confinement filter should
/// actually run rather than passing every surface point through.
///
/// Migrated verbatim from the identical `scallop.rs` / `ramp_finish.rs`
/// call sites (`params.slope_from > 0.01 || params.slope_to < 89.99`). A
/// third copy in `execute.rs` is out of scope for this pass.
pub fn slope_filter_active(slope_from: f64, slope_to: f64) -> bool {
    slope_from > SLOPE_FILTER_MIN_DEG || slope_to < SLOPE_FILTER_MAX_DEG
}

// ── Z ladder ──────────────────────────────────────────────────────────────

/// Default inclusive-bounds epsilon for [`z_ladder`], matching
/// `steep_shallow.rs`'s prior fixed `0.01` literal. `ramp_finish.rs` instead
/// derives its epsilon from the step size (`z_step * 0.5`) — pass that in
/// explicitly rather than using this default.
pub const Z_LADDER_DEFAULT_EPSILON: f64 = 0.01;

/// Step down from `top` to `bottom` in increments of `step`, returning the
/// visited levels in descending order starting at `top`.
///
/// Two pre-existing call sites (`steep_shallow.rs` and `ramp_finish.rs`)
/// diverged on how to treat the bottom edge when `(top - bottom)` isn't a
/// whole multiple of `step`. Neither op's tests pin an exact level count at
/// that boundary, but the two policies are genuinely different (not just a
/// differing epsilon), so both are preserved here via `snap_to_bottom`
/// rather than picked between:
///
/// - `snap_to_bottom = false` (steep_shallow's prior behavior): keep
///   stepping while `z >= bottom - epsilon`. The ladder is **not**
///   guaranteed to include `bottom` exactly — the last level can land
///   anywhere in `[bottom - epsilon, bottom + step)`.
/// - `snap_to_bottom = true` (ramp_finish's prior behavior): keep stepping
///   while `z > bottom + epsilon`, then unconditionally push `bottom` as
///   the final level. This guarantees the ladder starts at `top` and ends
///   exactly at `bottom`, and that the last two levels are never closer
///   than `epsilon` apart (a would-be near-duplicate final step is
///   replaced outright by the exact bottom value).
pub fn z_ladder(top: f64, bottom: f64, step: f64, epsilon: f64, snap_to_bottom: bool) -> Vec<f64> {
    let mut levels = Vec::new();
    let mut z = top;
    if snap_to_bottom {
        while z > bottom + epsilon {
            levels.push(z);
            z -= step;
        }
        levels.push(bottom);
    } else {
        while z >= bottom - epsilon {
            levels.push(z);
            z -= step;
        }
    }
    levels
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ── slope_filter_active ─────────────────────────────────────────────

    #[test]
    fn slope_filter_inactive_at_default_sentinels() {
        assert!(!slope_filter_active(
            SLOPE_FILTER_MIN_DEG,
            SLOPE_FILTER_MAX_DEG
        ));
        assert!(!slope_filter_active(0.0, 90.0));
    }

    #[test]
    fn slope_filter_active_just_past_min_sentinel() {
        assert!(slope_filter_active(0.011, SLOPE_FILTER_MAX_DEG));
    }

    #[test]
    fn slope_filter_active_just_past_max_sentinel() {
        assert!(slope_filter_active(SLOPE_FILTER_MIN_DEG, 89.98));
    }

    // ── z_ladder ─────────────────────────────────────────────────────────

    #[test]
    fn z_ladder_exact_multiple_hits_bottom_either_policy() {
        let no_snap = z_ladder(10.0, 0.0, 2.0, 0.01, false);
        assert_eq!(no_snap, vec![10.0, 8.0, 6.0, 4.0, 2.0, 0.0]);

        let snap = z_ladder(10.0, 0.0, 2.0, 1.0, true);
        assert_eq!(snap, vec![10.0, 8.0, 6.0, 4.0, 2.0, 0.0]);
    }

    #[test]
    fn z_ladder_no_snap_may_stop_short_of_bottom() {
        // steep_shallow's prior policy: no guarantee the ladder ever emits
        // `bottom` exactly when the range isn't a whole multiple of `step`.
        let levels = z_ladder(10.0, 1.0, 2.0, 0.01, false);
        assert_eq!(levels, vec![10.0, 8.0, 6.0, 4.0, 2.0]);
    }

    #[test]
    fn z_ladder_snap_always_ends_exactly_on_bottom() {
        // ramp_finish's prior policy: always append the exact bottom,
        // skipping a would-be near-duplicate final step.
        let levels = z_ladder(10.0, 1.0, 2.0, 1.0, true);
        assert_eq!(levels, vec![10.0, 8.0, 6.0, 4.0, 1.0]);
    }

    #[test]
    fn z_ladder_no_snap_boundary_inclusive_at_bottom_minus_epsilon() {
        // z == bottom - epsilon exactly must still be included (`>=`).
        let levels = z_ladder(4.0, 2.01, 2.0, 0.01, false);
        assert_eq!(levels, vec![4.0, 2.0]);
    }

    #[test]
    fn z_ladder_no_snap_boundary_exclusive_just_past_epsilon() {
        // z just below `bottom - epsilon` must be excluded.
        let levels = z_ladder(4.0, 2.02, 2.0, 0.01, false);
        assert_eq!(levels, vec![4.0]);
    }

    #[test]
    fn z_ladder_snap_boundary_exclusive_at_bottom_plus_epsilon() {
        // The natural next level (6.0) sits exactly at `bottom + epsilon`
        // (4.0 + 2.0); the strict `>` must exclude it from the loop so it's
        // superseded by the unconditional bottom push rather than appearing
        // twice.
        let levels = z_ladder(8.0, 4.0, 2.0, 2.0, true);
        assert_eq!(levels, vec![8.0, 4.0]);
    }

    #[test]
    fn z_ladder_single_level_when_step_exceeds_range() {
        let levels = z_ladder(10.0, 9.5, 100.0, 0.01, false);
        assert_eq!(levels, vec![10.0]);
    }
}
