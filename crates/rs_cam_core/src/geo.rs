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
            min: P3::new(
                self.min.x - margin,
                self.min.y - margin,
                self.min.z - margin,
            ),
            max: P3::new(
                self.max.x + margin,
                self.max.y + margin,
                self.max.z + margin,
            ),
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

    /// Test whether a point lies inside (or on the boundary of) this AABB.
    pub fn contains_point(&self, p: &P3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Ray-AABB intersection using the slab method (Kay/Kajiya).
    /// Returns the parametric `t` of the nearest intersection (entry point),
    /// or `None` if the ray misses. A hit at `t >= 0` means the intersection
    /// is in front of the ray origin.
    pub fn ray_intersect(&self, origin: &P3, dir: &V3) -> Option<f64> {
        let mut t_min = f64::NEG_INFINITY;
        let mut t_max = f64::INFINITY;

        for axis in 0..3 {
            let o = origin[axis];
            let d = dir[axis];
            let lo = self.min[axis];
            let hi = self.max[axis];

            if d.abs() < 1e-12 {
                // Ray is parallel to this slab — miss if origin outside
                if o < lo || o > hi {
                    return None;
                }
            } else {
                let inv_d = 1.0 / d;
                let mut t0 = (lo - o) * inv_d;
                let mut t1 = (hi - o) * inv_d;
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                t_min = t_min.max(t0);
                t_max = t_max.min(t1);
                if t_min > t_max {
                    return None;
                }
            }
        }

        // Return the entry t if it's in front, otherwise the exit t
        if t_min >= 0.0 {
            Some(t_min)
        } else if t_max >= 0.0 {
            Some(t_max) // origin inside box
        } else {
            None // box is behind the ray
        }
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
    #[inline]
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

    /// Moller-Trumbore ray-triangle intersection.
    ///
    /// Returns the parametric `t` of the intersection point along the ray
    /// (hit point = origin + t * dir), or `None` if the ray misses.
    /// Only returns hits with `t >= 0` (in front of the ray origin).
    pub fn ray_intersect(&self, origin: &P3, dir: &V3) -> Option<f64> {
        let eps = 1e-10;
        let e1 = self.v[1] - self.v[0];
        let e2 = self.v[2] - self.v[0];
        let h = dir.cross(&e2);
        let det = e1.dot(&h);

        // Ray parallel to triangle
        if det.abs() < eps {
            return None;
        }

        let inv_det = 1.0 / det;
        let s = origin - self.v[0];
        let u = inv_det * s.dot(&h);
        if u < -eps || u > 1.0 + eps {
            return None;
        }

        let q = s.cross(&e1);
        let v = inv_det * dir.dot(&q);
        if v < -eps || u + v > 1.0 + eps {
            return None;
        }

        let t = inv_det * e2.dot(&q);
        if t >= 0.0 {
            Some(t)
        } else {
            None // intersection behind the ray
        }
    }

    /// Compute Z on the triangle plane at (x, y). Returns None if nz ~ 0 (vertical triangle).
    #[inline]
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

/// Compute the minimum Euclidean distance from a point to a line segment.
pub fn point_to_segment_distance(p: &P2, a: &P2, b: &P2) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ab_len_sq = ab_x * ab_x + ab_y * ab_y;

    if ab_len_sq < 1e-20 {
        // Degenerate segment (point)
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return (dx * dx + dy * dy).sqrt();
    }

    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;
    let t = ((ap_x * ab_x + ap_y * ab_y) / ab_len_sq).clamp(0.0, 1.0);
    let closest_x = a.x + t * ab_x;
    let closest_y = a.y + t * ab_y;
    let dx = p.x - closest_x;
    let dy = p.y - closest_y;
    (dx * dx + dy * dy).sqrt()
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

    #[test]
    fn test_point_to_segment_distance() {
        let a = P2::new(0.0, 0.0);
        let b = P2::new(10.0, 0.0);

        // Point directly above the midpoint
        let p = P2::new(5.0, 3.0);
        let d = point_to_segment_distance(&p, &a, &b);
        assert!((d - 3.0).abs() < 1e-10, "Should be 3.0, got {}", d);

        // Point beyond the end
        let p2 = P2::new(12.0, 0.0);
        let d2 = point_to_segment_distance(&p2, &a, &b);
        assert!((d2 - 2.0).abs() < 1e-10, "Should be 2.0, got {}", d2);

        // Point at vertex
        let p3 = P2::new(0.0, 4.0);
        let d3 = point_to_segment_distance(&p3, &a, &b);
        assert!((d3 - 4.0).abs() < 1e-10, "Should be 4.0, got {}", d3);
    }

    #[test]
    fn test_point_to_segment_degenerate() {
        // Degenerate segment (single point)
        let a = P2::new(5.0, 5.0);
        let p = P2::new(8.0, 9.0);
        let d = point_to_segment_distance(&p, &a, &a);
        assert!((d - 5.0).abs() < 1e-10, "Should be 5.0, got {}", d);
    }

    #[test]
    fn test_tri_ray_intersect_hit_center() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray from above, pointing down at triangle center
        let origin = P3::new(2.0, 2.0, 5.0);
        let dir = V3::new(0.0, 0.0, -1.0);
        let hit = t.ray_intersect(&origin, &dir);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_tri_ray_intersect_miss() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray from above but outside the triangle
        let origin = P3::new(20.0, 20.0, 5.0);
        let dir = V3::new(0.0, 0.0, -1.0);
        assert!(t.ray_intersect(&origin, &dir).is_none());
    }

    #[test]
    fn test_tri_ray_intersect_parallel() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray parallel to triangle plane
        let origin = P3::new(2.0, 2.0, 5.0);
        let dir = V3::new(1.0, 0.0, 0.0);
        assert!(t.ray_intersect(&origin, &dir).is_none());
    }

    #[test]
    fn test_tri_ray_intersect_behind() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray pointing away from triangle
        let origin = P3::new(2.0, 2.0, 5.0);
        let dir = V3::new(0.0, 0.0, 1.0);
        assert!(t.ray_intersect(&origin, &dir).is_none());
    }

    #[test]
    fn test_tri_ray_intersect_at_edge() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray hitting the hypotenuse edge (u+v ≈ 1)
        let origin = P3::new(5.0, 5.0, 3.0);
        let dir = V3::new(0.0, 0.0, -1.0);
        // Point (5,5) is on the hypotenuse line x+y=10, so it should hit
        let hit = t.ray_intersect(&origin, &dir);
        assert!(hit.is_some());
    }

    #[test]
    fn test_tri_ray_intersect_backface() {
        let t = Triangle::new(
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        );
        // Ray from below hitting triangle from the back
        let origin = P3::new(2.0, 2.0, -5.0);
        let dir = V3::new(0.0, 0.0, 1.0);
        let hit = t.ray_intersect(&origin, &dir);
        assert!(hit.is_some(), "Backface hits should be detected");
        assert!((hit.unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_bbox_contains_point() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        assert!(bb.contains_point(&P3::new(5.0, 5.0, 5.0)));
        assert!(bb.contains_point(&P3::new(0.0, 0.0, 0.0))); // on boundary
        assert!(!bb.contains_point(&P3::new(-1.0, 5.0, 5.0)));
        assert!(!bb.contains_point(&P3::new(5.0, 11.0, 5.0)));
    }

    #[test]
    fn test_ray_intersect_hit() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        // Ray from outside, hitting the -X face
        let origin = P3::new(-5.0, 5.0, 5.0);
        let dir = V3::new(1.0, 0.0, 0.0);
        let t = bb.ray_intersect(&origin, &dir);
        assert!(t.is_some());
        assert!((t.unwrap() - 5.0).abs() < 1e-10, "t = {}", t.unwrap());
    }

    #[test]
    fn test_ray_intersect_miss() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        // Ray parallel to X axis but above the box
        let origin = P3::new(-5.0, 15.0, 5.0);
        let dir = V3::new(1.0, 0.0, 0.0);
        assert!(bb.ray_intersect(&origin, &dir).is_none());
    }

    #[test]
    fn test_ray_intersect_origin_inside() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        // Origin inside the box
        let origin = P3::new(5.0, 5.0, 5.0);
        let dir = V3::new(1.0, 0.0, 0.0);
        let t = bb.ray_intersect(&origin, &dir);
        assert!(t.is_some());
        // Should hit the +X face at t=5
        assert!((t.unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_ray_intersect_behind() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        // Ray pointing away from box
        let origin = P3::new(-5.0, 5.0, 5.0);
        let dir = V3::new(-1.0, 0.0, 0.0);
        assert!(bb.ray_intersect(&origin, &dir).is_none());
    }

    #[test]
    fn test_ray_intersect_diagonal() {
        let bb = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        let origin = P3::new(-1.0, -1.0, -1.0);
        let dir = V3::new(1.0, 1.0, 1.0).normalize();
        let t = bb.ray_intersect(&origin, &dir);
        assert!(t.is_some());
        // Ray enters at (0,0,0), distance from origin = sqrt(3)
        let expected = (3.0_f64).sqrt();
        assert!(
            (t.unwrap() - expected).abs() < 1e-10,
            "t = {}, expected {}",
            t.unwrap(),
            expected
        );
    }
}
