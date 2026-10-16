//! Zigzag/raster clearing pattern for 2D pockets.
//!
//! Clears material with back-and-forth parallel passes, like mowing a lawn.
//! Useful for large flat pockets and facing operations. Typically combined
//! with a contour finish pass for clean pocket walls.

use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::{MoveIntent, Toolpath};

/// Parameters for zigzag/raster clearing.
///
/// `Copy` since the G2 hoist (2026-08-20) — see [`crate::ops::pocket::PocketParams`]
/// for why a depth-stepping caller wants struct-update rather than a re-listed
/// literal per Z level.
#[derive(Debug, Clone, Copy)]
pub struct ZigzagParams {
    /// Tool radius in mm.
    pub tool_radius: f64,
    /// Distance between passes in mm (stepover).
    pub stepover: f64,
    /// Z height of the cut in mm.
    pub cut_depth: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Raster angle in degrees (0 = along X axis, 90 = along Y axis).
    pub angle: f64,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate the 2D zigzag scan lines (no Z, no toolpath yet).
///
/// Each line is a pair of P2 endpoints. Lines alternate direction for zigzag.
pub fn zigzag_lines(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    angle_deg: f64,
) -> Vec<[P2; 2]> {
    zigzag_lines_reported(polygon, tool_radius, stepover, angle_deg).0
}

/// [`zigzag_lines`] with Checkpoint C's offset failure channel attached.
#[must_use]
pub fn zigzag_lines_reported(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    angle_deg: f64,
) -> (Vec<[P2; 2]>, usize) {
    zigzag_lines_on_lattice_reported(polygon, tool_radius, stepover, angle_deg, None)
}

/// The perpendicular position of the first scan row of `polygon`: the
/// lowest point of its inset by `tool_radius`, read across the scan
/// direction. `None` when the inset is empty. [`zigzag_lines_reported`]
/// puts its rows at this position plus whole stepovers.
#[must_use]
pub(crate) fn scan_lattice_origin(
    polygon: &Polygon2,
    tool_radius: f64,
    angle_deg: f64,
) -> Option<f64> {
    let inset = crate::polygon::offset_polygon(polygon, tool_radius);
    let angle_rad = angle_deg.to_radians();
    let (perp_min, _) = perp_range(&inset, -angle_rad.sin(), angle_rad.cos());
    perp_min.is_finite().then_some(perp_min)
}

/// The range of the exterior vertices of `inset` across the scan direction.
fn perp_range(inset: &[Polygon2], perp_x: f64, perp_y: f64) -> (f64, f64) {
    let mut perp_min = f64::INFINITY;
    let mut perp_max = f64::NEG_INFINITY;
    for poly in inset {
        for p in &poly.exterior {
            let d = p.x * perp_x + p.y * perp_y;
            perp_min = perp_min.min(d);
            perp_max = perp_max.max(d);
        }
    }
    (perp_min, perp_max)
}

/// [`zigzag_lines_reported`] with the rows on a given lattice. `lattice`
/// is the perpendicular position of row 0 (see [`scan_lattice_origin`]);
/// row `k` lies at `lattice + k * stepover` and cuts in the direction of
/// its parity. A region cut from a larger one (a rest clip) then keeps the
/// rows of the larger one: each row it cuts lies on a row of the whole.
/// A row below `lattice` is not cut. `None` puts row 0 on the region's own
/// lowest point.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
#[must_use]
pub(crate) fn zigzag_lines_on_lattice_reported(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    angle_deg: f64,
    lattice: Option<f64>,
) -> (Vec<[P2; 2]>, usize) {
    if polygon.exterior.len() < 3 || stepover <= 0.0 {
        // Refused before any offset was attempted, so nothing was observed.
        return (Vec::new(), 0);
    }

    let angle_rad = angle_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Inset the polygon by tool radius to avoid wall contact
    let (inset, failure) = crate::polygon::offset_polygon_reported(polygon, tool_radius);
    let failures = usize::from(failure.is_some());
    if inset.is_empty() {
        return (Vec::new(), failures);
    }

    // Collect all inset polygon edges (exterior + holes from all result polygons)
    let mut all_edges: Vec<(P2, P2)> = Vec::new();
    for poly in &inset {
        let ext = &poly.exterior;
        for i in 0..ext.len() {
            let j = (i + 1) % ext.len();
            all_edges.push((ext[i], ext[j]));
        }
        for hole in &poly.holes {
            for i in 0..hole.len() {
                let j = (i + 1) % hole.len();
                all_edges.push((hole[i], hole[j]));
            }
        }
    }

    if all_edges.is_empty() {
        return (Vec::new(), failures);
    }

    // Project all vertices onto the scan direction to find range
    // Scan lines are perpendicular to the raster direction
    // For angle=0: scan along X, lines at constant Y
    // Perpendicular direction: (-sin, cos)
    let perp_x = -sin_a;
    let perp_y = cos_a;

    let (perp_min, perp_max) = perp_range(&inset, perp_x, perp_y);

    if perp_max - perp_min < 1e-10 {
        return (Vec::new(), failures);
    }

    // Generate scan lines at stepover intervals, from row 0 of the lattice
    // (the region's own lowest point when no lattice is given).
    let (origin, first) = match lattice {
        None => (perp_min, 0),
        Some(origin) => (
            origin,
            // The first lattice row at or above the region, with a float
            // tolerance so a row on the region's edge is kept.
            ((perp_min - origin) / stepover - LATTICE_EPS)
                .ceil()
                .max(0.0) as usize,
        ),
    };
    let mut lines = Vec::new();
    let n_lines = ((perp_max - origin) / stepover).ceil().max(0.0) as usize + 1;

    for i in first..n_lines {
        let perp_pos = origin + i as f64 * stepover;
        if perp_pos > perp_max {
            break;
        }

        for (t_enter, t_exit) in scan_line_intervals(&all_edges, perp_pos, cos_a, sin_a) {
            // Convert back to XY: point = perp_pos * perp_dir + t * scan_dir
            let x_enter = perp_pos * perp_x + t_enter * cos_a;
            let y_enter = perp_pos * perp_y + t_enter * sin_a;
            let x_exit = perp_pos * perp_x + t_exit * cos_a;
            let y_exit = perp_pos * perp_y + t_exit * sin_a;

            let p1 = P2::new(x_enter, y_enter);
            let p2 = P2::new(x_exit, y_exit);

            // Alternate direction for zigzag
            if i % 2 == 0 {
                lines.push([p1, p2]);
            } else {
                lines.push([p2, p1]);
            }
        }
    }

    (lines, failures)
}

/// The tolerance (in stepovers) of the first lattice row: float noise in
/// the offset must not drop a row that lies on the edge of a region.
const LATTICE_EPS: f64 = 1e-9;

/// The parts of a scan line inside the polygon or on its edge, as sorted,
/// disjoint `(t_enter, t_exit)` intervals along the scan direction.
///
/// A vertex ON the line is the degenerate case. Counted on both of its
/// edges, a vertex where the outline crosses the line gives a duplicate
/// entry; the duplicate pairs with itself and every later enter/exit pair
/// shifts, so the row is dropped or cuts across a gap. A half-open count
/// (the vertex belongs to ONE side) fixes that, but then a row along an
/// edge on the other side is lost (the top row of a rectangle). So each
/// side is counted half-open in turn and the two interval sets are merged:
/// the result is the closed intersection, with no duplicate.
fn scan_line_intervals(
    edges: &[(P2, P2)],
    perp_pos: f64,
    cos_a: f64,
    sin_a: f64,
) -> Vec<(f64, f64)> {
    let mut intervals: Vec<(f64, f64)> = [false, true]
        .into_iter()
        .flat_map(|vertex_above| {
            let mut ts = scan_line_intersections(edges, perp_pos, cos_a, sin_a, vertex_above);
            ts.sort_by(f64::total_cmp);
            ts.as_chunks::<2>()
                .0
                .iter()
                .map(|&[enter, exit]| (enter, exit))
                .collect::<Vec<_>>()
        })
        .collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(intervals.len());
    for (enter, exit) in intervals {
        match merged.last_mut() {
            Some(last) if enter <= last.1 => last.1 = last.1.max(exit),
            _ => merged.push((enter, exit)),
        }
    }
    merged
}

/// Find intersection parameters along the scan direction for a scan line,
/// counting a vertex ON the line as part of one side only: the side above
/// when `vertex_above`, else the side below.
///
/// The scan line is defined by: point = perp_pos * perp_dir + t * scan_dir
/// where scan_dir = (cos_a, sin_a) and perp_dir = (-sin_a, cos_a).
fn scan_line_intersections(
    edges: &[(P2, P2)],
    perp_pos: f64,
    cos_a: f64,
    sin_a: f64,
    vertex_above: bool,
) -> Vec<f64> {
    let perp_x = -sin_a;
    let perp_y = cos_a;

    let mut intersections = Vec::new();

    for (a, b) in edges {
        // Project edge endpoints onto perpendicular axis
        let da = a.x * perp_x + a.y * perp_y - perp_pos;
        let db = b.x * perp_x + b.y * perp_y - perp_pos;

        // Check if edge crosses the scan line (half-open, see above).
        let above = |d: f64| if vertex_above { d >= 0.0 } else { d > 0.0 };
        if above(da) == above(db) {
            continue; // Both on same side, or along the line
        }
        if (da - db).abs() < 1e-15 {
            continue; // Edge parallel to scan line
        }

        // Interpolation parameter along the edge
        let s = da / (da - db);

        // Intersection point
        let ix = a.x + s * (b.x - a.x);
        let iy = a.y + s * (b.y - a.y);

        // Project onto scan direction to get t parameter
        let t = ix * cos_a + iy * sin_a;
        intersections.push(t);
    }

    intersections
}

/// S.7 (planning/finishing_stack_review_2026-07.md): each line is emitted
/// via the shared `emit_path_segment_with_intent` rapid→plunge→feed→retract
/// envelope instead of open-coding it — byte-identical to the previous
/// hand-rolled sequence (a 2-point path yields exactly rapid, plunge, one
/// feed, retract).
pub fn lines_to_toolpath(lines: &[[P2; 2]], params: &ZigzagParams) -> Toolpath {
    let mut tp = Toolpath::new();

    if lines.is_empty() {
        return tp;
    }

    for line in lines {
        let start = &line[0];
        let end = &line[1];
        let path = [
            P3::new(start.x, start.y, params.cut_depth),
            P3::new(end.x, end.y, params.cut_depth),
        ];

        tp.emit_path_segment_with_intent(
            &path,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
            MoveIntent::ClearingCut,
        );
    }

    tp
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    /// One zigzag level through the kernels the Layered Zigzag adapter
    /// calls: the rows on the lattice of the outline (`prepare`, B9), then
    /// `lines_to_toolpath` (`emit_level`). D3 deleted the public wrapper.
    fn zigzag_level(polygon: &Polygon2, params: &ZigzagParams) -> Toolpath {
        let lattice = scan_lattice_origin(polygon, params.tool_radius, params.angle);
        let (lines, _failures) = zigzag_lines_on_lattice_reported(
            polygon,
            params.tool_radius,
            params.stepover,
            params.angle,
            lattice,
        );
        lines_to_toolpath(&lines, params)
    }

    fn default_params() -> ZigzagParams {
        ZigzagParams {
            tool_radius: 3.175,
            stepover: 2.0,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            angle: 0.0,
        }
    }

    fn edges_of(pts: &[P2]) -> Vec<(P2, P2)> {
        (0..pts.len())
            .map(|i| (pts[i], pts[(i + 1) % pts.len()]))
            .collect()
    }

    /// A scan line through a vertex where the outline crosses the line meets
    /// that vertex ONCE. Counted twice, the duplicate pairs with itself and
    /// every later enter/exit pair shifts: the row is dropped or cuts
    /// across a gap.
    #[test]
    fn scan_line_through_a_crossing_vertex_counts_it_once() {
        // Square 0..10 with a point at (-2, 5) on the left side.
        let edges = edges_of(&[
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(0.0, 10.0),
            P2::new(-2.0, 5.0),
        ]);
        let spans = scan_line_intervals(&edges, 5.0, 1.0, 0.0);
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert!(
            (spans[0].0 + 2.0).abs() < 1e-9 && (spans[0].1 - 10.0).abs() < 1e-9,
            "{spans:?}"
        );
    }

    /// A row ON a horizontal edge is kept, at the bottom AND at the top: a
    /// one-sided half-open count drops the top row of a rectangle (the
    /// inlay plug's outer row, G-OPGOLDEN).
    #[test]
    fn scan_line_along_an_edge_is_kept_on_both_sides() {
        let edges = edges_of(&[
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(0.0, 10.0),
        ]);
        for y in [0.0, 10.0] {
            let spans = scan_line_intervals(&edges, y, 1.0, 0.0);
            assert_eq!(spans, vec![(0.0, 10.0)], "row at y = {y}");
        }
    }

    /// Two crossing vertices on one row with a gap between: two spans, and
    /// nothing across the gap.
    #[test]
    fn scan_line_through_two_crossing_vertices_keeps_the_gap() {
        // A "U": two prongs joined at the bottom; the row at y = 5 meets a
        // side vertex on each outer wall.
        let edges = edges_of(&[
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(12.0, 5.0),
            P2::new(10.0, 10.0),
            P2::new(7.0, 10.0),
            P2::new(7.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, 10.0),
            P2::new(0.0, 10.0),
            P2::new(-2.0, 5.0),
        ]);
        let spans = scan_line_intervals(&edges, 5.0, 1.0, 0.0);
        assert_eq!(spans, vec![(-2.0, 3.0), (7.0, 12.0)]);
    }

    #[test]
    fn test_zigzag_lines_square() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let lines = zigzag_lines(&sq, 3.175, 2.0, 0.0);

        assert!(
            !lines.is_empty(),
            "Should produce scan lines for 30mm square"
        );

        // With 30mm width, 3.175mm inset each side → ~23.65mm usable
        // At 2mm stepover → about 11-12 lines
        assert!(
            lines.len() >= 8 && lines.len() <= 15,
            "Expected 8-15 lines, got {}",
            lines.len()
        );

        // All line endpoints should be within the original polygon bounds
        for line in &lines {
            for pt in line {
                assert!(
                    pt.x >= -0.1 && pt.x <= 30.1 && pt.y >= -0.1 && pt.y <= 30.1,
                    "Point ({}, {}) outside bounds",
                    pt.x,
                    pt.y
                );
            }
        }
    }

    #[test]
    fn test_zigzag_alternating_direction() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let lines = zigzag_lines(&sq, 2.0, 3.0, 0.0);

        if lines.len() >= 2 {
            // For angle=0, scan lines are horizontal (along X).
            // Even lines go left-to-right, odd go right-to-left.
            let line0_dx = lines[0][1].x - lines[0][0].x;
            let line1_dx = lines[1][1].x - lines[1][0].x;
            assert!(
                line0_dx * line1_dx < 0.0,
                "Adjacent lines should go in opposite X directions: dx0={}, dx1={}",
                line0_dx,
                line1_dx
            );
        }
    }

    #[test]
    fn test_zigzag_toolpath_basic() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let tp = zigzag_level(&sq, &params);

        assert!(!tp.moves.is_empty());

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
    fn test_zigzag_toolpath_structure() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params();
        let lines = zigzag_lines(&sq, params.tool_radius, params.stepover, params.angle);
        let tp = zigzag_level(&sq, &params);

        // Each line produces: rapid + plunge + cut + retract = 4 moves
        assert_eq!(
            tp.moves.len(),
            lines.len() * 4,
            "Expected 4 moves per line ({} lines), got {}",
            lines.len(),
            tp.moves.len()
        );
    }

    #[test]
    fn test_zigzag_too_small() {
        let tiny = Polygon2::rectangle(0.0, 0.0, 4.0, 4.0);
        let params = default_params(); // 3.175mm radius
        let tp = zigzag_level(&tiny, &params);

        assert!(
            tp.moves.is_empty(),
            "Too-small pocket should produce empty toolpath"
        );
    }

    #[test]
    fn test_zigzag_90_degree_angle() {
        let rect = Polygon2::rectangle(0.0, 0.0, 40.0, 20.0);
        let lines_0 = zigzag_lines(&rect, 2.0, 3.0, 0.0);
        let lines_90 = zigzag_lines(&rect, 2.0, 3.0, 90.0);

        assert!(!lines_0.is_empty());
        assert!(!lines_90.is_empty());

        // At 0 degrees, lines run along X (horizontal) → more lines for a tall rectangle
        // At 90 degrees, lines run along Y (vertical) → more lines for a wide rectangle
        // 40x20 rect: at 0°, usable height ~16mm → ~5 lines
        //              at 90°, usable width ~36mm → ~12 lines
        assert!(
            lines_90.len() > lines_0.len(),
            "90° should produce more lines ({}) than 0° ({}) for wide rectangle",
            lines_90.len(),
            lines_0.len()
        );
    }

    #[test]
    fn test_zigzag_non_convex() {
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(30.0, 0.0),
            P2::new(30.0, 15.0),
            P2::new(15.0, 15.0),
            P2::new(15.0, 30.0),
            P2::new(0.0, 30.0),
        ]);
        let lines = zigzag_lines(&l_shape, 2.0, 2.0, 0.0);
        assert!(!lines.is_empty(), "L-shape should produce zigzag lines");

        let tp = zigzag_level(
            &l_shape,
            &ZigzagParams {
                tool_radius: 2.0,
                stepover: 2.0,
                cut_depth: -2.0,
                feed_rate: 800.0,
                plunge_rate: 400.0,
                safe_z: 5.0,
                angle: 0.0,
            },
        );
        assert!(!tp.moves.is_empty());
    }
}
