//! Rest machining toolpath generation — the 2D polygon rest op.
//!
//! Generates toolpaths for a smaller tool to clean up material that a
//! larger tool could not reach. Uses geometric comparison: computes the
//! reachable region for the large tool and generates scan-line passes
//! only in areas the large tool missed (inside corners, narrow channels).
//!
//! Algorithm:
//! 1. Offset polygon inward by `prev_tool_radius` → region the large tool
//!    center can reach. Everything here is already cleared.
//! 2. Generate zigzag scan lines for the small tool (offset by `tool_radius`).
//! 3. Walk each line, checking if each sample point is inside the large
//!    tool's reachable region. Cut only where it isn't.
//!
//! Reference: research/raw_algorithms.md §4.5
//!
//! Not to be confused with [`crate::rest_field`], the 3D tool-radius-aware
//! rest-*depth* field used by pencil finishing to find where a fine detail
//! tool still has material left after a coarser rough/finish pass. That
//! module works on mesh dexel/heightmap depth comparisons; this one works
//! on 2D polygon offsets. Zero code overlap between the two.

use crate::geo::{P2, P3};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Parameters for rest machining.
///
/// `Copy` since the G2 hoist (2026-08-20) — see [`crate::pocket::PocketParams`]
/// for why a depth-stepping caller wants struct-update rather than a re-listed
/// literal per Z level.
#[derive(Debug, Clone, Copy)]
pub struct RestParams {
    /// Previous (larger) tool radius in mm.
    pub prev_tool_radius: f64,
    /// Current (smaller) tool radius in mm.
    pub tool_radius: f64,
    /// Z height to cut at in mm (negative for cuts below stock top).
    pub cut_depth: f64,
    /// Distance between scan lines in mm.
    pub stepover: f64,
    /// Cutting feed rate in mm/min.
    pub feed_rate: f64,
    /// Plunge feed rate in mm/min.
    pub plunge_rate: f64,
    /// Safe Z height for rapid moves in mm.
    pub safe_z: f64,
    /// Scan line angle in degrees (0 = X axis).
    pub angle: f64,
}

/// Check if a point is inside any polygon in a list.
fn point_in_any_polygon(p: &P2, polygons: &[Polygon2]) -> bool {
    polygons.iter().any(|poly| poly.contains_point(p))
}

/// Generate rest machining toolpath for a 2D polygon region.
///
/// Computes the geometric difference between what the small tool can
/// reach and what the previous large tool could reach. Generates
/// zigzag scan-line passes only in the "rest regions" — corners and
/// narrow areas the large tool missed.
///
/// If the small tool radius >= large tool radius, returns an empty toolpath.
/// If the large tool can't fit at all, falls back to a full zigzag.
#[tracing::instrument(skip(polygon, params), fields(
    tool_radius = params.tool_radius,
    prev_tool_radius = params.prev_tool_radius,
    stepover = params.stepover,
))]
pub fn rest_machining_toolpath(polygon: &Polygon2, params: &RestParams) -> Toolpath {
    rest_segments_to_toolpath(&rest_segments(polygon, params), params)
}

/// The **Z-independent half** of [`rest_machining_toolpath`]: the XY polylines
/// this rest pass will cut.
///
/// Split out for the G2 hoist (2026-08-20), and rest is the family that gained
/// most from it — the whole of this function is XY (an inward offset, a zigzag
/// scan-line build, and a per-sample containment walk over the large tool's
/// reachable region), while `cut_depth` reaches the output only through
/// [`rest_segments_to_toolpath`]'s stamp. A depth-stepped rest op used to
/// repeat all of it once per Z level.
///
/// Each returned polyline is one contiguous run of samples the previous larger
/// tool could NOT reach. The "large tool cannot fit at all" fallback returns
/// the raw scan lines as two-point polylines, which emit through the same
/// `emit_path_segment_with_intent(ClearingCut)` envelope
/// [`crate::zigzag::lines_to_toolpath`] uses — byte-identical to the previous
/// `zigzag_toolpath` delegation, and it no longer recomputes the scan lines
/// this function has already built.
#[must_use]
pub fn rest_segments(polygon: &Polygon2, params: &RestParams) -> Vec<Vec<P2>> {
    if params.tool_radius >= params.prev_tool_radius {
        return Vec::new();
    }

    // What the large tool center could reach (inward offset by large radius)
    let large_reachable = offset_polygon(polygon, params.prev_tool_radius);

    // Generate zigzag scan lines for the small tool
    let lines =
        crate::zigzag::zigzag_lines(polygon, params.tool_radius, params.stepover, params.angle);

    if lines.is_empty() {
        return Vec::new();
    }

    // If large tool can't fit at all, the entire pocket is rest region
    if large_reachable.is_empty() {
        return lines.iter().map(|l| vec![l[0], l[1]]).collect();
    }

    // Sampling resolution along each scan line, in mm. Deliberately clamped
    // to 0.25-0.5mm regardless of the actual tool radius passed in — a tiny
    // rest tool doesn't get a finer sample step, and a huge one doesn't get
    // a coarser one. This bounds the per-line sample count for very small or
    // very large tools; it is not a tool-radius-proportional resolution.
    let sample_step = params.tool_radius.clamp(0.25, 0.5);

    // G9/parallelism (2026-08-20): each scan line is independent — its
    // containment walk reads `large_reachable` and nothing else, and it
    // contributes a contiguous block of segments at a fixed position in the
    // output. Mapping per line and concatenating in order therefore produces
    // the byte-identical `Vec<Vec<P2>>` the serial walk did.
    #[cfg(feature = "parallel")]
    let per_line: Vec<Vec<Vec<P2>>> = lines
        .par_iter()
        .map(|line| rest_segments_on_line(line, &large_reachable, sample_step))
        .collect();
    #[cfg(not(feature = "parallel"))]
    let per_line: Vec<Vec<Vec<P2>>> = lines
        .iter()
        .map(|line| rest_segments_on_line(line, &large_reachable, sample_step))
        .collect();

    per_line.into_iter().flatten().collect()
}

/// Walk one scan line, returning the runs of samples the previous larger tool
/// could NOT reach. Empty for a degenerate (zero-length) line.
#[allow(clippy::indexing_slicing)]
// SAFETY: `line` is a fixed-size [P2; 2] array.
fn rest_segments_on_line(
    line: &[P2; 2],
    large_reachable: &[Polygon2],
    sample_step: f64,
) -> Vec<Vec<P2>> {
    let dx = line[1].x - line[0].x;
    let dy = line[1].y - line[0].y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return Vec::new();
    }

    let n_samples = (len / sample_step).ceil() as usize;

    let mut segments: Vec<Vec<P2>> = Vec::new();
    // Walk along the line, collecting segments NOT in large_reachable
    let mut segment_points: Vec<P2> = Vec::new();

    for i in 0..=n_samples {
        let t = i as f64 / n_samples.max(1) as f64;
        let x = line[0].x + t * dx;
        let y = line[0].y + t * dy;
        let p = P2::new(x, y);

        let in_large = point_in_any_polygon(&p, large_reachable);

        if !in_large {
            segment_points.push(p);
        } else if !segment_points.is_empty() {
            // Exiting rest region — close the segment
            segments.push(std::mem::take(&mut segment_points));
        }
    }

    // Keep the final segment if the line ended in a rest region
    if !segment_points.is_empty() {
        segments.push(segment_points);
    }

    segments
}

/// The **Z-dependent half**: stamp pre-computed rest polylines at
/// `params.cut_depth`.
#[must_use]
pub fn rest_segments_to_toolpath(segments: &[Vec<P2>], params: &RestParams) -> Toolpath {
    let mut tp = Toolpath::new();
    for seg in segments {
        emit_rest_segment(&mut tp, seg, params);
    }
    tp
}

/// Emit a single rest machining cut segment into the toolpath.
fn emit_rest_segment(tp: &mut Toolpath, points: &[P2], params: &RestParams) {
    if points.is_empty() {
        return;
    }
    let z = params.cut_depth;
    let path_3d: Vec<P3> = points.iter().map(|p| P3::new(p.x, p.y, z)).collect();
    tp.emit_path_segment_with_intent(
        &path_3d,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
        crate::toolpath::MoveIntent::ClearingCut,
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params() -> RestParams {
        RestParams {
            prev_tool_radius: 6.0, // 12mm large tool
            tool_radius: 1.5,      // 3mm small tool
            cut_depth: -3.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            angle: 0.0,
        }
    }

    // ── G2 hoist ────────────────────────────────────────────────────────

    fn move_bits(tp: &Toolpath) -> Vec<(u64, u64, u64, String)> {
        tp.moves
            .iter()
            .map(|m| {
                (
                    m.target.x.to_bits(),
                    m.target.y.to_bits(),
                    m.target.z.to_bits(),
                    format!("{:?}/{:?}", m.move_type, m.intent),
                )
            })
            .collect()
    }

    /// **G2, rest side, and the branch that needed proving.**
    ///
    /// The hoist replaced the "large tool cannot fit at all" fallback — which
    /// delegated to `zigzag_toolpath`, RE-deriving scan lines this function had
    /// already built — with the lines it already has, emitted through
    /// `emit_rest_segment`. Those are two different code paths reaching the
    /// same `emit_path_segment_with_intent(ClearingCut)` envelope, so "they
    /// agree" is a claim, not a tautology. This asserts it bit for bit.
    #[test]
    fn the_full_zigzag_fallback_still_matches_the_zigzag_op_exactly() {
        // 8 mm square: a 6 mm-radius large tool cannot fit, so the whole
        // pocket is rest region and the fallback fires.
        let sq = square_polygon(8.0);
        let params = default_params();
        assert!(
            offset_polygon(&sq, params.prev_tool_radius).is_empty(),
            "this fixture must actually take the fallback, or it proves nothing"
        );

        let got = rest_machining_toolpath(&sq, &params);
        let want = crate::zigzag::zigzag_toolpath(
            &sq,
            &crate::zigzag::ZigzagParams {
                tool_radius: params.tool_radius,
                stepover: params.stepover,
                cut_depth: params.cut_depth,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                angle: params.angle,
            },
        );

        assert!(!want.moves.is_empty(), "vacuous fixture");
        assert_eq!(
            move_bits(&got),
            move_bits(&want),
            "the fallback no longer matches the zigzag op it used to delegate to"
        );
    }

    /// **G2's premise, stated as a property.** The rest geometry — the inward
    /// offset, the scan lines and the containment walk — does not depend on
    /// `cut_depth`, which is why a depth-stepped rest op can compute it once.
    /// If this ever stopped holding, the hoist in `compute::execute` would be
    /// silently wrong rather than loudly wrong.
    #[test]
    fn rest_segments_are_independent_of_cut_depth() {
        let sq = square_polygon(40.0);
        let base = default_params();
        let reference = rest_segments(&sq, &base);
        assert!(
            !reference.is_empty(),
            "vacuous fixture — no rest region to compare"
        );

        for z in [-0.001_f64, -5.0, -37.5] {
            let other = rest_segments(
                &sq,
                &RestParams {
                    cut_depth: z,
                    ..base
                },
            );
            assert_eq!(other.len(), reference.len(), "segment count moved at z={z}");
            for (a, b) in other.iter().zip(reference.iter()) {
                let ab: Vec<(u64, u64)> =
                    a.iter().map(|p| (p.x.to_bits(), p.y.to_bits())).collect();
                let bb: Vec<(u64, u64)> =
                    b.iter().map(|p| (p.x.to_bits(), p.y.to_bits())).collect();
                assert_eq!(ab, bb, "segment geometry moved at z={z}");
            }
        }
    }

    /// **Parallel scan-line sentry (2026-08-20).**
    ///
    /// `rest_segments` now maps the per-line containment walk across the
    /// rayon pool. The claim is that concatenating the per-line results in
    /// order reproduces the serial walk exactly — including where segments
    /// break, which is the part a reordering would corrupt without changing
    /// the point count. Raw bits, not `==`: `-0.0 == 0.0` would mask it.
    #[test]
    fn parallel_rest_segments_match_the_serial_walk() {
        /// Verbatim pre-parallel body of `rest_segments`' sampling loop.
        fn serial_segments(polygon: &Polygon2, params: &RestParams) -> Vec<Vec<P2>> {
            if params.tool_radius >= params.prev_tool_radius {
                return Vec::new();
            }
            let large_reachable = offset_polygon(polygon, params.prev_tool_radius);
            let lines = crate::zigzag::zigzag_lines(
                polygon,
                params.tool_radius,
                params.stepover,
                params.angle,
            );
            if lines.is_empty() {
                return Vec::new();
            }
            if large_reachable.is_empty() {
                return lines.iter().map(|l| vec![l[0], l[1]]).collect();
            }
            let sample_step = params.tool_radius.clamp(0.25, 0.5);
            let mut segments: Vec<Vec<P2>> = Vec::new();
            for line in &lines {
                let dx = line[1].x - line[0].x;
                let dy = line[1].y - line[0].y;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-10 {
                    continue;
                }
                let n_samples = (len / sample_step).ceil() as usize;
                let mut segment_points: Vec<P2> = Vec::new();
                for i in 0..=n_samples {
                    let t = i as f64 / n_samples.max(1) as f64;
                    let p = P2::new(line[0].x + t * dx, line[0].y + t * dy);
                    if !point_in_any_polygon(&p, &large_reachable) {
                        segment_points.push(p);
                    } else if !segment_points.is_empty() {
                        segments.push(std::mem::take(&mut segment_points));
                    }
                }
                if !segment_points.is_empty() {
                    segments.push(segment_points);
                }
            }
            segments
        }

        fn seg_bits(segs: &[Vec<P2>]) -> Vec<Vec<(u64, u64)>> {
            segs.iter()
                .map(|s| s.iter().map(|p| (p.x.to_bits(), p.y.to_bits())).collect())
                .collect()
        }

        // A ring-shaped pocket: the 6 mm large tool clears the middle but
        // misses the corners, so the scan lines break into many segments.
        let mut ring = Polygon2::rectangle(-40.0, -40.0, 40.0, 40.0);
        ring.holes.push(vec![
            P2::new(-8.0, -8.0),
            P2::new(-8.0, 8.0),
            P2::new(8.0, 8.0),
            P2::new(8.0, -8.0),
        ]);

        for (label, poly, params) in [
            ("square", square_polygon(40.0), default_params()),
            ("ring", ring, default_params()),
            ("angled", square_polygon(60.0), RestParams {
                angle: 37.0,
                stepover: 0.7,
                tool_radius: 0.8,
                ..default_params()
            }),
            // The "large tool cannot fit at all" fallback branch.
            ("fallback", square_polygon(8.0), default_params()),
        ] {
            let got = rest_segments(&poly, &params);
            let want = serial_segments(&poly, &params);
            assert!(!want.is_empty(), "{label}: vacuous fixture");
            assert_eq!(
                seg_bits(&got),
                seg_bits(&want),
                "{label}: parallel scan lines diverged from the serial walk"
            );
        }
    }

    // ── Basic behavior tests ────────────────────────────────────────────

    #[test]
    fn test_same_tool_size_empty() {
        let sq = square_polygon(40.0);
        let params = RestParams {
            prev_tool_radius: 3.0,
            tool_radius: 3.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.is_empty(),
            "Same tool size should produce no rest passes"
        );
    }

    #[test]
    fn test_larger_new_tool_empty() {
        let sq = square_polygon(40.0);
        let params = RestParams {
            prev_tool_radius: 3.0,
            tool_radius: 5.0, // bigger than prev
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.is_empty(),
            "Larger new tool should produce no rest passes"
        );
    }

    #[test]
    fn test_large_tool_cant_fit_full_zigzag() {
        // 8mm square, 6mm radius tool can't fit (need at least 12mm)
        let sq = square_polygon(8.0);
        let params = RestParams {
            prev_tool_radius: 6.0,
            tool_radius: 1.5,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);
        assert!(
            tp.moves.len() > 5,
            "When large tool can't fit, small tool should do full zigzag, got {} moves",
            tp.moves.len()
        );
    }

    // ── Rest region detection ───────────────────────────────────────────

    #[test]
    fn test_rest_regions_at_corners() {
        // 30mm square with 6mm prev tool radius.
        // Large tool (radius 6) leaves material in corners. Small tool (radius 1.5)
        // should generate passes in those corner regions.
        let sq = square_polygon(30.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 5,
            "Should generate rest passes in corners, got {} moves",
            tp.moves.len()
        );

        // Rest passes should have significant cutting distance
        assert!(
            tp.total_cutting_distance() > 5.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_rest_in_wide_open_area_minimal() {
        // Very large polygon (200mm) — almost all is reachable by large tool.
        // Only the corners have rest material. Rest passes should be small
        // relative to the polygon size.
        let sq = square_polygon(200.0);
        let params = default_params();

        let tp = rest_machining_toolpath(&sq, &params);

        // For comparison, generate a full zigzag
        let full_tp = crate::zigzag::zigzag_toolpath(
            &sq,
            &crate::zigzag::ZigzagParams {
                tool_radius: params.tool_radius,
                stepover: params.stepover,
                cut_depth: params.cut_depth,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                angle: 0.0,
            },
        );

        // Rest passes should be much less than full zigzag.
        // The perimeter strip between tool offsets (4.5mm wide all around)
        // accounts for ~25% of scan line crossings on a 200mm square.
        let rest_dist = tp.total_cutting_distance();
        let full_dist = full_tp.total_cutting_distance();
        assert!(
            rest_dist < full_dist * 0.3,
            "Rest cutting ({:.0}mm) should be <30% of full zigzag ({:.0}mm)",
            rest_dist,
            full_dist
        );
    }

    #[test]
    fn test_narrow_channel_all_rest() {
        // 4mm wide channel (rectangle 4×40). 6mm radius tool can't fit at all.
        // Small tool (1.5mm radius) fits. All of it should be rest region.
        let channel = Polygon2::rectangle(-2.0, -20.0, 2.0, 20.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&channel, &params);

        assert!(
            tp.moves.len() > 5,
            "Narrow channel should be fully rest-machined, got {} moves",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 30.0,
            "Should cut most of the channel length, got {:.1}mm",
            tp.total_cutting_distance()
        );
    }

    // ── Toolpath structure tests ────────────────────────────────────────

    #[test]
    fn test_rest_z_correct() {
        let sq = square_polygon(30.0);
        let params = RestParams {
            cut_depth: -5.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);

        // All feed moves should be at cut_depth or safe_z
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    (m.target.z - (-5.0)).abs() < 0.01
                        || (m.target.z - params.safe_z).abs() < 0.01
                        || m.target.z > -5.0, // plunge moves
                    "Feed Z should be at cut_depth or safe_z, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn test_rest_safe_z_retract() {
        let sq = square_polygon(30.0);
        let tp = rest_machining_toolpath(&sq, &default_params());

        // Every rapid should be at safe_z
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Rapid = m.move_type {
                assert!(
                    (m.target.z - 10.0).abs() < 0.01,
                    "Rapids should be at safe_z=10, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Polygon with holes ──────────────────────────────────────────────

    #[test]
    fn test_rest_with_island() {
        // 40mm square with 10mm square hole in center
        let hole = vec![
            P2::new(-5.0, -5.0),
            P2::new(-5.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, -5.0),
        ];
        let poly = Polygon2::with_holes(square_polygon(40.0).exterior, vec![hole]);
        let params = default_params();
        let tp = rest_machining_toolpath(&poly, &params);

        // Should generate rest passes around the island corners
        assert!(
            tp.moves.len() > 5,
            "Should have rest passes near island corners, got {} moves",
            tp.moves.len()
        );
    }

    // ── Geometry correctness ────────────────────────────────────────────

    #[test]
    fn test_rest_stays_in_polygon() {
        let sq = square_polygon(30.0);
        let params = default_params();
        let tp = rest_machining_toolpath(&sq, &params);

        // All XY positions should be inside the polygon (within tool radius of boundary)
        let expanded = offset_polygon(&sq, -params.tool_radius - 0.5);
        for m in &tp.moves {
            if m.target.z < params.safe_z - 1.0 {
                // Cutting move — should be within the small tool offset
                // (zigzag_lines already handles this, but verify)
                let p = P2::new(m.target.x, m.target.y);
                assert!(
                    sq.contains_point(&p) || point_in_any_polygon(&p, &expanded),
                    "Cut at ({:.2}, {:.2}) should be inside polygon",
                    m.target.x,
                    m.target.y
                );
            }
        }
    }

    #[test]
    fn test_point_in_any_polygon_helper() {
        let polys = vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
            Polygon2::rectangle(20.0, 0.0, 30.0, 10.0),
        ];

        assert!(point_in_any_polygon(&P2::new(5.0, 5.0), &polys));
        assert!(point_in_any_polygon(&P2::new(25.0, 5.0), &polys));
        assert!(!point_in_any_polygon(&P2::new(15.0, 5.0), &polys));
    }

    // ── Scan angle ──────────────────────────────────────────────────────

    #[test]
    fn test_rest_angled_scan() {
        let sq = square_polygon(30.0);
        let params = RestParams {
            angle: 45.0,
            ..default_params()
        };
        let tp = rest_machining_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 3,
            "Angled scan should still produce rest passes, got {} moves",
            tp.moves.len()
        );
    }
}
