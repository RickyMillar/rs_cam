use std::f64::consts::{PI, TAU};

use crate::geo::P2;

/// Compute the target engagement fraction from stepover and tool radius.
///
/// `pub` (not `pub(crate)`): the viz crate's "Optimal load" slider uses
/// this and its inverse [`radial_woc_fraction_from_leading_arc`] to map
/// between operator-facing leading-arc load and raw stepover.
pub fn target_engagement_fraction(stepover: f64, tool_radius: f64) -> f64 {
    let woc = stepover.min(2.0 * tool_radius);
    let alpha = (1.0 - woc / tool_radius).clamp(-1.0, 1.0).acos();
    alpha / TAU
}

/// Convert a leading-arc engagement fraction (`α / 2π`, the quantity the
/// adaptive planner predicts and [`target_engagement_fraction`] emits)
/// into the radial width-of-cut fraction (`a_e / D`) the feed
/// modulator's chip-thinning model consumes.
///
/// Inverse of [`target_engagement_fraction`]: that maps stepover → `α/2π`
/// via `α = acos(1 − a_e/R)`; this maps the fraction back to
/// `a_e/D = a_e/(2R) = (1 − cos(2π·f)) / 2`. The result is exactly the
/// `radial_woc_fraction` field of
/// [`crate::dressup::feed_modulation::PerMoveEngagement`] — `0.0` = air,
/// `0.5` = half-immersion, `1.0` = full slot.
///
/// `f_arc` is clamped to `[0, 0.5]` (0.5 = full-slot leading arc, π
/// radians of contact); values outside that band are non-physical for a
/// leading-semicircle measure.
///
/// Stage 4 (planner-predicted engagement → feed modulation) wires this
/// into `session::compute::apply_adaptive_feed_modulation`; see
/// `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` §"Stage 4
/// implementation spec".
pub fn radial_woc_fraction_from_leading_arc(f_arc: f64) -> f64 {
    let f = f_arc.clamp(0.0, 0.5);
    (1.0 - (TAU * f).cos()) / 2.0
}

/// Average a buffer of angles, handling wraparound correctly.
pub(crate) fn average_angles(angles: &[f64]) -> f64 {
    let mut sx = 0.0;
    let mut sy = 0.0;
    for &angle in angles {
        sx += angle.cos();
        sy += angle.sin();
    }
    sy.atan2(sx)
}

/// Normalize an angle difference to [-π, π].
pub(crate) fn angle_diff(a: f64, b: f64) -> f64 {
    let mut delta = a - b;
    while delta > PI {
        delta -= TAU;
    }
    while delta < -PI {
        delta += TAU;
    }
    delta
}

/// Refine an angle bracket around a target engagement using interpolation.
///
/// The tuple shape is `(angle, engagement, payload)`, where `payload` can carry
/// caller-specific data such as score, z-height, or both.
pub(crate) fn refine_angle_bracket<T: Clone, F>(
    mut lo: (f64, f64, T),
    mut hi: (f64, f64, T),
    target_frac: f64,
    iterations: usize,
    mut eval: F,
) -> Option<(f64, f64, T)>
where
    F: FnMut(f64) -> Option<(f64, f64, T)>,
{
    let mut best = None;
    for _ in 0..iterations {
        let delta = hi.1 - lo.1;
        if delta.abs() <= 0.001 {
            break;
        }
        let t = ((target_frac - lo.1) / delta).clamp(0.0, 1.0);
        let angle = lo.0 + t * angle_diff(hi.0, lo.0);
        let candidate = eval(angle)?;
        if candidate.1 < target_frac {
            lo = candidate.clone();
        } else {
            hi = candidate.clone();
        }
        best = Some(candidate);
    }
    best
}

/// A move in a blended-corner path: either a straight line or a circular arc.
#[derive(Debug, Clone)]
pub(crate) enum BlendedMove {
    /// Linear move to a 2D point.
    Linear(P2),
    /// Circular arc to `end` around `center`. `clockwise` selects G2 vs G3.
    Arc {
        end: P2,
        center: P2,
        clockwise: bool,
    },
}

/// Blend sharp corners into arc moves, preserving arc geometry for G2/G3 emission.
///
/// Same corner detection as [`blend_corners`] but returns [`BlendedMove`] descriptors
/// so the caller can emit native arc commands instead of linearized segments.
// SAFETY: same indexing bounds as blend_corners
#[allow(clippy::indexing_slicing, clippy::expect_used)]
pub(crate) fn blend_corners_to_moves(path: &[P2], min_radius: f64) -> Vec<BlendedMove> {
    if min_radius <= 0.0 || path.len() < 3 {
        return path.iter().map(|&p| BlendedMove::Linear(p)).collect();
    }

    #[allow(clippy::indexing_slicing)]
    let mut result = vec![BlendedMove::Linear(path[0])];

    #[allow(clippy::indexing_slicing)]
    for i in 1..path.len() - 1 {
        let a = path[i - 1];
        let b = path[i];
        let c = path[i + 1];

        let ba_x = a.x - b.x;
        let ba_y = a.y - b.y;
        let bc_x = c.x - b.x;
        let bc_y = c.y - b.y;
        let ba_len = (ba_x * ba_x + ba_y * ba_y).sqrt();
        let bc_len = (bc_x * bc_x + bc_y * bc_y).sqrt();

        if ba_len < 1e-10 || bc_len < 1e-10 {
            result.push(BlendedMove::Linear(b));
            continue;
        }

        let cos_full = (ba_x * bc_x + ba_y * bc_y) / (ba_len * bc_len);
        let cos_full = cos_full.clamp(-1.0, 1.0);
        let full_angle = cos_full.acos();
        let half = full_angle / 2.0;

        if full_angle > 170.0_f64.to_radians() || half < 0.02 {
            result.push(BlendedMove::Linear(b));
            continue;
        }

        let setback = min_radius / half.tan();
        if setback > ba_len * 0.4 || setback > bc_len * 0.4 {
            result.push(BlendedMove::Linear(b));
            continue;
        }

        let t1 = P2::new(b.x + ba_x / ba_len * setback, b.y + ba_y / ba_len * setback);
        let t2 = P2::new(b.x + bc_x / bc_len * setback, b.y + bc_y / bc_len * setback);

        let bis_x = ba_x / ba_len + bc_x / bc_len;
        let bis_y = ba_y / ba_len + bc_y / bc_len;
        let bis_len = (bis_x * bis_x + bis_y * bis_y).sqrt();
        if bis_len < 1e-10 {
            result.push(BlendedMove::Linear(b));
            continue;
        }
        let center_dist = min_radius / half.sin();
        let arc_cx = b.x + bis_x / bis_len * center_dist;
        let arc_cy = b.y + bis_y / bis_len * center_dist;

        let a1 = (t1.y - arc_cy).atan2(t1.x - arc_cx);
        let a2 = (t2.y - arc_cy).atan2(t2.x - arc_cx);

        let mut sweep = a2 - a1;
        if sweep > PI {
            sweep -= TAU;
        }
        if sweep < -PI {
            sweep += TAU;
        }

        // Linear to the arc tangent point, then arc to the exit tangent
        result.push(BlendedMove::Linear(t1));
        result.push(BlendedMove::Arc {
            end: t2,
            center: P2::new(arc_cx, arc_cy),
            clockwise: sweep < 0.0,
        });
    }

    result.push(BlendedMove::Linear(
        *path.last().expect("path has at least 3 elements"),
    ));
    result
}

/// Blend sharp corners in a path with arcs of at least `min_radius`.
///
/// Returns a linearized point sequence (used by 3D paths where Z varies per
/// point). For 2D paths that need native G2/G3 arcs, use
/// [`blend_corners_to_moves`] instead.
///
/// **One corner walk (CUT-01).** The corner test, the reject thresholds, the
/// bisector and the sweep wrap lived here a second time until 2026-09-17, so a
/// fix to a threshold could land in one arm only. This function is now the
/// linearisation of [`blend_corners_to_moves`] and holds no corner geometry of
/// its own.
pub(crate) fn blend_corners(path: &[P2], min_radius: f64) -> Vec<P2> {
    linearise_blended_moves(&blend_corners_to_moves(path, min_radius), min_radius)
}

/// Replace every arc in a blended-corner walk with points on that arc.
///
/// The point count is `ceil(|sweep| / 10°)` clamped to `2..=20`, and the
/// points are spaced evenly in angle from the arc's entry tangent to its exit
/// tangent. The entry tangent is the move before the arc, so the sweep is
/// recovered from the two tangent points and the centre rather than carried:
/// `BlendedMove::clockwise` is the sign of that same sweep.
fn linearise_blended_moves(moves: &[BlendedMove], min_radius: f64) -> Vec<P2> {
    let mut result: Vec<P2> = Vec::with_capacity(moves.len());
    for m in moves {
        match *m {
            BlendedMove::Linear(p) => result.push(p),
            BlendedMove::Arc { end, center, .. } => {
                // SAFETY: `blend_corners_to_moves` pushes the entry tangent as
                // a `Linear` immediately before every `Arc`, and its first
                // move is always `Linear`, so `result` is never empty here.
                let start = result.last().copied().unwrap_or(end);
                let a1 = (start.y - center.y).atan2(start.x - center.x);
                let a2 = (end.y - center.y).atan2(end.x - center.x);

                let mut sweep = a2 - a1;
                if sweep > PI {
                    sweep -= TAU;
                }
                if sweep < -PI {
                    sweep += TAU;
                }

                // The upper clamp is unreachable: the wrap above puts
                // `sweep` in `[-PI, PI]`, so `ceil(|sweep| / 10 degrees)` is
                // at most 18. It is kept because the pre-CUT-01 body had it.
                let n_pts = ((sweep.abs() / (PI / 18.0)).ceil() as usize).clamp(2, 20);
                for j in 1..n_pts {
                    let t = j as f64 / n_pts as f64;
                    let angle = a1 + sweep * t;
                    result.push(P2::new(
                        center.x + min_radius * angle.cos(),
                        center.y + min_radius * angle.sin(),
                    ));
                }
                result.push(end);
            }
        }
    }
    result
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// CUT-01 — `blend_corners` is the linearisation of
    /// `blend_corners_to_moves`, and holds no corner geometry of its own.
    ///
    /// The two functions were the same 90 lines twice until 2026-09-17: the
    /// same corner test, the same 170° and `half < 0.02` rejects, the same
    /// `setback > len * 0.4` reject, the same bisector and sweep wrap. Only
    /// the tail differed. A fix to a threshold could land in one arm only.
    ///
    /// This test does NOT compare the production function with itself. It
    /// walks `blend_corners_to_moves` and linearises each arc HERE, from the
    /// rule the doc states: `ceil(|sweep| / 10°)` points clamped to `2..=20`,
    /// spaced evenly in angle. So the reference is the contract restated, and
    /// a change to the production clamp or the spacing fails.
    ///
    /// NOT MEASURED: whether a corner SHOULD be blended. That is
    /// `blend_corners_to_moves`'s decision and both arms now share it by
    /// construction.
    #[test]
    fn blend_corners_is_the_linearisation_of_blended_moves() {
        /// The linearisation rule, restated independently of the production
        /// helper.
        fn reference(moves: &[BlendedMove], radius: f64) -> Vec<P2> {
            let mut out: Vec<P2> = Vec::new();
            let mut at = P2::new(0.0, 0.0);
            for m in moves {
                match *m {
                    BlendedMove::Linear(p) => {
                        out.push(p);
                        at = p;
                    }
                    BlendedMove::Arc { end, center, .. } => {
                        let a1 = (at.y - center.y).atan2(at.x - center.x);
                        let a2 = (end.y - center.y).atan2(end.x - center.x);
                        let mut sweep = a2 - a1;
                        if sweep > PI {
                            sweep -= TAU;
                        }
                        if sweep < -PI {
                            sweep += TAU;
                        }
                        let n_pts = ((sweep.abs() / (PI / 18.0)).ceil() as usize).clamp(2, 20);
                        for j in 1..n_pts {
                            let t = j as f64 / n_pts as f64;
                            let angle = a1 + sweep * t;
                            out.push(P2::new(
                                center.x + radius * angle.cos(),
                                center.y + radius * angle.sin(),
                            ));
                        }
                        out.push(end);
                        at = end;
                    }
                }
            }
            out
        }

        // A deterministic xorshift over a wide corner set: sharp, shallow,
        // reversing and degenerate corners all appear.
        let mut seed = 0x2026_0917_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };

        let mut blended = 0usize;
        for case in 0..2000 {
            let n = 3 + (case % 9);
            let path: Vec<P2> = (0..n)
                .map(|_| P2::new(next() * 120.0 - 60.0, next() * 120.0 - 60.0))
                .collect();
            for &radius in &[0.0_f64, 0.05, 0.5, 2.0, 7.5, 40.0] {
                let walked = reference(&blend_corners_to_moves(&path, radius), radius);
                let linearised = blend_corners(&path, radius);
                assert_eq!(
                    walked.len(),
                    linearised.len(),
                    "case {case} radius={radius} path={path:?}"
                );
                for (want, got) in walked.iter().zip(linearised.iter()) {
                    assert_eq!(
                        want.x.to_bits(),
                        got.x.to_bits(),
                        "case {case} radius={radius}: x differs"
                    );
                    assert_eq!(
                        want.y.to_bits(),
                        got.y.to_bits(),
                        "case {case} radius={radius}: y differs"
                    );
                }
                if linearised.len() > path.len() {
                    blended += 1;
                }
            }
        }
        assert!(
            blended >= 500,
            "only {blended} of the cases blended a corner; the set is too tame to \
             prove anything"
        );
    }

    /// The leading-arc → radial-WOC bridge must invert
    /// `target_engagement_fraction` across the physical stepover range,
    /// recovering the originating `a_e / D` fraction exactly.
    #[test]
    fn leading_arc_round_trips_target_engagement_fraction() {
        let radius = 3.0_f64;
        for &woc in &[0.3_f64, 0.6, 1.5, 3.0, 4.5, 6.0] {
            let f_arc = target_engagement_fraction(woc, radius);
            let recovered = radial_woc_fraction_from_leading_arc(f_arc);
            let expected = woc.min(2.0 * radius) / (2.0 * radius);
            assert!(
                (recovered - expected).abs() < 1e-9,
                "woc={woc}: f_arc={f_arc} recovered={recovered} expected={expected}"
            );
        }
    }

    /// Closed-form anchors: air, half-immersion, full slot.
    #[test]
    fn leading_arc_known_points() {
        assert!(radial_woc_fraction_from_leading_arc(0.0).abs() < 1e-12);
        assert!((radial_woc_fraction_from_leading_arc(0.25) - 0.5).abs() < 1e-12);
        assert!((radial_woc_fraction_from_leading_arc(0.5) - 1.0).abs() < 1e-12);
    }

    /// Out-of-band inputs clamp to the physical [0, 1] WOC range.
    #[test]
    fn leading_arc_clamps_out_of_range() {
        assert!((radial_woc_fraction_from_leading_arc(0.9) - 1.0).abs() < 1e-12);
        assert!(radial_woc_fraction_from_leading_arc(-0.2).abs() < 1e-12);
    }
}
