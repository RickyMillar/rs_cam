//! Per-tool **reach map** — which parts of a model surface a single
//! finishing cutter can actually form, and which it cannot.
//!
//! The operator's question, in their words: *does this ball radius fit into
//! the mountain valleys?* The answer is a per-cell **gap** in mm between the
//! surface this cutter would leave and the true mesh, plus one area-weighted
//! percentage and one worst-case number.
//!
//! ```text
//! machined_z(x, y) = min over CL positions p of [ tip_z(p) + height_at_radius(|p − (x,y)|) ]
//! gap(x, y)        = machined_z(x, y) − mesh_z(x, y)
//! unreachable      = gap > tolerance
//! ```
//!
//! # Why this is not a one-rung tier map
//!
//! [`crate::tier_map`] is the multi-tool sibling and was the obvious host,
//! but a one-rung ladder cannot answer this. Its residual is
//! `drop_z(tool_k) − drop_z(finest)` and with one tool `tool_k` **is** the
//! finest, so every cell reads exactly zero. Two further mismatches:
//!
//! * [`crate::tier_map::TierMap`] deliberately stores a `u8` label and one
//!   `f32` reference drop — 5 B/cell — and **discards the residual**. The
//!   overlay shades by gap depth, so the millimetres are the payload.
//! * A tool-versus-true-mesh residual measured as a raw vertical drop
//!   difference is dominated by the tool-centre-offset bias the tier map
//!   documents at length: a ball of radius `R` resting on a plane of slope θ
//!   leaves its tip `R·(sec θ − 1)` above that plane — **1.24 mm at 45° for
//!   a Ø6 ball**, twenty-five times any sane tolerance, on a plane the ball
//!   machines perfectly.
//!
//! So this module builds on the layer *underneath* the tier map — the
//! drop-cutter contact heights ([`crate::dropcutter::point_drop_cutter`],
//! the same call [`crate::tier_map::ladder_drops_at`] hoists a query out of)
//! — and takes the residual against the **machined surface** instead of
//! against a second drop plane.
//!
//! # Why the machined surface, and not a slope correction
//!
//! [`crate::tier_map::cl_offset_bias_mm`] subtracts the bias analytically.
//! Two reasons that law is the wrong instrument here:
//!
//! 1. **It is the spherical-tip law.** A flat tip on a slope sits
//!    `R·tan θ` above the plane, not `R·(sec θ − 1)` — 1.0 R against 0.41 R
//!    at 45°, so a Ø6 flat endmill on a 45° plane would read a 1.76 mm gap
//!    and paint the whole flank unreachable. A tapered ball past its
//!    half-angle contacts the cone and sits lower again. The reach map is
//!    asked about all three shapes.
//! 2. **It is sensitive to a slope it must estimate.** `d(bias)/dθ =
//!    R·sec θ·tan θ` is 0.074 mm per degree for `R = 3` at 45°, and a
//!    central difference over real terrain is easily one to two degrees out
//!    per cell. Against a 0.05 mm tolerance that is speckle. The tier map
//!    survives it because a classifier plus morphology follows; a per-vertex
//!    overlay has no morphology.
//!
//! Two further notes on the min-filter as implemented: the minimum is taken
//! over **polar** offsets with the envelope radius always in the set, and
//! `tip_z` is read back by bilinear interpolation, because a grid-snapped
//! kernel misses the radius a flat or bull tip binds at — see
//! [`profile_kernel`] for the two measured failures that rule produced.
//!
//! The min-filter has neither problem. It is exact on a plane of any
//! slope for **any** profile: with `tip_z(p) = plane_z(p) + b` for the
//! (unknown) constant `b`, the minimum over `p` of
//! `plane_z(p) + b + height_at_radius(r)` recovers `plane_z(x, y)` exactly,
//! because `b` is by construction
//! `max over r of [ r·tan θ − height_at_radius(r) ]` — the same support
//! function, read the other way. It reads no slope, needs no cap and has no
//! abstain arm. Feed it a ball and it reproduces
//! [`crate::tier_map::cl_offset_bias_mm`] to the discretisation floor;
//! `tests/reach_map_p5.rs::a_sloped_plane_is_reachable_and_the_bias_it_cancels_is_large`
//! asserts that rather than assuming it.
//!
//! # The discretisation floor
//!
//! The minimum is taken over sampled positions, not over the continuum, so
//! it **over**-states `machined_z` — i.e. over-states the gap. Two terms
//! carry that, and until 2026-09-08 only the smaller one was measured:
//!
//! * the **profile** term ([`sampling_floor_mm`]): the largest shortfall,
//!   over the slopes the map is used on, between the continuum support
//!   function and the same maximum over the kernel's own radii. It is a
//!   *plane* measurement, and barely cell-sensitive because the kernel radii
//!   are sine-spaced: 0.010 mm on a Ø6 ball, 0.132 mm on the Ø4 tapered ball
//!   of the wanaka case at its 0.645 mm cell. Do not quote one tool's figure
//!   for another; the spread across two shipped tools is thirteenfold.
//! * the **curvature** term ([`curvature_floor_plane`]): the CL set is spaced
//!   at the cell and read back between its points by [`sample_tip_z`], so on
//!   a curved `tip_z` field the answer is high by about `cell² · κ / 8`.
//!   For a tool of radius `R` bridging a concave feature of radius `ρ`,
//!   `κ = 1/(ρ − R)` — **0.052 mm at a 0.645 mm cell and `ρ − R` of 1 mm,
//!   the whole default tolerance.** Measured from the second difference of
//!   the built `tip_z` plane, and read at the tap that ATTAINED each cell's
//!   minimum ([`gap_plane`]), because the error of a minimum is the error of
//!   the one term that won it.
//!
//! [`ReachMap::discretisation_floor_mm`] is the larger of the two (the
//! curvature term at p95), and the verdict ABSTAINS per cell wherever the
//! tolerance sits under a cell's own floor —
//! [`ReachMap::unresolved_area_mm2`], never counted as unreachable. Read
//! [`ReachMap::tolerance_below_floor`] before believing the percentage.
//!
//! The cell is chosen from the **tool and the model only**
//! ([`ReachMapParams::for_cutter`]); the tolerance classifies on the grid and
//! never re-sizes it, so a series of probes at moving tolerances is
//! comparable.
//!
//! # What this map does NOT answer
//!
//! It is a **top-down** measure, like every drop-cutter quantity in this
//! tree. A triangle is in the population only when it is the topmost surface
//! at its own centroid and faces upward; undersides, overhangs and vertical
//! walls are `NOT MEASURED`, never "reachable" and never "unreachable".
//! [`ReachMap::measured_area_mm2`] against [`ReachMap::surface_area_mm2`]
//! says how much of the model the answer covers — a gate handed an empty
//! population passes and looks healthy, so read the population first.
//!
//! The area verdict samples each triangle **at its centroid**, so a triangle
//! much larger than the cell carries one reading for its whole area. On a
//! terrain STL that is a non-issue (triangles are sub-millimetre); on a
//! two-triangle plane it means the whole plane takes the verdict at two
//! points. It is the same trade `tier_area_mm2` makes on the tier map, and
//! it is why the cell rule caps at a feature scale rather than a floor.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::dropcutter::point_drop_cutter;
use crate::geo::P3;
#[cfg(not(feature = "parallel"))]
use crate::interrupt::check_cancel;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};

/// Reach tolerance (mm) for an operation that declares no cusp or scallop
/// height of its own.
///
/// **Repo-authored**, like every other threshold in this tree that carries no
/// citation: it is the order of magnitude of the shipped finishing scallop
/// defaults (`ScallopConfig::scallop_height` and
/// `UnifiedFinishConfig::scallop_height` both default to 0.1 mm) taken one
/// step tighter, so an op with no declared quality target is judged against a
/// finish bar rather than a roughing one. Do not cite it to a vendor.
pub const DEFAULT_REACH_TOLERANCE_MM: f64 = 0.05;

/// Where a reach tolerance came from. Carried beside the number so every
/// surface that prints the bar can print its provenance — the operator's F2
/// finding was that a 0.05 mm bar looked authoritative while being a bare
/// default under a 1.5 mm raster.
///
/// Resolved by
/// [`crate::session::ProjectSession::reach_tolerance_with_override`]; this
/// module owns the vocabulary because it owns the tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReachToleranceSource {
    /// The caller's own probe dial (the MCP `tolerance_mm` argument).
    CallerOverride,
    /// The operation declares a scallop / cusp height of its own (`scallop`,
    /// `unified_finish`, or a `drop_cutter` with the Suggest dial set).
    DeclaredScallopHeight,
    /// Derived from the operation's own lateral raster spacing and the tool's
    /// TIP-sphere radius: `cusp = R − sqrt(R² − (s/2)²)`.
    CuspOfStepover {
        stepover_mm: f64,
        tip_radius_mm: f64,
    },
    /// Nothing to derive from: [`DEFAULT_REACH_TOLERANCE_MM`].
    Default,
}

impl ReachToleranceSource {
    /// One phrase naming the source, for a panel line or an MCP reply. Shared
    /// so the two cannot describe one bar two ways.
    #[must_use]
    pub fn describe(self, tolerance_mm: f64) -> String {
        match self {
            Self::CallerOverride => format!("tol {tolerance_mm:.3} mm \u{2014} caller override"),
            Self::DeclaredScallopHeight => {
                format!("tol {tolerance_mm:.3} mm \u{2014} the operation's declared scallop height")
            }
            Self::CuspOfStepover {
                stepover_mm,
                tip_radius_mm,
            } => format!(
                "tol {tolerance_mm:.3} mm \u{2014} cusp of stepover {stepover_mm:.3} mm \
                 on a R{tip_radius_mm:.2} tip"
            ),
            Self::Default => format!(
                "tol {tolerance_mm:.3} mm \u{2014} the default; this operation declares \
                 neither a scallop height nor a lateral raster stepover"
            ),
        }
    }

    /// A stable snake_case key for a JSON wire field, beside the prose.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::CallerOverride => "caller_override",
            Self::DeclaredScallopHeight => "declared_scallop_height",
            Self::CuspOfStepover { .. } => "cusp_of_stepover",
            Self::Default => "default",
        }
    }
}

/// Smallest cell (mm) [`ReachMapParams::for_cutter`] will choose. Below this
/// the drop pass dominates the interaction budget for no visible gain — the
/// overlay is a look-and-see instrument, not a planning artefact.
pub const MIN_REACH_CELL_MM: f64 = 0.25;

/// Largest cell (mm) [`ReachMapParams::for_cutter`] will choose.
///
/// The sampling floor alone does not bound the cell from above: with
/// sine-spaced kernel radii a Ø6 ball's floor is 0.010 mm even at 1.5 mm
/// cells, and the bisection would happily take it. That would be a **feature
/// scale** error, not a profile-sampling one — the mesh's own surface is read
/// once per cell, so a crevice narrower than the cell falls between the
/// samples and the map cannot see the very thing it is asked about. 0.6 mm is
/// the top of [`crate::tier_map`]'s own measured planning band (0.3–0.6 mm),
/// and [`ReachMapParams::for_cutter`] tightens it further to half the tip
/// radius on a fine tool.
pub const MAX_REACH_CELL_MM: f64 = 0.6;

/// Cell budget for one map. The walk coarsens its cell until the grid fits,
/// so a big board degrades resolution instead of the frame rate.
///
/// At roughly 70 µs per drop (the tier map's measured full-grid cost, T1
/// §1.3) this bounds a cold build at single-digit seconds on the reference
/// board, and the build runs off the UI thread behind a `computing…` state.
pub const MAX_REACH_CELLS: usize = 120_000;

/// Vertical slack (mm) allowed when asking whether a triangle or vertex is
/// the topmost surface at its own XY. Pure floating-point slack — the point
/// lies on the surface it is being compared against.
const TOP_SURFACE_EPS_MM: f64 = 1e-6;

/// Inputs to a reach-map walk. Mirrors [`crate::tier_map::TierMapParams`] in
/// shape and units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReachMapParams {
    /// XY grid cell size (mm). See [`ReachMapParams::for_cutter`] for the
    /// rule that picks one, and [`MAX_REACH_CELLS`] for the budget that can
    /// coarsen it further.
    pub cell_mm: f64,
    /// A cell is UNREACHABLE once its gap exceeds this (mm).
    pub tolerance_mm: f64,
    /// Extra grid padding (mm) beyond the cutter's envelope radius, so the
    /// min-filter kernel has source cells all round the part.
    pub margin_mm: f64,
}

impl Default for ReachMapParams {
    fn default() -> Self {
        Self {
            cell_mm: 0.5,
            tolerance_mm: DEFAULT_REACH_TOLERANCE_MM,
            margin_mm: 0.5,
        }
    }
}

impl ReachMapParams {
    /// The cell for one cutter: **half its TIP sphere**, clamped to
    /// `MIN_REACH_CELL_MM..=MAX_REACH_CELL_MM`, and coarsened further only by
    /// [`MAX_REACH_CELLS`] once the model bbox is known.
    ///
    /// Half the TIP sphere, so the grid resolves a crevice the tip could sit
    /// in. On a tapered ball that is the Ø1 tip, not the Ø6 shank — reading
    /// the envelope here would hand the finest tool in the library the
    /// coarsest grid.
    ///
    /// # Why the tolerance does not appear
    ///
    /// It used to. `for_cutter` bisected for the coarsest cell whose
    /// [`sampling_floor_mm`] stayed under half the tolerance, and the
    /// operator's own bug report is what retired that rule (F5, 2026-09-08):
    /// three probes of one tool at three tolerances came back on cells
    /// 0.645, 0.75 and 0.625 mm, so **three percentages that were meant to be
    /// compared sat on three different grids**. The map is asked "how much
    /// does this tool miss at bar X" over and over with X moving; a dial that
    /// re-grids under each answer makes the series meaningless.
    ///
    /// The rule is now: the **tool and the model** fix the grid, and the
    /// tolerance only classifies on it. What the tolerance bought before —
    /// the promise that a reported gap at the bar is geometry rather than
    /// arithmetic — is not lost, it moved to where it can be *measured*
    /// instead of assumed: [`ReachMap::discretisation_floor_mm`] is now
    /// measured on the surface the walk actually built (see
    /// [`curvature_floor_plane`]) and the verdict abstains per cell wherever
    /// the bar is under it. The bisection could never have delivered that
    /// promise anyway — it scored the cell against a **plane**, where the
    /// floor is 0.010 mm on a Ø6 ball and barely moves with the cell, while
    /// the term that actually bites is `cell² / (8·(ρ − R))` on a concave
    /// feature of radius ρ, which the plane sweep cannot see.
    #[must_use]
    pub fn for_cutter(cutter: &dyn MillingCutter, tolerance_mm: f64) -> Self {
        Self {
            cell_mm: (cutter.cusp_radius_mm() / 2.0).clamp(MIN_REACH_CELL_MM, MAX_REACH_CELL_MM),
            tolerance_mm,
            margin_mm: 0.5,
        }
    }
}

/// Everything one reach-map walk needs, owned, so the walk can run on a
/// thread that does not hold the session.
///
/// The `mesh` is whatever frame the caller machines in — for a session that
/// is the **setup-transformed** mesh, the same `Arc` the generator drops
/// against. Dropping against the untransformed model would answer for the
/// wrong face on a flipped setup.
///
/// `tool_id` / `model_id` are the session identities the answer is stamped
/// with. They are part of the memo key, so a map is never served under the
/// wrong name.
#[derive(Clone)]
pub struct ReachMapRequest {
    pub mesh: Arc<TriangleMesh>,
    pub index: Arc<SpatialIndex>,
    pub cutter: Arc<ToolDefinition>,
    pub params: ReachMapParams,
    pub tool_id: usize,
    pub model_id: usize,
    /// Where `params.tolerance_mm` came from. Not part of the memo key — it
    /// is derived from the tolerance, which is keyed — but it rides the
    /// request so a surface that reports the answer can name the bar's
    /// provenance without re-resolving it.
    pub tolerance_source: ReachToleranceSource,
}

impl std::fmt::Debug for ReachMapRequest {
    /// Hand-written because [`ToolDefinition`] is not `Debug`. A request
    /// prints as its identities and its dials — what a trace line or a
    /// failing sentry needs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachMapRequest")
            .field("triangles", &self.mesh.faces.len())
            .field("params", &self.params)
            .field("tool_id", &self.tool_id)
            .field("model_id", &self.model_id)
            .field("tolerance_source", &self.tolerance_source)
            .finish()
    }
}

/// Per-cell reach gaps over a regular XY grid, plus the area-weighted
/// verdict.
///
/// Row-major `r * nx + c`, cell centre at
/// `(origin_x + c * cell_mm, origin_y + r * cell_mm)` — the same convention
/// as [`crate::tier_map::TierMap`] and [`crate::grid2::Grid2`].
///
/// `None` in [`Self::cells`] means **not measured**, never zero: no surface
/// under that XY, or no CL position within an envelope radius held the tool
/// up. The cells are `Option<f32>` rather than `f32` with `NaN` on purpose —
/// `serde_json` writes `NaN` as `null` and cannot read it back, so a `NaN`
/// grid would not survive its own round trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReachMap {
    pub nx: usize,
    pub ny: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_mm: f64,
    /// Gap (mm) between the surface this cutter would leave and the true
    /// mesh. `None` = not measured.
    pub cells: Vec<Option<f32>>,
    /// The gap (mm) above which a cell is unreachable.
    pub tolerance_mm: f64,
    /// Unreachable share of the MEASURED surface, area-weighted by triangle
    /// area — not vertex-counted and not cell-counted. `0.0` with a zero
    /// [`Self::measured_area_mm2`] is an empty population, not a clean part;
    /// use [`Self::is_measured`].
    ///
    /// # The area base, stated because it moves the number by 3.5 points
    ///
    /// Both halves of this ratio are **true 3D surface area** (each triangle
    /// weighted by its own area, not by its XY footprint), over the
    /// **rim-eroded** population only. Neither is arbitrary and neither is
    /// free:
    ///
    /// * 3D area rather than planar. The unreachable places on a terrain are
    ///   the steep ones, and a steep triangle carries `sec θ` times the area
    ///   of its footprint. On the wanaka board the mean `sec θ` is 1.34, and
    ///   weighting by 3D area rather than by planar cell area moves the
    ///   answer from 53.4 % to 58.6 % at the same bar.
    /// * eroded rather than whole-board. The band one envelope radius inside
    ///   the outline is NOT MEASURED ([`rim_keep_mask`]), so it is out of the
    ///   denominator as well as the numerator — 52 835 mm² of 56 295 on that
    ///   board.
    ///
    /// A comparison against any other instrument must be put on this base
    /// first. P5.2 found a 12-point "unexplained offset" against an
    /// independent rasteriser that turned out to be, in the largest part,
    /// the rasteriser answering on a planar whole-board base.
    pub unreachable_fraction: f64,
    /// Worst gap (mm) over the measured cells.
    pub max_gap_mm: f64,
    /// Total 3D area (mm²) of every triangle in the mesh.
    pub surface_area_mm2: f64,
    /// 3D area (mm²) of the triangles the verdict is taken over — upward
    /// facing and topmost at their own centroid, with a measured cell under
    /// them.
    pub measured_area_mm2: f64,
    /// 3D area (mm²) of the measured triangles whose gap exceeds the bar —
    /// the tolerance, or this cell's own resolution floor where that is
    /// coarser. See [`Self::unresolved_area_mm2`] for the band between.
    pub unreachable_area_mm2: f64,
    /// 3D area (mm²) of the measured triangles whose gap is above the
    /// tolerance but **at or under the grid's own resolution floor there** —
    /// neither reached nor proven missed. `0.0` means the whole population
    /// was resolved; it is never `None`, because the walk always evaluates
    /// this.
    ///
    /// This is the band that made the operator's wanaka terrain read red on
    /// slopes a ball forms: at a 0.05 mm bar and a 0.645 mm cell the grid's
    /// own floor on that surface was 0.03–0.07 mm, so the bar sat *under*
    /// the arithmetic. A gap in this band is a statement about the grid, not
    /// about the tool.
    #[serde(default)]
    pub unresolved_area_mm2: f64,
    /// How much of a gap this grid cannot resolve (mm) — the larger of
    /// [`Self::profile_floor_mm`] and [`Self::curvature_floor_p95_mm`]. A
    /// reported gap of this order is discretisation, not geometry.
    ///
    /// **This number changed meaning on 2026-09-08 (F1).** It used to be the
    /// profile term alone, which is a *plane* measurement and nearly
    /// cell-independent because the kernel radii are sine-spaced. Measured on
    /// the wanaka case (Ø4 tapered ball, 0.645 mm cell): the profile term is
    /// **0.132 mm** and the curvature term **0.234 mm**, so the old published
    /// figure understated the grid's real limit there by 1.8x.
    ///
    /// The understatement is the smaller half of the finding. The larger half
    /// is that 0.132 mm was ALREADY far above the 0.05 mm default bar, so the
    /// module's own advice — treat a gap of this order as arithmetic — was
    /// enough to distrust that reading, and **no operator surface said so**.
    /// The verdict counted the whole sub-floor band as unreachable and
    /// nothing printed the comparison. That is what
    /// [`Self::unresolved_area_mm2`] and [`Self::tolerance_below_floor`]
    /// exist for.
    pub discretisation_floor_mm: f64,
    /// The old plane-only floor: the profile-sampling shortfall of
    /// [`sampling_floor_mm`], kept under its own name so the two terms can be
    /// read apart. Exact on a plane of any slope, blind to curvature.
    #[serde(default)]
    pub profile_floor_mm: f64,
    /// p95 of the per-cell curvature term ([`curvature_floor_plane`]) over
    /// the measured cells — the term the profile sweep cannot see.
    ///
    /// p95 rather than the maximum: one apex cell on a V-groove would
    /// otherwise set the floor for a whole board. Exactly `0.0` on any plane
    /// at any slope, because `tip_z` is linear there.
    #[serde(default)]
    pub curvature_floor_p95_mm: f64,
    /// Width (mm) of the band inside the mesh footprint boundary that the
    /// map ABSTAINS on — one envelope radius. See [`rim_keep_mask`] for what
    /// the min-filter cannot know there and the 0.20 mm artefact that
    /// measured it.
    pub rim_erosion_mm: f64,
    /// The per-cell resolution floor (mm) the verdict used, read at each
    /// cell's BINDING tap — row-major beside [`Self::cells`], `NaN` where the
    /// map reports nothing.
    ///
    /// Stored so the OVERLAY can paint the same three-way answer the
    /// percentages count. Colouring an unresolved cell green would say
    /// "reached" where the map abstained, and colouring it red would say
    /// "missed" where it did not; grey is the third thing, and it needs this
    /// vector to know which cells get it. ~4 bytes a cell, 0.5 MB on a
    /// budget-sized grid.
    #[serde(default)]
    pub cell_floor_mm: Vec<f32>,
    /// The library tool the map was built for. `None` when the map was built
    /// outside a session (a fixture, a probe) and no id exists.
    pub tool_id: Option<usize>,
    /// The model the map was built over. `None` for the same reason.
    pub model_id: Option<usize>,
}

impl ReachMap {
    /// Did the walk find any surface to judge? A map over a mesh with no
    /// upward-facing topmost triangle reports `unreachable_fraction 0.0` and
    /// is indistinguishable from a fully reachable part without this.
    #[must_use]
    pub fn is_measured(&self) -> bool {
        self.measured_area_mm2 > 0.0
    }

    /// Unreachable share as a percentage of measured surface area.
    #[must_use]
    pub fn unreachable_pct(&self) -> f64 {
        self.unreachable_fraction * 100.0
    }

    /// Share of the measured area the grid could not resolve at this
    /// tolerance, as a percentage. Read it beside
    /// [`Self::unreachable_pct`]: a large unresolved share means the answer
    /// is grid-limited and the two percentages should be quoted together.
    #[must_use]
    pub fn unresolved_pct(&self) -> f64 {
        if self.measured_area_mm2 > 0.0 {
            self.unresolved_area_mm2 / self.measured_area_mm2 * 100.0
        } else {
            0.0
        }
    }

    /// Is the bar under the grid's own arithmetic? Every surface that prints
    /// the percentage must print this too.
    ///
    /// # Which way the error runs, because this doc had it BACKWARDS
    ///
    /// `machined_z` is a minimum taken over a SAMPLED set of CL positions.
    /// A minimum over a subset is at or above the minimum over the continuum,
    /// so the reported gap is at or above the true gap — **the bias is
    /// non-negative, always**. The map therefore counts too MANY cells past
    /// the bar, never too few:
    ///
    /// * `reached` is SOUND. A cell the map calls reached has
    ///   `true gap <= reported gap <= tolerance`, so it really is formed.
    /// * `unreachable` OVER-states. Measured on the wanaka board against an
    ///   independent closing on the same area base: 59.05 % against a true
    ///   58.6 % at the 0.05 mm bar, 51.11 against 42.1 at 0.146, and 36.36
    ///   against 26.8 at 0.30.
    /// * a truly unreachable cell can never be classified `reached`, so what
    ///   the grid loses lands in `unresolved`, not in a false clean bill.
    ///
    /// This method's doc, [`Self::grid_note`] and four operator surfaces all
    /// said "lower bound" until 2026-09-08. That was exactly inverted, and it
    /// pointed a reader the wrong way at the one moment they were being told
    /// to distrust the number.
    #[must_use]
    pub fn tolerance_below_floor(&self) -> bool {
        self.discretisation_floor_mm > self.tolerance_mm
    }

    /// The area base, in words — the sentence every surface that prints a
    /// percentage owes beside it.
    ///
    /// P5.2: an unstated base is what turned a 3.5-point weighting
    /// difference into a hunt for a phantom instrument defect. Both halves of
    /// every percentage here are true 3D surface area over the rim-eroded
    /// population.
    #[must_use]
    pub fn area_basis_note(&self) -> String {
        format!(
            "of 3D surface area, rim-eroded {:.1} mm",
            self.rim_erosion_mm
        )
    }

    /// The single sentence that says which way this grid's error runs.
    ///
    /// One construction site, quoted by the panel, the MCP reply and the
    /// inspector, so the three cannot describe the bias three ways — which is
    /// how "lower bound" survived on four surfaces at once.
    #[must_use]
    pub fn over_statement_note(&self) -> String {
        format!(
            "the grid over-states gaps by up to its floor ({:.3} mm), so the true unreachable share is AT OR BELOW {:.1} %, and the {:.1} % unresolved is the band the grid cannot classify either way",
            self.discretisation_floor_mm,
            self.unreachable_pct(),
            self.unresolved_pct()
        )
    }

    /// One line naming the grid and what it can resolve, for the panel
    /// legend, the MCP reply and the CLI to share — so three surfaces cannot
    /// describe one grid three ways.
    #[must_use]
    pub fn grid_note(&self) -> String {
        let mut note = format!(
            "cell {:.3} mm \u{00B7} floor {:.3} mm \u{00B7} tol {:.3} mm",
            self.cell_mm, self.discretisation_floor_mm, self.tolerance_mm
        );
        note.push_str(&format!(" \u{00B7} {}", self.area_basis_note()));
        if self.tolerance_below_floor() {
            note.push_str(&format!(
                " \u{2014} the bar is UNDER the floor: {}",
                self.over_statement_note()
            ));
        }
        note
    }

    /// The grid cell whose centre is nearest `(x, y)`, or `None` off the
    /// grid.
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

    /// Gap (mm) at `(x, y)`, from the nearest cell. `None` = not measured.
    #[must_use]
    pub fn gap_at(&self, x: f64, y: f64) -> Option<f32> {
        let (row, col) = self.nearest_cell(x, y)?;
        self.cells.get(row * self.nx + col).copied().flatten()
    }

    /// One gap per mesh vertex, for per-vertex colouring.
    ///
    /// `f32::NAN` means **not measured** — off the grid, no surface, or the
    /// vertex is not on the topmost surface at its own XY (an underside, the
    /// foot of a wall). The caller renders those with the mesh's normal
    /// colour; [`reach_color`] does that for you.
    ///
    /// Takes the spatial index because the topmost-surface test is the exact
    /// zero-radius predicate, not a grid lookup: at half a cell on a steep
    /// flank the grid's own surface Z is a different number. Every caller
    /// already holds a memoised index
    /// ([`crate::geom_cache::cached_auto_index`]).
    #[must_use]
    pub fn vertex_gaps(&self, mesh: &TriangleMesh, index: &SpatialIndex) -> Vec<f32> {
        mesh.vertices
            .iter()
            .map(|v| {
                match surface_z_at(v.x, v.y, mesh, index) {
                    // Not the topmost surface here: an underside or the foot
                    // of a wall. Not measured, never "reachable".
                    Some(top) if top - v.z > TOP_SURFACE_EPS_MM => f32::NAN,
                    Some(_) => self.gap_at(v.x, v.y).unwrap_or(f32::NAN),
                    None => f32::NAN,
                }
            })
            .collect()
    }

    /// The per-vertex resolution floor, positionally beside
    /// [`Self::vertex_gaps`] — what [`reach_color`] needs to paint an
    /// UNRESOLVED vertex grey rather than calling it reached or missed.
    ///
    /// `NaN` means the same thing it means in the gap vector: not measured
    /// here, or no floor for this cell, in which case the colour falls back
    /// to the tolerance alone.
    #[must_use]
    pub fn vertex_floors(&self, mesh: &TriangleMesh) -> Vec<f32> {
        mesh.vertices
            .iter()
            .map(|v| {
                self.nearest_cell(v.x, v.y)
                    .and_then(|(row, col)| self.cell_floor_mm.get(row * self.nx + col))
                    .copied()
                    .unwrap_or(f32::NAN)
            })
            .collect()
    }

    /// One colour per model vertex — the whole overlay in one pass, so the
    /// gap, the floor and the ramp cannot be resolved from three different
    /// maps.
    #[must_use]
    pub fn vertex_colors(&self, mesh: &TriangleMesh, index: &SpatialIndex) -> Vec<[f32; 3]> {
        reach_colors(
            &self.vertex_gaps(mesh, index),
            &self.vertex_floors(mesh),
            self.ramp(),
        )
    }

    /// A coarse histogram of the measured gaps, for a wire surface that must
    /// not carry the whole grid: `bins` equal-width buckets over
    /// `0..=max_gap_mm`, plus the not-measured count.
    ///
    /// Returns `(bin_upper_edges_mm, counts, not_measured)`.
    #[must_use]
    pub fn gap_histogram(&self, bins: usize) -> (Vec<f64>, Vec<usize>, usize) {
        let bins = bins.max(1);
        let top = if self.max_gap_mm > 0.0 {
            self.max_gap_mm
        } else {
            self.tolerance_mm.max(1e-6)
        };
        let width = top / bins as f64;
        let edges: Vec<f64> = (1..=bins).map(|i| i as f64 * width).collect();
        let mut counts = vec![0usize; bins];
        let mut not_measured = 0usize;
        for cell in &self.cells {
            match cell {
                None => not_measured += 1,
                Some(gap) => {
                    let slot = ((f64::from(*gap) / width).floor() as usize).min(bins - 1);
                    if let Some(count) = counts.get_mut(slot) {
                        *count += 1;
                    }
                }
            }
        }
        (edges, counts, not_measured)
    }

    /// Stamp the session identities onto a map the walk built without them.
    #[must_use]
    pub fn with_ids(mut self, tool_id: usize, model_id: usize) -> Self {
        self.tool_id = Some(tool_id);
        self.model_id = Some(model_id);
        self
    }
}

/// The stops a reach overlay's colour ramp runs between — one struct so the
/// mesh, the legend and any screenshot cannot describe three different ramps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReachRamp {
    /// The bar. At or under it the surface is formed: green.
    pub tolerance_mm: f64,
    /// The published resolution floor. Between the bar and a cell's own floor
    /// the answer is UNRESOLVED and paints grey.
    pub floor_mm: f64,
    /// The deepest measured gap — the far end of the depth ramp.
    pub max_gap_mm: f64,
}

impl ReachRamp {
    /// Where the depth ramp starts: the bar, or the floor where that is
    /// coarser, so the ramp begins exactly where the verdict starts calling
    /// cells missed.
    #[must_use]
    pub fn bar_mm(&self) -> f64 {
        self.tolerance_mm.max(self.floor_mm).max(1e-6)
    }

    /// The ramp's midpoint in millimetres. The ramp is LOG-scaled, so this is
    /// the geometric mean of the bar and the deepest gap — the number the
    /// legend prints between its two ends.
    #[must_use]
    pub fn mid_stop_mm(&self) -> f64 {
        let bar = self.bar_mm();
        (bar * self.max_gap_mm.max(bar)).sqrt()
    }

    /// Position on the depth ramp, `0..=1`, for a gap past the bar.
    ///
    /// **Log, not linear, and the terrain is why.** On the wanaka board 83 %
    /// of the measured cells sit under 0.56 mm and under 1 % over 1.7 mm,
    /// against a 4.46 mm deepest gorge. A linear ramp puts that whole 83 %
    /// into the first eighth of its range, so a 0.3 mm near-miss and a 4.5 mm
    /// gorge floor arrive at the eye as the same red. A log ramp spends half
    /// its range under the geometric mean, which is where the population is.
    #[must_use]
    pub fn depth_t(&self, gap_mm: f64) -> f64 {
        let bar = self.bar_mm();
        let top = self.max_gap_mm.max(bar * 1.000_001);
        let span = (top / bar).ln();
        if !span.is_finite() || span <= 0.0 {
            return 0.0;
        }
        ((gap_mm.max(bar) / bar).ln() / span).clamp(0.0, 1.0)
    }
}

impl ReachMap {
    /// The ramp this map's colours and legend share.
    #[must_use]
    pub fn ramp(&self) -> ReachRamp {
        ReachRamp {
            tolerance_mm: self.tolerance_mm,
            floor_mm: self.discretisation_floor_mm,
            max_gap_mm: self.max_gap_mm,
        }
    }
}

/// Colour for one vertex gap, shared by the 3D overlay and its legend so the
/// two cannot drift — the same discipline
/// [`crate::rest_heatmap_mesh::rest_ramp_color`] keeps.
///
/// Four states, not two:
///
/// * **not measured** (`NaN` gap) → the model mesh's own neutral diffuse
///   colour, so an underside looks like plain model rather than like a
///   reachable surface;
/// * **reached** (`gap <= tolerance`) → green;
/// * **unresolved** (past the bar but at or under this CELL's own floor) →
///   **grey**. Not green: the map did not say the tool forms this. Not red:
///   it did not say the tool misses it either. It is the third answer, and
///   before this it was painted as a miss;
/// * **missed** → a LOG-scaled depth ramp from the bar to the deepest gap,
///   pale yellow → red → dark red.
///
/// The depth ramp is the P5.2 operator finding. The old ramp ran green→red
/// over `tol..5·tol` and saturated, so on the wanaka terrain a 0.3 mm miss
/// and the 4.46 mm gorge floor were the same red and the shape of the
/// distribution was invisible — which is exactly the shape that says which
/// valleys need the finer tool.
#[must_use]
pub fn reach_color(gap_mm: f32, cell_floor_mm: f32, ramp: ReachRamp) -> [f32; 3] {
    const NEUTRAL: [f32; 3] = [0.6, 0.55, 0.5];
    const UNRESOLVED: [f32; 3] = [0.55, 0.55, 0.58];
    if !gap_mm.is_finite() {
        return NEUTRAL;
    }
    let gap = f64::from(gap_mm);
    if gap <= ramp.tolerance_mm {
        let t = (gap / ramp.tolerance_mm.max(1e-6)).clamp(0.0, 1.0) as f32;
        return [0.10 + 0.25 * t, 0.70 - 0.08 * t, 0.20 + 0.08 * t];
    }
    // The cell's OWN floor decides the abstention, exactly as the area
    // verdict does — a cell with no floor (the grid rim) takes the tolerance
    // alone, which is the conservative direction.
    let floor = f64::from(cell_floor_mm);
    if floor.is_finite() && gap <= floor {
        return UNRESOLVED;
    }
    let t = ramp.depth_t(gap) as f32;
    // Pale yellow -> red -> dark red: monotone in lightness, so depth reads
    // as depth even in a greyscale screenshot.
    if t < 0.5 {
        let u = t * 2.0;
        [1.00 - 0.10 * u, 0.90 - 0.65 * u, 0.30 - 0.20 * u]
    } else {
        let u = (t - 0.5) * 2.0;
        [0.90 - 0.55 * u, 0.25 - 0.25 * u, 0.10 + 0.02 * u]
    }
}

/// [`reach_color`] over whole gap and floor vectors.
///
/// The two slices are read positionally beside each other; a missing floor
/// entry is `NaN`, which takes the tolerance-only branch.
#[must_use]
pub fn reach_colors(gaps: &[f32], floors: &[f32], ramp: ReachRamp) -> Vec<[f32; 3]> {
    gaps.iter()
        .enumerate()
        .map(|(i, &gap)| reach_color(gap, floors.get(i).copied().unwrap_or(f32::NAN), ramp))
        .collect()
}

/// The model surface as a renderable mesh coloured by reach — the form the
/// headless composite renderer ([`crate::fingerprint::render_toolpath_composite`])
/// takes a background in.
///
/// One vertex per model vertex and the model's own index buffer, so the
/// colour at index *i* is [`reach_color`] of `gaps[i]`. It shares
/// [`reach_color`] with the live viewport overlay and its legend, so a
/// screenshot and the screen cannot show two different verdicts — the same
/// discipline [`crate::rest_heatmap_mesh::rest_ramp_color`] keeps.
#[must_use]
pub fn reach_overlay_stock_mesh(
    mesh: &TriangleMesh,
    gaps: &[f32],
    floors: &[f32],
    ramp: ReachRamp,
) -> crate::stock_mesh::StockMesh {
    let mut vertices = Vec::with_capacity(mesh.vertices.len() * 3);
    let mut colors = Vec::with_capacity(mesh.vertices.len() * 3);
    for (i, v) in mesh.vertices.iter().enumerate() {
        vertices.push(v.x as f32);
        vertices.push(v.y as f32);
        vertices.push(v.z as f32);
        let rgb = reach_color(
            gaps.get(i).copied().unwrap_or(f32::NAN),
            floors.get(i).copied().unwrap_or(f32::NAN),
            ramp,
        );
        colors.extend_from_slice(&rgb);
    }
    let indices = mesh
        .triangles
        .iter()
        .flat_map(|t| t.iter().copied())
        .collect();
    crate::stock_mesh::StockMesh {
        vertices,
        indices,
        colors,
    }
}

/// The true mesh surface Z at `(x, y)`: the highest triangle plane whose XY
/// footprint contains the point, or `None` if the vertical ray misses the
/// mesh entirely.
///
/// This is the exact zero-radius drop, and it uses the same triangle set and
/// the same containment test as
/// [`crate::dropcutter::point_is_over_mesh_xy`] — so "is there surface here"
/// and "how high is it" can never disagree.
#[must_use]
pub fn surface_z_at(x: f64, y: f64, mesh: &TriangleMesh, index: &SpatialIndex) -> Option<f64> {
    let mut best: Option<f64> = None;
    for idx in index.query(x, y, 0.0) {
        let Some(tri) = mesh.faces.get(idx) else {
            continue;
        };
        if !tri.contains_point_xy(x, y) {
            continue;
        }
        let Some(z) = plane_z_at(tri, x, y) else {
            continue;
        };
        best = Some(best.map_or(z, |b: f64| b.max(z)));
    }
    best
}

/// Z of the triangle's own plane above `(x, y)`. `None` for a plane that is
/// vertical (or degenerate), where the question has no answer.
fn plane_z_at(tri: &crate::geo::Triangle, x: f64, y: f64) -> Option<f64> {
    let n = tri.normal;
    if n.z.abs() < 1e-12 {
        return None;
    }
    let v0 = tri.v.first()?;
    let z = v0.z - (n.x * (x - v0.x) + n.y * (y - v0.y)) / n.z;
    z.is_finite().then_some(z)
}

/// The grid a walk lays over the mesh bbox. Same row-major convention as
/// [`ReachMap`], which it becomes.
#[derive(Debug, Clone, Copy)]
struct GridSpec {
    nx: usize,
    ny: usize,
    origin_x: f64,
    origin_y: f64,
    cell_mm: f64,
}

impl GridSpec {
    /// Pad past the mesh bbox by the cutter's envelope plus the margin, so
    /// the min-filter kernel has source cells all round the part, then
    /// coarsen until the grid fits [`MAX_REACH_CELLS`].
    fn new(mesh: &TriangleMesh, cutter: &dyn MillingCutter, params: &ReachMapParams) -> Self {
        let pad_mm = cutter.envelope_radius_mm().max(0.0) + params.margin_mm.max(0.0);
        let bbox = &mesh.bbox;
        let span_x = (bbox.max.x - bbox.min.x).max(0.0);
        let span_y = (bbox.max.y - bbox.min.y).max(0.0);
        let mut cell_mm = params.cell_mm.max(1e-3);
        // Coarsen rather than truncate: a half-resolution answer over the
        // whole part beats a full-resolution answer over part of it.
        for _ in 0..24 {
            let spec = Self::at_cell(span_x, span_y, bbox.min.x, bbox.min.y, pad_mm, cell_mm);
            if spec.nx.saturating_mul(spec.ny) <= MAX_REACH_CELLS {
                return spec;
            }
            cell_mm *= 1.25;
        }
        Self::at_cell(span_x, span_y, bbox.min.x, bbox.min.y, pad_mm, cell_mm)
    }

    fn at_cell(
        span_x: f64,
        span_y: f64,
        min_x: f64,
        min_y: f64,
        pad_mm: f64,
        cell_mm: f64,
    ) -> Self {
        let margin_cells = (pad_mm / cell_mm).ceil().max(0.0) as usize + 1;
        let padding = 2 * margin_cells + 1;
        Self {
            nx: (span_x / cell_mm).ceil().max(0.0) as usize + padding,
            ny: (span_y / cell_mm).ceil().max(0.0) as usize + padding,
            origin_x: min_x - margin_cells as f64 * cell_mm,
            origin_y: min_y - margin_cells as f64 * cell_mm,
            cell_mm,
        }
    }

    fn x_of(&self, col: usize) -> f64 {
        self.origin_x + col as f64 * self.cell_mm
    }

    fn y_of(&self, row: usize) -> f64 {
        self.origin_y + row as f64 * self.cell_mm
    }
}

/// Run `row_fn` over every grid row and concatenate the results, polling
/// `cancel` once per row.
///
/// One site for both build configurations, so the cancellation granularity
/// cannot drift between them — the same rule
/// [`crate::tier_map`]'s `walk_rows` keeps.
fn walk_rows<T: Send>(
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
    row_fn: impl Fn(usize) -> Vec<T> + Sync,
) -> Result<Vec<T>, Cancelled> {
    #[cfg(feature = "parallel")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = AtomicBool::new(false);
        let collected: Vec<T> = (0..grid.ny)
            .into_par_iter()
            .flat_map(|row| {
                if cancelled.load(Ordering::Relaxed) || cancel.cancelled() {
                    cancelled.store(true, Ordering::Relaxed);
                    return Vec::new();
                }
                row_fn(row)
            })
            .collect();
        if cancelled.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        Ok(collected)
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut collected: Vec<T> = Vec::with_capacity(grid.nx * grid.ny);
        for row in 0..grid.ny {
            check_cancel(cancel)?;
            collected.extend(row_fn(row));
        }
        Ok(collected)
    }
}

/// One min-filter kernel tap: an XY offset in mm and the cutter's profile
/// rise at that distance from the tip.
struct KernelTap {
    dx_mm: f64,
    dy_mm: f64,
    rise_mm: f64,
}

/// Upper bound on kernel taps, so a big envelope on a fine cell cannot turn
/// the min-filter into the expensive half of the walk. Reached only past
/// roughly `envelope / cell = 12`, which no shipped finishing tool and cell
/// pair comes near.
const MAX_KERNEL_TAPS: usize = 4096;

/// The cutter profile sampled in **polar** coordinates, not on the grid.
///
/// The offsets are sub-cell and `tip_z` is read back by bilinear
/// interpolation ([`sample_tip_z`]) rather than by a grid lookup. That is not
/// refinement for its own sake — a grid-snapped kernel is *wrong* on two
/// shipped tool shapes, and the sentries caught both:
///
/// * **A flat or bull tip binds at exactly `r = envelope`.** Snap the kernel
///   to the grid and the largest tap lands at `r ≤ envelope`, short by up to
///   one cell, so a plane reads `(envelope − r_tap)·tan θ` of gap — a
///   measured **0.20 mm** on a Ø6 flat at 45° with 0.4 mm cells, four times
///   a 0.05 mm tolerance, painting every flank red.
/// * **A small ball binds near `r = R·sin θ`.** On a Ø1 ball in a 60° wall at
///   0.3 mm cells the nearest grid tap was 0.3 mm against a binding radius of
///   0.433 mm, and a plain wall read **0.08 mm** of gap.
///
/// Both are cured by sampling the radius the profile actually binds at.
/// `r = envelope` is always in the set; the interior pitch is half a cell or
/// an eighth of the envelope, whichever is finer.
///
/// Taps past the envelope are dropped, never clamped: `height_at_radius`
/// returns `None` there, which means "this cutter imposes no constraint at
/// that distance", not "the constraint is zero" — clamping would let a
/// position outside the tool cut, i.e. would model a gouge.
fn profile_kernel(cutter: &dyn MillingCutter, cell_mm: f64) -> Vec<KernelTap> {
    let reach = cutter.envelope_radius_mm().max(0.0);
    let mut taps = Vec::new();
    let Some(centre_rise) = cutter.height_at_radius(0.0) else {
        return taps;
    };
    taps.push(KernelTap {
        dx_mm: 0.0,
        dy_mm: 0.0,
        rise_mm: centre_rise,
    });
    if reach <= 0.0 {
        return taps;
    }
    let pitch = kernel_pitch_mm(reach, cell_mm);
    for r in kernel_radii(reach, pitch) {
        let Some(rise_mm) = cutter.height_at_radius(r) else {
            continue;
        };
        if !rise_mm.is_finite() {
            continue;
        }
        // Arc pitch matches the radial pitch, so the binding DIRECTION is
        // resolved as finely as the binding radius.
        let count = ((2.0 * std::f64::consts::PI * r / pitch).ceil() as usize).clamp(1, 256);
        for j in 0..count {
            let angle = 2.0 * std::f64::consts::PI * j as f64 / count as f64;
            taps.push(KernelTap {
                dx_mm: r * angle.cos(),
                dy_mm: r * angle.sin(),
                rise_mm,
            });
        }
    }
    taps
}

/// Radial and angular sampling pitch (mm) for [`profile_kernel`], coarsened
/// until the kernel fits [`MAX_KERNEL_TAPS`].
///
/// **The budget coarsens the pitch; it never truncates the ring list.** An
/// earlier version stopped emitting rings once the tap count passed the cap,
/// and because the rings run inner to outer the ring it dropped was
/// `r = envelope` — the one tap that fixes a flat tip's binding radius and a
/// steep ball's. A cap that silently deletes the fix it was written beside is
/// worse than a coarser kernel. Tap count grows as roughly
/// `(reach / pitch)²`, so one step of 1.25 removes about a third of them and
/// the loop converges in a few passes.
fn kernel_pitch_mm(reach_mm: f64, cell_mm: f64) -> f64 {
    let mut pitch = (cell_mm / 2.0).min(reach_mm / 8.0).max(1e-4);
    for _ in 0..32 {
        if estimated_tap_count(reach_mm, pitch) <= MAX_KERNEL_TAPS {
            return pitch;
        }
        pitch *= 1.25;
    }
    pitch
}

/// How many taps [`profile_kernel`] would emit at this pitch. Shares
/// [`kernel_radii`] with the kernel itself, so the budget cannot be checked
/// against a different ring list than the one that gets built.
fn estimated_tap_count(reach_mm: f64, pitch_mm: f64) -> usize {
    kernel_radii(reach_mm, pitch_mm)
        .into_iter()
        .map(|r| ((2.0 * std::f64::consts::PI * r / pitch_mm).ceil() as usize).clamp(1, 256))
        .sum::<usize>()
        + 1
}

/// The tap radii, interior first and the envelope last.
///
/// **Sine-spaced, not uniform**, and that is worth a paragraph. A ball of
/// radius `R` on a plane of slope θ binds at `r = R·sin θ`, so sine spacing
/// samples the binding radius uniformly in the ANGLE the map is used over.
/// Uniform spacing does not, and the profile's curvature
/// `h''(r) = R²/(R² − r²)^{3/2}` blows up exactly where the steep slopes bind:
/// on a Ø6 ball at 0.61 mm cells the measured floor was **0.171 mm** with
/// uniform radii and one seventh of that with these. The last radius is the
/// envelope exactly, which is what fixes a flat or bull tip
/// ([`profile_kernel`]).
fn kernel_radii(reach_mm: f64, pitch_mm: f64) -> Vec<f64> {
    let quarter_turn = std::f64::consts::FRAC_PI_2;
    let step = (pitch_mm / reach_mm).min(quarter_turn).max(1e-6);
    let rings = (quarter_turn / step).ceil().max(1.0) as usize;
    (1..=rings)
        .map(|j| {
            if j == rings {
                reach_mm
            } else {
                reach_mm * (j as f64 * step).sin()
            }
        })
        .collect()
}

/// How much gap this kernel cannot resolve (mm), measured rather than
/// assumed: the largest shortfall between the continuum support function
/// `max over r of [ r·tan θ − height_at_radius(r) ]` and the same maximum
/// taken over the kernel's own radii, swept over the slopes the map is used
/// on.
///
/// The support function is the plane gap the min-filter must cancel exactly
/// (see the module doc), so its sampling error IS the floor. Reported on the
/// map rather than subtracted from it.
fn sampling_floor_mm(cutter: &dyn MillingCutter, cell_mm: f64) -> f64 {
    let reach = cutter.envelope_radius_mm().max(0.0);
    if reach <= 0.0 {
        return 0.0;
    }
    let continuum = continuum_support(cutter, reach);
    floor_against(cutter, reach, cell_mm, &continuum)
}

/// Slope tangents the floor is swept over: 0° to 75° in 5° steps, the band
/// [`crate::tier_map::MAX_COMPENSATED_SLOPE_DEG`] names as the last angle at
/// which a vertical-gap reading is a measurement at all.
fn floor_slope_tangents() -> [f64; 16] {
    let mut tangents = [0.0f64; 16];
    for (step, slot) in tangents.iter_mut().enumerate() {
        *slot = (step as f64 * 5.0).to_radians().tan();
    }
    tangents
}

/// The continuum support maximum per slope — the answer the sampled kernel is
/// scored against.
///
/// **Cell-independent by construction**, which is why it is a separate
/// function. It was hoisted out of a bisection in
/// [`ReachMapParams::for_cutter`] that no longer exists (F5, 2026-09-08); the
/// split is kept because [`sampling_floor_mm`] still wants the two halves
/// apart, and because it documents which half of the floor moves with the
/// cell and which does not.
fn continuum_support(cutter: &dyn MillingCutter, reach_mm: f64) -> [f64; 16] {
    const SAMPLES: usize = 256;
    let mut best = [f64::NEG_INFINITY; 16];
    let tangents = floor_slope_tangents();
    for i in 0..=SAMPLES {
        let r = reach_mm * i as f64 / SAMPLES as f64;
        let Some(h) = cutter.height_at_radius(r) else {
            continue;
        };
        for (slot, tan) in best.iter_mut().zip(tangents) {
            *slot = slot.max(r * tan - h);
        }
    }
    best
}

/// The floor at one cell, against a [`continuum_support`] already taken.
fn floor_against(
    cutter: &dyn MillingCutter,
    reach_mm: f64,
    cell_mm: f64,
    continuum: &[f64; 16],
) -> f64 {
    let pitch = kernel_pitch_mm(reach_mm, cell_mm);
    let radii = kernel_radii(reach_mm, pitch);
    let tangents = floor_slope_tangents();
    let centre = cutter
        .height_at_radius(0.0)
        .map_or(f64::NEG_INFINITY, |h| -h);
    let mut sampled = [centre; 16];
    for &r in &radii {
        let Some(h) = cutter.height_at_radius(r) else {
            continue;
        };
        for (slot, tan) in sampled.iter_mut().zip(tangents) {
            *slot = slot.max(r * tan - h);
        }
    }
    let mut worst = 0.0f64;
    for (cont, samp) in continuum.iter().zip(sampled) {
        if cont.is_finite() && samp.is_finite() {
            worst = worst.max(cont - samp);
        }
    }
    worst.max(0.0)
}

/// The per-cell resolution floor (mm) the CELL SIZE imposes on this
/// surface — the term [`sampling_floor_mm`] cannot see.
///
/// # What it measures, and why the plane sweep misses it
///
/// `machined_z` is a minimum over CL positions **spaced at the cell**, read
/// back between them by [`sample_tip_z`]. A minimum over a subset over-states,
/// and an interpolation of a locally convex field over-reads, so both halves
/// push the gap the SAME way: up. On a plane neither happens — `tip_z` is
/// linear, bilinear interpolation of a linear field is exact, and the only
/// residual is the profile term. On a curved surface the error is of order
/// `cell² · κ / 8` where `κ` is the curvature of the `tip_z` field, which for
/// a tool of radius `R` bridging a concave feature of radius `ρ` is
/// `1/(ρ − R)`. At a 0.645 mm cell and `ρ − R = 1 mm` that is **0.052 mm —
/// the whole 0.05 mm default tolerance**.
///
/// So it is measured, not assumed, and measured on the field that carries it:
/// the second difference of the built `tip_z` plane. For a twice-differentiable
/// field the bilinear interpolation error over one cell is bounded by
/// `(|∂²/∂x²| + |∂²/∂y²|) · cell² / 8`, and a second difference over the
/// lattice IS `∂² · cell²` — so the bound is simply
/// `(|Δ²ₓ tip_z| + |Δ²_y tip_z|) / 8`, in millimetres, with no cell factor to
/// get wrong.
///
/// # The evidence this was written from
///
/// The wanaka terrain (661 212 triangles, Ø4 tapered ball R2.0, cell
/// 0.645 mm) reported **68.2 %** of its measured area unreachable at the
/// 0.05 mm default. An independent closing of the same STL on a 0.15 mm
/// lattice — same law, no code in common — put the true answer at **55.0 %**,
/// with the worst gap agreeing to three figures (4.32 against 4.35 mm). The
/// residual was reproduced exactly by replicating this module's own sampling
/// scheme at 0.645 mm (**63.7 %**, median gap 0.101 mm against a true
/// 0.051 mm) and it shrank on refinement — 59.5 % at 0.4 mm, 58.6 % at
/// 0.3 mm. The prototype's own floor reading over that board — see the
/// caveat below on which instrument that was — came out median 0.030 mm,
/// p75 0.050 mm, p95 0.075 mm: the same order as the excess it is meant to
/// bound.
///
/// Read together with the truth those figures say something an operator needs
/// said out loud: **the terrain really is mostly unreachable at 0.05 mm** —
/// its own facet roughness puts the MEDIAN gap at the bar — and the map was
/// additionally reporting arithmetic as geometry on top of it. The first half
/// is answered by the bar ([`crate::session`]'s cusp derivation); this
/// function answers the second.
///
/// # Why a 3 x 3 maximum, and not the second difference at the cell itself
///
/// The textbook bound is `(h²·max|∂²f/∂x²| + k²·max|∂²f/∂y²|) / 8` with the
/// maxima taken **over the interpolation cell**, not at one node. A centred
/// second difference estimates `∂² · cell²` *near* the node it is taken at,
/// and the corners of the cell a tap lands in are one node away — so the
/// bound wants the neighbourhood, and a single-node reading would understate
/// it wherever the curvature is itself changing, which on a terrain is
/// everywhere. One cell of dilation is the discrete form of "over this cell".
///
/// # The regime this bound does NOT cover
///
/// `(|Δ²x| + |Δ²y|) / 8` is the bilinear bound for a **twice-differentiable**
/// field. `tip_z` is not one: it has a slope KINK wherever the binding
/// contact switches feature — a ridge, a rim, the two sides of a trench the
/// tool bridges. Across a kink the interpolation over-read is
/// `O(cell · Δslope)`, not `O(cell² · ∂²)`, and this term is blind to it.
///
/// Measured, on the `ρ = 1.5` trough of
/// `tests/reach_map_residual_p5_1.rs` (a R2.0 ball bridging two rims, so
/// `tip_z` kinks at the centre with a one-sided slope of 0.750):
///
/// | cell mm | worst-gap excess over the closed form | this bound | kink envelope `s·cell/2` |
/// |---|---|---|---|
/// | 0.2 | 0.0006 | 0.033 | 0.075 |
/// | 0.3 | 0.0000 | 0.047 | 0.113 |
/// | 0.4 | 0.0000 | 0.058 | 0.150 |
/// | 0.5 | **0.1471** | 0.032 | 0.188 |
/// | 0.6 | 0.0000 | 0.077 | 0.225 |
/// | 0.645 | **0.0889** | 0.061 | 0.242 |
///
/// The excess is **non-monotone in the cell**, which is the signature of grid
/// PHASE against a feature only four cells wide, not of a smooth
/// discretisation law. The kink envelope covers every row; this bound does
/// not cover two of them.
///
/// So read this floor as a bound on the SMOOTH term. It is the term that
/// dominates on a terrain, and it is the one the wanaka reading needed. A
/// kink term of the form `2·t(1−t)·Δ²` (measured as the tight form, `t` being
/// the tap's fractional position in its cell) would cover both regimes, at
/// the cost of being 4x conservative on a smooth surface — which would move
/// the unresolved share on every real part, so it is an operator's call and
/// not a silent one. Ledgered, not fixed.
///
/// The other term this does not cover is the polar tap set's own pitch
/// (`min(cell/2, envelope/8)`), whose contribution scales as `(pitch/cell)²`
/// of the same curvature — a quarter of it at most, and in practice inside
/// the slack the dilation adds.
///
/// **The wanaka figures quoted in this module are the PROTOTYPE's, not this
/// function's.** They were taken with `(|Δ²x| + |Δ²y|)/8` read at the query
/// cell and UNDILATED — median 0.030 mm, p75 0.050, p95 0.075 over that
/// board, against a replicated sampling excess of median 0.050 mm, which is
/// what established the order of magnitude and the sign. This function reads
/// the same quantity at the BINDING TAP and after a 3 x 3 dilation, so its
/// own figure is a different number and is not yet recorded. Print it with
/// `tests/reach_map_residual_p5_1.rs::the_wanaka_terrain_reach_table` before
/// citing one.
///
/// # Where it is READ
///
/// This function returns the bound *at each grid cell*. The consumer is
/// [`gap_plane`], which reads it at the tap that attained each cell's
/// minimum, not at the cell being answered for — see that function for why
/// the two differ by more than a factor of two on a concave flank.
///
/// `NaN` for a cell with no `tip_z`, or on the grid edge where no second
/// difference exists.
#[must_use]
pub fn curvature_floor_plane(tip_z: &[f64], nx: usize, ny: usize) -> Vec<f64> {
    fn second(a: Option<&f64>, b: Option<&f64>, c: Option<&f64>) -> Option<f64> {
        let (a, b, c) = (a?, b?, c?);
        let d = a - 2.0 * b + c;
        d.is_finite().then_some(d.abs())
    }
    let mut raw = vec![f64::NAN; nx.saturating_mul(ny)];
    if nx < 3 || ny < 3 {
        return raw;
    }
    for row in 1..ny - 1 {
        for col in 1..nx - 1 {
            let at = |r: usize, c: usize| tip_z.get(r * nx + c);
            let dx = second(at(row, col - 1), at(row, col), at(row, col + 1));
            let dy = second(at(row - 1, col), at(row, col), at(row + 1, col));
            let (Some(dx), Some(dy)) = (dx, dy) else {
                continue;
            };
            let floor = (dx + dy) / 8.0;
            if let Some(slot) = raw.get_mut(row * nx + col) {
                *slot = floor;
            }
        }
    }
    // One cell of dilation. NaN neighbours are skipped rather than poisoning
    // the maximum, so the band just inside the measurable region keeps a
    // floor instead of losing one.
    let mut out = raw.clone();
    for row in 0..ny {
        for col in 0..nx {
            let mut best = f64::NAN;
            for dr in row.saturating_sub(1)..(row + 2).min(ny) {
                for dc in col.saturating_sub(1)..(col + 2).min(nx) {
                    let Some(v) = raw.get(dr * nx + dc).copied() else {
                        continue;
                    };
                    if v.is_finite() && (!best.is_finite() || v > best) {
                        best = v;
                    }
                }
            }
            if let Some(slot) = out.get_mut(row * nx + col) {
                *slot = best;
            }
        }
    }
    out
}

/// The `q`-quantile of the finite entries, or `0.0` when there are none.
///
/// Nearest-rank on a sorted copy. The population is one grid, so the sort is
/// O(cells log cells) once per walk — negligible beside the drop pass.
fn finite_quantile(values: &[f64], q: f64) -> f64 {
    let mut finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return 0.0;
    }
    finite.sort_by(f64::total_cmp);
    let rank = ((finite.len() as f64 - 1.0) * q.clamp(0.0, 1.0)).round() as usize;
    finite.get(rank).copied().unwrap_or(0.0)
}

/// Pass A: one drop-cutter CL and one exact surface Z per grid cell.
///
/// `NaN` in either plane means "nothing here" — no contact, or no surface
/// under the vertical ray. Both are `f64` while they are working state; they
/// narrow to `f32` only on the way into the [`ReachMap`].
fn drop_and_surface_planes(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<(Vec<f64>, Vec<f64>), Cancelled> {
    let row_fn = |row: usize| -> Vec<(f64, f64)> {
        let y = grid.y_of(row);
        (0..grid.nx)
            .map(|col| {
                let x = grid.x_of(col);
                let cl = point_drop_cutter(x, y, mesh, index, cutter);
                let tip_z = if cl.contacted { cl.z } else { f64::NAN };
                let surface = surface_z_at(x, y, mesh, index).unwrap_or(f64::NAN);
                (tip_z, surface)
            })
            .collect()
    };
    Ok(walk_rows(grid, cancel, row_fn)?.into_iter().unzip())
}

/// The CL plane read at an arbitrary XY by bilinear interpolation, in grid
/// index space.
///
/// A corner with no contact (`NaN`) leaves the stencil and the remaining
/// weights are renormalised; `None` only when the sample is off the grid or
/// no corner contacted anything.
///
/// Dropping the whole tap on one missing corner is the obvious rule and it is
/// **wrong at the part rim**, measurably: a Ø6 flat endmill on a 45° plane
/// binds at a CL exactly one envelope radius downhill, which at the plane's
/// low edge touches the mesh at a single point. One of that sample's four
/// corners has no contact, and an all-or-nothing stencil then falls back to
/// the next tap in and reads **0.2071 mm** of gap along the whole edge —
/// `a_flat_endmill_on_a_slope_is_reachable_too` measured exactly that.
/// Renormalising leans the estimate on the contacted side, which at a rim is
/// the only side that carries information.
///
/// Interpolation, not a nearest-cell lookup, is what makes the min-filter
/// exact on a plane: `tip_z` is linear on a plane, and bilinear
/// interpolation of a linear field is exact.
fn sample_tip_z(tip_z: &[f64], grid: &GridSpec, x: f64, y: f64) -> Option<f64> {
    let fx = (x - grid.origin_x) / grid.cell_mm;
    let fy = (y - grid.origin_y) / grid.cell_mm;
    if !fx.is_finite() || !fy.is_finite() || fx < 0.0 || fy < 0.0 {
        return None;
    }
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as usize, y0 as usize);
    let x1 = (x0 + 1).min(grid.nx.saturating_sub(1));
    let y1 = (y0 + 1).min(grid.ny.saturating_sub(1));
    if x0 >= grid.nx || y0 >= grid.ny {
        return None;
    }
    let at = |r: usize, c: usize| -> Option<f64> {
        let z = *tip_z.get(r * grid.nx + c)?;
        z.is_finite().then_some(z)
    };
    let corners = [
        (y0, x0, (1.0 - tx) * (1.0 - ty)),
        (y0, x1, tx * (1.0 - ty)),
        (y1, x0, (1.0 - tx) * ty),
        (y1, x1, tx * ty),
    ];
    let mut weighted = 0.0;
    let mut total = 0.0;
    for (r, c, w) in corners {
        if let Some(z) = at(r, c) {
            weighted += w * z;
            total += w;
        }
    }
    (total > 0.0).then(|| weighted / total)
}

/// Which cells are far enough inside the mesh footprint for the min-filter
/// to have CL positions all round them — `true` = keep, `false` = abstain.
///
/// **This is a measurement boundary, not a policy.** `machined_z` is a
/// minimum over CL positions within one envelope radius, and outside the
/// footprint there are no CL positions: `tip_z` is `NaN` there and the tap
/// carries no information. Within an envelope radius of the rim the minimum
/// is therefore taken over a truncated set, and it comes out HIGH.
///
/// The size of that error is not academic. A Ø6 flat endmill on a 45° plane
/// binds one envelope radius downhill; at the plane's low edge that CL is a
/// tangency the grid cannot represent, and the band read a flat **0.20 mm**
/// of gap — four times a 0.05 mm tolerance, on a plane the tool machines
/// perfectly. Interpolating over the surviving corners does not fix it,
/// because the field is genuinely discontinuous there.
///
/// [`crate::tier_map`] leaves the equivalent erosion to its consumer so the
/// map stays a measurement. This map IS the consumer surface, so it abstains
/// here instead — `None`, never a number. The eroded width is published as
/// [`ReachMap::rim_erosion_mm`].
///
/// The box is Chebyshev rather than Euclidean (a square, not a disc), which
/// erodes marginally more than the geometry demands — the conservative
/// direction for an abstention.
fn rim_keep_mask(surface_z: &[f64], grid: &GridSpec, radius_mm: f64) -> Vec<bool> {
    let k = (radius_mm.max(0.0) / grid.cell_mm).ceil() as usize;
    if k == 0 {
        return surface_z.iter().map(|z| z.is_finite()).collect();
    }
    // Integral image of "this cell is NOT over the mesh", so a box query is
    // four loads whatever the erosion width.
    let (nx, ny) = (grid.nx, grid.ny);
    let stride = nx + 1;
    let mut integral = vec![0u32; stride * (ny + 1)];
    for r in 0..ny {
        let mut row_sum = 0u32;
        for c in 0..nx {
            let uncovered = u32::from(
                surface_z
                    .get(r * nx + c)
                    .is_none_or(|z: &f64| !z.is_finite()),
            );
            row_sum += uncovered;
            let above = integral.get(r * stride + c + 1).copied().unwrap_or(0);
            if let Some(slot) = integral.get_mut((r + 1) * stride + c + 1) {
                *slot = above + row_sum;
            }
        }
    }
    let sum = |r0: usize, c0: usize, r1: usize, c1: usize| -> u32 {
        let at = |r: usize, c: usize| integral.get(r * stride + c).copied().unwrap_or(0);
        at(r1, c1) + at(r0, c0) - at(r0, c1) - at(r1, c0)
    };

    (0..ny)
        .flat_map(|r| {
            (0..nx).map(move |c| {
                if surface_z
                    .get(r * nx + c)
                    .is_none_or(|z: &f64| !z.is_finite())
                {
                    return false;
                }
                // A box that would leave the grid means the padding was not
                // wide enough; treat the missing part as uncovered.
                let (Some(r0), Some(c0)) = (r.checked_sub(k), c.checked_sub(k)) else {
                    return false;
                };
                if r + k + 1 > ny || c + k + 1 > nx {
                    return false;
                }
                sum(r0, c0, r + k + 1, c + k + 1) == 0
            })
        })
        .collect()
}

/// Pass B: the machined surface, the gap against the true mesh, and **the
/// resolution floor of the tap that set the answer**.
///
/// The third return is the F1 correction. `machined_z` is a minimum, so its
/// error is the error of the ONE tap that attained the minimum — not the
/// worst over the kernel, and not the value at the query cell. Those two
/// naive choices are both wrong in a way that matters:
///
/// * At the query cell. On a concave trough of radius `ρ` the binding CL sits
///   `R·sin θ` away, and `tip_z`'s curvature at the binding CL can be several
///   times its curvature under the query point — measured 1.54 against 0.70
///   per mm halfway up a `ρ = 3` flank for a R2 ball, so the query-cell
///   reading understates the bound by more than half and the flank reads
///   unreachable on a surface the ball forms.
/// * The worst over the kernel footprint. Correct as a bound and far too
///   pessimistic to use: it maxes a rough second-difference field over a disc
///   an envelope radius wide, so on a terrain nearly every cell would abstain.
///
/// Reading it at the argmin is exact and costs one array load per improvement.
fn gap_plane(
    tip_z: &[f64],
    surface_z: &[f64],
    keep: &[bool],
    kernel: &[KernelTap],
    cell_floor: &[f64],
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<(Vec<Option<f32>>, Vec<f64>), Cancelled> {
    let row_fn = |row: usize| -> Vec<(Option<f32>, f64)> {
        let y = grid.y_of(row);
        (0..grid.nx)
            .map(|col| {
                let cell = row * grid.nx + col;
                if !keep.get(cell).copied().unwrap_or(false) {
                    return (None, f64::NAN);
                }
                let mesh_z = surface_z.get(cell).copied().unwrap_or(f64::NAN);
                if !mesh_z.is_finite() {
                    return (None, f64::NAN);
                }
                let x = grid.x_of(col);
                let mut machined = f64::INFINITY;
                let mut floor = f64::NAN;
                for tap in kernel {
                    let (tx, ty) = (x + tap.dx_mm, y + tap.dy_mm);
                    let Some(z) = sample_tip_z(tip_z, grid, tx, ty) else {
                        continue;
                    };
                    let candidate = z + tap.rise_mm;
                    if candidate < machined {
                        machined = candidate;
                        floor = floor_at(cell_floor, grid, tx, ty);
                    }
                }
                if !machined.is_finite() {
                    return (None, f64::NAN);
                }
                // A drop cutter cannot gouge, so the machined surface is
                // never below the mesh; a negative reading is arithmetic
                // noise and is clamped, not reported as a gouge.
                (Some((machined - mesh_z).max(0.0) as f32), floor)
            })
            .collect()
    };
    Ok(walk_rows(grid, cancel, row_fn)?.into_iter().unzip())
}

/// The per-cell floor at the grid cell nearest `(x, y)`, or `NaN` off the
/// grid. Nearest rather than interpolated: the floor is a bound, and a bound
/// read between two cells of a dilated maximum is not a tighter bound.
fn floor_at(cell_floor: &[f64], grid: &GridSpec, x: f64, y: f64) -> f64 {
    let col = ((x - grid.origin_x) / grid.cell_mm).round();
    let row = ((y - grid.origin_y) / grid.cell_mm).round();
    if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
        return f64::NAN;
    }
    let (col, row) = (col as usize, row as usize);
    if col >= grid.nx || row >= grid.ny {
        return f64::NAN;
    }
    cell_floor
        .get(row * grid.nx + col)
        .copied()
        .unwrap_or(f64::NAN)
}

/// The area-weighted verdict over the mesh's own triangles.
///
/// A triangle joins the population only when it faces upward AND is the
/// topmost surface at its own centroid — see the module doc on what this
/// map does not answer.
struct AreaVerdict {
    surface_area_mm2: f64,
    measured_area_mm2: f64,
    unreachable_area_mm2: f64,
    unresolved_area_mm2: f64,
}

/// Area totals per triangle: `(surface, measured, unreachable, unresolved)`.
type FaceAreas = (f64, f64, f64, f64);

/// The verdict, THREE ways.
///
/// A cell is `unreachable` only when its gap clears BOTH the tolerance and
/// the cell's own resolution floor ([`curvature_floor_plane`]). Above the
/// tolerance but under the floor it is `unresolved` — the bar sits below the
/// arithmetic there, so the reading is a statement about the grid, not about
/// the tool, and it is counted into neither side.
///
/// A cell with no floor of its own (the grid rim, where no second difference
/// exists) takes the tolerance alone. That is the conservative direction: the
/// rim band is already abstained on by [`rim_keep_mask`].
fn area_verdict(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    map_cells: &[Option<f32>],
    floor: &[f64],
    grid: &GridSpec,
    tolerance_mm: f64,
) -> AreaVerdict {
    let per_face = |tri: &crate::geo::Triangle| -> FaceAreas {
        let area = triangle_area(tri);
        if tri.normal.z <= 0.0 {
            return (area, 0.0, 0.0, 0.0);
        }
        let Some(centroid) = triangle_centroid(tri) else {
            return (area, 0.0, 0.0, 0.0);
        };
        match surface_z_at(centroid.x, centroid.y, mesh, index) {
            Some(top) if top - centroid.z > TOP_SURFACE_EPS_MM => (area, 0.0, 0.0, 0.0),
            Some(_) => {
                let col = ((centroid.x - grid.origin_x) / grid.cell_mm).round();
                let row = ((centroid.y - grid.origin_y) / grid.cell_mm).round();
                if !col.is_finite() || !row.is_finite() || col < 0.0 || row < 0.0 {
                    return (area, 0.0, 0.0, 0.0);
                }
                let (col, row) = (col as usize, row as usize);
                if col >= grid.nx || row >= grid.ny {
                    return (area, 0.0, 0.0, 0.0);
                }
                let cell = row * grid.nx + col;
                match map_cells.get(cell).copied().flatten() {
                    None => (area, 0.0, 0.0, 0.0),
                    Some(gap) => {
                        let gap = f64::from(gap);
                        if gap <= tolerance_mm {
                            return (area, area, 0.0, 0.0);
                        }
                        let local = floor.get(cell).copied().filter(|f| f.is_finite());
                        if gap > tolerance_mm.max(local.unwrap_or(0.0)) {
                            (area, area, area, 0.0)
                        } else {
                            (area, area, 0.0, area)
                        }
                    }
                }
            }
            None => (area, 0.0, 0.0, 0.0),
        }
    };
    let add = |a: FaceAreas, b: FaceAreas| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3);

    #[cfg(feature = "parallel")]
    let (surface, measured, unreachable, unresolved) = mesh
        .faces
        .par_iter()
        .map(per_face)
        .reduce(|| (0.0, 0.0, 0.0, 0.0), add);
    #[cfg(not(feature = "parallel"))]
    let (surface, measured, unreachable, unresolved) = mesh
        .faces
        .iter()
        .map(per_face)
        .fold((0.0, 0.0, 0.0, 0.0), add);

    AreaVerdict {
        surface_area_mm2: surface,
        measured_area_mm2: measured,
        unreachable_area_mm2: unreachable,
        unresolved_area_mm2: unresolved,
    }
}

fn triangle_area(tri: &crate::geo::Triangle) -> f64 {
    let (Some(a), Some(b), Some(c)) = (tri.v.first(), tri.v.get(1), tri.v.get(2)) else {
        return 0.0;
    };
    (b - a).cross(&(c - a)).norm() / 2.0
}

fn triangle_centroid(tri: &crate::geo::Triangle) -> Option<P3> {
    let (a, b, c) = (tri.v.first()?, tri.v.get(1)?, tri.v.get(2)?);
    Some(P3::new(
        (a.x + b.x + c.x) / 3.0,
        (a.y + b.y + c.y) / 3.0,
        (a.z + b.z + c.z) / 3.0,
    ))
}

/// Walk the grid and measure what this cutter can and cannot reach.
///
/// Two passes over the same grid: one drop-cutter CL plus one exact surface
/// Z per cell, then a min-filter of the CL plane with the tool profile. The
/// drop-cutter work — the expensive half — is one call per cell, the same as
/// one rung of [`crate::tier_map::compute_tier_map`].
///
/// # Errors
///
/// [`Cancelled`] if `cancel` fires during either pass.
pub fn compute_reach_map(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ReachMapParams,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<ReachMap, Cancelled> {
    let grid = GridSpec::new(mesh, cutter, params);
    let (tip_z, surface_z) = drop_and_surface_planes(mesh, index, cutter, &grid, cancel)?;
    let kernel = profile_kernel(cutter, grid.cell_mm);
    let rim_erosion_mm = cutter.envelope_radius_mm().max(0.0);
    let keep = rim_keep_mask(&surface_z, &grid, rim_erosion_mm);
    // The floor is measured on the surface this walk built — see
    // `curvature_floor_plane` for the wanaka evidence that the plane-only
    // term understated it by 1.8x on the wanaka case — and `gap_plane` then reports it at
    // the tap that ATTAINED each cell's minimum, which is the only place the
    // error of a minimum can come from.
    let cell_floor = curvature_floor_plane(&tip_z, grid.nx, grid.ny);
    let (cells, binding_floor) = gap_plane(
        &tip_z,
        &surface_z,
        &keep,
        &kernel,
        &cell_floor,
        &grid,
        cancel,
    )?;

    let profile_floor_mm = sampling_floor_mm(cutter, grid.cell_mm);
    // `binding_floor` is `NaN` wherever the map reports nothing, so the
    // quantile's population is the answer's own footprint. That matters:
    // `tip_z` is finite for a band one envelope radius OUTSIDE the mesh
    // footprint too — the tool still contacts the part edge from there — and
    // that band is where `tip_z` stops following the surface and starts
    // pivoting on the rim, so its second difference is large and says nothing
    // about the answer. On a 45° plane fixture that band is 40 % of the grid,
    // which would have put a plane's published floor in the millimetres.
    let curvature_floor_p95_mm = finite_quantile(&binding_floor, 0.95);
    let discretisation_floor_mm = profile_floor_mm.max(curvature_floor_p95_mm);

    let tolerance_mm = params.tolerance_mm.max(0.0);
    let verdict = area_verdict(mesh, index, &cells, &binding_floor, &grid, tolerance_mm);
    let max_gap_mm = cells
        .iter()
        .filter_map(|c| c.map(f64::from))
        .fold(0.0f64, f64::max);
    let unreachable_fraction = if verdict.measured_area_mm2 > 0.0 {
        verdict.unreachable_area_mm2 / verdict.measured_area_mm2
    } else {
        0.0
    };

    tracing::debug!(
        target: "rs_cam_core::reach_map",
        cells = cells.len(),
        cell_mm = grid.cell_mm,
        tolerance_mm,
        unreachable_pct = unreachable_fraction * 100.0,
        unresolved_mm2 = verdict.unresolved_area_mm2,
        max_gap_mm,
        profile_floor_mm,
        curvature_floor_p95_mm,
        "reach map build"
    );

    Ok(ReachMap {
        nx: grid.nx,
        ny: grid.ny,
        origin_x: grid.origin_x,
        origin_y: grid.origin_y,
        cell_mm: grid.cell_mm,
        cells,
        tolerance_mm,
        unreachable_fraction,
        max_gap_mm,
        surface_area_mm2: verdict.surface_area_mm2,
        measured_area_mm2: verdict.measured_area_mm2,
        unreachable_area_mm2: verdict.unreachable_area_mm2,
        unresolved_area_mm2: verdict.unresolved_area_mm2,
        discretisation_floor_mm,
        profile_floor_mm,
        curvature_floor_p95_mm,
        rim_erosion_mm,
        cell_floor_mm: binding_floor.iter().map(|f| *f as f32).collect(),
        tool_id: None,
        model_id: None,
    })
}

/// [`compute_reach_map`] for a caller that holds no spatial index — a
/// fixture or a one-shot probe. Production callers hold a memoised index
/// ([`crate::geom_cache::cached_auto_index`]) and should pass it.
#[must_use]
pub fn reach_map_for_mesh(
    mesh: &TriangleMesh,
    cutter: &dyn MillingCutter,
    tolerance_mm: f64,
    cell_mm: f64,
) -> ReachMap {
    let index = SpatialIndex::build_auto(mesh);
    let params = ReachMapParams {
        cell_mm,
        tolerance_mm,
        margin_mm: 0.5,
    };
    let never_cancel = || false;
    // SAFETY: `never_cancel` never fires, so the walk cannot return
    // `Cancelled`; the fallback map is unreachable and carries no verdict.
    compute_reach_map(mesh, &index, cutter, &params, &never_cancel).unwrap_or(ReachMap {
        nx: 0,
        ny: 0,
        origin_x: 0.0,
        origin_y: 0.0,
        cell_mm,
        cells: Vec::new(),
        tolerance_mm,
        unreachable_fraction: 0.0,
        max_gap_mm: 0.0,
        surface_area_mm2: 0.0,
        measured_area_mm2: 0.0,
        unreachable_area_mm2: 0.0,
        unresolved_area_mm2: 0.0,
        discretisation_floor_mm: 0.0,
        profile_floor_mm: 0.0,
        curvature_floor_p95_mm: 0.0,
        rim_erosion_mm: 0.0,
        cell_floor_mm: Vec::new(),
        tool_id: None,
        model_id: None,
    })
}
