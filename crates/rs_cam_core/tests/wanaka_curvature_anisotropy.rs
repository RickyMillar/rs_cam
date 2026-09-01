//! **Does the direction-field method have anything to win on the operator's
//! terrain?** — a scale-controlled curvature-anisotropy measurement.
//!
//! # The question, in one line
//!
//! The Zou / Kumazawa preferred-feed-direction method picks, at each point, the
//! feed direction that maximises the machining strip width `W`. For a ball of
//! radius `R` at scallop `h`, the admissible stepover across a normal section
//! of curvature `κ_n` is Zou Eq. 2 with `r₁ = r₂ = R` (the paper's own best
//! case):
//!
//! ```text
//!     W(κ_n) = sqrt( 8h / (κ_n + 1/R) )            [convex-positive κ]
//! ```
//!
//! `κ_n` is the curvature **perpendicular** to the feed, so as the feed rotates
//! through the tangent plane `κ_n` sweeps Euler's range `[κ2, κ1]`. The method's
//! entire prize is therefore the per-point spread
//!
//! ```text
//!     W_max / W_min = sqrt( (κ1 + 1/R) / (κ2 + 1/R) )
//! ```
//!
//! and that spread scales with curvature **relative to `1/R`**, not with how
//! mountainous the landscape looks. At `R = 1 mm`, `1/R = 1.0 per mm`; a 20 mm
//! wide, 5 mm tall ridge has crest curvature ≈ 0.1 per mm — a tenth of it.
//! Kumazawa (UBC MASc 2012, Zou's [4]/[21]) measures 1.9 %–7.2 % against an
//! honest iso-scallop and states the precondition himself: the method "benefits
//! from surfaces that have a large number of features such as mounts and
//! valleys … where the difference between the maximum and minimum `W` are
//! notable". **This file measures whether that difference IS notable here.**
//!
//! Background: `planning/conformal_finish_2026-08-28/preferred_direction_field_research.md`
//! and `planning/finishing_synthesis_2026-08-30.md` §10.
//!
//! # The trap this instrument is built around
//!
//! The F1 run reported `k_s` up to 0.7 per mm on this mesh. The mesh's median
//! facet edge is **0.335 mm** (measured on the file, not quoted), and faceting
//! alone produces apparent curvature of that order. **A 1-ring curvature
//! estimate on this mesh probably measures the tessellation, not the
//! landscape.** This programme has been burned four times by fixtures that could
//! not express the quantity being measured; this instrument therefore measures
//! curvature at a **controlled spatial scale** and sweeps that scale from one
//! facet (0.3 mm) to landscape (8 mm), so that a fine-scale reading which
//! evaporates as the fit radius grows is *visible as noise* rather than
//! reported as terrain.
//!
//! # The estimator, stated in full
//!
//! ## Why a heightfield fit is legitimate here
//!
//! `terrain.stl` is **100 % up-facing** (every facet normal has `n_z > 0`;
//! measured min `n_z` = 0.0148) and spans x,y ∈ [0, 200] with z ∈ [−2.81, 7.00].
//! It is therefore a single-valued heightfield `z = f(x, y)`, and a Monge-patch
//! fit is not an approximation of a general surface fit — it is the same object.
//! That is simpler and far better conditioned than a general 3-D quadric with a
//! local frame, and it removes an entire class of frame-orientation bugs. **This
//! shortcut is valid for this mesh only**; a mesh with an overhang would need a
//! normal-aligned local frame.
//!
//! ## Step 1 — the fit
//!
//! At each sample `(x₀, y₀)` with surface height `z₀`, gather every mesh vertex
//! within XY distance `r` (the *fit radius*) via [`SpatialIndex`], and
//! least-squares fit, in the scale-normalised local frame
//! `u = (x − x₀)/r`, `v = (y − y₀)/r`, `w = z − z₀`:
//!
//! ```text
//!     w ≈ a u² + b uv + c v² + d u + e v + g
//! ```
//!
//! Normalising by `r` makes the 6×6 normal matrix scale-free, so its pivot
//! ratio is a meaningful conditioning number at every radius. Derivatives at
//! the sample point (`u = v = 0`) are then
//!
//! ```text
//!     f_x = d/r      f_y = e/r
//!     f_xx = 2a/r²   f_xy = b/r²   f_yy = 2c/r²
//! ```
//!
//! ## Step 2 — curvature from the shape operator, NOT the raw coefficients
//!
//! On slope the quadric's second derivatives are **not** the curvatures; the
//! metric matters. Monge fundamental forms with the upward unit normal
//! `n = (−f_x, −f_y, 1)/𝒲`, `𝒲 = sqrt(1 + f_x² + f_y²)`:
//!
//! ```text
//!     I  : E = 1 + f_x²    F = f_x f_y     G = 1 + f_y²      (EG − F² = 𝒲²)
//!     II : L = f_xx/𝒲      M = f_xy/𝒲      N = f_yy/𝒲
//! ```
//!
//! Shape operator `S = I⁻¹ II`; its invariants and eigenvalues:
//!
//! ```text
//!     K = (LN − M²)/(EG − F²)
//!     H = (E N − 2 F M + G L) / (2 (EG − F²))
//!     κ_monge = H ± sqrt(H² − K)
//! ```
//!
//! Worked check that this matters: on a sphere of radius ρ = 10, sampled at
//! (5, 0) where the slope is 30°, the raw reading `−f_xx = 0.15396` is 54 % high
//! while the shape operator returns exactly `0.1 = 1/ρ`. The non-ignored test
//! [`shape_operator_recovers_known_curvature`] pins that.
//!
//! ## Step 3 — the sign convention, enforced against the +Z normal
//!
//! The Monge form above is taken with the **upward** normal, in which a convex
//! dome reads *negative* (sphere cap `z = ρ − (x²+y²)/2ρ` gives
//! `L = N = −1/ρ`). Zou's Eq. 2 is **convex-positive**, and so is
//! `scallop_math`: `effective_radius` treats a positive curvature radius as
//! convex and shrinks the effective radius to `R·ρ/(R+ρ)`, i.e.
//! `1/R_eff = 1/R + 1/ρ`, which is Eq. 2's denominator with `κ = +1/ρ`. So this
//! file negates:
//!
//! ```text
//!     κ1 = −(H − sqrt(H² − K))     (max, "most convex")
//!     κ2 = −(H + sqrt(H² − K))     (min, "most concave")     κ1 ≥ κ2
//! ```
//!
//! A convex dome reads `+1/ρ`; a valley reads `−1/ρ`. `κ2 + 1/R ≤ 0` is then
//! exactly the **gouge** condition — a concavity tighter than the ball — and is
//! **counted, never clamped**.
//!
//! Consistency with the literature's rule: `W` is widest where `κ_perp` is
//! smallest, i.e. `κ_perp = κ2`, i.e. the feed runs along `t1` — "the most
//! convex principal direction". That is `D = t1`, the literature's own answer.
//!
//! ## Step 4 — normal curvature along an arbitrary fixed sweep direction
//!
//! For a raster sweeping along the XY direction `(p, q)`, the feed's tangent
//! vector is `p r_x + q r_y`; the in-tangent-plane perpendicular `n × t` has
//! parameter coordinates (derived from `I(t, w) = 0`, verified in
//! [`kappa_perp_zou`]'s comment):
//!
//! ```text
//!     (p′, q′) = ( −(F p + G q),  E p + F q )
//! ```
//!
//! and, by the second fundamental form's own quotient (Euler's formula, in
//! parameter coordinates so no principal frame is needed):
//!
//! ```text
//!     κ_perp = − (L p′² + 2M p′q′ + N q′²) / (E p′² + 2F p′q′ + G q′²)
//! ```
//!
//! (the leading minus is the same convex-positive flip as Step 3). This always
//! lands in `[κ2, κ1]`, which the synthetic test asserts.
//!
//! ## Step 5 — the prize bound, derived not asserted
//!
//! The cutting-distance floor with direction-optimal spacing is `∫dA/W_max`
//! (`finishing_synthesis_2026-08-30.md` §1; the same object as Kim 2001
//! Eq. 32's first term). With a **fixed** sweep direction it is
//! `∫dA/W(κ_perp along that direction)`. The ratio
//!
//! ```text
//!     bound(dir) = [∫ dA / W(κ_perp(dir))] / [∫ dA / W_max]   ≥ 1
//! ```
//!
//! is **the ceiling on what the direction field can win** — before any of the
//! four missing pipeline stages (degeneracy detection, classification,
//! separatrix tracing, segmentation) costs anything, before link/retract, and
//! before the field's own fragmentation. Kumazawa's measured margin over an
//! honest iso-scallop is 1.9 %–7.2 %; this number is directly comparable.
//!
//! Area element: the lattice is in XY, so `dA = 𝒲 · spacing²`.
//!
//! # Sampling scheme
//!
//! Deterministic regular XY lattices, never random.
//!
//! * **Whole up-facing surface** — 1.0 mm lattice on integer coordinates,
//!   inset [`EDGE_INSET_MM`] = 8.0 mm from the mesh XY bbox → 185 × 185 =
//!   **34 225** candidates.
//! * **Region 1** — 0.4 mm lattice over the captured boundary's bbox, kept when
//!   [`Polygon2::contains_point`] holds, same 8 mm inset → **≈ 18 800**
//!   candidates over its ~3104 mm².
//!
//! A candidate is *eligible* when a triangle of the mesh contains it in XY
//! (barycentric test against the single index cell — sound for a heightfield,
//! see [`SpatialIndex::cell_triangles_at`]'s own contract).
//!
//! **The 8 mm inset is a single fixed value, not per-radius, on purpose.** The
//! scale sweep's whole job is to compare radii; a per-radius inset would change
//! the population underneath the comparison. The cost is ~3 % of region 1 and
//! the outer 8 mm ring of the board, both reported.
//!
//! # Pre-registered thresholds — WRITTEN BEFORE THE NUMBERS
//!
//! ## The decision cell
//!
//! The verdict reads the **area-weighted median ratio** and the **prize bound**
//! at fit radius **r = 1.0 mm** — the smallest radius that is ≈3× the median
//! facet edge, hence the finest scale at which a quadric fit can be about the
//! landscape rather than the tessellation — for tool radii **R ∈ {1.0, 1.5}**,
//! the shipped multitool's two cusp radii
//! (`tests/wanaka_region_capture_f1.rs`: R1.5 tier 0, R1.0 tier 1). Every other
//! (r, R) cell is reported as context and decides nothing. Naming the cell in
//! advance is what stops this being a garden of forking paths.
//!
//! ## The prize verdict
//!
//! | area-weighted median `W_max/W_min` | verdict |
//! |---|---|
//! | ≲ **1.05** | prize is under ~2 % — **close the method on evidence** |
//! | **1.05 – 1.25** | prize inside the literature's own 1.9–7.2 % band — the four missing stages are a bounded but real piece of work |
//! | > **1.25** | a **larger** prize than the literature reports, which would itself need explaining before it is believed |
//!
//! ## The scale verdict
//!
//! Let `excess(r) = median_ratio(r) − 1`. Let `r_fine` be the **smallest** fit
//! radius whose valid-fit fraction is ≥ [`SCALE_RULE_MIN_VALID_FRACTION`]
//! (0.80) — a radius at which most fits are under-determined has no reading to
//! compare. If
//!
//! ```text
//!     excess(r_fine) ≥ 2.0 × max{ excess(r) : r ≥ 2.0 mm }
//! ```
//!
//! then **the fine-scale reading is tessellation, not terrain**, and every
//! earlier curvature number on this mesh — including F1's `|V|` range of
//! 0.007–0.461 — is suspect and must not be cited without a stated fit scale.
//!
//! ## The scale verdict's sharper half — the curvature decay exponent
//!
//! The ratio rule above is a blunt instrument, because *both* tessellation and
//! genuine fine structure make the anisotropy fall as `r` grows. The quantity
//! that separates them is the **log-log decay exponent** of the curvature
//! itself. Fit `κ1_p50 ∝ r^(−p)` between consecutive radii:
//!
//! ```text
//!     p = ln( κ1_p50(r_i) / κ1_p50(r_{i+1}) ) / ln( r_{i+1} / r_i )
//! ```
//!
//! Curvature at scale `r` is `≈ h(r)/r²`, where `h(r)` is the height variation
//! over that scale. Hence:
//!
//! * **Uncorrelated vertex displacement** — i.e. faceting noise — has `h`
//!   independent of `r`, so `p ≥ 2`. This is the tessellation signature.
//!   `p = 2` is the *no-averaging bound*: a least-squares fit additionally
//!   averages over `N ∝ r²` vertices, which suppresses iid noise faster still,
//!   so under **this** estimator pure faceting can read as high as `p ≈ 3`.
//!   A measured `p = 2.6` is therefore the same finding as `p = 2.0`, not a
//!   third category.
//! * A **self-affine landscape** of Hurst exponent `H` has `h(r) ∝ r^H`, so
//!   `p = 2 − H`. Real relief runs `H ≈ 0.5–1`, i.e. `p ≈ 1.0–1.5`.
//! * A **fully resolved smooth feature** gives `p → 0` while `r` is well inside
//!   it.
//!
//! Pre-registered: `p ≥ ` [`TESSELLATION_DECAY_EXPONENT`] (1.8) on the finest
//! supported pair ⇒ that pair is measuring faceting; `p ≤ 1.5` ⇒ it is
//! measuring landscape. The implied `H = 2 − p` is printed alongside. Note that
//! `p` also rises again at the coarse end for a legitimate reason — smoothing
//! past every feature of a bounded-relief surface — so the exponent is
//! diagnostic **at the fine end only**.
//!
//! # Prediction — stated so the run can refute it
//!
//! From the geometry alone (200 × 200 mm, 9.8 mm total relief, a river-map
//! relief with no single dominant feature wavelength) plus a Gaussian-smoothed
//! finite-difference preview of this same STL, taken on a 0.5 mm bin at
//! σ ≈ r/2:
//!
//! * **r = 0.3 mm: mostly UNDER-DETERMINED.** Vertex density is ≈ 8.3/mm², so a
//!   0.3 mm disc holds ≈ 2.3 vertices against 6 quadric parameters. Expect
//!   ≥ 90 % of samples skipped, and read that as *"the mesh cannot support a
//!   quadric at facet scale"* — a finding, not a failure. `r = 0.6` (≈ 9.3
//!   vertices) should be partly supported; `r ≥ 1.0` (≈ 26) fully.
//! * **r = 1.0 mm, R = 1.0**: median ratio **1.08 – 1.18**, prize bound
//!   **1.05 – 1.11**. R = 1.5 slightly higher.
//! * **r = 1.0 mm, R = 3.0**: bound **1.15 – 1.30** — but ~8 % of area is
//!   gouge-censored, so that figure is computed only where the ball fits.
//! * **r ≥ 4 mm**: bound **< 1.02**, ratio median < 1.04. The anisotropy is
//!   almost entirely sub-2 mm structure.
//! * **Prize verdict predicted: the middle band** — 2–7 %, the literature's own.
//! * **Scale rules predicted to SPLIT, and this is the interesting part.** The
//!   preview's exponents are `p ≈ 0.6` (0.3→0.6 mm) rising to `p ≈ 1.75`
//!   (4→8 mm), which says the fine end is *landscape*, not faceting — so the
//!   excess rule is predicted **NOT** to fire (excess(1.0) ≈ 0.16 against
//!   2 × excess(2.0) ≈ 0.19 — marginal), and `p` at the finest supported pair is
//!   predicted **below 1.5**.
//!   **But that preview binned vertices onto a 0.5 mm grid and averaged z**,
//!   which is itself a smoother that suppresses exactly the facet jitter in
//!   question. Its fine-end `p` is therefore a **lower bound**, and the real
//!   fit-on-raw-vertices instrument may well read `p ≈ 2` at 0.3→0.6. **That
//!   divergence is the single most informative thing this run can produce**: it
//!   is the difference between "F1's curvature was terrain" and "F1's curvature
//!   was the triangles".
//!
//! If the measured bound at the decision cell exceeds ~1.15, the prediction is
//! wrong and the reason must be found before the number is used.
//!
//! # Caveats a reader must not lose
//!
//! * **Fine-radius populations are censored toward channels.** Where a fit is
//!   rejected for want of points, it is rejected in the *sparse* parts of the
//!   mesh; the surviving fine-scale population is biased toward dense,
//!   high-curvature areas, i.e. biased **upward** in anisotropy. The
//!   valid-fit fraction printed on every row is what makes that visible.
//! * **Residual RMS grows with r by design.** A large-radius fit discards
//!   everything below its own scale; a rising residual is the instrument
//!   working, not a broken fit. A *small*-radius fit with a large residual, or
//!   a low pivot ratio, is the broken one.
//! * **`R = 3.0` statistics are gouge-censored** wherever the gouge fraction is
//!   non-trivial: the ratio and both integrals are taken only over samples
//!   where the ball fits, which is a systematically flatter subset.
//! * This instrument measures **premise**, not implementation. A small prize
//!   here does not prove the F1 arm's 6589-fragment failure was premise rather
//!   than implementation — it bounds how much a perfect implementation could
//!   ever have returned.
//! * **The decay verdict does not transfer to F1's estimator in either
//!   direction.** This file fits *vertices*, which lie on the source surface;
//!   F1's 1-ring Rusinkiewicz estimator reads *dihedral angles*, which at a
//!   0.335 mm facet edge are faceting almost by construction. A "landscape"
//!   verdict here does **not** validate F1's `|V|` numbers, and a
//!   "tessellation" verdict here condemns them a fortiori. Only the second
//!   inference is sound.
//! * **`MIN_FIT_POINTS = 7` bites the flattest mesh at r = 1.0.** The p90 facet
//!   edge is 0.658 mm, i.e. ≈ 2.2 vertices/mm², i.e. ≈ 6.9 vertices in a 1.0 mm
//!   disc — right on the floor. If the decision cell's valid fraction prints
//!   below ~95 %, the samples it dropped are the **flat** ones (ratio ≈ 1), so
//!   the median ratio and the prize bound are biased **upward**. That is the
//!   safe direction for a close-on-evidence verdict and the unsafe one for a
//!   middle-band verdict — say so if the verdict lands in the middle band.
//!
//! # Running it
//!
//! ```text
//! cargo test -p rs_cam_core --test wanaka_curvature_anisotropy \
//!   -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` because it needs the operator's wanaka mesh, which is not in the
//! repo — the same meaning `#[ignore]` carries everywhere else here. It SKIPs
//! (returns, never fails) when the mesh or the capture is absent. It asserts
//! nothing about the terrain: like `wanaka_region_capture_f1.rs`, an instrument
//! records what it finds and leaves the bar to a human.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{QueryScratch, SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;

// ── pinned inputs ───────────────────────────────────────────────────────

/// The operator's wanaka board. Absolute, outside the repo, by nature.
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

/// Triangle count the region-1 capture was cut against. Cross-checked and
/// **warned about**, never asserted — a re-exported mesh is a legitimate event.
const EXPECTED_TRIANGLES: usize = 661_212;

/// Fit radii (mm), one facet to landscape scale. Ascending — the scale rule
/// reads "smallest radius with enough valid fits" off this order.
const FIT_RADII_MM: [f64; 6] = [0.3, 0.6, 1.0, 2.0, 4.0, 8.0];

/// Ball radii (mm). The prize depends on the tool: R0.5/R1.0/R1.5 are the
/// finishing end of the shipped multitool ladder, R3.0 the coarse end.
const TOOL_RADII_MM: [f64; 4] = [0.5, 1.0, 1.5, 3.0];

/// Scallop constraint (mm) — the multitool's own cusp target.
const SCALLOP_H_MM: f64 = 0.03;

/// Whole-surface lattice spacing (mm).
const LATTICE_ALL_MM: f64 = 1.0;

/// Region-1 lattice spacing (mm). Finer, because region 1 is ~3104 mm² and a
/// 1 mm lattice would leave only ~3100 samples for area-weighted percentiles.
const LATTICE_REGION_MM: f64 = 0.4;

/// XY inset from the mesh bbox (mm). Equal to the largest fit radius, so no
/// reported fit is one-sided, and FIXED across radii so the scale sweep
/// compares the same population.
const EDGE_INSET_MM: f64 = 8.0;

/// Minimum gathered vertices for a fit: 6 quadric parameters + 1 degree of
/// freedom, so a residual exists at all.
const MIN_FIT_POINTS: usize = 7;

/// Pivot ratio (min |pivot| / max |pivot| during elimination on the
/// scale-normalised normal matrix) below which the fit is called rank
/// deficient rather than solved.
const MIN_PIVOT_RATIO: f64 = 1e-9;

/// The fit radius the verdict reads. ≈3× the median facet edge (0.335 mm).
const VERDICT_FIT_RADIUS_MM: f64 = 1.0;

/// The tool radii the verdict reads — the shipped multitool's two cusp radii.
const VERDICT_TOOL_RADII_MM: [f64; 2] = [1.0, 1.5];

/// Verdict band edges on the area-weighted median `W_max/W_min`.
const VERDICT_CLOSE_BELOW: f64 = rs_cam_core::metrology::census::PRIZE_CLOSE_BELOW;
/// Upper edge of the literature band.
const VERDICT_LITERATURE_ABOVE: f64 =
    rs_cam_core::metrology::census::PRIZE_ABOVE_LITERATURE;

/// A radius needs this valid-fit fraction before the scale rule will read it.
const SCALE_RULE_MIN_VALID_FRACTION: f64 = 0.80;

/// Radii at or above this are the "landscape" arm of the scale rule.
const SCALE_RULE_COARSE_FROM_MM: f64 = 2.0;

/// `excess(r_fine) ≥ this × max coarse excess` ⇒ tessellation verdict.
const TESSELLATION_FACTOR: f64 = 2.0;

/// Log-log decay exponent `p` in `κ1_p50 ∝ r^(−p)` at or above which the
/// fine-end reading is dominated by uncorrelated vertex displacement rather
/// than landscape. `p = 2` is the white-noise signature exactly (height
/// variation independent of scale, curvature `≈ h/r²`); a self-affine surface
/// of Hurst exponent `H` gives `p = 2 − H`, and real relief runs `H ≈ 0.5–1`.
const TESSELLATION_DECAY_EXPONENT: f64 = 1.8;

/// Below this `p`, the fine-end reading is called landscape outright.
const LANDSCAPE_DECAY_EXPONENT: f64 = 1.5;

/// Kumazawa's measured band against an honest iso-scallop, for comparison.
const KUMAZAWA_BAND_PCT: (f64, f64) = (1.9, 7.2);

// ── the quadric fit ─────────────────────────────────────────────────────

/// Per-thread reusable buffers. Held by `map_init`, so nothing here is
/// allocated per sample.
struct Scratch {
    query: QueryScratch,
    tris: Vec<usize>,
    /// Generation stamp per mesh vertex — dedups the triangle→vertex expansion
    /// without clearing a bitset per sample.
    stamp: Vec<u32>,
    generation: u32,
}

impl Scratch {
    fn new(vertex_count: usize) -> Self {
        Self {
            query: QueryScratch::default(),
            tris: Vec::new(),
            // Stamps start at 0 and `generation` is incremented *before* use,
            // so generation 1 is the first live value and 0 never matches.
            stamp: vec![0u32; vertex_count],
            generation: 0,
        }
    }
}

/// The local differential geometry at one sample, in the conventions of the
/// module doc. Everything downstream reads this and nothing re-derives it.
#[derive(Clone, Copy)]
struct Fit {
    /// Max principal curvature, **convex-positive** (Zou). `κ1 ≥ κ2`.
    kappa1: f64,
    /// Min principal curvature, convex-positive.
    kappa2: f64,
    /// First fundamental form.
    form_e: f64,
    form_f: f64,
    form_g: f64,
    /// Second fundamental form, Monge/upward-normal (so convex reads negative;
    /// [`kappa_perp_zou`] applies the flip).
    form_l: f64,
    form_m: f64,
    form_n: f64,
    /// `𝒲 = sqrt(1 + f_x² + f_y²)` — the XY→surface area magnification.
    area_weight: f64,
    /// RMS fit residual (mm).
    residual_rms: f64,
    /// Vertices that entered the fit.
    points: usize,
    /// RMS XY distance of those vertices from the sample — the fit's *effective*
    /// scale, which is not `r` when the neighbourhood is sparse or one-sided.
    gather_rms: f64,
    /// min|pivot| / max|pivot| on the scale-normalised normal matrix.
    pivot_ratio: f64,
}

impl Fit {
    /// Map onto the promoted estimator's type for the library kernels.
    /// `axis` is `None`: this instrument never derives `t1`, and the prize
    /// cell does not read it. `gather_rms` stays local.
    fn to_monge(&self) -> rs_cam_core::metrology::monge::MongeFit {
        rs_cam_core::metrology::monge::MongeFit {
            kappa1: self.kappa1,
            kappa2: self.kappa2,
            axis: None,
            form_e: self.form_e,
            form_f: self.form_f,
            form_g: self.form_g,
            form_l: self.form_l,
            form_m: self.form_m,
            form_n: self.form_n,
            area_weight: self.area_weight,
            residual_rms: self.residual_rms,
            points: self.points,
            pivot_ratio: self.pivot_ratio,
        }
    }
}

/// Why a sample produced no fit — counted separately, never silently dropped.
///
/// `Fit` is ~120 bytes, under `large_enum_variant`'s 200-byte threshold, so the
/// variant is carried inline rather than boxed.
enum Outcome {
    Fitted(Fit),
    /// Fewer than [`MIN_FIT_POINTS`] vertices in the disc; carries how many
    /// there were, because "2 vertices against 6 parameters" is the finding at
    /// facet scale, not merely a skip count.
    UnderDetermined(usize),
    /// Rank-deficient normal matrix (pivot ratio below [`MIN_PIVOT_RATIO`]).
    IllConditioned,
}

/// Height of the heightfield at `(x, y)`, or `None` when the point is off the
/// surface.
///
/// Sound because the mesh is a heightfield and `cell_triangles_at` is a
/// guaranteed superset of the triangles a vertical ray at `(x, y)` can pierce
/// (its own doc comment states that contract). `max` over the hits makes the
/// answer the topmost surface, which for a heightfield is the only surface.
fn surface_z(mesh: &TriangleMesh, index: &SpatialIndex, at: P2) -> Option<f64> {
    const BARY_EPS: f64 = 1e-9;
    let mut best: Option<f64> = None;
    for &t in index.cell_triangles_at(at.x, at.y) {
        let tri = mesh.triangles[t];
        let p0 = mesh.vertices[tri[0] as usize];
        let p1 = mesh.vertices[tri[1] as usize];
        let p2 = mesh.vertices[tri[2] as usize];
        let den = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
        if den.abs() < 1e-14 {
            continue;
        }
        let l0 = ((p1.y - p2.y) * (at.x - p2.x) + (p2.x - p1.x) * (at.y - p2.y)) / den;
        let l1 = ((p2.y - p0.y) * (at.x - p2.x) + (p0.x - p2.x) * (at.y - p2.y)) / den;
        let l2 = 1.0 - l0 - l1;
        if l0 < -BARY_EPS || l1 < -BARY_EPS || l2 < -BARY_EPS {
            continue;
        }
        let z = l0 * p0.z + l1 * p1.z + l2 * p2.z;
        best = Some(best.map_or(z, |b: f64| b.max(z)));
    }
    best
}

/// Solve the symmetric 6×6 system `A x = b` by Gauss-Jordan with partial
/// pivoting, returning `(x, min|pivot| / max|pivot|)`.
///
/// Hand-rolled rather than pulled from a linear-algebra crate for two reasons:
/// the pivot ratio is wanted as a *reported conditioning number* and a library
/// solve would not surrender it, and an integration test adding a dependency
/// edge for six rows is not worth the manifest churn. `A` is expected already
/// scale-normalised (see the module doc's `u = (x−x₀)/r`), so its pivots are
/// O(1) and their ratio is meaningful.
#[allow(clippy::needless_range_loop)] // Gauss-Jordan indexes three arrays by the same counter.
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
    let ratio = if pivot_max > 0.0 {
        pivot_min / pivot_max
    } else {
        0.0
    };
    Some((x, ratio))
}

/// Build [`Fit`] from the derivatives of the fitted heightfield — the Step 2/3
/// arithmetic of the module doc, and the ONLY place it happens.
fn fit_from_derivatives(
    (f_x, f_y): (f64, f64),
    (f_xx, f_xy, f_yy): (f64, f64, f64),
    residual_rms: f64,
    gather: (usize, f64, f64),
) -> Fit {
    let area_weight = (1.0 + f_x * f_x + f_y * f_y).sqrt();
    let form_e = 1.0 + f_x * f_x;
    let form_f = f_x * f_y;
    let form_g = 1.0 + f_y * f_y;
    let form_l = f_xx / area_weight;
    let form_m = f_xy / area_weight;
    let form_n = f_yy / area_weight;
    // EG − F² = 1 + f_x² + f_y² = 𝒲², always ≥ 1: no degenerate metric here.
    let det = form_e * form_g - form_f * form_f;
    let gauss = (form_l * form_n - form_m * form_m) / det;
    let mean = (form_e * form_n - 2.0 * form_f * form_m + form_g * form_l) / (2.0 * det);
    // H² − K = ((κa − κb)/2)² ≥ 0 exactly; the max() only absorbs round-off.
    let spread = (mean * mean - gauss).max(0.0).sqrt();
    let (points, gather_rms, pivot_ratio) = gather;
    Fit {
        // Convex-positive flip (module doc Step 3).
        kappa1: -(mean - spread),
        kappa2: -(mean + spread),
        form_e,
        form_f,
        form_g,
        form_l,
        form_m,
        form_n,
        area_weight,
        residual_rms,
        points,
        gather_rms,
        pivot_ratio,
    }
}

/// Mirror the accumulated upper triangle into the lower one and scale
/// everything by `inv = 1/count`, so the normal matrix is a *mean* of rank-1
/// terms with O(1) entries at every radius and its pivot ratio is comparable
/// across the sweep.
///
/// Row-major order means `normal[j][i]` for `j < i` has already been scaled by
/// the time it is copied, so the mirror never double-scales.
#[allow(clippy::needless_range_loop)] // symmetric mirror indexes both [i][j] and [j][i]
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

/// Fit the local quadric at `at` (surface height `z0`) over fit radius
/// `radius`. One streaming pass: nothing is stored per neighbour.
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
    let mut sum_w2 = 0.0f64;
    let mut sum_d2 = 0.0f64;
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
            let d2 = dx * dx + dy * dy;
            if d2 > r2 {
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
            sum_w2 += w * w;
            sum_d2 += d2;
            count += 1;
        }
    }
    scratch.tris = tris;

    if count < MIN_FIT_POINTS {
        return Outcome::UnderDetermined(count);
    }
    let inv = 1.0 / count as f64;
    finalise_normal_equations(&mut normal, &mut rhs, inv);
    let mean_w2 = sum_w2 * inv;
    let gather_rms = (sum_d2 * inv).sqrt();

    let Some((beta, pivot_ratio)) = solve_sym6(&normal, &rhs) else {
        return Outcome::IllConditioned;
    };
    if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO {
        return Outcome::IllConditioned;
    }
    if !beta.iter().all(|c| c.is_finite()) {
        return Outcome::IllConditioned;
    }

    // At the least-squares solution Aβ = b, so RSS/n = mean(w²) − β·b/n.
    // Exact, and it costs no second pass over the neighbourhood.
    let dot: f64 = beta.iter().zip(rhs.iter()).map(|(x, y)| x * y).sum();
    let residual_rms = (mean_w2 - dot).max(0.0).sqrt();

    let f_x = beta[3] / radius;
    let f_y = beta[4] / radius;
    let f_xx = 2.0 * beta[0] / (radius * radius);
    let f_xy = beta[1] / (radius * radius);
    let f_yy = 2.0 * beta[2] / (radius * radius);
    Outcome::Fitted(fit_from_derivatives(
        (f_x, f_y),
        (f_xx, f_xy, f_yy),
        residual_rms,
        (count, gather_rms, pivot_ratio),
    ))
}

// ── strip width and the fixed-direction curvature ───────────────────────

/// Normal curvature **perpendicular** to the XY sweep direction `(p, q)`,
/// convex-positive.
///
/// The perpendicular's parameter coordinates are `(p′, q′) = (−(Fp+Gq), Ep+Fq)`.
/// That is a genuine tangent-plane rotation, verified by expanding
/// `I(t, w) = E p p′ + F (p q′ + q p′) + G q q′`:
///
/// ```text
///   E p (−(Fp+Gq)) + F(p(Ep+Fq) + q(−(Fp+Gq))) + G q (Ep+Fq)
/// = −EFp² − EGpq + EFp² + F²pq − F²pq − FGq² + EGpq + FGq²  =  0
/// ```
///
/// so `w ⟂ t` in the surface metric. `κ_n` is a ratio of quadratic forms and is
/// therefore homogeneous — `(p′, q′)` need no normalisation.
fn kappa_perp_zou(fit: &Fit, dir: (f64, f64)) -> f64 {
    // PROMOTED (Track M): the arithmetic is
    // `metrology::monge::kappa_perp_zou`; the proof above stays here.
    rs_cam_core::metrology::monge::kappa_perp_zou(&fit.to_monge(), dir)
}

// PROMOTED (Track M): `metrology::monge::strip_width`.
use rs_cam_core::metrology::monge::strip_width;

// ── statistics ──────────────────────────────────────────────────────────

// PROMOTED (Track M): `metrology::monge::{Quantiles, quantiles,
// weighted_pick}`. The plain `median` below is NOT converted — see its
// comment.
use rs_cam_core::metrology::monge::{Quantiles, quantiles};

/// Plain (unweighted) median — used for diagnostic columns about the fits
/// themselves (point counts, residuals), which are properties of the estimator
/// rather than of the surface, and so are not area-weighted.
///
/// DISCLOSED DIVERGENCE (Track M, 2026-09-02): this copy returns the UPPER
/// middle element on an even population, where the promoted
/// `metrology::monge::median` averages the two middles. Converting would
/// move this instrument's printed diagnostic medians, so the copy stays,
/// stated. New measurements should use the library's.
fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

// ── reports ─────────────────────────────────────────────────────────────

// PROMOTED (Track M): `ToolReport` is `metrology::census::PrizeCell`.
use rs_cam_core::metrology::census::PrizeCell as ToolReport;

/// One (population, fit radius) row.
struct RadiusReport {
    radius_mm: f64,
    eligible: usize,
    fitted: usize,
    under_determined: usize,
    ill_conditioned: usize,
    median_points: f64,
    /// Median vertices found in the disc for the samples that were REJECTED as
    /// under-determined — the number that makes "the mesh cannot support a
    /// quadric at this scale" a measurement rather than an assertion.
    median_starved_points: f64,
    median_gather_rms: f64,
    median_residual_rms: f64,
    median_pivot_ratio: f64,
    kappa1: Quantiles,
    kappa2: Quantiles,
    tools: Vec<ToolReport>,
}

impl RadiusReport {
    fn valid_fraction(&self) -> f64 {
        if self.eligible == 0 {
            0.0
        } else {
            self.fitted as f64 / self.eligible as f64
        }
    }

    fn tool(&self, tool_radius: f64) -> Option<&ToolReport> {
        self.tools
            .iter()
            .find(|t| (t.tool_radius_mm - tool_radius).abs() < 1e-9)
    }
}

// ── the measurement ─────────────────────────────────────────────────────

/// Mesh + index, passed as one so no measurement function takes seven args.
struct MeshCtx<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
}

/// Regular lattice on multiples of `spacing`, inclusive of both ends.
///
/// Coordinates come from `index * spacing`, never from a running accumulator:
/// 0.4 is not representable in binary, and 475 accumulated additions of it
/// would put the lattice a few ULPs off the multiples the file claims — which
/// matters when the points are then fed to a ray-cast containment test.
fn lattice(x: (f64, f64), y: (f64, f64), spacing: f64) -> Vec<P2> {
    let first = |lo: f64| (lo / spacing).ceil() as i64;
    let last = |hi: f64| (hi / spacing + 1e-9).floor() as i64;
    let (ix0, ix1) = (first(x.0), last(x.1));
    let (iy0, iy1) = (first(y.0), last(y.1));
    let mut out = Vec::new();
    for iy in iy0..=iy1 {
        for ix in ix0..=ix1 {
            out.push(P2::new(ix as f64 * spacing, iy as f64 * spacing));
        }
    }
    out
}

/// Area-weighted-free PCA of the sample lattice's XY positions. Returns
/// `(unit direction, eigenvalue ratio λ_major/λ_minor)`; a ratio near 1 means
/// the population is isotropic and the "PCA axis" is arbitrary.
fn pca_axis(points: &[P2]) -> ((f64, f64), f64) {
    let n = points.len() as f64;
    if n < 2.0 {
        return ((1.0, 0.0), 1.0);
    }
    let mx = points.iter().map(|p| p.x).sum::<f64>() / n;
    let my = points.iter().map(|p| p.y).sum::<f64>() / n;
    let (mut cxx, mut cxy, mut cyy) = (0.0, 0.0, 0.0);
    for p in points {
        let dx = p.x - mx;
        let dy = p.y - my;
        cxx += dx * dx;
        cxy += dx * dy;
        cyy += dy * dy;
    }
    cxx /= n;
    cxy /= n;
    cyy /= n;
    let mean = 0.5 * (cxx + cyy);
    let spread = (0.25 * (cxx - cyy) * (cxx - cyy) + cxy * cxy).sqrt();
    let major = mean + spread;
    let minor = (mean - spread).max(1e-18);
    let theta = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
    ((theta.cos(), theta.sin()), major / minor)
}

/// Measure one population across the whole radius sweep.
fn measure(
    ctx: &MeshCtx<'_>,
    samples: &[(P2, f64)],
    spacing: f64,
    axis: (f64, f64),
) -> Vec<RadiusReport> {
    let cell_area = spacing * spacing;
    let mut reports = Vec::new();
    for &radius in &FIT_RADII_MM {
        // Collected into an order-stable Vec and reduced SERIALLY afterwards:
        // a parallel float reduction would make the printed numbers jitter
        // between runs, and an instrument whose output moves is not evidence.
        // `with_min_len` keeps rayon from splitting into tiny jobs: `map_init`
        // runs its initialiser once per sequential block, and this one
        // allocates a per-vertex stamp array.
        let outcomes: Vec<Outcome> = samples
            .par_iter()
            .with_min_len(512)
            .map_init(
                || Scratch::new(ctx.mesh.vertices.len()),
                |scratch, &sample| fit_quadric(ctx.mesh, ctx.index, scratch, sample, radius),
            )
            .collect();

        let mut fits: Vec<Fit> = Vec::new();
        let mut starved: Vec<f64> = Vec::new();
        let mut ill = 0usize;
        for outcome in outcomes {
            match outcome {
                Outcome::Fitted(fit) => fits.push(fit),
                Outcome::UnderDetermined(found) => starved.push(found as f64),
                Outcome::IllConditioned => ill += 1,
            }
        }
        let under = starved.len();

        let kappa1 = quantiles(
            fits.iter()
                .map(|f| (f.kappa1, f.area_weight * cell_area))
                .collect(),
        )
        .unwrap_or_default();
        let kappa2 = quantiles(
            fits.iter()
                .map(|f| (f.kappa2, f.area_weight * cell_area))
                .collect(),
        )
        .unwrap_or_default();

        let tools = TOOL_RADII_MM
            .iter()
            .map(|&tool_radius| tool_report(&fits, cell_area, tool_radius, axis))
            .collect();

        reports.push(RadiusReport {
            radius_mm: radius,
            eligible: samples.len(),
            fitted: fits.len(),
            under_determined: under,
            ill_conditioned: ill,
            median_points: median(fits.iter().map(|f| f.points as f64).collect()),
            median_starved_points: median(starved),
            median_gather_rms: median(fits.iter().map(|f| f.gather_rms).collect()),
            median_residual_rms: median(fits.iter().map(|f| f.residual_rms).collect()),
            median_pivot_ratio: median(fits.iter().map(|f| f.pivot_ratio).collect()),
            kappa1,
            kappa2,
            tools,
        });
    }
    reports
}

/// The per-tool cell — PROMOTED (Track M): the arithmetic is
/// `metrology::census::prize_cell`, extracted verbatim from this function.
/// This adapter maps the local `Fit` (which additionally carries
/// `gather_rms`, a diagnostic the library type does not) onto `MongeFit`.
fn tool_report(fits: &[Fit], cell_area: f64, tool_radius: f64, axis: (f64, f64)) -> ToolReport {
    let monge: Vec<rs_cam_core::metrology::monge::MongeFit> =
        fits.iter().map(Fit::to_monge).collect();
    rs_cam_core::metrology::census::prize_cell(&monge, cell_area, tool_radius, SCALLOP_H_MM, axis)
}

/// Log-log decay exponent `p` of the median `κ1` between two fit radii:
/// `p = ln(κ_lo / κ_hi) / ln(r_hi / r_lo)`. `None` when either median is
/// non-positive (a population whose median point is a saddle or a valley,
/// where the log is undefined) or the radii coincide.
fn decay_exponent(lo: &RadiusReport, hi: &RadiusReport) -> Option<f64> {
    let (k_lo, k_hi) = (lo.kappa1.p50, hi.kappa1.p50);
    if k_lo <= 0.0 || k_hi <= 0.0 || hi.radius_mm <= lo.radius_mm {
        return None;
    }
    let value = (k_lo / k_hi).ln() / (hi.radius_mm / lo.radius_mm).ln();
    value.is_finite().then_some(value)
}

// ── printing ────────────────────────────────────────────────────────────

fn print_population(name: &str, reports: &[RadiusReport], axis: (f64, f64), aniso: f64) {
    eprintln!();
    eprintln!("════════════════════════════════════════════════════════════════════════");
    eprintln!("POPULATION: {name}");
    eprintln!(
        "  PCA sweep axis {:.1}° (λ_major/λ_minor = {:.2}{})",
        axis.1.atan2(axis.0).to_degrees(),
        aniso,
        if aniso < 1.05 {
            " — ISOTROPIC, this axis is arbitrary"
        } else {
            ""
        }
    );
    eprintln!("════════════════════════════════════════════════════════════════════════");
    eprintln!();
    eprintln!("── fit quality by scale ────────────────────────────────────────────────");
    eprintln!(
        "  {:>5}  {:>8} {:>8} {:>8} {:>6} {:>6}  {:>6} {:>8} {:>9} {:>9}",
        "r_mm",
        "eligible",
        "fitted",
        "under",
        "starve",
        "ill",
        "pts",
        "gathRMS",
        "residRMS",
        "pivot"
    );
    for row in reports {
        eprintln!(
            "  {:>5.1}  {:>8} {:>8} {:>8} {:>6.0} {:>6}  {:>6.0} {:>8.3} {:>9.4} {:>9.2e}   valid {:.1}%",
            row.radius_mm,
            row.eligible,
            row.fitted,
            row.under_determined,
            row.median_starved_points,
            row.ill_conditioned,
            row.median_points,
            row.median_gather_rms,
            row.median_residual_rms,
            row.median_pivot_ratio,
            100.0 * row.valid_fraction()
        );
    }
    eprintln!(
        "  under = fewer than {MIN_FIT_POINTS} vertices in the disc; starve = median vertices"
    );
    eprintln!("  those rejected samples DID find. pts/gathRMS/residRMS/pivot are medians over");
    eprintln!("  FITTED samples. residRMS RISING with r is the instrument working — a coarse fit");
    eprintln!("  discards sub-scale structure by design; a FINE fit with a large residual, or a");
    eprintln!("  low pivot ratio, is the broken one. A low valid % also means the surviving");
    eprintln!("  population is censored toward DENSE mesh, hence biased UPWARD in anisotropy.");

    eprintln!();
    eprintln!("── principal curvatures, area-weighted (per mm, convex-positive) ───────");
    eprintln!(
        "  {:>5}  {:>9} {:>9} {:>9} {:>9} {:>9}   {:>9} {:>9} {:>9} {:>9} {:>9}",
        "r_mm",
        "k1 min",
        "k1 p10",
        "k1 p50",
        "k1 p90",
        "k1 max",
        "k2 min",
        "k2 p10",
        "k2 p50",
        "k2 p90",
        "k2 max"
    );
    for row in reports {
        eprintln!(
            "  {:>5.1}  {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>9.4}   {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>9.4}",
            row.radius_mm,
            row.kappa1.min,
            row.kappa1.p10,
            row.kappa1.p50,
            row.kappa1.p90,
            row.kappa1.max,
            row.kappa2.min,
            row.kappa2.p10,
            row.kappa2.p50,
            row.kappa2.p90,
            row.kappa2.max
        );
    }

    eprintln!();
    eprintln!("── curvature decay: k1_p50 ∝ r^-p, implied Hurst H = 2 - p ─────────────");
    eprintln!(
        "  {:>13}  {:>9} {:>9}  {:>7} {:>7}   reading",
        "r pair (mm)", "k1_p50 lo", "k1_p50 hi", "p", "H"
    );
    for pair in reports.windows(2) {
        let (lo, hi) = (&pair[0], &pair[1]);
        let Some(exponent) = decay_exponent(lo, hi) else {
            eprintln!(
                "  {:>5.1} -> {:>5.1}  {:>9.4} {:>9.4}  {:>7} {:>7}   not computable",
                lo.radius_mm, hi.radius_mm, lo.kappa1.p50, hi.kappa1.p50, "-", "-"
            );
            continue;
        };
        let reading = if lo.valid_fraction() < SCALE_RULE_MIN_VALID_FRACTION
            || hi.valid_fraction() < SCALE_RULE_MIN_VALID_FRACTION
        {
            "UNSUPPORTED PAIR (a radius below the valid-fit floor)"
        } else if exponent >= TESSELLATION_DECAY_EXPONENT {
            "p ~ 2: WHITE NOISE — this pair is measuring FACETING"
        } else if exponent <= LANDSCAPE_DECAY_EXPONENT {
            "landscape (self-affine relief)"
        } else {
            "between: neither clean noise nor clean relief"
        };
        eprintln!(
            "  {:>5.1} -> {:>5.1}  {:>9.4} {:>9.4}  {:>7.3} {:>7.3}   {}",
            lo.radius_mm,
            hi.radius_mm,
            lo.kappa1.p50,
            hi.kappa1.p50,
            exponent,
            2.0 - exponent,
            reading
        );
    }
    eprintln!("  Curvature at scale r is ~ h(r)/r^2. Uncorrelated vertex displacement (faceting)");
    eprintln!("  has h independent of r, so p >= 2 (p = 2 is the no-averaging bound; an LS fit");
    eprintln!("  averages N ~ r^2 vertices and can push pure noise to p ~ 3, which is the SAME");
    eprintln!("  finding, not a third category). A self-affine surface of Hurst H gives");
    eprintln!(
        "  p = 2 - H, and real relief runs H ~ 0.5-1.0 (p ~ 1.0-1.5). p is diagnostic at the"
    );
    eprintln!("  FINE end only: it rises again at the coarse end for a legitimate reason, namely");
    eprintln!("  smoothing past every feature of a surface whose relief is bounded.");

    eprintln!();
    eprintln!("── anisotropy W_max/W_min and the PRIZE BOUND, h = {SCALLOP_H_MM} mm ──────────");
    eprintln!(
        "  {:>5} {:>5}  {:>7} {:>7} {:>7} {:>7} {:>8}  {:>7} {:>7} {:>7} {:>7}  {:>7} {:>7} {:>7}  {:>7} {:>8}",
        "r_mm",
        "R_mm",
        "min",
        "p10",
        "p50",
        "p90",
        "max",
        ">1.05",
        ">1.10",
        ">1.25",
        ">1.50",
        "bnd 0deg",
        "bnd 90",
        "bnd PCA",
        "gouge%",
        "gouge n"
    );
    for row in reports {
        for tool in &row.tools {
            eprintln!(
                "  {:>5.1} {:>5.1}  {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>8.2}  {:>6.1}% {:>6.1}% {:>6.1}% {:>6.1}%  {:>7.4} {:>7.4} {:>7.4}  {:>6.1}% {:>8}",
                row.radius_mm,
                tool.tool_radius_mm,
                tool.ratio.min,
                tool.ratio.p10,
                tool.ratio.p50,
                tool.ratio.p90,
                tool.ratio.max,
                100.0 * tool.frac_above[0],
                100.0 * tool.frac_above[1],
                100.0 * tool.frac_above[2],
                100.0 * tool.frac_above[3],
                tool.bound_x,
                tool.bound_y,
                tool.bound_pca,
                100.0 * tool.gouge_area_frac,
                tool.gouge_samples
            );
        }
    }
    eprintln!("  bnd = [∫dA/W(fixed dir)] / [∫dA/W_max] — the CEILING on what the direction field");
    eprintln!(
        "  can win, before its four missing pipeline stages cost anything. gouge = area fraction"
    );
    eprintln!(
        "  with k2 + 1/R <= 0 (concavity tighter than the ball), EXCLUDED from every figure on"
    );
    eprintln!("  its row — a large gouge % means that row is measured on a flatter subset.");
}

/// Print the verdict block against the thresholds pre-registered in the module
/// doc. Reads exactly the cell named there and nothing else.
fn print_verdict(name: &str, reports: &[RadiusReport]) {
    eprintln!();
    eprintln!("── VERDICT: {name} ────────────────────────────────────────────────");
    let Some(cell) = reports
        .iter()
        .find(|r| (r.radius_mm - VERDICT_FIT_RADIUS_MM).abs() < 1e-9)
    else {
        eprintln!("  no r = {VERDICT_FIT_RADIUS_MM} mm row — cannot read the pre-registered cell.");
        return;
    };
    eprintln!(
        "  decision cell: r = {:.1} mm, valid fits {:.1}% ({} of {})",
        cell.radius_mm,
        100.0 * cell.valid_fraction(),
        cell.fitted,
        cell.eligible
    );
    for &tool_radius in &VERDICT_TOOL_RADII_MM {
        let Some(tool) = cell.tool(tool_radius) else {
            continue;
        };
        let median_ratio = tool.ratio.p50;
        let band = if median_ratio <= VERDICT_CLOSE_BELOW {
            "CLOSE ON EVIDENCE — prize under ~2%"
        } else if median_ratio <= VERDICT_LITERATURE_ABOVE {
            "LITERATURE BAND — 1.9-7.2% territory, four missing stages are real work"
        } else {
            "ABOVE LITERATURE — larger prize than Kumazawa reports; explain before believing"
        };
        let best_bound = tool.bound_x.min(tool.bound_y).min(tool.bound_pca);
        eprintln!(
            "  R = {:.1} mm: median W_max/W_min = {:.4}  ->  {}",
            tool_radius, median_ratio, band
        );
        eprintln!(
            "    prize bound (best fixed direction) = {:.4}  =  {:+.2}% ceiling  \
             [Kumazawa measured {:.1}-{:.1}% vs iso-scallop]",
            best_bound,
            100.0 * (best_bound - 1.0),
            KUMAZAWA_BAND_PCT.0,
            KUMAZAWA_BAND_PCT.1
        );
        if tool.gouge_area_frac > 0.02 {
            eprintln!(
                "    CAVEAT: {:.1}% of area gouge-censored at this R — figures are for the \
                 subset where the ball fits.",
                100.0 * tool.gouge_area_frac
            );
        }
    }

    // ── the scale verdict ──
    let fine = reports
        .iter()
        .find(|r| r.valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION);
    let coarse_excess = reports
        .iter()
        .filter(|r| r.radius_mm >= SCALE_RULE_COARSE_FROM_MM)
        .filter_map(|r| r.tool(VERDICT_TOOL_RADII_MM[0]))
        .map(|t| t.ratio.p50 - 1.0)
        .fold(f64::NEG_INFINITY, f64::max);
    eprintln!();
    match fine.and_then(|r| r.tool(VERDICT_TOOL_RADII_MM[0]).map(|t| (r, t))) {
        Some((row, tool)) if coarse_excess.is_finite() => {
            let fine_excess = tool.ratio.p50 - 1.0;
            eprintln!(
                "  scale rule: finest radius with >={:.0}% valid fits is r = {:.1} mm \
                 (excess {:.4}); max excess at r >= {:.1} mm is {:.4}.",
                100.0 * SCALE_RULE_MIN_VALID_FRACTION,
                row.radius_mm,
                fine_excess,
                SCALE_RULE_COARSE_FROM_MM,
                coarse_excess
            );
            // The `> 1e-6` arm matters: with both excesses at zero the
            // inequality `0 >= 2·0` would hold and the rule would "fire" on a
            // surface that has no anisotropy at ANY scale — which is the
            // opposite finding. A dead-flat reading is not a tessellation
            // artefact; it is the CLOSE verdict.
            if fine_excess > 1e-6 && fine_excess >= TESSELLATION_FACTOR * coarse_excess {
                eprintln!(
                    "  ==> FIRES ({:.4} >= {:.1} x {:.4}). THE FINE-SCALE READING IS \
                     TESSELLATION, NOT TERRAIN.",
                    fine_excess, TESSELLATION_FACTOR, coarse_excess
                );
                eprintln!(
                    "      Every earlier curvature number on this mesh is suspect unless it \
                     states a fit scale,"
                );
                eprintln!(
                    "      INCLUDING F1's |V| range of 0.007-0.461: a 1-ring estimate on a \
                     0.335 mm-facet mesh"
                );
                eprintln!(
                    "      measures the tessellation. Do not cite those numbers as terrain \
                     curvature."
                );
            } else {
                eprintln!(
                    "  ==> does NOT fire ({:.4} < {:.1} x {:.4}). The anisotropy survives \
                     coarsening, so the",
                    fine_excess, TESSELLATION_FACTOR, coarse_excess
                );
                eprintln!("      fine-scale reading is landscape, not faceting.");
            }
        }
        _ => eprintln!("  scale rule: not evaluable — no radius reached the valid-fit floor."),
    }

    // ── the sharper half: the decay exponent on the finest SUPPORTED pair ──
    let finest_pair = reports.windows(2).find(|pair| {
        pair[0].valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION
            && pair[1].valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION
    });
    match finest_pair.and_then(|pair| decay_exponent(&pair[0], &pair[1]).map(|p| (pair, p))) {
        Some((pair, exponent)) => {
            eprintln!(
                "  decay rule: finest supported pair r = {:.1} -> {:.1} mm gives p = {:.3} \
                 (implied Hurst H = {:.3}).",
                pair[0].radius_mm,
                pair[1].radius_mm,
                exponent,
                2.0 - exponent
            );
            if exponent >= TESSELLATION_DECAY_EXPONENT {
                eprintln!(
                    "  ==> p >= {TESSELLATION_DECAY_EXPONENT}, the WHITE-NOISE signature. THE \
                     FINE-SCALE CURVATURE ON THIS MESH IS"
                );
                eprintln!(
                    "      TESSELLATION, NOT TERRAIN. Every earlier curvature number here is"
                );
                eprintln!(
                    "      suspect unless it states a fit scale, INCLUDING F1's |V| range of"
                );
                eprintln!(
                    "      0.007-0.461 — a 1-ring estimate on a 0.335 mm-facet mesh measures the"
                );
                eprintln!("      triangles. Do not cite those numbers as terrain curvature.");
            } else if exponent <= LANDSCAPE_DECAY_EXPONENT {
                eprintln!(
                    "  ==> p <= {LANDSCAPE_DECAY_EXPONENT}: the fine-scale curvature is LANDSCAPE, \
                     not faceting. The fine-scale"
                );
                eprintln!(
                    "      anisotropy above is a real property of the relief, and the trap this"
                );
                eprintln!("      instrument was built to catch did NOT close on this mesh.");
                eprintln!(
                    "      This does NOT exonerate F1's |V| numbers. This instrument fits VERTICES,"
                );
                eprintln!(
                    "      which lie on the source surface; F1's 1-ring estimator reads DIHEDRAL"
                );
                eprintln!("      ANGLES, which at a 0.335 mm facet edge are faceting almost by");
                eprintln!(
                    "      construction. The two estimators do not see the same thing at facet"
                );
                eprintln!(
                    "      scale, and a landscape verdict here transfers to neither direction."
                );
            } else {
                eprintln!(
                    "  ==> p between {LANDSCAPE_DECAY_EXPONENT} and \
                     {TESSELLATION_DECAY_EXPONENT}: neither clean noise nor clean relief. Treat the"
                );
                eprintln!(
                    "      fine-scale reading as contaminated but not worthless, and quote a fit scale."
                );
            }
        }
        None => eprintln!("  decay rule: not evaluable — no adjacent pair of supported radii."),
    }
}

// ── the region-1 loader (restated, ~20 lines) ───────────────────────────

/// Absolute path to the workspace root. Restated from `tests/common/mod.rs` —
/// an integration test cannot import another one.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// Load `test_data/wanaka_region1_boundary_f1.json`.
///
/// **Deliberately restated** rather than imported: integration tests cannot
/// `use` each other, and that capture's own schema comment says a later F1
/// evidence test is expected to restate these ~20 lines. Same conventions:
/// [`Polygon2::with_holes_closed`], **no** `ensure_winding`, so the captured
/// vertex order is preserved exactly. Returns the polygon plus the triangle
/// count the capture was cut against, for the drift warning.
fn load_region1() -> Option<(Polygon2, usize)> {
    let path = repo_root().join("test_data/wanaka_region1_boundary_f1.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let ring = |val: &serde_json::Value| -> Vec<P2> {
        val.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some(P2::new(p[0].as_f64()?, p[1].as_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let exterior = ring(&v["exterior"]);
    let holes: Vec<Vec<P2>> = v["holes"]
        .as_array()
        .map(|a| a.iter().map(ring).collect())
        .unwrap_or_default();
    let closed = v["closed"].as_bool().unwrap_or(true);
    let triangles = v["params"]["mesh_triangle_count"]
        .as_u64()
        .unwrap_or_default() as usize;
    (exterior.len() >= 3).then(|| {
        (
            Polygon2::with_holes_closed(exterior, holes, closed),
            triangles,
        )
    })
}

// ── the instrument ──────────────────────────────────────────────────────

/// Keep the sample points that actually sit on the surface, paired with their
/// height.
fn eligible(ctx: &MeshCtx<'_>, candidates: &[P2]) -> Vec<(P2, f64)> {
    candidates
        .par_iter()
        .filter_map(|&p| surface_z(ctx.mesh, ctx.index, p).map(|z| (p, z)))
        .collect()
}

#[test]
#[ignore = "instrument — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_curvature_anisotropy() {
    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }

    eprintln!();
    eprintln!("########################################################################");
    eprintln!("# CURVATURE ANISOTROPY — does the direction-field method have a prize? #");
    eprintln!("########################################################################");
    eprintln!();
    eprintln!("PRE-REGISTERED (see the module doc; written before any number below):");
    eprintln!(
        "  decision cell   : r = {VERDICT_FIT_RADIUS_MM} mm, R in {VERDICT_TOOL_RADII_MM:?} mm, \
         h = {SCALLOP_H_MM} mm"
    );
    eprintln!(
        "  prize verdict   : median W_max/W_min <= {VERDICT_CLOSE_BELOW} => CLOSE; \
         <= {VERDICT_LITERATURE_ABOVE} => literature band; above => needs explaining"
    );
    eprintln!(
        "  scale verdict   : excess(finest r with >={:.0}% valid fits) >= {TESSELLATION_FACTOR} x \
         max excess(r >= {SCALE_RULE_COARSE_FROM_MM} mm) => TESSELLATION",
        100.0 * SCALE_RULE_MIN_VALID_FRACTION
    );
    eprintln!(
        "  decay verdict   : p in k1_p50 ~ r^-p, finest supported pair; \
         p >= {TESSELLATION_DECAY_EXPONENT} => faceting (white noise),"
    );
    eprintln!(
        "                    p <= {LANDSCAPE_DECAY_EXPONENT} => landscape. \
         p = 2 exactly is uncorrelated vertex displacement."
    );
    eprintln!(
        "  prediction      : r=1.0/R=1.0 median ratio 1.08-1.18, bound 1.05-1.11; \
         r>=4 bound < 1.02;"
    );
    eprintln!(
        "                    r=0.3 mostly UNDER-DETERMINED (~2.3 vertices per disc); \
         MIDDLE band on prize;"
    );
    eprintln!(
        "                    excess rule predicted NOT to fire (marginal), decay p predicted \
         BELOW 1.5 -"
    );
    eprintln!(
        "                    but the preview that predicted it pre-smoothed the facets, so a \
         p ~ 2 here is"
    );
    eprintln!("                    the live possibility and the most informative outcome.");
    eprintln!();

    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let ctx = MeshCtx {
        mesh: &mesh,
        index: &index,
    };
    let bbox = &mesh.bbox;
    eprintln!(
        "mesh: {} triangles, {} vertices, x [{:.2}, {:.2}], y [{:.2}, {:.2}], z [{:.2}, {:.2}]",
        mesh.triangles.len(),
        mesh.vertices.len(),
        bbox.min.x,
        bbox.max.x,
        bbox.min.y,
        bbox.max.y,
        bbox.min.z,
        bbox.max.z
    );
    if mesh.triangles.len() != EXPECTED_TRIANGLES {
        eprintln!(
            "  WARNING: {} triangles, expected {EXPECTED_TRIANGLES}. The mesh has been \
             re-exported; region 1 and every F1 reference time may no longer refer to this \
             surface. Reported, not failed.",
            mesh.triangles.len()
        );
    }
    eprintln!(
        "sampling: whole surface {LATTICE_ALL_MM} mm lattice, region 1 {LATTICE_REGION_MM} mm \
         lattice, both inset {EDGE_INSET_MM} mm from the mesh bbox"
    );
    eprintln!(
        "  (the inset is FIXED, not per-radius, so the scale sweep compares one population; \
         it costs the outer {EDGE_INSET_MM} mm ring.)"
    );

    let inset_x = (bbox.min.x + EDGE_INSET_MM, bbox.max.x - EDGE_INSET_MM);
    let inset_y = (bbox.min.y + EDGE_INSET_MM, bbox.max.y - EDGE_INSET_MM);

    // ── population A: the whole up-facing surface ──
    let candidates_all = lattice(inset_x, inset_y, LATTICE_ALL_MM);
    let samples_all = eligible(&ctx, &candidates_all);
    let (axis_all, aniso_all) = pca_axis(&candidates_all);
    eprintln!();
    eprintln!(
        "population A (whole up-facing surface): {} lattice points, {} on-surface",
        candidates_all.len(),
        samples_all.len()
    );
    let reports_all = measure(&ctx, &samples_all, LATTICE_ALL_MM, axis_all);
    print_population("whole up-facing surface", &reports_all, axis_all, aniso_all);
    print_verdict("whole up-facing surface", &reports_all);

    // ── population B: the captured region 1 ──
    let Some((region, captured_triangles)) = load_region1() else {
        eprintln!();
        eprintln!(
            "SKIP population B: no capture at test_data/wanaka_region1_boundary_f1.json — run \
             wanaka_region_capture_f1::capture_wanaka_region_1_boundary first."
        );
        return;
    };
    if captured_triangles != mesh.triangles.len() {
        eprintln!(
            "  WARNING: capture was cut against {captured_triangles} triangles, this mesh has \
             {}. Region 1 may not be the region the F1 reference times were measured on.",
            mesh.triangles.len()
        );
    }
    let rbox = region.bbox();
    // One containment pass over the WHOLE region bbox, then partitioned by the
    // inset — so the "what the inset costs" figure is a like-for-like count off
    // the same lattice, and the O(V) ray cast runs once per point, not twice.
    let in_region: Vec<P2> = lattice((rbox[0], rbox[2]), (rbox[1], rbox[3]), LATTICE_REGION_MM)
        .into_par_iter()
        .filter(|p| region.contains_point(p))
        .collect();
    let uninset = in_region.len();
    let inside: Vec<P2> = in_region
        .into_iter()
        .filter(|p| p.x >= inset_x.0 && p.x <= inset_x.1 && p.y >= inset_y.0 && p.y <= inset_y.1)
        .collect();
    let samples_region = eligible(&ctx, &inside);
    let (axis_region, aniso_region) = pca_axis(&inside);
    eprintln!();
    eprintln!(
        "population B (region 1, {:.1} mm² captured): {} lattice points inside the boundary \
         after the {EDGE_INSET_MM} mm inset ({} before it — the inset costs {:.1}%), {} on-surface",
        region.area(),
        inside.len(),
        uninset,
        if uninset > 0 {
            100.0 * (1.0 - inside.len() as f64 / uninset as f64)
        } else {
            0.0
        },
        samples_region.len()
    );
    eprintln!("  Region 1 is where the F1 direction-field arm actually failed (6589 vs 141");
    eprintln!("  fragments), so it is the population that decides whether that failure was");
    eprintln!("  PREMISE or IMPLEMENTATION. A small prize here BOUNDS what a perfect");
    eprintln!("  implementation could ever have returned - it does not prove ours was correct.");
    let reports_region = measure(&ctx, &samples_region, LATTICE_REGION_MM, axis_region);
    print_population("region 1", &reports_region, axis_region, aniso_region);
    print_verdict("region 1", &reports_region);

    eprintln!();
    eprintln!("########################################################################");
}

// ── self-check: the estimator on surfaces whose curvature is known ──────

/// Tessellate `z = f(x, y)` over `[-half, half]²` at `step`, two triangles per
/// cell. Deterministic; used only by the self-check below.
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

/// Fit at one point of a synthetic patch, panicking with a readable message
/// rather than returning an outcome the caller has to unwrap by hand.
fn fit_at(mesh: &TriangleMesh, index: &SpatialIndex, at: P2, radius: f64) -> Fit {
    let z0 = surface_z(mesh, index, at).expect("sample lands on the synthetic patch");
    let mut scratch = Scratch::new(mesh.vertices.len());
    match fit_quadric(mesh, index, &mut scratch, (at, z0), radius) {
        Outcome::Fitted(fit) => fit,
        Outcome::UnderDetermined(n) => panic!("under-determined with {n} points at {at:?}"),
        Outcome::IllConditioned => panic!("ill-conditioned at {at:?}"),
    }
}

/// **The one piece of this instrument a reviewer can check without the mesh.**
///
/// Runs the real [`fit_quadric`] / [`fit_from_derivatives`] / [`kappa_perp_zou`]
/// path — not a re-derivation — against three surfaces whose curvature is known
/// in closed form, and pins:
///
/// 1. **Sign convention.** A convex dome must read `+1/ρ` in both principal
///    curvatures. If the flip in Step 3 were dropped, every sign here inverts
///    and `κ2 + 1/R ≤ 0` would fire on hilltops instead of in valleys.
/// 2. **Shape operator, not raw coefficients.** The sphere is sampled at
///    `(5, 0)` where the slope is 30°: the naive reading `−f_xx` is 0.15396,
///    54 % high, while the shape operator returns `1/ρ = 0.1`.
/// 3. **`κ_perp` and the literature's rule.** On a cylinder ridge running along
///    `y`, feeding along `x` (across the ridge, i.e. along the most convex
///    principal direction `t1`) must leave `κ_perp = 0` — the widest strip;
///    feeding along `y` must leave `κ_perp = +1/ρ` — the narrowest. That IS
///    Kim Eq. 23's "the widest cut is made in the most convex direction".
#[test]
fn shape_operator_recovers_known_curvature() {
    const RHO: f64 = 10.0;
    const FIT_R: f64 = 1.0;
    // Fits on a smooth analytic patch are near-exact; 2 % catches a wrong
    // formula without being tripped by the quartic term the quadric discards.
    const REL: f64 = 0.02;
    const ABS: f64 = 2e-3;

    // 1. Sphere cap, curvature 1/ρ in every direction, at the apex and on slope.
    let sphere = synthetic_patch(6.5, 0.2, |x, y| (RHO * RHO - x * x - y * y).sqrt());
    let sphere_index = SpatialIndex::build_auto(&sphere);
    for &(x, label) in &[(0.0, "apex"), (5.0, "30-degree slope")] {
        let fit = fit_at(&sphere, &sphere_index, P2::new(x, 0.0), FIT_R);
        for (name, value) in [("k1", fit.kappa1), ("k2", fit.kappa2)] {
            assert!(
                (value - 1.0 / RHO).abs() <= REL / RHO,
                "sphere {label}: {name} = {value:.6}, expected {:.6} (convex must read POSITIVE; \
                 a negative value means the Step-3 sign flip was dropped)",
                1.0 / RHO
            );
        }
        // Umbilic: every direction is preferred, W_max/W_min = 1 exactly. This
        // is the SPHERE fixture's defect stated as an assertion.
        let ratio = ((fit.kappa1 + 1.0) / (fit.kappa2 + 1.0)).sqrt();
        assert!(
            (ratio - 1.0).abs() <= REL,
            "sphere {label}: W_max/W_min = {ratio:.6}, must be 1 on an umbilic"
        );
    }

    // 2. Cylinder ridge along y: κ1 = 1/ρ (across), κ2 = 0 (along).
    let cylinder = synthetic_patch(6.5, 0.2, |x, _| (RHO * RHO - x * x).sqrt());
    let cylinder_index = SpatialIndex::build_auto(&cylinder);
    let fit = fit_at(&cylinder, &cylinder_index, P2::new(0.0, 0.0), FIT_R);
    assert!(
        (fit.kappa1 - 1.0 / RHO).abs() <= REL / RHO,
        "cylinder: k1 = {:.6}, expected {:.6}",
        fit.kappa1,
        1.0 / RHO
    );
    assert!(
        fit.kappa2.abs() <= ABS,
        "cylinder: k2 = {:.6}, expected 0 along the ruling",
        fit.kappa2
    );
    // Feed across the ridge (+x, the most convex direction) => κ_perp = κ2 = 0.
    let across = kappa_perp_zou(&fit, (1.0, 0.0));
    assert!(
        across.abs() <= ABS,
        "cylinder: feeding across the ridge must leave k_perp = 0, got {across:.6}"
    );
    // Feed along the ruling (+y) => κ_perp = κ1 = 1/ρ, the narrowest strip.
    let along = kappa_perp_zou(&fit, (0.0, 1.0));
    assert!(
        (along - 1.0 / RHO).abs() <= REL / RHO,
        "cylinder: feeding along the ruling must leave k_perp = 1/rho, got {along:.6}"
    );
    let wide = strip_width(across, 1.0, SCALLOP_H_MM).expect("across is not a gouge");
    let narrow = strip_width(along, 1.0, SCALLOP_H_MM).expect("along is not a gouge");
    assert!(
        wide > narrow,
        "the widest cut must be in the most convex direction (Kim Eq. 23): {wide:.6} vs {narrow:.6}"
    );
    // κ_perp must stay inside Euler's range for every direction.
    for step in 0..12 {
        let theta = std::f64::consts::PI * step as f64 / 12.0;
        let kappa = kappa_perp_zou(&fit, (theta.cos(), theta.sin()));
        assert!(
            kappa >= fit.kappa2 - ABS && kappa <= fit.kappa1 + ABS,
            "k_perp {kappa:.6} escaped [{:.6}, {:.6}] at {:.1} degrees",
            fit.kappa2,
            fit.kappa1,
            theta.to_degrees()
        );
    }

    // 3. Tilted plane: zero curvature despite a large gradient — the metric
    //    terms must cancel exactly, not approximately.
    let plane = synthetic_patch(4.0, 0.2, |x, y| 0.5 * x + 0.3 * y);
    let plane_index = SpatialIndex::build_auto(&plane);
    let fit = fit_at(&plane, &plane_index, P2::new(0.0, 0.0), FIT_R);
    assert!(
        fit.kappa1.abs() <= ABS && fit.kappa2.abs() <= ABS,
        "tilted plane must read flat: k1 = {:.6}, k2 = {:.6}",
        fit.kappa1,
        fit.kappa2
    );
    assert!(
        (fit.area_weight - (1.0 + 0.25 + 0.09f64).sqrt()).abs() <= 1e-6,
        "tilted plane area weight = {:.6}, expected sqrt(1 + 0.25 + 0.09)",
        fit.area_weight
    );

    // 4. The gouge guard fires where the ball cannot fit, and NOWHERE else.
    //    A valley of radius 0.4 mm read by a 1.0 mm ball: κ2 = −2.5, so
    //    κ2 + 1/R = −1.5 < 0.
    let valley = synthetic_patch(3.0, 0.05, |x, _| {
        let clamped = x.clamp(-0.35, 0.35);
        0.4 - (0.16 - clamped * clamped).sqrt()
    });
    let valley_index = SpatialIndex::build_auto(&valley);
    let fit = fit_at(&valley, &valley_index, P2::new(0.0, 0.0), 0.2);
    assert!(
        fit.kappa2 < -1.0,
        "a 0.4 mm valley must read strongly concave, got k2 = {:.6}",
        fit.kappa2
    );
    assert!(
        strip_width(fit.kappa2, 1.0, SCALLOP_H_MM).is_none(),
        "a 1.0 mm ball in a 0.4 mm valley must be reported as a GOUGE, never clamped"
    );
    assert!(
        strip_width(fit.kappa2, 0.2, SCALLOP_H_MM).is_some(),
        "a 0.2 mm ball fits the same valley and must NOT be reported as a gouge"
    );
}
