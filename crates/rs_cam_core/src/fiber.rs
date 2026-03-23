//! Fiber and Interval types for push-cutter and waterline algorithms.
//!
//! A Fiber is a line segment in XY at a constant Z height, parameterized by t ∈ [0,1].
//! Push-cutter produces intervals on fibers where the cutter cannot go (would gouge).
//! The complement of these intervals gives the valid toolpath.

use crate::geo::P3;

/// A single interval on a fiber, parameterized by t ∈ [0,1].
#[derive(Debug, Clone, Copy)]
pub struct Interval {
    /// Lower bound (t parameter)
    pub lower: f64,
    /// Upper bound (t parameter)
    pub upper: f64,
}

impl Interval {
    pub fn new(lower: f64, upper: f64) -> Self {
        debug_assert!(
            lower <= upper + 1e-10,
            "Interval lower {} > upper {}",
            lower,
            upper
        );
        Self {
            lower: lower.min(upper),
            upper: upper.max(lower),
        }
    }

    #[inline]
    pub fn contains(&self, t: f64) -> bool {
        t >= self.lower - 1e-10 && t <= self.upper + 1e-10
    }

    #[inline]
    pub fn overlaps(&self, other: &Interval) -> bool {
        self.lower <= other.upper + 1e-10 && other.lower <= self.upper + 1e-10
    }

    #[inline]
    pub fn merge(&self, other: &Interval) -> Interval {
        Interval::new(self.lower.min(other.lower), self.upper.max(other.upper))
    }

    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }
}

/// A line segment in XY at constant Z, used by push-cutter.
///
/// The fiber goes from `p1` to `p2`, parameterized as:
///   point(t) = p1 + t * (p2 - p1)  for t ∈ [0, 1]
#[derive(Debug, Clone)]
pub struct Fiber {
    pub p1: P3,
    pub p2: P3,
    /// Merged intervals where the cutter is blocked (would gouge).
    intervals: Vec<Interval>,
}

impl Fiber {
    /// Create a new X-fiber (horizontal, constant Y and Z).
    pub fn new_x(y: f64, z: f64, x_min: f64, x_max: f64) -> Self {
        Self {
            p1: P3::new(x_min, y, z),
            p2: P3::new(x_max, y, z),
            intervals: Vec::new(),
        }
    }

    /// Create a new Y-fiber (vertical in XY, constant X and Z).
    pub fn new_y(x: f64, z: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            p1: P3::new(x, y_min, z),
            p2: P3::new(x, y_max, z),
            intervals: Vec::new(),
        }
    }

    /// The Z height of this fiber.
    pub fn z(&self) -> f64 {
        self.p1.z
    }

    /// Length of the fiber in XY.
    pub fn length(&self) -> f64 {
        let dx = self.p2.x - self.p1.x;
        let dy = self.p2.y - self.p1.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Convert parameter t to a 3D point.
    pub fn point(&self, t: f64) -> P3 {
        P3::new(
            self.p1.x + t * (self.p2.x - self.p1.x),
            self.p1.y + t * (self.p2.y - self.p1.y),
            self.p1.z,
        )
    }

    /// Convert a 3D point to a parameter t along this fiber.
    pub fn tval(&self, p: &P3) -> f64 {
        let dx = self.p2.x - self.p1.x;
        let dy = self.p2.y - self.p1.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq < 1e-20 {
            return 0.0;
        }
        ((p.x - self.p1.x) * dx + (p.y - self.p1.y) * dy) / len_sq
    }

    /// Add an interval where the cutter is blocked. Merges overlapping intervals.
    pub fn add_interval(&mut self, new: Interval) {
        // Clamp to [0, 1]
        let clamped = Interval::new(new.lower.max(0.0), new.upper.min(1.0));
        if clamped.width() < 1e-15 {
            return;
        }

        // Insert maintaining sorted order, merge overlaps
        let mut merged = Vec::with_capacity(self.intervals.len() + 1);
        let mut current = clamped;
        let mut inserted = false;

        for existing in &self.intervals {
            if current.overlaps(existing) {
                current = current.merge(existing);
            } else if !inserted && current.upper < existing.lower {
                merged.push(current);
                merged.push(*existing);
                inserted = true;
            } else {
                merged.push(*existing);
            }
        }

        if !inserted {
            merged.push(current);
        }

        self.intervals = merged;
    }

    /// Get the current intervals (sorted, non-overlapping).
    pub fn intervals(&self) -> &[Interval] {
        &self.intervals
    }

    /// Get the CL (cutter-location) points at interval boundaries.
    /// Returns the 3D points at each interval lower and upper bound.
    pub fn cl_points(&self) -> Vec<P3> {
        let mut points = Vec::new();
        for iv in &self.intervals {
            points.push(self.point(iv.lower));
            points.push(self.point(iv.upper));
        }
        points
    }

    /// Check if a parameter t is inside any blocked interval.
    pub fn is_blocked(&self, t: f64) -> bool {
        self.intervals.iter().any(|iv| iv.contains(t))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_basic() {
        let iv = Interval::new(0.2, 0.8);
        assert!((iv.lower - 0.2).abs() < 1e-10);
        assert!((iv.upper - 0.8).abs() < 1e-10);
        assert!((iv.width() - 0.6).abs() < 1e-10);
    }

    #[test]
    fn test_interval_contains() {
        let iv = Interval::new(0.2, 0.8);
        assert!(iv.contains(0.5));
        assert!(iv.contains(0.2));
        assert!(iv.contains(0.8));
        assert!(!iv.contains(0.1));
        assert!(!iv.contains(0.9));
    }

    #[test]
    fn test_interval_overlap() {
        let a = Interval::new(0.2, 0.5);
        let b = Interval::new(0.4, 0.8);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));

        let c = Interval::new(0.6, 0.9);
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_interval_merge() {
        let a = Interval::new(0.2, 0.5);
        let b = Interval::new(0.4, 0.8);
        let m = a.merge(&b);
        assert!((m.lower - 0.2).abs() < 1e-10);
        assert!((m.upper - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_x() {
        let f = Fiber::new_x(5.0, 10.0, 0.0, 100.0);
        assert!((f.z() - 10.0).abs() < 1e-10);
        assert!((f.length() - 100.0).abs() < 1e-10);

        let p = f.point(0.5);
        assert!((p.x - 50.0).abs() < 1e-10);
        assert!((p.y - 5.0).abs() < 1e-10);
        assert!((p.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_y() {
        let f = Fiber::new_y(5.0, 10.0, 0.0, 100.0);
        let p = f.point(0.25);
        assert!((p.x - 5.0).abs() < 1e-10);
        assert!((p.y - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_tval() {
        let f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        let t = f.tval(&P3::new(30.0, 0.0, 0.0));
        assert!((t - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_add_single_interval() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.2, 0.4));
        assert_eq!(f.intervals().len(), 1);
        assert!((f.intervals()[0].lower - 0.2).abs() < 1e-10);
        assert!((f.intervals()[0].upper - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_merge_overlapping() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.2, 0.5));
        f.add_interval(Interval::new(0.4, 0.7));
        assert_eq!(f.intervals().len(), 1);
        assert!((f.intervals()[0].lower - 0.2).abs() < 1e-10);
        assert!((f.intervals()[0].upper - 0.7).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_non_overlapping() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.1, 0.2));
        f.add_interval(Interval::new(0.5, 0.6));
        assert_eq!(f.intervals().len(), 2);
    }

    #[test]
    fn test_fiber_clamp_to_bounds() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(-0.5, 0.3));
        assert_eq!(f.intervals().len(), 1);
        assert!((f.intervals()[0].lower - 0.0).abs() < 1e-10);
        assert!((f.intervals()[0].upper - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_is_blocked() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.2, 0.4));
        assert!(f.is_blocked(0.3));
        assert!(!f.is_blocked(0.5));
    }

    #[test]
    fn test_fiber_cl_points() {
        let mut f = Fiber::new_x(0.0, 5.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.2, 0.4));
        f.add_interval(Interval::new(0.6, 0.8));
        let pts = f.cl_points();
        assert_eq!(pts.len(), 4);
        assert!((pts[0].x - 20.0).abs() < 1e-10);
        assert!((pts[1].x - 40.0).abs() < 1e-10);
        assert!((pts[2].x - 60.0).abs() < 1e-10);
        assert!((pts[3].x - 80.0).abs() < 1e-10);
    }

    #[test]
    fn test_fiber_multiple_merges() {
        let mut f = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
        f.add_interval(Interval::new(0.1, 0.3));
        f.add_interval(Interval::new(0.5, 0.7));
        f.add_interval(Interval::new(0.2, 0.6)); // bridges the gap
        assert_eq!(f.intervals().len(), 1);
        assert!((f.intervals()[0].lower - 0.1).abs() < 1e-10);
        assert!((f.intervals()[0].upper - 0.7).abs() < 1e-10);
    }
}
