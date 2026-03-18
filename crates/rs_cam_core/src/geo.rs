//! Geometry primitives and type aliases.

pub use nalgebra::{Point2, Point3, Vector2, Vector3};

/// 2D point alias
pub type P2 = Point2<f64>;
/// 3D point alias
pub type P3 = Point3<f64>;
/// 2D vector alias
pub type V2 = Vector2<f64>;
/// 3D vector alias
pub type V3 = Vector3<f64>;

/// Axis-aligned bounding box in 3D.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox3 {
    pub min: P3,
    pub max: P3,
}

impl BoundingBox3 {
    pub fn empty() -> Self {
        Self {
            min: P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    pub fn from_points(points: impl IntoIterator<Item = P3>) -> Self {
        let mut bb = Self::empty();
        for p in points {
            bb.expand_to(p);
        }
        bb
    }

    pub fn expand_to(&mut self, p: P3) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }

    pub fn expand_by(&self, margin: f64) -> Self {
        Self {
            min: P3::new(self.min.x - margin, self.min.y - margin, self.min.z - margin),
            max: P3::new(self.max.x + margin, self.max.y + margin, self.max.z + margin),
        }
    }

    pub fn center(&self) -> P3 {
        P3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    pub fn overlaps_xy(&self, other: &BoundingBox3) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

/// A triangle in 3D space with precomputed normal.
#[derive(Debug, Clone)]
pub struct Triangle {
    pub v: [P3; 3],
    pub normal: V3,
    pub bbox: BoundingBox3,
}

impl Triangle {
    pub fn new(v0: P3, v1: P3, v2: P3) -> Self {
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let normal = e1.cross(&e2);
        let len = normal.norm();
        let normal = if len > 1e-15 {
            normal / len
        } else {
            V3::new(0.0, 0.0, 1.0)
        };
        let bbox = BoundingBox3::from_points([v0, v1, v2]);
        Self {
            v: [v0, v1, v2],
            normal,
            bbox,
        }
    }

    /// Check if point (x,y) projected onto the triangle plane lands inside the triangle.
    /// Uses barycentric coordinates.
    pub fn contains_point_xy(&self, px: f64, py: f64) -> bool {
        let (x0, y0) = (self.v[0].x, self.v[0].y);
        let (x1, y1) = (self.v[1].x, self.v[1].y);
        let (x2, y2) = (self.v[2].x, self.v[2].y);

        let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
        if denom.abs() < 1e-15 {
            return false;
        }

        let a = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / denom;
        let b = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / denom;
        let c = 1.0 - a - b;

        const EPS: f64 = -1e-8;
        a >= EPS && b >= EPS && c >= EPS
    }

    /// Compute Z on the triangle plane at (x, y). Returns None if nz ~ 0 (vertical triangle).
    pub fn z_at_xy(&self, x: f64, y: f64) -> Option<f64> {
        let nz = self.normal.z;
        if nz.abs() < 1e-15 {
            return None;
        }
        let d = -(self.normal.x * self.v[0].x
            + self.normal.y * self.v[0].y
            + self.normal.z * self.v[0].z);
        Some(-(self.normal.x * x + self.normal.y * y + d) / nz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_from_points() {
        let bb = BoundingBox3::from_points([
            P3::new(1.0, 2.0, 3.0),
            P3::new(-1.0, -2.0, -3.0),
            P3::new(0.0, 0.0, 0.0),
        ]);
        assert_eq!(bb.min, P3::new(-1.0, -2.0, -3.0));
        assert_eq!(bb.max, P3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_triangle_normal() {
        // XY plane triangle at z=0
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(0.0, 1.0, 0.0),
        );
        assert!((t.normal.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_triangle_contains_point() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        assert!(t.contains_point_xy(1.0, 1.0));
        assert!(t.contains_point_xy(0.0, 0.0));
        assert!(!t.contains_point_xy(6.0, 6.0)); // outside
        assert!(!t.contains_point_xy(-1.0, 0.0));
    }

    #[test]
    fn test_triangle_z_at_xy() {
        // Sloped triangle: z = x + y
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(0.0, 10.0, 10.0),
        );
        let z = t.z_at_xy(5.0, 3.0).unwrap();
        assert!((z - 8.0).abs() < 1e-10);
    }
}
