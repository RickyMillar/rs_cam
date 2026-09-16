//! Tier map → per-tier **island sets**: the layer that turns a raw tier label
//! grid into an operator-approvable set of regions.
//!
//! This is Phase I of the multi-tool island-finishing plan
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`). Its input is a
//! [`crate::maps::tier_map::TierMap`] — a per-cell "coarsest tool that holds this
//! cell" label — and its output is, per fine tier, a [`RegionSet`] a
//! `unified_finish` op can be confined to.
//!
//! # Why this layer exists at all
//!
//! Measured on the real wanaka mesh (2026-08-26, 0.3 mm cells, tolerance
//! 0.05, R2.0 → R1.0 ladder, [`crate::maps::tier_map::ResidualTreatment::SlopeCompensated`]):
//! the fine tier's territory is **22.0% of the board / 8,815 mm²**, and it
//! arrives as roughly **566 raw islands**. Handed to
//! [`crate::geometry::region_mask::region_polygons_from_mask`] as-is, 502 of those are
//! silently dropped by [`crate::geometry::region_mask::MAX_REST_REGIONS`] and the caller
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
//! [`crate::finish::finish_planner::decompose`] are the same three operations, reused
//! rather than re-implemented: [`crate::finish::finish_planner`]'s
//! `morphological_close` and `label_components` are the actual functions
//! called here, and the polygon extraction is
//! [`crate::geometry::region_mask::region_polygons_from_mask_reported`].
//!
//! # The 64-region cap does not apply here, and that is deliberate
//!
//! [`crate::geometry::region_mask::MAX_REST_REGIONS`] is a hard 64 with no caller-facing
//! dial, and it truncates *inside* the extractor. Rather than raise it (which
//! would change every existing caller's behaviour) or accept it (which would
//! reinstate the silent truncation this layer exists to remove), this module
//! **extracts one island at a time**: each call to
//! [`crate::geometry::region_mask::region_polygons_from_mask_reported`] is handed a mask
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
use std::fmt::Write as _;

use tracing::warn;

use crate::finish::finish_planner::{and_masks_in_place, label_components, morphological_close};
use crate::geo::P2;
use crate::geometry::grid_field::distance_transform_2d;
use crate::geometry::grid2::Grid2;
use crate::geometry::region_mask::region_polygons_from_mask_reported;
use crate::geometry::region_set::RegionSet;
use crate::maps::tier_map::{NO_TIER, TierMap};
use crate::polygon::Polygon2;

// ── Constants ───────────────────────────────────────────────────────────

/// Default seam-blend band (mm) each fine tier's islands are grown by, into
/// the coarser tier's territory. 2.0 is the number `unified_finish` already
/// uses between its own slope bands (`FinishPlannerParams::overlap_mm`'s
/// shipped value on the P2 chain), so a tier seam and a band seam blend at the
/// same scale.
pub const DEFAULT_OVERLAP_MM: f64 = 2.0;

/// Default per-tier island cap. Above this an operator cannot meaningfully
/// veto a preview, and every extra island is one more tool-down/tool-up cycle.
/// Deliberately far below [`crate::geometry::region_mask::MAX_REST_REGIONS`] (64): that
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
/// derivation [`crate::finish::finish_planner::FinishPlannerParams::for_tool`] uses.
pub const CLOSE_RADIUS_PER_CUSP_RADIUS: f64 = 0.5;

/// `min_region_area_mm2 = (2·cusp_radius)² · MIN_REGION_AREA_TOOL_DIAMETERS_SQ`
/// — "a few tool diameters²", the same derivation
/// [`crate::finish::finish_planner::FinishPlannerParams::for_tool`] uses.
pub const MIN_REGION_AREA_TOOL_DIAMETERS_SQ: f64 = 4.0;

/// Bound on `machining_area ÷ owned_area` above which
/// [`TierIslandSet::band_advisory`] speaks (G-OVERLAPFILL, 2026-09-09).
///
/// **The band is not defective and this is not a threshold on a defect.** The
/// overlap band exists to reach into the coarser tier's territory (see the
/// module doc, "Ownership is a partition; overlap bands are not"), and a hole
/// narrower than `2 · overlap_mm` closes under any correct dilation — that is
/// what a dilation is. What was missing is that nobody could SEE it: on the
/// wanaka board at tolerance 0.05 the fine tier owns 12 224 mm² in 10 islands
/// and machines 29 954 mm², 75 % of a 40 000 mm² board, because the coarse
/// tool's territory inside the valley network arrives as 1 329 slivers with a
/// median area of 3.6 mm² and a 1.25 mm band on each side closes any gap
/// under 2.5 mm (`planning/island_clip_2026-09-09/SPEC.md` §5).
///
/// 1.5 is a REPORTING bound, not a machining limit: below it the band is a
/// seam blend, above it the band is most of the territory and the operator
/// should know which dial put it there. Nothing refuses at this value.
pub const BAND_RATIO_ADVISORY_BOUND: f64 = 1.5;

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
/// [`crate::finish::finish_planner::FinishPlannerParams::for_tool`].
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
///
/// # Serialization
///
/// `Serialize`/`Deserialize` because
/// [`crate::compute::config::BoundarySource::PlannedTierRegions`] stores this
/// whole dial set in the project file as part of the tier-map recipe.
/// `#[serde(default)]` at the STRUCT level, not per field: it fills every
/// absent key from [`Default`] itself, so the file format's defaults and the
/// type's defaults cannot drift — the divergence class this repo has already
/// paid for twice (`intra_region_hookup_mm`'s core default vs its serde
/// default).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TierIslandParams {
    /// Morphological close radius (mm), or `None` to derive
    /// (`cusp_radius · `[`CLOSE_RADIUS_PER_CUSP_RADIUS`]`· coarseness`).
    /// Merges islands whose gap is inside the radius.
    ///
    /// Skipped when `None`: TOML has no null, and `toml` refuses to
    /// serialize one rather than inventing a spelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_radius_mm: Option<f64>,
    /// Minimum island area (mm²) worth giving this tier, or `None` to derive
    /// (`(2·cusp_radius)² · `[`MIN_REGION_AREA_TOOL_DIAMETERS_SQ`]`·
    /// coarseness²`). Measured on CELL COUNT, not polygon area — it is a
    /// population test on the mask, applied before extraction.
    ///
    /// Skipped when `None`; see [`Self::close_radius_mm`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// oversight.** [`crate::maps::tier_map`]'s module doc assigns this erosion to
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

    /// The close radius in force for a tier with this cusp radius: the
    /// explicit dial verbatim, or the derivation scaled by coarseness.
    #[must_use]
    pub(crate) fn effective_close_radius_mm(&self, cusp_radius_mm: f64) -> f64 {
        self.close_radius_mm.map_or_else(
            || Self::derived_close_radius_mm(cusp_radius_mm, self.coarseness),
            |v| v.max(0.0),
        )
    }

    /// The min island area in force for a tier with this cusp radius: the
    /// explicit dial verbatim, or the derivation scaled by coarseness².
    #[must_use]
    pub(crate) fn effective_min_region_area_mm2(&self, cusp_radius_mm: f64) -> f64 {
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
/// argument [`crate::geometry::region_mask::RegionCapReport`] makes for itself.
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

/// What the overlap band cost this tier in territory — the typed answer to
/// *"why is the fine tool cutting most of the board?"* (G-OVERLAPFILL).
///
/// Produced by [`TierIslandSet::band_advisory`] only when
/// `machining_area_mm2 ÷ owned_area_mm2` exceeds
/// [`BAND_RATIO_ADVISORY_BOUND`]. **An advisory is a reading, never a
/// refusal**: the band is doing what it is for, and both levers named in
/// [`Self::fmt`] are operator dials, not repairs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierBandAdvisory {
    /// Ladder index of the tier this reads.
    pub tier: u8,
    /// [`TierIslandSet::owned_area_mm2`].
    pub owned_area_mm2: f64,
    /// [`TierIslandSet::machining_area_mm2`].
    pub machining_area_mm2: f64,
    /// `machining ÷ owned`. Always > [`BAND_RATIO_ADVISORY_BOUND`] here.
    pub ratio: f64,
    /// The bound that was crossed, carried so a renderer never re-derives it.
    pub bound: f64,
    /// [`TierIslandParams::overlap_mm`] in force.
    pub overlap_mm: f64,
    /// Holes in the owned polygons — the coarse tool's slivers inside this
    /// tier's outline.
    pub owned_hole_count: usize,
    /// [`TierIslandSet::net_holes_closed_by_band`] — NET, and `0` does not
    /// mean the band left the holes alone. Read it against
    /// [`Self::machining_hole_count`].
    pub net_holes_closed_by_band: usize,
    /// [`TierIslandSet::machining_hole_count`] — holes AFTER the band. It can
    /// exceed [`Self::owned_hole_count`]; see
    /// [`TierIslandSet::net_holes_closed_by_band`].
    pub machining_hole_count: usize,
    /// [`TierIslandSet::median_owned_hole_area_mm2`].
    pub median_owned_hole_area_mm2: Option<f64>,
}

impl TierBandAdvisory {
    /// The overlap (mm) at which a hole of the median owned area would
    /// survive the band: half the square-root of that area, i.e. half a
    /// nominal sliver width. `None` when the tier owns no hole.
    ///
    /// A square-root width is a PROXY — a sliver is long and thin, so its
    /// true width is under `sqrt(area)` and this reads optimistic. It is
    /// quoted as a starting dial, never as a guarantee.
    #[must_use]
    pub fn overlap_that_keeps_the_median_hole_mm(&self) -> Option<f64> {
        self.median_owned_hole_area_mm2
            .filter(|a| a.is_finite() && *a > 0.0)
            .map(|a| a.sqrt() * 0.5)
    }
}

impl fmt::Display for TierBandAdvisory {
    /// Names the measurement, then the two dials. Deliberately does NOT name
    /// a tolerance number: this layer is handed a label grid and cusp radii,
    /// never the tolerance the map was walked at or the coarse pass's own
    /// cusp height, and inventing either would be a number the operator
    /// could not reproduce.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tier {}: the {:.2} mm overlap band grows {:.0} mm² of owned territory into \
             {:.0} mm² of machining territory ({:.2}× the bound of {:.2}×), taking the \
             tier's holes from {} to {}",
            self.tier,
            self.overlap_mm,
            self.owned_area_mm2,
            self.machining_area_mm2,
            self.ratio,
            self.bound,
            self.owned_hole_count,
            self.machining_hole_count,
        )?;
        if let Some(median) = self.median_owned_hole_area_mm2 {
            write!(f, " (median hole {median:.1} mm²)")?;
        }
        write!(
            f,
            ". A band closes every hole narrower than 2 × overlap, so the coarse tool's \
             slivers inside this tier fall to the fine tool. Two dials move it: lower \
             `overlap_mm`"
        )?;
        if let Some(keep) = self.overlap_that_keeps_the_median_hole_mm() {
            write!(f, " toward {keep:.2} mm (half a median sliver width)")?;
        }
        write!(
            f,
            ", or raise the plan's `tolerance_mm` toward the coarse pass's own cusp height \
             so fewer slivers are claimed at all."
        )
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
    /// subtracted).
    ///
    /// **This is territory AFTER the morphological close, not the tier's raw
    /// labels.** When the cap's bounded auto-raise fires, the close radius
    /// reaches `first · 1.5^raises` and welds a dendritic network into slabs,
    /// which this then measures. On the wanaka map at tolerance 0.146
    /// (instrument `tests/tier_band_overlap_g_overlapfill.rs`) three raises
    /// took the radius to 1.688 mm and the owned area from **2 261 mm² to
    /// 17 812 mm²** — 7.9×, and enough to make owned area NON-MONOTONIC in
    /// the plan tolerance. Read [`TierCapReport::close_raises`] alongside it.
    ///
    /// Close to but not identical with `owned_cells · cell²`:
    /// marching squares treats grid cells as CORNERS and cuts at edge
    /// midpoints, so a solid rectangle traces to exactly its `n · cell` extent
    /// while a ragged or diagonal boundary trades half-cells either way.
    pub owned_area_mm2: f64,
    /// Sum of [`Self::machining`] polygon areas (mm², holes subtracted) —
    /// what this tier's tool actually sweeps, band included.
    ///
    /// **An upper bound where two islands of the SAME tier are closer than
    /// `2 · overlap_mm`**: their bands then overlap each other and the sum
    /// counts the shared strip twice. Ownership partitions and can be summed
    /// exactly; a band set cannot. See [`Self::machining_to_owned_ratio`].
    pub machining_area_mm2: f64,
    /// Holes in [`Self::owned`] — the coarser tool's slivers enclosed by this
    /// tier's outlines, before the band.
    pub owned_hole_count: usize,
    /// Holes in [`Self::machining`] — the slivers that SURVIVED the band.
    /// A hole narrower than `2 · overlap_mm` closes; see
    /// [`BAND_RATIO_ADVISORY_BOUND`].
    pub machining_hole_count: usize,
    /// Median area (mm²) of the holes in [`Self::owned`], or `None` when this
    /// tier owns no hole. The scale of the coarse tool's slivers, and the
    /// number [`TierBandAdvisory::overlap_that_keeps_the_median_hole_mm`]
    /// turns into a dial.
    pub median_owned_hole_area_mm2: Option<f64>,
    /// [`TierIslandParams::overlap_mm`] the band was grown at, carried so a
    /// consumer that has the set but not the params can still name the dial.
    pub overlap_mm: f64,
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

impl TierIslandSet {
    /// NET holes the overlap band removed — `owned_hole_count` minus
    /// `machining_hole_count`, floored at zero.
    ///
    /// **A band both closes AND creates holes, so read the two counts, not
    /// only this difference.** It closes a hole narrower than
    /// `2 · overlap_mm`, and merges two holes whose separating wall is
    /// narrower than the band (one closure). It CREATES one whenever the
    /// dilation seals the mouth of a concave bay, or wraps around ground the
    /// coverage clamp keeps out — the bay then encloses as a hole it was not
    /// before. Measured on the wanaka map at tolerance 0.146 (the instrument
    /// `tests/tier_band_overlap_g_overlapfill.rs`): 155 owned holes became
    /// **255** machining holes, and this reads `0`.
    ///
    /// So a `0` here means "creation matched or beat closure", never "the
    /// band left the holes alone". For that, compare the two counts.
    #[must_use]
    pub const fn net_holes_closed_by_band(&self) -> usize {
        self.owned_hole_count
            .saturating_sub(self.machining_hole_count)
    }

    /// `machining_area_mm2 ÷ owned_area_mm2`, or `None` when the tier owns no
    /// area — the ratio has no meaning against a zero denominator, and
    /// coercing it to 1.0 would read as a healthy band.
    ///
    /// `1.0` is the ratio with the band off. Above
    /// [`BAND_RATIO_ADVISORY_BOUND`] the band is most of the territory; see
    /// [`Self::band_advisory`].
    #[must_use]
    pub fn machining_to_owned_ratio(&self) -> Option<f64> {
        (self.owned_area_mm2 > 0.0 && self.owned_area_mm2.is_finite())
            .then(|| self.machining_area_mm2 / self.owned_area_mm2)
    }

    /// The typed reading when the band grew this tier past
    /// [`BAND_RATIO_ADVISORY_BOUND`], or `None` on a healthy tier.
    ///
    /// `None` is the clean answer here, not "not measured": every tier
    /// computes the areas, so a consumer that wants the raw numbers reads the
    /// fields.
    #[must_use]
    pub fn band_advisory(&self) -> Option<TierBandAdvisory> {
        let ratio = self.machining_to_owned_ratio()?;
        (ratio.is_finite() && ratio > BAND_RATIO_ADVISORY_BOUND).then_some(TierBandAdvisory {
            tier: self.tier,
            owned_area_mm2: self.owned_area_mm2,
            machining_area_mm2: self.machining_area_mm2,
            ratio,
            bound: BAND_RATIO_ADVISORY_BOUND,
            overlap_mm: self.overlap_mm,
            owned_hole_count: self.owned_hole_count,
            net_holes_closed_by_band: self.net_holes_closed_by_band(),
            machining_hole_count: self.machining_hole_count,
            median_owned_hole_area_mm2: self.median_owned_hole_area_mm2,
        })
    }
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

    /// Total territory the fine tools actually SWEEP (mm²), summed over
    /// `machining`.
    ///
    /// Read [`TierIslandSet::machining_area_mm2`]'s doc before comparing this
    /// with [`Self::total_owned_area_mm2`]: ownership partitions and sums
    /// exactly, band sets do not, so this is an upper bound wherever two
    /// bands touch — within one tier and across tiers.
    #[must_use]
    pub fn total_machining_area_mm2(&self) -> f64 {
        self.per_tier.iter().map(|s| s.machining_area_mm2).sum()
    }

    /// Every tier whose [`TierCapReport::acted`] is true — what a preview
    /// panel must tell the operator was merged or dropped.
    pub fn tiers_where_the_cap_acted(&self) -> impl Iterator<Item = &TierIslandSet> {
        self.per_tier.iter().filter(|s| s.cap.acted())
    }

    /// Every tier whose overlap band grew it past
    /// [`BAND_RATIO_ADVISORY_BOUND`] — what a preview panel must tell the
    /// operator about G-OVERLAPFILL. Empty on a healthy plan.
    pub fn band_advisories(&self) -> impl Iterator<Item = TierBandAdvisory> + '_ {
        self.per_tier
            .iter()
            .filter_map(TierIslandSet::band_advisory)
    }
}

// ── Extraction ──────────────────────────────────────────────────────────

/// Turn a [`TierMap`] into per-tier island sets. See the module doc for the
/// pipeline and for what ownership does and does not guarantee.
///
/// `cusp_radii` is the ladder's per-tier **cusp** (tip) radii in mm, coarse
/// first — the same ordering and the same radius
/// [`crate::maps::tier_map::TierLadder`] sorts on
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

        // G-OVERLAPFILL instrumentation. `Polygon2::area` already subtracts
        // holes and `detect_containment` has grouped every hole under its
        // own exterior, so these three lines measure exactly what the peer's
        // SVG script measured off the preview: outline, holes, net.
        let owned_area_mm2: f64 = owned.iter().map(Polygon2::area).sum();
        let machining_area_mm2: f64 = machining.iter().map(Polygon2::area).sum();
        let owned_hole_count: usize = owned.iter().map(|p| p.holes.len()).sum();
        let machining_hole_count: usize = machining.iter().map(|p| p.holes.len()).sum();
        let median_owned_hole_area_mm2 = median_hole_area_mm2(&owned);

        sets.push(TierIslandSet {
            tier,
            islands: kept.len(),
            raw_island_count,
            owned: RegionSet::new(owned),
            machining: RegionSet::new(machining),
            owned_area_mm2,
            machining_area_mm2,
            owned_hole_count,
            machining_hole_count,
            median_owned_hole_area_mm2,
            overlap_mm: params.overlap_mm.max(0.0),
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

/// Median hole area (mm²) over a polygon list, or `None` when it holds no
/// hole. The MEDIAN and not the mean: a valley network's slivers are a long
/// tail, and one 500 mm² pocket among a thousand 3 mm² slivers would move a
/// mean far enough to name the wrong dial.
///
/// Even counts take the LOWER of the two middles rather than their average,
/// so the answer is always the area of a hole that actually exists.
fn median_hole_area_mm2(polys: &[Polygon2]) -> Option<f64> {
    let mut areas: Vec<f64> = polys
        .iter()
        .flat_map(|p| p.holes.iter())
        .map(|h| crate::polygon::shoelace_area(h).abs())
        .filter(|a| a.is_finite())
        .collect();
    if areas.is_empty() {
        return None;
    }
    areas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    areas.get((areas.len() - 1) / 2).copied()
}

// ── SVG preview ─────────────────────────────────────────────────────────

/// Per-tier fill colours, cycled by ladder index. A fixed table rather than a
/// generated hue ramp so two renders of the same plan carry the same colours
/// and an operator can compare two previews by eye.
const TIER_SVG_COLORS: [&str; 6] = [
    "#ff9800", "#42a5f5", "#66bb6a", "#ab47bc", "#ef5350", "#ffee58",
];

/// Background, matching `finish_planner::planned_regions_to_svg`.
const TIER_SVG_BACKGROUND: &str = "#1a1a2e";

/// Decimal places every coordinate is written at. Three is one micron on a
/// millimetre grid — below the cell size any plan resolution is allowed to
/// use, and fixed so the output is byte-stable across runs.
const TIER_SVG_DECIMALS: usize = 3;

fn tier_svg_color(tier: u8) -> &'static str {
    let n = TIER_SVG_COLORS.len();
    TIER_SVG_COLORS
        .get(usize::from(tier) % n)
        .copied()
        .unwrap_or("#ffffff")
}

/// Compact SVG of the per-tier islands: one `<g>` per fine tier with a fill
/// per tier, `owned` polygons filled, the `machining` outline stroked so the
/// overlap band is visible; `viewBox` = the tier map's XY extent, in
/// millimetres.
///
/// # Why this and not the interactive HTML path
///
/// This is the agent-visible twin of the GUI preview overlay (plan Phase U
/// item 2). The existing interactive HTML dump measured **948 MB** on this
/// board (T3 §7), because it carries geometry per sample. This carries
/// POLYGONS ONLY — never a rect per cell — so a 64-island board renders in
/// tens of KB and the file can be read back by the agent that asked for it.
///
/// # Conventions
///
/// Follows [`crate::finish::finish_planner::planned_regions_to_svg`]: same header
/// shape, same background, `evenodd` so holes render, and the Y axis flipped
/// so north is up. It DIVERGES in one place — the viewBox is the map's own
/// millimetre extent rather than a pixel canvas, so stroke widths and dash
/// lengths below are in mm and a reader can measure the picture.
///
/// The extent is the map's **areal** extent: cell centres span
/// `(nx − 1) · cell`, and a half cell is added on each side because a cell is
/// an area, not a point. Marching-squares vertices sit at cell-edge midpoints,
/// which is exactly that boundary.
///
/// Deterministic: fixed colour table, fixed decimal count, `per_tier` order
/// as published (coarse first), polygons in extraction order.
///
/// A tier that kept no island emits no group — an empty
/// [`TierIslands`] therefore renders a valid, group-less SVG rather than
/// nothing at all, because "the coarse tool holds the whole board" is a
/// planning outcome an operator needs to SEE.
#[must_use]
pub fn tier_islands_to_svg(map: &TierMap, islands: &TierIslands) -> String {
    const EMPTY: &str = "<svg xmlns='http://www.w3.org/2000/svg'/>";
    let d = TIER_SVG_DECIMALS;
    let cell = map.cell_mm;
    if !cell.is_finite() || cell <= 0.0 || map.nx == 0 || map.ny == 0 {
        return String::from(EMPTY);
    }
    let half = cell * 0.5;
    let minx = map.origin_x - half;
    let miny = map.origin_y - half;
    let w = map.nx as f64 * cell;
    let h = map.ny as f64 * cell;
    if !minx.is_finite() || !miny.is_finite() || !w.is_finite() || !h.is_finite() {
        return String::from(EMPTY);
    }
    // SVG Y grows downward, world Y grows upward: `flip - y` reflects the
    // picture about the viewBox's own mid-line, which puts north at the top
    // without a transform a reader would have to unwind.
    let flip = 2.0 * miny + h;
    // Hairlines in mm: half a cell reads as a line at any plan resolution.
    let stroke = (cell * 0.5).max(0.05);
    let dash = cell * 2.0;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.d$}mm' height='{h:.d$}mm' \
         viewBox='{minx:.d$} {miny:.d$} {w:.d$} {h:.d$}'>"
    );
    let _ = writeln!(
        svg,
        "<title>Multi-tool tier preview: {} tiers, {} islands, {:.1} mm2 fine territory, \
         cell {:.d$} mm</title>",
        islands.tier_count,
        islands.total_islands(),
        islands.total_owned_area_mm2(),
        islands.cell_mm,
    );
    let _ = writeln!(
        svg,
        "<rect x='{minx:.d$}' y='{miny:.d$}' width='{w:.d$}' height='{h:.d$}' \
         fill='{TIER_SVG_BACKGROUND}'/>"
    );

    for set in &islands.per_tier {
        if set.owned.is_empty() && set.machining.is_empty() {
            continue;
        }
        let color = tier_svg_color(set.tier);
        let tier = set.tier;
        let _ = writeln!(svg, "<g id='tier-{tier}' fill='{color}' stroke='{color}'>");
        let _ = writeln!(
            svg,
            "<title>Tier {tier}: {} islands of {} raw, {:.1} mm2 owned, close radius \
             {:.d$} mm, min island {:.1} mm2{}</title>",
            set.islands,
            set.raw_island_count,
            set.owned_area_mm2,
            set.close_radius_mm,
            set.min_region_area_mm2,
            if set.cap.acted() { ", CAP ACTED" } else { "" },
        );
        for poly in set.owned.as_slice() {
            let path = tier_polygon_svg_path(poly, flip);
            if path.is_empty() {
                continue;
            }
            let _ = writeln!(
                svg,
                "<path d='{path}' fill-opacity='0.35' fill-rule='evenodd' \
                 stroke-width='{stroke:.d$}'/>"
            );
        }
        // Dashed and unfilled: the band between this and the solid fill IS
        // the overlap reaching into coarser territory. Identical to the
        // owned outline when `overlap_mm` is 0.0, which is the honest
        // rendering of a dial that is off.
        for poly in set.machining.as_slice() {
            let path = tier_polygon_svg_path(poly, flip);
            if path.is_empty() {
                continue;
            }
            let _ = writeln!(
                svg,
                "<path d='{path}' fill='none' stroke-width='{stroke:.d$}' \
                 stroke-dasharray='{dash:.d$} {dash:.d$}'/>"
            );
        }
        let _ = writeln!(svg, "</g>");
    }

    let _ = writeln!(svg, "</svg>");
    svg
}

/// Exterior + hole subpaths of one polygon, Y-flipped about `flip`.
fn tier_polygon_svg_path(poly: &Polygon2, flip: f64) -> String {
    let mut d = String::new();
    tier_ring_svg_subpath(&poly.exterior, flip, &mut d);
    for hole in &poly.holes {
        tier_ring_svg_subpath(hole, flip, &mut d);
    }
    d
}

/// One closed subpath. Rings under three vertices enclose no area and are
/// skipped: a renderer draws nothing for them either way, and emitting them
/// would put bytes in the file that say nothing.
fn tier_ring_svg_subpath(ring: &[P2], flip: f64, out: &mut String) {
    if ring.len() < 3 {
        return;
    }
    let d = TIER_SVG_DECIMALS;
    for (i, p) in ring.iter().enumerate() {
        let (x, y) = (p.x, flip - p.y);
        // No separator after the command letter: the letter IS one, and on a
        // marching-squares ring that saves a byte per vertex.
        if i == 0 {
            let _ = write!(out, "M{x:.d$} {y:.d$}");
        } else {
            let _ = write!(out, "L{x:.d$} {y:.d$}");
        }
    }
    out.push('Z');
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
    use crate::maps::tier_map::ResidualTreatment;

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
        let theirs = crate::finish::finish_planner::FinishPlannerParams::for_tool(cusp);
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

    /// A 40x40 grid at 0.5 mm with one tier-2 blob and one tier-1 blob, both
    /// clear of the grid-edge ring the extractor drops.
    fn two_fine_tier_map() -> TierMap {
        let (nx, ny) = (40usize, 40usize);
        let labels: Vec<u8> = (0..nx * ny)
            .map(|i| {
                let (r, c) = (i / nx, i % nx);
                if (5..12).contains(&r) && (5..12).contains(&c) {
                    2
                } else if (25..35).contains(&r) && (25..35).contains(&c) {
                    1
                } else {
                    0
                }
            })
            .collect();
        map_with(labels, nx, ny, 3)
    }

    fn svg_fixture_params() -> TierIslandParams {
        TierIslandParams {
            // Explicit so the fixture's islands survive verbatim: the
            // derivation from a 1.0 mm cusp radius would floor a 12 mm2 blob
            // out of existence.
            close_radius_mm: Some(0.0),
            min_region_area_mm2: Some(0.0),
            overlap_mm: 1.0,
            ..TierIslandParams::default()
        }
    }

    #[test]
    fn tier_islands_to_svg_smoke() {
        let map = two_fine_tier_map();
        let params = svg_fixture_params();
        let islands = extract_tier_islands(&map, &params, &[2.0, 1.0, 0.5]).unwrap();
        assert_eq!(islands.total_islands(), 2, "one blob per fine tier");

        let svg = tier_islands_to_svg(&map, &islands);
        assert!(!svg.is_empty());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.ends_with("</svg>\n"));
        assert_eq!(
            svg.matches("<g ").count(),
            2,
            "one group per fine tier that kept an island"
        );
        assert!(svg.contains("<g id='tier-1'"));
        assert!(svg.contains("<g id='tier-2'"));
        // Self-describing: a per-group title carrying tier + area.
        assert_eq!(svg.matches("<title>Tier ").count(), 2);
        assert!(svg.contains("mm2 owned"));
        // The viewBox is the map's areal extent (40 cells x 0.5 mm), not a
        // pixel canvas.
        assert!(svg.contains("viewBox='-0.250 -0.250 20.000 20.000'"));
        // Overlap band visible as a stroked, unfilled outline.
        assert!(svg.contains("stroke-dasharray="));
        assert!(svg.contains("fill='none'"));
        // Polygons only — never a rect per cell. One background rect.
        assert_eq!(svg.matches("<rect").count(), 1);
        assert!(
            svg.len() < 64 * 1024,
            "a 2-island preview must be KB, not MB: {} bytes",
            svg.len()
        );
    }

    #[test]
    fn tier_islands_to_svg_is_byte_stable_across_runs() {
        let map = two_fine_tier_map();
        let params = svg_fixture_params();
        let a = extract_tier_islands(&map, &params, &[2.0, 1.0, 0.5]).unwrap();
        let b = extract_tier_islands(&map, &params, &[2.0, 1.0, 0.5]).unwrap();
        assert_eq!(tier_islands_to_svg(&map, &a), tier_islands_to_svg(&map, &b));
    }

    #[test]
    fn empty_islands_render_a_valid_svg_with_no_groups() {
        // "The coarse tool holds the whole board" is a planning outcome, not
        // an error, and the operator has to be able to see it.
        let map = map_with(vec![0u8; 400], 20, 20, 2);
        let params = TierIslandParams::default();
        let islands = extract_tier_islands(&map, &params, &[2.0, 1.0]).unwrap();
        assert!(islands.is_empty());

        let svg = tier_islands_to_svg(&map, &islands);
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.ends_with("</svg>\n"));
        assert_eq!(svg.matches("<g ").count(), 0);
        assert!(svg.contains("<rect"), "the extent still renders");

        // And a `TierIslands` carrying no tier rows at all.
        let none = TierIslands {
            per_tier: Vec::new(),
            cell_mm: map.cell_mm,
            tier_count: 2,
        };
        let svg = tier_islands_to_svg(&map, &none);
        assert!(svg.starts_with("<svg xmlns="));
        assert_eq!(svg.matches("<g ").count(), 0);
    }

    #[test]
    fn a_degenerate_map_renders_an_empty_svg_rather_than_nan_coordinates() {
        let islands = TierIslands {
            per_tier: Vec::new(),
            cell_mm: 0.5,
            tier_count: 2,
        };
        let mut zero_cell = map_with(vec![0u8; 400], 20, 20, 2);
        zero_cell.cell_mm = 0.0;
        assert_eq!(
            tier_islands_to_svg(&zero_cell, &islands),
            "<svg xmlns='http://www.w3.org/2000/svg'/>"
        );

        let no_cells = map_with(Vec::new(), 0, 0, 2);
        assert_eq!(
            tier_islands_to_svg(&no_cells, &islands),
            "<svg xmlns='http://www.w3.org/2000/svg'/>"
        );
    }

    #[test]
    fn tier_colors_are_stable_and_cycle() {
        assert_eq!(tier_svg_color(0), TIER_SVG_COLORS[0]);
        assert_eq!(tier_svg_color(1), TIER_SVG_COLORS[1]);
        assert_eq!(
            tier_svg_color(6),
            TIER_SVG_COLORS[0],
            "the table cycles rather than falling off the end"
        );
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
