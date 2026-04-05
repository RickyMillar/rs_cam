//! Milling cutter definitions.
//!
//! Every tool implements `MillingCutter`, providing:
//! - Profile functions: `height_at_radius(r)` and `width_at_height(h)`
//! - Drop-cutter contact: `vertex_drop`, `facet_drop`, `edge_drop`
//!
//! Reference: research/03_tool_geometry.md and research/raw_opencamlib_math.md

mod ball;
mod bullnose;
mod flat;
mod tapered_ball;
mod vbit;

pub use ball::BallEndmill;
pub use bullnose::BullNoseEndmill;
pub use flat::FlatEndmill;
pub use tapered_ball::TaperedBallEndmill;
pub use vbit::VBitEndmill;
// Re-export ToolDefinition (defined below the trait in this file)

use crate::geo::{P3, Triangle};

/// Contact point from a drop-cutter test.
#[derive(Debug, Clone, Copy)]
pub struct CLPoint {
    /// Cutter-location position (tool tip)
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// True if at least one triangle contributed to this CL point's Z value.
    /// When false, the point is outside the mesh footprint and Z is NEG_INFINITY
    /// (or clamped by the caller).
    pub contacted: bool,
}

impl CLPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            z: f64::NEG_INFINITY,
            contacted: false,
        }
    }

    #[inline]
    pub fn update_z(&mut self, z: f64) {
        if z > self.z {
            self.z = z;
            self.contacted = true;
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
    fn radius(&self) -> f64 {
        self.diameter() / 2.0
    }
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
        let Some(cc_z) = tri.z_at_xy(cc_x, cc_y) else {
            return false;
        };

        // Compute the radiusvector Z component
        let rv_z = r2 * n.z;

        // CL.z = CC.z + rv_z - center_height
        let tip_z = cc_z + rv_z - self.center_height();

        cl.update_z(tip_z);
        true
    }

    /// Sample the cutter profile as (radius, height) pairs from center to edge.
    ///
    /// Returns `n + 1` points at evenly-spaced radii from 0 to `self.radius()`.
    /// Useful for rendering and visualization without hand-rolling per-shape geometry.
    fn profile_points(&self, n: usize) -> Vec<(f64, f64)> {
        let r = self.radius();
        (0..=n)
            .map(|i| {
                let dist = (i as f64 / n.max(1) as f64) * r;
                (dist, self.height_at_radius(dist).unwrap_or(0.0))
            })
            .collect()
    }

    /// Geometry classification for feeds/speeds effective-diameter calculation.
    ///
    /// Each tool type should override to return its specific hint variant.
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        crate::feeds::ToolGeometryHint::Flat
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

/// A complete tool definition: cutting geometry + assembly dimensions.
///
/// Wraps a `Box<dyn MillingCutter>` and adds shank/holder dimensions so that
/// collision detection, feeds calculation, and rendering can all derive their
/// inputs from a single source of truth.
///
/// Implements `MillingCutter` by delegating to the inner cutter.
pub struct ToolDefinition {
    cutter: Box<dyn MillingCutter>,
    /// Shank diameter above the cutting flutes (mm).
    pub shank_diameter: f64,
    /// Shank length above the cutting flutes (mm).
    pub shank_length: f64,
    /// Holder / collet diameter (mm).
    pub holder_diameter: f64,
    /// Total stickout from holder face to cutter tip (mm).
    pub stickout: f64,
    /// Number of cutting flutes.
    pub flute_count: u32,
}

impl ToolDefinition {
    pub fn new(
        cutter: Box<dyn MillingCutter>,
        shank_diameter: f64,
        shank_length: f64,
        holder_diameter: f64,
        stickout: f64,
        flute_count: u32,
    ) -> Self {
        Self {
            cutter,
            shank_diameter,
            shank_length,
            holder_diameter,
            stickout,
            flute_count,
        }
    }

    /// Computed holder length from stickout minus cutting length and shank.
    pub fn holder_length(&self) -> f64 {
        (self.stickout - self.cutter.length() - self.shank_length).max(0.0)
    }

    /// Build a `ToolAssembly` for collision detection.
    ///
    /// Uses `self.cutter.radius()` for the cutter envelope, which for
    /// `TaperedBallEndmill` correctly returns `shaft_diameter / 2` (the maximum
    /// cutting radius), not the ball tip radius.
    pub fn to_assembly(&self) -> crate::collision::ToolAssembly {
        crate::collision::ToolAssembly {
            cutter_radius: self.cutter.radius(),
            cutter_length: self.cutter.length(),
            shank_diameter: self.shank_diameter,
            shank_length: self.shank_length,
            holder_diameter: self.holder_diameter,
            holder_length: self.holder_length(),
        }
    }

    /// Derive the feeds geometry hint from the inner cutter.
    pub fn to_geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        self.cutter.geometry_hint()
    }
}

impl MillingCutter for ToolDefinition {
    fn diameter(&self) -> f64 {
        self.cutter.diameter()
    }
    fn length(&self) -> f64 {
        self.cutter.length()
    }
    fn height_at_radius(&self, r: f64) -> Option<f64> {
        self.cutter.height_at_radius(r)
    }
    fn width_at_height(&self, h: f64) -> f64 {
        self.cutter.width_at_height(h)
    }
    fn center_height(&self) -> f64 {
        self.cutter.center_height()
    }
    fn normal_length(&self) -> f64 {
        self.cutter.normal_length()
    }
    fn xy_normal_length(&self) -> f64 {
        self.cutter.xy_normal_length()
    }
    fn profile_points(&self, n: usize) -> Vec<(f64, f64)> {
        self.cutter.profile_points(n)
    }
    fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        self.cutter.geometry_hint()
    }
    fn edge_drop(&self, cl: &mut CLPoint, p1: &P3, p2: &P3) {
        self.cutter.edge_drop(cl, p1, p2);
    }
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle) -> bool {
        self.cutter.facet_drop(cl, tri)
    }
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &P3) {
        self.cutter.vertex_drop(cl, vertex);
    }
    fn drop_cutter(&self, cl: &mut CLPoint, tri: &Triangle) {
        self.cutter.drop_cutter(cl, tri);
    }
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

    #[test]
    fn test_profile_points_flat() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let pts = tool.profile_points(10);
        assert_eq!(pts.len(), 11);
        // First point is center
        assert!((pts[0].0).abs() < 1e-10);
        assert!((pts[0].1).abs() < 1e-10);
        // Last point is at tool radius
        assert!((pts[10].0 - 5.0).abs() < 1e-10);
        // All heights should be 0 for flat endmill
        for &(_r, h) in &pts {
            assert!((h).abs() < 1e-10, "flat endmill profile should be 0");
        }
    }

    #[test]
    fn test_profile_points_ball() {
        let tool = BallEndmill::new(10.0, 25.0);
        let pts = tool.profile_points(10);
        assert_eq!(pts.len(), 11);
        // Center height = 0
        assert!((pts[0].1).abs() < 1e-10);
        // Heights should increase monotonically
        for i in 1..pts.len() {
            assert!(pts[i].1 >= pts[i - 1].1 - 1e-10);
        }
        // At full radius, height = R (top of hemisphere)
        let last = pts[10];
        assert!((last.0 - 5.0).abs() < 1e-10);
        assert!((last.1 - 5.0).abs() < 1e-8);
    }

    #[test]
    fn test_profile_points_tapered_ball() {
        let tool = TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0);
        let pts = tool.profile_points(50);
        assert_eq!(pts.len(), 51);
        // Center height = 0
        assert!((pts[0].1).abs() < 1e-10);
        // Heights should increase monotonically (no dips at junction)
        for i in 1..pts.len() {
            assert!(
                pts[i].1 >= pts[i - 1].1 - 1e-10,
                "profile not monotonic at i={}: h[{}]={} > h[{}]={}",
                i,
                i - 1,
                pts[i - 1].1,
                i,
                pts[i].1
            );
        }
    }

    #[test]
    fn test_geometry_hint_flat() {
        let tool = FlatEndmill::new(10.0, 25.0);
        assert_eq!(tool.geometry_hint(), crate::feeds::ToolGeometryHint::Flat);
    }

    #[test]
    fn test_geometry_hint_ball() {
        let tool = BallEndmill::new(10.0, 25.0);
        assert_eq!(tool.geometry_hint(), crate::feeds::ToolGeometryHint::Ball);
    }

    #[test]
    fn test_geometry_hint_bullnose() {
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::Bull { corner_radius } => {
                assert!((corner_radius - 2.0).abs() < 1e-10);
            }
            _ => panic!("expected Bull hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_geometry_hint_vbit() {
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::VBit {
                included_angle,
                tip_diameter,
            } => {
                assert!((included_angle - 90.0).abs() < 1e-10);
                assert!((tip_diameter).abs() < 1e-10); // pointed
            }
            _ => panic!("expected VBit hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_geometry_hint_tapered_ball() {
        let tool = TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0);
        let hint = tool.geometry_hint();
        match hint {
            crate::feeds::ToolGeometryHint::TaperedBall {
                tip_radius,
                taper_angle_deg,
            } => {
                assert!((tip_radius - 3.175 / 2.0).abs() < 1e-10);
                assert!((taper_angle_deg - 15.0).abs() < 1e-10);
            }
            _ => panic!("expected TaperedBall hint, got {:?}", hint),
        }
    }

    #[test]
    fn test_tool_definition_delegates() {
        let cutter = Box::new(BallEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 45.0, 2);
        // Trait methods delegate correctly
        assert!((td.diameter() - 10.0).abs() < 1e-10);
        assert!((td.radius() - 5.0).abs() < 1e-10);
        assert!((td.length() - 25.0).abs() < 1e-10);
        assert!((td.center_height() - 5.0).abs() < 1e-10);
        // Profile sampling delegates
        let pts = td.profile_points(4);
        assert_eq!(pts.len(), 5);
    }

    #[test]
    fn test_tool_definition_assembly_flat() {
        let cutter = Box::new(FlatEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2);
        let asm = td.to_assembly();
        assert!((asm.cutter_radius - 5.0).abs() < 1e-10);
        assert!((asm.cutter_length - 25.0).abs() < 1e-10);
        assert!((asm.shank_diameter - 6.35).abs() < 1e-10);
        assert!((asm.shank_length - 20.0).abs() < 1e-10);
        assert!((asm.holder_diameter - 25.0).abs() < 1e-10);
        assert!((asm.holder_length - 5.0).abs() < 1e-10); // 50 - 25 - 20
    }

    #[test]
    fn test_tool_definition_assembly_tapered_ball_uses_shaft_radius() {
        // This is the bug regression: collision must use shaft_radius, not ball_radius
        let cutter = Box::new(TaperedBallEndmill::new(3.175, 15.0, 6.35, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2);
        let asm = td.to_assembly();
        // cutter_radius must be shaft_diameter/2 = 3.175, NOT ball_diameter/2 = 1.5875
        assert!(
            (asm.cutter_radius - 6.35 / 2.0).abs() < 1e-10,
            "expected shaft_radius {}, got {}",
            6.35 / 2.0,
            asm.cutter_radius
        );
    }

    #[test]
    fn test_tool_definition_geometry_hint() {
        let cutter = Box::new(BullNoseEndmill::new(10.0, 2.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2);
        match td.to_geometry_hint() {
            crate::feeds::ToolGeometryHint::Bull { corner_radius } => {
                assert!((corner_radius - 2.0).abs() < 1e-10);
            }
            other => panic!("expected Bull, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_definition_holder_length() {
        let cutter = Box::new(FlatEndmill::new(10.0, 25.0));
        let td = ToolDefinition::new(cutter, 6.35, 20.0, 25.0, 50.0, 2);
        assert!((td.holder_length() - 5.0).abs() < 1e-10);
        // Clamps to 0 if stickout is short
        let td2 = ToolDefinition::new(
            Box::new(FlatEndmill::new(10.0, 25.0)),
            6.35,
            20.0,
            25.0,
            30.0, // 30 - 25 - 20 = -15 -> clamped to 0
            2,
        );
        assert!((td2.holder_length()).abs() < 1e-10);
    }
}
