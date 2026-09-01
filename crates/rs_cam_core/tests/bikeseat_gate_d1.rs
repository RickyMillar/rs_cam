//! **Track D, gate stage** — does the bike-seat fixture class pass the two
//! gates that Wanaka failed?
//!
//! # The question
//!
//! The F1 direction-field arm was CLOSED on Wanaka for one reason: the
//! preferred direction turns inside one stepover there (coherence length
//! 0.35 mm against a 0.4862 mm stepover, `w30 <= 0.43` on every product
//! zone). The anisotropy prize itself is real on Wanaka (+9.75 % ceiling on
//! region 1). The method's home turf — Zou's own validation set — is the
//! bike-seat class: smooth, swept, simply connected sheets. Zou's Table 1
//! reports −13.0 % path length on the bike seat versus classic iso-scallop.
//!
//! This file builds an ANALYTIC bike-seat-class fixture and runs the two
//! existing censuses on it. No pathing. No field solve. The gates decide
//! whether the full pipeline is worth scheduling
//! (`planning/bikeseat_gate_2026-09-01/TRACK.md`).
//!
//! # The fixture (closed form, no file on disk)
//!
//! With `u = x - 80`, `v = y - 45` over `x in [0, 160]`, `y in [0, 90]` mm:
//!
//! ```text
//!   z(u, v) = spine + dish + wave + nose
//!   spine   = 14 (1 - (u/80)^2)                       convex along the sweep
//!   dish    = 8 ((v/45)^2 - 1)                        concave across it
//!   wave    = A(u) cos(k psi),  k = 2 pi / 14
//!             A(u) = 1.54 + 0.55 cos(pi u / 80)
//!             psi  = v + 0.35 u^2 / 80                fanned flow lines
//!   nose    = 6 exp(-((u+55)^2 + v^2) / (2 * 7^2))    convex nose dome
//! ```
//!
//! Why each term is there:
//!
//! * The **wave** is the prize carrier. Crest bands (convex across) prefer a
//!   feed ACROSS the band; trough bands (concave across) prefer a feed ALONG
//!   it. Bands are 7 mm wide — about 14 stepovers — so the direction zones
//!   are coherent by construction *intent*. The census decides whether they
//!   are coherent in fact.
//! * The **fan** (`psi`'s `u^2` term) bends the bands through ±35 degrees
//!   across the sheet. Without it, one fixed raster angle rides every trough
//!   and captures most of the prize, and the ceiling — which is measured
//!   AGAINST the best fixed direction — collapses. A real seat's flow lines
//!   curve; this is the analytic version of that.
//! * `A(u)` varies the wave from `kappa ~ 0.20` to `~ 0.42` per mm along the
//!   sweep, so `s_max` genuinely varies (the adaptive-spacing prize, fixture
//!   audit error #3).
//! * The **nose dome** adds a convex-both-ways region, which pulls
//!   `min(s_max)` below the flat-stepover anchor, and makes the sheet a
//!   seat: horn, dished middle, spread tail.
//! * **spine + dish** give the seat its tens-of-mm relief and keep
//!   `kappa_along` strictly positive and small, so the trough/crest regime
//!   split is clean.
//!
//! The mesh is a heightfield grid at 0.065 mm — every vertex EXACTLY on the
//! closed form — and the instrument asserts, not assumes: max 3-D edge
//! (diagonals included) <= stepover/3, 100 % up-facing, relief >= 15 mm,
//! `mean(s_max)/min(s_max) >= 1.10`.
//!
//! # The two gates (pre-registered in
//! `planning/bikeseat_gate_2026-09-01/FINDINGS.md` BEFORE the run)
//!
//! * **GATE 1 (anisotropy prize).** At fit radius 1.0 mm, ball R = 1.0 mm,
//!   scallop 0.03 mm: prize ceiling = `min over fixed directions (x, y,
//!   PCA)` of `int dA/W_fixed / int dA/W_max`, as a percent. **PASS bar
//!   is 5.0 % or more.** Kumazawa's honest prize against iso-scallop is 1.9–7.2 %;
//!   Wanaka measured a 9.75 % ceiling and the method still lost there for
//!   coherence reasons. A home-turf fixture offering under 5 % — below the
//!   middle of the literature band — cannot justify the arm. Both
//!   anti-faceting scale rules must also clear, or the number is not
//!   quotable.
//! * **GATE 2 (coherence).** Two conditions, both required:
//!   G2-a: whole-sheet median coherence length >= 3 stepovers (1.459 mm).
//!   G2-b: both orientation-regime zones (trusted `t1` within 45 degrees of
//!   x, respectively of y — the same 45-degree segmentation angle §F1-2 ran
//!   on Wanaka) read `w30 >= 0.70`, and together they cover >= 70 % of the
//!   sheet area. Wanaka's failure reference: `w30 <= 0.43`, coherence length
//!   0.35 mm vs the 0.486 mm stepover.
//!
//!   The whole-sheet `w30` is printed but is NOT a gate: a two-regime
//!   surface fails it by construction (the regimes prefer perpendicular
//!   directions), and the method under test segments by direction before it
//!   paths — Kumazawa's own step.
//!
//! # Negative control
//!
//! A 60 x 60 mm patch summing six cosine plane waves at scattered angles,
//! wavelengths 2.6–3.7 mm. Its direction field turns at sub-stepover scale
//! by construction. Expectation: GATE 1 passes (anisotropy is real, exactly
//! as on Wanaka), GATE 2 fails (coherence length under 3 stepovers,
//! whole-patch w30 low). A gate-2 pass on the sheet is meaningful only if
//! the control fails it.
//!
//! # Estimator
//!
//! `tests/common/monge.rs` — the Monge-quadric machinery extracted from
//! `wanaka_curvature_anisotropy.rs` and `zone_coherence_census.rs`. The
//! non-ignored self-check below runs the extracted path against surfaces
//! whose curvature is known in closed form, so the extraction cannot have
//! drifted.
//!
//! # Running it
//!
//! ```text
//! # The gate run (analytic — needs no external mesh):
//! cargo test --release -p rs_cam_core --test bikeseat_gate_d1 -- --ignored --nocapture
//!
//! # The self-check (runs in the normal gate):
//! cargo test -p rs_cam_core --test bikeseat_gate_d1 monge_extraction
//! ```
//!
//! The instrument records what it finds. The verdict lines compare against
//! the pre-registered bars and say PASS or FAIL; the human reads FINDINGS.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::monge::{
    self, MongeFit, MongeOutcome, MongeScratch, Quantiles, axis_cos, dominant_axis, fit_quadric,
    kappa_perp_zou, lattice, median, quantiles, strip_width, surface_z, tessellate_heightfield,
};
use rayon::prelude::*;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};

// ════════════════════════════════════════════════════════════════════════
// Pre-registered constants — written before any number was measured
// ════════════════════════════════════════════════════════════════════════

/// Ball radius (mm) both gates read. The programme's standard finishing tool.
const TOOL_RADIUS_MM: f64 = 1.0;

/// Scallop constraint (mm) — the multitool's own cusp target.
const SCALLOP_H_MM: f64 = 0.03;

/// Flat-surface stepover for R = 1.0, h = 0.03: `2 sqrt(2Rh - h^2)`.
/// Pinned to the same value the coherence census pinned.
const STEPOVER_MM: f64 = 0.4862;

/// Facet budget: max 3-D triangle edge (diagonals included) <= stepover/3.
/// The programme's standing anti-faceting rule; ASSERTED on the built mesh.
const FACET_MAX_EDGE_MM: f64 = STEPOVER_MM / 3.0;

/// GATE 1 pass bar: prize ceiling percent at the decision cell.
const GATE1_CEILING_MIN_PCT: f64 = 5.0;

/// GATE 2 pass bars.
const GATE2_W30_MIN: f64 = 0.70;
const GATE2_LENGTH_MIN_STEPOVERS: f64 = 3.0;
const GATE2_LENGTH_MIN_MM: f64 = GATE2_LENGTH_MIN_STEPOVERS * STEPOVER_MM;
const GATE2_REGIME_COVERAGE_MIN: f64 = 0.70;

/// Fixture requirement: area-weighted `mean(s_max) / min(s_max)` at the
/// decision cell. "Genuinely varying curvature", asserted not assumed.
const FIXTURE_SMAX_SPREAD_MIN: f64 = 1.10;

/// Fixture requirement: relief (z range) in mm — "tens of mm, not a coupon".
const FIXTURE_RELIEF_MIN_MM: f64 = 15.0;

/// Fit radii (mm) for the anisotropy scale sweep.
const FIT_RADII_MM: [f64; 5] = [0.3, 0.6, 1.0, 2.0, 4.0];

/// The fit radius the verdicts read — same decision cell as Wanaka's.
const VERDICT_FIT_RADIUS_MM: f64 = 1.0;

/// Gate-1 sample lattice spacing (mm).
const GATE1_LATTICE_MM: f64 = 0.8;

/// Gate-2 field lattice spacing (mm). Comparable to the wanaka census's
/// per-triangle unit (0.335 mm median facet edge there): fine enough to see
/// a sub-stepover turn, cheap enough to field-fit a 6.8M-triangle mesh.
const GATE2_LATTICE_MM: f64 = 0.25;

/// XY inset from the domain edge (mm) — the largest fit radius, so no
/// reported fit is one-sided. Fixed across radii (one population).
const EDGE_INSET_MM: f64 = 4.0;

/// Scale-rule constants, same values as the anisotropy census.
const SCALE_RULE_MIN_VALID_FRACTION: f64 = 0.80;
const SCALE_RULE_COARSE_FROM_MM: f64 = 2.0;
const TESSELLATION_FACTOR: f64 = 2.0;
const TESSELLATION_DECAY_EXPONENT: f64 = 1.8;
const LANDSCAPE_DECAY_EXPONENT: f64 = 1.5;

/// Coherence-census constants, same values as the coherence census.
const COHERENCE_TURN_DEG: f64 = 30.0;
const COHERENCE_ANGLES_DEG: [f64; 4] = [10.0, 20.0, 30.0, 45.0];
const COHERENCE_SEARCH_BOUND_MM: f64 = 20.0 * STEPOVER_MM;
const COHERENCE_GRID_CELL_MM: f64 = 1.0;
const COHERENCE_QUERY_CAP: usize = 4_000;
const MIN_TRUSTED_CELLS: usize = 2;

/// A zone smaller than a (3-stepover)^2 square cannot hold the gate's own
/// coherence bar in both directions; it is reported but never usable.
const MIN_VERDICT_AREA_MM2: f64 = GATE2_LENGTH_MIN_MM * GATE2_LENGTH_MIN_MM;

/// The regime split angle: trusted `t1` within 45 degrees of +x is regime
/// ALONG; the rest is regime ACROSS. 45 degrees is the §F1-2 segmentation
/// angle that cut Wanaka's region 1 into 1,452 patches.
const REGIME_SPLIT_DEG: f64 = 45.0;

/// Tile edge sizes (mm) for the size control.
const TILE_SIZES_MM: [f64; 3] = [16.0, 8.0, 4.0];

/// Failure / comparison references (measured elsewhere, cited not rerun).
const WANAKA_REGION1_CEILING_PCT: f64 = 9.75;
const WANAKA_REGION1_MEDIAN_RATIO: f64 = 1.0950;
const WANAKA_W30_BEST: f64 = 0.43;
const WANAKA_COHERENCE_MM: f64 = 0.35;
const KUMAZAWA_BAND_PCT: (f64, f64) = (1.9, 7.2);

// ── the sheet fixture ────────────────────────────────────────────────────

const SHEET_X_MM: f64 = 160.0;
const SHEET_Y_MM: f64 = 90.0;
const SHEET_STEP_MM: f64 = 0.065;
const SHEET_HALF_U: f64 = 80.0;
const SHEET_HALF_V: f64 = 45.0;
const SPINE_H_MM: f64 = 14.0;
const DISH_D_MM: f64 = 8.0;
const WAVE_LAMBDA_MM: f64 = 14.0;
const WAVE_A0_MM: f64 = 1.54;
const WAVE_A1_MM: f64 = 0.55;
const WAVE_FAN: f64 = 0.35;
const NOSE_A_MM: f64 = 6.0;
const NOSE_SIGMA_MM: f64 = 7.0;
const NOSE_U_MM: f64 = -55.0;

/// The bike-seat sheet, closed form. See the module doc for each term.
fn seat_height(x: f64, y: f64) -> f64 {
    let u = x - SHEET_HALF_U;
    let v = y - SHEET_HALF_V;
    let spine = SPINE_H_MM * (1.0 - (u / SHEET_HALF_U) * (u / SHEET_HALF_U));
    let dish = DISH_D_MM * ((v / SHEET_HALF_V) * (v / SHEET_HALF_V) - 1.0);
    let k = 2.0 * std::f64::consts::PI / WAVE_LAMBDA_MM;
    let amp = WAVE_A0_MM + WAVE_A1_MM * (std::f64::consts::PI * u / SHEET_HALF_U).cos();
    let psi = v + WAVE_FAN * u * u / SHEET_HALF_U;
    let wave = amp * (k * psi).cos();
    let nd = (u - NOSE_U_MM) * (u - NOSE_U_MM) + v * v;
    let nose = NOSE_A_MM * (-nd / (2.0 * NOSE_SIGMA_MM * NOSE_SIGMA_MM)).exp();
    spine + dish + wave + nose
}

// ── the negative control ─────────────────────────────────────────────────

const NOISE_XY_MM: f64 = 60.0;
const NOISE_STEP_MM: f64 = 0.10;

/// Six cosine plane waves: (wavelength mm, direction deg, phase rad,
/// curvature amplitude per mm). Deterministic — no RNG, so the run is
/// reproducible from this table.
const NOISE_WAVES: [(f64, f64, f64, f64); 6] = [
    (2.6, 10.0, 0.7, 0.11),
    (2.9, 47.0, 2.1, 0.11),
    (3.3, 86.0, 4.0, 0.11),
    (3.7, 121.0, 1.3, 0.11),
    (3.05, 152.0, 5.2, 0.11),
    (2.75, 68.0, 3.3, 0.11),
];

/// Noise-like terrain: direction turns at sub-stepover scale by construction.
fn noise_height(x: f64, y: f64) -> f64 {
    let mut z = 0.0;
    for &(wavelength, theta_deg, phase, kappa) in &NOISE_WAVES {
        let k = 2.0 * std::f64::consts::PI / wavelength;
        let amp = kappa / (k * k);
        let theta = theta_deg.to_radians();
        let s = x * theta.cos() + y * theta.sin();
        z += amp * (k * s + phase).cos();
    }
    z
}

// ════════════════════════════════════════════════════════════════════════
// Mesh census — the fixture requirements, measured
// ════════════════════════════════════════════════════════════════════════

struct MeshCensus {
    area_mm2: f64,
    min_normal_z: f64,
    edge_min_mm: f64,
    edge_median_mm: f64,
    edge_max_mm: f64,
    z_min: f64,
    z_max: f64,
}

fn mesh_census(mesh: &TriangleMesh) -> MeshCensus {
    let per_tri: Vec<(f64, f64, [f64; 3], f64, f64)> = mesh
        .triangles
        .par_iter()
        .map(|tri| {
            let a = mesh.vertices[tri[0] as usize];
            let b = mesh.vertices[tri[1] as usize];
            let c = mesh.vertices[tri[2] as usize];
            let n = (b - a).cross(&(c - a));
            let norm = n.norm();
            let area = 0.5 * norm;
            let nz = if norm > 0.0 {
                (n.z / norm).abs() * n.z.signum()
            } else {
                0.0
            };
            let edges = [(b - a).norm(), (c - b).norm(), (a - c).norm()];
            let zlo = a.z.min(b.z).min(c.z);
            let zhi = a.z.max(b.z).max(c.z);
            (area, nz, edges, zlo, zhi)
        })
        .collect();
    let mut area = 0.0;
    let mut min_nz = f64::INFINITY;
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    let mut edges: Vec<f64> = Vec::with_capacity(per_tri.len() * 3);
    for (a, nz, e, zlo, zhi) in per_tri {
        area += a;
        min_nz = min_nz.min(nz);
        z_min = z_min.min(zlo);
        z_max = z_max.max(zhi);
        edges.extend_from_slice(&e);
    }
    edges.sort_by(f64::total_cmp);
    MeshCensus {
        area_mm2: area,
        min_normal_z: min_nz,
        edge_min_mm: edges[0],
        edge_median_mm: edges[edges.len() / 2],
        edge_max_mm: edges[edges.len() - 1],
        z_min,
        z_max,
    }
}

// ════════════════════════════════════════════════════════════════════════
// Gate 1 — the anisotropy census (lattice unit, as in the wanaka original)
// ════════════════════════════════════════════════════════════════════════

/// The R = 1.0 cell of one fit radius.
struct ToolCell {
    gouge_area_frac: f64,
    ratio: Quantiles,
    smax: Quantiles,
    smax_mean: f64,
    bound_x: f64,
    bound_y: f64,
    bound_pca: f64,
}

struct RadiusRow {
    radius_mm: f64,
    eligible: usize,
    fitted: usize,
    under_determined: usize,
    ill_conditioned: usize,
    kappa1: Quantiles,
    kappa2: Quantiles,
    tool: Option<ToolCell>,
}

impl RadiusRow {
    fn valid_fraction(&self) -> f64 {
        if self.eligible == 0 {
            0.0
        } else {
            self.fitted as f64 / self.eligible as f64
        }
    }
}

/// PCA of the sample lattice's XY positions (unit direction). Restated from
/// the anisotropy census.
fn pca_axis(points: &[P2]) -> (f64, f64) {
    let n = points.len() as f64;
    if n < 2.0 {
        return (1.0, 0.0);
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
    let theta = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
    (theta.cos(), theta.sin())
}

/// The per-tool cell: anisotropy distribution + the three prize bounds +
/// the s_max spread. Same arithmetic as the wanaka `tool_report`.
fn tool_cell(fits: &[MongeFit], cell_area: f64, axis: (f64, f64)) -> ToolCell {
    let mut ratios: Vec<(f64, f64)> = Vec::with_capacity(fits.len());
    let mut smax_pairs: Vec<(f64, f64)> = Vec::with_capacity(fits.len());
    let mut gouge_area = 0.0f64;
    let mut total_area = 0.0f64;
    let mut smax_weighted = 0.0f64;
    let mut smax_area = 0.0f64;
    let mut floor_opt = 0.0f64;
    let mut fixed = [0.0f64; 3];
    let directions = [(1.0, 0.0), (0.0, 1.0), axis];

    for fit in fits {
        let area = fit.area_weight * cell_area;
        total_area += area;
        // kappa2 is the minimum normal curvature, so kappa2 + 1/R <= 0 is the
        // tightest the denominator gets. If it survives, every direction does.
        let (Some(w_max), Some(w_min)) = (
            strip_width(fit.kappa2, TOOL_RADIUS_MM, SCALLOP_H_MM),
            strip_width(fit.kappa1, TOOL_RADIUS_MM, SCALLOP_H_MM),
        ) else {
            gouge_area += area;
            continue;
        };
        ratios.push((w_max / w_min, area));
        smax_pairs.push((w_max, area));
        smax_weighted += w_max * area;
        smax_area += area;
        floor_opt += area / w_max;
        for (slot, &dir) in fixed.iter_mut().zip(directions.iter()) {
            let kappa = kappa_perp_zou(fit, dir);
            match strip_width(kappa, TOOL_RADIUS_MM, SCALLOP_H_MM) {
                Some(width) => *slot += area / width,
                // Unreachable given the kappa2 guard (kappa_perp in
                // [kappa2, kappa1]); if it fires, charge the tightest
                // admissible width rather than biasing the bound DOWN.
                None => *slot += area / w_min,
            }
        }
    }
    let bound = |value: f64| {
        if floor_opt > 0.0 {
            value / floor_opt
        } else {
            f64::NAN
        }
    };
    ToolCell {
        gouge_area_frac: if total_area > 0.0 {
            gouge_area / total_area
        } else {
            0.0
        },
        ratio: quantiles(ratios).unwrap_or_default(),
        smax: quantiles(smax_pairs).unwrap_or_default(),
        smax_mean: if smax_area > 0.0 {
            smax_weighted / smax_area
        } else {
            f64::NAN
        },
        bound_x: bound(fixed[0]),
        bound_y: bound(fixed[1]),
        bound_pca: bound(fixed[2]),
    }
}

fn measure_gate1(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    samples: &[(P2, f64)],
    spacing: f64,
    axis: (f64, f64),
) -> Vec<RadiusRow> {
    let cell_area = spacing * spacing;
    let mut rows = Vec::new();
    for &radius in &FIT_RADII_MM {
        // Collected into an order-stable Vec and reduced SERIALLY afterwards,
        // so the printed numbers do not jitter between runs.
        let outcomes: Vec<MongeOutcome> = samples
            .par_iter()
            .with_min_len(256)
            .map_init(
                || MongeScratch::new(mesh.vertices.len()),
                |scratch, &sample| fit_quadric(mesh, index, scratch, sample, radius),
            )
            .collect();
        let mut fits: Vec<MongeFit> = Vec::new();
        let mut under = 0usize;
        let mut ill = 0usize;
        for outcome in outcomes {
            match outcome {
                MongeOutcome::Fitted(fit) => fits.push(fit),
                MongeOutcome::UnderDetermined(_) => under += 1,
                MongeOutcome::IllConditioned => ill += 1,
            }
        }
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
        let tool = (!fits.is_empty()).then(|| tool_cell(&fits, cell_area, axis));
        rows.push(RadiusRow {
            radius_mm: radius,
            eligible: samples.len(),
            fitted: fits.len(),
            under_determined: under,
            ill_conditioned: ill,
            kappa1,
            kappa2,
            tool,
        });
    }
    rows
}

/// Log-log decay exponent `p` of median kappa1 between two fit radii. `None`
/// when either median is non-positive or the radii coincide.
fn decay_exponent(lo: &RadiusRow, hi: &RadiusRow) -> Option<f64> {
    let (k_lo, k_hi) = (lo.kappa1.p50, hi.kappa1.p50);
    if k_lo <= 0.0 || k_hi <= 0.0 || hi.radius_mm <= lo.radius_mm {
        return None;
    }
    let value = (k_lo / k_hi).ln() / (hi.radius_mm / lo.radius_mm).ln();
    value.is_finite().then_some(value)
}

/// Everything the gate-1 verdict needs, extracted from the sweep.
struct Gate1Verdict {
    median_ratio: f64,
    ceiling_pct: f64,
    gouge_area_frac: f64,
    smax_mean: f64,
    smax: Quantiles,
    /// The excess rule: `Some(true)` = fired (tessellation), `Some(false)` =
    /// clean, `None` = not evaluable.
    excess_fired: Option<bool>,
    /// The decay exponent on the finest supported pair.
    decay_p: Option<f64>,
}

fn gate1_verdict(label: &str, rows: &[RadiusRow]) -> Gate1Verdict {
    println!();
    println!("── GATE 1 [{label}]: anisotropy sweep ─────────────────────────────────");
    println!(
        "  {:>6} {:>9} {:>7} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>8}",
        "r_mm",
        "fitted",
        "under",
        "ill",
        "k1_p50",
        "k2_p50",
        "ratio_p50",
        "ratio_p90",
        "gouge%",
        "smax_p50"
    );
    for row in rows {
        let (r50, r90, gouge, s50) = row
            .tool
            .as_ref()
            .map(|t| {
                (
                    t.ratio.p50,
                    t.ratio.p90,
                    100.0 * t.gouge_area_frac,
                    t.smax.p50,
                )
            })
            .unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
        println!(
            "  {:>6.1} {:>9} {:>7} {:>5} {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>9.2} {:>8.4}",
            row.radius_mm,
            row.fitted,
            row.under_determined,
            row.ill_conditioned,
            row.kappa1.p50,
            row.kappa2.p50,
            r50,
            r90,
            gouge,
            s50
        );
    }

    let cell = rows
        .iter()
        .find(|r| (r.radius_mm - VERDICT_FIT_RADIUS_MM).abs() < 1e-9)
        .expect("the sweep contains the verdict radius");
    let tool = cell
        .tool
        .as_ref()
        .expect("verdict cell fitted at least one sample");
    let best_bound = tool.bound_x.min(tool.bound_y).min(tool.bound_pca);
    let ceiling_pct = 100.0 * (best_bound - 1.0);
    println!(
        "  decision cell r = {VERDICT_FIT_RADIUS_MM} mm, R = {TOOL_RADIUS_MM} mm, h = {SCALLOP_H_MM} mm:"
    );
    println!(
        "    median W_max/W_min = {:.4}   [wanaka region 1: {WANAKA_REGION1_MEDIAN_RATIO}]",
        tool.ratio.p50
    );
    println!(
        "    fixed-direction bounds: x {:.4}  y {:.4}  pca {:.4}",
        tool.bound_x, tool.bound_y, tool.bound_pca
    );
    println!(
        "    PRIZE CEILING = {ceiling_pct:+.2} %   [bar {GATE1_CEILING_MIN_PCT} %; wanaka {WANAKA_REGION1_CEILING_PCT} %; Kumazawa {:.1}-{:.1} %]",
        KUMAZAWA_BAND_PCT.0, KUMAZAWA_BAND_PCT.1
    );
    println!(
        "    s_max (mm): mean {:.4}  min {:.4}  p10 {:.4}  p90 {:.4}  max {:.4}  mean/min {:.3}",
        tool.smax_mean,
        tool.smax.min,
        tool.smax.p10,
        tool.smax.p90,
        tool.smax.max,
        tool.smax_mean / tool.smax.min
    );
    if tool.gouge_area_frac > 0.02 {
        println!(
            "    CAVEAT: {:.1} % of area gouge-censored at R = {TOOL_RADIUS_MM} — figures are for the subset where the ball fits.",
            100.0 * tool.gouge_area_frac
        );
    }

    // Excess rule (anti-faceting arm 1).
    let fine = rows
        .iter()
        .find(|r| r.valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION);
    let coarse_excess = rows
        .iter()
        .filter(|r| r.radius_mm >= SCALE_RULE_COARSE_FROM_MM)
        .filter_map(|r| r.tool.as_ref())
        .map(|t| t.ratio.p50 - 1.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let excess_fired = match fine.and_then(|r| r.tool.as_ref().map(|t| (r, t))) {
        Some((row, t)) if coarse_excess.is_finite() => {
            let fine_excess = t.ratio.p50 - 1.0;
            let fired = fine_excess > 1e-6 && fine_excess >= TESSELLATION_FACTOR * coarse_excess;
            println!(
                "    excess rule: fine r = {:.1} excess {:.4} vs max coarse excess {:.4} -> {}",
                row.radius_mm,
                fine_excess,
                coarse_excess,
                if fired {
                    "FIRES (tessellation)"
                } else {
                    "clean (landscape)"
                }
            );
            Some(fired)
        }
        _ => {
            println!("    excess rule: not evaluable.");
            None
        }
    };

    // Decay rule (anti-faceting arm 2).
    let finest_pair = rows.windows(2).find(|pair| {
        pair[0].valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION
            && pair[1].valid_fraction() >= SCALE_RULE_MIN_VALID_FRACTION
    });
    let decay_p = finest_pair.and_then(|pair| decay_exponent(&pair[0], &pair[1]));
    match decay_p {
        Some(p) => {
            let read = if p >= TESSELLATION_DECAY_EXPONENT {
                "WHITE-NOISE signature (faceting)"
            } else if p <= LANDSCAPE_DECAY_EXPONENT {
                "landscape"
            } else {
                "between (contaminated)"
            };
            println!("    decay rule: p = {p:.3} -> {read}");
        }
        None => println!("    decay rule: not evaluable (non-positive median kappa1)."),
    }

    Gate1Verdict {
        median_ratio: tool.ratio.p50,
        ceiling_pct,
        gouge_area_frac: tool.gouge_area_frac,
        smax_mean: tool.smax_mean,
        smax: tool.smax,
        excess_fired,
        decay_p,
    }
}

// ════════════════════════════════════════════════════════════════════════
// Gate 2 — the coherence census (lattice-cell unit)
// ════════════════════════════════════════════════════════════════════════

/// One field cell — the lattice analogue of the coherence census's TriField.
#[derive(Clone, Copy)]
struct CellField {
    pos: P2,
    /// Surface area the cell represents: `W * lattice^2` (mm^2).
    area_mm2: f64,
    /// Trusted unit XY `t1`, sign arbitrary. `None` when the fit failed, the
    /// operator is identity-like, or the anisotropy is under the isotropy
    /// floor — the coherence census's TRUSTED condition, via
    /// `MongeFit::trusted_axis`.
    axis: Option<[f64; 2]>,
    fitted: bool,
    degenerate: bool,
}

#[derive(Default)]
struct FieldCensus {
    fitted: usize,
    under_determined: usize,
    ill_conditioned: usize,
    degenerate: usize,
}

fn build_cell_field(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    samples: &[(P2, f64)],
    cell_area_xy: f64,
) -> (Vec<CellField>, FieldCensus) {
    let outcomes: Vec<(P2, MongeOutcome)> = samples
        .par_iter()
        .with_min_len(256)
        .map_init(
            || MongeScratch::new(mesh.vertices.len()),
            |scratch, &(at, z0)| {
                (
                    at,
                    fit_quadric(mesh, index, scratch, (at, z0), VERDICT_FIT_RADIUS_MM),
                )
            },
        )
        .collect();
    let mut census = FieldCensus::default();
    let mut field = Vec::with_capacity(outcomes.len());
    for (pos, outcome) in outcomes {
        let mut entry = CellField {
            pos,
            area_mm2: cell_area_xy,
            axis: None,
            fitted: false,
            degenerate: false,
        };
        match outcome {
            MongeOutcome::Fitted(fit) => {
                census.fitted += 1;
                entry.fitted = true;
                entry.area_mm2 = fit.area_weight * cell_area_xy;
                match fit.trusted_axis() {
                    Some(axis) => entry.axis = Some(axis),
                    None => {
                        census.degenerate += 1;
                        entry.degenerate = true;
                    }
                }
            }
            MongeOutcome::UnderDetermined(_) => census.under_determined += 1,
            MongeOutcome::IllConditioned => census.ill_conditioned += 1,
        }
        field.push(entry);
    }
    (field, census)
}

/// A zone's trusted cells in a bucket grid, for the nearest-turn search.
/// Restated from the coherence census's TurnGrid; logic unchanged.
struct TurnGrid {
    pts: Vec<P2>,
    axes: Vec<[f64; 2]>,
    origin: P2,
    cols: i64,
    rows: i64,
    buckets: Vec<Vec<u32>>,
}

impl TurnGrid {
    fn build(field: &[CellField], members: &[u32]) -> Option<Self> {
        let mut pts = Vec::new();
        let mut axes = Vec::new();
        for &m in members {
            let entry = &field[m as usize];
            if let Some(axis) = entry.axis {
                pts.push(entry.pos);
                axes.push(axis);
            }
        }
        if pts.len() < MIN_TRUSTED_CELLS {
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
    /// `from`'s by more than [`COHERENCE_TURN_DEG`]. `None` = right-censored
    /// at [`COHERENCE_SEARCH_BOUND_MM`].
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    Usable,
    Marginal,
    NotUsable,
    TooSmall,
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

struct ZoneStats {
    cells: usize,
    area_mm2: f64,
    trusted_area_mm2: f64,
    degenerate_fraction: f64,
    dominant: Option<[f64; 2]>,
    /// Coherent area at [`COHERENCE_ANGLES_DEG`], over TOTAL zone area.
    within: [f64; 4],
    coherence_length_mm: f64,
    censored_fraction: f64,
    queries: usize,
    verdict: Verdict,
}

fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b } else { 0.0 }
}

/// Measure one zone — the coherence census's `census_zone`, on cells, with
/// the TRACK's bars (w30 >= 0.70, length >= 3 stepovers) in the verdict.
fn census_zone(field: &[CellField], members: &[u32]) -> ZoneStats {
    let mut area = 0.0f64;
    let mut trusted_area = 0.0f64;
    let mut degenerate_area = 0.0f64;
    let mut trusted: Vec<(f64, [f64; 2])> = Vec::new();
    for &m in members {
        let entry = &field[m as usize];
        area += entry.area_mm2;
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
    if let Some(d) = dominant {
        let mut coherent = [0.0f64; 4];
        for &(w, a) in &trusted {
            let cos = axis_cos(a, d);
            for (bucket, &angle) in coherent.iter_mut().zip(COHERENCE_ANGLES_DEG.iter()) {
                if cos >= angle.to_radians().cos() {
                    *bucket += w;
                }
            }
        }
        for (slot, &c) in within.iter_mut().zip(coherent.iter()) {
            // Denominator: TOTAL zone area. An untrusted cell never counts
            // as within — the conservative choice, as in the census.
            *slot = ratio(c, area);
        }
    }

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

    // w30 is index 2 of COHERENCE_ANGLES_DEG.
    let coherent_enough = within[2] >= GATE2_W30_MIN;
    let verdict = if queries == 0 {
        Verdict::NotMeasurable
    } else if area < MIN_VERDICT_AREA_MM2 {
        Verdict::TooSmall
    } else if coherent_enough && coherence_length >= GATE2_LENGTH_MIN_MM {
        Verdict::Usable
    } else if within[2] < 0.50 {
        Verdict::NotUsable
    } else {
        Verdict::Marginal
    };

    ZoneStats {
        cells: members.len(),
        area_mm2: area,
        trusted_area_mm2: trusted_area,
        degenerate_fraction: ratio(degenerate_area, area),
        dominant,
        within,
        coherence_length_mm: coherence_length,
        censored_fraction,
        queries,
        verdict,
    }
}

fn dominant_deg(stats: &ZoneStats) -> f64 {
    match stats.dominant {
        Some(d) => {
            let deg = d[1].atan2(d[0]).to_degrees();
            if deg < 0.0 { deg + 180.0 } else { deg }
        }
        None => f64::NAN,
    }
}

fn print_zone(label: &str, stats: &ZoneStats) {
    let censor_mark = if stats.censored_fraction > 0.5 {
        ">="
    } else {
        "  "
    };
    println!(
        "  {:<24} {:>9.1} {:>7.3} {:>6.3} {:>6.3} {:>6.3} {:>6.3} {:>6.1} {}{:>7.3} {:>6.2} {:>5.2} {:>10}",
        label,
        stats.area_mm2,
        ratio(stats.trusted_area_mm2, stats.area_mm2),
        stats.within[0],
        stats.within[1],
        stats.within[2],
        stats.within[3],
        dominant_deg(stats),
        censor_mark,
        stats.coherence_length_mm,
        stats.coherence_length_mm / STEPOVER_MM,
        stats.censored_fraction,
        stats.verdict.label()
    );
    println!(
        "  {:<24} {} cells, degenerate area fraction {:.4}, {} coherence queries",
        "", stats.cells, stats.degenerate_fraction, stats.queries
    );
}

fn zone_header() {
    println!(
        "  {:<24} {:>9} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>9} {:>6} {:>5} {:>10}",
        "zone",
        "area_mm2",
        "trust",
        "w10",
        "w20",
        "w30",
        "w45",
        "dom_deg",
        "cohl_mm",
        "steps",
        "cens",
        "verdict"
    );
}

/// Square tiles over the field cells. Restated from the census's tile_zones.
fn tile_zones(field: &[CellField], size: f64) -> Vec<Vec<u32>> {
    if field.is_empty() || size <= 0.0 {
        return Vec::new();
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for entry in field {
        min_x = min_x.min(entry.pos.x);
        min_y = min_y.min(entry.pos.y);
        max_x = max_x.max(entry.pos.x);
        max_y = max_y.max(entry.pos.y);
    }
    let x0 = (min_x / size).floor() * size;
    let y0 = (min_y / size).floor() * size;
    let cols = (((max_x - x0) / size).floor() as i64 + 1).max(1);
    let rows = (((max_y - y0) / size).floor() as i64 + 1).max(1);
    let mut tiles: Vec<Vec<u32>> = vec![Vec::new(); (cols * rows) as usize];
    for (i, entry) in field.iter().enumerate() {
        let col = (((entry.pos.x - x0) / size).floor() as i64).clamp(0, cols - 1);
        let row = (((entry.pos.y - y0) / size).floor() as i64).clamp(0, rows - 1);
        tiles[(row * cols + col) as usize].push(i as u32);
    }
    tiles.retain(|t| !t.is_empty());
    tiles
}

/// Everything the gate-2 verdict needs.
struct Gate2Verdict {
    whole: ZoneStats,
    regime_along: ZoneStats,
    regime_across: ZoneStats,
    regime_coverage: f64,
}

fn measure_gate2(label: &str, field: &[CellField], census: &FieldCensus) -> Gate2Verdict {
    println!();
    println!("── GATE 2 [{label}]: coherence census ─────────────────────────────────");
    println!(
        "  field: {} cells fitted, {} under-determined, {} ill-conditioned, {} degenerate (untrusted)",
        census.fitted, census.under_determined, census.ill_conditioned, census.degenerate
    );

    let all: Vec<u32> = (0..field.len() as u32).collect();
    let split_cos = REGIME_SPLIT_DEG.to_radians().cos();
    let x_axis = [1.0, 0.0];
    let mut along: Vec<u32> = Vec::new();
    let mut across: Vec<u32> = Vec::new();
    for (i, entry) in field.iter().enumerate() {
        if let Some(axis) = entry.axis {
            if axis_cos(axis, x_axis) >= split_cos {
                along.push(i as u32);
            } else {
                across.push(i as u32);
            }
        }
    }

    let whole = census_zone(field, &all);
    let regime_along = census_zone(field, &along);
    let regime_across = census_zone(field, &across);
    let regime_coverage =
        (regime_along.area_mm2 + regime_across.area_mm2) / whole.area_mm2.max(1e-9);

    zone_header();
    print_zone("whole (reference)", &whole);
    print_zone("regime ALONG (t1~x)", &regime_along);
    print_zone("regime ACROSS (t1~y)", &regime_across);
    println!(
        "  regime coverage: {:.3} of zone area carries a trusted direction in one regime [bar {GATE2_REGIME_COVERAGE_MIN}]",
        regime_coverage
    );
    println!(
        "  note: regime zones contain only trusted cells by construction, so their w-columns are"
    );
    println!(
        "  selection-biased at 45 degrees; the coverage figure and the 30-degree bar carry the load."
    );

    // Size control: tiles.
    println!("  tiles (size control):");
    for &size in &TILE_SIZES_MM {
        let tiles = tile_zones(field, size);
        let mut total = 0.0f64;
        let mut usable = 0.0f64;
        let mut w30_pass = 0.0f64;
        let mut count_usable = 0usize;
        for members in &tiles {
            let stats = census_zone(field, members);
            total += stats.area_mm2;
            if stats.within[2] >= GATE2_W30_MIN {
                w30_pass += stats.area_mm2;
            }
            if stats.verdict == Verdict::Usable {
                usable += stats.area_mm2;
                count_usable += 1;
            }
        }
        println!(
            "    {size:>5.1} mm tiles: {:>4} tiles, area in w30-passing tiles {:.3}, in USABLE tiles {:.3} ({count_usable} tiles)",
            tiles.len(),
            ratio(w30_pass, total),
            ratio(usable, total)
        );
    }

    Gate2Verdict {
        whole,
        regime_along,
        regime_across,
        regime_coverage,
    }
}

// ════════════════════════════════════════════════════════════════════════
// The fixture pipeline
// ════════════════════════════════════════════════════════════════════════

struct FixtureRun {
    gate1: Gate1Verdict,
    gate2: Gate2Verdict,
    census: MeshCensus,
}

fn eligible(mesh: &TriangleMesh, index: &SpatialIndex, candidates: &[P2]) -> Vec<(P2, f64)> {
    candidates
        .par_iter()
        .filter_map(|&p| surface_z(mesh, index, p).map(|z| (p, z)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn run_fixture(
    label: &str,
    height: impl Fn(f64, f64) -> f64 + Sync,
    extent: (f64, f64),
    step: f64,
    gate1_lattice: f64,
) -> FixtureRun {
    println!();
    println!("======================================================================");
    println!(
        "FIXTURE [{label}]: {} x {} mm, grid {step} mm",
        extent.0, extent.1
    );
    println!("======================================================================");

    let mesh = tessellate_heightfield((0.0, extent.0), (0.0, extent.1), step, &height);
    let census = mesh_census(&mesh);
    println!(
        "  mesh: {} triangles, {} vertices, surface area {:.0} mm2",
        mesh.triangles.len(),
        mesh.vertices.len(),
        census.area_mm2
    );
    println!(
        "  z range [{:.2}, {:.2}] mm (relief {:.2}), min face normal.z {:.4}",
        census.z_min,
        census.z_max,
        census.z_max - census.z_min,
        census.min_normal_z
    );
    println!(
        "  3-D edge min/median/max: {:.4} / {:.4} / {:.4} mm; bar max <= stepover/3 = {:.5} mm -> ratio {:.3}",
        census.edge_min_mm,
        census.edge_median_mm,
        census.edge_max_mm,
        FACET_MAX_EDGE_MM,
        census.edge_max_mm / FACET_MAX_EDGE_MM
    );
    // Fixture requirements, asserted not assumed.
    assert!(
        census.edge_max_mm <= FACET_MAX_EDGE_MM,
        "[{label}] facet budget violated: max 3-D edge {:.5} > stepover/3 = {:.5}",
        census.edge_max_mm,
        FACET_MAX_EDGE_MM
    );
    assert!(
        census.min_normal_z > 0.0,
        "[{label}] mesh is not 100 % up-facing (min normal.z = {})",
        census.min_normal_z
    );

    let index = SpatialIndex::build_auto(&mesh);
    let inset_x = (EDGE_INSET_MM, extent.0 - EDGE_INSET_MM);
    let inset_y = (EDGE_INSET_MM, extent.1 - EDGE_INSET_MM);

    // Gate 1.
    let candidates = lattice(inset_x, inset_y, gate1_lattice);
    let samples = eligible(&mesh, &index, &candidates);
    println!(
        "  gate-1 lattice: {} points at {gate1_lattice} mm (inset {EDGE_INSET_MM} mm), {} on-surface",
        candidates.len(),
        samples.len()
    );
    let axis = pca_axis(&candidates);
    let rows = measure_gate1(&mesh, &index, &samples, gate1_lattice, axis);
    let gate1 = gate1_verdict(label, &rows);

    // Gate 2.
    let candidates2 = lattice(inset_x, inset_y, GATE2_LATTICE_MM);
    let samples2 = eligible(&mesh, &index, &candidates2);
    println!();
    println!(
        "  gate-2 lattice: {} points at {GATE2_LATTICE_MM} mm, {} on-surface",
        candidates2.len(),
        samples2.len()
    );
    let (field, field_census) = build_cell_field(
        &mesh,
        &index,
        &samples2,
        GATE2_LATTICE_MM * GATE2_LATTICE_MM,
    );
    let gate2 = measure_gate2(label, &field, &field_census);

    FixtureRun {
        gate1,
        gate2,
        census,
    }
}

// ════════════════════════════════════════════════════════════════════════
// The gate run
// ════════════════════════════════════════════════════════════════════════

/// **The Track D gate stage.** Builds the analytic bike-seat sheet and the
/// noise control, runs both censuses on both, and prints PASS/FAIL against
/// the pre-registered bars. It generates no toolpath.
#[test]
#[ignore = "evidence run — minutes of release-mode compute; results land in planning/bikeseat_gate_2026-09-01/FINDINGS.md"]
fn bikeseat_gate_censuses() {
    println!();
    println!("########################################################################");
    println!("# TRACK D GATE STAGE — bike-seat fixture class vs the two Wanaka gates #");
    println!("########################################################################");
    println!();
    println!("PRE-REGISTERED (planning/bikeseat_gate_2026-09-01/FINDINGS.md, written first):");
    println!(
        "  GATE 1: prize ceiling >= {GATE1_CEILING_MIN_PCT} % at r = {VERDICT_FIT_RADIUS_MM} mm, R = {TOOL_RADIUS_MM} mm, h = {SCALLOP_H_MM} mm,"
    );
    println!("          with both anti-faceting scale rules clean.");
    println!(
        "  GATE 2: (a) whole-sheet median coherence length >= {GATE2_LENGTH_MIN_STEPOVERS} stepovers = {GATE2_LENGTH_MIN_MM:.3} mm,"
    );
    println!(
        "          (b) both 45-degree orientation regimes read w30 >= {GATE2_W30_MIN} and cover >= {GATE2_REGIME_COVERAGE_MIN} of sheet area."
    );
    println!(
        "  FIXTURE: max edge <= {FACET_MAX_EDGE_MM:.5} mm, up-facing, relief >= {FIXTURE_RELIEF_MIN_MM} mm, mean(s_max)/min(s_max) >= {FIXTURE_SMAX_SPREAD_MIN}."
    );
    println!(
        "  CONTROL: the noise patch must FAIL gate 2 (expected coherence < 1 mm, whole w30 < 0.5)"
    );
    println!("           for a sheet pass to mean anything. Gate 1 may pass on it, as on Wanaka.");
    println!(
        "  references: wanaka ceiling {WANAKA_REGION1_CEILING_PCT} %, median ratio {WANAKA_REGION1_MEDIAN_RATIO}, w30 <= {WANAKA_W30_BEST}, coherence {WANAKA_COHERENCE_MM} mm vs stepover {STEPOVER_MM} mm."
    );

    // ── the sheet ──
    let sheet = run_fixture(
        "BIKESEAT SHEET",
        seat_height,
        (SHEET_X_MM, SHEET_Y_MM),
        SHEET_STEP_MM,
        GATE1_LATTICE_MM,
    );
    let relief = sheet.census.z_max - sheet.census.z_min;
    assert!(
        relief >= FIXTURE_RELIEF_MIN_MM,
        "sheet relief {relief:.2} mm under the {FIXTURE_RELIEF_MIN_MM} mm fixture bar"
    );
    let smax_spread = sheet.gate1.smax_mean / sheet.gate1.smax.min;
    assert!(
        smax_spread >= FIXTURE_SMAX_SPREAD_MIN,
        "sheet mean(s_max)/min(s_max) = {smax_spread:.3} under the {FIXTURE_SMAX_SPREAD_MIN} fixture bar — the fixture cannot express the adaptive-spacing prize"
    );

    // ── the negative control ──
    let noise = run_fixture(
        "NOISE CONTROL",
        noise_height,
        (NOISE_XY_MM, NOISE_XY_MM),
        NOISE_STEP_MM,
        0.5,
    );

    // ── verdicts ──
    println!();
    println!("########################################################################");
    println!("# VERDICTS — against the pre-registered bars                           #");
    println!("########################################################################");

    let scale_clean = |g: &Gate1Verdict| {
        let excess_ok = g.excess_fired == Some(false);
        let decay_ok = g.decay_p.is_some_and(|p| p < TESSELLATION_DECAY_EXPONENT);
        excess_ok && decay_ok
    };

    let g1_pass = sheet.gate1.ceiling_pct >= GATE1_CEILING_MIN_PCT && scale_clean(&sheet.gate1);
    println!();
    println!(
        "GATE 1 (sheet): ceiling {:+.2} % vs bar {GATE1_CEILING_MIN_PCT} %, median ratio {:.4}, scale rules {} -> {}",
        sheet.gate1.ceiling_pct,
        sheet.gate1.median_ratio,
        if scale_clean(&sheet.gate1) {
            "clean"
        } else {
            "NOT clean"
        },
        if g1_pass { "PASS" } else { "FAIL" }
    );
    if sheet.gate1.gouge_area_frac > 0.02 {
        println!(
            "  caveat: {:.1} % gouge-censored area at R = {TOOL_RADIUS_MM}.",
            100.0 * sheet.gate1.gouge_area_frac
        );
    }

    let g2a = sheet.gate2.whole.coherence_length_mm >= GATE2_LENGTH_MIN_MM;
    let g2b = sheet.gate2.regime_along.within[2] >= GATE2_W30_MIN
        && sheet.gate2.regime_across.within[2] >= GATE2_W30_MIN
        && sheet.gate2.regime_coverage >= GATE2_REGIME_COVERAGE_MIN;
    let g2_pass = g2a && g2b;
    println!(
        "GATE 2 (sheet): G2-a coherence {:.3} mm = {:.2} stepovers vs bar {GATE2_LENGTH_MIN_STEPOVERS} -> {}",
        sheet.gate2.whole.coherence_length_mm,
        sheet.gate2.whole.coherence_length_mm / STEPOVER_MM,
        if g2a { "pass" } else { "fail" }
    );
    println!(
        "                G2-b regime w30 {:.3} / {:.3}, coverage {:.3} -> {}   ==> {}",
        sheet.gate2.regime_along.within[2],
        sheet.gate2.regime_across.within[2],
        sheet.gate2.regime_coverage,
        if g2b { "pass" } else { "fail" },
        if g2_pass { "PASS" } else { "FAIL" }
    );
    println!(
        "                whole-sheet w30 {:.3} (reference only — two-regime construction)",
        sheet.gate2.whole.within[2]
    );

    let c_g2a = noise.gate2.whole.coherence_length_mm >= GATE2_LENGTH_MIN_MM;
    let c_g2b = noise.gate2.regime_along.within[2] >= GATE2_W30_MIN
        && noise.gate2.regime_across.within[2] >= GATE2_W30_MIN
        && noise.gate2.regime_coverage >= GATE2_REGIME_COVERAGE_MIN;
    println!(
        "CONTROL (noise): gate 1 ceiling {:+.2} % ({}); gate 2 coherence {:.3} mm = {:.2} stepovers, whole w30 {:.3} -> {}",
        noise.gate1.ceiling_pct,
        if noise.gate1.ceiling_pct >= GATE1_CEILING_MIN_PCT {
            "passes, as Wanaka did"
        } else {
            "fails"
        },
        noise.gate2.whole.coherence_length_mm,
        noise.gate2.whole.coherence_length_mm / STEPOVER_MM,
        noise.gate2.whole.within[2],
        if c_g2a && c_g2b {
            "UNEXPECTED PASS — the instrument is suspect"
        } else {
            "FAILS, as required"
        }
    );

    println!();
    println!(
        "OVERALL: {}",
        match (g1_pass, g2_pass) {
            (true, true) => "GATES PASS — the full F1 pipeline on this fixture is justified.",
            (true, false) =>
                "GATE 2 FAILS — prize real, coherence absent: the Wanaka signature on the fixture's own turf.",
            (false, true) =>
                "GATE 1 FAILS — coherent but prize under bar: a raster at the right angle already wins here.",
            (false, false) =>
                "BOTH GATES FAIL — this construction cannot express the method's advantage.",
        }
    );
    println!("Track D stops here either way. Results land in FINDINGS.md.");
    println!("########################################################################");
}

// ════════════════════════════════════════════════════════════════════════
// Self-check: the extracted estimator on surfaces of known curvature
// ════════════════════════════════════════════════════════════════════════

/// Fit at one point of a synthetic patch, panicking with a readable message.
fn fit_at(mesh: &TriangleMesh, index: &SpatialIndex, at: P2, radius: f64) -> MongeFit {
    let z0 = surface_z(mesh, index, at).expect("sample lands on the synthetic patch");
    let mut scratch = MongeScratch::new(mesh.vertices.len());
    match fit_quadric(mesh, index, &mut scratch, (at, z0), radius) {
        MongeOutcome::Fitted(fit) => fit,
        MongeOutcome::UnderDetermined(n) => panic!("under-determined with {n} points at {at:?}"),
        MongeOutcome::IllConditioned => panic!("ill-conditioned at {at:?}"),
    }
}

/// **The one piece a reviewer can check without running the evidence test.**
///
/// Runs the EXTRACTED `common::monge` path against surfaces whose curvature
/// is known in closed form. Mirrors the assertions of
/// `wanaka_curvature_anisotropy::shape_operator_recovers_known_curvature`
/// plus the `t1`-axis checks the coherence census relies on, so the
/// extraction cannot have drifted from either source.
#[test]
fn monge_extraction_recovers_known_curvature_and_axis() {
    const RHO: f64 = 10.0;
    const FIT_R: f64 = 1.0;
    const REL: f64 = 0.02;
    const ABS: f64 = 2e-3;
    let patch = |half: f64, step: f64, f: &dyn Fn(f64, f64) -> f64| {
        tessellate_heightfield((-half, half), (-half, half), step, f)
    };

    // 1. Sphere cap: convex must read POSITIVE, umbilic ratio must be 1.
    let sphere = patch(6.5, 0.2, &|x, y| (RHO * RHO - x * x - y * y).sqrt());
    let sphere_index = SpatialIndex::build_auto(&sphere);
    for &(x, label) in &[(0.0, "apex"), (5.0, "30-degree slope")] {
        let fit = fit_at(&sphere, &sphere_index, P2::new(x, 0.0), FIT_R);
        for (name, value) in [("k1", fit.kappa1), ("k2", fit.kappa2)] {
            assert!(
                (value - 1.0 / RHO).abs() <= REL / RHO,
                "sphere {label}: {name} = {value:.6}, expected {:.6} (convex must read POSITIVE)",
                1.0 / RHO
            );
        }
        let ratio = ((fit.kappa1 + 1.0) / (fit.kappa2 + 1.0)).sqrt();
        assert!(
            (ratio - 1.0).abs() <= REL,
            "sphere {label}: W_max/W_min = {ratio:.6}, must be 1 on an umbilic"
        );
        // The isotropy floor must void the axis on an umbilic.
        assert!(
            fit.trusted_axis().is_none(),
            "sphere {label}: an umbilic must carry no trusted axis"
        );
    }

    // 2. Cylinder ridge along y: k1 = 1/rho across, k2 = 0 along; t1 = x.
    let cylinder = patch(6.5, 0.2, &|x, _| (RHO * RHO - x * x).sqrt());
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
    let axis = fit
        .trusted_axis()
        .expect("a cylinder is nowhere near umbilic");
    assert!(
        axis_cos(axis, [1.0, 0.0]) >= (2.0f64).to_radians().cos(),
        "cylinder: t1 must run ACROSS the ridge (x axis), got [{:.4}, {:.4}]",
        axis[0],
        axis[1]
    );
    // Feed across the ridge (t1) leaves kappa_perp = 0 — the widest strip.
    let across = kappa_perp_zou(&fit, (1.0, 0.0));
    assert!(
        across.abs() <= ABS,
        "feeding across the ridge must leave k_perp = 0, got {across:.6}"
    );
    let along = kappa_perp_zou(&fit, (0.0, 1.0));
    assert!(
        (along - 1.0 / RHO).abs() <= REL / RHO,
        "feeding along the ruling must leave k_perp = 1/rho, got {along:.6}"
    );
    let wide = strip_width(across, 1.0, SCALLOP_H_MM).expect("across is not a gouge");
    let narrow = strip_width(along, 1.0, SCALLOP_H_MM).expect("along is not a gouge");
    assert!(
        wide > narrow,
        "the widest cut must be in the most convex direction (Kim Eq. 23): {wide:.6} vs {narrow:.6}"
    );

    // 3. Tilted plane: zero curvature despite a large gradient.
    let plane = patch(4.0, 0.2, &|x, y| 0.5 * x + 0.3 * y);
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
        "tilted plane area weight = {:.6}, expected sqrt(1.34)",
        fit.area_weight
    );

    // 4. The gouge guard fires where the ball cannot fit, and NOWHERE else.
    let valley = patch(3.0, 0.05, &|x, _| {
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
        "a 1.0 mm ball in a 0.4 mm valley must be a GOUGE, never clamped"
    );
    assert!(
        strip_width(fit.kappa2, 0.2, SCALLOP_H_MM).is_some(),
        "a 0.2 mm ball fits the same valley and must NOT be a gouge"
    );

    // 5. Line-field mean: dominant_axis must not cancel t1 against -t1.
    let entries = vec![(1.0, [1.0, 0.0]), (1.0, [-1.0, 0.0]), (0.5, [0.0, 1.0])];
    let dom = dominant_axis(&entries).expect("a dominated field has a dominant axis");
    assert!(
        axis_cos(dom, [1.0, 0.0]) >= (5.0f64).to_radians().cos(),
        "dominant axis must be x, got [{:.4}, {:.4}]",
        dom[0],
        dom[1]
    );
    let _ = monge::MIN_FIT_POINTS;
}
