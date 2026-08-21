//! Profile cutting operation with tool radius compensation.
//!
//! Cuts along a polygon boundary (inside or outside) with the tool edge
//! following the design contour. A single pass at each depth level.

use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::{MoveIntent, Toolpath};

/// Which side of the boundary the tool cuts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSide {
    /// Tool outside the boundary (cutting out a part).
    Outside,
    /// Tool inside the boundary (cutting an internal contour/hole).
    Inside,
}

/// Parameters for profile cutting.
///
/// `Copy` since the G2 hoist (2026-08-20) — see [`crate::pocket::PocketParams`]
/// for why a depth-stepping caller wants struct-update rather than a re-listed
/// literal per Z level.
#[derive(Debug, Clone, Copy)]
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
    /// When true, the controller handles tool radius compensation (G41/G42),
    /// so the toolpath follows the exact boundary geometry with no software offset.
    pub compensate_in_controller: bool,
}

/// Generate a profile cutting toolpath along the polygon boundary.
///
/// The tool is offset by `tool_radius` to the specified side so the tool
/// edge follows the design boundary exactly.
///
/// Returns an empty toolpath if the offset collapses (e.g., inside profile
/// on a polygon smaller than the tool diameter).
#[tracing::instrument(skip(polygon, params), fields(
    tool_radius = params.tool_radius,
    side = ?params.side,
))]
pub fn profile_toolpath(polygon: &Polygon2, params: &ProfileParams) -> Toolpath {
    profile_toolpath_reported(polygon, params).0
}

/// [`profile_toolpath`] with Checkpoint C's offset failure channel attached.
///
/// The second element counts offset CALLS that failed rather than collapsed
/// (see [`crate::polygon::OffsetFailure`]). `0` is a measurement — every
/// offset was clean — and the caller may record it as such; there is no
/// "not measured" state here because this function always makes the call.
///
/// It matters for profile specifically because a failed offset and a
/// collapsed one produce the same empty toolpath, and for an OUTSIDE profile
/// a collapse is nearly impossible: an outward offset of a valid ring has no
/// geometric reason to vanish, so an empty result there is almost always the
/// failure this channel names.
#[must_use]
pub fn profile_toolpath_reported(polygon: &Polygon2, params: &ProfileParams) -> (Toolpath, usize) {
    let (contour, failures) = profile_path_reported(polygon, params);
    let tp = match contour {
        Some(pts) => profile_path_to_toolpath(&pts, params),
        None => Toolpath::new(),
    };
    (tp, failures)
}

/// The **Z-independent half** of [`profile_toolpath_reported`]: the tool-centre
/// path this profile will cut, plus Checkpoint C's offset failure count.
///
/// Split out for the G2 hoist (2026-08-20). A depth-stepped profile used to
/// re-run the compensation offset once per Z level to produce the same
/// polyline every time; `cut_depth` reaches the output only through
/// [`profile_path_to_toolpath`]'s stamp. `profile_toolpath_reported` is now
/// literally this function composed with that one, so a hoisted caller and a
/// per-level caller cannot drift apart.
///
/// Handles both compensation modes: `compensate_in_controller` returns the
/// exterior verbatim (no offset is made, so the count is a measured `0`), and
/// the software-compensation branch delegates to [`profile_contour_reported`].
#[must_use]
pub fn profile_path_reported(
    polygon: &Polygon2,
    params: &ProfileParams,
) -> (Option<Vec<P2>>, usize) {
    if params.compensate_in_controller {
        // Controller handles the offset — toolpath follows the exact boundary.
        let pts = polygon.exterior.clone();
        if pts.len() < 3 {
            return (None, 0);
        }
        // No offset is made on this branch, so there is nothing to count.
        (Some(pts), 0)
    } else {
        profile_contour_reported(polygon, params.tool_radius, params.side)
    }
}

/// Generate the 2D profile contour (tool center path).
///
/// Returns None if the offset collapses.
pub fn profile_contour(polygon: &Polygon2, tool_radius: f64, side: ProfileSide) -> Option<Vec<P2>> {
    profile_contour_reported(polygon, tool_radius, side).0
}

/// [`profile_contour`] with Checkpoint C's offset failure channel attached:
/// `1` when the single offset this makes failed rather than collapsed, `0`
/// otherwise.
///
/// # Precondition: `polygon` must be wound CCW
///
/// The sign below is the whole of the inside/outside decision, and cavalier's
/// offset sign is relative to the direction of travel (positive = to the left
/// of the segment tangent), not to the enclosed area. On a CW ring the two
/// arms therefore mean the opposite of what they say. Every importer
/// normalises to CCW (`Polygon2::ensure_winding`), so the precondition holds
/// in production — but it held only by luck until G-PROFILE-FLIP, when a
/// `face_up = Bottom` setup mirrored the polygon on its way into the setup
/// frame and turned an Outside profile into an Inside one. The normalisation
/// now lives at that mirror (`SetupTransformInfo::apply_to_polygons`); this
/// note is here so the dependency is written down at the place that has it.
#[must_use]
pub fn profile_contour_reported(
    polygon: &Polygon2,
    tool_radius: f64,
    side: ProfileSide,
) -> (Option<Vec<P2>>, usize) {
    let distance = match side {
        ProfileSide::Inside => tool_radius,   // inward (positive)
        ProfileSide::Outside => -tool_radius, // outward (negative)
    };

    let (results, failure) = crate::polygon::offset_polygon_reported(polygon, distance);

    // Take the first (largest) result contour
    let contour = results
        .into_iter()
        .next()
        .filter(|p| p.exterior.len() >= 3)
        .map(|p| p.exterior);
    (contour, usize::from(failure.is_some()))
}

/// S.7 (planning/finishing_stack_review_2026-07.md): emits via the shared
/// `emit_closed_contour_with_intent` rapid→plunge→feed→close→retract
/// envelope. Byte-identical to the previous hand-rolled sequence: both
/// callers of this function guarantee `contour.len() >= 3` before invoking
/// it (`profile_contour` filters `exterior.len() >= 3`; the
/// `compensate_in_controller` path checks `pts.len() < 3` and bails early),
/// so the shared emitter's `< 3` no-op guard is never exercised differently
/// than the old `is_empty()` guard was.
///
/// `pub` since the G2 hoist (2026-08-20): this is the Z-dependent half, and
/// `params.cut_depth` is the only thing a depth-stepped caller varies across
/// levels. See [`profile_path_reported`] for the other half.
pub fn profile_path_to_toolpath(contour: &[P2], params: &ProfileParams) -> Toolpath {
    let mut tp = Toolpath::new();

    if contour.is_empty() {
        return tp;
    }

    // Optionally reverse for climb milling.
    let ordered: Vec<&P2> = if params.climb {
        contour.iter().rev().collect()
    } else {
        contour.iter().collect()
    };
    let points: Vec<P3> = ordered
        .iter()
        .map(|p| P3::new(p.x, p.y, params.cut_depth))
        .collect();

    tp.emit_closed_contour_with_intent(
        &points,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
        MoveIntent::FinishingCut,
    );

    tp
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
            compensate_in_controller: false,
        }
    }

    /// **G2, profile side.** The hoisted composition (`profile_path_reported`
    /// once, `profile_path_to_toolpath` per level) against the naive one
    /// (`profile_toolpath` per level), bit for bit — and on BOTH compensation
    /// modes, because `compensate_in_controller` is the branch the split had to
    /// absorb and the one most likely to drift.
    #[test]
    fn the_hoisted_contour_emits_exactly_what_the_per_level_offset_did() {
        let poly = Polygon2::new(
            (0..400)
                .map(|i| {
                    let t = std::f64::consts::TAU * i as f64 / 400.0;
                    let r = 25.0 + 0.7 * (5.0 * t).sin();
                    P2::new(r * t.cos(), r * t.sin())
                })
                .collect(),
        );
        let levels: Vec<f64> = (1..=5).map(|i| -1.5 * i as f64).collect();

        for in_control in [false, true] {
            for side in [ProfileSide::Outside, ProfileSide::Inside] {
                let base = ProfileParams {
                    side,
                    compensate_in_controller: in_control,
                    ..default_params(side)
                };

                let naive = crate::depth::toolpath_at_levels(&levels, base.safe_z, |z| {
                    profile_toolpath(
                        &poly,
                        &ProfileParams {
                            cut_depth: z,
                            ..base
                        },
                    )
                });

                let (contour, _failures) = profile_path_reported(&poly, &base);
                let hoisted =
                    crate::depth::toolpath_at_levels(&levels, base.safe_z, |z| match &contour {
                        Some(pts) => profile_path_to_toolpath(
                            pts,
                            &ProfileParams {
                                cut_depth: z,
                                ..base
                            },
                        ),
                        None => Toolpath::new(),
                    });

                let label = format!("in_control={in_control} side={side:?}");
                assert!(
                    !naive.moves.is_empty(),
                    "{label}: the fixture emitted nothing — vacuous comparison"
                );
                assert_eq!(
                    hoisted.moves.len(),
                    naive.moves.len(),
                    "{label}: move count"
                );
                for (i, (h, n)) in hoisted.moves.iter().zip(naive.moves.iter()).enumerate() {
                    assert_eq!(
                        (
                            h.target.x.to_bits(),
                            h.target.y.to_bits(),
                            h.target.z.to_bits()
                        ),
                        (
                            n.target.x.to_bits(),
                            n.target.y.to_bits(),
                            n.target.z.to_bits()
                        ),
                        "{label}: move {i} diverges"
                    );
                    assert_eq!(h.move_type, n.move_type, "{label}: move {i} type");
                    assert_eq!(h.intent, n.intent, "{label}: move {i} intent");
                }
            }
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
        let pts = contour.expect("asserted Some above");
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
        let pts = contour.expect("asserted Some above");

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
            .expect("toolpath should contain a plunge move");

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
            .expect("conventional toolpath should have a cutting move");
        let climb_first_cut = climb_tp
            .moves
            .iter()
            .find(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            })
            .expect("climb toolpath should have a cutting move");

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
                compensate_in_controller: false,
            },
        );
        assert!(
            !inside.moves.is_empty(),
            "Inside profile of L-shape should work"
        );
    }
}
