//! Trace/follow-path operation for engraving and decorative routing.
//!
//! Follows polygon paths exactly at a specified depth, optionally offset
//! by the tool radius for left/right cutter compensation.

use crate::depth::{DepthStepping, depth_stepped_toolpath};
use crate::geo::{P2, P3};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

/// Cutter compensation direction relative to the travel direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceCompensation {
    /// No compensation — tool center follows the path exactly.
    None,
    /// Offset tool to the left of the travel direction by tool_radius.
    Left,
    /// Offset tool to the right of the travel direction by tool_radius.
    Right,
}

/// Parameters for a trace/follow-path operation.
#[derive(Debug, Clone)]
pub struct TraceParams {
    /// Tool radius in mm. Used for cutter compensation offset.
    pub tool_radius: f64,
    /// Target cutting depth (positive value, e.g. 2.0 for 2mm deep).
    pub depth: f64,
    /// Maximum depth per pass in mm.
    pub depth_per_pass: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for downward moves (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves above the workpiece.
    pub safe_z: f64,
    /// Cutter compensation mode.
    pub compensation: TraceCompensation,
    /// Starting Z (top of material). Depth stepping goes from top_z down.
    pub top_z: f64,
}

/// Generate a toolpath that traces polygon contours at the specified depth.
///
/// For each contour (exterior + holes), the tool follows the exact path
/// with optional cutter compensation offset. Multi-pass depth stepping
/// is handled automatically when `depth > depth_per_pass`.
///
/// Processing order: exterior ring first, then each hole.
pub fn trace_toolpath(polygon: &Polygon2, params: &TraceParams) -> Toolpath {
    // Apply cutter compensation by offsetting the polygon.
    // cavalier_contours convention for CCW exterior:
    //   positive distance = inward, negative distance = outward
    // Left compensation (tool left of travel on CCW path) = outward = negative offset
    // Right compensation (tool right of travel on CCW path) = inward = positive offset
    let working_polygons: Vec<Polygon2> = match params.compensation {
        TraceCompensation::None => vec![polygon.clone()],
        TraceCompensation::Left => {
            let result = offset_polygon(polygon, -params.tool_radius);
            if result.is_empty() {
                return Toolpath::new();
            }
            result
        }
        TraceCompensation::Right => {
            let result = offset_polygon(polygon, params.tool_radius);
            if result.is_empty() {
                return Toolpath::new();
            }
            result
        }
    };

    let depth = DepthStepping::new(params.top_z, params.top_z - params.depth, params.depth_per_pass);

    depth_stepped_toolpath(&depth, params.safe_z, |z| {
        let mut tp = Toolpath::new();
        for poly in &working_polygons {
            trace_ring(&mut tp, &poly.exterior, z, params);
            for hole in &poly.holes {
                trace_ring(&mut tp, hole, z, params);
            }
        }
        tp
    })
}

/// Trace a single closed ring at the given Z depth.
///
/// Emits: rapid to safe_z -> rapid to XY of first point -> plunge ->
/// feed along all points -> close loop -> retract.
fn trace_ring(tp: &mut Toolpath, ring: &[P2], cut_z: f64, params: &TraceParams) {
    if ring.is_empty() {
        return;
    }

    // SAFETY: ring is non-empty (checked above)
    #[allow(clippy::indexing_slicing)]
    let first = ring[0];

    // Rapid to safe_z above the first point
    tp.rapid_to(P3::new(first.x, first.y, params.safe_z));

    // Plunge to cutting depth
    tp.feed_to(P3::new(first.x, first.y, cut_z), params.plunge_rate);

    // Feed along all subsequent points
    for pt in ring.iter().skip(1) {
        tp.feed_to(P3::new(pt.x, pt.y, cut_z), params.feed_rate);
    }

    // Close the loop by feeding back to the first point
    tp.feed_to(P3::new(first.x, first.y, cut_z), params.feed_rate);

    // Retract to safe_z
    tp.rapid_to(P3::new(first.x, first.y, params.safe_z));
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn square_polygon() -> Polygon2 {
        Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)
    }

    fn default_params() -> TraceParams {
        TraceParams {
            tool_radius: 3.175,
            depth: 2.0,
            depth_per_pass: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            compensation: TraceCompensation::None,
            top_z: 0.0,
        }
    }

    #[test]
    fn trace_square_produces_vertex_moves() {
        let poly = square_polygon();
        let params = default_params();
        let tp = trace_toolpath(&poly, &params);

        assert!(!tp.moves.is_empty(), "Toolpath should not be empty");

        // Expected sequence for a 4-vertex square at single depth:
        //   rapid to safe_z, plunge, 3 feeds (pts 1..3), close loop, retract
        //   = 1 rapid + 1 plunge + 4 feeds + 1 retract = 7 moves
        assert_eq!(
            tp.moves.len(),
            7,
            "Expected 7 moves for a 4-vertex square trace, got {}",
            tp.moves.len()
        );

        // First move: rapid to safe_z above first vertex
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!((tp.moves[0].target.z - 10.0).abs() < 1e-10);
        assert!((tp.moves[0].target.x - 0.0).abs() < 1e-10);
        assert!((tp.moves[0].target.y - 0.0).abs() < 1e-10);

        // Second move: plunge at plunge_rate
        let MoveType::Linear { feed_rate } = tp.moves[1].move_type else {
            unreachable!("Second move should be linear plunge")
        };
        assert!(
            (feed_rate - 500.0).abs() < 1e-10,
            "Plunge should use plunge_rate"
        );
        assert!((tp.moves[1].target.z - -2.0).abs() < 1e-10);

        // Feed moves should visit the square vertices at cut depth
        let feed_moves: Vec<&crate::toolpath::Move> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .collect();

        // 3 intermediate vertices + 1 closing = 4 feed moves at feed_rate
        assert_eq!(
            feed_moves.len(),
            4,
            "Expected 4 feed moves at feed_rate, got {}",
            feed_moves.len()
        );

        // All feed moves should be at cut depth
        for m in &feed_moves {
            assert!(
                (m.target.z - -2.0).abs() < 1e-10,
                "Feed move at z={}, expected -2.0",
                m.target.z
            );
        }

        // The square vertices (CCW): (0,0), (10,0), (10,10), (0,10)
        // Feed moves should visit (10,0), (10,10), (0,10), then close to (0,0)
        assert!((feed_moves[0].target.x - 10.0).abs() < 1e-10);
        assert!((feed_moves[0].target.y - 0.0).abs() < 1e-10);
        assert!((feed_moves[1].target.x - 10.0).abs() < 1e-10);
        assert!((feed_moves[1].target.y - 10.0).abs() < 1e-10);
        assert!((feed_moves[2].target.x - 0.0).abs() < 1e-10);
        assert!((feed_moves[2].target.y - 10.0).abs() < 1e-10);
        assert!((feed_moves[3].target.x - 0.0).abs() < 1e-10);
        assert!((feed_moves[3].target.y - 0.0).abs() < 1e-10);

        // Last move: retract
        let last = &tp.moves[tp.moves.len() - 1];
        assert_eq!(last.move_type, MoveType::Rapid);
        assert!((last.target.z - 10.0).abs() < 1e-10);
    }

    #[test]
    fn trace_with_compensation_offsets_path() {
        let poly = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mut params = default_params();
        params.tool_radius = 2.0;

        // No compensation: tool follows the exact polygon
        params.compensation = TraceCompensation::None;
        let tp_none = trace_toolpath(&poly, &params);

        // Right compensation: tool offset inward (for CCW exterior)
        params.compensation = TraceCompensation::Right;
        let tp_right = trace_toolpath(&poly, &params);

        assert!(!tp_none.moves.is_empty());
        assert!(!tp_right.moves.is_empty());

        // With right compensation, the cutting area should be smaller
        // (offset inward). Check that X/Y positions differ from uncompensated.
        let none_feed_xs: Vec<f64> = tp_none
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .collect();

        let right_feed_xs: Vec<f64> = tp_right
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .collect();

        // The uncompensated path touches x=0 and x=20.
        // The right-compensated path should be inward of those bounds.
        let none_x_min = none_feed_xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let none_x_max = none_feed_xs
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let right_x_min = right_feed_xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let right_x_max = right_feed_xs
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            right_x_min > none_x_min + 0.5,
            "Right-compensated x_min ({}) should be inward of uncompensated ({})",
            right_x_min,
            none_x_min
        );
        assert!(
            right_x_max < none_x_max - 0.5,
            "Right-compensated x_max ({}) should be inward of uncompensated ({})",
            right_x_max,
            none_x_max
        );
    }

    #[test]
    fn trace_with_left_compensation_offsets_outward() {
        let poly = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mut params = default_params();
        params.tool_radius = 2.0;

        // No compensation baseline
        params.compensation = TraceCompensation::None;
        let tp_none = trace_toolpath(&poly, &params);

        // Left compensation: tool offset outward (for CCW exterior)
        params.compensation = TraceCompensation::Left;
        let tp_left = trace_toolpath(&poly, &params);

        assert!(!tp_none.moves.is_empty());
        assert!(!tp_left.moves.is_empty());

        let none_x_max = tp_none
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        let left_x_max = tp_left
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            left_x_max > none_x_max + 0.5,
            "Left-compensated x_max ({}) should be outward of uncompensated ({})",
            left_x_max,
            none_x_max
        );
    }

    #[test]
    fn trace_multipass_depth_stepping() {
        let poly = square_polygon();
        let mut params = default_params();
        params.depth = 6.0;
        params.depth_per_pass = 2.0;

        let tp = trace_toolpath(&poly, &params);
        assert!(!tp.moves.is_empty());

        // Should have 3 depth levels: -2, -4, -6
        let mut cut_zs: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| (m.target.z * 100.0).round() / 100.0)
            .collect();
        cut_zs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        cut_zs.dedup();

        assert_eq!(cut_zs.len(), 3, "Expected 3 depth levels, got {:?}", cut_zs);
        assert!((cut_zs[0] - -2.0).abs() < 0.01);
        assert!((cut_zs[1] - -4.0).abs() < 0.01);
        assert!((cut_zs[2] - -6.0).abs() < 0.01);
    }

    #[test]
    fn trace_with_holes_processes_exterior_then_holes() {
        let hole = vec![
            P2::new(3.0, 3.0),
            P2::new(3.0, 7.0),
            P2::new(7.0, 7.0),
            P2::new(7.0, 3.0),
        ];
        let poly = Polygon2::with_holes(square_polygon().exterior, vec![hole]);
        let params = default_params();

        let tp = trace_toolpath(&poly, &params);
        assert!(!tp.moves.is_empty());

        // Should have two retract-rapid sequences (one for exterior, one for hole)
        let rapid_count = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        // Exterior: 1 rapid approach + 1 retract = 2 rapids
        // Hole:     1 rapid approach + 1 retract = 2 rapids
        // Total: 4 rapids
        assert_eq!(
            rapid_count, 4,
            "Expected 4 rapids (2 per ring), got {}",
            rapid_count
        );
    }

    #[test]
    fn trace_empty_polygon_produces_empty_toolpath() {
        let poly = Polygon2::new(vec![]);
        let params = default_params();
        let tp = trace_toolpath(&poly, &params);
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn trace_compensation_collapse_returns_empty() {
        // A very small polygon with large tool radius should collapse on compensation
        let tiny = Polygon2::rectangle(0.0, 0.0, 1.0, 1.0);
        let mut params = default_params();
        params.tool_radius = 5.0;
        params.compensation = TraceCompensation::Right;

        let tp = trace_toolpath(&tiny, &params);
        assert!(
            tp.moves.is_empty(),
            "Collapsed compensation should produce empty toolpath"
        );
    }

    #[test]
    fn trace_all_rapids_at_safe_z() {
        let poly = square_polygon();
        let params = default_params();
        let tp = trace_toolpath(&poly, &params);

        for m in &tp.moves {
            if m.move_type == MoveType::Rapid {
                assert!(
                    (m.target.z - params.safe_z).abs() < 1e-10,
                    "Rapid at z={}, expected safe_z={}",
                    m.target.z,
                    params.safe_z
                );
            }
        }
    }
}
