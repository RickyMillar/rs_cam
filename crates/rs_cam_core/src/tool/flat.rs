//! Flat end mill (CylCutter) implementation.
//!
//! Profile: height(r) = 0 for r <= R
//! The flat bottom means CC is always at the same Z as the surface contact point.
//!
//! Parameters: center_height=0, normal_length=0, xy_normal_length=R

use super::{CLPoint, MillingCutter};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct FlatEndmill {
    pub diameter: f64,
    pub cutting_length: f64,
}

impl FlatEndmill {
    pub fn new(diameter: f64, cutting_length: f64) -> Self {
        Self {
            diameter,
            cutting_length,
        }
    }
}

impl MillingCutter for FlatEndmill {
    fn diameter(&self) -> f64 {
        self.diameter
    }
    fn length(&self) -> f64 {
        self.cutting_length
    }
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        crate::feeds::ToolGeometryHint::Flat
    }

    fn height_at_radius(&self, r: f64) -> Option<f64> {
        if r <= self.radius() + 1e-10 {
            Some(0.0) // flat bottom
        } else {
            None
        }
    }

    fn width_at_height(&self, _h: f64) -> f64 {
        self.radius() // constant width
    }

    fn center_height(&self) -> f64 {
        0.0
    }
    fn normal_length(&self) -> f64 {
        0.0
    }
    fn xy_normal_length(&self) -> f64 {
        self.radius()
    }

    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        // Flat endmill edge test: circle-line intersection
        // In canonical coords (CL at origin, edge as line segment),
        // find where the flat bottom circle intersects the edge.
        let r = self.radius();

        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dz = p2.z - p1.z;
        let edge_len_xy_sq = dx * dx + dy * dy;

        if edge_len_xy_sq < 1e-20 {
            return; // degenerate edge (zero XY length)
        }

        // Parameter t for closest point on the infinite line to CL
        let t_closest = ((cl.x - p1.x) * dx + (cl.y - p1.y) * dy) / edge_len_xy_sq;

        // Perpendicular XY distance from CL to the infinite line
        let px = p1.x + t_closest * dx;
        let py = p1.y + t_closest * dy;
        let d_sq = (cl.x - px) * (cl.x - px) + (cl.y - py) * (cl.y - py);

        if d_sq > r * r {
            return; // edge too far away
        }

        // Half-width of cutter at this distance
        let s = ((r * r - d_sq) / edge_len_xy_sq).sqrt();

        // Two candidate contact points along the edge parameter
        let edge_len_xy = edge_len_xy_sq.sqrt();
        for &t in &[t_closest - s * edge_len_xy, t_closest + s * edge_len_xy] {
            // Actually, s is already in edge-parameter units when computed this way.
            // Let me recompute properly.
            let _ = t; // discard
        }

        // Correct approach: the contact points are at t = t_closest ± s
        // where s = sqrt(r² - d²) / |edge_xy|
        let s = (r * r - d_sq).sqrt() / edge_len_xy;

        for &t in &[t_closest - s, t_closest + s] {
            if (-1e-8..=1.0 + 1e-8).contains(&t) {
                let z = p1.z + t * dz;
                cl.update_z(z); // flat bottom: CL.z = CC.z
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::geo::{P3, Triangle};

    #[test]
    fn test_flat_profile() {
        let tool = FlatEndmill::new(10.0, 25.0);
        assert_eq!(tool.radius(), 5.0);
        assert_eq!(tool.height_at_radius(0.0), Some(0.0));
        assert_eq!(tool.height_at_radius(3.0), Some(0.0));
        assert_eq!(tool.height_at_radius(5.0), Some(0.0));
        assert!(tool.height_at_radius(6.0).is_none());
    }

    #[test]
    fn test_flat_vertex_drop_on_flat_surface() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);

        // Vertex directly below CL at z=5
        tool.vertex_drop(&mut cl, &P3::new(0.0, 0.0, 5.0));
        assert!((cl.z - 5.0).abs() < 1e-10);

        // Vertex at radius 3 (within tool) at z=10
        let mut cl2 = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl2, &P3::new(3.0, 0.0, 10.0));
        // Flat endmill: height(3) = 0, so CL.z = 10 - 0 = 10
        assert!((cl2.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_flat_facet_drop_horizontal() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 5.0),
            P3::new(50.0, -50.0, 5.0),
            P3::new(0.0, 50.0, 5.0),
        );

        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(hit);
        // Flat endmill on horizontal surface: CL.z = surface z
        assert!((cl.z - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_flat_edge_drop() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Edge along Y axis at x=3, z=7
        let p1 = P3::new(3.0, -10.0, 7.0);
        let p2 = P3::new(3.0, 10.0, 7.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Distance from CL to edge = 3, which is within radius 5
        // Flat endmill: CL.z = edge z = 7
        assert!((cl.z - 7.0).abs() < 1e-10);
    }

    // --- Task E-flat: edge_drop with vertical edge ---

    #[test]
    fn test_flat_edge_drop_vertical_edge_is_ignored() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Vertical edge: same XY, different Z
        let p1 = P3::new(2.0, 3.0, 0.0);
        let p2 = P3::new(2.0, 3.0, 10.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Vertical edge has zero XY length, should be skipped
        assert_eq!(
            cl.z,
            f64::NEG_INFINITY,
            "Vertical edge should not produce contact"
        );
    }

    // --- edge_drop with horizontal edge (parallel to X) ---

    #[test]
    fn test_flat_edge_drop_horizontal_along_x() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Edge along X axis at y=4, z=3
        let p1 = P3::new(-10.0, 4.0, 3.0);
        let p2 = P3::new(10.0, 4.0, 3.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Distance from CL(0,0) to edge line at y=4 is 4, which is within radius 5
        // Flat endmill: CL.z = edge z = 3
        assert!(
            (cl.z - 3.0).abs() < 1e-10,
            "Horizontal edge along X: CL.z = {}, expected 3.0",
            cl.z
        );
    }

    // --- edge_drop with diagonal edge ---

    #[test]
    fn test_flat_edge_drop_diagonal_sloped() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Diagonal sloped edge from (0,-5,0) to (0,5,10)
        // Edge is along Y with a Z slope of 10/10 = 1
        let p1 = P3::new(0.0, -5.0, 0.0);
        let p2 = P3::new(0.0, 5.0, 10.0);

        let mut cl = CLPoint::new(3.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // d = 3 (within radius 5)
        // s = sqrt(25 - 9) / 10 = 4/10 = 0.4
        // t_closest = ((3-0)*0 + (0-(-5))*10) / (0+100) = 50/100 = 0.5
        // t = 0.5 +/- 0.4 => 0.1 and 0.9
        // z at t=0.1: 0 + 0.1*10 = 1.0
        // z at t=0.9: 0 + 0.9*10 = 9.0
        // Flat endmill picks max z: 9.0
        assert!(
            (cl.z - 9.0).abs() < 1e-10,
            "Diagonal sloped edge: CL.z = {}, expected 9.0",
            cl.z
        );
    }

    // --- edge_drop at exact tool radius boundary ---

    #[test]
    fn test_flat_edge_drop_at_exact_radius() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius = 5

        // Edge exactly at distance = radius (5.0) from CL
        let p1 = P3::new(5.0, -10.0, 2.0);
        let p2 = P3::new(5.0, 10.0, 2.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // d = 5 = r, so d_sq = r*r => s = 0
        // This means one contact point at t_closest
        assert!(
            (cl.z - 2.0).abs() < 1e-6,
            "Edge at exact radius: CL.z = {}, expected 2.0",
            cl.z
        );
    }

    #[test]
    fn test_flat_edge_drop_just_beyond_radius() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius = 5

        // Edge just beyond radius (5.01 from CL)
        let p1 = P3::new(5.01, -10.0, 2.0);
        let p2 = P3::new(5.01, 10.0, 2.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // d = 5.01 > r = 5, so no contact
        assert_eq!(
            cl.z,
            f64::NEG_INFINITY,
            "Edge beyond radius should produce no contact"
        );
    }

    // --- vertex_drop edge cases ---

    #[test]
    fn test_flat_vertex_drop_at_exact_radius() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius = 5

        // Vertex at exactly the tool radius
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(5.0, 0.0, 8.0));
        // height_at_radius(5.0) = Some(0.0), so CL.z = 8 - 0 = 8
        assert!(
            (cl.z - 8.0).abs() < 1e-10,
            "Vertex at exact radius: CL.z = {}, expected 8.0",
            cl.z
        );
    }

    #[test]
    fn test_flat_vertex_drop_beyond_radius() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius = 5

        // Vertex beyond tool radius
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(6.0, 0.0, 8.0));
        // height_at_radius(6.0) = None, so no contact
        assert_eq!(
            cl.z,
            f64::NEG_INFINITY,
            "Vertex beyond radius should not update CL"
        );
    }

    // --- facet_drop edge cases ---

    #[test]
    fn test_flat_facet_drop_vertical_triangle_returns_false() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Vertical triangle (normal in XY plane, nz ~ 0)
        let tri = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(0.0, 0.0, 10.0),
            P3::new(0.0, 10.0, 5.0),
        );

        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(!hit, "Facet drop on vertical triangle should return false");
    }

    #[test]
    fn test_flat_facet_drop_cl_outside_triangle() {
        let tool = FlatEndmill::new(10.0, 25.0);

        // Small triangle, CL is far outside
        let tri = Triangle::new(
            P3::new(100.0, 100.0, 5.0),
            P3::new(101.0, 100.0, 5.0),
            P3::new(100.5, 101.0, 5.0),
        );

        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(
            !hit,
            "Facet drop with CL far outside triangle should return false"
        );
    }

    // --- Zero-radius tool ---

    #[test]
    fn test_flat_zero_radius_profile() {
        let tool = FlatEndmill::new(0.0, 10.0);
        assert_eq!(tool.radius(), 0.0);
        assert_eq!(tool.height_at_radius(0.0), Some(0.0));
        assert!(tool.height_at_radius(0.1).is_none());
    }
}
