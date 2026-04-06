//! Machining boundary and tool containment.
//!
//! Provides boundary offset for tool containment modes (center, inside, outside),
//! toolpath clipping to keep moves within a boundary polygon, and model
//! silhouette extraction for automatic machining boundaries.

use crate::geo::{P2, P3};
use crate::mesh::TriangleMesh;
use crate::polygon::{Polygon2, detect_containment, offset_polygon};
use crate::toolpath::{MoveType, Toolpath};

/// How the tool relates to the machining boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolContainment {
    /// Tool center stays inside boundary.
    Center,
    /// Entire tool stays inside boundary (inset by tool_radius).
    Inside,
    /// Tool edge can extend outside boundary (outset by tool_radius).
    Outside,
}

/// Compute the effective boundary polygon after applying containment offset.
///
/// - `Center`: returns boundary unchanged
/// - `Inside`: shrinks boundary by `tool_radius` so the full cutter stays within
/// - `Outside`: expands boundary by `tool_radius` so the cutter center can reach the edge
///
/// May return multiple polygons if the offset splits the shape, or empty if it collapses.
pub fn effective_boundary(
    boundary: &Polygon2,
    containment: ToolContainment,
    tool_radius: f64,
) -> Vec<Polygon2> {
    match containment {
        ToolContainment::Center => vec![boundary.clone()],
        // cavalier_contours: positive = inward for CCW exterior
        ToolContainment::Inside => offset_polygon(boundary, tool_radius),
        // negative = outward
        ToolContainment::Outside => offset_polygon(boundary, -tool_radius),
    }
}

/// Add rectangular keep-out zones as holes in a boundary polygon.
///
/// Each keep-out polygon's exterior is reversed to CW winding and added as a hole.
/// The resulting polygon excludes the keep-out regions from `contains_point` checks,
/// which means `clip_toolpath_to_boundary` will automatically retract the tool
/// when entering a keep-out area.
pub fn subtract_keepouts(boundary: &Polygon2, keepouts: &[Polygon2]) -> Polygon2 {
    let mut result = boundary.clone();
    for ko in keepouts {
        let mut hole = ko.exterior.clone();
        hole.reverse();
        result.holes.push(hole);
    }
    result
}

/// Clip a toolpath to stay within a boundary polygon.
///
/// Moves whose target is inside the boundary are kept. Moves that cross from
/// inside to outside get a retract to `safe_z` and become rapids. Moves that
/// cross from outside to inside get a rapid to `safe_z` above the target
/// followed by a plunge.
///
/// The first move is treated as if the previous position was outside the boundary
/// (the tool starts from a safe location).
pub fn clip_toolpath_to_boundary(tp: &Toolpath, boundary: &Polygon2, safe_z: f64) -> Toolpath {
    let mut result = Toolpath::new();

    if tp.moves.is_empty() {
        return result;
    }

    let mut prev_inside = false;
    let mut prev_pos: Option<P3> = None;

    for m in &tp.moves {
        let target_xy = P2::new(m.target.x, m.target.y);
        let cur_inside = boundary.contains_point(&target_xy);

        match (prev_inside, cur_inside) {
            (_, true) if prev_pos.is_none() => {
                // First move, target is inside. Keep it (may be a rapid approach).
                result.moves.push(m.clone());
            }
            (false, true) => {
                // Crossing from outside to inside: rapid above target, then plunge.
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));

                // Preserve feed rate from the original move for the plunge.
                let feed = feed_rate_of(&m.move_type);
                match feed {
                    Some(fr) => result.feed_to(m.target, fr),
                    None => result.rapid_to(m.target),
                }
            }
            (true, true) => {
                // Both inside: keep the move unchanged.
                result.moves.push(m.clone());
            }
            (true, false) => {
                // Crossing from inside to outside: retract, then rapid.
                if let Some(prev) = prev_pos {
                    result.rapid_to(P3::new(prev.x, prev.y, safe_z));
                }
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));
            }
            (false, false) => {
                // Both outside (or first move with outside target):
                // convert to rapid at safe_z.
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));
            }
        }

        prev_inside = cur_inside;
        prev_pos = Some(m.target);
    }

    result
}

/// Extract the feed rate from a move type, if it has one.
fn feed_rate_of(mt: &MoveType) -> Option<f64> {
    match mt {
        MoveType::Rapid => None,
        MoveType::Linear { feed_rate } => Some(*feed_rate),
        MoveType::ArcCW { feed_rate, .. } => Some(*feed_rate),
        MoveType::ArcCCW { feed_rate, .. } => Some(*feed_rate),
    }
}

/// Default silhouette grid resolution in mm.
const SILHOUETTE_CELL_SIZE: f64 = 0.5;

/// Compute the 2D silhouette (XY projection) of a 3D mesh.
///
/// Projects all mesh triangles onto the XY plane, rasterizes them onto a
/// boolean grid, then extracts the outline via marching squares.  Returns
/// one or more `Polygon2` with correct winding (outer boundaries CCW,
/// holes CW via `detect_containment`).
///
/// `cell_size` controls grid resolution in mm (smaller = more detail, slower).
/// Pass `None` for the default (0.5 mm).
#[allow(clippy::indexing_slicing)] // bounded by grid dimensions computed from mesh bbox
pub fn model_silhouette(mesh: &TriangleMesh, cell_size: Option<f64>) -> Vec<Polygon2> {
    let cell = cell_size.unwrap_or(SILHOUETTE_CELL_SIZE);
    let bbox = &mesh.bbox;

    // Grid dimensions — add 1-cell margin so contours don't touch edges.
    let x_min = bbox.min.x - cell;
    let y_min = bbox.min.y - cell;
    let x_max = bbox.max.x + cell;
    let y_max = bbox.max.y + cell;

    let nx = ((x_max - x_min) / cell).ceil() as usize + 1;
    let ny = ((y_max - y_min) / cell).ceil() as usize + 1;

    // Flat boolean grid (row-major: grid[row * nx + col]).
    let mut grid = vec![false; ny * nx];

    // Rasterize each triangle's XY projection via scanline fill.
    for face in &mesh.faces {
        rasterize_triangle_xy(&face.v, &mut grid, nx, ny, x_min, y_min, cell);
    }

    // Extract contour loops via marching squares.
    let loops = marching_squares_grid(&grid, nx, ny, x_min, y_min, cell);

    // Convert to Polygon2, fix winding, and nest inner contours as holes.
    let polygons: Vec<Polygon2> = loops
        .into_iter()
        .map(|pts| {
            let mut p = Polygon2::new(pts);
            p.ensure_winding();
            p
        })
        .collect();
    detect_containment(polygons)
}

/// Rasterize one triangle's XY projection onto the boolean grid (scanline).
#[allow(clippy::indexing_slicing)]
fn rasterize_triangle_xy(
    v: &[P3; 3],
    grid: &mut [bool],
    nx: usize,
    ny: usize,
    x_min: f64,
    y_min: f64,
    cell: f64,
) {
    // Project to 2D grid coordinates (floating point).
    let gx = [
        (v[0].x - x_min) / cell,
        (v[1].x - x_min) / cell,
        (v[2].x - x_min) / cell,
    ];
    let gy = [
        (v[0].y - y_min) / cell,
        (v[1].y - y_min) / cell,
        (v[2].y - y_min) / cell,
    ];

    // Bounding rows for the triangle.
    let row_lo = (gy[0].min(gy[1]).min(gy[2]).floor() as usize).min(ny.saturating_sub(1));
    let row_hi = (gy[0].max(gy[1]).max(gy[2]).ceil() as usize).min(ny.saturating_sub(1));

    for row in row_lo..=row_hi {
        let y = row as f64 + 0.5; // cell centre

        // Find X-intersections of the scanline with each triangle edge.
        let mut xs = Vec::with_capacity(2);
        for edge in &[[0, 1], [1, 2], [2, 0]] {
            let y0 = gy[edge[0]];
            let y1 = gy[edge[1]];
            if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                let t = (y - y0) / (y1 - y0);
                xs.push(gx[edge[0]] + t * (gx[edge[1]] - gx[edge[0]]));
            }
        }

        if xs.len() < 2 {
            // Degenerate (scanline touches vertex) — fill the vertex cell.
            if let Some(&x_val) = xs.first() {
                let col = (x_val as usize).min(nx.saturating_sub(1));
                grid[row * nx + col] = true;
            }
            continue;
        }

        // Sort and fill between the two X intersections.
        let (xl, xr) = if xs[0] < xs[1] {
            (xs[0], xs[1])
        } else {
            (xs[1], xs[0])
        };
        let col_lo = (xl.floor() as usize).min(nx.saturating_sub(1));
        let col_hi = (xr.ceil() as usize).min(nx.saturating_sub(1));
        for col in col_lo..=col_hi {
            grid[row * nx + col] = true;
        }
    }
}

/// Marching squares on a boolean grid, producing closed contour loops
/// in world XY coordinates.
///
/// Edge keys are canonicalized so adjacent cells sharing an edge use the
/// same key, enabling correct segment chaining.
///
/// Canonical edge keys `(row, col, orientation)`:
/// - Horizontal edge (0): midpoint of the edge between nodes (row,col) and (row,col+1)
/// - Vertical edge (1): midpoint of the edge between nodes (row,col) and (row+1,col)
#[allow(clippy::indexing_slicing)]
fn marching_squares_grid(
    grid: &[bool],
    nx: usize,
    ny: usize,
    x_min: f64,
    y_min: f64,
    cell: f64,
) -> Vec<Vec<P2>> {
    if nx < 2 || ny < 2 {
        return Vec::new();
    }

    // Canonical edge key: (row, col, orientation).
    //   orientation 0 = horizontal (between (row,col) and (row,col+1))
    //   orientation 1 = vertical   (between (row,col) and (row+1,col))
    type EdgeKey = (usize, usize, u8);
    use std::collections::HashMap;

    let val = |row: usize, col: usize| -> bool { grid[row * nx + col] };

    let mut segments: Vec<(EdgeKey, EdgeKey)> = Vec::new();

    let rows = ny - 1;
    let cols = nx - 1;

    for row in 0..rows {
        for col in 0..cols {
            // Corners: bottom-left, bottom-right, top-right, top-left
            let bl = val(row, col) as u8;
            let br = val(row, col + 1) as u8;
            let tr = val(row + 1, col + 1) as u8;
            let tl = val(row + 1, col) as u8;
            let case = bl | (br << 1) | (tr << 2) | (tl << 3);

            // Canonical edge keys for this cell (row, col):
            let bottom: EdgeKey = (row, col, 0); // h-edge at row, between col and col+1
            let right: EdgeKey = (row, col + 1, 1); // v-edge at col+1, between row and row+1
            let top: EdgeKey = (row + 1, col, 0); // h-edge at row+1, between col and col+1
            let left: EdgeKey = (row, col, 1); // v-edge at col, between row and row+1

            match case {
                0 | 15 => {}
                1 | 14 => segments.push((bottom, left)),
                2 | 13 => segments.push((right, bottom)),
                3 | 12 => segments.push((right, left)),
                4 | 11 => segments.push((top, right)),
                6 | 9 => segments.push((top, bottom)),
                7 | 8 => segments.push((top, left)),
                5 => {
                    segments.push((bottom, left));
                    segments.push((top, right));
                }
                10 => {
                    segments.push((right, bottom));
                    segments.push((top, left));
                }
                _ => {}
            }
        }
    }

    // Build adjacency map.
    let mut adj: HashMap<EdgeKey, Vec<EdgeKey>> = HashMap::new();
    for (a, b) in &segments {
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }

    let mut used_edges: HashMap<EdgeKey, Vec<bool>> = HashMap::new();
    for (k, v) in &adj {
        used_edges.insert(*k, vec![false; v.len()]);
    }

    // Convert canonical edge key to world coordinate (midpoint of edge).
    let edge_pos = |key: &EdgeKey| -> P2 {
        let (row, col, orient) = *key;
        match orient {
            0 => {
                // Horizontal edge at row, between col and col+1
                P2::new(x_min + (col as f64 + 0.5) * cell, y_min + row as f64 * cell)
            }
            _ => {
                // Vertical edge at col, between row and row+1
                P2::new(x_min + col as f64 * cell, y_min + (row as f64 + 0.5) * cell)
            }
        }
    };

    let mut loops: Vec<Vec<P2>> = Vec::new();

    for start_key in adj.keys().copied().collect::<Vec<_>>() {
        let Some(flags) = used_edges.get(&start_key) else {
            continue;
        };
        let Some(start_idx) = flags.iter().position(|&u| !u) else {
            continue;
        };

        let mut contour = vec![edge_pos(&start_key)];
        let mut cur = start_key;
        let mut cur_idx = start_idx;

        loop {
            if let Some(flags) = used_edges.get_mut(&cur)
                && let Some(f) = flags.get_mut(cur_idx)
            {
                *f = true;
            }

            let Some(neighbors) = adj.get(&cur) else {
                break;
            };
            let Some(&next) = neighbors.get(cur_idx) else {
                break;
            };

            // Mark reverse direction used.
            if let Some(neighbors) = adj.get(&next)
                && let Some(idx) = neighbors.iter().position(|k| *k == cur)
                && let Some(flags) = used_edges.get_mut(&next)
                && let Some(f) = flags.get_mut(idx)
            {
                *f = true;
            }

            if next == start_key {
                break; // closed loop
            }

            contour.push(edge_pos(&next));

            let Some(flags) = used_edges.get(&next) else {
                break;
            };
            let Some(next_idx) = flags.iter().position(|&u| !u) else {
                break;
            };
            cur = next;
            cur_idx = next_idx;
        }

        if contour.len() >= 3 {
            loops.push(contour);
        }
    }

    loops
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::toolpath::Move;

    /// Ray-casting point-in-polygon test for a single ring (test helper).
    fn point_in_ring(px: f64, py: f64, ring: &[P2]) -> bool {
        let n = ring.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let pi = &ring[i];
            let pj = &ring[j];
            if ((pi.y > py) != (pj.y > py))
                && (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Full point-in-polygon test: inside exterior and outside all holes (test helper).
    fn point_in_polygon(px: f64, py: f64, polygon: &Polygon2) -> bool {
        if !point_in_ring(px, py, &polygon.exterior) {
            return false;
        }
        !polygon.holes.iter().any(|h| point_in_ring(px, py, h))
    }

    /// Helper: build a CCW square boundary centered at origin.
    fn square_boundary(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    // --- effective_boundary tests ---

    #[test]
    fn test_effective_boundary_center_unchanged() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Center, 3.0);
        assert_eq!(result.len(), 1);
        assert!(
            (result[0].area() - boundary.area()).abs() < 1e-10,
            "Center containment should not change the boundary"
        );
    }

    #[test]
    fn test_effective_boundary_inside_shrinks() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Inside, 2.0);
        assert!(!result.is_empty(), "Inside offset should produce a polygon");

        let total_area: f64 = result.iter().map(|p| p.area()).sum();
        // Original is 400, inset by 2 should be roughly (20-4)^2 = 256 (with rounded corners slightly more)
        assert!(
            total_area < boundary.area(),
            "Inside containment should shrink: got {} vs original {}",
            total_area,
            boundary.area()
        );
        assert!(
            total_area > 200.0,
            "Shrunk area {} is unexpectedly small",
            total_area
        );
    }

    #[test]
    fn test_effective_boundary_outside_expands() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Outside, 2.0);
        assert!(
            !result.is_empty(),
            "Outside offset should produce a polygon"
        );

        let total_area: f64 = result.iter().map(|p| p.area()).sum();
        assert!(
            total_area > boundary.area(),
            "Outside containment should expand: got {} vs original {}",
            total_area,
            boundary.area()
        );
    }

    #[test]
    fn test_effective_boundary_inside_collapse() {
        // A 4x4 square with tool_radius=3 should collapse (inset by 3 exceeds half-width of 2)
        let boundary = square_boundary(4.0);
        let result = effective_boundary(&boundary, ToolContainment::Inside, 3.0);
        assert!(
            result.is_empty(),
            "Inside containment that exceeds half-width should collapse"
        );
    }

    // --- clip_toolpath_to_boundary tests ---

    #[test]
    fn test_clip_keeps_moves_inside() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 10.0, 20.0));
        tp.feed_to(P3::new(50.0, 50.0, -5.0), 1000.0);
        tp.feed_to(P3::new(90.0, 50.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, 20.0);

        // All targets are inside the boundary, so all moves should be preserved
        assert_eq!(clipped.moves.len(), tp.moves.len());
        for (orig, clip) in tp.moves.iter().zip(clipped.moves.iter()) {
            assert!(
                (orig.target.x - clip.target.x).abs() < 1e-10
                    && (orig.target.y - clip.target.y).abs() < 1e-10
                    && (orig.target.z - clip.target.z).abs() < 1e-10,
                "Inside moves should be preserved exactly"
            );
        }
    }

    #[test]
    fn test_clip_converts_outside_to_rapids() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let safe_z = 20.0;

        let mut tp = Toolpath::new();
        // Start inside
        tp.rapid_to(P3::new(25.0, 25.0, safe_z));
        tp.feed_to(P3::new(25.0, 25.0, -5.0), 1000.0);
        // Move outside
        tp.feed_to(P3::new(75.0, 25.0, -5.0), 1000.0);
        // Move back inside
        tp.feed_to(P3::new(25.0, 25.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // Check that the outside move (target 75,25) became a rapid at safe_z
        let outside_moves: Vec<&Move> = clipped
            .moves
            .iter()
            .filter(|m| (m.target.x - 75.0).abs() < 1e-10)
            .collect();
        assert!(
            !outside_moves.is_empty(),
            "Should have a move targeting the outside position"
        );
        for m in &outside_moves {
            assert_eq!(
                m.move_type,
                MoveType::Rapid,
                "Outside move should be converted to rapid"
            );
            assert!(
                (m.target.z - safe_z).abs() < 1e-10,
                "Outside move should be at safe_z, got z={}",
                m.target.z
            );
        }
    }

    #[test]
    fn test_clip_empty_toolpath() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let tp = Toolpath::new();
        let clipped = clip_toolpath_to_boundary(&tp, &boundary, 20.0);
        assert!(clipped.moves.is_empty());
    }

    #[test]
    fn test_clip_all_outside() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let safe_z = 20.0;

        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, -5.0), 1000.0);
        tp.feed_to(P3::new(60.0, 50.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // All moves should be rapids at safe_z
        for m in &clipped.moves {
            assert_eq!(m.move_type, MoveType::Rapid);
            assert!(
                (m.target.z - safe_z).abs() < 1e-10,
                "All-outside moves should be at safe_z"
            );
        }
    }

    #[test]
    fn test_clip_reentry_plunges() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let safe_z = 20.0;
        let feed = 1000.0;

        let mut tp = Toolpath::new();
        // Approach from outside
        tp.rapid_to(P3::new(-10.0, 25.0, safe_z));
        // Enter the boundary
        tp.feed_to(P3::new(25.0, 25.0, -5.0), feed);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // After re-entry, we should see a rapid to safe_z above (25,25) then a plunge
        let has_rapid_above_target = clipped.moves.iter().any(|m| {
            m.move_type == MoveType::Rapid
                && (m.target.x - 25.0).abs() < 1e-10
                && (m.target.y - 25.0).abs() < 1e-10
                && (m.target.z - safe_z).abs() < 1e-10
        });
        assert!(
            has_rapid_above_target,
            "Re-entry should produce a rapid to safe_z above the target"
        );

        let has_plunge = clipped.moves.iter().any(|m| {
            matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - feed).abs() < 1e-10)
                && (m.target.x - 25.0).abs() < 1e-10
                && (m.target.z - (-5.0)).abs() < 1e-10
        });
        assert!(
            has_plunge,
            "Re-entry should produce a plunge to cutting depth"
        );
    }

    // --- point_in_polygon helper tests ---

    #[test]
    fn test_point_in_polygon_inside() {
        let sq = square_boundary(10.0);
        assert!(point_in_polygon(0.0, 0.0, &sq));
        assert!(point_in_polygon(4.0, 4.0, &sq));
    }

    #[test]
    fn test_point_in_polygon_outside() {
        let sq = square_boundary(10.0);
        assert!(!point_in_polygon(10.0, 10.0, &sq));
        assert!(!point_in_polygon(-6.0, 0.0, &sq));
    }

    #[test]
    fn test_point_in_polygon_with_hole() {
        // 20x20 square with 6x6 hole in center
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(square_boundary(20.0).exterior, vec![hole]);
        // Inside exterior but outside hole
        assert!(point_in_polygon(8.0, 0.0, &poly));
        // Inside the hole
        assert!(!point_in_polygon(0.0, 0.0, &poly));
    }

    #[test]
    fn subtract_keepouts_adds_holes() {
        let boundary = Polygon2::rectangle(-50.0, -50.0, 50.0, 50.0);
        let keepout = Polygon2::rectangle(10.0, 10.0, 20.0, 20.0);

        let result = subtract_keepouts(&boundary, &[keepout]);
        assert_eq!(result.holes.len(), 1);

        assert!(result.contains_point(&P2::new(0.0, 0.0)));
        assert!(!result.contains_point(&P2::new(15.0, 15.0)));
    }

    #[test]
    fn subtract_keepouts_multiple() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let ko1 = Polygon2::rectangle(10.0, 10.0, 20.0, 20.0);
        let ko2 = Polygon2::rectangle(70.0, 70.0, 90.0, 90.0);

        let result = subtract_keepouts(&boundary, &[ko1, ko2]);
        assert_eq!(result.holes.len(), 2);

        assert!(result.contains_point(&P2::new(50.0, 50.0)));
        assert!(!result.contains_point(&P2::new(15.0, 15.0)));
        assert!(!result.contains_point(&P2::new(80.0, 80.0)));
    }

    #[test]
    fn subtract_keepouts_empty_is_noop() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let result = subtract_keepouts(&boundary, &[]);
        assert_eq!(result.holes.len(), 0);
        assert!(result.contains_point(&P2::new(50.0, 50.0)));
    }

    // --- model_silhouette tests ---

    #[test]
    fn silhouette_flat_square() {
        // A 40×40 flat quad at Z=0 centred at origin should produce a
        // silhouette polygon covering roughly that area.
        let mesh = crate::mesh::make_test_flat(40.0);
        let polys = model_silhouette(&mesh, Some(1.0));

        assert!(!polys.is_empty(), "Silhouette should produce polygons");
        let total_area: f64 = polys.iter().map(|p| p.area()).sum();
        // Expect roughly 40×40 = 1600, with some rasterization error.
        assert!(
            total_area > 1200.0 && total_area < 2000.0,
            "Silhouette area {} should be near 1600",
            total_area
        );

        // Centre of mesh should be inside the silhouette.
        assert!(
            polys.iter().any(|p| p.contains_point(&P2::new(0.0, 0.0))),
            "Centre should be inside silhouette"
        );
        // Well outside the mesh should not be inside.
        assert!(
            !polys.iter().any(|p| p.contains_point(&P2::new(50.0, 50.0))),
            "Point outside mesh should be outside silhouette"
        );
    }

    #[test]
    fn silhouette_hemisphere() {
        // A hemisphere of radius 10 should have silhouette area ≈ π·10² ≈ 314.
        let mesh = crate::mesh::make_test_hemisphere(10.0, 8);
        let polys = model_silhouette(&mesh, Some(0.5));

        assert!(!polys.is_empty(), "Hemisphere should produce silhouette");
        let total_area: f64 = polys.iter().map(|p| p.area()).sum();
        assert!(
            total_area > 200.0 && total_area < 450.0,
            "Hemisphere silhouette area {} should be near π·100 ≈ 314",
            total_area
        );

        // Centre should be inside.
        assert!(polys.iter().any(|p| p.contains_point(&P2::new(0.0, 0.0))));
    }

    #[test]
    fn silhouette_winding_correct() {
        // Verify that silhouette output has correct winding
        // (exteriors CCW = positive signed area after detect_containment).
        let mesh = crate::mesh::make_test_flat(40.0);
        let polys = model_silhouette(&mesh, Some(1.0));
        assert!(!polys.is_empty());

        for p in &polys {
            assert!(
                p.has_correct_winding(),
                "Polygon should have correct winding"
            );
        }
    }
}
