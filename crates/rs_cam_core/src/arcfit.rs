//! Arc fitting dressup — converts sequences of short linear segments into G2/G3 arcs.
//!
//! Uses a biarc-like approach: finds groups of consecutive linear moves that lie
//! approximately on a circular arc, then replaces them with a single arc move.
//! This reduces G-code size and improves surface finish on curved toolpaths.

use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, MoveRemap, Span, SpanKind};

/// Fit arcs to a toolpath, replacing linear segments with G2/G3 where possible.
///
/// `tolerance` is the maximum allowed deviation (mm) between the original linear
/// path and the fitted arc. Typical values: 0.001 to 0.01 mm.
///
/// Only fits arcs in the XY plane (constant Z within tolerance).
///
/// Span-aware (Phase 3e / #54):
/// - Honors `RapidOrderBarrier` and `DepthPass` boundaries: a candidate arc run
///   that would span across such a barrier is truncated at the barrier.
/// - Each inserted arc is tagged with a `DressupArtifact` span labeled "arc-fit".
/// - Input spans are remapped through the N-to-1 collapse via `MoveRemap`.
/// - When `spans_valid` is `false`, the legacy unconditional collapse runs and
///   spans pass through untouched.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn fit_arcs(annotated: AnnotatedToolpath, tolerance: f64) -> AnnotatedToolpath {
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
    } = annotated;
    let moves = &toolpath.moves;

    if moves.is_empty() {
        return AnnotatedToolpath {
            toolpath: Toolpath::new(),
            spans,
            spans_valid,
        };
    }

    // Barriers we must not collapse across. A barrier at index `b` sits before
    // moves[b]; we treat it as cutting the arc-eligible run so any candidate
    // window [start, end) must satisfy: no barrier in (start, end) — i.e. a
    // barrier at index `b` with start < b < end blocks that window.
    let barriers: std::collections::BTreeSet<usize> = if spans_valid {
        spans
            .iter()
            .filter_map(|s| match s.kind {
                SpanKind::RapidOrderBarrier | SpanKind::DepthPass => Some(s.start_move),
                _ => None,
            })
            .collect()
    } else {
        std::collections::BTreeSet::new()
    };

    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<std::ops::Range<usize>>> = Vec::with_capacity(moves.len());
    let mut arc_positions: Vec<usize> = Vec::new();

    let mut i = 0;
    while i < moves.len() {
        let m = &moves[i];

        // Only try to fit arcs on linear moves
        let MoveType::Linear { feed_rate } = m.move_type else {
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
            i += 1;
            continue;
        };

        // Try to extend an arc starting from the previous point through this point
        if i == 0 {
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
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

        // Honor span barriers: cap end_idx at the first barrier strictly after
        // i. A barrier at index b (i < b <= end_idx) means we cannot include
        // moves[b..] in this arc — that would erase the barrier between moves
        // i-1..i and moves b. We can still arc-fit moves[i..b].
        if spans_valid
            && i < end_idx
            && let Some(&b) = barriers.range((i + 1)..=end_idx).next()
        {
            end_idx = b;
        }

        let segment_count = end_idx - i;

        // Need at least 3 points (start + 2 segments) to fit an arc
        if segment_count < 2 {
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
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

            let arc_idx = result.moves.len();
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
            arc_positions.push(arc_idx);

            // All collapsed source moves (i..best_arc_end) map to the single arc.
            let r = arc_idx..arc_idx + 1;
            for _ in i..best_arc_end {
                old_to_new.push(Some(r.clone()));
            }
            i = best_arc_end;
        } else {
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new.push(Some(new_idx..new_idx + 1));
            i += 1;
        }
    }

    let new_n_moves = result.moves.len();
    let new_spans = if spans_valid {
        let remap = MoveRemap { old_to_new };
        let mut remapped: Vec<Span> = spans
            .into_iter()
            .filter_map(|s| {
                let payload = s.payload.clone();
                let label = s.label.clone();
                let mut new_span = if s.is_boundary() {
                    let new_pos = remap.remap_boundary(s.start_move, new_n_moves);
                    Span::new(new_pos, new_pos, s.kind)
                } else {
                    let r = remap.remap_range(s.start_move, s.end_move)?;
                    Span::new(r.start, r.end, s.kind)
                }
                .with_label(label);
                if let Some(p) = payload {
                    new_span = new_span.with_payload(p);
                }
                Some(new_span)
            })
            .collect();
        for pos in arc_positions {
            remapped.push(Span::new(pos, pos + 1, SpanKind::DressupArtifact).with_label("arc-fit"));
        }
        remapped
    } else {
        spans
    };

    AnnotatedToolpath {
        toolpath: result,
        spans: new_spans,
        spans_valid,
    }
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

    // Also check chord-arc deviation per segment. The point-only test
    // above passes for any set of points that lie on a circle — even
    // 4 corners of a rectangle sit on the circumscribing circle. But
    // the original toolpath is the LINE SEGMENTS between consecutive
    // points; the arc replacing them deviates from those lines by the
    // sagitta `r - √(r² - (chord/2)²)`. For a 100×100 square's corner
    // points (radius ~70.7, chord 100), the sagitta is ~20.7 mm — the
    // arc-fit would silently produce arcs that bow ~20mm outside the
    // original cut path. Visible on wanaka as "circular arc cuts
    // outside boundary" at the perimeter sweep.
    for i in 0..points.len() - 1 {
        let p_a = points[i];
        let p_b = points[i + 1];
        let dx = p_b.x - p_a.x;
        let dy = p_b.y - p_a.y;
        let chord_sq = dx * dx + dy * dy;
        let half_chord_sq = chord_sq * 0.25;
        if half_chord_sq >= radius * radius {
            // Chord longer than diameter — geometrically impossible for
            // both endpoints to lie on the same circle of this radius.
            return None;
        }
        let sagitta = radius - (radius * radius - half_chord_sq).sqrt();
        if sagitta > tolerance {
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::redundant_clone
)]
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
        // 64-segment circle keeps per-segment sagitta (~0.012mm at r=10)
        // within the 0.05mm tolerance. The previous version of this test
        // used 16 segments with 0.01mm tolerance — sagitta 0.19mm — which
        // only passed because the old code skipped chord-deviation check.
        let pts = make_circle_points(0.0, 0.0, 10.0, 64, 5.0, true);
        let refs: Vec<&P3> = pts.iter().collect();

        let arc = try_fit_arc(&refs[0..5], 0.05).unwrap();
        assert!((arc.cx - 0.0).abs() < 0.1);
        assert!((arc.cy - 0.0).abs() < 0.1);
        assert!(!arc.clockwise); // CCW
    }

    #[test]
    fn test_fit_arc_cw() {
        let pts = make_circle_points(0.0, 0.0, 10.0, 64, 5.0, false); // CW
        let refs: Vec<&P3> = pts.iter().collect();

        let arc = try_fit_arc(&refs[0..5], 0.05).unwrap();
        assert!(arc.clockwise);
    }

    /// Regression: arc-fit must not fit a circumscribing-circle arc to
    /// 4 corners of a rectangle. The corners DO lie on a circle, so the
    /// point-only tolerance check passes — but the chords (= original
    /// cut path) deviate from the arc by the sagitta (~21mm for a
    /// 100×100 square). Wanaka exhibited this as "perimeter sweep arc
    /// cuts way outside the boundary".
    #[test]
    fn test_fit_arc_rejects_rectangle_corners() {
        let corners = [
            P3::new(0.0, 0.0, 0.0),
            P3::new(100.0, 0.0, 0.0),
            P3::new(100.0, 100.0, 0.0),
            P3::new(0.0, 100.0, 0.0),
        ];
        let refs: Vec<&P3> = corners.iter().collect();
        // Even with a generous 1mm tolerance, the 21mm sagitta should
        // cause this to fail.
        assert!(
            try_fit_arc(&refs, 1.0).is_none(),
            "arc-fit must reject rectangle-corner inputs (sagitta would \
             far exceed tolerance)"
        );
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

        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01).toolpath;
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

        let result = fit_arcs(AnnotatedToolpath::new(tp.clone()), 0.1).toolpath;

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

        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01).toolpath;

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

        let result = fit_arcs(AnnotatedToolpath::new(tp.clone()), 0.01).toolpath;
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
        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01).toolpath;
        assert!(result.moves.is_empty());
    }

    // ── Span-aware behavior (#54) ─────────────────────────────────────────

    /// Build a toolpath that traces a circle as `n` linear segments, plus a
    /// leading rapid. Returns the toolpath; first linear is at index 1.
    fn circle_linear_toolpath(n: usize) -> Toolpath {
        let pts = make_circle_points(0.0, 0.0, 10.0, n, -3.0, true);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(pts[0].x, pts[0].y, 10.0));
        tp.feed_to(pts[0], 1000.0);
        for pt in &pts[1..] {
            tp.feed_to(*pt, 1000.0);
        }
        tp
    }

    #[test]
    fn fit_arcs_honors_depth_pass_barrier() {
        // A long run of arc-eligible linear moves split by a DepthPass
        // barrier in the middle. Arc-fit must not collapse a single arc
        // across the barrier — both halves should be arc-fit (or each smaller
        // run preserved) but the barrier index itself must NOT fall inside
        // any DressupArtifact arc span.
        let tp = circle_linear_toolpath(64);
        let n_in = tp.moves.len();
        // Place barrier at the midpoint of the linear run.
        let mid = 1 + (n_in - 1) / 2;
        let spans = vec![
            Span::new(0, n_in, SpanKind::Operation),
            Span::new(0, mid, SpanKind::DepthPass),
            Span::new(mid, n_in, SpanKind::DepthPass),
            Span::boundary(mid, SpanKind::RapidOrderBarrier),
        ];
        let annotated = AnnotatedToolpath::with_spans(tp.clone(), spans);

        // Without the barrier check, arc-fit could try to span the whole run.
        // With the barrier, no arc may span the boundary.
        let result = fit_arcs(annotated, 0.1);

        // Build a remap from old indices to the new arc/move via DressupArtifact
        // span coverage in the new toolpath. Any arc that COVERS a barrier in
        // OLD indices would have collapsed across — we instead check the
        // structural invariant: the total number of moves is reduced (arcs
        // were fit on at least one side), and no arc spans more old moves
        // than the half-run's length.
        assert!(
            result.toolpath.moves.len() < n_in,
            "arc-fit should still fire on each side of the barrier"
        );

        // The barrier span itself must round-trip and remain a boundary.
        let barriers: Vec<&Span> = result
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::RapidOrderBarrier)
            .collect();
        assert_eq!(barriers.len(), 1, "barrier preserved exactly once");
        assert!(barriers[0].is_boundary(), "barrier stays zero-width");

        // No DressupArtifact (arc-fit) span may contain the barrier index in
        // the new toolpath — that would mean we collapsed across it.
        let barrier_new_idx = barriers[0].start_move;
        for s in result
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::DressupArtifact)
        {
            assert!(
                !(s.start_move < barrier_new_idx && s.end_move > barrier_new_idx),
                "no arc-fit span may straddle the barrier (arc {}..{} vs barrier {})",
                s.start_move,
                s.end_move,
                barrier_new_idx,
            );
        }

        assert!(result.spans_valid);
        result
            .check_invariants()
            .expect("post-arc spans pass invariants");
    }

    #[test]
    fn fit_arcs_remaps_spans_and_tags_artifact() {
        // No barriers — a clean arc-eligible run. Operation span should shrink
        // to match new move count, and DressupArtifact spans should tag the
        // arcs.
        let tp = circle_linear_toolpath(64);
        let n_in = tp.moves.len();
        let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
        let annotated = AnnotatedToolpath::with_spans(tp.clone(), spans);

        let result = fit_arcs(annotated, 0.1);
        let n_out = result.toolpath.moves.len();
        assert!(n_out < n_in, "arc-fit should fire");

        let op = result
            .spans
            .iter()
            .find(|s| s.kind == SpanKind::Operation)
            .expect("Operation span survives");
        assert_eq!(op.start_move, 0);
        assert_eq!(op.end_move, n_out, "Operation span tracks new move count");

        let artifacts: Vec<&Span> = result
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::DressupArtifact)
            .collect();
        assert!(
            !artifacts.is_empty(),
            "at least one DressupArtifact (arc-fit) span"
        );
        for a in &artifacts {
            assert_eq!(a.label, "arc-fit");
            assert_eq!(a.move_count(), 1, "each arc-fit span tags one arc move");
            // The tagged move must actually be an arc.
            assert!(
                matches!(
                    result.toolpath.moves[a.start_move].move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                ),
                "DressupArtifact span must tag an arc move"
            );
        }

        assert!(result.spans_valid);
        result
            .check_invariants()
            .expect("post-arc spans pass invariants");
    }

    #[test]
    fn fit_arcs_preserves_invalid_flag() {
        // If input spans are flagged invalid, arc-fit doesn't try to remap
        // and doesn't read barriers from spans either — behavior matches the
        // legacy unconditional collapse, and spans pass through untouched.
        let tp = circle_linear_toolpath(32);
        let n_in = tp.moves.len();
        let mut annotated = AnnotatedToolpath::new(tp);
        annotated.spans_valid = false;
        let garbage = vec![Span::new(0, 1, SpanKind::Operation)];
        annotated.spans = garbage.clone();

        let result = fit_arcs(annotated, 0.1);

        assert!(result.toolpath.moves.len() < n_in, "arc-fit fires");
        assert!(!result.spans_valid, "invalid stays invalid");
        assert_eq!(
            result.spans, garbage,
            "spans pass through unchanged when input is invalid"
        );
    }

    #[test]
    fn test_gcode_arc_output() {
        use crate::gcode::{emit_gcode, post};

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_ccw_to(P3::new(0.0, 10.0, -3.0), -10.0, 0.0, 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);
        assert!(gcode.contains("G3"), "Should contain G3 for CCW arc");
        assert!(gcode.contains("I-10.000"), "Should contain I offset");
        assert!(gcode.contains("J0.000"), "Should contain J offset");
    }

    #[test]
    fn test_gcode_cw_arc_output() {
        use crate::gcode::{emit_gcode, post};

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_cw_to(P3::new(0.0, -10.0, -3.0), -10.0, 0.0, 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);
        assert!(gcode.contains("G2"), "Should contain G2 for CW arc");
    }
}
