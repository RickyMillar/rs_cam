//! Gouge-safe, drop-cutter-sampled surface links between two cut points.
//!
//! Promoted out of `crate::pencil` (where it joins consecutive pencil passes
//! without a retract-and-replunge) so the P2 unified-finish planner's global
//! router can reuse it as one of the two link-cost candidates (surface link
//! vs. retract link) between regions — see
//! `planning/unified_finish_planner_design.md` step 4.

use crate::dropcutter::point_drop_cutter;
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

/// Build a gouge-safe, surface-following link between two cut points whose XY gap
/// is within `hookup_distance`, so consecutive passes join without retracting to
/// safe Z and re-plunging. Samples the connecting segment and drop-cutters each
/// interior point — the same gouge-free lift the cut path uses — so the tool rides
/// the surface across the gap instead of lifting clear. Endpoints are excluded
/// (the caller is already at `from` and feeds to `to` itself). Returns `None` if
/// the tool loses surface contact anywhere along the link (over a hole / off the
/// mesh) — the caller then falls back to a clean retract-and-replunge.
pub fn build_surface_link(
    from: P3,
    to: P3,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    spacing: f64,
) -> Option<Vec<P3>> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return Some(Vec::new());
    }
    let n = (dist / spacing.max(1e-3)).ceil().max(1.0) as usize;
    let mut pts = Vec::new();
    for k in 1..n {
        let t = k as f64 / n as f64;
        let x = from.x + dx * t;
        let y = from.y + dy * t;
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        if !cl.contacted {
            return None; // lost contact → not safe to link at surface, retract instead
        }
        pts.push(P3::new(x, y, cl.z + stock_to_leave));
    }
    Some(pts)
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

    /// A surface link rides the mesh (finite Z everywhere) when both ends sit on
    /// it, and returns None when the span is off the mesh (caller then retracts).
    #[test]
    fn build_surface_link_follows_surface_and_detects_offmesh() {
        let mesh = make_v_valley(20.0, 6.0, 0.5, 20, 24);
        let index = SpatialIndex::build(&mesh, 5.0);
        let tool = BallEndmill::new(2.0, 25.0);

        let on = build_surface_link(
            P3::new(5.0, 0.0, 0.0),
            P3::new(9.0, 0.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        let pts = on.unwrap();
        assert!(
            !pts.is_empty(),
            "a 4mm link at 0.5mm spacing has interior points"
        );
        for p in &pts {
            assert!(p.z.is_finite(), "each link point rides the surface");
        }

        let off = build_surface_link(
            P3::new(100.0, 100.0, 0.0),
            P3::new(105.0, 100.0, 0.0),
            &mesh,
            &index,
            &tool,
            0.0,
            0.5,
        );
        assert!(off.is_none(), "a link entirely off the mesh must be None");
    }
}
