//! Radial finishing strategy: spoke-like passes radiating from a center point.
//!
//! Generates spokes at regular angular intervals from the bounding box center
//! outward. Each spoke samples the surface via drop-cutter to produce Z heights.
//! Adjacent spokes alternate direction (center-to-edge, then edge-to-center)
//! for efficient zigzag linking with rapid retracts between spokes.

use crate::dropcutter::point_drop_cutter;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
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
    /// Stock to leave on the surface (mm). Subtracted from drop-cutter Z.
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
pub fn radial_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RadialFinishParams,
) -> Toolpath {
    let bbox = &mesh.bbox;
    let cx = (bbox.min.x + bbox.max.x) * 0.5;
    let cy = (bbox.min.y + bbox.max.y) * 0.5;
    let max_radius = compute_max_radius(bbox, cx, cy);

    let min_z_fallback = bbox.min.z - 1000.0;
    let num_spokes = (360.0 / params.angular_step).ceil() as usize;

    let mut tp = Toolpath::new();

    for spoke_idx in 0..num_spokes {
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
            let cl = point_drop_cutter(x, y, mesh, index, cutter);
            let z = if cl.contacted {
                cl.z - params.stock_to_leave
            } else {
                // Point is outside the mesh footprint; use fallback Z clamped to min_z.
                min_z_fallback
            };
            spoke_points.push(P3::new(x, y, z));
        }

        // Filter out points that fell through (no mesh contact) at the edges.
        // Keep the longest contiguous run of contacted points.
        let spoke_points = trim_uncontacted(&spoke_points, min_z_fallback);

        if spoke_points.len() < 2 {
            continue;
        }

        // Zigzag: odd spokes go edge-to-center (reverse direction).
        let spoke_points = if spoke_idx % 2 == 1 {
            let mut reversed = spoke_points;
            reversed.reverse();
            reversed
        } else {
            spoke_points
        };

        tp.emit_path_segment(
            &spoke_points,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
        );
    }

    tp.final_retract(params.safe_z);
    tp
}

/// Compute the maximum radius from center to any corner of the bounding box.
fn compute_max_radius(bbox: &BoundingBox3, cx: f64, cy: f64) -> f64 {
    let corners = [
        (bbox.min.x, bbox.min.y),
        (bbox.max.x, bbox.min.y),
        (bbox.max.x, bbox.max.y),
        (bbox.min.x, bbox.max.y),
    ];
    let mut max_r2: f64 = 0.0;
    for (x, y) in &corners {
        let dx = x - cx;
        let dy = y - cy;
        max_r2 = max_r2.max(dx * dx + dy * dy);
    }
    max_r2.sqrt()
}

/// Trim leading and trailing points that have no mesh contact (Z at fallback).
///
/// Returns the longest prefix/suffix-trimmed slice of contacted points.
fn trim_uncontacted(points: &[P3], fallback_z: f64) -> Vec<P3> {
    let is_contacted = |p: &P3| (p.z - fallback_z).abs() > 0.001;

    let start = match points.iter().position(&is_contacted) {
        Some(i) => i,
        None => return Vec::new(),
    };

    // Safe: we know at least one point is contacted, so rposition will find it.
    let end = match points.iter().rposition(is_contacted) {
        Some(i) => i,
        None => return Vec::new(),
    };

    points[start..=end].to_vec()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    /// Build a flat 100x100 mm mesh at z=0, centered at origin.
    fn flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(100.0);
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

    // ── compute_max_radius tests ─────────────────────────────────────

    #[test]
    fn test_max_radius_square_centered() {
        let bbox = BoundingBox3 {
            min: P3::new(-50.0, -50.0, 0.0),
            max: P3::new(50.0, 50.0, 10.0),
        };
        let r = compute_max_radius(&bbox, 0.0, 0.0);
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
        let r = compute_max_radius(&bbox, 5.0, 5.0);
        assert!((r - 7.071).abs() < 0.1, "Expected ~7.07, got {:.2}", r);
    }

    // ── trim_uncontacted tests ───────────────────────────────────────

    #[test]
    fn test_trim_all_contacted() {
        let pts = vec![
            P3::new(0.0, 0.0, 5.0),
            P3::new(1.0, 0.0, 5.0),
            P3::new(2.0, 0.0, 5.0),
        ];
        let trimmed = trim_uncontacted(&pts, -1000.0);
        assert_eq!(trimmed.len(), 3);
    }

    #[test]
    fn test_trim_leading_trailing() {
        let fallback = -1000.0;
        let pts = vec![
            P3::new(0.0, 0.0, fallback),
            P3::new(1.0, 0.0, 5.0),
            P3::new(2.0, 0.0, 5.0),
            P3::new(3.0, 0.0, fallback),
        ];
        let trimmed = trim_uncontacted(&pts, fallback);
        assert_eq!(trimmed.len(), 2);
        assert!((trimmed[0].x - 1.0).abs() < 1e-10);
        assert!((trimmed[1].x - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_trim_none_contacted() {
        let fallback = -1000.0;
        let pts = vec![P3::new(0.0, 0.0, fallback), P3::new(1.0, 0.0, fallback)];
        let trimmed = trim_uncontacted(&pts, fallback);
        assert!(trimmed.is_empty());
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

        // With stock_to_leave=1.0, Z values should be 1mm lower (further from surface).
        let avg_z_no_stl = avg_feed_z(&tp_no_stl);
        let avg_z_stl = avg_feed_z(&tp_stl);

        let diff = avg_z_no_stl - avg_z_stl;
        assert!(
            (diff - 1.0).abs() < 0.1,
            "stock_to_leave=1.0 should shift Z down by ~1mm, got diff={:.3}",
            diff
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
