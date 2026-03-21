//! Profile cutting operation with tool radius compensation.
//!
//! Cuts along a polygon boundary (inside or outside) with the tool edge
//! following the design contour. A single pass at each depth level.

use crate::geo::{P2, P3};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

/// Which side of the boundary the tool cuts on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileSide {
    /// Tool outside the boundary (cutting out a part).
    Outside,
    /// Tool inside the boundary (cutting an internal contour/hole).
    Inside,
}

/// Parameters for profile cutting.
pub struct ProfileParams {
    /// Tool radius in mm.
    pub tool_radius: f64,
    /// Which side of the boundary to cut.
    pub side: ProfileSide,
    /// Z height of the cut in mm.
    pub cut_depth: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Climb milling: true = CW (climb), false = CCW (conventional).
    pub climb: bool,
}

/// Generate a profile cutting toolpath along the polygon boundary.
///
/// The tool is offset by `tool_radius` to the specified side so the tool
/// edge follows the design boundary exactly.
///
/// Returns an empty toolpath if the offset collapses (e.g., inside profile
/// on a polygon smaller than the tool diameter).
pub fn profile_toolpath(polygon: &Polygon2, params: &ProfileParams) -> Toolpath {
    let contour = profile_contour(polygon, params.tool_radius, params.side);
    match contour {
        Some(pts) => contour_to_toolpath(&pts, params),
        None => Toolpath::new(),
    }
}

/// Generate the 2D profile contour (tool center path).
///
/// Returns None if the offset collapses.
pub fn profile_contour(polygon: &Polygon2, tool_radius: f64, side: ProfileSide) -> Option<Vec<P2>> {
    let distance = match side {
        ProfileSide::Inside => tool_radius,   // inward (positive)
        ProfileSide::Outside => -tool_radius, // outward (negative)
    };

    let results = offset_polygon(polygon, distance);

    // Take the first (largest) result contour
    results
        .into_iter()
        .next()
        .filter(|p| p.exterior.len() >= 3)
        .map(|p| p.exterior)
}

fn contour_to_toolpath(contour: &[P2], params: &ProfileParams) -> Toolpath {
    let mut tp = Toolpath::new();

    if contour.is_empty() {
        return tp;
    }

    let pts: Vec<&P2> = if params.climb {
        contour.iter().rev().collect()
    } else {
        contour.iter().collect()
    };

    let start = pts[0];

    // Rapid to start at safe Z
    tp.rapid_to(P3::new(start.x, start.y, params.safe_z));
    // Plunge to cut depth
    tp.feed_to(
        P3::new(start.x, start.y, params.cut_depth),
        params.plunge_rate,
    );
    // Feed around contour
    for pt in &pts[1..] {
        tp.feed_to(P3::new(pt.x, pt.y, params.cut_depth), params.feed_rate);
    }
    // Close the loop
    tp.feed_to(
        P3::new(start.x, start.y, params.cut_depth),
        params.feed_rate,
    );
    // Retract
    tp.rapid_to(P3::new(start.x, start.y, params.safe_z));

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn default_params(side: ProfileSide) -> ProfileParams {
        ProfileParams {
            tool_radius: 3.175,
            side,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            climb: false,
        }
    }

    #[test]
    fn test_outside_profile_contour() {
        let sq = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let contour = profile_contour(&sq, 3.175, ProfileSide::Outside);

        assert!(
            contour.is_some(),
            "Outside profile should produce a contour"
        );
        let pts = contour.unwrap();
        assert!(pts.len() >= 4, "Should have at least 4 vertices");

        // Outside offset should extend beyond the original boundary
        let x_min = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let x_max = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        assert!(x_min < 0.0, "Outside profile should extend below x=0");
        assert!(x_max > 20.0, "Outside profile should extend above x=20");
    }

    #[test]
    fn test_inside_profile_contour() {
        let sq = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let contour = profile_contour(&sq, 3.175, ProfileSide::Inside);

        assert!(contour.is_some(), "Inside profile should produce a contour");
        let pts = contour.unwrap();

        // Inside offset should be within the original boundary
        let x_min = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let x_max = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        assert!(x_min > 0.0, "Inside profile x_min={} should be > 0", x_min);
        assert!(
            x_max < 20.0,
            "Inside profile x_max={} should be < 20",
            x_max
        );
    }

    #[test]
    fn test_inside_profile_too_small() {
        // 5mm square with 3.175mm radius tool → collapses
        let tiny = Polygon2::rectangle(0.0, 0.0, 5.0, 5.0);
        let contour = profile_contour(&tiny, 3.175, ProfileSide::Inside);
        assert!(
            contour.is_none(),
            "Inside profile on tiny polygon should collapse"
        );

        let tp = profile_toolpath(&tiny, &default_params(ProfileSide::Inside));
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn test_profile_toolpath_structure() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params(ProfileSide::Outside);
        let tp = profile_toolpath(&sq, &params);

        assert!(!tp.moves.is_empty());

        // Should have exactly: rapid, plunge, N cutting moves, closing move, retract
        // = 2 rapids, 1 plunge, N+1 cutting moves
        let n_rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert_eq!(n_rapids, 2, "Expected 2 rapids (approach + retract)");

        let n_plunges = tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.plunge_rate).abs() < 1e-10)
            })
            .count();
        assert_eq!(n_plunges, 1, "Expected 1 plunge");

        // All cutting moves at cut_depth
        for m in &tp.moves {
            if let MoveType::Linear { feed_rate } = m.move_type
                && (feed_rate - params.feed_rate).abs() < 1e-10
            {
                assert!(
                    (m.target.z - params.cut_depth).abs() < 1e-10,
                    "Cutting at z={}, expected {}",
                    m.target.z,
                    params.cut_depth
                );
            }
        }

        // All rapids at safe_z
        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - params.safe_z).abs() < 1e-10,
                    "Rapid at z={}, expected {}",
                    m.target.z,
                    params.safe_z
                );
            }
        }
    }

    #[test]
    fn test_profile_closes_loop() {
        let sq = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let params = default_params(ProfileSide::Outside);
        let tp = profile_toolpath(&sq, &params);

        // The closing move (last feed at feed_rate) should return to the plunge point XY
        let plunge = tp
            .moves
            .iter()
            .find(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.plunge_rate).abs() < 1e-10)
            })
            .unwrap();

        let cutting_moves: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - params.feed_rate).abs() < 1e-10))
            .collect();

        assert!(cutting_moves.len() >= 2);
        let last_cut = &cutting_moves[cutting_moves.len() - 1].target;
        assert!(
            (plunge.target.x - last_cut.x).abs() < 1e-6
                && (plunge.target.y - last_cut.y).abs() < 1e-6,
            "Profile should close: plunge=({},{}), last_cut=({},{})",
            plunge.target.x,
            plunge.target.y,
            last_cut.x,
            last_cut.y
        );
    }

    #[test]
    fn test_profile_climb_reverses_direction() {
        let sq = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);

        let conv_tp = profile_toolpath(&sq, &default_params(ProfileSide::Outside));
        let mut climb_params = default_params(ProfileSide::Outside);
        climb_params.climb = true;
        let climb_tp = profile_toolpath(&sq, &climb_params);

        // Same number of moves
        assert_eq!(conv_tp.moves.len(), climb_tp.moves.len());

        // First cutting move after plunge should differ (reversed contour)
        let conv_first_cut = conv_tp
            .moves
            .iter()
            .find(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            })
            .unwrap();
        let climb_first_cut = climb_tp
            .moves
            .iter()
            .find(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            })
            .unwrap();

        let different = (conv_first_cut.target.x - climb_first_cut.target.x).abs() > 0.01
            || (conv_first_cut.target.y - climb_first_cut.target.y).abs() > 0.01;
        assert!(
            different,
            "Climb and conventional should have different first cutting move"
        );
    }

    #[test]
    fn test_profile_non_convex() {
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(30.0, 0.0),
            P2::new(30.0, 15.0),
            P2::new(15.0, 15.0),
            P2::new(15.0, 30.0),
            P2::new(0.0, 30.0),
        ]);

        let outside = profile_toolpath(&l_shape, &default_params(ProfileSide::Outside));
        assert!(
            !outside.moves.is_empty(),
            "Outside profile of L-shape should work"
        );

        let inside = profile_toolpath(
            &l_shape,
            &ProfileParams {
                tool_radius: 2.0,
                side: ProfileSide::Inside,
                cut_depth: -2.0,
                feed_rate: 800.0,
                plunge_rate: 400.0,
                safe_z: 5.0,
                climb: false,
            },
        );
        assert!(
            !inside.moves.is_empty(),
            "Inside profile of L-shape should work"
        );
    }
}
