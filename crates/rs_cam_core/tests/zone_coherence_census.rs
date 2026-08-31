//! **Do the decompositions the product already produces contain coherent
//! direction zones?** — a census, not a toolpath.
//!
//! # The question
//!
//! `planning/conformal_finish_2026-08-28/FINDINGS.md` §F1-2 established that
//! the preferred-direction method needs two properties, not one.
//!
//! 1. **Anisotropy magnitude.** Wanaka has it. §11 of
//!    `planning/finishing_synthesis_2026-08-30.md` measured a median
//!    `W_max/W_min` of 1.0950 on region 1 at `R = 1.0` mm, a +9.75 % prize
//!    ceiling. Both scale rules cleared. The surface has a preferred direction
//!    at each point.
//! 2. **Direction coherence over an area.** Region 1 does not have it. A
//!    coherence gate at 45 degrees cut region 1 into 1,452 patches. 1,064 of
//!    those were under 1 mm². At 10 degrees, 48.2 % of the region fell into
//!    patches under 1 mm². The segmentation cuts the surface into slivers.
//!
//! The operator then asked whether the pipeline's own regions are coherent
//! zones. The pipeline already produces regions. This file measures the
//! coherence of each one. It generates no toolpath and it costs no path.
//!
//! # The zone sources
//!
//! 1. **Tier islands.** `compute_tier_map` with
//!    `ResidualTreatment::SlopeCompensated`, then `extract_tier_islands`.
//!    Every tier is censused, not only tier 1.
//! 2. **Slope bands.** `build_classification_surface_with_sampler_and_cancel`,
//!    then `finish_planner::decompose`. Every `FinishBand` is censused.
//! 3. **Monotone cells — NOT REACHABLE, and this file says so rather than
//!    rebuilding them.** The C2 monotone cells live in
//!    `crates/rs_cam_core/tests/thin_organic_island_widths.rs`. An integration
//!    test cannot import another integration test, and that file is out of
//!    scope for this work. Restating its lattice machinery would put a second
//!    unverified copy of it in the repo. **In its place this file censuses a
//!    plain slope-band region cut into equal square tiles at three sizes.**
//!    That is a size control. It isolates one variable: does a zone become
//!    coherent simply by being small? A monotone cell is small. If small tiles
//!    read coherent, then size alone explains the reading, and the census does
//!    not need the cells to say so.
//!
//! # The estimator, restated
//!
//! Restated from `crates/rs_cam_core/tests/wanaka_curvature_anisotropy.rs`
//! (commit `7f341f9d`), and the `t1` extraction from
//! `crates/rs_cam_core/tests/direction_field_wanaka_f1.rs`. Both files are
//! read-only here. An integration test cannot import another one, so the
//! arithmetic is restated. It is not modified.
//!
//! ## Why a heightfield fit is valid
//!
//! `terrain.stl` is 100 % up-facing. It is a single-valued heightfield
//! `z = f(x, y)`. A Monge-patch fit is therefore the same object as a general
//! surface fit, not an approximation of one. This shortcut is valid for this
//! mesh only. A mesh with an overhang would need a normal-aligned local frame.
//!
//! ## Step 1 — the fit
//!
//! At a sample `(x0, y0)` with surface height `z0`, gather every mesh vertex
//! within XY distance `r` = [`FIT_RADIUS_MM`]. Least-squares fit, in the
//! scale-normalised frame `u = (x - x0)/r`, `v = (y - y0)/r`, `w = z - z0`:
//!
//! ```text
//!     w = a u^2 + b uv + c v^2 + d u + e v + g
//! ```
//!
//! Derivatives at the sample point are `f_x = d/r`, `f_y = e/r`,
//! `f_xx = 2a/r^2`, `f_xy = b/r^2`, `f_yy = 2c/r^2`.
//!
//! ## Step 2 — curvature from the shape operator
//!
//! On slope the second derivatives are not the curvatures. The metric matters.
//! Take the upward unit normal and `W = sqrt(1 + f_x^2 + f_y^2)`:
//!
//! ```text
//!     I  : E = 1 + f_x^2   F = f_x f_y   G = 1 + f_y^2     (EG - F^2 = W^2)
//!     II : L = f_xx/W      M = f_xy/W    N = f_yy/W
//!     S  = I^-1 II
//!     K  = (LN - M^2)/(EG - F^2)
//!     H  = (E N - 2 F M + G L) / (2 (EG - F^2))
//!     kappa_monge = H +/- sqrt(H^2 - K)
//! ```
//!
//! ## Step 3 — the sign convention
//!
//! The Monge form above takes the upward normal. A convex dome then reads
//! negative. Zou's Eq. 2 is convex-positive. This file negates:
//!
//! ```text
//!     kappa1 = -(H - sqrt(H^2 - K))      (max, most convex)
//!     kappa2 = -(H + sqrt(H^2 - K))      (min, most concave)
//! ```
//!
//! ## Step 4 — `t1`, the quantity this census is about
//!
//! `t1` is the principal direction of `kappa1`, the most convex direction. It
//! is the eigenvector of `S` for the **smaller** Monge eigenvalue
//! `lambda = H - sqrt(H^2 - K)`, because Step 3 flipped the sign. A feed along
//! `t1` leaves `kappa2` perpendicular, which gives the widest strip. That is
//! `D = t1`, the literature's own answer (synthesis §10).
//!
//! A parameter-space direction `(p, q)` on a Monge patch is the 3-D tangent
//! `p (1, 0, f_x) + q (0, 1, f_y)`. Its XY part is exactly `(p, q)`. **The
//! eigenvector is already the XY direction.** No projection is needed and none
//! can fail. A 3-axis machine commands a feed direction in XY, so this is also
//! the quantity a strategy would consume.
//!
//! `t1` is a **line field**, not a vector field. `t1` and `-t1` are the same
//! direction. Every operation below respects that. The mean is the principal
//! eigenvector of the summed outer product. Every angle difference is
//! `acos(|dot|)`, which lands in `[0, 90]` degrees.
//!
//! ## Step 5 — when `t1` carries no meaning
//!
//! Two conditions void the direction. Both are counted.
//!
//! * The fit failed. It was under-determined (fewer than [`MIN_FIT_POINTS`]
//!   vertices) or rank deficient (pivot ratio under [`MIN_PIVOT_RATIO`]).
//! * The point is isotropic:
//!   `|kappa1 - kappa2| <= max(ABS, REL * max(|kappa1|, |kappa2|))`, with
//!   [`ISOTROPY_ABS_TOL`] and [`ISOTROPY_REL_TOL`]. **These are §F1-2's own
//!   floor, restated.** This census and the segmentation that produced the
//!   1,452 patches therefore call the same triangles degenerate. The two
//!   constants come in turn from `direction_field::FieldParams`' defaults.
//!
//! A triangle with a voided direction is **untrusted**. It never joins the
//! coherent area and it is never a search target.
//!
//! # Sampling
//!
//! ## The census unit is a mesh triangle
//!
//! One Monge fit per triangle, at the triangle's XY centroid, with `z0` the
//! centroid height. The fit runs **once over the whole mesh**. Every zone then
//! indexes into that one field. Two consequences follow, and both are wanted.
//! Every zone is measured by the same estimator on the same samples. A
//! triangle in two overlapping zones carries one direction, not two.
//!
//! Area is the triangle's true 3-D area. Every area fraction below is weighted
//! by it.
//!
//! ## Zone membership
//!
//! A triangle belongs to a zone when its XY centroid satisfies
//! `Polygon2::contains_point`. That is the same rule
//! `direction_field_wanaka_f1.rs` uses to select region 1.
//!
//! ## Coherence fraction
//!
//! Every triangle of a zone is used. Nothing is subsampled.
//!
//! The dominant direction is the principal eigenvector of the area-weighted
//! outer-product sum `sum(area * t1 t1^T)` over the zone's **trusted**
//! triangles. The reported coherence fraction at angle `A` is
//!
//! ```text
//!     (trusted area whose t1 is within A of the dominant) / (TOTAL zone area)
//! ```
//!
//! **The denominator is the total zone area, not the trusted area.** An
//! untrusted triangle never counts as within. That is the conservative choice.
//! It is also the right one for the routing decision: a zone fails equally
//! whether its field turns or whether it has no field. The trusted-only
//! fraction is printed beside it, so the two causes stay separable.
//!
//! ## Coherence length
//!
//! For a query triangle, this is the distance to the **nearest trusted
//! triangle of the same zone whose `t1` differs from the query's by more than
//! [`COHERENCE_TURN_DEG`] (30 degrees)**. Distance is between XY centroids,
//! because the stepover is an XY quantity.
//!
//! * **Search set**: every trusted triangle of that zone. Never another zone.
//! * **Search bound**: [`COHERENCE_SEARCH_BOUND_MM`] = 20 stepovers =
//!   9.724 mm. That is twice the usability bar, so the bound never decides a
//!   passing zone.
//! * **Query set**: the zone's trusted triangles, capped at
//!   [`COHERENCE_QUERY_CAP`] by a deterministic stride over triangle-index
//!   order. Never random.
//! * **Censoring**: a query that finds no differing triangle inside the bound
//!   is right-censored. It enters the median at exactly the bound. The
//!   censored fraction is printed. When that fraction exceeds 0.5 the median
//!   equals the bound, and the row prints `>=`.
//!
//! A bucket grid at [`COHERENCE_GRID_CELL_MM`] serves the search, expanded
//! ring by ring. Ring `k` is entered only while the best distance so far
//! exceeds `k * cell`, which is the smallest distance an unvisited cell can
//! hold.
//!
//! # Pre-registered thresholds — WRITTEN BEFORE THE NUMBERS
//!
//! ## The two bars the task sets
//!
//! * **Usable**: at least [`USABLE_WITHIN_30_MIN`] (0.70) of the zone's area
//!   lies within 30 degrees of its dominant direction, AND its coherence
//!   length is at least [`USABLE_LENGTH_MIN_STEPOVERS`] (10) stepovers. That
//!   is 4.862 mm at the pinned [`STEPOVER_MM`] = 0.4862.
//! * **Not usable**: the 30-degree coherence fraction falls below
//!   [`NOT_USABLE_WITHIN_30_BELOW`] (0.50).
//! * Anything between the two is **marginal**. It does not count as usable.
//!
//! ## Three guards this file adds, and why
//!
//! These are **my additions**, not the task's. Each one closes a way of
//! scoring a win that is not there.
//!
//! 1. **[`MIN_VERDICT_AREA_MM2`] = (10 x 0.4862)^2 = 23.64 mm².** A zone
//!    smaller than a 10-stepover square cannot hold 10 stepovers in both
//!    directions. It cannot meet the coherence-length bar on its own merits.
//!    Worse, it reads a **long** coherence length for the wrong reason: a
//!    small zone holds nothing to differ from, so every query censors. Such a
//!    zone gets the verdict `TooSmall` and is **never usable**. Its full
//!    metrics are still printed. That is the size control's data.
//! 2. **Fewer than [`MIN_TRUSTED_TRIANGLES`] (2) trusted triangles** leaves
//!    the coherence length undefined. Verdict `NotMeasurable`. Never usable,
//!    never a panic.
//! 3. **The covered-area rollup reports a union, not a sum.** Band polygons
//!    are dilated by `overlap_mm = 2.0` at extraction
//!    (`finish_planner.rs:563`), so neighbouring bands overlap. Summing zone
//!    areas would count the seams twice. Each rollup marks triangles and
//!    reports the union area, and prints the sum beside it so the overlap
//!    stays visible.
//!
//! ## Which tier set is censused
//!
//! `TierIslandSet::owned`, not `machining`. `owned` is the tier's own
//! territory and those sets are disjoint, which is what the union rollup
//! needs. `machining` is `owned` dilated by 2.0 mm. That dilation imports the
//! neighbouring tier's geometry into the fringe, so its coherence can only be
//! equal or worse. The number reported here is the optimistic one.
//!
//! ## Tile sizes
//!
//! [`TILE_SIZES_MM`] = 16, 8, 4. The area bar sits at 23.64 mm². So 16 mm
//! (256 mm²) and 8 mm (64 mm²) tiles carry verdicts, and 4 mm (16 mm²) tiles
//! do not. The trend is visible on both sides of the bar.
//!
//! Each tile row prints **three** area fractions, not two, so that guard 1 is
//! never mistaken for a measurement. The first is the area in tiles clearing
//! the 30-degree bar. The second is the area in tiles that also clear the area
//! bar. The third is the area in tiles that are `Usable`. On the 4 mm row the
//! second and third are zero by guard 1, whatever the surface does, and the
//! row says so. **Read the first figure across the three sizes.** That is the
//! size control's actual output.
//!
//! # Prediction — stated so the run can refute it
//!
//! Per source, because a global prediction cannot be refuted usefully.
//!
//! * **Shallow slope bands: fail.** Region 1 is the largest of them and is the
//!   §F1-2 failure site. Its field turns continuously. I predict its
//!   30-degree coherence fraction below 0.50, and its coherence length near 1
//!   to 2 stepovers.
//! * **VerySteep slope bands: the one source with a geometric reason to
//!   pass.** §F1-1's scope note says the method survives on steep, curved,
//!   simply-connected surfaces. I predict the VerySteep band carries the
//!   highest coherence fraction of any source. I also predict its total area
//!   on this terrain is small, so a pass there wins little.
//! * **Tier islands: fail, and no better than the slope bands.** A tier island
//!   comes from residual height and reach. Neither is a function of curvature
//!   direction. I expect no relationship to `t1` at all.
//! * **Tiles: coherence rises as the tile shrinks, and the verdict-eligible
//!   sizes still fail.** I predict 16 mm tiles below 0.70, 8 mm tiles near it,
//!   and 4 mm tiles above it but disqualified by the area bar. If the run
//!   shows that, then size alone produces coherence on this terrain, and the
//!   monotone cells this file cannot reach would read the same way for the
//!   same reason.
//! * **Overall: I predict usable zones cover under 5 % of the surface area.**
//!   That number decides whether routing a subset to a different strategy is
//!   worth building.
//!
//! # Running it
//!
//! ```text
//! # The census (needs the operator's wanaka mesh):
//! cargo test -p rs_cam_core --test zone_coherence_census \
//!   wanaka_zone_coherence_census -- --ignored --nocapture
//!
//! # The self-check (no mesh needed, runs in the normal gate):
//! cargo test -p rs_cam_core --test zone_coherence_census \
//!   census_reports_known_coherence
//! ```
//!
//! The census is `#[ignore]` because it needs the operator's mesh, which is
//! not in the repo. It SKIPs, and never fails, when the mesh is absent. It
//! asserts nothing about the terrain. An instrument records what it finds and
//! leaves the bar to a human. The self-check is the only part a reviewer can
//! check without the mesh, and it is not `#[ignore]`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::Path;

use rayon::prelude::*;
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

// ── the frozen chain's constants ────────────────────────────────────────
//
// Restated verbatim from `tests/wanaka_region_capture_f1.rs`, which pinned
// them from `thin_organic_island_widths.rs`. The chain is FROZEN. A ball-end
// substitution would derive different zones and would destroy comparability
// with §F1-2's numbers, which this census exists to sit beside.

/// The operator's wanaka board. The path is absolute because the file lives
/// outside the repo.
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

/// Triangle count the region-1 capture was cut against. Reported and warned
/// about, never asserted. A re-exported mesh is a legitimate event.
const EXPECTED_TRIANGLES: usize = 661_212;

/// Tier-map grid cell (mm).
const CELL_MM: f64 = 0.3;
/// Tier-map tolerance (mm).
const TOLERANCE_MM: f64 = 0.05;
/// Tier-map margin (mm).
const MARGIN_MM: f64 = 0.5;
/// Tier-island coarseness.
const COARSENESS: f64 = 1.0;
/// Tier-island and finish-planner band dilation (mm).
const OVERLAP_MM: f64 = 2.0;
/// Tier-island region cap per tier.
const MAX_REGIONS_PER_TIER: usize = 24;
/// The unified op's own path tolerance, which sizes its classification grid.
const OP_TOLERANCE_MM: f64 = 0.05;

// `TaperedBallEndmill::new(ball_diameter, taper_half_angle_deg,
// shaft_diameter, cutting_length)`. The second slot is a HALF-angle from the
// tool axis. The fourth is cutting length, not stickout.

/// Tier-0 cutter: R1.5 tapered ball.
const R15_TUPLE: [f64; 4] = [3.0, 2.8, 6.0, 30.5];
/// Tier-1 cutter: R1.0 tapered ball.
const R10_TUPLE: [f64; 4] = [2.0, 5.7, 6.0, 20.0];

// ── the estimator's constants ───────────────────────────────────────────

/// Monge-fit radius (mm). `wanaka_curvature_anisotropy.rs`'s
/// `VERDICT_FIT_RADIUS_MM`: about 3x the mesh's 0.335 mm median facet edge. It
/// is the radius that instrument's verdict reads, and the one at which both of
/// its scale rules cleared.
const FIT_RADIUS_MM: f64 = 1.0;

/// Minimum gathered vertices for a fit: 6 quadric parameters plus 1 degree of
/// freedom, so a residual exists. Restated
/// (`wanaka_curvature_anisotropy.rs:389`).
const MIN_FIT_POINTS: usize = 7;

/// Pivot ratio below which a fit is called rank deficient rather than solved.
/// Restated (`wanaka_curvature_anisotropy.rs:394`).
const MIN_PIVOT_RATIO: f64 = 1e-9;

/// Relative isotropy floor. Restated from
/// `direction_field_wanaka_f1.rs:SEG_ISOTROPY_REL_TOL`, which restated it from
/// `direction_field::FieldParams`' defaults. A triangle is degenerate when
/// `|kappa1 - kappa2| <= max(ISOTROPY_ABS_TOL, ISOTROPY_REL_TOL * scale)`.
const ISOTROPY_REL_TOL: f64 = 0.10;
/// Absolute isotropy floor (1/mm). See [`ISOTROPY_REL_TOL`].
const ISOTROPY_ABS_TOL: f64 = 1e-6;

/// Below this a 2-vector has no direction.
const EPS_VEC: f64 = 1e-12;

// ── the census's constants ──────────────────────────────────────────────

/// Equal-cusp stepover (mm) at `R = 1.0`, `h = 0.03`. Restated; pinned in
/// `direction_field_wanaka_f1.rs` by
/// `equal_cusp_stepover_at_r1_h003_is_0_4862`.
const STEPOVER_MM: f64 = 0.4862;

/// The turn that ends a coherent run, in degrees.
const COHERENCE_TURN_DEG: f64 = 30.0;

/// Angles at which the coherence fraction is reported.
const COHERENCE_ANGLES_DEG: [f64; 4] = [10.0, 20.0, 30.0, 45.0];

/// Index of 30 degrees in [`COHERENCE_ANGLES_DEG`] — the verdict column.
const VERDICT_ANGLE_INDEX: usize = 2;

/// Coherence-length search bound (mm): 20 stepovers. Twice the usability bar,
/// so the bound never decides a passing zone.
const COHERENCE_SEARCH_BOUND_MM: f64 = 20.0 * STEPOVER_MM;

/// Bucket-grid cell (mm) for the coherence-length search.
const COHERENCE_GRID_CELL_MM: f64 = 1.0;

/// Query cap per zone. Above this the queries are a deterministic stride over
/// triangle-index order.
const COHERENCE_QUERY_CAP: usize = 4_000;

/// Coherence-fraction bar for `Usable`.
const USABLE_WITHIN_30_MIN: f64 = 0.70;

/// Coherence-length bar for `Usable`, in stepovers.
const USABLE_LENGTH_MIN_STEPOVERS: f64 = 10.0;

/// The same bar in mm.
const USABLE_LENGTH_MIN_MM: f64 = USABLE_LENGTH_MIN_STEPOVERS * STEPOVER_MM;

/// Coherence fraction below which a zone is `NotUsable` outright.
const NOT_USABLE_WITHIN_30_BELOW: f64 = 0.50;

/// A zone below this area (mm²) cannot hold 10 stepovers in both directions.
/// It is `TooSmall` and never usable. See the module doc's guard 1.
const MIN_VERDICT_AREA_MM2: f64 = USABLE_LENGTH_MIN_MM * USABLE_LENGTH_MIN_MM;

/// Below this many trusted triangles the coherence length is undefined.
const MIN_TRUSTED_TRIANGLES: usize = 2;

/// Tile edges (mm) for the size control. 16 and 8 clear
/// [`MIN_VERDICT_AREA_MM2`]; 4 does not.
const TILE_SIZES_MM: [f64; 3] = [16.0, 8.0, 4.0];

/// Rows printed per source before the remainder is aggregated.
const MAX_ROWS_PER_SOURCE: usize = 16;

// ── §F1-2's own numbers, for the same-scale comparison ──────────────────

/// Patches §F1-2's 45-degree coherence gate cut region 1 into.
const F1_2_PATCHES_AT_45: usize = 1_452;
/// Of those, the ones under 1 mm².
const F1_2_SLIVERS_AT_45: usize = 1_064;
/// Fraction of region 1 that fell into patches under 1 mm² at 10 degrees.
const F1_2_SLIVER_AREA_FRACTION_AT_10: f64 = 0.482;
/// Region 1's measured prize ceiling at `R = 1.0`
/// (`planning/finishing_synthesis_2026-08-30.md` §11).
const F1_2_REGION1_PRIZE_CEILING_PCT: f64 = 9.75;

// ════════════════════════════════════════════════════════════════════════
// The estimator
// ════════════════════════════════════════════════════════════════════════

/// Per-thread reusable buffers. Held by `map_init`, so nothing here is
/// allocated per sample. Restated from `wanaka_curvature_anisotropy.rs`.
struct Scratch {
    query: QueryScratch,
    tris: Vec<usize>,
    /// Generation stamp per mesh vertex. Dedups the triangle-to-vertex
    /// expansion without clearing a bitset per sample.
    stamp: Vec<u32>,
    generation: u32,
}

impl Scratch {
    fn new(vertex_count: usize) -> Self {
        Self {
            query: QueryScratch::default(),
            tris: Vec::new(),
            // Stamps start at 0 and `generation` increments BEFORE use, so
            // generation 1 is the first live value and 0 never matches.
            stamp: vec![0u32; vertex_count],
            generation: 0,
        }
    }
}

/// The local differential geometry at one sample.
#[derive(Clone, Copy)]
struct Fit {
    /// Max principal curvature, convex-positive. `kappa1 >= kappa2`.
    kappa1: f64,
    /// Min principal curvature, convex-positive.
    kappa2: f64,
    /// Unit XY `t1` axis. The sign is arbitrary. `None` when the shape
    /// operator is a multiple of the identity, so no principal direction
    /// exists.
    axis: Option<[f64; 2]>,
}

/// Why a sample produced no fit. Counted, never silently dropped.
enum Outcome {
    Fitted(Fit),
    /// Fewer than [`MIN_FIT_POINTS`] vertices in the disc.
    UnderDetermined,
    /// Rank-deficient normal matrix, or a non-finite solution.
    IllConditioned,
}

/// Solve the symmetric 6x6 system `A x = b` by Gauss-Jordan with partial
/// pivoting. Returns `(x, min|pivot| / max|pivot|)`.
///
/// Hand-rolled rather than pulled from a linear-algebra crate, for the two
/// reasons the source instrument gives. The pivot ratio is wanted as a
/// reported conditioning number, and a library solve would not surrender it.
/// An integration test must add no dependency edge for six rows. `A` arrives
/// scale-normalised, so its pivots are O(1) and their ratio is meaningful.
#[allow(clippy::needless_range_loop)] // Gauss-Jordan indexes three arrays by one counter.
fn solve_sym6(a: &[[f64; 6]; 6], b: &[f64; 6]) -> Option<([f64; 6], f64)> {
    let mut m = [[0.0f64; 7]; 6];
    for row in 0..6 {
        for col in 0..6 {
            m[row][col] = a[row][col];
        }
        m[row][6] = b[row];
    }
    let mut pivot_min = f64::INFINITY;
    let mut pivot_max = 0.0f64;
    for col in 0..6 {
        let mut best = col;
        for row in (col + 1)..6 {
            if m[row][col].abs() > m[best][col].abs() {
                best = row;
            }
        }
        m.swap(col, best);
        let pivot = m[col][col];
        let mag = pivot.abs();
        pivot_min = pivot_min.min(mag);
        pivot_max = pivot_max.max(mag);
        if mag < f64::MIN_POSITIVE {
            return None;
        }
        let inv = 1.0 / pivot;
        for k in col..7 {
            m[col][k] *= inv;
        }
        for row in 0..6 {
            if row == col {
                continue;
            }
            let factor = m[row][col];
            if factor == 0.0 {
                continue;
            }
            for k in col..7 {
                // Hoisted so the statement never reads and writes `m` in one
                // place expression.
                let pivot_row = m[col][k];
                m[row][k] -= factor * pivot_row;
            }
        }
    }
    let mut x = [0.0f64; 6];
    for row in 0..6 {
        x[row] = m[row][6];
    }
    // `pivot_max` is a maximum of absolute values, so it is never negative.
    // [`ratio`] therefore returns the pivot ratio, or 0 for a zero matrix.
    Some((x, ratio(pivot_min, pivot_max)))
}

/// Mirror the accumulated upper triangle into the lower one and scale by
/// `inv = 1/count`, so the normal matrix is a mean of rank-1 terms with O(1)
/// entries and a comparable pivot ratio.
///
/// Row-major order means `normal[j][i]` for `j < i` is already scaled when it
/// is copied, so the mirror never double-scales.
#[allow(clippy::needless_range_loop)] // the symmetric mirror indexes [i][j] and [j][i].
fn finalise_normal_equations(normal: &mut [[f64; 6]; 6], rhs: &mut [f64; 6], inv: f64) {
    for i in 0..6 {
        for j in 0..6 {
            if j < i {
                let mirrored = normal[j][i];
                normal[i][j] = mirrored;
            } else {
                normal[i][j] *= inv;
            }
        }
        rhs[i] *= inv;
    }
}

/// Build [`Fit`] from the fitted heightfield's derivatives. Steps 2, 3 and 4
/// of the module doc, and the only place they happen.
///
/// Worked check on the bowl `z = (A x^2 + B y^2)/2` with `A > B > 0`:
/// `S = diag(A, B)`, the smaller eigenvalue is `B`, its eigenvector is `y`,
/// and `kappa1 = -B >= -A = kappa2`. So `t1 = y`, the gentler direction. The
/// step-over then runs along `x`, the sharply concave one, where the ball fits
/// deepest and the strip is widest.
fn fit_from_derivatives((f_x, f_y): (f64, f64), (f_xx, f_xy, f_yy): (f64, f64, f64)) -> Fit {
    let area_weight = (1.0 + f_x * f_x + f_y * f_y).sqrt();
    let form_e = 1.0 + f_x * f_x;
    let form_f = f_x * f_y;
    let form_g = 1.0 + f_y * f_y;
    let form_l = f_xx / area_weight;
    let form_m = f_xy / area_weight;
    let form_n = f_yy / area_weight;
    // EG - F^2 = W^2 >= 1: no degenerate metric here.
    let det = form_e * form_g - form_f * form_f;
    let gauss = (form_l * form_n - form_m * form_m) / det;
    let mean = (form_e * form_n - 2.0 * form_f * form_m + form_g * form_l) / (2.0 * det);
    // H^2 - K = ((ka - kb)/2)^2 >= 0 exactly. The max() absorbs round-off.
    let spread = (mean * mean - gauss).max(0.0).sqrt();

    // Shape operator entries, S = I^-1 II.
    let s_a = (form_g * form_l - form_f * form_m) / det;
    let s_b = (form_g * form_m - form_f * form_n) / det;
    let s_c = (form_e * form_m - form_f * form_l) / det;
    let s_d = (form_e * form_n - form_f * form_m) / det;
    let lambda = mean - spread;
    // Both rows of (S - lambda I) v = 0. The better-conditioned one wins.
    let row1 = [s_b, lambda - s_a];
    let row2 = [s_d - lambda, -s_c];
    let n1 = (row1[0] * row1[0] + row1[1] * row1[1]).sqrt();
    let n2 = (row2[0] * row2[0] + row2[1] * row2[1]).sqrt();
    // A relative floor. On this terrain the operator's entries run about
    // 1e-3 per mm, so an absolute epsilon would call nothing umbilic.
    let magnitude = s_a.abs().max(s_b.abs()).max(s_c.abs()).max(s_d.abs());
    let floor = (magnitude * 1e-9).max(f64::MIN_POSITIVE);
    let axis = if n1 >= n2 && n1 > floor {
        Some([row1[0] / n1, row1[1] / n1])
    } else if n2 > floor {
        Some([row2[0] / n2, row2[1] / n2])
    } else {
        None
    };

    Fit {
        // Convex-positive flip (module doc Step 3).
        kappa1: -(mean - spread),
        kappa2: -(mean + spread),
        axis,
    }
}

/// Fit the local quadric at `at`, whose surface height is `z0`, over `radius`.
/// One streaming pass. Nothing is stored per neighbour.
fn fit_quadric(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    scratch: &mut Scratch,
    (at, z0): (P2, f64),
    radius: f64,
) -> Outcome {
    // Taken out and put back so the neighbour list is reused across samples
    // while `scratch.query` can be borrowed mutably at the same time.
    let mut tris = std::mem::take(&mut scratch.tris);
    index.query_rect_into(
        at.x - radius,
        at.x + radius,
        at.y - radius,
        at.y + radius,
        &mut scratch.query,
        &mut tris,
    );
    scratch.generation = scratch.generation.wrapping_add(1);
    let generation = scratch.generation;

    let mut normal = [[0.0f64; 6]; 6];
    let mut rhs = [0.0f64; 6];
    let mut count = 0usize;
    let r2 = radius * radius;

    for &t in &tris {
        for &vi in &mesh.triangles[t] {
            let vi = vi as usize;
            if scratch.stamp[vi] == generation {
                continue;
            }
            scratch.stamp[vi] = generation;
            let p = mesh.vertices[vi];
            let dx = p.x - at.x;
            let dy = p.y - at.y;
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let u = dx / radius;
            let v = dy / radius;
            let w = p.z - z0;
            let basis = [u * u, u * v, v * v, u, v, 1.0];
            for (i, &bi) in basis.iter().enumerate() {
                for (j, &bj) in basis.iter().enumerate().skip(i) {
                    normal[i][j] += bi * bj;
                }
                rhs[i] += bi * w;
            }
            count += 1;
        }
    }
    scratch.tris = tris;

    if count < MIN_FIT_POINTS {
        return Outcome::UnderDetermined;
    }
    finalise_normal_equations(&mut normal, &mut rhs, 1.0 / count as f64);

    let Some((beta, pivot_ratio)) = solve_sym6(&normal, &rhs) else {
        return Outcome::IllConditioned;
    };
    if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO {
        return Outcome::IllConditioned;
    }
    if !beta.iter().all(|c| c.is_finite()) {
        return Outcome::IllConditioned;
    }

    let f_x = beta[3] / radius;
    let f_y = beta[4] / radius;
    let f_xx = 2.0 * beta[0] / (radius * radius);
    let f_xy = beta[1] / (radius * radius);
    let f_yy = 2.0 * beta[2] / (radius * radius);
    Outcome::Fitted(fit_from_derivatives((f_x, f_y), (f_xx, f_xy, f_yy)))
}

// ════════════════════════════════════════════════════════════════════════
// The per-triangle field
// ════════════════════════════════════════════════════════════════════════

/// Everything the census needs about one mesh triangle. Built once for the
/// whole mesh. Every zone indexes into it.
#[derive(Clone, Copy)]
struct TriField {
    /// XY centroid. Zone membership and every distance read this.
    centroid: P2,
    /// True 3-D surface area (mm²). Every area fraction is weighted by it.
    area_mm2: f64,
    /// Unit XY `t1`, sign arbitrary. `Some` only when the fit succeeded, the
    /// shape operator is not a multiple of the identity, and the anisotropy
    /// clears the isotropy floor. That is the TRUSTED condition.
    axis: Option<[f64; 2]>,
    /// The fit itself succeeded, whatever the anisotropy was.
    fitted: bool,
    /// Fitted, but umbilic or below the isotropy floor. `t1` carries no
    /// meaning here. Disjoint from `axis.is_some()`.
    degenerate: bool,
}

/// Fit-outcome counts over the whole mesh. Nothing is dropped silently.
#[derive(Default)]
struct FieldCensus {
    fitted: usize,
    under_determined: usize,
    ill_conditioned: usize,
    /// Fitted, but the shape operator is a multiple of the identity.
    umbilic: usize,
    /// Fitted with an axis, but `|kappa1 - kappa2|` is at or under the
    /// isotropy floor. `t1` exists numerically and is not believed.
    below_isotropy_floor: usize,
}

/// `a / b`, or 0 when `b` is not positive. Keeps every reported fraction free
/// of NaN when a zone is empty.
fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b } else { 0.0 }
}

/// XY centroid, centroid height and true area of one mesh triangle.
fn triangle_geometry(mesh: &TriangleMesh, tri_index: usize) -> (P2, f64, f64) {
    let tri = mesh.triangles[tri_index];
    let a = mesh.vertices[tri[0] as usize];
    let b = mesh.vertices[tri[1] as usize];
    let c = mesh.vertices[tri[2] as usize];
    let centroid = P2::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
    let z0 = (a.z + b.z + c.z) / 3.0;
    let area = 0.5 * (b - a).cross(&(c - a)).norm();
    (centroid, z0, area)
}

/// Fit every triangle of `mesh` once.
///
/// The parallel pass collects into an order-stable `Vec`, and the counts are
/// reduced SERIALLY afterwards. A parallel float reduction would make the
/// printed numbers jitter between runs, and an instrument whose output moves
/// is not evidence.
fn build_field(mesh: &TriangleMesh, index: &SpatialIndex) -> (Vec<TriField>, FieldCensus) {
    let outcomes: Vec<(TriField, Outcome)> = (0..mesh.triangles.len())
        .into_par_iter()
        .with_min_len(512)
        .map_init(
            || Scratch::new(mesh.vertices.len()),
            |scratch, t| {
                let (centroid, z0, area_mm2) = triangle_geometry(mesh, t);
                let sample = (centroid, z0);
                let outcome = fit_quadric(mesh, index, scratch, sample, FIT_RADIUS_MM);
                let blank = TriField {
                    centroid,
                    area_mm2,
                    axis: None,
                    fitted: false,
                    degenerate: false,
                };
                (blank, outcome)
            },
        )
        .collect();

    let mut census = FieldCensus::default();
    let mut field = Vec::with_capacity(outcomes.len());
    for (mut entry, outcome) in outcomes {
        match outcome {
            Outcome::Fitted(fit) => {
                census.fitted += 1;
                entry.fitted = true;
                let anisotropy = (fit.kappa1 - fit.kappa2).abs();
                let scale = fit.kappa1.abs().max(fit.kappa2.abs());
                let floor = ISOTROPY_ABS_TOL.max(ISOTROPY_REL_TOL * scale);
                let singular = anisotropy <= floor;
                match fit.axis {
                    None => {
                        census.umbilic += 1;
                        entry.degenerate = true;
                    }
                    Some(_) if singular => {
                        census.below_isotropy_floor += 1;
                        entry.degenerate = true;
                    }
                    Some(axis) => entry.axis = Some(axis),
                }
            }
            Outcome::UnderDetermined => census.under_determined += 1,
            Outcome::IllConditioned => census.ill_conditioned += 1,
        }
        field.push(entry);
    }
    (field, census)
}

// ════════════════════════════════════════════════════════════════════════
// Line-field arithmetic
// ════════════════════════════════════════════════════════════════════════

/// `|cos|` of the angle between two unit axes. `t1` and `-t1` are the same
/// direction, so the absolute value is the whole point.
fn axis_cos(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] * b[0] + a[1] * b[1]).abs().min(1.0)
}

/// The dominant direction of a line field: the principal eigenvector of the
/// area-weighted outer-product sum `sum(w t1 t1^T)`.
///
/// This is the correct mean for a line field. A plain vector mean would cancel
/// `t1` against `-t1` and return noise. `None` when the sum is isotropic, that
/// is when the field has no dominant direction at all.
fn dominant_axis(entries: &[(f64, [f64; 2])]) -> Option<[f64; 2]> {
    let mut mxx = 0.0f64;
    let mut mxy = 0.0f64;
    let mut myy = 0.0f64;
    for &(w, a) in entries {
        mxx += w * a[0] * a[0];
        mxy += w * a[0] * a[1];
        myy += w * a[1] * a[1];
    }
    let mean = 0.5 * (mxx + myy);
    let spread = (0.25 * (mxx - myy) * (mxx - myy) + mxy * mxy).sqrt();
    if !spread.is_finite() || spread <= EPS_VEC * mean.abs().max(EPS_VEC) {
        return None;
    }
    let lambda = mean + spread;
    let row1 = [mxy, lambda - mxx];
    let row2 = [lambda - myy, mxy];
    let n1 = (row1[0] * row1[0] + row1[1] * row1[1]).sqrt();
    let n2 = (row2[0] * row2[0] + row2[1] * row2[1]).sqrt();
    if n1 >= n2 && n1 > EPS_VEC {
        Some([row1[0] / n1, row1[1] / n1])
    } else if n2 > EPS_VEC {
        Some([row2[0] / n2, row2[1] / n2])
    } else {
        None
    }
}

// ════════════════════════════════════════════════════════════════════════
// Coherence length
// ════════════════════════════════════════════════════════════════════════

/// A zone's trusted triangles in a bucket grid, for the nearest-turn search.
struct TurnGrid {
    pts: Vec<P2>,
    axes: Vec<[f64; 2]>,
    origin: P2,
    cols: i64,
    rows: i64,
    buckets: Vec<Vec<u32>>,
}

impl TurnGrid {
    /// `None` when the zone holds fewer than [`MIN_TRUSTED_TRIANGLES`].
    fn build(field: &[TriField], members: &[u32]) -> Option<Self> {
        let mut pts = Vec::new();
        let mut axes = Vec::new();
        for &m in members {
            let entry = &field[m as usize];
            if let Some(axis) = entry.axis {
                pts.push(entry.centroid);
                axes.push(axis);
            }
        }
        if pts.len() < MIN_TRUSTED_TRIANGLES {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let cell = COHERENCE_GRID_CELL_MM;
        let cols = (((max_x - min_x) / cell).floor() as i64 + 1).max(1);
        let rows = (((max_y - min_y) / cell).floor() as i64 + 1).max(1);
        let mut buckets = vec![Vec::new(); (cols * rows) as usize];
        for (i, p) in pts.iter().enumerate() {
            let c = (((p.x - min_x) / cell).floor() as i64).clamp(0, cols - 1);
            let r = (((p.y - min_y) / cell).floor() as i64).clamp(0, rows - 1);
            buckets[(r * cols + c) as usize].push(i as u32);
        }
        Some(Self {
            pts,
            axes,
            origin: P2::new(min_x, min_y),
            cols,
            rows,
            buckets,
        })
    }

    fn cell_of(&self, p: P2) -> (i64, i64) {
        let cell = COHERENCE_GRID_CELL_MM;
        let c = (((p.x - self.origin.x) / cell).floor() as i64).clamp(0, self.cols - 1);
        let r = (((p.y - self.origin.y) / cell).floor() as i64).clamp(0, self.rows - 1);
        (c, r)
    }

    /// Distance to the nearest trusted member whose axis differs from
    /// `from`'s by more than [`COHERENCE_TURN_DEG`]. `None` means the search
    /// was right-censored: nothing differing lies inside
    /// [`COHERENCE_SEARCH_BOUND_MM`].
    ///
    /// Ring `k` holds the cells at Chebyshev distance `k` from the query's
    /// cell. After ring `k` is processed, every unvisited member lies at
    /// Euclidean distance at least `k * cell`, because the query can sit
    /// anywhere inside its own cell. The loop stops on that guarantee.
    fn nearest_turn(&self, from: usize, cos_turn: f64) -> Option<f64> {
        let p = self.pts[from];
        let a = self.axes[from];
        let (qc, qr) = self.cell_of(p);
        let mut best = f64::INFINITY;
        let max_ring = self.cols.max(self.rows);
        for ring in 0..=max_ring {
            let lo_c = (qc - ring).max(0);
            let hi_c = (qc + ring).min(self.cols - 1);
            let lo_r = (qr - ring).max(0);
            let hi_r = (qr + ring).min(self.rows - 1);
            for r in lo_r..=hi_r {
                for c in lo_c..=hi_c {
                    if (c - qc).abs().max((r - qr).abs()) != ring {
                        continue;
                    }
                    for &idx in &self.buckets[(r * self.cols + c) as usize] {
                        let i = idx as usize;
                        if i == from || axis_cos(a, self.axes[i]) >= cos_turn {
                            continue;
                        }
                        let dx = self.pts[i].x - p.x;
                        let dy = self.pts[i].y - p.y;
                        let d = (dx * dx + dy * dy).sqrt();
                        if d < best {
                            best = d;
                        }
                    }
                }
            }
            let guaranteed = ring as f64 * COHERENCE_GRID_CELL_MM;
            if best <= guaranteed || guaranteed > COHERENCE_SEARCH_BOUND_MM {
                break;
            }
        }
        (best <= COHERENCE_SEARCH_BOUND_MM).then_some(best)
    }
}

// ════════════════════════════════════════════════════════════════════════
// The census
// ════════════════════════════════════════════════════════════════════════

/// The pre-registered verdict. Only [`Verdict::Usable`] counts as usable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    Usable,
    /// Between the two bars, or over the fraction bar with too short a
    /// coherence length.
    Marginal,
    /// Under [`NOT_USABLE_WITHIN_30_BELOW`].
    NotUsable,
    /// Under [`MIN_VERDICT_AREA_MM2`]. Guard 1 of the module doc.
    TooSmall,
    /// Under [`MIN_TRUSTED_TRIANGLES`] trusted triangles. Guard 2.
    NotMeasurable,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Usable => "USABLE",
            Self::Marginal => "marginal",
            Self::NotUsable => "not-usable",
            Self::TooSmall => "too-small",
            Self::NotMeasurable => "not-meas",
        }
    }
}

/// One zone's measurement.
struct ZoneStats {
    triangles: usize,
    area_mm2: f64,
    /// Area of triangles whose fit succeeded, whatever the anisotropy.
    fitted_area_mm2: f64,
    /// Area of triangles carrying a believed `t1`.
    trusted_area_mm2: f64,
    /// Fraction of TRIANGLES whose fit succeeded.
    fit_fraction: f64,
    /// Fraction of AREA that is fitted but degenerate. `t1` carries no meaning
    /// there. Floor stated at [`ISOTROPY_REL_TOL`] / [`ISOTROPY_ABS_TOL`].
    degenerate_fraction: f64,
    dominant: Option<[f64; 2]>,
    /// Coherent area at [`COHERENCE_ANGLES_DEG`], over TOTAL zone area.
    within: [f64; 4],
    /// The same, over trusted area only. A secondary column, so a turning
    /// field and an absent field stay separable.
    within_trusted: [f64; 4],
    /// Median coherence length (mm). Censored queries enter at the bound.
    coherence_length_mm: f64,
    /// Fraction of queries that found no turn inside the bound.
    censored_fraction: f64,
    queries: usize,
    /// XY bbox extent of the zone's centroids (mm). Printed beside the
    /// coherence length, so a censored read is legible.
    extent_mm: [f64; 2],
    verdict: Verdict,
}

/// Median of `values`, which this function sorts. `NaN` for an empty slice.
fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        0.5 * (values[mid - 1] + values[mid])
    }
}

/// Measure one zone. `members` are triangle indices into `field`.
fn census_zone(field: &[TriField], members: &[u32]) -> ZoneStats {
    let mut area = 0.0f64;
    let mut fitted_area = 0.0f64;
    let mut trusted_area = 0.0f64;
    let mut degenerate_area = 0.0f64;
    let mut fitted_count = 0usize;
    let mut trusted: Vec<(f64, [f64; 2])> = Vec::new();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &m in members {
        let entry = &field[m as usize];
        area += entry.area_mm2;
        min_x = min_x.min(entry.centroid.x);
        min_y = min_y.min(entry.centroid.y);
        max_x = max_x.max(entry.centroid.x);
        max_y = max_y.max(entry.centroid.y);
        if entry.fitted {
            fitted_count += 1;
            fitted_area += entry.area_mm2;
        }
        if entry.degenerate {
            degenerate_area += entry.area_mm2;
        }
        if let Some(axis) = entry.axis {
            trusted_area += entry.area_mm2;
            trusted.push((entry.area_mm2, axis));
        }
    }
    let dominant = dominant_axis(&trusted);

    let mut within = [0.0f64; 4];
    let mut within_trusted = [0.0f64; 4];
    if let Some(d) = dominant {
        let mut coherent = [0.0f64; 4];
        for &(w, a) in &trusted {
            let cos = axis_cos(a, d);
            let buckets = coherent.iter_mut().zip(COHERENCE_ANGLES_DEG.iter());
            for (bucket, &angle) in buckets {
                if cos >= angle.to_radians().cos() {
                    *bucket += w;
                }
            }
        }
        let slots = within.iter_mut().zip(within_trusted.iter_mut());
        for ((total, trusted_only), &c) in slots.zip(coherent.iter()) {
            *total = ratio(c, area);
            *trusted_only = ratio(c, trusted_area);
        }
    }

    // Coherence length. The queries are the zone's trusted triangles, capped
    // by a deterministic stride over triangle-index order. The SEARCH set is
    // every trusted triangle of the zone, never a subsample.
    let mut coherence_length = f64::NAN;
    let mut censored_fraction = f64::NAN;
    let mut queries = 0usize;
    if let Some(grid) = TurnGrid::build(field, members) {
        let cos_turn = COHERENCE_TURN_DEG.to_radians().cos();
        let stride = grid.pts.len().div_ceil(COHERENCE_QUERY_CAP).max(1);
        let picks: Vec<usize> = (0..grid.pts.len()).step_by(stride).collect();
        let found: Vec<Option<f64>> = picks
            .par_iter()
            .with_min_len(64)
            .map(|&i| grid.nearest_turn(i, cos_turn))
            .collect();
        queries = found.len();
        let censored = found.iter().filter(|d| d.is_none()).count();
        censored_fraction = censored as f64 / queries.max(1) as f64;
        let bound = COHERENCE_SEARCH_BOUND_MM;
        let distances: Vec<f64> = found.iter().map(|d| d.unwrap_or(bound)).collect();
        coherence_length = median(distances);
    }

    let extent_mm = if members.is_empty() {
        [0.0, 0.0]
    } else {
        [max_x - min_x, max_y - min_y]
    };
    let coherent_enough = within[VERDICT_ANGLE_INDEX] >= USABLE_WITHIN_30_MIN;
    let verdict = if queries == 0 {
        Verdict::NotMeasurable
    } else if area < MIN_VERDICT_AREA_MM2 {
        Verdict::TooSmall
    } else if coherent_enough && coherence_length >= USABLE_LENGTH_MIN_MM {
        Verdict::Usable
    } else if within[VERDICT_ANGLE_INDEX] < NOT_USABLE_WITHIN_30_BELOW {
        Verdict::NotUsable
    } else {
        Verdict::Marginal
    };

    ZoneStats {
        triangles: members.len(),
        area_mm2: area,
        fitted_area_mm2: fitted_area,
        trusted_area_mm2: trusted_area,
        fit_fraction: ratio(fitted_count as f64, members.len() as f64),
        degenerate_fraction: ratio(degenerate_area, area),
        dominant,
        within,
        within_trusted,
        coherence_length_mm: coherence_length,
        censored_fraction,
        queries,
        extent_mm,
        verdict,
    }
}

/// A zone plus its identity and its membership.
struct Zone {
    label: String,
    members: Vec<u32>,
    stats: ZoneStats,
}

/// Triangles of `field` whose XY centroid lies inside `poly`.
fn zone_members(field: &[TriField], poly: &Polygon2) -> Vec<u32> {
    (0..field.len() as u32)
        .into_par_iter()
        .filter(|&i| poly.contains_point(&field[i as usize].centroid))
        .collect()
}

// ════════════════════════════════════════════════════════════════════════
// Reporting
// ════════════════════════════════════════════════════════════════════════

/// The dominant direction as an angle from +X, in degrees, in `[0, 180)`.
fn dominant_deg(stats: &ZoneStats) -> f64 {
    match stats.dominant {
        Some(d) => {
            let deg = d[1].atan2(d[0]).to_degrees();
            if deg < 0.0 { deg + 180.0 } else { deg }
        }
        None => f64::NAN,
    }
}

fn print_header() {
    eprintln!(
        "  {:<26} {:>9} {:>7} {:>5} {:>5} {:>6} {:>6} {:>6} {:>6} {:>7} {:>5} {:>10}",
        "zone",
        "area mm2",
        "tris",
        "fit",
        "degen",
        "w10",
        "w20",
        "w30",
        "w45",
        "coh mm",
        "cens",
        "verdict"
    );
}

/// `>=` when more than half the queries censored, so the printed median IS the
/// search bound rather than a measured distance. Blank otherwise.
fn censor_mark(fraction: f64) -> &'static str {
    if fraction > 0.5 { ">=" } else { "  " }
}

fn print_row(label: &str, stats: &ZoneStats) {
    let mark = censor_mark(stats.censored_fraction);
    eprintln!(
        "  {:<26} {:>9.1} {:>7} {:>5.2} {:>5.2} {:>6.3} {:>6.3} {:>6.3} {:>6.3} \
         {mark}{:>5.2} {:>5.2} {:>10}",
        label,
        stats.area_mm2,
        stats.triangles,
        stats.fit_fraction,
        stats.degenerate_fraction,
        stats.within[0],
        stats.within[1],
        stats.within[2],
        stats.within[3],
        stats.coherence_length_mm,
        stats.censored_fraction,
        stats.verdict.label()
    );
    eprintln!(
        "  {:<26} dominant {:>6.1} deg, extent {:.1} x {:.1} mm, trusted area {:.1} mm2, \
         fitted area {:.1} mm2, w30 over trusted {:.3}, {} queries",
        "",
        dominant_deg(stats),
        stats.extent_mm[0],
        stats.extent_mm[1],
        stats.trusted_area_mm2,
        stats.fitted_area_mm2,
        stats.within_trusted[2],
        stats.queries
    );
}

/// Print one source's rows and its rollup. Returns the union area (mm²) of its
/// usable zones, so the caller can total across sources.
fn report_source(name: &str, zones: &mut [Zone], field: &[TriField], mesh_area: f64) -> f64 {
    zones.sort_by(|a, b| b.stats.area_mm2.total_cmp(&a.stats.area_mm2));
    eprintln!();
    eprintln!("── {name} — {} zones ──", zones.len());
    print_header();
    for zone in zones.iter().take(MAX_ROWS_PER_SOURCE) {
        print_row(&zone.label, &zone.stats);
    }
    if zones.len() > MAX_ROWS_PER_SOURCE {
        let rest = &zones[MAX_ROWS_PER_SOURCE..];
        let rest_area: f64 = rest.iter().map(|z| z.stats.area_mm2).sum();
        eprintln!(
            "  ... {} further zones, {:.1} mm2 total, largest {:.1} mm2",
            rest.len(),
            rest_area,
            rest.first().map_or(0.0, |z| z.stats.area_mm2)
        );
    }

    let mut counts = [0usize; 5];
    for zone in zones.iter() {
        let slot = match zone.stats.verdict {
            Verdict::Usable => 0,
            Verdict::Marginal => 1,
            Verdict::NotUsable => 2,
            Verdict::TooSmall => 3,
            Verdict::NotMeasurable => 4,
        };
        counts[slot] += 1;
    }

    // Union area, not the sum. Slope bands and tier machining sets are dilated
    // by overlap_mm = 2.0, so neighbouring zones share their seams.
    let mut usable_marked = vec![false; field.len()];
    let mut usable_union = 0.0f64;
    let mut usable_sum = 0.0f64;
    let mut all_marked = vec![false; field.len()];
    let mut all_union = 0.0f64;
    let mut all_sum = 0.0f64;
    for zone in zones.iter() {
        let usable = zone.stats.verdict == Verdict::Usable;
        all_sum += zone.stats.area_mm2;
        if usable {
            usable_sum += zone.stats.area_mm2;
        }
        for &m in &zone.members {
            let slot = m as usize;
            if !all_marked[slot] {
                all_marked[slot] = true;
                all_union += field[slot].area_mm2;
            }
            if usable && !usable_marked[slot] {
                usable_marked[slot] = true;
                usable_union += field[slot].area_mm2;
            }
        }
    }

    let w30: Vec<f64> = zones.iter().map(|z| z.stats.within[2]).collect();
    eprintln!(
        "  ROLLUP {name}: usable {} / marginal {} / not-usable {} / too-small {} / \
         not-measurable {}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    eprintln!(
        "  ROLLUP {name}: median w30 across zones {:.3}; source covers {:.1} mm2 union \
         ({:.1} mm2 summed, {:.1}% of the mesh's {:.1} mm2)",
        median(w30),
        all_union,
        all_sum,
        100.0 * ratio(all_union, mesh_area),
        mesh_area
    );
    eprintln!(
        "  ROLLUP {name}: USABLE zones cover {:.1} mm2 union ({:.1} mm2 summed) = \
         {:.2}% of the mesh, {:.2}% of this source's own area",
        usable_union,
        usable_sum,
        100.0 * ratio(usable_union, mesh_area),
        100.0 * ratio(usable_union, all_union)
    );
    usable_union
}

// ════════════════════════════════════════════════════════════════════════
// The zone sources
// ════════════════════════════════════════════════════════════════════════

/// Everything the frozen chain produces that this census consumes.
struct Decomposition {
    /// One entry per tier: `(tier, owned polygons)`.
    tier_owned: Vec<(u8, Vec<Polygon2>)>,
    /// One entry per planned region: `(band, polygon)`.
    bands: Vec<(FinishBand, Polygon2)>,
    classification_cell_mm: f64,
}

/// Run the frozen derivation chain. `Err` is a SKIP reason, not a failure.
///
/// Restated from `tests/wanaka_region_capture_f1.rs::derive_region_1`, with
/// the artifact serialisation removed and the tier loop widened from tier 1 to
/// every tier. The chain itself is unchanged.
fn derive(mesh: &TriangleMesh, index: &SpatialIndex) -> Result<Decomposition, &'static str> {
    let r15 = TaperedBallEndmill::new(R15_TUPLE[0], R15_TUPLE[1], R15_TUPLE[2], R15_TUPLE[3]);
    let r10 = TaperedBallEndmill::new(R10_TUPLE[0], R10_TUPLE[1], R10_TUPLE[2], R10_TUPLE[3]);
    let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
    let ladder = TierLadder::new(&tools).expect("ladder");
    let never_cancel = || false;
    let map = compute_tier_map(
        mesh,
        index,
        &ladder,
        &TierMapParams {
            cell_mm: CELL_MM,
            tolerance_mm: TOLERANCE_MM,
            margin_mm: MARGIN_MM,
            treatment: ResidualTreatment::SlopeCompensated,
        },
        &never_cancel,
    )
    .expect("tier map");
    let cusp_radii: Vec<f64> = tools.iter().map(|tool| tool.cusp_radius_mm()).collect();
    let islands = extract_tier_islands(
        &map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let tier_owned: Vec<(u8, Vec<Polygon2>)> = islands
        .per_tier
        .iter()
        .map(|set| (set.tier, set.owned.as_slice().to_vec()))
        .collect();
    let Some(fine) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        return Err("no tier 1 machining region");
    };
    if fine.machining.is_empty() {
        return Err("tier 1 machining region is empty");
    }

    let resolution = unified_finish_classification_resolution(&r10, OP_TOLERANCE_MM);
    let surface = build_classification_surface_with_sampler_and_cancel(
        mesh,
        index,
        &r10,
        resolution,
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let heightmap = &surface.heightmap;
    let covered: Vec<bool> = heightmap
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &covered)| {
            if !covered {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let bands: Vec<(FinishBand, Polygon2)> = planned
        .regions
        .iter()
        .map(|region| (region.band, region.polygon.clone()))
        .collect();

    Ok(Decomposition {
        tier_owned,
        bands,
        classification_cell_mm: resolution.cell_mm(),
    })
}

/// Short name for a band, for the row labels.
fn band_name(band: FinishBand) -> &'static str {
    match band {
        FinishBand::Shallow => "shallow",
        FinishBand::MidSteep => "midsteep",
        FinishBand::VerySteep => "verysteep",
    }
}

/// Cut `base`'s triangles into `size` mm square tiles on an absolute lattice.
/// Empty tiles are dropped. The lattice is anchored at the multiple of `size`
/// below the base's minimum, so the tiling is deterministic and independent of
/// the member order.
fn tile_zones(field: &[TriField], base: &[u32], size: f64) -> Vec<Vec<u32>> {
    if base.is_empty() || size <= 0.0 {
        return Vec::new();
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &m in base {
        let c = field[m as usize].centroid;
        min_x = min_x.min(c.x);
        min_y = min_y.min(c.y);
        max_x = max_x.max(c.x);
        max_y = max_y.max(c.y);
    }
    let x0 = (min_x / size).floor() * size;
    let y0 = (min_y / size).floor() * size;
    let cols = (((max_x - x0) / size).floor() as i64 + 1).max(1);
    let rows = (((max_y - y0) / size).floor() as i64 + 1).max(1);
    let mut tiles: Vec<Vec<u32>> = vec![Vec::new(); (cols * rows) as usize];
    for &m in base {
        let c = field[m as usize].centroid;
        let col = (((c.x - x0) / size).floor() as i64).clamp(0, cols - 1);
        let row = (((c.y - y0) / size).floor() as i64).clamp(0, rows - 1);
        tiles[(row * cols + col) as usize].push(m);
    }
    tiles.retain(|t| !t.is_empty());
    tiles
}

// ════════════════════════════════════════════════════════════════════════
// The census run
// ════════════════════════════════════════════════════════════════════════

/// **The census.** Measures direction coherence of the decompositions the
/// product already generates. It generates no toolpath.
///
/// `#[ignore]` because it needs the operator's wanaka mesh, which is not in
/// the repo. It SKIPs rather than fails when the mesh is absent, following
/// `tests/wanaka_region_capture_f1.rs`.
#[test]
#[ignore = "census — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_zone_coherence_census() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let mesh = match TriangleMesh::from_stl_scaled(path, 1.0) {
        Ok(mesh) => mesh,
        Err(err) => {
            eprintln!("SKIP: cannot load {WANAKA_MESH}: {err:?}");
            return;
        }
    };
    if mesh.triangles.len() != EXPECTED_TRIANGLES {
        eprintln!(
            "WARNING: {} triangles, expected {EXPECTED_TRIANGLES}. The mesh has been \
             re-exported. Every comparison below against §F1-2 is then off its own scale.",
            mesh.triangles.len()
        );
    }
    let index = SpatialIndex::build_auto(&mesh);

    eprintln!("════ zone coherence census ════");
    eprintln!(
        "estimator: Monge quadric, fit radius {FIT_RADIUS_MM} mm, curvature from the \
         shape operator, t1 = eigenvector of the smaller Monge eigenvalue."
    );
    eprintln!(
        "degenerate floor: |k1 - k2| <= max({ISOTROPY_ABS_TOL:e}, \
         {ISOTROPY_REL_TOL} * max(|k1|, |k2|)) — §F1-2's own floor, restated."
    );
    eprintln!(
        "bars: USABLE needs w30 >= {USABLE_WITHIN_30_MIN} AND coherence length >= \
         {USABLE_LENGTH_MIN_STEPOVERS} stepovers ({USABLE_LENGTH_MIN_MM:.3} mm). \
         NOT-USABLE is w30 < {NOT_USABLE_WITHIN_30_BELOW}. Below \
         {MIN_VERDICT_AREA_MM2:.2} mm2 a zone is TOO-SMALL and never usable."
    );
    eprintln!(
        "coherence length: nearest same-zone trusted triangle turning more than \
         {COHERENCE_TURN_DEG} deg, search bound {COHERENCE_SEARCH_BOUND_MM:.3} mm, \
         at most {COHERENCE_QUERY_CAP} queries per zone by deterministic stride."
    );
    eprintln!(
        "MONOTONE CELLS ARE NOT CENSUSED. They live in \
         tests/thin_organic_island_widths.rs, which this work must not touch and \
         cannot import. The equal-square tile control below stands in their place: \
         it isolates whether a zone becomes coherent simply by being small."
    );

    let (field, census) = build_field(&mesh, &index);
    let mesh_area: f64 = field.iter().map(|f| f.area_mm2).sum();
    let trusted_area: f64 = field
        .iter()
        .filter(|f| f.axis.is_some())
        .map(|f| f.area_mm2)
        .sum();
    eprintln!();
    eprintln!(
        "field: {} triangles, {:.1} mm2 surface. fitted {} / under-determined {} / \
         ill-conditioned {} / umbilic {} / below isotropy floor {}.",
        field.len(),
        mesh_area,
        census.fitted,
        census.under_determined,
        census.ill_conditioned,
        census.umbilic,
        census.below_isotropy_floor
    );
    eprintln!(
        "field: {:.1}% of surface area carries a believed t1.",
        100.0 * ratio(trusted_area, mesh_area)
    );

    let decomposition = match derive(&mesh, &index) {
        Ok(value) => value,
        Err(reason) => {
            eprintln!("SKIP: {reason}.");
            return;
        }
    };
    eprintln!(
        "chain: classification cell {:.6} mm, {} planned regions, {} tiers.",
        decomposition.classification_cell_mm,
        decomposition.bands.len(),
        decomposition.tier_owned.len()
    );

    // ── source 1: tier islands ──────────────────────────────────────────
    let mut tier_zones: Vec<Zone> = Vec::new();
    for (tier, polys) in &decomposition.tier_owned {
        for (i, poly) in polys.iter().enumerate() {
            let members = zone_members(&field, poly);
            if members.is_empty() {
                continue;
            }
            let stats = census_zone(&field, &members);
            tier_zones.push(Zone {
                label: format!("tier {tier} island {i}"),
                members,
                stats,
            });
        }
    }
    let tier_usable = report_source("tier islands (owned)", &mut tier_zones, &field, mesh_area);

    // ── source 2: slope bands ───────────────────────────────────────────
    //
    // Region 1 is picked by the FROZEN rule: Shallow band, largest
    // `Polygon2::area()`, first. That is `wanaka_region_capture_f1.rs`'s
    // `SELECTION_RULE`, restated. It is the polygon's XY area, NOT this
    // file's summed 3-D triangle area, so that region 1 here is the same
    // object §F1-2 measured.
    let mut band_zones: Vec<Zone> = Vec::new();
    let mut region1: Option<Vec<u32>> = None;
    let mut region1_poly_area = f64::NEG_INFINITY;
    for (i, (band, poly)) in decomposition.bands.iter().enumerate() {
        let members = zone_members(&field, poly);
        if members.is_empty() {
            continue;
        }
        let stats = census_zone(&field, &members);
        if *band == FinishBand::Shallow && poly.area() > region1_poly_area {
            region1_poly_area = poly.area();
            region1 = Some(members.clone());
        }
        band_zones.push(Zone {
            label: format!("{} region {i}", band_name(*band)),
            members,
            stats,
        });
    }
    let band_usable = report_source("slope bands", &mut band_zones, &field, mesh_area);

    // Per-band rollup, because the prediction was made per band.
    for band in [
        FinishBand::Shallow,
        FinishBand::MidSteep,
        FinishBand::VerySteep,
    ] {
        let rows: Vec<&Zone> = band_zones
            .iter()
            .filter(|z| z.label.starts_with(band_name(band)))
            .collect();
        if rows.is_empty() {
            eprintln!("  BAND {}: no regions.", band_name(band));
            continue;
        }
        let area: f64 = rows.iter().map(|z| z.stats.area_mm2).sum();
        let usable = rows
            .iter()
            .filter(|z| z.stats.verdict == Verdict::Usable)
            .count();
        let w30: Vec<f64> = rows.iter().map(|z| z.stats.within[2]).collect();
        let lengths: Vec<f64> = rows.iter().map(|z| z.stats.coherence_length_mm).collect();
        eprintln!(
            "  BAND {}: {} regions, {:.1} mm2 summed ({:.1}% of mesh), median w30 {:.3}, \
             median coherence {:.2} mm, {usable} usable",
            band_name(band),
            rows.len(),
            area,
            100.0 * ratio(area, mesh_area),
            median(w30),
            median(lengths)
        );
    }

    // ── source 3: the size control ──────────────────────────────────────
    let Some(base) = region1 else {
        eprintln!("SKIP tile control: no Shallow region to tile.");
        return;
    };
    let region1_stats = census_zone(&field, &base);
    eprintln!();
    eprintln!(
        "── size control: the largest Shallow region (region 1, polygon area {:.1} mm2, \
         surface area {:.1} mm2, {} triangles) cut into equal square tiles ──",
        region1_poly_area,
        region1_stats.area_mm2,
        base.len()
    );
    eprintln!(
        "  §F1-2 anchor: a 45 deg coherence gate cut this region into \
         {F1_2_PATCHES_AT_45} patches, {F1_2_SLIVERS_AT_45} of them under 1 mm2. At \
         10 deg, {:.1}% of its area lay in patches under 1 mm2. Its measured prize \
         ceiling is +{F1_2_REGION1_PRIZE_CEILING_PCT}%.",
        100.0 * F1_2_SLIVER_AREA_FRACTION_AT_10
    );
    print_header();
    print_row("region 1 (undivided)", &region1_stats);

    for &size in &TILE_SIZES_MM {
        let tiles = tile_zones(&field, &base, size);
        let mut zones: Vec<Zone> = Vec::new();
        for (i, members) in tiles.into_iter().enumerate() {
            let stats = census_zone(&field, &members);
            zones.push(Zone {
                label: format!("tile {size:.0}mm #{i}"),
                members,
                stats,
            });
        }
        if zones.is_empty() {
            eprintln!("  tiles {size:.0} mm: none.");
            continue;
        }
        let total: f64 = zones.iter().map(|z| z.stats.area_mm2).sum();
        let usable_area: f64 = zones
            .iter()
            .filter(|z| z.stats.verdict == Verdict::Usable)
            .map(|z| z.stats.area_mm2)
            .sum();
        let over_bar: f64 = zones
            .iter()
            .filter(|z| z.stats.within[2] >= USABLE_WITHIN_30_MIN)
            .map(|z| z.stats.area_mm2)
            .sum();
        // Clears the coherence bar AND is big enough to be judged. The middle
        // figure. It separates "disqualified by guard 1" from "measured
        // incoherent", which the usable figure alone cannot do: a tile below
        // MIN_VERDICT_AREA_MM2 is TooSmall whatever its coherence, so the
        // 4 mm row's usable figure is zero by construction, not by
        // measurement.
        let over_bar_eligible: f64 = zones
            .iter()
            .filter(|z| {
                z.stats.within[2] >= USABLE_WITHIN_30_MIN
                    && z.stats.area_mm2 >= MIN_VERDICT_AREA_MM2
            })
            .map(|z| z.stats.area_mm2)
            .sum();
        let eligible = zones
            .iter()
            .filter(|z| z.stats.area_mm2 >= MIN_VERDICT_AREA_MM2)
            .count();
        let mut weighted = 0.0f64;
        for zone in &zones {
            weighted += zone.stats.within[2] * zone.stats.area_mm2;
        }
        let w30: Vec<f64> = zones.iter().map(|z| z.stats.within[2]).collect();
        let lengths: Vec<f64> = zones.iter().map(|z| z.stats.coherence_length_mm).collect();
        eprintln!(
            "  tiles {size:>4.0} mm: {} tiles ({eligible} above the \
             {MIN_VERDICT_AREA_MM2:.1} mm2 bar), median w30 {:.3}, area-weighted w30 \
             {:.3}, median coherence {:.2} mm",
            zones.len(),
            median(w30),
            ratio(weighted, total),
            median(lengths)
        );
        // Three figures, because two would conflate the guard with the
        // measurement. A tile of edge `size` has area `size^2`; when that is
        // under MIN_VERDICT_AREA_MM2 the third figure is zero BY GUARD 1 and
        // says nothing about the surface. The second figure is the one to
        // read across sizes.
        eprintln!(
            "  tiles {size:>4.0} mm: {:.1}% of the region lies in tiles clearing w30 >= \
             {USABLE_WITHIN_30_MIN}; {:.1}% in tiles that ALSO clear the \
             {MIN_VERDICT_AREA_MM2:.1} mm2 area bar; {:.1}% in tiles that are USABLE \
             (area bar plus a {USABLE_LENGTH_MIN_MM:.3} mm coherence length)",
            100.0 * ratio(over_bar, total),
            100.0 * ratio(over_bar_eligible, total),
            100.0 * ratio(usable_area, total)
        );
        if size * size < MIN_VERDICT_AREA_MM2 {
            eprintln!(
                "  tiles {size:>4.0} mm: a full tile is {:.1} mm2, under the \
                 {MIN_VERDICT_AREA_MM2:.1} mm2 bar, so the second and third figures \
                 above are zero BY GUARD 1 and are not measurements of the surface. \
                 Read the first figure on this row.",
                size * size
            );
        }
    }

    // ── the answer ──────────────────────────────────────────────────────
    eprintln!();
    eprintln!(
        "ANSWER: usable zones cover {:.1} mm2 from the tier islands and {:.1} mm2 from \
         the slope bands, against a {:.1} mm2 surface — {:.2}% and {:.2}%.",
        tier_usable,
        band_usable,
        mesh_area,
        100.0 * ratio(tier_usable, mesh_area),
        100.0 * ratio(band_usable, mesh_area)
    );
    eprintln!(
        "That second number decides whether a usable subset is worth routing to a \
         different strategy."
    );
}

// ════════════════════════════════════════════════════════════════════════
// Self-check — the only part a reviewer can check without the mesh
// ════════════════════════════════════════════════════════════════════════

/// Tessellate `z = f(x, y)` over `[-half, half]^2` at `step`, two triangles
/// per cell. Deterministic. Restated from `wanaka_curvature_anisotropy.rs`.
fn synthetic_patch(half: f64, step: f64, height: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let n = ((2.0 * half) / step).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for row in 0..n {
        for col in 0..n {
            let x = -half + col as f64 * step;
            let y = -half + row as f64 * step;
            vertices.push(P3::new(x, y, height(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (n - 1) * (n - 1));
    for row in 0..(n - 1) {
        for col in 0..(n - 1) {
            let a = (row * n + col) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Build the field over a synthetic patch and census the triangles `pick`
/// accepts, by XY centroid. Runs the real [`build_field`] and [`census_zone`],
/// not a re-derivation.
fn census_synthetic(mesh: &TriangleMesh, pick: impl Fn(P2) -> bool) -> ZoneStats {
    let index = SpatialIndex::build_auto(mesh);
    let (field, _) = build_field(mesh, &index);
    let members: Vec<u32> = (0..field.len() as u32)
        .filter(|&i| pick(field[i as usize].centroid))
        .collect();
    assert!(
        !members.is_empty(),
        "the synthetic zone selected no triangles"
    );
    census_zone(&field, &members)
}

/// **The self-check.**
///
/// Two patches whose direction field is known in closed form.
///
/// **Patch A, constant direction.** A cylinder ridge running along `y`:
/// `z = sqrt(rho^2 - x^2)`. Its principal curvatures are `kappa1 = 1/rho`
/// across the ridge and `kappa2 = 0` along the ruling, so `t1 = x` at every
/// point. The census must report full coherence, and a coherence length that
/// censors at the search bound, because the zone holds nothing that turns.
/// The anisotropy is 0.1 per mm against an isotropy floor of 0.01, a factor of
/// 10, so no triangle is degenerate.
///
/// **Patch B, a known 90 degree turn.** A ring ridge
/// `z = c exp(-((r - r0)/w)^2 / 2)` with `c = 1.0`, `w = 1.5`, `r0 = 5.0`. On
/// a surface of revolution the principal directions are exactly radial and
/// circumferential. At the crest the radial curvature is `c/w^2 = 0.444` per
/// mm and the circumferential curvature is 0, so `t1` is radial. The zone is
/// the quadrant `|r - r0| <= 1.1`, `x >= 0`, `y >= 0`, over which the radial
/// direction turns exactly 90 degrees. At the band edge the radial curvature
/// is still 0.157 per mm against a circumferential magnitude of at most
/// 0.090 per mm, a factor of 1.7, so `t1` never flips to circumferential
/// inside the zone and the turn stays a clean 90 degrees.
///
/// A radial field sweeping `[0, 90]` degrees uniformly puts its outer-product
/// mean at 45 degrees, and its coherence fraction at angle `A` is `A/45`
/// capped at 1: 0.222 at 10 degrees, 0.667 at 30 degrees, 1.0 at 45 degrees.
/// Its coherence length is the 30-degree chord, `2 r0 sin(15 deg)` = 2.59 mm.
///
/// Patch B is asserted from **both** sides. The upper bounds at 10 and 30
/// degrees say the turn was seen. The lower bound at 45 degrees says the field
/// was measured at all. A patch whose fits all failed would clear every upper
/// bound for the wrong reason, and the 45-degree assertion is what catches
/// that.
///
/// Both zones are inset from the patch edge by more than the fit radius, so no
/// reported fit is one-sided.
///
/// Patch B is about 17 mm², under [`MIN_VERDICT_AREA_MM2`]. That is by
/// construction: a 90 degree turn is only detectable at 30 degrees when it
/// happens over a short arc, and a short arc bounds the area. The assertions
/// are therefore on the two measured quantities. The verdict is asserted only
/// as "not usable", which both `TooSmall` and `NotUsable` satisfy.
#[test]
fn census_reports_known_coherence() {
    const RHO: f64 = 10.0;
    const RING_C: f64 = 1.0;
    const RING_W: f64 = 1.5;
    const RING_R0: f64 = 5.0;
    const RING_BAND: f64 = 1.1;

    // ── patch A: constant t1 = x ────────────────────────────────────────
    let cylinder = synthetic_patch(6.5, 0.2, |x, _| (RHO * RHO - x * x).sqrt());
    let a = census_synthetic(&cylinder, |p| p.x.abs() <= 5.0 && p.y.abs() <= 5.0);

    assert!(
        a.fit_fraction >= 0.99,
        "patch A: only {:.3} of triangles fitted. The estimator is starved, so nothing \
         below this is a coherence reading",
        a.fit_fraction
    );
    assert!(
        a.degenerate_fraction <= 0.01,
        "patch A: {:.3} of area read degenerate. A cylinder has |k1 - k2| = 1/rho = 0.1 \
         against a 0.01 floor and must not",
        a.degenerate_fraction
    );
    let a_dominant = a.dominant.expect("patch A has a dominant direction");
    assert!(
        axis_cos(a_dominant, [1.0, 0.0]) >= (2.0f64).to_radians().cos(),
        "patch A: dominant direction is {:.1} deg, expected 0 deg (t1 = x, across the \
         ridge)",
        dominant_deg(&a)
    );
    assert!(
        a.within[0] >= 0.99,
        "patch A: only {:.4} of area lies within 10 deg of the dominant direction. A \
         constant field must read 1.0",
        a.within[0]
    );
    assert!(
        a.within[2] >= 0.99 && a.within[3] >= 0.99,
        "patch A: w30 = {:.4}, w45 = {:.4}. Both must be 1.0 on a constant field",
        a.within[2],
        a.within[3]
    );
    assert!(
        a.censored_fraction >= 0.99,
        "patch A: {:.3} of queries found a turn over 30 deg. A constant field holds none",
        a.censored_fraction
    );
    assert!(
        (a.coherence_length_mm - COHERENCE_SEARCH_BOUND_MM).abs() < 1e-9,
        "patch A: coherence length {:.4} mm, expected the censored value \
         {COHERENCE_SEARCH_BOUND_MM:.4} mm",
        a.coherence_length_mm
    );
    assert_eq!(
        a.verdict,
        Verdict::Usable,
        "patch A is {:.1} mm2 with w30 = {:.3} and coherence length {:.3} mm, which \
         clears every pre-registered bar",
        a.area_mm2,
        a.within[2],
        a.coherence_length_mm
    );

    // ── patch B: a 90 degree turn ───────────────────────────────────────
    let ring = synthetic_patch(8.5, 0.2, |x, y| {
        let r = (x * x + y * y).sqrt();
        let t = (r - RING_R0) / RING_W;
        RING_C * (-0.5 * t * t).exp()
    });
    let b = census_synthetic(&ring, |p| {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        p.x >= 0.0 && p.y >= 0.0 && (r - RING_R0).abs() <= RING_BAND
    });

    assert!(
        b.fit_fraction >= 0.99,
        "patch B: only {:.3} of triangles fitted",
        b.fit_fraction
    );
    assert!(
        b.degenerate_fraction <= 0.15,
        "patch B: {:.3} of area read degenerate. The ring crest separates the two \
         curvatures by 1.7x or better across the whole band",
        b.degenerate_fraction
    );
    let b_dominant = b.dominant.expect("patch B has a dominant direction");
    let diagonal = [(0.5f64).sqrt(), (0.5f64).sqrt()];
    assert!(
        axis_cos(b_dominant, diagonal) >= (10.0f64).to_radians().cos(),
        "patch B: dominant direction is {:.1} deg, expected 45 deg by the symmetry of a \
         quadrant of radial directions",
        dominant_deg(&b)
    );
    assert!(
        b.within[0] <= 0.40,
        "patch B: {:.3} of area lies within 10 deg of the dominant direction. A uniform \
         90 deg sweep predicts 0.22, so a high value means the turn was not seen",
        b.within[0]
    );
    assert!(
        b.within[2] <= 0.75,
        "patch B: w30 = {:.3}. A uniform 90 deg sweep predicts 0.67 and the bar is 0.70, \
         so the census must not call this coherent",
        b.within[2]
    );
    // The positive half of the same claim. Without it, a patch whose field was
    // never measured at all would pass every bound above for the wrong reason.
    // A radial quadrant puts ALL of its area within 45 degrees of the
    // diagonal, so w45 must stay high while w10 and w30 fall.
    assert!(
        b.within[3] >= 0.80,
        "patch B: w45 = {:.3}. A radial quadrant predicts 1.0, so a low value means the \
         field was not measured rather than that the turn was seen",
        b.within[3]
    );
    assert!(
        b.coherence_length_mm <= 0.5 * COHERENCE_SEARCH_BOUND_MM,
        "patch B: coherence length {:.3} mm. The 30 deg chord at r = {RING_R0} is \
         {:.3} mm, so the turn must be found well inside half the {:.3} mm bound",
        b.coherence_length_mm,
        2.0 * RING_R0 * (0.5 * COHERENCE_TURN_DEG).to_radians().sin(),
        COHERENCE_SEARCH_BOUND_MM
    );
    assert!(
        b.coherence_length_mm < a.coherence_length_mm,
        "patch B's coherence length {:.3} mm must be shorter than patch A's {:.3} mm",
        b.coherence_length_mm,
        a.coherence_length_mm
    );
    assert_ne!(
        b.verdict,
        Verdict::Usable,
        "patch B turns 90 deg across {:.1} mm2 and must never be called usable",
        b.area_mm2
    );
}
