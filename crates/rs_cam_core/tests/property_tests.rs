//! Property-based tests for geometric invariants.
//!
//! Since proptest and rand are not available as dev-dependencies, these tests
//! use deterministic edge-case inputs that cover a range of shapes and sizes
//! to verify geometric invariants hold.
#![allow(clippy::unwrap_used, clippy::panic)]

use rs_cam_core::geo::P2;
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::profile::{ProfileParams, ProfileSide, profile_toolpath};
use rs_cam_core::toolpath::MoveType;
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};

/// Helper: create a regular polygon (approximating a circle) with given center,
/// radius, and number of vertices. Winding is CCW.
fn regular_polygon(cx: f64, cy: f64, radius: f64, n: usize) -> Polygon2 {
    let pts: Vec<P2> = (0..n)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            P2::new(cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect();
    Polygon2::new(pts)
}

/// Helper: collection of test polygons with varying shapes and sizes.
fn test_polygons() -> Vec<(&'static str, Polygon2)> {
    vec![
        ("small_square", Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)),
        (
            "large_rectangle",
            Polygon2::rectangle(0.0, 0.0, 100.0, 50.0),
        ),
        ("thin_rectangle", Polygon2::rectangle(0.0, 0.0, 80.0, 8.0)),
        ("circle_16", regular_polygon(50.0, 50.0, 25.0, 16)),
        ("circle_64", regular_polygon(0.0, 0.0, 40.0, 64)),
        (
            "triangle",
            Polygon2::new(vec![
                P2::new(0.0, 0.0),
                P2::new(60.0, 0.0),
                P2::new(30.0, 50.0),
            ]),
        ),
        (
            "l_shape",
            Polygon2::new(vec![
                P2::new(0.0, 0.0),
                P2::new(40.0, 0.0),
                P2::new(40.0, 20.0),
                P2::new(20.0, 20.0),
                P2::new(20.0, 40.0),
                P2::new(0.0, 40.0),
            ]),
        ),
        (
            "offset_square",
            Polygon2::rectangle(100.0, 100.0, 150.0, 150.0),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Property 1: Toolpath bounds — pocket toolpath stays within polygon bbox + tool_radius
// ---------------------------------------------------------------------------

#[test]
fn test_pocket_toolpath_within_bounds() {
    let tool_radius = 3.175; // 1/4" endmill
    let tolerance = 0.5; // small tolerance for offset geometry

    for (name, polygon) in test_polygons() {
        let params = PocketParams {
            tool_radius,
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        };

        let tp = pocket_toolpath(&polygon, &params);
        if tp.moves.is_empty() {
            continue; // polygon too small for tool, which is fine
        }

        // Compute polygon bounding box
        let xs: Vec<f64> = polygon.exterior.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = polygon.exterior.iter().map(|p| p.y).collect();
        let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // All cutting moves should be within bbox + tool_radius + tolerance
        // (Rapid moves at safe_z are allowed to be anywhere, so we skip them.)
        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                continue;
            }
            let x = m.target.x;
            let y = m.target.y;
            assert!(
                x >= min_x - tool_radius - tolerance
                    && x <= max_x + tool_radius + tolerance
                    && y >= min_y - tool_radius - tolerance
                    && y <= max_y + tool_radius + tolerance,
                "Polygon '{}': cutting move ({:.3}, {:.3}) outside bbox [{:.1}..{:.1}] x [{:.1}..{:.1}] + tool_radius {:.3}",
                name,
                x,
                y,
                min_x,
                max_x,
                min_y,
                max_y,
                tool_radius
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property 2: No NaN in toolpath output
// ---------------------------------------------------------------------------

#[test]
fn test_no_nan_in_pocket_toolpath() {
    // Use a moderate tool radius and stepover to keep runtime reasonable in
    // debug builds (pocket contour generation is O(area / stepover^2)).
    let tool_radii = [3.175, 5.0];

    for (name, polygon) in test_polygons() {
        for &tool_radius in &tool_radii {
            let params = PocketParams {
                tool_radius,
                stepover: tool_radius * 0.8,
                cut_depth: -3.0,
                feed_rate: 1000.0,
                plunge_rate: 500.0,
                safe_z: 10.0,
                climb: false,
            };

            let tp = pocket_toolpath(&polygon, &params);
            for (i, m) in tp.moves.iter().enumerate() {
                assert!(
                    !m.target.x.is_nan() && !m.target.y.is_nan() && !m.target.z.is_nan(),
                    "Polygon '{}' (tool_radius={:.1}): NaN at move {} — ({}, {}, {})",
                    name,
                    tool_radius,
                    i,
                    m.target.x,
                    m.target.y,
                    m.target.z
                );
            }
        }
    }
}

#[test]
fn test_no_nan_in_profile_toolpath() {
    for (name, polygon) in test_polygons() {
        for &side in &[ProfileSide::Inside, ProfileSide::Outside] {
            let params = ProfileParams {
                tool_radius: 3.175,
                side,
                cut_depth: -3.0,
                feed_rate: 1000.0,
                plunge_rate: 500.0,
                safe_z: 10.0,
                climb: false,
            };

            let tp = profile_toolpath(&polygon, &params);
            for (i, m) in tp.moves.iter().enumerate() {
                assert!(
                    !m.target.x.is_nan() && !m.target.y.is_nan() && !m.target.z.is_nan(),
                    "Polygon '{}' (side={:?}): NaN at move {} — ({}, {}, {})",
                    name,
                    side,
                    i,
                    m.target.x,
                    m.target.y,
                    m.target.z
                );
            }
        }
    }
}

#[test]
fn test_no_nan_in_zigzag_toolpath() {
    let angles = [0.0, 45.0, 90.0];
    for (name, polygon) in test_polygons() {
        for &angle in &angles {
            let params = ZigzagParams {
                tool_radius: 3.175,
                stepover: 2.0,
                cut_depth: -3.0,
                feed_rate: 1000.0,
                plunge_rate: 500.0,
                safe_z: 10.0,
                angle,
            };

            let tp = zigzag_toolpath(&polygon, &params);
            for (i, m) in tp.moves.iter().enumerate() {
                assert!(
                    !m.target.x.is_nan() && !m.target.y.is_nan() && !m.target.z.is_nan(),
                    "Polygon '{}' (angle={:.0}): NaN at move {} — ({}, {}, {})",
                    name,
                    angle,
                    i,
                    m.target.x,
                    m.target.y,
                    m.target.z
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property 3: Polygon winding preserved — offset_polygon preserves CCW exterior
// ---------------------------------------------------------------------------

#[test]
fn test_offset_preserves_ccw_winding() {
    let offset_distances = [1.0, 2.0, 5.0];

    for (name, polygon) in test_polygons() {
        // Verify source polygon is CCW
        assert!(
            polygon.signed_area() > 0.0,
            "Test polygon '{}' should be CCW (positive signed area), got {}",
            name,
            polygon.signed_area()
        );

        for &dist in &offset_distances {
            let results = offset_polygon(&polygon, dist);
            for (j, result) in results.iter().enumerate() {
                let area = result.signed_area();
                assert!(
                    area > 0.0,
                    "Polygon '{}' offset by {:.1}: result {} has non-CCW winding (signed_area = {:.4})",
                    name,
                    dist,
                    j,
                    area
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property 4: Inward offset shrinks area
// ---------------------------------------------------------------------------

#[test]
fn test_inward_offset_shrinks_area() {
    let offset_distances = [1.0, 2.0, 3.0, 5.0];

    for (name, polygon) in test_polygons() {
        let original_area = polygon.area();

        for &dist in &offset_distances {
            let results = offset_polygon(&polygon, dist);
            // Total area of all result polygons should be less than original
            let total_offset_area: f64 = results.iter().map(|p| p.area()).sum();

            // If the polygon collapsed entirely, area is 0 which is < original
            assert!(
                total_offset_area < original_area + 1e-6,
                "Polygon '{}' (area={:.2}): inward offset by {:.1} should shrink area, got {:.2}",
                name,
                original_area,
                dist,
                total_offset_area
            );

            // For non-degenerate results, area should be strictly smaller
            if !results.is_empty() && total_offset_area > 0.0 {
                assert!(
                    total_offset_area < original_area - 1e-6,
                    "Polygon '{}': inward offset by {:.1} should strictly shrink (original={:.2}, offset={:.2})",
                    name,
                    dist,
                    original_area,
                    total_offset_area
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property 5: Zigzag bounds — zigzag toolpath stays within polygon bbox + tool_radius
// ---------------------------------------------------------------------------

#[test]
fn test_zigzag_toolpath_within_bounds() {
    let tool_radius = 3.175;
    let tolerance = 0.5;

    for (name, polygon) in test_polygons() {
        let params = ZigzagParams {
            tool_radius,
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            angle: 0.0,
        };

        let tp = zigzag_toolpath(&polygon, &params);
        if tp.moves.is_empty() {
            continue;
        }

        let xs: Vec<f64> = polygon.exterior.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = polygon.exterior.iter().map(|p| p.y).collect();
        let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                continue;
            }
            let x = m.target.x;
            let y = m.target.y;
            assert!(
                x >= min_x - tool_radius - tolerance
                    && x <= max_x + tool_radius + tolerance
                    && y >= min_y - tool_radius - tolerance
                    && y <= max_y + tool_radius + tolerance,
                "Polygon '{}': zigzag cutting move ({:.3}, {:.3}) outside bounds",
                name,
                x,
                y
            );
        }
    }
}
