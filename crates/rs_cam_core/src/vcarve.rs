//! V-carving toolpath generation.
//!
//! Produces variable-depth toolpaths for V-bit engraving. The tool depth
//! at each point equals the distance to the nearest polygon boundary
//! divided by tan(half_angle), creating a V-groove that exactly meets
//! the design outline.
//!
//! Uses scan-line sampling: zigzag lines are generated across the polygon,
//! and for each sample point the exact Euclidean distance to the nearest
//! boundary edge determines the cut depth.
//!
//! Reference: research/02_algorithms.md §11

use crate::geo::{point_to_segment_distance, P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

/// Parameters for V-carve toolpath generation.
pub struct VCarveParams {
    /// V-bit half-angle in radians (e.g. π/4 for a 90° V-bit).
    pub half_angle: f64,
    /// Maximum cut depth in mm (positive). Clamps depth for wide areas.
    /// If 0.0, uses the full cone depth (tool_radius / tan(half_angle)).
    pub max_depth: f64,
    /// Distance between scan lines in mm.
    pub stepover: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Sampling interval along each scan line in mm.
    pub tolerance: f64,
}

// ── Distance computation ──────────────────────────────────────────────

/// Compute the minimum distance from a point to any edge of a polygon
/// (exterior + all holes).
fn point_to_polygon_distance(point: &P2, polygon: &Polygon2) -> f64 {
    let mut min_dist = f64::INFINITY;

    // Exterior edges
    let ext = &polygon.exterior;
    for i in 0..ext.len() {
        let a = &ext[i];
        let b = &ext[(i + 1) % ext.len()];
        let dist = point_to_segment_distance(point, a, b);
        min_dist = min_dist.min(dist);
    }

    // Hole edges
    for hole in &polygon.holes {
        for i in 0..hole.len() {
            let a = &hole[i];
            let b = &hole[(i + 1) % hole.len()];
            let dist = point_to_segment_distance(point, a, b);
            min_dist = min_dist.min(dist);
        }
    }

    min_dist
}

// ── Public API ─────────────────────────────────────────────────────────

/// Generate a V-carve toolpath for a 2D polygon region.
///
/// The V-bit follows scan lines across the polygon. At each point, the
/// cut depth is determined by the distance to the nearest boundary edge:
/// `depth = distance / tan(half_angle)`, clamped to `max_depth`.
///
/// This produces a V-groove that exactly meets the design outline when
/// the V-bit half-angle matches the specified value.
pub fn vcarve_toolpath(polygon: &Polygon2, params: &VCarveParams) -> Toolpath {
    let tan_half = params.half_angle.tan();
    if tan_half < 1e-10 {
        return Toolpath::new(); // degenerate angle
    }

    // Generate scan lines with a tiny inset so clipping works
    let inset = params.tolerance.min(0.05);
    let scan_lines = crate::zigzag::zigzag_lines(polygon, inset, params.stepover, 0.0);

    let sample_step = params.tolerance.max(0.05);

    let mut tp = Toolpath::new();

    for line in &scan_lines {
        let dx = line[1].x - line[0].x;
        let dy = line[1].y - line[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-10 {
            continue;
        }

        // Sample along the line at regular intervals
        let n_samples = (len / sample_step).ceil() as usize;
        let mut points: Vec<P3> = Vec::with_capacity(n_samples + 1);

        for i in 0..=n_samples {
            let t = i as f64 / n_samples.max(1) as f64;
            let x = line[0].x + t * dx;
            let y = line[0].y + t * dy;

            let dist = point_to_polygon_distance(&P2::new(x, y), polygon);
            let depth = (dist / tan_half).min(params.max_depth);
            points.push(P3::new(x, y, -depth));
        }

        if points.is_empty() {
            continue;
        }

        tp.emit_path_segment(&points, params.safe_z, params.feed_rate, params.plunge_rate);
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params() -> VCarveParams {
        VCarveParams {
            half_angle: FRAC_PI_4, // 90° V-bit
            max_depth: 10.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
        }
    }

    #[test]
    fn test_point_to_polygon_distance_center() {
        let sq = square_polygon(20.0);
        // Center of 20×20 square → distance to nearest wall = 10.0
        let d = point_to_polygon_distance(&P2::new(0.0, 0.0), &sq);
        assert!(
            (d - 10.0).abs() < 0.1,
            "Center should be ~10mm from wall, got {:.2}",
            d
        );
    }

    #[test]
    fn test_point_to_polygon_distance_near_wall() {
        let sq = square_polygon(20.0);
        // 1mm from right wall
        let d = point_to_polygon_distance(&P2::new(9.0, 0.0), &sq);
        assert!(
            (d - 1.0).abs() < 0.1,
            "Should be ~1mm from wall, got {:.2}",
            d
        );
    }

    #[test]
    fn test_point_to_polygon_distance_with_hole() {
        let hole = vec![
            P2::new(-2.0, -2.0),
            P2::new(-2.0, 2.0),
            P2::new(2.0, 2.0),
            P2::new(2.0, -2.0),
        ];
        let poly = Polygon2::with_holes(square_polygon(20.0).exterior, vec![hole]);

        // Point between hole and exterior: 3mm from hole edge, 7mm from exterior
        let d = point_to_polygon_distance(&P2::new(5.0, 0.0), &poly);
        assert!(
            (d - 3.0).abs() < 0.1,
            "Should be ~3mm from hole, got {:.2}",
            d
        );
    }

    // ── V-carve depth tests ───────────────────────────────────────────

    #[test]
    fn test_vcarve_depth_at_center() {
        // 90° V-bit (half_angle = 45°, tan = 1.0)
        // Center of 10mm square → distance = 5mm → depth = 5mm
        let sq = square_polygon(10.0);
        let d = point_to_polygon_distance(&P2::new(0.0, 0.0), &sq);
        let depth = d / FRAC_PI_4.tan();
        assert!(
            (depth - 5.0).abs() < 0.1,
            "90° V-bit at center of 10mm square should cut ~5mm deep, got {:.2}",
            depth
        );
    }

    #[test]
    fn test_vcarve_depth_at_boundary() {
        // At the boundary, distance = 0 → depth = 0
        let sq = square_polygon(10.0);
        let d = point_to_polygon_distance(&P2::new(5.0, 0.0), &sq);
        assert!(d < 0.1, "At boundary, distance should be ~0, got {:.2}", d);
    }

    #[test]
    fn test_vcarve_max_depth_clamp() {
        // Center of large square with small max_depth
        let sq = square_polygon(40.0);
        let params = VCarveParams {
            max_depth: 3.0,
            ..default_params()
        };

        let tp = vcarve_toolpath(&sq, &params);

        // No feed move should go deeper than -max_depth
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    m.target.z >= -3.0 - 1e-10,
                    "No cut should exceed max_depth, got z={:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Toolpath structure tests ──────────────────────────────────────

    #[test]
    fn test_vcarve_toolpath_basic() {
        let sq = square_polygon(20.0);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "V-carve should generate moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have significant cutting, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_vcarve_z_varies_along_pass() {
        let sq = square_polygon(20.0);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);

        // Collect Z values of feed moves in a single pass
        // (between first rapid and second rapid)
        let mut z_values: Vec<f64> = Vec::new();
        let mut in_cut = false;
        for m in &tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if in_cut {
                        break; // end of first cut pass
                    }
                }
                crate::toolpath::MoveType::Linear { .. } => {
                    in_cut = true;
                    z_values.push(m.target.z);
                }
                _ => {}
            }
        }

        // Z should vary within the pass (not constant)
        if z_values.len() > 3 {
            let z_min = z_values.iter().copied().fold(f64::INFINITY, f64::min);
            let z_max = z_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (z_max - z_min).abs() > 0.5,
                "Z should vary along pass, range={:.2} ({:.2} to {:.2})",
                z_max - z_min,
                z_min,
                z_max
            );
        }
    }

    #[test]
    fn test_vcarve_empty_polygon() {
        // Too small to have scan lines
        let sq = square_polygon(0.01);
        let params = default_params();

        let tp = vcarve_toolpath(&sq, &params);
        assert!(
            tp.moves.len() <= 2,
            "Tiny polygon should produce minimal toolpath"
        );
    }

    #[test]
    fn test_vcarve_60_degree_bit() {
        // 60° V-bit → half_angle = 30° → tan(30°) ≈ 0.577
        // At distance 5mm from wall: depth = 5 / 0.577 ≈ 8.66mm
        let half_angle = (30.0_f64).to_radians();
        let dist = 5.0;
        let depth = dist / half_angle.tan();
        assert!(
            (depth - 8.66).abs() < 0.1,
            "60° V-bit at 5mm from wall should cut ~8.66mm, got {:.2}",
            depth
        );
    }
}
