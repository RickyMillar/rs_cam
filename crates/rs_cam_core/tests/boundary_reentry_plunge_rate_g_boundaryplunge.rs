//! G-BOUNDARYPLUNGE (2026-09-07): the boundary clipper's re-entry descent
//! runs at the OPERATION's plunge rate, not at the crossing move's cut feed.
//!
//! # What was wrong
//!
//! `boundary::clip_toolpath_to_boundary_set_with_provenance` invents a new
//! move on an outside→inside crossing: a rapid to `safe_z` above the target,
//! then a descent to the target tagged `MoveIntent::EntryPlunge`. That
//! descent used to carry the CROSSING MOVE's feed
//! (`feed_rate_of(&m.move_type)`), i.e. the lateral cutting feed, on a
//! pure-vertical plunge. The Phase 3 feed-modulation guard cannot repair it:
//! `should_skip_modulation` is intent-based and skips an `EntryPlunge` by
//! design, so the descent reached the post-processor at the cut feed.
//!
//! Measured on the wanaka front rough (op dials: plunge 541 / feed
//! 750 mm/min): 7 residual plunge-class moves at 1.386× the op's own plunge
//! rate, every one of them `EntryPlunge`-tagged
//! (`planning/machine_kinematics_confidence_2026-09-07.md`, "Phase 3
//! result").
//!
//! # Red-then-green
//!
//! **The pre-fix walk emits F.** With the plunge-rate parameter threaded but
//! the walk body still preserving the cut feed, `a_reentry_descends_at_the_
//! operations_plunge_rate` fails with the re-entry feed reading 750 (F), not
//! 541 (P). The fix flips the four lines that build the re-entry move; the
//! control arms below are byte-identical across the flip.
//!
//! | Test | Pins |
//! |---|---|
//! | `a_reentry_descends_at_the_operations_plunge_rate` | P < F ⇒ the re-emitted `EntryPlunge` carries P, and the rapid-to-safe-Z before it is unchanged |
//! | `an_equal_plunge_rate_leaves_todays_output_byte_identical` | P == F ⇒ every move equals the `None` arm (= the shipped 3-argument wrapper) |
//! | `no_plunge_rate_preserves_the_cut_feed` | `None`, a zero and a non-finite rate all keep the cut feed — the disable condition the Phase 3 guard uses |
//! | `the_reentry_descent_is_pure_vertical_and_plunge_rated` | the walk rapids to the target XY first, so its descent is pure-vertical BY CONSTRUCTION — and it is plunge-rated whether or not it is |

#![allow(clippy::indexing_slicing, clippy::expect_used)]

use rs_cam_core::geo::P3;
use rs_cam_core::geometry::boundary::{
    clip_toolpath_to_boundary, clip_toolpath_to_boundary_set_with_provenance,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

/// The operation's cut feed (F) and plunge rate (P), in the wanaka front
/// rough's own proportion: P < F, which is the whole point of the dial.
const CUT_FEED: f64 = 750.0;
const PLUNGE_RATE: f64 = 541.0;
const SAFE_Z: f64 = 20.0;

/// A 10×10 box at the origin. Everything outside it is outside the boundary.
fn boundary() -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)
}

/// One cutting move that starts OUTSIDE the boundary and one that crosses
/// back in. The clipper turns move 0 into a rapid at `safe_z` and move 1
/// into the rapid-then-descend pair this sentry measures.
fn path_crossing_into_the_boundary() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(-5.0, 5.0, -1.0), CUT_FEED);
    tp.feed_to(P3::new(5.0, 5.0, -1.0), CUT_FEED);
    tp
}

/// The feed a move commands, or `None` for a rapid.
fn feed_of(m: &Move) -> Option<f64> {
    match m.move_type {
        MoveType::Rapid => None,
        MoveType::Linear { feed_rate } => Some(feed_rate),
        MoveType::ArcCW { feed_rate, .. } | MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
    }
}

/// Index of the single `EntryPlunge` move the clip emitted.
fn only_entry_plunge(tp: &Toolpath) -> usize {
    let found: Vec<usize> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.intent == MoveIntent::EntryPlunge)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "the fixture must produce exactly one boundary re-entry, got {found:?}",
    );
    found[0]
}

fn clip_with(plunge_rate: Option<f64>) -> Toolpath {
    clip_toolpath_to_boundary_set_with_provenance(
        &path_crossing_into_the_boundary(),
        &[boundary()],
        SAFE_Z,
        plunge_rate,
    )
    .0
}

/// The fix. `P < F` ⇒ the re-entry descends at P.
#[test]
fn a_reentry_descends_at_the_operations_plunge_rate() {
    let clipped = clip_with(Some(PLUNGE_RATE));
    let k = only_entry_plunge(&clipped);

    assert_eq!(
        feed_of(&clipped.moves[k]),
        Some(PLUNGE_RATE),
        "the re-emitted EntryPlunge must carry the operation's plunge rate \
         ({PLUNGE_RATE}), not the crossing move's cut feed ({CUT_FEED}) — \
         G-BOUNDARYPLUNGE",
    );
    assert_eq!(
        clipped.moves[k].intent,
        MoveIntent::EntryPlunge,
        "the intent tag is unchanged by the fix",
    );

    // The rapid to safe-Z that precedes it is untouched: same position, same
    // Linking intent, still a rapid.
    assert!(
        k >= 1,
        "a re-entry is always preceded by its rapid to safe-Z"
    );
    let rapid = &clipped.moves[k - 1];
    assert_eq!(rapid.move_type, MoveType::Rapid);
    assert_eq!(rapid.intent, MoveIntent::Linking);
    assert_eq!(rapid.target.z, SAFE_Z);
    assert_eq!(rapid.target.x, clipped.moves[k].target.x);
    assert_eq!(rapid.target.y, clipped.moves[k].target.y);
}

/// Control arm: `P == F` changes nothing at all, and the `None` arm is the
/// shipped 3-argument wrapper's output.
#[test]
fn an_equal_plunge_rate_leaves_todays_output_byte_identical() {
    let baseline =
        clip_toolpath_to_boundary(&path_crossing_into_the_boundary(), &boundary(), SAFE_Z);
    let equal = clip_with(Some(CUT_FEED));

    assert_eq!(equal.moves.len(), baseline.moves.len());
    for (i, (a, b)) in equal.moves.iter().zip(baseline.moves.iter()).enumerate() {
        assert_eq!(a.target, b.target, "move {i} position moved");
        assert_eq!(a.move_type, b.move_type, "move {i} feed/type moved");
        assert_eq!(a.intent, b.intent, "move {i} intent moved");
    }
}

/// The disable condition, mirroring the Phase 3 guard: `None`, zero, and a
/// non-finite rate all leave the crossing move's cut feed in place. An
/// operation may carry no plunge rate, and a zero would stop the machine.
#[test]
fn no_plunge_rate_preserves_the_cut_feed() {
    for rate in [None, Some(0.0), Some(-10.0), Some(f64::NAN)] {
        let clipped = clip_with(rate);
        let k = only_entry_plunge(&clipped);
        assert_eq!(
            feed_of(&clipped.moves[k]),
            Some(CUT_FEED),
            "an unusable plunge rate ({rate:?}) must leave the pre-fix feed alone",
        );
    }
}

/// This walk's re-entry descent is PURE-VERTICAL by construction: the rapid
/// before it goes to the target's own XY, so the descent moves in Z only,
/// whatever the crossing move's XY delta was. That is measured here rather
/// than assumed, because it is what makes the descent a plunge in
/// `kinematic_utilization::classify_move`'s terms.
///
/// The plunge rate does not depend on it. The descent is tagged
/// `EntryPlunge`, the modulator will never touch it, and the plunge dial is
/// the only rate that bounds it — so were this walk ever to emit a descent
/// that also moved in XY, that descent would still be plunge-rated,
/// deliberately.
#[test]
fn the_reentry_descent_is_pure_vertical_and_plunge_rated() {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(-5.0, 5.0, -1.0), CUT_FEED);
    // A crossing with a large XY delta: the walk still rapids to (5, 8) at
    // safe-Z first, so only Z is left for the descent.
    tp.feed_to(P3::new(5.0, 8.0, -1.0), CUT_FEED);

    let clipped = clip_toolpath_to_boundary_set_with_provenance(
        &tp,
        &[boundary()],
        SAFE_Z,
        Some(PLUNGE_RATE),
    )
    .0;
    let k = only_entry_plunge(&clipped);

    let from = clipped.moves[k - 1].target;
    let to = clipped.moves[k].target;
    assert_eq!(from.x, to.x, "the re-entry descent must not move in X");
    assert_eq!(from.y, to.y, "the re-entry descent must not move in Y");
    assert!(to.z < from.z, "the re-entry descent must go down");

    assert_eq!(
        feed_of(&clipped.moves[k]),
        Some(PLUNGE_RATE),
        "the descent is a plunge, so it runs at the operation's plunge rate",
    );
}
