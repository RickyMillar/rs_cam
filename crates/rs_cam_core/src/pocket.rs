//! Contour-parallel pocket clearing operation.
//!
//! Takes a 2D polygon boundary and generates a toolpath that clears material
//! using concentric inward offsets. Tool radius compensation is applied
//! automatically so the tool edge follows the pocket wall.

use crate::geo::{P2, P3};
use crate::polygon::{offset_polygon, Polygon2};
use crate::toolpath::Toolpath;

/// Parameters for pocket clearing.
pub struct PocketParams {
    /// Tool radius in mm (half of tool diameter).
    pub tool_radius: f64,
    /// Distance between concentric passes in mm.
    pub stepover: f64,
    /// Z height of the cut in mm (typically negative, e.g. -3.0 for 3mm deep).
    pub cut_depth: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Climb milling: true = CW tool direction (climb), false = CCW (conventional).
    pub climb: bool,
}

/// Generate a contour-parallel pocket clearing toolpath.
///
/// Process:
/// 1. Offset boundary inward by tool radius (tool compensation)
/// 2. Generate concentric inward offsets by stepover
/// 3. Convert contours to toolpath with rapids, plunges, and contour following
///
/// Contours are cut from outermost to innermost (finish pass first).
/// Returns an empty toolpath if the pocket is too small for the tool.
pub fn pocket_toolpath(polygon: &Polygon2, params: &PocketParams) -> Toolpath {
    let contours = pocket_contours(polygon, params.tool_radius, params.stepover);
    contours_to_toolpath(&contours, params)
}

/// Generate the 2D contour rings for pocket clearing (no Z, no toolpath yet).
///
/// Useful for visualization or when you need the geometry separately.
pub fn pocket_contours(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
) -> Vec<Vec<P2>> {
    // First offset: tool radius compensation (tool edge touches wall)
    let compensated = offset_polygon(polygon, tool_radius);

    let mut all_contours: Vec<Vec<P2>> = Vec::new();

    for comp in &compensated {
        if comp.exterior.len() < 3 {
            continue;
        }
        // Compensated boundary is the outermost cutting contour
        all_contours.push(comp.exterior.clone());

        // Include hole contours (reversed to CCW so direction logic works uniformly)
        for hole in &comp.holes {
            if hole.len() >= 3 {
                let mut reversed = hole.clone();
                reversed.reverse(); // CW→CCW
                all_contours.push(reversed);
            }
        }

        // Generate inner contours by repeated stepover offset
        let mut current = vec![comp.clone()];
        loop {
            let mut next = Vec::new();
            for poly in &current {
                for inner in offset_polygon(poly, stepover) {
                    if inner.exterior.len() >= 3 {
                        all_contours.push(inner.exterior.clone());

                        // Include hole contours from inner offsets
                        for hole in &inner.holes {
                            if hole.len() >= 3 {
                                let mut reversed = hole.clone();
                                reversed.reverse(); // CW→CCW
                                all_contours.push(reversed);
                            }
                        }

                        next.push(inner);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            current = next;
        }
    }

    all_contours
}

/// Convert 2D contour rings into a 3D toolpath at the given parameters.
fn contours_to_toolpath(contours: &[Vec<P2>], params: &PocketParams) -> Toolpath {
    let mut tp = Toolpath::new();

    for contour in contours {
        if contour.is_empty() {
            continue;
        }

        // Optionally reverse for climb milling (CW direction)
        let pts: Vec<&P2> = if params.climb {
            contour.iter().rev().collect()
        } else {
            contour.iter().collect()
        };

        let start = pts[0];

        // Rapid to start point at safe Z
        tp.rapid_to(P3::new(start.x, start.y, params.safe_z));
        // Plunge to cutting depth
        tp.feed_to(
            P3::new(start.x, start.y, params.cut_depth),
            params.plunge_rate,
        );
        // Feed around the contour
        for pt in &pts[1..] {
            tp.feed_to(P3::new(pt.x, pt.y, params.cut_depth), params.feed_rate);
        }
        // Close the loop (back to start)
        tp.feed_to(
            P3::new(start.x, start.y, params.cut_depth),
            params.feed_rate,
        );
        // Retract to safe Z
        tp.rapid_to(P3::new(start.x, start.y, params.safe_z));
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn default_params() -> PocketParams {
        PocketParams {
            tool_radius: 3.175, // 1/4" endmill
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        }
    }

    #[test]
    fn test_pocket_contours_square() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let contours = pocket_contours(&sq, 3.175, 2.0);

        // 30mm square, tool radius 3.175mm → compensated is ~23.65mm
        // Then stepover 2.0mm → about 5-6 inner contours
        assert!(
            contours.len() >= 3,
            "Expected at least 3 contours, got {}",
            contours.len()
        );

        // Each contour should be a closed ring (at least 3 points)
        for (i, contour) in contours.iter().enumerate() {
            assert!(
                contour.len() >= 3,
                "Contour {} has only {} points",
                i,
                contour.len()
            );
        }
    }

    #[test]
    fn test_pocket_toolpath_basic() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        assert!(
            !tp.moves.is_empty(),
            "Pocket toolpath should have moves"
        );

        // All cutting moves should be at cut_depth
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && feed_rate == params.feed_rate
            {
                assert!(
                    (m.target.z - params.cut_depth).abs() < 1e-10,
                    "Cutting move at z={} should be at cut_depth={}",
                    m.target.z,
                    params.cut_depth
                );
            }
        }

        // All rapid moves should be at safe_z
        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - params.safe_z).abs() < 1e-10,
                    "Rapid at z={} should be at safe_z={}",
                    m.target.z,
                    params.safe_z
                );
            }
        }
    }

    #[test]
    fn test_pocket_toolpath_structure() {
        // Verify the rapid-plunge-cut-retract pattern
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        let contours = pocket_contours(&sq, params.tool_radius, params.stepover);
        let n_contours = contours.len();

        // Count rapid moves: 2 per contour (approach + retract)
        let n_rapids = tp.moves.iter().filter(|m| m.move_type == MoveType::Rapid).count();
        assert_eq!(
            n_rapids,
            n_contours * 2,
            "Expected 2 rapids per contour ({} contours), got {}",
            n_contours,
            n_rapids
        );

        // Count plunge moves (feed at plunge_rate)
        let n_plunges = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.plunge_rate).abs() < 1e-10))
            .count();
        assert_eq!(
            n_plunges, n_contours,
            "Expected 1 plunge per contour ({} contours), got {}",
            n_contours, n_plunges
        );
    }

    #[test]
    fn test_pocket_too_small_for_tool() {
        // 5mm square with 3.175mm radius tool → pocket collapses
        let tiny = Polygon2::rectangle(0.0, 0.0, 5.0, 5.0);
        let params = default_params();
        let tp = pocket_toolpath(&tiny, &params);

        assert!(
            tp.moves.is_empty(),
            "Pocket too small for tool should produce empty toolpath, got {} moves",
            tp.moves.len()
        );
    }

    #[test]
    fn test_pocket_climb_vs_conventional() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);

        let mut conv_params = default_params();
        conv_params.climb = false;
        let conv_tp = pocket_toolpath(&sq, &conv_params);

        let mut climb_params = default_params();
        climb_params.climb = true;
        let climb_tp = pocket_toolpath(&sq, &climb_params);

        // Both should have the same number of moves
        assert_eq!(conv_tp.moves.len(), climb_tp.moves.len());

        // Both should have the same total cutting distance (same contours, different direction)
        let conv_dist = conv_tp.total_cutting_distance();
        let climb_dist = climb_tp.total_cutting_distance();
        assert!(
            (conv_dist - climb_dist).abs() < 1.0,
            "Cutting distance should be similar: conv={}, climb={}",
            conv_dist,
            climb_dist
        );

        // But the actual XY coordinates of cutting moves should differ (reversed direction)
        // Find first cutting move after first plunge in each
        let conv_cuts: Vec<_> = conv_tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .collect();
        let climb_cuts: Vec<_> = climb_tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .collect();

        if conv_cuts.len() > 1 && climb_cuts.len() > 1 {
            // First cutting move after plunge should go in opposite directions
            let conv_dir_x = conv_cuts[0].target.x;
            let climb_dir_x = climb_cuts[0].target.x;
            // They won't be identical since one is reversed
            assert!(
                (conv_dir_x - climb_dir_x).abs() > 0.01
                    || (conv_cuts[0].target.y - climb_cuts[0].target.y).abs() > 0.01,
                "Climb and conventional should traverse contour in different directions"
            );
        }
    }

    #[test]
    fn test_pocket_no_cutting_above_surface() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = pocket_toolpath(&sq, &params);

        // No feed moves should be above cut_depth (except plunge which goes from safe_z to cut_depth)
        for i in 1..tp.moves.len() {
            if let MoveType::Linear { feed_rate } = tp.moves[i].move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                // This is a cutting move (not plunge) - must be at cut_depth
                assert!(
                    (tp.moves[i].target.z - params.cut_depth).abs() < 1e-10,
                    "Cutting move {} at z={} is not at cut_depth={}",
                    i,
                    tp.moves[i].target.z,
                    params.cut_depth
                );
            }
        }
    }

    #[test]
    fn test_pocket_contours_with_hole() {
        // 40×40 rect with 10×10 center hole — pocket should have more contours
        // than the same shape without a hole.
        let hole = vec![
            P2::new(15.0, 15.0),
            P2::new(15.0, 25.0),
            P2::new(25.0, 25.0),
            P2::new(25.0, 15.0),
        ]; // CW
        let poly_with_hole = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0).exterior,
            vec![hole],
        );
        let poly_no_hole = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);

        let contours_with = pocket_contours(&poly_with_hole, 2.0, 2.0);
        let contours_without = pocket_contours(&poly_no_hole, 2.0, 2.0);

        assert!(
            !contours_with.is_empty(),
            "Pocket with hole should produce contours"
        );

        // The polygon with a hole should produce more contours (the extra hole rings)
        assert!(
            contours_with.len() > contours_without.len(),
            "Pocket with hole should have more contours ({}) than without ({})",
            contours_with.len(),
            contours_without.len()
        );
    }

    #[test]
    fn test_pocket_toolpath_no_cuts_inside_island() {
        // 40×40 rect with 10×10 center hole
        let hole = vec![
            P2::new(15.0, 15.0),
            P2::new(15.0, 25.0),
            P2::new(25.0, 25.0),
            P2::new(25.0, 15.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 40.0, 40.0).exterior,
            vec![hole],
        );

        let params = PocketParams {
            tool_radius: 2.0,
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        };
        let tp = pocket_toolpath(&poly, &params);

        // Verify no feed moves have XY inside the hole (with tool radius margin)
        let hole_margin = 1.0; // allow some tolerance for tool radius
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-6
            {
                let x = m.target.x;
                let y = m.target.y;
                // Inside the hole = x in (15+margin, 25-margin) and y in (15+margin, 25-margin)
                let inside_hole = x > 15.0 + hole_margin
                    && x < 25.0 - hole_margin
                    && y > 15.0 + hole_margin
                    && y < 25.0 - hole_margin;
                assert!(
                    !inside_hole,
                    "Feed move at ({:.1}, {:.1}) is inside the island",
                    x, y
                );
            }
        }
    }

    #[test]
    fn test_pocket_contours_l_shape() {
        // Non-convex L-shape
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(30.0, 0.0),
            P2::new(30.0, 15.0),
            P2::new(15.0, 15.0),
            P2::new(15.0, 30.0),
            P2::new(0.0, 30.0),
        ]);
        let contours = pocket_contours(&l_shape, 2.0, 2.0);
        assert!(
            !contours.is_empty(),
            "L-shape pocket should produce contours"
        );

        let tp = pocket_toolpath(
            &l_shape,
            &PocketParams {
                tool_radius: 2.0,
                stepover: 2.0,
                cut_depth: -2.0,
                feed_rate: 800.0,
                plunge_rate: 400.0,
                safe_z: 5.0,
                climb: false,
            },
        );
        assert!(!tp.moves.is_empty());
    }
}
