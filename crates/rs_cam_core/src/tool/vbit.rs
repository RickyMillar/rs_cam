//! V-bit / cone cutter implementation.
//!
//! Profile: height(r) = r / tan(alpha)  for r <= R
//! where alpha = half_angle (radians), R = diameter/2.
//!
//! The cone tip is at the tool tip (Z=0). The center height is at R/tan(alpha).
//!
//! Parameters: center_height = R/tan(alpha), normal_length = 0, xy_normal_length = R
//!
//! Edge contact uses the hyperbola intersection approach from OpenCAMLib.

use super::{CLPoint, MillingCutter};
use crate::geo::P3;

#[derive(Debug, Clone)]
pub struct VBitEndmill {
    pub diameter: f64,
    /// Full included angle in degrees (e.g., 90° V-bit has half_angle = 45°)
    pub included_angle_deg: f64,
    pub cutting_length: f64,
    // Precomputed trig values
    half_angle_rad: f64,
    tan_half_angle: f64,
}

impl VBitEndmill {
    pub fn new(diameter: f64, included_angle_deg: f64, cutting_length: f64) -> Self {
        assert!(
            included_angle_deg > 0.0 && included_angle_deg < 180.0,
            "Included angle must be between 0 and 180 degrees, got {}",
            included_angle_deg
        );
        let half_angle_rad = (included_angle_deg / 2.0).to_radians();
        let tan_half_angle = half_angle_rad.tan();
        Self {
            diameter,
            included_angle_deg,
            cutting_length,
            half_angle_rad,
            tan_half_angle,
        }
    }

    /// Half-angle in radians.
    fn half_angle(&self) -> f64 {
        self.half_angle_rad
    }

    /// Height of the cone at the full radius (where cone meets shaft).
    fn cone_height(&self) -> f64 {
        self.radius() / self.tan_half_angle
    }
}

impl MillingCutter for VBitEndmill {
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
            Some(r.min(big_r) / self.tan_half_angle)
        }
    }

    fn width_at_height(&self, h: f64) -> f64 {
        let big_r = self.radius();
        let w = h * self.tan_half_angle;
        w.min(big_r)
    }

    fn center_height(&self) -> f64 {
        self.cone_height()
    }
    fn normal_length(&self) -> f64 {
        0.0
    }
    fn xy_normal_length(&self) -> f64 {
        self.radius()
    }

    fn facet_drop(&self, cl: &mut CLPoint, tri: &crate::geo::Triangle) -> bool {
        let n = &tri.normal;
        if n.z.abs() < 1e-12 {
            return false;
        }

        let _alpha = self.half_angle();
        let big_r = self.radius();
        let nxy_len = (n.x * n.x + n.y * n.y).sqrt();

        // Two contact modes for cone:
        // Mode 1: Tip contact (the point touches the facet)
        // Mode 2: Conical surface contact (side of cone touches facet)

        let mut best_z = f64::NEG_INFINITY;
        let mut found = false;

        // Mode 1: Tip contact — CC at (cl.x, cl.y) projected onto triangle
        if tri.contains_point_xy(cl.x, cl.y)
            && let Some(cc_z) = tri.z_at_xy(cl.x, cl.y)
            && cc_z > best_z
        {
            best_z = cc_z;
            found = true;
        }

        // Mode 2: Conical surface contact
        // The cone contacts when the surface normal angle matches the cone angle.
        // Surface slope angle from horizontal = atan(nxy_len / n.z)
        // Cone half-angle from axis = alpha
        // Contact occurs when the slope angle complement matches alpha.
        //
        // Using the standard radiusvector formula with normal_length=0, xy_normal_length=R:
        // CC = CL - R * xyNormal
        if nxy_len > 1e-15 {
            let xy_nx = n.x / nxy_len;
            let xy_ny = n.y / nxy_len;
            let cc_x = cl.x - big_r * xy_nx;
            let cc_y = cl.y - big_r * xy_ny;

            if tri.contains_point_xy(cc_x, cc_y)
                && let Some(cc_z) = tri.z_at_xy(cc_x, cc_y)
            {
                // tip_z = cc_z + rv_z - center_height
                // rv_z = normal_length * n.z = 0
                let tip_z = cc_z - self.center_height();
                if tip_z > best_z {
                    best_z = tip_z;
                    found = true;
                }
            }
        }

        if found {
            cl.update_z(best_z);
        }
        found
    }

    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        let alpha = self.half_angle();
        let tan_a = alpha.tan();
        let big_r = self.radius();
        let ch = self.cone_height(); // R / tan(alpha)

        // Edge vector
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dz = p2.z - p1.z;
        let edge_len_xy_sq = dx * dx + dy * dy;

        if edge_len_xy_sq < 1e-20 {
            return; // vertical edge
        }

        let edge_len_xy = edge_len_xy_sq.sqrt();

        // Parameter t for closest XY approach
        let t_closest = ((cl.x - p1.x) * dx + (cl.y - p1.y) * dy) / edge_len_xy_sq;

        // Perpendicular XY distance
        let px = p1.x + t_closest * dx;
        let py = p1.y + t_closest * dy;
        let d_sq = (cl.x - px) * (cl.x - px) + (cl.y - py) * (cl.y - py);

        if d_sq > big_r * big_r {
            return;
        }

        let _d = d_sq.sqrt();

        // Edge slope
        let slope = dz / edge_len_xy; // dz per unit XY distance along edge

        // Outermost cutter point at distance d from axis
        let xu = (big_r * big_r - d_sq).max(0.0).sqrt();

        // The cone profile at distance d from axis:
        // At XY distance d, the cone surface ranges from the tip (at height 0)
        // up along the slope. The cutter width at distance d from axis at
        // offset u along the edge is: sqrt(u^2 + d^2) from axis.
        // The cone height at that radius = sqrt(u^2 + d^2) / tan(alpha)
        //
        // The edge at offset u: z_edge = z_closest + (u/edge_len_xy) * dz
        //   where u is measured along the edge from the closest point.
        //
        // Contact where: cone_height(sqrt(u^2 + d^2)) = z_edge - tip_z
        // => sqrt(u^2 + d^2) / tan(alpha) = z_closest + u*slope - tip_z
        //
        // This is a hyperbola intersection problem.

        // Maximum slope of the cone cross-section at distance d:
        // mu = (ch/R) * xu / sqrt(xu^2 + d^2)  (simplified)
        // Actually: d(cone_height)/du = u / (tan(alpha) * sqrt(u^2 + d^2))
        // At u = xu: mu = xu / (tan(alpha) * sqrt(xu^2 + d^2)) = xu / (tan(alpha) * R)
        let mu = if big_r > 1e-10 {
            xu / (tan_a * big_r)
        } else {
            return;
        };

        // Case 1: |slope| <= mu — contact on the conical surface
        // Case 2: |slope| > mu — contact at the cone rim (circular edge at R)

        if slope.abs() <= mu + 1e-10 {
            // Contact on conical surface
            // Solve: sqrt(u^2 + d^2) / tan(alpha) + tip_z = z_closest + u * slope
            // Let L = 1/tan(alpha), rearrange:
            // L * sqrt(u^2 + d^2) = z_closest + u * slope - tip_z
            // Square both sides:
            // L^2 * (u^2 + d^2) = (z_closest + u*slope - tip_z)^2
            //
            // This is quadratic in u if we substitute tip_z.
            // Instead, use the parameterization from OpenCAMLib:
            // ccu = sign(slope) * sqrt(R^2 * slope^2 * d^2 / (L^2 - R^2 * slope^2))
            // where L = ch = R/tan(alpha), so L^2 = R^2/tan^2(alpha)

            let l_sq = ch * ch; // (R/tan(alpha))^2
            let denom = l_sq - big_r * big_r * slope * slope;
            if denom.abs() < 1e-15 {
                // Degenerate: slope matches cone angle exactly
                // Fall through to rim contact
            } else if denom > 0.0 {
                let ccu_sq = big_r * big_r * slope * slope * d_sq / denom;
                let ccu = ccu_sq.max(0.0).sqrt();
                let ccu_signed = if slope >= 0.0 { ccu } else { -ccu };

                // Parameter along edge
                let dt = ccu_signed / edge_len_xy;
                let t = t_closest + dt;

                if (-1e-8..=1.0 + 1e-8).contains(&t) {
                    let cc_z = p1.z + t * dz;
                    // Distance from axis at contact
                    let r_contact = (ccu_signed * ccu_signed + d_sq).sqrt();
                    // Cone height at contact
                    let h_contact = r_contact / tan_a;
                    let tip_z = cc_z - h_contact;
                    cl.update_z(tip_z);
                }

                // Also try the negative solution (opposite direction)
                if slope.abs() > 1e-10 {
                    let ccu_neg = -ccu_signed;
                    let dt = ccu_neg / edge_len_xy;
                    let t = t_closest + dt;
                    if (-1e-8..=1.0 + 1e-8).contains(&t) {
                        let cc_z = p1.z + t * dz;
                        let r_contact = (ccu_neg * ccu_neg + d_sq).sqrt();
                        let h_contact = r_contact / tan_a;
                        let tip_z = cc_z - h_contact;
                        cl.update_z(tip_z);
                    }
                }
            }
        }

        // Case 2: Rim contact (at the circular edge where cone meets shaft)
        // Contact at u = ±xu (outermost point of cutter at distance d)
        for &sign in &[1.0, -1.0] {
            let u = sign * xu;
            let dt = u / edge_len_xy;
            let t = t_closest + dt;
            if (-1e-8..=1.0 + 1e-8).contains(&t) {
                let cc_z = p1.z + t * dz;
                let tip_z = cc_z - ch;
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
    fn test_vbit_construction() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        assert_eq!(tool.diameter, 10.0);
        assert_eq!(tool.included_angle_deg, 90.0);
        assert!((tool.half_angle() - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
        assert_eq!(tool.radius(), 5.0);
    }

    #[test]
    #[should_panic(expected = "Included angle")]
    fn test_vbit_invalid_angle() {
        VBitEndmill::new(10.0, 0.0, 25.0);
    }

    #[test]
    fn test_vbit_profile_90deg() {
        // 90° V-bit: half_angle = 45°, tan(45°) = 1
        // height(r) = r / tan(45°) = r
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        assert!((tool.height_at_radius(0.0).unwrap()).abs() < 1e-10);
        assert!((tool.height_at_radius(1.0).unwrap() - 1.0).abs() < 1e-10);
        assert!((tool.height_at_radius(5.0).unwrap() - 5.0).abs() < 1e-10);
        assert!(tool.height_at_radius(5.5).is_none());
    }

    #[test]
    fn test_vbit_profile_60deg() {
        // 60° V-bit: half_angle = 30°, tan(30°) = 1/sqrt(3)
        // height(r) = r * sqrt(3)
        let tool = VBitEndmill::new(10.0, 60.0, 25.0);
        let expected = 3.0 * 3.0_f64.sqrt(); // r=3, h = 3*sqrt(3) ≈ 5.196
        let h = tool.height_at_radius(3.0).unwrap();
        assert!(
            (h - expected).abs() < 1e-10,
            "h={}, expected={}",
            h,
            expected
        );
    }

    #[test]
    fn test_vbit_width_at_height() {
        // 90° V-bit: width(h) = h * tan(45°) = h
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        assert!((tool.width_at_height(0.0)).abs() < 1e-10);
        assert!((tool.width_at_height(3.0) - 3.0).abs() < 1e-10);
        // Above cone height: clamped to R
        assert!((tool.width_at_height(10.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vbit_parameters_90deg() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        // center_height = R / tan(45°) = 5
        assert!((tool.center_height() - 5.0).abs() < 1e-10);
        assert_eq!(tool.normal_length(), 0.0);
        assert!((tool.xy_normal_length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vbit_vertex_drop_center() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.vertex_drop(&mut cl, &P3::new(0.0, 0.0, 10.0));
        // height(0) = 0, CL.z = 10
        assert!((cl.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_vbit_vertex_drop_edge() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        // Vertex at r=5 (edge), z=0
        tool.vertex_drop(&mut cl, &P3::new(5.0, 0.0, 0.0));
        // height(5) = 5 (for 90° V-bit), CL.z = 0 - 5 = -5
        assert!((cl.z - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_vbit_facet_drop_horizontal() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 5.0),
            P3::new(50.0, -50.0, 5.0),
            P3::new(0.0, 50.0, 5.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        assert!(hit);
        // Tip contact on horizontal surface: CL.z = surface_z = 5
        assert!((cl.z - 5.0).abs() < 1e-10, "cl.z = {}", cl.z);
    }

    #[test]
    fn test_vbit_facet_drop_sloped() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        // 45-degree slope
        let tri = Triangle::new(
            P3::new(-20.0, -50.0, -20.0),
            P3::new(50.0, -50.0, 50.0),
            P3::new(-20.0, 50.0, -20.0),
        );
        let mut cl = CLPoint::new(10.0, 0.0);
        let hit = tool.facet_drop(&mut cl, &tri);
        if hit {
            assert!(cl.z > f64::NEG_INFINITY);
        }
    }

    #[test]
    fn test_vbit_edge_drop_horizontal() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        // Horizontal edge along Y at x=3, z=0
        let p1 = P3::new(3.0, -10.0, 0.0);
        let p2 = P3::new(3.0, 10.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        // Rim contact: u = xu = sqrt(25-9) = 4, tip_z = 0 - 5 = -5
        // Cone contact at d=3: ccu_sq = R^2*slope^2*d^2/(L^2-R^2*slope^2) with slope=0
        //   ccu_sq = 0 → contact at t_closest, r=d=3, h=3, tip_z = 0-3 = -3
        // Best is -3 (higher)
        assert!(
            (cl.z - (-3.0)).abs() < 1e-10,
            "cl.z = {}, expected -3.0",
            cl.z
        );
    }

    #[test]
    fn test_vbit_edge_out_of_range() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let p1 = P3::new(10.0, -5.0, 0.0);
        let p2 = P3::new(10.0, 5.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert_eq!(cl.z, f64::NEG_INFINITY);
    }

    #[test]
    fn test_vbit_edge_drop_sloped() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        // Sloped edge from (0,-5,0) to (0,5,10)
        let p1 = P3::new(0.0, -5.0, 0.0);
        let p2 = P3::new(0.0, 5.0, 10.0);
        let mut cl = CLPoint::new(3.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert!(
            cl.z > f64::NEG_INFINITY,
            "Should find contact on sloped edge"
        );
    }

    #[test]
    fn test_vbit_full_drop_cutter() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let tri = Triangle::new(
            P3::new(-50.0, -50.0, 3.0),
            P3::new(50.0, -50.0, 3.0),
            P3::new(0.0, 50.0, 3.0),
        );
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.drop_cutter(&mut cl, &tri);
        // Tip touches horizontal surface at z=3
        assert!((cl.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_vbit_drop_on_hemisphere() {
        use crate::mesh::make_test_hemisphere;
        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 32);
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);

        let mut cl = CLPoint::new(0.0, 0.0);
        for face in &mesh.faces {
            tool.drop_cutter(&mut cl, face);
        }

        // V-bit tip at apex: vertex_drop gives z = hemisphere_r
        assert!(
            (cl.z - hemisphere_r).abs() < 1.0,
            "cl.z = {}, expected ~{}",
            cl.z,
            hemisphere_r
        );
    }

    #[test]
    fn test_vbit_edge_drop_near_zero_ccu_sq_no_nan() {
        // When the edge is nearly parallel to the cone slope, ccu_sq can be
        // very small or slightly negative due to floating-point rounding.
        // The .max(0.0) guard should prevent NaN from sqrt.
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);

        // Horizontal edge at exactly d=0 (on the tool axis) — slope=0 produces ccu_sq=0
        let p1 = P3::new(0.0, -5.0, 0.0);
        let p2 = P3::new(0.0, 5.0, 0.0);
        let mut cl = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl, &p1, &p2);
        assert!(
            !cl.z.is_nan(),
            "edge_drop should not produce NaN, got {}",
            cl.z
        );

        // Edge very close to the tool axis with a tiny slope
        let p1 = P3::new(0.001, -5.0, 0.0);
        let p2 = P3::new(0.001, 5.0, 1e-15);
        let mut cl2 = CLPoint::new(0.0, 0.0);
        tool.edge_drop(&mut cl2, &p1, &p2);
        assert!(
            !cl2.z.is_nan(),
            "edge_drop with near-zero slope should not produce NaN, got {}",
            cl2.z
        );
    }

    #[test]
    fn test_vbit_steeper_profile() {
        // 60° V-bit has steeper sides than 90°
        let tool_90 = VBitEndmill::new(10.0, 90.0, 25.0);
        let tool_60 = VBitEndmill::new(10.0, 60.0, 25.0);

        // At same radius, 60° V-bit is taller
        let h_90 = tool_90.height_at_radius(3.0).unwrap();
        let h_60 = tool_60.height_at_radius(3.0).unwrap();
        assert!(
            h_60 > h_90,
            "60° V-bit should be taller: h_60={}, h_90={}",
            h_60,
            h_90
        );
    }
}
