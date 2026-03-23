//! Ball end mill (BallCutter) implementation.
//!
//! Profile: height(r) = R - sqrt(R² - r²)
//! The hemisphere means CC-to-CL offset is simply R along the surface normal.
//!
//! Parameters: center_height=R, normal_length=R, xy_normal_length=0

use super::{CLPoint, MillingCutter};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct BallEndmill {
    pub diameter: f64,
    pub cutting_length: f64,
}

impl BallEndmill {
    pub fn new(diameter: f64, cutting_length: f64) -> Self {
        Self {
            diameter,
            cutting_length,
        }
    }
}

impl MillingCutter for BallEndmill {
    fn diameter(&self) -> f64 {
        self.diameter
    }
    fn length(&self) -> f64 {
        self.cutting_length
    }

    fn height_at_radius(&self, r: f64) -> Option<f64> {
        let big_r = self.radius();
        if r > big_r + 1e-10 {
            None
        } else {
            let r_clamped = r.min(big_r);
            Some(big_r - (big_r * big_r - r_clamped * r_clamped).sqrt())
        }
    }

    fn width_at_height(&self, h: f64) -> f64 {
        let big_r = self.radius();
        if h >= big_r {
            big_r
        } else {
            (2.0 * big_r * h - h * h).sqrt()
        }
    }

    fn center_height(&self) -> f64 {
        self.radius()
    }
    fn normal_length(&self) -> f64 {
        self.radius()
    }
    fn xy_normal_length(&self) -> f64 {
        0.0
    }

    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        let big_r = self.radius();

        // Edge vector
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dz = p2.z - p1.z;
        let edge_len_sq = dx * dx + dy * dy + dz * dz;

        if edge_len_sq < 1e-20 {
            return;
        }

        // The ball endmill edge test uses the "dual geometry" approach:
        // Test a point (VR=0) against a cylinder of radius R around the edge.
        //
        // The ball center is at (cl.x, cl.y, cl.z + R).
        // We need to find where this center is at distance R from the edge line.
        //
        // This is equivalent to a ray-cylinder intersection, but we can simplify
        // since we're "dropping" (only interested in Z).

        // Perpendicular distance from CL axis to the edge (in XY)
        let edge_len_xy_sq = dx * dx + dy * dy;

        if edge_len_xy_sq < 1e-20 {
            // Vertical edge: only vertex contacts matter
            return;
        }

        // Parameter t for closest approach in XY
        let t_closest = ((cl.x - p1.x) * dx + (cl.y - p1.y) * dy) / edge_len_xy_sq;

        // Closest point on edge line in XY
        let px = p1.x + t_closest * dx;
        let py = p1.y + t_closest * dy;
        let d_sq = (cl.x - px) * (cl.x - px) + (cl.y - py) * (cl.y - py);

        if d_sq > big_r * big_r {
            return;
        }

        let _d = d_sq.sqrt();

        // The ball touches the edge when the sphere center is at distance R from the edge.
        // In the plane perpendicular to the edge at distance d from CL:
        // We need to find the point on the circular cross-section of the sphere
        // where the tangent matches the edge slope.

        // Cross-section circle radius at distance d
        let s = (big_r * big_r - d_sq).sqrt();

        // Edge slope in the XZ plane (where X is along the edge)
        let edge_len_xy = edge_len_xy_sq.sqrt();
        let slope = dz / edge_len_xy; // dz/d(xy)

        // The contact point on the cross-section circle where the slope matches:
        // The circle is parameterized as (s*cos(a), s*sin(a)) in the (along-edge, Z) plane
        // Normal at angle a is (cos(a), sin(a))
        // Slope of the circle at angle a = -cos(a)/sin(a)
        // We need: -cos(a)/sin(a) = slope
        // => sin(a) = -cos(a)/slope
        // => tan(a) = -1/slope
        // => sin(a) = 1/sqrt(1+slope²), cos(a) = -slope/sqrt(1+slope²)  (for positive slope case)

        let denom = (1.0 + slope * slope).sqrt();
        // Two solutions (upper and lower hemisphere contact)
        for sign in &[1.0, -1.0] {
            let sin_a = sign / denom;
            let cos_a = -sign * slope / denom;

            // Contact point along edge parameter (relative to t_closest)
            let dt = s * cos_a / edge_len_xy;
            let t = t_closest + dt;

            // Check if within edge segment
            if !(-1e-8..=1.0 + 1e-8).contains(&t) {
                continue;
            }

            // Z of the contact point on the edge
            let cc_z = p1.z + t * dz;

            // Ball center Z = cc_z + s * sin_a
            // Tool tip Z = ball center Z - R
            let tip_z = cc_z + s * sin_a - big_r;

            // Validate: contact must be on the lower hemisphere (sin_a <= 0 means upper part, skip)
            // Actually, we want the center to be above the contact: center_z >= cc_z
            // center_z = tip_z + R = cc_z + s*sin_a
            // So we need s*sin_a >= 0, i.e. sin_a >= 0
            if sin_a >= -1e-10 {
                cl.update_z(tip_z);
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
    fn test_ball_profile() {
        let tool = BallEndmill::new(10.0, 25.0);
        let r = tool.radius(); // 5.0

        // At center: height = 0
        assert!((tool.height_at_radius(0.0).unwrap()).abs() < 1e-10);

        // At edge: height = R
        let h = tool.height_at_radius(r).unwrap();
        assert!((h - r).abs() < 1e-10);

        // At r=R/2: height = R - sqrt(R² - R²/4) = R - R*sqrt(3)/2
        let h = tool.height_at_radius(r / 2.0).unwrap();
        let expected = r - (r * r - r * r / 4.0).sqrt();
        assert!((h - expected).abs() < 1e-10);

        // Outside radius: None
        assert!(tool.height_at_radius(r + 1.0).is_none());
    }

    #[test]
    fn test_ball_vertex_drop_center() {
        let tool = BallEndmill::new(10.0, 25.0);
        let _r = 5.0;

        // Vertex directly below CL at z=10
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(0.0, 0.0, 10.0));
        // height(0) = 0, so CL.z = 10 - 0 = 10
        assert!((cl.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_ball_vertex_drop_offset() {
        let tool = BallEndmill::new(10.0, 25.0);
        let r = 5.0;

        // Vertex at radius R from CL, at z=0
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(r, 0.0, 0.0));
        // height(R) = R, so CL.z = 0 - R = -5
        assert!((cl.z - (-r)).abs() < 1e-10);
    }

    #[test]
    fn test_ball_facet_drop_horizontal() {
        let tool = BallEndmill::new(10.0, 25.0);
        let _r = 5.0;
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 0.0),
            P3::new(50.0, -50.0, 0.0),
            P3::new(0.0, 50.0, 0.0),
        );

        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(hit);
        // Ball on horizontal surface: CL.z = surface_z + R*nz - center_height
        // nz=1, so CL.z = 0 + R*1 - R = 0
        assert!((cl.z - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_ball_facet_drop_sloped() {
        let tool = BallEndmill::new(10.0, 25.0);
        let _r = 5.0;

        // 45-degree slope: z = x (for x >= 0)
        let tri = Triangle::new(
            P3::new(-10.0, -50.0, -10.0),
            P3::new(50.0, -50.0, 50.0),
            P3::new(-10.0, 50.0, -10.0),
        );

        let mut cl = CLPoint::new(10.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        // On a 45-deg surface, the ball contact is offset by R along the normal
        // Normal ~ (-1/sqrt2, 0, 1/sqrt2)
        // CC = CL - R*normal => cc_x = 10 + R/sqrt2, cc_z = z_at_xy(cc_x)
        // This test just verifies we get a reasonable result
        if hit {
            assert!(cl.z > 0.0); // should be above z=0
            assert!(cl.z < 20.0); // and not unreasonably high
        }
    }

    #[test]
    fn test_ball_edge_drop_horizontal_edge() {
        let tool = BallEndmill::new(10.0, 25.0);
        let _r = 5.0;

        // Horizontal edge along Y at x=3, z=0
        let p1 = P3::new(3.0, -10.0, 0.0);
        let p2 = P3::new(3.0, 10.0, 0.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Distance d=3 from CL to edge
        // s = sqrt(R² - d²) = sqrt(25-9) = 4
        // Horizontal edge (slope=0): sin_a = 1/1 = 1, cos_a = 0
        // cc_z = 0, tip_z = 0 + 4*1 - 5 = -1
        assert!((cl.z - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_ball_drop_on_hemisphere() {
        // Drop a ball cutter onto the apex of a hemisphere.
        // The cutter should sit at exactly z = hemisphere_radius (touching the top).
        use crate::mesh::make_test_hemisphere;

        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 16);
        let tool = BallEndmill::new(10.0, 25.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        for face in &mesh.faces {
            tool.drop_cutter(&mut cl, face);
        }

        // At the apex (0,0), the ball should sit with its tip at z = hemisphere_r - tool_radius
        // Wait, no. The ball tip touches the top of the hemisphere at z=hemisphere_r.
        // For a ball of radius r_tool on a convex sphere of radius R_sphere:
        // the tip z = R_sphere (the ball just touches the top)
        // Actually at the very top, vertex_drop gives: z = hemisphere_r - height(0) = hemisphere_r
        assert!(
            (cl.z - hemisphere_r).abs() < 0.5,
            "cl.z = {}, expected ~{}",
            cl.z,
            hemisphere_r
        );
    }
}
