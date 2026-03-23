//! Push-cutter algorithm — pushes a cutter horizontally along a Fiber at constant Z.
//!
//! For each triangle, computes the interval(s) on the fiber where the cutter
//! would gouge (contact the triangle). Three contact types are tested:
//! vertex, facet, and edge — analogous to drop-cutter but in the horizontal plane.
//!
//! Used by the waterline algorithm to find contours at constant Z heights.

use crate::fiber::{Fiber, Interval};
use crate::geo::{P3, Triangle};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

/// Push a cutter along a fiber against a single triangle.
/// Adds any gouge intervals to the fiber.
pub fn push_cutter_triangle(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    vertex_push(fiber, tri, cutter);
    facet_push(fiber, tri, cutter);
    edge_push(fiber, tri, cutter);
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Push a cutter along a fiber against all triangles near it using the spatial index.
pub fn push_cutter_fiber(
    fiber: &mut Fiber,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) {
    let r = cutter.radius();
    let length = cutter.length();

    // Query the spatial index with a circle covering the fiber + cutter radius.
    // Center is the midpoint of the fiber, radius covers half-length + cutter radius.
    let cx = (fiber.p1.x + fiber.p2.x) / 2.0;
    let cy = (fiber.p1.y + fiber.p2.y) / 2.0;
    let half_len = fiber.length() / 2.0;
    let query_r = half_len + r;
    let z_min = fiber.z();
    let z_max = fiber.z() + length;

    let candidates = index.query(cx, cy, query_r);

    for &tri_idx in &candidates {
        let tri = &mesh.faces[tri_idx];
        // Quick Z check: skip triangles entirely above or below the cutter at this Z
        let tri_z_min = tri.v[0].z.min(tri.v[1].z).min(tri.v[2].z);
        let tri_z_max = tri.v[0].z.max(tri.v[1].z).max(tri.v[2].z);
        if tri_z_min > z_max || tri_z_max < z_min {
            continue;
        }
        push_cutter_triangle(fiber, tri, cutter);
    }
}

/// Batch push-cutter: process multiple fibers against the mesh.
pub fn batch_push_cutter(
    fibers: &mut [Fiber],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) {
    let never_cancel = || false;
    let _ = batch_push_cutter_with_cancel(fibers, mesh, index, cutter, &never_cancel);
}

pub fn batch_push_cutter_with_cancel(
    fibers: &mut [Fiber],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    #[cfg(feature = "parallel")]
    {
        let _ = (&mesh, &index, &cutter);
    }

    for (i, fiber) in fibers.iter_mut().enumerate() {
        if i % 32 == 0 {
            check_cancel(cancel)?;
        }
        push_cutter_fiber(fiber, mesh, index, cutter);
    }

    Ok(())
}

/// Vertex push: for each triangle vertex, compute the interval on the fiber
/// where the cutter would contact that vertex.
fn vertex_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    for v in &tri.v {
        // Height of vertex above fiber Z
        let h = v.z - fiber.z();
        if h < -1e-10 || h > cutter.length() {
            continue; // vertex below fiber or above cutter
        }

        // Cutter width at this height
        let w = cutter.width_at_height(h);
        if w < 1e-15 {
            continue;
        }

        // Perpendicular distance from vertex to fiber line (in XY)
        let fiber_dx = fiber.p2.x - fiber.p1.x;
        let fiber_dy = fiber.p2.y - fiber.p1.y;
        let fiber_len_sq = fiber_dx * fiber_dx + fiber_dy * fiber_dy;
        if fiber_len_sq < 1e-20 {
            return;
        }
        let fiber_len = fiber_len_sq.sqrt();

        // Signed perpendicular distance
        let qx = v.x - fiber.p1.x;
        let qy = v.y - fiber.p1.y;
        let perp_dist = (qx * fiber_dy - qy * fiber_dx).abs() / fiber_len;

        if perp_dist > w + 1e-10 {
            continue; // vertex too far from fiber
        }

        // t parameter for the projection of vertex onto fiber
        let t_proj = (qx * fiber_dx + qy * fiber_dy) / fiber_len_sq;

        // Half-width along fiber at this perpendicular distance
        let half_len = (w * w - perp_dist * perp_dist).max(0.0).sqrt() / fiber_len;

        let t_lower = t_proj - half_len;
        let t_upper = t_proj + half_len;

        if t_upper >= 0.0 && t_lower <= 1.0 {
            fiber.add_interval(Interval::new(t_lower, t_upper));
        }
    }
}

/// Facet push: find the interval where the cutter contacts the triangle face.
fn facet_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    let n = &tri.normal;

    // Skip nearly-vertical triangles (no horizontal contact)
    if n.z.abs() < 1e-12 {
        return;
    }

    let r1 = cutter.xy_normal_length();
    let r2 = cutter.normal_length();
    let ch = cutter.center_height();

    let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
    let (xy_nx, xy_ny) = if nxy_len > 1e-15 {
        (n.x / nxy_len, n.y / nxy_len)
    } else {
        (0.0, 0.0)
    };

    // For each point on the fiber at parameter t:
    //   CL = fiber.point(t) = (p1.x + t*dx, p1.y + t*dy, z)
    //   CC = CL - r1*(xy_nx, xy_ny) - r2*(n.x, n.y)
    //
    // CC must lie on the triangle plane AND inside the triangle.
    // We solve for t where cc_z (from plane equation) gives:
    //   tip_z = cc_z + r2*n.z - ch
    // and tip_z = fiber.z (constant Z fiber)
    //
    // This gives a single t value (one contact point for the whole fiber).

    let fiber_dx = fiber.p2.x - fiber.p1.x;
    let fiber_dy = fiber.p2.y - fiber.p1.y;
    let z = fiber.z();

    // CC in terms of t
    let cc_x0 = fiber.p1.x - r1 * xy_nx - r2 * n.x;
    let cc_y0 = fiber.p1.y - r1 * xy_ny - r2 * n.y;
    let cc_dx = fiber_dx;
    let cc_dy = fiber_dy;

    // CC on triangle plane: n.x*(cc_x - v0.x) + n.y*(cc_y - v0.y) + n.z*(cc_z - v0.z) = 0
    // => cc_z = v0.z - (n.x*(cc_x - v0.x) + n.y*(cc_y - v0.y)) / n.z
    let v0 = &tri.v[0];
    let num0 = n.x * (cc_x0 - v0.x) + n.y * (cc_y0 - v0.y);
    let num_dt = n.x * cc_dx + n.y * cc_dy;

    // cc_z(t) = v0.z - (num0 + num_dt * t) / n.z
    // tip_z(t) = cc_z(t) + r2 * n.z - ch
    // We want tip_z(t) = z
    // => v0.z - (num0 + num_dt*t)/n.z + r2*n.z - ch = z
    // => -(num0 + num_dt*t)/n.z = z - v0.z - r2*n.z + ch
    // => num0 + num_dt*t = -n.z * (z - v0.z - r2*n.z + ch)
    // => t = (-n.z * (z - v0.z - r2*n.z + ch) - num0) / num_dt

    if num_dt.abs() < 1e-15 {
        // Fiber is parallel to the facet intersection — check if it's at the right Z
        let cc_z = v0.z - num0 / n.z;
        let tip_z = cc_z + r2 * n.z - ch;
        if (tip_z - z).abs() > 1e-8 {
            return;
        }
        // The entire fiber could be in contact — check endpoints
        for t in [0.0, 1.0] {
            let cc_x = cc_x0 + t * cc_dx;
            let cc_y = cc_y0 + t * cc_dy;
            if tri.contains_point_xy(cc_x, cc_y) {
                // The whole fiber is potentially in contact
                fiber.add_interval(Interval::new(0.0, 1.0));
                return;
            }
        }
        return;
    }

    let rhs = -n.z * (z - v0.z - r2 * n.z + ch) - num0;
    let t = rhs / num_dt;

    if !(-1e-8..=1.0 + 1e-8).contains(&t) {
        return;
    }

    let cc_x = cc_x0 + t * cc_dx;
    let cc_y = cc_y0 + t * cc_dy;

    if !tri.contains_point_xy(cc_x, cc_y) {
        return;
    }

    // The facet contact is a single point on the fiber, but we add a tiny interval
    // to mark it as blocked
    let eps = 1e-6;
    fiber.add_interval(Interval::new(t - eps, t + eps));
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Edge push: for each triangle edge, compute the interval on the fiber
/// where the cutter contacts that edge.
fn edge_push(fiber: &mut Fiber, tri: &Triangle, cutter: &dyn MillingCutter) {
    for i in 0..3 {
        let p1 = &tri.v[i];
        let p2 = &tri.v[(i + 1) % 3];
        edge_push_single(fiber, p1, p2, cutter);
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Push-cutter for a single edge against a fiber.
///
/// Finds where the cutter profile, swept along the fiber at constant Z,
/// contacts the edge. This is done by sampling contact points along the edge
/// and checking if the cutter width at the contact height covers the fiber.
fn edge_push_single(fiber: &mut Fiber, p1: &P3, p2: &P3, cutter: &dyn MillingCutter) {
    let z = fiber.z();

    let fiber_dx = fiber.p2.x - fiber.p1.x;
    let fiber_dy = fiber.p2.y - fiber.p1.y;
    let fiber_len_sq = fiber_dx * fiber_dx + fiber_dy * fiber_dy;
    if fiber_len_sq < 1e-20 {
        return;
    }
    let fiber_len = fiber_len_sq.sqrt();

    // Edge vector
    let ex = p2.x - p1.x;
    let ey = p2.y - p1.y;
    let ez = p2.z - p1.z;
    let edge_len_sq = ex * ex + ey * ey + ez * ez;
    if edge_len_sq < 1e-20 {
        return;
    }

    // Coarse+bisection sampling: 9 coarse samples to find contact intervals,
    // then bisect at boundaries for higher accuracy with fewer evaluations.
    let n_coarse = 8;
    let mut t_min = f64::INFINITY;
    let mut t_max = f64::NEG_INFINITY;

    // Helper: evaluate contact at parameter s along the edge.
    // Returns the (t_lo, t_hi) fiber interval if contact, else None.
    let eval_at = |s: f64| -> Option<(f64, f64)> {
        let edge_x = p1.x + s * ex;
        let edge_y = p1.y + s * ey;
        let edge_z = p1.z + s * ez;

        let h = edge_z - z;
        if h < -1e-10 || h > cutter.length() {
            return None;
        }

        let w = cutter.width_at_height(h);
        if w < 1e-15 {
            return None;
        }

        let qx = edge_x - fiber.p1.x;
        let qy = edge_y - fiber.p1.y;
        let perp_dist = (qx * fiber_dy - qy * fiber_dx).abs() / fiber_len;

        if perp_dist > w + 1e-10 {
            return None;
        }

        let t_proj = (qx * fiber_dx + qy * fiber_dy) / fiber_len_sq;
        let half_len = (w * w - perp_dist * perp_dist).max(0.0).sqrt() / fiber_len;

        let tl = t_proj - half_len;
        let tu = t_proj + half_len;

        if tu >= 0.0 && tl <= 1.0 {
            Some((tl, tu))
        } else {
            None
        }
    };

    // Phase 1: 9 coarse samples at s = 0, 1/8, 2/8, ..., 1
    let mut coarse_contact = [false; 9];
    for (i, contacted) in coarse_contact.iter_mut().enumerate().take(n_coarse + 1) {
        let s = i as f64 / n_coarse as f64;
        if let Some((tl, tu)) = eval_at(s) {
            t_min = t_min.min(tl);
            t_max = t_max.max(tu);
            *contacted = true;
        }
    }

    // Phase 2: Bisect at boundaries (contact ↔ no-contact transitions)
    for i in 0..n_coarse {
        if coarse_contact[i] != coarse_contact[i + 1] {
            let mut s_lo = i as f64 / n_coarse as f64;
            let mut s_hi = (i + 1) as f64 / n_coarse as f64;
            // 5 bisection iterations → 1/32 of interval precision
            for _ in 0..5 {
                let s_mid = (s_lo + s_hi) * 0.5;
                let has_contact = eval_at(s_mid).is_some();
                if has_contact == coarse_contact[i] {
                    s_lo = s_mid;
                } else {
                    s_hi = s_mid;
                }
                // Evaluate at boundary for t_min/t_max update
                if let Some((tl, tu)) = eval_at(s_mid) {
                    t_min = t_min.min(tl);
                    t_max = t_max.max(tu);
                }
            }
        }
    }

    if t_min < t_max {
        fiber.add_interval(Interval::new(t_min, t_max));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tool::{BallEndmill, FlatEndmill};

    fn horizontal_tri(z: f64) -> Triangle {
        Triangle::new(
            P3::new(-50.0, -50.0, z),
            P3::new(50.0, -50.0, z),
            P3::new(0.0, 50.0, z),
        )
    }

    #[test]
    fn test_vertex_push_below_fiber() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 5.0, -50.0, 50.0);
        let tri = horizontal_tri(0.0);
        vertex_push(&mut fiber, &tri, &tool);
        // All vertices at z=0, fiber at z=5. h = 0 - 5 = -5 < 0. No contact.
        assert!(fiber.intervals().is_empty());
    }

    #[test]
    fn test_vertex_push_at_fiber_height() {
        let tool = FlatEndmill::new(10.0, 25.0);
        // Fiber at y=0, vertices near the fiber (within cutter radius = 5)
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        let tri = Triangle::new(
            P3::new(-10.0, 2.0, 0.0), // y=2, within R=5
            P3::new(10.0, 2.0, 0.0),
            P3::new(0.0, -2.0, 0.0),
        );
        vertex_push(&mut fiber, &tri, &tool);
        // Vertices at z=0 = fiber.z, h=0, width=R=5, perp dist=2 < 5. Should find contact.
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_vertex_push_ball_endmill() {
        let tool = BallEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        // Vertex at (0, 0, 3) — h=3, width_at_height(3) = sqrt(2*5*3 - 9) = sqrt(21) ≈ 4.58
        let tri = Triangle::new(
            P3::new(0.0, 0.0, 3.0),
            P3::new(100.0, 100.0, 100.0),
            P3::new(-100.0, 100.0, 100.0),
        );
        vertex_push(&mut fiber, &tri, &tool);
        // Only the first vertex should contribute (others above cutter)
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_facet_push_horizontal_at_fiber() {
        let tool = FlatEndmill::new(10.0, 25.0);
        // Use a big triangle that definitely contains the fiber's CC points
        let mut fiber = Fiber::new_x(0.0, 0.0, -10.0, 10.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 0.0),
            P3::new(50.0, -50.0, 0.0),
            P3::new(0.0, 50.0, 0.0),
        );
        facet_push(&mut fiber, &tri, &tool);
        // Horizontal triangle at fiber Z: flat endmill has n=(0,0,1), r1=R, r2=0
        // CC = CL (since xy_normal=0 when nxy_len=0). Fiber endpoints are inside triangle.
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_push_cutter_triangle_combines() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 0.0, -50.0, 50.0);
        let tri = horizontal_tri(0.0);
        push_cutter_triangle(&mut fiber, &tri, &tool);
        assert!(!fiber.intervals().is_empty());
    }

    #[test]
    fn test_push_cutter_fiber_with_mesh() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Fiber at z=10 (midway through hemisphere)
        let mut fiber = Fiber::new_x(0.0, 10.0, -30.0, 30.0);
        push_cutter_fiber(&mut fiber, &mesh, &index, &tool);

        // Should have intervals (the cutter contacts the hemisphere)
        assert!(
            !fiber.intervals().is_empty(),
            "Should find contacts on hemisphere at z=10"
        );
    }

    #[test]
    fn test_push_cutter_no_contact_above() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Fiber at z=25 (above hemisphere, apex at z=20)
        let mut fiber = Fiber::new_x(0.0, 25.0, -30.0, 30.0);
        push_cutter_fiber(&mut fiber, &mesh, &index, &tool);
        assert!(fiber.intervals().is_empty(), "No contact above hemisphere");
    }

    #[test]
    fn test_batch_push_cutter() {
        use crate::mesh::make_test_hemisphere;
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let mut fibers: Vec<Fiber> = (-5..=5)
            .map(|i| Fiber::new_x(i as f64 * 4.0, 10.0, -30.0, 30.0))
            .collect();

        batch_push_cutter(&mut fibers, &mesh, &index, &tool);

        // Center fibers should have intervals, outer ones may not
        let center_fiber = &fibers[5]; // y=0
        assert!(!center_fiber.intervals().is_empty());
    }

    #[test]
    fn test_edge_push_diagonal_flat() {
        // Known diagonal edge with flat endmill, verify interval accuracy.
        let tool = FlatEndmill::new(10.0, 25.0); // R=5
        let mut fiber = Fiber::new_x(0.0, 0.0, -30.0, 30.0);
        // Diagonal edge from (-10, -2, 0) to (10, 2, 0) at z=0 = fiber.z
        // h=0 → width = R = 5, perp_dist from y=0 fiber varies along edge
        let p1 = P3::new(-10.0, -2.0, 0.0);
        let p2 = P3::new(10.0, 2.0, 0.0);
        edge_push_single(&mut fiber, &p1, &p2, &tool);

        let intervals = fiber.intervals();
        assert!(
            !intervals.is_empty(),
            "Diagonal edge at fiber height should produce contact"
        );

        // The contact interval should be roughly centered near t=0.5 (x=0)
        // and span a reasonable fraction of the fiber
        let (lo, hi) = (intervals[0].lower, intervals[0].upper);
        let fiber_range = 60.0; // fiber from -30 to 30
        let contact_len = (hi - lo) * fiber_range;
        assert!(
            contact_len > 5.0 && contact_len < 40.0,
            "Contact length {:.1} should be reasonable for R=5 tool on diagonal edge",
            contact_len
        );
    }

    #[test]
    fn test_edge_push_diagonal_ball() {
        // Same diagonal edge with ball endmill.
        let tool = BallEndmill::new(10.0, 25.0); // R=5
        let mut fiber = Fiber::new_x(0.0, 3.0, -30.0, 30.0); // fiber at z=3
        // Edge from (-10, -2, 0) to (10, 2, 6) — crosses fiber z=3 midway
        let p1 = P3::new(-10.0, -2.0, 0.0);
        let p2 = P3::new(10.0, 2.0, 6.0);
        edge_push_single(&mut fiber, &p1, &p2, &tool);

        let intervals = fiber.intervals();
        assert!(
            !intervals.is_empty(),
            "Diagonal edge crossing fiber height should produce contact with ball endmill"
        );

        // Ball endmill has narrower width at height — interval should be narrower than flat
        let (lo, hi) = (intervals[0].lower, intervals[0].upper);
        assert!(
            hi > lo,
            "Should have non-zero contact interval: lo={:.3}, hi={:.3}",
            lo,
            hi
        );
    }

    #[test]
    fn test_edge_push_sloped_edge() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut fiber = Fiber::new_x(0.0, 5.0, -50.0, 50.0);
        // Triangle with edge crossing fiber height
        let tri = Triangle::new(
            P3::new(0.0, -1.0, 0.0),
            P3::new(0.0, 1.0, 10.0),
            P3::new(20.0, 0.0, 5.0),
        );
        edge_push(&mut fiber, &tri, &tool);
        // Edge from z=0 to z=10 crosses fiber z=5
        assert!(
            !fiber.intervals().is_empty(),
            "Should find edge contact at z=5"
        );
    }
}
