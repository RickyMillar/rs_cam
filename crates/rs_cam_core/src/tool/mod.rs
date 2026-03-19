//! Milling cutter definitions.
//!
//! Every tool implements `MillingCutter`, providing:
//! - Profile functions: `height_at_radius(r)` and `width_at_height(h)`
//! - Drop-cutter contact: `vertex_drop`, `facet_drop`, `edge_drop`
//!
//! Reference: research/03_tool_geometry.md and research/raw_opencamlib_math.md

mod flat;
mod ball;
mod bullnose;
mod vbit;
mod tapered_ball;

pub use flat::FlatEndmill;
pub use ball::BallEndmill;
pub use bullnose::BullNoseEndmill;
pub use vbit::VBitEndmill;
pub use tapered_ball::TaperedBallEndmill;

use crate::geo::{P3, Triangle};

/// Contact point from a drop-cutter test.
#[derive(Debug, Clone, Copy)]
pub struct CLPoint {
    /// Cutter-location position (tool tip)
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CLPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            z: f64::NEG_INFINITY,
        }
    }

    #[inline]
    pub fn update_z(&mut self, z: f64) {
        if z > self.z {
            self.z = z;
        }
    }

    pub fn position(&self) -> P3 {
        P3::new(self.x, self.y, self.z)
    }
}

/// The core trait for all milling cutter types.
///
/// Follows OpenCAMLib's template-method pattern: the drop-cutter algorithm
/// calls vertex_drop/facet_drop/edge_drop, and each cutter type implements
/// them according to its geometry.
pub trait MillingCutter: Send + Sync {
    fn diameter(&self) -> f64;
    fn radius(&self) -> f64 { self.diameter() / 2.0 }
    fn length(&self) -> f64;

    /// Profile height at radial distance r from tool axis.
    /// Returns the Z offset from the tool tip to the cutter surface at radius r.
    fn height_at_radius(&self, r: f64) -> Option<f64>;

    /// Profile radius at height h above tool tip.
    fn width_at_height(&self, h: f64) -> f64;

    /// Key parameters for the generalized facet contact formula:
    /// radiusvector = xy_normal_length * xyNormal + normal_length * surfaceNormal
    fn center_height(&self) -> f64;
    fn normal_length(&self) -> f64;
    fn xy_normal_length(&self) -> f64;

    /// Test contact with a triangle vertex. Updates cl.z if this gives a higher position.
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &P3) {
        let dx = vertex.x - cl.x;
        let dy = vertex.y - cl.y;
        let q = (dx * dx + dy * dy).sqrt();
        if let Some(h) = self.height_at_radius(q) {
            cl.update_z(vertex.z - h);
        }
    }

    /// Test contact with a triangle facet. Updates cl.z if contact found.
    /// Returns true if contact was on the facet (inside the triangle).
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle) -> bool {
        let n = &tri.normal;
        // Skip nearly-vertical triangles
        if n.z.abs() < 1e-12 {
            return false;
        }

        // Compute the XY-normalized normal for the radius vector
        let nxy_len = (n.x * n.x + n.y * n.y).sqrt();
        let (xy_nx, xy_ny) = if nxy_len > 1e-15 {
            (n.x / nxy_len, n.y / nxy_len)
        } else {
            (0.0, 0.0)
        };

        // CC = CL - radiusvector (XY only)
        let r1 = self.xy_normal_length();
        let r2 = self.normal_length();
        let cc_x = cl.x - r1 * xy_nx - r2 * n.x;
        let cc_y = cl.y - r1 * xy_ny - r2 * n.y;

        // Check if CC is inside the triangle
        if !tri.contains_point_xy(cc_x, cc_y) {
            return false;
        }

        // Compute CC.z on the triangle plane
        let cc_z = match tri.z_at_xy(cc_x, cc_y) {
            Some(z) => z,
            None => return false,
        };

        // Compute the radiusvector Z component
        let rv_z = r2 * n.z;

        // CL.z = CC.z + rv_z - center_height
        let tip_z = cc_z + rv_z - self.center_height();

        cl.update_z(tip_z);
        true
    }

    /// Test contact with a triangle edge. Updates cl.z if contact found.
    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3);

    /// Run the full drop-cutter test against a single triangle.
    fn drop_cutter(&self, cl: &mut CLPoint, tri: &Triangle) {
        // Facet test first (if hit, edge/vertex are redundant per OpenCAMLib)
        if self.facet_drop(cl, tri) {
            return;
        }

        // Vertex tests
        for v in &tri.v {
            self.vertex_drop(cl, v);
        }

        // Edge tests
        self.edge_drop(cl, &tri.v[0], &tri.v[1]);
        self.edge_drop(cl, &tri.v[1], &tri.v[2]);
        self.edge_drop(cl, &tri.v[2], &tri.v[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cl_point() {
        let mut cl = CLPoint::new(5.0, 3.0);
        assert_eq!(cl.z, f64::NEG_INFINITY);
        cl.update_z(10.0);
        assert_eq!(cl.z, 10.0);
        cl.update_z(5.0); // lower, should not update
        assert_eq!(cl.z, 10.0);
        cl.update_z(15.0);
        assert_eq!(cl.z, 15.0);
    }
}
