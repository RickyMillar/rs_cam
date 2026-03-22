//! Drop-cutter algorithms for 3D surface finishing.
//!
//! Given a cutter at (x,y), find the maximum Z where it contacts the mesh
//! without gouging. The cutter is "dropped" along Z until first contact.

#[cfg(not(feature = "parallel"))]
use crate::interrupt::check_cancel;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{CLPoint, MillingCutter};

#[cfg(feature = "parallel")]
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
    let never_cancel = || false;
    batch_drop_cutter_with_cancel(
        mesh,
        index,
        cutter,
        step_over,
        direction_deg,
        min_z,
        &never_cancel,
    )
    .expect("non-cancellable drop-cutter should never be cancelled")
}

pub fn batch_drop_cutter_with_cancel<C: MillingCutter + ?Sized>(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    step_over: f64,
    direction_deg: f64,
    min_z: f64,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<DropCutterGrid, Cancelled> {
    let r = cutter.radius();
    let bbox = mesh.bbox.expand_by(r);

    let angle_rad = direction_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // For near-zero angles, skip rotation overhead
    let use_rotation = direction_deg.abs() > 0.01
        && (direction_deg - 90.0).abs() > 0.01
        && (direction_deg - 180.0).abs() > 0.01;

    if !use_rotation {
        // Axis-aligned fast path (original behavior)
        let x_start = bbox.min.x;
        let x_end = bbox.max.x;
        let y_start = bbox.min.y;
        let y_end = bbox.max.y;

        let cols = ((x_end - x_start) / step_over).ceil() as usize + 1;
        let rows = ((y_end - y_start) / step_over).ceil() as usize + 1;

        let points = batch_compute_points(
            rows, cols, cancel, mesh, index, cutter, min_z,
            |i| {
                let row = i / cols;
                let col = i % cols;
                let x = x_start + col as f64 * step_over;
                let y = y_start + row as f64 * step_over;
                (x, y)
            },
        )?;

        return Ok(DropCutterGrid {
            points,
            rows,
            cols,
            x_start,
            y_start,
            x_step: step_over,
            y_step: step_over,
        });
    }

    // Rotated grid: transform bbox corners into rotated frame to find bounds
    let corners = [
        (bbox.min.x, bbox.min.y),
        (bbox.max.x, bbox.min.y),
        (bbox.max.x, bbox.max.y),
        (bbox.min.x, bbox.max.y),
    ];

    let mut u_min = f64::INFINITY;
    let mut u_max = f64::NEG_INFINITY;
    let mut v_min = f64::INFINITY;
    let mut v_max = f64::NEG_INFINITY;

    for &(x, y) in &corners {
        // Rotate into aligned frame (forward rotation)
        let u = x * cos_a + y * sin_a;
        let v = -x * sin_a + y * cos_a;
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }

    let cols = ((u_max - u_min) / step_over).ceil() as usize + 1;
    let rows = ((v_max - v_min) / step_over).ceil() as usize + 1;

    let points = batch_compute_points(
        rows, cols, cancel, mesh, index, cutter, min_z,
        |i| {
            let row = i / cols;
            let col = i % cols;
            let u = u_min + col as f64 * step_over;
            let v = v_min + row as f64 * step_over;
            // Inverse rotation: (u,v) -> (x,y)
            let x = u * cos_a - v * sin_a;
            let y = u * sin_a + v * cos_a;
            (x, y)
        },
    )?;

    Ok(DropCutterGrid {
        points,
        rows,
        cols,
        x_start: u_min,
        y_start: v_min,
        x_step: step_over,
        y_step: step_over,
    })
}

#[allow(clippy::too_many_arguments)]
/// Shared helper: compute CL points for a grid, using rayon parallelism when available.
///
/// `coord_fn` maps a flat index to (x, y) world coordinates.
/// With the `parallel` feature, rows are processed in parallel via `par_chunks`.
/// Cancellation is checked per-chunk in the parallel path, and every 64 points
/// in the sequential fallback.
fn batch_compute_points<C: MillingCutter + ?Sized>(
    rows: usize,
    cols: usize,
    cancel: &(dyn CancelCheck + Sync),
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
    min_z: f64,
    coord_fn: impl Fn(usize) -> (f64, f64) + Sync,
) -> Result<Vec<CLPoint>, Cancelled> {
    let total = rows * cols;

    #[cfg(feature = "parallel")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = AtomicBool::new(false);

        // Process by rows: each row is `cols` points and is independent.
        let points: Vec<CLPoint> = (0..rows)
            .into_par_iter()
            .flat_map(|row| {
                // Check cancellation once per row
                if cancelled.load(Ordering::Relaxed) || cancel.cancelled() {
                    cancelled.store(true, Ordering::Relaxed);
                    return Vec::new();
                }
                let start = row * cols;
                (start..start + cols)
                    .map(|i| {
                        let (x, y) = coord_fn(i);
                        let mut cl = point_drop_cutter(x, y, mesh, index, cutter);
                        if cl.z < min_z {
                            cl.z = min_z;
                        }
                        cl
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        if cancelled.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        debug_assert_eq!(points.len(), total);
        Ok(points)
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut points = Vec::with_capacity(total);
        for i in 0..total {
            if i % 64 == 0 {
                check_cancel(cancel)?;
            }
            let (x, y) = coord_fn(i);
            let mut cl = point_drop_cutter(x, y, mesh, index, cutter);
            if cl.z < min_z {
                cl.z = min_z;
            }
            points.push(cl);
        }
        Ok(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_flat};
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

    #[test]
    fn test_point_drop_cutter_contacted_flag() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Point over the mesh should be contacted
        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted, "Point over mesh should be contacted");
        assert!(cl.z > f64::NEG_INFINITY, "Z should be finite");

        // Point far outside mesh footprint should not be contacted
        let cl_outside = point_drop_cutter(500.0, 500.0, &mesh, &index, &tool);
        assert!(
            !cl_outside.contacted,
            "Point far outside mesh should not be contacted"
        );
    }

    #[test]
    fn test_batch_drop_cutter_rotated_45() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let grid_0 = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        let grid_45 = batch_drop_cutter(&mesh, &index, &tool, 5.0, 45.0, -100.0);

        // Both should produce valid grids
        assert!(grid_0.rows > 0 && grid_0.cols > 0);
        assert!(grid_45.rows > 0 && grid_45.cols > 0);

        // The 45° grid should have contacted points over the mesh
        let center = grid_45.get(grid_45.rows / 2, grid_45.cols / 2);
        assert!(
            center.contacted,
            "Center of 45° grid should contact flat mesh"
        );
        assert!(
            (center.z - 0.0).abs() < 0.5,
            "Center CL.z on flat = {}, expected ~0.0",
            center.z
        );
    }
}
