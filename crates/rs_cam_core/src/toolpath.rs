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
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Simplify a 3D path using Douglas-Peucker with cross-product distance.
///
/// Removes points that deviate less than `tolerance` from the line between
/// their neighbors. Uses 3D perpendicular distance via cross product for
/// accurate distance computation on slopes.
pub fn simplify_path_3d(points: &[P3], tolerance: f64) -> Vec<P3> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let dz = last.z - first.z;
    let seg_len = (dx * dx + dy * dy + dz * dz).sqrt();

    if seg_len < 1e-10 {
        return vec![first, last];
    }

    // Find point farthest from the line first→last
    let mut max_dist = 0.0;
    let mut max_idx = 0;

    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        // Vector from first to p
        let vx = p.x - first.x;
        let vy = p.y - first.y;
        let vz = p.z - first.z;
        // Cross product magnitude: |v x d| / |d|
        let cx = vy * dz - vz * dy;
        let cy = vz * dx - vx * dz;
        let cz = vx * dy - vy * dx;
        let dist = (cx * cx + cy * cy + cz * cz).sqrt() / seg_len;
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist <= tolerance {
        return vec![first, last];
    }

    let mut left = simplify_path_3d(&points[..=max_idx], tolerance);
    let right = simplify_path_3d(&points[max_idx..], tolerance);
    left.pop(); // Remove duplicate at split point
    left.extend(right);
    left
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
        // Zigzag: even rows go left-to-right, odd rows go right-to-left
        let cols: Box<dyn Iterator<Item = usize>> = if row % 2 == 0 {
            Box::new(0..grid.cols)
        } else {
            Box::new((0..grid.cols).rev())
        };

        let col_vec: Vec<usize> = cols.collect();
        if col_vec.is_empty() {
            continue;
        }
        let first_col = col_vec[0];
        let first_pt = grid.get(row, first_col);

        // Rapid to the start of this row at safe Z
        tp.rapid_to(P3::new(first_pt.x, first_pt.y, safe_z));
        // Plunge to cutting depth
        tp.feed_to(first_pt.position(), plunge_rate);

        // Feed along the row
        for &col in &col_vec[1..] {
            let cl = grid.get(row, col);
            tp.feed_to(cl.position(), feed_rate);
        }

        // Retract at end of row
        let last_col = col_vec[col_vec.len() - 1];
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
