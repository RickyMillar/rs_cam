//! **Phase F2.1 research module — simply-connected conformal-spiral prototype.**
//!
//! Research-only. Nothing here is on a production path: no operation, no
//! generator, no GUI surface and no MCP tool reaches this module, and none
//! should until the Phase F2 evidence in
//! `planning/conformal_finish_2026-08-28/PROGRAMME.md` says what it is worth.
//! It follows the precedent of [`crate::finish::direction_field`] (Phase F1) and
//! [`crate::finish::scallop_isofield`] — unshipped research candidates that document
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
//! * **[SOURCE-FLOATER]** the flattening's interior weights only — Floater,
//!   *Mean value coordinates*, CAGD 20(1):19–27 (2003), with the embedding
//!   guarantee from Tutte (1963). See "Why mean-value weights" below.
//! * **[REPO]** everything else — in particular the *choice* of a plain disk
//!   flattening in place of the papers' BFF + slit map, the arc-length
//!   boundary correspondence, the Gauss–Seidel solver, the surface sampling
//!   scheme, the spiral's angular bookkeeping and every metric in
//!   [`SpiralReport`].
//!
//! # What this module does
//!
//! For a **simply connected** (single boundary loop, disk topology) region of
//! a triangle mesh and a 3-axis ball-end cutter:
//!
//! 1. **Disk map.** Flatten the induced submesh to the unit disk: boundary
//!    vertices prescribed onto the unit circle by cumulative **arc length**
//!    (**[REPO]**), interior vertices as convex combinations of their
//!    neighbours under **[SOURCE-FLOATER]** mean-value weights, solved by
//!    Gauss–Seidel (**[REPO]**). The result is a guaranteed fold-free
//!    embedding — see below for the measurement that forced this choice.
//! 2. **[SOURCE-2025 §2.2.1, Eqs. 1–4]** Ring spacing: each ring's radius is
//!    binary-searched in the disk until every sampled iso-scallop point
//!    *outside* that radius is swept by the ring's tool-centre curve.
//! 3. **[SOURCE-2025 §2.2.2, Eqs. 7–9]** Bridging: unroll the disk to the
//!    `(angle, radius)` rectangle, connect ring `i` to ring `i+1` with the
//!    blend `σ(t)` of **[SOURCE-2024 Eq. A-11]**, and sweep the start angle
//!    for the minimum-total-3D-length spiral.
//!
//! # Why not conformal — a labelled [REPO] substitution
//!
//! The 2025 paper flattens with BFF and then applies a conformal **slit map**;
//! the slit map is what makes holes work, and it is absent from that paper
//! (extraction gap 1). This module handles the **simply-connected** case only,
//! where there are no slits at all, so the front-end degenerates to "some map
//! of the region onto the unit disk". A discrete map with a prescribed convex
//! boundary is used instead of a conformal one, because:
//!
//! * the paper's spacing mechanism is a **sampled 3D coverage check** and the
//!   extraction is explicit that this check *is* the only distortion
//!   compensation in the pipeline — "there is no conformal-distortion-factor
//!   formula anywhere" (§3.1). Any bijective map therefore yields correct 3D
//!   spacing. **Half of that claim is now under measurement.** The
//!   *correctness* half stands and is distortion-proof by construction. The
//!   *spacing* half does not follow: the search has one scalar degree of
//!   freedom per ring and sizes it by the ring's **worst sector**, so the
//!   variation of the map's radial scale around that circle is over-cover
//!   forced on every other sector — and a conformal map, being a local
//!   similarity, has none of that anisotropy. [`SpiralReport::dilatation_min`]
//!   and [`RingAnisotropy`] were added to decide this by measurement rather
//!   than by argument;
//! * with **mean-value weights** (below) the map is a *guaranteed embedding*,
//!   which the coverage mechanism needs far more than it needs conformality.
//!
//! **This substitution is valid only while there are no slits.** The moment a
//! hole enters the region it stops being a substitution for anything and the
//! slit map (Phase F2 step 3) is required. [`plan_spiral`] refuses a
//! multi-boundary region rather than approximating one.
//!
//! # Why mean-value weights and not the cotangent Laplacian — measured
//!
//! The first version of this module used the cotangent Laplacian (the
//! discrete harmonic map, reusing Phase F1's assembly). **The F2.1 evidence
//! run falsified that choice.** On `terrain_small` the flattening produced
//! **298 flipped triangles of 9107**, and on a flat 308-triangle sub-region
//! still **7**; both arms then stalled the ring search hard, and the radial
//! area-distortion profile came back nearly flat (climb 1.833), which ruled
//! out the competing "distortion makes the bands razor-thin" explanation.
//!
//! A flipped triangle is a **fold**: the map is not injective there, so the
//! disk→3D direction is multi-valued. `FlatLocator::locate` resolves that
//! silently by taking the first containing triangle it finds, so a ring
//! sweeping through a folded sector lifts onto the *wrong sheet* — while an
//! `S^h` sample's own disk radius, which comes from forward barycentric
//! provenance, stays correct. Forward and inverse map then disagree, the
//! sample is 3D-far from the ring that its disk radius says should sweep it,
//! **every** candidate radius becomes infeasible, and the search stalls. That
//! mechanism is scale-free in the flip count, which is why 7 flips broke the
//! flat arm exactly as 298 broke the steep one.
//!
//! **[SOURCE-FLOATER]** the fix is Floater's **mean-value weights**
//! (M. S. Floater, *Mean value coordinates*, Computer Aided Geometric Design
//! 20(1):19–27, 2003; the embedding guarantee is Tutte's, W. T. Tutte, *How to
//! draw a graph*, Proc. London Math. Soc. 13:743–767, 1963):
//!
//! ```text
//! w_ij = ( tan(δ_ij / 2) + tan(γ_ij / 2) ) / ‖v_i − v_j‖
//! ```
//!
//! where `δ_ij` and `γ_ij` are the two angles **at vertex i** flanking edge
//! `ij` in the two triangles incident to it. Every triangle interior angle
//! lies in `(0, π)`, so every `tan(θ/2)` — and therefore every weight — is
//! **strictly positive**. Each interior vertex is then a convex combination of
//! its neighbours, the boundary goes to a convex polygon, and Tutte's
//! spring-embedding theorem makes the result a valid embedding: **folds become
//! structurally impossible rather than empirically rare.**
//! [`SpiralReport::flipped_triangles`] is kept exactly as it was and is now
//! the **tripwire on that invariant** — a nonzero count means a bug, not bad
//! luck — joined by [`SpiralReport::mean_value_weight_nonpositive`], which
//! measures the precondition itself instead of trusting it.
//!
//! **The trade, stated:** mean-value is *not* the Laplace/harmonic map, so
//! angular distortion may be slightly worse. That is a good trade here — the
//! paper's spacing mechanism compensates distortion by design and does not
//! compensate folds at all — and the report's distortion tables measure the
//! cost rather than hiding it.
//!
//! # The solver, and why it is not Phase F1's CG
//!
//! Mean-value weights are **unsymmetric**: `w_ij ≠ w_ji`, because the angles
//! are taken at `i` and the edge length divides only once. Conjugate gradient
//! assumes symmetric positive definite, so F1's `cg_solve` is **not usable
//! here and is no longer called** — and symmetrising the matrix would silently
//! destroy the Tutte guarantee that motivated the whole change, so it is not
//! done.
//!
//! Instead the interior system is solved by **Gauss–Seidel sweeps** in
//! ascending vertex order (the [`crate::finish::scallop_isofield`] precedent for a
//! hand-rolled sweep solver). Each sweep sets every interior vertex to the
//! weighted average of its neighbours, boundary values held fixed:
//! `u_i ← (Σ_j w_ij u_j) / (Σ_j w_ij)`. Convergence is unconditional: the
//! weights are positive and every interior vertex reaches the boundary, so the
//! iteration matrix is irreducibly diagonally dominant. The criterion is the
//! **maximum absolute coordinate change over a sweep** — an absolute tolerance
//! is the right shape here because the codomain is the unit disk — and both
//! coordinates are swept together, since they share one weight matrix.
//! Hitting [`SpiralParams::solver_max_sweeps`] is a typed refusal carrying the
//! measured delta, never a silent under-solve; that matters because an
//! under-converged iterate *can* still show folds, so the tripwire and the
//! convergence criterion are guarding the same invariant from two sides.
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
//!   convention `crest_lines` and [`crate::finish::direction_field`] use). Correct
//!   for the terrain-like regions F2 targets and for both fixtures here;
//!   **wrong for a region whose surface faces away from +Z**.
//! * **No reachability, no gouge check, no collision check.** 3-axis
//!   reachability of steep walls is outside the paper's scope entirely.
//! * **The paper's spacing criterion has ZERO MARGIN by construction, and
//!   that is a property of the method, not a bug.** Eqs. 1–4 push each ring
//!   in until it just reaches the outermost still-uncovered sample, so
//!   adjacent rings end up exactly `2·reach` apart and the midline between
//!   them lands exactly *on* the coverage boundary. That is the iso-scallop
//!   condition restated — at the optimal stepover the scallop crest sits
//!   exactly at height `h`, i.e. exactly on `S^h`. **The criterion therefore
//!   guarantees coverage of the search's own sample set and of nothing
//!   else**; any denser or offset population lands on the knife edge.
//!   Measured 2026-08-30 on the exact-affine flat disk: three whole
//!   48-triangle centroid families sat within 0.003–0.049 mm of a band edge
//!   and read as unmachined. [`SpiralParams::ring_spacing_safety`] is the
//!   dial that buys margin; it defaults to `1.0`, the paper's criterion
//!   unchanged.
//! * **The ring step is QUANTISED BY THE SAMPLE SET, and overshoots.** Ring
//!   `i+1` is placed at `(largest sample radius not covered by ring i) −
//!   reach'`, so the step is `2·reach'` *plus* whatever radial gap the samples
//!   happen to have there — it can never be smaller, and it is larger by the
//!   local sample granularity. On a mesh whose per-triangle quota is 1 the
//!   barycentric lattice puts one sample at each centroid, leaving a
//!   sample-free annulus of `2/3` of a facet's radial width at **every** mesh
//!   vertex ring; the step then snaps to the mesh's own feature pitch.
//!   Measured on the flat fixture (1 mm facets, 1.520 mm stepover): a derated
//!   `2·reach'` of 0.866 mm produced a ring series of
//!   **1.000, 1.000, 1.000, 1.000, 1.056 mm** — the mesh pitch, not the
//!   scallop rule. Coverage survives only while
//!   `ring_spacing_margin_mm ≥ max_local_sample_spacing_mm`; both are
//!   reported, and this is the same lesson the F2 phase-1 withdrawal recorded
//!   at instrument scale (`planning/conformal_finish_2026-08-28/`): **you
//!   cannot measure a stepover on a mesh whose facets are comparable to it.**
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
//!   [`crate::finish::scallop_math`].

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use crate::finish::direction_field::{RegionMesh, build_region_mesh};
use crate::geo::{P3, V3, polyline_length};
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
pub(crate) const PAPER_NEAR_CENTRE_RADIUS: f64 = 0.3;

/// **[SOURCE-2025 Pseudocode A-2 lines 6–9]** The near-centre bridge shift.
pub(crate) const PAPER_NEAR_CENTRE_BRIDGE_SHIFT: f64 = TAU;

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
pub(crate) const PAPER_SHIFT_STEP: f64 = PI / 50.0;

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

/// Triangle interior angles are clamped into `[MIN_TRIANGLE_ANGLE, π − …]`
/// before `tan(θ/2)`, so a degenerate corner produces a large finite weight
/// rather than an infinity. Positivity — the property the Tutte guarantee
/// rests on — is preserved by the clamp.
const MIN_TRIANGLE_ANGLE: f64 = 1e-9;

/// **[REPO]** Inward radial probe step, in disk units, for the per-ring
/// radial-scale measurement. Small enough to stay inside one flat triangle on
/// any realistic mesh — where the map is affine and the finite difference is
/// therefore *exact* — and far above `f64` noise.
const RADIAL_PROBE_DELTA: f64 = 1e-4;

/// **[REPO]** A ring whose band is this small or smaller is counted as
/// degenerate: it costs a full pass and removes almost nothing.
const DEGENERATE_BAND_MAX: usize = 2;

/// **[REPO]** Radial buckets for the area-distortion profile. Five is enough
/// to see whether distortion explodes toward the disk centre — the shape the
/// ring-stall hypothesis predicts on a high-relief region — without turning a
/// report row into a histogram nobody reads.
const RADIAL_BUCKETS: usize = 5;

/// **[REPO]** Cap on how many still-uncovered samples the stall diagnostic
/// measures distances for. The measurement is brute-force point-to-polyline
/// (the bucket grid can only answer "within `K_c`?", not "how far?"), so it is
/// strided rather than exhaustive; each [`DistanceStats::samples`] reports how
/// many were actually taken.
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
    /// `N_S` — **a floor on** how many points `S^h` is sampled into, not a
    /// cap.
    ///
    /// **[REPO]** No selection rule appears in either paper
    /// (**G-SAMPLING**); the 2025 paper's own Table 1 case 1.5 is a
    /// *documented failure* from choosing this too small.
    ///
    /// **Why a floor.** Apportioning a fixed budget by area starves small
    /// triangles: with `N_S` below the triangle count the per-triangle share
    /// averages under 1, most triangles floor to 0, and on a polar mesh the
    /// ones that lose out are exactly the small central ones. The predicate
    /// then has **no population** in the middle, "everything outside `R` is
    /// covered" is vacuously true, and the search stops satisfied — measured
    /// on a sphere cap as a 2.783 mm unmachined hole reported as clean. Every
    /// region triangle therefore gets **at least one** sample, on top of its
    /// area-proportional share, and [`SpiralReport::samples_placed`] reports
    /// the real number. Read
    /// [`SpiralReport::max_local_sample_spacing_mm`], not `√(area/N_S)`, to
    /// judge whether the sampling is fine enough.
    pub n_surface_samples: usize,
    /// `N_C` — samples per full turn, and **the shared angular lattice**.
    ///
    /// **[REPO]** Ring runs and bridges are both sampled on
    /// `a₀ + j·(2π/N_C)`. A separate bridge lattice would put bridge chords
    /// out of phase with ring chords in the band where `σ′(0) = 0` makes the
    /// bridge hug its ring, and the disk-domain self-intersection count would
    /// then report *sampling* artefacts as crossings.
    pub n_angular_samples: usize,
    /// **[REPO] Spacing safety factor on the ring search's LATERAL REACH.**
    ///
    /// The ring search reasons with `reach × this`; the tool centre is still
    /// offset by the true `K_c`, and the independent coverage audit still
    /// tests against the true `K_c`. `1.0` is **the paper's own criterion
    /// exactly** and is the default, so nothing changes silently.
    ///
    /// **Why it exists** (measured 2026-08-30 on the exact-affine flat disk).
    /// Eqs. 1–4 place ring `i+1` exactly `2·reach` inside ring `i`, because
    /// each ring is pushed in until it just reaches the outermost still-
    /// uncovered sample. The midline between two adjacent rings therefore sits
    /// **exactly on the coverage boundary** — which is the iso-scallop
    /// condition restated: at the optimal stepover the scallop crest is
    /// exactly at height `h`, i.e. exactly *on* `S^h`. The criterion has
    /// **zero margin by construction**, so it guarantees coverage of the
    /// search's own sample set and of nothing else. Any denser or offset test
    /// population — such as [`CoverageAudit`]'s mesh centroids — finds points
    /// on the wrong side of that knife edge.
    ///
    /// Setting this below 1 buys real margin: adjacent bands then **overlap**
    /// by `2·reach·(1 − factor)`, and the whole family of near-boundary
    /// ambiguities disappears. Values above 1 are clamped away: claiming more
    /// reach than the tool has is not a safety factor.
    ///
    /// # It derates the LATERAL REACH, not the coverage radius
    ///
    /// This distinction is not cosmetic and the first version of this
    /// parameter got it wrong. The lateral reach is
    /// `reach = √(K_c² − (K_c − h)²)`, which is a **steep** function of `K_c`
    /// near the top: `d(reach)/d(K_c) = K_c / reach`, which is 2.63 on the
    /// module's own flat fixture. Derating the *radius* by 5 % there cut the
    /// *reach* by **43 %** (0.760 → 0.433 mm) and collapsed the ring spacing
    /// to 0.997 mm against a 1.520 mm closed form. So the factor is applied
    /// where it means what its name says:
    ///
    /// ```text
    /// reach'          = factor · reach
    /// search radius   = √( reach'² + (K_c − h)² )        [= K_c at factor 1]
    /// spacing margin  = 2·(reach − reach')
    /// ```
    ///
    /// # Adequacy condition
    ///
    /// The margin is only useful if it exceeds the **radial gap in the sample
    /// set**. The search places ring `i+1` at `(largest sample radius not yet
    /// covered) − reach'`, so the step overshoots `2·reach'` by whatever the
    /// local radial sample gap is. Coverage therefore survives only while
    /// `2·(reach − reach') ≥ radial sample gap`. Compare
    /// [`SpiralReport::ring_spacing_margin_mm`] against
    /// [`SpiralReport::max_local_sample_spacing_mm`]; when the margin loses,
    /// the ring step quantises to the mesh's own feature pitch and the audit
    /// is what will notice.
    pub ring_spacing_safety: f64,
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
    /// Near-centre switch radius — `PAPER_NEAR_CENTRE_RADIUS`.
    pub near_centre_radius: f64,
    /// Near-centre bridge shift — `PAPER_NEAR_CENTRE_BRIDGE_SHIFT`.
    pub near_centre_bridge_shift: f64,
    /// Initial along-line shift after an outer bridge.
    ///
    /// The paper's value is [`PAPER_SECONDARY_LINE_SHIFT`] (`8π/5`); this
    /// defaults to **0.0** for the reason in the module header's
    /// **G-BRIDGE-BOOKKEEPING**. Set it to the paper's constant to reproduce
    /// the literal reading and watch [`SpiralReport::bridge_overhead_pct`].
    pub secondary_line_shift: f64,
    /// Bridge-repair shift increment — `PAPER_SHIFT_STEP`.
    pub shift_step: f64,
    /// **[REPO]** Cap on bridge-repair iterations.
    pub max_bridge_repair_steps: usize,
    /// Start-angle sweep step over `[0, 2π)` —
    /// [`PAPER_START_ANGLE_STEP`]. Coarsen it in tests; the paper's own
    /// description of the sweep is "trial and error".
    pub start_angle_step: f64,
    /// Cap on Gauss–Seidel sweeps for the flattening solve. Reaching it is
    /// [`SpiralRefusal::FlattenDidNotConverge`], never a silent under-solve.
    pub solver_max_sweeps: usize,
    /// Gauss–Seidel stopping criterion: the **maximum absolute change** in any
    /// disk coordinate over one sweep. Absolute, not relative, because the
    /// codomain is the unit disk.
    pub solver_tolerance: f64,
}

impl Default for SpiralParams {
    fn default() -> Self {
        Self {
            ball_radius_mm: 3.0,
            scallop_h_mm: 0.03,
            n_surface_samples: 20_000,
            n_angular_samples: 360,
            ring_spacing_safety: 1.0,
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
            solver_max_sweeps: 50_000,
            solver_tolerance: 1e-9,
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

    /// The tool's true lateral reach on flat ground:
    /// `√(K_c² − (K_c − h)²)`, i.e. half the flat iso-scallop stepover.
    #[must_use]
    pub fn lateral_reach_mm(&self) -> f64 {
        let v = (self.ball_radius_mm - self.scallop_h_mm).max(0.0);
        (self.ball_radius_mm * self.ball_radius_mm - v * v)
            .max(0.0)
            .sqrt()
    }

    /// The lateral reach the **ring search** reasons with:
    /// `lateral_reach_mm × ring_spacing_safety`.
    #[must_use]
    pub fn ring_search_lateral_reach_mm(&self) -> f64 {
        self.lateral_reach_mm() * self.ring_spacing_safety.clamp(1e-6, 1.0)
    }

    /// The coverage radius the **ring search** reasons with — the radius whose
    /// lateral reach is [`SpiralParams::ring_search_lateral_reach_mm`].
    /// Exactly `K_c` at factor 1, so the paper's criterion is reproduced bit
    /// for bit.
    ///
    /// Distinct from [`SpiralParams::ball_radius_mm`], which is what the tool
    /// centre is offset by and what [`CoverageAudit`] tests against. Only the
    /// planner's own criterion is derated; the geometry is not.
    #[must_use]
    pub fn ring_search_radius_mm(&self) -> f64 {
        let v = (self.ball_radius_mm - self.scallop_h_mm).max(0.0);
        let r = self.ring_search_lateral_reach_mm();
        (r * r + v * v).sqrt().min(self.ball_radius_mm)
    }

    /// How much overlap the safety factor buys between adjacent ring bands:
    /// `2·(reach − reach')`. Zero at factor 1 — the paper's criterion.
    #[must_use]
    pub fn ring_spacing_margin_mm(&self) -> f64 {
        2.0 * (self.lateral_reach_mm() - self.ring_search_lateral_reach_mm())
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
    /// The flattening solve hit [`SpiralParams::solver_max_sweeps`] before
    /// the sweep delta fell below [`SpiralParams::solver_tolerance`]. Carries
    /// what was actually measured, because an under-converged map can still
    /// fold and must not be used.
    FlattenDidNotConverge { max_delta: f64, sweeps: usize },
    /// The surface produced no `S^h` samples: the region's total area is
    /// degenerate.
    ///
    /// `n_surface_samples == 0` no longer reaches this arm — under the
    /// one-per-triangle floor a zero budget still yields one sample per
    /// triangle, which is the intended semantic. Since `build_region_mesh`
    /// already drops sliver triangles, this variant is close to unreachable in
    /// practice; it is kept because "close to" is not "is".
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
    /// Quasi-conformal dilatation `K = s_max / s_min` over this band — the
    /// anisotropy profile, alongside the area profile. `1.0` is conformal.
    pub dilatation_min: f64,
    /// See [`RadialDistortion::dilatation_min`].
    pub dilatation_median: f64,
    /// See [`RadialDistortion::dilatation_min`].
    pub dilatation_max: f64,
}

/// Variation of the map's **local radial scale around each placed ring**.
///
/// **[REPO]** This is the directly decisive number for spacing, and it is not
/// derivable from any per-triangle statistic. A ring is **one circle at one
/// disk radius**, and the Eqs. 1–4 binary search sizes it by its **worst
/// sector** — the single uncovered point that is hardest to reach. Every other
/// sector is then over-covered by however much the map's radial scale varies
/// *around* that circle. So the per-ring ratio below **is** the over-cover
/// factor the search is forced into on that ring: a ring whose radial scale
/// varies 3× cannot be spaced correctly anywhere except in its worst sector,
/// no matter how good the search is.
///
/// Area distortion cannot see this and neither can `K` on its own: a map can
/// preserve area while stretching radially and compressing tangentially, and a
/// map can have a uniform `K` while its radial scale still swings around a
/// given circle.
#[derive(Debug, Clone, Default)]
pub struct RingAnisotropy {
    /// Per placed ring, outermost first: `max / min` of the local radial scale
    /// sampled around that ring's disk circle. `1.0` means the ring can be
    /// spaced correctly everywhere at once.
    pub per_ring_radial_scale_ratio: Vec<f64>,
    /// Median of the above.
    pub median_ratio: f64,
    /// Worst of the above.
    pub worst_ratio: f64,
    /// Index of the worst ring in [`SpiralReport::ring_radii`].
    pub worst_ring: usize,
    /// Rings the ratio could be measured on.
    pub rings_measured: usize,
    /// Rings where too few probes landed to form a ratio (a degenerate radius,
    /// or probes falling outside the flattened polygon).
    pub rings_unmeasurable: usize,
    /// The inward probe step actually used, in disk units.
    pub probe_delta_disk: f64,
    /// Probes discarded because the disk query needed the radial pullback
    /// ladder, summed over all rings.
    ///
    /// A pulled-back probe does not measure what it claims to: the ladder
    /// moves the query point radially by a fraction comparable to the probe
    /// step itself, so the effective separation collapses and the "scale" it
    /// reports is noise — small, but far above any epsilon filter, and it
    /// would feed straight into a `max/min` this row exists to be trusted on.
    /// They are therefore excluded, and counted here so a ring whose worst
    /// sectors were all skipped reads as under-measured rather than quietly
    /// optimistic.
    pub probes_skipped_pulled_back: usize,
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

/// Min / median / max 3D distance over one population of sampled points.
#[derive(Debug, Clone, Default)]
pub struct DistanceStats {
    /// How many points the summary was actually measured on (strided to the
    /// module's `STALL_DISTANCE_SAMPLE_CAP`). Zero means the population was
    /// empty — which for some of these is the *healthy* reading.
    pub samples: usize,
    /// Smallest distance (mm).
    pub min_mm: f64,
    /// Median distance (mm).
    pub median_mm: f64,
    /// Largest distance (mm).
    pub max_mm: f64,
}

/// Why the Eqs. 1–4 ring search could not empty the uncovered set, in enough
/// detail to attribute the failure from the report alone.
///
/// **[REPO]** The paper reports no such thing: its Table 1 records two
/// sampling failure modes with no k value and no diagnosis.
///
/// Read it in this order:
///
/// 1. [`StallContext::uncovered_outside_last_ring`] — the **fold census**.
///    Every point outside the last placed ring's radius was, by that ring's
///    own feasibility check, supposed to be swept by it. A nonzero count means
///    the forward map (a sample's barycentric disk radius) and the inverse map
///    (where a ring at that radius actually lifts to) **disagree**, which is
///    what a non-injective flattening does. Under mean-value weights this
///    should be 0.
/// 2. [`StallContext::interior_radius_feasible`] — was *any* radius strictly
///    inside the previous ring ever feasible?
/// 3. [`StallContext::blockers`] — the points that actually stopped the
///    descent, measured against the ring at [`StallContext::search_lo`].
/// 4. [`StallContext::near_band`] vs [`StallContext::all_uncovered`] — the
///    distance distribution close to the frontier, versus over everything
///    still uncovered including the whole untouched interior.
#[derive(Debug, Clone)]
pub struct StallContext {
    /// Rings successfully placed before the stall.
    pub rings_placed: usize,
    /// Their disk radii, outermost first (duplicated here so the stall block
    /// is self-contained).
    pub ring_radii: Vec<f64>,
    /// Lower bound of the binary-search interval when the search gave up —
    /// the largest radius *proven infeasible*.
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
    /// **Fold census.** Still-uncovered points whose disk radius is *greater*
    /// than the last placed ring's radius — points the ring search already
    /// certified as swept. **Must be 0** for an injective map; see the type's
    /// own docs for why.
    pub uncovered_outside_last_ring: usize,
    /// Estimated width, in disk units, of the band one ring sweeps at the last
    /// placed radius — `2·K_c` converted through the local linear scale of the
    /// map (`√` of the area distortion in that radial bucket). This is what
    /// [`StallContext::near_band`] is restricted by, reported so the
    /// restriction is interpretable rather than magic.
    pub band_width_disk: f64,
    /// Which curve the distances were measured against.
    pub distance_reference: StallDistanceReference,
    /// The coverage radius the ring search reasoned with — the **derated**
    /// `ring_search_radius_mm`, not the true `K_c` — so every distance row
    /// below is compared against the bar the search actually applied.
    pub coverage_radius_mm: f64,
    /// Distances from **every** still-uncovered point to the reference curve.
    /// Dominated by the untouched disk interior, so a large median here is
    /// expected and means little on its own.
    pub all_uncovered: DistanceStats,
    /// Distances restricted to uncovered points within one estimated band
    /// width of the last placed radius — the points the stall is *about*.
    /// A median far above `2·K_c` here is the razor-thin-band reading; a
    /// median near `K_c` with a nonzero fold census is the fold reading.
    pub near_band: DistanceStats,
    /// Distances from the points that **blocked the descent** — uncovered
    /// points outside [`StallContext::search_lo`] that the ring at `search_lo`
    /// fails to sweep — to that ring's own centre curve. Empty when the
    /// search never proved any radius infeasible.
    pub blockers: DistanceStats,
    /// Disk radii of those same blockers, as min/median/max, so their position
    /// in the domain is visible alongside their distance. Values are disk
    /// units, not mm.
    pub blocker_disk_radius: DistanceStats,
}

/// An **independent** check that the emitted spiral actually machines the
/// region.
///
/// **[REPO]** Added 2026-08-30 after a sphere-cap fixture left a 2.783 mm
/// unmachined hole — **23.5 % of the region** — while
/// [`SpiralReport::uncovered_after_rings`] and
/// [`SpiralReport::uncovered_after_bridging`] both read 0 and the phase
/// falsifier passed.
///
/// Those two rows are computed over the `S^h` sample set, which is also what
/// the ring search's own feasibility predicate consumes. **A search cannot be
/// its own witness**: when the sampling has no population in a region, "every
/// uncovered point outside `R` is covered" is vacuously true and the search
/// stops satisfied. This audit therefore uses a population defined by the
/// **mesh** — every region triangle's centroid — and never touches the sample
/// set. It is the row that would have caught that hole in one line.
#[derive(Debug, Clone, Default)]
pub struct CoverageAudit {
    /// Triangle centroids tested — the whole region, by construction.
    pub centroids_tested: usize,
    /// Centroids whose `S^h` point the final spiral's centre curve does not
    /// sweep.
    pub uncovered_centroid_triangles: usize,
    /// Summed area (mm²) of those triangles: the estimated unmachined area.
    pub unmachined_area_mm2: f64,
    /// That area as a fraction of [`CoverageAudit::region_area_mm2`].
    pub unmachined_area_fraction: f64,
    /// 3D area (mm²) of the region.
    pub region_area_mm2: f64,
    /// Largest single unmachined triangle (mm²).
    pub largest_unmachined_triangle_area_mm2: f64,
    /// The true coverage radius the audit tested with (`K_c`, mm), so the
    /// distances below are self-interpreting.
    pub coverage_radius_mm: f64,
    /// How far the unmachined centroids are from the spiral's centre curve.
    /// **Read this against `coverage_radius_mm`**: microns above it is a
    /// zero-margin knife edge at a ring midline (see
    /// [`SpiralParams::ring_spacing_safety`]); a millimetre above it is a
    /// genuine hole.
    pub uncovered_distance: DistanceStats,
    /// **Disk radius** of the unmachined centroids, min/median/max — the
    /// spatial discriminator. Values clustered near 1 mean the rim, near 0 the
    /// centre, and a spread across the domain means thin bands at ring
    /// midlines. Units are disk radius, not mm; the `DistanceStats` field
    /// names say mm because the type is shared.
    pub uncovered_disk_radius: DistanceStats,
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
    /// Interior (non-prescribed) vertices — the size of the solved system.
    pub flatten_interior_vertices: usize,
    /// Gauss–Seidel sweeps taken. Both disk coordinates are swept together,
    /// so this is one number, not two.
    pub flatten_solver_sweeps: usize,
    /// Maximum absolute coordinate change on the final sweep.
    pub flatten_solver_delta: f64,
    /// Smallest mean-value weight assembled. **Must be > 0**: positivity is
    /// the precondition of the Tutte embedding guarantee, and this measures it
    /// rather than assuming it.
    pub mean_value_weight_min: f64,
    /// Largest mean-value weight assembled. A very large value means a nearly
    /// degenerate corner angle, which is legal but worth seeing.
    pub mean_value_weight_max: f64,
    /// Mean-value weights that came out non-positive. **Must be 0.** A nonzero
    /// count means the embedding guarantee does not hold and any
    /// [`SpiralReport::flipped_triangles`] below is unsurprising rather than a
    /// bug in the solver.
    pub mean_value_weight_nonpositive: usize,
    /// Triangles whose flat signed area has the opposite sign to the map's
    /// overall orientation — i.e. **folds**.
    ///
    /// Under mean-value weights this is the **tripwire on a structural
    /// invariant**, not a quality metric: Tutte's theorem makes a valid
    /// embedding certain for a manifold disk mapped onto a convex boundary
    /// with positive weights, so a nonzero count means a bug (or an
    /// under-converged solve), never bad luck. It kept its name and meaning
    /// from the cotangent version on purpose, so the two are comparable: that
    /// version measured 298 of 9107 on `terrain_small`.
    pub flipped_triangles: usize,
    /// Disk radius of each flipped triangle's flattened centroid, ascending —
    /// the fold-zone census. Empty is the expected reading; when it is not
    /// empty this says *where* in the disk the invariant broke, which is what
    /// a ring-stall postmortem needs.
    pub flipped_triangle_disk_radii: Vec<f64>,
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
    /// Area **and** dilatation distortion bucketed by disk radius,
    /// `RADIAL_BUCKETS` (5) entries outward from the centre. Empty means the
    /// flattening never ran.
    pub area_distortion_by_disk_radius: Vec<RadialDistortion>,
    /// Quasi-conformal **dilatation** `K = s_max / s_min` of the per-triangle
    /// 3D→flat Jacobian, minimum over the region.
    ///
    /// `K ≥ 1` always, and `K = 1` exactly iff the map is a local similarity
    /// there — i.e. conformal. Unlike area distortion, `K` is **scale
    /// invariant**, so it is comparable across arms, fixtures and units. This
    /// is the row that decides whether substituting mean-value weights for a
    /// conformal flattening costs *spacing* as well as buying fold-freeness:
    /// area distortion cannot see radial-vs-tangential anisotropy, and it is
    /// the anisotropy that turns a well-chosen ring radius into bad spacing.
    pub dilatation_min: f64,
    /// See [`SpiralReport::dilatation_min`].
    pub dilatation_median: f64,
    /// 90th percentile of `K` — the tail matters more than the median here,
    /// because one badly anisotropic sector is enough to mis-size a ring.
    pub dilatation_p90: f64,
    /// See [`SpiralReport::dilatation_min`].
    pub dilatation_max: f64,
    /// Triangles whose Jacobian was too degenerate to take singular values
    /// from. `None` of these are expected once folds are impossible.
    pub dilatation_unmeasurable: usize,

    // --- sampling + rings ------------------------------------------------
    /// `N_S` as requested — a **floor**, not a cap. See
    /// [`SpiralParams::n_surface_samples`].
    pub samples_requested: usize,
    /// `S^h` samples actually placed. At least
    /// [`SpiralReport::samples_requested`], and at least one per region
    /// triangle.
    pub samples_placed: usize,
    /// Region triangles that received no sample. **MUST be 0** — a tripwire
    /// like [`SpiralReport::flipped_triangles`], because a triangle with no
    /// sample is invisible to the coverage predicate and makes it vacuously
    /// satisfiable there. Zero by construction under the one-per-triangle
    /// floor; measured anyway, because the whole point is that this class of
    /// defect reads as success.
    pub triangles_without_samples: usize,
    /// Largest region triangle that received no sample (mm²). `0.0` when none
    /// did, which is the expected reading.
    pub largest_unsampled_triangle_area_mm2: f64,
    /// **The adequacy number.** `max over triangles of √(area_t / quota_t)` —
    /// the coarsest local sample spacing anywhere in the region, in mm.
    ///
    /// This replaces the naive `√(region area / N_S)`, which assumes uniform
    /// placement and is therefore blind to apportionment starvation: it
    /// happily reported 0.076 mm implied spacing on a mesh where 46 % of the
    /// triangles held no sample at all. Compare this against **half the
    /// stepover**; above that, the coverage predicate is under-resolved
    /// wherever the maximum is attained, regardless of how large `N_S` is.
    pub max_local_sample_spacing_mm: f64,
    /// Independent, mesh-defined coverage audit of the emitted spiral. `None`
    /// means no spiral was produced, so it was never measured.
    pub coverage_audit: Option<CoverageAudit>,
    /// The **lateral** reach the ring search reasons with (mm), and the true
    /// tool reach it is derated from — the pair the spacing is actually set
    /// by. Equal under the paper's own criterion.
    pub ring_search_lateral_reach_mm: f64,
    /// The tool's true lateral reach (mm), i.e. half the flat stepover.
    pub lateral_reach_mm: f64,
    /// `2·(reach − reach')` — the band overlap the safety factor buys.
    /// **Compare against [`SpiralReport::max_local_sample_spacing_mm`]**: when
    /// the margin is the smaller of the two, the ring step quantises to the
    /// mesh's feature pitch instead of following the scallop rule.
    pub ring_spacing_margin_mm: f64,
    /// The coverage radius the ring search reasons with. Equals `K_c` under
    /// the paper's own criterion.
    /// Recorded as soon as the region is built, so it is meaningful on every
    /// refusal path too; only an `EmptyRegion` refusal leaves it at `0.0`.
    pub ring_search_radius_mm: f64,
    /// Rings produced by the Eqs. 1–4 search.
    pub ring_count: usize,
    /// Disk radius of each ring, outermost first.
    pub ring_radii: Vec<f64>,
    /// 3D length (mm) of each ring's contact polyline, outermost first.
    pub ring_lengths_mm: Vec<f64>,
    /// `S^h` points first covered by each ring — the paper's milling band
    /// `BP_i`.
    pub ring_newly_covered: Vec<usize>,
    /// Smallest band over the placed rings.
    pub band_size_min: usize,
    /// Median band size.
    pub band_size_median: usize,
    /// Largest band size. A wide min–max spread is itself a spacing defect
    /// signature: the rings are not sharing the surface evenly.
    pub band_size_max: usize,
    /// Rings whose band is at most `DEGENERATE_BAND_MAX` (2) samples — a pass
    /// that costs full price in motion and removes essentially nothing.
    /// Counted explicitly because it is a defect signature, not a curiosity.
    pub degenerate_rings: usize,
    /// Radial-scale variation around each placed ring. `None` means the ring
    /// search never placed a ring, so it was never measured.
    pub ring_anisotropy: Option<RingAnisotropy>,
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
    ///
    /// **This is not an independent witness.** It is computed over the same
    /// `S^h` population the search's own feasibility predicate consumes, so
    /// wherever that population is empty it reads 0 *because there was nothing
    /// to disagree with*. Read it together with
    /// [`SpiralReport::triangles_without_samples`] and
    /// [`SpiralReport::coverage_audit`], which are the rows that cannot be
    /// fooled the same way.
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
    ///
    /// Shares [`SpiralReport::uncovered_after_rings`]' blind spot exactly: its
    /// population is the per-ring bands, which are subsets of the same `S^h`
    /// sample set. It answers "did bridging lose anything the rings had", not
    /// "is the region machined" — [`SpiralReport::coverage_audit`] answers the
    /// second.
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
    // Recorded up front so it is never 0.0-on-a-refusal, which a reader would
    // misread as "searched with zero radius".
    report.ring_search_radius_mm = params.ring_search_radius_mm();
    report.ring_search_lateral_reach_mm = params.ring_search_lateral_reach_mm();
    report.lateral_reach_mm = params.lateral_reach_mm();
    report.ring_spacing_margin_mm = params.ring_spacing_margin_mm();

    // 1. Topology: one boundary loop, disk Euler characteristic.
    let topo = region_topology(&region)?;
    report.region_edges = topo.edge_count;
    report.euler_characteristic = topo.euler;
    report.boundary_loop_vertices = topo.loop_vertices.len();
    report.boundary_loop_length_mm = topo.loop_length_mm;

    // 2. Disk map: mean-value weights, arc-length boundary correspondence.
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
    let samples = sample_iso_scallop(&region, &flat, &vertex_normals, params, report);
    if samples.is_empty() {
        return Err(SpiralRefusal::NoSurfaceSamples);
    }

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
    // 8. The independent witness. The ring search cannot be its own — see
    //    `CoverageAudit`.
    audit_coverage(
        &region,
        &vertex_normals,
        &flat,
        &spiral_meta.centre,
        params,
        report,
    );
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
// Disk map — [SOURCE-FLOATER] weights, [REPO] boundary and solver
// ---------------------------------------------------------------------------

/// Per-local-vertex position in the unit disk.
struct Flattening {
    uv: Vec<(f64, f64)>,
}

/// The assembled mean-value system: **unsymmetric** directed neighbour lists
/// plus each row's weight sum.
///
/// **[SOURCE-FLOATER]** Floater, *Mean value coordinates*, CAGD 20(1):19–27,
/// 2003. Deliberately a separate assembly from
/// [`crate::finish::direction_field`]'s cotangent one: they are different matrices
/// with different guarantees, and sharing an assembly would invite exactly the
/// silent symmetrisation the module header rules out.
struct MeanValueWeights {
    /// `(neighbour, w_ij)` per vertex, ascending by neighbour for determinism.
    /// **Directed**: `w_ij` lives on row `i` only, and `w_ji ≠ w_ij`.
    nbr: Vec<Vec<(usize, f64)>>,
    /// `Σ_j w_ij` per row.
    row_sum: Vec<f64>,
    min_weight: f64,
    max_weight: f64,
    nonpositive: usize,
}

/// Assemble `w_ij = (tan(δ_ij/2) + tan(γ_ij/2)) / ‖v_i − v_j‖`.
///
/// **[SOURCE-FLOATER Eq. for mean value coordinates]** `δ_ij` and `γ_ij` are
/// the two angles **at `i`** flanking edge `ij`. Rather than building an
/// ordered one-ring per vertex, this walks triangles: in triangle `(i, j, k)`
/// the angle `θ` at `i` is one of the two flanking angles for edge `ij` *and*
/// for edge `ik`, so it contributes `tan(θ/2)/‖e‖` to both. Summing over
/// incident triangles therefore reproduces the two-angle formula exactly — and
/// for a boundary vertex it sums the one available angle, which is harmless
/// because boundary rows are prescribed and never solved.
///
/// Every interior angle is in `(0, π)`, so every `tan(θ/2)` is **strictly
/// positive** — the Tutte precondition. It is measured, not assumed:
/// [`MeanValueWeights::nonpositive`] counts any violation.
fn mean_value_weights(region: &RegionMesh) -> MeanValueWeights {
    let nv = region.verts.len();
    let mut acc: HashMap<(usize, usize), f64> = HashMap::new();
    for t in 0..region.tris.len() {
        if region.area(t) < MIN_TRIANGLE_AREA_MM2 {
            continue;
        }
        let c = region.corners(t);
        for m in 0..3 {
            let (i, j, k) = (
                c.get(m).copied().unwrap_or_default(),
                c.get((m + 1) % 3).copied().unwrap_or_default(),
                c.get((m + 2) % 3).copied().unwrap_or_default(),
            );
            let (pi, pj, pk) = (region.point(i), region.point(j), region.point(k));
            let (eij, eik) = (pj - pi, pk - pi);
            let (lij, lik) = (eij.norm(), eik.norm());
            if lij <= EPS_VEC || lik <= EPS_VEC {
                continue;
            }
            let theta = (eij.dot(&eik) / (lij * lik))
                .clamp(-1.0, 1.0)
                .acos()
                .clamp(MIN_TRIANGLE_ANGLE, PI - MIN_TRIANGLE_ANGLE);
            let half = (0.5 * theta).tan();
            *acc.entry((i, j)).or_insert(0.0) += half / lij;
            *acc.entry((i, k)).or_insert(0.0) += half / lik;
        }
    }

    let mut nbr: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nv];
    let mut row_sum = vec![0.0_f64; nv];
    let mut min_weight = f64::INFINITY;
    let mut max_weight = 0.0_f64;
    let mut nonpositive = 0usize;
    let mut edges: Vec<((usize, usize), f64)> = acc.into_iter().collect();
    edges.sort_by_key(|a| a.0);
    for ((i, j), w) in edges {
        if i >= nv || j >= nv || i == j {
            continue;
        }
        if w.is_nan() || w <= 0.0 {
            nonpositive += 1;
        }
        min_weight = min_weight.min(w);
        max_weight = max_weight.max(w);
        if let (Some(list), Some(sum)) = (nbr.get_mut(i), row_sum.get_mut(i)) {
            list.push((j, w));
            *sum += w;
        }
    }
    if !min_weight.is_finite() {
        min_weight = 0.0;
    }
    MeanValueWeights {
        nbr,
        row_sum,
        min_weight,
        max_weight,
        nonpositive,
    }
}

/// Flatten the region onto the unit disk.
///
/// **[REPO] Boundary correspondence is plain cumulative arc length.** Walking
/// the single boundary loop, vertex `k` at cumulative 3D arc length `s_k` on a
/// perimeter of `L` goes to `(cos 2πs_k/L, sin 2πs_k/L)`. This is *not*
/// Shen 2024's Eq. A-14: that is a corner-graded B-spline parameterisation
/// whose purpose is Nyström convergence for the boundary-integral slit map,
/// which this module does not solve. Phase 1 needs a correspondence, not a
/// quadrature. Because the images are distinct points in cyclic order on a
/// circle, the boundary polygon is **convex** and the boundary map is a
/// homeomorphism onto it — the other half of the Tutte precondition.
///
/// **Interior vertices** are solved by Gauss–Seidel sweeps over the
/// unsymmetric mean-value system, both coordinates together, boundary values
/// held fixed:
///
/// ```text
/// u_i ← ( Σ_j w_ij u_j ) / ( Σ_j w_ij )
/// ```
///
/// See the module header for why this replaced Phase F1's cotangent + CG.
#[allow(clippy::result_large_err)]
fn flatten_to_disk(
    region: &RegionMesh,
    topo: &Topology,
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> Result<Flattening, SpiralRefusal> {
    let nv = region.verts.len();
    let weights = mean_value_weights(region);
    report.mean_value_weight_min = weights.min_weight;
    report.mean_value_weight_max = weights.max_weight;
    report.mean_value_weight_nonpositive = weights.nonpositive;

    // Prescribed boundary values by arc length.
    let mut uv = vec![(0.0_f64, 0.0_f64); nv];
    let mut acc = 0.0_f64;
    for (i, &v) in topo.loop_vertices.iter().enumerate() {
        let theta = TAU * acc / topo.loop_length_mm;
        if let Some(slot) = uv.get_mut(v) {
            *slot = (theta.cos(), theta.sin());
        }
        let w = topo
            .loop_vertices
            .get((i + 1) % topo.loop_vertices.len())
            .copied()
            .unwrap_or(v);
        acc += (region.point(w) - region.point(v)).norm();
    }

    let pinned: &[bool] = &topo.on_boundary;
    report.flatten_interior_vertices = pinned.iter().filter(|&&b| !b).count();

    let mut sweeps = 0usize;
    let mut delta = 0.0_f64;
    while sweeps < params.solver_max_sweeps {
        sweeps += 1;
        delta = 0.0;
        for i in 0..nv {
            if pinned.get(i).copied().unwrap_or(false) {
                continue;
            }
            let sum = weights.row_sum.get(i).copied().unwrap_or(0.0);
            if sum <= EPS_VEC {
                continue;
            }
            let Some(list) = weights.nbr.get(i) else {
                continue;
            };
            let mut su = 0.0_f64;
            let mut sv = 0.0_f64;
            for &(j, w) in list {
                let (uj, vj) = uv.get(j).copied().unwrap_or((0.0, 0.0));
                su += w * uj;
                sv += w * vj;
            }
            let next = (su / sum, sv / sum);
            if let Some(slot) = uv.get_mut(i) {
                delta = delta
                    .max((next.0 - slot.0).abs())
                    .max((next.1 - slot.1).abs());
                *slot = next;
            }
        }
        if delta <= params.solver_tolerance {
            break;
        }
    }
    report.flatten_solver_sweeps = sweeps;
    report.flatten_solver_delta = delta;
    if delta > params.solver_tolerance {
        return Err(SpiralRefusal::FlattenDidNotConverge {
            max_delta: delta,
            sweeps,
        });
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
    let mut centroid_r: Vec<f64> = Vec::with_capacity(region.tris.len());
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
        let cx = (a.0 + b.0 + d.0) / 3.0;
        let cy = (a.1 + b.1 + d.1) / 3.0;
        centroid_r.push(cx.hypot(cy).clamp(0.0, 1.0));
    }
    let orientation = if total < 0.0 { -1.0_f64 } else { 1.0_f64 };
    report.orientation_sign = orientation;
    // Fold census: the count, and where in the disk each fold sits. Under
    // mean-value weights both are expected to be empty — this is the tripwire
    // on the Tutte guarantee, not a quality metric.
    let mut fold_radii: Vec<f64> = signed
        .iter()
        .zip(centroid_r.iter())
        .filter(|&(&s, _)| s * orientation <= 0.0)
        .map(|(_, &r)| r)
        .collect();
    fold_radii.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.flipped_triangles = fold_radii.len();
    report.flipped_triangle_disk_radii = fold_radii;

    let mut ratios: Vec<f64> = Vec::with_capacity(region.tris.len());
    let mut angle_err: Vec<f64> = Vec::with_capacity(region.tris.len() * 3);
    let mut dilatations: Vec<f64> = Vec::with_capacity(region.tris.len());
    let mut radial: Vec<Vec<f64>> = vec![Vec::new(); RADIAL_BUCKETS];
    let mut radial_k: Vec<Vec<f64>> = vec![Vec::new(); RADIAL_BUCKETS];
    let mut unmeasurable = 0usize;
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
        // Quasi-conformal dilatation: the anisotropy area distortion cannot see.
        let k = triangle_dilatation(&p3, &p2);
        match k {
            Some(k) => dilatations.push(k),
            None => unmeasurable += 1,
        }

        // Radial buckets, keyed on the disk radius of the flattened centroid.
        let rad = centroid_r.get(t).copied().unwrap_or(0.0);
        let b = ((rad * RADIAL_BUCKETS as f64).floor() as usize).min(RADIAL_BUCKETS - 1);
        if let (Some(r), Some(slot)) = (ratio, radial.get_mut(b)) {
            slot.push(r);
        }
        if let (Some(k), Some(slot)) = (k, radial_k.get_mut(b)) {
            slot.push(k);
        }
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.area_distortion_min = ratios.first().copied().unwrap_or(0.0);
    report.area_distortion_max = ratios.last().copied().unwrap_or(0.0);
    report.area_distortion_median = median_sorted(&ratios);
    angle_err.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.angle_distortion_max_deg = angle_err.last().copied().unwrap_or(0.0);
    report.angle_distortion_median_deg = median_sorted(&angle_err);

    dilatations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.dilatation_min = dilatations.first().copied().unwrap_or(0.0);
    report.dilatation_median = median_sorted(&dilatations);
    report.dilatation_p90 = percentile_sorted(&dilatations, 0.90);
    report.dilatation_max = dilatations.last().copied().unwrap_or(0.0);
    report.dilatation_unmeasurable = unmeasurable;

    let width = 1.0 / RADIAL_BUCKETS as f64;
    report.area_distortion_by_disk_radius = radial
        .into_iter()
        .zip(radial_k)
        .enumerate()
        .map(|(b, (mut vals, mut ks))| {
            vals.sort_by(|a, c| a.partial_cmp(c).unwrap_or(std::cmp::Ordering::Equal));
            ks.sort_by(|a, c| a.partial_cmp(c).unwrap_or(std::cmp::Ordering::Equal));
            RadialDistortion {
                r_lo: b as f64 * width,
                r_hi: (b + 1) as f64 * width,
                triangles: vals.len(),
                area_distortion_min: vals.first().copied().unwrap_or(0.0),
                area_distortion_median: median_sorted(&vals),
                area_distortion_max: vals.last().copied().unwrap_or(0.0),
                dilatation_min: ks.first().copied().unwrap_or(0.0),
                dilatation_median: median_sorted(&ks),
                dilatation_max: ks.last().copied().unwrap_or(0.0),
            }
        })
        .collect();
}

/// Measure the map's **local radial scale variation around each placed ring**.
///
/// **[REPO]** For each ring radius `R`, the disk circle is sampled on the same
/// angular lattice the rings themselves use, and at each sample the local
/// radial scale is taken as an **inward** finite difference of the lifted
/// contact point:
///
/// ```text
/// scale(φ) = ‖ lift(R, φ) − lift(R − d, φ) ‖ / d      [mm per unit disk radius]
/// ```
///
/// Inward, not centred, so the *inner* probe cannot leave the flattened
/// polygon. The **outer** probe sits at `R` itself, which can exceed the
/// polygon's inradius on a coarse boundary, so any probe that needed the
/// radial pullback ladder is **excluded and counted** in
/// [`RingAnisotropy::probes_skipped_pulled_back`] rather than trusted: the
/// ladder's first rung moves the point by about the probe step, which would
/// collapse the difference and fabricate a ratio. `d` is
/// `RADIAL_PROBE_DELTA`, shrunk to `R/2` near the centre;
/// it is far smaller than any flat triangle, and the map is affine inside a
/// triangle, so the difference is **exact** rather than approximate there.
///
/// The reported per-ring `max/min` is the over-cover factor the worst-sector
/// rule forces onto that ring — see [`RingAnisotropy`].
fn measure_ring_anisotropy(
    locator: &FlatLocator,
    region: &RegionMesh,
    normals: &[V3],
    ring_radii: &[f64],
    params: &SpiralParams,
) -> Option<RingAnisotropy> {
    if ring_radii.is_empty() {
        return None;
    }
    let n = params.n_angular_samples.max(3);
    let mut scratch = QueryScratch::new();
    let mut hits: Vec<usize> = Vec::new();
    let mut ratios: Vec<f64> = Vec::with_capacity(ring_radii.len());
    let mut unmeasurable = 0usize;
    let mut skipped_pulled = 0usize;

    for &r in ring_radii {
        let d = RADIAL_PROBE_DELTA.min(0.5 * r);
        if d <= 0.0 || !d.is_finite() {
            unmeasurable += 1;
            ratios.push(f64::NAN);
            continue;
        }
        let mut lo = f64::INFINITY;
        let mut hi = 0.0_f64;
        let mut seen = 0usize;
        for j in 0..n {
            let a = TAU * (j as f64) / (n as f64);
            let (ca, sa) = (a.cos(), a.sin());
            let Some((outer, outer_pulled)) =
                locator.locate(r * ca, r * sa, &mut scratch, &mut hits)
            else {
                continue;
            };
            let Some((inner, inner_pulled)) =
                locator.locate((r - d) * ca, (r - d) * sa, &mut scratch, &mut hits)
            else {
                continue;
            };
            // A pulled-back probe measures the ladder, not the map.
            if outer_pulled || inner_pulled {
                skipped_pulled += 1;
                continue;
            }
            let (po, _) = lift(region, normals, &outer);
            let (pi, _) = lift(region, normals, &inner);
            let scale = (po - pi).norm() / d;
            if !scale.is_finite() || scale <= EPS_VEC {
                continue;
            }
            lo = lo.min(scale);
            hi = hi.max(scale);
            seen += 1;
        }
        if seen < 2 || !lo.is_finite() || lo <= EPS_VEC {
            unmeasurable += 1;
            ratios.push(f64::NAN);
        } else {
            ratios.push(hi / lo);
        }
    }

    let mut finite: Vec<f64> = ratios.iter().copied().filter(|v| v.is_finite()).collect();
    finite.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let worst = finite.last().copied().unwrap_or(0.0);
    let worst_ring = ratios
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);
    Some(RingAnisotropy {
        per_ring_radial_scale_ratio: ratios,
        median_ratio: median_sorted(&finite),
        worst_ratio: worst,
        worst_ring,
        rings_measured: finite.len(),
        rings_unmeasurable: unmeasurable,
        probe_delta_disk: RADIAL_PROBE_DELTA,
        probes_skipped_pulled_back: skipped_pulled,
    })
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

/// Nearest-rank percentile of an already-sorted slice, `q ∈ [0, 1]`.
fn percentile_sorted(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let idx = (q.clamp(0.0, 1.0) * ((v.len() - 1) as f64)).round() as usize;
    v.get(idx.min(v.len() - 1)).copied().unwrap_or(0.0)
}

/// Quasi-conformal dilatation `K = s_max / s_min` of one triangle's 3D→flat
/// affine map.
///
/// **[REPO]** The 3D triangle is expressed in **its own orthonormal frame**
/// (`u` along the first edge, the in-plane perpendicular as the second axis),
/// which is what makes the comparison a genuine 2×2 Jacobian rather than a
/// projection artefact. With `a1 − a0 = (L, 0)` and `a2 − a0 = (x2, y2)` in
/// that frame, `A⁻¹` is closed-form, so `J = Q·A⁻¹` needs no general inverse.
/// Singular values come from the standard 2×2 form
/// `s_max = q + r`, `s_min = |q − r|` with `q = ‖(E, H)‖`, `r = ‖(F, G)‖`.
///
/// Returns `None` when the triangle or its image is too degenerate to take
/// singular values from. `K ≥ 1` by construction, and `K = 1` exactly when the
/// map is a local similarity — the property a conformal map has everywhere and
/// the one that makes concentric circles a good level-set family.
fn triangle_dilatation(p3: &[P3; 3], p2: &[(f64, f64); 3]) -> Option<f64> {
    let (a, b, c) = (p3.first()?, p3.get(1)?, p3.get(2)?);
    let e1 = b - a;
    let len = e1.norm();
    if len <= EPS_VEC {
        return None;
    }
    let u = e1 / len;
    let w = c - a;
    let x2 = w.dot(&u);
    let y2 = (w - u * x2).norm();
    if y2 <= EPS_VEC {
        return None;
    }

    let (q0, q1, q2) = (p2.first()?, p2.get(1)?, p2.get(2)?);
    let u1 = (q1.0 - q0.0, q1.1 - q0.1);
    let u2 = (q2.0 - q0.0, q2.1 - q0.1);
    let j00 = u1.0 / len;
    let j10 = u1.1 / len;
    let j01 = (u2.0 * len - u1.0 * x2) / (len * y2);
    let j11 = (u2.1 * len - u1.1 * x2) / (len * y2);

    let e = 0.5 * (j00 + j11);
    let f = 0.5 * (j00 - j11);
    let g = 0.5 * (j10 + j01);
    let h = 0.5 * (j10 - j01);
    let qq = e.hypot(h);
    let rr = f.hypot(g);
    let s_max = qq + rr;
    let s_min = (qq - rr).abs();
    if !s_max.is_finite() || s_min <= EPS_VEC {
        return None;
    }
    Some(s_max / s_min)
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
/// convention `crest_lines` and [`crate::finish::direction_field`] use, and a stated
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

/// Sample the region into `S^h`: at least `N_S` points **and at least one per
/// triangle**, each offset `+h` along its interpolated normal.
///
/// **[REPO]** The paper states no sampling rule at all (**G-SAMPLING**). The
/// per-triangle quota here is `max(1, area-proportional share)`, apportioned
/// by largest remainder for determinism.
///
/// **Why the `max(1, …)` is load-bearing.** Without it — the form this
/// function shipped with until 2026-08-30 — `N_S` is a hard cap apportioned by
/// area, so once `N_S` drops below the triangle count the average share falls
/// under 1, most triangles floor to zero, and the largest-remainder top-up
/// goes to the *largest* remainders, i.e. the biggest triangles. On a polar
/// sphere-cap mesh whose central facets are ~55× smaller than its rim facets,
/// that starves precisely the centre. The coverage predicate then has no
/// population there, `∀ uncovered outside R: covered` is **vacuously true**,
/// and the ring search terminates satisfied — measured as a 2.783 mm
/// unmachined hole, 23.5 % of the region, under a report reading
/// `uncovered_after_rings = 0`. The floor makes the domain, not the budget,
/// the thing that is guaranteed; `N_S` becomes a floor and
/// [`SpiralReport::samples_placed`] reports what was really used.
fn sample_iso_scallop(
    region: &RegionMesh,
    flat: &Flattening,
    normals: &[V3],
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> Vec<Sample> {
    report.samples_requested = params.n_surface_samples;
    let total_area: f64 = (0..region.tris.len()).map(|t| region.area(t)).sum();
    if total_area <= MIN_TRIANGLE_AREA_MM2 || region.tris.is_empty() {
        return Vec::new();
    }
    let want = params.n_surface_samples as f64;
    // Largest-remainder apportionment over a floor of one per triangle.
    let mut quota: Vec<usize> = Vec::with_capacity(region.tris.len());
    let mut rema: Vec<(f64, usize)> = Vec::with_capacity(region.tris.len());
    let mut assigned = 0usize;
    for t in 0..region.tris.len() {
        let exact = want * region.area(t) / total_area;
        let base = (exact.floor().max(0.0) as usize).max(1);
        quota.push(base);
        assigned += base;
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

    // Tripwires on the failure class this function exists to make impossible,
    // plus the adequacy number that is aware of apportionment.
    let mut starved = 0usize;
    let mut largest_starved = 0.0_f64;
    let mut worst_spacing = 0.0_f64;
    for (t, &q) in quota.iter().enumerate() {
        let area = region.area(t);
        if q == 0 {
            starved += 1;
            largest_starved = largest_starved.max(area);
            continue;
        }
        worst_spacing = worst_spacing.max((area / q as f64).sqrt());
    }
    report.triangles_without_samples = starved;
    report.largest_unsampled_triangle_area_mm2 = largest_starved;
    report.max_local_sample_spacing_mm = worst_spacing;

    let mut out: Vec<Sample> = Vec::with_capacity(assigned.max(params.n_surface_samples));
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
    report.samples_placed = out.len();
    out
}

/// **Independent** coverage audit of the emitted spiral — see
/// [`CoverageAudit`] for why the ring search's own numbers cannot serve.
///
/// **[REPO]** Population is every region triangle's centroid, lifted to `S^h`
/// the same way a sample is (`+h` along the averaged vertex normal), and
/// tested against the **final spiral's** tool-centre curve. Nothing here
/// reads the `S^h` sample set, `N_S`, or any ring.
fn audit_coverage(
    region: &RegionMesh,
    normals: &[V3],
    flat: &Flattening,
    spiral_centre: &[P3],
    params: &SpiralParams,
    report: &mut SpiralReport,
) {
    // The audit tests the TRUE tool, never the search's derated criterion.
    let radius = params.ball_radius_mm;
    let curve = CentreCurve::new(spiral_centre, radius);
    let mut uncovered_pts: Vec<P3> = Vec::new();
    let mut uncovered_disk_r: Vec<f64> = Vec::new();
    let mut uncovered_tris = 0usize;
    let mut unmachined = 0.0_f64;
    let mut largest = 0.0_f64;
    let mut region_area = 0.0_f64;
    let mut tested = 0usize;

    for t in 0..region.tris.len() {
        let area = region.area(t);
        region_area += area;
        let c = region.corners(t);
        let mut p = V3::zeros();
        let mut n = V3::zeros();
        let mut fx = 0.0_f64;
        let mut fy = 0.0_f64;
        for &v in c.iter() {
            p += region.point(v).coords / 3.0;
            n += normals.get(v).copied().unwrap_or_else(V3::zeros) / 3.0;
            let f = flat_of(flat, v);
            fx += f.0 / 3.0;
            fy += f.1 / 3.0;
        }
        let len = n.norm();
        let n = if len > EPS_VEC {
            n / len
        } else {
            V3::new(0.0, 0.0, 1.0)
        };
        let at = P3::from(p + n * params.scallop_h_mm);
        tested += 1;
        if curve.covers(at, radius) {
            continue;
        }
        uncovered_tris += 1;
        unmachined += area;
        largest = largest.max(area);
        uncovered_pts.push(at);
        uncovered_disk_r.push(fx.hypot(fy));
    }

    // Distances only for the uncovered subset, strided: grazing at a boundary
    // and a hole in the middle look identical in an area figure alone.
    let stride = uncovered_pts
        .len()
        .div_ceil(STALL_DISTANCE_SAMPLE_CAP)
        .max(1);
    let d: Vec<f64> = uncovered_pts
        .iter()
        .step_by(stride)
        .map(|&q| distance_to_polyline_mm(q, spiral_centre))
        .filter(|v| v.is_finite())
        .collect();

    report.coverage_audit = Some(CoverageAudit {
        centroids_tested: tested,
        uncovered_centroid_triangles: uncovered_tris,
        unmachined_area_mm2: unmachined,
        unmachined_area_fraction: if region_area > EPS_VEC {
            unmachined / region_area
        } else {
            0.0
        },
        region_area_mm2: region_area,
        largest_unmachined_triangle_area_mm2: largest,
        coverage_radius_mm: radius,
        uncovered_distance: summarise(&d),
        uncovered_disk_radius: summarise(&uncovered_disk_r),
    });
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

/// Min / median / max over a stride-sampled subset of `values`.
fn summarise(values: &[f64]) -> DistanceStats {
    if values.is_empty() {
        return DistanceStats::default();
    }
    let stride = values.len().div_ceil(STALL_DISTANCE_SAMPLE_CAP).max(1);
    let mut d: Vec<f64> = values
        .iter()
        .step_by(stride)
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    DistanceStats {
        samples: d.len(),
        min_mm: d.first().copied().unwrap_or(0.0),
        median_mm: median_sorted(&d),
        max_mm: d.last().copied().unwrap_or(0.0),
    }
}

/// Distances from a set of samples to a reference polyline.
fn distances_to(uncovered: &[usize], samples: &[Sample], reference: &[P3]) -> Vec<f64> {
    if reference.len() < 2 {
        return Vec::new();
    }
    uncovered
        .iter()
        .filter_map(|&s| samples.get(s))
        .map(|s| distance_to_polyline_mm(s.at, reference))
        .collect()
}

/// What blocked the descent, measured at the largest radius proven infeasible.
struct BlockerCensus {
    distances: Vec<f64>,
    disk_radii: Vec<f64>,
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
///
/// **[REPO]** The band-width estimate that [`StallContext::near_band`] is
/// restricted by converts the 3D coverage diameter `2·K_c` into disk units
/// through the map's own local linear scale. Area distortion is
/// `flat area / 3D area` (1/mm²), so its square root is disk-units-per-mm;
/// the radial bucket containing the last placed radius supplies it, with the
/// global median as the fallback when that bucket is empty.
#[allow(clippy::too_many_arguments)]
fn publish_ring_rows(
    rings: &[Ring],
    uncovered: &[usize],
    samples: &[Sample],
    last_centre: &[P3],
    failed_centre: &[P3],
    blockers: Option<&BlockerCensus>,
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

    let mut bands: Vec<usize> = rings.iter().map(|r| r.band.len()).collect();
    report.degenerate_rings = bands.iter().filter(|&&b| b <= DEGENERATE_BAND_MAX).count();
    bands.sort_unstable();
    report.band_size_min = bands.first().copied().unwrap_or(0);
    report.band_size_max = bands.last().copied().unwrap_or(0);
    report.band_size_median = if bands.is_empty() {
        0
    } else {
        bands.get(bands.len() / 2).copied().unwrap_or(0)
    };
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
    let last_radius = rings.last().map_or(1.0, |r| r.radius);

    // Disk-units-per-mm from the local area distortion, so `2·K_c` can be
    // expressed as a disk-domain band width.
    let global_median = report.area_distortion_median;
    let scale = report
        .area_distortion_by_disk_radius
        .iter()
        .find(|b| last_radius >= b.r_lo && last_radius <= b.r_hi && b.triangles > 0)
        .map_or(global_median, |b| b.area_distortion_median)
        .max(0.0)
        .sqrt();
    let band_width_disk = (2.0 * params.ball_radius_mm * scale).clamp(0.0, 1.0);

    let all_d = distances_to(uncovered, samples, reference);
    let near: Vec<usize> = uncovered
        .iter()
        .copied()
        .filter(|&s| {
            samples
                .get(s)
                .is_some_and(|p| p.disk_r > last_radius - band_width_disk)
        })
        .collect();
    let outside = uncovered
        .iter()
        .filter(|&&s| samples.get(s).is_some_and(|p| p.disk_r > last_radius))
        .count();

    report.stall = Some(StallContext {
        rings_placed: rings.len(),
        ring_radii: rings.iter().map(|r| r.radius).collect(),
        search_lo: state.search_lo,
        search_hi: state.search_hi,
        interior_radius_feasible: state.interior_feasible,
        uncovered: uncovered.len(),
        uncovered_outside_last_ring: outside,
        band_width_disk,
        distance_reference: which,
        coverage_radius_mm: params.ring_search_radius_mm(),
        all_uncovered: summarise(&all_d),
        near_band: summarise(&distances_to(&near, samples, reference)),
        blockers: blockers
            .map(|b| summarise(&b.distances))
            .unwrap_or_default(),
        blocker_disk_radius: blockers
            .map(|b| summarise(&b.disk_radii))
            .unwrap_or_default(),
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
    // The tool centre sits at the true `K_c`; only the search's own coverage
    // criterion is derated. See `SpiralParams::ring_spacing_safety`.
    let radius = params.ball_radius_mm;
    let search_radius = params.ring_search_radius_mm();
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
            report.ring_anisotropy = measure_ring_anisotropy(
                locator,
                region,
                normals,
                &rings.iter().map(|r| r.radius).collect::<Vec<f64>>(),
                params,
            );
            publish_ring_rows(
                &rings,
                &uncovered,
                samples,
                &last_centre,
                &[],
                None,
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
                sample.disk_r <= mid || cc.covers(sample.at, search_radius)
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
            if cc.covers(sample.at, search_radius) {
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
            // The blockers are the points that made `lo` infeasible: uncovered,
            // outside `lo`, and not swept by the ring at `lo`. They are what
            // stopped the descent, so they are what a postmortem must look at.
            let census = if state.search_lo > 0.0 {
                let (_, lo_centre, lo_cc) = curve_at(state.search_lo, &mut stats);
                let mut distances: Vec<f64> = Vec::new();
                let mut disk_radii: Vec<f64> = Vec::new();
                for &sidx in &uncovered {
                    let Some(sample) = samples.get(sidx) else {
                        continue;
                    };
                    if sample.disk_r <= state.search_lo || lo_cc.covers(sample.at, search_radius) {
                        continue;
                    }
                    distances.push(distance_to_polyline_mm(sample.at, &lo_centre));
                    disk_radii.push(sample.disk_r);
                }
                Some(BlockerCensus {
                    distances,
                    disk_radii,
                })
            } else {
                None
            };
            report.ring_anisotropy = measure_ring_anisotropy(
                locator,
                region,
                normals,
                &rings.iter().map(|r| r.radius).collect::<Vec<f64>>(),
                params,
            );
            publish_ring_rows(
                &rings,
                &uncovered,
                samples,
                &last_centre,
                &centre,
                census.as_ref(),
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

    report.ring_anisotropy = measure_ring_anisotropy(
        locator,
        region,
        normals,
        &rings.iter().map(|r| r.radius).collect::<Vec<f64>>(),
        params,
    );
    publish_ring_rows(
        &rings,
        &uncovered,
        samples,
        &last_centre,
        &[],
        None,
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
    /// The emitted spiral's tool-centre polyline, kept so the independent
    /// coverage audit can test against the path that will actually be cut.
    centre: Vec<P3>,
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
    // The repair must judge by the same criterion the search used, or it would
    // chase a bar the rings were never placed against.
    let search_radius = params.ring_search_radius_mm();
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
                .filter(|&&s| {
                    samples
                        .get(s)
                        .is_some_and(|p| !cc.covers(p.at, search_radius))
                })
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
        centre,
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
mod tests;
