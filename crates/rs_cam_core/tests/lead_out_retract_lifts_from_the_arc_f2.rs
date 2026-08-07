//! A closing retract must lift from where the tool IS, not from where it was
//! before the lead-out arc moved it.
//!
//! ## The defect
//!
//! `dressup::apply_lead_in_out` INSERTS a quarter-circle lead-out arc between
//! the last cutting move of a pass and the rapid that closes it. Generators
//! emit that rapid at the cut endpoint — a pure vertical lift, which is the
//! only shape a retract may legitimately have. After the arc is inserted the
//! tool is one arc radius away, so the untouched rapid becomes a **diagonal
//! rapid travelling backwards across the surface just finished, while
//! climbing to safe Z**. Over any feature taller than the climb reaches at
//! that XY, that is a rapid through stock.
//!
//! ## How it surfaced
//!
//! F2 (2026-08-06) replaced scallop's rounded coverage guard with the exact
//! point-in-triangle test. On the corrugated A/M7 fixture that moved the last
//! relinked fragment from a corner of the plate to its centre — and the same
//! closing retract that had always been diagonal started clipping a sawtooth
//! ridge. `scallop_intra_pass_relink_am7::no_new_collisions_at_the_finest_
//! resolution` went 0 -> 1 rapid collision at 0.1 mm.
//!
//! **The retract was latent, not new.** Nothing about F2 created it: the
//! relinker has always emitted its trailing retract at the FRAGMENT exit
//! (`surface_link.rs`), and this dressup has always made that stale. What F2
//! changed was where the last fragment ends, i.e. whether there happened to
//! be material between the two points. That is exactly the kind of defect
//! that hides until an unrelated change moves the geometry, which is why it
//! gets its own sentry rather than a note in a commit body.
//!
//! ## Form
//!
//! The parent revision's behaviour is transcribed as
//! [`parent_revision_retract_target`] and asserted to DISAGREE with the
//! shipped output, so the fix stays demonstrated in perpetuity — the same
//! substitution `exporter_span_classifier_x1.rs` discloses, and for the same
//! reason (the working tree is shared with another live lane, so checking out
//! the parent was not available).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dressup::apply_lead_in_out_with_feeds;
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

const CUT_Z: f64 = 0.65;
const SAFE_Z: f64 = 11.0;
const LEAD_RADIUS_MM: f64 = 2.0;

/// One straight cutting pass at `CUT_Z`, closed by a rapid that lifts
/// straight up from the cut endpoint — which is what every generator emits
/// and what the machine should do.
fn pass_with_a_vertical_closing_retract() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-5.0, 0.0, SAFE_Z));
    tp.feed_to_with_intent(P3::new(-5.0, 0.0, CUT_Z), 250.0, MoveIntent::EntryPlunge);
    for x in [-3.0, -1.0, 1.0, 3.0, 5.0] {
        tp.feed_to_with_intent(P3::new(x, 0.0, CUT_Z), 1000.0, MoveIntent::FinishingCut);
    }
    // The closing retract: same XY as the cut endpoint, straight up.
    tp.rapid_to_with_intent(P3::new(5.0, 0.0, SAFE_Z), MoveIntent::Retract);
    AnnotatedToolpath::new(tp)
}

/// The parent revision copied the closing rapid VERBATIM after inserting the
/// arc, so its target stayed at the pre-arc cut endpoint.
fn parent_revision_retract_target(cut_end: P3) -> (f64, f64) {
    (cut_end.x, cut_end.y)
}

fn last_rapid(tp: &Toolpath) -> (usize, P3) {
    let idx = tp
        .moves
        .iter()
        .rposition(|m| m.move_type == MoveType::Rapid)
        .expect("the fixture ends with a rapid");
    (idx, tp.moves[idx].target)
}

/// **The sentry.** After the lead-out arc, the closing rapid must sit at the
/// ARC's endpoint — i.e. it must still be a pure vertical lift.
#[test]
fn the_closing_retract_lifts_from_the_lead_out_endpoint_not_the_cut_endpoint() {
    let input = pass_with_a_vertical_closing_retract();
    let cut_end = P3::new(5.0, 0.0, CUT_Z);

    let out = apply_lead_in_out_with_feeds(input, LEAD_RADIUS_MM, None, None);
    let tp = &out.toolpath;

    // Non-vacuity 1: a lead-out arc was actually inserted. Without it the
    // assertion below is trivially true and proves nothing.
    let lead_out_count = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadOut)
        .count();
    assert!(
        lead_out_count >= 4,
        "the dressup emitted only {lead_out_count} LeadOut moves — no arc was inserted, so \
         this fixture cannot exhibit the defect",
    );

    let (idx, retract) = last_rapid(tp);
    // Non-vacuity 2: the rapid we are inspecting is the LAST move, i.e. the
    // pass's closing retract and not some interior traverse.
    assert_eq!(
        idx,
        tp.moves.len() - 1,
        "expected the closing rapid to be the final move",
    );

    let arc_end = tp.moves[idx - 1].target;
    assert_eq!(
        tp.moves[idx - 1].intent,
        MoveIntent::LeadOut,
        "the move before the closing retract must be the lead-out arc's last segment",
    );

    // The defect had teeth: parent and shipped must not agree, or this test
    // is pinning a no-op.
    let (px, py) = parent_revision_retract_target(cut_end);
    let parent_travel = ((arc_end.x - px).powi(2) + (arc_end.y - py).powi(2)).sqrt();
    assert!(
        parent_travel > 1.0,
        "the parent revision's retract target is {parent_travel:.3} mm from where the arc \
         left the tool — too close for this fixture to demonstrate anything. Increase the \
         lead radius.",
    );

    let travel = ((retract.x - arc_end.x).powi(2) + (retract.y - arc_end.y).powi(2)).sqrt();
    assert!(
        travel < 1e-9,
        "the closing retract travels {travel:.4} mm in XY while climbing from z = {CUT_Z} to \
         z = {}, i.e. straight back across the surface it just finished.\n\
         \n\
         retract target ({:.4}, {:.4}); the lead-out arc left the tool at ({:.4}, {:.4}); the \
         cut ended at ({:.4}, {:.4}) — the parent revision aimed the retract THERE.\n\
         \n\
         A retract must be a pure vertical lift. Over any feature taller than the climb \
         reaches, a diagonal one is a rapid through stock: that is how this was found \
         (A/M7 corrugated fixture, 0 -> 1 collision at 0.1 mm).",
        retract.z,
        retract.x,
        retract.y,
        arc_end.x,
        arc_end.y,
        cut_end.x,
        cut_end.y,
    );
}

/// The repair must not touch a rapid that is going somewhere on purpose.
/// A closing rapid aimed anywhere other than the pre-arc cut endpoint is a
/// real traverse and is copied verbatim, as before.
#[test]
fn a_rapid_heading_elsewhere_is_left_alone() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-5.0, 0.0, SAFE_Z));
    tp.feed_to_with_intent(P3::new(-5.0, 0.0, CUT_Z), 250.0, MoveIntent::EntryPlunge);
    for x in [-3.0, -1.0, 1.0, 3.0, 5.0] {
        tp.feed_to_with_intent(P3::new(x, 0.0, CUT_Z), 1000.0, MoveIntent::FinishingCut);
    }
    // A genuine traverse to the next pass's start, not a lift-in-place.
    let elsewhere = P3::new(-40.0, 25.0, SAFE_Z);
    tp.rapid_to_with_intent(elsewhere, MoveIntent::Linking);
    let out = apply_lead_in_out_with_feeds(AnnotatedToolpath::new(tp), LEAD_RADIUS_MM, None, None);

    let (_, retract) = last_rapid(&out.toolpath);
    assert!(
        (retract.x - elsewhere.x).abs() < 1e-9 && (retract.y - elsewhere.y).abs() < 1e-9,
        "a rapid heading elsewhere must be copied verbatim; got ({:.4}, {:.4})",
        retract.x,
        retract.y,
    );
}
