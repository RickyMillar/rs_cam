//! Arc fitting dressup — converts sequences of short linear segments into G2/G3 arcs.
//!
//! Uses a biarc-like approach: finds groups of consecutive linear moves that lie
//! approximately on a circular arc, then replaces them with a single arc move.
//! This reduces G-code size and improves surface finish on curved toolpaths.
//!
//! Roadmap F.10: fits are also rejected when the recovered radius exceeds
//! `tool_radius * LARGE_ARC_RADIUS_MULTIPLIER`. Kåsa's algebraic least-squares
//! is biased toward huge circles on barely-curving polylines; without this cap
//! adaptive3d occasionally emits arcs of R = 257mm on parts whose largest
//! feature is ~72mm. Pass `f64::INFINITY` to disable the cap (e.g. in tests
//! that don't model a specific tool).
//!
//! The cap is **ENVELOPE-relative**: production supplies `tool_diameter / 2.0`
//! (`compute/execute.rs`), i.e. `MillingCutter::envelope_radius_mm()`. It must
//! track `narrate::append_large_arc_anomalies`, which applies the same
//! multiplier to the same radius so narration stays a post-condition on this
//! cap rather than an independent quality hint. Do not switch either side to
//! `cusp_radius_mm()` — on a tapered tool that drops one side 6× and floods
//! narration with false positives (H2.6, `TOOL_SCALE_SEMANTICS.md` §5;
//! pinned by `tests/tool_scale_semantics_pr2.rs`).

use crate::condition::FEED_EPS;
use crate::geo::P3;
use crate::narrate::LARGE_ARC_RADIUS_MULTIPLIER;
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
pub fn fit_arcs(
    annotated: AnnotatedToolpath,
    tolerance: f64,
    tool_radius: f64,
) -> AnnotatedToolpath {
    // Barriers we must not collapse across. A barrier at index `b` sits before
    // moves[b]; we treat it as cutting the arc-eligible run so any candidate
    // window [start, end) must satisfy: no barrier in (start, end) — i.e. a
    // barrier at index `b` with start < b < end blocks that window.
    let barriers: std::collections::BTreeSet<usize> = if annotated.spans_valid {
        annotated.rapid_order_barriers().into_iter().collect()
    } else {
        std::collections::BTreeSet::new()
    };

    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;
    let moves = &toolpath.moves;

    if moves.is_empty() {
        return AnnotatedToolpath {
            toolpath: Toolpath::new(),
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        };
    }

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

        // Collect consecutive linear moves at the same feed rate. Z may vary:
        // `try_fit_arc` accepts a run only if it forms a valid planar arc
        // (constant Z) OR a helix (Z linear with swept angle), so runs no longer
        // split on every Z change — helical entries and spiral descents now
        // arc-fit into G2/G3 with a Z endpoint instead of dozens of tiny G1s.
        let mut end_idx = i;
        while end_idx < moves.len() {
            match moves[end_idx].move_type {
                MoveType::Linear { feed_rate: f } if (f - feed_rate).abs() < FEED_EPS => {
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

            if let Some(arc) = try_fit_arc(&points, tolerance, tool_radius) {
                best_arc_end = run_end;
                best_arc = Some(arc);
                run_end += 1;
            } else {
                break;
            }
        }

        if let Some(arc) = best_arc {
            let end_pt = &moves[best_arc_end - 1].target;
            // Helical-aware: emit the run's END Z. GRBL interpolates Z linearly
            // from the current position (start.z) to this commanded Z over the
            // swept angle — and try_fit_arc only accepted the run if its Z is
            // linear with swept angle, so the helix matches the source path.
            // For a constant-Z run end_pt.z == start.z (unchanged behaviour).
            let z = end_pt.z;

            // I, J = offset from start point to center
            let ij_i = arc.cx - start.x;
            let ij_j = arc.cy - start.y;

            // Collapsed-arc intent inherits from the source feed segments.
            // All collapsed segments share `feed_rate` already; intent
            // taken from the first collapsed source move.
            let arc_intent = moves[i].intent;

            let arc_idx = result.moves.len();
            if arc.clockwise {
                result.moves.push(Move {
                    target: P3::new(end_pt.x, end_pt.y, z),
                    move_type: MoveType::ArcCW {
                        i: ij_i,
                        j: ij_j,
                        feed_rate,
                    },
                    intent: arc_intent,
                });
            } else {
                result.moves.push(Move {
                    target: P3::new(end_pt.x, end_pt.y, z),
                    move_type: MoveType::ArcCCW {
                        i: ij_i,
                        j: ij_j,
                        feed_rate,
                    },
                    intent: arc_intent,
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
        let mut remapped = remap.remap_spans(&spans, new_n_moves);
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
        planner_engagement,
        rest_grid,
        rest_regions,
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
///
/// `tool_radius` caps the fitted radius at
/// `tool_radius * LARGE_ARC_RADIUS_MULTIPLIER` (Roadmap F.10). Pass
/// `f64::INFINITY` to disable the cap. The bound is the ENVELOPE radius and
/// must stay in lockstep with narration's threshold — see the module doc.
fn try_fit_arc(points: &[&P3], tolerance: f64, tool_radius: f64) -> Option<ArcParams> {
    if points.len() < 3 {
        return None;
    }

    // Least-squares circle fit (Kåsa's algebraic method) for better accuracy
    // on noisy or partial-arc points. Falls back to 3-point if too few points.
    let (cx_raw, cy_raw, _radius_raw) = if points.len() >= 5 {
        circle_from_least_squares(points)?
    } else {
        let p0 = points[0];
        let pm = points[points.len() / 2];
        let pn = points[points.len() - 1];
        circle_from_3_points(p0.x, p0.y, pm.x, pm.y, pn.x, pn.y)?
    };

    // GRBL endpoint-consistency correction.
    //
    // GRBL (and most real controllers) rejects an arc command when the
    // start radius `hypot(I, J)` and the implied end radius
    // `hypot(end.x - center.x, end.y - center.y)` differ by more than
    // `$12` (default 0.010 mm). Kåsa's algebraic LSQ minimises the
    // collective squared algebraic distance — it does NOT guarantee
    // that start and end lie exactly equidistant from the recovered
    // center. The two radii can drift by up to 2 × `tolerance` apart
    // even when every point passes the per-point tolerance gate.
    //
    // Fix: project the LSQ centre onto the perpendicular bisector of
    // start↔end. The projected centre is the closest point to the LSQ
    // estimate that is exactly equidistant from start and end, so the
    // emitted `(I, J)` and the controller-computed end radius agree to
    // floating-point precision (typ. ~1e-15 mm).
    //
    // The projection may push the centre slightly away from the LSQ
    // optimum; the subsequent tolerance loop re-checks every
    // intermediate point against the corrected centre and rejects the
    // fit if any drifts past `tolerance`. So we trade a few rejected
    // candidates (which fall back to linear segments) for arcs that
    // every real GRBL build will accept at its default `$12`.
    let p_start = points[0];
    let p_end = points[points.len() - 1];
    let mid_x = 0.5 * (p_start.x + p_end.x);
    let mid_y = 0.5 * (p_start.y + p_end.y);
    let chord_dx = p_end.x - p_start.x;
    let chord_dy = p_end.y - p_start.y;
    let chord_len_sq = chord_dx * chord_dx + chord_dy * chord_dy;
    if chord_len_sq < 1e-20 {
        // Start ≈ end: the run forms (nearly) a closed loop. GRBL needs
        // the R-form for full circles and we never emit that; fall back
        // to leaving the source linear segments in place.
        return None;
    }
    let chord_len = chord_len_sq.sqrt();
    // Perpendicular bisector direction (unit vector).
    let bisector_x = -chord_dy / chord_len;
    let bisector_y = chord_dx / chord_len;
    // Component of (LSQ_centre - midpoint) along the bisector.
    let v_x = cx_raw - mid_x;
    let v_y = cy_raw - mid_y;
    let t = v_x * bisector_x + v_y * bisector_y;
    let cx = mid_x + t * bisector_x;
    let cy = mid_y + t * bisector_y;
    // Recompute radius from the corrected centre. By construction,
    // `hypot(start - centre) == hypot(end - centre) == radius`.
    let r_dx = p_start.x - cx;
    let r_dy = p_start.y - cy;
    let radius = (r_dx * r_dx + r_dy * r_dy).sqrt();

    // Reject degenerate arcs (very large radius = nearly straight line)
    if radius > 1e6 {
        return None;
    }

    // Roadmap F.10: cap fitted radius at LARGE_ARC_RADIUS_MULTIPLIER × tool
    // ENVELOPE radius. Kåsa's algebraic least-squares is biased toward huge
    // circles on barely-curving inputs; without this cap, adaptive3d output
    // occasionally collapses a slightly-bowed polyline into an arc whose
    // radius dwarfs any feature on the part. The narration warns on these via
    // the same multiplier applied to the same radius — keeping the fitter and
    // narrator in sync is the contract, not a coincidence (§5 / H2.6).
    if radius > tool_radius * LARGE_ARC_RADIUS_MULTIPLIER {
        return None;
    }

    // Check all intermediate points are within tolerance of the corrected
    // circle. Start and end are exactly on the circle by construction;
    // interior points may have drifted slightly when the centre was
    // projected onto the bisector — that drift is what this loop guards.
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
    let mut clockwise = cross < 0.0;

    // …and then CHECK it, because the cross product is only a hint. On a
    // shallow run the three sample points are nearly collinear, `cross`
    // is dominated by rounding, and its sign flips at random. Getting it
    // wrong is not a small error: the arc through the same two endpoints
    // on the same circle is then the REFLEX one, so a 3.6° sweep becomes
    // 356° and the machine drives a full circle through the part.
    //
    // Measured in the field (wanaka ×2, Op B, 2026-07-28): an `ArcCW`
    // with endpoints 3.55 mm apart on a 57.04 mm circle, swept angle
    // 356.4°, arc length 354 mm for a ~3.5 mm polyline. 421 of Op B's
    // 11 346 arcs were reflex like this, worst 359.12°. The simulator cut
    // along them — 5.5 mm off a column 97 mm from the move.
    //
    // The invariant the fitter was missing: a fitted arc must be about as
    // LONG as the polyline it replaces. Both directions pass through both
    // endpoints, so length is what distinguishes them. Pick the direction
    // whose arc length is closer to the polyline's, then reject outright
    // if even that one is implausible — a genuine reflex arc cannot be
    // fit from points whose own path is far shorter than its sweep.
    {
        let mut poly_len = 0.0_f64;
        for w in points.windows(2) {
            // SAFETY: `windows(2)` always yields exactly two elements.
            #[allow(clippy::indexing_slicing)]
            let (p, q) = (w[0], w[1]);
            poly_len += ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt();
        }
        let a0 = (p_first.y - cy).atan2(p_first.x - cx);
        let a1 = (p_last.y - cy).atan2(p_last.x - cx);
        let sweep_for = |cw: bool| -> f64 {
            let mut s = if cw { a0 - a1 } else { a1 - a0 };
            while s <= 0.0 {
                s += std::f64::consts::TAU;
            }
            s
        };
        let err = |cw: bool| (radius * sweep_for(cw) - poly_len).abs();
        if err(!clockwise) < err(clockwise) {
            clockwise = !clockwise;
        }
        // A correct fit tracks the polyline closely; the sagitta test above
        // already bounds how far the arc may stray from it. Half the
        // polyline length of slack is far beyond any legitimate fit and
        // still rejects every reflex mis-direction (354 mm vs 3.5 mm).
        if (radius * sweep_for(clockwise) - poly_len).abs() > 0.5 * poly_len + tolerance {
            return None;
        }
    }

    // Helical / planar Z validity. GRBL interpolates Z linearly with the swept
    // angle along a G2/G3 arc, so a Z-varying run is a valid arc only if it is a
    // true helix (Z linear with cumulative swept angle). A constant-Z run
    // (|z_end - z_start| <= tolerance) trivially passes. Reject everything else
    // so terrain/ramp paths whose Z wanders fall back to linear segments.
    let z_start = p_first.z;
    let z_end = p_last.z;
    if (z_end - z_start).abs() > tolerance {
        // Cumulative swept angle per point, in the arc's travel direction.
        let mut swept = Vec::with_capacity(points.len());
        let mut cum = 0.0_f64;
        let mut prev_ang = (points[0].y - cy).atan2(points[0].x - cx);
        swept.push(0.0);
        for pt in &points[1..] {
            let a = (pt.y - cy).atan2(pt.x - cx);
            let step = if clockwise {
                prev_ang - a
            } else {
                a - prev_ang
            };
            cum += step.rem_euclid(std::f64::consts::TAU);
            swept.push(cum);
            prev_ang = a;
        }
        if cum < 1e-9 {
            // No net sweep but Z changed → a vertical/degenerate move, not a helix.
            return None;
        }
        for (pt, &s) in points.iter().zip(swept.iter()) {
            let frac = s / cum;
            let expected_z = z_start + (z_end - z_start) * frac;
            if (pt.z - expected_z).abs() > tolerance {
                return None;
            }
        }
    }

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

        // tool_radius = INFINITY disables the F.10 radius cap; this test
        // exercises the geometric fit, not the cap.
        let arc = try_fit_arc(&refs[0..5], 0.05, f64::INFINITY).unwrap();
        assert!((arc.cx - 0.0).abs() < 0.1);
        assert!((arc.cy - 0.0).abs() < 0.1);
        assert!(!arc.clockwise); // CCW
    }

    #[test]
    fn test_fit_arc_cw() {
        let pts = make_circle_points(0.0, 0.0, 10.0, 64, 5.0, false); // CW
        let refs: Vec<&P3> = pts.iter().collect();

        let arc = try_fit_arc(&refs[0..5], 0.05, f64::INFINITY).unwrap();
        assert!(arc.clockwise);
    }

    /// Regression, wanaka ×2 Op B 2026-07-28: a shallow run whose three
    /// direction-sample points are nearly collinear got the cross-product
    /// hint backwards, and the arc through the same endpoints on the same
    /// circle in the WRONG direction is the reflex one. Field case: an
    /// `ArcCW` with endpoints 3.55 mm apart on a 57.04 mm circle sweeping
    /// 356.4° — 354 mm of arc for a 3.5 mm path — which the simulator cut
    /// along, taking 5.5 mm off a column 97 mm away, and which a machine
    /// would have driven as a 114 mm circle through the workpiece. 421 of
    /// Op B's 11 346 arcs were reflex like this.
    ///
    /// A fitted arc must be about as long as the polyline it replaces;
    /// both directions share the endpoints, so length is what tells them
    /// apart. Either the fitter picks the short way or it declines.
    #[test]
    fn fit_arc_never_emits_the_reflex_direction() {
        // The field arc's own circle: centre (158.544, 93.391), r 57.04.
        let (cx, cy, r) = (158.5441, 93.3912, 57.0421);
        let a0: f64 = 71.34_f64.to_radians();
        let a1: f64 = 74.90_f64.to_radians();
        // A short CCW run along that circle, sampled densely enough that
        // the direction hint is numerically marginal — which is exactly
        // the regime that produced the bug.
        let pts: Vec<P3> = (0..=8)
            .map(|i| {
                let t = f64::from(i) / 8.0;
                let a = a0 + (a1 - a0) * t;
                P3::new(cx + r * a.cos(), cy + r * a.sin(), 1.218 + 1.636 * t)
            })
            .collect();
        let refs: Vec<&P3> = pts.iter().collect();
        let poly_len: f64 = pts
            .windows(2)
            .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
            .sum();

        if let Some(arc) = try_fit_arc(&refs, 0.05, f64::INFINITY) {
            let s0 = (pts[0].y - arc.cy).atan2(pts[0].x - arc.cx);
            let s1 = (pts[8].y - arc.cy).atan2(pts[8].x - arc.cx);
            let mut sweep = if arc.clockwise { s0 - s1 } else { s1 - s0 };
            while sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let arc_len = r * sweep;
            assert!(
                arc_len < 2.0 * poly_len,
                "fitted arc sweeps {:.1}° ({arc_len:.2}mm) for a {poly_len:.2}mm polyline — \
                 the reflex direction was emitted",
                sweep.to_degrees()
            );
        }
        // Declining to fit is also correct; emitting the reflex arc is not.
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
            try_fit_arc(&refs, 1.0, f64::INFINITY).is_none(),
            "arc-fit must reject rectangle-corner inputs (sagitta would \
             far exceed tolerance)"
        );
    }

    /// Regression for Roadmap F.10: a barely-curving polyline must not be
    /// fitted as a huge arc when a tool radius is supplied. Wanaka's
    /// adaptive3d emitted R=257mm arcs on a 6mm end-mill (radius 3mm) — the
    /// Kåsa algebraic fit slides the centre far away for small-bow inputs,
    /// even though every chord stays within the 0.05mm sagitta envelope.
    /// With the F.10 cap (30 × 3mm = 90mm), the fitter must refuse.
    #[test]
    fn test_fit_arc_rejects_huge_radius_for_tool_radius() {
        // y = 0.001 * x², x = 0..20 in 1mm steps → 21 points along a barely
        // curving parabola. Best-fit circle radius ~ 500mm.
        let pts: Vec<P3> = (0..=20)
            .map(|i| {
                let x = i as f64;
                P3::new(x, 0.001 * x * x, 0.0)
            })
            .collect();
        let refs: Vec<&P3> = pts.iter().collect();

        // With tool_radius=3.0 the cap is 90mm; any fit at this scale is far
        // larger than that and must be rejected.
        assert!(
            try_fit_arc(&refs, 0.05, 3.0).is_none(),
            "arc-fit must reject implausibly large radii (>30× tool radius)"
        );

        // Sanity: without the cap, the algorithm DOES produce a fit (this is
        // exactly the F.10 failure mode we're guarding against).
        if let Some(arc) = try_fit_arc(&refs, 0.05, f64::INFINITY) {
            let dx = refs[0].x - arc.cx;
            let dy = refs[0].y - arc.cy;
            let r = (dx * dx + dy * dy).sqrt();
            assert!(
                r > 90.0,
                "uncapped fit should expose the Kåsa large-R bias (got R={})",
                r,
            );
        }
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
        assert!(try_fit_arc(&refs, 0.01, f64::INFINITY).is_none());
    }

    #[test]
    fn test_fit_arcs_passthrough_rapids() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(10.0, 0.0, 0.0));

        // tool_radius = INFINITY: tests check structural behavior independent
        // of the F.10 large-radius cap.
        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01, f64::INFINITY).toolpath;
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

        let result = fit_arcs(AnnotatedToolpath::new(tp.clone()), 0.1, f64::INFINITY).toolpath;

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

        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01, f64::INFINITY).toolpath;

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

        let result = fit_arcs(AnnotatedToolpath::new(tp.clone()), 0.01, f64::INFINITY).toolpath;
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
        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.01, f64::INFINITY).toolpath;
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
        let result = fit_arcs(annotated, 0.1, f64::INFINITY);

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

        let result = fit_arcs(annotated, 0.1, f64::INFINITY);
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

        let result = fit_arcs(annotated, 0.1, f64::INFINITY);

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

    /// Phase 2: a clean helix (circle in XY, Z linear with swept angle) arc-fits
    /// into G2/G3 with a Z endpoint — the case that lets helical descents and
    /// (lead-separated) helix entries collapse from dozens of G1s to a few arcs.
    #[test]
    fn test_fit_arcs_helix_descent() {
        let r = 10.0;
        let n = 27; // 10° steps over 270°
        let pt = |k: usize| {
            let frac = k as f64 / n as f64;
            let ang = frac * 0.75 * std::f64::consts::TAU; // 270°
            P3::new(r * ang.cos(), r * ang.sin(), -3.0 * frac)
        };
        let mut tp = Toolpath::new();
        // Anchor ON the circle at the run's start Z (no vertical lead in the run).
        tp.rapid_to(pt(0));
        for k in 1..=n {
            tp.feed_to(pt(k), 500.0);
        }
        let result = fit_arcs(AnnotatedToolpath::new(tp.clone()), 0.05, f64::INFINITY).toolpath;
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
        assert!(
            arc_count >= 1,
            "clean helix must arc-fit; got {} moves, 0 arcs",
            result.moves.len()
        );
        assert!(
            result.moves.len() < tp.moves.len(),
            "helix should reduce moves"
        );
        // The fitted arc carries the descended Z (GRBL helically interpolates).
        let last_arc_z = result
            .moves
            .iter()
            .rev()
            .find_map(|m| match m.move_type {
                MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => Some(m.target.z),
                _ => None,
            })
            .unwrap();
        assert!(
            last_arc_z < -2.0,
            "helix arc must descend in Z, end z={last_arc_z}"
        );
    }

    /// Phase 2 (real-impact): replicate `dressup::emit_helix`'s exact structure
    /// — rapid to the center, then orbit the center at `radius` descending in Z
    /// (36 steps/rev), then a final return to center. The orbit anchor (center)
    /// and the return move are OFF the circle, so this verifies the greedy fitter
    /// recovers and still collapses the bulk of the helix into arcs (a roughing
    /// plunge goes from dozens of G1s to a couple of G2/G3s).
    #[test]
    fn test_fit_arcs_helix_entry_structure() {
        let (cx, cy, radius) = (0.0, 0.0, 2.0);
        let steps_per_rev = 36usize;
        let revs = 2.0;
        let total_steps = (revs * steps_per_rev as f64) as usize;
        let z_top = 1.0;
        let dz = 2.0;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(cx, cy, z_top)); // anchor AT center (off-circle)
        for i in 1..=total_steps {
            let t = i as f64 / total_steps as f64;
            let ang = revs * std::f64::consts::TAU * t;
            let z = z_top - dz * t;
            tp.feed_to(
                P3::new(cx + radius * ang.cos(), cy + radius * ang.sin(), z),
                300.0,
            );
        }
        tp.feed_to(P3::new(cx, cy, z_top - dz), 300.0); // return to center
        let n_in = tp.moves.len();
        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.05, f64::INFINITY).toolpath;
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
        assert!(
            arc_count >= 1,
            "helix entry must arc-fit despite the center anchor; {n_in} in -> {} out, {arc_count} arcs",
            result.moves.len()
        );
        // 73 input moves (rapid + 72 helix + return) must collapse substantially.
        assert!(
            result.moves.len() < n_in / 2,
            "helix entry should at least halve the move count: {n_in} -> {}",
            result.moves.len()
        );
    }

    /// Phase 2: a run that is circular in XY but whose Z does NOT vary linearly
    /// with swept angle is not a helix — it must fall back to linear segments,
    /// not emit a G2/G3 that GRBL would interpolate into the wrong Z path.
    #[test]
    fn test_fit_arc_rejects_nonlinear_z() {
        let r = 10.0;
        let n = 12;
        let mut tp = Toolpath::new();
        let pt = |k: usize, z: f64| {
            let ang = (k as f64 / n as f64) * 0.5 * std::f64::consts::TAU; // 180°
            P3::new(r * ang.cos(), r * ang.sin(), z)
        };
        tp.rapid_to(pt(0, 0.0));
        for k in 1..=n {
            // Z oscillates between 0 and -2 — circular in XY, non-linear in Z.
            let z = if k % 2 == 0 { -2.0 } else { 0.0 };
            tp.feed_to(pt(k, z), 500.0);
        }
        let result = fit_arcs(AnnotatedToolpath::new(tp), 0.05, f64::INFINITY).toolpath;
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
        assert_eq!(arc_count, 0, "non-helical Z variation must not arc-fit");
    }
}
