//! TSP (Traveling Salesman Problem) optimization for toolpath segment reordering.
//!
//! Reorders independent toolpath segments to minimize total rapid travel distance
//! using a nearest-neighbor heuristic followed by 2-opt improvement.

use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};

/// A continuous sequence of cutting moves between rapids.
struct Segment {
    moves: Vec<Move>,
    start: P3,
    end: P3,
}

/// XY-plane distance between two 3D points (ignores Z).
fn xy_distance(a: &P3, b: &P3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// Split a toolpath into segments of consecutive non-rapid moves.
///
/// Each segment tracks the start and end positions of its cutting moves.
/// Rapids between segments are discarded (they will be regenerated).
fn split_into_segments(toolpath: &Toolpath) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current_moves: Vec<Move> = Vec::new();

    for m in &toolpath.moves {
        match m.move_type {
            MoveType::Rapid => {
                // Flush any accumulated cutting moves as a segment.
                if !current_moves.is_empty() {
                    let start = current_moves[0].target;
                    let end = current_moves[current_moves.len() - 1].target;
                    segments.push(Segment {
                        moves: std::mem::take(&mut current_moves),
                        start,
                        end,
                    });
                }
            }
            _ => {
                current_moves.push(m.clone());
            }
        }
    }

    // Flush trailing cutting moves.
    if !current_moves.is_empty() {
        let start = current_moves[0].target;
        let end = current_moves[current_moves.len() - 1].target;
        segments.push(Segment {
            moves: current_moves,
            start,
            end,
        });
    }

    segments
}

/// Total XY rapid travel distance for a given segment visitation order.
///
/// Measures the sum of XY distances from the end of each segment to the
/// start of the next segment in the order.
#[cfg(test)]
fn total_rapid_distance(order: &[usize], segments: &[Segment]) -> f64 {
    if order.len() <= 1 {
        return 0.0;
    }
    let mut dist = 0.0;
    for i in 0..order.len() - 1 {
        dist += xy_distance(&segments[order[i]].end, &segments[order[i + 1]].start);
    }
    dist
}

/// Reorder independent toolpath segments to minimize total rapid travel distance.
///
/// A "segment" is a continuous sequence of cutting moves between rapids.
/// Returns a new toolpath with segments reordered for shorter rapids,
/// with proper retract/rapid/plunge moves inserted between segments.
///
/// # Algorithm
///
/// 1. Split the toolpath into cutting segments (strips rapids).
/// 2. Apply nearest-neighbor heuristic starting from segment 0.
/// 3. Improve with 2-opt swaps (up to 100 iterations).
/// 4. Reassemble with rapids between segments: retract to `safe_z`, rapid to
///    next segment start XY at `safe_z`, then the segment's cutting moves.
pub fn optimize_rapid_order(toolpath: &Toolpath, safe_z: f64) -> Toolpath {
    let segments = split_into_segments(toolpath);

    // Trivial cases: 0 or 1 segments need no optimization.
    if segments.len() <= 1 {
        return toolpath.clone();
    }

    // --- Nearest-neighbor heuristic ---
    let n = segments.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);

    // Start from segment 0.
    order.push(0);
    visited[0] = true;

    for _ in 1..n {
        let current = order[order.len() - 1];
        let current_end = &segments[current].end;

        let mut best_idx = 0;
        let mut best_dist = f64::INFINITY;

        for j in 0..n {
            if visited[j] {
                continue;
            }
            let d = xy_distance(current_end, &segments[j].start);
            if d < best_dist {
                best_dist = d;
                best_idx = j;
            }
        }

        visited[best_idx] = true;
        order.push(best_idx);
    }

    // --- 2-opt improvement ---
    let max_iterations = 100;
    for _ in 0..max_iterations {
        let mut improved = false;

        for i in 0..n.saturating_sub(1) {
            for j in (i + 2)..n {
                // Cost of current edges: (i -> i+1) and (j -> j+1 if exists).
                // Cost if we reverse the sub-tour i+1..=j.
                let cost_before = xy_distance(
                    &segments[order[i]].end,
                    &segments[order[i + 1]].start,
                ) + if j + 1 < n {
                    xy_distance(&segments[order[j]].end, &segments[order[j + 1]].start)
                } else {
                    0.0
                };

                let cost_after = xy_distance(
                    &segments[order[i]].end,
                    &segments[order[j]].end,
                ) + if j + 1 < n {
                    xy_distance(&segments[order[i + 1]].start, &segments[order[j + 1]].start)
                } else {
                    0.0
                };

                if cost_after < cost_before - 1e-10 {
                    order[i + 1..=j].reverse();
                    improved = true;
                }
            }
        }

        if !improved {
            break;
        }
    }

    // --- Reassemble toolpath ---
    let mut result = Toolpath::new();

    for (idx, &seg_idx) in order.iter().enumerate() {
        let seg = &segments[seg_idx];

        // Insert rapid travel: retract + move to next segment start.
        if idx == 0 {
            // Rapid to above the first segment's start.
            result.rapid_to(P3::new(seg.start.x, seg.start.y, safe_z));
        } else {
            // Retract from previous segment end, then rapid to this segment start.
            let prev_seg = &segments[order[idx - 1]];
            result.rapid_to(P3::new(prev_seg.end.x, prev_seg.end.y, safe_z));
            result.rapid_to(P3::new(seg.start.x, seg.start.y, safe_z));
        }

        // Emit all cutting moves in this segment.
        for m in &seg.moves {
            result.moves.push(m.clone());
        }
    }

    // Final retract after the last segment.
    if let Some(last_seg_idx) = order.last() {
        let last_seg = &segments[*last_seg_idx];
        result.rapid_to(P3::new(last_seg.end.x, last_seg.end.y, safe_z));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    /// Build a simple cutting segment: rapid to start at safe_z, feed moves along points,
    /// rapid retract at the end.
    fn make_segment_toolpath(points: &[P3], safe_z: f64, feed_rate: f64) -> Vec<Move> {
        let mut moves = Vec::new();
        if points.is_empty() {
            return moves;
        }
        // Rapid to above start
        moves.push(Move {
            target: P3::new(points[0].x, points[0].y, safe_z),
            move_type: MoveType::Rapid,
        });
        // Feed through all points
        for p in points {
            moves.push(Move {
                target: *p,
                move_type: MoveType::Linear { feed_rate },
            });
        }
        // Retract
        let last = points[points.len() - 1];
        moves.push(Move {
            target: P3::new(last.x, last.y, safe_z),
            move_type: MoveType::Rapid,
        });
        moves
    }

    #[test]
    fn test_four_segments_square_optimizer_improves() {
        // Four segments arranged in a square pattern but given in worst-case order:
        // Segment 0: (0,0) -> (1,0)   (bottom-left)
        // Segment 1: (100,100) -> (101,100)  (top-right, far away)
        // Segment 2: (1,0) -> (2,0)   (next to seg 0)
        // Segment 3: (100,0) -> (101,0)  (bottom-right, near seg 2's end via longer path)
        //
        // A bad order like [0, 1, 2, 3] would jump far; optimizer should find something better.
        let safe_z = 10.0;
        let feed = 1000.0;

        let mut tp = Toolpath::new();
        // Segment 0: cut at (0,0,-1) to (10,0,-1)
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, -1.0), P3::new(10.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        // Segment 1: cut at (100,100,-1) to (110,100,-1)  -- far corner
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 100.0, -1.0), P3::new(110.0, 100.0, -1.0)],
            safe_z,
            feed,
        ));
        // Segment 2: cut at (12,0,-1) to (20,0,-1)  -- near segment 0
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(12.0, 0.0, -1.0), P3::new(20.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        // Segment 3: cut at (100,0,-1) to (110,0,-1)  -- near segment 1 in X
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 0.0, -1.0), P3::new(110.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));

        let optimized = optimize_rapid_order(&tp, safe_z);

        // Compute rapid distances for the original vs optimized.
        let original_rapid = tp.total_rapid_distance();
        let optimized_rapid = optimized.total_rapid_distance();

        assert!(
            optimized_rapid < original_rapid,
            "Optimized rapid distance ({:.2}) should be less than original ({:.2})",
            optimized_rapid,
            original_rapid,
        );

        // Verify that all cutting moves are preserved (4 segments, 2 feed moves each = 8).
        let cutting_count = optimized
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        assert_eq!(cutting_count, 8, "All cutting moves must be preserved");
    }

    #[test]
    fn test_single_segment_unchanged() {
        let safe_z = 10.0;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, safe_z));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 1000.0);
        tp.rapid_to(P3::new(10.0, 0.0, safe_z));

        let optimized = optimize_rapid_order(&tp, safe_z);

        // With one segment the output should have the same cutting moves.
        let orig_cutting: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .collect();
        let opt_cutting: Vec<_> = optimized
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .collect();
        assert_eq!(orig_cutting.len(), opt_cutting.len());
        for (a, b) in orig_cutting.iter().zip(opt_cutting.iter()) {
            assert!(
                (a.target - b.target).norm() < 1e-10,
                "Cutting moves should be identical"
            );
        }
    }

    #[test]
    fn test_empty_toolpath_unchanged() {
        let tp = Toolpath::new();
        let optimized = optimize_rapid_order(&tp, 10.0);
        assert!(
            optimized.moves.is_empty(),
            "Empty toolpath should produce empty result"
        );
    }

    #[test]
    fn test_split_into_segments() {
        let mut tp = Toolpath::new();
        // Segment 1
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, -1.0), 1000.0);
        // Segment 2
        tp.rapid_to(P3::new(5.0, 0.0, 10.0));
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp.feed_to(P3::new(20.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(25.0, 0.0, -1.0), 1000.0);
        tp.rapid_to(P3::new(25.0, 0.0, 10.0));

        let segments = split_into_segments(&tp);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].moves.len(), 2);
        assert_eq!(segments[1].moves.len(), 2);
        assert!((segments[0].start.x - 0.0).abs() < 1e-10);
        assert!((segments[0].end.x - 5.0).abs() < 1e-10);
        assert!((segments[1].start.x - 20.0).abs() < 1e-10);
        assert!((segments[1].end.x - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_xy_distance() {
        let a = P3::new(0.0, 0.0, -5.0);
        let b = P3::new(3.0, 4.0, 100.0);
        let d = xy_distance(&a, &b);
        assert!((d - 5.0).abs() < 1e-10, "XY distance should be 5.0, got {}", d);
    }

    #[test]
    fn test_total_rapid_distance_helper() {
        let segments = vec![
            Segment {
                moves: vec![],
                start: P3::new(0.0, 0.0, 0.0),
                end: P3::new(10.0, 0.0, 0.0),
            },
            Segment {
                moves: vec![],
                start: P3::new(10.0, 10.0, 0.0),
                end: P3::new(20.0, 10.0, 0.0),
            },
            Segment {
                moves: vec![],
                start: P3::new(20.0, 0.0, 0.0),
                end: P3::new(30.0, 0.0, 0.0),
            },
        ];

        let order = vec![0, 1, 2];
        let dist = total_rapid_distance(&order, &segments);
        // end[0]=(10,0) -> start[1]=(10,10): 10.0
        // end[1]=(20,10) -> start[2]=(20,0): 10.0
        assert!((dist - 20.0).abs() < 1e-10, "Expected 20.0, got {}", dist);
    }
}
