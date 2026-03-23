//! Rest machining toolpath generation.
//!
//! Generates toolpaths for a smaller tool to clean up material that a
//! larger tool could not reach. Uses geometric comparison: computes the
//! reachable region for the large tool and generates scan-line passes
//! only in areas the large tool missed (inside corners, narrow channels).
//!
//! Algorithm:
//! 1. Offset polygon inward by `prev_tool_radius` → region the large tool
//!    center can reach. Everything here is already cleared.
//! 2. Generate zigzag scan lines for the small tool (offset by `tool_radius`).
//! 3. Walk each line, checking if each sample point is inside the large
//!    tool's reachable region. Cut only where it isn't.
//!
//! Reference: research/raw_algorithms.md §4.5

use crate::geo::{P2, P3};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

/// Parameters for rest machining.
pub struct RestParams {
    /// Previous (larger) tool radius in mm.
    pub prev_tool_radius: f64,
    /// Current (smaller) tool radius in mm.
    pub tool_radius: f64,
    /// Z height to cut at in mm (negative for cuts below stock top).
    pub cut_depth: f64,
    /// Distance between scan lines in mm.
    pub stepover: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Scan line angle in degrees (0 = X axis).
    pub angle: f64,
}

/// Check if a point is inside any polygon in a list.
fn point_in_any_polygon(p: &P2, polygons: &[Polygon2]) -> bool {
    polygons.iter().any(|poly| poly.contains_point(p))
}

/// Generate rest machining toolpath for a 2D polygon region.
///
/// Computes the geometric difference between what the small tool can
/// reach and what the previous large tool could reach. Generates
/// zigzag scan-line passes only in the "rest regions" — corners and
/// narrow areas the large tool missed.
///
/// If the small tool radius >= large tool radius, returns an empty toolpath.
/// If the large tool can't fit at all, falls back to a full zigzag.
pub fn rest_machining_toolpath(polygon: &Polygon2, params: &RestParams) -> Toolpath {
    let mut tp = Toolpath::new();

    if params.tool_radius >= params.prev_tool_radius {
        return tp;
    }

    // What the large tool center could reach (inward offset by large radius)
    let large_reachable = offset_polygon(polygon, params.prev_tool_radius);

    // Generate zigzag scan lines for the small tool
    let lines =
        crate::zigzag::zigzag_lines(polygon, params.tool_radius, params.stepover, params.angle);

    if lines.is_empty() {
        return tp;
    }

    // If large tool can't fit at all, the entire pocket is rest region
    if large_reachable.is_empty() {
        return crate::zigzag::zigzag_toolpath(
            polygon,
            &crate::zigzag::ZigzagParams {
                tool_radius: params.tool_radius,
                stepover: params.stepover,
                cut_depth: params.cut_depth,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                angle: params.angle,
            },
        );
    }

    let sample_step = params.tool_radius.clamp(0.25, 0.5);

    for line in &lines {
        let dx = line[1].x - line[0].x;
        let dy = line[1].y - line[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-10 {
            continue;
        }

        let n_samples = (len / sample_step).ceil() as usize;

        // Walk along the line, collecting segments NOT in large_reachable
        let mut segment_points: Vec<P2> = Vec::new();

        for i in 0..=n_samples {
            let t = i as f64 / n_samples.max(1) as f64;
            let x = line[0].x + t * dx;
            let y = line[0].y + t * dy;
            let p = P2::new(x, y);

            let in_large = point_in_any_polygon(&p, &large_reachable);

            if !in_large {
                segment_points.push(p);
            } else if !segment_points.is_empty() {
                // Exiting rest region — emit the segment
                emit_rest_segment(&mut tp, &segment_points, params);
                segment_points.clear();
            }
        }

        // Emit final segment if line ended in a rest region
        if !segment_points.is_empty() {
            emit_rest_segment(&mut tp, &segment_points, params);
        }
    }

    tp
}

/// Emit a single rest machining cut segment into the toolpath.
fn emit_rest_segment(tp: &mut Toolpath, points: &[P2], params: &RestParams) {
    if points.is_empty() {
        return;
    }
    let z = params.cut_depth;
    let path_3d: Vec<P3> = points.iter().map(|p| P3::new(p.x, p.y, z)).collect();
    tp.emit_path_segment(
        &path_3d,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params() -> RestParams {
        RestParams {
            prev_tool_radius: 6.0, // 12mm large tool
            tool_radius: 1.5,      // 3mm small tool
            cut_depth: -3.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            angle: 0.0,
        }
    }

    // ── Basic behavior tests ────────────────────────────────────────────

    #[test]
    fn test_same_tool_size_empty() {
        let sq = square_polygon(40.0);
        let params = RestParams {
            prev_tool_radius: 3.0,
            tool_radius: 3.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.is_empty(),
            "Same tool size should produce no rest passes"
        );
    }

    #[test]
    fn test_larger_new_tool_empty() {
        let sq = square_polygon(40.0);
        let params = RestParams {
            prev_tool_radius: 3.0,
            tool_radius: 5.0, // bigger than prev
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.is_empty(),
            "Larger new tool should produce no rest passes"
        );
    }

    #[test]
    fn test_large_tool_cant_fit_full_zigzag() {
        // 8mm square, 6mm radius tool can't fit (need at least 12mm)
        let sq = square_polygon(8.0);
        let params = RestParams {
            prev_tool_radius: 6.0,
            tool_radius: 1.5,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.len() > 5,
            "When large tool can't fit, small tool should do full zigzag, got {} moves",
            tp.moves.len()
        );
    }

    // ── Rest region detection ───────────────────────────────────────────

    #[test]
    fn test_rest_regions_at_corners() {
        // 30mm square with 6mm prev tool radius.
        // Large tool (radius 6) leaves material in corners. Small tool (radius 1.5)
        // should generate passes in those corner regions.
        let sq = square_polygon(30.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 5,
            "Should generate rest passes in corners, got {} moves",
            tp.moves.len()
        );

        // Rest passes should have significant cutting distance
        assert!(
            tp.total_cutting_distance() > 5.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_rest_in_wide_open_area_minimal() {
        // Very large polygon (200mm) — almost all is reachable by large tool.
        // Only the corners have rest material. Rest passes should be small
        // relative to the polygon size.
        let sq = square_polygon(200.0);
        let params = default_params();

        let tp = rest_machining_toolpath(&sq, &params);

        // For comparison, generate a full zigzag
        let full_tp = crate::zigzag::zigzag_toolpath(
            &sq,
            &crate::zigzag::ZigzagParams {
                tool_radius: params.tool_radius,
                stepover: params.stepover,
                cut_depth: params.cut_depth,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                angle: 0.0,
            },
        );

        // Rest passes should be much less than full zigzag.
        // The perimeter strip between tool offsets (4.5mm wide all around)
        // accounts for ~25% of scan line crossings on a 200mm square.
        let rest_dist = tp.total_cutting_distance();
        let full_dist = full_tp.total_cutting_distance();
        assert!(
            rest_dist < full_dist * 0.3,
            "Rest cutting ({:.0}mm) should be <30% of full zigzag ({:.0}mm)",
            rest_dist,
            full_dist
        );
    }

    #[test]
    fn test_narrow_channel_all_rest() {
        // 4mm wide channel (rectangle 4×40). 6mm radius tool can't fit at all.
        // Small tool (1.5mm radius) fits. All of it should be rest region.
        let channel = Polygon2::rectangle(-2.0, -20.0, 2.0, 20.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&channel, &params);

        assert!(
            tp.moves.len() > 5,
            "Narrow channel should be fully rest-machined, got {} moves",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 30.0,
            "Should cut most of the channel length, got {:.1}mm",
            tp.total_cutting_distance()
        );
    }

    // ── Toolpath structure tests ────────────────────────────────────────

    #[test]
    fn test_rest_z_correct() {
        let sq = square_polygon(30.0);
        let params = RestParams {
            cut_depth: -5.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);

        // All feed moves should be at cut_depth or safe_z
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    (m.target.z - (-5.0)).abs() < 0.01
                        || (m.target.z - params.safe_z).abs() < 0.01
                        || m.target.z > -5.0, // plunge moves
                    "Feed Z should be at cut_depth or safe_z, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn test_rest_safe_z_retract() {
        let sq = square_polygon(30.0);
        let tp = rest_machining_toolpath(&sq, &default_params());

        // Every rapid should be at safe_z
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Rapid = m.move_type {
                assert!(
                    (m.target.z - 10.0).abs() < 0.01,
                    "Rapids should be at safe_z=10, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Polygon with holes ──────────────────────────────────────────────

    #[test]
    fn test_rest_with_island() {
        // 40mm square with 10mm square hole in center
        let hole = vec![
            P2::new(-5.0, -5.0),
            P2::new(-5.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, -5.0),
        ];
        let poly = Polygon2::with_holes(square_polygon(40.0).exterior, vec![hole]);
        let params = default_params();
        let tp = rest_machining_toolpath(&poly, &params);

        // Should generate rest passes around the island corners
        assert!(
            tp.moves.len() > 5,
            "Should have rest passes near island corners, got {} moves",
            tp.moves.len()
        );
    }

    // ── Geometry correctness ────────────────────────────────────────────

    #[test]
    fn test_rest_stays_in_polygon() {
        let sq = square_polygon(30.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&sq, &params);

        // All XY positions should be inside the polygon (within tool radius of boundary)
        let expanded = offset_polygon(&sq, -params.tool_radius - 0.5);
        for m in &tp.moves {
            if m.target.z < params.safe_z - 1.0 {
                // Cutting move — should be within the small tool offset
                // (zigzag_lines already handles this, but verify)
                let p = P2::new(m.target.x, m.target.y);
                assert!(
                    sq.contains_point(&p) || point_in_any_polygon(&p, &expanded),
                    "Cut at ({:.2}, {:.2}) should be inside polygon",
                    m.target.x,
                    m.target.y
                );
            }
        }
    }

    #[test]
    fn test_point_in_any_polygon_helper() {
        let polys = vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
            Polygon2::rectangle(20.0, 0.0, 30.0, 10.0),
        ];

        assert!(point_in_any_polygon(&P2::new(5.0, 5.0), &polys));
        assert!(point_in_any_polygon(&P2::new(25.0, 5.0), &polys));
        assert!(!point_in_any_polygon(&P2::new(15.0, 5.0), &polys));
    }

    // ── Scan angle ──────────────────────────────────────────────────────

    #[test]
    fn test_rest_angled_scan() {
        let sq = square_polygon(30.0);
        let params = RestParams {
            angle: 45.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 3,
            "Angled scan should still produce rest passes, got {} moves",
            tp.moves.len()
        );
    }
}
