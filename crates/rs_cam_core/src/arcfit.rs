//! Arc fitting dressup — converts sequences of short linear segments into G2/G3 arcs.
//!
//! Uses a biarc-like approach: finds groups of consecutive linear moves that lie
//! approximately on a circular arc, then replaces them with a single arc move.
//! This reduces G-code size and improves surface finish on curved toolpaths.

use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Fit arcs to a toolpath, replacing linear segments with G2/G3 where possible.
///
/// `tolerance` is the maximum allowed deviation (mm) between the original linear
/// path and the fitted arc. Typical values: 0.001 to 0.01 mm.
///
/// Only fits arcs in the XY plane (constant Z within tolerance).
pub fn fit_arcs(toolpath: &Toolpath, tolerance: f64) -> Toolpath {
    let mut result = Toolpath::new();
    let moves = &toolpath.moves;

    if moves.is_empty() {
        return result;
    }

    let mut i = 0;
    while i < moves.len() {
        let m = &moves[i];

        // Only try to fit arcs on linear moves
        let feed_rate = match m.move_type {
            MoveType::Linear { feed_rate } => feed_rate,
            _ => {
                result.moves.push(m.clone());
                i += 1;
                continue;
            }
        };

        // Try to extend an arc starting from the previous point through this point
        if i == 0 {
            result.moves.push(m.clone());
            i += 1;
            continue;
        }

        let start = &moves[i - 1].target;

        // Collect consecutive linear moves at same feed rate and approximately same Z
        let mut end_idx = i;
        while end_idx < moves.len() {
            match moves[end_idx].move_type {
                MoveType::Linear { feed_rate: f } if (f - feed_rate).abs() < 1e-6 => {
                    // Check Z is approximately constant
                    if (moves[end_idx].target.z - start.z).abs() > tolerance {
                        break;
                    }
                    end_idx += 1;
                }
                _ => break,
            }
        }

        let segment_count = end_idx - i;

        // Need at least 3 points (start + 2 segments) to fit an arc
        if segment_count < 2 {
            result.moves.push(m.clone());
            i += 1;
            continue;
        }

        // Try to fit arcs greedily: find the longest run that fits within tolerance
        let mut best_arc_end = i;
        let mut best_arc: Option<ArcParams> = None;

        // Try progressively longer runs
        let mut run_end = i + 2; // minimum 2 segments (3 points)
        while run_end <= end_idx {
            let points: Vec<&P3> = std::iter::once(start)
                .chain((i..run_end).map(|j| &moves[j].target))
                .collect();

            if let Some(arc) = try_fit_arc(&points, tolerance) {
                best_arc_end = run_end;
                best_arc = Some(arc);
                run_end += 1;
            } else {
                break;
            }
        }

        if let Some(arc) = best_arc {
            let end_pt = &moves[best_arc_end - 1].target;
            let z = start.z; // Use start Z (constant within tolerance)

            // I, J = offset from start point to center
            let ij_i = arc.cx - start.x;
            let ij_j = arc.cy - start.y;

            if arc.clockwise {
                result.moves.push(Move {
                    target: P3::new(end_pt.x, end_pt.y, z),
                    move_type: MoveType::ArcCW {
                        i: ij_i,
                        j: ij_j,
                        feed_rate,
                    },
                });
            } else {
                result.moves.push(Move {
                    target: P3::new(end_pt.x, end_pt.y, z),
                    move_type: MoveType::ArcCCW {
                        i: ij_i,
                        j: ij_j,
                        feed_rate,
                    },
                });
            }

            i = best_arc_end;
        } else {
            result.moves.push(m.clone());
            i += 1;
        }
    }

    result
}

struct ArcParams {
    cx: f64,
    cy: f64,
    clockwise: bool,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Try to fit a circular arc through a sequence of XY points.
/// Returns arc parameters if all points are within tolerance of the arc.
fn try_fit_arc(points: &[&P3], tolerance: f64) -> Option<ArcParams> {
    if points.len() < 3 {
        return None;
    }

    // Least-squares circle fit (Kåsa's algebraic method) for better accuracy
    // on noisy or partial-arc points. Falls back to 3-point if too few points.
    let (cx, cy, radius) = if points.len() >= 5 {
        circle_from_least_squares(points)?
    } else {
        let p0 = points[0];
        let pm = points[points.len() / 2];
        let pn = points[points.len() - 1];
        circle_from_3_points(p0.x, p0.y, pm.x, pm.y, pn.x, pn.y)?
    };

    // Reject degenerate arcs (very large radius = nearly straight line)
    if radius > 1e6 {
        return None;
    }

    // Check all intermediate points are within tolerance of the circle
    for &pt in points {
        let ddx = pt.x - cx;
        let ddy = pt.y - cy;
        let dist = (ddx * ddx + ddy * ddy).sqrt();
        if (dist - radius).abs() > tolerance {
            return None;
        }
    }

    // Determine CW vs CCW using the cross product of the first two segments
    let p_first = points[0];
    let p_mid = points[points.len() / 2];
    let p_last = points[points.len() - 1];
    let dx1 = p_mid.x - p_first.x;
    let dy1 = p_mid.y - p_first.y;
    let dx2 = p_last.x - p_mid.x;
    let dy2 = p_last.y - p_mid.y;
    let cross = dx1 * dy2 - dy1 * dx2;

    // Negative cross product = CW (G2), positive = CCW (G3)
    let clockwise = cross < 0.0;

    Some(ArcParams { cx, cy, clockwise })
}

/// Least-squares circle fit using Kåsa's algebraic method.
///
/// Minimizes the algebraic distance sum(xi² + yi² + D*xi + E*yi + F)²
/// by solving a 3×3 linear system. Returns (cx, cy, radius).
fn circle_from_least_squares(points: &[&P3]) -> Option<(f64, f64, f64)> {
    let n = points.len() as f64;
    if n < 3.0 {
        return None;
    }

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sx2 = 0.0;
    let mut sy2 = 0.0;
    let mut sxy = 0.0;
    let mut sx3 = 0.0;
    let mut sy3 = 0.0;
    let mut sx2y = 0.0;
    let mut sxy2 = 0.0;

    for &p in points {
        let x = p.x;
        let y = p.y;
        let x2 = x * x;
        let y2 = y * y;
        sx += x;
        sy += y;
        sx2 += x2;
        sy2 += y2;
        sxy += x * y;
        sx3 += x2 * x;
        sy3 += y2 * y;
        sx2y += x2 * y;
        sxy2 += x * y2;
    }

    // Solve 2×2 system for A, B:
    //   [sx2  sxy] [A]   [-(sx3 + sxy2)]
    //   [sxy  sy2] [B] = [-(sx2y + sy3)]
    // Then cx = A/(-2), cy = B/(-2)
    let a11 = sx2 - sx * sx / n;
    let a12 = sxy - sx * sy / n;
    let a22 = sy2 - sy * sy / n;

    let b1 = 0.5 * (sx3 + sxy2 - sx * (sx2 + sy2) / n);
    let b2 = 0.5 * (sx2y + sy3 - sy * (sx2 + sy2) / n);

    let det = a11 * a22 - a12 * a12;
    if det.abs() < 1e-20 {
        return None; // Degenerate (collinear or single point)
    }

    let cx = (b1 * a22 - b2 * a12) / det;
    let cy = (a11 * b2 - a12 * b1) / det;

    let r_sq = (sx2 + sy2 - 2.0 * cx * sx - 2.0 * cy * sy) / n + cx * cx + cy * cy;
    if r_sq <= 0.0 {
        return None;
    }
    let radius = r_sq.sqrt();

    Some((cx, cy, radius))
}

/// Find the center and radius of a circle through 3 points.
#[allow(dead_code)]
fn circle_from_3_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
) -> Option<(f64, f64, f64)> {
    let ax = x1 - x2;
    let ay = y1 - y2;
    let bx = x1 - x3;
    let by = y1 - y3;

    let det = 2.0 * (ax * by - ay * bx);
    if det.abs() < 1e-12 {
        return None; // collinear
    }

    let a_sq = ax * ax + ay * ay;
    let b_sq = bx * bx + by * by;

    let cx = x1 - (a_sq * by - b_sq * ay) / det;
    let cy = y1 - (b_sq * ax - a_sq * bx) / det;
    let rdx = x1 - cx;
    let rdy = y1 - cy;
    let radius = (rdx * rdx + rdy * rdy).sqrt();

    Some((cx, cy, radius))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn make_circle_points(cx: f64, cy: f64, r: f64, n: usize, z: f64, ccw: bool) -> Vec<P3> {
        (0..=n)
            .map(|i| {
                let angle = if ccw {
                    std::f64::consts::TAU * i as f64 / n as f64
                } else {
                    -std::f64::consts::TAU * i as f64 / n as f64
                };
                P3::new(cx + r * angle.cos(), cy + r * angle.sin(), z)
            })
            .collect()
    }

    #[test]
    fn test_circle_from_3_points() {
        let (cx, cy, r) = circle_from_3_points(
            10.0, 0.0, // right
            0.0, 10.0, // top
            -10.0, 0.0, // left
        )
        .unwrap();
        assert!((cx - 0.0).abs() < 1e-8);
        assert!((cy - 0.0).abs() < 1e-8);
        assert!((r - 10.0).abs() < 1e-8);
    }

    #[test]
    fn test_circle_from_collinear() {
        let result = circle_from_3_points(0.0, 0.0, 1.0, 0.0, 2.0, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_fit_arc_basic() {
        // Generate points on a circle
        let pts = make_circle_points(0.0, 0.0, 10.0, 16, 5.0, true);
        let refs: Vec<&P3> = pts.iter().collect();

        let arc = try_fit_arc(&refs[0..5], 0.01).unwrap();
        assert!((arc.cx - 0.0).abs() < 0.1);
        assert!((arc.cy - 0.0).abs() < 0.1);
        assert!(!arc.clockwise); // CCW
    }

    #[test]
    fn test_fit_arc_cw() {
        let pts = make_circle_points(0.0, 0.0, 10.0, 16, 5.0, false); // CW
        let refs: Vec<&P3> = pts.iter().collect();

        let arc = try_fit_arc(&refs[0..5], 0.01).unwrap();
        assert!(arc.clockwise);
    }

    #[test]
    fn test_fit_arc_straight_line_rejected() {
        let pts = [
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
        ];
        let refs: Vec<&P3> = pts.iter().collect();
        assert!(try_fit_arc(&refs, 0.01).is_none());
    }

    #[test]
    fn test_fit_arcs_passthrough_rapids() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 0.0));

        let result = fit_arcs(&tp, 0.01);
        assert_eq!(result.moves.len(), 2);
        assert_eq!(result.moves[0].move_type, MoveType::Rapid);
        assert_eq!(result.moves[1].move_type, MoveType::Rapid);
    }

    #[test]
    fn test_fit_arcs_converts_circle() {
        // Create a toolpath that traces a circle with linear segments
        let pts = make_circle_points(0.0, 0.0, 10.0, 32, -3.0, true);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(pts[0].x, pts[0].y, 10.0));
        tp.feed_to(pts[0], 500.0);
        for pt in &pts[1..] {
            tp.feed_to(*pt, 1000.0);
        }
        tp.rapid_to(P3::new(pts[0].x, pts[0].y, 10.0));

        let result = fit_arcs(&tp, 0.1);

        // Should have fewer moves (arcs replace multiple linears)
        assert!(
            result.moves.len() < tp.moves.len(),
            "Arc fitting should reduce move count: {} < {}",
            result.moves.len(),
            tp.moves.len()
        );

        // Should contain at least one arc move
        let arc_count = result
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count();
        assert!(arc_count > 0, "Should have at least one arc move");
    }

    #[test]
    fn test_fit_arcs_preserves_straight_lines() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(30.0, 0.0, 0.0), 1000.0);

        let result = fit_arcs(&tp, 0.01);

        // Straight line segments should pass through unchanged
        let arc_count = result
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count();
        assert_eq!(arc_count, 0, "Straight lines should not become arcs");
    }

    #[test]
    fn test_fit_arcs_different_z_breaks_arc() {
        // Points on a circle but with a Z change partway through
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, 0.0));
        tp.feed_to(P3::new(0.0, 10.0, 0.0), 1000.0);
        tp.feed_to(P3::new(-10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(0.0, -10.0, 5.0), 1000.0); // Z jump

        let result = fit_arcs(&tp, 0.01);
        // First 2 linears at Z=0 can be arc-fit, but the Z=5 one breaks the arc.
        // So we get: rapid + arc + linear = 3 moves (fewer than 4)
        assert!(
            result.moves.len() <= tp.moves.len(),
            "Should not add moves: {} <= {}",
            result.moves.len(),
            tp.moves.len()
        );
        // The Z=5 move must be preserved as linear
        let last = result.moves.last().unwrap();
        assert!(
            matches!(last.move_type, MoveType::Linear { .. }),
            "Z-changed segment should remain linear"
        );
        assert!((last.target.z - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_least_squares_exact_circle() {
        // 8 points on a known circle — verify center/radius match.
        let cx = 5.0;
        let cy = -3.0;
        let r = 12.0;
        let pts: Vec<P3> = (0..8)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / 8.0;
                P3::new(cx + r * angle.cos(), cy + r * angle.sin(), 0.0)
            })
            .collect();
        let refs: Vec<&P3> = pts.iter().collect();

        let (fx, fy, fr) = circle_from_least_squares(&refs).unwrap();
        assert!((fx - cx).abs() < 0.01, "cx: expected {}, got {}", cx, fx);
        assert!((fy - cy).abs() < 0.01, "cy: expected {}, got {}", cy, fy);
        assert!((fr - r).abs() < 0.01, "r: expected {}, got {}", r, fr);
    }

    #[test]
    fn test_least_squares_beats_3_point() {
        // 16 points with small noise — least-squares should have lower mean error.
        let cx = 0.0;
        let cy = 0.0;
        let r = 10.0;

        // Add small deterministic "noise" using sin pattern
        let pts: Vec<P3> = (0..16)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / 16.0;
                let noise = (i as f64 * 1.7).sin() * 0.05;
                P3::new(
                    cx + (r + noise) * angle.cos(),
                    cy + (r + noise) * angle.sin(),
                    0.0,
                )
            })
            .collect();
        let refs: Vec<&P3> = pts.iter().collect();

        // Least-squares fit
        let (lx, ly, lr) = circle_from_least_squares(&refs).unwrap();
        let ls_err: f64 = refs
            .iter()
            .map(|p| {
                let d = ((p.x - lx).powi(2) + (p.y - ly).powi(2)).sqrt();
                (d - lr).abs()
            })
            .sum::<f64>()
            / refs.len() as f64;

        // 3-point fit (first, middle, last)
        let (tx, ty, tr) = circle_from_3_points(
            refs[0].x, refs[0].y, refs[8].x, refs[8].y, refs[15].x, refs[15].y,
        )
        .unwrap();
        let tp_err: f64 = refs
            .iter()
            .map(|p| {
                let d = ((p.x - tx).powi(2) + (p.y - ty).powi(2)).sqrt();
                (d - tr).abs()
            })
            .sum::<f64>()
            / refs.len() as f64;

        assert!(
            ls_err <= tp_err + 1e-10,
            "Least-squares error ({:.6}) should be <= 3-point error ({:.6})",
            ls_err,
            tp_err
        );
    }

    #[test]
    fn test_fit_arcs_empty() {
        let tp = Toolpath::new();
        let result = fit_arcs(&tp, 0.01);
        assert!(result.moves.is_empty());
    }

    #[test]
    fn test_gcode_arc_output() {
        use crate::gcode::{GrblPost, emit_gcode};

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_ccw_to(P3::new(0.0, 10.0, -3.0), -10.0, 0.0, 1000.0);

        let gcode = emit_gcode(&tp, &GrblPost, 18000);
        assert!(gcode.contains("G3"), "Should contain G3 for CCW arc");
        assert!(gcode.contains("I-10.000"), "Should contain I offset");
        assert!(gcode.contains("J0.000"), "Should contain J offset");
    }

    #[test]
    fn test_gcode_cw_arc_output() {
        use crate::gcode::{GrblPost, emit_gcode};

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_cw_to(P3::new(0.0, -10.0, -3.0), -10.0, 0.0, 1000.0);

        let gcode = emit_gcode(&tp, &GrblPost, 18000);
        assert!(gcode.contains("G2"), "Should contain G2 for CW arc");
    }
}
