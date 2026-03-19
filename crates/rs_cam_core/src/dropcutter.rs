//! Drop-cutter algorithms for 3D surface finishing.
//!
//! Given a cutter at (x,y), find the maximum Z where it contacts the mesh
//! without gouging. The cutter is "dropped" along Z until first contact.

use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{CLPoint, MillingCutter};
use rayon::prelude::*;

/// Drop a single cutter at position (x, y) onto the mesh.
pub fn point_drop_cutter<C: MillingCutter + ?Sized>(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
) -> CLPoint {
    let mut cl = CLPoint::new(x, y);
    let tri_indices = index.query(x, y, cutter.radius());

    for &idx in &tri_indices {
        let tri = &mesh.faces[idx];
        cutter.drop_cutter(&mut cl, tri);
    }

    cl
}

/// Result of a batch drop-cutter operation: a grid of CL points.
#[derive(Debug)]
pub struct DropCutterGrid {
    pub points: Vec<CLPoint>,
    pub rows: usize,
    pub cols: usize,
    pub x_start: f64,
    pub y_start: f64,
    pub x_step: f64,
    pub y_step: f64,
}

impl DropCutterGrid {
    /// Get the CL point at grid position (row, col).
    pub fn get(&self, row: usize, col: usize) -> &CLPoint {
        &self.points[row * self.cols + col]
    }
}

/// Run batch drop-cutter across a grid of points, parallelized with rayon.
///
/// Generates a regular grid covering the mesh XY extent (plus one cutter radius margin),
/// with the specified step-over distance.
pub fn batch_drop_cutter<C: MillingCutter + ?Sized>(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    step_over: f64,
    direction_deg: f64,
    min_z: f64,
) -> DropCutterGrid {
    let r = cutter.radius();
    let bbox = mesh.bbox.expand_by(r);

    // For now, only axis-aligned raster (direction = 0 or 90)
    // TODO: support arbitrary angles
    let _ = direction_deg;

    let x_start = bbox.min.x;
    let x_end = bbox.max.x;
    let y_start = bbox.min.y;
    let y_end = bbox.max.y;

    let cols = ((x_end - x_start) / step_over).ceil() as usize + 1;
    let rows = ((y_end - y_start) / step_over).ceil() as usize + 1;

    let total = rows * cols;

    let points: Vec<CLPoint> = (0..total)
        .into_par_iter()
        .map(|i| {
            let row = i / cols;
            let col = i % cols;
            let x = x_start + col as f64 * step_over;
            let y = y_start + row as f64 * step_over;
            let mut cl = point_drop_cutter(x, y, mesh, index, cutter);
            // Clamp to minimum Z
            if cl.z < min_z {
                cl.z = min_z;
            }
            cl
        })
        .collect();

    DropCutterGrid {
        points,
        rows,
        cols,
        x_start,
        y_start,
        x_step: step_over,
        y_step: step_over,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{make_test_flat, SpatialIndex};
    use crate::tool::BallEndmill;

    #[test]
    fn test_batch_drop_cutter_flat() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);

        assert!(grid.rows > 0);
        assert!(grid.cols > 0);

        // Points over the flat surface should be at z=0 (ball tip touches z=0 flat surface)
        let center_row = grid.rows / 2;
        let center_col = grid.cols / 2;
        let cl = grid.get(center_row, center_col);
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "Center CL.z = {}, expected ~0.0",
            cl.z
        );
    }
}
