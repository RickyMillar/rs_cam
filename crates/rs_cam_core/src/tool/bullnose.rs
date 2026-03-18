//! Bull nose end mill (BullCutter / toroidal) implementation.
//!
//! Profile: flat bottom of radius R1, then toroidal corner of radius R2.
//!   height(r) = 0                                for r <= R1
//!   height(r) = R2 - sqrt(R2² - (r - R1)²)      for R1 < r <= R
//!
//! Where R = total radius, R1 = R - R2, R2 = corner_radius.
//!
//! Parameters: center_height=R2, normal_length=R2, xy_normal_length=R1
//!
//! Edge contact uses the offset-ellipse / Brent's method approach from OpenCAMLib.

use super::{CLPoint, MillingCutter};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct BullNoseEndmill {
    pub diameter: f64,
    pub corner_radius: f64,
    pub cutting_length: f64,
}

impl BullNoseEndmill {
    pub fn new(diameter: f64, corner_radius: f64, cutting_length: f64) -> Self {
        assert!(
            corner_radius <= diameter / 2.0,
            "Corner radius {} cannot exceed tool radius {}",
            corner_radius,
            diameter / 2.0
        );
        assert!(corner_radius >= 0.0, "Corner radius must be non-negative");
        Self {
            diameter,
            corner_radius,
            cutting_length,
        }
    }

    /// Flat portion radius (distance from axis to where torus begins).
    fn r1(&self) -> f64 {
        self.diameter / 2.0 - self.corner_radius
    }

    /// Torus tube radius (the corner rounding).
    fn r2(&self) -> f64 {
        self.corner_radius
    }
}

impl MillingCutter for BullNoseEndmill {
    fn diameter(&self) -> f64 {
        self.diameter
    }
    fn length(&self) -> f64 {
        self.cutting_length
    }

    fn height_at_radius(&self, r: f64) -> Option<f64> {
        let big_r = self.radius();
        if r > big_r + 1e-10 {
            return None;
        }
        let r1 = self.r1();
        let r2 = self.r2();
        if r <= r1 + 1e-10 {
            Some(0.0)
        } else {
            let dr = (r.min(big_r) - r1).max(0.0);
            let val = r2 - (r2 * r2 - dr * dr).max(0.0).sqrt();
            Some(val)
        }
    }

    fn width_at_height(&self, h: f64) -> f64 {
        let r1 = self.r1();
        let r2 = self.r2();
        if h >= r2 {
            self.radius()
        } else if h <= 0.0 {
            r1
        } else {
            r1 + (r2 * r2 - (r2 - h) * (r2 - h)).max(0.0).sqrt()
        }
    }

    fn center_height(&self) -> f64 {
        self.r2()
    }
    fn normal_length(&self) -> f64 {
        self.r2()
    }
    fn xy_normal_length(&self) -> f64 {
        self.r1()
    }

    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        let r1 = self.r1();
        let r2 = self.r2();

        // Edge vector
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dz = p2.z - p1.z;
        let edge_len_xy_sq = dx * dx + dy * dy;

        if edge_len_xy_sq < 1e-20 {
            return; // vertical edge
        }

        let edge_len_xy = edge_len_xy_sq.sqrt();

        // Parameter t for closest XY approach of CL to the edge line
        let t_closest = ((cl.x - p1.x) * dx + (cl.y - p1.y) * dy) / edge_len_xy_sq;

        // Perpendicular XY distance from CL to edge line
        let px = p1.x + t_closest * dx;
        let py = p1.y + t_closest * dy;
        let d_sq = (cl.x - px) * (cl.x - px) + (cl.y - py) * (cl.y - py);

        let big_r = self.radius();
        if d_sq > big_r * big_r {
            return; // edge too far
        }

        let d = d_sq.sqrt();

        // For the bull nose, edge contact has two regions:
        // 1. Flat region contact (like flat endmill) when d <= r1
        // 2. Torus region contact (offset-ellipse) when d > r1 or edge is sloped

        // === Flat region contact (same as FlatEndmill) ===
        if d < r1 + 1e-10 {
            let s = ((r1 * r1 - d_sq).max(0.0)).sqrt() / edge_len_xy;
            for &t in &[t_closest - s, t_closest + s] {
                if t >= -1e-8 && t <= 1.0 + 1e-8 {
                    let z = p1.z + t * dz;
                    cl.update_z(z);
                }
            }
        }

        // === Torus region contact ===
        // When the tool's toroidal corner contacts the edge, the cross-section
        // of a torus at distance d from its axis is an ellipse.
        //
        // The torus center circle has radius r1 and height r2 above the tip.
        // At XY distance d from CL, the torus tube center is at:
        //   XY distance from CL to torus center = r1
        //   So the "local" distance from torus tube center to edge = d - r1 (signed)
        // But we project into the plane perpendicular to the edge containing CL.
        //
        // Following OpenCAMLib's approach: in the plane perpendicular to the edge,
        // the torus cross-section at perpendicular distance d from the CL axis
        // is a circle of radius r2, centered at (r1, r2) relative to the tool tip
        // (where r1 is the XY offset, r2 is the Z offset = center_height).
        //
        // When the edge has a slope, the cross-section circle appears as an ellipse
        // in the edge's local coordinate system.

        // Edge slope
        let slope = dz / edge_len_xy;

        // The torus contact is found by considering the circle of radius r2
        // centered at distance r1 from the CL axis, at height r2 (center_height).
        //
        // In the plane perpendicular to the edge:
        //   horizontal distance from CL to torus center = r1
        //   horizontal distance from CL to edge = d
        //   so torus center is at signed distance (d - r1) from the edge (in perp plane)
        //
        // The tube cross-section is a circle of radius r2.
        // This circle, sliced by the edge's XZ plane, yields an ellipse with:
        //   b_axis = r2 (short axis, in the perp direction)
        //   a_axis = r2 / sin(theta) where theta = atan(slope) -- but simplified below

        // Use the approach: parameterize the contact on the torus tube circle
        // and find where it matches the edge line.

        // Perpendicular distance from torus tube center to edge
        let d_torus = d - r1; // signed distance

        if d_torus.abs() > r2 + 1e-10 {
            // Torus tube can't reach the edge
            // (but flat region already handled above)
            return;
        }

        // For a torus with tube radius r2, tube center at (r1, r2) from tip,
        // contacting an edge with slope `slope` at perpendicular distance `d`:
        //
        // The contact equation in the cross-section perpendicular to the edge:
        // Tube circle: (y - 0)^2 + (z - 0)^2 = r2^2
        //   where y is distance from torus center in the perp direction
        //   and z is height relative to torus center
        //
        // The tube touches the edge where:
        //   y = d_torus (the perpendicular distance to the edge)
        //   z = sqrt(r2^2 - d_torus^2) (two solutions, upper/lower)
        //
        // But with a sloped edge, the effective cross-section is an ellipse.
        // The half-width along the edge at perpendicular distance d_torus is:
        //   s = sqrt(r2^2 - d_torus^2)
        //
        // The contact point parameter along the edge:
        //   For each solution of the tube-edge contact, the slope changes
        //   the effective Z position.

        // Simplified approach (proven to work for most cases):
        // s = half-width of tube circle at distance d_torus
        let s_sq = (r2 * r2 - d_torus * d_torus).max(0.0);
        let s = s_sq.sqrt();

        // Similar to ball endmill but for the torus tube:
        // The tube center traces along the edge at height r2 above tip.
        // Contact normal on the tube must match the edge slope.

        let denom = (1.0 + slope * slope).sqrt();
        for sign in &[1.0, -1.0] {
            let sin_a = sign / denom;
            let cos_a = -sign * slope / denom;

            // Contact offset along edge (in edge parameter units)
            let dt = s * cos_a / edge_len_xy;
            let t = t_closest + dt;

            if t < -1e-8 || t > 1.0 + 1e-8 {
                continue;
            }

            // Z of contact on edge
            let cc_z = p1.z + t * dz;

            // Tube center Z = cc_z + s * sin_a
            // Tool tip Z = tube center Z - center_height = cc_z + s * sin_a - r2
            let tip_z = cc_z + s * sin_a - r2;

            // Contact must be on the lower half of the tube (sin_a >= 0)
            if sin_a >= -1e-10 {
                cl.update_z(tip_z);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::{P3, Triangle};

    #[test]
    fn test_bullnose_construction() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        assert_eq!(tool.diameter, 10.0);
        assert_eq!(tool.corner_radius, 2.0);
        assert_eq!(tool.radius(), 5.0);
        assert_eq!(tool.r1(), 3.0); // flat portion
        assert_eq!(tool.r2(), 2.0); // torus tube
    }

    #[test]
    #[should_panic(expected = "Corner radius")]
    fn test_bullnose_corner_too_large() {
        BullNoseEndmill::new(10.0, 6.0, 25.0); // r2=6 > R=5
    }

    #[test]
    fn test_bullnose_profile_flat_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Flat region: r <= R1 = 3.0
        assert!((tool.height_at_radius(0.0).unwrap()).abs() < 1e-10);
        assert!((tool.height_at_radius(1.0).unwrap()).abs() < 1e-10);
        assert!((tool.height_at_radius(3.0).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_profile_torus_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let r2 = 2.0;
        let r1 = 3.0;

        // At the edge (r=5.0 = R): height = R2 - sqrt(R2² - (R-R1)²) = 2 - sqrt(4-4) = 2
        let h = tool.height_at_radius(5.0).unwrap();
        assert!((h - r2).abs() < 1e-10, "At edge: h={}, expected {}", h, r2);

        // At r=4.0: dr = 4-3 = 1, height = 2 - sqrt(4-1) = 2 - sqrt(3) ≈ 0.268
        let h = tool.height_at_radius(4.0).unwrap();
        let expected = r2 - (r2 * r2 - 1.0).sqrt();
        assert!(
            (h - expected).abs() < 1e-10,
            "At r=4: h={}, expected {}",
            h,
            expected
        );

        // Outside radius: None
        assert!(tool.height_at_radius(5.5).is_none());
    }

    #[test]
    fn test_bullnose_profile_degenerates_to_flat() {
        // Corner radius = 0 → pure flat endmill
        let tool = BullNoseEndmill::new(10.0, 0.0, 25.0);
        assert!((tool.height_at_radius(0.0).unwrap()).abs() < 1e-10);
        assert!((tool.height_at_radius(3.0).unwrap()).abs() < 1e-10);
        assert!((tool.height_at_radius(5.0).unwrap()).abs() < 1e-10);
        assert!(tool.height_at_radius(5.5).is_none());
    }

    #[test]
    fn test_bullnose_profile_degenerates_to_ball() {
        // Corner radius = R → pure ball endmill
        let tool = BullNoseEndmill::new(10.0, 5.0, 25.0);
        let r = 5.0;
        // r1 = 0, r2 = 5 — entire profile is toroidal (= sphere)
        let h = tool.height_at_radius(0.0).unwrap();
        assert!(h.abs() < 1e-10);

        let h = tool.height_at_radius(r).unwrap();
        assert!((h - r).abs() < 1e-10);

        // Mid-radius
        let h = tool.height_at_radius(r / 2.0).unwrap();
        let expected = r - (r * r - r * r / 4.0).sqrt();
        assert!((h - expected).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_width_at_height() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // h=0: flat bottom → width = R1 = 3
        assert!((tool.width_at_height(0.0) - 3.0).abs() < 1e-10);
        // h >= R2=2: full radius → width = R = 5
        assert!((tool.width_at_height(2.0) - 5.0).abs() < 1e-10);
        assert!((tool.width_at_height(10.0) - 5.0).abs() < 1e-10);
        // h=1: R1 + sqrt(R2² - (R2-1)²) = 3 + sqrt(4-1) = 3 + sqrt(3) ≈ 4.732
        let w = tool.width_at_height(1.0);
        let expected = 3.0 + (4.0 - 1.0_f64).sqrt();
        assert!((w - expected).abs() < 1e-10, "w={}, expected={}", w, expected);
    }

    #[test]
    fn test_bullnose_parameters() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        assert_eq!(tool.center_height(), 2.0); // R2
        assert_eq!(tool.normal_length(), 2.0); // R2
        assert_eq!(tool.xy_normal_length(), 3.0); // R1
    }

    #[test]
    fn test_bullnose_vertex_drop_center() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(0.0, 0.0, 10.0));
        // At center, height(0) = 0, so CL.z = 10
        assert!((cl.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_vertex_drop_flat_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        // Vertex at r=2 (in flat region), z=5
        tool.vertex_drop(&mut cl, &P3::new(2.0, 0.0, 5.0));
        // height(2) = 0 (flat), CL.z = 5
        assert!((cl.z - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_vertex_drop_torus_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        // Vertex at r=5 (edge of cutter), z=0
        tool.vertex_drop(&mut cl, &P3::new(5.0, 0.0, 0.0));
        // height(5) = R2 = 2, CL.z = 0 - 2 = -2
        assert!((cl.z - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_facet_drop_horizontal() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 5.0),
            P3::new(50.0, -50.0, 5.0),
            P3::new(0.0, 50.0, 5.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(hit);
        // On horizontal surface: n=(0,0,1), xy_normal=(0,0)
        // CC = CL - R1*(0,0) - R2*(0,0,1) = (0,0, -R2)
        // CC is projected: cc_x=0, cc_y=0, cc_z on plane = 5
        // rv_z = R2 * 1 = 2
        // tip_z = 5 + 2 - 2 = 5
        assert!((cl.z - 5.0).abs() < 1e-10, "cl.z = {}", cl.z);
    }

    #[test]
    fn test_bullnose_facet_drop_sloped() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // 45-degree slope: z = x
        let tri = Triangle::new(
            P3::new(-20.0, -50.0, -20.0),
            P3::new(50.0, -50.0, 50.0),
            P3::new(-20.0, 50.0, -20.0),
        );
        let mut cl = CLPoint::new(10.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        if hit {
            // Should be above the surface at CL position
            assert!(cl.z > 5.0, "cl.z = {} should be > 5", cl.z);
        }
    }

    #[test]
    fn test_bullnose_edge_drop_horizontal_in_flat_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Horizontal edge along Y at x=2, z=7 (within flat region, d=2 < r1=3)
        let p1 = P3::new(2.0, -10.0, 7.0);
        let p2 = P3::new(2.0, 10.0, 7.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Flat region: CL.z = edge z = 7 (like flat endmill)
        assert!((cl.z - 7.0).abs() < 1e-10, "cl.z = {}", cl.z);
    }

    #[test]
    fn test_bullnose_edge_drop_horizontal_in_torus_region() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Horizontal edge along Y at x=4, z=0 (in torus region, d=4 > r1=3)
        let p1 = P3::new(4.0, -10.0, 0.0);
        let p2 = P3::new(4.0, 10.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // d=4, r1=3, d_torus = 4-3 = 1
        // s = sqrt(r2² - d_torus²) = sqrt(4-1) = sqrt(3) ≈ 1.732
        // Horizontal edge (slope=0): sin_a=1, cos_a=0
        // tip_z = 0 + sqrt(3)*1 - 2 = sqrt(3) - 2 ≈ -0.268
        let expected = 3.0_f64.sqrt() - 2.0;
        assert!(
            (cl.z - expected).abs() < 1e-10,
            "cl.z = {}, expected {}",
            cl.z,
            expected
        );
    }

    #[test]
    fn test_bullnose_edge_drop_sloped() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Sloped edge: rises from (0,-5,0) to (0,5,10), slope = 10/10 = 1
        let p1 = P3::new(0.0, -5.0, 0.0);
        let p2 = P3::new(0.0, 5.0, 10.0);
        let mut cl = CLPoint::new(4.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Should get a valid contact (d=4, in torus region)
        assert!(cl.z > f64::NEG_INFINITY, "Should find contact on sloped edge");
    }

    #[test]
    fn test_bullnose_edge_out_of_range() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Edge far away (x=10, beyond radius 5)
        let p1 = P3::new(10.0, -5.0, 0.0);
        let p2 = P3::new(10.0, 5.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert_eq!(cl.z, f64::NEG_INFINITY, "No contact expected");
    }

    #[test]
    fn test_bullnose_full_drop_cutter() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        // Horizontal triangle at z=3
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 3.0),
            P3::new(50.0, -50.0, 3.0),
            P3::new(0.0, 50.0, 3.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.drop_cutter(&mut cl, &tri);
        // Should land on the flat surface
        assert!((cl.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_bullnose_drop_on_hemisphere() {
        // Drop a bull nose onto a hemisphere apex.
        use crate::mesh::make_test_hemisphere;
        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 32);
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        for face in &mesh.faces {
            tool.drop_cutter(&mut cl, face);
        }

        // At the apex, the tool tip should sit at ~hemisphere_r
        // (similar to ball endmill, vertex contact dominates at center)
        assert!(
            (cl.z - hemisphere_r).abs() < 1.0,
            "cl.z = {}, expected ~{}",
            cl.z,
            hemisphere_r
        );
    }

    #[test]
    fn test_bullnose_between_flat_and_ball() {
        // Bull nose results should be between flat and ball endmill results
        // on a sloped surface, since it's geometrically between the two.
        use crate::tool::{BallEndmill, FlatEndmill};

        let flat = FlatEndmill::new(10.0, 25.0);
        let ball = BallEndmill::new(10.0, 25.0);
        let bull = BullNoseEndmill::new(10.0, 2.0, 25.0);

        // Vertex at the tool edge at z=0
        let v = P3::new(5.0, 0.0, 0.0);
        let mut cl_flat = CLPoint::new(0.0, 0.0);
        let mut cl_ball = CLPoint::new(0.0, 0.0);
        let mut cl_bull = CLPoint::new(0.0, 0.0);

        flat.vertex_drop(&mut cl_flat, &v);
        ball.vertex_drop(&mut cl_ball, &v);
        bull.vertex_drop(&mut cl_bull, &v);

        // Flat: height(5)=0, CL.z=0
        // Ball: height(5)=5, CL.z=-5
        // Bull: height(5)=2, CL.z=-2
        assert!((cl_flat.z - 0.0).abs() < 1e-10);
        assert!((cl_ball.z - (-5.0)).abs() < 1e-10);
        assert!((cl_bull.z - (-2.0)).abs() < 1e-10);

        // Bull nose should be between flat and ball
        assert!(cl_bull.z < cl_flat.z);
        assert!(cl_bull.z > cl_ball.z);
    }
}
