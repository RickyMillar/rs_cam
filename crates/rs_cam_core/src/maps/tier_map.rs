//! Multi-tool **tier map** — one grid walk that labels every cell with the
//! COARSEST tool on a ladder that can hold the surface there.
//!
//! This is task T1 of the multi-tool island-finishing plan
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase T), and it is
//! the *n*-tool generalisation of the two-tool residual that
//! [`crate::surface::rest_field::detect_rest_valleys`] computes
//! (`rest = drop_z(reference) − drop_z(fine)`).
//!
//! ```text
//! residual_k(x, y) = drop_z(tool_k, x, y) − drop_z(finest, x, y)
//! label(x, y)      = min { k : residual_k ≤ tolerance }
//! ```
//!
//! The ladder is ordered **coarse → fine**, so the smallest passing index is
//! the biggest tool that does the job, which is what makes the map an
//! economic statement and not just a geometric one.
//!
//! # G1 — one index query serves the whole ladder
//!
//! [`crate::surface::dropcutter::point_drop_cutter`] runs
//! `index.query(x, y, cutter.radius())` per call, so an *n*-tool map done
//! naively is *n* queries per cell. The largest ladder tool's query window is
//! a **superset** of every smaller tool's, and
//! [`crate::tool::drop_cutter_can_contact`] then prunes per tool by that
//! tool's own envelope — the same prune `point_drop_cutter` applies to its own
//! (already over-inclusive) candidate list. So one query at the ladder's
//! largest envelope radius, filtered per tool, is **bit-identical** to *n*
//! separate queries; `tests/tier_map_walk_t1.rs::one_max_radius_query_
//! reproduces_point_drop_cutter_exactly` asserts that rather than assuming it.
//!
//! Two facts make the identity sound rather than lucky:
//!
//! 1. every drop reaches `cl` through [`crate::tool::CLPoint::update_z`],
//!    which is a strict max, so the final Z is independent of the order the
//!    candidates arrive in — and a bigger query changes only the order and the
//!    rejected tail;
//! 2. `drop_cutter_can_contact`'s XY reject is a pure distance test against
//!    the tool's envelope, so triangles pulled in by the larger window and
//!    unreachable by a smaller tool are rejected before any contact math.
//!
//! The walk additionally stops at the **first** (coarsest) tool that passes,
//! so a cell of flat ground costs two drops (the finest reference plus the
//! coarsest candidate) however long the ladder is.
//!
//! # Cost and memory
//!
//! Per-tool full-grid drop-cutter cost on the wanaka board (200 × 200 mm,
//! 661 k triangles) is measured/extrapolated in
//! `planning/multitool_2026-08-23/T1_FINDINGS.md` §1.3 at **≈ 8 s at 0.6 mm,
//! ≈ 31 s at 0.3 mm, ≈ 125 s at 0.15 mm**. Plan tiers at 0.3–0.6 mm; 0.15 mm
//! is not affordable as a naive sweep.
//!
//! Memory is deliberately **5 B/cell** — one `u8` label plus one `f32`
//! finest-tool drop — against the 49 B/cell a `FinishSurface`
//! (`SurfaceHeightmap` + `SlopeMap`) costs. Storing a per-tool drop plane
//! instead would be `4·n` B/cell, i.e. 4× worse at a three-tool ladder, and
//! this board already OOMs simulation at 0.1 mm cells. At 0.3 mm over wanaka
//! (473 k cells) the map is **2.4 MB**; the same grid as five cached
//! per-tool `FinishSurface`s would be 463 MB.
//!
//! # What this module deliberately does NOT do
//!
//! * **No boundary erosion.** Within roughly one envelope radius of the part
//!   edge a big tool hangs off and rests on the rim, reading a false-high
//!   residual, so the rim reads as fine-tier territory.
//!   [`crate::surface::rest_field`] erodes that band with a chamfer distance transform
//!   over its contact mask; the equivalent input here is
//!   [`TierMap::covered_mask`], and the erosion belongs to the consumer
//!   (Phase I) so that the map itself stays a measurement rather than a
//!   policy.
//! * **No slope-bias compensation by DEFAULT.** [`ResidualTreatment::Raw`] is
//!   the default and is deliberately untreated — see the next section for the
//!   bias, and for the opt-in analytic treatment that removes it.
//!
//! # The slope bias, and the analytic treatment (T2)
//!
//! The drop-cutter Z is the tool-CENTRE offset surface, not the machined
//! surface. A tip sphere of radius `R` resting on a plane of slope θ leaves
//! its reference point `R·(sec θ − 1)` above that plane, so the residual
//! between two ladder tools on a plain sloped plane — which **either** ball
//! machines perfectly — is
//!
//! ```text
//! bias_k(θ) = (R_k − R_finest) · (sec θ − 1)
//! ```
//!
//! **0.62 mm at 45°** for R2 against R0.5 and 1.5 mm at 60° (T1 §3.4): one to
//! two orders of magnitude above any sane tolerance. Untreated, *every slope
//! on a terrain reads as fine-tier*, which is the measured cause of the T4
//! two-tier arm losing 5.5 h — the fine tool was charged for the whole
//! mid-steep band (plan §0, `T1_FINDINGS.md` §3.4).
//!
//! [`ResidualTreatment::SlopeCompensated`] subtracts that term before the
//! tolerance comparison. Four things it is, and is not:
//!
//! 1. **θ is measured on the REFERENCE tool's own drop surface**, by central
//!    finite difference over the [`TierMap::finest_z`] plane the walk already
//!    computes — not from probe triangle normals and not from a second
//!    `SurfaceHeightmap`. The offset surface of a plane is parallel to that
//!    plane, so on the fixture the law is written for the two agree exactly;
//!    on curvature the CL surface is the smoother of the two, which
//!    under-states θ and therefore under-compensates — erring toward the fine
//!    tier, the safe direction. It also costs no extra drops, no extra
//!    allocation per cell and no trig in the hot loop
//!    (`slope_bias_scale` compares `|∇z|²` against a squared cap).
//!    Its failure modes are stated at `reference_gradient`.
//! 2. **The law is the SPHERICAL-tip law.** A flat or bull tip on a slope
//!    carries an extra `(R_envelope − R_cusp)·tan θ` term this treatment does
//!    not model, and a tapered ball past its half-angle contacts the cone
//!    rather than the tip sphere and sits LOWER than the sphere law predicts.
//!    Both mismatches are under-compensations while the finest tool is the
//!    non-spherical one (the shipped case — the Ø1-tip taper is the fine
//!    tool), i.e. they err toward the fine tier. A ladder with a flat or
//!    tapered tool in a COARSE slot is the arm that could over-compensate;
//!    that is not a shipped ladder and is not covered.
//! 3. **Above [`MAX_COMPENSATED_SLOPE_DEG`] it abstains** and the raw residual
//!    is compared, rather than clamping `sec θ`. See that constant.
//! 4. **It is not free.** The compensated arm is a two-pass walk (reference
//!    plane, then verdicts), because a central difference needs the row below
//!    a cell before that cell can be judged. Same total drop-cutter work as
//!    [`ResidualTreatment::Raw`], one extra `SpatialIndex::query` per *owned*
//!    cell, and a transient `f64` reference plane (8 B/cell, released before
//!    the map is returned) on top of the 5 B/cell the map itself costs.
//!    [`ResidualTreatment::Raw`] keeps its single-pass, one-query-per-cell
//!    shape untouched.
//!
//! # Cancellation
//!
//! The walk polls the cancel token **once per grid row**, in *both* passes —
//! [`crate::maps::grid::walk_rows`] is the one site that does it, so the
//! granularity cannot drift between the two arms, between the parallel and
//! serial builds, or between this map and [`crate::maps::reach_map`].
//! `rest_field`'s walk has no polling at all, which is why a rest analysis on
//! a big board cannot be interrupted; this one can.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::interrupt::{CancelCheck, Cancelled};
use crate::maps::grid::{GridSpec, walk_rows};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::dropcutter::point_is_over_mesh_xy;
use crate::tool::{CLPoint, MillingCutter, drop_cutter_can_contact};

/// Label for a cell no ladder tool owns: off the part, or outside the mesh
/// footprint entirely. Reserved, so a ladder may carry at most
/// [`MAX_TIERS`] tools.
pub const NO_TIER: u8 = u8::MAX;

/// Maximum ladder length. Labels are one byte and [`NO_TIER`] is reserved.
pub const MAX_TIERS: usize = 255;

/// Cumulative count of drop-cutter evaluations this module has performed,
/// since process start.
///
/// The instrument for T3: a cache hit must move this by **zero**. It is
/// updated once per grid row (not once per drop), so it is a measurement of
/// work done, not a contended hot-loop counter.
static DROP_CALLS: AtomicU64 = AtomicU64::new(0);

/// Read the cumulative drop-cutter work counter — the T3 instrument.
#[must_use]
pub fn drop_call_count() -> u64 {
    DROP_CALLS.load(Ordering::Relaxed)
}

/// Zero the drop-cutter work counter, for harnesses that want a per-run
/// delta. Touches no cached value, so it cannot change any result.
///
/// **Test door.** The harnesses
/// `crates/rs_cam_core/tests/tier_map_slope_t2.rs` and
/// `crates/rs_cam_core/tests/tier_map_cache_t3.rs` are the only callers. No
/// production path reads it, so it sits behind `test-support` (FLD-05).
#[cfg(feature = "test-support")]
pub fn reset_drop_call_count() {
    DROP_CALLS.store(0, Ordering::Relaxed);
}

/// How the raw tool-vs-tool residual is turned into the number the tolerance
/// is compared against.
///
/// This exists as an enum rather than a closure because it is part of the
/// [`crate::maps::tier_map_cache`] key: a slope-compensated map and a raw one over
/// the same mesh, ladder and grid are different answers, and a memo that
/// could not tell them apart would serve one for the other.
///
/// Both variants are payload-free on purpose: every dial they could carry
/// would have to be `to_bits`-keyed in [`crate::maps::tier_map_cache`], and a
/// discriminant cannot be got wrong. The compensation cap is therefore a
/// module constant ([`MAX_COMPENSATED_SLOPE_DEG`]), not a field.
/// It is `Serialize`/`Deserialize` because
/// [`crate::compute::config::BoundarySource::PlannedTierRegions`] stores the
/// tier-map RECIPE in the project file, and a recipe that omitted the
/// treatment would silently re-plan a slope-compensated boundary as a raw one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualTreatment {
    /// `drop_z(tool_k) − drop_z(finest)`, untreated. Honest, and biased on
    /// slopes by `(R_k − R_finest)·(sec θ − 1)` — see the module doc.
    #[default]
    Raw,
    /// [`Self::Raw`] minus the analytic tool-centre-offset bias
    /// `(R_k − R_finest)·(sec θ − 1)`, with `R` the tip-sphere
    /// ([`MillingCutter::cusp_radius_mm`]) radius and θ the slope of the
    /// reference tool's own drop surface at the cell.
    ///
    /// This is the arm the plan calls "analytic compensation"
    /// (`ORCHESTRATION_PLAN.md` Phase T task T2 (a), blocker B1); the
    /// alternative arm is a stock-referenced residual, which needs the coarse
    /// tier generated and simulated first and is therefore a cascade rather
    /// than a plan-time oracle. Read the module doc's T2 section for what the
    /// compensation models, what it does not, and which direction each
    /// mismatch errs in.
    ///
    /// On a plane of any slope inside the cap this reads ~0 for every ladder
    /// tool, so the plane is claimed by the coarsest — which is the point.
    SlopeCompensated,
}

/// Slope (degrees from horizontal) above which
/// [`ResidualTreatment::SlopeCompensated`] **abstains**: the cell is judged on
/// its raw residual instead, exactly as [`ResidualTreatment::Raw`] would. The
/// comparison is strict (`>`), so the cap angle itself is still compensated.
///
/// The number is the shipped `waterline_threshold_deg` default — the slope at
/// which `finish_planner` hands a surface to the very-steep waterline band
/// (`FinishPlannerParams::for_tool`, `compute/operation_configs.rs`). Three
/// reasons the treatment stops there rather than clamping `sec θ`:
///
/// 1. **`sec` is unusable as an estimator near vertical.** `d(sec θ)/dθ =
///    sec θ · tan θ` is 0.25 per degree at 75° but 2.3 per degree at 85°, so a
///    one-degree slope error there moves the subtracted term by `2.3·ΔR` mm.
///    A compensated residual on a near-vertical wall is not a measurement.
/// 2. **Clamping would hand walls to the COARSE tool.** A clamped `sec θ` still
///    subtracts its full capped term — 15.7 mm for `ΔR = 1.5` at 85° — which
///    drives the compared residual arbitrarily negative and passes *any* cell.
///    Abstaining leaves the raw (large) residual in place, so a wall stays
///    fine-tier: the safe direction for surface quality.
/// 3. **It is not this map's territory anyway.** Above this slope the
///    consumer's band split gives the surface to waterline, whose Z-level
///    contours are not decided by a drop-cutter residual.
///
/// The cost is a deliberate **discontinuity at the cap** — just below it a
/// large term is subtracted, just above it none is. The tier map is a
/// classifier input, not a continuous field, and Phase I's morphology
/// (hysteresis / close / min-area) is what smooths label boundaries.
pub const MAX_COMPENSATED_SLOPE_DEG: f64 = 75.0;

/// `tan²(MAX_COMPENSATED_SLOPE_DEG)` = `(2 + √3)²` = `7 + 4√3`.
///
/// The cap is applied in the **gradient** domain so the walk needs no trig:
/// `sec θ = √(1 + |∇z|²)`, and `θ ≥ cap ⟺ |∇z|² ≥ tan²(cap)`. `f64::tan` is
/// not a `const fn`, so the value is written out; `the_gradient_cap_matches_
/// its_documented_angle` pins the pair so the two cannot drift apart.
const MAX_COMPENSATED_GRADIENT_SQ: f64 = 13.928_203_230_275_509;

/// `sec θ − 1` from a surface gradient, or `None` where the treatment
/// abstains (above [`MAX_COMPENSATED_SLOPE_DEG`], or a non-finite gradient).
///
/// The single site the cap is applied at, shared by the walk and by
/// [`cl_offset_bias_mm`] — two spellings of one law is the instrument-integrity
/// trap this repo has paid for before.
fn slope_bias_scale(dz_dx: f64, dz_dy: f64) -> Option<f64> {
    let gradient_sq = dz_dx * dz_dx + dz_dy * dz_dy;
    if gradient_sq.is_nan() || gradient_sq > MAX_COMPENSATED_GRADIENT_SQ {
        return None;
    }
    Some((1.0 + gradient_sq).sqrt() - 1.0)
}

/// The analytic tool-centre-offset bias between two tip spheres on a plane of
/// slope `slope_deg`: `cusp_excess_mm · (sec θ − 1)`.
///
/// `cusp_excess_mm` is `R_coarse − R_fine` in
/// [`MillingCutter::cusp_radius_mm`] terms — non-negative for any ladder
/// [`TierLadder::new`] accepts.
///
/// `None` where [`ResidualTreatment::SlopeCompensated`] abstains: above
/// [`MAX_COMPENSATED_SLOPE_DEG`], or outside `0..=90` degrees, or `NaN`.
/// Exactly at the cap the answer is whichever side `tan(75°)` lands on in
/// binary floating point — do not build anything on the boundary cell itself.
///
/// This is the law the walk applies, reachable so a harness can state the
/// expected bias in the units the finding does (`T1_FINDINGS.md` §3.4:
/// **0.62 mm at 45° for R2 against R0.5**) rather than re-deriving it.
#[must_use]
pub fn cl_offset_bias_mm(cusp_excess_mm: f64, slope_deg: f64) -> Option<f64> {
    if !(0.0..=90.0).contains(&slope_deg) {
        return None;
    }
    let scale = slope_bias_scale(slope_deg.to_radians().tan(), 0.0)?;
    Some(cusp_excess_mm * scale)
}

/// Why a tier map could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierMapError {
    /// A ladder needs at least one tool — there is nothing to reference
    /// against otherwise.
    EmptyLadder,
    /// More tools than a one-byte label can carry (see [`MAX_TIERS`]).
    LadderTooLong { len: usize },
    /// The ladder is not ordered coarse → fine at this index. "The coarsest
    /// tool that reaches" is not a defined quantity over an unordered
    /// ladder, so this is refused rather than silently mislabelled.
    LadderNotCoarseToFine { index: usize },
    /// The cancel token fired during the walk.
    Cancelled,
}

impl fmt::Display for TierMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLadder => f.write_str("tier ladder is empty"),
            Self::LadderTooLong { len } => {
                write!(f, "tier ladder has {len} tools, the maximum is {MAX_TIERS}")
            }
            Self::LadderNotCoarseToFine { index } => write!(
                f,
                "tier ladder is not ordered coarse to fine at index {index}"
            ),
            Self::Cancelled => f.write_str("tier map walk was cancelled"),
        }
    }
}

impl std::error::Error for TierMapError {}

impl From<Cancelled> for TierMapError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// An ordered coarse → fine ladder of candidate finishing cutters.
///
/// The last entry is the **reference**: every residual is measured against
/// it, so it is the finest detail the map can express. Ordering is by
/// [`MillingCutter::cusp_radius_mm`] — the tip-sphere feature scale — not by
/// [`MillingCutter::envelope_radius_mm`], because on a tapered ball the
/// envelope is the shank and would call the project's Ø1-tip finisher a
/// coarser tool than a Ø4 ball.
///
/// The tool list is private: [`TierLadder::new`] enforces the ordering
/// invariant the label semantics rest on, and a public field would let a
/// caller break it after the fact.
pub struct TierLadder<'a> {
    tools: Vec<&'a dyn MillingCutter>,
    max_envelope_radius_mm: f64,
}

impl<'a> TierLadder<'a> {
    /// Validate and adopt a coarse → fine ladder.
    ///
    /// # Errors
    ///
    /// [`TierMapError::EmptyLadder`], [`TierMapError::LadderTooLong`] or
    /// [`TierMapError::LadderNotCoarseToFine`].
    pub fn new(tools: &[&'a dyn MillingCutter]) -> Result<Self, TierMapError> {
        if tools.is_empty() {
            return Err(TierMapError::EmptyLadder);
        }
        if tools.len() > MAX_TIERS {
            return Err(TierMapError::LadderTooLong { len: tools.len() });
        }
        for (i, pair) in tools.windows(2).enumerate() {
            let (Some(coarser), Some(finer)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            if finer.cusp_radius_mm() > coarser.cusp_radius_mm() {
                return Err(TierMapError::LadderNotCoarseToFine { index: i + 1 });
            }
        }
        let max_envelope_radius_mm = tools
            .iter()
            .map(|t| t.envelope_radius_mm())
            .fold(0.0f64, f64::max);
        Ok(Self {
            tools: tools.to_vec(),
            max_envelope_radius_mm,
        })
    }

    /// The ladder, coarse first.
    #[must_use]
    pub fn tools(&self) -> &[&'a dyn MillingCutter] {
        &self.tools
    }

    /// Number of tiers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Always `false` — [`TierLadder::new`] refuses an empty ladder. Present
    /// so the type reads normally next to `len`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// The radius the single shared spatial-index query is taken at (G1).
    #[must_use]
    pub fn max_envelope_radius_mm(&self) -> f64 {
        self.max_envelope_radius_mm
    }

    /// The reference cutter — the finest tool, which every residual is
    /// measured against.
    #[must_use]
    pub fn finest(&self) -> Option<&'a dyn MillingCutter> {
        self.tools.last().copied()
    }
}

/// Hand-written because `dyn MillingCutter` is not `Debug`. A ladder prints as
/// its tier count and the cusp radii that define its ordering — which is what
/// a [`TierMapError::LadderNotCoarseToFine`] refusal, a trace line or a failing
/// sentry actually needs to read.
impl fmt::Debug for TierLadder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cusp_radii_mm: Vec<f64> = self.tools.iter().map(|t| t.cusp_radius_mm()).collect();
        f.debug_struct("TierLadder")
            .field("tiers", &self.tools.len())
            .field("cusp_radii_mm", &cusp_radii_mm)
            .field("max_envelope_radius_mm", &self.max_envelope_radius_mm)
            .finish()
    }
}

/// Inputs to the tier-map walk. Mirrors [`crate::surface::rest_field::RestFieldParams`]
/// in shape and in units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierMapParams {
    /// XY grid cell size (mm). Smaller = finer islands, quadratically more
    /// drops. See the module doc for the measured cost band — plan at
    /// 0.3–0.6 mm.
    pub cell_mm: f64,
    /// A cell is claimed by tool *k* once its residual against the finest
    /// tool is no more than this (mm).
    ///
    /// It is a *residual* threshold, not a cusp height: a sane value is above
    /// the coarse tier's own cusp, because a cell the coarse tool leaves at
    /// its own cusp height is not rest material.
    pub tolerance_mm: f64,
    /// Extra grid padding (mm) beyond the finest tool's envelope radius, so
    /// the outer ring of cells is genuinely non-contact. Same role as
    /// `RestFieldParams::region_margin_mm`.
    pub margin_mm: f64,
    /// How the raw residual is treated before the comparison — the T2 seam,
    /// and part of the cache key. See [`ResidualTreatment`].
    pub treatment: ResidualTreatment,
}

impl Default for TierMapParams {
    fn default() -> Self {
        Self {
            cell_mm: 0.5,
            tolerance_mm: 0.03,
            margin_mm: 0.5,
            treatment: ResidualTreatment::Raw,
        }
    }
}

/// Per-cell tier labels over a regular XY grid.
///
/// The grid is a [`GridSpec`]: row-major `r * nx + c`, cell centre at
/// `(grid.x_of(c), grid.y_of(r))` — the same representation as
/// [`crate::surface::rest_field::RestGrid`] and
/// [`crate::maps::reach_map::ReachMap`] (FLD-01).
#[derive(Debug, Clone)]
pub struct TierMap {
    /// The XY walk grid.
    pub grid: GridSpec,
    /// Index into the ladder of the coarsest tool that holds this cell, or
    /// [`NO_TIER`] where no tool reaches (off the part / outside the mesh
    /// footprint).
    pub labels: Vec<u8>,
    /// The finest (reference) tool's drop Z (mm) per cell; `NaN` wherever the
    /// label is [`NO_TIER`]. One `f32`, not a per-tool plane — see the module
    /// doc on memory.
    pub finest_z: Vec<f32>,
    /// Ladder length this map was built with; labels are `0..tier_count`.
    pub tier_count: usize,
    /// The residual tolerance (mm) the labels were decided at.
    pub tolerance_mm: f64,
    /// Which residual treatment produced the labels.
    pub treatment: ResidualTreatment,
}

impl TierMap {
    /// Label at `(row, col)`, or `None` off the grid.
    ///
    /// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
    /// only callers. No production path reads it, so it sits behind
    /// `test-support` (FLD-05).
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn label_at(&self, row: usize, col: usize) -> Option<u8> {
        if row >= self.grid.ny || col >= self.grid.nx {
            return None;
        }
        self.labels.get(self.grid.index_of(row, col)).copied()
    }

    /// World XY of the cell centre at `(row, col)`, or `None` off the grid.
    #[must_use]
    pub fn cell_center(&self, row: usize, col: usize) -> Option<(f64, f64)> {
        if row >= self.grid.ny || col >= self.grid.nx {
            return None;
        }
        Some((
            self.grid.origin_x + col as f64 * self.grid.cell_mm,
            self.grid.origin_y + row as f64 * self.grid.cell_mm,
        ))
    }

    /// The grid cell whose centre is nearest `(x, y)`, or `None` if that is
    /// off the grid.
    #[must_use]
    pub fn nearest_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col = ((x - self.grid.origin_x) / self.grid.cell_mm).round();
        let row = ((y - self.grid.origin_y) / self.grid.cell_mm).round();
        if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= self.grid.nx || row >= self.grid.ny {
            return None;
        }
        Some((row, col))
    }

    /// The boolean mask of cells tier `k` owns — the input Phase I's
    /// morphology (`finish_planner`'s hysteresis / close / min-area steps)
    /// consumes. A `k` past the ladder selects nothing.
    #[must_use]
    pub fn tier_mask(&self, k: usize) -> Vec<bool> {
        let Ok(want) = u8::try_from(k) else {
            return vec![false; self.labels.len()];
        };
        if want == NO_TIER {
            return vec![false; self.labels.len()];
        }
        self.labels.iter().map(|&l| l == want).collect()
    }

    /// The mask of cells any tool reaches — i.e. `label != NO_TIER`. This is
    /// the contact/coverage mask a consumer erodes to remove the false-high
    /// rim band the module doc warns about.
    #[must_use]
    pub fn covered_mask(&self) -> Vec<bool> {
        self.labels.iter().map(|&l| l != NO_TIER).collect()
    }

    /// Cell count per tier, indexed like the ladder.
    #[must_use]
    pub fn tier_cell_counts(&self) -> Vec<usize> {
        let mut counts = vec![0usize; self.tier_count];
        for &label in &self.labels {
            if let Some(slot) = counts.get_mut(label as usize) {
                *slot += 1;
            }
        }
        counts
    }

    /// Cells no tool reaches.
    #[must_use]
    pub fn unassigned_cells(&self) -> usize {
        self.labels.iter().filter(|&&l| l == NO_TIER).count()
    }

    /// Tier `k`'s territory in mm² (cell count × cell area). A grid-quantised
    /// area, not a polygon area — the polygons come later, from marching
    /// squares over [`TierMap::tier_mask`].
    #[must_use]
    pub fn tier_area_mm2(&self, k: usize) -> f64 {
        let cells = self.tier_cell_counts().get(k).copied().unwrap_or(0);
        cells as f64 * self.grid.cell_mm * self.grid.cell_mm
    }
}

/// One CL point per ladder tool at `(x, y)`, from a **single** spatial-index
/// query at the ladder's largest envelope radius (G1).
///
/// Bit-identical to calling [`crate::surface::dropcutter::point_drop_cutter`] once per
/// tool — see the module doc for why, and
/// `tests/tier_map_walk_t1.rs` for the assertion. Returned coarse-first, in
/// ladder order.
#[must_use]
pub fn ladder_drops_at(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
) -> Vec<CLPoint> {
    let candidates = index.query(x, y, ladder.max_envelope_radius_mm());
    ladder
        .tools()
        .iter()
        .map(|tool| drop_against_candidates(x, y, mesh, &candidates, *tool))
        .collect()
}

/// [`crate::surface::dropcutter::point_drop_cutter`]'s body with the index query lifted
/// out, so one candidate set can serve every tool on the ladder.
fn drop_against_candidates(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    candidates: &[usize],
    cutter: &dyn MillingCutter,
) -> CLPoint {
    let mut cl = CLPoint::new(x, y);
    // Hoisted once per CL point, exactly as `point_drop_cutter` does: on a
    // `Box<dyn MillingCutter>` `radius()` is itself an indirect call.
    let envelope_radius = cutter.radius();
    for &idx in candidates {
        let Some(tri) = mesh.faces.get(idx) else {
            continue;
        };
        if !drop_cutter_can_contact(&cl, tri, envelope_radius) {
            continue;
        }
        cutter.drop_cutter(&mut cl, tri);
    }
    cl
}

/// The reference (finest) tool's drop at `(x, y)`, together with the shared
/// candidate set it was taken from (G1) and the drop-cutter calls made.
///
/// `None` means the cell has no owner, for either of two independent reasons.
/// `contacted` answers "did any triangle hold the tool up";
/// [`point_is_over_mesh_xy`] answers "is there surface under this XY at all" —
/// the cutter has a radius, so it reports contact while merely hanging off the
/// rim, and only the second predicate separates surface from no surface
/// (`dropcutter`'s own doc).
fn reference_drop(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
) -> (Option<f64>, Vec<usize>, u64) {
    let candidates = index.query(x, y, ladder.max_envelope_radius_mm());
    let Some(finest) = ladder.finest() else {
        return (None, candidates, 0);
    };
    let cl = drop_against_candidates(x, y, mesh, &candidates, finest);
    if !cl.contacted || !point_is_over_mesh_xy(x, y, mesh, index) {
        return (None, candidates, 1);
    }
    (Some(cl.z), candidates, 1)
}

/// Everything the tolerance comparison needs beyond the cell's own drops.
struct ResidualRule<'a> {
    /// The ladder minus its reference entry, coarse first.
    coarser: &'a [&'a dyn MillingCutter],
    /// `R_k − R_finest` (mm) in cusp-radius terms, indexed like `coarser`.
    /// Empty is legal and means "no compensation available".
    cusp_excess_mm: &'a [f64],
    /// Residual at or below which a tool claims the cell (mm).
    tolerance_mm: f64,
    /// `sec θ − 1` at this cell — the slope-bias scale. Exactly `0.0` for
    /// [`ResidualTreatment::Raw`] *and* wherever
    /// [`ResidualTreatment::SlopeCompensated`] abstains, which is what makes
    /// those two cases bit-identical to an uncompensated comparison rather
    /// than merely close to it.
    bias_scale: f64,
}

/// One cell's tier verdict: `(label, drop-cutter calls made)`.
///
/// **The single site that turns a residual into a verdict** — both treatments
/// and both passes come through here, so a compensated map and a raw one
/// differ by exactly one term.
fn tier_for(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    candidates: &[usize],
    reference_z: f64,
    rule: &ResidualRule<'_>,
) -> (u8, u64) {
    let mut drops = 0u64;
    for (k, tool) in rule.coarser.iter().enumerate() {
        let cl = drop_against_candidates(x, y, mesh, candidates, *tool);
        drops += 1;
        if !cl.contacted {
            continue;
        }
        let raw_residual = cl.z - reference_z;
        let bias = rule.cusp_excess_mm.get(k).copied().unwrap_or(0.0) * rule.bias_scale;
        if raw_residual - bias <= rule.tolerance_mm {
            // `k` indexes `coarser`, which is the ladder minus its last
            // entry, so it is already the ladder index.
            let label = u8::try_from(k).unwrap_or(NO_TIER);
            return (label, drops);
        }
    }

    // Nothing coarser held it: the reference tool owns the cell. The ladder
    // length is bounded by MAX_TIERS at construction, so this cannot collide
    // with NO_TIER.
    let label = u8::try_from(rule.coarser.len()).unwrap_or(NO_TIER);
    (label, drops)
}

/// One cell of the untreated ([`ResidualTreatment::Raw`]) single-pass walk:
/// `(label, finest drop Z, drop-cutter calls made)`, one index query.
fn classify_cell(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
) -> (u8, f32, u64) {
    let Some((_, coarser)) = ladder.tools().split_last() else {
        return (NO_TIER, f32::NAN, 0);
    };
    let (reference_z, candidates, mut drops) = reference_drop(x, y, mesh, index, ladder);
    let Some(reference_z) = reference_z else {
        return (NO_TIER, f32::NAN, drops);
    };
    let rule = ResidualRule {
        coarser,
        cusp_excess_mm: &[],
        tolerance_mm: params.tolerance_mm,
        bias_scale: 0.0,
    };
    let (label, verdict_drops) = tier_for(x, y, mesh, &candidates, reference_z, &rule);
    drops += verdict_drops;
    (label, reference_z as f32, drops)
}

/// The ladder's own [`GridSpec`] constructor. The grid type and its
/// accessors live in [`crate::maps::grid`]; the padding rule is this module's.
impl GridSpec {
    /// Pad past the mesh bbox by the finest tool's envelope plus the margin,
    /// so the outer ring of cells is genuinely non-contact and a consumer's
    /// distance transform has somewhere to start. Same rule as
    /// `rest_field::detect_rest_valleys`.
    fn for_ladder(mesh: &TriangleMesh, ladder: &TierLadder<'_>, params: &TierMapParams) -> Self {
        let cell_mm = params.cell_mm.max(1e-3);
        let finest_envelope = ladder.finest().map_or(0.0, |t| t.envelope_radius_mm());
        let pad_mm = finest_envelope + params.margin_mm.max(0.0);
        let margin_cells = (pad_mm / cell_mm).ceil().max(0.0) as usize + 1;
        let bbox = &mesh.bbox;
        let cols = (bbox.max.x - bbox.min.x) / cell_mm;
        let rows = (bbox.max.y - bbox.min.y) / cell_mm;
        let padding = 2 * margin_cells + 1;
        Self {
            nx: cols.ceil().max(0.0) as usize + padding,
            ny: rows.ceil().max(0.0) as usize + padding,
            origin_x: bbox.min.x - margin_cells as f64 * cell_mm,
            origin_y: bbox.min.y - margin_cells as f64 * cell_mm,
            cell_mm,
        }
    }
}

/// [`ResidualTreatment::Raw`]: one pass, one index query per cell.
fn raw_walk(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<(Vec<u8>, Vec<f32>), TierMapError> {
    let row_cells = |row: usize| -> (Vec<(u8, f32)>, u64) {
        let y = grid.y_of(row);
        let mut drops = 0u64;
        let cells = (0..grid.nx)
            .map(|col| {
                let x = grid.x_of(col);
                let (label, z, d) = classify_cell(x, y, mesh, index, ladder, params);
                drops += d;
                (label, z)
            })
            .collect();
        (cells, drops)
    };
    Ok(walk_rows(grid, cancel, Some(&DROP_CALLS), row_cells)?
        .into_iter()
        .unzip())
}

/// Central finite difference of the reference-drop plane at `(row, col)`,
/// in mm per mm.
///
/// Failure modes, all of which degrade toward "no compensation" and therefore
/// toward the FINE tier — never toward silently handing territory to a coarse
/// tool:
///
/// * **Grid edge.** A missing neighbour drops out of the stencil and the
///   difference becomes one-sided; with neither neighbour usable on an axis
///   that axis reads zero gradient. In practice the outer ring is padding and
///   carries no owned cell, so this is reachable only on a degenerate grid.
/// * **`NO_TIER` neighbour.** An unowned neighbour is `NaN` and is treated
///   exactly like a missing one — never averaged in, which would poison the
///   whole cell to `NaN` and abstain. So a cell on the part rim measures its
///   slope from the inward side alone. That rim already reads false-high
///   residuals from the coarse tool hanging off the edge (see the module doc
///   on boundary erosion), and the consumer erodes it; the one-sided gradient
///   does not make that band worse.
/// * **Curvature.** A central difference over a `cell_mm` stencil smooths, so
///   a ridge or a valley floor narrower than two cells under-reads its slope
///   and is under-compensated.
fn reference_gradient(reference: &[f64], grid: &GridSpec, row: usize, col: usize) -> (f64, f64) {
    let at = |r: usize, c: usize| -> Option<f64> {
        if r >= grid.ny || c >= grid.nx {
            return None;
        }
        let z = *reference.get(r * grid.nx + c)?;
        // An unowned neighbour is NaN and must leave the stencil, not enter
        // it — averaging one in poisons the whole cell.
        z.is_finite().then_some(z)
    };
    let here = at(row, col).unwrap_or(f64::NAN);
    let axis = |back: Option<f64>, forward: Option<f64>| -> f64 {
        match (back, forward) {
            (Some(b), Some(f)) => (f - b) / (2.0 * grid.cell_mm),
            (Some(b), None) => (here - b) / grid.cell_mm,
            (None, Some(f)) => (f - here) / grid.cell_mm,
            (None, None) => 0.0,
        }
    };
    let west = col.checked_sub(1).and_then(|c| at(row, c));
    let east = at(row, col + 1);
    let south = row.checked_sub(1).and_then(|r| at(r, col));
    let north = at(row + 1, col);
    (axis(west, east), axis(south, north))
}

/// [`ResidualTreatment::SlopeCompensated`]: two passes over the same grid.
///
/// Pass 1 lays down the reference tool's drop plane (one drop and one index
/// query per cell). Pass 2 differentiates that plane per cell for θ and then
/// takes the coarse-tool drops, so the verdict costs one further index query
/// on cells the reference owns. Total drop-cutter work is identical to
/// [`raw_walk`]'s — the extra pass buys slope, not drops.
fn compensated_walk(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<(Vec<u8>, Vec<f32>), TierMapError> {
    let Some((finest, coarser)) = ladder.tools().split_last() else {
        return Ok((
            vec![NO_TIER; grid.cell_count()],
            vec![f32::NAN; grid.cell_count()],
        ));
    };
    // Hoisted out of the cell loop: these are `dyn` calls, and the ladder
    // ordering invariant makes every entry non-negative. `max` also absorbs a
    // NaN cusp radius into "no compensation for that tier".
    let finest_cusp_mm = finest.cusp_radius_mm();
    let cusp_excess_mm: Vec<f64> = coarser
        .iter()
        .map(|tool| (tool.cusp_radius_mm() - finest_cusp_mm).max(0.0))
        .collect();

    let reference_row = |row: usize| -> (Vec<f64>, u64) {
        let y = grid.y_of(row);
        let mut drops = 0u64;
        let cells = (0..grid.nx)
            .map(|col| {
                let x = grid.x_of(col);
                let (z, _candidates, d) = reference_drop(x, y, mesh, index, ladder);
                drops += d;
                z.unwrap_or(f64::NAN)
            })
            .collect();
        (cells, drops)
    };
    let reference = walk_rows(grid, cancel, Some(&DROP_CALLS), reference_row)?;

    let tier_row = |row: usize| -> (Vec<u8>, u64) {
        let y = grid.y_of(row);
        let mut drops = 0u64;
        let cells = (0..grid.nx)
            .map(|col| {
                let cell = row * grid.nx + col;
                let reference_z = reference.get(cell).copied().unwrap_or(f64::NAN);
                if !reference_z.is_finite() {
                    return NO_TIER;
                }
                let (dz_dx, dz_dy) = reference_gradient(&reference, grid, row, col);
                let rule = ResidualRule {
                    coarser,
                    cusp_excess_mm: &cusp_excess_mm,
                    tolerance_mm: params.tolerance_mm,
                    // Abstain (above the cap) compares the raw residual.
                    bias_scale: slope_bias_scale(dz_dx, dz_dy).unwrap_or(0.0),
                };
                let x = grid.x_of(col);
                let candidates = index.query(x, y, ladder.max_envelope_radius_mm());
                let (label, d) = tier_for(x, y, mesh, &candidates, reference_z, &rule);
                drops += d;
                label
            })
            .collect();
        (cells, drops)
    };
    let labels = walk_rows(grid, cancel, Some(&DROP_CALLS), tier_row)?;

    let finest_z = reference.iter().map(|z| *z as f32).collect();
    Ok((labels, finest_z))
}

/// Walk the grid once and label every cell with the coarsest ladder tool that
/// holds it.
///
/// [`TierMapParams::treatment`] selects the walk: [`ResidualTreatment::Raw`]
/// is single-pass, [`ResidualTreatment::SlopeCompensated`] is two-pass. Both
/// produce the same shape of [`TierMap`] and take the same number of
/// drop-cutter calls.
///
/// # Errors
///
/// [`TierMapError::Cancelled`] if `cancel` fires during the walk. Ladder
/// validity is established by [`TierLadder::new`], so it cannot fail here.
pub fn compute_tier_map(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<TierMap, TierMapError> {
    let grid = GridSpec::for_ladder(mesh, ladder, params);
    let (labels, finest_z) = match params.treatment {
        ResidualTreatment::Raw => raw_walk(mesh, index, ladder, params, &grid, cancel)?,
        ResidualTreatment::SlopeCompensated => {
            compensated_walk(mesh, index, ladder, params, &grid, cancel)?
        }
    };

    Ok(TierMap {
        grid,
        labels,
        finest_z,
        tier_count: ladder.len(),
        tolerance_mm: params.tolerance_mm,
        treatment: params.treatment,
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
    use super::{
        MAX_COMPENSATED_GRADIENT_SQ, MAX_COMPENSATED_SLOPE_DEG, NO_TIER, ResidualTreatment,
        TierLadder, TierMapError, TierMapParams, cl_offset_bias_mm, compute_tier_map,
        slope_bias_scale,
    };
    use crate::mesh::{SpatialIndex, make_test_flat};
    use crate::tool::{BallEndmill, MillingCutter};

    fn never_cancel() -> impl Fn() -> bool + Send + Sync {
        || false
    }

    #[test]
    fn the_gradient_cap_matches_its_documented_angle() {
        // The walk compares |∇z|² against a written-out constant because
        // `f64::tan` is not const. If someone edits the degrees and not the
        // constant, the cap silently moves; this is the tie.
        let from_degrees = MAX_COMPENSATED_SLOPE_DEG.to_radians().tan().powi(2);
        assert!(
            (from_degrees - MAX_COMPENSATED_GRADIENT_SQ).abs() < 1e-9,
            "tan^2({MAX_COMPENSATED_SLOPE_DEG}) = {from_degrees}, constant says \
             {MAX_COMPENSATED_GRADIENT_SQ}"
        );
        // 7 + 4√3, stated independently of `tan`.
        assert!((MAX_COMPENSATED_GRADIENT_SQ - (7.0 + 4.0 * 3.0f64.sqrt())).abs() < 1e-12);
    }

    #[test]
    fn the_bias_law_reproduces_the_finding_number() {
        // `T1_FINDINGS.md` §3.4: R2 against R0.5 reads 0.62 mm at 45° and
        // 1.5 mm at 60° on a plane BOTH tools machine perfectly.
        let excess = 2.0 - 0.5;
        let at_45 = cl_offset_bias_mm(excess, 45.0).expect("45 deg is inside the cap");
        let at_60 = cl_offset_bias_mm(excess, 60.0).expect("60 deg is inside the cap");
        assert!((at_45 - 0.6213).abs() < 5e-4, "45 deg read {at_45}");
        assert!((at_60 - 1.5).abs() < 5e-4, "60 deg read {at_60}");
        // Flat ground is exactly zero, so the treatment is a no-op there.
        assert_eq!(cl_offset_bias_mm(excess, 0.0), Some(0.0));
    }

    #[test]
    fn the_law_abstains_rather_than_clamping_above_the_cap() {
        let excess = 1.5;
        assert!(cl_offset_bias_mm(excess, 74.0).is_some());
        assert_eq!(cl_offset_bias_mm(excess, 80.0), None);
        assert_eq!(cl_offset_bias_mm(excess, 89.999), None);
        assert_eq!(cl_offset_bias_mm(excess, 90.0), None);
        // Nonsense in, abstention out — never a number.
        assert_eq!(cl_offset_bias_mm(excess, -1.0), None);
        assert_eq!(cl_offset_bias_mm(excess, f64::NAN), None);
        // The gradient-domain entry point agrees with the degree one.
        assert_eq!(slope_bias_scale(f64::NAN, 0.0), None);
        assert_eq!(slope_bias_scale(0.0, 0.0), Some(0.0));
        let diagonal = slope_bias_scale(1.0, 0.0).expect("45 deg along X");
        assert!((diagonal - (2.0f64.sqrt() - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn a_flat_plate_is_entirely_coarse_tier() {
        let mesh = make_test_flat(20.0);
        let index = SpatialIndex::build_auto(&mesh);
        let coarse = BallEndmill::new(6.0, 25.0);
        let fine = BallEndmill::new(1.0, 25.0);
        let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
        let ladder = TierLadder::new(&tools).unwrap();
        let params = TierMapParams::default();
        let map = compute_tier_map(&mesh, &index, &ladder, &params, &never_cancel()).unwrap();

        assert_eq!(map.tier_count, 2);
        assert_eq!(map.labels.len(), map.grid.nx * map.grid.ny);
        assert_eq!(map.finest_z.len(), map.labels.len());
        assert_eq!(
            map.tier_cell_counts()[1],
            0,
            "nothing on a plane needs the fine tool"
        );
        assert!(map.tier_cell_counts()[0] > 0);
        assert!(map.unassigned_cells() > 0, "the padded ring is unassigned");
    }

    #[test]
    fn sentinel_cells_carry_a_nan_reference_height() {
        let mesh = make_test_flat(20.0);
        let index = SpatialIndex::build_auto(&mesh);
        let fine = BallEndmill::new(1.0, 25.0);
        let tools: [&dyn MillingCutter; 1] = [&fine];
        let ladder = TierLadder::new(&tools).unwrap();
        let map = compute_tier_map(
            &mesh,
            &index,
            &ladder,
            &TierMapParams::default(),
            &never_cancel(),
        )
        .unwrap();
        for (label, z) in map.labels.iter().zip(&map.finest_z) {
            if *label == NO_TIER {
                assert!(z.is_nan(), "an unowned cell must not publish a height");
            } else {
                assert!(z.is_finite());
            }
        }
    }

    #[test]
    fn ladder_ordering_is_by_cusp_radius_not_envelope() {
        // A Ø1-tip taper on a Ø6 shank has envelope 3.0 and cusp 0.5. Placed
        // after a Ø4 ball (envelope 2.0, cusp 2.0) it is FINER by the only
        // measure that matters here, even though its envelope is larger.
        let ball = BallEndmill::new(4.0, 25.0);
        let taper = crate::tool::TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
        let tools: [&dyn MillingCutter; 2] = [&ball, &taper];
        let ladder = TierLadder::new(&tools).expect("ball -> taper is coarse to fine");
        assert!((ladder.max_envelope_radius_mm() - 3.0).abs() < 1e-9);

        let reversed: [&dyn MillingCutter; 2] = [&taper, &ball];
        assert!(matches!(
            TierLadder::new(&reversed),
            Err(TierMapError::LadderNotCoarseToFine { index: 1 })
        ));
    }

    #[test]
    fn treatment_rides_onto_the_map() {
        let mesh = make_test_flat(10.0);
        let index = SpatialIndex::build_auto(&mesh);
        let fine = BallEndmill::new(1.0, 25.0);
        let tools: [&dyn MillingCutter; 1] = [&fine];
        let ladder = TierLadder::new(&tools).unwrap();
        let params = TierMapParams {
            tolerance_mm: 0.04,
            ..TierMapParams::default()
        };
        let map = compute_tier_map(&mesh, &index, &ladder, &params, &never_cancel()).unwrap();
        assert_eq!(map.treatment, ResidualTreatment::Raw);
        assert!((map.tolerance_mm - 0.04).abs() < 1e-12);

        // The compensated arm is a different walk (two-pass) and must publish
        // the same grid, so a consumer cannot tell them apart by shape — only
        // by the tag, which is what keeps the cache honest.
        let compensated = TierMapParams {
            treatment: ResidualTreatment::SlopeCompensated,
            ..params
        };
        let cancel = never_cancel();
        let other = compute_tier_map(&mesh, &index, &ladder, &compensated, &cancel).unwrap();
        assert_eq!(other.treatment, ResidualTreatment::SlopeCompensated);
        assert_eq!((other.grid.nx, other.grid.ny), (map.grid.nx, map.grid.ny));
        assert!((other.grid.origin_x - map.grid.origin_x).abs() < 1e-12);
        assert!((other.grid.origin_y - map.grid.origin_y).abs() < 1e-12);
    }
}
