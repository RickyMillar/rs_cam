//! Tier map → per-tier **island sets**: the layer that turns a raw tier label
//! grid into an operator-approvable set of regions.
//!
//! This is Phase I of the multi-tool island-finishing plan
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`). Its input is a
//! [`crate::tier_map::TierMap`] — a per-cell "coarsest tool that holds this
//! cell" label — and its output is, per fine tier, a [`RegionSet`] a
//! `unified_finish` op can be confined to.
//!
//! # Why this layer exists at all
//!
//! Measured on the real wanaka mesh (2026-08-26, 0.3 mm cells, tolerance
//! 0.05, R2.0 → R1.0 ladder, [`crate::tier_map::ResidualTreatment::SlopeCompensated`]):
//! the fine tier's territory is **22.0% of the board / 8,815 mm²**, and it
//! arrives as roughly **566 raw islands**. Handed to
//! [`crate::region_mask::region_polygons_from_mask`] as-is, 502 of those are
//! silently dropped by [`crate::region_mask::MAX_REST_REGIONS`] and the caller
//! reads a 64-long list as the whole answer. A per-island generation pass on
//! even the surviving 64 is the fragmentation half of the T4 loss (19 k
//! retracts, 16,140 s of rapids — plan §0).
//!
//! So: **close** (merge neighbours), **filter** (drop what is not worth a tool
//! change), **cap** (bounded auto-raise, then an honest truncation), and
//! **overlap** (grow each survivor into coarser territory so the seam blends).
//!
//! # Pipeline
//!
//! ```text
//! covered      = map.covered_mask() minus the grid-edge ring
//! rim demotion = fine-tier cells within rim_erosion_mm of uncovered -> tier 0
//! for k = finest .. 1:                       (finest FIRST — see Ownership)
//!     allowed  = covered AND NOT owned-by-any-finer-tier
//!     loop i in 0 ..= MAX_CLOSE_RAISES:
//!         r     = close_radius * CAP_CLOSE_RAISE_FACTOR^i
//!         mask  = close(raw_tier_mask, r) AND allowed
//!         comps = 8-connected components of mask
//!         kept  = comps with cells*cell_area >= min_region_area_mm2
//!         break when kept.len() <= max_regions_per_tier
//!     truncate kept to the cap, largest first, and REPORT it
//!     owned     = one polygon per kept island
//!     machining = the same islands dilated by overlap_mm, clamped to covered
//! ```
//!
//! Steps 2 (close), 4 (min-area) and 6 (marching squares) of
//! [`crate::finish_planner::decompose`] are the same three operations, reused
//! rather than re-implemented: [`crate::finish_planner`]'s
//! `morphological_close` and `label_components` are the actual functions
//! called here, and the polygon extraction is
//! [`crate::region_mask::region_polygons_from_mask_reported`].
//!
//! # The 64-region cap does not apply here, and that is deliberate
//!
//! [`crate::region_mask::MAX_REST_REGIONS`] is a hard 64 with no caller-facing
//! dial, and it truncates *inside* the extractor. Rather than raise it (which
//! would change every existing caller's behaviour) or accept it (which would
//! reinstate the silent truncation this layer exists to remove), this module
//! **extracts one island at a time**: each call to
//! [`crate::region_mask::region_polygons_from_mask_reported`] is handed a mask
//! containing exactly one connected component, so the cap can never fire and
//! its report is uniformly neutral. Counting and capping happen here instead,
//! on the mask, where [`TierIslandParams::max_regions_per_tier`] is an operator
//! dial and [`TierCapReport`] says what it did.
//!
//! Step 1 (hysteresis) is deliberately **absent**. Hysteresis needs the
//! continuous scalar a threshold was applied to; [`TierMap`] stores a `u8`
//! label and a single `f32` reference plane, not the per-tool residual field —
//! that is the 5 B/cell memory budget its module doc defends. Widening the map
//! to carry residuals purely so this module could hysterese them would cost
//! `4·n` B/cell on a board that already OOMs simulation at 0.1 mm.
//!
//! # Ownership is a partition; overlap bands are not
//!
//! Tiers are processed **finest first**, and each tier's mask is clipped to
//! the cells no finer tier already took. So pre-overlap ownership
//! ([`TierIslandSet::owned_mask`]) partitions the covered grid: no cell is
//! owned twice, and the coarsest tier (index 0) owns the complement — which is
//! why it gets no island set at all. The coarse tool is going to sweep its
//! territory as one pass; it does not need islands.
//!
//! [`TierIslandSet::machining`] — the same islands grown by `overlap_mm` — is
//! explicitly **not** a partition. Its whole purpose is to reach into coarser
//! territory so the fine tool's first pass lands on ground the coarse tool
//! already cut, blending the two cusp patterns instead of butting them.
//! `stock_to_leave` must be held EQUAL across tiers for that to blend cusps
//! rather than print a height step (T2 §7).
//!
//! # What a dropped island means
//!
//! An island under `min_region_area_mm2` is not deleted from the job — it
//! falls out of the fine tier's ownership and back into the complement, i.e.
//! the **coarser** tool keeps it. That is the conservative direction for time
//! and the honest one for quality: the operator's min-island dial says "not
//! worth a tool change here", and the coarse tool's own cusp is what remains.

use std::fmt;

use tracing::warn;

use crate::finish_planner::{and_masks_in_place, label_components, morphological_close};
use crate::grid_field::distance_transform_2d;
use crate::grid2::Grid2;
use crate::polygon::Polygon2;
use crate::region_mask::region_polygons_from_mask_reported;
use crate::region_set::RegionSet;
use crate::tier_map::{NO_TIER, TierMap};

// ── Constants ───────────────────────────────────────────────────────────

/// Default seam-blend band (mm) each fine tier's islands are grown by, into
/// the coarser tier's territory. 2.0 is the number `unified_finish` already
/// uses between its own slope bands (`FinishPlannerParams::overlap_mm`'s
/// shipped value on the P2 chain), so a tier seam and a band seam blend at the
/// same scale.
pub const DEFAULT_OVERLAP_MM: f64 = 2.0;

/// Default per-tier island cap. Above this an operator cannot meaningfully
/// veto a preview, and every extra island is one more tool-down/tool-up cycle.
/// Deliberately far below [`crate::region_mask::MAX_REST_REGIONS`] (64): that
/// constant is a *sliver-storm backstop* inside the extractor, this one is a
/// *planning* decision the operator can move.
pub const DEFAULT_MAX_REGIONS_PER_TIER: usize = 24;

/// Factor the close radius is multiplied by on each auto-raise pass when a
/// tier still has more islands than its cap.
pub const CAP_CLOSE_RAISE_FACTOR: f64 = 1.5;

/// How many auto-raise passes the cap loop may take. Bounded because closing
/// is a *lossy* merge: at some radius the islands stop being the operator's
/// features and start being one blob covering the board. Three passes at 1.5×
/// is a 3.375× radius ceiling — enough to bridge cusp-scale fragmentation,
/// not enough to weld a terrain.
pub const MAX_CLOSE_RAISES: usize = 3;

/// `close_radius_mm = cusp_radius · CLOSE_RADIUS_PER_CUSP_RADIUS`, the same
/// derivation [`crate::finish_planner::FinishPlannerParams::for_tool`] uses.
pub const CLOSE_RADIUS_PER_CUSP_RADIUS: f64 = 0.5;

/// `min_region_area_mm2 = (2·cusp_radius)² · MIN_REGION_AREA_TOOL_DIAMETERS_SQ`
/// — "a few tool diameters²", the same derivation
/// [`crate::finish_planner::FinishPlannerParams::for_tool`] uses.
pub const MIN_REGION_AREA_TOOL_DIAMETERS_SQ: f64 = 4.0;

/// Clamp band for [`TierIslandParams::coarseness`]. A slider that could reach
/// 0 would make the derived dials inert (no close, no floor — the 566-island
/// storm), and one that could reach 1000 would weld the board into one island;
/// neither is a setting, both are a mis-typed number.
pub const COARSENESS_MIN: f64 = 0.05;
/// See [`COARSENESS_MIN`].
pub const COARSENESS_MAX: f64 = 20.0;

// ── Errors ──────────────────────────────────────────────────────────────

/// Why [`extract_tier_islands`] refused.
///
/// `PartialEq` but not `Eq`: [`Self::MalformedMap`] carries the offending
/// `cell_mm`, and the whole point of quoting it back is that it may be a `NaN`
/// or a zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TierIslandError {
    /// `cusp_radii.len() != map.tier_count`.
    ///
    /// Refused rather than papered over: every dial in this module is derived
    /// from the tier's **own** tool's cusp radius, so borrowing a neighbour's
    /// radius would silently mis-size the close radius and the min-island
    /// floor — the exact failure `FinishPlannerParams::for_tool`'s doc
    /// records from the Ø1-tip taper (144 mm² instead of 4 mm²).
    LadderMismatch {
        /// Tiers the map was built with.
        tier_count: usize,
        /// Cusp radii the caller supplied.
        cusp_radii: usize,
    },
    /// The map's `labels` length does not match `nx * ny`, or `cell_mm` is not
    /// positive — a corrupt or hand-built map, not a planning outcome.
    MalformedMap {
        /// `nx * ny`.
        expected_cells: usize,
        /// `labels.len()`.
        labels: usize,
        /// `cell_mm` as given.
        cell_mm: f64,
    },
}

impl fmt::Display for TierIslandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::LadderMismatch {
                tier_count,
                cusp_radii,
            } => write!(
                f,
                "tier_islands: the map has {tier_count} tiers but {cusp_radii} cusp radii were \
                 supplied; every dial derives from the tier's own tool"
            ),
            Self::MalformedMap {
                expected_cells,
                labels,
                cell_mm,
            } => write!(
                f,
                "tier_islands: malformed tier map — {expected_cells} cells expected, \
                 {labels} labels, cell_mm = {cell_mm}"
            ),
        }
    }
}

impl std::error::Error for TierIslandError {}

// ── Params ──────────────────────────────────────────────────────────────

/// Island-filtering dials. Every `Option` is "`None` = derive from this tier's
/// own cusp radius", mirroring
/// [`crate::finish_planner::FinishPlannerParams::for_tool`].
///
/// # The coarseness slider
///
/// [`Self::coarseness`] is the operator's single knob (plan Phase U): "lots of
/// small regions" ↔ "a few large ones". It scales the **derived** dials only —
/// linearly for the close radius, quadratically for the min island area,
/// because those are one length scale expressed once as a radius and once as
/// its square. A dial the operator typed explicitly is taken **verbatim** at
/// every coarseness; otherwise the number showing in the advanced flyout would
/// not be the number in force.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierIslandParams {
    /// Morphological close radius (mm), or `None` to derive
    /// (`cusp_radius · `[`CLOSE_RADIUS_PER_CUSP_RADIUS`]`· coarseness`).
    /// Merges islands whose gap is inside the radius.
    pub close_radius_mm: Option<f64>,
    /// Minimum island area (mm²) worth giving this tier, or `None` to derive
    /// (`(2·cusp_radius)² · `[`MIN_REGION_AREA_TOOL_DIAMETERS_SQ`]`·
    /// coarseness²`). Measured on CELL COUNT, not polygon area — it is a
    /// population test on the mask, applied before extraction.
    pub min_region_area_mm2: Option<f64>,
    /// 1.0 = neutral. Scales both derived dials together; see the struct doc.
    /// Clamped to [`COARSENESS_MIN`]..=[`COARSENESS_MAX`]; non-finite reads as
    /// 1.0.
    pub coarseness: f64,
    /// Seam-blend band (mm) each fine tier's islands are grown by, into the
    /// coarser tier's territory. Applied AFTER filtering, per island, so it
    /// never changes the island count. `0.0` turns it off. See
    /// [`DEFAULT_OVERLAP_MM`].
    pub overlap_mm: f64,
    /// Per-tier island cap. See [`DEFAULT_MAX_REGIONS_PER_TIER`] and
    /// [`TierCapReport`].
    pub max_regions_per_tier: usize,
    /// Hand fine-tier cells within this distance (mm) of an uncovered cell
    /// back to the coarsest tier, before any morphology.
    ///
    /// **Off (0.0) by default, and that is a stated limitation, not an
    /// oversight.** [`crate::tier_map`]'s module doc assigns this erosion to
    /// its consumer: within roughly one ENVELOPE radius of the part edge a big
    /// tool hangs off and rests on the rim, reading a false-high residual, so
    /// the rim reads as fine-tier territory. This module is handed CUSP radii
    /// (the feature scale every other dial needs) and cannot derive an
    /// envelope band from them — on a Ø1-tip/Ø6-shank taper the two differ by
    /// 3×. A caller that knows the ladder's tools should pass the coarsest
    /// tool's envelope radius here.
    pub rim_erosion_mm: f64,
}

impl Default for TierIslandParams {
    fn default() -> Self {
        Self {
            close_radius_mm: None,
            min_region_area_mm2: None,
            coarseness: 1.0,
            overlap_mm: DEFAULT_OVERLAP_MM,
            max_regions_per_tier: DEFAULT_MAX_REGIONS_PER_TIER,
            rim_erosion_mm: 0.0,
        }
    }
}

impl TierIslandParams {
    /// The derived close radius (mm) for a tier whose tool has this cusp
    /// radius, at this coarseness. Public because the GUI's advanced flyout
    /// pre-fills its raw dials from exactly this.
    #[must_use]
    pub fn derived_close_radius_mm(cusp_radius_mm: f64, coarseness: f64) -> f64 {
        let c = clamp_coarseness(coarseness);
        cusp_radius_mm.max(0.0) * CLOSE_RADIUS_PER_CUSP_RADIUS * c
    }

    /// The derived minimum island area (mm²) for a tier whose tool has this
    /// cusp radius, at this coarseness. See
    /// [`Self::derived_close_radius_mm`].
    #[must_use]
    pub fn derived_min_region_area_mm2(cusp_radius_mm: f64, coarseness: f64) -> f64 {
        let c = clamp_coarseness(coarseness);
        (2.0 * cusp_radius_mm.max(0.0)).powi(2) * MIN_REGION_AREA_TOOL_DIAMETERS_SQ * c * c
    }

    /// [`Self::coarseness`] after clamping.
    #[must_use]
    pub fn coarseness_clamped(&self) -> f64 {
        clamp_coarseness(self.coarseness)
    }

    /// The close radius in force for a tier with this cusp radius: the
    /// explicit dial verbatim, or the derivation scaled by coarseness.
    #[must_use]
    pub fn effective_close_radius_mm(&self, cusp_radius_mm: f64) -> f64 {
        self.close_radius_mm.map_or_else(
            || Self::derived_close_radius_mm(cusp_radius_mm, self.coarseness),
            |v| v.max(0.0),
        )
    }

    /// The min island area in force for a tier with this cusp radius: the
    /// explicit dial verbatim, or the derivation scaled by coarseness².
    #[must_use]
    pub fn effective_min_region_area_mm2(&self, cusp_radius_mm: f64) -> f64 {
        self.min_region_area_mm2.map_or_else(
            || Self::derived_min_region_area_mm2(cusp_radius_mm, self.coarseness),
            |v| v.max(0.0),
        )
    }
}

fn clamp_coarseness(coarseness: f64) -> f64 {
    if coarseness.is_finite() {
        coarseness.clamp(COARSENESS_MIN, COARSENESS_MAX)
    } else {
        1.0
    }
}

// ── Reports ─────────────────────────────────────────────────────────────

/// What the per-tier island cap did — the typed answer to *"did anything get
/// merged or thrown away, and why?"*.
///
/// **Not a three-valued channel.** Every tier measures all of this, so there
/// is no "not measured" state to preserve here; the `Option` belongs one level
/// up, wherever a consumer records whether island extraction ran at all. Same
/// argument [`crate::region_mask::RegionCapReport`] makes for itself.
///
/// Read [`Self::acted`] first: on a healthy tier every field is neutral and
/// the record exists only to say so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierCapReport {
    /// Islands after the FINAL morphological close, before the min-area
    /// filter.
    pub islands_after_close: usize,
    /// Islands after the min-area filter, before any truncation.
    pub islands_after_min_area: usize,
    /// Islands actually kept — `min(islands_after_min_area, cap)`.
    pub kept: usize,
    /// The cap in force ([`TierIslandParams::max_regions_per_tier`]), carried
    /// so a renderer never has to re-derive it.
    pub cap: usize,
    /// How many auto-raise passes the close radius took. `0` on a healthy
    /// tier; never more than [`MAX_CLOSE_RAISES`].
    pub close_raises: usize,
    /// The close radius (mm) the tier started at — the dial as configured.
    pub first_close_radius_mm: f64,
    /// The close radius (mm) actually in force after the raises:
    /// `first · `[`CAP_CLOSE_RAISE_FACTOR`]`^close_raises` — exactly, so a
    /// renderer can state the merge scale rather than imply it.
    ///
    /// **A zero first radius cannot be raised** (`0 · 1.5ⁿ = 0`). A tier
    /// configured with `close_radius_mm: Some(0.0)`, or one whose tool reports
    /// a zero cusp radius, therefore has the truncation as its only backstop —
    /// [`Self::close_raises`] will still read [`MAX_CLOSE_RAISES`] because the
    /// loop ran, and [`Self::truncated`] is what actually held the cap.
    pub final_close_radius_mm: f64,
}

impl TierCapReport {
    /// `true` when the cap changed the answer — the radius was raised, or
    /// islands were truncated. A `false` here is the healthy reading.
    #[must_use]
    pub const fn acted(&self) -> bool {
        self.close_raises > 0 || self.islands_after_min_area > self.kept
    }

    /// `true` when the hard truncation fired, i.e. the bounded raise loop
    /// could not bring the count under the cap and islands were dropped.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.islands_after_min_area > self.kept
    }

    /// How many islands the truncation dropped. `0` unless
    /// [`Self::truncated`].
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.islands_after_min_area.saturating_sub(self.kept)
    }

    /// How many islands the min-area filter absorbed back into the coarser
    /// tier.
    #[must_use]
    pub const fn absorbed_by_min_area(&self) -> usize {
        self.islands_after_close
            .saturating_sub(self.islands_after_min_area)
    }
}

// ── Output ──────────────────────────────────────────────────────────────

/// One fine tier's islands.
#[derive(Debug, Clone)]
pub struct TierIslandSet {
    /// Ladder index — always ≥ 1. Tier 0 is the complement and has no set.
    pub tier: u8,
    /// Islands kept, as 8-connected components of the ownership mask. **This
    /// is the authoritative count.** `owned.len()` normally equals it, but the
    /// two can disagree in two pathological cases: a component that pinches to
    /// a single diagonal link traces as two marching-squares loops, and a
    /// component whose enclosed area is under one cell is dropped as a
    /// degenerate loop. Count islands from here, not from `owned`.
    pub islands: usize,
    /// Connected components of this tier's RAW label mask, before rim
    /// demotion, close, min-area or cap — the "566 islands" number.
    pub raw_island_count: usize,
    /// Pre-overlap ownership polygons. Disjoint from every other tier's.
    pub owned: RegionSet<'static>,
    /// [`Self::owned`] grown by [`TierIslandParams::overlap_mm`] and clamped
    /// to the tier map's coverage — the set a consumer op should take as its
    /// `machining_boundary`. Identical to `owned` when the dial is `0.0`.
    /// **Not** disjoint from other tiers' overlap bands, by design.
    pub machining: RegionSet<'static>,
    /// Sum of [`Self::owned`] polygon areas (mm², XY-projected, holes
    /// subtracted). Close to but not identical with `owned_cells · cell²`:
    /// marching squares treats grid cells as CORNERS and cuts at edge
    /// midpoints, so a solid rectangle traces to exactly its `n · cell` extent
    /// while a ragged or diagonal boundary trades half-cells either way.
    pub owned_area_mm2: f64,
    /// Cells this tier owns.
    pub owned_cells: usize,
    /// Per-cell ownership over the tier map's grid, row-major `r·nx + c`.
    /// The partition record, and the input a preview overlay renders.
    pub owned_mask: Vec<bool>,
    /// What the cap did. See [`TierCapReport`].
    pub cap: TierCapReport,
    /// Close radius (mm) actually in force, after coarseness and any
    /// auto-raise. Equals `cap.final_close_radius_mm`.
    pub close_radius_mm: f64,
    /// Min island area (mm²) actually in force, after coarseness.
    pub min_region_area_mm2: f64,
}

/// Per-tier island sets for one [`TierMap`].
#[derive(Debug, Clone)]
pub struct TierIslands {
    /// One entry per FINE tier (ladder index ≥ 1), in ascending tier order.
    /// The coarsest tier (0) is the complement and is deliberately absent.
    pub per_tier: Vec<TierIslandSet>,
    /// Grid cell size (mm) the sets were extracted at.
    pub cell_mm: f64,
    /// Ladder length the source map was built with.
    pub tier_count: usize,
}

impl TierIslands {
    /// The set for ladder index `k`, or `None` for tier 0 / off the ladder.
    #[must_use]
    pub fn set_for_tier(&self, k: u8) -> Option<&TierIslandSet> {
        self.per_tier.iter().find(|s| s.tier == k)
    }

    /// `true` when no fine tier kept a single island — the whole board is the
    /// coarse tool's.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.per_tier.iter().all(|s| s.owned.is_empty())
    }

    /// Total fine-tier territory (mm²), summed over `owned` — safe to sum
    /// because ownership partitions. Summing `machining` would double-count
    /// the overlap bands.
    #[must_use]
    pub fn total_owned_area_mm2(&self) -> f64 {
        self.per_tier.iter().map(|s| s.owned_area_mm2).sum()
    }

    /// Total islands the operator would be asked to approve.
    #[must_use]
    pub fn total_islands(&self) -> usize {
        self.per_tier.iter().map(|s| s.islands).sum()
    }

    /// Every tier whose [`TierCapReport::acted`] is true — what a preview
    /// panel must tell the operator was merged or dropped.
    pub fn tiers_where_the_cap_acted(&self) -> impl Iterator<Item = &TierIslandSet> {
        self.per_tier.iter().filter(|s| s.cap.acted())
    }
}

// ── Extraction ──────────────────────────────────────────────────────────

/// Turn a [`TierMap`] into per-tier island sets. See the module doc for the
/// pipeline and for what ownership does and does not guarantee.
///
/// `cusp_radii` is the ladder's per-tier **cusp** (tip) radii in mm, coarse
/// first — the same ordering and the same radius
/// [`crate::tier_map::TierLadder`] sorts on
/// ([`crate::tool::MillingCutter::cusp_radius_mm`], never `radius()`: on a
/// tapered tool `radius()` reports the shank, which is what made
/// `min_region_area_mm2` 144 mm² instead of 4 mm² in the incident
/// `FinishPlannerParams::for_tool` records). Its length must equal
/// `map.tier_count`.
///
/// Deterministic: no clock, no RNG, no hash-order iteration, and every sort
/// carries a total tie-break.
pub fn extract_tier_islands(
    map: &TierMap,
    params: &TierIslandParams,
    cusp_radii: &[f64],
) -> Result<TierIslands, TierIslandError> {
    let total = map.nx.saturating_mul(map.ny);
    if map.labels.len() != total || map.cell_mm <= 0.0 || !map.cell_mm.is_finite() {
        return Err(TierIslandError::MalformedMap {
            expected_cells: total,
            labels: map.labels.len(),
            cell_mm: map.cell_mm,
        });
    }
    if cusp_radii.len() != map.tier_count {
        return Err(TierIslandError::LadderMismatch {
            tier_count: map.tier_count,
            cusp_radii: cusp_radii.len(),
        });
    }
    if total == 0 || map.tier_count < 2 {
        return Ok(TierIslands {
            per_tier: Vec::new(),
            cell_mm: map.cell_mm,
            tier_count: map.tier_count,
        });
    }

    let (nx, ny, cell) = (map.nx, map.ny, map.cell_mm);
    let cell_area = cell * cell;

    // ── Coverage, minus the grid-edge ring ──────────────────────────────
    // Marching squares cannot close a loop that touches the grid boundary
    // (`region_mask`'s own cap fixture documents the failure: boundary-flush
    // blocks trace to nothing). `TierMapParams::margin_mm` already pads the
    // walk so the outer ring is non-contact; this is the belt to that braces,
    // and it costs one pass.
    let covered: Vec<bool> = (0..total)
        .map(|i| {
            let on_edge = i / nx == 0 || i / nx + 1 >= ny || i % nx == 0 || i % nx + 1 >= nx;
            !on_edge && map.labels.get(i).copied().unwrap_or(NO_TIER) != NO_TIER
        })
        .collect();

    // ── Rim demotion (opt-in) ───────────────────────────────────────────
    let rim: Option<Vec<bool>> = if params.rim_erosion_mm > 0.0 {
        let uncovered: Vec<bool> = covered.iter().map(|&c| !c).collect();
        let dist = distance_transform_2d(&uncovered, ny, nx);
        let radius_cells = params.rim_erosion_mm / cell;
        Some(dist.iter().map(|&d| d <= radius_cells).collect())
    } else {
        None
    };

    // Base labels: uncovered cells are NO_TIER, rim-band fine cells fall back
    // to the coarsest tier.
    let base_labels: Vec<u8> = (0..total)
        .map(|i| {
            if !covered.get(i).copied().unwrap_or(false) {
                return NO_TIER;
            }
            let label = map.labels.get(i).copied().unwrap_or(NO_TIER);
            if label >= 1
                && rim
                    .as_ref()
                    .is_some_and(|r| r.get(i).copied().unwrap_or(false))
            {
                0
            } else {
                label
            }
        })
        .collect();

    // ── Per tier, FINEST FIRST ──────────────────────────────────────────
    let mut owned_by_finer = vec![false; total];
    let mut sets: Vec<TierIslandSet> = Vec::new();

    for k in (1..map.tier_count).rev() {
        let Ok(tier) = u8::try_from(k) else { continue };
        let cusp = cusp_radii.get(k).copied().unwrap_or(0.0);
        let first_close_radius_mm = params.effective_close_radius_mm(cusp);
        let min_region_area_mm2 = params.effective_min_region_area_mm2(cusp);

        // Cells no finer tier took, and that are on the part at all.
        let allowed: Vec<bool> = covered
            .iter()
            .zip(owned_by_finer.iter())
            .map(|(&cov, &taken)| cov && !taken)
            .collect();

        let raw_mask: Vec<bool> = base_labels.iter().map(|&l| l == tier).collect();
        let raw_comps = label_components(ny, nx, |i| raw_mask.get(i).copied().unwrap_or(false));
        let raw_island_count = raw_comps.len();

        // ── The bounded cap loop ────────────────────────────────────────
        // Each pass re-closes the RAW mask at the raised radius rather than
        // re-closing the previous pass's output: closing is not idempotent
        // under a growing radius, and compounding it would merge by a
        // different (and unstateable) amount than the radius the report
        // publishes.
        let mut close_raises = 0usize;
        let mut close_radius_mm = first_close_radius_mm;
        let mut kept: Vec<Vec<usize>> = Vec::new();
        let mut islands_after_close = 0usize;
        for pass in 0..=MAX_CLOSE_RAISES {
            close_raises = pass;
            let exponent = i32::try_from(pass).unwrap_or(0);
            close_radius_mm = first_close_radius_mm * CAP_CLOSE_RAISE_FACTOR.powi(exponent);

            let mut mask: Vec<bool> = raw_mask
                .iter()
                .zip(allowed.iter())
                .map(|(&m, &a)| m && a)
                .collect();
            if close_radius_mm > 0.0 {
                mask = morphological_close(&mask, ny, nx, close_radius_mm / cell);
                and_masks_in_place(&mut mask, &allowed);
            }

            let comps = label_components(ny, nx, |i| mask.get(i).copied().unwrap_or(false));
            islands_after_close = comps.len();
            kept = comps
                .into_iter()
                .filter(|c| c.len() as f64 * cell_area >= min_region_area_mm2)
                .collect();

            if kept.len() <= params.max_regions_per_tier {
                break;
            }
        }

        // Largest first, with a total tie-break on the component's smallest
        // cell index so equal-size islands order the same way on every run.
        kept.sort_by(|a, b| {
            let a_min = a.iter().copied().min().unwrap_or(0);
            let b_min = b.iter().copied().min().unwrap_or(0);
            b.len().cmp(&a.len()).then_with(|| a_min.cmp(&b_min))
        });

        let islands_after_min_area = kept.len();
        if kept.len() > params.max_regions_per_tier {
            warn!(
                tier = k,
                total_islands = islands_after_min_area,
                cap = params.max_regions_per_tier,
                close_raises,
                final_close_radius_mm = close_radius_mm,
                "tier_islands: island count still over the cap after the bounded close-radius \
                 raise; keeping the largest by cell count. The typed record is \
                 TierIslandSet::cap."
            );
            kept.truncate(params.max_regions_per_tier);
        }

        let cap = TierCapReport {
            islands_after_close,
            islands_after_min_area,
            kept: kept.len(),
            cap: params.max_regions_per_tier,
            close_raises,
            first_close_radius_mm,
            final_close_radius_mm: close_radius_mm,
        };

        // ── Ownership, then polygons ────────────────────────────────────
        let mut owned_mask = vec![false; total];
        let mut owned_cells = 0usize;
        for comp in &kept {
            owned_cells += comp.len();
            for &i in comp {
                if let Some(slot) = owned_mask.get_mut(i) {
                    *slot = true;
                }
            }
        }
        for (dst, &src) in owned_by_finer.iter_mut().zip(owned_mask.iter()) {
            *dst = *dst || src;
        }

        let mut owned: Vec<Polygon2> = Vec::new();
        let mut machining: Vec<Polygon2> = Vec::new();
        for comp in &kept {
            let mut island = vec![false; total];
            for &i in comp {
                if let Some(slot) = island.get_mut(i) {
                    *slot = true;
                }
            }
            let Ok(grid) = Grid2::from_vec(nx, ny, island) else {
                continue;
            };
            let base = region_polygons_from_mask_reported(
                &grid,
                map.origin_x,
                map.origin_y,
                cell,
                0.0,
                None,
            );
            let base_polys = base.polygons;
            // Overlap is grown PER ISLAND from the same mask, not by
            // re-extracting a dilated union — that is what keeps the island
            // count fixed and makes containment geometric rather than
            // hopeful. Clamped to coverage so the band reaches into coarser
            // territory and never off the part (`region_mask`'s D-16.1 stage
            // 1: a seed boundary that leaves the part is a pathological input
            // in its own right).
            if params.overlap_mm > 0.0 {
                let grown = region_polygons_from_mask_reported(
                    &grid,
                    map.origin_x,
                    map.origin_y,
                    cell,
                    params.overlap_mm,
                    Some(&covered),
                );
                machining.extend(grown.polygons);
            } else {
                machining.extend(base_polys.clone());
            }
            owned.extend(base_polys);
        }

        let owned_area_mm2: f64 = owned.iter().map(Polygon2::area).sum();
        sets.push(TierIslandSet {
            tier,
            islands: kept.len(),
            raw_island_count,
            owned: RegionSet::new(owned),
            machining: RegionSet::new(machining),
            owned_area_mm2,
            owned_cells,
            owned_mask,
            cap,
            close_radius_mm,
            min_region_area_mm2,
        });
    }

    // Built finest-first for the ownership rule; published coarse-first
    // because that is the order the op chain is emitted in.
    sets.reverse();

    Ok(TierIslands {
        per_tier: sets,
        cell_mm: map.cell_mm,
        tier_count: map.tier_count,
    })
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
    use crate::tier_map::ResidualTreatment;

    fn map_with(labels: Vec<u8>, nx: usize, ny: usize, tier_count: usize) -> TierMap {
        let len = labels.len();
        TierMap {
            nx,
            ny,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_mm: 0.5,
            labels,
            finest_z: vec![0.0f32; len],
            tier_count,
            tolerance_mm: 0.05,
            treatment: ResidualTreatment::Raw,
        }
    }

    #[test]
    fn coarseness_clamps_and_survives_nonsense() {
        assert!((clamp_coarseness(1.0) - 1.0).abs() < 1e-12);
        assert!((clamp_coarseness(f64::NAN) - 1.0).abs() < 1e-12);
        assert!((clamp_coarseness(-5.0) - COARSENESS_MIN).abs() < 1e-12);
        assert!((clamp_coarseness(1e9) - COARSENESS_MAX).abs() < 1e-12);
        assert!((clamp_coarseness(f64::INFINITY) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_derivation_matches_finish_planner_for_tool() {
        // The two derivations must not drift: this module's defaults ARE
        // `FinishPlannerParams::for_tool`'s, scaled by the slider.
        let cusp = 1.5;
        let theirs = crate::finish_planner::FinishPlannerParams::for_tool(cusp);
        let close = TierIslandParams::derived_close_radius_mm(cusp, 1.0);
        let area = TierIslandParams::derived_min_region_area_mm2(cusp, 1.0);
        assert!((close - theirs.close_radius_mm).abs() < 1e-12);
        assert!((area - theirs.min_region_area_mm2).abs() < 1e-12);
    }

    #[test]
    fn coarseness_scales_radius_linearly_and_area_quadratically() {
        let cusp = 2.0;
        let r1 = TierIslandParams::derived_close_radius_mm(cusp, 1.0);
        let r2 = TierIslandParams::derived_close_radius_mm(cusp, 2.0);
        let a1 = TierIslandParams::derived_min_region_area_mm2(cusp, 1.0);
        let a2 = TierIslandParams::derived_min_region_area_mm2(cusp, 2.0);
        assert!((r2 - 2.0 * r1).abs() < 1e-12);
        assert!((a2 - 4.0 * a1).abs() < 1e-12);
    }

    #[test]
    fn explicit_dials_are_taken_verbatim_at_every_coarseness() {
        let params = TierIslandParams {
            close_radius_mm: Some(0.7),
            min_region_area_mm2: Some(3.0),
            coarseness: 8.0,
            ..TierIslandParams::default()
        };
        assert!((params.effective_close_radius_mm(1.0) - 0.7).abs() < 1e-12);
        assert!((params.effective_min_region_area_mm2(1.0) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_ladder_length_mismatch_is_refused() {
        let map = map_with(vec![0u8; 100], 10, 10, 2);
        let err = extract_tier_islands(&map, &TierIslandParams::default(), &[1.0]);
        assert_eq!(
            err.err(),
            Some(TierIslandError::LadderMismatch {
                tier_count: 2,
                cusp_radii: 1,
            })
        );
    }

    #[test]
    fn a_malformed_map_is_refused_before_anything_else() {
        // 99 labels for a 10x10 grid: the shape contract is broken.
        let map = map_with(vec![0u8; 99], 10, 10, 2);
        assert!(matches!(
            extract_tier_islands(&map, &TierIslandParams::default(), &[2.0, 1.0]),
            Err(TierIslandError::MalformedMap { .. })
        ));

        let mut zero_cell = map_with(vec![0u8; 100], 10, 10, 2);
        zero_cell.cell_mm = 0.0;
        assert!(matches!(
            extract_tier_islands(&zero_cell, &TierIslandParams::default(), &[2.0, 1.0]),
            Err(TierIslandError::MalformedMap { .. })
        ));
    }

    #[test]
    fn an_all_coarse_map_yields_an_empty_but_present_tier_set() {
        let map = map_with(vec![0u8; 400], 20, 20, 2);
        let out = extract_tier_islands(&map, &TierIslandParams::default(), &[2.0, 1.0]).unwrap();
        assert_eq!(out.per_tier.len(), 1);
        assert_eq!(out.per_tier[0].tier, 1);
        assert_eq!(out.per_tier[0].raw_island_count, 0);
        assert_eq!(out.per_tier[0].islands, 0);
        assert!(!out.per_tier[0].cap.acted());
        assert!(out.is_empty());
    }

    #[test]
    fn per_tier_is_published_coarse_first() {
        let map = map_with(vec![0u8; 400], 20, 20, 4);
        let params = TierIslandParams::default();
        let radii = [4.0, 3.0, 2.0, 1.0];
        let out = extract_tier_islands(&map, &params, &radii).unwrap();
        let tiers: Vec<u8> = out.per_tier.iter().map(|s| s.tier).collect();
        assert_eq!(tiers, vec![1, 2, 3], "ascending, and never tier 0");
    }

    #[test]
    fn a_neutral_cap_report_reads_as_untouched() {
        let report = TierCapReport {
            islands_after_close: 3,
            islands_after_min_area: 3,
            kept: 3,
            cap: 24,
            close_raises: 0,
            first_close_radius_mm: 0.5,
            final_close_radius_mm: 0.5,
        };
        assert!(!report.acted());
        assert!(!report.truncated());
        assert_eq!(report.dropped(), 0);
        assert_eq!(report.absorbed_by_min_area(), 0);
    }

    #[test]
    fn a_truncating_cap_report_says_what_it_dropped() {
        let report = TierCapReport {
            islands_after_close: 566,
            islands_after_min_area: 80,
            kept: 24,
            cap: 24,
            close_raises: 3,
            first_close_radius_mm: 0.5,
            final_close_radius_mm: 0.5 * CAP_CLOSE_RAISE_FACTOR.powi(3),
        };
        assert!(report.acted());
        assert!(report.truncated());
        assert_eq!(report.dropped(), 56);
        assert_eq!(report.absorbed_by_min_area(), 486);
    }
}
