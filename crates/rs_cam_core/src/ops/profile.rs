//! Profile cutting operation with tool radius compensation.
//!
//! Cuts along a polygon boundary (inside or outside) with the tool edge
//! following the design contour. A single pass at each depth level.

use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::toolpath::{ClosedPassMoves, MoveIntent, RingMaterial, SidedToolpath};

/// Which side of the boundary the tool cuts on.
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
pub enum ProfileSide {
    /// Tool outside the boundary (cutting out a part).
    Outside,
    /// Tool inside the boundary (cutting an internal contour/hole).
    Inside,
    /// Tool centre on the line (P6.1, P6.3): the cut has no side, so both
    /// sides are material. The op reads its items as lines
    /// (`ItemUse::Curves`): every closed ring is cut on the line (the
    /// exterior and each nested ring, as Trace compensation None), in the
    /// direction of [`profile_ring`] (climb reverses it, L-1). An open path
    /// is cut on the line from its first point to its last point, the
    /// drawn direction; climb does not act on it (L-1), and it takes no
    /// tab (L-8) and no lead. It asks the controller for no compensation
    /// (G40), and `InControl` compensation refuses at every door
    /// (`compute::domain`, L-5).
    On,
}

impl ProfileSide {
    /// Where the material is beside the tool-centre ring of this side
    /// (P6.0c): `Outside` cuts round a part, so the material is inside the
    /// ring; `Inside` cuts a hole, so the material is outside it. The ring
    /// is the closed offset of the region, so the statement holds for
    /// either cut direction (climb or conventional). `On` cuts on the
    /// line, so the material is on both sides (as Trace compensation
    /// None).
    #[must_use]
    pub const fn ring_material(self) -> RingMaterial {
        match self {
            Self::Outside => RingMaterial::Inside,
            Self::Inside => RingMaterial::Outside,
            Self::On => RingMaterial::BothSides,
        }
    }
}

/// Parameters for profile cutting.
///
/// `Copy` since the G2 hoist (2026-08-20) — see [`crate::ops::pocket::PocketParams`]
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

/// The **Z-independent half** of a profile pass (the Layered Profile
/// adapter's `prepare`): the tool-centre path this profile will cut, plus
/// Checkpoint C's offset failure count.
///
/// Split out for the G2 hoist (2026-08-20). A depth-stepped profile used to
/// re-run the compensation offset once per Z level to produce the same
/// polyline every time; `cut_depth` reaches the output only through
/// [`profile_path_to_toolpath`]'s stamp (`emit_level`).
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
        ProfileSide::On => {
            // On the line: the contour is the ring itself. No offset is
            // made, so there is nothing to count. This is a CLOSED contour:
            // an open path gives none here (P6.3), because the closed
            // emitter would close it with a chord. An open path goes
            // through [`on_line_contours`] and [`profile_path_open_to_toolpath`].
            if !polygon.closed {
                return (None, 0);
            }
            let pts = polygon.exterior.clone();
            return (if pts.len() < 3 { None } else { Some(pts) }, 0);
        }
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

/// One line of a Profile with side `On` (P6.3): a closed ring or an open
/// path, with its points in the drawn order (a closed ring in the winding
/// of the selection view).
#[derive(Debug, Clone, PartialEq)]
pub enum OnLineContour {
    /// A closed ring of at least 3 points. The pass cuts it in the order of
    /// [`profile_ring`] and closes it.
    Closed(Vec<P2>),
    /// An open path of at least 2 points. The pass cuts it from its first
    /// point to its last point and does not close it.
    Open(Vec<P2>),
}

/// The lines a Profile with side `On` cuts in `polygon` (P6.3): every ring
/// of a closed polygon (the exterior, then each nested ring), or the open
/// path itself. The open/closed choice reads `polygon.closed` before any
/// point count, so an open path never reaches the closed emitter. A ring
/// below 3 points and a path below 2 points give no line.
#[must_use]
pub fn on_line_contours(polygon: &Polygon2) -> Vec<OnLineContour> {
    if !polygon.closed {
        return if polygon.exterior.len() >= 2 {
            vec![OnLineContour::Open(polygon.exterior.clone())]
        } else {
            Vec::new()
        };
    }
    std::iter::once(&polygon.exterior)
        .chain(&polygon.holes)
        .filter(|ring| ring.len() >= 3)
        .map(|ring| OnLineContour::Closed(ring.clone()))
        .collect()
}

/// One pass along the open path `path` at `params.cut_depth` (P6.3, side
/// `On`): rapid over the first point, plunge, feed through the points to
/// the last point, retract. No move goes back to the first point, and the
/// pass states no closed side, so no lead goes on it. `params.climb` is
/// not read: an open path keeps its drawn direction (L-1). A path below 2
/// points emits nothing.
#[must_use]
pub fn profile_path_open_to_toolpath(path: &[P2], params: &ProfileParams) -> SidedToolpath {
    let mut tp = SidedToolpath::new();
    if path.len() < 2 {
        return tp;
    }
    let points: Vec<P3> = path
        .iter()
        .map(|p| P3::new(p.x, p.y, params.cut_depth))
        .collect();
    tp.toolpath.emit_path_segment_with_intent(
        &points,
        params.safe_z,
        params.feed_rate,
        params.plunge_rate,
        MoveIntent::FinishingCut,
    );
    tp
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
///
/// `material` (P6.0c): where the material is beside the ring. Profile
/// states [`ProfileSide::ring_material`]; Chamfer states it per ring.
pub fn profile_path_to_toolpath(
    contour: &[P2],
    params: &ProfileParams,
    material: RingMaterial,
) -> SidedToolpath {
    profile_ring_to_toolpath(&profile_ring(contour, params.climb), params, material)
}

/// The closed ring a profile pass cuts, in cut order: the contour, reversed
/// for climb milling. The pass plunges at its first point and closes back
/// to it. The tab placer (P5) measures along this ring, so a tab position
/// lands where [`crate::dressup::apply_tabs`] measures it.
#[must_use]
pub fn profile_ring(contour: &[P2], climb: bool) -> Vec<P2> {
    if climb {
        contour.iter().rev().copied().collect()
    } else {
        contour.to_vec()
    }
}

/// One pass along `ring` (already in cut order, see [`profile_ring`]) at
/// `params.cut_depth`, with the stated `material` side (P6.0c).
pub fn profile_ring_to_toolpath(
    ring: &[P2],
    params: &ProfileParams,
    material: RingMaterial,
) -> SidedToolpath {
    let mut tp = SidedToolpath::new();
    let points: Vec<P3> = ring
        .iter()
        .map(|p| P3::new(p.x, p.y, params.cut_depth))
        .collect();
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
    tp
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::toolpath::{MoveType, Toolpath};

    /// One profile level through the kernels the Layered Profile adapter
    /// calls: `profile_path_reported` (`prepare`), then
    /// `profile_path_to_toolpath` (`emit_level`). A collapsed offset gives
    /// no contour and an empty toolpath. D3 deleted the public wrapper.
    fn profile_level(polygon: &Polygon2, params: &ProfileParams) -> Toolpath {
        match profile_path_reported(polygon, params).0 {
            Some(contour) => {
                profile_path_to_toolpath(&contour, params, params.side.ring_material()).toolpath
            }
            None => Toolpath::new(),
        }
    }

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

        let tp = profile_level(&tiny, &default_params(ProfileSide::Inside));
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn test_profile_toolpath_structure() {
        let sq = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let params = default_params(ProfileSide::Outside);
        let tp = profile_level(&sq, &params);

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
        let tp = profile_level(&sq, &params);

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

        let conv_tp = profile_level(&sq, &default_params(ProfileSide::Outside));
        let mut climb_params = default_params(ProfileSide::Outside);
        climb_params.climb = true;
        let climb_tp = profile_level(&sq, &climb_params);

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

        let outside = profile_level(&l_shape, &default_params(ProfileSide::Outside));
        assert!(
            !outside.moves.is_empty(),
            "Outside profile of L-shape should work"
        );

        let inside = profile_level(
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
