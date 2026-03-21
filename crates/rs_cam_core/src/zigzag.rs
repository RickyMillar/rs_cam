//! Zigzag/raster clearing pattern for 2D pockets.
//!
//! Clears material with back-and-forth parallel passes, like mowing a lawn.
//! Useful for large flat pockets and facing operations. Typically combined
//! with a contour finish pass for clean pocket walls.

use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

/// Parameters for zigzag/raster clearing.
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

/// Generate a zigzag raster clearing toolpath inside a polygon boundary.
///
/// The tool is offset inward by `tool_radius` to avoid cutting the walls,
/// then parallel scan lines fill the interior. Lines alternate direction
/// (zigzag) to minimize rapids.
///
/// For clean pocket walls, combine with a profile or contour finish pass.
pub fn zigzag_toolpath(polygon: &Polygon2, params: &ZigzagParams) -> Toolpath {
    let lines = zigzag_lines(polygon, params.tool_radius, params.stepover, params.angle);
    lines_to_toolpath(&lines, params)
}

/// Generate the 2D zigzag scan lines (no Z, no toolpath yet).
///
/// Each line is a pair of P2 endpoints. Lines alternate direction for zigzag.
pub fn zigzag_lines(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    angle_deg: f64,
) -> Vec<[P2; 2]> {
    if polygon.exterior.len() < 3 || stepover <= 0.0 {
        return Vec::new();
    }

    let angle_rad = angle_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Inset the polygon by tool radius to avoid wall contact
    let inset = crate::polygon::offset_polygon(polygon, tool_radius);
    if inset.is_empty() {
        return Vec::new();
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
        return Vec::new();
    }

    // Project all vertices onto the scan direction to find range
    // Scan lines are perpendicular to the raster direction
    // For angle=0: scan along X, lines at constant Y
    // Perpendicular direction: (-sin, cos)
    let perp_x = -sin_a;
    let perp_y = cos_a;

    let mut perp_min = f64::INFINITY;
    let mut perp_max = f64::NEG_INFINITY;
    for poly in &inset {
        for p in &poly.exterior {
            let d = p.x * perp_x + p.y * perp_y;
            perp_min = perp_min.min(d);
            perp_max = perp_max.max(d);
        }
    }

    if perp_max - perp_min < 1e-10 {
        return Vec::new();
    }

    // Generate scan lines at stepover intervals
    let mut lines = Vec::new();
    let n_lines = ((perp_max - perp_min) / stepover).ceil() as usize + 1;

    for i in 0..n_lines {
        let perp_pos = perp_min + i as f64 * stepover;
        if perp_pos > perp_max {
            break;
        }

        // Find intersections of this scan line with all polygon edges
        let mut intersections = scan_line_intersections(&all_edges, perp_pos, cos_a, sin_a);
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Pair up intersections (enter/exit pairs)
        let mut j = 0;
        while j + 1 < intersections.len() {
            let t_enter = intersections[j];
            let t_exit = intersections[j + 1];

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

            j += 2;
        }
    }

    lines
}

/// Find intersection parameters along the scan direction for a scan line.
///
/// The scan line is defined by: point = perp_pos * perp_dir + t * scan_dir
/// where scan_dir = (cos_a, sin_a) and perp_dir = (-sin_a, cos_a).
fn scan_line_intersections(edges: &[(P2, P2)], perp_pos: f64, cos_a: f64, sin_a: f64) -> Vec<f64> {
    let perp_x = -sin_a;
    let perp_y = cos_a;

    let mut intersections = Vec::new();

    for (a, b) in edges {
        // Project edge endpoints onto perpendicular axis
        let da = a.x * perp_x + a.y * perp_y - perp_pos;
        let db = b.x * perp_x + b.y * perp_y - perp_pos;

        // Check if edge crosses the scan line
        if da * db > 0.0 {
            continue; // Both on same side
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

fn lines_to_toolpath(lines: &[[P2; 2]], params: &ZigzagParams) -> Toolpath {
    let mut tp = Toolpath::new();

    if lines.is_empty() {
        return tp;
    }

    for line in lines {
        let start = &line[0];
        let end = &line[1];

        // Rapid to start of line at safe Z
        tp.rapid_to(P3::new(start.x, start.y, params.safe_z));
        // Plunge
        tp.feed_to(
            P3::new(start.x, start.y, params.cut_depth),
            params.plunge_rate,
        );
        // Cut across
        tp.feed_to(P3::new(end.x, end.y, params.cut_depth), params.feed_rate);
        // Retract
        tp.rapid_to(P3::new(end.x, end.y, params.safe_z));
    }

    tp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

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
        let tp = zigzag_toolpath(&sq, &params);

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
        let tp = zigzag_toolpath(&sq, &params);

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
        let tp = zigzag_toolpath(&tiny, &params);

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

        let tp = zigzag_toolpath(
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
