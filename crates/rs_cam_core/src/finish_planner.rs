//! P2.b of the unified finishing pass planner
//! (`planning/unified_finish_planner_design.md`): slope-band decomposition +
//! R1 region conditioning + the crease-corridor rule.
//!
//! [`decompose`] turns a [`SlopeMap`] + coverage mask + a set of rest-valley
//! [`RestCenterline`]s into [`PlannedRegions`] — closed polygons tagged with
//! a [`FinishBand`] (which cutting strategy owns them) plus any crease
//! corridors promoted to their own region. This module emits **no
//! toolpaths**; it is purely the decomposition stage that feeds the P2.c
//! per-band generation orchestrator.
//!
//! ## Why conditioning exists (R1 — the steep_shallow ghost)
//!
//! A naive threshold classification (`angle >= threshold_deg`, exactly what
//! [`crate::steep_shallow::classify_steep_shallow`] does today) shreds into
//! dozens to hundreds of tiny islands wherever the surface slope oscillates
//! around the threshold — sampling noise, a slightly wavy surface, or (as
//! the unit tests below demonstrate) a genuinely striped slope pattern. The
//! risk register in the design doc calls this "the steep_shallow ghost" and
//! names it the planner's top risk. [`decompose`] runs three conditioning
//! passes before any polygon is extracted:
//!
//! 1. **Hysteresis** (`hysteresis_deg`): a cell only *enters* a band once
//!    the slope crosses the upper `enter` threshold, but only *leaves* once
//!    it drops below the lower `enter - hysteresis_deg` threshold. This is
//!    implemented as a flood fill seeded from `angle >= enter` cells,
//!    constrained to the wider `angle >= leave` mask — see
//!    [`hysteresis_mask`].
//! 2. **Morphological close** (`close_radius_mm`): dilate-then-erode via two
//!    whole-grid Euclidean distance transforms ([`morphological_close`]),
//!    bridging small gaps without the O(cells × radius) per-ring dilation
//!    loop `steep_shallow::dilate_grid` uses.
//! 3. **Min-area absorption** (`min_region_area_mm2`): connected band
//!    islands smaller than a few tool-diameters² are reassigned to the
//!    majority band among their outside neighbours — see
//!    [`absorb_small_regions`]. A genuinely isolated tiny island (no banded
//!    neighbour at all) is left alone rather than force-merged.
//!
//! Acceptance target (design doc, R1): region count on a real mesh should be
//! O(10), not O(100) — see `finish_planner_wanaka_decompose.rs`.
//!
//! ## Crease corridors (v3 claims semantics)
//!
//! Rest-valley centerlines ([`RestCenterline`], from
//! [`crate::rest_field::detect_rest_valleys`]) implement the design doc's
//! "pencil claims creases first" rule (`planning/unified_v3_design.md`
//! §2.1): **every** crease claims a corridor and it is subtracted from the
//! band label grid before polygon extraction, not just canyon-width ones.
//! The claimed half-width is `max(half_width_mm, pencil_claim_floor)` —
//! the detector's measured width floored by
//! [`FinishPlannerParams::pencil_claim_floor`], so a claim is never
//! thinner than what the tip-tool pencil pass will actually cut. The
//! claimed polygon is recorded on every routed crease as
//! [`PlannedCrease::corridor`] (`None` only when the centerline is too
//! degenerate to rasterize — fewer than two in-grid points, or a mask that
//! extracts to nothing).
//!
//! `corridor` answers "what did this crease claim"; a separate, narrower
//! question — "does this crease become its own clearing zone instead of
//! staying folded into the pencil pass" — is still gated by the original
//! canyon rule: a crease at/above
//! [`FinishPlannerParams::crease_own_region_half_width_mm`] (CUSP-scaled —
//! see that field for which tool scale and why) is promoted to its own
//! [`PlannedRegion`]-shaped output via
//! [`PlannedCrease::own_region`]. Since `pencil_claim_floor` is normally far
//! below the canyon threshold, a canyon's `own_region` and `corridor`
//! polygons coincide; a hairline crease gets `corridor: Some(..)`,
//! `own_region: None` — it still claims territory out of the band grid, it
//! just doesn't become a standalone region.
//!
//! ## What this module does *not* do
//!
//! No toolpath emission, no strategy assignment beyond the band tag itself,
//! no routing between regions. Those are P2.c/P2.d.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Write as _;

use tracing::warn;

use crate::finish_setup::FinishSurface;
use crate::geo::P2;
use crate::grid_field::distance_transform_2d;
use crate::grid2::Grid2;
use crate::measurement::{CellSource, MeasurementDomain, MeasurementProvenance, MeasurementStage};
use crate::polygon::Polygon2;
use crate::region_mask::region_polygons_from_mask;
use crate::rest_field::RestCenterline;
use crate::slope::SlopeMap;

// ── Types ────────────────────────────────────────────────────────────────

/// Which cutting strategy a planned region is assigned to (design doc:
/// "Three bands, two thresholds").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinishBand {
    /// Slope below `steep_threshold_deg` — parallel raster passes.
    Shallow,
    /// Between the two thresholds — scallop-continuous rings.
    MidSteep,
    /// At/above `waterline_threshold_deg` — waterline Z-level contours.
    VerySteep,
}

/// Decomposition dials (design doc "Decisions" + risk R4). All new dials the
/// unified op introduces live here; everything else inherits from the
/// per-strategy params (one-new-dial rule).
#[derive(Debug, Clone)]
pub struct FinishPlannerParams {
    /// Slope entering steep (deg from horizontal). Primary dial. Default 45.
    pub steep_threshold_deg: f64,
    /// Slope entering very-steep/waterline (deg). Advanced dial. Default 65.
    pub waterline_threshold_deg: f64,
    /// Hysteresis width (deg): a cell LEAVES a band only below
    /// `enter - hysteresis_deg`. Default 10.
    pub hysteresis_deg: f64,
    /// Band polygons are dilated by this much at extraction so neighbouring
    /// bands overlap (like steep_shallow's `overlap_distance`). Caller
    /// derives this from stepover in P2.c. Default 0.0 (no overlap).
    pub overlap_mm: f64,
    /// Crease-own-region threshold (mm): a crease becomes its own
    /// [`PlannedRegion`] once its MEASURED `half_width_mm` reaches this.
    /// Below it the crease still claims a corridor, it just stays folded
    /// into the pencil pass. `for_tool` derives it as
    /// [`CREASE_OWN_REGION_K`] × the tool's CUSP radius.
    ///
    /// # Which tool scale, and why (PR-6b, H2.4)
    ///
    /// This used to be a unitless `corridor_k` multiplied by a `tool_radius`
    /// scalar passed positionally into [`decompose`], and the only production
    /// caller — `unified_finish` — passed `cutter.radius()`, the ENVELOPE.
    /// On the shipped Ø1-tip / 7° / Ø6-shank taper that put the bar at 6.0 mm
    /// while every other dial in this struct was sized off the 0.5 mm cusp:
    /// one struct, two tool scales, and a canyon rule no valley a Ø1 tip
    /// works could ever clear.
    ///
    /// The answer is CUSP scale, and the reason is that this is a
    /// **planner-territory** question, not a fit question. "Is this valley
    /// wide enough to plan as a zone in its own right" is asked in the same
    /// vocabulary as `pencil_claim_floor`, `close_radius_mm` and
    /// `min_region_area_mm2` — feature scales, all cusp-derived. The FIT
    /// question ("can the cutter stand off the centreline, and how far") was
    /// already answered upstream by [`crate::reach`]: a crease only reaches
    /// this function at all when the detector's coverage criterion routed it
    /// to Pencil, i.e. when the fan the operation can emit already covers its
    /// reachable band. Re-deriving reach here would be a second routing
    /// decision on a struct that has no cross-section to derive it from —
    /// [`decompose`] sees a [`SlopeMap`], not a rest field.
    pub crease_own_region_half_width_mm: f64,
    /// Claim floor (mm): every crease's claimed corridor half-width is
    /// `max(half_width_mm, pencil_claim_floor)`, so the claim can never be
    /// thinner than the tip-tool pencil pass that will actually clean the
    /// corridor. `for_tool` defaults this to `tool_radius * 0.25` (a
    /// placeholder sized off the finishing tool, since this module has no
    /// tool-catalog plumbing) — callers that know the real pencil tip
    /// radius should pass it directly.
    pub pencil_claim_floor: f64,
    /// Morphological close radius (mm) applied to band masks. Default
    /// `tool_radius / 2` via [`FinishPlannerParams::for_tool`].
    pub close_radius_mm: f64,
    /// Connected band islands smaller than this are absorbed into their
    /// surrounding band. Default `4 * (2 * tool_radius)^2` via
    /// [`FinishPlannerParams::for_tool`].
    pub min_region_area_mm2: f64,
}

/// Cusp radii of valley half-width at which a crease is planned as its own
/// region rather than folded into the pencil pass — the multiplier
/// [`FinishPlannerParams::for_tool`] applies. 2.0 = "wider than the tool's
/// own cutting footprint", the value the rule shipped with (as the retired
/// `corridor_k`).
pub const CREASE_OWN_REGION_K: f64 = 2.0;

impl FinishPlannerParams {
    /// Dials derived from a tool radius, per the design doc's default
    /// formulas: `close_radius_mm = tool_radius / 2`,
    /// `min_region_area_mm2 = (2 * tool_radius)^2 * 4` (roughly four
    /// tool-diameters² — "a few tool diameters²" from the R1 risk register).
    ///
    /// Thresholds locked by the P2.e sweep (2026-07-08, wanaka chain +
    /// conditioning tables in `planning/unified_finishing_pass_plan.md`):
    /// `steep 45` is the quality-neutral anchor (lower is faster — −14%
    /// finish at 35 — but scallop's held cusp is coarser than raster's
    /// effective cusp in the 35–45° band, so speed-hunting via this dial
    /// costs quality there); `waterline 75` because Z-contouring is the
    /// most expensive strategy per area — 65→75 measured −11.7% finish
    /// with collisions unchanged and cusp still bounded by scallop height
    /// on the 65–75° slopes, while 65→55 EXPLODED +27%. Every ±1-step
    /// neighbour of every dial stays conditioned (no cliffs); hysteresis
    /// is load-bearing (0 → the raw masks storm to O(100) islands).
    /// `cusp_radius` MUST be the tool's cusp-forming (tip) radius —
    /// [`crate::tool::MillingCutter::cusp_radius`] — not `radius()`. Every
    /// dial below is a FEATURE SCALE, and on a tapered tool `radius()`
    /// reports the shank: a Ø1 tip on a 6 mm shank made
    /// `min_region_area_mm2` 144 mm² instead of 4, which closed and
    /// absorbed every steep ribbon on terrain that is 25% steeper than 55°
    /// (design doc §14q).
    pub fn for_tool(cusp_radius: f64) -> Self {
        Self {
            steep_threshold_deg: 45.0,
            waterline_threshold_deg: 75.0,
            hysteresis_deg: 10.0,
            overlap_mm: 0.0,
            crease_own_region_half_width_mm: CREASE_OWN_REGION_K * cusp_radius,
            pencil_claim_floor: cusp_radius * 0.25,
            close_radius_mm: cusp_radius * 0.5,
            min_region_area_mm2: (2.0 * cusp_radius).powi(2) * 4.0,
        }
    }
}

impl Default for FinishPlannerParams {
    /// Ø6 ball nose — the common finishing tool — via
    /// [`FinishPlannerParams::for_tool`].
    fn default() -> Self {
        Self::for_tool(3.0)
    }
}

/// One conditioned region with its assigned band.
#[derive(Debug, Clone)]
pub struct PlannedRegion {
    pub band: FinishBand,
    pub polygon: Polygon2,
}

impl PlannedRegion {
    /// This region's area, tagged with its domain: **XY-projected**, never a
    /// 3D surface area (M1, `MEASUREMENT_DOMAINS.md` X-1). Holes are
    /// subtracted; when the decomposition ran with `overlap_mm > 0` the
    /// polygons of neighbouring bands OVERLAP and these areas must not be
    /// summed across bands — [`DecomposeStats::provenance`] records both
    /// facts.
    ///
    /// Additive accessor: `polygon.area()` still exists and still returns a
    /// bare `f64`; this is the call new code should reach for, because
    /// [`crate::measurement::ProjectedXyAreaMm2`] cannot be divided by a
    /// [`crate::measurement::SurfaceAreaMm2`].
    #[must_use]
    pub fn projected_xy_area_mm2(&self) -> crate::measurement::ProjectedXyAreaMm2 {
        crate::measurement::ProjectedXyAreaMm2::new(self.polygon.area())
    }
}

/// A crease routed through the claims pipeline (design doc §2.1: "pencil
/// claims creases first").
#[derive(Debug, Clone)]
pub struct PlannedCrease {
    /// The detector centerline (world points + measured half-width).
    pub centerline: RestCenterline,
    /// `Some` — this crease is a canyon (`half_width_mm >=
    /// `[`FinishPlannerParams::crease_own_region_half_width_mm`]) promoted
    /// to its own clearing region, routed like a
    /// band rather than folded into the pencil pass. `None` for every
    /// narrower crease — it still claims a corridor (see [`Self::corridor`]),
    /// it just isn't its own zone.
    pub own_region: Option<Polygon2>,
    /// What this crease claimed: the corridor polygon carved out of the
    /// band label grid, buffered by `max(half_width_mm,
    /// pencil_claim_floor)`. `None` only when the centerline was too
    /// degenerate to rasterize (fewer than two in-grid points, or the mask
    /// extracted to no polygon) — in that case nothing was claimed either.
    pub corridor: Option<Polygon2>,
}

/// Conditioning telemetry: proves R1 conditioning actually did work.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecomposeStats {
    /// Raw 8-connected steep islands BEFORE conditioning (hysteresis mask only).
    pub raw_steep_islands: usize,
    /// Raw very-steep islands before conditioning.
    pub raw_very_steep_islands: usize,
    /// Band islands merged away by min-area absorption.
    pub absorbed_regions: usize,
    /// Final planned region count: band regions + crease own-regions.
    pub region_count: usize,
    /// Creases that successfully claimed a corridor (`corridor.is_some()`).
    pub claimed_creases: usize,
    /// Label-grid cells that were `Some(band)` and got set to `None` by a
    /// crease claim (a cell claimed by two overlapping creases counts once,
    /// against whichever crease claimed it first — later claims over
    /// already-`None` cells aren't recounted).
    pub claimed_cells: usize,
    /// What the region areas in this decomposition MEAN (M1): XY-projected,
    /// captured at polygon extraction, on the grid `decompose` was handed,
    /// dilated by `params.overlap_mm`, on a coverage mask eroded by one cell.
    ///
    /// [`crate::measurement::CellSource`] is `Explicit` when `decompose` was
    /// called directly — that entry point sees a [`SlopeMap`] and cannot know
    /// which tool scale sized it. [`decompose_surface`] overwrites it with the
    /// surface's own source, so the classification/generation distinction
    /// (§14q, X-6) survives into the report.
    pub provenance: crate::measurement::MeasurementProvenance,
}

/// Output of [`decompose`].
#[derive(Debug, Clone)]
pub struct PlannedRegions {
    /// Band regions, ordered Shallow, MidSteep, VerySteep; within a band,
    /// largest area first (the extractor's order).
    pub regions: Vec<PlannedRegion>,
    pub creases: Vec<PlannedCrease>,
    pub stats: DecomposeStats,
}

/// Iteration order used everywhere a definite band ordering matters:
/// extraction order, the SVG legend, and tie-breaking in min-area
/// absorption.
const BAND_ORDER: [FinishBand; 3] = [
    FinishBand::Shallow,
    FinishBand::MidSteep,
    FinishBand::VerySteep,
];

fn band_rank(band: FinishBand) -> usize {
    match band {
        FinishBand::Shallow => 0,
        FinishBand::MidSteep => 1,
        FinishBand::VerySteep => 2,
    }
}

fn band_from_rank(rank: usize) -> FinishBand {
    match rank {
        0 => FinishBand::Shallow,
        1 => FinishBand::MidSteep,
        _ => FinishBand::VerySteep,
    }
}

// ── Entry points ─────────────────────────────────────────────────────────

/// Decompose a slope map + coverage mask + crease set into planned regions.
///
/// `covered` must be row-major `rows * cols` (the same convention
/// `SurfaceHeightmap::covered` and `SlopeMap` use). A mismatched length or
/// an empty grid is a caller bug, not a panic: this logs a `tracing::warn!`
/// and returns an empty [`PlannedRegions`].
pub fn decompose(
    slope_map: &SlopeMap,
    covered: &[bool],
    creases: &[RestCenterline],
    params: &FinishPlannerParams,
) -> PlannedRegions {
    let rows = slope_map.rows;
    let cols = slope_map.cols;
    let total = rows * cols;

    if total == 0 || covered.len() != total {
        warn!(
            rows,
            cols,
            covered_len = covered.len(),
            "finish_planner::decompose: empty grid or covered-length mismatch; returning no planned regions"
        );
        return PlannedRegions {
            regions: Vec::new(),
            creases: Vec::new(),
            stats: DecomposeStats::default(),
        };
    }

    let cell = slope_map.cell_size;
    let origin_x = slope_map.origin_x;
    let origin_y = slope_map.origin_y;

    // ── Step 0: stencil-safe coverage ───────────────────────────────────
    // A covered cell with an uncovered 8-neighbour has an unreliable slope:
    // `SlopeMap::from_z_grid`'s finite differences read the neighbour's
    // clamped/extrapolated Z and fabricate a cliff (heightmaps clamp
    // uncovered cells to min_z). Erode coverage by one cell so every
    // classified cell has a fully-covered stencil. This also guarantees no
    // classified cell touches the grid edge (marching squares drops
    // boundary-touching loops).
    let covered: Vec<bool> = (0..total)
        .map(|i| {
            if !covered.get(i).copied().unwrap_or(false) {
                return false;
            }
            let r = i / cols;
            let c = i % cols;
            if r == 0 || c == 0 || r + 1 >= rows || c + 1 >= cols {
                return false;
            }
            let mut interior = true;
            for_each_neighbor(i, rows, cols, |ni| {
                if !covered.get(ni).copied().unwrap_or(false) {
                    interior = false;
                }
            });
            interior
        })
        .collect();
    let covered = covered.as_slice();

    // ── Step 1: hysteresis classification ──────────────────────────────
    let mut steep = hysteresis_mask(
        &slope_map.angles,
        covered,
        rows,
        cols,
        params.steep_threshold_deg,
        params.steep_threshold_deg - params.hysteresis_deg,
    );
    let mut very = hysteresis_mask(
        &slope_map.angles,
        covered,
        rows,
        cols,
        params.waterline_threshold_deg,
        params.waterline_threshold_deg - params.hysteresis_deg,
    );
    // Very-steep must stay a subset of steep even under pathological dials
    // (e.g. waterline_threshold_deg < steep_threshold_deg).
    and_masks_in_place(&mut very, &steep);

    let raw_steep_islands = count_components(&steep, rows, cols);
    let raw_very_steep_islands = count_components(&very, rows, cols);

    // ── Step 2: morphological close ─────────────────────────────────────
    if params.close_radius_mm > 0.0 {
        let radius_cells = params.close_radius_mm / cell;
        steep = morphological_close(&steep, rows, cols, radius_cells);
        and_masks_in_place(&mut steep, covered);
        very = morphological_close(&very, rows, cols, radius_cells);
        and_masks_in_place(&mut very, covered);
    }
    and_masks_in_place(&mut very, &steep);

    // ── Step 3: three-way label grid ────────────────────────────────────
    let mut labels: Vec<Option<FinishBand>> = covered
        .iter()
        .zip(steep.iter())
        .zip(very.iter())
        .map(|((&cov, &st), &vy)| {
            if !cov {
                None
            } else if vy {
                Some(FinishBand::VerySteep)
            } else if st {
                Some(FinishBand::MidSteep)
            } else {
                Some(FinishBand::Shallow)
            }
        })
        .collect();

    // ── Step 4: min-area absorption (R1) ────────────────────────────────
    let absorbed_regions =
        absorb_small_regions(&mut labels, rows, cols, cell, params.min_region_area_mm2);

    // ── Step 5: crease claims ────────────────────────────────────────────
    let mut claimed_creases = 0usize;
    let mut claimed_cells = 0usize;
    let planned_creases: Vec<PlannedCrease> = creases
        .iter()
        .map(|c| {
            let (crease, cells) = apply_crease_corridor(c, &mut labels, slope_map, params);
            if crease.corridor.is_some() {
                claimed_creases += 1;
            }
            claimed_cells += cells;
            crease
        })
        .collect();

    // ── Step 6: polygon extraction per band ─────────────────────────────
    let mut regions = Vec::new();
    for band in BAND_ORDER {
        let mask = band_mask_grid(&labels, rows, cols, band);
        let polys =
            region_polygons_from_mask(&mask, origin_x, origin_y, cell, params.overlap_mm.max(0.0));
        regions.extend(
            polys
                .into_iter()
                .map(|polygon| PlannedRegion { band, polygon }),
        );
    }

    // ── Step 7: stats ────────────────────────────────────────────────────
    let region_count = regions.len()
        + planned_creases
            .iter()
            .filter(|c| c.own_region.is_some())
            .count();

    let stats = DecomposeStats {
        raw_steep_islands,
        raw_very_steep_islands,
        absorbed_regions,
        region_count,
        claimed_creases,
        claimed_cells,
        provenance: decompose_provenance(cell, CellSource::Explicit, params),
    };
    // M1 §4.3: the diagnostic line that carries region counts also carries
    // what those regions' areas MEAN, so a log a reader finds later cannot be
    // mistaken for a measurement on a different grid or stage.
    tracing::info!(
        region_count = stats.region_count,
        raw_steep_islands = stats.raw_steep_islands,
        raw_very_steep_islands = stats.raw_very_steep_islands,
        absorbed_regions = stats.absorbed_regions,
        claimed_creases = stats.claimed_creases,
        claimed_cells = stats.claimed_cells,
        measurement = %stats.provenance,
        "finish_planner::decompose complete"
    );

    PlannedRegions {
        regions,
        creases: planned_creases,
        stats,
    }
}

/// The measurement contract every [`decompose`] area obeys (M1).
///
/// One place, so the entry points cannot disagree about what they produced.
pub(crate) fn decompose_provenance(
    cell_mm: f64,
    cell_source: CellSource,
    params: &FinishPlannerParams,
) -> MeasurementProvenance {
    MeasurementProvenance::new(
        MeasurementDomain::ProjectedXyArea,
        MeasurementStage::PolygonExtraction,
    )
    .with_cell(cell_mm, cell_source)
    .with_extraction_dilation_mm(params.overlap_mm)
    // Step 0 erodes coverage by one cell, unconditionally.
    .with_coverage_eroded(true)
}

/// Convenience wrapper delegating to [`decompose`] with the surface's own
/// heightmap coverage mask and slope map.
pub fn decompose_surface(
    surface: &FinishSurface,
    creases: &[RestCenterline],
    params: &FinishPlannerParams,
) -> PlannedRegions {
    let mut planned = decompose(
        &surface.slope_map,
        surface.heightmap.covered_flags(),
        creases,
        params,
    );
    // The surface knows which tool scale sized its grid; `decompose` does not.
    planned.stats.provenance.cell_source = surface.cell_source;
    planned
}

// ── Step 1: hysteresis ──────────────────────────────────────────────────

/// Union of 8-connected components of the `angle >= leave_deg` ("weak")
/// mask that contain at least one `angle >= enter_deg` ("seed") cell.
///
/// Implemented as a single multi-source BFS flood from every seed cell,
/// constrained to weak cells — O(cells), one pass.
fn hysteresis_mask(
    angles: &[f64],
    covered: &[bool],
    rows: usize,
    cols: usize,
    enter_deg: f64,
    leave_deg: f64,
) -> Vec<bool> {
    let total = rows * cols;
    if total == 0 || angles.len() != total || covered.len() != total {
        return vec![false; total];
    }
    let enter_rad = enter_deg.to_radians();
    let leave_rad = leave_deg.to_radians();

    let weak: Vec<bool> = angles
        .iter()
        .zip(covered.iter())
        .map(|(&a, &c)| c && a >= leave_rad)
        .collect();

    let mut result = vec![false; total];
    let mut visited = vec![false; total];
    let mut queue: VecDeque<usize> = VecDeque::new();

    for (i, (&a, &c)) in angles.iter().zip(covered.iter()).enumerate() {
        if c && a >= enter_rad {
            mark(&mut visited, &mut result, i);
            queue.push_back(i);
        }
    }

    while let Some(i) = queue.pop_front() {
        for_each_neighbor(i, rows, cols, |ni| {
            let is_visited = visited.get(ni).copied().unwrap_or(true);
            let is_weak = weak.get(ni).copied().unwrap_or(false);
            if !is_visited && is_weak {
                mark(&mut visited, &mut result, ni);
                queue.push_back(ni);
            }
        });
    }

    result
}

fn mark(visited: &mut [bool], result: &mut [bool], i: usize) {
    if let Some(slot) = visited.get_mut(i) {
        *slot = true;
    }
    if let Some(slot) = result.get_mut(i) {
        *slot = true;
    }
}

/// Visit the (up to 8) in-bounds 8-connected neighbours of flat index `i`.
fn for_each_neighbor(i: usize, rows: usize, cols: usize, mut visit: impl FnMut(usize)) {
    if cols == 0 {
        return;
    }
    let r = i / cols;
    let c = i % cols;
    for dr in -1i32..=1 {
        for dc in -1i32..=1 {
            if dr == 0 && dc == 0 {
                continue;
            }
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;
            if nr < 0 || nc < 0 || nr as usize >= rows || nc as usize >= cols {
                continue;
            }
            visit(nr as usize * cols + nc as usize);
        }
    }
}

fn and_masks_in_place(a: &mut [bool], b: &[bool]) {
    for (av, &bv) in a.iter_mut().zip(b.iter()) {
        *av = *av && bv;
    }
}

// ── Connected components ────────────────────────────────────────────────

/// 8-connected components of `in_mask`, each a `Vec` of flat indices, in
/// row-major discovery order.
fn label_components(rows: usize, cols: usize, in_mask: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let total = rows * cols;
    let mut visited = vec![false; total];
    let mut out = Vec::new();

    for start in 0..total {
        if !in_mask(start) || visited.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(start);
        if let Some(slot) = visited.get_mut(start) {
            *slot = true;
        }
        while let Some(i) = queue.pop_front() {
            comp.push(i);
            for_each_neighbor(i, rows, cols, |ni| {
                if in_mask(ni) && !visited.get(ni).copied().unwrap_or(true) {
                    if let Some(slot) = visited.get_mut(ni) {
                        *slot = true;
                    }
                    queue.push_back(ni);
                }
            });
        }
        out.push(comp);
    }
    out
}

/// Count of 8-connected `true` islands in a boolean mask.
fn count_components(mask: &[bool], rows: usize, cols: usize) -> usize {
    label_components(rows, cols, |i| mask.get(i).copied().unwrap_or(false)).len()
}

/// 8-connected components of same-band cells, tagged with their band. Each
/// band's components are computed independently (a component never spans
/// two bands by construction), then concatenated in [`BAND_ORDER`].
fn banded_components(
    labels: &[Option<FinishBand>],
    rows: usize,
    cols: usize,
) -> Vec<(FinishBand, Vec<usize>)> {
    let mut out = Vec::new();
    for band in BAND_ORDER {
        for comp in label_components(rows, cols, |i| {
            labels.get(i).copied().flatten() == Some(band)
        }) {
            out.push((band, comp));
        }
    }
    out
}

// ── Step 2: morphological close ─────────────────────────────────────────

/// `close(mask) = erode(dilate(mask))`, both via whole-grid Euclidean
/// distance transforms (O(cells)) rather than the per-ring dilation loop
/// `steep_shallow::dilate_grid` uses.
fn morphological_close(mask: &[bool], rows: usize, cols: usize, radius_cells: f64) -> Vec<bool> {
    if radius_cells <= 0.0 {
        return mask.to_vec();
    }
    let dilate_dist = distance_transform_2d(mask, rows, cols);
    let dilated: Vec<bool> = dilate_dist.iter().map(|&d| d <= radius_cells).collect();

    // Erosion via the standard double-EDT trick: erode(m) = NOT(dilate(NOT m)).
    let inverted: Vec<bool> = dilated.iter().map(|&v| !v).collect();
    let erode_dist = distance_transform_2d(&inverted, rows, cols);
    erode_dist.iter().map(|&d| d > radius_cells).collect()
}

// ── Step 4: min-area absorption ─────────────────────────────────────────

/// Reassign connected band islands smaller than `min_region_area_mm2` to
/// the majority band among their outside neighbours. Up to 4 passes (a
/// reassignment can shrink a neighbouring island below threshold too).
/// Returns the total number of reassigned components (an island absorbed
/// twice across passes counts twice — it is a fresh, still-too-small
/// component each time).
fn absorb_small_regions(
    labels: &mut [Option<FinishBand>],
    rows: usize,
    cols: usize,
    cell: f64,
    min_region_area_mm2: f64,
) -> usize {
    let cell_area = cell * cell;
    let mut absorbed_total = 0usize;

    for _pass in 0..4 {
        let mut components = banded_components(labels, rows, cols);
        // Deterministic processing order: smallest components first, ties
        // broken by the component's smallest cell index.
        components.sort_by(|a, b| {
            let a_min = a.1.iter().copied().min().unwrap_or(0);
            let b_min = b.1.iter().copied().min().unwrap_or(0);
            a.1.len().cmp(&b.1.len()).then_with(|| a_min.cmp(&b_min))
        });

        let mut changed = false;
        for (_band, cells) in &components {
            let area = cells.len() as f64 * cell_area;
            if area >= min_region_area_mm2 {
                continue;
            }

            let cell_set: HashSet<usize> = cells.iter().copied().collect();
            // Votes indexed by band_rank: a same-band neighbour can never
            // occur here (it would already be part of this connected
            // component), so `majority != _band` is guaranteed whenever
            // any vote exists.
            let mut votes = [0usize; 3];
            for &idx in cells {
                for_each_neighbor(idx, rows, cols, |ni| {
                    if cell_set.contains(&ni) {
                        return;
                    }
                    if let Some(nb) = labels.get(ni).copied().flatten()
                        && let Some(v) = votes.get_mut(band_rank(nb))
                    {
                        *v += 1;
                    }
                });
            }

            let total_votes: usize = votes.iter().sum();
            if total_votes == 0 {
                // Isolated island surrounded by uncovered cells: a genuine
                // tiny isolated region, not noise to merge away.
                continue;
            }
            // Deterministic tie-break: equal votes go to the lower band rank
            // (Shallow < MidSteep < VerySteep).
            let best_rank = votes
                .iter()
                .enumerate()
                .max_by_key(|&(rank, &v)| (v, std::cmp::Reverse(rank)))
                .map_or(0, |(rank, _)| rank);
            let majority = band_from_rank(best_rank);

            for &idx in cells {
                if let Some(slot) = labels.get_mut(idx) {
                    *slot = Some(majority);
                }
            }
            absorbed_total += 1;
            changed = true;
        }

        if !changed {
            break;
        }
    }

    absorbed_total
}

// ── Step 5: crease claims ────────────────────────────────────────────────

/// Apply the claims rule to one centerline (design doc §2.1): every crease
/// claims a corridor of half-width `max(half_width_mm,
/// pencil_claim_floor)`, buffered from its rasterized points, and carved
/// out of `labels` so band polygons exclude the claimed territory —
/// overlap dilation at extraction may reach back over it, which is the
/// intended overlap semantics (design doc, "Crease corridors"). Canyons
/// (`half_width_mm >= params.crease_own_region_half_width_mm`) additionally get promoted
/// to their own clearing region (`own_region`), using the same corridor
/// polygon.
///
/// Returns the routed crease plus the count of previously-labelled cells
/// this claim cleared (for [`DecomposeStats::claimed_cells`]).
fn apply_crease_corridor(
    centerline: &RestCenterline,
    labels: &mut [Option<FinishBand>],
    slope_map: &SlopeMap,
    params: &FinishPlannerParams,
) -> (PlannedCrease, usize) {
    let degenerate = || {
        (
            PlannedCrease {
                centerline: centerline.clone(),
                own_region: None,
                corridor: None,
            },
            0,
        )
    };

    let rows = slope_map.rows;
    let cols = slope_map.cols;
    let cell = slope_map.cell_size;

    let mut scratch = Grid2::new_fill(cols, rows, false);
    let mut in_grid_count = 0usize;
    for p in &centerline.points {
        if let Some((row, col)) = slope_map.world_to_cell(p.x, p.y) {
            scratch.set(row, col, true);
            in_grid_count += 1;
        }
    }

    if in_grid_count < 2 {
        warn!(
            half_width_mm = centerline.half_width_mm,
            in_grid_count,
            "finish_planner: crease centerline has fewer than 2 in-grid points; claiming nothing"
        );
        return degenerate();
    }

    let claim_half_width = centerline.half_width_mm.max(params.pencil_claim_floor);

    let polys = region_polygons_from_mask(
        &scratch,
        slope_map.origin_x,
        slope_map.origin_y,
        cell,
        claim_half_width,
    );
    let Some(corridor) = polys.into_iter().next() else {
        warn!(
            half_width_mm = centerline.half_width_mm,
            claim_half_width,
            "finish_planner: crease produced no corridor polygon; claiming nothing"
        );
        return degenerate();
    };

    // Clear the claimed corridor from the label grid so band polygons
    // exclude it.
    let dist = distance_transform_2d(scratch.as_slice(), rows, cols);
    let mut claimed_cells = 0usize;
    for (label, &d) in labels.iter_mut().zip(dist.iter()) {
        if d * cell <= claim_half_width {
            if label.is_some() {
                claimed_cells += 1;
            }
            *label = None;
        }
    }

    let is_canyon = centerline.half_width_mm >= params.crease_own_region_half_width_mm;
    let own_region = is_canyon.then(|| corridor.clone());

    (
        PlannedCrease {
            centerline: centerline.clone(),
            own_region,
            corridor: Some(corridor),
        },
        claimed_cells,
    )
}

// ── Step 6: polygon extraction ──────────────────────────────────────────

/// A `cols x rows` boolean mask (`nx = cols`, `ny = rows`, matching
/// [`region_polygons_from_mask`]'s row-major convention) of cells labelled
/// `band`.
fn band_mask_grid(
    labels: &[Option<FinishBand>],
    rows: usize,
    cols: usize,
    band: FinishBand,
) -> Grid2<bool> {
    let mut grid = Grid2::new_fill(cols, rows, false);
    for (i, label) in labels.iter().enumerate() {
        if *label == Some(band) {
            grid.set(i / cols, i % cols, true);
        }
    }
    grid
}

// ── SVG debug view ──────────────────────────────────────────────────────

const SVG_BAND_COLORS: [(FinishBand, &str); 3] = [
    (FinishBand::Shallow, "#4caf50"),
    (FinishBand::MidSteep, "#ff9800"),
    (FinishBand::VerySteep, "#f44336"),
];
const SVG_CREASE_COLOR: &str = "#ab47bc";
const SVG_NARROW_CREASE_COLOR: &str = "#42a5f5";

fn band_svg_color(band: FinishBand) -> &'static str {
    SVG_BAND_COLORS
        .iter()
        .find(|(b, _)| *b == band)
        .map_or("#ffffff", |(_, c)| *c)
}

/// Render [`PlannedRegions`] as a top-down SVG debug view: band polygons as
/// filled/stroked paths (evenodd fill rule so holes render correctly), wide
/// crease own-regions the same way, narrow crease centerlines as
/// polylines, and a small legend. Mirrors `viz::toolpath_to_svg`'s
/// conventions (header, background rect, margin, scale, Y-flip).
pub fn planned_regions_to_svg(planned: &PlannedRegions, width: f64, height: f64) -> String {
    let empty_svg = "<svg xmlns='http://www.w3.org/2000/svg'/>";

    if planned.regions.is_empty() && planned.creases.is_empty() {
        return String::from(empty_svg);
    }

    let mut minx = f64::INFINITY;
    let mut miny = f64::INFINITY;
    let mut maxx = f64::NEG_INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    let mut expand = |x: f64, y: f64| {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    };
    for region in &planned.regions {
        for p in &region.polygon.exterior {
            expand(p.x, p.y);
        }
        for hole in &region.polygon.holes {
            for p in hole {
                expand(p.x, p.y);
            }
        }
    }
    for crease in &planned.creases {
        for p in &crease.centerline.points {
            expand(p.x, p.y);
        }
        if let Some(poly) = &crease.own_region {
            for p in &poly.exterior {
                expand(p.x, p.y);
            }
        }
    }

    if !minx.is_finite() || !miny.is_finite() {
        return String::from(empty_svg);
    }

    let margin = 10.0;
    let data_w = maxx - minx;
    let data_h = maxy - miny;
    if data_w < 1e-10 || data_h < 1e-10 {
        return String::from(empty_svg);
    }
    let scale = ((width - 2.0 * margin) / data_w).min((height - 2.0 * margin) / data_h);

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>"
    );
    let _ = writeln!(
        svg,
        "<rect width='{width}' height='{height}' fill='#1a1a2e'/>"
    );

    for region in &planned.regions {
        let color = band_svg_color(region.band);
        let d = polygon_to_svg_path(&region.polygon, minx, miny, scale, height, margin);
        let _ = writeln!(
            svg,
            "<path d='{d}' fill='{color}' fill-opacity='0.35' fill-rule='evenodd' stroke='{color}' stroke-width='1'/>"
        );
    }

    for crease in &planned.creases {
        if let Some(poly) = &crease.own_region {
            let d = polygon_to_svg_path(poly, minx, miny, scale, height, margin);
            let _ = writeln!(
                svg,
                "<path d='{d}' fill='{SVG_CREASE_COLOR}' fill-opacity='0.35' fill-rule='evenodd' stroke='{SVG_CREASE_COLOR}' stroke-width='1'/>"
            );
        } else {
            let d =
                polyline_to_svg_path(&crease.centerline.points, minx, miny, scale, height, margin);
            let _ = writeln!(
                svg,
                "<path d='{d}' stroke='{SVG_NARROW_CREASE_COLOR}' stroke-width='1.5' fill='none'/>"
            );
        }
    }

    let crease_regions = planned
        .creases
        .iter()
        .filter(|c| c.own_region.is_some())
        .count();
    let mut y = 15.0;
    for (band, color) in SVG_BAND_COLORS {
        let n = planned.regions.iter().filter(|r| r.band == band).count();
        let _ = writeln!(
            svg,
            "<text x='5' y='{y:.0}' fill='{color}' font-size='10' font-family='monospace'>{band:?}: {n}</text>"
        );
        y += 12.0;
    }
    let _ = writeln!(
        svg,
        "<text x='5' y='{y:.0}' fill='{SVG_CREASE_COLOR}' font-size='10' font-family='monospace'>Creases: {crease_regions}</text>"
    );

    let _ = writeln!(svg, "</svg>");
    svg
}

/// Exterior + hole subpaths of a polygon, in SVG-space (margin offset,
/// scaled, Y-flipped to match `viz::toolpath_to_svg`).
fn polygon_to_svg_path(
    poly: &Polygon2,
    minx: f64,
    miny: f64,
    scale: f64,
    height: f64,
    margin: f64,
) -> String {
    let mut d = String::new();
    ring_to_svg_subpath(
        &poly.exterior,
        minx,
        miny,
        scale,
        height,
        margin,
        true,
        &mut d,
    );
    for hole in &poly.holes {
        ring_to_svg_subpath(hole, minx, miny, scale, height, margin, true, &mut d);
    }
    d
}

fn polyline_to_svg_path(
    points: &[crate::geo::P3],
    minx: f64,
    miny: f64,
    scale: f64,
    height: f64,
    margin: f64,
) -> String {
    let pts_2d: Vec<P2> = points.iter().map(|p| P2::new(p.x, p.y)).collect();
    let mut d = String::new();
    ring_to_svg_subpath(&pts_2d, minx, miny, scale, height, margin, false, &mut d);
    d
}

#[allow(clippy::too_many_arguments)]
fn ring_to_svg_subpath(
    ring: &[P2],
    minx: f64,
    miny: f64,
    scale: f64,
    height: f64,
    margin: f64,
    close: bool,
    out: &mut String,
) {
    for (i, p) in ring.iter().enumerate() {
        let x = margin + (p.x - minx) * scale;
        let y = height - margin - (p.y - miny) * scale; // flip Y
        if i == 0 {
            let _ = write!(out, "M{x:.1} {y:.1} ");
        } else {
            let _ = write!(out, "L{x:.1} {y:.1} ");
        }
    }
    if close {
        out.push_str("Z ");
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    /// Marching squares (inside `region_polygons_from_mask`) drops loops
    /// that touch the grid boundary, so every mask fixture needs a >=2-cell
    /// false margin ring on every edge to produce closed polygons.
    fn margin_covered(rows: usize, cols: usize) -> Vec<bool> {
        let mut covered = vec![true; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                if (r < 2 || c < 2 || r >= rows - 2 || c >= cols - 2)
                    && let Some(slot) = covered.get_mut(r * cols + c)
                {
                    *slot = false;
                }
            }
        }
        covered
    }

    /// Column-striped ramp: every 3-column stripe alternates between
    /// `angle_a` and `angle_b` (deg), constant across rows.
    fn stripe_ramp_z_grid(
        rows: usize,
        cols: usize,
        cell: f64,
        angle_a: f64,
        angle_b: f64,
    ) -> Vec<f64> {
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            let mut acc = 0.0f64;
            for col in 0..cols {
                if let Some(slot) = z.get_mut(row * cols + col) {
                    *slot = acc;
                }
                let stripe = col / 3;
                let angle_deg = if stripe % 2 == 0 { angle_a } else { angle_b };
                acc += angle_deg.to_radians().tan() * cell;
            }
        }
        z
    }

    /// A ramp-then-plateau patch: flat, then a `width`-column ramp at
    /// `angle_deg`, then flat again. Central differences at the two
    /// transition columns blend with the flat neighbour, so only the
    /// interior columns of the ramp read the pure angle — the patch never
    /// exceeds `angle_deg` anywhere.
    fn isolated_ramp_patch_z_grid(
        rows: usize,
        cols: usize,
        cell: f64,
        angle_deg: f64,
        c0: usize,
        width: usize,
    ) -> Vec<f64> {
        let slope = angle_deg.to_radians().tan();
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let val = if col < c0 {
                    0.0
                } else if col < c0 + width {
                    (col - c0) as f64 * slope * cell
                } else {
                    (width - 1) as f64 * slope * cell
                };
                if let Some(slot) = z.get_mut(row * cols + col) {
                    *slot = val;
                }
            }
        }
        z
    }

    /// Hemisphere dome (z = sqrt(R^2 - r^2), 0 outside), centered in grid.
    fn dome_z_grid(rows: usize, cols: usize, cell: f64, radius: f64) -> Vec<f64> {
        let cx = (cols - 1) as f64 * cell * 0.5;
        let cy = (rows - 1) as f64 * cell * 0.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cell - cx;
                let y = row as f64 * cell - cy;
                let r_sq = radius * radius - x * x - y * y;
                let val = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                if let Some(slot) = z.get_mut(row * cols + col) {
                    *slot = val;
                }
            }
        }
        z
    }

    /// A small conical bump: linear slope `angle_deg` from `(cx, cy)` out
    /// to `radius_cells`, flat (0) beyond.
    fn small_cone_z_grid(
        rows: usize,
        cols: usize,
        cell: f64,
        cx: usize,
        cy: usize,
        radius_cells: f64,
        angle_deg: f64,
    ) -> Vec<f64> {
        let k = angle_deg.to_radians().tan();
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let dx = (col as f64 - cx as f64) * cell;
                let dy = (row as f64 - cy as f64) * cell;
                let dist = (dx * dx + dy * dy).sqrt();
                let val = if dist < radius_cells * cell {
                    (radius_cells * cell - dist) * k
                } else {
                    0.0
                };
                if let Some(slot) = z.get_mut(row * cols + col) {
                    *slot = val;
                }
            }
        }
        z
    }

    fn poly_bbox(p: &Polygon2) -> (f64, f64, f64, f64) {
        let xs = p.exterior.iter().map(|q| q.x);
        let ys = p.exterior.iter().map(|q| q.y);
        (
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        )
    }

    // ── decompose: measurement provenance (M1 slice 1) ──────────────────

    /// Every region area this function returns is XY-projected, extracted on
    /// the grid it was handed, dilated by `overlap_mm`, on a mask eroded by
    /// one cell. The stats must SAY so — a reader who has only the numbers
    /// cannot tell a 0.125 mm classification grid from a 0.75 mm generation
    /// grid, and that difference is the entire §14q/X-6 correction.
    #[test]
    fn decompose_stamps_the_grid_and_dilation_it_measured_on() {
        let rows = 60;
        let cols = 60;
        let cell = 0.25;
        let z = stripe_ramp_z_grid(rows, cols, cell, 43.0, 47.0);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let params = FinishPlannerParams {
            overlap_mm: 1.5,
            ..FinishPlannerParams::for_tool(3.0)
        };
        let planned = decompose(&slope_map, &covered, &[], &params);
        let prov = planned.stats.provenance;

        assert_eq!(prov.domain, MeasurementDomain::ProjectedXyArea);
        assert_eq!(prov.stage, MeasurementStage::PolygonExtraction);
        assert_eq!(
            prov.cell_mm,
            Some(slope_map.cell_size),
            "provenance must carry the grid the value was measured on"
        );
        assert!(
            (prov.extraction_dilation_mm - params.overlap_mm).abs() < 1e-12,
            "non-zero dilation means band polygons OVERLAP; the report must say so"
        );
        assert!(
            prov.coverage_eroded,
            "step 0 erodes coverage by one cell, unconditionally"
        );
        // `decompose` sees a bare SlopeMap — it cannot claim a tool scale.
        assert_eq!(prov.cell_source, CellSource::Explicit);

        // A rendered line carries all of it, so a diagnostic table can print
        // the contract beside the number (§4.3).
        let described = prov.describe();
        assert!(described.contains("XY-projected"), "{described}");
        assert!(described.contains("polygon extraction"), "{described}");
        assert!(described.contains("0.250 mm cell"), "{described}");
        assert!(described.contains("OVERLAP"), "{described}");
    }

    // ── decompose: hysteresis / R1 ──────────────────────────────────────

    #[test]
    fn hysteresis_bridges_threshold_noise() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = stripe_ramp_z_grid(rows, cols, cell, 43.0, 47.0);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let conditioned = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams::for_tool(3.0),
        );
        let mid_conditioned = conditioned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::MidSteep)
            .count();
        assert_eq!(
            mid_conditioned, 1,
            "hysteresis should merge the 43/47 stripe pattern into one region"
        );

        let raw_params = FinishPlannerParams {
            hysteresis_deg: 0.0,
            close_radius_mm: 0.0,
            min_region_area_mm2: 0.0,
            ..FinishPlannerParams::for_tool(3.0)
        };
        let raw = decompose(&slope_map, &covered, &[], &raw_params);
        let mid_raw = raw
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::MidSteep)
            .count();
        assert!(
            mid_raw > 1,
            "without conditioning the stripe pattern should fragment, got {mid_raw}"
        );
    }

    #[test]
    fn hysteresis_requires_seed() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = isolated_ramp_patch_z_grid(rows, cols, cell, 40.0, 25, 5);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let planned = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams::for_tool(3.0),
        );
        let mid = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::MidSteep)
            .count();
        assert_eq!(mid, 0, "a 40° patch must never seed a MidSteep region");
        let shallow = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::Shallow)
            .count();
        assert_eq!(shallow, 1);
    }

    #[test]
    fn dome_decomposes_into_four_regions() {
        let rows = 80;
        let cols = 80;
        let cell = 1.0;
        let radius = 30.0;
        let z = dome_z_grid(rows, cols, cell, radius);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let planned = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams::for_tool(3.0),
        );

        let shallow: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::Shallow)
            .collect();
        let mid: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::MidSteep)
            .collect();
        let very: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::VerySteep)
            .collect();
        assert_eq!(shallow.len(), 2, "expected the cap + the surrounding plane");
        assert_eq!(mid.len(), 1);
        assert_eq!(very.len(), 1);
        assert_eq!(planned.stats.region_count, 4);

        let center = (cols - 1) as f64 * cell * 0.5;
        let apex = P2::new(center, center);
        assert!(
            shallow.iter().any(|r| r.polygon.contains_point(&apex)),
            "apex should be inside a Shallow polygon"
        );

        let rim_r = 0.97 * radius;
        let rim_pt = P2::new(center + rim_r, center);
        assert!(
            very.iter().any(|r| r.polygon.contains_point(&rim_pt)),
            "rim-adjacent point should be inside the VerySteep polygon"
        );

        assert!(planned.stats.raw_steep_islands >= 1);
        assert!(planned.stats.raw_very_steep_islands >= 1);
    }

    #[test]
    fn min_area_bump_absorbed() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = small_cone_z_grid(rows, cols, cell, 30, 30, 3.0, 60.0);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let planned = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams::for_tool(3.0),
        );

        let mid = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::MidSteep)
            .count();
        let very = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::VerySteep)
            .count();
        let shallow = planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::Shallow)
            .count();
        assert_eq!(mid, 0, "the small cone bump must be absorbed");
        assert_eq!(very, 0);
        assert_eq!(shallow, 1);
        assert!(planned.stats.absorbed_regions >= 1);
    }

    // ── crease claims ────────────────────────────────────────────────────

    /// A straight centerline along `y = 30.0`, `x` in `[10, 50)` — shared by
    /// the claim tests below.
    fn straight_crease(half_width_mm: f64) -> RestCenterline {
        let points: Vec<P3> = (10..50).map(|x| P3::new(x as f64, 30.0, 0.0)).collect();
        RestCenterline::without_samples(points, half_width_mm)
    }

    #[test]
    fn narrow_crease_claims_corridor() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = vec![0.0; rows * cols];
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        // half_width_mm (1.0) is below the crease-own-region canyon
        // threshold (6.0 at cusp radius 3.0), so this stays a non-canyon
        // crease, but it must still claim a corridor.
        let creases = vec![straight_crease(1.0)];

        let planned = decompose(
            &slope_map,
            &covered,
            &creases,
            &FinishPlannerParams::for_tool(3.0),
        );

        assert!(
            planned.creases[0].own_region.is_none(),
            "a non-canyon crease must not become its own clearing region"
        );
        assert!(
            planned.creases[0].corridor.is_some(),
            "every crease must claim a corridor, even a narrow one"
        );

        let mid_pt = P2::new(30.0, 30.0);
        assert!(
            planned.creases[0]
                .corridor
                .as_ref()
                .is_some_and(|c| c.contains_point(&mid_pt)),
            "the claimed corridor should cover the centerline's midpoint"
        );
        assert!(
            !planned
                .regions
                .iter()
                .any(|r| r.band == FinishBand::Shallow && r.polygon.contains_point(&mid_pt)),
            "the claimed corridor must be carved out of the Shallow band, not left inside it"
        );
        assert_eq!(planned.stats.claimed_creases, 1);
        assert!(planned.stats.claimed_cells > 0);
    }

    #[test]
    fn wide_crease_becomes_own_region_and_claims_corridor() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = vec![0.0; rows * cols];
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);

        let creases = vec![straight_crease(9.0)];
        let mid_pt = P2::new(30.0, 30.0);

        let planned = decompose(
            &slope_map,
            &covered,
            &creases,
            &FinishPlannerParams::for_tool(3.0),
        );

        assert!(planned.creases[0].own_region.is_some());
        assert!(planned.creases[0].corridor.is_some());
        let own = planned.creases[0]
            .own_region
            .as_ref()
            .expect("wide crease must produce an own_region");
        assert!(
            own.contains_point(&mid_pt),
            "the crease midpoint should be inside its own region"
        );
        assert!(
            !planned
                .regions
                .iter()
                .any(|r| r.band == FinishBand::Shallow && r.polygon.contains_point(&mid_pt)),
            "the crease corridor should be carved out of the Shallow band"
        );
        assert_eq!(planned.stats.region_count, planned.regions.len() + 1);
        assert_eq!(planned.stats.claimed_creases, 1);
        assert!(planned.stats.claimed_cells > 0);
    }

    #[test]
    fn claimed_cells_shrink_band_area() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = vec![0.0; rows * cols];
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);
        let params = FinishPlannerParams::for_tool(3.0);

        let baseline = decompose(&slope_map, &covered, &[], &params);
        let creases = vec![straight_crease(1.0)];
        let claimed = decompose(&slope_map, &covered, &creases, &params);

        assert_eq!(baseline.stats.claimed_cells, 0);
        assert!(claimed.stats.claimed_cells > 0);
        assert_eq!(claimed.stats.claimed_creases, 1);

        let band_area = |planned: &PlannedRegions, band: FinishBand| -> f64 {
            planned
                .regions
                .iter()
                .filter(|r| r.band == band)
                .map(|r| r.polygon.area())
                .sum()
        };

        let baseline_area = band_area(&baseline, FinishBand::Shallow);
        let claimed_area = band_area(&claimed, FinishBand::Shallow);
        assert!(
            claimed_area < baseline_area,
            "claimed run's Shallow band area ({claimed_area}) should be smaller than baseline ({baseline_area})"
        );
    }

    #[test]
    fn empty_creases_unchanged_behavior() {
        let rows = 80;
        let cols = 80;
        let cell = 1.0;
        let radius = 30.0;
        let z = dome_z_grid(rows, cols, cell, radius);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);
        let params = FinishPlannerParams::for_tool(3.0);

        let planned = decompose(&slope_map, &covered, &[], &params);

        assert!(planned.creases.is_empty());
        assert_eq!(planned.stats.claimed_creases, 0);
        assert_eq!(planned.stats.claimed_cells, 0);
        // Same shape as the dedicated dome-decompose test: 2 shallow + 1 mid
        // + 1 very-steep, region_count 4 — the claims pipeline must not
        // perturb the no-crease path at all.
        assert_eq!(planned.stats.region_count, 4);
        assert_eq!(planned.regions.len(), 4);
    }

    // ── overlap / determinism / degenerate inputs ───────────────────────

    #[test]
    fn overlap_dilates_band_polygons() {
        let rows = 60;
        let cols = 60;
        let cell = 1.0;
        let z = vec![0.0; rows * cols];
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);

        // Wider margin (6 cells) than the shared helper's minimum: the
        // dilated (overlap=2.0) polygon must stay clear of the grid's true
        // boundary or marching squares drops the loop entirely.
        let margin = 6usize;
        let mut covered = vec![true; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                if (r < margin || c < margin || r >= rows - margin || c >= cols - margin)
                    && let Some(slot) = covered.get_mut(r * cols + c)
                {
                    *slot = false;
                }
            }
        }

        let base = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams {
                overlap_mm: 0.0,
                ..FinishPlannerParams::for_tool(3.0)
            },
        );
        let dilated = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams {
                overlap_mm: 2.0,
                ..FinishPlannerParams::for_tool(3.0)
            },
        );

        assert_eq!(base.regions.len(), 1);
        assert_eq!(dilated.regions.len(), 1);

        let (bminx, bminy, bmaxx, bmaxy) = poly_bbox(&base.regions[0].polygon);
        let (dminx, dminy, dmaxx, dmaxy) = poly_bbox(&dilated.regions[0].polygon);

        assert!((bminx - dminx - 2.0).abs() <= 1.0, "left growth");
        assert!((bminy - dminy - 2.0).abs() <= 1.0, "bottom growth");
        assert!((dmaxx - bmaxx - 2.0).abs() <= 1.0, "right growth");
        assert!((dmaxy - bmaxy - 2.0).abs() <= 1.0, "top growth");
    }

    #[test]
    fn deterministic_output() {
        let rows = 80;
        let cols = 80;
        let cell = 1.0;
        let radius = 30.0;
        let z = dome_z_grid(rows, cols, cell, radius);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);
        let params = FinishPlannerParams::for_tool(3.0);

        let a = decompose(&slope_map, &covered, &[], &params);
        let b = decompose(&slope_map, &covered, &[], &params);

        assert_eq!(a.regions.len(), b.regions.len());
        assert_eq!(a.stats.region_count, b.stats.region_count);
        for (ra, rb) in a.regions.iter().zip(b.regions.iter()) {
            assert_eq!(ra.band, rb.band);
            let fa = ra.polygon.exterior.first().map(|p| (p.x, p.y));
            let fb = rb.polygon.exterior.first().map(|p| (p.x, p.y));
            assert_eq!(fa, fb);
        }
    }

    #[test]
    fn degenerate_inputs_never_panic() {
        let empty_slope = SlopeMap::from_z_grid(&[], 0, 0, 0.0, 0.0, 1.0);
        let out = decompose(&empty_slope, &[], &[], &FinishPlannerParams::default());
        assert!(out.regions.is_empty());
        assert!(out.creases.is_empty());

        let z = vec![0.0; 10 * 10];
        let slope_map = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);

        let mismatched_covered = vec![true; 5];
        let out2 = decompose(
            &slope_map,
            &mismatched_covered,
            &[],
            &FinishPlannerParams::default(),
        );
        assert!(out2.regions.is_empty());

        let all_uncovered = vec![false; 100];
        let out3 = decompose(
            &slope_map,
            &all_uncovered,
            &[],
            &FinishPlannerParams::default(),
        );
        assert!(out3.regions.is_empty());
    }

    // ── helper-level ─────────────────────────────────────────────────────

    #[test]
    fn morphological_close_bridges_small_gap() {
        let rows = 10;
        let cols = 10;
        let mut mask = vec![false; rows * cols];
        for r in 2..8 {
            for c in 2..4 {
                if let Some(slot) = mask.get_mut(r * cols + c) {
                    *slot = true;
                }
            }
            for c in 5..7 {
                if let Some(slot) = mask.get_mut(r * cols + c) {
                    *slot = true;
                }
            }
        }
        assert_eq!(
            count_components(&mask, rows, cols),
            2,
            "1-cell slit should leave two separate blocks"
        );

        let closed = morphological_close(&mask, rows, cols, 1.5);
        assert_eq!(
            count_components(&closed, rows, cols),
            1,
            "closing at radius 1.5 should bridge a 1-cell slit"
        );
    }

    #[test]
    fn planned_regions_to_svg_smoke() {
        let rows = 80;
        let cols = 80;
        let cell = 1.0;
        let radius = 30.0;
        let z = dome_z_grid(rows, cols, cell, radius);
        let slope_map = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cell);
        let covered = margin_covered(rows, cols);
        let planned = decompose(
            &slope_map,
            &covered,
            &[],
            &FinishPlannerParams::for_tool(3.0),
        );

        let svg = planned_regions_to_svg(&planned, 800.0, 800.0);
        assert!(svg.contains("svg"));
        assert!(svg.contains("#4caf50"));
        assert!(svg.contains("#ff9800"));
        assert!(svg.contains("#f44336"));
        assert!(!svg.is_empty());

        let empty = PlannedRegions {
            regions: Vec::new(),
            creases: Vec::new(),
            stats: DecomposeStats::default(),
        };
        let empty_svg = planned_regions_to_svg(&empty, 800.0, 800.0);
        assert!(empty_svg.contains("svg"));
    }
}
