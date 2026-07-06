//! Radial finishing strategy: spoke-like passes radiating from a center point.
//!
//! Generates spokes at regular angular intervals from the bounding box center
//! outward. Each spoke samples the surface via drop-cutter to produce Z heights.
//! Adjacent spokes alternate direction (center-to-edge, then edge-to-center)
//! for efficient zigzag linking with rapid retracts between spokes.

use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::region_set::RegionSet;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Parameters for the radial finishing operation.
pub struct RadialFinishParams {
    /// Degrees between adjacent spokes (default: 5.0).
    pub angular_step: f64,
    /// Distance in mm between sample points along each spoke (default: 0.5).
    pub point_spacing: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for entry moves (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid positioning (mm).
    pub safe_z: f64,
    /// Stock to leave on the surface (mm). Added to drop-cutter Z so the
    /// tool stays above the surface rather than cutting into it.
    pub stock_to_leave: f64,
}

impl Default for RadialFinishParams {
    fn default() -> Self {
        Self {
            angular_step: 5.0,
            point_spacing: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        }
    }
}

/// Generate a radial finishing toolpath over a mesh.
///
/// Spokes radiate from the XY center of the mesh bounding box outward to the
/// perimeter. Each spoke is sampled at `point_spacing` intervals and Z heights
/// come from `point_drop_cutter`. Even-numbered spokes run center-to-edge;
/// odd-numbered spokes run edge-to-center (zigzag linking). Between spokes
/// the tool rapids to `safe_z`.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn radial_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RadialFinishParams,
) -> Toolpath {
    let never_cancel = || false;
    radial_finish_toolpath_with_cancel(mesh, index, cutter, params, None, &never_cancel)
        .expect("non-cancellable radial finish toolpath should never be cancelled")
}

/// Cancellable variant of [`radial_finish_toolpath`]. Polls `cancel` once
/// per spoke (each spoke samples `point_spacing`-spaced drop-cutter queries
/// out to the mesh's max radius, which is the expensive part).
///
/// `boundary_regions` (P2.3): when `Some`, a sample point outside every
/// region is skipped BEFORE the drop-cutter query (the region check is a
/// cheap XY containment test; the query is the expensive step this pass
/// pre-clips generation to avoid wasting) — the point is treated exactly
/// like a non-contacted point (falls to the min-Z sentinel and gets
/// excluded by the existing contiguous-run split below). Regions are
/// already dilated by fine-tool radius + margin at derivation, and exact
/// containment is still enforced by the post-generation boundary clip —
/// this is a conservative superset filter for performance, not the source
/// of truth for correctness. `None` reproduces today's full-mesh sampling
/// byte-for-byte.
pub fn radial_finish_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RadialFinishParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    let bbox = &mesh.bbox;
    let cx = (bbox.min.x + bbox.max.x) * 0.5;
    let cy = (bbox.min.y + bbox.max.y) * 0.5;
    let max_radius = bbox.max_corner_distance_xy(cx, cy);

    let min_z_fallback = bbox.min.z - 1000.0;
    let num_spokes = (360.0 / params.angular_step).ceil() as usize;

    let mut tp = Toolpath::new();

    for spoke_idx in 0..num_spokes {
        check_cancel(cancel)?;
        let angle_deg = spoke_idx as f64 * params.angular_step;
        let angle_rad = angle_deg.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // Sample points along the spoke from center to perimeter.
        let num_points = (max_radius / params.point_spacing).ceil() as usize + 1;
        let mut spoke_points: Vec<P3> = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let r = i as f64 * params.point_spacing;
            let x = cx + r * cos_a;
            let y = cy + r * sin_a;
            let in_region = boundary_regions.is_none_or(|regions| regions.contains(&P2::new(x, y)));
            let z = if in_region {
                let cl = point_drop_cutter(x, y, mesh, index, cutter);
                if cl.contacted {
                    cl.z + params.stock_to_leave
                } else {
                    // Point is outside the mesh footprint; use fallback Z clamped to min_z.
                    min_z_fallback
                }
            } else {
                // Outside every machining-boundary region — skip the
                // drop-cutter query entirely and treat it as a gap, same as
                // a non-contacted point.
                min_z_fallback
            };
            spoke_points.push(P3::new(x, y, z));
        }

        // Split into contiguous runs of mesh-contacted points. A spoke can
        // cross an interior gap (a hole or notch) as well as have
        // uncontacted leading/trailing tails; each contacted run is emitted
        // as its own path segment so the tool retracts between runs instead
        // of feeding through the gap at the fallback Z.
        let mut runs = crate::point_runs::split_runs(
            &spoke_points,
            |_, p: &P3| (p.z - min_z_fallback).abs() > 0.001,
            crate::point_runs::RunTopology::Open,
            2,
        );

        if runs.is_empty() {
            continue;
        }

        // Zigzag: odd spokes go edge-to-center. Reverse both the run order
        // and each run's points so the overall traversal direction flips,
        // same as reversing the whole point sequence would.
        if spoke_idx % 2 == 1 {
            runs.reverse();
            for run in &mut runs {
                run.reverse();
            }
        }

        for run in &runs {
            tp.emit_path_segment_with_intent(
                run,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
    }

    tp.final_retract(params.safe_z);
    Ok(tp)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::geo::BoundingBox3;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    /// Build a flat 100x100 mm mesh at z=0, centered at origin.
    fn flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(100.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    /// Build a mesh made of two disjoint flat patches at z=0, separated by
    /// an interior gap, so a spoke pointed along +X crosses:
    /// contacted (hub, x in [-11,11]) → gap (x in (11,29)) → contacted
    /// (outer patch, x in [29,51]) → gap (past the outer patch's edge).
    /// Two unreferenced padding vertices at the far corners keep the mesh
    /// bounding box (and hence the spoke center) symmetric about the origin.
    fn gapped_spoke_mesh() -> (TriangleMesh, SpatialIndex) {
        let z = 0.0;
        let mut vertices = vec![
            // Hub patch: x,y in [-11, 11].
            P3::new(-11.0, -11.0, z),
            P3::new(11.0, -11.0, z),
            P3::new(11.0, 11.0, z),
            P3::new(-11.0, 11.0, z),
            // Outer patch: x,y in [29, 51].
            P3::new(29.0, -11.0, z),
            P3::new(51.0, -11.0, z),
            P3::new(51.0, 11.0, z),
            P3::new(29.0, 11.0, z),
        ];
        let triangles = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]];
        // Padding-only vertices (not part of any triangle) to make the mesh
        // bounding box symmetric about the origin.
        vertices.push(P3::new(-51.0, -51.0, z));
        vertices.push(P3::new(51.0, 51.0, z));

        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    fn default_params() -> RadialFinishParams {
        RadialFinishParams {
            angular_step: 30.0, // coarse for fast tests
            point_spacing: 2.0,
            safe_z: 20.0,
            ..RadialFinishParams::default()
        }
    }

    // ── BoundingBox3::max_corner_distance_xy tests (shared geo.rs helper) ──

    #[test]
    fn test_max_radius_square_centered() {
        let bbox = BoundingBox3 {
            min: P3::new(-50.0, -50.0, 0.0),
            max: P3::new(50.0, 50.0, 10.0),
        };
        let r = bbox.max_corner_distance_xy(0.0, 0.0);
        // Diagonal of 100x100 square / 2 = 50*sqrt(2) ~ 70.71
        assert!((r - 70.710).abs() < 0.1, "Expected ~70.71, got {:.2}", r);
    }

    #[test]
    fn test_max_radius_off_center() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        // Center at (5, 5), farthest corner is any corner at distance 5*sqrt(2)
        let r = bbox.max_corner_distance_xy(5.0, 5.0);
        assert!((r - 7.071).abs() < 0.1, "Expected ~7.07, got {:.2}", r);
    }

    // ── Integration tests ────────────────────────────────────────────

    #[test]
    fn test_radial_flat_produces_moves() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = default_params();

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Flat mesh radial should produce moves, got {}",
            tp.moves.len()
        );
    }

    #[test]
    fn test_radial_flat_cutting_distance() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = default_params();

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.total_cutting_distance() > 50.0,
            "Should have meaningful cutting distance over flat 100mm mesh, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_radial_spoke_count() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = RadialFinishParams {
            angular_step: 90.0, // exactly 4 spokes
            point_spacing: 2.0,
            safe_z: 20.0,
            ..RadialFinishParams::default()
        };

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);

        // Count rapids to safe_z as spoke transitions.
        // emit_path_segment emits: rapid(safe_z) + plunge + feeds + retract(safe_z)
        // So we expect 4 spokes = 4 rapid-to-safe_z entries (the approach rapids).
        // Plus final_retract may add one more if needed.
        let rapid_count = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, crate::toolpath::MoveType::Rapid)
                    && (m.target.z - params.safe_z).abs() < 0.01
            })
            .count();
        // Each spoke produces 2 rapids at safe_z (approach + retract), so 4 spokes = 8.
        // final_retract might not add one if last move is already at safe_z.
        assert!(
            rapid_count >= 8,
            "4 spokes should produce at least 8 safe_z rapids, got {}",
            rapid_count
        );
    }

    #[test]
    fn test_radial_z_at_surface() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = RadialFinishParams {
            angular_step: 90.0,
            point_spacing: 5.0,
            safe_z: 20.0,
            stock_to_leave: 0.0,
            ..RadialFinishParams::default()
        };

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);

        // The flat mesh is at z=0. With a ball endmill, the tool tip CL point
        // should be at z = -R (the ball center touches the surface, tip is R below).
        // For ball endmill diameter 6.35, R = 3.175.
        // Feed moves (not rapids) should have Z near the surface.
        let feed_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .collect();
        assert!(!feed_moves.is_empty(), "Should have feed moves");

        // Ball nose on flat surface: CL.z = surface_z + 0 (ball vertex_drop on flat
        // gives z = surface_z for the CL point because vertex_drop lifts by R, but
        // for a flat surface the contact is at the tip). Actually for a flat surface
        // facet_drop returns z = surface_z. Check that feed Z values are near 0.
        for m in &feed_moves {
            assert!(
                m.target.z > -5.0 && m.target.z < 5.0,
                "Feed Z should be near surface (0), got {:.2}",
                m.target.z
            );
        }
    }

    #[test]
    fn test_radial_stock_to_leave() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();

        let params_no_stl = RadialFinishParams {
            angular_step: 90.0,
            point_spacing: 5.0,
            safe_z: 20.0,
            stock_to_leave: 0.0,
            ..RadialFinishParams::default()
        };
        let tp_no_stl = radial_finish_toolpath(&mesh, &si, &cutter, &params_no_stl);

        let params_stl = RadialFinishParams {
            stock_to_leave: 1.0,
            ..params_no_stl
        };
        let tp_stl = radial_finish_toolpath(&mesh, &si, &cutter, &params_stl);

        // With stock_to_leave=1.0, the tool must stay 1mm ABOVE the surface
        // (positive stock_to_leave leaves material — the cutter never dips
        // below the s=0 baseline).
        let avg_z_no_stl = avg_feed_z(&tp_no_stl);
        let avg_z_stl = avg_feed_z(&tp_stl);

        let diff = avg_z_stl - avg_z_no_stl;
        assert!(
            (diff - 1.0).abs() < 0.1,
            "stock_to_leave=1.0 should shift Z up by ~1mm, got diff={:.3}",
            diff
        );
        assert!(
            avg_z_stl + 1e-6 >= avg_z_no_stl,
            "stock_to_leave should never move the cutter below the s=0 baseline: no_stl={:.3}, stl={:.3}",
            avg_z_no_stl,
            avg_z_stl
        );
    }

    #[test]
    fn test_radial_zigzag_direction() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = RadialFinishParams {
            angular_step: 90.0,
            point_spacing: 2.0,
            safe_z: 20.0,
            ..RadialFinishParams::default()
        };

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);

        // Extract the first feed point of each spoke (after rapid+plunge).
        // Even spokes start near center, odd spokes start near edge.
        // We identify spoke boundaries by rapids to safe_z.
        let spokes = extract_spoke_feeds(&tp, params.safe_z);
        assert!(spokes.len() >= 4, "Expected 4 spokes, got {}", spokes.len());

        let cx = 0.0;
        let cy = 0.0;

        // For spokes 0 (even) and 1 (odd), check starting distance from center.
        if spokes.len() >= 2 && !spokes[0].is_empty() && !spokes[1].is_empty() {
            let dist_start_0 =
                ((spokes[0][0].x - cx).powi(2) + (spokes[0][0].y - cy).powi(2)).sqrt();
            let dist_start_1 =
                ((spokes[1][0].x - cx).powi(2) + (spokes[1][0].y - cy).powi(2)).sqrt();

            // Even spoke starts near center (small distance), odd near edge (large distance).
            assert!(
                dist_start_0 < dist_start_1,
                "Even spoke should start nearer to center ({:.1}) than odd spoke ({:.1})",
                dist_start_0,
                dist_start_1
            );
        }
    }

    #[test]
    fn test_radial_interior_gap_no_deep_dive() {
        let (mesh, si) = gapped_spoke_mesh();
        let cutter = ball_cutter();
        let params = RadialFinishParams {
            angular_step: 360.0, // a single spoke, along +X from center.
            point_spacing: 2.0,
            safe_z: 20.0,
            ..RadialFinishParams::default()
        };

        let tp = radial_finish_toolpath(&mesh, &si, &cutter, &params);

        // No move (rapid or feed) should ever approach the 1000mm fallback
        // sentinel depth. A generous margin (50mm) is used so this catches
        // any regression of the sentinel dive without being sensitive to
        // legitimate cutter geometry near the surface.
        let worst_z = tp
            .moves
            .iter()
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        assert!(
            worst_z > mesh.bbox.min.z - 50.0,
            "no move should dive toward the min_z_fallback sentinel; worst Z = {:.1}",
            worst_z
        );

        // The interior gap must split the spoke into at least two separate
        // cutting segments (no feed move should bridge the gap directly).
        let linking_count = tp
            .moves
            .iter()
            .filter(|m| m.intent == crate::toolpath::MoveIntent::Linking)
            .count();
        assert!(
            linking_count >= 2,
            "interior gap should split the spoke into >= 2 segments, got {} Linking rapids",
            linking_count
        );
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn radial_boundary_regions_none_matches_call_without_param() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = default_params();
        let never_cancel = || false;

        let tp_default = radial_finish_toolpath(&mesh, &si, &cutter, &params);
        let tp_none =
            radial_finish_toolpath_with_cancel(&mesh, &si, &cutter, &params, None, &never_cancel)
                .unwrap();

        assert_eq!(tp_default.moves.len(), tp_none.moves.len());
        for (a, b) in tp_default.moves.iter().zip(tp_none.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-9);
            assert!((a.target.y - b.target.y).abs() < 1e-9);
            assert!((a.target.z - b.target.z).abs() < 1e-9);
        }
    }

    #[test]
    fn radial_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = flat_mesh();
        let cutter = ball_cutter();
        let params = default_params();
        let never_cancel = || false;

        // Left half of the 100mm flat mesh (bbox [-50,50]).
        let left_half = crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(-50.0, -50.0),
            crate::geo::P2::new(0.0, -50.0),
            crate::geo::P2::new(0.0, 50.0),
            crate::geo::P2::new(-50.0, 50.0),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let tp = radial_finish_toolpath_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
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

    // ── Helpers ──────────────────────────────────────────────────────

    fn avg_feed_z(tp: &Toolpath) -> f64 {
        let feed_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect();
        if feed_zs.is_empty() {
            return 0.0;
        }
        feed_zs.iter().sum::<f64>() / feed_zs.len() as f64
    }

    /// Extract the feed-move points grouped by spoke.
    /// Spokes are delimited by rapids to safe_z.
    fn extract_spoke_feeds(tp: &Toolpath, _safe_z: f64) -> Vec<Vec<P3>> {
        let mut spokes: Vec<Vec<P3>> = Vec::new();
        let mut current: Vec<P3> = Vec::new();

        for m in &tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if !current.is_empty() {
                        spokes.push(std::mem::take(&mut current));
                    }
                }
                crate::toolpath::MoveType::Linear { .. } => {
                    current.push(m.target);
                }
                _ => {}
            }
        }
        if !current.is_empty() {
            spokes.push(current);
        }
        spokes
    }
}
