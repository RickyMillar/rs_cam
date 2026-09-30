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

use super::{
    CLPoint, ChipGeometry, EngagementError, EngagementMode, MillingCutter,
    flat_chip_geometry_for_radius,
};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct BullNoseEndmill {
    pub diameter: f64,
    pub corner_radius: f64,
    pub cutting_length: f64,
    pub helix_deg: f64,
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
            helix_deg: 30.0,
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
    fn helix_deg(&self) -> f64 {
        self.helix_deg
    }
    fn corner_radius_mm(&self) -> f64 {
        self.corner_radius
    }
    fn chip_geometry(
        &self,
        axial_doc_mm: f64,
        arc_engagement_radians: f64,
        feed_per_tooth_mm: f64,
        flute_count: u32,
        _mode: EngagementMode,
    ) -> Result<ChipGeometry, EngagementError> {
        if axial_doc_mm <= self.corner_radius {
            return Err(EngagementError::Unsupported {
                reason: "toroidal engagement at corner not modeled".to_owned(),
            });
        }
        flat_chip_geometry_for_radius(
            self.radius(),
            self.helix_deg,
            axial_doc_mm - self.corner_radius,
            arc_engagement_radians,
            feed_per_tooth_mm,
            flute_count,
        )
    }
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        crate::feeds::ToolGeometryHint::Bull {
            corner_radius: self.corner_radius,
        }
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
        // G-TIERBURIAL: the exact contact of the flat-and-torus profile
        // (`super::edge_drop_by_profile`). The torus has no closed-form
        // edge contact (a quartic), so the argmax is found on the slope of
        // the touch height, `g(a) = |m| - height'(rho) a / rho`, which is
        // positive at `a = 0` and falls monotonically (the height is
        // convex): bisection to the float floor. In the flat part
        // `height' = 0`, so a sloped edge's contact is always on the torus
        // or at the rim.
        let r1 = self.r1();
        let r2 = self.r2();
        let big_r = self.radius();
        let slope_at = |rho: f64| {
            if rho <= r1 {
                return 0.0;
            }
            let e = rho - r1;
            let den = r2 * r2 - e * e;
            if den <= 0.0 {
                f64::INFINITY
            } else {
                e / den.sqrt()
            }
        };
        super::edge_drop_by_profile(
            cl,
            p1,
            p2,
            big_r,
            |rho| self.height_at_radius(rho).unwrap_or(f64::INFINITY),
            |d, m| {
                if m == 0.0 {
                    return 0.0;
                }
                let w_reach = (big_r * big_r - d * d).max(0.0).sqrt();
                let g = |a: f64| {
                    let rho = (d * d + a * a).sqrt();
                    if rho <= 0.0 {
                        m.abs()
                    } else {
                        m.abs() - slope_at(rho) * a / rho
                    }
                };
                if g(w_reach) >= 0.0 {
                    return m.signum() * w_reach;
                }
                let (mut lo, mut hi) = (0.0, w_reach);
                for _ in 0..100 {
                    let mid = 0.5 * (lo + hi);
                    if g(mid) > 0.0 {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                m.signum() * 0.5 * (lo + hi)
            },
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
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
        let _r1 = 3.0;

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
        assert!(
            (w - expected).abs() < 1e-10,
            "w={}, expected={}",
            w,
            expected
        );
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
        assert!(
            cl.z > f64::NEG_INFINITY,
            "Should find contact on sloped edge"
        );
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
