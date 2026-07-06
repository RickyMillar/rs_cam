//! Waterline algorithm — generates closed contour toolpaths at constant Z heights.
//!
//! Uses the push-cutter algorithm to find cutter contact intervals on X and Y fibers,
//! then extracts CL boundary points and connects them into closed contours.
//!
//! The algorithm:
//! 1. Generate grids of X-fibers and Y-fibers at the target Z height
//! 2. Run batch push-cutter on both fiber sets
//! 3. Extract CL boundary points from interval endpoints
//! 4. Connect boundary points into closed loops using nearest-neighbor chaining

use crate::contour_extract::weave_contours;
use crate::fiber::Fiber;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pushcutter::batch_push_cutter;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};

/// Parameters for waterline toolpath generation.
pub struct WaterlineParams {
    /// Fiber sampling spacing (mm). Smaller = more accurate but slower.
    pub sampling: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves.
    pub safe_z: f64,
}

/// Generate a single waterline contour at a given Z height.
///
/// Returns boundary CL points organized as closed loops.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn waterline_contours(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
) -> Vec<Vec<P3>> {
    let never_cancel = || false;
    waterline_contours_with_cancel(mesh, index, cutter, z, sampling, &never_cancel)
        .expect("non-cancellable waterline contours should never be cancelled")
}

/// Z levels a waterline pass will cut at, from `start_z` down to `final_z`
/// (matching the generator's own `while z >= final_z - 1e-10 { .. z -= z_step }`
/// ladder exactly). Exposed so callers building depth-run spans can pass
/// waterline's REAL level ladder (R2.8) instead of an empty slice — the
/// generator itself is refactored to consume this same helper below, so
/// there is exactly one place the ladder math lives.
pub fn waterline_z_levels(start_z: f64, final_z: f64, z_step: f64) -> Vec<f64> {
    let mut levels = Vec::new();
    let mut z = start_z;
    while z >= final_z - 1e-10 {
        levels.push(z);
        z -= z_step;
    }
    levels
}

/// Generate waterline toolpaths at multiple Z heights.
///
/// Z heights are generated from start_z down to final_z with the given step.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
#[tracing::instrument(skip(mesh, index, cutter, params), fields(
    start_z, final_z, z_step,
    tri_count = mesh.triangles.len(),
))]
pub fn waterline_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    params: &WaterlineParams,
) -> Toolpath {
    let never_cancel = || false;
    waterline_toolpath_with_cancel(
        mesh,
        index,
        cutter,
        start_z,
        final_z,
        z_step,
        params,
        &never_cancel,
    )
    .expect("non-cancellable waterline should never be cancelled")
}

#[allow(clippy::too_many_arguments)]
pub fn waterline_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    params: &WaterlineParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut toolpath = Toolpath::new();

    for z in waterline_z_levels(start_z, final_z, z_step) {
        check_cancel(cancel)?;
        let contours =
            waterline_contours_with_cancel(mesh, index, cutter, z, params.sampling, cancel)?;

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }
            toolpath.emit_closed_contour_with_intent(
                contour,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                MoveIntent::FinishingCut,
            );
        }
    }

    Ok(toolpath)
}

pub fn waterline_contours_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<P3>>, Cancelled> {
    let bbox = &mesh.bbox;
    let r = cutter.radius();

    let x_min = bbox.min.x - r;
    let x_max = bbox.max.x + r;
    let y_min = bbox.min.y - r;
    let y_max = bbox.max.y + r;

    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let mut x_fibers: Vec<Fiber> = (0..ny)
        .map(|i| {
            let y = y_min + i as f64 * sampling;
            Fiber::new_x(y, z, x_min, x_max)
        })
        .collect();

    let nx = ((x_max - x_min) / sampling).ceil() as usize + 1;
    let mut y_fibers: Vec<Fiber> = (0..nx)
        .map(|i| {
            let x = x_min + i as f64 * sampling;
            Fiber::new_y(x, z, y_min, y_max)
        })
        .collect();

    // Run push-cutter on both fiber sets in parallel — this mirrors the
    // rayon::join structure `waterline_contours` uses, restoring the
    // parallelism this cancellable variant had lost (it previously pushed
    // x- and y-fibers through `batch_push_cutter_with_cancel` one after the
    // other). Cancellation is checked once the join completes rather than
    // per-chunk inside each direction — the same trade-off
    // `SurfaceHeightmap::from_mesh_with_cancel` makes for its parallel grid
    // batch (slope.rs): a single Z-level's fiber batch is short enough that
    // per-chunk polling isn't worth losing join concurrency for, and the
    // per-Z-level `check_cancel` in `waterline_toolpath_with_cancel`'s loop
    // still bounds how much uncancelled work a stale cancel signal can cost.
    #[cfg(feature = "parallel")]
    rayon::join(
        || batch_push_cutter(&mut x_fibers, mesh, index, cutter),
        || batch_push_cutter(&mut y_fibers, mesh, index, cutter),
    );
    #[cfg(not(feature = "parallel"))]
    {
        batch_push_cutter(&mut x_fibers, mesh, index, cutter);
        batch_push_cutter(&mut y_fibers, mesh, index, cutter);
    }

    check_cancel(cancel)?;

    Ok(weave_contours(&x_fibers, &y_fibers, z))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::tool::BallEndmill;
    use crate::toolpath::MoveType;

    #[test]
    fn z_levels_matches_generator_ladder() {
        // R2.8: the exposed ladder must match the generator's own
        // start-down-to-final stepping exactly (same 1e-10 tail epsilon).
        let levels = waterline_z_levels(10.0, 0.0, 2.5);
        assert_eq!(levels, vec![10.0, 7.5, 5.0, 2.5, 0.0]);
    }

    #[test]
    fn z_levels_empty_when_start_below_final() {
        assert!(waterline_z_levels(-1.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn test_waterline_hemisphere_midheight() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 10.0, 2.0);
        // At z=10 (midway), should find at least one contour
        assert!(
            !contours.is_empty(),
            "Should find contours at z=10 on hemisphere"
        );
    }

    #[test]
    fn test_waterline_above_mesh() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 25.0, 2.0);
        assert!(contours.is_empty(), "No contours above mesh");
    }

    #[test]
    fn test_waterline_well_below_mesh() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Well below the mesh — z=-30, cutter length=25, so can't reach z=0 base
        let contours = waterline_contours(&mesh, &index, &tool, -30.0, 2.0);
        assert!(contours.is_empty(), "No contours well below mesh");
    }

    #[test]
    fn test_waterline_toolpath_multiple_z() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
        };

        let tp = waterline_toolpath(&mesh, &index, &tool, 15.0, 5.0, 5.0, &params);
        // Should have multiple Z levels: 15, 10, 5
        assert!(!tp.moves.is_empty(), "Waterline toolpath should have moves");

        // Should have rapids (retracts between contours)
        let rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert!(rapids >= 2, "Should have retracts between Z levels");
    }

    #[test]
    fn test_waterline_contour_is_roughly_circular() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 10.0, 1.5);
        if contours.is_empty() {
            return; // Skip if no contours found (sampling too coarse)
        }

        let contour = &contours[0];
        // Hemisphere at z=10: radius = sqrt(20^2 - 10^2) = sqrt(300) ≈ 17.3
        // With ball cutter r=3, CL radius ≈ 17.3 + 3 = 20.3 (outer) or 17.3 - 3 = 14.3 (inner)
        // Points should be roughly equidistant from center
        let cx: f64 = contour.iter().map(|p| p.x).sum::<f64>() / contour.len() as f64;
        let cy: f64 = contour.iter().map(|p| p.y).sum::<f64>() / contour.len() as f64;

        // Check that points are approximately on a circle
        let radii: Vec<f64> = contour
            .iter()
            .map(|p| {
                let dx = p.x - cx;
                let dy = p.y - cy;
                (dx * dx + dy * dy).sqrt()
            })
            .collect();
        let mean_r = radii.iter().sum::<f64>() / radii.len() as f64;

        // All radii should be within 50% of mean (rough check for circular shape)
        for &r in &radii {
            assert!(
                r > mean_r * 0.5 && r < mean_r * 1.5,
                "Point radius {} far from mean {}, contour may not be circular",
                r,
                mean_r
            );
        }
    }
}
