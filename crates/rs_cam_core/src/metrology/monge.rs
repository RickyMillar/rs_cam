//! Monge-quadric curvature estimation on an up-facing heightfield mesh.
//!
//! **Extraction, not new math.** This estimator was built by
//! `tests/wanaka_curvature_anisotropy.rs` (commit `7f341f9d`), restated by
//! `tests/zone_coherence_census.rs`, extracted to `tests/common/monge.rs`
//! for the bikeseat gate, and PROMOTED here by Track M (2026-09-02) so the
//! strategy censuses read one implementation. The census source files carry
//! the full derivation in their module docs. `tests/common/monge.rs`
//! re-exports this module, so instrument import paths are unchanged.
//! Validation: `bikeseat_gate_d1.rs::monge_extraction_recovers_known_curvature_and_axis`
//! runs this code against closed-form surfaces.
//!
//! The one merge this extraction performs: [`MongeFit`] carries BOTH the
//! fundamental forms (the anisotropy census needs them for
//! [`kappa_perp_zou`]) AND the `t1` axis (the coherence census needs it for
//! the line field). Each source file carried only its own half.
//!
//! Contract, restated in brief (see the source files for the derivation):
//!
//! * The mesh must be a single-valued heightfield `z = f(x, y)` — every facet
//!   up-facing. A mesh with an overhang needs a normal-aligned local frame,
//!   which this module does not provide.
//! * The fit is least squares over every mesh vertex within XY distance
//!   `radius` of the sample, in the scale-normalised frame `u = (x - x0)/r`.
//! * Curvature comes from the shape operator `S = I^-1 II`, never from the
//!   raw quadric coefficients — on slope the metric matters.
//! * The sign convention is **convex-positive** (Zou Eq. 2): a dome reads
//!   `+1/rho`, a valley reads `-1/rho`, and `kappa1 >= kappa2`.
//! * `axis` is the XY direction of `t1`, the most convex principal
//!   direction. It is a **line field**: `t1` and `-t1` are the same
//!   direction, so consumers must use [`axis_cos`] / [`dominant_axis`].
//! * `kappa2 + 1/R <= 0` is the gouge condition and is **never clamped**:
//!   [`strip_width`] returns `None` there.

use crate::geo::{P2, P3};
use crate::mesh::{QueryScratch, SpatialIndex, TriangleMesh};

/// Minimum gathered vertices for a fit: 6 quadric parameters + 1 degree of
/// freedom, so a residual exists at all. (Both source censuses use 7.)
pub const MIN_FIT_POINTS: usize = 7;

/// Pivot ratio (min |pivot| / max |pivot| on the scale-normalised normal
/// matrix) below which the fit is called rank deficient rather than solved.
pub const MIN_PIVOT_RATIO: f64 = 1e-9;

/// Isotropy floor, relative arm: `|kappa1 - kappa2| <= REL * max|kappa|`
/// voids the axis. From `direction_field::FieldParams`' defaults via the
/// coherence census.
pub const ISOTROPY_REL_TOL: f64 = 0.10;

/// Isotropy floor, absolute arm.
pub const ISOTROPY_ABS_TOL: f64 = 1e-6;

/// Below this, a denominator is treated as non-positive. Same role as
/// `direction_field::EPS_DENOM`.
pub const EPS_DENOM: f64 = 1e-12;

const EPS_VEC: f64 = 1e-12;

/// Per-thread reusable buffers for [`fit_quadric`]. Held by `map_init` in the
/// callers, so nothing here is allocated per sample.
pub struct MongeScratch {
    query: QueryScratch,
    tris: Vec<usize>,
    /// Generation stamp per mesh vertex — dedups the triangle-to-vertex
    /// expansion without clearing a bitset per sample.
    stamp: Vec<u32>,
    generation: u32,
}

impl MongeScratch {
    pub fn new(vertex_count: usize) -> Self {
        Self {
            query: QueryScratch::default(),
            tris: Vec::new(),
            // Stamps start at 0 and `generation` is incremented *before*
            // use, so generation 1 is the first live value and 0 never
            // matches.
            stamp: vec![0u32; vertex_count],
            generation: 0,
        }
    }
}

/// The local differential geometry at one sample. Everything downstream reads
/// this and nothing re-derives it.
#[derive(Clone, Copy)]
pub struct MongeFit {
    /// Max principal curvature, **convex-positive** (Zou). `kappa1 >= kappa2`.
    pub kappa1: f64,
    /// Min principal curvature, convex-positive.
    pub kappa2: f64,
    /// Unit XY direction of `t1`, sign arbitrary. `None` when the shape
    /// operator is a multiple of the identity (umbilic). NOT gated by the
    /// isotropy floor — apply [`trusted_axis`] for that.
    pub axis: Option<[f64; 2]>,
    /// First fundamental form.
    pub form_e: f64,
    pub form_f: f64,
    pub form_g: f64,
    /// Second fundamental form, Monge/upward-normal (so convex reads
    /// negative; [`kappa_perp_zou`] applies the flip).
    pub form_l: f64,
    pub form_m: f64,
    pub form_n: f64,
    /// `W = sqrt(1 + f_x^2 + f_y^2)` — the XY-to-surface area magnification.
    pub area_weight: f64,
    /// RMS fit residual (mm).
    pub residual_rms: f64,
    /// Vertices that entered the fit.
    pub points: usize,
    /// min|pivot| / max|pivot| on the scale-normalised normal matrix.
    pub pivot_ratio: f64,
}

impl MongeFit {
    /// The axis, voided when the anisotropy is at or under the isotropy
    /// floor. This is the coherence census's TRUSTED condition.
    pub fn trusted_axis(&self) -> Option<[f64; 2]> {
        let anisotropy = (self.kappa1 - self.kappa2).abs();
        let scale = self.kappa1.abs().max(self.kappa2.abs());
        let floor = ISOTROPY_ABS_TOL.max(ISOTROPY_REL_TOL * scale);
        if anisotropy <= floor { None } else { self.axis }
    }
}

/// Why a sample produced no fit — counted separately, never silently dropped.
pub enum MongeOutcome {
    Fitted(MongeFit),
    /// Fewer than [`MIN_FIT_POINTS`] vertices in the disc; carries how many
    /// there were.
    UnderDetermined(usize),
    /// Rank-deficient normal matrix (pivot ratio below
    /// [`MIN_PIVOT_RATIO`]), or a non-finite solution.
    IllConditioned,
}

/// Height of the heightfield at `(x, y)`, or `None` when the point is off
/// the surface.
///
/// Sound because the mesh is a heightfield and `cell_triangles_at` is a
/// guaranteed superset of the triangles a vertical ray at `(x, y)` can
/// pierce. `max` over the hits makes the answer the topmost surface, which
/// for a heightfield is the only surface.
// SAFETY: `index.cell_triangles_at` returns indices into `mesh.triangles`,
// and a triangle's vertex ids index `mesh.vertices` by mesh construction.
#[allow(clippy::indexing_slicing)]
pub fn surface_z(mesh: &TriangleMesh, index: &SpatialIndex, at: P2) -> Option<f64> {
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

/// Solve the symmetric 6x6 system `A x = b` by Gauss-Jordan with partial
/// pivoting, returning `(x, min|pivot| / max|pivot|)`.
///
/// Hand-rolled so the pivot ratio is available as a reported conditioning
/// number. `A` is expected already scale-normalised, so its pivots are O(1)
/// and their ratio is meaningful.
// SAFETY: every index is a constant or a loop counter bounded by the fixed
// 6x7 array dimensions.
#[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
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

/// Mirror the accumulated upper triangle into the lower one and scale
/// everything by `inv = 1/count`, so the normal matrix is a *mean* of rank-1
/// terms with O(1) entries at every radius.
// SAFETY: loop counters bounded by the fixed 6x6 dimensions.
#[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
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

/// Build a [`MongeFit`] from the derivatives of the fitted heightfield — the
/// shape-operator + sign-flip arithmetic, and the ONLY place it happens.
pub fn fit_from_derivatives(
    (f_x, f_y): (f64, f64),
    (f_xx, f_xy, f_yy): (f64, f64, f64),
    residual_rms: f64,
    points: usize,
    pivot_ratio: f64,
) -> MongeFit {
    let area_weight = (1.0 + f_x * f_x + f_y * f_y).sqrt();
    let form_e = 1.0 + f_x * f_x;
    let form_f = f_x * f_y;
    let form_g = 1.0 + f_y * f_y;
    let form_l = f_xx / area_weight;
    let form_m = f_xy / area_weight;
    let form_n = f_yy / area_weight;
    // EG - F^2 = 1 + f_x^2 + f_y^2 = W^2, always >= 1: no degenerate metric.
    let det = form_e * form_g - form_f * form_f;
    let gauss = (form_l * form_n - form_m * form_m) / det;
    let mean = (form_e * form_n - 2.0 * form_f * form_m + form_g * form_l) / (2.0 * det);
    // H^2 - K = ((ka - kb)/2)^2 >= 0 exactly; the max() absorbs round-off.
    let spread = (mean * mean - gauss).max(0.0).sqrt();

    // Shape operator entries, S = I^-1 II. `t1` (most convex after the flip)
    // is the eigenvector for the SMALLER Monge eigenvalue `mean - spread`.
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
    // A relative floor, so near-flat readings can still be called umbilic.
    let magnitude = s_a.abs().max(s_b.abs()).max(s_c.abs()).max(s_d.abs());
    let floor = (magnitude * 1e-9).max(f64::MIN_POSITIVE);
    let axis = if n1 >= n2 && n1 > floor {
        Some([row1[0] / n1, row1[1] / n1])
    } else if n2 > floor {
        Some([row2[0] / n2, row2[1] / n2])
    } else {
        None
    };

    MongeFit {
        // Convex-positive flip.
        kappa1: -(mean - spread),
        kappa2: -(mean + spread),
        axis,
        form_e,
        form_f,
        form_g,
        form_l,
        form_m,
        form_n,
        area_weight,
        residual_rms,
        points,
        pivot_ratio,
    }
}

/// Fit the local quadric at `at` (surface height `z0`) over fit radius
/// `radius`. One streaming pass: nothing is stored per neighbour.
// SAFETY: triangle ids index `mesh.triangles`, vertex ids index
// `mesh.vertices` and `scratch.stamp` (sized to the vertex count by
// `MongeScratch::new`); `beta`/`basis`/`normal` indices are constants
// bounded by the fixed 6-parameter quadric.
#[allow(clippy::indexing_slicing)]
pub fn fit_quadric(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    scratch: &mut MongeScratch,
    (at, z0): (P2, f64),
    radius: f64,
) -> MongeOutcome {
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
            count += 1;
        }
    }
    scratch.tris = tris;

    if count < MIN_FIT_POINTS {
        return MongeOutcome::UnderDetermined(count);
    }
    let inv = 1.0 / count as f64;
    finalise_normal_equations(&mut normal, &mut rhs, inv);
    let mean_w2 = sum_w2 * inv;

    let Some((beta, pivot_ratio)) = solve_sym6(&normal, &rhs) else {
        return MongeOutcome::IllConditioned;
    };
    if !pivot_ratio.is_finite() || pivot_ratio < MIN_PIVOT_RATIO {
        return MongeOutcome::IllConditioned;
    }
    if !beta.iter().all(|c| c.is_finite()) {
        return MongeOutcome::IllConditioned;
    }

    // At the least-squares solution A beta = b, so RSS/n = mean(w^2) - beta.b/n.
    let dot: f64 = beta.iter().zip(rhs.iter()).map(|(x, y)| x * y).sum();
    let residual_rms = (mean_w2 - dot).max(0.0).sqrt();

    let f_x = beta[3] / radius;
    let f_y = beta[4] / radius;
    let f_xx = 2.0 * beta[0] / (radius * radius);
    let f_xy = beta[1] / (radius * radius);
    let f_yy = 2.0 * beta[2] / (radius * radius);
    MongeOutcome::Fitted(fit_from_derivatives(
        (f_x, f_y),
        (f_xx, f_xy, f_yy),
        residual_rms,
        count,
        pivot_ratio,
    ))
}

/// Normal curvature **perpendicular** to the XY sweep direction `(p, q)`,
/// convex-positive. See the anisotropy census for the tangent-plane-rotation
/// proof.
pub fn kappa_perp_zou(fit: &MongeFit, dir: (f64, f64)) -> f64 {
    let (p, q) = dir;
    let pp = -(fit.form_f * p + fit.form_g * q);
    let qq = fit.form_e * p + fit.form_f * q;
    let num = fit.form_l * pp * pp + 2.0 * fit.form_m * pp * qq + fit.form_n * qq * qq;
    let den = fit.form_e * pp * pp + 2.0 * fit.form_f * pp * qq + fit.form_g * qq * qq;
    if den.abs() < EPS_DENOM {
        // Degenerate only if the tangent plane collapsed, which the metric
        // (EG - F^2 >= 1) forbids; return the mean rather than a NaN.
        return 0.5 * (fit.kappa1 + fit.kappa2);
    }
    -num / den
}

/// `W(kappa_n) = sqrt(8h / (kappa_n + 1/R))`, or `None` when
/// `kappa_n + 1/R <= 0` — the gouge condition. **Never clamped.**
pub fn strip_width(kappa_n: f64, tool_radius: f64, scallop_h: f64) -> Option<f64> {
    let denom = kappa_n + 1.0 / tool_radius;
    if !denom.is_finite() || denom <= EPS_DENOM {
        return None;
    }
    Some((8.0 * scallop_h / denom).sqrt())
}

// ── line-field arithmetic ───────────────────────────────────────────────

/// `|cos|` of the angle between two unit axes. `t1` and `-t1` are the same
/// direction, so the absolute value is the whole point.
pub fn axis_cos(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] * b[0] + a[1] * b[1]).abs().min(1.0)
}

/// The dominant direction of a line field: the principal eigenvector of the
/// weighted outer-product sum `sum(w t1 t1^T)`. `None` when the sum is
/// isotropic. A plain vector mean would cancel `t1` against `-t1`.
pub fn dominant_axis(entries: &[(f64, [f64; 2])]) -> Option<[f64; 2]> {
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

// ── statistics ──────────────────────────────────────────────────────────

/// Area-weighted five-number summary.
#[derive(Clone, Copy, Default)]
pub struct Quantiles {
    pub min: f64,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub max: f64,
}

/// `pairs` is `(value, area weight)`; consumed because it is sorted in place.
// SAFETY: guarded by the `is_empty` early return.
#[allow(clippy::indexing_slicing)]
pub fn quantiles(mut pairs: Vec<(f64, f64)>) -> Option<Quantiles> {
    if pairs.is_empty() {
        return None;
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    Some(Quantiles {
        min: pairs[0].0,
        p10: weighted_pick(&pairs, total, 0.10),
        p50: weighted_pick(&pairs, total, 0.50),
        p90: weighted_pick(&pairs, total, 0.90),
        max: pairs[pairs.len() - 1].0,
    })
}

/// First value whose cumulative area weight reaches `q * total`.
// SAFETY: callers pass a non-empty slice (the `quantiles` guard); the
// final index is `len - 1` of that slice.
#[allow(clippy::indexing_slicing)]
pub fn weighted_pick(sorted: &[(f64, f64)], total: f64, q: f64) -> f64 {
    if total <= 0.0 {
        return f64::NAN;
    }
    let target = q * total;
    let mut acc = 0.0;
    for &(value, weight) in sorted {
        acc += weight;
        if acc >= target {
            return value;
        }
    }
    sorted[sorted.len() - 1].0
}

/// Plain (unweighted) median of `values`, which this function sorts.
/// `NaN` for an empty slice.
// SAFETY: guarded by the `is_empty` early return; `mid` < `len`, and the
// even arm requires `len >= 2` so `mid - 1` is in range.
#[allow(clippy::indexing_slicing)]
pub fn median(mut values: Vec<f64>) -> f64 {
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

// ── analytic tessellation ───────────────────────────────────────────────

/// Tessellate `z = f(x, y)` over `[x0, x1] x [y0, y1]` at `step`, two
/// triangles per cell. Deterministic. Vertices lie EXACTLY on the analytic
/// surface, so a vertex fit measures the surface, not the tessellation.
pub fn tessellate_heightfield(
    x: (f64, f64),
    y: (f64, f64),
    step: f64,
    height: impl Fn(f64, f64) -> f64,
) -> TriangleMesh {
    let nx = ((x.1 - x.0) / step).round() as usize + 1;
    let ny = ((y.1 - y.0) / step).round() as usize + 1;
    let mut vertices = Vec::with_capacity(nx * ny);
    for row in 0..ny {
        for col in 0..nx {
            let px = x.0 + col as f64 * step;
            let py = y.0 + row as f64 * step;
            vertices.push(P3::new(px, py, height(px, py)));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (nx - 1) * (ny - 1));
    for row in 0..(ny - 1) {
        for col in 0..(nx - 1) {
            let a = (row * nx + col) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Regular lattice on multiples of `spacing`, inclusive of both ends.
/// Coordinates come from `index * spacing`, never a running accumulator.
pub fn lattice(x: (f64, f64), y: (f64, f64), spacing: f64) -> Vec<P2> {
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
