//! Trace/follow-path operation for engraving and decorative routing.
//!
//! Follows polygon paths exactly at a specified depth, optionally offset
//! by the tool radius for left/right cutter compensation.

use crate::compute::toolpath_stats::OpenOffsetSplit;
use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::{ClosedPassMoves, MoveIntent, RingMaterial, SidedToolpath, Toolpath};

/// Cutter compensation direction relative to the travel direction.
///
/// O-1 (operator ruling 2026-10-21): the label is the side of the tool
/// centre, as G41 and G42: `Left` puts the tool on the LEFT of the
/// direction of travel, `Right` on the right. This holds for a closed loop
/// (travel = the cut direction of the ring: an exterior counter-clockwise,
/// a hole clockwise, so `Left` cuts inside an exterior) and for an open
/// path (travel = the drawn direction, the first point to the last).
/// Before O-1 a closed `Left` cut to the right of travel (design D-A).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    rs_cam_derive::ParamEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum TraceCompensation {
    /// No compensation — tool center follows the path exactly.
    None,
    /// Offset tool to the left of the travel direction by tool_radius.
    Left,
    /// Offset tool to the right of the travel direction by tool_radius.
    Right,
}

impl TraceCompensation {
    /// The signed offset (mm) for a tool of `tool_radius`: positive is to
    /// the LEFT of travel, the sign of `polygon::offset_polygon` on a CCW
    /// ring (inward) and of `polygon::offset_open_path_reported`. `None`
    /// for no compensation. THE sign of each arm (O-1): the cut and the
    /// material side ([`Self::ring_material`]) both read it, so a change of
    /// the sign moves both.
    #[must_use]
    pub fn region_offset(self, tool_radius: f64) -> Option<f64> {
        match self {
            Self::None => None,
            Self::Left => Some(tool_radius),
            Self::Right => Some(-tool_radius),
        }
    }

    /// Where the material is beside a compensated ring (P6.0c). A grown
    /// region puts the tool outside the drawn loop, so the loop is the
    /// material: inside an exterior ring, outside a hole ring. A shrunk
    /// region puts the tool inside the loop, so the material is on the
    /// other side. On the line (`None`), both sides are material.
    #[must_use]
    pub fn ring_material(self, hole: bool) -> RingMaterial {
        match self.region_offset(1.0) {
            None => RingMaterial::BothSides,
            Some(d) if (d < 0.0) != hole => RingMaterial::Inside,
            Some(_) => RingMaterial::Outside,
        }
    }
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
    /// True when the setup frame mirrors the drawing (a `Bottom` setup:
    /// `(x, y)` to `(x, D - y)`, determinant -1). P6.2: the side of an open
    /// path is the side of its drawn direction IN THE DRAWING (P2.2 rule),
    /// and the mirror keeps the point order, so the open offset takes the
    /// other sign in the setup frame. A closed ring needs no change: the
    /// mirror re-winds it, and inside stays inside.
    pub drawing_mirrored: bool,
}

/// The compensated paths of one input polygon (P6.2), and what the offset
/// reported.
#[derive(Debug, Clone, Default)]
pub struct CompensatedPaths {
    /// The tool-centre paths: closed rings, or open pieces.
    pub paths: Vec<Polygon2>,
    /// The offsets that came back with an `OffsetFailure` (0 or 1).
    pub failures: usize,
    /// An open path whose offset is not one piece.
    pub split: Option<OpenOffsetSplit>,
}

/// The **Z-independent half** of a trace pass (the Layered Trace adapter's
/// `prepare`): the cutter-compensated paths this trace will follow, plus
/// Checkpoint C's offset failure count.
///
/// Split out for the G2 hoist (2026-08-20). A depth-stepped trace used to
/// re-run this offset once per Z level to produce the same rings every time;
/// `z` reaches the output only through [`trace_polygons_at_z`]'s stamp.
///
/// An empty result means the compensation offset collapsed (or failed and was
/// contained), which is why the failure count is returned alongside rather
/// than inferred from the emptiness — the two are not the same event.
///
/// P6.2: a closed loop offsets through `polygon::offset_polygon_reported`;
/// an open path through `polygon::offset_open_path_reported`, to the side
/// of its drawn direction. The open pieces start and end on the end
/// normals of the path, and no piece comes nearer than the tool radius to
/// it. A bend tighter than the radius splits the offset ([`OpenOffsetSplit`]).
#[must_use]
pub fn trace_compensated_polygons_reported(
    polygon: &Polygon2,
    params: &TraceParams,
) -> CompensatedPaths {
    let Some(distance) = params.compensation.region_offset(params.tool_radius) else {
        return CompensatedPaths {
            paths: vec![polygon.clone()],
            ..CompensatedPaths::default()
        };
    };
    if polygon.closed {
        let (paths, failure) = crate::polygon::offset_polygon_reported(polygon, distance);
        return CompensatedPaths {
            paths,
            failures: usize::from(failure.is_some()),
            split: None,
        };
    }
    let distance = if params.drawing_mirrored {
        -distance
    } else {
        distance
    };
    let (paths, failure) = crate::polygon::offset_open_path_reported(polygon, distance);
    // A failure is reported through the count; a split is a different
    // event (the offset worked, and the bend left no room).
    let split = (failure.is_none() && paths.len() != 1).then(|| OpenOffsetSplit {
        start: polygon.exterior.first().map_or([0.0, 0.0], |p| [p.x, p.y]),
        length_mm: polygon
            .exterior
            .windows(2)
            .map(|w| match w {
                [a, b] => (b - a).norm(),
                _ => 0.0,
            })
            .sum(),
        pieces: paths.len(),
    });
    CompensatedPaths {
        paths,
        failures: usize::from(failure.is_some()),
        split,
    }
}

/// The **Z-dependent half**: stamp pre-compensated rings at `z`.
///
/// `z` is the only thing a depth-stepped caller varies, which is the whole of
/// G2 for this family.
///
/// P1: an open path (`closed == false`) is cut from its first point to its
/// last, with no chord back to the start. P6.2: with compensation `Left` or
/// `Right` the open paths here are the offset pieces of
/// [`trace_compensated_polygons_reported`], cut the same way.
///
/// P6.0c: each closed ring states its material side
/// ([`TraceCompensation::ring_material`]); an open path states none, and the
/// lead dressup gives it no lead. P6.2 keeps that for an open offset piece:
/// a lead arc starts one lead radius back along the travel, past the end
/// normal of the drawn path.
#[must_use]
pub fn trace_polygons_at_z(polygons: &[Polygon2], z: f64, params: &TraceParams) -> SidedToolpath {
    let mut tp = SidedToolpath::new();
    for poly in polygons {
        if poly.closed {
            let material = params.compensation.ring_material(false);
            trace_ring(&mut tp, &poly.exterior, z, params, material);
            let material = params.compensation.ring_material(true);
            for hole in &poly.holes {
                trace_ring(&mut tp, hole, z, params, material);
            }
        } else {
            trace_open_path(&mut tp.toolpath, &poly.exterior, z, params);
        }
    }
    tp
}

/// Trace a single closed ring at the given Z depth.
///
/// S.7 (planning/finishing_stack_review_2026-07.md): emits via the shared
/// `emit_closed_contour_with_intent` rapid→plunge→feed→close→retract
/// envelope. A ring below 3 points is no closed shape, and the emitter
/// skips it. An open path never comes here ([`trace_open_path`]).
fn trace_ring(
    tp: &mut SidedToolpath,
    ring: &[P2],
    cut_z: f64,
    params: &TraceParams,
    material: RingMaterial,
) {
    if ring.is_empty() {
        return;
    }

    let points: Vec<P3> = ring.iter().map(|p| P3::new(p.x, p.y, cut_z)).collect();

    tp.emit_closed_pass(
        &points,
        ClosedPassMoves {
            safe_z: params.safe_z,
            feed: params.feed_rate,
            plunge: params.plunge_rate,
            body_intent: MoveIntent::FinishingCut,
        },
        material,
    );
}

/// Trace an open path at the given Z depth (P1): rapid over the first
/// point, plunge, feed through the points to the last one, retract. No
/// move goes back to the first point. A 2-point line is cut; a single
/// point is no line, so it emits nothing.
fn trace_open_path(tp: &mut Toolpath, path: &[P2], cut_z: f64, params: &TraceParams) {
    if path.len() < 2 {
        return;
    }
    let points: Vec<P3> = path.iter().map(|p| P3::new(p.x, p.y, cut_z)).collect();
    tp.emit_path_segment_with_intent(
        &points,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
        MoveIntent::FinishingCut,
    );
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::ops::depth::DepthStepping;
    use crate::toolpath::MoveType;

    /// The trace through the kernels the Layered Trace adapter calls: the
    /// compensation once (`prepare`), then `trace_polygons_at_z` at each Z
    /// of the 2.5D ladder (`emit_level`). The levels are concatenated; the
    /// link between levels belongs to `run_layered`, and no test here reads
    /// it. D3 deleted the public wrapper.
    fn trace_levels(polygon: &Polygon2, params: &TraceParams) -> Toolpath {
        let rings = trace_compensated_polygons_reported(polygon, params).paths;
        let mut tp = Toolpath::new();
        for z in DepthStepping::new(
            params.top_z,
            params.top_z - params.depth,
            params.depth_per_pass,
        )
        .all_levels()
        {
            tp.moves
                .extend(trace_polygons_at_z(&rings, z, params).toolpath.moves);
        }
        tp
    }

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
            drawing_mirrored: false,
        }
    }

    #[test]
    fn trace_square_produces_vertex_moves() {
        let poly = square_polygon();
        let params = default_params();
        let tp = trace_levels(&poly, &params);

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
    fn trace_left_on_a_loop_offsets_inward_o1() {
        let poly = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mut params = default_params();
        params.tool_radius = 2.0;

        // No compensation: tool follows the exact polygon
        params.compensation = TraceCompensation::None;
        let tp_none = trace_levels(&poly, &params);

        // O-1: Left compensation is left of the CCW travel, so inward.
        params.compensation = TraceCompensation::Left;
        let tp_left = trace_levels(&poly, &params);

        assert!(!tp_none.moves.is_empty());
        assert!(!tp_left.moves.is_empty());

        // With left compensation, the cutting area should be smaller
        // (offset inward). Check that X/Y positions differ from uncompensated.
        let none_feed_xs: Vec<f64> = tp_none
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .collect();

        let left_feed_xs: Vec<f64> = tp_left
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .collect();

        // The uncompensated path touches x=0 and x=20.
        // The left-compensated path should be inward of those bounds.
        let none_x_min = none_feed_xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let none_x_max = none_feed_xs
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let left_x_min = left_feed_xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let left_x_max = left_feed_xs
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            left_x_min > none_x_min + 0.5,
            "Left-compensated x_min ({}) should be inward of uncompensated ({})",
            left_x_min,
            none_x_min
        );
        assert!(
            left_x_max < none_x_max - 0.5,
            "Left-compensated x_max ({}) should be inward of uncompensated ({})",
            left_x_max,
            none_x_max
        );
    }

    #[test]
    fn trace_right_on_a_loop_offsets_outward_o1() {
        let poly = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mut params = default_params();
        params.tool_radius = 2.0;

        // No compensation baseline
        params.compensation = TraceCompensation::None;
        let tp_none = trace_levels(&poly, &params);

        // O-1: Right compensation is right of the CCW travel, so outward.
        params.compensation = TraceCompensation::Right;
        let tp_right = trace_levels(&poly, &params);

        assert!(!tp_none.moves.is_empty());
        assert!(!tp_right.moves.is_empty());

        let none_x_max = tp_none
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        let right_x_max = tp_right
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10))
            .map(|m| m.target.x)
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            right_x_max > none_x_max + 0.5,
            "Right-compensated x_max ({}) should be outward of uncompensated ({})",
            right_x_max,
            none_x_max
        );
    }

    #[test]
    fn trace_multipass_depth_stepping() {
        let poly = square_polygon();
        let mut params = default_params();
        params.depth = 6.0;
        params.depth_per_pass = 2.0;

        let tp = trace_levels(&poly, &params);
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

        let tp = trace_levels(&poly, &params);
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

    /// P6.2 and O-1: an open line east, offset Left, lies at +Y; Right at
    /// -Y; on a mirrored setup frame each lies on the other side (the side
    /// holds in the drawing). The open path stays open.
    #[test]
    fn an_open_path_takes_the_side_of_its_drawn_direction_p6_2() {
        let line = Polygon2::open_path(vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)]);
        let mut params = default_params();
        params.tool_radius = 2.0;
        for (compensation, mirrored, y) in [
            (TraceCompensation::Left, false, 2.0),
            (TraceCompensation::Right, false, -2.0),
            (TraceCompensation::Left, true, -2.0),
            (TraceCompensation::Right, true, 2.0),
        ] {
            params.compensation = compensation;
            params.drawing_mirrored = mirrored;
            let out = trace_compensated_polygons_reported(&line, &params);
            assert_eq!((out.failures, out.split), (0, None));
            assert_eq!(out.paths.len(), 1, "{compensation:?} {mirrored}");
            let piece = &out.paths[0];
            assert!(!piece.closed);
            assert!(
                piece.exterior.iter().all(|p| (p.y - y).abs() < 1e-9),
                "{compensation:?} mirrored {mirrored}: {:?} at Y {y}",
                piece.exterior
            );
        }
    }

    #[test]
    fn trace_empty_polygon_produces_empty_toolpath() {
        let poly = Polygon2::new(vec![]);
        let params = default_params();
        let tp = trace_levels(&poly, &params);
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn trace_compensation_collapse_returns_empty() {
        // A very small polygon with large tool radius should collapse on compensation
        let tiny = Polygon2::rectangle(0.0, 0.0, 1.0, 1.0);
        let mut params = default_params();
        params.tool_radius = 5.0;
        // O-1: Left is inward on a CCW loop.
        params.compensation = TraceCompensation::Left;

        let tp = trace_levels(&tiny, &params);
        assert!(
            tp.moves.is_empty(),
            "Collapsed compensation should produce empty toolpath"
        );
    }

    #[test]
    fn trace_all_rapids_at_safe_z() {
        let poly = square_polygon();
        let params = default_params();
        let tp = trace_levels(&poly, &params);

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
