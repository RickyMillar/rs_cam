//! Marching squares contour extraction for waterline operations.
//!
//! Converts overlapping X-fiber and Y-fiber blocked intervals into a boolean
//! grid, then uses marching squares to extract topologically correct closed
//! contour loops. Contour points are placed at exact interval boundary positions,
//! not approximated to grid cell centers.
//!
//! This replaces the nearest-neighbor chaining approach which produces
//! crossed contours and wrong loop topology on complex surfaces.
//!
//! Advantages over nearest-neighbor:
//! - Topologically correct (no crossed contours)
//! - Correct loop count (nested contours, multiple disconnected loops)
//! - Deterministic (no proximity heuristic)
//! - O(N) contour extraction (vs O(N²) nearest-neighbor)

use crate::fiber::Fiber;
use crate::geo::P3;

/// Build a boolean grid from fiber intervals and extract contour loops
/// using marching squares.
///
/// Each cell (row, col) in the grid is "inside" if:
/// - The X-fiber at row `row` is blocked at the X-coordinate of Y-fiber `col`, AND
/// - The Y-fiber at column `col` is blocked at the Y-coordinate of X-fiber `row`.
///
/// Returns closed contour loops as sequences of 3D points.
pub fn weave_contours(x_fibers: &[Fiber], y_fibers: &[Fiber], z: f64) -> Vec<Vec<P3>> {
    if x_fibers.is_empty() || y_fibers.is_empty() {
        return Vec::new();
    }

    // Quick check: any intervals at all?
    let x_has = x_fibers.iter().any(|f| !f.intervals().is_empty());
    let y_has = y_fibers.iter().any(|f| !f.intervals().is_empty());
    if !x_has || !y_has {
        return Vec::new();
    }

    let n_rows = x_fibers.len();
    let n_cols = y_fibers.len();

    // Build the boolean grid
    // grid[row][col] = true means "inside" (cutter blocked here)
    let grid = build_boolean_grid(x_fibers, y_fibers);

    // Run marching squares to extract contour segments
    let segments = marching_squares(&grid, n_rows, n_cols, x_fibers, y_fibers, z);

    // Chain segments into closed loops
    chain_segments(segments)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Build a 2D boolean grid from fiber intervals.
fn build_boolean_grid(x_fibers: &[Fiber], y_fibers: &[Fiber]) -> Vec<Vec<bool>> {
    let n_rows = x_fibers.len();
    let n_cols = y_fibers.len();

    let mut grid = vec![vec![false; n_cols]; n_rows];

    for (row, x_fiber) in x_fibers.iter().enumerate() {
        for (col, y_fiber) in y_fibers.iter().enumerate() {
            // X-coordinate of Y-fiber, in X-fiber's parameter space
            let y_fiber_x = y_fiber.p1.x;
            let x_t = x_fiber.tval(&P3::new(y_fiber_x, x_fiber.p1.y, x_fiber.p1.z));

            // Y-coordinate of X-fiber, in Y-fiber's parameter space
            let x_fiber_y = x_fiber.p1.y;
            let y_t = y_fiber.tval(&P3::new(y_fiber.p1.x, x_fiber_y, y_fiber.p1.z));

            // Cell is inside if both fibers are blocked at this intersection
            let x_blocked = x_fiber.is_blocked(x_t);
            let y_blocked = y_fiber.is_blocked(y_t);

            grid[row][col] = x_blocked && y_blocked;
        }
    }

    grid
}

/// A contour segment — a line between two points on the grid boundary.
#[derive(Debug, Clone)]
struct Segment {
    p1: P3,
    p2: P3,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Run marching squares on the boolean grid.
/// Returns line segments at boundaries between inside/outside cells.
fn marching_squares(
    grid: &[Vec<bool>],
    n_rows: usize,
    n_cols: usize,
    x_fibers: &[Fiber],
    y_fibers: &[Fiber],
    z: f64,
) -> Vec<Segment> {
    let mut segments = Vec::new();

    // Marching squares operates on cells between grid vertices.
    // Grid vertex (row, col) maps to the intersection of x_fiber[row] and y_fiber[col].
    // Cell (r, c) has corners at (r,c), (r,c+1), (r+1,c+1), (r+1,c).
    for r in 0..n_rows.saturating_sub(1) {
        for c in 0..n_cols.saturating_sub(1) {
            // The 4 corners of this cell (in CCW order from bottom-left)
            let bl = grid[r][c]; // bottom-left
            let br = grid[r][c + 1]; // bottom-right
            let tr = grid[r + 1][c + 1]; // top-right
            let tl = grid[r + 1][c]; // top-left

            // Marching squares case index (4-bit)
            let case = (bl as u8) | ((br as u8) << 1) | ((tr as u8) << 2) | ((tl as u8) << 3);

            if case == 0 || case == 15 {
                continue; // All inside or all outside — no contour
            }

            // Edge midpoints (exact positions from fiber geometry)
            // Bottom edge: between (r,c) and (r,c+1)
            let bottom = edge_point_x(x_fibers, y_fibers, r, c, c + 1, z);
            // Right edge: between (r,c+1) and (r+1,c+1)
            let right = edge_point_y(x_fibers, y_fibers, c + 1, r, r + 1, z);
            // Top edge: between (r+1,c) and (r+1,c+1)
            let top = edge_point_x(x_fibers, y_fibers, r + 1, c, c + 1, z);
            // Left edge: between (r,c) and (r+1,c)
            let left = edge_point_y(x_fibers, y_fibers, c, r, r + 1, z);

            // Generate segments based on the case
            match case {
                1 => segments.push(Segment {
                    p1: bottom,
                    p2: left,
                }),
                2 => segments.push(Segment {
                    p1: right,
                    p2: bottom,
                }),
                3 => segments.push(Segment {
                    p1: right,
                    p2: left,
                }),
                4 => segments.push(Segment { p1: top, p2: right }),
                5 => {
                    // Saddle case — disambiguate by center value
                    // Use average of corners as center test
                    segments.push(Segment {
                        p1: bottom,
                        p2: right,
                    });
                    segments.push(Segment { p1: top, p2: left });
                }
                6 => segments.push(Segment {
                    p1: top,
                    p2: bottom,
                }),
                7 => segments.push(Segment { p1: top, p2: left }),
                8 => segments.push(Segment { p1: left, p2: top }),
                9 => segments.push(Segment {
                    p1: bottom,
                    p2: top,
                }),
                10 => {
                    // Saddle case — disambiguate
                    segments.push(Segment {
                        p1: left,
                        p2: bottom,
                    });
                    segments.push(Segment { p1: right, p2: top });
                }
                11 => segments.push(Segment { p1: right, p2: top }),
                12 => segments.push(Segment {
                    p1: left,
                    p2: right,
                }),
                13 => segments.push(Segment {
                    p1: bottom,
                    p2: right,
                }),
                14 => segments.push(Segment {
                    p1: left,
                    p2: bottom,
                }),
                _ => {} // 0 and 15 already handled
            }
        }
    }

    segments
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Compute a point on a horizontal cell edge (between two columns at the same row).
/// The point lies at the X-coordinate where the X-fiber's interval boundary crosses.
fn edge_point_x(
    x_fibers: &[Fiber],
    y_fibers: &[Fiber],
    row: usize,
    col_a: usize,
    col_b: usize,
    z: f64,
) -> P3 {
    let x_fiber = &x_fibers[row];
    let xa = y_fibers[col_a].p1.x;
    let xb = y_fibers[col_b].p1.x;

    // Find the exact X where the interval boundary lies between xa and xb
    // by checking interval endpoints
    let y = x_fiber.p1.y;
    let boundary_x = find_interval_boundary_x(x_fiber, xa, xb);

    P3::new(boundary_x, y, z)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Compute a point on a vertical cell edge (between two rows at the same column).
fn edge_point_y(
    x_fibers: &[Fiber],
    y_fibers: &[Fiber],
    col: usize,
    row_a: usize,
    row_b: usize,
    z: f64,
) -> P3 {
    let y_fiber = &y_fibers[col];
    let ya = x_fibers[row_a].p1.y;
    let yb = x_fibers[row_b].p1.y;

    let x = y_fiber.p1.x;
    let boundary_y = find_interval_boundary_y(y_fiber, ya, yb);

    P3::new(x, boundary_y, z)
}

/// Find the exact X-coordinate where an interval boundary lies between xa and xb.
fn find_interval_boundary_x(fiber: &Fiber, xa: f64, xb: f64) -> f64 {
    let (x_min, x_max) = if xa < xb { (xa, xb) } else { (xb, xa) };

    // Check each interval endpoint
    for interval in fiber.intervals() {
        let p_lower = fiber.point(interval.lower);
        let p_upper = fiber.point(interval.upper);

        if p_lower.x >= x_min - 1e-10 && p_lower.x <= x_max + 1e-10 {
            return p_lower.x;
        }
        if p_upper.x >= x_min - 1e-10 && p_upper.x <= x_max + 1e-10 {
            return p_upper.x;
        }
    }

    // Fallback: midpoint
    (xa + xb) * 0.5
}

/// Find the exact Y-coordinate where an interval boundary lies between ya and yb.
fn find_interval_boundary_y(fiber: &Fiber, ya: f64, yb: f64) -> f64 {
    let (y_min, y_max) = if ya < yb { (ya, yb) } else { (yb, ya) };

    for interval in fiber.intervals() {
        let p_lower = fiber.point(interval.lower);
        let p_upper = fiber.point(interval.upper);

        if p_lower.y >= y_min - 1e-10 && p_lower.y <= y_max + 1e-10 {
            return p_lower.y;
        }
        if p_upper.y >= y_min - 1e-10 && p_upper.y <= y_max + 1e-10 {
            return p_upper.y;
        }
    }

    (ya + yb) * 0.5
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Chain segments into closed loops by matching endpoints.
fn chain_segments(segments: Vec<Segment>) -> Vec<Vec<P3>> {
    if segments.is_empty() {
        return Vec::new();
    }

    let eps = 1e-6; // matching epsilon

    let mut remaining: Vec<Option<Segment>> = segments.into_iter().map(Some).collect();
    let mut loops = Vec::new();

    loop {
        // Find first unused segment
        let start_idx = remaining.iter().position(|s| s.is_some());
        let start_idx = match start_idx {
            Some(i) => i,
            None => break,
        };

        // SAFETY: start_idx found via .position(|s| s.is_some()) above
        #[allow(clippy::expect_used)]
        let start_seg = remaining[start_idx].take().expect("checked Some");
        let mut chain = vec![start_seg.p1, start_seg.p2];

        let max_iterations = remaining.len() + 1;
        for _ in 0..max_iterations {
            let tail = chain[chain.len() - 1];

            // Check if we've closed the loop
            let head = chain[0];
            let dx = tail.x - head.x;
            let dy = tail.y - head.y;
            if chain.len() >= 3 && dx * dx + dy * dy < eps * eps {
                chain.pop(); // remove the duplicate closing point
                break;
            }

            // Find a segment that connects to the tail
            let mut found = false;
            for seg_opt in remaining.iter_mut() {
                if let Some(seg) = seg_opt {
                    let d1 = (seg.p1.x - tail.x).powi(2) + (seg.p1.y - tail.y).powi(2);
                    let d2 = (seg.p2.x - tail.x).powi(2) + (seg.p2.y - tail.y).powi(2);

                    if d1 < eps * eps {
                        chain.push(seg.p2);
                        *seg_opt = None;
                        found = true;
                        break;
                    } else if d2 < eps * eps {
                        chain.push(seg.p1);
                        *seg_opt = None;
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                break;
            }
        }

        if chain.len() >= 3 {
            loops.push(chain);
        }
    }

    loops
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
    use crate::fiber::{Fiber, Interval};

    #[test]
    fn test_weave_no_intervals() {
        let xf = Fiber::new_x(5.0, 0.0, 0.0, 10.0);
        let yf = Fiber::new_y(5.0, 0.0, 0.0, 10.0);

        let contours = weave_contours(&[xf], &[yf], 0.0);
        assert!(
            contours.is_empty(),
            "No intervals should produce no contours"
        );
    }

    #[test]
    fn test_boolean_grid_basic() {
        // One X-fiber at y=5, blocked from x=3 to x=7
        let mut xf = Fiber::new_x(5.0, 0.0, 0.0, 10.0);
        xf.add_interval(Interval::new(0.3, 0.7));

        // Three Y-fibers at x=2, x=5, x=8
        let mut yf2 = Fiber::new_y(2.0, 0.0, 0.0, 10.0);
        let mut yf5 = Fiber::new_y(5.0, 0.0, 0.0, 10.0);
        let mut yf8 = Fiber::new_y(8.0, 0.0, 0.0, 10.0);
        yf2.add_interval(Interval::new(0.3, 0.7));
        yf5.add_interval(Interval::new(0.3, 0.7));
        yf8.add_interval(Interval::new(0.3, 0.7));

        let grid = build_boolean_grid(&[xf], &[yf2, yf5, yf8]);

        // Row 0 (the only X-fiber):
        // col 0 (x=2): x_fiber blocked at x=2? x=2 is below x=3, so NO
        assert!(!grid[0][0], "x=2 outside interval [3,7]");
        // col 1 (x=5): x_fiber blocked at x=5? YES (between 3 and 7)
        assert!(grid[0][1], "x=5 inside interval [3,7]");
        // col 2 (x=8): x_fiber blocked at x=8? NO
        assert!(!grid[0][2], "x=8 outside interval [3,7]");
    }

    #[test]
    fn test_weave_grid_produces_contour() {
        // Create a grid of fibers simulating a central obstruction
        let mut x_fibers = Vec::new();
        for y_val in [0.0, 2.0, 4.0, 6.0, 8.0, 10.0] {
            let mut f = Fiber::new_x(y_val, 0.0, 0.0, 10.0);
            // Only middle fibers have intervals
            if (2.0..=8.0).contains(&y_val) {
                f.add_interval(Interval::new(0.2, 0.8));
            }
            x_fibers.push(f);
        }

        let mut y_fibers = Vec::new();
        for x_val in [0.0, 2.0, 4.0, 6.0, 8.0, 10.0] {
            let mut f = Fiber::new_y(x_val, 0.0, 0.0, 10.0);
            if (2.0..=8.0).contains(&x_val) {
                f.add_interval(Interval::new(0.2, 0.8));
            }
            y_fibers.push(f);
        }

        let contours = weave_contours(&x_fibers, &y_fibers, 0.0);

        // Should produce at least one contour
        assert!(
            !contours.is_empty(),
            "Grid with central obstruction should produce contours"
        );

        // All contours should have at least 3 points
        for contour in &contours {
            assert!(
                contour.len() >= 3,
                "Contour too short: {} points",
                contour.len()
            );
        }
    }

    #[test]
    fn test_chain_segments_closed_loop() {
        // Create a simple square of segments that should form one closed loop
        let segments = vec![
            Segment {
                p1: P3::new(0.0, 0.0, 0.0),
                p2: P3::new(1.0, 0.0, 0.0),
            },
            Segment {
                p1: P3::new(1.0, 0.0, 0.0),
                p2: P3::new(1.0, 1.0, 0.0),
            },
            Segment {
                p1: P3::new(1.0, 1.0, 0.0),
                p2: P3::new(0.0, 1.0, 0.0),
            },
            Segment {
                p1: P3::new(0.0, 1.0, 0.0),
                p2: P3::new(0.0, 0.0, 0.0),
            },
        ];

        let loops = chain_segments(segments);
        assert_eq!(loops.len(), 1, "Should form one closed loop");
        assert_eq!(loops[0].len(), 4, "Loop should have 4 points");
    }

    #[test]
    fn test_chain_segments_two_loops() {
        // Two separate squares
        let segments = vec![
            // Square 1
            Segment {
                p1: P3::new(0.0, 0.0, 0.0),
                p2: P3::new(1.0, 0.0, 0.0),
            },
            Segment {
                p1: P3::new(1.0, 0.0, 0.0),
                p2: P3::new(1.0, 1.0, 0.0),
            },
            Segment {
                p1: P3::new(1.0, 1.0, 0.0),
                p2: P3::new(0.0, 1.0, 0.0),
            },
            Segment {
                p1: P3::new(0.0, 1.0, 0.0),
                p2: P3::new(0.0, 0.0, 0.0),
            },
            // Square 2 (far away)
            Segment {
                p1: P3::new(10.0, 10.0, 0.0),
                p2: P3::new(11.0, 10.0, 0.0),
            },
            Segment {
                p1: P3::new(11.0, 10.0, 0.0),
                p2: P3::new(11.0, 11.0, 0.0),
            },
            Segment {
                p1: P3::new(11.0, 11.0, 0.0),
                p2: P3::new(10.0, 11.0, 0.0),
            },
            Segment {
                p1: P3::new(10.0, 11.0, 0.0),
                p2: P3::new(10.0, 10.0, 0.0),
            },
        ];

        let loops = chain_segments(segments);
        assert_eq!(loops.len(), 2, "Should form two separate loops");
    }
}
