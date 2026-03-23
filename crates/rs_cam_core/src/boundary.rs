//! Machining boundary and tool containment.
//!
//! Provides boundary offset for tool containment modes (center, inside, outside)
//! and toolpath clipping to keep moves within a boundary polygon.

use crate::geo::{P2, P3};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::{MoveType, Toolpath};

/// How the tool relates to the machining boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolContainment {
    /// Tool center stays inside boundary.
    Center,
    /// Entire tool stays inside boundary (inset by tool_radius).
    Inside,
    /// Tool edge can extend outside boundary (outset by tool_radius).
    Outside,
}

/// Compute the effective boundary polygon after applying containment offset.
///
/// - `Center`: returns boundary unchanged
/// - `Inside`: shrinks boundary by `tool_radius` so the full cutter stays within
/// - `Outside`: expands boundary by `tool_radius` so the cutter center can reach the edge
///
/// May return multiple polygons if the offset splits the shape, or empty if it collapses.
pub fn effective_boundary(
    boundary: &Polygon2,
    containment: ToolContainment,
    tool_radius: f64,
) -> Vec<Polygon2> {
    match containment {
        ToolContainment::Center => vec![boundary.clone()],
        // cavalier_contours: positive = inward for CCW exterior
        ToolContainment::Inside => offset_polygon(boundary, tool_radius),
        // negative = outward
        ToolContainment::Outside => offset_polygon(boundary, -tool_radius),
    }
}

/// Add rectangular keep-out zones as holes in a boundary polygon.
///
/// Each keep-out polygon's exterior is reversed to CW winding and added as a hole.
/// The resulting polygon excludes the keep-out regions from `contains_point` checks,
/// which means `clip_toolpath_to_boundary` will automatically retract the tool
/// when entering a keep-out area.
pub fn subtract_keepouts(boundary: &Polygon2, keepouts: &[Polygon2]) -> Polygon2 {
    let mut result = boundary.clone();
    for ko in keepouts {
        let mut hole = ko.exterior.clone();
        hole.reverse();
        result.holes.push(hole);
    }
    result
}

/// Clip a toolpath to stay within a boundary polygon.
///
/// Moves whose target is inside the boundary are kept. Moves that cross from
/// inside to outside get a retract to `safe_z` and become rapids. Moves that
/// cross from outside to inside get a rapid to `safe_z` above the target
/// followed by a plunge.
///
/// The first move is treated as if the previous position was outside the boundary
/// (the tool starts from a safe location).
pub fn clip_toolpath_to_boundary(tp: &Toolpath, boundary: &Polygon2, safe_z: f64) -> Toolpath {
    let mut result = Toolpath::new();

    if tp.moves.is_empty() {
        return result;
    }

    let mut prev_inside = false;
    let mut prev_pos: Option<P3> = None;

    for m in &tp.moves {
        let target_xy = P2::new(m.target.x, m.target.y);
        let cur_inside = boundary.contains_point(&target_xy);

        match (prev_inside, cur_inside) {
            (_, true) if prev_pos.is_none() => {
                // First move, target is inside. Keep it (may be a rapid approach).
                result.moves.push(m.clone());
            }
            (false, true) => {
                // Crossing from outside to inside: rapid above target, then plunge.
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));

                // Preserve feed rate from the original move for the plunge.
                let feed = feed_rate_of(&m.move_type);
                match feed {
                    Some(fr) => result.feed_to(m.target, fr),
                    None => result.rapid_to(m.target),
                }
            }
            (true, true) => {
                // Both inside: keep the move unchanged.
                result.moves.push(m.clone());
            }
            (true, false) => {
                // Crossing from inside to outside: retract, then rapid.
                if let Some(prev) = prev_pos {
                    result.rapid_to(P3::new(prev.x, prev.y, safe_z));
                }
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));
            }
            (false, false) => {
                // Both outside (or first move with outside target):
                // convert to rapid at safe_z.
                result.rapid_to(P3::new(m.target.x, m.target.y, safe_z));
            }
        }

        prev_inside = cur_inside;
        prev_pos = Some(m.target);
    }

    result
}

/// Extract the feed rate from a move type, if it has one.
fn feed_rate_of(mt: &MoveType) -> Option<f64> {
    match mt {
        MoveType::Rapid => None,
        MoveType::Linear { feed_rate } => Some(*feed_rate),
        MoveType::ArcCW { feed_rate, .. } => Some(*feed_rate),
        MoveType::ArcCCW { feed_rate, .. } => Some(*feed_rate),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::toolpath::Move;

    /// Ray-casting point-in-polygon test for a single ring (test helper).
    fn point_in_ring(px: f64, py: f64, ring: &[P2]) -> bool {
        let n = ring.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let pi = &ring[i];
            let pj = &ring[j];
            if ((pi.y > py) != (pj.y > py))
                && (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Full point-in-polygon test: inside exterior and outside all holes (test helper).
    fn point_in_polygon(px: f64, py: f64, polygon: &Polygon2) -> bool {
        if !point_in_ring(px, py, &polygon.exterior) {
            return false;
        }
        !polygon.holes.iter().any(|h| point_in_ring(px, py, h))
    }

    /// Helper: build a CCW square boundary centered at origin.
    fn square_boundary(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    // --- effective_boundary tests ---

    #[test]
    fn test_effective_boundary_center_unchanged() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Center, 3.0);
        assert_eq!(result.len(), 1);
        assert!(
            (result[0].area() - boundary.area()).abs() < 1e-10,
            "Center containment should not change the boundary"
        );
    }

    #[test]
    fn test_effective_boundary_inside_shrinks() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Inside, 2.0);
        assert!(!result.is_empty(), "Inside offset should produce a polygon");

        let total_area: f64 = result.iter().map(|p| p.area()).sum();
        // Original is 400, inset by 2 should be roughly (20-4)^2 = 256 (with rounded corners slightly more)
        assert!(
            total_area < boundary.area(),
            "Inside containment should shrink: got {} vs original {}",
            total_area,
            boundary.area()
        );
        assert!(
            total_area > 200.0,
            "Shrunk area {} is unexpectedly small",
            total_area
        );
    }

    #[test]
    fn test_effective_boundary_outside_expands() {
        let boundary = square_boundary(20.0);
        let result = effective_boundary(&boundary, ToolContainment::Outside, 2.0);
        assert!(
            !result.is_empty(),
            "Outside offset should produce a polygon"
        );

        let total_area: f64 = result.iter().map(|p| p.area()).sum();
        assert!(
            total_area > boundary.area(),
            "Outside containment should expand: got {} vs original {}",
            total_area,
            boundary.area()
        );
    }

    #[test]
    fn test_effective_boundary_inside_collapse() {
        // A 4x4 square with tool_radius=3 should collapse (inset by 3 exceeds half-width of 2)
        let boundary = square_boundary(4.0);
        let result = effective_boundary(&boundary, ToolContainment::Inside, 3.0);
        assert!(
            result.is_empty(),
            "Inside containment that exceeds half-width should collapse"
        );
    }

    // --- clip_toolpath_to_boundary tests ---

    #[test]
    fn test_clip_keeps_moves_inside() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 10.0, 20.0));
        tp.feed_to(P3::new(50.0, 50.0, -5.0), 1000.0);
        tp.feed_to(P3::new(90.0, 50.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, 20.0);

        // All targets are inside the boundary, so all moves should be preserved
        assert_eq!(clipped.moves.len(), tp.moves.len());
        for (orig, clip) in tp.moves.iter().zip(clipped.moves.iter()) {
            assert!(
                (orig.target.x - clip.target.x).abs() < 1e-10
                    && (orig.target.y - clip.target.y).abs() < 1e-10
                    && (orig.target.z - clip.target.z).abs() < 1e-10,
                "Inside moves should be preserved exactly"
            );
        }
    }

    #[test]
    fn test_clip_converts_outside_to_rapids() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let safe_z = 20.0;

        let mut tp = Toolpath::new();
        // Start inside
        tp.rapid_to(P3::new(25.0, 25.0, safe_z));
        tp.feed_to(P3::new(25.0, 25.0, -5.0), 1000.0);
        // Move outside
        tp.feed_to(P3::new(75.0, 25.0, -5.0), 1000.0);
        // Move back inside
        tp.feed_to(P3::new(25.0, 25.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // Check that the outside move (target 75,25) became a rapid at safe_z
        let outside_moves: Vec<&Move> = clipped
            .moves
            .iter()
            .filter(|m| (m.target.x - 75.0).abs() < 1e-10)
            .collect();
        assert!(
            !outside_moves.is_empty(),
            "Should have a move targeting the outside position"
        );
        for m in &outside_moves {
            assert_eq!(
                m.move_type,
                MoveType::Rapid,
                "Outside move should be converted to rapid"
            );
            assert!(
                (m.target.z - safe_z).abs() < 1e-10,
                "Outside move should be at safe_z, got z={}",
                m.target.z
            );
        }
    }

    #[test]
    fn test_clip_empty_toolpath() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let tp = Toolpath::new();
        let clipped = clip_toolpath_to_boundary(&tp, &boundary, 20.0);
        assert!(clipped.moves.is_empty());
    }

    #[test]
    fn test_clip_all_outside() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let safe_z = 20.0;

        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(50.0, 50.0, -5.0), 1000.0);
        tp.feed_to(P3::new(60.0, 50.0, -5.0), 1000.0);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // All moves should be rapids at safe_z
        for m in &clipped.moves {
            assert_eq!(m.move_type, MoveType::Rapid);
            assert!(
                (m.target.z - safe_z).abs() < 1e-10,
                "All-outside moves should be at safe_z"
            );
        }
    }

    #[test]
    fn test_clip_reentry_plunges() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let safe_z = 20.0;
        let feed = 1000.0;

        let mut tp = Toolpath::new();
        // Approach from outside
        tp.rapid_to(P3::new(-10.0, 25.0, safe_z));
        // Enter the boundary
        tp.feed_to(P3::new(25.0, 25.0, -5.0), feed);

        let clipped = clip_toolpath_to_boundary(&tp, &boundary, safe_z);

        // After re-entry, we should see a rapid to safe_z above (25,25) then a plunge
        let has_rapid_above_target = clipped.moves.iter().any(|m| {
            m.move_type == MoveType::Rapid
                && (m.target.x - 25.0).abs() < 1e-10
                && (m.target.y - 25.0).abs() < 1e-10
                && (m.target.z - safe_z).abs() < 1e-10
        });
        assert!(
            has_rapid_above_target,
            "Re-entry should produce a rapid to safe_z above the target"
        );

        let has_plunge = clipped.moves.iter().any(|m| {
            matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - feed).abs() < 1e-10)
                && (m.target.x - 25.0).abs() < 1e-10
                && (m.target.z - (-5.0)).abs() < 1e-10
        });
        assert!(
            has_plunge,
            "Re-entry should produce a plunge to cutting depth"
        );
    }

    // --- point_in_polygon helper tests ---

    #[test]
    fn test_point_in_polygon_inside() {
        let sq = square_boundary(10.0);
        assert!(point_in_polygon(0.0, 0.0, &sq));
        assert!(point_in_polygon(4.0, 4.0, &sq));
    }

    #[test]
    fn test_point_in_polygon_outside() {
        let sq = square_boundary(10.0);
        assert!(!point_in_polygon(10.0, 10.0, &sq));
        assert!(!point_in_polygon(-6.0, 0.0, &sq));
    }

    #[test]
    fn test_point_in_polygon_with_hole() {
        // 20x20 square with 6x6 hole in center
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(square_boundary(20.0).exterior, vec![hole]);
        // Inside exterior but outside hole
        assert!(point_in_polygon(8.0, 0.0, &poly));
        // Inside the hole
        assert!(!point_in_polygon(0.0, 0.0, &poly));
    }

    #[test]
    fn subtract_keepouts_adds_holes() {
        let boundary = Polygon2::rectangle(-50.0, -50.0, 50.0, 50.0);
        let keepout = Polygon2::rectangle(10.0, 10.0, 20.0, 20.0);

        let result = subtract_keepouts(&boundary, &[keepout]);
        assert_eq!(result.holes.len(), 1);

        assert!(result.contains_point(&P2::new(0.0, 0.0)));
        assert!(!result.contains_point(&P2::new(15.0, 15.0)));
    }

    #[test]
    fn subtract_keepouts_multiple() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let ko1 = Polygon2::rectangle(10.0, 10.0, 20.0, 20.0);
        let ko2 = Polygon2::rectangle(70.0, 70.0, 90.0, 90.0);

        let result = subtract_keepouts(&boundary, &[ko1, ko2]);
        assert_eq!(result.holes.len(), 2);

        assert!(result.contains_point(&P2::new(50.0, 50.0)));
        assert!(!result.contains_point(&P2::new(15.0, 15.0)));
        assert!(!result.contains_point(&P2::new(80.0, 80.0)));
    }

    #[test]
    fn subtract_keepouts_empty_is_noop() {
        let boundary = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let result = subtract_keepouts(&boundary, &[]);
        assert_eq!(result.holes.len(), 0);
        assert!(result.contains_point(&P2::new(50.0, 50.0)));
    }
}
