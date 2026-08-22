//! G-PROFILE-FLIP — a `face_up = Bottom` setup inverted `ProfileSide`.
//!
//! # What was measured
//!
//! A draft export sentry (`planning/airrun_2026-08-19/RUN_LOG.md`) ran one
//! Outside profile twice — once on the identity setup, once on a
//! `FaceUp::Bottom` setup — and read the emitted X extent as `17..53` on the
//! first and `23..47` on the second. `53 - 47 == 17 - 23 == 6 == 2 · 3 mm`,
//! the tool radius: the flipped run was an **Inside** profile. That was one
//! measurement of absolute coordinates on two different frames, so the RUN_LOG
//! logged it as an unverified finding rather than a defect.
//!
//! This file re-measures it frame-independently. A flip relocates the polygon,
//! so absolute X proves nothing; what is invariant is the **signed clearance**
//! between the emitted tool-centre path and the polygon *as the generator saw
//! it in the same frame*. An Outside profile must be wider than its polygon by
//! exactly the tool radius on every side, an Inside profile narrower by the
//! same. That is the delta [`profile_side_survives_a_face_up_flip`] asserts,
//! for both sides on both setups.
//!
//! The numbers this fixture works in: the 30 mm square reads `-15..15` in the
//! identity setup's world frame and `12..42` in the flipped setup's local one
//! (the mirror about the stock mid-plane, plus the stock-origin shift). With
//! the Ø6 tool, Outside must therefore emit `-18..18` and `9..45`, and Inside
//! `-12..12` and `15..39`. Inverting the offset side swaps those two rows on
//! the flipped setup only — the identity setup is correct either way, which is
//! why nothing caught it.
//!
//! # The cause
//!
//! Not the profile generator. `FaceUp::Bottom` maps `(x, y)` to `(x, D - y)`
//! — a mirror, determinant `-1` — so `SetupTransformInfo::apply_to_polygons`
//! handed the generator a ring wound the opposite way. The crate's
//! exterior-CCW convention is established by the importers
//! (`svg_input`/`dxf_input` both call `ensure_winding`) and nothing between
//! that mirror and the offset re-established it. cavalier's offset sign is
//! defined against the direction of travel, not the enclosed area — positive
//! offsets to the LEFT of the segment tangent — so a reversed ring inverts
//! every offset in the 2.5D stack, and `profile_contour_reported`'s
//! `Inside => +r / Outside => -r` means the opposite of what it says.
//!
//! The fix re-winds closed rings at that mirror, so the invariant has one
//! owner instead of seven consumers. Open paths are left alone — their point
//! order is the machining direction, not a winding — which
//! [`flip_leaves_open_path_direction_alone`] pins, and the winding itself is
//! pinned directly by [`flip_preserves_the_ccw_winding_convention`] so a
//! regression is attributable without generating a toolpath.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{generate, polygon_model, square_polygon, stock_under, toolpath_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{ProfileConfig, ProfileSide};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::MoveType;

/// Half-extent of the part outline: a 30 mm square, world `-15..15` in both
/// axes.
const PART_HALF: f64 = 15.0;

/// Half-extent used to size the stock, giving 10 mm of clear air around the
/// part on every side. An Outside profile's tool centre runs 3 mm outboard of
/// the part, and a stock that only just contained the part would put that path
/// on the stock edge where a boundary clamp could truncate it and the measure
/// would then be reading the clamp, not the offset.
const STOCK_HALF: f64 = 25.0;

/// Deep enough for the profile's default 6 mm depth at 2 mm per pass.
const STOCK_HEIGHT: f64 = 12.0;

/// `make_endmill_6mm` is Ø6, so every offset in this file is 3 mm.
const TOOL_RADIUS: f64 = 3.0;

/// Offsets of an axis-aligned square are exact — the extent is set by the
/// straight edge midpoints, not by the rounded corners — so this only absorbs
/// float noise.
const EPS: f64 = 1e-6;

/// One profile op on the given setup orientation, generated.
///
/// Entry, lead-in/out, arc fitting and segment merging are all switched off.
/// None of them can widen the extent of an axis-aligned square, but each one
/// adds a way for a future default change to move this measurement for a
/// reason that has nothing to do with offset side, and the sentry is only
/// worth having if a failure means what it says.
fn generated_profile(face_up: FaceUp, side: ProfileSide) -> ProjectSession {
    let op = OperationConfig::Profile(ProfileConfig {
        side,
        spindle_rpm: Some(18_000),
        ..ProfileConfig::default()
    });

    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_under(STOCK_HALF, STOCK_HEIGHT));
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model(
        vec![square_polygon(PART_HALF)],
        "profile_square",
    ));

    let mut cfg = toolpath_config("Profile", op, tool_id, model_id);
    cfg.dressups.entry_style = DressupEntryStyle::None;
    cfg.dressups.lead_in_out = false;
    cfg.dressups.arc_fitting = false;
    cfg.dressups.segment_merge = false;

    // `new_empty` already made the identity setup at index 0; a flip needs its
    // own setup, and the toolpath has to be routed onto it.
    let setup = if face_up == FaceUp::Top {
        0
    } else {
        session.add_setup("Flipped".to_owned(), face_up)
    };
    session
        .add_toolpath(setup, cfg)
        .expect("add the profile toolpath to its setup");

    generate(&mut session, 0);
    session
}

/// `[x_min, y_min, x_max, y_max]` of the emitted **cutting** moves.
///
/// Linear only: with arc fitting off every cutting move is one, and the rapids
/// this drops are the safe-Z approach and retract. The plunge is Linear and
/// sits on the contour start, so it neither widens nor narrows the box.
fn cutting_extent(session: &ProjectSession) -> [f64; 4] {
    let tp = session
        .get_result(0)
        .expect("profile produced a compute result")
        .toolpath();
    let pts: Vec<P2> = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect();
    assert!(
        pts.len() >= 4,
        "profile emitted {} cutting moves — nothing to measure",
        pts.len()
    );
    Polygon2::new(pts).bbox()
}

/// The part outline in the frame the generator emitted in: world for the
/// identity setup, setup-local for a flipped one (`SetupEvalContext`'s
/// `local_to_global == None` ⇔ identity rule). This is the half of the
/// comparison that makes it frame-independent — both sides move together.
fn reference_extent(session: &ProjectSession, face_up: FaceUp) -> [f64; 4] {
    let poly = square_polygon(PART_HALF);
    if face_up == FaceUp::Top {
        return poly.bbox();
    }
    session
        .setup_transform_info(face_up, ZRotation::Deg0)
        .apply_to_polygons(&[poly])
        .first()
        .expect("apply_to_polygons returns one polygon per input")
        .bbox()
}

/// Signed outward clearance of the path box from the polygon box, per side.
/// Positive means the tool centre runs outboard of the outline.
fn clearances(path: [f64; 4], reference: [f64; 4]) -> [f64; 4] {
    [
        reference[0] - path[0], // -X
        reference[1] - path[1], // -Y
        path[2] - reference[2], // +X
        path[3] - reference[3], // +Y
    ]
}

/// The sentry. On both setups and both sides, the emitted tool-centre path
/// must sit exactly one tool radius outboard (Outside) or inboard (Inside) of
/// the polygon as the generator saw it.
#[test]
fn profile_side_survives_a_face_up_flip() {
    for face_up in [FaceUp::Top, FaceUp::Bottom] {
        for (side, expected) in [
            (ProfileSide::Outside, TOOL_RADIUS),
            (ProfileSide::Inside, -TOOL_RADIUS),
        ] {
            let session = generated_profile(face_up, side);
            let path = cutting_extent(&session);
            let reference = reference_extent(&session, face_up);
            let got = clearances(path, reference);

            for (axis, actual) in ["-X", "-Y", "+X", "+Y"].iter().zip(got) {
                assert!(
                    (actual - expected).abs() < EPS,
                    "{face_up:?} / {side:?}: {axis} clearance {actual:.4} mm, \
                     expected {expected:.4} mm. Path box {path:?} against the \
                     polygon's own box {reference:?} in the same frame. A \
                     sign flip here is the offset side inverting — see this \
                     file's module doc."
                );
            }
        }
    }
}

/// The cause, pinned without a generator in the way: the setup transform must
/// hand on a ring that still obeys the crate's exterior-CCW / holes-CW
/// convention, because that convention is the entire meaning of an offset's
/// sign. A bare mirror does not, which is what G-PROFILE-FLIP was.
#[test]
fn flip_preserves_the_ccw_winding_convention() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_under(STOCK_HALF, STOCK_HEIGHT));

    let mut with_hole = Polygon2::with_holes(
        square_polygon(PART_HALF).exterior,
        vec![square_polygon(PART_HALF / 3.0).exterior],
    );
    with_hole.ensure_winding();
    assert!(
        with_hole.has_correct_winding(),
        "fixture precondition: the input is the normalised ring an importer \
         would produce"
    );

    for face_up in [FaceUp::Top, FaceUp::Bottom] {
        let out = session
            .setup_transform_info(face_up, ZRotation::Deg0)
            .apply_to_polygons(&[with_hole.clone()]);
        assert!(
            out[0].closed,
            "{face_up:?}: the closed flag must survive the transform in this \
             direction too — an exterior silently turned open would skip the \
             re-wind below entirely"
        );
        assert!(
            out[0].has_correct_winding(),
            "{face_up:?}: apply_to_polygons returned exterior signed area \
             {:.3}, holes {:?} — the offset sign downstream is defined \
             against the direction of travel, so a reversed ring inverts \
             every offset that follows",
            out[0].signed_area(),
            out[0]
                .holes
                .iter()
                .map(|h| Polygon2::new(h.clone()).signed_area())
                .collect::<Vec<_>>()
        );
    }
}

/// The other half of the fix's contract. An open path has no winding to
/// normalise — its point order is the direction the tool travels — so the
/// re-wind must not touch it. Reversing a river would machine it backwards,
/// which is the failure the `closed` flag was already being preserved to
/// avoid (see `apply_to_polygons`' open/closed note).
///
/// # The fixture was vacuous until G-POLYTRANSFORM-DUP (2026-08-22)
///
/// `ensure_winding` scores the ring it is handed, which here is the ring
/// **after** the mirror — and the mirror flips the sign. The original fixture
/// was authored CW and its comment reasoned from that, but the `FaceUp::Bottom`
/// map sent it out at `+400` mm², i.e. CCW, which `ensure_winding` leaves
/// alone. So the test passed either way and pinned nothing: the viz crate's
/// (now deleted) copy of `apply_to_polygons` re-wound open paths
/// unconditionally and no sentry anywhere noticed. The order is now chosen so
/// the *transformed* ring is CW, and [`OPEN_PATH_MIRRORED`] states the answer
/// as literal coordinates rather than re-deriving it from `world_to_local`,
/// which would agree with a reversal that moved every point consistently.
#[test]
fn flip_leaves_open_path_direction_alone() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_under(STOCK_HALF, STOCK_HEIGHT));

    // Authored CCW so that the Bottom mirror (y -> stock_y - y) makes the
    // transformed ring CW — the one case where a re-wind is not a no-op.
    let river = Polygon2::open_path(vec![
        P2::new(10.0, -10.0),
        P2::new(10.0, 10.0),
        P2::new(-10.0, 10.0),
        P2::new(-10.0, -10.0),
    ]);

    let info = session.setup_transform_info(FaceUp::Bottom, ZRotation::Deg0);
    let out = info.apply_to_polygons(std::slice::from_ref(&river));

    // Precondition on the FIXTURE, scored on the expected constant rather
    // than on the output — scoring the output would read the ring after any
    // re-wind had already flipped its sign, which is the same circularity
    // that made this test vacuous in the first place. If the mirrored ring
    // were CCW, `ensure_winding` would be a no-op and the assertions below
    // would pass against a broken transform.
    let expected_ring = Polygon2::new(
        OPEN_PATH_MIRRORED
            .iter()
            .map(|(x, y)| P2::new(*x, *y))
            .collect(),
    );
    assert!(
        expected_ring.signed_area() < 0.0,
        "fixture precondition: the mirrored ring scores {:.1} mm², but the \
         re-wind this test guards against only fires on a NEGATIVE area — a \
         positive one makes the assertions below vacuous",
        expected_ring.signed_area()
    );

    assert!(
        !out[0].closed,
        "the open/closed flag must survive the transform"
    );
    assert_eq!(
        out[0].exterior.len(),
        OPEN_PATH_MIRRORED.len(),
        "the transform must neither add nor drop vertices"
    );
    for (i, (got, (ex, ey))) in out[0].exterior.iter().zip(OPEN_PATH_MIRRORED).enumerate() {
        assert!(
            (got.x - ex).abs() < EPS && (got.y - ey).abs() < EPS,
            "open path vertex {i} came back at {got:?}, expected \
             ({ex}, {ey}) — the mirrored coordinates in the SAME order. The \
             point order of an open path is its machining direction and must \
             not be re-wound; a reversal shows up here as the sequence read \
             back to front."
        );
    }
}

/// [`flip_leaves_open_path_direction_alone`]'s river through the
/// `FaceUp::Bottom` transform, stated rather than derived: `stock_under`
/// puts the stock at origin `(-27, -27)` with `y = 54`, so the map is
/// `(x, y) -> (x + 27, 54 - (y + 27))`. Same order as authored.
const OPEN_PATH_MIRRORED: [(f64, f64); 4] =
    [(37.0, 37.0), (37.0, 17.0), (17.0, 17.0), (17.0, 37.0)];
