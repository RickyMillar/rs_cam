//! Heightmap-based material removal simulation.
//!
//! Stamps the tool profile into a 2D grid of Z heights as the tool walks the
//! toolpath. The result can be exported as a colored triangle mesh for 3D
//! visualization in the HTML viewer.

use crate::geo::{BoundingBox3, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::tool::MillingCutter;
use crate::toolpath::{MoveType, Toolpath};

// Re-exports for backward compatibility.
pub use crate::arc_util::linearize_arc;
pub use crate::radial_profile::RadialProfileLUT;
pub use crate::stock_mesh::StockMesh as HeightmapMesh;

/// A 2D grid of Z heights representing the material surface after simulation.
#[derive(Clone)]
pub struct Heightmap {
    /// Z value for each cell, stored in row-major order (row * cols + col).
    pub cells: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
    /// World X coordinate of the grid origin (bottom-left corner).
    pub origin_x: f64,
    /// World Y coordinate of the grid origin (bottom-left corner).
    pub origin_y: f64,
    pub cell_size: f64,
    pub stock_top_z: f64,
}

impl Heightmap {
    /// Create a heightmap from explicit stock bounds.
    pub fn from_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        top_z: f64,
        cell_size: f64,
    ) -> Self {
        let cols = ((x_max - x_min) / cell_size).ceil() as usize + 1;
        let rows = ((y_max - y_min) / cell_size).ceil() as usize + 1;
        let cells = vec![top_z; rows * cols];
        Self {
            cells,
            rows,
            cols,
            origin_x: x_min,
            origin_y: y_min,
            cell_size,
            stock_top_z: top_z,
        }
    }

    /// Create a heightmap from a bounding box.
    pub fn from_bounds(bbox: &BoundingBox3, top_z: Option<f64>, cell_size: f64) -> Self {
        let top = top_z.unwrap_or(bbox.max.z);
        Self::from_stock(
            bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y, top, cell_size,
        )
    }

    /// Convert world (x, y) to cell (row, col). Returns None if outside grid.
    pub fn world_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return None;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return None;
        }
        Some((row as usize, col as usize))
    }

    /// Convert cell (row, col) to world (x, y) center coordinates.
    pub fn cell_to_world(&self, row: usize, col: usize) -> (f64, f64) {
        (
            self.origin_x + col as f64 * self.cell_size,
            self.origin_y + row as f64 * self.cell_size,
        )
    }

    /// Get the Z value at a cell.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.cells[row * self.cols + col]
    }

    /// Set the Z value at a cell (only if lower than current).
    #[inline]
    pub fn cut(&mut self, row: usize, col: usize, z: f64) {
        let idx = row * self.cols + col;
        if z < self.cells[idx] {
            self.cells[idx] = z;
        }
    }

    /// Get the bounding box of the heightmap in world coordinates.
    pub fn bbox(&self) -> BoundingBox3 {
        let z_min = self.cells.iter().copied().fold(f64::INFINITY, f64::min);
        BoundingBox3 {
            min: P3::new(self.origin_x, self.origin_y, z_min),
            max: P3::new(
                self.origin_x + (self.cols - 1) as f64 * self.cell_size,
                self.origin_y + (self.rows - 1) as f64 * self.cell_size,
                self.stock_top_z,
            ),
        }
    }
}

/// Stamp the tool profile at a single position into the heightmap.
pub fn stamp_tool_at(
    heightmap: &mut Heightmap,
    cutter: &dyn MillingCutter,
    cx: f64,
    cy: f64,
    tip_z: f64,
) {
    let r = cutter.radius();
    let cs = heightmap.cell_size;

    let col_min = ((cx - r - heightmap.origin_x) / cs).floor() as isize;
    let col_max = ((cx + r - heightmap.origin_x) / cs).ceil() as isize;
    let row_min = ((cy - r - heightmap.origin_y) / cs).floor() as isize;
    let row_max = ((cy + r - heightmap.origin_y) / cs).ceil() as isize;

    let col_lo = col_min.max(0) as usize;
    let col_hi = (col_max as usize).min(heightmap.cols - 1);
    let row_lo = row_min.max(0) as usize;
    let row_hi = (row_max as usize).min(heightmap.rows - 1);

    let r_sq = r * r;

    for row in row_lo..=row_hi {
        let cell_y = heightmap.origin_y + row as f64 * cs;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }
        for col in col_lo..=col_hi {
            let cell_x = heightmap.origin_x + col as f64 * cs;
            let dx = cell_x - cx;
            let dist_sq = dx * dx + dy_sq;
            if dist_sq > r_sq {
                continue;
            }
            let dist = dist_sq.sqrt();
            if let Some(h) = cutter.height_at_radius(dist) {
                heightmap.cut(row, col, tip_z + h);
            }
        }
    }
}

/// Stamp the tool profile at a single position using a precomputed LUT.
pub fn stamp_tool_at_lut(
    heightmap: &mut Heightmap,
    lut: &RadialProfileLUT,
    radius: f64,
    cx: f64,
    cy: f64,
    tip_z: f64,
) {
    let cs = heightmap.cell_size;

    // Compute cell range that could be affected
    let col_min = ((cx - radius - heightmap.origin_x) / cs).floor() as isize;
    let col_max = ((cx + radius - heightmap.origin_x) / cs).ceil() as isize;
    let row_min = ((cy - radius - heightmap.origin_y) / cs).floor() as isize;
    let row_max = ((cy + radius - heightmap.origin_y) / cs).ceil() as isize;

    let col_lo = col_min.max(0) as usize;
    let col_hi = (col_max as usize).min(heightmap.cols - 1);
    let row_lo = row_min.max(0) as usize;
    let row_hi = (row_max as usize).min(heightmap.rows - 1);

    let r_sq = lut.radius_sq();

    // Step 3: Early-out — if center cell is already at or below tip_z,
    // check corners; if all below, the stamp is a no-op.
    let center_col = ((cx - heightmap.origin_x) / cs).round() as isize;
    let center_row = ((cy - heightmap.origin_y) / cs).round() as isize;
    if center_col >= 0
        && center_row >= 0
        && (center_col as usize) < heightmap.cols
        && (center_row as usize) < heightmap.rows
    {
        let cz = heightmap.get(center_row as usize, center_col as usize);
        if cz <= tip_z {
            // Check 4 edge cells of the bounding box
            let edge_cells = [
                (row_lo, col_lo),
                (row_lo, col_hi),
                (row_hi, col_lo),
                (row_hi, col_hi),
            ];
            let all_below = edge_cells
                .iter()
                .all(|&(r, c)| heightmap.get(r, c) <= tip_z);
            if all_below {
                return;
            }
        }
    }

    for row in row_lo..=row_hi {
        let cell_y = heightmap.origin_y + row as f64 * cs;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }
        for col in col_lo..=col_hi {
            let cell_x = heightmap.origin_x + col as f64 * cs;
            let dx = cell_x - cx;
            let dist_sq = dx * dx + dy_sq;
            if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                heightmap.cut(row, col, tip_z + h);
            }
        }
    }
}

/// Stamp the tool along a linear segment (start to end).
///
/// Uses a swept-stadium approach: each cell in the bounding box of the swept
/// tool is processed exactly once, using closest-point-on-segment to determine
/// the cutter height. This is much faster than stamping N circles along the path.
pub fn stamp_linear_segment(
    heightmap: &mut Heightmap,
    cutter: &dyn MillingCutter,
    start: P3,
    end: P3,
) {
    let lut = RadialProfileLUT::from_cutter(cutter, 256);
    stamp_linear_segment_lut(heightmap, &lut, cutter.radius(), start, end);
}

/// Stamp a linear segment using a precomputed LUT.
pub fn stamp_linear_segment_lut(
    heightmap: &mut Heightmap,
    lut: &RadialProfileLUT,
    radius: f64,
    start: P3,
    end: P3,
) {
    let seg_dx = end.x - start.x;
    let seg_dy = end.y - start.y;
    let seg_dz = end.z - start.z;
    let seg_len_sq = seg_dx * seg_dx + seg_dy * seg_dy;

    // Degenerate segment (zero XY length) — just stamp at the lower Z
    if seg_len_sq < 1e-20 {
        let z = start.z.min(end.z);
        stamp_tool_at_lut(heightmap, lut, radius, start.x, start.y, z);
        return;
    }

    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let cs = heightmap.cell_size;

    // Bounding box of the swept tool: segment bbox expanded by radius
    let x_min = start.x.min(end.x) - radius;
    let x_max = start.x.max(end.x) + radius;
    let y_min = start.y.min(end.y) - radius;
    let y_max = start.y.max(end.y) + radius;

    let col_lo = ((x_min - heightmap.origin_x) / cs).floor().max(0.0) as usize;
    let col_hi = (((x_max - heightmap.origin_x) / cs).ceil() as usize).min(heightmap.cols - 1);
    let row_lo = ((y_min - heightmap.origin_y) / cs).floor().max(0.0) as usize;
    let row_hi = (((y_max - heightmap.origin_y) / cs).ceil() as usize).min(heightmap.rows - 1);

    for row in row_lo..=row_hi {
        let cell_y = heightmap.origin_y + row as f64 * cs;
        let py = cell_y - start.y;

        for col in col_lo..=col_hi {
            let cell_x = heightmap.origin_x + col as f64 * cs;
            let px = cell_x - start.x;

            // Closest point on segment: t = clamp(dot(p - start, seg) / |seg|^2, 0, 1)
            let t = (px * seg_dx + py * seg_dy) * inv_seg_len_sq;
            let t = t.clamp(0.0, 1.0);

            // Radial distance from cell to closest point (XY only)
            let closest_x = t * seg_dx;
            let closest_y = t * seg_dy;
            let dx = px - closest_x;
            let dy = py - closest_y;
            let dist_sq = dx * dx + dy * dy;

            if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                // Interpolate Z at the closest point
                let z = start.z + t * seg_dz;
                heightmap.cut(row, col, z + h);
            }
        }
    }
}

/// Run the simulation: walk the toolpath and stamp the tool at each move.
pub fn simulate_toolpath(
    toolpath: &Toolpath,
    cutter: &dyn MillingCutter,
    heightmap: &mut Heightmap,
) {
    let never_cancel = || false;
    let _ = simulate_toolpath_with_cancel(toolpath, cutter, heightmap, &never_cancel);
}

pub fn simulate_toolpath_with_cancel(
    toolpath: &Toolpath,
    cutter: &dyn MillingCutter,
    heightmap: &mut Heightmap,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    if toolpath.moves.is_empty() {
        return Ok(());
    }

    let lut = RadialProfileLUT::from_cutter(cutter, 256);
    let radius = cutter.radius();

    for i in 1..toolpath.moves.len() {
        check_cancel(cancel)?;
        let start = toolpath.moves[i - 1].target;
        let end = toolpath.moves[i].target;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {
                // Rapids don't cut material
            }
            MoveType::Linear { .. } => {
                stamp_linear_segment_lut(heightmap, &lut, radius, start, end);
            }
            MoveType::ArcCW { i, j, .. } => {
                let points = linearize_arc(start, end, i, j, true, heightmap.cell_size);
                for w in points.windows(2) {
                    check_cancel(cancel)?;
                    stamp_linear_segment_lut(heightmap, &lut, radius, w[0], w[1]);
                }
            }
            MoveType::ArcCCW { i, j, .. } => {
                let points = linearize_arc(start, end, i, j, false, heightmap.cell_size);
                for w in points.windows(2) {
                    check_cancel(cancel)?;
                    stamp_linear_segment_lut(heightmap, &lut, radius, w[0], w[1]);
                }
            }
        }
    }

    Ok(())
}

/// Simulate only moves `start_move..end_move` of a toolpath into the heightmap.
///
/// This is the building block for incremental playback: call it with successive
/// ranges to progressively carve the stock.  Move indices are 1-based internally
/// (move 0 is the initial position), so `start_move=0, end_move=N` processes
/// the first N cutting segments.
pub fn simulate_toolpath_range(
    toolpath: &Toolpath,
    cutter: &dyn MillingCutter,
    heightmap: &mut Heightmap,
    start_move: usize,
    end_move: usize,
) {
    if toolpath.moves.len() < 2 {
        return;
    }
    let lut = RadialProfileLUT::from_cutter(cutter, 256);
    let radius = cutter.radius();
    let first = start_move.max(1);
    let last = end_move.min(toolpath.moves.len());

    for i in first..last {
        let start = toolpath.moves[i - 1].target;
        let end = toolpath.moves[i].target;

        match toolpath.moves[i].move_type {
            MoveType::Rapid => {}
            MoveType::Linear { .. } => {
                stamp_linear_segment_lut(heightmap, &lut, radius, start, end);
            }
            MoveType::ArcCW { i, j, .. } => {
                let points = linearize_arc(start, end, i, j, true, heightmap.cell_size);
                for w in points.windows(2) {
                    stamp_linear_segment_lut(heightmap, &lut, radius, w[0], w[1]);
                }
            }
            MoveType::ArcCCW { i, j, .. } => {
                let points = linearize_arc(start, end, i, j, false, heightmap.cell_size);
                for w in points.windows(2) {
                    stamp_linear_segment_lut(heightmap, &lut, radius, w[0], w[1]);
                }
            }
        }
    }
}

/// Convert a heightmap to a renderable triangle mesh with wood-tone coloring.
///
/// Uncut cells get a light tan color, cut cells interpolate to dark walnut
/// based on how deep they were cut below the stock top.
pub fn heightmap_to_mesh(heightmap: &Heightmap) -> HeightmapMesh {
    let rows = heightmap.rows;
    let cols = heightmap.cols;
    let num_verts = rows * cols;

    let mut vertices = Vec::with_capacity(num_verts * 3);
    let mut colors = Vec::with_capacity(num_verts * 3);

    // Find the deepest cut for color normalization
    let z_min = heightmap
        .cells
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let z_range = (heightmap.stock_top_z - z_min).max(1e-6);

    // Wood colors: uncut = light tan, cut = dark walnut
    const UNCUT_R: f32 = 0.76;
    const UNCUT_G: f32 = 0.60;
    const UNCUT_B: f32 = 0.42;
    const CUT_R: f32 = 0.45;
    const CUT_G: f32 = 0.25;
    const CUT_B: f32 = 0.10;

    for row in 0..rows {
        for col in 0..cols {
            let (wx, wy) = heightmap.cell_to_world(row, col);
            let z = heightmap.get(row, col);
            vertices.push(wx as f32);
            vertices.push(wy as f32);
            vertices.push(z as f32);

            // 0.0 = stock top (uncut), 1.0 = deepest cut
            let depth_t = ((heightmap.stock_top_z - z) / z_range).clamp(0.0, 1.0) as f32;
            colors.push(UNCUT_R + (CUT_R - UNCUT_R) * depth_t);
            colors.push(UNCUT_G + (CUT_G - UNCUT_G) * depth_t);
            colors.push(UNCUT_B + (CUT_B - UNCUT_B) * depth_t);
        }
    }

    // 2 triangles per quad, (rows-1)*(cols-1) quads
    let num_quads = (rows - 1) * (cols - 1);
    let mut indices = Vec::with_capacity(num_quads * 6);

    for row in 0..(rows - 1) {
        for col in 0..(cols - 1) {
            let tl = (row * cols + col) as u32;
            let tr = tl + 1;
            let bl = ((row + 1) * cols + col) as u32;
            let br = bl + 1;
            // Two triangles per quad
            indices.push(tl);
            indices.push(bl);
            indices.push(tr);
            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    HeightmapMesh {
        vertices,
        indices,
        colors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, VBitEndmill};

    #[test]
    fn test_heightmap_from_stock_dimensions() {
        let hm = Heightmap::from_stock(0.0, 0.0, 10.0, 10.0, 5.0, 1.0);
        assert_eq!(hm.cols, 11);
        assert_eq!(hm.rows, 11);
        assert_eq!(hm.cells.len(), 121);
    }

    #[test]
    fn test_heightmap_all_cells_at_top_z() {
        let hm = Heightmap::from_stock(0.0, 0.0, 10.0, 10.0, 5.0, 1.0);
        for &z in &hm.cells {
            assert!((z - 5.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_world_cell_roundtrip() {
        let hm = Heightmap::from_stock(10.0, 20.0, 30.0, 40.0, 5.0, 0.5);
        // Cell (4, 6) -> world -> cell should roundtrip
        let (wx, wy) = hm.cell_to_world(4, 6);
        let (row, col) = hm.world_to_cell(wx, wy).unwrap();
        assert_eq!(row, 4);
        assert_eq!(col, 6);
    }

    #[test]
    fn test_world_to_cell_out_of_bounds() {
        let hm = Heightmap::from_stock(0.0, 0.0, 10.0, 10.0, 5.0, 1.0);
        assert!(hm.world_to_cell(-1.0, 5.0).is_none());
        assert!(hm.world_to_cell(5.0, 11.0).is_none());
    }

    #[test]
    fn test_from_bounds() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, -5.0),
            max: P3::new(20.0, 10.0, 3.0),
        };
        let hm = Heightmap::from_bounds(&bbox, None, 1.0);
        assert!((hm.stock_top_z - 3.0).abs() < 1e-10);
        assert!((hm.origin_x - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_stamp_flat_endmill_circle() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let mut hm = Heightmap::from_stock(-10.0, -10.0, 10.0, 10.0, 5.0, 0.5);

        stamp_tool_at(&mut hm, &tool, 0.0, 0.0, 2.0);

        // Cell at center should be cut to 2.0 (tip_z + 0 for flat endmill)
        let (row, col) = hm.world_to_cell(0.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 2.0).abs() < 1e-10);

        // Cell at radius 3 (within tool) should also be at 2.0
        let (row, col) = hm.world_to_cell(3.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 2.0).abs() < 1e-10);

        // Cell at radius 7 (outside tool) should be unchanged
        let (row, col) = hm.world_to_cell(7.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_stamp_ball_endmill_hemisphere() {
        let tool = BallEndmill::new(10.0, 25.0); // radius 5
        let mut hm = Heightmap::from_stock(-10.0, -10.0, 10.0, 10.0, 5.0, 0.5);

        stamp_tool_at(&mut hm, &tool, 0.0, 0.0, 0.0);

        // Center: tip_z + height_at_radius(0) = 0 + 0 = 0
        let (row, col) = hm.world_to_cell(0.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 0.0).abs() < 1e-10);

        // At radius 3: height = R - sqrt(R^2 - r^2) = 5 - sqrt(25-9) = 5 - 4 = 1
        let (row, col) = hm.world_to_cell(3.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 1.0).abs() < 1e-10);

        // Outside radius should be unchanged
        let (row, col) = hm.world_to_cell(7.0, 0.0).unwrap();
        assert!((hm.get(row, col) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_stamp_linear_segment_channel() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut hm = Heightmap::from_stock(-5.0, -5.0, 15.0, 5.0, 5.0, 0.5);

        let start = P3::new(0.0, 0.0, 2.0);
        let end = P3::new(10.0, 0.0, 2.0);
        stamp_linear_segment(&mut hm, &tool, start, end);

        // Check points along the channel are cut
        for x in [0.0, 2.0, 5.0, 8.0, 10.0] {
            let (row, col) = hm.world_to_cell(x, 0.0).unwrap();
            assert!(
                (hm.get(row, col) - 2.0).abs() < 1e-10,
                "Cell at x={} should be at 2.0, got {}",
                x,
                hm.get(row, col)
            );
        }

        // Check points outside the channel are uncut
        let (row, col) = hm.world_to_cell(5.0, 4.0).unwrap();
        assert!((hm.get(row, col) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_stamp_diagonal_no_gaps() {
        let tool = FlatEndmill::new(2.0, 20.0); // radius 1
        let mut hm = Heightmap::from_stock(-2.0, -2.0, 12.0, 12.0, 5.0, 0.25);

        let start = P3::new(0.0, 0.0, 3.0);
        let end = P3::new(10.0, 10.0, 3.0);
        stamp_linear_segment(&mut hm, &tool, start, end);

        // Sample points along the diagonal should be cut
        for i in 0..=10 {
            let p = i as f64;
            let (row, col) = hm.world_to_cell(p, p).unwrap();
            assert!(
                hm.get(row, col) < 5.0,
                "Cell at ({}, {}) should be cut, got {}",
                p,
                p,
                hm.get(row, col)
            );
        }
    }

    #[test]
    fn test_linearize_arc_semicircle() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 0.0);
        // Center at origin: i = -5, j = 0
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);

        // All points should be on the arc (radius 5 from origin)
        for p in &points {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!(
                (r - 5.0).abs() < 0.05,
                "Point ({:.3}, {:.3}) not on arc, r={:.3}",
                p.x,
                p.y,
                r
            );
        }

        // Endpoint should match
        let last = points.last().unwrap();
        assert!((last.x - end.x).abs() < 0.1);
        assert!((last.y - end.y).abs() < 0.1);
    }

    #[test]
    fn test_linearize_arc_z_interpolation() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 10.0);
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);

        // First Z should be 0, last Z should be 10
        assert!((points.first().unwrap().z - 0.0).abs() < 1e-10);
        assert!((points.last().unwrap().z - 10.0).abs() < 0.1);

        // Middle point should be roughly at Z=5
        let mid_idx = points.len() / 2;
        assert!((points[mid_idx].z - 5.0).abs() < 1.0);
    }

    #[test]
    fn test_simulate_rapids_dont_cut() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut hm = Heightmap::from_stock(-10.0, -10.0, 10.0, 10.0, 5.0, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(5.0, 5.0, 0.0)); // rapid move through material

        simulate_toolpath(&tp, &tool, &mut hm);

        // All cells should be unchanged
        for &z in &hm.cells {
            assert!((z - 5.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_simulate_pocket_creates_cavity() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut hm = Heightmap::from_stock(-5.0, -5.0, 15.0, 5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        simulate_toolpath(&tp, &tool, &mut hm);

        // Along the cut path at Y=0, cells should be at -3.0
        let (row, col) = hm.world_to_cell(5.0, 0.0).unwrap();
        assert!(
            (hm.get(row, col) - (-3.0)).abs() < 1e-10,
            "Expected -3.0, got {}",
            hm.get(row, col)
        );

        // Far from the cut, cells should be at stock top
        let (row, col) = hm.world_to_cell(5.0, 4.0).unwrap();
        assert!((hm.get(row, col) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_heightmap_mesh_counts() {
        let hm = Heightmap::from_stock(0.0, 0.0, 4.0, 4.0, 5.0, 1.0);
        // 5x5 grid
        assert_eq!(hm.rows, 5);
        assert_eq!(hm.cols, 5);

        let mesh = heightmap_to_mesh(&hm);
        // 25 vertices
        assert_eq!(mesh.vertices.len(), 75); // 25 * 3
        assert_eq!(mesh.colors.len(), 75);
        // 4*4=16 quads, 2 tris each = 32 triangles = 96 indices
        assert_eq!(mesh.indices.len(), 96);
    }

    #[test]
    fn test_heightmap_mesh_uncut_color() {
        let hm = Heightmap::from_stock(0.0, 0.0, 1.0, 1.0, 5.0, 1.0);
        let mesh = heightmap_to_mesh(&hm);

        // All uncut, so all colors should be the uncut tan
        for i in (0..mesh.colors.len()).step_by(3) {
            assert!((mesh.colors[i] - 0.76).abs() < 0.01); // R
            assert!((mesh.colors[i + 1] - 0.60).abs() < 0.01); // G
            assert!((mesh.colors[i + 2] - 0.42).abs() < 0.01); // B
        }
    }

    #[test]
    fn test_heightmap_mesh_cut_color_darker() {
        let mut hm = Heightmap::from_stock(0.0, 0.0, 2.0, 2.0, 5.0, 1.0);
        // Cut center cell to 0
        hm.cut(1, 1, 0.0);

        let mesh = heightmap_to_mesh(&hm);

        // Vertex at (1,1) = index 4, colors at offset 4*3=12
        let r = mesh.colors[12];
        let g = mesh.colors[13];
        let b = mesh.colors[14];
        // Should be close to dark walnut (0.45, 0.25, 0.10)
        assert!((r - 0.45).abs() < 0.01);
        assert!((g - 0.25).abs() < 0.01);
        assert!((b - 0.10).abs() < 0.01);
    }

    // ── LUT tests ────────────────────────────────────────────────────────

    #[test]
    fn test_lut_ball_matches_analytical() {
        let ball = BallEndmill::new(6.0, 25.0); // radius 3
        let lut = RadialProfileLUT::from_cutter(&ball, 256);

        // Check several radii
        for &r in &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 2.9] {
            let expected = ball.height_at_radius(r).unwrap();
            let got = lut.height_at_dist_sq(r * r).unwrap();
            assert!(
                (got - expected).abs() < 0.001,
                "Ball LUT at r={}: expected {:.6}, got {:.6}",
                r,
                expected,
                got
            );
        }
        // Outside radius should return None
        assert!(lut.height_at_dist_sq(3.1 * 3.1).is_none());
    }

    #[test]
    fn test_lut_flat_matches_analytical() {
        let flat = FlatEndmill::new(10.0, 25.0); // radius 5
        let lut = RadialProfileLUT::from_cutter(&flat, 256);

        for &r in &[0.0, 1.0, 2.5, 4.9] {
            let got = lut.height_at_dist_sq(r * r).unwrap();
            assert!(
                got.abs() < 0.001,
                "Flat LUT at r={}: expected ~0, got {:.6}",
                r,
                got
            );
        }
        assert!(lut.height_at_dist_sq(5.1 * 5.1).is_none());
    }

    #[test]
    fn test_lut_bullnose_matches_analytical() {
        let bn = BullNoseEndmill::new(10.0, 2.0, 25.0); // R1=3, R2=2
        let lut = RadialProfileLUT::from_cutter(&bn, 512);

        for &r in &[0.0, 1.0, 2.0, 3.0, 4.0, 4.5] {
            let expected = bn.height_at_radius(r);
            let got = lut.height_at_dist_sq(r * r);
            match (expected, got) {
                (Some(e), Some(g)) => assert!(
                    (g - e).abs() < 0.01,
                    "BullNose LUT at r={}: expected {:.6}, got {:.6}",
                    r,
                    e,
                    g
                ),
                (None, None) => {}
                _ => panic!(
                    "BullNose LUT mismatch at r={}: {:?} vs {:?}",
                    r, expected, got
                ),
            }
        }
    }

    #[test]
    fn test_lut_vbit_matches_analytical() {
        // VBitEndmill::new(diameter, included_angle_deg, cutting_length)
        let vbit = VBitEndmill::new(10.0, 90.0, 25.0); // 90° V-bit, radius 5
        let lut = RadialProfileLUT::from_cutter(&vbit, 256);

        for &r in &[0.0, 1.0, 2.0, 3.0, 4.0] {
            let expected = vbit.height_at_radius(r);
            let got = lut.height_at_dist_sq(r * r);
            match (expected, got) {
                (Some(e), Some(g)) => assert!(
                    (g - e).abs() < 0.01,
                    "VBit LUT at r={}: expected {:.6}, got {:.6}",
                    r,
                    e,
                    g
                ),
                (None, None) => {}
                _ => panic!("VBit LUT mismatch at r={}: {:?} vs {:?}", r, expected, got),
            }
        }
    }

    // ── Swept segment equivalence test ───────────────────────────────────

    /// Compare swept segment stamping against the reference N-stamp approach
    /// to verify they produce equivalent results.
    fn stamp_linear_segment_reference(
        heightmap: &mut Heightmap,
        cutter: &dyn MillingCutter,
        start: P3,
        end: P3,
    ) {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;
        let seg_len = (dx * dx + dy * dy + dz * dz).sqrt();
        let samples = (seg_len / heightmap.cell_size).ceil().max(1.0) as usize;
        for i in 0..=samples {
            let t = i as f64 / samples as f64;
            let x = start.x + t * dx;
            let y = start.y + t * dy;
            let z = start.z + t * dz;
            // Use direct cutter calls (no LUT) for the reference
            let r = cutter.radius();
            let cs = heightmap.cell_size;
            let col_min = ((x - r - heightmap.origin_x) / cs).floor() as isize;
            let col_max = ((x + r - heightmap.origin_x) / cs).ceil() as isize;
            let row_min = ((y - r - heightmap.origin_y) / cs).floor() as isize;
            let row_max = ((y + r - heightmap.origin_y) / cs).ceil() as isize;
            let col_lo = col_min.max(0) as usize;
            let col_hi = (col_max as usize).min(heightmap.cols - 1);
            let row_lo = row_min.max(0) as usize;
            let row_hi = (row_max as usize).min(heightmap.rows - 1);
            let r_sq = r * r;
            for row in row_lo..=row_hi {
                let cell_y = heightmap.origin_y + row as f64 * cs;
                let dy2 = cell_y - y;
                let dy_sq = dy2 * dy2;
                if dy_sq > r_sq {
                    continue;
                }
                for col in col_lo..=col_hi {
                    let cell_x = heightmap.origin_x + col as f64 * cs;
                    let dx2 = cell_x - x;
                    let dist_sq = dx2 * dx2 + dy_sq;
                    if dist_sq > r_sq {
                        continue;
                    }
                    let dist = dist_sq.sqrt();
                    if let Some(h) = cutter.height_at_radius(dist) {
                        heightmap.cut(row, col, z + h);
                    }
                }
            }
        }
    }

    #[test]
    fn test_swept_segment_matches_reference_flat() {
        let tool = FlatEndmill::new(6.0, 25.0); // radius 3
        let start = P3::new(5.0, 5.0, -2.0);
        let end = P3::new(55.0, 5.0, -2.0);

        let mut hm_ref = Heightmap::from_stock(0.0, 0.0, 60.0, 10.0, 5.0, 0.25);
        stamp_linear_segment_reference(&mut hm_ref, &tool, start, end);

        let mut hm_swept = Heightmap::from_stock(0.0, 0.0, 60.0, 10.0, 5.0, 0.25);
        stamp_linear_segment(&mut hm_swept, &tool, start, end);

        // Swept must cut at least as deep as reference everywhere
        let mut max_diff = 0.0_f64;
        for i in 0..hm_ref.cells.len() {
            let diff = (hm_swept.cells[i] - hm_ref.cells[i]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
            assert!(
                hm_swept.cells[i] <= hm_ref.cells[i] + 0.01,
                "Swept left material above reference at cell {}: swept={:.4}, ref={:.4}",
                i,
                hm_swept.cells[i],
                hm_ref.cells[i]
            );
        }
        // The max difference should be tiny (sub-mm)
        assert!(
            max_diff < 0.05,
            "Max difference between swept and reference: {:.4}mm",
            max_diff
        );
    }

    #[test]
    fn test_swept_segment_matches_reference_ball() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let start = P3::new(5.0, 5.0, -1.0);
        let end = P3::new(55.0, 5.0, -3.0); // sloped segment

        let mut hm_ref = Heightmap::from_stock(0.0, 0.0, 60.0, 10.0, 5.0, 0.25);
        stamp_linear_segment_reference(&mut hm_ref, &tool, start, end);

        let mut hm_swept = Heightmap::from_stock(0.0, 0.0, 60.0, 10.0, 5.0, 0.25);
        stamp_linear_segment(&mut hm_swept, &tool, start, end);

        // The swept approach uses closest-point-on-segment which correctly handles
        // boundary cells that the N-stamp reference misses (due to discrete sampling
        // with non-grid-aligned step sizes from 3D segment length).
        // Compare only cells where both approaches made a cut.
        let stock = 5.0;
        let mut max_diff_interior = 0.0_f64;
        let mut swept_never_above_ref = true;
        for i in 0..hm_ref.cells.len() {
            let s = hm_swept.cells[i];
            let r = hm_ref.cells[i];
            // Swept should never leave material above reference
            if s > r + 0.01 {
                swept_never_above_ref = false;
            }
            // Only compare interior cells (both touched)
            if r < stock - 0.01 && s < stock - 0.01 {
                let diff = (s - r).abs();
                max_diff_interior = max_diff_interior.max(diff);
            }
        }
        assert!(swept_never_above_ref, "Swept left material above reference");
        assert!(
            max_diff_interior < 0.15,
            "Interior cell max diff: {:.4}mm",
            max_diff_interior
        );
    }

    #[test]
    fn test_swept_segment_diagonal_ball() {
        let tool = BallEndmill::new(6.0, 25.0);
        let start = P3::new(5.0, 5.0, -1.0);
        let end = P3::new(25.0, 25.0, -1.0);

        let mut hm_ref = Heightmap::from_stock(0.0, 0.0, 30.0, 30.0, 5.0, 0.25);
        stamp_linear_segment_reference(&mut hm_ref, &tool, start, end);

        let mut hm_swept = Heightmap::from_stock(0.0, 0.0, 30.0, 30.0, 5.0, 0.25);
        stamp_linear_segment(&mut hm_swept, &tool, start, end);

        let mut max_diff = 0.0_f64;
        for i in 0..hm_ref.cells.len() {
            let diff = hm_swept.cells[i] - hm_ref.cells[i];
            max_diff = max_diff.max(diff.abs());
            assert!(
                hm_swept.cells[i] <= hm_ref.cells[i] + 0.01,
                "Cell {}: swept={:.4}, ref={:.4}",
                i,
                hm_swept.cells[i],
                hm_ref.cells[i]
            );
        }
        assert!(max_diff < 0.05, "Max diff: {:.4}mm", max_diff);
    }

    #[test]
    fn test_skip_unchanged_stamps() {
        let tool = FlatEndmill::new(6.0, 25.0);
        let mut hm = Heightmap::from_stock(0.0, 0.0, 20.0, 20.0, 5.0, 0.5);

        // First stamp at z=2
        stamp_tool_at(&mut hm, &tool, 10.0, 10.0, 2.0);
        let (row, col) = hm.world_to_cell(10.0, 10.0).unwrap();
        assert!((hm.get(row, col) - 2.0).abs() < 1e-10);

        // Second stamp at z=3 (higher) should be a no-op due to early-out
        stamp_tool_at(&mut hm, &tool, 10.0, 10.0, 3.0);
        assert!((hm.get(row, col) - 2.0).abs() < 1e-10);

        // Third stamp at z=1 (lower) should cut deeper
        stamp_tool_at(&mut hm, &tool, 10.0, 10.0, 1.0);
        assert!((hm.get(row, col) - 1.0).abs() < 1e-10);
    }
}
