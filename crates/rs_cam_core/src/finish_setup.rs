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
use crate::measurement::CellSource;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;

// ── Resolution policy (H3 steps 1-2) ────────────────────────────────────

/// Which formula resolved a finish grid's cell size.
///
/// H3 step 1 (`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`)
/// takes the cell-size derivation OUT of the shared grid builder and makes it
/// a value the caller selects. Before this, `build_finish_surface_with_cancel`
/// applied `(envelope_radius / 4).max(tolerance)` internally and every finish
/// op inherited it silently — so a resolution experiment on ONE op was
/// impossible without moving all three. The formula is now
/// [`Self::LegacyEnvelopeQuarter`]: one named variant among several, not a
/// hidden default.
///
/// PR-3 changes NO cell size. All three standalone consumers (Scallop,
/// RampFinish, SteepShallow) select `LegacyEnvelopeQuarter` and the
/// classification builder selects [`Self::CuspQuarter`], which is exactly
/// what each computed before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinishResolutionMode {
    /// `(envelope_radius / 4).max(tolerance)` — the pre-H3 generation-grid
    /// formula, shared verbatim by `scallop.rs`, `ramp_finish.rs` and
    /// `steep_shallow.rs`. On a tapered ball the envelope is the SHAFT, so
    /// this is the coarse end of the split (0.75 mm for a Ø1 tip on a Ø6
    /// shank) — the open question H3 exists to answer.
    LegacyEnvelopeQuarter,
    /// `(cusp_radius / 4).max(tolerance)` — the tip scale, i.e. the finest
    /// feature the cutter can actually leave. Used today by the
    /// classification grid; offered here so a *generation* consumer can be
    /// moved onto it one at a time (H3 fix-sequence step 4) without touching
    /// the shared builder or its siblings.
    CuspQuarter,
    /// Caller-pinned cell size: harness fixtures, resolution A/B experiments,
    /// and any dial that is not derived from the tool at all.
    Explicit,
}

impl FinishResolutionMode {
    /// Human-readable label for reports and failure messages.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LegacyEnvelopeQuarter => "legacy envelope/4",
            Self::CuspQuarter => "cusp/4",
            Self::Explicit => "caller-pinned",
        }
    }

    /// The [`CellSource`] a surface built under this mode carries.
    ///
    /// This is the one place the mode↔provenance mapping lives, so a new
    /// mode cannot be added without deciding what it claims about scale.
    #[must_use]
    pub const fn cell_source(self) -> CellSource {
        match self {
            Self::LegacyEnvelopeQuarter => CellSource::EnvelopeRadius,
            Self::CuspQuarter => CellSource::CuspRadius,
            Self::Explicit => CellSource::Explicit,
        }
    }
}

/// A resolved finish-grid resolution: the formula that was selected, the
/// millimetre value it produced, and whether the tolerance floor bound.
///
/// Constructed at the CALL SITE (see `scallop::scallop_generation_resolution`
/// and its siblings) and handed to
/// [`build_finish_surface_with_policy_and_cancel`]. Carrying the resolved
/// value alongside the mode is what lets PR-8's A/B swap ONE consumer's
/// policy — and lets a sentry assert which policy that consumer selected —
/// without reaching into the shared builder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinishResolutionPolicy {
    mode: FinishResolutionMode,
    cell_mm: f64,
    tolerance_floor_applied: bool,
}

impl FinishResolutionPolicy {
    /// `(envelope_radius / 4).max(tolerance)` — [`FinishResolutionMode::LegacyEnvelopeQuarter`].
    #[must_use]
    pub fn legacy_envelope_quarter(cutter: &dyn MillingCutter, tolerance: f64) -> Self {
        // The envelope on a tapered tool is the SHAFT. Kept deliberately
        // spelled `envelope_radius_mm()` (PR-2 named accessor) rather than
        // `radius()`: the scale is no longer implicit, it is the name of the
        // variant. Whether it is the RIGHT scale is H3 step 3+.
        Self::from_formula(
            FinishResolutionMode::LegacyEnvelopeQuarter,
            cutter.envelope_radius_mm() / 4.0,
            tolerance,
        )
    }

    /// `(cusp_radius / 4).max(tolerance)` — [`FinishResolutionMode::CuspQuarter`].
    #[must_use]
    pub fn cusp_quarter(cutter: &dyn MillingCutter, tolerance: f64) -> Self {
        Self::from_formula(
            FinishResolutionMode::CuspQuarter,
            cutter.cusp_radius_mm() / 4.0,
            tolerance,
        )
    }

    /// A caller-pinned cell size — [`FinishResolutionMode::Explicit`].
    #[must_use]
    pub const fn explicit(cell_mm: f64) -> Self {
        Self {
            mode: FinishResolutionMode::Explicit,
            cell_mm,
            tolerance_floor_applied: false,
        }
    }

    fn from_formula(mode: FinishResolutionMode, derived_mm: f64, tolerance: f64) -> Self {
        Self {
            mode,
            cell_mm: derived_mm.max(tolerance),
            tolerance_floor_applied: tolerance > derived_mm,
        }
    }

    /// Which formula was selected.
    #[must_use]
    pub const fn mode(self) -> FinishResolutionMode {
        self.mode
    }

    /// The resolved grid cell size (mm).
    #[must_use]
    pub const fn cell_mm(self) -> f64 {
        self.cell_mm
    }

    /// The provenance tag a surface built under this policy carries.
    #[must_use]
    pub const fn cell_source(self) -> CellSource {
        self.mode.cell_source()
    }

    /// True when the `.max(tolerance)` floor — not the tool scale — set
    /// [`Self::cell_mm`].
    ///
    /// [`Self::cell_source`] still names the FORMULA FAMILY in that case
    /// (unchanged from pre-PR-3 behavior, which the PR-2 tripwire pins), so
    /// this flag is how a report says "the tool scale did not actually decide
    /// this grid". Two policies with different modes but a bound tolerance
    /// floor produce the same cell yet compare as different sources — a
    /// conservative false negative in `MeasurementProvenance::comparable_to`,
    /// logged as an adjacent defect rather than silently repaired here.
    #[must_use]
    pub const fn tolerance_floor_applied(self) -> bool {
        self.tolerance_floor_applied
    }
}

// ── Heightmap + slope map setup ─────────────────────────────────────────

/// A surface sampled onto a regular XY grid, ready for finish-op planning:
/// the raw heightmap (Z values + coverage mask) and its derived slope map
/// (angles, normals, curvature).
pub struct FinishSurface {
    pub heightmap: SurfaceHeightmap,
    pub slope_map: SlopeMap,
    /// Which tool scale sized [`Self::cell_size`] (M1 provenance, §14q/X-6).
    ///
    /// The generation and classification builders below choose *different*
    /// radii, so on a tapered tool two surfaces of the same mesh have cells
    /// that differ by the shaft/tip ratio and do NOT align 1:1. Areas
    /// measured on one are not comparable with areas measured on the other,
    /// and this field is what lets a report say so.
    ///
    /// Always equal to `resolution.cell_source()` — kept as its own field
    /// because it is the M1 provenance tag consumers read; the invariant is
    /// pinned by `tests/finish_resolution_policy_pr3.rs`.
    pub cell_source: CellSource,
    /// The resolution policy that sized this grid (H3 step 1). Says which
    /// FORMULA was selected, not just what it produced, so a sentry can
    /// assert a consumer's choice and a report can name it.
    pub resolution: FinishResolutionPolicy,
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
/// coverage right up to the model boundary), at the resolution `resolution`
/// resolved to.
///
/// **The single generation-grid builder** (H3 step 1). It applies no formula
/// of its own: the caller selects a [`FinishResolutionPolicy`] and this
/// function honours it, tagging the surface with the policy's provenance. The
/// two older entry points below are thin adapters that select a policy on the
/// caller's behalf.
pub fn build_finish_surface_with_policy_and_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    let cell_size = resolution.cell_mm();
    // Grid PADDING is physical sweep, so it is ENVELOPE by contract
    // (`TOOL_SCALE_SEMANTICS.md` §8 row 2 — must stay envelope), regardless
    // of which scale the RESOLUTION policy picked.
    let tool_radius = cutter.envelope_radius_mm();
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
        cell_source: resolution.cell_source(),
        resolution,
    })
}

/// Build a [`FinishSurface`] at an explicit grid resolution of `cell_size`.
///
/// Adapter over [`build_finish_surface_with_policy_and_cancel`] selecting
/// [`FinishResolutionPolicy::explicit`]. Some pre-existing tests pin a fixed
/// `cell_size` directly (e.g. `1.0`, independent of tool radius/tolerance);
/// those keep calling this variant so migrating them onto the shared helper
/// doesn't silently change their sampling resolution. It is also the entry
/// point H3's resolution A/B harness drives.
pub fn build_finish_surface_with_cell_size_and_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    build_finish_surface_with_policy_and_cancel(
        mesh,
        index,
        cutter,
        FinishResolutionPolicy::explicit(cell_size),
        cancel,
    )
}

/// Build a [`FinishSurface`] under [`FinishResolutionMode::LegacyEnvelopeQuarter`]:
/// `(envelope_radius / 4).max(tolerance)`.
///
/// Adapter kept for callers (and sentries) that want the legacy formula
/// without naming a policy. **Production ops do not call this**: since H3
/// step 2, `scallop.rs`, `ramp_finish.rs` and `steep_shallow.rs` each select
/// their own policy at their own call site and go through
/// [`build_finish_surface_with_policy_and_cancel`], so one op's resolution
/// can be changed without touching the others. Changing the formula HERE
/// would no longer move production — which is the point.
pub fn build_finish_surface_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    // The scale question `TOOL_SCALE_SEMANTICS.md` §6.B row B1 routed to H3
    // is now ANSWERED-BY-NAME rather than hidden: `LegacyEnvelopeQuarter`
    // says which radius sizes the cell and admits, in its own name, that it
    // is the inherited choice rather than a justified one. H3 steps 3+ decide
    // whether any consumer should move off it.
    build_finish_surface_with_policy_and_cancel(
        mesh,
        index,
        cutter,
        FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance),
        cancel,
    )
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
/// Grid origin and extent mirror [`build_finish_surface_with_cancel`] for the
/// same `cutter`. **Resolution no longer does**: classification selects
/// [`FinishResolutionMode::CuspQuarter`] while the generation consumers select
/// [`FinishResolutionMode::LegacyEnvelopeQuarter`], so on a tapered ball
/// the two grids differ by the shaft/tip ratio (0.125 mm vs 0.75 mm for a Ø1
/// tip on a 6 mm shank) and cells do NOT align 1:1. Whether generation should
/// follow classification is an open question with real cost on both sides —
/// see `planning/unified_v3_design.md` §14u.
///
/// Cells beyond the mesh footprint are simply uncovered (the probe contacts
/// nothing there), which also guarantees the non-contact margin ring the
/// mask→polygon extractor needs.
pub fn build_classification_surface_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    build_classification_surface_with_policy_and_cancel(
        mesh,
        index,
        cutter,
        FinishResolutionPolicy::cusp_quarter(cutter, tolerance),
        cancel,
    )
}

/// Resolution-explicit variant of [`build_classification_surface_with_cancel`]
/// (H3 step 1).
///
/// Same probe and same sharp slope stencil — only the grid resolution comes
/// from the caller. `UnifiedFinish` selects
/// [`FinishResolutionMode::CuspQuarter`] here (see
/// `unified_finish::unified_finish_classification_resolution`), which is what
/// the builder computed internally before.
pub fn build_classification_surface_with_policy_and_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<FinishSurface, Cancelled> {
    // Physical extent below (padding, grid coverage) keeps the FULL radius —
    // the tool really does sweep that far. The CELL SIZE does not: it sets
    // the finest feature this grid can represent, so it follows the
    // cusp-forming (tip) radius. On a tapered ball `radius()` is the SHAFT
    // — 3.0 mm for a Ø1 tip — which made the classification cell 0.75 mm
    // and left wanaka's steep ribbons unrepresentable: the decomposition
    // emitted ZERO VerySteep regions on terrain whose true faces reach 89°
    // (design doc §14q). Do NOT restate the recovered area as a fraction of
    // the mesh's ≥75° face area — region polygons are XY-PROJECTED and mesh
    // faces are 3D, a ~10× difference on near-vertical ribbons; the audit
    // that caught that is §14t.
    let tool_radius = cutter.envelope_radius_mm();
    let cell_size = resolution.cell_mm();
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
        // Production callers select `CuspQuarter` here — the TIP scale.
        cell_source: resolution.cell_source(),
        resolution,
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
