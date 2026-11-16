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
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::stock::dexel::ray_top;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;

use tracing::info;

mod centre_clip;
mod clearing;
pub(crate) use clearing::effective_trochoid_cap_mult;
mod intended_floor;
mod keep_down;
mod left_stock;
mod level_contour;
mod outer_band;
mod path;
mod region_map;
mod rest_region_filter;
mod rest_regions;
mod search;
pub use intended_floor::{FloorGrid, intended_floor_grid};
pub use left_stock::{LeftStock, LeftWhy};
use path::adaptive_3d_segments;
#[cfg(test)]
use path::segments_to_toolpath;
pub use region_map::{AreaRegion, AreaRegionMap, AreaRegionRun};

// F-029 probe: re-export the planner-state probe for the harnesses that
// compare planner stock to simulator stock. A test door behind
// `test-support` (F085).
#[cfg(feature = "test-support")]
pub use path::debug_adaptive_3d_segments_for_f029_probe;
// P22f-2a: the level plan probe (2D plan cut cells against the 3D planner
// stock), a test door behind `test-support`.
#[cfg(feature = "test-support")]
pub use path::debug_level_plan_probe;
// P22f-2c: the step-read probe of the 3D stock read (test-support only).
#[cfg(feature = "test-support")]
pub use clearing::step_probe;

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
/// `GeodesicAdaptive` — the geodesic-wavefront plan per slice piece.
///
/// The removal steps (operator ruling 2026-10-28) removed `Adaptive` (R3,
/// the curvature-adjusted variable-offset EDT planner), `AgentSearch` and
/// `ContourSpiral` (R4). Their slice pipeline and the 2D agent planner
/// stay: Geodesic Adaptive plans through them (the F-038 forecast reads the
/// 2D agent planner).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClearingStrategy3d {
    /// Fast contour-parallel offset clearing via EDT (default).
    #[default]
    ContourParallel,
    /// Geodesic Adaptive (integration step 7.4): the 2D geodesic-wavefront
    /// plan (`adaptive::geodesic`, C35 plan A) plans each slice piece
    /// through the slice dispatch (`clear_z_level_slice`). It clears the body: a piece
    /// where the field does not apply is left to a rest pass. Its planned
    /// turns carry a feed cap (`Move::feed_cap`).
    GeodesicAdaptive,
}

impl From<crate::compute::operation_configs::RegionOrdering> for RegionOrdering {
    /// The config order to the planner order (W6.5). The Layered runner
    /// reads the same config enum for its own ops (R4).
    fn from(o: crate::compute::operation_configs::RegionOrdering) -> Self {
        use crate::compute::operation_configs::RegionOrdering as Config;
        match o {
            Config::Global => Self::Global,
            Config::ByArea => Self::ByArea,
        }
    }
}

impl From<crate::compute::operation_configs::ClearingStrategy> for ClearingStrategy3d {
    /// The config strategy to the planner strategy (W6.5).
    fn from(s: crate::compute::operation_configs::ClearingStrategy) -> Self {
        use crate::compute::operation_configs::ClearingStrategy as Config;
        match s {
            Config::ContourParallel => Self::ContourParallel,
            Config::GeodesicAdaptive => Self::GeodesicAdaptive,
        }
    }
}

/// Entry strategy for 3D adaptive (replaces vertical plunge).
#[derive(Debug, Clone, Copy, Default)]
pub enum EntryStyle3d {
    /// Vertical plunge (default prior behavior).
    #[default]
    Plunge,
    /// Helical entry: spiral down with given radius and pitch (mm/rev).
    /// `fallback_ramp_angle_deg` is the angle of the ramp that replaces a
    /// helix bore with no room (P29-8, lead ruling 2026-11-12: the op's own
    /// ramp angle, from `feeds::entry_rule::ResolvedEntry`).
    Helix {
        radius: f64,
        pitch: f64,
        fallback_ramp_angle_deg: f64,
    },
    /// Ramp entry: descend at a shallow angle along the next cutting direction.
    Ramp { max_angle_deg: f64 },
}

/// The cutter and the XY frame it works in: what cuts, how wide it steps,
/// and where it may not go.
pub struct Adaptive3dGeometry {
    /// Engagement radius — the cutter's actual contact radius at
    /// `depth_per_pass` below the tip. Used for stepover, region detection,
    /// and material clearing modeling. For flat/ball cutters this equals the
    /// nominal radius; for tapered cutters it is narrower than the shank
    /// radius.
    pub tool_radius: f64,
    /// Envelope radius — the widest extent of the cutter at any height (shank
    /// radius for tapered tools). Used only for keep-out / bbox margins so the
    /// tool's shank does not overrun the workpiece footprint.
    pub envelope_radius: f64,
    pub stepover: f64,
    pub tolerance: f64,
    /// The operation's segment merge tolerance (mm) when its segment merge
    /// is on (`SegmentMergeParams::tolerance`), else `None`.
    ///
    /// G-PLANSIMGAP (2026-09-26): the planner simplifies each cut at
    /// [`Self::cut_simplify_tolerance`] BEFORE it stamps the cut, so it
    /// stamps the path that ships. The dressup pipeline then does not merge
    /// the 3D Rough again
    /// (`OperationTransformCapabilities::planner_applies_segment_merge`).
    /// Before, the merge ran after the planner: the shipped path left up to
    /// 2.34 mm of material that the planner stock showed as cut (rivmap100),
    /// and entries read that stock.
    pub segment_merge_tolerance: Option<f64>,
    pub min_cutting_radius: f64,
    /// Optional 2D boundary polygon (e.g. model silhouette) the cutter
    /// center must stay inside. Cells outside this boundary are pre-cleared
    /// in the internal material stock so the bool-grid polygon at every
    /// z-level reflects the boundary, not just the stock bbox. Without this,
    /// the slice polygon at top z-levels covers the full stock; the
    /// downstream toolpath clip then converts the outside-boundary cuts to
    /// rapids, leaving stock unstamped, and deeper z-levels then bite
    /// through fresh stock with full-depth axial DOC. See investigation
    /// log O5b for the wanaka repro.
    pub boundary: Option<crate::polygon::Polygon2>,
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
    /// and call sites that do not have a world stock bbox handy.
    pub world_stock_xy_bbox: Option<(f64, f64, f64, f64)>,
    /// The containment polygons the session clips this path to after
    /// generation (the boundary regions inset by the tool radius for
    /// `Inside`, outset for `Outside`, as is for `Center`). Empty: no clip.
    ///
    /// G-BOUNDARYPHANTOM (2026-10-01): the planner keeps every tool centre
    /// inside these polygons and stamps only what the clip keeps. Before,
    /// it planned and stamped cuts between `boundary` and the containment
    /// line; the clip deleted them and a later entry read the phantom cut
    /// as its rapid floor (rivmap350 "3D Rough 8", rapids up to 11 mm into
    /// stock).
    pub centre_boundary: Vec<crate::polygon::Polygon2>,
    /// G-DRAPEGAP (2026-11-02): the model gouge guard's reduced tool (the
    /// cutter less the op's chord tolerance on its radius,
    /// `compute::model_gouge::allowance_probe_radial_tolerance_mm`). The
    /// chord lift (`path::lift_chords_to_leave`) reads the rest height with
    /// it, so a tool tangent to a wall is not lifted over the wall and the
    /// path holds the guard's own limit. `None`: no chord lift (the path the
    /// planner shipped before G-DRAPEGAP); the session always sets it for a
    /// tool that can shrink (`compute::execute::rough_3d`).
    pub gouge_probe: Option<std::sync::Arc<dyn crate::tool::MillingCutter>>,
}

impl Adaptive3dGeometry {
    /// The RDP tolerance the emitter and the planner stamp both apply to a
    /// cut: the larger of the operation tolerance and the segment merge
    /// tolerance. One value for both, so the planner stamps what ships.
    pub fn cut_simplify_tolerance(&self) -> f64 {
        Self::chord_tolerance(self.tolerance, self.segment_merge_tolerance)
    }

    /// The rule of [`Self::cut_simplify_tolerance`] on its two inputs: the
    /// operation `tolerance` and the segment merge tolerance (when the
    /// merge is on). The model gouge guard of the 3D Rough reads it too
    /// (`OperationConfig::model_gouge_limit_offset_mm`, G-ROUGHGOUGE): a
    /// simplified chord can sit this far below a draped point.
    #[must_use]
    pub fn chord_tolerance(tolerance: f64, segment_merge_tolerance: Option<f64>) -> f64 {
        segment_merge_tolerance.map_or(tolerance, |m| tolerance.max(m))
    }
}

/// The Z plan: how deep each pass goes and which extra levels join the
/// ladder.
pub struct Adaptive3dDepth {
    pub depth_per_pass: f64,
    /// Vertical (Z) leave-stock offset above the surface heightmap —
    /// the only stock-to-leave axis this planner supports. All uses key
    /// off `point_drop_cutter` / the surface heightmap in Z; there is no
    /// wall-normal offset, so a distinct radial (sidewall) allowance
    /// cannot be represented here. Adapter callers collapse a
    /// user-facing axial/radial pair down to this single field — see
    /// `compute::execute::adaptive3d_effective_stock_to_leave`.
    pub stock_to_leave: f64,
    pub stock_top_z: f64,
    /// User-pinned floor for the Z-level plan (heights audit 2026-06-12,
    /// finding 2). `Some(z)` clamps `z_bottom` — no pass is planned below
    /// it, even where the surface heightmap reads deeper (e.g. through
    /// holes in an open mesh). `None` (heights Auto) keeps the
    /// surface-derived floor.
    pub z_floor: Option<f64>,
    /// Detect flat areas in the mesh and insert Z levels at shelf heights.
    pub detect_flat_areas: bool,
    /// D50: the top Z of the stock the source op left in this op's
    /// `DerivedRestRegions` boundary
    /// ([`crate::trace::produced::Produced::rest_regions_top_z`]). On
    /// remaining stock the level plan starts no higher than it (a ceiling
    /// on the G-LADDERANCHOR anchor, `path.rs::rest_capped_level_top`). On
    /// fresh stock it is not read: the planner then holds the whole stock
    /// above it. `None`: no such boundary.
    pub rest_top_z: Option<f64>,
}

/// What the planner does between cuts: the order it takes the regions in,
/// which regions are worth an entry at all, and when it keeps the tool down.
pub struct Adaptive3dLinking {
    /// Region ordering strategy (default: Global for backward compat).
    pub region_ordering: RegionOrdering,
    /// F-038: minimum total horizontal cutting length (mm) a marching-squares
    /// region must produce in its 2D adaptive sub-pass before the planner
    /// commits an entry plunge to it. GeodesicAdaptive strategy only. Regions
    /// whose forecast cut length (perimeter-sweep + 2D adaptive walk) is
    /// below this threshold are dropped at plan time, eliminating the
    /// "perimeter micro-plunge" fragmentation the Wanaka Back Rough .nc
    /// exhibited (149 plunges, 90 of which cut <= 10 mm). Default 5.0 mm.
    pub min_region_cut_length_mm: f64,
    /// F-038b: maximum XY distance to attempt a keep-tool-down link
    /// between cut groups instead of retract-rapid-plunge. `None` means
    /// "use 8 x tool diameter computed at toolpath build time" — matches
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
    /// hair of safety margin so the rapid step over a low peak does not
    /// scrape the surface. Default 0.5 mm.
    pub stay_down_clearance_mm: f64,
    /// Integration step 7.2 (research §16 Part A, D43): how a draped cut is
    /// split at a large Z step ([`StepSplit`]). `None` is the strategy's
    /// default: [`StepSplit::PathShape`], today's rule, for every shipped
    /// strategy. No config sets it yet; the geodesic arm (step 7.4) is its
    /// first user.
    pub step_split: Option<StepSplit>,
    /// Integration step 7.2 (research §16 Part B): the routed keep-down
    /// travel at an entry ([`KeepDownRoute`]). `None` is the strategy's
    /// default: [`KeepDownRoute::Off`] for every shipped strategy.
    pub keep_down_route: Option<KeepDownRoute>,
    /// C4 (`Adaptive3dConfig::contour_each_level`): the contour on each
    /// level along the machining boundary where stock in reach stands
    /// (G-RESTDOC). `None`: off, and the plan is today's.
    pub level_contour: Option<LevelContour>,
    /// The rest region filter (`Adaptive3dConfig::rest_max_region_area_mm2`,
    /// `rest_max_region_width_mm`). `None`: every region is cut. The
    /// session sets it only on remaining stock (a rest rough).
    pub rest_region_filter: Option<RestRegionFilter>,
}

/// The rest region filter ([`Adaptive3dLinking::rest_region_filter`]).
///
/// At each Z level, a region of material that the level would clear is cut
/// only when it passes every bound that is set. A region that fails is
/// skipped at that level, not clipped. This is the opposite of the
/// micro-region filter (`clearing.rs`, it drops regions that are too
/// small): this filter drops regions that are too large, so a small tool
/// cuts only the parts that a larger tool could not reach.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RestRegionFilter {
    /// The largest area (mm²) of a region that is cut. `None`: no bound.
    pub max_area_mm2: Option<f64>,
    /// The largest width (mm) of a region that is cut: the diameter of the
    /// largest circle inside the region. `None`: no bound.
    pub max_width_mm: Option<f64>,
}

/// C4: the settings of the contour on each level
/// ([`Adaptive3dLinking::level_contour`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelContour {
    /// The machine's link timing, for the one feed-or-retract rule
    /// (`machine::kinematics::retract_saves_time`) that joins two runs on
    /// one ring. `None`: the runs are not joined.
    pub link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
}

/// Where a draped cut is split at a step between two lifted points
/// (research §16 Part A, D43; INTEGRATION_PLAN §8: kept, Q6 reversed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StepSplit {
    /// Today's rule: a step of `|dz| > max(1.1·DPP, 1 mm)` or a slope over
    /// 0.3 (`clearing.rs`, `PLUNGE_SLOPE_LIMIT`).
    #[default]
    PathShape,
    /// Of the steps the path rule flags, split only where the tool's swept
    /// disc along the step meets stock deeper than one Depth/Pass below its
    /// top (plus the stay-down clearance), or where the step's chip-area
    /// load ([`crate::tool_load::load_fraction::chip_load_fraction`], the
    /// one load) passes the pass ceiling. A step through air, or onto a
    /// face or down a slope that holds a light layer, is cut at feed.
    StockDepth,
}

/// The routed keep-down travel at an entry (research §16 Part B,
/// `keep_down.rs`).
///
/// The research's third value, a route at the rapid rate (`LowRapid`), is
/// not ported: a rapid at depth over cut ground is not allowed yet
/// (INTEGRATION_PLAN Q7, lead default 2026-10-27).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeepDownRoute {
    /// No routed travel: the entry is a straight stay-down link or a
    /// retract (today's rule).
    #[default]
    Off,
    /// A route over cut ground at the level, at the cut feed, held by the
    /// full-disc proof at the stay-down clearance.
    CutFeed,
}

/// The step split a run uses: the parameter, else the strategy's default:
/// [`StepSplit::StockDepth`] for Geodesic Adaptive (INTEGRATION_PLAN §8,
/// the D43 load split), [`StepSplit::PathShape`] for every other strategy.
fn effective_step_split(params: &Adaptive3dParams) -> StepSplit {
    match params.clearing_strategy {
        ClearingStrategy3d::GeodesicAdaptive => {
            params.linking.step_split.unwrap_or(StepSplit::StockDepth)
        }
        ClearingStrategy3d::ContourParallel => params.linking.step_split.unwrap_or_default(),
    }
}

/// The routed keep-down travel a run uses: the parameter, else the
/// strategy's default: [`KeepDownRoute::CutFeed`] for Geodesic Adaptive
/// (research §16 Part B), [`KeepDownRoute::Off`] for every other strategy.
fn effective_keep_down_route(params: &Adaptive3dParams) -> KeepDownRoute {
    match params.clearing_strategy {
        ClearingStrategy3d::GeodesicAdaptive => params
            .linking
            .keep_down_route
            .unwrap_or(KeepDownRoute::CutFeed),
        ClearingStrategy3d::ContourParallel => params.linking.keep_down_route.unwrap_or_default(),
    }
}

/// Parameters for 3D adaptive clearing.
///
/// CUT-04 grouped the thirty flat fields this struct used to carry into
/// [`Adaptive3dGeometry`], [`Adaptive3dDepth`] and [`Adaptive3dLinking`].
/// What stays at the top level is what belongs to no one group: the feeds,
/// the entry and clearing strategies, and the starting material.
pub struct Adaptive3dParams {
    /// The cutter and the XY frame it works in.
    pub geometry: Adaptive3dGeometry,
    /// The Z plan.
    pub depth: Adaptive3dDepth,
    /// What the planner does between cuts.
    pub linking: Adaptive3dLinking,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    /// Feed (mm/min) of a helix or ramp entry through material. `None`
    /// uses `plunge_rate`. A peck plunge keeps `plunge_rate`.
    pub ramp_feed_rate: Option<f64>,
    /// Height (mm) above the material top where a helix or ramp entry
    /// starts. The rapid floor stays 0.5 mm over the planner read.
    pub entry_clearance_mm: f64,
    pub safe_z: f64,
    /// Entry strategy (default: Plunge for backward compat).
    pub entry_style: EntryStyle3d,
    /// Clearing strategy per Z level (default: ContourParallel).
    pub clearing_strategy: ClearingStrategy3d,
    /// Load-only: no strategy reads it. The removed ContourSpiral strategy
    /// read it as its trochoid trigger cap (removal step R4).
    pub trochoid_cap_mult: f64,
    /// Load-only: no strategy reads it. The removed AgentSearch strategy
    /// read it as its 2D engagement quantity (removal step R4).
    pub engagement_measure: crate::adaptive::EngagementMeasure,
    /// Pre-machined stock for two-sided machining.
    /// When Some, used as starting material instead of a fresh block at
    /// `Adaptive3dDepth::stock_top_z`.
    pub initial_stock: Option<TriDexelStock>,
}

impl Adaptive3dParams {
    /// The chord tolerance (mm) of the RDP that simplifies each emitted cut.
    /// The emitter, the waterline cleanup and the planner stamp all read
    /// this one value (G-PLANSIMGAP). The rule is ONE function,
    /// [`Adaptive3dGeometry::chord_tolerance`], through
    /// [`Adaptive3dGeometry::cut_simplify_tolerance`]; every shipped arm
    /// uses it unchanged.
    ///
    /// Geodesic Adaptive (research C26, G-RDPLOAD): the same value, capped at the load
    /// margin of a straight pass (`adaptive::geodesic::slice_load_margin`,
    /// `c·D - s`). The 2D load rule judged the walked wraps; a chord whose
    /// sagitta is over that margin makes the next pass meet more than
    /// `c·D` at mid-chord. The cap only lowers the tolerance, so the model
    /// gouge guard, which reads the uncapped rule (G-ROUGHGOUGE), stays on
    /// the safe side.
    #[must_use]
    pub fn emission_chord_tolerance(&self) -> f64 {
        let chord = self.geometry.cut_simplify_tolerance();
        if self.clearing_strategy == ClearingStrategy3d::GeodesicAdaptive {
            return chord.min(crate::adaptive::geodesic::slice_load_margin(
                self.geometry.stepover,
                self.geometry.tool_radius,
                self.geometry.tolerance,
            ));
        }
        chord
    }
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
    /// `min_region_cut_length_mm` and were dropped from the slice
    /// emission. Parallel to `dropped_micro_region_count` (which is the
    /// Fusion-style area filter) but gated on length, not area.
    pub dropped_short_region_count: usize,
}

/// P22a-2: the planner rule that admitted a capped cut over the pass
/// ceiling. [`Adaptive3dRuntimeEvent::FeedCapStart`] carries it, and the
/// semantic trace names it on a `FeedCap` item over the capped moves
/// (`compute::annotate`), so a reader maps a simulated sample to its rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeedCapSource {
    /// A planned clearing turn of the geodesic plan (`LinkLoad::admits_turn`).
    PlannedTurn,
    /// D65 in the 2D replay: a step over the ceiling that reads at most a
    /// slot (`LinkLoad::admits_capped`).
    Capped2d,
    /// A plan window out of a modelled straight plunge (`PassLoad::departing`).
    DepartureWindow,
    /// D65 in the 3D lift: a draped step that the stock-read split admits at
    /// a capped feed (`clearing::admits_capped_split`).
    CappedSplit3d,
    /// C4 "Contour each level" (`linking.level_contour`, off by default): a
    /// contour run whose load passes the ceiling (`level_contour_pass`).
    LevelContour,
    /// P22f-2a: a step that the shape rule does not flag (flat, or shallow),
    /// read on the 3D planner stock by the one load (`clearing::step_load`)
    /// over the ceiling and at most a slot, so cut at a capped feed. The 3D
    /// stock read of every fed step of a geodesic cut.
    StockRead3d,
}

impl FeedCapSource {
    /// Every variant, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::PlannedTurn,
        Self::Capped2d,
        Self::DepartureWindow,
        Self::CappedSplit3d,
        Self::LevelContour,
        Self::StockRead3d,
    ];

    /// The stable snake_case key the semantic trace records.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::PlannedTurn => "planned_turn",
            Self::Capped2d => "capped_2d",
            Self::DepartureWindow => "departure_window",
            Self::CappedSplit3d => "capped_split_3d",
            Self::LevelContour => "level_contour",
            Self::StockRead3d => "stock_read_3d",
        }
    }

    /// The variant whose [`Self::key`] is `key`.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    /// The source of a capped run of the 2D slice plan.
    pub(crate) const fn of_slice(kind: crate::adaptive::geodesic::SliceCapKind) -> Self {
        match kind {
            crate::adaptive::geodesic::SliceCapKind::Turn => Self::PlannedTurn,
            crate::adaptive::geodesic::SliceCapKind::Capped => Self::Capped2d,
            crate::adaptive::geodesic::SliceCapKind::Departure => Self::DepartureWindow,
        }
    }
}

/// P22f-1: why a Geodesic Adaptive region at one level opens on its grown
/// outline or keeps the shipped perimeter sweep (`clearing::grown_outlines`,
/// research C31). The first case that holds, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningDecision {
    /// The smoothing or the grow gives no outline.
    NoOutline,
    /// The shortfall inset split the outline into more polygons.
    InsetSplit,
    /// The measured margin after the shortfall inset is under one planner
    /// cell.
    MeasuredMarginShort,
    /// The closing of the grow exceeds one planner cell times the boundary
    /// length.
    ClosingGuard,
    /// The region opens on its grown outline (no sweep, unless a clip
    /// leaves nothing).
    Grown,
}

impl OpeningDecision {
    /// The text of the "C31 region opening" log line.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::NoOutline => "no smoothed or grown outline: shipped sweep",
            Self::InsetSplit => "the shortfall inset split the outline: shipped sweep",
            Self::MeasuredMarginShort => "measured margin under one planner cell: shipped sweep",
            Self::ClosingGuard => "closing guard: shipped sweep",
            Self::Grown => "grown, no sweep",
        }
    }
}

/// P22f-1: what one clip of the grown outline did (research C38 item B, the
/// stock box; §13.7, the centre reach).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningClip {
    /// The grow refused before the clip, or an earlier clip left nothing.
    NotRun,
    /// The outline is unchanged (wholly inside, or no clip applies).
    Kept,
    /// The clip cut the outline; something is left.
    Clipped,
    /// The clip left nothing: the region keeps the shipped sweep.
    Empty,
}

/// P22f-1: the opening decision of one Geodesic Adaptive region at one level,
/// with the numbers the "C31 region opening" log line prints (ruling 5).
/// `Produced::region_openings` holds one per region and level, in plan
/// order, so a reader needs no tracing subscriber.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionOpening {
    /// The level Z (emission frame).
    pub z_level: f64,
    /// The region of the level, 1-based.
    pub region: usize,
    /// The planner cell (mm).
    pub cell_mm: f64,
    /// The least distance between the smoothed and the raw boundary, plus
    /// `m`, before the shortfall inset (mm).
    pub measured_before_mm: f64,
    /// The shortfall inset (mm; 0 when none was needed).
    pub shortfall_inset_mm: f64,
    /// The measured margin after the shortfall inset (mm).
    pub measured_after_mm: f64,
    /// The closing guard: the excess area and its allowance (mm2).
    pub excess_mm2: f64,
    pub allowed_mm2: f64,
    pub decision: OpeningDecision,
    pub stock_box_clip: OpeningClip,
    pub centre_reach_clip: OpeningClip,
}

impl RegionOpening {
    /// Whether the region keeps the shipped perimeter sweep: the grow
    /// refused, or a clip left nothing.
    #[must_use]
    pub fn cuts_sweep(&self) -> bool {
        self.decision != OpeningDecision::Grown
            || self.stock_box_clip == OpeningClip::Empty
            || self.centre_reach_clip == OpeningClip::Empty
    }
}

/// P22f-2a probe (`path::debug_level_plan_probe`, test-support only): one
/// cell that the 2D plan of a level holds cut, with the level's floor there
/// and the 3D planner stock top after the level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanCutPoint {
    pub x: f64,
    pub y: f64,
    /// The level's floor at the point: `max(surface + leave, z_level)`
    /// (`clearing::lift_to_level`).
    pub floor_z: f64,
    /// The 3D planner stock top at the point after the level (the highest
    /// column top within half a stock-cell diagonal), or `None` with no
    /// material there.
    pub top_after: Option<f64>,
}

/// P22f-2a probe: the cells the 2D plans of one level hold cut, read
/// against the 3D planner stock after the level. Test-only; no product
/// reader.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelPlanProbe {
    pub z_level: f64,
    /// The 3D planner stock cell (mm).
    pub stock_cell_mm: f64,
    pub points: Vec<PlanCutPoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Adaptive3dRuntimeEvent {
    RegionStart {
        /// The By Area region order, 1-based. It is the `order` of the
        /// region in [`AreaRegionMap`] and the `region_id` of its span.
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
    /// Geodesic Adaptive (integration step 7.4, design part F): the cuts up
    /// to the next [`Self::FeedCapEnd`] lie on one capped run (a planned
    /// turn, a D65 window or split step, a C4 contour run, or a run of
    /// departure windows) and run at `factor` of the feed. The emitter
    /// writes the lowered feed and `Move::feed_cap` on the cut moves between
    /// them, and keeps the pair as annotations (P22a-2): `compute::annotate`
    /// binds a `FeedCap` item over the capped moves that names `source`.
    FeedCapStart {
        factor: f64,
        source: FeedCapSource,
    },
    /// The end of a [`Self::FeedCapStart`] run.
    FeedCapEnd,
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
            | Self::PassSummary { .. }
            | Self::FeedCapStart { .. }
            | Self::FeedCapEnd => {}
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
            Self::FeedCapStart { factor, source } => {
                format!("Feed cap {factor:.3} of the feed ({})", source.key())
            }
            Self::FeedCapEnd => "Feed cap end".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adaptive3dRuntimeAnnotation {
    pub move_index: usize,
    pub event: Adaptive3dRuntimeEvent,
}

/// **Test door.** The `#[cfg(test)]` module of this file is the only
/// caller: [`adaptive_3d_toolpath_output_with_cancel`] under
/// [`crate::interrupt::NeverCancel`], the toolpath only.
#[cfg(test)]
pub(crate) fn adaptive_3d_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> Toolpath {
    crate::interrupt::run_uncancellable(|cancel| {
        adaptive_3d_toolpath_output_with_cancel(mesh, index, cutter, params, cancel, None)
            .map(|out| out.toolpath)
    })
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
    adaptive_3d_toolpath_output_with_cancel(mesh, index, cutter, params, cancel, debug)
        .map(|out| out.toolpath)
}

/// The full output of one adaptive3d run.
#[derive(Debug, Clone)]
pub struct Adaptive3dOutput {
    pub toolpath: Toolpath,
    pub annotations: Vec<Adaptive3dRuntimeAnnotation>,
    /// W6.2: what the planner produced beside the path. It fills
    /// `area_regions` (the regions that By Area detected; `None` for
    /// Global) and the rest regions (D44, D50). `planner_engagement` stays
    /// empty: ContourSpiral, its one 3D writer, left in removal step R4.
    pub produced: crate::trace::produced::Produced,
    /// P37: the stock the plan held as material and did not plan
    /// ([`LeftStock`]). `None`: the plan dropped nothing.
    pub left_stock: Option<LeftStock>,
}

/// Run the planner and return every output. The compute adapter calls
/// this form, and it is the one public door of this folder (F085).
pub fn adaptive_3d_toolpath_output_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Adaptive3dOutput, Cancelled> {
    let result = adaptive_3d_segments(mesh, index, cutter, params, debug, cancel)?;
    let segments = result.segments;
    let left_stock = result.left_stock;
    // D44, D48: the regions the run left to a rest pass, one XY union over
    // the levels grown by one planner cell, in tool-centre space inside
    // this run's own containment polygons. The runner keeps them only on
    // demand (`run_volumetric`). D50: with the top Z of the stock left in
    // them.
    let (rest_regions, rest_regions_top_z) = rest_regions::left_regions(
        &result.left_pieces,
        crate::adaptive::planner_cell_size(params.geometry.tool_radius, params.geometry.tolerance),
        &params.geometry.centre_boundary,
    )
    .map_or((None, None), |(regions, top)| {
        (Some(std::sync::Arc::new(regions)), Some(top))
    });
    // F-038b: pass mesh + spatial index + cutter so segments_to_toolpath
    // can query the heightfield along each candidate stay-down link.
    let (tp, annotations, non_centre_cutting_refusal, helix_core_fallbacks) =
        path::segments_to_toolpath_checked(&segments, params, mesh, index, cutter);
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
        "3D adaptive toolpath complete"
    );

    Ok(Adaptive3dOutput {
        toolpath: tp,
        annotations,
        produced: crate::trace::produced::Produced {
            area_regions: result.area_regions.map(std::sync::Arc::new),
            rest_regions,
            rest_regions_top_z,
            non_centre_cutting_refusal,
            helix_core_fallbacks,
            region_openings: result.region_openings,
            ..crate::trace::produced::Produced::default()
        },
        left_stock,
    })
}

#[cfg(test)]
mod tests;
