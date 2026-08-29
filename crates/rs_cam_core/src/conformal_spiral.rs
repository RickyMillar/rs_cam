//! **Phase F2.1 research module — simply-connected conformal-spiral prototype.**
//!
//! Research-only. Nothing here is on a production path: no operation, no
//! generator, no GUI surface and no MCP tool reaches this module, and none
//! should until the Phase F2 evidence in
//! `planning/conformal_finish_2026-08-28/PROGRAMME.md` says what it is worth.
//! It follows the precedent of [`crate::direction_field`] (Phase F1) and
//! [`crate::scallop_isofield`] — unshipped research candidates that document
//! their own limitations rather than pretending to be features.
//!
//! # Sources — three, and they must be kept apart
//!
//! * **[SOURCE-2025]** Shen, Xu, Zhang, Yan & Ding, *Conformal Slit Mapping
//!   Based Spiral Tool Trajectory Planning for Ball-end Milling on Complex
//!   Freeform Surfaces*, **arXiv:2504.06310** (v2, 2025). Supplies the
//!   pipeline: coverage-driven ring spacing (Eqs. 1–4) and log-rectangle
//!   bridging (Eqs. 7–9). Transcribed in
//!   `planning/conformal_finish_2026-08-28/paper_2504.06310_extraction.md`.
//! * **[SOURCE-2024]** Shen et al., **arXiv:2309.10655** (v2, 2024;
//!   IJRR 43). Supplies **Equation A-11 (and its companion A-12) only** —
//!   the blend `σ(t)`. Transcribed in
//!   `planning/conformal_finish_2026-08-28/paper_2309.10655_extraction.md`.
//!   **Nothing else from that paper's Appendix A is used here**: A-14, the
//!   Nyström discretisation and the B-spline boundary machinery are the
//!   slit-map front-end, which is Phase F2 step 3, not this module.
//! * **[REPO]** everything else — in particular the harmonic disk map, the
//!   arc-length boundary correspondence, the surface sampling scheme, the
//!   spiral's angular bookkeeping and every metric in [`SpiralReport`].
//!
//! # What this module does
//!
//! For a **simply connected** (single boundary loop, disk topology) region of
//! a triangle mesh and a 3-axis ball-end cutter:
//!
//! 1. **[REPO] Harmonic disk map.** Flatten the induced submesh to the unit
//!    disk: boundary vertices prescribed onto the unit circle by cumulative
//!    **arc length**, interior vertices by two Laplace solves (one per
//!    coordinate) on the cotangent Laplacian.
//! 2. **[SOURCE-2025 §2.2.1, Eqs. 1–4]** Ring spacing: each ring's radius is
//!    binary-searched in the disk until every sampled iso-scallop point
//!    *outside* that radius is swept by the ring's tool-centre curve.
//! 3. **[SOURCE-2025 §2.2.2, Eqs. 7–9]** Bridging: unroll the disk to the
//!    `(angle, radius)` rectangle, connect ring `i` to ring `i+1` with the
//!    blend `σ(t)` of **[SOURCE-2024 Eq. A-11]**, and sweep the start angle
//!    for the minimum-total-3D-length spiral.
//!
//! # Why harmonic and not conformal — a labelled [REPO] substitution
//!
//! The 2025 paper flattens with BFF and then applies a conformal **slit map**;
//! the slit map is what makes holes work, and it is absent from that paper
//! (extraction gap 1). This module handles the **simply-connected** case only,
//! where there are no slits at all, so the front-end degenerates to "some map
//! of the region onto the unit disk". A discrete *harmonic* map with a
//! prescribed convex boundary is used instead of a conformal one, because:
//!
//! * the paper's spacing mechanism is a **sampled 3D coverage check** and the
//!   extraction is explicit that this check *is* the only distortion
//!   compensation in the pipeline — "there is no conformal-distortion-factor
//!   formula anywhere" (§3.1). Any bijective map therefore yields correct 3D
//!   spacing; conformality buys ring *smoothness*, not correctness;
//! * a harmonic map onto a convex boundary is injective in the continuum
//!   (Radó–Kneser–Choquet), so a flipped triangle is a **discretisation**
//!   symptom — which is why [`SpiralReport::flipped_triangles`] counts them
//!   instead of assuming none;
//! * it needs no new dependency: the cotangent Laplacian assembly and the
//!   Jacobi-preconditioned CG already exist in [`crate::direction_field`] and
//!   are reused verbatim.
//!
//! **This substitution is valid only while there are no slits.** The moment a
//! hole enters the region it stops being a substitution for anything and the
//! slit map (Phase F2 step 3) is required. [`plan_spiral`] refuses a
//! multi-boundary region rather than approximating one.
//!
//! # Numerical extension over Phase F1
//!
//! F1's CG solves a pure-Neumann system with *zero-valued* pins. The harmonic
//! map needs **non-zero Dirichlet** data, so the known boundary values are
//! moved to the right-hand side and only the interior block is solved:
//! `A_II x_I = Σ_{j ∈ ∂} w_ij u_j`. The pinned rows of
//! [`crate::direction_field`]'s `cg_solve` are held at zero, which is exactly
//! the interior-only system once the boundary contribution is on the RHS.
//!
//! # Limitations a reader must not lose
//!
//! * **Simply connected only.** One boundary loop, Euler characteristic 1.
//!   Anything else is refused with a typed [`SpiralRefusal`].
//! * **Contact points, not cutter-centre points.** The polyline this module
//!   returns is the cutter *contact* curve on the mesh. Internally the
//!   tool-centre curve is the contact curve offset `+K_c` along the surface
//!   normal — the paper's own construction (§3.1) — and that is what the
//!   coverage check measures against. For machining, the drop-cutter CL of
//!   the evidence instrument **supersedes** it: the paper has no gouge
//!   handling anywhere (extraction gap 3), and drop-cutter projection is the
//!   rs_cam convention.
//! * **Normals inherit `build_region_mesh`'s +Z forcing** (the same
//!   convention `crest_lines` and [`crate::direction_field`] use). Correct
//!   for the terrain-like regions F2 targets and for both fixtures here;
//!   **wrong for a region whose surface faces away from +Z**.
//! * **No reachability, no gouge check, no collision check.** 3-axis
//!   reachability of steep walls is outside the paper's scope entirely.
//! * **The scallop bound is sampled, not proved.** It is exactly as strong as
//!   [`SpiralParams::n_surface_samples`]. The paper's own cutting trial
//!   overshot its nominal scallop by up to 12 % (§5), and its Table 1 case
//!   1.5 is a *documented failure* from too-sparse sampling. Treat 12 % as
//!   the expected floor, not the ceiling.
//! * **Ring-radius monotonicity is assumed, not proved.** The binary search
//!   of Eqs. 1–4 presumes the feasibility predicate is monotone in `R`. The
//!   paper assumes it too and says nothing about it; a non-monotone region
//!   would make the search return *a* feasible radius rather than the
//!   smallest one. Recorded as a gap, not repaired.
//!
//! # Gaps carried forward from the extractions (not invented around)
//!
//! * **G-BRIDGE-BOOKKEEPING [REPO resolution].** The 2025 paper never says
//!   what the span of the straight run `L_i^i` is. It fixes
//!   `P_Start^1 = P_End^1 − 2π` (ring 1 spans exactly one turn), defines
//!   `real(P_Start^{i+1}) = D_{i−i+1} + real(P_End^i)`, and then lists a
//!   *second* family of shifts `D_{i−i}` whose default is 0 but whose
//!   near-centre initial value is `8π/5` — four fifths of a turn, on every
//!   outer ring. Read literally, with each run spanning `2π + D_{i−i}`, that
//!   constant alone inflates the path by ~80 %, which contradicts the
//!   paper's own length claims; read as `2π − D_{i−1−i} + D_{i−i}` it still
//!   does. The bookkeeping is not recoverable from the text.
//!   **Resolution here:** each run spans `2π + line_shift_i`, so every ring
//!   is traversed in full at its own radius (the strongest coverage
//!   guarantee, and the reason the bridge coverage repair of §2.2.2 step 2 is
//!   a no-op in the simply-connected case); `line_shift_i` starts at **0**
//!   and is grown only by that repair loop, in `shift_step` increments. The
//!   paper's `8π/5` is preserved as the named constant
//!   [`PAPER_SECONDARY_LINE_SHIFT`] and as the parameter
//!   [`SpiralParams::secondary_line_shift`], defaulted to 0 with this reason.
//! * **G-SIGMA-REFINEMENT.** The 2025 paper says the blend is obtained "by
//!   refining the function σ(t) in Equation A-11". **Neither paper specifies
//!   the refinement** (2309.10655 extraction gap 1) and A-11's own grading
//!   parameter `p` is given no value in either (only `p ≥ 2`). A-11 is used
//!   here **verbatim, unrefined**, with `p` a parameter defaulting to 2.
//! * **G-SAMPLING.** No rule for `N_S`, `N_C` or `ε` appears in either paper
//!   (2504.06310 extraction gap 6), only the two failure modes. All three are
//!   parameters and all three are reported.
//! * **Eq. 13 is not used.** The 2025 paper's printed scallop formula mixes a
//!   curvature and a radius and is dimensionally inconsistent (extraction
//!   gap 3). Where scallop arithmetic is needed — only in this module's
//!   tests, to state the expected ring spacing — it comes from
//!   [`crate::scallop_math`].

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use crate::direction_field::{
    FieldParams, RegionMesh, assemble_poisson, build_region_mesh, cg_solve,
};
use crate::geo::{P3, V3};
use crate::mesh::{QueryScratch, SpatialIndex, TriangleMesh};

// ---------------------------------------------------------------------------
// The paper's constants, named and attributed
// ---------------------------------------------------------------------------

/// **[SOURCE-2025 §2.2.2 / Pseudocode A-2 line 6]** Initial bridge shift
/// `D_{i−i+1}^{Real}` away from the centre: `π/10`.
pub const PAPER_INITIAL_BRIDGE_SHIFT: f64 = PI / 10.0;

/// **[SOURCE-2025 Pseudocode A-2 lines 6–9]** Near-centre switch: when the
/// ring radius `R_i^S` is at or below this, the bridge shift becomes `2π`
/// because "the abrupt turns in the corresponding `C_i^T` make it challenging
/// to maintain smooth transitions".
pub const PAPER_NEAR_CENTRE_RADIUS: f64 = 0.3;

/// **[SOURCE-2025 Pseudocode A-2 lines 6–9]** The near-centre bridge shift.
pub const PAPER_NEAR_CENTRE_BRIDGE_SHIFT: f64 = TAU;

/// **[SOURCE-2025 Pseudocode A-2 lines 6–9]** The secondary along-line shift
/// `D_{i+1−i+1}^{Real} = 8π/5`, applied in the paper after an *outer* bridge.
///
/// Present for attribution. See the module header's **G-BRIDGE-BOOKKEEPING**:
/// under this module's `2π + line_shift` reading of the straight run, the
/// coverage this constant buys is already guaranteed, so
/// [`SpiralParams::secondary_line_shift`] defaults to `0.0`, not to this.
pub const PAPER_SECONDARY_LINE_SHIFT: f64 = 8.0 * PI / 5.0;

/// **[SOURCE-2025 §2.2.2 / Pseudocode A-2 line 17]** Shift increment for the
/// bridge-repair loop, `π/50`.
///
/// Note `π/50 = 2π/100`: the paper's step is exactly one cell of a 100-point
/// angular lattice. This module snaps it to whichever lattice
/// [`SpiralParams::n_angular_samples`] defines.
pub const PAPER_SHIFT_STEP: f64 = PI / 50.0;

/// **[SOURCE-2025 §2.2.2 / Pseudocode A-2 lines 3, 30–35]** Start-angle sweep
/// step over `[0, 2π)`, `π/50`.
pub const PAPER_START_ANGLE_STEP: f64 = PI / 50.0;

/// **[SOURCE-2024 Eq. A-11]** The grading parameter is stated only as
/// `p ≥ 2`; no value appears in either paper (**G-SIGMA-REFINEMENT**). 2 is
/// this module's **[REPO]** choice — the smallest admissible, and the one
/// where A-12 collapses to `v(t) = t/2π`.
pub const DEFAULT_BLEND_P: f64 = 2.0;

// ---------------------------------------------------------------------------
// Local numerical constants
// ---------------------------------------------------------------------------

/// Triangles below this area (mm²) contribute nothing; matches the guard
/// [`build_region_mesh`] already applies.
const MIN_TRIANGLE_AREA_MM2: f64 = 1e-18;

/// Below this, a vector is treated as having no direction.
const EPS_VEC: f64 = 1e-12;

/// Two disk points closer than this are the same point.
const EPS_DISK: f64 = 1e-12;

/// Orientation determinants below this are treated as touching, not crossing.
const EPS_CROSS: f64 = 1e-14;

/// **[REPO]** Radial pullback ladder for a disk query point that lands
/// outside the flattened polygon.
///
/// The flat boundary is the *polygon* through the boundary vertices, which is
/// inscribed in the unit circle, so a ring point at radius close to 1 can sit
/// in the sliver between chord and circle. Rather than dropping it (a hole in
/// the ring) or extrapolating (an invented surface point), the query is pulled
/// radially inward by these fractions in turn and the first hit is taken.
/// Every pullback is counted in [`SpiralReport::ring_points_pulled_back`].
const PULLBACK_LADDER: [f64; 6] = [1e-4, 1e-3, 3e-3, 1e-2, 3e-2, 1e-1];

/// Upper bound on either axis of an internal bucket grid.
const MAX_GRID_AXIS: usize = 512;

/// **[REPO]** Radial buckets for the area-distortion profile. Five is enough
/// to see whether distortion explodes toward the disk centre — the shape the
/// ring-stall hypothesis predicts on a high-relief region — without turning a
/// report row into a histogram nobody reads.
const RADIAL_BUCKETS: usize = 5;

/// **[REPO]** Cap on how many still-uncovered samples the stall diagnostic
/// measures distances for. The measurement is brute-force point-to-polyline
/// (the bucket grid can only answer "within `K_c`?", not "how far?"), so it is
/// strided rather than exhaustive; [`StallContext::distance_samples`] reports
/// how many were actually taken.
const STALL_DISTANCE_SAMPLE_CAP: usize = 2_000;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Tunables for one spiral plan.
///
/// Every value that came from a paper says so; every value that did not is
/// labelled **[REPO]**.
#[derive(Debug, Clone)]
pub struct SpiralParams {
    /// Ball-end cutter radius `K_c` (mm). **[SOURCE-2025 §3.1]** the
    /// tool-centre curve is the contact curve offset `+K_c` along the surface
    /// normal, and a sampled iso-scallop point is *covered* when it lies
    /// within `K_c` of that curve.
    pub ball_radius_mm: f64,
    /// Scallop-height constraint `h` (mm): the iso-scallop surface `S^h` is
    /// the region offset `+h` along its normals.
    pub scallop_h_mm: f64,
    /// `N_S` — how many points `S^h` is sampled into.
    ///
    /// **[REPO]** No selection rule appears in either paper
    /// (**G-SAMPLING**); the 2025 paper's own Table 1 case 1.5 is a
    /// *documented failure* from choosing this too small ("overly sparse
    /// discrete points, resulting in a small k-value and excessively large
    /// trajectory spacing"). The coverage bound is exactly as strong as this
    /// number: sample spacing must be well under half the expected stepover
    /// or ring spacing reads long.
    pub n_surface_samples: usize,
    /// `N_C` — samples per full turn, and **the shared angular lattice**.
    ///
    /// **[REPO]** Ring runs and bridges are both sampled on
    /// `a₀ + j·(2π/N_C)`. A separate bridge lattice would put bridge chords
    /// out of phase with ring chords in the band where `σ′(0) = 0` makes the
    /// bridge hug its ring, and the disk-domain self-intersection count would
    /// then report *sampling* artefacts as crossings.
    pub n_angular_samples: usize,
    /// `ε` — binary-search termination tolerance on the disk radius `R`.
    /// **[SOURCE-2025 Pseudocode A-1 line 12]** has an `ε` but states no
    /// value (**G-SAMPLING**).
    pub ring_eps: f64,
    /// **[REPO]** Hard cap on ring count, so a pathological region cannot
    /// spin. Hitting it is a typed refusal, not a silent truncation.
    pub max_rings: usize,
    /// **[SOURCE-2024 Eq. A-11]** grading parameter `p ≥ 2`; see
    /// [`DEFAULT_BLEND_P`].
    pub blend_p: f64,
    /// Bridge shift away from the centre — [`PAPER_INITIAL_BRIDGE_SHIFT`].
    pub initial_bridge_shift: f64,
    /// Near-centre switch radius — [`PAPER_NEAR_CENTRE_RADIUS`].
    pub near_centre_radius: f64,
    /// Near-centre bridge shift — [`PAPER_NEAR_CENTRE_BRIDGE_SHIFT`].
    pub near_centre_bridge_shift: f64,
    /// Initial along-line shift after an outer bridge.
    ///
    /// The paper's value is [`PAPER_SECONDARY_LINE_SHIFT`] (`8π/5`); this
    /// defaults to **0.0** for the reason in the module header's
    /// **G-BRIDGE-BOOKKEEPING**. Set it to the paper's constant to reproduce
    /// the literal reading and watch [`SpiralReport::bridge_overhead_pct`].
    pub secondary_line_shift: f64,
    /// Bridge-repair shift increment — [`PAPER_SHIFT_STEP`].
    pub shift_step: f64,
    /// **[REPO]** Cap on bridge-repair iterations.
    pub max_bridge_repair_steps: usize,
    /// Start-angle sweep step over `[0, 2π)` —
    /// [`PAPER_START_ANGLE_STEP`]. Coarsen it in tests; the paper's own
    /// description of the sweep is "trial and error".
    pub start_angle_step: f64,
    /// Conjugate-gradient iteration cap for each Laplace solve.
    pub cg_max_iters: usize,
    /// Conjugate-gradient stopping tolerance, relative to `‖rhs‖`.
    pub cg_rel_tolerance: f64,
}

impl Default for SpiralParams {
    fn default() -> Self {
        Self {
            ball_radius_mm: 3.0,
            scallop_h_mm: 0.03,
            n_surface_samples: 20_000,
            n_angular_samples: 360,
            ring_eps: 1e-4,
            max_rings: 4096,
            blend_p: DEFAULT_BLEND_P,
            initial_bridge_shift: PAPER_INITIAL_BRIDGE_SHIFT,
            near_centre_radius: PAPER_NEAR_CENTRE_RADIUS,
            near_centre_bridge_shift: PAPER_NEAR_CENTRE_BRIDGE_SHIFT,
            secondary_line_shift: 0.0,
            shift_step: PAPER_SHIFT_STEP,
            max_bridge_repair_steps: 200,
            start_angle_step: PAPER_START_ANGLE_STEP,
            cg_max_iters: 20_000,
            cg_rel_tolerance: 1e-10,
        }
    }
}

impl SpiralParams {
    /// Defaults with the two physical dials set.
    #[must_use]
    pub fn new(ball_radius_mm: f64, scallop_h_mm: f64) -> Self {
        Self {
            ball_radius_mm,
            scallop_h_mm,
            ..Self::default()
        }
    }

    /// The angular lattice cell, `2π / N_C`.
    #[must_use]
    pub fn lattice_step(&self) -> f64 {
        TAU / (self.n_angular_samples.max(3) as f64)
    }
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why [`plan_spiral`] declined to plan.
///
/// Every arm is a **precondition** this module refuses to approximate around.
/// In particular a region with holes is not "nearly" simply connected: the
/// slit map is what makes holes work and it is Phase F2 step 3.
#[derive(Debug, Clone, PartialEq)]
pub enum SpiralRefusal {
    /// Nothing usable survived [`build_region_mesh`].
    EmptyRegion,
    /// A boundary edge is shared by more than two triangles, or a vertex has
    /// more than one outgoing boundary half-edge (a pinch point). The
    /// boundary loop is then not well defined.
    NonManifoldBoundary { detail: NonManifoldDetail },
    /// The induced submesh has a boundary-loop count other than one. `1` is
    /// the disk; `0` is a closed surface with no boundary to prescribe; `≥ 2`
    /// is the holed case the slit map exists for.
    NotSimplyConnected { boundary_loops: usize },
    /// One boundary loop, but Euler characteristic `V − E + F ≠ 1` — a
    /// handle, or a self-touching loop. Not a disk.
    NotADisk { euler_characteristic: i64 },
    /// The single boundary loop is too short or too small to prescribe.
    DegenerateBoundaryLoop { vertices: usize, length_mm: f64 },
    /// A Laplace solve hit its iteration cap.
    FlattenDidNotConverge {
        /// `"u"` or `"v"` — which coordinate.
        component: &'static str,
        residual: f64,
        iterations: usize,
    },
    /// The surface produced no `S^h` samples (degenerate area, or
    /// `n_surface_samples == 0`).
    NoSurfaceSamples,
    /// A ring was placed but the uncovered set did not shrink, so no finite
    /// number of further rings can empty it.
    RingSearchStalled { rings: usize, uncovered: usize },
    /// [`SpiralParams::max_rings`] was reached with points still uncovered.
    RingLimitReached { rings: usize, uncovered: usize },
}

/// Which non-manifold condition fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonManifoldDetail {
    /// An undirected edge is used by three or more triangle corners.
    EdgeOverUsed,
    /// A vertex has two or more outgoing boundary half-edges.
    BoundaryPinch,
}

// ---------------------------------------------------------------------------
// Result + report
// ---------------------------------------------------------------------------

/// One continuous spiral, plus the rings it was bridged from.
///
/// Points are **cutter-contact points on the mesh surface**. See the module
/// header for why the cutter-centre conversion belongs to the evidence
/// instrument's drop-cutter, not here.
#[derive(Debug, Clone, Default)]
pub struct SpiralResult {
    /// The spiral as a single 3D contact polyline. Exactly one polyline:
    /// there is no lift and no split anywhere in the construction.
    pub spiral_contact: Vec<P3>,
    /// The same polyline in the unit-disk domain, index for index with
    /// [`SpiralResult::spiral_contact`].
    pub spiral_disk: Vec<(f64, f64)>,
    /// The pre-bridge rings, outermost first, each a closed 3D contact
    /// polyline whose last point repeats its first.
    pub rings_contact: Vec<Vec<P3>>,
}

/// Area distortion over one band of disk radius.
///
/// **[REPO]** The scalar min/median/max on [`SpiralReport`] cannot tell a
/// uniformly stretched map from one that is near-isometric at the rim and
/// exploding at the centre — and those two have completely different
/// consequences for ring spacing, because a disk circle at small `R` is what
/// pulls back to a razor-thin 3D band. This is the same measurement bucketed
/// by the disk radius of each triangle's flattened centroid.
#[derive(Debug, Clone, Default)]
pub struct RadialDistortion {
    /// Inclusive lower bound of the disk-radius band.
    pub r_lo: f64,
    /// Exclusive upper bound (inclusive in the outermost bucket).
    pub r_hi: f64,
    /// Triangles whose flattened centroid landed in this band.
    pub triangles: usize,
    /// `flat area / 3D area` (1/mm²) over this band.
    pub area_distortion_min: f64,
    /// See [`RadialDistortion::area_distortion_min`].
    pub area_distortion_median: f64,
    /// See [`RadialDistortion::area_distortion_min`].
    pub area_distortion_max: f64,
}

/// Which curve the stall distances were measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StallDistanceReference {
    /// The centre curve of the last ring that was successfully placed,
    /// identified by its index in [`SpiralReport::ring_radii`].
    LastPlacedRing(usize),
    /// No ring was ever placed, so the distances are against the centre curve
    /// of the candidate that failed.
    FailedCandidate,
}

/// Why the Eqs. 1–4 ring search could not empty the uncovered set, in enough
/// detail to attribute the failure from the report alone.
///
/// **[REPO]** The paper reports no such thing: its Table 1 records two
/// sampling failure modes with no k value and no diagnosis. The load-bearing
/// row is the distance distribution — if the median distance from the
/// still-uncovered points to the last placed ring's centre curve is far
/// larger than `2·K_c`, the surface genuinely cannot be covered ring-by-ring
/// at this map's distortion, and the refusal is correct rather than a bug in
/// the search.
#[derive(Debug, Clone)]
pub struct StallContext {
    /// Rings successfully placed before the stall.
    pub rings_placed: usize,
    /// Their disk radii, outermost first (duplicated here so the stall block
    /// is self-contained).
    pub ring_radii: Vec<f64>,
    /// Lower bound of the binary-search interval when the search gave up.
    pub search_lo: f64,
    /// Upper bound of that interval — for a stalled ring this equals the
    /// previous ring's radius, which is the signature of the failure.
    pub search_hi: f64,
    /// Whether **any** radius strictly inside the previous ring was ever
    /// feasible. `false` means the search never lowered its bound at all: no
    /// interior circle can sweep everything outside it, so the returned
    /// radius is the previous ring's own and the "new" ring is a duplicate.
    pub interior_radius_feasible: bool,
    /// `S^h` points still uncovered at the stall.
    pub uncovered: usize,
    /// How many of those the distances below were measured on (strided to
    /// the module's `STALL_DISTANCE_SAMPLE_CAP`).
    pub distance_samples: usize,
    /// Which curve the distances are against.
    pub distance_reference: StallDistanceReference,
    /// The coverage radius the distances should be compared to (`K_c`, mm).
    pub coverage_radius_mm: f64,
    /// Minimum 3D distance (mm) from a measured uncovered point to that curve.
    pub uncovered_distance_min_mm: f64,
    /// Median 3D distance (mm).
    pub uncovered_distance_median_mm: f64,
    /// Maximum 3D distance (mm).
    pub uncovered_distance_max_mm: f64,
}

/// Everything the F2 contract wants **counted rather than assumed**.
///
/// A count of zero here means *measured zero*: every row is computed on every
/// call that reaches it, so there is no "not measured" arm to conflate with
/// clean. **The report survives a refusal** — [`plan_spiral`] returns it
/// alongside the `Result`, filled as far as the pipeline got, precisely so a
/// refusal can be attributed to a bad map or a bad mechanism instead of
/// vanishing with the error.
#[derive(Debug, Clone, Default)]
pub struct SpiralReport {
    // --- region ---------------------------------------------------------
    /// Triangles actually planned on, after slivers and bad indices dropped.
    pub region_triangles: usize,
    /// Vertices of the induced submesh.
    pub region_vertices: usize,
    /// Undirected edges of the induced submesh.
    pub region_edges: usize,
    /// `V − E + F`. A disk is 1; this is refused otherwise.
    pub euler_characteristic: i64,
    /// Vertices on the single boundary loop.
    pub boundary_loop_vertices: usize,
    /// 3D length of the boundary loop (mm).
    pub boundary_loop_length_mm: f64,

    // --- flattening -----------------------------------------------------
    /// Interior (non-prescribed) vertices — the size of each Laplace solve.
    pub flatten_interior_vertices: usize,
    /// CG iterations for the `u` and `v` solves.
    pub flatten_cg_iterations: [usize; 2],
    /// Final relative residual of the `u` and `v` solves.
    pub flatten_residual: [f64; 2],
    /// Triangles whose flat signed area has the opposite sign to the map's
    /// overall orientation. A harmonic map onto a convex boundary is
    /// injective in the continuum, so any nonzero count here is a
    /// **discretisation** symptom — obtuse triangles giving negative
    /// cotangent weights. Counted, never assumed away.
    pub flipped_triangles: usize,
    /// `+1.0` when the flattening preserved the boundary-loop orientation,
    /// `-1.0` when it reversed it (an inconsistently wound input submesh).
    pub orientation_sign: f64,
    /// Per-triangle `flat area / 3D area` (units 1/mm²), min / median / max.
    /// A conformal map would make this proportional to the squared conformal
    /// factor; a *constant* value means an affine map, which is what the flat
    /// fixture must produce.
    pub area_distortion_min: f64,
    /// See [`SpiralReport::area_distortion_min`].
    pub area_distortion_median: f64,
    /// See [`SpiralReport::area_distortion_min`].
    pub area_distortion_max: f64,
    /// Per-corner `|flat angle − 3D angle|` in degrees, median and max over
    /// all `3 × region_triangles` corners. Zero everywhere means the map is
    /// conformal on this mesh.
    pub angle_distortion_median_deg: f64,
    /// See [`SpiralReport::angle_distortion_median_deg`].
    pub angle_distortion_max_deg: f64,
    /// Area distortion bucketed by disk radius, `RADIAL_BUCKETS` (5) entries
    /// outward from the centre. Empty means the flattening never ran.
    pub area_distortion_by_disk_radius: Vec<RadialDistortion>,

    // --- sampling + rings ------------------------------------------------
    /// `N_S` actually placed on `S^h`.
    pub surface_samples: usize,
    /// Rings produced by the Eqs. 1–4 search.
    pub ring_count: usize,
    /// Disk radius of each ring, outermost first.
    pub ring_radii: Vec<f64>,
    /// 3D length (mm) of each ring's contact polyline, outermost first.
    pub ring_lengths_mm: Vec<f64>,
    /// `S^h` points first covered by each ring — the paper's milling band
    /// `BP_i`.
    pub ring_newly_covered: Vec<usize>,
    /// Sum of [`SpiralReport::ring_lengths_mm`].
    pub total_ring_length_mm: f64,
    /// Binary-search iterations summed over every ring.
    pub binary_search_iterations: usize,
    /// Disk→3D queries that needed the radial `PULLBACK_LADDER`, summed over
    /// the ring search **and** the spiral construction.
    pub ring_points_pulled_back: usize,
    /// Disk→3D queries that found no triangle even after the ladder and were
    /// dropped from their polyline, summed over the ring search **and** the
    /// spiral construction. A nonzero count means a polyline has a gap in it
    /// that no other number here will show.
    pub ring_points_unlocated: usize,
    /// `S^h` points still uncovered when the ring search finished. **Want 0.**
    pub uncovered_after_rings: usize,

    // --- bridging --------------------------------------------------------
    /// Start angles evaluated in the sweep.
    pub start_angle_candidates: usize,
    /// The start angle that minimised total 3D length (radians).
    pub start_angle_rad: f64,
    /// Bridges emitted — always `ring_count − 1` for a nonempty plan.
    pub bridge_count: usize,
    /// Summed 3D length of the bridge sections (mm).
    pub total_bridge_length_mm: f64,
    /// `(spiral length − Σ ring lengths) / Σ ring lengths × 100`.
    pub bridge_overhead_pct: f64,
    /// Shift increments consumed by the §2.2.2 step-2 coverage repair.
    pub bridge_repair_steps: usize,
    /// `S^h` points a ring covered that the **bridged** spiral's centre curve
    /// does not. **Want 0.**
    pub uncovered_after_bridging: usize,

    // --- spiral ----------------------------------------------------------
    /// 3D length of the emitted spiral (mm).
    pub spiral_length_mm: f64,
    /// Points in the emitted spiral.
    pub spiral_points: usize,
    /// Proper segment-segment crossings of the spiral **in the disk domain**.
    /// Measured, not inferred from the construction.
    pub disk_self_intersections: usize,
    /// Structurally zero: [`plan_spiral`] emits exactly one polyline and
    /// never lifts. Carried so the F2 contract's retract row is on the same
    /// footing as the raster baselines' — and paired with
    /// [`SpiralReport::max_consecutive_step_mm`], which is the actual
    /// evidence that no hidden jump is hiding inside the single polyline.
    pub retract_count: usize,
    /// Largest 3D gap between consecutive spiral points (mm).
    pub max_consecutive_step_mm: f64,
    /// Median 3D gap between consecutive spiral points (mm).
    pub median_consecutive_step_mm: f64,

    // --- refusal context -------------------------------------------------
    /// Present exactly when the ring search finished with `S^h` points still
    /// uncovered — whether that ended in a refusal
    /// ([`SpiralRefusal::RingSearchStalled`],
    /// [`SpiralRefusal::RingLimitReached`]) or in the search reaching the disk
    /// centre. `None` means the rings closed coverage.
    pub stall: Option<StallContext>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Plan one continuous conformal-style spiral over a **simply connected**
/// mesh region for a 3-axis ball-end cutter.
///
/// **Always returns a [`SpiralReport`]**, filled as far as the pipeline got,
/// plus a `Result` carrying either the spiral or a typed [`SpiralRefusal`].
///
/// # Deviation from the F2.1 spec, recorded
///
/// The spec shape was `Option<(SpiralResult, SpiralReport)>` with a typed
/// reason. Two things are wrong with that: an `Option` cannot carry a reason,
/// and — the one measurement showed — a `Result<(result, report), refusal>`
/// **discards the report on the path where it is most needed**. A refusal is
/// exactly the moment someone has to decide whether the map was bad or the
/// mechanism was, and the flatten-metrics table exists to answer that. So the
/// report comes out of the tuple's first slot on every path, and the refusal
/// lives in the second. `SpiralRefusal` is small, so nothing here trips
/// `clippy::result_large_err`, and no boxing is needed.
///
/// # The `index` parameter
///
/// It is deliberately unused: the planner builds and indexes its **own**
/// flattened mesh, and every disk→3D query goes through that. It is kept in
/// the signature because the F2 evidence instrument that wraps this call needs
/// the input mesh's index for the drop-cutter CL conversion that supersedes
/// this module's normal-offset centre curve, and the two should not disagree
/// about which index they mean.
pub fn plan_spiral(
    mesh: &TriangleMesh,
    _index: &SpatialIndex,
    region_triangles: &[u32],
    params: &SpiralParams,
) -> (SpiralReport, Result<SpiralResult, SpiralRefusal>) {
    let mut report = SpiralReport::default();
    let outcome = plan_into(mesh, region_triangles, params, &mut report);
    (report, outcome)
}

/// The pipeline itself, writing into a caller-owned report so that every
/// early return leaves behind whatever was measured before it.
#[allow(clippy::result_large_err)]
fn plan_into(
    mesh: &TriangleMesh,
    region_triangles: &[u32],
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> Result<SpiralResult, SpiralRefusal> {
    let Some(region) = build_region_mesh(mesh, region_triangles) else {
        return Err(SpiralRefusal::EmptyRegion);
    };
    report.region_triangles = region.tris.len();
    report.region_vertices = region.verts.len();

    // 1. Topology: one boundary loop, disk Euler characteristic.
    let topo = region_topology(&region)?;
    report.region_edges = topo.edge_count;
    report.euler_characteristic = topo.euler;
    report.boundary_loop_vertices = topo.loop_vertices.len();
    report.boundary_loop_length_mm = topo.loop_length_mm;

    // 2. Harmonic disk map with arc-length boundary correspondence.
    let flat = flatten_to_disk(&region, &topo, params, report)?;

    // 3. Flatten metrics — the table that separates "bad map" from "bad
    //    mechanism" if the mechanism turns out to disappoint. Measured before
    //    anything can refuse downstream of it, so a refusal still carries it.
    measure_flattening(&region, &flat, report);

    // 4. Locator over the flattened mesh (z = 0), and per-vertex normals.
    let locator = FlatLocator::build(&region, &flat);
    let vertex_normals = vertex_normals(&region);

    // 5. Sample S^h with barycentric provenance (Eqs. 2–3 store the element
    //    and barycentrics precisely so the disk coordinate never needs a
    //    point location).
    let samples = sample_iso_scallop(&region, &flat, &vertex_normals, params);
    if samples.is_empty() {
        return Err(SpiralRefusal::NoSurfaceSamples);
    }
    report.surface_samples = samples.len();

    // 6. Coverage-driven ring spacing (Eqs. 1–4).
    let rings = search_rings(&locator, &vertex_normals, &region, &samples, params, report)?;

    // 7. Bridge into one spiral (Eqs. 7–9 + A-11), with the start-angle sweep.
    let (result, spiral_meta) = build_best_spiral(
        &locator,
        &vertex_normals,
        &region,
        &rings,
        &samples,
        params,
        report,
    );
    finish_report(&result, &rings, &spiral_meta, params, report);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Topology of the induced submesh
// ---------------------------------------------------------------------------

/// The one boundary loop, in half-edge order, plus the Euler data.
struct Topology {
    /// Local vertex ids around the single boundary loop, in half-edge
    /// direction (interior on the left for a consistently wound submesh).
    loop_vertices: Vec<usize>,
    /// 3D perimeter (mm).
    loop_length_mm: f64,
    /// Distinct undirected edges.
    edge_count: usize,
    /// `V − E + F`.
    euler: i64,
    /// Per local vertex: true when the vertex is on the boundary loop.
    on_boundary: Vec<bool>,
}

/// Extract the boundary loop and refuse anything that is not a disk.
///
/// **[REPO]** The 2025 paper takes "the longest boundary" as `Γ₀` and treats
/// the rest as holes; with no slit map there is no *rest*, so a second loop is
/// a refusal, not a choice. Loop walks start at the smallest local vertex id
/// so the emitted order is deterministic.
#[allow(clippy::result_large_err)]
fn region_topology(region: &RegionMesh) -> Result<Topology, SpiralRefusal> {
    // Directed half-edges in winding order; undirected use counts.
    let mut undirected: HashMap<(usize, usize), usize> = HashMap::new();
    let mut directed: HashMap<(usize, usize), usize> = HashMap::new();
    for t in 0..region.tris.len() {
        let c = region.corners(t);
        for k in 0..3 {
            let (a, b) = (
                c.get(k).copied().unwrap_or_default(),
                c.get((k + 1) % 3).copied().unwrap_or_default(),
            );
            if a == b {
                continue;
            }
            let key = if a <= b { (a, b) } else { (b, a) };
            *undirected.entry(key).or_insert(0) += 1;
            *directed.entry((a, b)).or_insert(0) += 1;
        }
    }
    if undirected.values().any(|&n| n > 2) {
        return Err(SpiralRefusal::NonManifoldBoundary {
            detail: NonManifoldDetail::EdgeOverUsed,
        });
    }

    // A boundary half-edge is one whose undirected edge is used exactly once.
    let mut next: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(a, b) in directed.keys() {
        let key = if a <= b { (a, b) } else { (b, a) };
        if undirected.get(&key).copied().unwrap_or(0) == 1 {
            next.entry(a).or_default().push(b);
        }
    }
    if next.values().any(|v| v.len() > 1) {
        return Err(SpiralRefusal::NonManifoldBoundary {
            detail: NonManifoldDetail::BoundaryPinch,
        });
    }

    // Walk every loop. Deterministic: ascending start vertex.
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_unstable();
    let mut visited: Vec<bool> = vec![false; region.verts.len()];
    let mut loops: Vec<Vec<usize>> = Vec::new();
    for &s in &starts {
        if visited.get(s).copied().unwrap_or(true) {
            continue;
        }
        let mut walk: Vec<usize> = Vec::new();
        let mut cur = s;
        // Bounded: each step consumes one boundary half-edge.
        for _ in 0..=next.len() {
            if visited.get(cur).copied().unwrap_or(true) {
                break;
            }
            if let Some(slot) = visited.get_mut(cur) {
                *slot = true;
            }
            walk.push(cur);
            let Some(nexts) = next.get(&cur) else {
                break;
            };
            let Some(&n) = nexts.first() else {
                break;
            };
            cur = n;
        }
        if !walk.is_empty() {
            loops.push(walk);
        }
    }

    let edge_count = undirected.len();
    let euler = region.verts.len() as i64 - edge_count as i64 + region.tris.len() as i64;
    if loops.len() != 1 {
        return Err(SpiralRefusal::NotSimplyConnected {
            boundary_loops: loops.len(),
        });
    }
    if euler != 1 {
        return Err(SpiralRefusal::NotADisk {
            euler_characteristic: euler,
        });
    }
    let Some(loop_vertices) = loops.into_iter().next() else {
        return Err(SpiralRefusal::NotSimplyConnected { boundary_loops: 0 });
    };

    let mut loop_length_mm = 0.0_f64;
    for (i, &v) in loop_vertices.iter().enumerate() {
        let w = loop_vertices
            .get((i + 1) % loop_vertices.len())
            .copied()
            .unwrap_or(v);
        loop_length_mm += (region.point(w) - region.point(v)).norm();
    }
    if loop_vertices.len() < 3 || loop_length_mm <= EPS_VEC {
        return Err(SpiralRefusal::DegenerateBoundaryLoop {
            vertices: loop_vertices.len(),
            length_mm: loop_length_mm,
        });
    }

    let mut on_boundary = vec![false; region.verts.len()];
    for &v in &loop_vertices {
        if let Some(slot) = on_boundary.get_mut(v) {
            *slot = true;
        }
    }
    Ok(Topology {
        loop_vertices,
        loop_length_mm,
        edge_count,
        euler,
        on_boundary,
    })
}

// ---------------------------------------------------------------------------
// [REPO] Harmonic disk map
// ---------------------------------------------------------------------------

/// Per-local-vertex position in the unit disk.
struct Flattening {
    uv: Vec<(f64, f64)>,
}

/// Flatten the region onto the unit disk.
///
/// **[REPO] Boundary correspondence is plain cumulative arc length.** Walking
/// the single boundary loop, vertex `k` at cumulative 3D arc length `s_k` on a
/// perimeter of `L` goes to `(cos 2πs_k/L, sin 2πs_k/L)`. This is *not*
/// Shen 2024's Eq. A-14: that is a corner-graded B-spline parameterisation
/// whose purpose is Nyström convergence for the boundary-integral slit map,
/// which this module does not solve. Phase 1 needs a correspondence, not a
/// quadrature.
///
/// **Interior vertices** solve two Laplace problems with **non-zero Dirichlet**
/// data. The cotangent stiffness matrix comes from
/// [`crate::direction_field`]'s `assemble_poisson` with an empty target field
/// (which makes its divergence right-hand side identically zero, leaving the
/// bare Laplacian); the known boundary values are then moved to the RHS:
///
/// ```text
/// interior i:  A_ii x_i − Σ_{j interior} w_ij x_j = Σ_{j ∈ ∂} w_ij u_j
/// ```
///
/// which is exactly the system `cg_solve` already solves when the boundary
/// rows are pinned, because it holds pinned unknowns at zero.
#[allow(clippy::result_large_err)]
fn flatten_to_disk(
    region: &RegionMesh,
    topo: &Topology,
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> Result<Flattening, SpiralRefusal> {
    let nv = region.verts.len();
    // An empty target field makes `assemble_poisson`'s Eq. 17 divergence term
    // vanish, so only the Eq. 16 cotangent stiffness survives.
    let (lap, _zero_rhs) = assemble_poisson(region, &[]);

    // Prescribed boundary values by arc length.
    let mut prescribed: Vec<Option<(f64, f64)>> = vec![None; nv];
    let mut acc = 0.0_f64;
    for (i, &v) in topo.loop_vertices.iter().enumerate() {
        let theta = TAU * acc / topo.loop_length_mm;
        if let Some(slot) = prescribed.get_mut(v) {
            *slot = Some((theta.cos(), theta.sin()));
        }
        let w = topo
            .loop_vertices
            .get((i + 1) % topo.loop_vertices.len())
            .copied()
            .unwrap_or(v);
        acc += (region.point(w) - region.point(v)).norm();
    }

    let pinned: &[bool] = &topo.on_boundary;
    let interior = pinned.iter().filter(|&&b| !b).count();
    report.flatten_interior_vertices = interior;

    // `cg_solve` takes the F1 parameter block; only its two CG dials are read.
    let cg_params = FieldParams {
        cg_max_iters: params.cg_max_iters,
        cg_rel_tolerance: params.cg_rel_tolerance,
        ..FieldParams::default()
    };

    let mut uv = vec![(0.0_f64, 0.0_f64); nv];
    for (axis, name) in [(0usize, "u"), (1usize, "v")] {
        let mut rhs = vec![0.0_f64; nv];
        for i in 0..nv {
            if pinned.get(i).copied().unwrap_or(false) {
                continue;
            }
            let Some(list) = lap.nbr.get(i) else {
                continue;
            };
            let mut acc = 0.0_f64;
            for &(j, w) in list {
                let Some(Some(val)) = prescribed.get(j) else {
                    continue;
                };
                acc += w * if axis == 0 { val.0 } else { val.1 };
            }
            if let Some(slot) = rhs.get_mut(i) {
                *slot = acc;
            }
        }
        let out = cg_solve(&lap, &rhs, pinned, &cg_params);
        if let Some(slot) = report.flatten_cg_iterations.get_mut(axis) {
            *slot = out.iterations;
        }
        if let Some(slot) = report.flatten_residual.get_mut(axis) {
            *slot = out.residual;
        }
        if !out.converged {
            return Err(SpiralRefusal::FlattenDidNotConverge {
                component: name,
                residual: out.residual,
                iterations: out.iterations,
            });
        }
        for i in 0..nv {
            let value = match prescribed.get(i) {
                Some(Some(val)) => {
                    if axis == 0 {
                        val.0
                    } else {
                        val.1
                    }
                }
                _ => out.x.get(i).copied().unwrap_or(0.0),
            };
            if let Some(slot) = uv.get_mut(i) {
                if axis == 0 {
                    slot.0 = value;
                } else {
                    slot.1 = value;
                }
            }
        }
    }
    Ok(Flattening { uv })
}

/// Flat position of a local vertex.
fn flat_of(flat: &Flattening, v: usize) -> (f64, f64) {
    flat.uv.get(v).copied().unwrap_or((0.0, 0.0))
}

/// Signed area of a flat triangle, positive when counter-clockwise.
fn signed_area_2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    0.5 * ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0))
}

/// Measure the map: flips, area distortion, angle distortion.
///
/// **[REPO]** The orientation reference is the *sum* of signed flat areas: a
/// map that preserves the boundary-loop orientation covers the disk with
/// positive area, one that reverses it with negative. A triangle whose sign
/// disagrees with that total is flipped. This makes the count independent of
/// whether the caller's submesh happens to be wound clockwise, while still
/// reporting a genuinely degenerate map (where the two populations are
/// comparable) as a large count rather than hiding it.
fn measure_flattening(region: &RegionMesh, flat: &Flattening, report: &mut SpiralReport) {
    let mut signed: Vec<f64> = Vec::with_capacity(region.tris.len());
    let mut total = 0.0_f64;
    for t in 0..region.tris.len() {
        let c = region.corners(t);
        let (a, b, d) = (
            flat_of(flat, c.first().copied().unwrap_or_default()),
            flat_of(flat, c.get(1).copied().unwrap_or_default()),
            flat_of(flat, c.get(2).copied().unwrap_or_default()),
        );
        let s = signed_area_2d(a, b, d);
        total += s;
        signed.push(s);
    }
    let orientation = if total < 0.0 { -1.0_f64 } else { 1.0_f64 };
    report.orientation_sign = orientation;
    report.flipped_triangles = signed.iter().filter(|&&s| s * orientation <= 0.0).count();

    let mut ratios: Vec<f64> = Vec::with_capacity(region.tris.len());
    let mut angle_err: Vec<f64> = Vec::with_capacity(region.tris.len() * 3);
    let mut radial: Vec<Vec<f64>> = vec![Vec::new(); RADIAL_BUCKETS];
    for (t, &s) in signed.iter().enumerate() {
        let area3 = region.area(t);
        let ratio = if area3 > MIN_TRIANGLE_AREA_MM2 {
            let r = s.abs() / area3;
            ratios.push(r);
            Some(r)
        } else {
            None
        };
        let c = region.corners(t);
        let p3: [P3; 3] = [
            region.point(c.first().copied().unwrap_or_default()),
            region.point(c.get(1).copied().unwrap_or_default()),
            region.point(c.get(2).copied().unwrap_or_default()),
        ];
        let p2: [(f64, f64); 3] = [
            flat_of(flat, c.first().copied().unwrap_or_default()),
            flat_of(flat, c.get(1).copied().unwrap_or_default()),
            flat_of(flat, c.get(2).copied().unwrap_or_default()),
        ];
        for k in 0..3 {
            let (i, j, m) = (k, (k + 1) % 3, (k + 2) % 3);
            let a3 = corner_angle_3d(&p3, i, j, m);
            let a2 = corner_angle_2d(&p2, i, j, m);
            if let (Some(a3), Some(a2)) = (a3, a2) {
                angle_err.push((a3 - a2).abs().to_degrees());
            }
        }
        // Radial bucket, keyed on the disk radius of the flattened centroid.
        if let Some(r) = ratio {
            let cx = p2.iter().map(|q| q.0).sum::<f64>() / 3.0;
            let cy = p2.iter().map(|q| q.1).sum::<f64>() / 3.0;
            let rad = cx.hypot(cy).clamp(0.0, 1.0);
            let b = ((rad * RADIAL_BUCKETS as f64).floor() as usize).min(RADIAL_BUCKETS - 1);
            if let Some(slot) = radial.get_mut(b) {
                slot.push(r);
            }
        }
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.area_distortion_min = ratios.first().copied().unwrap_or(0.0);
    report.area_distortion_max = ratios.last().copied().unwrap_or(0.0);
    report.area_distortion_median = median_sorted(&ratios);
    angle_err.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.angle_distortion_max_deg = angle_err.last().copied().unwrap_or(0.0);
    report.angle_distortion_median_deg = median_sorted(&angle_err);

    let width = 1.0 / RADIAL_BUCKETS as f64;
    report.area_distortion_by_disk_radius = radial
        .into_iter()
        .enumerate()
        .map(|(b, mut vals)| {
            vals.sort_by(|a, c| a.partial_cmp(c).unwrap_or(std::cmp::Ordering::Equal));
            RadialDistortion {
                r_lo: b as f64 * width,
                r_hi: (b + 1) as f64 * width,
                triangles: vals.len(),
                area_distortion_min: vals.first().copied().unwrap_or(0.0),
                area_distortion_median: median_sorted(&vals),
                area_distortion_max: vals.last().copied().unwrap_or(0.0),
            }
        })
        .collect();
}

/// Interior angle at corner `i` of a 3D triangle.
fn corner_angle_3d(p: &[P3; 3], i: usize, j: usize, k: usize) -> Option<f64> {
    let (a, b, c) = (p.get(i)?, p.get(j)?, p.get(k)?);
    let u = b - a;
    let v = c - a;
    let (nu, nv) = (u.norm(), v.norm());
    if nu <= EPS_VEC || nv <= EPS_VEC {
        return None;
    }
    Some((u.dot(&v) / (nu * nv)).clamp(-1.0, 1.0).acos())
}

/// Interior angle at corner `i` of a 2D triangle.
fn corner_angle_2d(p: &[(f64, f64); 3], i: usize, j: usize, k: usize) -> Option<f64> {
    let (a, b, c) = (p.get(i)?, p.get(j)?, p.get(k)?);
    let u = (b.0 - a.0, b.1 - a.1);
    let v = (c.0 - a.0, c.1 - a.1);
    let nu = u.0.hypot(u.1);
    let nv = v.0.hypot(v.1);
    if nu <= EPS_VEC || nv <= EPS_VEC {
        return None;
    }
    Some(
        ((u.0 * v.0 + u.1 * v.1) / (nu * nv))
            .clamp(-1.0, 1.0)
            .acos(),
    )
}

/// Median of an already-sorted slice.
fn median_sorted(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let n = v.len();
    if n % 2 == 1 {
        v.get(n / 2).copied().unwrap_or(0.0)
    } else {
        let a = v.get(n / 2 - 1).copied().unwrap_or(0.0);
        let b = v.get(n / 2).copied().unwrap_or(0.0);
        0.5 * (a + b)
    }
}

/// Area-weighted per-vertex normals over the region.
///
/// Inherits [`build_region_mesh`]'s +Z-forced triangle normals — the same
/// convention `crest_lines` and [`crate::direction_field`] use, and a stated
/// limitation of this module.
fn vertex_normals(region: &RegionMesh) -> Vec<V3> {
    let mut acc = vec![V3::zeros(); region.verts.len()];
    for t in 0..region.tris.len() {
        let n = region.normal(t) * region.area(t);
        for v in region.corners(t) {
            if let Some(slot) = acc.get_mut(v) {
                *slot += n;
            }
        }
    }
    for n in &mut acc {
        let len = n.norm();
        *n = if len > EPS_VEC {
            *n / len
        } else {
            V3::new(0.0, 0.0, 1.0)
        };
    }
    acc
}

// ---------------------------------------------------------------------------
// Disk → 3D: the flattened mesh as a z = 0 `TriangleMesh`
// ---------------------------------------------------------------------------

/// The flattened region as a mesh at `z = 0`, plus its spatial index.
///
/// Representing the flattening as a real [`TriangleMesh`] is what lets
/// [`SpatialIndex`] and [`crate::geo::Triangle::contains_point_xy`] be reused
/// verbatim for disk→3D lookup instead of a bespoke point locator. Triangle
/// index `t` here is local triangle `t` of the region, so a barycentric hit
/// interpolates the region's own 3D vertices directly.
struct FlatLocator {
    flat_mesh: TriangleMesh,
    index: SpatialIndex,
}

/// A located disk point.
struct Located {
    /// Local triangle index.
    tri: usize,
    /// Barycentric weights on that triangle's corners.
    bary: [f64; 3],
}

impl FlatLocator {
    fn build(region: &RegionMesh, flat: &Flattening) -> Self {
        let vertices: Vec<P3> = (0..region.verts.len())
            .map(|v| {
                let (x, y) = flat_of(flat, v);
                P3::new(x, y, 0.0)
            })
            .collect();
        let triangles: Vec<[u32; 3]> = region
            .tris
            .iter()
            .map(|c| {
                [
                    c.first().copied().unwrap_or_default() as u32,
                    c.get(1).copied().unwrap_or_default() as u32,
                    c.get(2).copied().unwrap_or_default() as u32,
                ]
            })
            .collect();
        let flat_mesh = TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build_auto(&flat_mesh);
        Self { flat_mesh, index }
    }

    /// Locate `(x, y)` in the flattened mesh, applying the
    /// [`PULLBACK_LADDER`] when the point lands in the sliver between a
    /// boundary chord and the unit circle.
    ///
    /// Returns the hit and whether a pullback was needed.
    fn locate(
        &self,
        x: f64,
        y: f64,
        scratch: &mut QueryScratch,
        hits: &mut Vec<usize>,
    ) -> Option<(Located, bool)> {
        if let Some(l) = self.locate_exact(x, y, scratch, hits) {
            return Some((l, false));
        }
        for f in PULLBACK_LADDER {
            let s = 1.0 - f;
            if let Some(l) = self.locate_exact(x * s, y * s, scratch, hits) {
                return Some((l, true));
            }
        }
        None
    }

    fn locate_exact(
        &self,
        x: f64,
        y: f64,
        scratch: &mut QueryScratch,
        hits: &mut Vec<usize>,
    ) -> Option<Located> {
        self.index.query_into(x, y, 0.0, scratch, hits);
        for &t in hits.iter() {
            let Some(face) = self.flat_mesh.faces.get(t) else {
                continue;
            };
            if !face.contains_point_xy(x, y) {
                continue;
            }
            let a = face.v.first()?;
            let b = face.v.get(1)?;
            let c = face.v.get(2)?;
            let denom = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
            if denom.abs() < 1e-15 {
                continue;
            }
            let w0 = ((b.y - c.y) * (x - c.x) + (c.x - b.x) * (y - c.y)) / denom;
            let w1 = ((c.y - a.y) * (x - c.x) + (a.x - c.x) * (y - c.y)) / denom;
            let w2 = 1.0 - w0 - w1;
            return Some(Located {
                tri: t,
                bary: [w0, w1, w2],
            });
        }
        None
    }
}

/// Interpolate a 3D contact point and its normal from a located disk point.
fn lift(region: &RegionMesh, normals: &[V3], loc: &Located) -> (P3, V3) {
    let c = region.corners(loc.tri);
    let mut p = V3::zeros();
    let mut n = V3::zeros();
    for (k, &v) in c.iter().enumerate() {
        let w = loc.bary.get(k).copied().unwrap_or(0.0);
        p += region.point(v).coords * w;
        n += normals.get(v).copied().unwrap_or_else(V3::zeros) * w;
    }
    let len = n.norm();
    let n = if len > EPS_VEC {
        n / len
    } else {
        V3::new(0.0, 0.0, 1.0)
    };
    (P3::from(p), n)
}

// ---------------------------------------------------------------------------
// [SOURCE-2025 §3.1] Sampling the iso-scallop surface S^h
// ---------------------------------------------------------------------------

/// One sampled point of `S^h`, with the barycentric provenance the paper's
/// Eqs. 2–3 require.
struct Sample {
    /// Position on `S^h` = contact point offset `+h` along the interpolated
    /// vertex normal.
    at: P3,
    /// Disk radius of the sample, from its **barycentric provenance** — never
    /// from a point location. This is exactly what the paper stores the
    /// element and barycentrics for ("to eliminate redundant coordinate
    /// transformations", §3.1).
    disk_r: f64,
}

/// Sample the region into `N_S` points and offset each `+h` along its normal.
///
/// **[REPO]** The paper states no sampling rule at all (**G-SAMPLING**). Here
/// the per-triangle quota is area-proportional by largest remainder — no
/// minimum of one per triangle, so `n_surface_samples` means what it says on a
/// mesh with more triangles than samples — and within a triangle the points
/// are the centroids of a deterministic `n × n` barycentric subdivision,
/// taken in a fixed order.
fn sample_iso_scallop(
    region: &RegionMesh,
    flat: &Flattening,
    normals: &[V3],
    params: &SpiralParams,
) -> Vec<Sample> {
    let total_area: f64 = (0..region.tris.len()).map(|t| region.area(t)).sum();
    if total_area <= MIN_TRIANGLE_AREA_MM2 || params.n_surface_samples == 0 {
        return Vec::new();
    }
    let want = params.n_surface_samples as f64;
    // Largest-remainder apportionment, deterministic in triangle order.
    let mut quota: Vec<usize> = Vec::with_capacity(region.tris.len());
    let mut rema: Vec<(f64, usize)> = Vec::with_capacity(region.tris.len());
    let mut assigned = 0usize;
    for t in 0..region.tris.len() {
        let exact = want * region.area(t) / total_area;
        let floor = exact.floor().max(0.0) as usize;
        quota.push(floor);
        assigned += floor;
        rema.push((exact - exact.floor(), t));
    }
    rema.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    let mut left = params.n_surface_samples.saturating_sub(assigned);
    for &(_, t) in &rema {
        if left == 0 {
            break;
        }
        if let Some(slot) = quota.get_mut(t) {
            *slot += 1;
            left -= 1;
        }
    }

    let mut out: Vec<Sample> = Vec::with_capacity(params.n_surface_samples);
    for (t, &q) in quota.iter().enumerate() {
        if q == 0 {
            continue;
        }
        let c = region.corners(t);
        for bary in barycentric_lattice(q) {
            let mut p = V3::zeros();
            let mut n = V3::zeros();
            let mut fx = 0.0_f64;
            let mut fy = 0.0_f64;
            for (k, &v) in c.iter().enumerate() {
                let w = bary.get(k).copied().unwrap_or(0.0);
                p += region.point(v).coords * w;
                n += normals.get(v).copied().unwrap_or_else(V3::zeros) * w;
                let f = flat_of(flat, v);
                fx += f.0 * w;
                fy += f.1 * w;
            }
            let len = n.norm();
            let n = if len > EPS_VEC {
                n / len
            } else {
                V3::new(0.0, 0.0, 1.0)
            };
            out.push(Sample {
                at: P3::from(p + n * params.scallop_h_mm),
                disk_r: fx.hypot(fy),
            });
        }
    }
    out
}

/// `q` deterministic barycentric coordinates inside the standard triangle.
///
/// The reference triangle is subdivided into `n²` sub-triangles; the returned
/// points are their centroids, uprights first then inverted, each family in
/// row-major order. `n` is the smallest integer with `n² ≥ q`, and the first
/// `q` centroids are taken.
fn barycentric_lattice(q: usize) -> Vec<[f64; 3]> {
    if q == 0 {
        return Vec::new();
    }
    let n = ((q as f64).sqrt().ceil() as usize).max(1);
    let nf = n as f64;
    let mut out: Vec<[f64; 3]> = Vec::with_capacity(q);
    for a in 0..n {
        for b in 0..(n - a) {
            let u = (a as f64 + 1.0 / 3.0) / nf;
            let v = (b as f64 + 1.0 / 3.0) / nf;
            out.push([1.0 - u - v, u, v]);
            if out.len() == q {
                return out;
            }
        }
    }
    for a in 0..n.saturating_sub(1) {
        for b in 0..(n.saturating_sub(1) - a) {
            let u = (a as f64 + 2.0 / 3.0) / nf;
            let v = (b as f64 + 2.0 / 3.0) / nf;
            out.push([1.0 - u - v, u, v]);
            if out.len() == q {
                return out;
            }
        }
    }
    // `n² ≥ q` guarantees the loops above fill the quota; the centroid is the
    // deterministic filler if a degenerate `n` ever left room.
    while out.len() < q {
        out.push([1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
    }
    out
}

// ---------------------------------------------------------------------------
// Coverage: a bucket grid over polyline segments
// ---------------------------------------------------------------------------

/// A uniform XY bucket grid over segment bounding boxes.
///
/// **[REPO]** The paper accelerates its Eq. 1 distance queries with KD-trees
/// (§3.1). No KD-tree is available here and none is being added — `kiddo` is
/// pinned in the workspace but used by no crate and stays that way — so a
/// uniform bucket over the segments' padded bounding boxes does the same job
/// for the modest `N_C` this prototype uses.
struct SegGrid {
    origin: (f64, f64),
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl SegGrid {
    /// Build over `boxes` (`[min_x, min_y, max_x, max_y]`), each padded by
    /// `pad` so a point within `pad` of a segment always lands in one of its
    /// cells.
    fn build(boxes: &[[f64; 4]], pad: f64, target_cell: f64) -> Self {
        let mut lo = (f64::INFINITY, f64::INFINITY);
        let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for b in boxes {
            lo.0 = lo.0.min(b.first().copied().unwrap_or(0.0) - pad);
            lo.1 = lo.1.min(b.get(1).copied().unwrap_or(0.0) - pad);
            hi.0 = hi.0.max(b.get(2).copied().unwrap_or(0.0) + pad);
            hi.1 = hi.1.max(b.get(3).copied().unwrap_or(0.0) + pad);
        }
        if !lo.0.is_finite() || !hi.0.is_finite() {
            lo = (0.0, 0.0);
            hi = (0.0, 0.0);
        }
        let span_x = (hi.0 - lo.0).max(0.0);
        let span_y = (hi.1 - lo.1).max(0.0);
        let axis_floor = (span_x.max(span_y) / MAX_GRID_AXIS as f64).max(1e-9);
        let cell = target_cell.max(axis_floor);
        let nx = ((span_x / cell).ceil() as usize + 1).min(MAX_GRID_AXIS);
        let ny = ((span_y / cell).ceil() as usize + 1).min(MAX_GRID_AXIS);
        let mut grid = Self {
            origin: lo,
            cell,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        };
        for (i, b) in boxes.iter().enumerate() {
            let (x0, y0, x1, y1) = grid.cell_range(&[
                b.first().copied().unwrap_or(0.0) - pad,
                b.get(1).copied().unwrap_or(0.0) - pad,
                b.get(2).copied().unwrap_or(0.0) + pad,
                b.get(3).copied().unwrap_or(0.0) + pad,
            ]);
            let stride = grid.nx;
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    if let Some(list) = grid.cells.get_mut(cy * stride + cx) {
                        list.push(i as u32);
                    }
                }
            }
        }
        grid
    }

    fn cell_range(&self, b: &[f64; 4]) -> (usize, usize, usize, usize) {
        let idx = |v: f64, o: f64, n: usize| -> usize {
            let i = ((v - o) / self.cell).floor();
            if i < 0.0 {
                0
            } else {
                (i as usize).min(n.saturating_sub(1))
            }
        };
        (
            idx(b.first().copied().unwrap_or(0.0), self.origin.0, self.nx),
            idx(b.get(1).copied().unwrap_or(0.0), self.origin.1, self.ny),
            idx(b.get(2).copied().unwrap_or(0.0), self.origin.0, self.nx),
            idx(b.get(3).copied().unwrap_or(0.0), self.origin.1, self.ny),
        )
    }

    /// Candidate segment indices for a query point.
    fn at(&self, x: f64, y: f64) -> &[u32] {
        let (cx, cy, _, _) = self.cell_range(&[x, y, x, y]);
        let empty: &[u32] = &[];
        self.cells
            .get(cy * self.nx + cx)
            .map_or(empty, |v| v.as_slice())
    }

    /// Candidate segment indices whose cells intersect a box.
    fn in_box(&self, b: &[f64; 4]) -> Vec<u32> {
        let (x0, y0, x1, y1) = self.cell_range(b);
        let mut out: Vec<u32> = Vec::new();
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if let Some(list) = self.cells.get(cy * self.nx + cx) {
                    out.extend_from_slice(list);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// Squared distance from `p` to the segment `a`–`b`.
fn dist2_point_segment(p: P3, a: P3, b: P3) -> f64 {
    let ab = b - a;
    let len2 = ab.norm_squared();
    if len2 <= EPS_VEC {
        return (p - a).norm_squared();
    }
    let t = ((p - a).dot(&ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).norm_squared()
}

/// A tool-centre polyline plus its query acceleration.
struct CentreCurve {
    pts: Vec<P3>,
    grid: SegGrid,
}

impl CentreCurve {
    fn new(pts: &[P3], radius: f64) -> Self {
        let mut boxes: Vec<[f64; 4]> = Vec::with_capacity(pts.len().saturating_sub(1));
        for w in pts.windows(2) {
            let (a, b) = (w.first(), w.get(1));
            if let (Some(a), Some(b)) = (a, b) {
                boxes.push([a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]);
            }
        }
        let grid = SegGrid::build(&boxes, radius, radius.max(1e-6));
        Self {
            pts: pts.to_vec(),
            grid,
        }
    }

    /// **[SOURCE-2025 Eq. 1]** Is `p` (a point of `S^h`) swept by a ball of
    /// radius `K_c` travelling along this centre curve? Distance is measured
    /// to the **segments**, not to the sampled vertices.
    fn covers(&self, p: P3, radius: f64) -> bool {
        let r2 = radius * radius;
        for &i in self.grid.at(p.x, p.y) {
            let i = i as usize;
            let (Some(&a), Some(&b)) = (self.pts.get(i), self.pts.get(i + 1)) else {
                continue;
            };
            if dist2_point_segment(p, a, b) <= r2 {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// [SOURCE-2025 §2.2.1, Eqs. 1–4] Coverage-driven ring spacing
// ---------------------------------------------------------------------------

/// Running counters for disk→3D lookups.
#[derive(Default)]
struct LiftStats {
    pulled_back: usize,
    unlocated: usize,
}

/// Lift a disk-domain polyline to 3D contact points and tool-centre points.
///
/// The centre curve is the contact curve offset `+K_c` along the interpolated
/// surface normal — **[SOURCE-2025 §3.1]**'s own construction. See the module
/// header for why the instrument's drop-cutter CL supersedes it downstream.
#[allow(clippy::too_many_arguments)]
fn lift_polyline(
    locator: &FlatLocator,
    region: &RegionMesh,
    normals: &[V3],
    disk: &[(f64, f64)],
    radius: f64,
    scratch: &mut QueryScratch,
    hits: &mut Vec<usize>,
    stats: &mut LiftStats,
) -> (Vec<P3>, Vec<P3>) {
    let flags = vec![false; disk.len()];
    let out = lift_disk_aligned(
        locator, region, normals, disk, &flags, radius, scratch, hits, stats,
    );
    (out.contact, out.centre)
}

/// Disk-domain points of a closed circle of radius `r`, on the shared angular
/// lattice starting at `a0`. The first point is repeated as the last.
fn circle_disk_points(r: f64, a0: f64, n: usize) -> Vec<(f64, f64)> {
    let n = n.max(3);
    (0..=n)
        .map(|j| {
            let a = a0 + TAU * (j as f64) / (n as f64);
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// One placed ring.
struct Ring {
    /// Disk radius `R_i^S`.
    radius: f64,
    /// Closed 3D contact polyline.
    contact: Vec<P3>,
    /// `S^h` points first covered by this ring — the paper's band `BP_i`.
    band: Vec<usize>,
}

/// 3D polyline length.
fn polyline_length(p: &[P3]) -> f64 {
    p.windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .map(|(a, b)| (b - a).norm())
        .sum()
}

/// Brute-force 3D distance from a point to a polyline.
///
/// [`CentreCurve::covers`] answers "within `K_c`?" through a bucket grid; it
/// cannot answer "how far?", because a point outside every padded cell has no
/// candidates at all. The stall diagnostic needs the actual distance, so it
/// walks every segment. Strided sampling keeps that affordable — see
/// [`STALL_DISTANCE_SAMPLE_CAP`].
fn distance_to_polyline_mm(p: P3, pts: &[P3]) -> f64 {
    let mut best = f64::INFINITY;
    for w in pts.windows(2) {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
            continue;
        };
        best = best.min(dist2_point_segment(p, a, b));
    }
    if best.is_finite() {
        best.sqrt()
    } else {
        f64::INFINITY
    }
}

/// Min / median / max distance from a stride-sampled subset of `uncovered` to
/// `reference`, plus how many samples were taken.
fn uncovered_distance_summary(
    uncovered: &[usize],
    samples: &[Sample],
    reference: &[P3],
) -> (usize, f64, f64, f64) {
    if uncovered.is_empty() || reference.len() < 2 {
        return (0, 0.0, 0.0, 0.0);
    }
    let stride = uncovered.len().div_ceil(STALL_DISTANCE_SAMPLE_CAP).max(1);
    let mut d: Vec<f64> = uncovered
        .iter()
        .step_by(stride)
        .filter_map(|&s| samples.get(s))
        .map(|s| distance_to_polyline_mm(s.at, reference))
        .filter(|v| v.is_finite())
        .collect();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (
        d.len(),
        d.first().copied().unwrap_or(0.0),
        median_sorted(&d),
        d.last().copied().unwrap_or(0.0),
    )
}

/// Everything the ring loop must publish on **every** exit path, successful
/// or not, so a refusal is diagnosable from the report alone.
struct RingLoopState {
    iterations: usize,
    search_lo: f64,
    search_hi: f64,
    interior_feasible: bool,
}

/// Write the ring rows, and the stall block when coverage did not close.
#[allow(clippy::too_many_arguments)]
fn publish_ring_rows(
    rings: &[Ring],
    uncovered: &[usize],
    samples: &[Sample],
    last_centre: &[P3],
    failed_centre: &[P3],
    state: &RingLoopState,
    stats: &LiftStats,
    params: &SpiralParams,
    report: &mut SpiralReport,
) {
    report.binary_search_iterations = state.iterations;
    report.ring_points_pulled_back = stats.pulled_back;
    report.ring_points_unlocated = stats.unlocated;
    report.uncovered_after_rings = uncovered.len();
    report.ring_count = rings.len();
    report.ring_radii = rings.iter().map(|r| r.radius).collect();
    report.ring_lengths_mm = rings.iter().map(|r| polyline_length(&r.contact)).collect();
    report.ring_newly_covered = rings.iter().map(|r| r.band.len()).collect();
    report.total_ring_length_mm = report.ring_lengths_mm.iter().sum();
    if uncovered.is_empty() {
        return;
    }
    let (reference, which) = if last_centre.len() >= 2 {
        (
            last_centre,
            StallDistanceReference::LastPlacedRing(rings.len().saturating_sub(1)),
        )
    } else {
        (failed_centre, StallDistanceReference::FailedCandidate)
    };
    let (taken, dmin, dmed, dmax) = uncovered_distance_summary(uncovered, samples, reference);
    report.stall = Some(StallContext {
        rings_placed: rings.len(),
        ring_radii: rings.iter().map(|r| r.radius).collect(),
        search_lo: state.search_lo,
        search_hi: state.search_hi,
        interior_radius_feasible: state.interior_feasible,
        uncovered: uncovered.len(),
        distance_samples: taken,
        distance_reference: which,
        coverage_radius_mm: params.ball_radius_mm,
        uncovered_distance_min_mm: dmin,
        uncovered_distance_median_mm: dmed,
        uncovered_distance_max_mm: dmax,
    });
}

/// Place rings by the paper's binary search until every `S^h` sample is swept.
///
/// **[SOURCE-2025 Eqs. 1–4]** For ring `i`, search `R` in `[0, R_{i−1}]` for
/// the **smallest** radius at which the set
/// `{ uncovered samples with |P^S| > R that the ring at R does not sweep }` is
/// exactly empty. `R = R_{i−1}` is always feasible (everything outside it is
/// already covered, by the previous ring's own guarantee), which anchors the
/// upper end of every search; `R = 1` anchors the first.
///
/// Monotonicity of that predicate in `R` is **assumed**, as it is in the
/// paper — see the module header's limitations. Two effects fight as `R`
/// grows: more points fall inside `R` and are excused, but the ring moves and
/// can *drop* a point it used to sweep. When no interior radius is feasible at
/// all, the search returns its upper anchor unchanged, the "new" ring is the
/// previous one, and it covers nothing — which is the
/// [`SpiralRefusal::RingSearchStalled`] path.
/// [`StallContext::interior_radius_feasible`] is `false` exactly then.
#[allow(clippy::result_large_err)]
fn search_rings(
    locator: &FlatLocator,
    normals: &[V3],
    region: &RegionMesh,
    samples: &[Sample],
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> Result<Vec<Ring>, SpiralRefusal> {
    let radius = params.ball_radius_mm;
    let mut scratch = QueryScratch::new();
    let mut hits: Vec<usize> = Vec::new();
    let mut stats = LiftStats::default();

    let mut uncovered: Vec<usize> = (0..samples.len()).collect();
    let mut rings: Vec<Ring> = Vec::new();
    let mut last_centre: Vec<P3> = Vec::new();
    let mut hi = 1.0_f64;
    let mut state = RingLoopState {
        iterations: 0,
        search_lo: 0.0,
        search_hi: 1.0,
        interior_feasible: false,
    };

    // A ring's contact polyline, its centre polyline and the query structure.
    let mut curve_at = |r: f64, stats: &mut LiftStats| -> (Vec<P3>, Vec<P3>, CentreCurve) {
        let disk = circle_disk_points(r, 0.0, params.n_angular_samples);
        let (contact, centre) = lift_polyline(
            locator,
            region,
            normals,
            &disk,
            radius,
            &mut scratch,
            &mut hits,
            stats,
        );
        let cc = CentreCurve::new(&centre, radius);
        (contact, centre, cc)
    };

    while !uncovered.is_empty() {
        if rings.len() >= params.max_rings {
            let err = SpiralRefusal::RingLimitReached {
                rings: rings.len(),
                uncovered: uncovered.len(),
            };
            publish_ring_rows(
                &rings,
                &uncovered,
                samples,
                &last_centre,
                &[],
                &state,
                &stats,
                params,
                report,
            );
            return Err(err);
        }
        // Binary search for the smallest feasible R in [lo, hi].
        let mut lo = 0.0_f64;
        let mut best_hi = hi;
        let mut interior_feasible = false;
        let mut guard = 0usize;
        while best_hi - lo > params.ring_eps && guard < 4096 {
            guard += 1;
            state.iterations += 1;
            let mid = 0.5 * (lo + best_hi);
            let (_, _, cc) = curve_at(mid, &mut stats);
            let feasible = uncovered.iter().all(|&s| {
                let Some(sample) = samples.get(s) else {
                    return true;
                };
                sample.disk_r <= mid || cc.covers(sample.at, radius)
            });
            if feasible {
                best_hi = mid;
                interior_feasible = true;
            } else {
                lo = mid;
            }
        }
        state.search_lo = lo;
        state.search_hi = best_hi;
        state.interior_feasible = interior_feasible;

        let (contact, centre, cc) = curve_at(best_hi, &mut stats);
        let mut band: Vec<usize> = Vec::new();
        let mut still: Vec<usize> = Vec::with_capacity(uncovered.len());
        for &s in &uncovered {
            let Some(sample) = samples.get(s) else {
                continue;
            };
            if cc.covers(sample.at, radius) {
                band.push(s);
            } else {
                still.push(s);
            }
        }
        if band.is_empty() {
            let err = SpiralRefusal::RingSearchStalled {
                rings: rings.len(),
                uncovered: uncovered.len(),
            };
            publish_ring_rows(
                &rings,
                &uncovered,
                samples,
                &last_centre,
                &centre,
                &state,
                &stats,
                params,
                report,
            );
            return Err(err);
        }
        rings.push(Ring {
            radius: best_hi,
            contact,
            band,
        });
        last_centre = centre;
        uncovered = still;
        hi = best_hi;
        if hi <= params.ring_eps && !uncovered.is_empty() {
            // The search has reached the disk centre with samples still
            // uncovered: no further ring can be placed inside this one.
            break;
        }
    }

    publish_ring_rows(
        &rings,
        &uncovered,
        samples,
        &last_centre,
        &[],
        &state,
        &stats,
        params,
        report,
    );
    Ok(rings)
}

// ---------------------------------------------------------------------------
// [SOURCE-2024 Eq. A-11 / A-12] The bridge blend σ(t)
// ---------------------------------------------------------------------------

/// **[SOURCE-2024 arXiv:2309.10655 v2, Eq. A-11 with A-12]**, verbatim:
///
/// ```text
/// σ(t) = 2π · v(t)^p / ( v(t)^p + v(2π − t)^p )
/// v(t) = (1/p − 1/2)·((π − t)/π)³ + (1/p)·((t − π)/π) + 1/2 ,  t ∈ [0, 2π]
/// ```
///
/// `p ≥ 2` is the grading parameter; **no value is given in either paper**
/// (**G-SIGMA-REFINEMENT**), so [`DEFAULT_BLEND_P`] is a [REPO] choice.
///
/// Properties the bridge relies on, all in the source: `σ` is a bijection of
/// `[0, 2π]` onto itself, strictly increasing, `C^∞`, and — the load-bearing
/// one — **`σ′(0) = σ′(2π) = 0`**, which is what makes the transition curve of
/// **[SOURCE-2025 Eq. 9]** tangent to both parallel lines it joins.
///
/// In its own paper σ is a *corner-grading reparameterisation of a boundary*,
/// not a bridge blend; the 2025 paper reuses its shape. That repurposing is
/// the 2025 paper's, not this module's, and the "refining" it mentions is
/// specified in neither text — so σ is used here unrefined.
#[must_use]
pub fn blend_sigma(t: f64, p: f64) -> f64 {
    let p = p.max(2.0);
    let v = |x: f64| -> f64 {
        let a = (1.0 / p - 0.5) * ((PI - x) / PI).powi(3);
        let b = (1.0 / p) * ((x - PI) / PI);
        (a + b + 0.5).clamp(0.0, 1.0)
    };
    let num = v(t).powf(p);
    let den = num + v(TAU - t).powf(p);
    if !den.is_finite() || den <= f64::MIN_POSITIVE {
        return 0.0;
    }
    TAU * num / den
}

// ---------------------------------------------------------------------------
// [SOURCE-2025 §2.2.2, Eqs. 7–9] Bridging the rings into one spiral
// ---------------------------------------------------------------------------

/// **[SOURCE-2025 Eq. 8]** `S^S = imag(S^R) · e^{i·real(S^R)}` — roll one
/// rectangle point back to the disk. (Eq. 7 is its inverse,
/// `arg(z) + i·|z|`; only this direction is needed, because the rings are
/// *constructed* in rectangle coordinates rather than measured from the disk.)
fn roll_to_disk(real: f64, imag: f64) -> (f64, f64) {
    (imag * real.cos(), imag * real.sin())
}

/// Everything the report needs about one built spiral.
struct SpiralMeta {
    start_angle: f64,
    candidates: usize,
    bridge_count: usize,
    bridge_length_mm: f64,
    repair_steps: usize,
    uncovered_after_bridging: usize,
}

/// The rectangle-domain construction, before any lifting.
struct SpiralDomain {
    disk: Vec<(f64, f64)>,
    /// `true` for a point emitted by a bridge. Segment `k → k+1` is a bridge
    /// segment exactly when `is_bridge[k + 1]`.
    is_bridge: Vec<bool>,
}

/// Round an angular shift onto the shared lattice.
fn shift_cells(shift: f64, dtheta: f64, floor: usize) -> usize {
    let raw = (shift / dtheta).round();
    if !raw.is_finite() || raw < floor as f64 {
        floor
    } else {
        raw as usize
    }
}

/// Build the spiral in the `(angle, radius)` rectangle and roll it to the disk.
///
/// **[SOURCE-2025 Eqs. 7–9]**, with the **[REPO]** bookkeeping resolution of
/// the module header's **G-BRIDGE-BOOKKEEPING**: run `i` spans
/// `2π + line_shift_i` of angle at radius `R_i`, then a bridge of angular span
/// `D_i` descends to `R_{i+1}` along Eq. 9. `D_i` follows the paper's own
/// near-centre rule (Pseudocode A-2 lines 6–9). The last ring gets its run and
/// **no trailing bridge**.
///
/// Both runs and bridges are sampled on **one** lattice `a₀ + j·2π/N_C`; see
/// [`SpiralParams::n_angular_samples`] for why that matters to the
/// self-intersection measurement.
fn build_spiral_domain(
    rings: &[Ring],
    a0: f64,
    line_shift_cells: &[usize],
    params: &SpiralParams,
) -> SpiralDomain {
    let n = params.n_angular_samples.max(3);
    let dtheta = TAU / (n as f64);
    let mut disk: Vec<(f64, f64)> = Vec::new();
    let mut is_bridge: Vec<bool> = Vec::new();
    let mut push = |real: f64, imag: f64, bridge: bool| {
        let p = roll_to_disk(real, imag);
        if let Some(&last) = disk.last()
            && (last.0 - p.0).hypot(last.1 - p.1) <= EPS_DISK
        {
            return;
        }
        disk.push(p);
        is_bridge.push(bridge);
    };

    let mut real = a0;
    for (i, ring) in rings.iter().enumerate() {
        let run = n + line_shift_cells.get(i).copied().unwrap_or(0);
        let first = usize::from(i > 0);
        for j in first..=run {
            push(real + (j as f64) * dtheta, ring.radius, false);
        }
        real += (run as f64) * dtheta;

        let Some(next) = rings.get(i + 1) else {
            continue;
        };
        let shift = if ring.radius > params.near_centre_radius {
            params.initial_bridge_shift
        } else {
            params.near_centre_bridge_shift
        };
        let bc = shift_cells(shift, dtheta, 1);
        let (r0, r1) = (ring.radius, next.radius);
        for j in 1..=bc {
            let t = (j as f64) / (bc as f64);
            let imag = r0 + (r1 - r0) * blend_sigma(TAU * t, params.blend_p) / TAU;
            push(real + (j as f64) * dtheta, imag, true);
        }
        real += (bc as f64) * dtheta;
    }
    SpiralDomain { disk, is_bridge }
}

/// A lifted disk polyline: disk domain, bridge flags, 3D contact points and
/// 3D tool-centre points, all four index-aligned.
struct Lifted {
    disk: Vec<(f64, f64)>,
    flags: Vec<bool>,
    contact: Vec<P3>,
    centre: Vec<P3>,
}

/// Lift a disk polyline, **keeping the disk points index-aligned** with the
/// 3D output: a point that cannot be located even after the
/// [`PULLBACK_LADDER`] is dropped from all four lists at once, so
/// [`SpiralResult::spiral_disk`] and [`SpiralResult::spiral_contact`] can
/// never drift apart.
#[allow(clippy::too_many_arguments)]
fn lift_disk_aligned(
    locator: &FlatLocator,
    region: &RegionMesh,
    normals: &[V3],
    disk: &[(f64, f64)],
    flags: &[bool],
    radius: f64,
    scratch: &mut QueryScratch,
    hits: &mut Vec<usize>,
    stats: &mut LiftStats,
) -> Lifted {
    let mut kept_disk: Vec<(f64, f64)> = Vec::with_capacity(disk.len());
    let mut kept_flags: Vec<bool> = Vec::with_capacity(disk.len());
    let mut contact: Vec<P3> = Vec::with_capacity(disk.len());
    let mut centre: Vec<P3> = Vec::with_capacity(disk.len());
    for (i, &(x, y)) in disk.iter().enumerate() {
        let Some((loc, pulled)) = locator.locate(x, y, scratch, hits) else {
            stats.unlocated += 1;
            continue;
        };
        if pulled {
            stats.pulled_back += 1;
        }
        let (p, n) = lift(region, normals, &loc);
        kept_disk.push((x, y));
        kept_flags.push(flags.get(i).copied().unwrap_or(false));
        contact.push(p);
        centre.push(P3::from(p.coords + n * radius));
    }
    Lifted {
        disk: kept_disk,
        flags: kept_flags,
        contact,
        centre,
    }
}

/// Build the spiral for every start angle in the sweep and keep the shortest,
/// then run the §2.2.2 step-2 coverage repair on the winner.
///
/// **[SOURCE-2025 §2.2.2, A-2 lines 3 and 30–35]** sweeps `real(P_Start^1)`
/// over `[0, 2π)` and keeps the minimum-total-3D-length spiral.
///
/// **[REPO ordering]** The paper's pseudocode repairs coverage inside the
/// sweep. Here the sweep runs with the initial shifts and the repair runs once
/// on the winner, which is materially the same because in the simply-connected
/// case the repair is a no-op by construction — each run already traverses its
/// whole ring at that ring's own radius — and `bridge_repair_steps` reports
/// whether that held.
fn build_best_spiral(
    locator: &FlatLocator,
    normals: &[V3],
    region: &RegionMesh,
    rings: &[Ring],
    samples: &[Sample],
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> (SpiralResult, SpiralMeta) {
    let radius = params.ball_radius_mm;
    let mut scratch = QueryScratch::new();
    let mut hits: Vec<usize> = Vec::new();
    let mut stats = LiftStats::default();
    let dtheta = params.lattice_step();

    // Initial along-line shifts. The paper's secondary constant applies to the
    // ring *after* an outer bridge; it is 0 by default here — see the module
    // header's G-BRIDGE-BOOKKEEPING.
    let secondary = shift_cells(params.secondary_line_shift, dtheta, 0);
    let mut line_shift_cells: Vec<usize> = (0..rings.len())
        .map(|i| {
            let after_outer = i > 0
                && rings
                    .get(i - 1)
                    .is_some_and(|prev| prev.radius > params.near_centre_radius);
            if after_outer { secondary } else { 0 }
        })
        .collect();

    let step = if params.start_angle_step > 1e-6 {
        params.start_angle_step
    } else {
        TAU
    };
    let candidates = ((TAU / step).floor() as usize).max(1);

    // Start-angle sweep: keep the minimum-total-3D-length spiral.
    let mut best_angle = 0.0_f64;
    let mut best_len = f64::INFINITY;
    let mut kept_disk: Vec<(f64, f64)> = Vec::new();
    let mut kept_flags: Vec<bool> = Vec::new();
    let mut contact: Vec<P3> = Vec::new();
    let mut centre: Vec<P3> = Vec::new();
    for k in 0..candidates {
        // **[REPO] lattice discretisation of the sweep.** The start angle is
        // snapped onto the same `2π/N_C` lattice as everything else. Without
        // this, a run's chords sit out of phase with the ring chords that made
        // the coverage decision, and a band sample within one chord sagitta of
        // the `K_c` boundary reads uncovered — which the repair loop *cannot*
        // fix, because growing the along-line shift re-traverses the same
        // lattice at the same phase. Snapping is also arguably the more
        // faithful reading: the paper's own π/50 step is exactly one cell of a
        // 100-point lattice.
        let a0 = ((k as f64) * step / dtheta).round() * dtheta;
        let dom = build_spiral_domain(rings, a0, &line_shift_cells, params);
        let lifted = lift_disk_aligned(
            locator,
            region,
            normals,
            &dom.disk,
            &dom.is_bridge,
            radius,
            &mut scratch,
            &mut hits,
            &mut stats,
        );
        let len = polyline_length(&lifted.contact);
        if len < best_len {
            best_len = len;
            best_angle = a0;
            kept_disk = lifted.disk;
            kept_flags = lifted.flags;
            contact = lifted.contact;
            centre = lifted.centre;
        }
    }

    // SOURCE-2025 §2.2.2 step 2: grow the along-line shift until every ring's
    // band is still swept by the *bridged* spiral's centre curve.
    let mut repair_steps = 0usize;
    let mut uncovered_after;
    let mut previous_uncovered = usize::MAX;
    loop {
        let cc = CentreCurve::new(&centre, radius);
        let mut worst: Option<usize> = None;
        let mut uncovered = 0usize;
        for (i, ring) in rings.iter().enumerate() {
            let missing = ring
                .band
                .iter()
                .filter(|&&s| samples.get(s).is_some_and(|p| !cc.covers(p.at, radius)))
                .count();
            if missing > 0 {
                uncovered += missing;
                if worst.is_none() {
                    worst = Some(i);
                }
            }
        }
        uncovered_after = uncovered;
        let Some(i) = worst else {
            break;
        };
        if repair_steps >= params.max_bridge_repair_steps {
            break;
        }
        // Stall guard: a shift that removes nothing will never remove
        // anything, and 200 fruitless full rebuilds is a hang, not a repair.
        if uncovered >= previous_uncovered {
            break;
        }
        previous_uncovered = uncovered;
        let bump = shift_cells(params.shift_step, dtheta, 1);
        if let Some(slot) = line_shift_cells.get_mut(i) {
            *slot += bump;
        }
        repair_steps += 1;
        let dom = build_spiral_domain(rings, best_angle, &line_shift_cells, params);
        let lifted = lift_disk_aligned(
            locator,
            region,
            normals,
            &dom.disk,
            &dom.is_bridge,
            radius,
            &mut scratch,
            &mut hits,
            &mut stats,
        );
        kept_disk = lifted.disk;
        kept_flags = lifted.flags;
        contact = lifted.contact;
        centre = lifted.centre;
    }

    report.ring_points_pulled_back += stats.pulled_back;
    report.ring_points_unlocated += stats.unlocated;

    // A segment is a bridge segment exactly when its later endpoint is one.
    let mut bridge_length = 0.0_f64;
    for k in 0..contact.len().saturating_sub(1) {
        if !kept_flags.get(k + 1).copied().unwrap_or(false) {
            continue;
        }
        let (Some(&a), Some(&b)) = (contact.get(k), contact.get(k + 1)) else {
            continue;
        };
        bridge_length += (b - a).norm();
    }

    let result = SpiralResult {
        spiral_contact: contact,
        spiral_disk: kept_disk,
        rings_contact: rings.iter().map(|r| r.contact.clone()).collect(),
    };
    let meta = SpiralMeta {
        start_angle: best_angle,
        candidates,
        bridge_count: rings.len().saturating_sub(1),
        bridge_length_mm: bridge_length,
        repair_steps,
        uncovered_after_bridging: uncovered_after,
    };
    (result, meta)
}

// ---------------------------------------------------------------------------
// [REPO] Measurements on the finished spiral
// ---------------------------------------------------------------------------

/// Count **proper** segment-segment crossings of the spiral in the disk
/// domain.
///
/// Measured rather than inferred. The construction argues that concentric
/// runs at distinct radii, joined by radius-monotone bridges, cannot cross —
/// but that argument is about the continuum, and the emitted path is chords.
/// Only crossings in the open interior of both segments count: a run's first
/// and last points coincide (a full turn), and every bridge shares an endpoint
/// with the runs on either side, so endpoint contact is expected and is not a
/// crossing.
fn count_disk_self_intersections(disk: &[(f64, f64)]) -> usize {
    let n = disk.len().saturating_sub(1);
    if n < 2 {
        return 0;
    }
    let mut boxes: Vec<[f64; 4]> = Vec::with_capacity(n);
    for k in 0..n {
        let (Some(&a), Some(&b)) = (disk.get(k), disk.get(k + 1)) else {
            continue;
        };
        boxes.push([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]);
    }
    // One cell per few segments keeps the candidate lists short on a disk of
    // radius 1 without letting the grid explode.
    let target = (2.0 / (n as f64).sqrt()).max(1e-6);
    let grid = SegGrid::build(&boxes, 0.0, target);
    let mut count = 0usize;
    for (k, bx) in boxes.iter().enumerate() {
        for j in grid.in_box(bx) {
            let j = j as usize;
            if j <= k + 1 {
                continue;
            }
            let (Some(&a), Some(&b), Some(&c), Some(&d)) =
                (disk.get(k), disk.get(k + 1), disk.get(j), disk.get(j + 1))
            else {
                continue;
            };
            if segments_properly_cross(a, b, c, d) {
                count += 1;
            }
        }
    }
    count
}

/// 2D cross product of `(b − a)` and `(c − a)`.
fn cross2(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Proper crossing only: both orientation pairs must have strictly opposite
/// signs, with `EPS_CROSS` treating a touching or collinear configuration as
/// no crossing.
fn segments_properly_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let d1 = cross2(a, b, c);
    let d2 = cross2(a, b, d);
    let d3 = cross2(c, d, a);
    let d4 = cross2(c, d, b);
    if d1.abs() <= EPS_CROSS
        || d2.abs() <= EPS_CROSS
        || d3.abs() <= EPS_CROSS
        || d4.abs() <= EPS_CROSS
    {
        return false;
    }
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Fill the spiral-side report rows.
fn finish_report(
    result: &SpiralResult,
    rings: &[Ring],
    meta: &SpiralMeta,
    _params: &SpiralParams,
    report: &mut SpiralReport,
) {
    report.start_angle_rad = meta.start_angle;
    report.start_angle_candidates = meta.candidates;
    report.bridge_count = meta.bridge_count;
    report.total_bridge_length_mm = meta.bridge_length_mm;
    report.bridge_repair_steps = meta.repair_steps;
    report.uncovered_after_bridging = meta.uncovered_after_bridging;

    report.spiral_points = result.spiral_contact.len();
    report.spiral_length_mm = polyline_length(&result.spiral_contact);
    let ring_total: f64 = rings.iter().map(|r| polyline_length(&r.contact)).sum();
    report.bridge_overhead_pct = if ring_total > EPS_VEC {
        100.0 * (report.spiral_length_mm - ring_total) / ring_total
    } else {
        0.0
    };
    // Structural: one polyline, no lift anywhere in the construction.
    report.retract_count = 0;

    let mut steps: Vec<f64> = result
        .spiral_contact
        .windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .map(|(a, b)| (b - a).norm())
        .collect();
    steps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.max_consecutive_step_mm = steps.last().copied().unwrap_or(0.0);
    report.median_consecutive_step_mm = median_sorted(&steps);

    report.disk_self_intersections = count_disk_self_intersections(&result.spiral_disk);
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
        Flattening, SpiralParams, SpiralRefusal, SpiralReport, StallDistanceReference, Topology,
        blend_sigma, build_region_mesh, flatten_to_disk, measure_flattening, plan_spiral,
        region_topology,
    };
    use crate::direction_field::all_triangles;
    use crate::geo::P3;
    use crate::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
    use crate::scallop_math::{stepover_from_scallop_curved, stepover_from_scallop_flat};
    use std::f64::consts::TAU;

    /// A triangulated disk: a centre vertex, `n_rings` concentric rings and a
    /// fan/strip triangulation. Wound counter-clockwise seen from +Z, so the
    /// single boundary loop runs counter-clockwise with the interior on its
    /// left — the convention `region_topology` and the flip count assume.
    fn disk_mesh(radius: f64, n_rings: usize, n_around: usize) -> TriangleMesh {
        let mut vertices = vec![P3::new(0.0, 0.0, 0.0)];
        for i in 1..=n_rings {
            let r = radius * (i as f64) / (n_rings as f64);
            for j in 0..n_around {
                let t = TAU * (j as f64) / (n_around as f64);
                vertices.push(P3::new(r * t.cos(), r * t.sin(), 0.0));
            }
        }
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        for j in 0..n_around {
            let jn = (j + 1) % n_around;
            triangles.push([0, (1 + j) as u32, (1 + jn) as u32]);
        }
        for i in 0..(n_rings - 1) {
            let s = 1 + i * n_around;
            let ns = 1 + (i + 1) * n_around;
            for j in 0..n_around {
                let jn = (j + 1) % n_around;
                let (a, b) = ((s + j) as u32, (s + jn) as u32);
                let (c, d) = ((ns + j) as u32, (ns + jn) as u32);
                triangles.push([a, c, b]);
                triangles.push([b, c, d]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// Run only the front half of the pipeline: submesh, topology, flattening,
    /// flatten metrics. Used where the map itself is under test and the ring
    /// search would only add runtime.
    fn flatten_only(mesh: &TriangleMesh) -> (Topology, Flattening, SpiralReport) {
        let region = build_region_mesh(mesh, &all_triangles(mesh)).unwrap();
        let topo = region_topology(&region).unwrap();
        let params = SpiralParams::default();
        let mut report = SpiralReport {
            region_triangles: region.tris.len(),
            region_vertices: region.verts.len(),
            // Mirror plan_spiral's topology rows — the helper bypasses the
            // pipeline, so it must fill what the pipeline fills or the report
            // under test silently reads Default zeros.
            region_edges: topo.edge_count,
            euler_characteristic: topo.euler,
            boundary_loop_vertices: topo.loop_vertices.len(),
            boundary_loop_length_mm: topo.loop_length_mm,
            ..SpiralReport::default()
        };
        let flat = flatten_to_disk(&region, &topo, &params, &mut report).unwrap();
        measure_flattening(&region, &flat, &mut report);
        (topo, flat, report)
    }

    // -----------------------------------------------------------------
    // SOURCE-2024 Eq. A-11
    // -----------------------------------------------------------------

    #[test]
    fn sigma_has_the_a11_endpoint_and_monotonicity_properties() {
        let p = 2.0;
        assert!(blend_sigma(0.0, p).abs() < 1e-12, "σ(0) must be 0");
        assert!(
            (blend_sigma(TAU, p) - TAU).abs() < 1e-12,
            "σ(2π) must be 2π"
        );

        // σ'(0) = σ'(2π) = 0 — the property Eq. 9's tangency depends on.
        let dt = 1e-5;
        let d0 = (blend_sigma(dt, p) - blend_sigma(0.0, p)) / dt;
        let d1 = (blend_sigma(TAU, p) - blend_sigma(TAU - dt, p)) / dt;
        assert!(d0.abs() < 1e-3, "σ'(0) should vanish, got {d0}");
        assert!(d1.abs() < 1e-3, "σ'(2π) should vanish, got {d1}");

        // Strictly increasing across the interval.
        let mut prev = -1.0_f64;
        for k in 0..=200 {
            let t = TAU * (k as f64) / 200.0;
            let s = blend_sigma(t, p);
            assert!(s > prev - 1e-15, "σ must be monotone at t={t}");
            assert!((0.0..=TAU + 1e-12).contains(&s), "σ out of range at t={t}");
            prev = s;
        }

        // Higher p is admissible and keeps the endpoint conditions.
        for p in [2.0_f64, 3.0, 5.0] {
            assert!(blend_sigma(0.0, p).abs() < 1e-12);
            assert!((blend_sigma(TAU, p) - TAU).abs() < 1e-12);
            assert!((blend_sigma(PI_HALF_TURN, p) - TAU / 2.0).abs() < 1e-9);
        }
    }

    /// `σ(π) = π` for every `p`: A-12 gives `v(π) = 1/2`, so numerator and
    /// denominator halve exactly.
    const PI_HALF_TURN: f64 = std::f64::consts::PI;

    // -----------------------------------------------------------------
    // [REPO] Harmonic disk map — closed forms
    // -----------------------------------------------------------------

    #[test]
    fn flat_disk_flattens_to_the_exact_affine_map() {
        // The cotangent stiffness matrix annihilates linear functions at every
        // interior vertex of a *planar* triangulation, and equally spaced
        // boundary vertices on a circle get equal arc-length shares, so
        // `z ↦ z/ρ` is the exact discrete solution. Everything below is that
        // closed form.
        let radius = 10.0_f64;
        let mesh = disk_mesh(radius, 8, 40);
        let region = build_region_mesh(&mesh, &all_triangles(&mesh)).unwrap();
        let (_topo, flat, report) = flatten_only(&mesh);

        assert_eq!(report.euler_characteristic, 1);
        assert_eq!(report.boundary_loop_vertices, 40);
        assert_eq!(report.flipped_triangles, 0);
        assert!(report.orientation_sign > 0.0);

        for v in 0..region.verts.len() {
            let p = region.point(v);
            let (u, w) = flat.uv[v];
            assert!(
                (u - p.x / radius).abs() < 5e-6 && (w - p.y / radius).abs() < 5e-6,
                "vertex {v}: harmonic map should be z/ρ, got ({u}, {w}) vs ({}, {})",
                p.x / radius,
                p.y / radius
            );
        }

        // Affine ⇒ constant area distortion (1/ρ²) and zero angle distortion.
        let expect = 1.0 / (radius * radius);
        assert!((report.area_distortion_median - expect).abs() < 1e-7);
        assert!(
            report.area_distortion_max - report.area_distortion_min < 1e-7,
            "area distortion should be constant, spread was {}",
            report.area_distortion_max - report.area_distortion_min
        );
        assert!(report.angle_distortion_max_deg < 1e-3);
    }

    #[test]
    fn all_boundary_region_solves_nothing_and_prescribes_everything() {
        // `make_test_flat` is two triangles and four vertices, every one of
        // them on the boundary. The interior system is empty; the solve must
        // report that honestly rather than iterating on nothing.
        let mesh = make_test_flat(100.0);
        let (topo, flat, report) = flatten_only(&mesh);

        assert_eq!(topo.loop_vertices.len(), 4);
        assert_eq!(report.euler_characteristic, 1);
        assert_eq!(report.flatten_interior_vertices, 0);
        assert_eq!(report.flatten_cg_iterations, [0, 0]);
        for (u, v) in &flat.uv {
            assert!(
                (u.hypot(*v) - 1.0).abs() < 1e-12,
                "every prescribed vertex lands on the unit circle"
            );
        }
        assert_eq!(report.flipped_triangles, 0);
    }

    // -----------------------------------------------------------------
    // Refusals
    // -----------------------------------------------------------------

    #[test]
    fn a_punched_interior_triangle_is_refused_as_not_simply_connected() {
        let mesh = disk_mesh(10.0, 4, 24);
        let index = SpatialIndex::build_auto(&mesh);
        let all = all_triangles(&mesh);
        // Triangle 0 is a centre-fan triangle: centre vertex plus two ring-1
        // vertices, none of them on the outer boundary. Removing a
        // boundary-adjacent triangle would only notch the outer loop and still
        // leave one loop, which is why the punch must be interior.
        let holed: Vec<u32> = all.iter().copied().filter(|&t| t != 0).collect();
        let params = SpiralParams::new(2.0, 0.15);
        let (report, outcome) = plan_spiral(&mesh, &index, &holed, &params);
        assert_eq!(
            outcome.unwrap_err(),
            SpiralRefusal::NotSimplyConnected { boundary_loops: 2 },
            "an annulus must be refused, not approximated: the slit map is F2 step 3"
        );
        // The report survives the refusal, filled as far as the pipeline got:
        // the submesh was built, so its rows are there; the flattening never
        // ran, so its rows are not.
        assert_eq!(report.region_triangles, all.len() - 1);
        assert!(report.area_distortion_by_disk_radius.is_empty());
        assert!(report.stall.is_none());
    }

    // -----------------------------------------------------------------
    // Full pipeline — flat disk (exact map, closed-form spacing)
    // -----------------------------------------------------------------

    #[test]
    fn flat_disk_spirals_at_the_flat_scallop_stepover() {
        let radius = 10.0_f64;
        let ball = 2.0_f64;
        let h = 0.15_f64;
        let mesh = disk_mesh(radius, 10, 48);
        let index = SpatialIndex::build_auto(&mesh);
        let params = SpiralParams {
            n_surface_samples: 4000,
            n_angular_samples: 120,
            ring_eps: 0.002,
            // The paper sweeps in π/50 steps (100 candidates). On a disk every
            // start angle is equivalent by symmetry, so two candidates
            // exercise the sweep without paying for 100 rebuilds.
            start_angle_step: TAU / 2.0,
            ..SpiralParams::new(ball, h)
        };
        let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);
        let result = outcome.expect("simply-connected fixture must plan");

        // Map quality first: if the map were bad, every spacing number below
        // would be measuring the wrong thing.
        assert_eq!(report.flipped_triangles, 0);
        assert!(report.angle_distortion_max_deg < 1e-3);
        assert_eq!(report.ring_points_unlocated, 0);
        // Affine map ⇒ the radial profile is flat: every bucket carries the
        // same 1/ρ² distortion. This is the control reading for the profile
        // that a high-relief region is expected to make explode.
        assert_eq!(report.area_distortion_by_disk_radius.len(), 5);
        for b in &report.area_distortion_by_disk_radius {
            assert!(
                b.triangles > 0,
                "empty radial bucket {}..{}",
                b.r_lo,
                b.r_hi
            );
            assert!((b.area_distortion_median - 1.0 / (radius * radius)).abs() < 1e-7);
        }
        assert!(report.stall.is_none(), "coverage closed, so no stall block");

        // Coverage closed on both sides of the bridging step.
        assert_eq!(
            report.uncovered_after_rings, 0,
            "rings must sweep all of S^h"
        );
        assert_eq!(report.uncovered_after_bridging, 0);
        // A full-2π run reproduces its ring's own centre polyline exactly —
        // same radius, same lattice — so the repair has nothing to do. That
        // holds because TAU/2 is an exact multiple of the TAU/120 lattice, so
        // the swept start angle keeps run points in phase with ring points.
        assert_eq!(
            report.bridge_repair_steps, 0,
            "a full-2π run needs no repair"
        );

        // One continuous path.
        assert!(!result.spiral_contact.is_empty());
        assert_eq!(result.spiral_contact.len(), result.spiral_disk.len());
        assert_eq!(report.retract_count, 0);
        assert_eq!(report.disk_self_intersections, 0);
        // The longest ring chord is 2π·10·0.92/120 ≈ 0.48 mm; a hidden jump
        // would show up here as a step an order of magnitude larger.
        assert!(
            report.max_consecutive_step_mm < 1.0,
            "max step {} mm",
            report.max_consecutive_step_mm
        );
        assert_eq!(report.bridge_count, report.ring_count - 1);
        assert_eq!(result.rings_contact.len(), report.ring_count);
        assert_eq!(report.start_angle_candidates, 2);

        // Bridge overhead. Analytic estimate for this fixture: five outer
        // bridges at π/10 (5 % of their own ring each) over rings summing to
        // ΣR ≈ 3.10, plus one near-centre bridge at 2π (≈ 100 % of a ring at
        // R ≈ 0.09), against a total ring length ∝ ΣR ≈ 3.28 — about 7.4 %.
        // The bar is 20 % so sampling jitter in the ring set cannot flip it.
        assert!(
            report.bridge_overhead_pct > 0.0 && report.bridge_overhead_pct < 20.0,
            "bridge overhead {} %",
            report.bridge_overhead_pct
        );

        // Rings pull back to exact circles, because the map is exactly z/ρ.
        assert!(report.ring_count >= 5);
        for (i, ring) in result.rings_contact.iter().enumerate() {
            let want = radius * report.ring_radii[i];
            for p in ring {
                assert!(
                    (p.x.hypot(p.y) - want).abs() < 1e-4,
                    "ring {i} should be a circle of radius {want}, saw {}",
                    p.x.hypot(p.y)
                );
                assert!(p.z.abs() < 1e-6);
            }
        }

        // Spacing against the closed form. Sampling is the only error source:
        // 4000 samples over π·10² mm² is ~0.28 mm apart, 18 % of the 1.52 mm
        // stepover, so the bar is 25 %. Pairs whose inner ring is closer to
        // the centre than one stepover are excluded — there the spacing is
        // bounded by the ring's own radius, not by the scallop rule.
        let expect = stepover_from_scallop_flat(ball, h);
        let mut checked = 0usize;
        for i in 0..report.ring_count.saturating_sub(1) {
            let inner = radius * report.ring_radii[i + 1];
            if inner <= expect {
                continue;
            }
            let gap = radius * (report.ring_radii[i] - report.ring_radii[i + 1]);
            assert!(
                (gap - expect).abs() <= 0.25 * expect,
                "ring {i}->{} spacing {gap} mm vs closed form {expect} mm",
                i + 1
            );
            checked += 1;
        }
        assert!(checked >= 3, "only {checked} spacings were in range");
    }

    // -----------------------------------------------------------------
    // Full pipeline — hemisphere (curved scallop rule)
    // -----------------------------------------------------------------

    #[test]
    fn hemisphere_spirals_at_the_curved_scallop_stepover() {
        let sphere_r = 20.0_f64;
        let ball = 3.0_f64;
        let h = 0.8_f64;
        let mesh = make_test_hemisphere(sphere_r, 20);
        let index = SpatialIndex::build_auto(&mesh);
        let params = SpiralParams {
            n_surface_samples: 4000,
            n_angular_samples: 120,
            ring_eps: 0.003,
            start_angle_step: TAU / 2.0,
            ..SpiralParams::new(ball, h)
        };
        let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);
        let result = outcome.expect("simply-connected fixture must plan");

        // One boundary loop — the equator — and disk topology.
        assert_eq!(report.boundary_loop_vertices, 80);
        assert_eq!(report.euler_characteristic, 1);
        assert!(
            (report.boundary_loop_length_mm - TAU * sphere_r).abs() < 0.2 * sphere_r,
            "equator length {}",
            report.boundary_loop_length_mm
        );
        // Every triangle of this fixture is acute or right, so no cotangent
        // weight is negative and Tutte's condition holds: the discrete
        // harmonic map cannot flip. Counted rather than assumed.
        assert_eq!(report.flipped_triangles, 0);
        // The map is harmonic, not conformal, so area distortion varies from
        // pole to boundary — that is expected and is why it is reported.
        assert!(report.area_distortion_max >= report.area_distortion_min);

        assert_eq!(report.uncovered_after_rings, 0);
        assert!(report.stall.is_none());
        assert_eq!(report.area_distortion_by_disk_radius.len(), 5);
        assert_eq!(report.uncovered_after_bridging, 0);
        assert!(!result.spiral_contact.is_empty());
        assert_eq!(result.spiral_contact.len(), result.spiral_disk.len());
        assert_eq!(report.retract_count, 0);
        assert_eq!(report.disk_self_intersections, 0);
        assert!(report.ring_count >= 4, "only {} rings", report.ring_count);
        assert_eq!(report.bridge_count, report.ring_count - 1);

        // Mean polar angle per ring, measured from the emitted contact points
        // so a dropped point cannot shift the comparison.
        let polar: Vec<f64> = result
            .rings_contact
            .iter()
            .map(|ring| {
                let n = ring.len().max(1) as f64;
                ring.iter().map(|p| p.x.hypot(p.y).atan2(p.z)).sum::<f64>() / n
            })
            .collect();

        // 4000 samples over 2π·20² mm² sit ~0.71 mm apart, 19 % of the
        // 3.76 mm curved stepover, so the bar is 30 %.
        let expect = stepover_from_scallop_curved(ball, h, 1.0 / sphere_r);
        let mut checked = 0usize;
        for i in 0..polar.len().saturating_sub(1) {
            if polar[i + 1] <= expect / sphere_r {
                continue;
            }
            let gap = sphere_r * (polar[i] - polar[i + 1]).abs();
            assert!(
                (gap - expect).abs() <= 0.30 * expect,
                "ring {i}->{} meridian spacing {gap} mm vs curved form {expect} mm",
                i + 1
            );
            checked += 1;
        }
        assert!(checked >= 2, "only {checked} spacings were in range");
    }

    /// The stall diagnostic is itself an instrument, so it gets measured on a
    /// case whose answer is known in closed form rather than only on the
    /// terrain run that motivated it.
    ///
    /// `max_rings: 1` forces the ring loop to give up after one ring on the
    /// exact-affine flat disk. Every remaining uncovered sample is, by
    /// definition, farther than `K_c` from that ring's centre curve — so the
    /// measured minimum distance must be at least `K_c`, and the distribution
    /// must widen from there toward the disk centre.
    #[test]
    fn a_forced_ring_limit_publishes_a_measurable_stall_block() {
        let radius = 10.0_f64;
        let ball = 2.0_f64;
        let mesh = disk_mesh(radius, 10, 48);
        let index = SpatialIndex::build_auto(&mesh);
        let params = SpiralParams {
            n_surface_samples: 1500,
            n_angular_samples: 90,
            ring_eps: 0.005,
            max_rings: 1,
            start_angle_step: TAU,
            ..SpiralParams::new(ball, 0.15)
        };
        let (report, outcome) = plan_spiral(&mesh, &index, &all_triangles(&mesh), &params);

        match outcome.unwrap_err() {
            SpiralRefusal::RingLimitReached { rings, uncovered } => {
                assert_eq!(rings, 1);
                assert!(uncovered > 0);
            }
            other => panic!("expected RingLimitReached, got {other:?}"),
        }

        // The report survived, with the ring rows the refusal was about.
        assert_eq!(report.ring_count, 1);
        assert_eq!(report.ring_radii.len(), 1);
        assert_eq!(report.ring_lengths_mm.len(), 1);
        assert!(report.uncovered_after_rings > 0);
        assert_eq!(report.area_distortion_by_disk_radius.len(), 5);

        let stall = report.stall.expect("an unfinished ring search must say so");
        assert_eq!(stall.rings_placed, 1);
        assert_eq!(stall.ring_radii.len(), 1);
        assert_eq!(stall.uncovered, report.uncovered_after_rings);
        assert_eq!(
            stall.distance_reference,
            StallDistanceReference::LastPlacedRing(0)
        );
        assert!((stall.coverage_radius_mm - ball).abs() < 1e-12);
        assert!(stall.distance_samples > 0);
        assert!(stall.distance_samples <= 2000, "sample cap not honoured");
        // Uncovered means farther than K_c, by construction.
        assert!(
            stall.uncovered_distance_min_mm >= ball - 1e-6,
            "min distance {} should be at least K_c {ball}",
            stall.uncovered_distance_min_mm
        );
        assert!(stall.uncovered_distance_median_mm >= stall.uncovered_distance_min_mm);
        assert!(stall.uncovered_distance_max_mm >= stall.uncovered_distance_median_mm);
        // The farthest uncovered point is the disk centre: its distance to a
        // ring at 3D radius ρR₁ ≈ 9.24, whose centre curve sits K_c above the
        // plane, is √(9.24² + (K_c − h)²) ≈ 9.4 mm. Nothing can exceed that.
        assert!(
            stall.uncovered_distance_max_mm < radius + ball,
            "max distance {} mm",
            stall.uncovered_distance_max_mm
        );
    }

    #[test]
    fn an_empty_region_is_refused() {
        let mesh = disk_mesh(10.0, 3, 12);
        let index = SpatialIndex::build_auto(&mesh);
        let params = SpiralParams::default();
        let (report, outcome) = plan_spiral(&mesh, &index, &[], &params);
        assert_eq!(outcome.unwrap_err(), SpiralRefusal::EmptyRegion);
        assert_eq!(report.region_triangles, 0);
    }
}
