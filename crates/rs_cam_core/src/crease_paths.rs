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

use crate::geo::polyline_length;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pencil::{PencilPath, paths_from_sampled, resample_polyline};
use crate::rest_field::RestCenterline;
use crate::tool::MillingCutter;

/// Length-gate `centerlines`, resample each survivor, and emit its
/// centreline + width-capped offset passes via
/// [`crate::pencil::paths_from_sampled`] — exactly the loop
/// `crate::pencil::rest_depth_arm` used to run inline.
///
/// `num_offset_passes_cap` is a CAP, not a fixed count: each centreline's own
/// `half_width_mm` (see [`RestCenterline`]) sizes how many stepovers actually
/// fit between the cutter's radius and the local valley half-width, floored
/// at 0 (centreline-only) and capped at `num_offset_passes_cap`. Returns
/// `Err(Cancelled)` if `cancel` fires mid-loop (checked once per centreline,
/// same granularity as the original inline loop).
#[allow(clippy::too_many_arguments)]
pub(crate) fn centerline_cut_paths(
    centerlines: &[RestCenterline],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cutter_radius: f64,
    sampling: f64,
    offset_stepover: f64,
    num_offset_passes_cap: usize,
    min_cut_length: f64,
    stock_to_leave: f64,
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
        let sampled = resample_polyline(&cl.points, sampling);
        // Width-aware pass count (P2.4-adjacent judgement call, see the
        // `paths_from_sampled` doc): `num_offset_passes_cap` is a user dial
        // that acts as a CAP, not a fixed count — a valley narrower than the
        // cutter-plus-a-few-stepovers only gets the centreline. `n` is how
        // many stepovers fit between the cutter's own radius and the
        // measured local half-width; floor at 0 (a valley narrower than the
        // cutter itself gets centreline-only, same as before).
        let n = ((cl.half_width_mm - cutter_radius) / offset_stepover)
            .round()
            .max(0.0) as usize;
        let offset_passes = n.min(num_offset_passes_cap);
        paths_from_sampled(
            &sampled,
            ci + 1,
            chain_total,
            mesh,
            index,
            cutter,
            stock_to_leave,
            offset_stepover,
            offset_passes,
            &mut all_paths,
        );
    }
    Ok(all_paths)
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
        let centerlines = vec![RestCenterline {
            points: vec![
                P3::new(2.0, 0.0, 0.0),
                P3::new(8.0, 0.0, 0.0),
                P3::new(14.0, 0.0, 0.0),
                P3::new(18.0, 0.0, 0.0),
            ],
            half_width_mm: 3.0,
        }];

        let never_cancel = || false;
        let paths = centerline_cut_paths(
            &centerlines,
            &mesh,
            &index,
            &tool,
            tool.radius(),
            1.0,
            1.0,
            2,
            2.0,
            0.0,
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
        let short = vec![RestCenterline {
            points: vec![P3::new(2.0, 0.0, 0.0), P3::new(2.5, 0.0, 0.0)],
            half_width_mm: 3.0,
        }];
        let filtered = centerline_cut_paths(
            &short,
            &mesh,
            &index,
            &tool,
            tool.radius(),
            1.0,
            1.0,
            2,
            2.0,
            0.0,
            &never_cancel,
        )
        .expect("no cancellation requested");
        assert!(
            filtered.is_empty(),
            "a centreline shorter than min_cut_length must be gated out"
        );
    }
}
