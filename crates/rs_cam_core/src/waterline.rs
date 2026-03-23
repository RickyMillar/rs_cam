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
use crate::pushcutter::{batch_push_cutter, batch_push_cutter_with_cancel};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

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
pub fn waterline_contours(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
) -> Vec<Vec<P3>> {
    let bbox = &mesh.bbox;
    let r = cutter.radius();

    // Expand bbox by cutter radius
    let x_min = bbox.min.x - r;
    let x_max = bbox.max.x + r;
    let y_min = bbox.min.y - r;
    let y_max = bbox.max.y + r;

    // Generate X-fibers (horizontal, one per Y step)
    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let mut x_fibers: Vec<Fiber> = (0..ny)
        .map(|i| {
            let y = y_min + i as f64 * sampling;
            Fiber::new_x(y, z, x_min, x_max)
        })
        .collect();

    // Generate Y-fibers (vertical, one per X step)
    let nx = ((x_max - x_min) / sampling).ceil() as usize + 1;
    let mut y_fibers: Vec<Fiber> = (0..nx)
        .map(|i| {
            let x = x_min + i as f64 * sampling;
            Fiber::new_y(x, z, y_min, y_max)
        })
        .collect();

    // Run push-cutter on both fiber sets
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

    // Extract contour loops using the Weave graph (topologically correct)
    weave_contours(&x_fibers, &y_fibers, z)
}

/// Generate waterline toolpaths at multiple Z heights.
///
/// Z heights are generated from start_z down to final_z with the given step.
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

    let mut z = start_z;
    while z >= final_z - 1e-10 {
        check_cancel(cancel)?;
        let contours =
            waterline_contours_with_cancel(mesh, index, cutter, z, params.sampling, cancel)?;

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }

            // SAFETY: contour.len() >= 3 checked above; [0] and [1..] are valid
            #[allow(clippy::indexing_slicing)]
            {
                // Rapid to above first point
                toolpath.rapid_to(P3::new(contour[0].x, contour[0].y, params.safe_z));

                // Plunge to Z
                toolpath.feed_to(P3::new(contour[0].x, contour[0].y, z), params.plunge_rate);

                // Follow contour
                for pt in &contour[1..] {
                    toolpath.feed_to(P3::new(pt.x, pt.y, z), params.feed_rate);
                }

                // Close the contour
                toolpath.feed_to(P3::new(contour[0].x, contour[0].y, z), params.feed_rate);

                // Retract
                toolpath.rapid_to(P3::new(contour[0].x, contour[0].y, params.safe_z));
            }
        }

        z -= z_step;
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

    batch_push_cutter_with_cancel(&mut x_fibers, mesh, index, cutter, cancel)?;
    batch_push_cutter_with_cancel(&mut y_fibers, mesh, index, cutter, cancel)?;

    Ok(weave_contours(&x_fibers, &y_fibers, z))
}

/// Chain boundary points into closed contour loops using nearest-neighbor.
///
/// Points within `max_gap` distance are connected. Loops shorter than 3 points
/// are discarded.
///
/// Public variant for use by the weave module's fallback path.
pub fn chain_contours_pub(points: &[P3], max_gap: f64) -> Vec<Vec<P3>> {
    chain_contours(points, max_gap)
}

// SAFETY: all indexing into points/used is guarded by iterator position or bounds checks
#[allow(clippy::indexing_slicing)]
fn chain_contours(points: &[P3], max_gap: f64) -> Vec<Vec<P3>> {
    if points.is_empty() {
        return Vec::new();
    }

    let max_gap_sq = max_gap * max_gap;
    let mut used = vec![false; points.len()];
    let mut contours = Vec::new();

    while let Some(start) = used.iter().position(|&u| !u) {
        let mut chain = vec![points[start]];
        used[start] = true;

        // Greedy nearest-neighbor chain
        loop {
            // Safety: chain always has at least one element (pushed on the line above
            // on first iteration, or extended via `chain.push` before looping back).
            #[allow(clippy::unwrap_used, clippy::panic)]
            let last = chain.last().unwrap();
            let mut best_idx = None;
            let mut best_dist_sq = max_gap_sq;

            for (i, pt) in points.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let dx = pt.x - last.x;
                let dy = pt.y - last.y;
                let d_sq = dx * dx + dy * dy;
                if d_sq < best_dist_sq {
                    best_dist_sq = d_sq;
                    best_idx = Some(i);
                }
            }

            match best_idx {
                Some(i) => {
                    chain.push(points[i]);
                    used[i] = true;
                }
                None => break,
            }
        }

        // Only keep loops with enough points and that close back near the start
        if chain.len() >= 3 {
            let first = chain[0];
            // Safety: chain.len() >= 3 guard above guarantees non-empty.
            #[allow(clippy::unwrap_used, clippy::panic)]
            let last = chain.last().unwrap();
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let close_dist_sq = dx * dx + dy * dy;
            // Accept if the loop roughly closes
            if close_dist_sq < max_gap_sq * 4.0 {
                contours.push(chain);
            }
        }
    }

    contours
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::tool::BallEndmill;
    use crate::toolpath::MoveType;

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

    #[test]
    fn test_chain_contours_basic() {
        // Simple square of points
        let points = vec![
            P3::new(0.0, 0.0, 5.0),
            P3::new(10.0, 0.0, 5.0),
            P3::new(10.0, 10.0, 5.0),
            P3::new(0.0, 10.0, 5.0),
        ];
        let contours = chain_contours(&points, 15.0);
        assert_eq!(contours.len(), 1, "Should form one loop");
        assert_eq!(contours[0].len(), 4);
    }

    #[test]
    fn test_chain_contours_two_separate() {
        // Two clusters far apart
        let points = vec![
            P3::new(0.0, 0.0, 5.0),
            P3::new(1.0, 0.0, 5.0),
            P3::new(1.0, 1.0, 5.0),
            P3::new(0.0, 1.0, 5.0),
            P3::new(100.0, 100.0, 5.0),
            P3::new(101.0, 100.0, 5.0),
            P3::new(101.0, 101.0, 5.0),
            P3::new(100.0, 101.0, 5.0),
        ];
        let contours = chain_contours(&points, 3.0);
        assert_eq!(contours.len(), 2, "Should form two separate loops");
    }

    #[test]
    fn test_chain_contours_empty() {
        let contours = chain_contours(&[], 5.0);
        assert!(contours.is_empty());
    }
}
