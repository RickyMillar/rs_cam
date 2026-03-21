//! Spiral finishing strategy for 3D surface machining.
//!
//! Generates a continuous Archimedean spiral toolpath from center outward
//! (or reversed for outside-in), drop-cutting each point onto the mesh surface.
//! This produces a single uninterrupted cut with no retract-reposition cycles,
//! ideal for smooth concave surfaces like bowls and dishes.
//!
//! Algorithm:
//! 1. Compute mesh bounding box center and max radius to farthest corner.
//! 2. Walk an Archimedean spiral r(θ) = stepover·θ/(2π) with adaptive angular
//!    stepping for consistent point density.
//! 3. Drop-cutter each spiral point onto the mesh.
//! 4. Build a single continuous toolpath segment.

use crate::dropcutter::point_drop_cutter;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Whether the spiral cuts from center outward or rim inward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiralDirection {
    /// Start at the center and spiral outward to the rim.
    InsideOut,
    /// Start at the rim and spiral inward to the center.
    OutsideIn,
}

/// Parameters for spiral finishing.
pub struct SpiralFinishParams {
    /// Radial distance between adjacent spiral revolutions (mm).
    pub stepover: f64,
    /// Spiral traversal direction.
    pub direction: SpiralDirection,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for initial descent (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid positioning (mm).
    pub safe_z: f64,
    /// Extra material to leave on the surface (mm).
    pub stock_to_leave: f64,
}

/// Generate a spiral finishing toolpath over a 3D mesh.
///
/// Produces an Archimedean spiral of drop-cutter points covering the mesh XY
/// footprint. Points that miss the mesh entirely are skipped.
pub fn spiral_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SpiralFinishParams,
) -> Toolpath {
    let bbox = &mesh.bbox;
    let cx = (bbox.min.x + bbox.max.x) * 0.5;
    let cy = (bbox.min.y + bbox.max.y) * 0.5;

    // Max radius: distance from center to farthest bounding-box corner, plus
    // one cutter radius so the tool fully covers the edge.
    let max_radius = corner_distance(bbox, cx, cy) + cutter.radius();

    // Generate spiral XY coordinates.
    let spiral_xy = generate_spiral_points(cx, cy, max_radius, params.stepover);

    // Drop-cut each point onto the mesh.
    let mut path: Vec<P3> = Vec::with_capacity(spiral_xy.len());
    for (sx, sy) in &spiral_xy {
        let cl = point_drop_cutter(*sx, *sy, mesh, index, cutter);
        if cl.contacted {
            let z = cl.z - params.stock_to_leave;
            path.push(P3::new(cl.x, cl.y, z));
        }
        // Non-contacted points are outside the mesh footprint — skip them.
    }

    if path.is_empty() {
        return Toolpath::new();
    }

    // Reverse for outside-in cutting.
    if params.direction == SpiralDirection::OutsideIn {
        path.reverse();
    }

    // Build toolpath: rapid → plunge → feed → retract.
    let mut tp = Toolpath::new();
    tp.emit_path_segment(&path, params.safe_z, params.feed_rate, params.plunge_rate);
    tp.final_retract(params.safe_z);
    tp
}

// ── helpers ────────────────────────────────────────────────────────────────

/// Distance from (cx,cy) to the farthest XY corner of a 3D bounding box.
fn corner_distance(bbox: &BoundingBox3, cx: f64, cy: f64) -> f64 {
    let corners = [
        (bbox.min.x, bbox.min.y),
        (bbox.max.x, bbox.min.y),
        (bbox.max.x, bbox.max.y),
        (bbox.min.x, bbox.max.y),
    ];
    corners
        .iter()
        .map(|(x, y)| {
            let dx = x - cx;
            let dy = y - cy;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0_f64, f64::max)
}

/// Walk an Archimedean spiral from center (cx,cy) outward, returning (x,y)
/// sample points with approximately `stepover` spacing between adjacent turns.
///
/// r(θ) = stepover · θ / (2π)
///
/// The angular increment is adaptive: dθ = stepover / max(r, stepover) so that
/// the linear spacing between consecutive points stays roughly constant.
fn generate_spiral_points(cx: f64, cy: f64, max_radius: f64, stepover: f64) -> Vec<(f64, f64)> {
    let two_pi = std::f64::consts::TAU;
    // θ_max where r(θ_max) = max_radius  ⟹  θ_max = max_radius * 2π / stepover
    let theta_max = max_radius * two_pi / stepover;

    let mut points = Vec::new();
    let mut theta = 0.0_f64;

    while theta <= theta_max {
        let r = stepover * theta / two_pi;
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin();
        points.push((x, y));

        // Adaptive step: at small r use a minimum to avoid near-zero division.
        let dtheta = stepover / r.max(stepover);
        theta += dtheta;
    }

    // Include the outermost point exactly at max_radius.
    let r_last = stepover * theta_max / two_pi;
    if (r_last - max_radius).abs() > stepover * 0.1 {
        let x = cx + max_radius * theta_max.cos();
        let y = cy + max_radius * theta_max.sin();
        points.push((x, y));
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    /// Build a flat 50×50 mm mesh at z=0 with its spatial index.
    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    // ── Spiral point generation ────────────────────────────────────────

    #[test]
    fn spiral_points_cover_radius() {
        let pts = generate_spiral_points(0.0, 0.0, 20.0, 2.0);
        assert!(
            pts.len() > 50,
            "Should produce many points, got {}",
            pts.len()
        );

        // The last few points should be near the max radius.
        let last = pts.last().expect("non-empty");
        let r_last = (last.0 * last.0 + last.1 * last.1).sqrt();
        assert!(
            r_last >= 18.0,
            "Outermost point should be near max_radius=20, got r={:.2}",
            r_last,
        );
    }

    #[test]
    fn spiral_points_start_at_center() {
        let pts = generate_spiral_points(5.0, 10.0, 20.0, 2.0);
        let first = pts[0];
        assert!(
            (first.0 - 5.0).abs() < 0.01 && (first.1 - 10.0).abs() < 0.01,
            "First point should be at center (5,10), got ({:.2},{:.2})",
            first.0,
            first.1,
        );
    }

    // ── Corner distance helper ─────────────────────────────────────────

    #[test]
    fn corner_distance_square() {
        let bbox = BoundingBox3 {
            min: P3::new(-10.0, -10.0, 0.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let d = corner_distance(&bbox, 0.0, 0.0);
        // Diagonal of a 20×20 square / 2 = √200 ≈ 14.14
        assert!(
            (d - 14.142).abs() < 0.1,
            "Corner distance should be ~14.14, got {:.3}",
            d,
        );
    }

    // ── Full toolpath integration ──────────────────────────────────────

    #[test]
    fn flat_mesh_produces_nonempty_toolpath() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 2.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Flat mesh spiral should produce moves, got {}",
            tp.moves.len(),
        );
        assert!(
            tp.total_cutting_distance() > 50.0,
            "Cutting distance should be substantial, got {:.1}",
            tp.total_cutting_distance(),
        );
    }

    #[test]
    fn outside_in_reverses_direction() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let base = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp_in = spiral_finish_toolpath(&mesh, &si, &cutter, &base);
        let tp_out = spiral_finish_toolpath(
            &mesh,
            &si,
            &cutter,
            &SpiralFinishParams {
                direction: SpiralDirection::OutsideIn,
                ..base
            },
        );

        // Both should have moves.
        assert!(tp_in.moves.len() > 5);
        assert!(tp_out.moves.len() > 5);

        // First cutting move (index 2: after rapid-to-safe-z and rapid-to-XY) should
        // differ in XY between the two directions. Inside-out starts near center,
        // outside-in starts near the rim.
        let first_cut_in = &tp_in.moves[2].target;
        let first_cut_out = &tp_out.moves[2].target;

        let r_in = (first_cut_in.x * first_cut_in.x + first_cut_in.y * first_cut_in.y).sqrt();
        let r_out = (first_cut_out.x * first_cut_out.x + first_cut_out.y * first_cut_out.y).sqrt();

        assert!(
            r_out > r_in + 1.0,
            "OutsideIn first cut should be farther from center: r_in={:.1}, r_out={:.1}",
            r_in,
            r_out,
        );
    }

    #[test]
    fn stock_to_leave_lowers_z() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let base = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        };

        let tp_zero = spiral_finish_toolpath(&mesh, &si, &cutter, &base);
        let tp_leave = spiral_finish_toolpath(
            &mesh,
            &si,
            &cutter,
            &SpiralFinishParams {
                stock_to_leave: 1.0,
                ..base
            },
        );

        // Find the minimum Z among cutting (non-rapid) moves.
        let min_z = |tp: &Toolpath| -> f64 {
            tp.moves
                .iter()
                .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
                .map(|m| m.target.z)
                .fold(f64::INFINITY, f64::min)
        };

        let z0 = min_z(&tp_zero);
        let z1 = min_z(&tp_leave);

        assert!(
            (z0 - z1 - 1.0).abs() < 0.1,
            "stock_to_leave=1 should lower Z by ~1mm: z0={:.3}, z1={:.3}",
            z0,
            z1,
        );
    }

    #[test]
    fn safe_z_respected() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = SpiralFinishParams {
            stepover: 3.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 42.0,
            stock_to_leave: 0.0,
        };

        let tp = spiral_finish_toolpath(&mesh, &si, &cutter, &params);

        // First and last moves should be rapids at safe_z.
        let first = &tp.moves[0];
        assert!(
            (first.target.z - 42.0).abs() < 0.01,
            "First rapid should be at safe_z=42, got {:.2}",
            first.target.z,
        );
        let last = &tp.moves[tp.moves.len() - 1];
        assert!(
            (last.target.z - 42.0).abs() < 0.01,
            "Last rapid should be at safe_z=42, got {:.2}",
            last.target.z,
        );
    }
}
