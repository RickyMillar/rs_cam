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
    use crate::geo::P3;
    use crate::mesh::{SpatialIndex, make_test_flat, make_test_hemisphere};
    use crate::tool::{
        BallEndmill, BullNoseEndmill, FlatEndmill, TaperedBallEndmill, VBitEndmill,
    };

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

    // --- Task E-dc: All 5 tool types on flat mesh ---

    #[test]
    fn test_flat_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(cl.contacted, "FlatEndmill center should contact flat mesh");
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "FlatEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_ball_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted);
        // Ball on flat: facet_drop yields z = surface_z + R*nz - R = 0
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "BallEndmill CL.z = {}, expected ~0.0",
            cl.z
        );
    }

    #[test]
    fn test_bullnose_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BullNoseEndmill::new(10.0, 2.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(
            cl.contacted,
            "BullNoseEndmill center should contact flat mesh"
        );
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "BullNoseEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_vbit_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = VBitEndmill::new(10.0, 90.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(cl.contacted, "VBitEndmill center should contact flat mesh");
        // V-bit tip contact on flat surface: z = 0
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "VBitEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    #[test]
    fn test_tapered_ball_endmill_on_flat_mesh() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = TaperedBallEndmill::new(6.0, 10.0, 12.0, 30.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 5.0, 0.0, -100.0);
        assert!(grid.rows > 0 && grid.cols > 0);

        let cl = grid.get(grid.rows / 2, grid.cols / 2);
        assert!(
            cl.contacted,
            "TaperedBallEndmill center should contact flat mesh"
        );
        assert!(
            (cl.z - 0.0).abs() < 0.5,
            "TaperedBallEndmill CL.z = {}, expected ~0.0 on flat mesh",
            cl.z
        );
    }

    // --- All 5 tool types on hemisphere mesh ---

    #[test]
    fn test_all_tools_on_hemisphere_produce_valid_heights() {
        let hemisphere_r = 20.0;
        let mesh = make_test_hemisphere(hemisphere_r, 16);
        let index = SpatialIndex::build(&mesh, 10.0);

        let flat = FlatEndmill::new(10.0, 25.0);
        let ball = BallEndmill::new(10.0, 25.0);
        let bull = BullNoseEndmill::new(10.0, 2.0, 25.0);
        let vbit = VBitEndmill::new(10.0, 90.0, 25.0);
        let tapered = TaperedBallEndmill::new(6.0, 10.0, 12.0, 30.0);

        // Drop each tool at the apex (0,0)
        let tools: Vec<(&str, &dyn crate::tool::MillingCutter)> = vec![
            ("flat", &flat),
            ("ball", &ball),
            ("bullnose", &bull),
            ("vbit", &vbit),
            ("tapered_ball", &tapered),
        ];

        for (name, tool) in &tools {
            let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, *tool);
            assert!(cl.contacted, "{} should contact hemisphere at apex", name);
            assert!(
                cl.z.is_finite() && cl.z > 0.0,
                "{} CL.z = {} should be finite and positive on hemisphere apex",
                name,
                cl.z
            );
            // At the apex, all tools should land near hemisphere_r
            assert!(
                (cl.z - hemisphere_r).abs() < 1.0,
                "{} CL.z = {}, expected ~{} at hemisphere apex",
                name,
                cl.z,
                hemisphere_r
            );
        }
    }

    // --- Edge cases ---

    #[test]
    fn test_single_triangle_mesh() {
        // Build a mesh from a single triangle
        let vertices = vec![
            P3::new(-10.0, -10.0, 5.0),
            P3::new(10.0, -10.0, 5.0),
            P3::new(0.0, 10.0, 5.0),
        ];
        let triangles = vec![[0, 1, 2]];
        let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        // Point inside the single triangle
        let cl = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(cl.contacted, "Should contact single triangle");
        assert!(
            (cl.z - 5.0).abs() < 1e-6,
            "Flat endmill on single triangle at z=5 should give CL.z=5, got {}",
            cl.z
        );
    }

    #[test]
    fn test_near_boundary_grid_points() {
        // Mesh is from -50 to +50. Points near the boundary should still
        // be contacted (tool radius extends the query range).
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Point at the mesh boundary edge: tool extends 5mm past edge,
        // so at x=49 we should still have contact via vertex/edge
        let cl = point_drop_cutter(49.0, 0.0, &mesh, &index, &tool);
        assert!(
            cl.contacted,
            "Near-boundary point at x=49 should be contacted (tool radius=5)"
        );
        assert!(
            cl.z.is_finite(),
            "Near-boundary CL.z should be finite, got {}",
            cl.z
        );
    }

    #[test]
    fn test_vertical_edge_mesh() {
        // Mesh with a vertical wall: two triangles forming a 90-degree step
        let vertices = vec![
            P3::new(-20.0, -20.0, 0.0),
            P3::new(0.0, -20.0, 0.0),
            P3::new(0.0, -20.0, 10.0),
            P3::new(-20.0, -20.0, 10.0),
            P3::new(0.0, 20.0, 0.0),
            P3::new(0.0, 20.0, 10.0),
            // Top surface
            P3::new(-20.0, 20.0, 10.0),
            P3::new(20.0, 20.0, 10.0),
            P3::new(20.0, -20.0, 10.0),
        ];
        let triangles = vec![
            [0, 1, 2],
            [0, 2, 3],
            [1, 4, 5],
            [1, 5, 2],
            // Top surface
            [3, 2, 5],
            [3, 5, 6],
            [2, 8, 7],
            [2, 7, 5],
        ];
        let mesh = crate::mesh::TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 20.0);

        // Drop on the top surface, well inside
        let cl_top = point_drop_cutter(5.0, 0.0, &mesh, &index, &tool);
        assert!(cl_top.contacted, "Should contact top surface");
        assert!(
            cl_top.z > 5.0,
            "CL on top surface should be above 5, got {}",
            cl_top.z
        );

        // Drop at the vertical wall boundary: should still get valid results
        let cl_wall = point_drop_cutter(0.0, 0.0, &mesh, &index, &tool);
        assert!(
            cl_wall.z.is_finite(),
            "CL at vertical wall should be finite, got {}",
            cl_wall.z
        );
    }

    #[test]
    fn test_min_z_clamping() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // With min_z = 5.0, all points should be at least 5.0
        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, 5.0);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cl = grid.get(row, col);
                assert!(
                    cl.z >= 5.0 - 1e-10,
                    "CL.z = {} should be >= min_z=5.0 at ({}, {})",
                    cl.z,
                    row,
                    col
                );
            }
        }
    }

    // --- Cancellation ---

    #[test]
    fn test_cancellation_returns_error() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Cancel immediately
        let always_cancel = || true;
        let result = batch_drop_cutter_with_cancel(
            &mesh,
            &index,
            &tool,
            5.0,
            0.0,
            -100.0,
            &always_cancel,
        );
        assert!(
            result.is_err(),
            "Immediately-cancelling predicate should return Err(Cancelled)"
        );
    }

    #[test]
    fn test_no_cancellation_succeeds() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        let never_cancel = || false;
        let result = batch_drop_cutter_with_cancel(
            &mesh,
            &index,
            &tool,
            5.0,
            0.0,
            -100.0,
            &never_cancel,
        );
        assert!(
            result.is_ok(),
            "Never-cancelling predicate should return Ok"
        );
        let grid = result.unwrap();
        assert!(grid.rows > 0 && grid.cols > 0);
    }

    #[test]
    fn test_cancellation_after_some_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = BallEndmill::new(10.0, 25.0);

        // Cancel after being checked a few times
        let check_count = AtomicUsize::new(0);
        let cancel_after_3 = || {
            check_count.fetch_add(1, Ordering::Relaxed);
            check_count.load(Ordering::Relaxed) > 3
        };

        let result = batch_drop_cutter_with_cancel(
            &mesh,
            &index,
            &tool,
            5.0,
            0.0,
            -100.0,
            &cancel_after_3,
        );

        // Depending on parallelism, it may or may not cancel in time, but
        // the cancel predicate should have been called at least once
        assert!(
            check_count.load(Ordering::Relaxed) > 0,
            "Cancel check should have been invoked at least once"
        );
        // The result should be either Ok or Err(Cancelled) - never a panic
        let _ = result;
    }

    // --- Grid accessor ---

    #[test]
    fn test_drop_cutter_grid_get_accessor() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, -100.0);

        // Verify that get(row, col) matches the underlying flat array
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cl = grid.get(row, col);
                let flat_cl = &grid.points[row * grid.cols + col];
                assert_eq!(cl.x, flat_cl.x);
                assert_eq!(cl.y, flat_cl.y);
                assert_eq!(cl.z, flat_cl.z);
            }
        }
    }

    #[test]
    fn test_batch_grid_dimensions_match_step_over() {
        let mesh = make_test_flat(100.0);
        let index = SpatialIndex::build(&mesh, 20.0);
        let tool = FlatEndmill::new(10.0, 25.0);

        let grid = batch_drop_cutter(&mesh, &index, &tool, 10.0, 0.0, -100.0);

        // Grid should have rows*cols points
        assert_eq!(
            grid.points.len(),
            grid.rows * grid.cols,
            "Grid points count should be rows * cols"
        );

        // Step sizes should match
        assert!(
            (grid.x_step - 10.0).abs() < 1e-10,
            "x_step should be 10.0, got {}",
            grid.x_step
        );
        assert!(
            (grid.y_step - 10.0).abs() < 1e-10,
            "y_step should be 10.0, got {}",
            grid.y_step
        );
    }
}
