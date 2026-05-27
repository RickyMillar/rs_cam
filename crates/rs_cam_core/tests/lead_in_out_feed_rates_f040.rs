//! F-040 — Lead-in / lead-out feed rate breakout.
//!
//! Acceptance bar:
//!
//! 1. **`lead_in_feed_rate = Some(500)` is honoured.** Lead-in arc moves
//!    carry F500; cutting moves carry the original feed.
//! 2. **`lead_in_feed_rate = None` falls back to cutting feed.** Pre-F-040
//!    behaviour byte-identical (modulo the `MoveIntent` tag change, which
//!    doesn't affect emitted G-code).
//! 3. **F-039 modulator skips `MoveIntent::{LeadIn, LeadOut}`.** Operator's
//!    explicit lead-in feed survives modulation.

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

/// Build a minimal profile-style toolpath: rapid to start, plunge, two cut
/// moves at depth, retract. apply_lead_in_out reads this pattern and
/// inserts lead-in / lead-out arcs.
fn minimal_profile_toolpath(cut_feed: f64, plunge_feed: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(0.0, 0.0, -2.0), plunge_feed); // plunge
    tp.feed_to(P3::new(20.0, 0.0, -2.0), cut_feed); // cut 1
    tp.feed_to(P3::new(20.0, 20.0, -2.0), cut_feed); // cut 2
    tp.rapid_to(P3::new(20.0, 20.0, 5.0)); // retract
    tp
}

#[test]
fn lead_in_feed_rate_applied_when_set() {
    let cut_feed = 1500.0;
    let li_feed = 500.0;
    let tp = minimal_profile_toolpath(cut_feed, 300.0);
    let annotated = AnnotatedToolpath::new(tp);
    let result = apply_lead_in_out_with_feeds(annotated, 2.0, Some(li_feed), None).toolpath;

    let lead_in_moves: Vec<_> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadIn)
        .collect();
    assert!(
        !lead_in_moves.is_empty(),
        "expected ≥1 lead-in-tagged move; got 0"
    );
    for m in &lead_in_moves {
        match m.move_type {
            MoveType::Linear { feed_rate } => assert!(
                (feed_rate - li_feed).abs() < 1.0,
                "lead-in feed should be {li_feed}, got {feed_rate}"
            ),
            _ => panic!("lead-in move should be Linear"),
        }
    }

    // Cutting moves keep their original feed.
    let cutting: Vec<_> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Unknown)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        })
        .filter(|&f| (f - cut_feed).abs() < 1.0)
        .collect();
    assert!(!cutting.is_empty(), "cutting moves should still carry F1500");
}

#[test]
fn lead_in_falls_back_to_pre_f040_feed_when_none() {
    // Pre-F-040 behaviour: lead-in arc moves used the feed_rate of the
    // *plunge* move that they replace (since the original plunge is what
    // gets re-routed into the lead-in arc). To keep F-040's default
    // behaviour byte-identical, `None` must reproduce that — i.e. the
    // plunge feed, not the cut feed.
    let cut_feed = 1500.0;
    let plunge_feed = 300.0;
    let tp = minimal_profile_toolpath(cut_feed, plunge_feed);
    let annotated = AnnotatedToolpath::new(tp);
    let result = apply_lead_in_out_with_feeds(annotated, 2.0, None, None).toolpath;

    let lead_in_moves: Vec<_> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadIn)
        .collect();
    assert!(
        !lead_in_moves.is_empty(),
        "expected ≥1 lead-in-tagged move even with default feed"
    );
    for m in &lead_in_moves {
        match m.move_type {
            MoveType::Linear { feed_rate } => assert!(
                (feed_rate - plunge_feed).abs() < 1.0,
                "lead-in feed should fall back to plunge feed {plunge_feed} (pre-F-040), got {feed_rate}"
            ),
            _ => panic!("lead-in move should be Linear"),
        }
    }
}

#[test]
fn lead_out_feed_rate_applied_when_set() {
    let cut_feed = 1500.0;
    let lo_feed = 4500.0;
    let tp = minimal_profile_toolpath(cut_feed, 300.0);
    let annotated = AnnotatedToolpath::new(tp);
    let result = apply_lead_in_out_with_feeds(annotated, 2.0, None, Some(lo_feed)).toolpath;

    let lead_out_moves: Vec<_> = result
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadOut)
        .collect();
    assert!(
        !lead_out_moves.is_empty(),
        "expected ≥1 lead-out-tagged move"
    );
    for m in &lead_out_moves {
        match m.move_type {
            MoveType::Linear { feed_rate } => assert!(
                (feed_rate - lo_feed).abs() < 1.0,
                "lead-out feed should be {lo_feed}, got {feed_rate}"
            ),
            _ => panic!("lead-out move should be Linear"),
        }
    }
}

#[test]
fn modulation_skips_lead_in_lead_out_moves() {
    use rs_cam_core::feed_modulation::{
        ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
        adaptive_feed_modulate,
    };
    use rs_cam_core::machine_kinematics::MachineKinematics;

    let cut_feed = 1500.0;
    let li_feed = 500.0;
    let lo_feed = 4500.0;
    let tp = minimal_profile_toolpath(cut_feed, 300.0);
    let annotated = AnnotatedToolpath::new(tp);
    let mut tp =
        apply_lead_in_out_with_feeds(annotated, 2.0, Some(li_feed), Some(lo_feed)).toolpath;

    // Capture lead-in / lead-out feeds before modulation.
    let pre_lead_in_feeds: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadIn)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        })
        .collect();
    let pre_lead_out_feeds: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadOut)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        })
        .collect();

    let engagements: Vec<PerMoveEngagement> = (0..tp.moves.len())
        .map(|_| PerMoveEngagement {
            radial_woc_fraction: 0.5,
            axial_doc_fraction: 0.5,
        })
        .collect();
    let kinematics = MachineKinematics::generic_wood_router();
    let band = ChiploadBand::new(0.03, 0.08).unwrap();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        max_feed_mm_min: 5000.0,
        rapid_feed_mm_min: 5000.0,
        chipload_band: band,
        kinematics: &kinematics,
        strategy: ModulationStrategy::ConstrainedMax,
        aggressiveness: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        nominal_axial_doc_mm: 2.0,
    };
    let _ = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();

    // Post-modulation: lead-in / lead-out feeds untouched.
    let post_lead_in_feeds: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadIn)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        })
        .collect();
    let post_lead_out_feeds: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::LeadOut)
        .filter_map(|m| match m.move_type {
            MoveType::Linear { feed_rate } => Some(feed_rate),
            _ => None,
        })
        .collect();

    assert_eq!(
        pre_lead_in_feeds, post_lead_in_feeds,
        "modulator must skip MoveIntent::LeadIn"
    );
    assert_eq!(
        pre_lead_out_feeds, post_lead_out_feeds,
        "modulator must skip MoveIntent::LeadOut"
    );
}
