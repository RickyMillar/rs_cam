//! The finish-length floor: `L_min = ∫∫ dA / s_max(x)`, and the `× floor`
//! score.
//!
//! Promoted from `tests/conformal_spiral_synthetic_f2.rs` (`region_floor`,
//! `AreaWeighted`); restated copies lived in
//! `tests/spiral_finish_compact_c1.rs` and, as the flat-law reference, in
//! `tests/whole_board_spiral_ledger_g1.rs`.
//!
//! **§1 of `planning/finishing_synthesis_2026-08-30.md`.** `L_min` is the
//! shortest cutting distance that can meet the finish spec: every pass
//! spaced exactly at the local iso-scallop limit, nothing cut twice. It is
//! a HARD FLOOR, not a target — a path shorter than it has not met the
//! spec. A candidate's score is `cut_mm / l_min_mm`
//! ([`FloorReport::times_floor`]).
//!
//! # Which curvature goes into `s_max`, and why it is `κ_min`
//!
//! `s_max(x)` is the **maximum admissible** pass spacing at a point.
//! Spacing is taken ACROSS the passes, so the admissible value depends on
//! which direction the passes step in, and the largest of those — the one
//! a perfectly-oriented strategy could achieve — is the direction of LEAST
//! convex curvature. So the headline uses `κ_min`, which makes `s_max`
//! largest, `L_min` smallest, and the result a genuine **lower bound that
//! no strategy can beat while meeting spec**. A floor computed on `κ_max`
//! is a floor only for strategies forced to step across the worst
//! direction; it is reported beside the headline as exactly that, and on
//! an UMBILIC surface the two coincide identically.
//!
//! # Disclosed divergence between the promoted copies
//!
//! The `spiral_finish_compact_c1.rs` copy computed only the `κ_min` basis
//! and counted a triangle as degenerate on that basis alone; the F2 copy
//! computed both bases and counted a triangle as degenerate when EITHER
//! collapsed. This module keeps the F2 (both-bases) rule. On c1's umbilic
//! fixtures the two rules coincide, so c1's numbers do not move.

use crate::mesh::TriangleMesh;
use crate::scallop_math;

/// Area-weighted percentiles of a per-triangle scalar. `pairs` is
/// `(value, area)`.
///
/// Weighted by AREA, never by triangle count: a polar sphere-cap mesh's
/// innermost triangles are ~55× smaller than its rim ones, so an
/// unweighted percentile reports the geometry of the mesh's centre rather
/// than of the ground being cut.
#[derive(Debug, Clone, Copy)]
pub struct AreaWeighted {
    pub samples: usize,
    pub total_area_mm2: f64,
    pub min: f64,
    pub p50: f64,
    pub p90: f64,
    pub max: f64,
}

/// Build [`AreaWeighted`] from `(value, area)` pairs. Sorts its input.
#[must_use]
pub fn area_weighted(mut pairs: Vec<(f64, f64)>) -> AreaWeighted {
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    let at = |fraction: f64| -> f64 {
        if pairs.is_empty() || total <= 0.0 {
            return f64::NAN;
        }
        let target = fraction * total;
        let mut running = 0.0f64;
        for &(value, weight) in &pairs {
            running += weight;
            if running >= target {
                return value;
            }
        }
        pairs.last().map_or(f64::NAN, |p| p.0)
    };
    AreaWeighted {
        samples: pairs.len(),
        total_area_mm2: total,
        min: pairs.first().map_or(f64::NAN, |p| p.0),
        p50: at(0.50),
        p90: at(0.90),
        max: pairs.last().map_or(f64::NAN, |p| p.0),
    }
}

/// One region's theoretical minimum cutting distance, measured two ways.
#[derive(Debug, Clone)]
pub struct FloorReport {
    /// 3D surface area of the region — the `dA` the integral runs over.
    pub area_mm2: f64,
    /// `Σ area_t / s_max(t)` with `s_max` taken on the `κ_min` basis.
    /// **This is THE FLOOR.**
    pub l_min_mm: f64,
    /// The same sum on the `κ_max` basis — the direction-worst companion,
    /// never the headline (see the module doc).
    pub l_min_worst_mm: f64,
    /// Area-weighted distribution of `s_max(t)`, `κ_min` basis.
    pub s_max: AreaWeighted,
    /// Area-weighted distribution of `s_max(t)`, `κ_max` basis.
    pub s_worst: AreaWeighted,
    /// Triangles whose `s_max` came back non-positive on either basis and
    /// were therefore left out of the sum. Must be 0 on the analytic
    /// fixtures; a nonzero is a tripwire.
    pub degenerate: usize,
}

impl FloorReport {
    /// A candidate's `× floor` score: its cutting distance over the floor.
    /// `NaN` when the floor is empty — not measured, never zero.
    #[must_use]
    pub fn times_floor(&self, cutting_mm: f64) -> f64 {
        if self.l_min_mm > 0.0 {
            cutting_mm / self.l_min_mm
        } else {
            f64::NAN
        }
    }
}

/// Integrate the floor over `region` (triangle indices into `mesh`;
/// `None` = the whole mesh), with the principal curvatures supplied by
/// `principal_curvatures(cx, cy) -> (κ_min, κ_max)` in the
/// convex-positive convention, evaluated at each triangle's XY centroid.
///
/// The curvature source stays a parameter on purpose: the analytic
/// fixtures pass their closed forms so no estimator sits inside the floor,
/// and a mesh-only caller may pass a fitted estimator and thereby STATES
/// that its floor carries estimator error.
///
/// `s_max(t) = scallop_math::stepover_from_scallop_curved(ball_radius,
/// cusp_height, κ_t)`.
#[must_use]
pub fn region_floor(
    mesh: &TriangleMesh,
    region: Option<&[u32]>,
    ball_radius_mm: f64,
    cusp_height_mm: f64,
    principal_curvatures: &dyn Fn(f64, f64) -> (f64, f64),
) -> FloorReport {
    let count = region.map_or(mesh.faces.len(), <[u32]>::len);
    let mut best: Vec<(f64, f64)> = Vec::with_capacity(count);
    let mut worst: Vec<(f64, f64)> = Vec::with_capacity(count);
    let mut area_mm2 = 0.0f64;
    let mut l_min_mm = 0.0f64;
    let mut l_min_worst_mm = 0.0f64;
    let mut degenerate = 0usize;
    let indices: Vec<u32> = match region {
        Some(list) => list.to_vec(),
        None => (0..mesh.faces.len() as u32).collect(),
    };
    for t in indices {
        let Some(face) = mesh.faces.get(t as usize) else {
            continue;
        };
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        let area = 0.5 * e1.cross(&e2).norm();
        if area.is_nan() || area <= 0.0 {
            continue;
        }
        let cx = (face.v[0].x + face.v[1].x + face.v[2].x) / 3.0;
        let cy = (face.v[0].y + face.v[1].y + face.v[2].y) / 3.0;
        let (k_min, k_max) = principal_curvatures(cx, cy);
        let widest =
            scallop_math::stepover_from_scallop_curved(ball_radius_mm, cusp_height_mm, k_min);
        let tightest =
            scallop_math::stepover_from_scallop_curved(ball_radius_mm, cusp_height_mm, k_max);
        if !widest.is_finite() || widest <= 0.0 || !tightest.is_finite() || tightest <= 0.0 {
            degenerate += 1;
            continue;
        }
        area_mm2 += area;
        l_min_mm += area / widest;
        l_min_worst_mm += area / tightest;
        best.push((widest, area));
        worst.push((tightest, area));
    }
    FloorReport {
        area_mm2,
        l_min_mm,
        l_min_worst_mm,
        s_max: area_weighted(best),
        s_worst: area_weighted(worst),
        degenerate,
    }
}

/// 3D surface area of a mesh (mm²) — the `dA` term, exposed for the
/// flat-law reference floor `area / s_flat` the whole-board ledger states.
/// The flat law carries no per-triangle curvature; state it as a
/// reference, never as an exact floor.
///
/// **Test door.** The harnesses under `crates/rs_cam_core/tests` are the
/// only callers. No production path reads it.
#[must_use]
pub fn mesh_area_mm2(mesh: &TriangleMesh) -> f64 {
    mesh.faces
        .iter()
        .map(|f| 0.5 * (f.v[1] - f.v[0]).cross(&(f.v[2] - f.v[0])).norm())
        .sum()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::{area_weighted, region_floor};
    use crate::geo::P3;
    use crate::mesh::TriangleMesh;

    /// Moved with `area_weighted` from `conformal_spiral_synthetic_f2.rs`
    /// (`area_weighted_percentiles_follow_area_not_triangle_count`).
    #[test]
    fn area_weighted_percentiles_follow_area_not_triangle_count() {
        // Nine tiny triangles reading 1.0 and one large one reading 9.0: by
        // COUNT the median is 1.0, by AREA it is 9.0.
        let mut pairs: Vec<(f64, f64)> = (0..9).map(|_| (1.0, 0.01)).collect();
        pairs.push((9.0, 10.0));
        let stats = area_weighted(pairs);
        assert_eq!(stats.samples, 10);
        assert!((stats.p50 - 9.0).abs() < 1e-12, "p50 = {}", stats.p50);
        assert!((stats.min - 1.0).abs() < 1e-12);
        assert!((stats.max - 9.0).abs() < 1e-12);
    }

    /// New pin (the promoted function had none): on a flat unit square the
    /// floor is exactly `area / s_flat`, because zero curvature makes
    /// `s_max` the flat-law stepover everywhere.
    #[test]
    fn flat_square_floor_matches_the_flat_law() {
        let mesh = TriangleMesh::from_raw(
            vec![
                P3::new(0.0, 0.0, 0.0),
                P3::new(10.0, 0.0, 0.0),
                P3::new(10.0, 10.0, 0.0),
                P3::new(0.0, 10.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        let (radius, cusp) = (1.0, 0.03);
        let flat = crate::scallop_math::stepover_from_scallop_flat(radius, cusp);
        let report = region_floor(&mesh, None, radius, cusp, &|_, _| (0.0, 0.0));
        assert_eq!(report.degenerate, 0);
        assert!((report.area_mm2 - 100.0).abs() < 1e-9);
        let expected = 100.0 / flat;
        assert!(
            (report.l_min_mm - expected).abs() < 1e-9,
            "l_min {} vs flat-law {}",
            report.l_min_mm,
            expected
        );
        // Umbilic (here: flat) ground makes both bases coincide.
        assert!((report.l_min_mm - report.l_min_worst_mm).abs() < 1e-12);
        assert!((report.times_floor(2.0 * expected) - 2.0).abs() < 1e-12);
    }
}
