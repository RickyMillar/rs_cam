//! The two strategy-gate censuses: direction coherence and the curvature
//! anisotropy prize — with their gate thresholds as named constants.
//!
//! Promoted by Track M (2026-09-02) from `tests/zone_coherence_census.rs`
//! (the coherence census: `TurnGrid`, the per-zone aggregation, the
//! usability verdict) and `tests/wanaka_curvature_anisotropy.rs` (the
//! prize cell: anisotropy ratio distribution and the fixed-direction
//! prize bounds). Both instruments carry the full derivations in their
//! module docs; this module carries the code and the bars, so a gate that
//! quotes "w30 ≥ 0.70" quotes ONE constant.

use crate::geo::P2;
use crate::metrology::monge::{
    MongeFit, Quantiles, axis_cos, dominant_axis, kappa_perp_zou, median, quantiles, strip_width,
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

// ── the gate thresholds, named ──────────────────────────────────────────

/// The turn that ends a coherent run, in degrees.
///
/// Evidence: 30° is the direction-error budget at which the anisotropy
/// prize survives — `tests/wanaka_curvature_anisotropy.rs` measured the
/// prize's decay with misalignment, and the thin-organic programme's zone
/// censuses report coherence at 10/20/30/45° with 30° as the verdict
/// column (`planning/thin_organic_2026-08-27/FINDINGS.md`).
pub const COHERENCE_TURN_DEG: f64 = 30.0;

/// Angles at which the coherence fraction is reported.
pub const COHERENCE_ANGLES_DEG: [f64; 4] = [10.0, 20.0, 30.0, 45.0];

/// Index of 30 degrees in [`COHERENCE_ANGLES_DEG`] — the verdict column.
pub const VERDICT_ANGLE_INDEX: usize = 2;

/// `W30_COHERENT` — the coherence-fraction bar a zone must clear to be
/// USABLE: at least this fraction of the zone's area must carry a trusted
/// `t1` within 30° of the zone's dominant direction.
///
/// Evidence: on Wanaka region 1 every shipped decomposition reads
/// `w30 ≤ 0.43` (`tests/zone_coherence_census.rs`, evidence commit
/// `a17de695`/`d22e8b53`: "no shipped decomposition is a coherent zone —
/// the field turns inside one stepover"); the bikeseat control geometry
/// reads `w30 ≥ 0.70` per orientation regime
/// (`planning/bikeseat_gate_2026-09-01/FINDINGS.md`). The bar sits at the
/// bikeseat side of that separation. The bikeseat gate-2 bar
/// (`GATE2_W30_MIN`) is this same value.
pub const W30_COHERENT: f64 = 0.70;

/// Coherence fraction below which a zone is NOT-USABLE outright.
pub const NOT_USABLE_W30_BELOW: f64 = 0.50;

/// The coherence-length bar for a USABLE zone, in stepovers: the median
/// distance to the nearest trusted triangle whose axis turns more than
/// [`COHERENCE_TURN_DEG`] must reach 10 stepovers.
///
/// Evidence: Wanaka region 1's coherence length is 0.35 mm against a
/// 0.4862 mm stepover — the field turns INSIDE one stepover, so no raster
/// can follow it (`tests/zone_coherence_census.rs`; restated as the
/// failure reference in `tests/bikeseat_gate_d1.rs`). Note the bikeseat
/// GATE uses its own, weaker 3-stepover bar (`GATE2_LENGTH_MIN_STEPOVERS`)
/// because its gate asks "is there any usable run", not "is the zone
/// raster-grade"; that bar stays with the gate.
pub const COHERENCE_LENGTH_MIN_STEPOVERS: f64 = 10.0;

/// Coherence-length search bound, in stepovers: twice the usability bar,
/// so the bound never decides a passing zone.
pub const COHERENCE_SEARCH_BOUND_STEPOVERS: f64 = 20.0;

/// Bucket-grid cell (mm) for the coherence-length search.
pub const COHERENCE_GRID_CELL_MM: f64 = 1.0;

/// Query cap per zone. Above this the queries are a deterministic stride
/// over triangle-index order.
pub const COHERENCE_QUERY_CAP: usize = 4_000;

/// Below this many trusted triangles the coherence length is undefined.
pub const MIN_TRUSTED_TRIANGLES: usize = 2;

/// Anisotropy-ratio bands the prize census reports area fractions above.
pub(crate) const PRIZE_RATIO_BANDS: [f64; 4] = [1.05, 1.10, 1.25, 1.50];

/// The anisotropy prize bar, lower edge: an area-weighted median
/// `W_max/W_min` at or under this means the prize is under ~2 % — close
/// the direction-field method on evidence.
///
/// Evidence: `tests/wanaka_curvature_anisotropy.rs` (verdict table);
/// Wanaka region 1 measured median ratio 1.0950 at `R = 1.0` — a
/// **+9.75 %** prize ceiling ([`WANAKA_REGION1_PRIZE_CEILING_PCT`],
/// `planning/finishing_synthesis_2026-08-30.md` §11) — which sits in the
/// literature band below.
///
/// **Test door.** The harness
/// `crates/rs_cam_core/tests/wanaka_curvature_anisotropy.rs` is the only
/// caller. No production path reads it.
pub const PRIZE_CLOSE_BELOW: f64 = 1.05;

/// The anisotropy prize bar, upper edge: above this the measured prize is
/// LARGER than the literature's own 1.9–7.2 % band (Kumazawa) and needs
/// explaining before it is believed.
///
/// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
/// only callers. No production path reads it.
pub const PRIZE_ABOVE_LITERATURE: f64 = 1.25;

/// Wanaka region 1's measured prize ceiling at `R = 1.0`, percent —
/// the same-scale reference every later census compares against
/// (`planning/finishing_synthesis_2026-08-30.md` §11; restated by both
/// census instruments and the bikeseat gate).
pub const WANAKA_REGION1_PRIZE_CEILING_PCT: f64 = 9.75;

// ── the per-triangle field ──────────────────────────────────────────────

/// Everything the coherence census needs about one mesh triangle. The
/// instrument builds it (from [`crate::metrology::monge::fit_quadric`] or
/// its own estimator) and the census only reads it.
#[derive(Clone, Copy)]
pub struct TriField {
    /// XY centroid. Zone membership and every distance read this.
    pub centroid: P2,
    /// True 3-D surface area (mm²). Every area fraction is weighted by it.
    pub area_mm2: f64,
    /// Unit XY `t1`, sign arbitrary. `Some` only when the fit succeeded,
    /// the shape operator is not a multiple of the identity, and the
    /// anisotropy clears the isotropy floor — the TRUSTED condition.
    pub axis: Option<[f64; 2]>,
    /// The fit itself succeeded, whatever the anisotropy was.
    pub fitted: bool,
    /// Fitted, but umbilic or below the isotropy floor. `t1` carries no
    /// meaning here. Disjoint from `axis.is_some()`.
    pub degenerate: bool,
}

/// `a / b`, or 0 when `b` is not positive. Keeps every reported fraction
/// free of NaN when a zone is empty.
#[must_use]
pub fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b } else { 0.0 }
}

// ── the coherence-length search ─────────────────────────────────────────

/// Bucket grid over a zone's trusted triangles, for the nearest-turn
/// search. Promoted verbatim from `tests/zone_coherence_census.rs`.
pub struct TurnGrid {
    pts: Vec<P2>,
    axes: Vec<[f64; 2]>,
    origin: P2,
    cols: i64,
    rows: i64,
    buckets: Vec<Vec<u32>>,
    cell: f64,
}

impl TurnGrid {
    /// `None` when the zone holds fewer than [`MIN_TRUSTED_TRIANGLES`].
    #[must_use]
    pub fn build(field: &[TriField], members: &[u32]) -> Option<Self> {
        let mut pts = Vec::new();
        let mut axes = Vec::new();
        for &m in members {
            let Some(entry) = field.get(m as usize) else {
                continue;
            };
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
            if let Some(bucket) = buckets.get_mut((r * cols + c) as usize) {
                bucket.push(i as u32);
            }
        }
        Some(Self {
            pts,
            axes,
            origin: P2::new(min_x, min_y),
            cols,
            rows,
            buckets,
            cell,
        })
    }

    /// Trusted points in the grid.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pts.is_empty()
    }

    fn cell_of(&self, p: P2) -> (i64, i64) {
        let c = (((p.x - self.origin.x) / self.cell).floor() as i64).clamp(0, self.cols - 1);
        let r = (((p.y - self.origin.y) / self.cell).floor() as i64).clamp(0, self.rows - 1);
        (c, r)
    }

    /// Distance to the nearest trusted member whose axis differs from
    /// `from`'s by more than the turn whose cosine is `cos_turn`. `None`
    /// means the search was right-censored: nothing differing lies inside
    /// `bound_mm`.
    ///
    /// Ring `k` holds the cells at Chebyshev distance `k` from the query's
    /// cell. After ring `k` is processed, every unvisited member lies at
    /// Euclidean distance at least `k * cell`, because the query can sit
    /// anywhere inside its own cell. The loop stops on that guarantee.
    #[must_use]
    pub fn nearest_turn(&self, from: usize, cos_turn: f64, bound_mm: f64) -> Option<f64> {
        let (Some(&p), Some(&a)) = (self.pts.get(from), self.axes.get(from)) else {
            return None;
        };
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
                    let Some(bucket) = self.buckets.get((r * self.cols + c) as usize) else {
                        continue;
                    };
                    for &idx in bucket {
                        let i = idx as usize;
                        let (Some(&q), Some(&axis)) = (self.pts.get(i), self.axes.get(i)) else {
                            continue;
                        };
                        if i == from || axis_cos(a, axis) >= cos_turn {
                            continue;
                        }
                        let dx = q.x - p.x;
                        let dy = q.y - p.y;
                        let d = (dx * dx + dy * dy).sqrt();
                        if d < best {
                            best = d;
                        }
                    }
                }
            }
            let guaranteed = ring as f64 * self.cell;
            if best <= guaranteed || guaranteed > bound_mm {
                break;
            }
        }
        (best <= bound_mm).then_some(best)
    }
}

// ── the coherence census ────────────────────────────────────────────────

/// The pre-registered verdict. Only [`ZoneVerdict::Usable`] counts as
/// usable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZoneVerdict {
    Usable,
    /// Between the two bars, or over the fraction bar with too short a
    /// coherence length.
    Marginal,
    /// Under [`NOT_USABLE_W30_BELOW`].
    NotUsable,
    /// Too small to hold [`COHERENCE_LENGTH_MIN_STEPOVERS`] stepovers in
    /// both directions.
    TooSmall,
    /// Under [`MIN_TRUSTED_TRIANGLES`] trusted triangles.
    NotMeasurable,
}

impl ZoneVerdict {
    #[must_use]
    pub const fn label(self) -> &'static str {
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
pub struct ZoneStats {
    pub triangles: usize,
    pub area_mm2: f64,
    /// Area of triangles whose fit succeeded, whatever the anisotropy.
    pub fitted_area_mm2: f64,
    /// Area of triangles carrying a believed `t1`.
    pub trusted_area_mm2: f64,
    /// Fraction of TRIANGLES whose fit succeeded.
    pub fit_fraction: f64,
    /// Fraction of AREA that is fitted but degenerate.
    pub degenerate_fraction: f64,
    pub dominant: Option<[f64; 2]>,
    /// Coherent area at [`COHERENCE_ANGLES_DEG`], over TOTAL zone area.
    /// Index [`VERDICT_ANGLE_INDEX`] is the `w30` the gates read.
    pub within: [f64; 4],
    /// The same, over trusted area only — so a turning field and an absent
    /// field stay separable.
    pub within_trusted: [f64; 4],
    /// Median coherence length (mm). Censored queries enter at the bound.
    pub coherence_length_mm: f64,
    /// Fraction of queries that found no turn inside the bound.
    pub censored_fraction: f64,
    pub queries: usize,
    /// XY bbox extent of the zone's centroids (mm).
    pub extent_mm: [f64; 2],
    pub verdict: ZoneVerdict,
}

impl ZoneStats {
    /// The verdict column: coherent area fraction at 30°.
    #[must_use]
    pub fn w30(&self) -> f64 {
        self.within
            .get(VERDICT_ANGLE_INDEX)
            .copied()
            .unwrap_or(f64::NAN)
    }
}

/// Measure one zone. `members` are triangle indices into `field`;
/// `stepover_mm` scales the length bars (the census is stepover-relative
/// by design — a zone is usable FOR a tool, not in the abstract).
///
/// The queries are the zone's trusted triangles, capped by a
/// deterministic stride over triangle-index order. The SEARCH set is
/// every trusted triangle of the zone, never a subsample.
#[must_use]
pub fn census_zone(field: &[TriField], members: &[u32], stepover_mm: f64) -> ZoneStats {
    let usable_length_min_mm = COHERENCE_LENGTH_MIN_STEPOVERS * stepover_mm;
    let search_bound_mm = COHERENCE_SEARCH_BOUND_STEPOVERS * stepover_mm;
    let min_verdict_area_mm2 = usable_length_min_mm * usable_length_min_mm;

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
        let Some(entry) = field.get(m as usize) else {
            continue;
        };
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

    // Coherence length.
    let mut coherence_length = f64::NAN;
    let mut censored_fraction = f64::NAN;
    let mut queries = 0usize;
    if let Some(grid) = TurnGrid::build(field, members) {
        let cos_turn = COHERENCE_TURN_DEG.to_radians().cos();
        let stride = grid.len().div_ceil(COHERENCE_QUERY_CAP).max(1);
        let picks: Vec<usize> = (0..grid.len()).step_by(stride).collect();
        #[cfg(feature = "parallel")]
        let found: Vec<Option<f64>> = picks
            .par_iter()
            .with_min_len(64)
            .map(|&i| grid.nearest_turn(i, cos_turn, search_bound_mm))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let found: Vec<Option<f64>> = picks
            .iter()
            .map(|&i| grid.nearest_turn(i, cos_turn, search_bound_mm))
            .collect();
        queries = found.len();
        let censored = found.iter().filter(|d| d.is_none()).count();
        censored_fraction = censored as f64 / queries.max(1) as f64;
        let distances: Vec<f64> = found.iter().map(|d| d.unwrap_or(search_bound_mm)).collect();
        coherence_length = median(distances);
    }

    let extent_mm = if members.is_empty() {
        [0.0, 0.0]
    } else {
        [max_x - min_x, max_y - min_y]
    };
    let w30 = within.get(VERDICT_ANGLE_INDEX).copied().unwrap_or(0.0);
    let coherent_enough = w30 >= W30_COHERENT;
    let verdict = if queries == 0 {
        ZoneVerdict::NotMeasurable
    } else if area < min_verdict_area_mm2 {
        ZoneVerdict::TooSmall
    } else if coherent_enough && coherence_length >= usable_length_min_mm {
        ZoneVerdict::Usable
    } else if w30 < NOT_USABLE_W30_BELOW {
        ZoneVerdict::NotUsable
    } else {
        ZoneVerdict::Marginal
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

// ── the anisotropy prize census ─────────────────────────────────────────

/// The per-tool prize cell: anisotropy-ratio distribution plus the three
/// fixed-direction prize bounds. Promoted from
/// `tests/wanaka_curvature_anisotropy.rs::tool_report`.
pub struct PrizeCell {
    pub tool_radius_mm: f64,
    pub gouge_samples: usize,
    pub gouge_area_frac: f64,
    /// Area-weighted `W_max / W_min` distribution — the per-point spread
    /// the entire direction-field prize lives in.
    pub ratio: Quantiles,
    /// Area fraction above each of `PRIZE_RATIO_BANDS`.
    pub frac_above: [f64; 4],
    /// `∫dA/W(fixed) ÷ ∫dA/W_max` for the three fixed sweep directions
    /// (X, Y, and the population's dominant axis). The BEST of these,
    /// minus 1, is the prize CEILING a perfect direction field could
    /// recover over the best fixed direction.
    pub bound_x: f64,
    pub bound_y: f64,
    pub bound_pca: f64,
}

impl PrizeCell {
    /// The prize bound: the best fixed-direction integral over the
    /// direction-optimal floor. `1.0` means no prize at all.
    ///
    /// **Test door.** The `#[cfg(test)]` module of this file is the only
    /// caller. No production path reads it (S29, tech debt 2026-09-16).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn best_fixed_bound(&self) -> f64 {
        self.bound_x.min(self.bound_y).min(self.bound_pca)
    }
}

/// Measure one (population, tool) prize cell. `fits` are the population's
/// Monge fits; `cell_area` is the XY area one sample represents, so
/// `fit.area_weight * cell_area` is its surface area.
#[must_use]
pub fn prize_cell(
    fits: &[MongeFit],
    cell_area: f64,
    tool_radius: f64,
    scallop_h: f64,
    axis: (f64, f64),
) -> PrizeCell {
    let mut ratios: Vec<(f64, f64)> = Vec::with_capacity(fits.len());
    let mut gouge_samples = 0usize;
    let mut gouge_area = 0.0f64;
    let mut total_area = 0.0f64;
    // The three fixed-direction integrals and the direction-optimal floor.
    let mut floor_opt = 0.0f64;
    let mut fixed = [0.0f64; 3];
    let directions = [(1.0, 0.0), (0.0, 1.0), axis];

    for fit in fits {
        let area = fit.area_weight * cell_area;
        total_area += area;
        // κ2 is the minimum normal curvature, so κ2 + 1/R ≤ 0 is the
        // tightest the denominator ever gets. If it survives, every
        // direction does.
        let (Some(w_max), Some(w_min)) = (
            strip_width(fit.kappa2, tool_radius, scallop_h),
            strip_width(fit.kappa1, tool_radius, scallop_h),
        ) else {
            gouge_samples += 1;
            gouge_area += area;
            continue;
        };
        ratios.push((w_max / w_min, area));
        floor_opt += area / w_max;
        for (slot, &dir) in fixed.iter_mut().zip(directions.iter()) {
            let kappa = kappa_perp_zou(fit, dir);
            match strip_width(kappa, tool_radius, scallop_h) {
                Some(width) => *slot += area / width,
                // Unreachable given the κ2 guard above (κ_perp ∈ [κ2, κ1]);
                // if it ever fires, charge the tightest admissible width
                // rather than silently dropping the area from one integral
                // only, which would bias the bound DOWNWARD.
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
    let mut frac_above = [0.0f64; 4];
    let ratio_area: f64 = ratios.iter().map(|r| r.1).sum();
    if ratio_area > 0.0 {
        for (slot, &band) in frac_above.iter_mut().zip(PRIZE_RATIO_BANDS.iter()) {
            *slot = ratios
                .iter()
                .filter(|r| r.0 > band)
                .map(|r| r.1)
                .sum::<f64>()
                / ratio_area;
        }
    }

    PrizeCell {
        tool_radius_mm: tool_radius,
        gouge_samples,
        gouge_area_frac: if total_area > 0.0 {
            gouge_area / total_area
        } else {
            0.0
        },
        ratio: quantiles(ratios).unwrap_or_default(),
        frac_above,
        bound_x: bound(fixed.first().copied().unwrap_or(f64::NAN)),
        bound_y: bound(fixed.get(1).copied().unwrap_or(f64::NAN)),
        bound_pca: bound(fixed.get(2).copied().unwrap_or(f64::NAN)),
    }
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
        COHERENCE_ANGLES_DEG, COHERENCE_TURN_DEG, TriField, VERDICT_ANGLE_INDEX, ZoneVerdict,
        census_zone, prize_cell,
    };
    use crate::geo::P2;
    use crate::metrology::monge::fit_from_derivatives;

    /// New pin (the promoted aggregation had none as a unit): a uniform
    /// field is fully coherent and USABLE; a field that alternates 90°
    /// every millimetre is NOT-USABLE with a ~1 mm coherence length.
    #[test]
    fn census_separates_uniform_from_alternating_fields() {
        assert!((COHERENCE_ANGLES_DEG[VERDICT_ANGLE_INDEX] - COHERENCE_TURN_DEG).abs() < 1e-12);
        let stepover = 0.5;
        let entry = |x: f64, y: f64, axis: [f64; 2]| TriField {
            centroid: P2::new(x, y),
            area_mm2: 1.0,
            axis: Some(axis),
            fitted: true,
            degenerate: false,
        };
        // 40 x 40 mm of uniform +X axes at 1 mm pitch.
        let mut uniform = Vec::new();
        let mut alternating = Vec::new();
        for row in 0..40 {
            for col in 0..40 {
                let (x, y) = (f64::from(col), f64::from(row));
                uniform.push(entry(x, y, [1.0, 0.0]));
                let axis = if (row + col) % 2 == 0 {
                    [1.0, 0.0]
                } else {
                    [0.0, 1.0]
                };
                alternating.push(entry(x, y, axis));
            }
        }
        let members: Vec<u32> = (0..uniform.len() as u32).collect();
        let coherent = census_zone(&uniform, &members, stepover);
        assert_eq!(coherent.verdict, ZoneVerdict::Usable);
        assert!(coherent.w30() > 0.99);
        // Censored everywhere: the length reads the bound, never infinity.
        assert!(coherent.coherence_length_mm >= 10.0 * stepover);

        let turning = census_zone(&alternating, &members, stepover);
        assert_eq!(turning.verdict, ZoneVerdict::NotUsable);
        // Nearest 90-degree turn is the 1 mm lattice neighbour.
        assert!((turning.coherence_length_mm - 1.0).abs() < 1e-9);
        assert!(turning.w30() <= 0.51, "w30 = {}", turning.w30());
    }

    /// New pin: on an isotropic population the prize is exactly 1.0 in
    /// every direction; on an anisotropic one the ratio is above 1 and a
    /// fixed direction can only be worse than the optimal floor.
    #[test]
    fn prize_cell_reads_one_on_isotropic_ground() {
        // A dome: f_xx = f_yy = -0.05 (umbilic, convex +0.05).
        let dome = fit_from_derivatives((0.0, 0.0), (-0.05, 0.0, -0.05), 0.0, 12, 1.0);
        let cell = prize_cell(&[dome], 1.0, 1.0, 0.03, (1.0, 0.0));
        assert_eq!(cell.gouge_samples, 0);
        assert!((cell.ratio.p50 - 1.0).abs() < 1e-9);
        assert!((cell.best_fixed_bound() - 1.0).abs() < 1e-9);

        // A cylinder: curved across X, flat along Y.
        let cyl = fit_from_derivatives((0.0, 0.0), (-0.2, 0.0, 0.0), 0.0, 12, 1.0);
        let cell = prize_cell(&[cyl], 1.0, 1.0, 0.03, (1.0, 0.0));
        assert!(cell.ratio.p50 > 1.0);
        assert!(cell.bound_x >= 1.0 - 1e-12);
        assert!(cell.bound_y >= 1.0 - 1e-12);
    }
}
