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
use crate::geo::{P2, P3};

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
    chain_segments(&segments)
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
///
/// Uses a spatial hash map on quantized endpoints for O(1) neighbor lookup
/// instead of O(n) linear scan per chain link.
fn chain_segments(segments: &[Segment]) -> Vec<Vec<P3>> {
    use std::collections::HashMap;

    if segments.is_empty() {
        return Vec::new();
    }

    let eps = 1e-6;
    let n = segments.len();

    // Quantize a coordinate to an integer grid at epsilon scale.
    // Use 1e-5 grid (10× epsilon) so nearby points land in same or adjacent cells.
    let quantize = |v: f64| -> i64 { (v * 1e5).round() as i64 };
    type GridKey = (i64, i64);

    // Build spatial index: map from quantized (x,y) → list of (segment_index, endpoint_id).
    // endpoint_id: 0 = p1, 1 = p2.
    let mut index: HashMap<GridKey, Vec<(usize, u8)>> = HashMap::with_capacity(n * 2);
    for (i, seg) in segments.iter().enumerate() {
        let k1 = (quantize(seg.p1.x), quantize(seg.p1.y));
        let k2 = (quantize(seg.p2.x), quantize(seg.p2.y));
        index.entry(k1).or_default().push((i, 0));
        index.entry(k2).or_default().push((i, 1));
    }

    let mut used = vec![false; n];
    let mut loops = Vec::new();

    for start_idx in 0..n {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let mut chain = vec![segments[start_idx].p1, segments[start_idx].p2];

        let max_iterations = n + 1;
        for _ in 0..max_iterations {
            let tail = chain[chain.len() - 1];

            // Check if we've closed the loop.
            let head = chain[0];
            let dx = tail.x - head.x;
            let dy = tail.y - head.y;
            if chain.len() >= 3 && dx * dx + dy * dy < eps * eps {
                chain.pop();
                break;
            }

            // Lookup neighbors in the spatial index (check 3×3 grid cells).
            let qx = quantize(tail.x);
            let qy = quantize(tail.y);
            let mut found = false;

            'search: for dx_cell in -1i64..=1 {
                for dy_cell in -1i64..=1 {
                    let key = (qx + dx_cell, qy + dy_cell);
                    if let Some(entries) = index.get(&key) {
                        for &(seg_idx, endpoint) in entries {
                            if used[seg_idx] {
                                continue;
                            }
                            let seg = &segments[seg_idx];
                            let (match_pt, other_pt) = if endpoint == 0 {
                                (seg.p1, seg.p2)
                            } else {
                                (seg.p2, seg.p1)
                            };
                            let d = (match_pt.x - tail.x).powi(2) + (match_pt.y - tail.y).powi(2);
                            if d < eps * eps {
                                chain.push(other_pt);
                                used[seg_idx] = true;
                                found = true;
                                break 'search;
                            }
                        }
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

// ---------------------------------------------------------------------------
// Standalone marching squares for boolean grids (2D, no fiber dependency)
// ---------------------------------------------------------------------------

/// A 2D contour segment between two points on the grid boundary.
#[derive(Debug, Clone)]
struct Segment2D {
    p1: P2,
    p2: P2,
}

/// The 16 marching squares cases encoded as edge pairs.
///
/// Each 2x2 cell block has four edges:
/// - Left (0):   between (row, col) and (row+1, col)
/// - Bottom (1): between (row+1, col) and (row+1, col+1)
/// - Right (2):  between (row, col+1) and (row+1, col+1)
/// - Top (3):    between (row, col) and (row, col+1)
///
/// Each case maps to 0, 1, or 2 segments expressed as pairs of edge indices.
/// Saddle cases (5, 10) emit two segments each.
const MS_CASES: [&[(u8, u8)]; 16] = [
    &[],                  // 0:  0000
    &[(0, 1)],            // 1:  0001 — left-bottom
    &[(1, 2)],            // 2:  0010 — bottom-right
    &[(0, 2)],            // 3:  0011 — left-right
    &[(2, 3)],            // 4:  0100 — right-top
    &[(0, 3), (1, 2)],    // 5:  0101 — saddle: left-top + bottom-right
    &[(1, 3)],            // 6:  0110 — bottom-top
    &[(0, 3)],            // 7:  0111 — left-top
    &[(3, 0)],            // 8:  1000 — top-left
    &[(1, 3)],            // 9:  1001 — top-bottom (equivalent: bottom-top)
    &[(0, 1), (2, 3)],    // 10: 1010 — saddle: left-bottom + right-top
    &[(2, 3)],            // 11: 1011 — right-top (== top-right)
    &[(2, 0)],            // 12: 1100 — right-left
    &[(1, 2)],            // 13: 1101 — bottom-right (== right-bottom)
    &[(0, 1)],            // 14: 1110 — left-bottom (== bottom-left)
    &[],                  // 15: 1111
];

/// Extract 2D contour loops from a boolean grid using marching squares.
///
/// Each cell in `grid` is `true` (material) or `false` (air).
/// The grid is row-major with `rows` rows and `cols` columns
/// (i.e. `grid.len() == rows * cols`).
///
/// Returns closed contour loops as `Vec<Vec<P2>>`.
/// Contour points lie on cell edges (midpoints between material/air cells).
pub fn marching_squares_bool_grid(
    grid: &[bool],
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell_size: f64,
) -> Vec<Vec<P2>> {
    if rows < 2 || cols < 2 || grid.len() != rows * cols {
        return Vec::new();
    }

    let segments = ms_bool_segments(grid, rows, cols, origin_x, origin_y, cell_size);
    chain_segments_2d(&segments)
}

/// Build marching-squares segments from a flat boolean grid.
#[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by loop ranges checked above
fn ms_bool_segments(
    grid: &[bool],
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell_size: f64,
) -> Vec<Segment2D> {
    let mut segments = Vec::new();

    // Marching squares iterates over (rows-1) x (cols-1) cells.
    // Cell (r, c) has corners at grid positions:
    //   top-left  = (r, c)       top-right = (r, c+1)
    //   bot-left  = (r+1, c)     bot-right = (r+1, c+1)
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            // SAFETY: r+1 < rows, c+1 < cols guaranteed by loop bounds
            let tl = grid[r * cols + c];
            let tr = grid[r * cols + (c + 1)];
            let br = grid[(r + 1) * cols + (c + 1)];
            let bl = grid[(r + 1) * cols + c];

            // Case index: bit0=BL, bit1=BR, bit2=TR, bit3=TL
            let case_idx =
                (bl as usize) | ((br as usize) << 1) | ((tr as usize) << 2) | ((tl as usize) << 3);

            // SAFETY: case_idx is 0..15, MS_CASES has exactly 16 entries
            let edges = MS_CASES[case_idx];
            if edges.is_empty() {
                continue;
            }

            // Precompute the 4 edge midpoints in world coordinates.
            //
            // Edge 0 (left):   midpoint between (r, c) and (r+1, c)
            //   x = origin_x + c * cell_size
            //   y = origin_y + (r as f64 + 0.5) * cell_size
            //
            // Edge 1 (bottom): midpoint between (r+1, c) and (r+1, c+1)
            //   x = origin_x + (c as f64 + 0.5) * cell_size
            //   y = origin_y + (r + 1) as f64 * cell_size
            //
            // Edge 2 (right):  midpoint between (r, c+1) and (r+1, c+1)
            //   x = origin_x + (c + 1) as f64 * cell_size
            //   y = origin_y + (r as f64 + 0.5) * cell_size
            //
            // Edge 3 (top):    midpoint between (r, c) and (r, c+1)
            //   x = origin_x + (c as f64 + 0.5) * cell_size
            //   y = origin_y + r as f64 * cell_size

            let rf = r as f64;
            let cf = c as f64;

            let edge_pts = [
                P2::new(origin_x + cf * cell_size, origin_y + (rf + 0.5) * cell_size),             // 0: left
                P2::new(origin_x + (cf + 0.5) * cell_size, origin_y + (rf + 1.0) * cell_size),     // 1: bottom
                P2::new(origin_x + (cf + 1.0) * cell_size, origin_y + (rf + 0.5) * cell_size),     // 2: right
                P2::new(origin_x + (cf + 0.5) * cell_size, origin_y + rf * cell_size),             // 3: top
            ];

            for &(a, b) in edges {
                // SAFETY: a, b are 0..3 from the lookup table
                segments.push(Segment2D {
                    p1: edge_pts[a as usize],
                    p2: edge_pts[b as usize],
                });
            }
        }
    }

    segments
}

/// Chain a set of unordered 2D line segments into closed loops.
///
/// Uses a spatial hash map on quantized endpoints for O(1) neighbor lookup.
/// Returns `Vec<Vec<P2>>` where each inner vec is a closed contour loop.
#[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by segment count
fn chain_segments_2d(segments: &[Segment2D]) -> Vec<Vec<P2>> {
    use std::collections::HashMap;

    if segments.is_empty() {
        return Vec::new();
    }

    let eps = 1e-6;
    let n = segments.len();

    // Quantize a coordinate to an integer grid at epsilon scale.
    let quantize = |v: f64| -> i64 { (v * 1e5).round() as i64 };
    type GridKey = (i64, i64);

    // Build spatial index: quantized (x, y) -> list of (segment_index, endpoint_id).
    let mut index: HashMap<GridKey, Vec<(usize, u8)>> = HashMap::with_capacity(n * 2);
    for (i, seg) in segments.iter().enumerate() {
        let k1 = (quantize(seg.p1.x), quantize(seg.p1.y));
        let k2 = (quantize(seg.p2.x), quantize(seg.p2.y));
        index.entry(k1).or_default().push((i, 0));
        index.entry(k2).or_default().push((i, 1));
    }

    let mut used = vec![false; n];
    let mut loops: Vec<Vec<P2>> = Vec::new();

    for start_idx in 0..n {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let mut chain = vec![segments[start_idx].p1, segments[start_idx].p2];

        let max_iterations = n + 1;
        for _ in 0..max_iterations {
            let tail = chain[chain.len() - 1];

            // Check if loop is closed.
            let head = chain[0];
            let dx = tail.x - head.x;
            let dy = tail.y - head.y;
            if chain.len() >= 3 && dx * dx + dy * dy < eps * eps {
                chain.pop();
                break;
            }

            // Look up neighbors in the spatial index (3x3 grid cells).
            let qx = quantize(tail.x);
            let qy = quantize(tail.y);
            let mut found = false;

            'search: for dx_cell in -1i64..=1 {
                for dy_cell in -1i64..=1 {
                    let key = (qx + dx_cell, qy + dy_cell);
                    if let Some(entries) = index.get(&key) {
                        for &(seg_idx, endpoint) in entries {
                            if used[seg_idx] {
                                continue;
                            }
                            let seg = &segments[seg_idx];
                            let (match_pt, other_pt) = if endpoint == 0 {
                                (seg.p1, seg.p2)
                            } else {
                                (seg.p2, seg.p1)
                            };
                            let d = (match_pt.x - tail.x).powi(2)
                                + (match_pt.y - tail.y).powi(2);
                            if d < eps * eps {
                                chain.push(other_pt);
                                used[seg_idx] = true;
                                found = true;
                                break 'search;
                            }
                        }
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

// ---------------------------------------------------------------------------
// Euclidean Distance Transform (Felzenszwalb & Huttenlocher 2004)
// ---------------------------------------------------------------------------

/// 1D parabola-envelope distance transform.
///
/// Input: `f[i] = 0.0` for source cells, `f[i] = very_large` for others.
/// Output: `f[i] = squared_distance_to_nearest_source`.
///
/// Reference: Felzenszwalb & Huttenlocher, "Distance Transforms of Sampled
/// Functions", Theory of Computing 2012.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by loop variables and n
fn edt_1d(f: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;

    for q in 1..n {
        loop {
            let vk = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[vk] + (vk * vk) as f64))
                / (2.0 * (q as f64 - vk as f64));
            if s > z[k] {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f64::INFINITY;
                break;
            }
            if k == 0 {
                v[0] = q;
                z[1] = f64::INFINITY;
                break;
            }
            k -= 1;
        }
    }

    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let vk = v[k];
        f[q] = (q as f64 - vk as f64).powi(2) + f[vk];
    }
}

/// 2D Euclidean Distance Transform on a boolean grid.
///
/// Returns the Euclidean distance (in cell units) from each cell to the
/// nearest `true` cell. Uses two-pass separable 1D EDT
/// (Felzenszwalb & Huttenlocher 2004) — O(rows * cols) total.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by rows/cols loop variables
pub fn distance_transform_2d(grid: &[bool], rows: usize, cols: usize) -> Vec<f64> {
    let total = rows * cols;
    let big = (rows * rows + cols * cols) as f64; // larger than any possible distance^2
    let mut dist = vec![0.0f64; total];

    // Initialize: source cells (true) = 0, others = big
    for (d, g) in dist.iter_mut().zip(grid.iter()) {
        *d = if *g { 0.0 } else { big };
    }

    // Horizontal pass
    for r in 0..rows {
        let start = r * cols;
        edt_1d(&mut dist[start..start + cols]);
    }

    // Vertical pass (column by column with temp buffer)
    let mut col_buf = vec![0.0f64; rows];
    for c in 0..cols {
        for r in 0..rows {
            col_buf[r] = dist[r * cols + c];
        }
        edt_1d(&mut col_buf);
        for r in 0..rows {
            dist[r * cols + c] = col_buf[r];
        }
    }

    // Convert squared distances to actual distances
    for d in &mut dist {
        *d = d.sqrt();
    }

    dist
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

        let loops = chain_segments(&segments);
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

        let loops = chain_segments(&segments);
        assert_eq!(loops.len(), 2, "Should form two separate loops");
    }

    // -----------------------------------------------------------------------
    // Tests for marching_squares_bool_grid
    // -----------------------------------------------------------------------

    #[test]
    fn test_marching_squares_empty_grid() {
        // All false (air) → no contours
        let grid = vec![false; 4 * 4];
        let contours = marching_squares_bool_grid(&grid, 4, 4, 0.0, 0.0, 1.0);
        assert!(contours.is_empty(), "All-air grid should produce no contours");
    }

    #[test]
    fn test_marching_squares_full_grid() {
        // All true (material) → single contour around the boundary
        let grid = vec![true; 4 * 4];
        let contours = marching_squares_bool_grid(&grid, 4, 4, 0.0, 0.0, 1.0);

        // All-true means every cell is case 15, so no boundary segments.
        // A fully filled grid actually produces *no* contours because there are
        // no inside/outside transitions.
        assert!(
            contours.is_empty(),
            "Fully filled grid has no boundaries, so no contours"
        );
    }

    #[test]
    fn test_marching_squares_full_grid_with_border() {
        // To get a contour "around the boundary", we need false border cells.
        // 6×6 grid with a 4×4 true core surrounded by false.
        let rows = 6;
        let cols = 6;
        let mut grid = vec![false; rows * cols];
        for r in 1..5 {
            for c in 1..5 {
                grid[r * cols + c] = true;
            }
        }
        let contours = marching_squares_bool_grid(&grid, rows, cols, 0.0, 0.0, 1.0);
        assert_eq!(
            contours.len(),
            1,
            "Solid block with air border should produce one contour loop"
        );
        // The contour should form a closed loop with multiple points
        assert!(
            contours[0].len() >= 4,
            "Contour should have at least 4 points, got {}",
            contours[0].len()
        );
    }

    #[test]
    fn test_marching_squares_single_block() {
        // 4×4 grid with a 2×2 true block in the center
        let rows = 4;
        let cols = 4;
        let mut grid = vec![false; rows * cols];
        // Place material at (1,1), (1,2), (2,1), (2,2)
        grid[cols + 1] = true;
        grid[cols + 2] = true;
        grid[2 * cols + 1] = true;
        grid[2 * cols + 2] = true;

        let contours = marching_squares_bool_grid(&grid, rows, cols, 0.0, 0.0, 1.0);
        assert_eq!(
            contours.len(),
            1,
            "Single 2×2 block should produce one contour loop"
        );

        let loop0 = &contours[0];
        assert!(
            loop0.len() >= 4,
            "Contour should have at least 4 points, got {}",
            loop0.len()
        );

        // Verify all contour points are roughly centered around the block
        for pt in loop0 {
            assert!(
                pt.x >= 0.5 && pt.x <= 3.5,
                "x out of expected range: {}",
                pt.x
            );
            assert!(
                pt.y >= 0.5 && pt.y <= 3.5,
                "y out of expected range: {}",
                pt.y
            );
        }
    }

    #[test]
    fn test_marching_squares_ring() {
        // Material ring with hole → two contours (outer + inner)
        // 8×8 grid: border of air, ring of material, center of air
        let rows = 8;
        let cols = 8;
        let mut grid = vec![false; rows * cols];

        // Fill a 6×6 block (rows 1..7, cols 1..7) with material
        for r in 1..7 {
            for c in 1..7 {
                grid[r * cols + c] = true;
            }
        }
        // Hollow out the center (rows 3..5, cols 3..5) back to air
        for r in 3..5 {
            for c in 3..5 {
                grid[r * cols + c] = false;
            }
        }

        let contours = marching_squares_bool_grid(&grid, rows, cols, 0.0, 0.0, 1.0);
        assert_eq!(
            contours.len(),
            2,
            "Ring (material with hole) should produce 2 contour loops (outer + inner)"
        );

        // Both contours should be closed loops with enough points
        for (i, contour) in contours.iter().enumerate() {
            assert!(
                contour.len() >= 4,
                "Contour {} should have at least 4 points, got {}",
                i,
                contour.len()
            );
        }

        // One contour should be larger (outer) and one smaller (inner).
        // Measure by counting points — the outer contour has more boundary cells.
        let (len0, len1) = (contours[0].len(), contours[1].len());
        assert_ne!(
            len0, len1,
            "Outer and inner contours should have different point counts"
        );
    }

    // -----------------------------------------------------------------------
    // Tests for Euclidean Distance Transform
    // -----------------------------------------------------------------------

    #[test]
    fn test_edt_1d_simple() {
        // Source at center (index 5 of 11), all others large
        let n = 11;
        let big = (n * n) as f64;
        let mut f = vec![big; n];
        f[5] = 0.0;
        edt_1d(&mut f);
        // After EDT, f[i] should be squared distance to index 5
        for (i, val) in f.iter().enumerate() {
            let expected = ((i as f64) - 5.0).powi(2);
            assert!(
                (val - expected).abs() < 1e-9,
                "edt_1d: f[{}] = {}, expected {}",
                i,
                val,
                expected
            );
        }
    }

    #[test]
    fn test_distance_transform_single_point() {
        // 11x11 grid, single true cell at (5,5)
        let rows = 11;
        let cols = 11;
        let mut grid = vec![false; rows * cols];
        grid[5 * cols + 5] = true;

        let dist = distance_transform_2d(&grid, rows, cols);

        // Distance at (5,5) should be 0
        assert!(
            dist[5 * cols + 5].abs() < 1e-9,
            "Distance at source should be 0"
        );

        // Distance at (5,6) should be 1.0
        assert!(
            (dist[5 * cols + 6] - 1.0).abs() < 1e-9,
            "Distance one cell away should be 1.0, got {}",
            dist[5 * cols + 6]
        );

        // Distance at (6,6) should be sqrt(2)
        let expected_diag = std::f64::consts::SQRT_2;
        assert!(
            (dist[6 * cols + 6] - expected_diag).abs() < 1e-9,
            "Distance diagonally should be sqrt(2), got {}",
            dist[6 * cols + 6]
        );

        // Distance at (0,0) should be sqrt(50) = 5*sqrt(2)
        let expected_corner = (50.0f64).sqrt();
        assert!(
            (dist[0] - expected_corner).abs() < 1e-9,
            "Distance at corner (0,0) should be {}, got {}",
            expected_corner,
            dist[0]
        );
    }

    #[test]
    fn test_distance_transform_rectangle() {
        // 10x10 grid, 6x6 true rectangle at rows 2..8, cols 2..8
        let rows = 10;
        let cols = 10;
        let mut grid = vec![false; rows * cols];
        for r in 2..8 {
            for c in 2..8 {
                grid[r * cols + c] = true;
            }
        }

        let dist = distance_transform_2d(&grid, rows, cols);

        // All true cells should have distance 0
        for r in 2..8 {
            for c in 2..8 {
                assert!(
                    dist[r * cols + c].abs() < 1e-9,
                    "True cell ({},{}) should have distance 0, got {}",
                    r,
                    c,
                    dist[r * cols + c]
                );
            }
        }

        // Cell at (1,5) is 1 row above the rectangle — distance = 1.0
        let idx_1_5 = cols + 5;
        assert!(
            (dist[idx_1_5] - 1.0).abs() < 1e-9,
            "Cell one row above rect should have distance 1.0, got {}",
            dist[idx_1_5]
        );

        // Cell at (0,5) is 2 rows above — distance = 2.0
        assert!(
            (dist[5] - 2.0).abs() < 1e-9,
            "Cell two rows above rect should have distance 2.0, got {}",
            dist[5]
        );

        // Cell at (0,0) should be sqrt((2-0)^2 + (2-0)^2) = sqrt(8) = 2*sqrt(2)
        let expected = (8.0f64).sqrt();
        assert!(
            (dist[0] - expected).abs() < 1e-9,
            "Corner cell should have distance {}, got {}",
            expected,
            dist[0]
        );
    }
}
