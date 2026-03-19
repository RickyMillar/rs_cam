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
    ArcCW {
        i: f64,
        j: f64,
        feed_rate: f64,
    },
    /// Counter-clockwise arc (G3) in the XY plane. I/J are offsets from start to center.
    ArcCCW {
        i: f64,
        j: f64,
        feed_rate: f64,
    },
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

/// Convert a drop-cutter grid into a zigzag raster toolpath.
pub fn raster_toolpath_from_grid(
    grid: &DropCutterGrid,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let mut tp = Toolpath::new();

    // Start at safe Z
    let first = &grid.points[0];
    tp.rapid_to(P3::new(first.x, first.y, safe_z));

    for row in 0..grid.rows {
        // Zigzag: even rows go left-to-right, odd rows go right-to-left
        let cols: Box<dyn Iterator<Item = usize>> = if row % 2 == 0 {
            Box::new(0..grid.cols)
        } else {
            Box::new((0..grid.cols).rev())
        };

        let col_vec: Vec<usize> = cols.collect();
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
        let last_col = *col_vec.last().unwrap();
        let last_pt = grid.get(row, last_col);
        tp.rapid_to(P3::new(last_pt.x, last_pt.y, safe_z));
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;

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
