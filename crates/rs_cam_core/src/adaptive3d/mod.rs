//! 3D adaptive clearing with constant engagement on mesh surfaces.
//!
//! Maintains constant tool engagement while following an STL mesh surface.
//! Uses tri-dexel stock for volumetric material tracking, drop-cutter
//! queries for Z following, and precomputed surface heightmap for fast
//! engagement computation.
//!
//! Key differences from 2D adaptive:
//! - Material state: tri-dexel stock (volumetric interval lists)
//! - Z at each step: from point_drop_cutter (not constant)
//! - Engagement: "material above surface" not "material vs cleared"
//! - Multi-level: Z levels from stock_top down to mesh surface
//! - Boundary cleanup: waterline contours (not polygon offset contours)

use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::stock::dexel::ray_top;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;

use tracing::info;

mod clearing;
mod path;
mod search;
use path::{adaptive_3d_segments, segments_to_toolpath};

// F-029 probe: re-export the planner-state probe for the F-029 acceptance
// test. Internal — hidden from rustdoc. Will be removed once F-029 lands and
// the parity gap is closed.
#[doc(hidden)]
pub use path::debug_adaptive_3d_segments_for_f029_probe;

/// Region ordering strategy for 3D adaptive clearing.
///
/// `Global` clears all areas at each Z level before moving to the next (default).
/// `ByArea` detects connected material regions via flood fill and clears each
/// region fully (all Z levels) before moving to the next, reducing tool travel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegionOrdering {
    /// Clear all areas at each Z level globally (default, backward compat).
    #[default]
    Global,
    /// Detect connected pockets and clear each fully before moving to the next.
    ByArea,
}

/// Roughing strategy for 3D clearing.
///
/// `ContourParallel` — fast, predictable contour-offset pocketing via EDT.
///   Fixed stepover, concentric contours from boundary inward. Best for
///   bulk roughing where speed matters more than constant engagement.
///   The default.
///
/// `Adaptive` — curvature-adjusted EDT clearing with variable stepover.
///   Produces shorter cutting distance than ContourParallel on curved
///   terrain by adapting the offset spacing to local curvature, without
///   paying the per-step direction-search cost of AgentSearch.
///
/// `AgentSearch` — per-step direction search with preflight skip and
///   widen-band recovery. Slower to generate than ContourParallel or
///   Adaptive but offers finer per-step control; retained for advanced
///   cases where the geometry defeats the EDT-based strategies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClearingStrategy3d {
    /// Per-step direction search with preflight skip and widen-band
    /// recovery. Slow to generate; reach for it when ContourParallel or
    /// Adaptive produce uncut bands on difficult geometry.
    AgentSearch,
    /// Fast contour-parallel offset clearing via EDT (default).
    #[default]
    ContourParallel,
    /// Curvature-adjusted adaptive clearing via variable-offset EDT.
    Adaptive,
    /// Constructive inside-out contour spiral per slice (Stage 1 of the
    /// adaptive algorithm review): one continuous stay-down pass per
    /// region with engagement bounded by wrap spacing. Routes through
    /// the AgentSearch slice dispatch with the spiral as the 2D
    /// generator; falls back to the agent where no starter pocket fits.
    ContourSpiral,
}

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
    /// Engagement radius — the cutter's actual contact radius at `depth_per_pass`
    /// below the tip. Used for stepover, region detection, and material clearing
    /// modeling. For flat/ball cutters this equals the nominal radius; for
    /// tapered cutters it's narrower than the shank radius.
    pub tool_radius: f64,
    /// Envelope radius — the widest extent of the cutter at any height (shank
    /// radius for tapered tools). Used only for keep-out / bbox margins so the
    /// tool's shank doesn't overrun the workpiece footprint.
    pub envelope_radius: f64,
    pub stepover: f64,
    pub depth_per_pass: f64,
    /// Vertical (Z) leave-stock offset above the surface heightmap —
    /// the only stock-to-leave axis this planner supports. All uses key
    /// off `point_drop_cutter` / the surface heightmap in Z; there is no
    /// wall-normal offset, so a distinct radial (sidewall) allowance
    /// cannot be represented here. Adapter callers collapse a
    /// user-facing axial/radial pair down to this single field — see
    /// `compute::execute::adaptive3d_effective_stock_to_leave`.
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
    pub min_cutting_radius: f64,
    pub stock_top_z: f64,
    /// User-pinned floor for the Z-level plan (heights audit 2026-06-12,
    /// finding 2). `Some(z)` clamps `z_bottom` — no pass is planned below
    /// it, even where the surface heightmap reads deeper (e.g. through
    /// holes in an open mesh). `None` (heights Auto) keeps the
    /// surface-derived floor.
    pub z_floor: Option<f64>,
    /// Entry strategy (default: Plunge for backward compat).
    pub entry_style: EntryStyle3d,
    /// Fine stepdown: when set, insert intermediate Z levels at this interval.
    pub fine_stepdown: Option<f64>,
    /// Detect flat areas in the mesh and insert Z levels at shelf heights.
    pub detect_flat_areas: bool,
    /// Region ordering strategy (default: Global for backward compat).
    pub region_ordering: RegionOrdering,
    /// Pre-machined stock for two-sided machining.
    /// When Some, used as starting material instead of a fresh block at stock_top_z.
    pub initial_stock: Option<TriDexelStock>,
    /// Clearing strategy per Z level (default: ContourParallel).
    pub clearing_strategy: ClearingStrategy3d,
    /// Trochoid trigger cap for the ContourSpiral slice path: relief loops
    /// fire when predicted leading-arc engagement exceeds `target × this`.
    /// Low (≈1.0–1.2) = flattest load + more travel; high (≈2.0–3.0) =
    /// relaxed + less travel. Surfaced as the GUI "Nibble" dial; ignored
    /// by the other strategies. See the cap sweep in
    /// `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`.
    pub trochoid_cap_mult: f64,
    /// Engagement quantity the AgentSearch 2D sub-pass measures against
    /// the α/2π target. See `crate::adaptive::EngagementMeasure` (F1,
    /// algorithm review 2026-06-12). Ignored by ContourParallel/Adaptive.
    pub engagement_measure: crate::adaptive::EngagementMeasure,
    /// Blend Z toward terrain surface across contour offsets.
    /// When true, outer contours stay near z_level and inner contours
    /// progressively descend toward the surface. Best for terrain/relief.
    /// When false (default), all contours cut at z_level. Best for pockets.
    pub z_blend: bool,
    /// Optional 2D boundary polygon (e.g. model silhouette) the cutter
    /// center must stay inside. Cells outside this boundary are pre-cleared
    /// in the internal material stock so the bool-grid polygon at every
    /// z-level reflects the boundary, not just the stock bbox. Without this,
    /// AgentSearch's polygon at top z-levels covers the full stock; the
    /// downstream toolpath clip then converts the outside-boundary cuts to
    /// rapids, leaving stock unstamped, and deeper z-levels then bite
    /// through fresh stock with full-depth axial DOC. See investigation
    /// log O5b for the wanaka repro.
    pub boundary: Option<crate::polygon::Polygon2>,
    /// Mill shallow areas: insert fine sub-passes (at
    /// `shallow_stepdown` increments) on cells whose surface slope is
    /// below `shallow_angle_rad`, within each DPP descent. Steep cells
    /// keep the normal DPP cadence. See planning doc
    /// `ADAPTIVE3D_DPP_ISLANDS_AND_SHALLOW_MILL.md` Part B.
    pub mill_shallow_areas: bool,
    /// Slope angle threshold (radians from horizontal) for the shallow
    /// mask. `None` ⇒ disabled. Typical 30° = ~0.524 rad.
    pub shallow_angle_rad: Option<f64>,
    /// Stepdown to use within shallow regions. `None` ⇒ disabled.
    /// Should be < `depth_per_pass`; planner ignores the feature when
    /// either is None or when stepdown >= depth_per_pass.
    pub shallow_stepdown: Option<f64>,
    /// World stock XY bounds the simulator's per-setup dexel grid uses.
    /// When `Some((x_min, y_min, x_max, y_max))`, the planner's internal
    /// `material_stock` is widened so its XY extent encloses **both** the
    /// mesh bbox (+ tool radius) and the world stock bbox. Without this
    /// the planner's grid is bounded by `mesh.bbox + tool_radius`, while
    /// the simulator's per-setup dexel grid is bounded by the world stock
    /// bbox. Cells inside the simulator's grid but outside the planner's
    /// never see planner stamps and carry virgin material across the
    /// entire toolpath; the final pass scrapes the full pre-stamp ray in
    /// one stamp, inflating per-sample axial engagement and the
    /// downstream deflection gate. See finding F-027.
    ///
    /// `None` falls back to the mesh-bbox-only initialization for tests
    /// and call sites that don't have a world stock bbox handy.
    pub world_stock_xy_bbox: Option<(f64, f64, f64, f64)>,
    /// F-038: minimum total horizontal cutting length (mm) a marching-squares
    /// region must produce in its 2D adaptive sub-pass before the planner
    /// commits an entry plunge to it. AgentSearch strategy only. Regions
    /// whose forecast cut length (perimeter-sweep + 2D adaptive walk) is
    /// below this threshold are dropped at plan time, eliminating the
    /// "perimeter micro-plunge" fragmentation the Wanaka Back Rough .nc
    /// exhibited (149 plunges, 90 of which cut ≤ 10 mm). Default 5.0 mm.
    pub min_region_cut_length_mm: f64,
    /// F-038b: maximum XY distance to attempt a keep-tool-down link
    /// between cut groups instead of retract-rapid-plunge. `None` means
    /// "use 8 × tool diameter computed at toolpath build time" — matches
    /// Fusion HSM's typical "stay down distance" setting for roughing on
    /// hardwood. `Some(0.0)` disables the feature (every transition becomes
    /// a retract). Applied in `segments_to_toolpath` against the mesh
    /// heightfield: each candidate link samples the highest mesh Z along
    /// the XY straight line and emits a feed-rate stay-down at
    /// `max(samples, end.z, start.z) + stay_down_clearance_mm` only when
    /// that link Z stays below `safe_z` AND within the tool's cutting
    /// length. Falls back to retract on any safety violation.
    pub max_stay_down_distance_mm: Option<f64>,
    /// F-038b: vertical clearance added on top of the maximum heightfield
    /// sample along a stay-down link. 0.5 mm absorbs dexel/mesh
    /// discretisation noise (~0.5 mm at standard sim resolution) plus a
    /// hair of safety margin so the rapid step over a low peak doesn't
    /// scrape the surface. Default 0.5 mm.
    pub stay_down_clearance_mm: f64,
}

// SurfaceHeightmap is now in crate::surface::slope (shared across finishing strategies)

// ── Helpers mapping TriDexelStock to f64 world used by adaptive ────────

/// Top Z at (row, col) from the Z-grid, as f64. Returns `bottom_z` if the
/// ray is empty (no material).
#[inline]
pub(super) fn stock_top_z_at(stock: &TriDexelStock, row: usize, col: usize) -> f64 {
    ray_top(stock.z_grid.ray(row, col))
        .map(|z| z as f64)
        .unwrap_or(stock.stock_bbox.min.z)
}

/// Whether the Z-grid ray at (row, col) has material above `floor` (f64).
#[inline]
pub(super) fn stock_has_material_above(
    stock: &TriDexelStock,
    row: usize,
    col: usize,
    floor: f64,
) -> bool {
    let ray = stock.z_grid.ray(row, col);
    // Any segment whose exit > floor means material above floor.
    ray.iter().any(|seg| seg.exit as f64 > floor)
}

// ── Segment types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZLevelPlanMetrics {
    pub available: bool,
    pub marching_squares_regions: usize,
    pub region_areas_mm2: Vec<f64>,
    pub dropped_micro_region_count: usize,
    pub perimeter_sweep_length_mm: f64,
    pub agent_walk_cut_length_mm: f64,
    pub residual_cleanup_cell_count: usize,
    /// F-038: number of regions whose forecast cut length fell below
    /// `min_region_cut_length_mm` and were dropped from the AgentSearch
    /// emission. Parallel to `dropped_micro_region_count` (which is the
    /// Fusion-style area filter) but gated on length, not area.
    pub dropped_short_region_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Adaptive3dRuntimeEvent {
    RegionStart {
        region_index: usize,
        region_total: usize,
        cell_count: usize,
    },
    RegionZLevel {
        region_index: usize,
        z_level: f64,
        level_index: usize,
        level_total: usize,
        metrics: ZLevelPlanMetrics,
    },
    GlobalZLevel {
        z_level: f64,
        level_index: usize,
        level_total: usize,
        metrics: ZLevelPlanMetrics,
    },
    WaterlineCleanup,
    PassEntry {
        pass_index: usize,
        entry_x: f64,
        entry_y: f64,
        entry_z: f64,
        /// D4 — exclusive end move index of the entry sequence
        /// (peck/helix/ramp). The event is emitted at
        /// `move_index = entry_start`. Span construction in
        /// `compute::spans::spans_from_adaptive3d_annotations` reads
        /// this to delimit the `SpanKind::Entry` span.
        entry_end_move_idx: usize,
        /// Operator-facing label of the configured entry style at
        /// emission time. One of `"plunge entry"`, `"helix entry"`,
        /// `"ramp entry"`. Stored as `&'static str` to keep the enum
        /// variant small.
        style_label: &'static str,
    },
    PassPreflightSkip {
        pass_index: usize,
    },
    PassSummary {
        pass_index: usize,
        step_count: usize,
        exit_reason: String,
        yield_ratio: f64,
        short: bool,
    },
}

impl Adaptive3dRuntimeEvent {
    pub fn set_z_level_metrics(&mut self, metrics: ZLevelPlanMetrics) {
        match self {
            Self::RegionZLevel { metrics: slot, .. } | Self::GlobalZLevel { metrics: slot, .. } => {
                *slot = metrics;
            }
            Self::RegionStart { .. }
            | Self::WaterlineCleanup
            | Self::PassEntry { .. }
            | Self::PassPreflightSkip { .. }
            | Self::PassSummary { .. } => {}
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::RegionStart {
                region_index,
                region_total,
                cell_count,
            } => format!("Region {region_index}/{region_total} ({cell_count} cells)"),
            Self::RegionZLevel {
                region_index,
                z_level,
                level_index,
                level_total,
                metrics: _,
            } => format!(
                "Region {region_index} — Z {:.1} ({level_index}/{level_total})",
                z_level
            ),
            Self::GlobalZLevel {
                z_level,
                level_index,
                level_total,
                metrics: _,
            } => format!("Adaptive Z {:.1} ({level_index}/{level_total})", z_level),
            Self::WaterlineCleanup => "Waterline cleanup".to_owned(),
            Self::PassEntry {
                pass_index,
                entry_x,
                entry_y,
                entry_z,
                style_label,
                ..
            } => {
                format!(
                    "Pass {pass_index} — {style_label} at ({entry_x:.1}, {entry_y:.1}) Z {entry_z:.1}"
                )
            }
            Self::PassPreflightSkip { pass_index } => {
                format!("Pass {pass_index} — preflight skip (no viable direction)")
            }
            Self::PassSummary {
                pass_index,
                step_count,
                exit_reason,
                yield_ratio,
                short,
            } => {
                if *short {
                    format!(
                        "Pass {pass_index} — short ({step_count} steps, {exit_reason}, yield {yield_ratio:.3})"
                    )
                } else {
                    format!(
                        "Pass {pass_index} — {step_count} steps ({exit_reason}, yield {yield_ratio:.3})"
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adaptive3dRuntimeAnnotation {
    pub move_index: usize,
    pub event: Adaptive3dRuntimeEvent,
}

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
    crate::interrupt::run_uncancellable(|cancel| {
        adaptive_3d_toolpath_with_cancel(mesh, index, cutter, params, cancel)
    })
}

pub fn adaptive_3d_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = adaptive_3d_toolpath_annotated_traced_with_cancel(
        mesh, index, cutter, params, cancel, None,
    )?;
    Ok(tp)
}

/// **Test door.** The `#[cfg(test)]` module of this file is the only
/// caller. No production path reads it (S29, tech debt 2026-09-16).
#[cfg(test)]
pub(crate) fn adaptive_3d_toolpath_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = adaptive_3d_toolpath_annotated_traced_with_cancel(
        mesh, index, cutter, params, cancel, debug,
    )?;
    Ok(tp)
}

/// Like `adaptive_3d_toolpath` but also returns annotations for simulation display.
/// Each annotation is `(move_index, label)`.
///
/// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
/// only callers. No production path reads it.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(tool_radius = params.tool_radius, stepover = params.stepover))]
#[allow(clippy::expect_used)]
pub fn adaptive_3d_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> (Toolpath, Vec<(usize, String)>) {
    crate::interrupt::run_uncancellable(|cancel| {
        adaptive_3d_toolpath_annotated_with_cancel(mesh, index, cutter, params, cancel)
    })
}

pub(crate) fn adaptive_3d_toolpath_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    adaptive_3d_toolpath_annotated_traced_with_cancel(mesh, index, cutter, params, cancel, None)
}

// Stage 4 — the third tuple element carries planner-predicted leading-arc
// engagement samples `(cut_point, α/2π)`; empty for non-ContourSpiral
// strategies. The return is a 3-tuple rather than a named struct to keep
// the existing callers' destructuring; the `type_complexity` allow is
// scoped to this one signature.
#[allow(clippy::type_complexity)]
pub fn adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<Adaptive3dRuntimeAnnotation>, Vec<(P3, f64)>), Cancelled> {
    let result = adaptive_3d_segments(mesh, index, cutter, params, debug, cancel)?;
    let segments = result.segments;
    let planner_engagement = result.planner_engagement;
    // F-038b: pass mesh + spatial index + cutter so segments_to_toolpath
    // can query the heightfield along each candidate stay-down link.
    let (tp, annotations) = segments_to_toolpath(&segments, params, mesh, index, cutter);
    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    info!(
        moves = tp.moves.len(),
        annotations = annotations.len(),
        cutting_mm = tp.total_cutting_distance(),
        rapid_mm = tp.total_rapid_distance(),
        planner_eng_samples = planner_engagement.len(),
        "3D adaptive toolpath complete"
    );

    Ok((tp, annotations, planner_engagement))
}

pub fn adaptive_3d_toolpath_annotated_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    let (tp, annotations, _planner_engagement) =
        adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
            mesh, index, cutter, params, cancel, debug,
        )?;
    Ok((
        tp,
        crate::compute::spans::runtime_annotations_to_labels(&annotations),
    ))
}

#[cfg(test)]
mod tests;
