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
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::pushcutter::batch_push_cutter;
use crate::region_set::RegionSet;
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
    /// Stock to leave (mm), applied as a **+Z shift on every emitted contour
    /// point** — the repo-wide finish convention (`scallop.rs`'s
    /// `cl.z + stock_to_leave`, `steep_shallow.rs`'s `z_adjusted = z +
    /// stock_to_leave` for exactly this contour shape).
    ///
    /// Added by F3 / D-16.2 (2026-08-06). Until then `WaterlineParams` had no
    /// such field, so the `UnifiedFinish` VerySteep band **could not** pass
    /// one and silently dropped the operator's dial — the twin of the Shallow
    /// band's defect, found while mapping it
    /// (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.B).
    /// Fixing only Shallow would have put a `stock_to_leave`-sized step at
    /// the shallow↔waterline seam that does not exist today, so both bands
    /// were fixed together.
    ///
    /// `0.0` reproduces the pre-fix toolpath byte-for-byte: the lift is
    /// skipped entirely, not applied as `+0.0`.
    ///
    /// **Known approximation, deliberately shared rather than corrected
    /// here.** A vertical lift leaves `stock_to_leave·cos θ` measured normal
    /// to the surface, which on the near-vertical walls this band exists for
    /// is a small fraction of the dialled value. Every finish op in the repo
    /// makes the same approximation; whether the convention should become
    /// normal-direction is a separate repo-wide question and is NOT coupled
    /// to this field.
    pub stock_to_leave: f64,
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

/// Inclusive-bounds epsilon for waterline's Z ladder.
///
/// Eight orders of magnitude tighter than
/// [`crate::finish_setup::Z_LADDER_DEFAULT_EPSILON`], which is what
/// `steep_shallow` uses. That is not an oversight on either side: this value
/// reproduces the generator's original `while z >= final_z - 1e-10` loop
/// exactly, and waterline's callers pass ladder bounds taken straight from a
/// mesh bbox, where a 0.01 mm tolerance would silently add a level.
pub const WATERLINE_LADDER_EPSILON: f64 = 1e-10;

/// Z levels a waterline pass will cut at, from `start_z` down to `final_z`.
///
/// Exposed so callers building depth-run spans can pass waterline's REAL
/// level ladder (R2.8) instead of an empty slice; the generator below
/// consumes the same helper, so there is one ladder per operation and — since
/// C3 — one ladder IMPLEMENTATION for the crate.
///
/// C3: this was a private copy of [`crate::finish_setup::z_ladder`]'s
/// `snap_to_bottom = false` arm, differing only in hard-coding its epsilon
/// where the shared version takes one. It is now an adapter that names the
/// epsilon and delegates. `tests/waterline_shared_finish_setup_c3.rs` pins
/// the two against each other across the boundary cases (exact multiples, a
/// remainder, an inverted range, a zero range).
pub fn waterline_z_levels(start_z: f64, final_z: f64, z_step: f64) -> Vec<f64> {
    crate::finish_setup::z_ladder(start_z, final_z, z_step, WATERLINE_LADDER_EPSILON, false)
}

/// PR-8d's minimum-segment floor, inherited by waterline at C3.
///
/// `contour_extract::weave_contours` places each cell-edge vertex at an
/// EXACT fiber interval boundary, so two adjacent cells whose boundaries
/// resolve to the same crossing chain two vertices that are coincident to
/// floating-point noise. `steep_shallow.rs` fixed this at its emission site
/// in PR-8d and left waterline alone to keep that commit one operation wide
/// — but waterline is where the offenders Checkpoint B §8.1 attributed to
/// the steep half actually came from: `steep_shallow` generates its steep
/// passes by calling `waterline_contours`.
///
/// Measured on the §8.1 mixed-slope ribbon: 2 cutting segments of
/// **0.000891 mm** — the same value §8.1 reports, bit for bit — removed at
/// zero cost in cutting length.
///
/// `closed` matches the emitter chosen at the call site: a whole-contour
/// survivor chords back to its start, so a trailing vertex within the floor
/// of the FIRST would make THAT move the degenerate one.
fn floor_contour(points: &[P3], closed: bool) -> Vec<P3> {
    crate::toolpath::drop_sub_minimum_segments(
        points,
        crate::toolpath::MIN_EMITTED_SEGMENT_MM,
        closed,
    )
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
        None,
        &never_cancel,
    )
    .expect("non-cancellable waterline should never be cancelled")
}

/// `boundary_regions` (P2.3): after each Z level's closed contours come back
/// from [`waterline_contours_with_cancel`], every contour is run through the
/// shared run-splitter with a "point is inside a boundary region" keep
/// predicate. A contour where every point survives is still a genuine
/// closed loop — emitted via [`Toolpath::emit_closed_contour_with_intent`]
/// exactly as before. A contour with only a partial survivor set is an open
/// run: emitted via [`Toolpath::emit_path_segment_with_intent`] instead, so
/// the excluded stretch becomes a retract/replunge gap rather than a
/// straight-line close across material outside the boundary. `None`
/// reproduces today's unconditional closed-contour emission byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn waterline_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    params: &WaterlineParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut toolpath = Toolpath::new();

    for z in waterline_z_levels(start_z, final_z, z_step) {
        check_cancel(cancel)?;
        let mut contours =
            waterline_contours_with_cancel(mesh, index, cutter, z, params.sampling, cancel)?;

        // D-16.2: the whole contour rides up by `stock_to_leave`. Applied to
        // the CL points BEFORE the boundary run-splitter and the
        // minimum-segment floor, so neither the run topology nor the emitted
        // point count can change with the dial — only Z. Skipped entirely at
        // `0.0`, so the pre-fix path is byte-identical there.
        if params.stock_to_leave != 0.0 {
            for contour in &mut contours {
                for p in contour.iter_mut() {
                    p.z += params.stock_to_leave;
                }
            }
        }

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }
            match boundary_regions {
                None => {
                    let path = floor_contour(contour, true);
                    if path.len() < 3 {
                        continue;
                    }
                    toolpath.emit_closed_contour_with_intent(
                        &path,
                        params.safe_z,
                        params.feed_rate,
                        params.plunge_rate,
                        MoveIntent::FinishingCut,
                    );
                }
                Some(regions) => {
                    let runs = crate::point_runs::split_runs(
                        contour,
                        |_, p: &P3| regions.contains(&P2::new(p.x, p.y)),
                        crate::point_runs::RunTopology::Closed,
                        2,
                    );
                    for run in runs {
                        if run.len() < 2 {
                            continue;
                        }
                        // Decided on the RAW run: whether this is a whole
                        // surviving loop is a question about the boundary
                        // filter, not about the degeneracy floor.
                        let whole_loop = run.len() == contour.len();
                        let path = floor_contour(&run, whole_loop);
                        if path.len() < 2 {
                            continue;
                        }
                        if whole_loop {
                            // The whole loop survived — safe to close it.
                            toolpath.emit_closed_contour_with_intent(
                                &path,
                                params.safe_z,
                                params.feed_rate,
                                params.plunge_rate,
                                MoveIntent::FinishingCut,
                            );
                        } else {
                            // Partial survivor: an open run, not a closed
                            // loop — closing it would chord straight across
                            // the excluded stretch.
                            toolpath.emit_path_segment_with_intent(
                                &path,
                                params.safe_z,
                                params.feed_rate,
                                params.plunge_rate,
                                MoveIntent::FinishingCut,
                            );
                        }
                    }
                }
            }
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
            stock_to_leave: 0.0,
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

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn waterline_boundary_regions_none_matches_call_without_param() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        let tp_default = waterline_toolpath(&mesh, &index, &tool, 15.0, 5.0, 5.0, &params);
        let tp_none = waterline_toolpath_with_cancel(
            &mesh,
            &index,
            &tool,
            15.0,
            5.0,
            5.0,
            &params,
            None,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(tp_default.moves.len(), tp_none.moves.len());
        for (a, b) in tp_default.moves.iter().zip(tp_none.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-9);
            assert!((a.target.y - b.target.y).abs() < 1e-9);
            assert!((a.target.z - b.target.z).abs() < 1e-9);
        }
    }

    #[test]
    fn waterline_boundary_regions_confines_cuts_to_region() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        // Left half of the hemisphere's XY footprint.
        let bbox = &mesh.bbox;
        let left_half = crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(bbox.min.x, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.max.y),
            crate::geo::P2::new(bbox.min.x, bbox.max.y),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let tp = waterline_toolpath_with_cancel(
            &mesh,
            &index,
            &tool,
            15.0,
            5.0,
            5.0,
            &params,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            if let MoveType::Linear { .. } = m.move_type {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "feed move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one feed move");
    }
}
