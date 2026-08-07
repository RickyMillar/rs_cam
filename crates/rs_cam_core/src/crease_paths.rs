//! Turn [`RestCenterline`]s into pencil-style centreline + width-capped
//! offset-pass cut paths.
//!
//! Factored out of `crate::pencil::rest_depth_arm`'s emission loop so a
//! future caller (the P2 unified-finish planner's crease pass — see
//! `planning/unified_finish_planner_design.md`) can turn
//! `detect_rest_valleys`'s centrelines into cut paths without depending on
//! pencil's detector-arm dispatch (reference-tool resolution, dihedral/
//! curvature front-ends, path ordering/emission). `rest_depth_arm` itself is
//! now a thin caller of [`centerline_cut_paths`].
//!
//! [`centerline_cut_paths`] and [`crate::pencil::PencilPath`] are `pub`
//! (C9): a caller OUTSIDE the crate can hand this function a hand-built
//! [`RestCenterline`] — `samples` and all — and inspect exactly what came
//! out, without a mesh detector in the loop. `tests/per_point_claims_fan_c9.rs`
//! is that caller: it pins the per-point stepover fix against a
//! `RestCenterline` whose depths are literal numbers, not detector output,
//! so the sentry cannot be confused by detector noise.

use crate::geo::polyline_length;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pencil::{PencilPath, paths_from_sampled, resample_polyline};
use crate::rest_field::RestCenterline;
use crate::tool::MillingCutter;

/// Whether `cl` carries genuine per-point cross-section data — the
/// detector's own measurement, one [`crate::rest_field::CenterlineSample`]
/// per point — rather than the synthetic fallback
/// ([`RestCenterline::without_samples`]). The one boolean both
/// [`resample_with_reach`] and [`centerline_cut_paths`] key off, so "is this
/// centreline measured" is decided in exactly one place instead of the two
/// call sites re-deriving it and risking drift.
fn centerline_is_measured(cl: &RestCenterline) -> bool {
    cl.samples.len() == cl.points.len() && cl.points.len() >= 2
}

/// Length-gate `centerlines`, resample each survivor, and emit its
/// centreline + width-capped offset passes via
/// [`crate::pencil::paths_from_sampled`] — exactly the loop
/// `crate::pencil::rest_depth_arm` used to run inline.
///
/// `num_offset_passes_cap` is a CAP, not a fixed count.
///
/// # How many passes fit, and how far apart (PR-5/PR-6a, H2.2/H2.3, C9)
///
/// The fit question is answered by the canonical reach policy
/// ([`crate::reach`]), per side and PER SAMPLED POINT, from the
/// cross-section the detector measured
/// ([`RestCenterline::samples`]) — not by a tool scalar.
///
/// C9 closed the last scalar left in this equation: the STEPOVER itself.
/// Before C9, a MEASURED centreline's fan was still spaced at one constant
/// `offset_stepover` (this function's own argument) end to end, even though
/// [`crate::reach::suggested_offset_stepover_mm`] is monotone
/// non-decreasing in depth. Both shipped callers size that scalar from the
/// branch's SHALLOWEST reported depth (`unified_finish.rs`'s
/// `claims_offset_stepover_mm`, sized at `min_valley_depth`; `rest_depth_arm`
/// passes `params.offset_stepover` the same way) — the conservative choice
/// for a single number, but every deeper point on the same branch then got a
/// stepover narrower than its own working width. Under a fixed pass-count
/// cap that means passes packed closer together than the tool actually
/// needs instead of reaching outward, so the deep end's reachable band went
/// under-covered by construction (`tests/per_point_claims_fan_c9.rs`
/// measures the shortfall on the shipped taper). Now every sampled point
/// gets its OWN stepover, `suggested_offset_stepover_mm(cutter, depth_i)` —
/// [`resample_with_reach`] carries the per-point depth alongside the reach
/// for exactly this — and the per-side pass COUNT is the max over points of
/// how many of THAT point's own stepovers its OWN reach supports, not a
/// single scalar-stepover count folded from the branch's widest reach.
/// [`crate::pencil::paths_from_sampled`] does the matching per-point
/// truncation and emission.
///
/// The equation this replaces was
/// `n = round((half_width_mm − cutter.envelope_radius_mm()) / stepover)`.
/// PR-2 deleted the redundant `cutter_radius: f64` argument so the semantic
/// class was chosen in exactly one place (`TOOL_SCALE_SEMANTICS.md` §9.1
/// row 18), and that place read the ENVELOPE: on a Ø1-tip / Ø6-shank taper
/// `half_width_mm − 3.0` is negative for every valley narrower than 6 mm, so
/// `n` was always 0 and the whole width-aware pass count was dead code —
/// 0 of 107 sub-shank matrix cells got a pass though 48 of them physically
/// support one (`CHECKPOINT_A_EVIDENCE.md` §2). Swapping the scalar alone
/// was not an option: the same number fed the pencil/clearing routing rule,
/// and shrinking it collapsed routing instead (§8.3), which is why the fit
/// and the routing criterion landed in the same PR.
///
/// A centreline with no samples (`without_samples` — the finish planner's
/// synthetic creases, tests; see [`centerline_is_measured`]) falls back to
/// the branch's own `half_width_mm` read through the same policy at zero
/// wall angle, AND to this function's `offset_stepover` argument for every
/// point's spacing — "not measured" stays "not measured" for spacing too,
/// exactly as it always has for fit. Not measured is not the same as
/// measured flat.
///
/// Returns `Err(Cancelled)` if `cancel` fires mid-loop (checked once per
/// centreline, same granularity as the original inline loop).
#[allow(clippy::too_many_arguments)]
pub fn centerline_cut_paths(
    centerlines: &[RestCenterline],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    sampling: f64,
    offset_stepover: f64,
    num_offset_passes_cap: usize,
    min_cut_length: f64,
    stock_to_leave: f64,
    // Wave D1: accumulates the centreline TIP-FLOAT tally across every
    // emitted crease (see `crate::compute::config::TipFloatFinding`). This
    // path is shared by the pencil `RestDepth` arm and the unified-finish
    // crease node, so both report through one measurement.
    float: &mut crate::compute::config::TipFloatFinding,
    cancel: &dyn CancelCheck,
) -> Result<Vec<PencilPath>, Cancelled> {
    let mut all_paths: Vec<PencilPath> = Vec::new();
    let kept: Vec<&RestCenterline> = centerlines
        .iter()
        .filter(|cl| polyline_length(&cl.points) >= min_cut_length)
        .collect();
    if kept.is_empty() {
        return Ok(all_paths);
    }
    let chain_total = kept.len();
    for (ci, cl) in kept.iter().enumerate() {
        check_cancel(cancel)?;
        let (sampled, reach, depth_mm) = resample_with_reach(cl, cutter, sampling);
        let measured = centerline_is_measured(cl);
        // Per-point stepover (C9): meaningful only when the centreline
        // carries genuine per-point depth. An empty vector reads as "not
        // measured" to `OffsetFan::stepover`, which then falls back to the
        // flat `offset_stepover` argument — see `paths_from_sampled`'s doc.
        let stepovers: Vec<f64> = if measured {
            depth_mm
                .iter()
                .map(|&d| crate::reach::suggested_offset_stepover_mm(cutter, d))
                .collect()
        } else {
            Vec::new()
        };
        // Per-side counts, floored at 0 (a valley narrower than the cutter
        // gets centreline-only) and capped by the user's dial. The reach
        // vector rides along so each pass is additionally truncated to the
        // points that actually support it.
        let (left, right) = if measured {
            // A branch reachable at one end and pinched at the other must
            // not have its fan sized by folding the two into one scalar —
            // that fold-then-divide-by-one-stepover is exactly the
            // width/spacing mismatch C9 retired. Each point votes with its
            // OWN reach and its OWN stepover; the max across points is how
            // many passes the branch supports, and `paths_from_sampled`
            // re-applies the identical per-point comparison when it decides
            // which of those passes survive at each point.
            let mut left = 0usize;
            let mut right = 0usize;
            for (r, &s) in reach.iter().zip(stepovers.iter()) {
                let (l, rr) = crate::reach::offset_passes_per_side(r, s, num_offset_passes_cap);
                left = left.max(l);
                right = right.max(rr);
            }
            (left, right)
        } else {
            let widest = reach.iter().fold(crate::reach::Reach::default(), |acc, r| {
                crate::reach::Reach {
                    left_mm: acc.left_mm.max(r.left_mm),
                    right_mm: acc.right_mm.max(r.right_mm),
                    refused: acc.refused,
                    // Provenance rides along; every element of `reach` was solved
                    // under the same model.
                    model: r.model,
                }
            });
            crate::reach::offset_passes_per_side(&widest, offset_stepover, num_offset_passes_cap)
        };
        paths_from_sampled(
            &sampled,
            ci + 1,
            chain_total,
            mesh,
            index,
            cutter,
            stock_to_leave,
            offset_stepover,
            crate::pencil::OffsetFan {
                left,
                right,
                reach: &reach,
                stepover: &stepovers,
            },
            &mut all_paths,
            float,
        );
    }
    Ok(all_paths)
}

/// Resample one centreline to cut spacing and carry its per-point reach AND
/// depth with it, so all three stay aligned 1:1.
///
/// `resample_polyline` moves the points, so the detector's per-cell samples
/// cannot simply be zipped onto the result. Each output point is placed by
/// its ARC LENGTH along the source polyline — the same quantity the
/// resampler walks — and takes the reach/depth of the source vertex it fell
/// in. Nearest-vertex rather than interpolated because reach is a geometric
/// solve, not a field: averaging two solves produces a number neither of
/// them supports.
///
/// A centreline with no samples ([`centerline_is_measured`] false) returns a
/// reach vector solved once from the branch's own `half_width_mm` at zero
/// wall angle — which every consumer reads as "not measured": the fan falls
/// back to the branch scalar — and a depth vector of `0.0` for every point,
/// the depth that fallback valley was itself solved at. Callers must key off
/// [`centerline_is_measured`], not this depth vector, to decide whether
/// per-point spacing applies (see [`centerline_cut_paths`]); feeding a
/// fallback `0.0` straight to
/// [`crate::reach::suggested_offset_stepover_mm`] would silently produce the
/// cusp floor instead of the caller's own scalar.
fn resample_with_reach(
    cl: &RestCenterline,
    cutter: &dyn MillingCutter,
    sampling: f64,
) -> (Vec<crate::geo::P3>, Vec<crate::reach::Reach>, Vec<f64>) {
    let sampled = resample_polyline(&cl.points, sampling);
    if !centerline_is_measured(cl) {
        // Not measured. Solve the branch scalar once through the SAME policy
        // so there is still only one fit implementation, and hand it to every
        // point so per-point truncation is a no-op rather than a refusal.
        let valley = crate::reach::LocalValley {
            rest_depth_mm: 0.0,
            left: crate::reach::ValleySide::vertical(cl.half_width_mm),
            right: crate::reach::ValleySide::vertical(cl.half_width_mm),
        };
        let r = crate::reach::solve_reach(cutter, &valley);
        return (
            sampled.clone(),
            vec![r; sampled.len()],
            vec![0.0; sampled.len()],
        );
    }
    // Cumulative arc length of the source polyline.
    let mut cum = Vec::with_capacity(cl.points.len());
    let mut acc = 0.0f64;
    cum.push(0.0);
    for w in cl.points.windows(2) {
        let (a, b) = (w.first(), w.get(1));
        if let (Some(a), Some(b)) = (a, b) {
            acc += ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        }
        cum.push(acc);
    }
    let mut reach = Vec::with_capacity(sampled.len());
    let mut depth_mm = Vec::with_capacity(sampled.len());
    let mut walked = 0.0f64;
    for (i, p) in sampled.iter().enumerate() {
        if i > 0
            && let Some(prev) = sampled.get(i - 1)
        {
            walked +=
                ((p.x - prev.x).powi(2) + (p.y - prev.y).powi(2) + (p.z - prev.z).powi(2)).sqrt();
        }
        // First source vertex at or past this arc length; take the nearer of
        // it and its predecessor.
        let j = cum.partition_point(|&c| c < walked);
        let j = j.min(cl.samples.len().saturating_sub(1));
        let k = if j > 0 {
            let (before, after) = (cum.get(j - 1).copied(), cum.get(j).copied());
            match (before, after) {
                (Some(b), Some(a)) if (walked - b) <= (a - walked) => j - 1,
                _ => j,
            }
        } else {
            0
        };
        reach.push(cl.samples.get(k).map(|s| s.reach).unwrap_or_default());
        depth_mm.push(
            cl.samples
                .get(k)
                .map(|s| s.valley.rest_depth_mm)
                .unwrap_or(0.0),
        );
    }
    (sampled, reach, depth_mm)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::tool::BallEndmill;

    /// A symmetric V-valley running along X: two inclined planes meeting at y=0.
    fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
        let mut verts = Vec::new();
        let sx = len_x / nx as f64;
        let sy = 2.0 * half_y / ny as f64;
        for j in 0..=ny {
            for i in 0..=nx {
                let x = i as f64 * sx;
                let y = -half_y + j as f64 * sy;
                let z = -slope * half_y + slope * y.abs();
                verts.push(P3::new(x, y, z));
            }
        }
        let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
                tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
        TriangleMesh::from_raw(verts, tris)
    }

    /// A known small input — one straight-along-X centreline sitting in the
    /// V-valley trough, wide enough for exactly one offset pass on each
    /// side — reproduces `rest_depth_arm`'s pre-factor output: one centreline
    /// path plus two offset paths, all riding the surface.
    #[test]
    fn centerline_cut_paths_reproduces_known_small_input() {
        let mesh = make_v_valley(20.0, 6.0, 0.5, 20, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);
        let centerlines = vec![RestCenterline::without_samples(
            vec![
                P3::new(2.0, 0.0, 0.0),
                P3::new(8.0, 0.0, 0.0),
                P3::new(14.0, 0.0, 0.0),
                P3::new(18.0, 0.0, 0.0),
            ],
            3.0,
        )];

        let never_cancel = || false;
        let paths = centerline_cut_paths(
            &centerlines,
            &mesh,
            &index,
            &tool,
            1.0,
            1.0,
            2,
            2.0,
            0.0,
            &mut Default::default(),
            &never_cancel,
        )
        .expect("no cancellation requested");

        // half_width 3.0, cutter radius 1.0, stepover 1.0 -> n = round((3-1)/1) = 2,
        // capped at 2 -> centreline + 2*2 offset passes = 5 paths total.
        assert_eq!(
            paths.len(),
            5,
            "expected 1 centreline + 2 offset passes each side"
        );

        // Below the length gate: no paths at all.
        let short = vec![RestCenterline::without_samples(
            vec![P3::new(2.0, 0.0, 0.0), P3::new(2.5, 0.0, 0.0)],
            3.0,
        )];
        let filtered = centerline_cut_paths(
            &short,
            &mesh,
            &index,
            &tool,
            1.0,
            1.0,
            2,
            2.0,
            0.0,
            &mut Default::default(),
            &never_cancel,
        )
        .expect("no cancellation requested");
        assert!(
            filtered.is_empty(),
            "a centreline shorter than min_cut_length must be gated out"
        );
    }
}
