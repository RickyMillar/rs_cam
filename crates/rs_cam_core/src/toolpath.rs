//! Toolpath intermediate representation.
//!
//! Operations produce Toolpath (not G-code). G-code is a final serialization step.
//! This enables dressups, visualization, and analysis without G-code parsing.

use crate::dropcutter::DropCutterGrid;
use crate::geo::P3;

/// Type of motion for a toolpath move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveType {
    /// Rapid positioning (G0). Never used for cutting.
    Rapid,
    /// Linear feed move (G1) at specified feed rate (mm/min).
    Linear { feed_rate: f64 },
    /// Clockwise arc (G2) in the XY plane. I/J are offsets from start to center.
    ArcCW { i: f64, j: f64, feed_rate: f64 },
    /// Counter-clockwise arc (G3) in the XY plane. I/J are offsets from start to center.
    ArcCCW { i: f64, j: f64, feed_rate: f64 },
}

/// A single toolpath move to a target position.
#[derive(Debug, Clone)]
pub struct Move {
    pub target: P3,
    pub move_type: MoveType,
}

/// A complete toolpath: a sequence of moves.
#[derive(Debug, Clone, Default)]
pub struct Toolpath {
    pub moves: Vec<Move>,
}

impl Toolpath {
    pub fn new() -> Self {
        Self { moves: Vec::new() }
    }

    pub fn rapid_to(&mut self, target: P3) {
        self.moves.push(Move {
            target,
            move_type: MoveType::Rapid,
        });
    }

    pub fn feed_to(&mut self, target: P3, feed_rate: f64) {
        self.moves.push(Move {
            target,
            move_type: MoveType::Linear { feed_rate },
        });
    }

    pub fn arc_cw_to(&mut self, target: P3, i: f64, j: f64, feed_rate: f64) {
        self.moves.push(Move {
            target,
            move_type: MoveType::ArcCW { i, j, feed_rate },
        });
    }

    pub fn arc_ccw_to(&mut self, target: P3, i: f64, j: f64, feed_rate: f64) {
        self.moves.push(Move {
            target,
            move_type: MoveType::ArcCCW { i, j, feed_rate },
        });
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn total_cutting_distance(&self) -> f64 {
        let mut dist = 0.0;
        for i in 1..self.moves.len() {
            match self.moves[i].move_type {
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    let p1 = &self.moves[i - 1].target;
                    let p2 = &self.moves[i].target;
                    dist += (p2 - p1).norm();
                }
                _ => {}
            }
        }
        dist
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Emit rapid→plunge→feed→retract for a 3D path.
    ///
    /// For an empty path, this is a no-op. For a single point, emits
    /// rapid+plunge+retract only (no feed moves).
    pub fn emit_path_segment(
        &mut self,
        path: &[P3],
        safe_z: f64,
        feed_rate: f64,
        plunge_rate: f64,
    ) {
        if path.is_empty() {
            return;
        }
        // Rapid to above first point
        self.rapid_to(P3::new(path[0].x, path[0].y, safe_z));
        // Plunge to first point
        self.feed_to(path[0], plunge_rate);
        // Feed along remaining points
        for p in path.iter().skip(1) {
            self.feed_to(*p, feed_rate);
        }
        // Retract
        if let Some(last) = path.last() {
            self.rapid_to(P3::new(last.x, last.y, safe_z));
        }
    }

    /// Retract to safe_z if currently below it (0.001mm epsilon).
    pub fn final_retract(&mut self, safe_z: f64) {
        if let Some(last) = self.moves.last()
            && last.target.z < safe_z - 0.001
        {
            self.rapid_to(P3::new(last.target.x, last.target.y, safe_z));
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn total_rapid_distance(&self) -> f64 {
        let mut dist = 0.0;
        for i in 1..self.moves.len() {
            if self.moves[i].move_type == MoveType::Rapid {
                let p1 = &self.moves[i - 1].target;
                let p2 = &self.moves[i].target;
                dist += (p2 - p1).norm();
            }
        }
        dist
    }

    /// Sorted unique Z levels in the toolpath, deduplicated with given epsilon.
    pub fn z_levels(&self, epsilon: f64) -> Vec<f64> {
        let mut zs: Vec<f64> = self.moves.iter().map(|m| m.target.z).collect();
        zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        zs.dedup_by(|a, b| (*a - *b).abs() < epsilon);
        zs
    }

    /// Sorted unique feed rates in the toolpath, deduplicated with given epsilon.
    pub fn feed_rates(&self, epsilon: f64) -> Vec<f64> {
        let mut rates: Vec<f64> = self
            .moves
            .iter()
            .filter_map(|m| match m.move_type {
                MoveType::Linear { feed_rate } => Some(feed_rate),
                MoveType::ArcCW { feed_rate, .. } => Some(feed_rate),
                MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
                MoveType::Rapid => None,
            })
            .collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        rates.dedup_by(|a, b| (*a - *b).abs() < epsilon);
        rates
    }

    /// XYZ bounding box of all move targets: ([min_x, min_y, min_z], [max_x, max_y, max_z]).
    /// Returns zeros for empty toolpaths.
    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        if self.moves.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        for m in &self.moves {
            let p = &m.target;
            if p.x < min[0] {
                min[0] = p.x;
            }
            if p.y < min[1] {
                min[1] = p.y;
            }
            if p.z < min[2] {
                min[2] = p.z;
            }
            if p.x > max[0] {
                max[0] = p.x;
            }
            if p.y > max[1] {
                max[1] = p.y;
            }
            if p.z > max[2] {
                max[2] = p.z;
            }
        }
        (min, max)
    }
}

/// Simplify a 3D path using Douglas-Peucker with cross-product distance.
///
/// Removes points that deviate less than `tolerance` from the line between
/// their neighbors. Uses 3D perpendicular distance via cross product for
/// accurate distance computation on slopes.
///
/// Iterative stack-based implementation avoids per-recursion Vec allocations.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn simplify_path_3d(points: &[P3], tolerance: f64) -> Vec<P3> {
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }

    // Mark which points to keep. First and last are always kept.
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    // Explicit stack of (start, end) index ranges to process.
    let mut stack = Vec::with_capacity(16);
    stack.push((0usize, n - 1));

    while let Some((start, end)) = stack.pop() {
        if end - start < 2 {
            continue;
        }

        let first = points[start];
        let last = points[end];
        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let dz = last.z - first.z;
        let seg_len = (dx * dx + dy * dy + dz * dz).sqrt();

        if seg_len < 1e-10 {
            continue;
        }

        let mut max_dist = 0.0;
        let mut max_idx = start;

        for (i, p) in points.iter().enumerate().skip(start + 1).take(end - start - 1) {
            let vx = p.x - first.x;
            let vy = p.y - first.y;
            let vz = p.z - first.z;
            let cx = vy * dz - vz * dy;
            let cy = vz * dx - vx * dz;
            let cz = vx * dy - vy * dx;
            let dist = (cx * cx + cy * cy + cz * cz).sqrt() / seg_len;
            if dist > max_dist {
                max_dist = dist;
                max_idx = i;
            }
        }

        if max_dist > tolerance {
            keep[max_idx] = true;
            stack.push((start, max_idx));
            stack.push((max_idx, end));
        }
    }

    points
        .iter()
        .zip(keep.iter())
        .filter(|&(_, &k)| k)
        .map(|(p, _)| *p)
        .collect()
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Convert a drop-cutter grid into a zigzag raster toolpath.
pub fn raster_toolpath_from_grid(
    grid: &DropCutterGrid,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let mut tp = Toolpath::new();

    if grid.points.is_empty() {
        return tp;
    }

    for row in 0..grid.rows {
        if grid.cols == 0 {
            continue;
        }
        // Zigzag: even rows go left-to-right, odd rows go right-to-left
        let reverse = row % 2 != 0;
        let first_col = if reverse { grid.cols - 1 } else { 0 };
        let first_pt = grid.get(row, first_col);

        // Rapid to the start of this row at safe Z
        tp.rapid_to(P3::new(first_pt.x, first_pt.y, safe_z));
        // Plunge to cutting depth
        tp.feed_to(first_pt.position(), plunge_rate);

        // Feed along the row
        if reverse {
            for col in (0..grid.cols.saturating_sub(1)).rev() {
                let cl = grid.get(row, col);
                tp.feed_to(cl.position(), feed_rate);
            }
        } else {
            for col in 1..grid.cols {
                let cl = grid.get(row, col);
                tp.feed_to(cl.position(), feed_rate);
            }
        }

        // Retract at end of row
        let last_col = if reverse { 0 } else { grid.cols - 1 };
        let last_pt = grid.get(row, last_col);
        tp.rapid_to(P3::new(last_pt.x, last_pt.y, safe_z));
    }

    tp
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_path_segment_basic() {
        let path = vec![
            P3::new(0.0, 0.0, -1.0),
            P3::new(5.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        ];
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&path, 10.0, 1000.0, 500.0);

        // rapid + plunge + 2 feeds + retract = 5 moves
        assert_eq!(
            tp.moves.len(),
            5,
            "Expected 5 moves, got {}",
            tp.moves.len()
        );
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!((tp.moves[0].target.z - 10.0).abs() < 1e-10);
        assert!(
            matches!(tp.moves[1].move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-10)
        );
        assert!(
            matches!(tp.moves[2].move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
        );
        assert!(
            matches!(tp.moves[3].move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
        );
        assert_eq!(tp.moves[4].move_type, MoveType::Rapid);
        assert!((tp.moves[4].target.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_emit_path_segment_empty() {
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&[], 10.0, 1000.0, 500.0);
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn test_emit_path_segment_single_point() {
        let path = vec![P3::new(5.0, 5.0, -2.0)];
        let mut tp = Toolpath::new();
        tp.emit_path_segment(&path, 10.0, 1000.0, 500.0);

        // rapid + plunge + retract = 3 moves
        assert_eq!(tp.moves.len(), 3);
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!(matches!(tp.moves[1].move_type, MoveType::Linear { .. }));
        assert_eq!(tp.moves[2].move_type, MoveType::Rapid);
    }

    #[test]
    fn test_final_retract_below() {
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(5.0, 5.0, -3.0), 1000.0);
        tp.final_retract(10.0);
        assert_eq!(tp.moves.len(), 2);
        assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
        assert!((tp.moves[1].target.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_final_retract_already_safe() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 5.0, 10.0));
        tp.final_retract(10.0);
        // No retract added since we're already at safe_z
        assert_eq!(tp.moves.len(), 1);
    }

    #[test]
    fn test_simplify_path_3d_collinear() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 2, "Collinear points should reduce to 2");
    }

    #[test]
    fn test_simplify_path_3d_preserves_corners() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(5.0, 5.0, 5.0),
            P3::new(10.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 3, "Corner should be preserved");
    }

    #[test]
    fn test_z_levels() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(10.0, 0.0, -6.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -6.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));

        let levels = tp.z_levels(0.001);
        assert_eq!(levels, vec![-6.0, -3.0, 10.0]);
    }

    #[test]
    fn test_feed_rates() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));

        let rates = tp.feed_rates(0.1);
        assert_eq!(rates, vec![500.0, 1000.0]);
    }

    #[test]
    fn test_bounding_box() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-5.0, 2.0, 10.0));
        tp.feed_to(P3::new(15.0, -3.0, -8.0), 1000.0);

        let (min, max) = tp.bounding_box();
        assert!((min[0] - (-5.0)).abs() < 1e-10);
        assert!((min[1] - (-3.0)).abs() < 1e-10);
        assert!((min[2] - (-8.0)).abs() < 1e-10);
        assert!((max[0] - 15.0).abs() < 1e-10);
        assert!((max[1] - 2.0).abs() < 1e-10);
        assert!((max[2] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_bounding_box_empty() {
        let tp = Toolpath::new();
        let (min, max) = tp.bounding_box();
        assert_eq!(min, [0.0; 3]);
        assert_eq!(max, [0.0; 3]);
    }

    #[test]
    fn test_toolpath_distances() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 10.0)); // rapid 10mm
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 100.0); // feed 10mm
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 100.0); // feed 10mm

        assert!((tp.total_rapid_distance() - 10.0).abs() < 1e-10);
        assert!((tp.total_cutting_distance() - 20.0).abs() < 1e-10);
    }
}
