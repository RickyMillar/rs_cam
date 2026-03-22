//! Tapered ball end mill (BallConeCutter) implementation.
//!
//! A composite cutter: hemispherical tip transitioning tangentially to a
//! conical taper. Used for 3D finishing where undercut clearance is needed.
//!
//! Profile:
//!   Ball region (r <= r_contact): height(r) = R_ball - sqrt(R_ball² - r²)
//!   Cone region (r > r_contact):  height(r) = r / tan(alpha) + cone_offset
//!
//! Where:
//!   R_ball = ball radius (tip)
//!   alpha = taper half-angle
//!   r_contact = R_ball * cos(alpha)  (tangent junction radius)
//!   h_contact = R_ball * (1 - sin(alpha))  (junction height)
//!   cone_offset = h_contact - r_contact / tan(alpha)  (Z shift for cone continuity)

use super::{CLPoint, MillingCutter};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct TaperedBallEndmill {
    /// Ball tip diameter
    pub ball_diameter: f64,
    /// Shaft diameter at top of taper
    pub shaft_diameter: f64,
    /// Taper half-angle in degrees (from the tool axis)
    pub taper_half_angle_deg: f64,
    pub cutting_length: f64,
    // Precomputed trig values
    alpha_rad: f64,
    tan_alpha: f64,
    sin_alpha: f64,
    cos_alpha: f64,
}

impl TaperedBallEndmill {
    pub fn new(
        ball_diameter: f64,
        taper_half_angle_deg: f64,
        shaft_diameter: f64,
        cutting_length: f64,
    ) -> Self {
        assert!(
            taper_half_angle_deg > 0.0 && taper_half_angle_deg < 90.0,
            "Taper half-angle must be between 0 and 90 degrees, got {}",
            taper_half_angle_deg
        );
        assert!(
            shaft_diameter >= ball_diameter,
            "Shaft diameter {} must be >= ball diameter {}",
            shaft_diameter,
            ball_diameter
        );
        let alpha_rad = taper_half_angle_deg.to_radians();
        let (sin_alpha, cos_alpha) = alpha_rad.sin_cos();
        let tan_alpha = alpha_rad.tan();
        Self {
            ball_diameter,
            shaft_diameter,
            taper_half_angle_deg,
            cutting_length,
            alpha_rad,
            tan_alpha,
            sin_alpha,
            cos_alpha,
        }
    }

    fn ball_radius(&self) -> f64 {
        self.ball_diameter / 2.0
    }

    fn shaft_radius(&self) -> f64 {
        self.shaft_diameter / 2.0
    }

    fn alpha(&self) -> f64 {
        self.alpha_rad
    }

    /// Radius where ball meets cone tangentially.
    fn r_contact(&self) -> f64 {
        self.ball_radius() * self.cos_alpha
    }

    /// Height where ball meets cone tangentially.
    fn h_contact(&self) -> f64 {
        self.ball_radius() * (1.0 - self.sin_alpha)
    }

    /// Z offset applied to the cone region to ensure continuity.
    fn cone_offset(&self) -> f64 {
        self.h_contact() - self.r_contact() / self.tan_alpha
    }
}

impl MillingCutter for TaperedBallEndmill {
    fn diameter(&self) -> f64 {
        self.shaft_diameter // effective cutting diameter at widest point
    }
    fn length(&self) -> f64 {
        self.cutting_length
    }

    fn height_at_radius(&self, r: f64) -> Option<f64> {
        let r_max = self.shaft_radius();
        if r > r_max + 1e-10 {
            return None;
        }

        let r_ball = self.ball_radius();
        let rc = self.r_contact();

        if r <= rc {
            // Ball region
            let r_clamped = r.min(r_ball);
            Some(r_ball - (r_ball * r_ball - r_clamped * r_clamped).max(0.0).sqrt())
        } else {
            // Cone region
            Some(r.min(r_max) / self.tan_alpha + self.cone_offset())
        }
    }

    fn width_at_height(&self, h: f64) -> f64 {
        let r_ball = self.ball_radius();
        let hc = self.h_contact();
        let r_max = self.shaft_radius();

        if h <= hc {
            // Ball region
            if h <= 0.0 {
                0.0
            } else if h >= r_ball {
                r_ball
            } else {
                (2.0 * r_ball * h - h * h).max(0.0).sqrt()
            }
        } else {
            // Cone region
            let w = (h - self.cone_offset()) * self.tan_alpha;
            w.min(r_max)
        }
    }

    // For the composite cutter, the facet_drop uses different parameters
    // depending on which region the CC falls in. We override facet_drop
    // to handle both regions.

    fn center_height(&self) -> f64 {
        // Ball region center height (used by default facet_drop)
        self.ball_radius()
    }
    fn normal_length(&self) -> f64 {
        self.ball_radius()
    }
    fn xy_normal_length(&self) -> f64 {
        0.0
    }

    fn facet_drop(&self, cl: &mut CLPoint, tri: &crate::geo::Triangle) -> bool {
        let n = &tri.normal;
        if n.z.abs() < 1e-12 {
            return false;
        }

        let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
        let r_ball = self.ball_radius();
        let r_shaft = self.shaft_radius();
        let rc = self.r_contact();

        let mut found = false;

        // Region 1: Ball contact (center_height=R_ball, normal_length=R_ball, xy_normal_length=0)
        {
            let cc_x = cl.x - r_ball * n.x;
            let cc_y = cl.y - r_ball * n.y;

            if tri.contains_point_xy(cc_x, cc_y)
                && let Some(cc_z) = tri.z_at_xy(cc_x, cc_y)
            {
                let rv_z = r_ball * n.z;
                let tip_z = cc_z + rv_z - r_ball;

                // Validate: CC must be in ball region (r <= r_contact from CL axis)
                let cc_dx = cc_x - cl.x;
                let cc_dy = cc_y - cl.y;
                let cc_r = (cc_dx * cc_dx + cc_dy * cc_dy).sqrt();
                if cc_r <= rc + 1e-8 {
                    cl.update_z(tip_z);
                    found = true;
                }
            }
        }

        // Region 2: Cone contact (center_height=cone_ch, normal_length=0, xy_normal_length=R_shaft)
        if nxy_len > 1e-15 {
            let xy_nx = n.x / nxy_len;
            let xy_ny = n.y / nxy_len;
            let cc_x = cl.x - r_shaft * xy_nx;
            let cc_y = cl.y - r_shaft * xy_ny;

            if tri.contains_point_xy(cc_x, cc_y)
                && let Some(cc_z) = tri.z_at_xy(cc_x, cc_y)
            {
                let cone_ch = r_shaft / self.tan_alpha + self.cone_offset();
                let tip_z = cc_z - cone_ch;

                // Validate: CC must be in cone region (r > r_contact from CL axis)
                let cc_dx = cc_x - cl.x;
                let cc_dy = cc_y - cl.y;
                let cc_r = (cc_dx * cc_dx + cc_dy * cc_dy).sqrt();
                if cc_r > rc - 1e-8 {
                    cl.update_z(tip_z);
                    found = true;
                }
            }
        }

        // Region 2 also: Tip contact on horizontal surfaces (cone tip is the ball tip)
        if tri.contains_point_xy(cl.x, cl.y)
            && let Some(cc_z) = tri.z_at_xy(cl.x, cl.y)
        {
            cl.update_z(cc_z);
            found = true;
        }

        found
    }

    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        let r_ball = self.ball_radius();
        let r_shaft = self.shaft_radius();
        let rc = self.r_contact();
        let alpha = self.alpha();
        let tan_a = alpha.tan();

        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dz = p2.z - p1.z;
        let edge_len_xy_sq = dx * dx + dy * dy;

        if edge_len_xy_sq < 1e-20 {
            return;
        }

        let edge_len_xy = edge_len_xy_sq.sqrt();
        let t_closest = ((cl.x - p1.x) * dx + (cl.y - p1.y) * dy) / edge_len_xy_sq;

        let px = p1.x + t_closest * dx;
        let py = p1.y + t_closest * dy;
        let d_sq = (cl.x - px) * (cl.x - px) + (cl.y - py) * (cl.y - py);

        if d_sq > r_shaft * r_shaft {
            return;
        }

        let d = d_sq.sqrt();
        let slope = dz / edge_len_xy;

        // === Ball region edge contact ===
        // Same as BallEndmill but only valid when contact radius <= r_contact
        if d < r_ball + 1e-10 {
            let s_sq = (r_ball * r_ball - d_sq).max(0.0);
            let s = s_sq.sqrt();

            let denom = (1.0 + slope * slope).sqrt();
            for sign in &[1.0, -1.0] {
                let sin_a = sign / denom;
                let cos_a = -sign * slope / denom;

                let dt = s * cos_a / edge_len_xy;
                let t = t_closest + dt;

                if !(-1e-8..=1.0 + 1e-8).contains(&t) {
                    continue;
                }

                // Validate contact is in ball region
                // Contact point on edge at parameter t, distance from CL axis
                let edge_x = p1.x + t * dx;
                let edge_y = p1.y + t * dy;
                let _rdx = edge_x - cl.x;
                let _rdy = edge_y - cl.y;
                let _contact_r_from_cl = (_rdx * _rdx + _rdy * _rdy).sqrt();

                // The CC point on the ball at this contact should be within r_contact
                // For ball: CC is at the point on the sphere closest to the edge
                // The XY distance from CL to CC ≈ d (perpendicular distance to edge)
                if d <= rc + 1e-8 {
                    let cc_z = p1.z + t * dz;
                    let tip_z = cc_z + s * sin_a - r_ball;
                    if sin_a >= -1e-10 {
                        cl.update_z(tip_z);
                    }
                }
            }
        }

        // === Cone region edge contact ===
        // Similar to VBit edge_drop but with cone_offset
        if d > rc - 1e-8 && d <= r_shaft + 1e-10 {
            let cone_ch = r_shaft / tan_a + self.cone_offset();
            let xu = (r_shaft * r_shaft - d_sq).max(0.0).sqrt();

            // Rim contact at shaft edge
            for &sign in &[1.0, -1.0] {
                let u = sign * xu;
                let dt = u / edge_len_xy;
                let t = t_closest + dt;
                if (-1e-8..=1.0 + 1e-8).contains(&t) {
                    let cc_z = p1.z + t * dz;
                    let tip_z = cc_z - cone_ch;
                    cl.update_z(tip_z);
                }
            }

            // Conical surface contact
            let l_sq = cone_ch * cone_ch;
            let denom_cone = l_sq - r_shaft * r_shaft * slope * slope;
            if denom_cone > 1e-15 {
                let ccu_sq = r_shaft * r_shaft * slope * slope * d_sq / denom_cone;
                let ccu = ccu_sq.max(0.0).sqrt();

                for &ccu_signed in &[ccu, -ccu] {
                    let dt = ccu_signed / edge_len_xy;
                    let t = t_closest + dt;
                    if (-1e-8..=1.0 + 1e-8).contains(&t) {
                        let cc_z = p1.z + t * dz;
                        let r_contact_edge = (ccu_signed * ccu_signed + d_sq).sqrt();
                        // Must be in cone region
                        if r_contact_edge >= rc - 1e-8 {
                            let h_at_r = r_contact_edge / tan_a + self.cone_offset();
                            let tip_z = cc_z - h_at_r;
                            cl.update_z(tip_z);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::{P3, Triangle};

    fn make_tool() -> TaperedBallEndmill {
        // 6mm ball tip, 10° taper, 12mm shaft
        TaperedBallEndmill::new(6.0, 10.0, 12.0, 30.0)
    }

    #[test]
    fn test_tapered_ball_construction() {
        let tool = make_tool();
        assert_eq!(tool.ball_diameter, 6.0);
        assert_eq!(tool.ball_radius(), 3.0);
        assert_eq!(tool.shaft_radius(), 6.0);
        assert!((tool.alpha() - 10.0_f64.to_radians()).abs() < 1e-10);
    }

    #[test]
    #[should_panic(expected = "Taper half-angle")]
    fn test_tapered_ball_invalid_angle() {
        TaperedBallEndmill::new(6.0, 0.0, 12.0, 30.0);
    }

    #[test]
    #[should_panic(expected = "Shaft diameter")]
    fn test_tapered_ball_shaft_too_small() {
        TaperedBallEndmill::new(6.0, 10.0, 4.0, 30.0);
    }

    #[test]
    fn test_tapered_ball_junction() {
        let tool = make_tool();
        let rc = tool.r_contact();
        let hc = tool.h_contact();

        // Junction should be where ball and cone meet tangentially
        assert!(rc > 0.0 && rc < tool.ball_radius());
        assert!(hc > 0.0 && hc < tool.ball_radius());

        // Both formulas should give the same height at r_contact
        let h_ball =
            tool.ball_radius() - (tool.ball_radius() * tool.ball_radius() - rc * rc).sqrt();
        let h_cone = rc / tool.alpha().tan() + tool.cone_offset();
        assert!(
            (h_ball - h_cone).abs() < 1e-10,
            "Ball height {} != cone height {} at r_contact",
            h_ball,
            h_cone
        );
        assert!(
            (hc - h_ball).abs() < 1e-10,
            "h_contact {} != h_ball {} at junction",
            hc,
            h_ball
        );
    }

    #[test]
    fn test_tapered_ball_profile_continuous() {
        let tool = make_tool();
        let rc = tool.r_contact();

        // Profile should be continuous across the junction
        // At exactly r_contact, both formulas give the same value.
        // With a small step, the difference should be small (proportional to curvature mismatch).
        let h_at = tool.height_at_radius(rc).unwrap();
        let h_below = tool.height_at_radius(rc - 0.0001).unwrap();
        let h_above = tool.height_at_radius(rc + 0.0001).unwrap();
        assert!(
            (h_at - h_below).abs() < 0.01,
            "Below junction: h_at={}, h_below={}",
            h_at,
            h_below
        );
        assert!(
            (h_at - h_above).abs() < 0.01,
            "Above junction: h_at={}, h_above={}",
            h_at,
            h_above
        );
    }

    #[test]
    fn test_tapered_ball_profile_at_center() {
        let tool = make_tool();
        assert!((tool.height_at_radius(0.0).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn test_tapered_ball_profile_at_shaft() {
        let tool = make_tool();
        let h = tool.height_at_radius(tool.shaft_radius()).unwrap();
        assert!(h > 0.0, "Height at shaft should be positive: {}", h);
        assert!(tool.height_at_radius(tool.shaft_radius() + 1.0).is_none());
    }

    #[test]
    fn test_tapered_ball_profile_monotonic() {
        let tool = make_tool();
        let r_max = tool.shaft_radius();
        let mut prev_h = 0.0;
        for i in 0..100 {
            let r = r_max * i as f64 / 99.0;
            let h = tool.height_at_radius(r).unwrap();
            assert!(
                h >= prev_h - 1e-10,
                "Profile not monotonic at r={}: h={} < prev_h={}",
                r,
                h,
                prev_h
            );
            prev_h = h;
        }
    }

    #[test]
    fn test_tapered_ball_width_at_height() {
        let tool = make_tool();
        // At h=0: width = 0 (ball tip)
        assert!(tool.width_at_height(0.0).abs() < 1e-10);
        // Width should increase with height
        let w1 = tool.width_at_height(1.0);
        let w2 = tool.width_at_height(2.0);
        assert!(w2 > w1, "Width should increase: w1={}, w2={}", w1, w2);
    }

    #[test]
    fn test_tapered_ball_vertex_drop_center() {
        let tool = make_tool();
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(0.0, 0.0, 10.0));
        assert!((cl.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_tapered_ball_vertex_drop_ball_region() {
        let tool = make_tool();
        let mut cl = CLPoint::new(0.0, 0.0);
        // Small radius (in ball region)
        tool.vertex_drop(&mut cl, &P3::new(1.0, 0.0, 5.0));
        let expected_h = tool.height_at_radius(1.0).unwrap();
        assert!(
            (cl.z - (5.0 - expected_h)).abs() < 1e-10,
            "cl.z={}, expected={}",
            cl.z,
            5.0 - expected_h
        );
    }

    #[test]
    fn test_tapered_ball_vertex_drop_cone_region() {
        let tool = make_tool();
        let mut cl = CLPoint::new(0.0, 0.0);
        // At shaft edge
        tool.vertex_drop(&mut cl, &P3::new(tool.shaft_radius(), 0.0, 0.0));
        let expected_h = tool.height_at_radius(tool.shaft_radius()).unwrap();
        assert!(
            (cl.z - (0.0 - expected_h)).abs() < 1e-10,
            "cl.z={}, expected={}",
            cl.z,
            0.0 - expected_h
        );
    }

    #[test]
    fn test_tapered_ball_facet_drop_horizontal() {
        let tool = make_tool();
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 5.0),
            P3::new(50.0, -50.0, 5.0),
            P3::new(0.0, 50.0, 5.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(hit);
        // Tip contact on horizontal: CL.z = surface_z = 5
        assert!((cl.z - 5.0).abs() < 1e-10, "cl.z = {}", cl.z);
    }

    #[test]
    fn test_tapered_ball_edge_drop_horizontal() {
        let tool = make_tool();
        // Edge within ball region
        let p1 = P3::new(1.0, -10.0, 0.0);
        let p2 = P3::new(1.0, 10.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert!(
            cl.z > f64::NEG_INFINITY,
            "Should find ball-region edge contact"
        );
    }

    #[test]
    fn test_tapered_ball_edge_out_of_range() {
        let tool = make_tool();
        let p1 = P3::new(20.0, -5.0, 0.0);
        let p2 = P3::new(20.0, 5.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert_eq!(cl.z, f64::NEG_INFINITY);
    }

    #[test]
    fn test_tapered_ball_full_drop_cutter() {
        let tool = make_tool();
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 3.0),
            P3::new(50.0, -50.0, 3.0),
            P3::new(0.0, 50.0, 3.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.drop_cutter(&mut cl, &tri);
        assert!((cl.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_tapered_ball_drop_on_hemisphere() {
        use crate::mesh::make_test_hemisphere;
        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 32);
        let tool = make_tool();

        let mut cl = CLPoint::new(0.0, 0.0);
        for face in &mesh.faces {
            tool.drop_cutter(&mut cl, face);
        }

        assert!(
            (cl.z - hemisphere_r).abs() < 1.0,
            "cl.z = {}, expected ~{}",
            cl.z,
            hemisphere_r
        );
    }

    #[test]
    fn test_tapered_ball_edge_drop_near_zero_discriminant_no_nan() {
        // When ccu_sq is near zero due to floating-point rounding,
        // the .max(0.0) guard should prevent NaN from sqrt.
        let tool = make_tool();
        let rc = tool.r_contact();

        // Edge in the cone region with near-zero slope — ccu_sq will be near zero
        let d = (rc + tool.shaft_radius()) / 2.0; // midway in cone region
        let p1 = P3::new(d, -5.0, 0.0);
        let p2 = P3::new(d, 5.0, 1e-15); // nearly zero slope
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert!(
            !cl.z.is_nan(),
            "edge_drop with near-zero slope should not produce NaN, got {}",
            cl.z
        );

        // Edge at exactly zero slope
        let p1 = P3::new(d, -5.0, 0.0);
        let p2 = P3::new(d, 5.0, 0.0);
        let mut cl2 = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl2, &p1, &p2);
        assert!(
            !cl2.z.is_nan(),
            "edge_drop with zero slope should not produce NaN, got {}",
            cl2.z
        );
    }

    #[test]
    fn test_tapered_ball_lower_than_ball_at_edge() {
        // A tapered ball should reach lower than a plain ball at the same XY offset
        // because its cone region extends further from the axis.
        use crate::tool::BallEndmill;

        let ball = BallEndmill::new(6.0, 30.0);
        let tapered = make_tool(); // 6mm ball, 10° taper, 12mm shaft

        // Vertex at shaft radius (6mm from axis) at z=0
        let v = P3::new(6.0, 0.0, 0.0);

        let mut cl_ball = CLPoint::new(0.0, 0.0);
        let mut cl_tapered = CLPoint::new(0.0, 0.0);

        ball.vertex_drop(&mut cl_ball, &v);
        tapered.vertex_drop(&mut cl_tapered, &v);

        // Ball can't reach (6 > ball_radius=3), so no contact
        assert_eq!(cl_ball.z, f64::NEG_INFINITY);
        // Tapered can reach (6 = shaft_radius)
        assert!(
            cl_tapered.z > f64::NEG_INFINITY,
            "Tapered ball should reach vertices at shaft radius"
        );
    }
}
