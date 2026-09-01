//! Achieved surface spacing — what the emitted passes actually left on the
//! surface, contact ring to contact ring.
//!
//! Two promoted instruments live here:
//!
//! * [`measure_raster_spacing`] — the Track B acceptance measurement
//!   (`tests/shipped_raster_spacing_b1.rs`): CL samples grouped into raster
//!   rows, mapped to analytic CONTACT points, then each contact's minimum
//!   3D distance to the next pass's contact polyline. The quantity the
//!   Track B gate reads is `achieved / s_max` over the sloped population.
//! * [`path_structure`] — the M4 path-side census
//!   (`tests/common/scallop_oracle.rs`): segment lengths, ring count, and
//!   achieved 3D spacing to the nearest point on a DIFFERENT ring, through
//!   a uniform spatial hash.
//!
//! The raster mechanism spaces passes in XY, so on ground with cross-pass
//! slope `θ` the achieved surface spacing is `s_XY / cos θ` — the Track B
//! defect and the reason the honest derate exists
//! (`tests/shallow_raster_slope_derate.rs`).

use std::collections::{BTreeMap, HashMap};

use crate::geo::P3;
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

/// One achieved-spacing sample: a contact point on pass `i` and its minimum
/// 3D distance to pass `i+1`'s contact polyline.
#[derive(Debug, Clone, Copy)]
pub struct SpacingSample {
    pub achieved_mm: f64,
    pub total_slope_deg: f64,
    pub cross_slope_deg: f64,
}

/// The whole spacing measurement of one toolpath against one fixture.
#[derive(Debug, Clone)]
pub struct SpacingMeasurement {
    pub samples: Vec<SpacingSample>,
    /// Raster rows found (distinct CL `y` groups).
    pub rows: usize,
    /// Adjacent-row `Δy` census — the XY mechanism check.
    pub row_dy_min: f64,
    pub row_dy_max: f64,
    /// Max `|dist(center, surface) − K_c|` over the sampled centers — the
    /// contact-mapping self-check (F-B3). A large value means the mapping
    /// is reading rim roll-off or facet error, and the samples are void.
    pub center_distance_err_max: f64,
}

/// The fixture-side closed forms the measurement consumes. The Track B
/// instrument keeps these ANALYTIC so the ruler carries no estimator; a
/// caller that fits them from a mesh thereby states that its spacing
/// carries estimator error.
pub struct ContactMaps<'a> {
    /// Ball-centre → contact point on the true surface.
    pub contact_of_center: &'a dyn Fn(P3) -> P3,
    /// Whether a CL sample is interior (rim roll-off excluded).
    pub interior_cl: &'a dyn Fn(f64, f64) -> bool,
    /// Distance from a ball centre to the true surface — must read the
    /// ball radius on every interior sample (the F-B3 self-check).
    pub center_surface_distance: &'a dyn Fn(P3) -> f64,
    /// Total slope (degrees) of the surface at a contact XY.
    pub total_slope_deg: &'a dyn Fn(f64, f64) -> f64,
    /// Cross-pass slope (degrees) at a contact XY.
    pub cross_slope_deg: &'a dyn Fn(f64, f64) -> f64,
}

/// Minimum distance from `p` to segment `ab`.
#[must_use]
pub fn dist_point_segment(p: P3, a: P3, b: P3) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(&ab);
    if len2 <= 1e-18 {
        return (p - a).norm();
    }
    let t = ((p - a).dot(&ab) / len2).clamp(0.0, 1.0);
    let q = P3::new(a.x + ab.x * t, a.y + ab.y * t, a.z + ab.z * t);
    (p - q).norm()
}

/// Measure the achieved surface spacing of an emitted 0-degree raster
/// toolpath against the fixture's closed forms.
///
/// `ball_radius_mm` is the cusp-forming tip radius (`K_c`); `stepover_mm`
/// is the COMMANDED stepover, used only to split clipped rows and gaps
/// (`> 1.5 ×` reads as a skipped lattice row or a region clip, never as
/// adjacent passes).
#[must_use]
pub fn measure_raster_spacing(
    toolpath: &Toolpath,
    maps: &ContactMaps<'_>,
    ball_radius_mm: f64,
    stepover_mm: f64,
) -> SpacingMeasurement {
    // 1. Cutting CL samples, grouped into raster rows by CL y (the shipped
    //    0-degree lattice holds y constant along a pass).
    let mut rows: BTreeMap<i64, Vec<P3>> = BTreeMap::new();
    for mv in &toolpath.moves {
        if mv.intent != MoveIntent::FinishingCut || !mv.move_type.is_cutting() {
            continue;
        }
        let key = (mv.target.y * 1e6).round() as i64;
        rows.entry(key).or_default().push(mv.target);
    }
    let mut row_list: Vec<(f64, Vec<P3>)> = rows
        .into_iter()
        .map(|(k, mut pts)| {
            pts.sort_by(|a, b| a.x.total_cmp(&b.x));
            (k as f64 * 1e-6, pts)
        })
        .collect();
    row_list.sort_by(|a, b| a.0.total_cmp(&b.0));

    // 2. Row Delta-y census (falsifier F-B1).
    let mut dy_min = f64::INFINITY;
    let mut dy_max = f64::NEG_INFINITY;
    for pair in row_list.windows(2) {
        let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let dy = b.0 - a.0;
        dy_min = dy_min.min(dy);
        dy_max = dy_max.max(dy);
    }

    // 3. Contact mapping + nearest-pass distance.
    let to_center = |cl: P3| P3::new(cl.x, cl.y, cl.z + ball_radius_mm);
    let mut samples = Vec::new();
    let mut center_err_max = 0.0f64;
    for pair in row_list.windows(2) {
        let (Some((y0, row)), Some((y1, next))) = (pair.first(), pair.get(1)) else {
            continue;
        };
        if y1 - y0 > 1.5 * stepover_mm {
            continue; // a skipped/clipped lattice row, not adjacent passes
        }
        // The next pass's contact polyline, split where the CL x gap shows
        // a region clip.
        let next_contacts: Vec<(f64, P3)> = next
            .iter()
            .map(|&cl| (cl.x, (maps.contact_of_center)(to_center(cl))))
            .collect();
        for &cl in row {
            if !(maps.interior_cl)(cl.x, cl.y) {
                continue;
            }
            let center = to_center(cl);
            let err = ((maps.center_surface_distance)(center) - ball_radius_mm).abs();
            center_err_max = center_err_max.max(err);
            let contact = (maps.contact_of_center)(center);
            let mut best = f64::INFINITY;
            for seg in next_contacts.windows(2) {
                let (Some(&(x_a, a)), Some(&(x_b, b))) = (seg.first(), seg.get(1)) else {
                    continue;
                };
                if (x_b - x_a).abs() > 1.5 * stepover_mm {
                    continue; // clipped gap: not a cut segment
                }
                best = best.min(dist_point_segment(contact, a, b));
            }
            if let (1, Some(&(_, only))) = (next_contacts.len(), next_contacts.first()) {
                best = best.min((contact - only).norm());
            }
            if !best.is_finite() {
                continue;
            }
            samples.push(SpacingSample {
                achieved_mm: best,
                total_slope_deg: (maps.total_slope_deg)(contact.x, contact.y),
                cross_slope_deg: (maps.cross_slope_deg)(contact.x, contact.y),
            });
        }
    }
    SpacingMeasurement {
        samples,
        rows: row_list.len(),
        row_dy_min: dy_min,
        row_dy_max: dy_max,
        center_distance_err_max: center_err_max,
    }
}

// ---------------------------------------------------------------------------
// Path-side structure (from tests/common/scallop_oracle.rs)
// ---------------------------------------------------------------------------

/// Path-side structure the envelope cannot see: how far apart neighbouring
/// passes actually ended up, how long the emitted segments are, and how
/// many rings there were.
///
/// The achieved-stepover measurement reuses Checkpoint B's `measured_cusp`
/// bucketing idea — nearest point on a DIFFERENT ring, found through a
/// uniform spatial hash — but reports the whole distribution in 3D rather
/// than a single flat-ground cusp conversion.
#[derive(Debug, Clone)]
pub struct PathStructure {
    pub cut_moves: usize,
    pub rings: usize,
    pub total_cut_mm: f64,
    pub min_segment_mm: f64,
    pub seg_p01_mm: f64,
    pub seg_p50_mm: f64,
    /// Segments shorter than 10 µm, and their share of all segments.
    ///
    /// A single short segment is noise; a population of them is a
    /// feed-rate collapse, because every junction costs the controller a
    /// decel/accel pair regardless of how short the move is.
    pub segs_under_10um: usize,
    pub segs_under_10um_frac: f64,
    /// Achieved 3D spacing to the nearest point on another ring.
    pub stepover_p05_mm: f64,
    pub stepover_p50_mm: f64,
    pub stepover_p95_mm: f64,
    pub stepover_min_mm: f64,
    /// `p50 / p05` — how far the tightest spacing is below the typical
    /// one. A per-ring constant stepover chosen by MIN drives this toward
    /// 1.0 by dragging the typical spacing down to the tightest one.
    pub stepover_spread: f64,
}

/// `idx = round((len − 1) · q)` — the same estimator Checkpoint B and the
/// M3 COLUMNS harness use, so quantiles are comparable across the three.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted.get(idx.min(sorted.len() - 1)).copied().unwrap_or(f64::NAN)
}

/// Measure [`PathStructure`] from a toolpath and the ring-start move
/// indices the scallop generator annotates.
#[must_use]
pub fn path_structure(toolpath: &Toolpath, ring_starts: &[usize], bucket_mm: f64) -> PathStructure {
    // Which ring each move index belongs to.
    let mut ring_of = vec![usize::MAX; toolpath.moves.len()];
    if !ring_starts.is_empty() {
        let mut sorted = ring_starts.to_vec();
        sorted.sort_unstable();
        let mut r = 0usize;
        for (i, slot) in ring_of.iter_mut().enumerate() {
            while sorted.get(r + 1).is_some_and(|&s| i >= s) {
                r += 1;
            }
            if sorted.first().is_some_and(|&s| i >= s) {
                *slot = r;
            }
        }
    }

    let mut segs: Vec<f64> = Vec::new();
    let mut total = 0.0;
    let mut pts: Vec<(P3, usize)> = Vec::new();
    let mut prev: Option<P3> = None;
    for (i, mv) in toolpath.moves.iter().enumerate() {
        let cutting = !matches!(mv.move_type, MoveType::Rapid);
        if cutting {
            if let Some(a) = prev {
                let d = ((mv.target.x - a.x).powi(2)
                    + (mv.target.y - a.y).powi(2)
                    + (mv.target.z - a.z).powi(2))
                .sqrt();
                if d > 1e-9 {
                    segs.push(d);
                    total += d;
                }
            }
            pts.push((mv.target, ring_of.get(i).copied().unwrap_or(usize::MAX)));
        }
        prev = Some(mv.target);
    }

    // Nearest point on a different ring, via a uniform spatial hash.
    let cell = bucket_mm.max(1e-3);
    let mut hash: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (p, _)) in pts.iter().enumerate() {
        hash.entry(((p.x / cell).floor() as i64, (p.y / cell).floor() as i64))
            .or_default()
            .push(i);
    }
    let mut steps: Vec<f64> = Vec::new();
    for (i, (p, ring)) in pts.iter().enumerate() {
        if *ring == usize::MAX {
            continue;
        }
        let (bx, by) = ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let mut best = f64::INFINITY;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bucket) = hash.get(&(bx + dx, by + dy)) else {
                    continue;
                };
                for &j in bucket {
                    if j == i {
                        continue;
                    }
                    let Some(&(q, qr)) = pts.get(j) else {
                        continue;
                    };
                    if qr == *ring || qr == usize::MAX {
                        continue;
                    }
                    let d =
                        ((q.x - p.x).powi(2) + (q.y - p.y).powi(2) + (q.z - p.z).powi(2)).sqrt();
                    if d < best {
                        best = d;
                    }
                }
            }
        }
        if best.is_finite() {
            steps.push(best);
        }
    }

    segs.sort_by(f64::total_cmp);
    steps.sort_by(f64::total_cmp);
    let p05 = quantile(&steps, 0.05);
    let p50 = quantile(&steps, 0.50);
    PathStructure {
        cut_moves: pts.len(),
        rings: ring_starts.len(),
        total_cut_mm: total,
        min_segment_mm: segs.first().copied().unwrap_or(f64::NAN),
        seg_p01_mm: quantile(&segs, 0.01),
        seg_p50_mm: quantile(&segs, 0.50),
        segs_under_10um: segs.partition_point(|d| *d < 0.010),
        segs_under_10um_frac: if segs.is_empty() {
            f64::NAN
        } else {
            segs.partition_point(|d| *d < 0.010) as f64 / segs.len() as f64
        },
        stepover_p05_mm: p05,
        stepover_p50_mm: p50,
        stepover_p95_mm: quantile(&steps, 0.95),
        stepover_min_mm: steps.first().copied().unwrap_or(f64::NAN),
        stepover_spread: if p05 > 1e-9 { p50 / p05 } else { f64::NAN },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{dist_point_segment, quantile};
    use crate::geo::P3;

    /// New pin (the promoted helpers had none): the point-segment distance
    /// clamps to the segment ends and the quantile estimator is the
    /// round-index one Checkpoint B uses.
    #[test]
    fn dist_point_segment_clamps_and_quantile_rounds() {
        let a = P3::new(0.0, 0.0, 0.0);
        let b = P3::new(10.0, 0.0, 0.0);
        assert!((dist_point_segment(P3::new(5.0, 3.0, 0.0), a, b) - 3.0).abs() < 1e-12);
        assert!((dist_point_segment(P3::new(-4.0, 3.0, 0.0), a, b) - 5.0).abs() < 1e-12);
        // Degenerate segment falls back to point distance.
        assert!((dist_point_segment(P3::new(0.0, 2.0, 0.0), a, a) - 2.0).abs() < 1e-12);

        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((quantile(&sorted, 0.5) - 3.0).abs() < 1e-12);
        assert!((quantile(&sorted, 0.0) - 1.0).abs() < 1e-12);
        assert!((quantile(&sorted, 1.0) - 5.0).abs() < 1e-12);
        assert!(quantile(&[], 0.5).is_nan());
    }
}
