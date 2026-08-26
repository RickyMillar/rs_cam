//! Multi-tool **tier map** — one grid walk that labels every cell with the
//! COARSEST tool on a ladder that can hold the surface there.
//!
//! This is task T1 of the multi-tool island-finishing plan
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase T), and it is
//! the *n*-tool generalisation of the two-tool residual that
//! [`crate::rest_field::detect_rest_valleys`] computes
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
//! [`crate::dropcutter::point_drop_cutter`] runs
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
//!   [`crate::rest_field`] erodes that band with a chamfer distance transform
//!   over its contact mask; the equivalent input here is
//!   [`TierMap::covered_mask`], and the erosion belongs to the consumer
//!   (Phase I) so that the map itself stays a measurement rather than a
//!   policy.
//! * **No slope-bias compensation.** The drop-cutter Z is the tool-CENTRE
//!   offset surface, so on a plain sloped plane — which any ball machines
//!   perfectly — the residual is `(R_ref − R_fine)·(sec θ − 1)`: 0.62 mm at
//!   45° for R2 against R0.5, i.e. one to two orders of magnitude above any
//!   sane tolerance (T1 §3.4). Untreated, **every slope on a terrain reads as
//!   fine-tier**. The treatment is task T2's A/B (analytic compensation vs a
//!   stock-referenced residual) and is NOT decided here; the seam it plugs
//!   into is [`ResidualTreatment`], which is part of the cache key precisely
//!   so a compensated map can never be served from a raw one's entry.
//!
//! # Cancellation
//!
//! The walk polls the cancel token **once per grid row**.
//! `rest_field`'s walk has no polling at all, which is why a rest analysis on
//! a big board cannot be interrupted; this one can.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::dropcutter::point_is_over_mesh_xy;
#[cfg(not(feature = "parallel"))]
use crate::interrupt::check_cancel;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
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
pub fn reset_drop_call_count() {
    DROP_CALLS.store(0, Ordering::Relaxed);
}

/// How the raw tool-vs-tool residual is turned into the number the tolerance
/// is compared against.
///
/// This exists as an enum rather than a closure because it is part of the
/// [`crate::tier_map_cache`] key: a slope-compensated map and a raw one over
/// the same mesh, ladder and grid are different answers, and a memo that
/// could not tell them apart would serve one for the other.
///
/// **T2 seam.** The slope-bias treatment (T1 §3.4 / plan blocker B1) adds its
/// variant here and its arm in [`compute_tier_map`]'s cell classifier, which
/// is the single site that turns a residual into a verdict. A variant that
/// needs per-cell surface slope will also need the walk to carry it — the
/// contact normal is available from the drop, and `crate::slope::SlopeMap` is
/// the shipped alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResidualTreatment {
    /// `drop_z(tool_k) − drop_z(finest)`, untreated. Honest, and biased on
    /// slopes by `(R_k − R_finest)·(sec θ − 1)` — see the module doc.
    #[default]
    Raw,
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

/// Inputs to the tier-map walk. Mirrors [`crate::rest_field::RestFieldParams`]
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
/// Row-major `r * nx + c`, cell centre at
/// `(origin_x + c * cell_mm, origin_y + r * cell_mm)` — the same convention as
/// [`crate::rest_field::RestGrid`] and [`crate::grid2::Grid2`].
#[derive(Debug, Clone)]
pub struct TierMap {
    pub nx: usize,
    pub ny: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_mm: f64,
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
    #[must_use]
    pub fn label_at(&self, row: usize, col: usize) -> Option<u8> {
        if row >= self.ny || col >= self.nx {
            return None;
        }
        self.labels.get(row * self.nx + col).copied()
    }

    /// World XY of the cell centre at `(row, col)`, or `None` off the grid.
    #[must_use]
    pub fn cell_center(&self, row: usize, col: usize) -> Option<(f64, f64)> {
        if row >= self.ny || col >= self.nx {
            return None;
        }
        Some((
            self.origin_x + col as f64 * self.cell_mm,
            self.origin_y + row as f64 * self.cell_mm,
        ))
    }

    /// The grid cell whose centre is nearest `(x, y)`, or `None` if that is
    /// off the grid.
    #[must_use]
    pub fn nearest_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col = ((x - self.origin_x) / self.cell_mm).round();
        let row = ((y - self.origin_y) / self.cell_mm).round();
        if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= self.nx || row >= self.ny {
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
        cells as f64 * self.cell_mm * self.cell_mm
    }
}

/// One CL point per ladder tool at `(x, y)`, from a **single** spatial-index
/// query at the ladder's largest envelope radius (G1).
///
/// Bit-identical to calling [`crate::dropcutter::point_drop_cutter`] once per
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

/// [`crate::dropcutter::point_drop_cutter`]'s body with the index query lifted
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

/// One cell's verdict: `(label, finest drop Z, drop-cutter calls made)`.
fn classify_cell(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
) -> (u8, f32, u64) {
    let Some((finest, coarser)) = ladder.tools().split_last() else {
        return (NO_TIER, f32::NAN, 0);
    };
    let candidates = index.query(x, y, ladder.max_envelope_radius_mm());
    let finest_cl = drop_against_candidates(x, y, mesh, &candidates, *finest);
    let mut drops = 1u64;

    // Two independent reasons a cell has no owner. `contacted` answers "did
    // any triangle hold the tool up"; `point_is_over_mesh_xy` answers "is
    // there surface under this XY at all" — the cutter has a radius, so it
    // reports contact while merely hanging off the rim, and only the second
    // predicate separates surface from no surface (`dropcutter`'s own doc).
    if !finest_cl.contacted || !point_is_over_mesh_xy(x, y, mesh, index) {
        return (NO_TIER, f32::NAN, drops);
    }

    for (k, tool) in coarser.iter().enumerate() {
        let cl = drop_against_candidates(x, y, mesh, &candidates, *tool);
        drops += 1;
        if !cl.contacted {
            continue;
        }
        let raw_residual = cl.z - finest_cl.z;
        // T2 SEAM — the single site that turns a residual into a verdict.
        // A slope-compensated arm subtracts `(R_k − R_finest)·(sec θ − 1)`
        // here; see `ResidualTreatment`.
        let observed = match params.treatment {
            ResidualTreatment::Raw => raw_residual,
        };
        if observed <= params.tolerance_mm {
            // `k` indexes `coarser`, which is the ladder minus its last
            // entry, so it is already the ladder index.
            let label = u8::try_from(k).unwrap_or(NO_TIER);
            return (label, finest_cl.z as f32, drops);
        }
    }

    // Nothing coarser held it: the reference tool owns the cell. The ladder
    // length is bounded by MAX_TIERS at construction, so this cannot collide
    // with NO_TIER.
    let label = u8::try_from(coarser.len()).unwrap_or(NO_TIER);
    (label, finest_cl.z as f32, drops)
}

/// Walk the grid once and label every cell with the coarsest ladder tool that
/// holds it.
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
    let cell = params.cell_mm.max(1e-3);
    let finest_envelope = ladder.finest().map_or(0.0, |t| t.envelope_radius_mm());

    // Pad past the mesh bbox by the finest tool's envelope plus the margin, so
    // the outer ring of cells is genuinely non-contact and a consumer's
    // distance transform has somewhere to start. Same rule as
    // `rest_field::detect_rest_valleys`.
    let pad_mm = finest_envelope + params.margin_mm.max(0.0);
    let margin_cells = (pad_mm / cell).ceil().max(0.0) as usize + 1;
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - margin_cells as f64 * cell;
    let origin_y = bbox.min.y - margin_cells as f64 * cell;
    let nx = ((bbox.max.x - bbox.min.x) / cell).ceil().max(0.0) as usize + 2 * margin_cells + 1;
    let ny = ((bbox.max.y - bbox.min.y) / cell).ceil().max(0.0) as usize + 2 * margin_cells + 1;

    let row_cells = |row: usize| -> (Vec<(u8, f32)>, u64) {
        let y = origin_y + row as f64 * cell;
        let mut drops = 0u64;
        let cells = (0..nx)
            .map(|col| {
                let x = origin_x + col as f64 * cell;
                let (label, z, d) = classify_cell(x, y, mesh, index, ladder, params);
                drops += d;
                (label, z)
            })
            .collect();
        (cells, drops)
    };

    let (labels, finest_z): (Vec<u8>, Vec<f32>) = {
        #[cfg(feature = "parallel")]
        {
            use std::sync::atomic::AtomicBool;
            let cancelled = AtomicBool::new(false);
            let collected: (Vec<u8>, Vec<f32>) = (0..ny)
                .into_par_iter()
                .flat_map(|row| {
                    // One poll per row — the granularity `rest_field`'s
                    // walk lacks entirely.
                    if cancelled.load(Ordering::Relaxed) || cancel.cancelled() {
                        cancelled.store(true, Ordering::Relaxed);
                        return Vec::new();
                    }
                    let (cells, drops) = row_cells(row);
                    DROP_CALLS.fetch_add(drops, Ordering::Relaxed);
                    cells
                })
                .unzip();
            if cancelled.load(Ordering::Relaxed) {
                return Err(TierMapError::Cancelled);
            }
            collected
        }
        #[cfg(not(feature = "parallel"))]
        {
            let mut labels = Vec::with_capacity(nx * ny);
            let mut finest_z = Vec::with_capacity(nx * ny);
            let mut drops = 0u64;
            for row in 0..ny {
                check_cancel(cancel)?;
                let (cells, row_drops) = row_cells(row);
                drops += row_drops;
                for (label, z) in cells {
                    labels.push(label);
                    finest_z.push(z);
                }
            }
            DROP_CALLS.fetch_add(drops, Ordering::Relaxed);
            (labels, finest_z)
        }
    };

    Ok(TierMap {
        nx,
        ny,
        origin_x,
        origin_y,
        cell_mm: cell,
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
        NO_TIER, ResidualTreatment, TierLadder, TierMapError, TierMapParams, compute_tier_map,
    };
    use crate::mesh::{SpatialIndex, make_test_flat};
    use crate::tool::{BallEndmill, MillingCutter};

    fn never_cancel() -> impl Fn() -> bool + Send + Sync {
        || false
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
        assert_eq!(map.labels.len(), map.nx * map.ny);
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
    }
}
