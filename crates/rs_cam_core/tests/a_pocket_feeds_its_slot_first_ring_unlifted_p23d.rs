//! Feeds contract P23d (`planning/feeds_contract_2026-11-03/phases/P23d.md`;
//! operator ruling 2026-11-10, "yes to the lower feed"): the first ring of
//! each level of a contour Pocket cuts a full slot in fresh stock, where
//! the radial chip-thinning lift does not hold. The generator feeds that
//! ring at the unlifted feed (the op feed over the lift,
//! `feeds::geometry::UnliftedFeed::of_slot_ring`) and holds it as
//! `Move::feed_cap`. Every later ring runs at the op feed.
//!
//! The fixture: a 6 mm end mill, stepover 2.0 mm, feed 1000 mm/min, a
//! square pocket in two levels. The lift is Garr's
//! `1 / sqrt(1 - (1 - 2 x 2/6)^2)` = 1.0607, so the slot ring runs at
//! 1000 / 1.0607 = 942.8 mm/min.
//!
//! The claims:
//!
//! 1. on each level, the cutting moves of the first ring carry the unlifted
//!    feed and the cap; the later rings carry no cap and run above the slot
//!    feed; the entry moves carry no cap;
//! 2. the session's dressup chain keeps it (the moves read here are the
//!    session result), and the G-code prints both feeds;
//! 3. Suggest prints the same value (`SuggestedSpeeds::slot_ring_feed_mm_min`)
//!    for the op it suggests, and `None` for an op with no slot-first ring;
//! 4. a Pocket whose lift is 1 (stepover at half the diameter) and the
//!    Zigzag pattern carry no cap, and no cut runs below the op feed.
//!
//! Red: emit every ring at `params.feed_rate` in
//! `ops/pocket.rs::pocket_contours_to_sided_toolpath_slot_first`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::operation_configs::PocketPattern;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};

mod common;
use common::session::{generate, polygon_model, single_op_session, square_polygon, stock_under};
use common::tools::endmill_tool_config;

const TOOL_D: f64 = 6.0;
const STEPOVER: f64 = 2.0;
const HALF: f64 = 20.0;
const STOCK_Z: f64 = 6.0;

/// Garr's lift at the width `ae` on the diameter `d`.
fn garr_lift(ae: f64, d: f64) -> f64 {
    if ae >= d / 2.0 {
        1.0
    } else {
        1.0 / (1.0 - (1.0 - 2.0 * ae / d).powi(2)).sqrt()
    }
}

fn pocket(stepover: f64, pattern: PocketPattern) -> OperationConfig {
    let mut op = fixture_op(OperationType::Pocket);
    assert!(op.set_stepover(stepover));
    let OperationConfig::Pocket(cfg) = &mut op else {
        panic!("the fixture is a Pocket");
    };
    cfg.pattern = pattern;
    op
}

fn generated(op: OperationConfig) -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        op,
    );
    generate(&mut session, 0);
    session
}

fn feed_of(m: &Move) -> Option<f64> {
    match m.move_type {
        MoveType::Linear { feed_rate }
        | MoveType::ArcCW { feed_rate, .. }
        | MoveType::ArcCCW { feed_rate, .. } => Some(feed_rate),
        MoveType::Rapid => None,
    }
}

/// The passes of a toolpath (each run of fed moves between two rapids): for
/// each, the Z of its cut and its cutting moves. The entry dressup of the
/// session turns each plunge into its entry moves, which are not cuts.
fn passes(moves: &[Move]) -> Vec<(f64, Vec<&Move>)> {
    let mut out: Vec<(f64, Vec<&Move>)> = Vec::new();
    let mut open = false;
    for m in moves {
        if feed_of(m).is_none() {
            open = false;
            continue;
        }
        if !open {
            out.push((f64::NAN, Vec::new()));
            open = true;
        }
        if m.intent == MoveIntent::ClearingCut {
            let pass = out.last_mut().expect("a pass is open");
            pass.0 = m.target.z;
            pass.1.push(m);
        }
    }
    out.retain(|(_, cuts)| !cuts.is_empty());
    out
}

/// Claims 1 and 2.
#[test]
fn the_first_ring_of_each_level_runs_at_the_unlifted_feed_with_the_cap_p23d() {
    let op = pocket(STEPOVER, PocketPattern::Contour);
    let op_feed = op.feed_rate().expect("the fixture states a feed");
    let plunge = op.plunge_rate().expect("the fixture states a plunge");
    let slot = op_feed / garr_lift(STEPOVER, TOOL_D);
    assert!(slot < op_feed * 0.99, "non-vacuity: the lift is above 1");

    let session = generated(op);
    let moves = &session.result_at(0).expect("a result").toolpath().moves;
    let passes = passes(moves);
    let mut levels: Vec<f64> = passes.iter().map(|(z, _)| *z).collect();
    levels.dedup();
    assert!(levels.len() >= 2, "non-vacuity: two levels: {levels:?}");
    assert!(
        passes.len() >= 2 * levels.len(),
        "non-vacuity: more than one ring per level: {} passes",
        passes.len()
    );

    let mut seen: Vec<f64> = Vec::new();
    for (z, cuts) in &passes {
        let first_of_level = !seen.contains(z);
        seen.push(*z);
        for m in cuts {
            let feed = feed_of(m).expect("a cut has a feed");
            if first_of_level {
                assert!(
                    (feed - slot).abs() < 1e-9,
                    "z {z}: the slot ring feed {feed} is not {slot}"
                );
                assert_eq!(m.feed_cap, Some(feed), "z {z}: the slot ring holds its cap");
            } else {
                // The feed optimisation of the session may move a free
                // feed; it starts from the op feed and has no cap.
                assert!(
                    feed > slot + 1.0,
                    "z {z}: a later ring runs above the slot feed: {feed}"
                );
                assert_eq!(m.feed_cap, None, "z {z}: a later ring has no cap");
            }
        }
    }
    // The entry moves (the plunge, or the ramp that the entry dressup
    // makes of it) carry no cap: the cap is on the cut alone.
    let entries: Vec<&Move> = moves
        .iter()
        .filter(|m| feed_of(m).is_some() && m.intent != MoveIntent::ClearingCut)
        .collect();
    assert!(!entries.is_empty(), "non-vacuity: the op has entry moves");
    for m in entries {
        assert_eq!(m.feed_cap, None, "{:?}", m.intent);
    }
    assert!(plunge > 0.0);

    // Claim 2: the post prints both feeds.
    let gcode = session
        .export_gcode_checked(None)
        .expect("the export runs")
        .gcode;
    // The post writes a feed as a whole mm/min.
    let words: std::collections::BTreeSet<&str> = gcode
        .split_whitespace()
        .filter(|w| w.starts_with('F'))
        .collect();
    assert!(
        words.contains(format!("F{:.0}", slot).as_str()),
        "the G-code holds the slot ring feed F{slot:.0}: {words:?}"
    );
    assert!(
        words.contains(format!("F{:.0}", op_feed).as_str()),
        "the G-code holds the op feed F{op_feed:.0}: {words:?}"
    );
}

/// Claim 3: Suggest prints the feed that the generator cuts.
#[test]
fn suggest_prints_the_slot_ring_feed_that_the_generator_cuts_p23d() {
    let session = generated(pocket(STEPOVER, PocketPattern::Contour));
    let tc = session.toolpath_configs()[0].clone();
    let profile = session.cutter_op_profile(&tc).expect("the op has a tool");
    let speeds = profile
        .suggested_speeds()
        .expect("Suggest serves the fixture");
    let suggested_feed = speeds.feed_mm_min.expect("a suggested feed");
    let slot = speeds
        .slot_ring_feed_mm_min
        .expect("a contour Pocket has a slot-first ring");
    assert!(
        (slot - suggested_feed / garr_lift(STEPOVER, TOOL_D)).abs() < 1e-9,
        "the slot ring feed {slot} is the suggested feed {suggested_feed} over the lift"
    );
    assert!(
        (speeds.radial_lift - garr_lift(STEPOVER, TOOL_D)).abs() < 1e-9,
        "the lift of the same record"
    );

    // An op with no slot-first ring prints none: the Zigzag pattern.
    let zigzag = generated(pocket(STEPOVER, PocketPattern::Zigzag));
    let tc = zigzag.toolpath_configs()[0].clone();
    let speeds = zigzag
        .cutter_op_profile(&tc)
        .expect("the op has a tool")
        .suggested_speeds()
        .expect("Suggest serves the fixture");
    assert_eq!(speeds.slot_ring_feed_mm_min, None);
}

/// Claim 4: no lift, or no contour rings: no cap, every cut at the op feed.
#[test]
fn a_pocket_with_no_lift_and_a_zigzag_pocket_carry_no_cap_p23d() {
    for op in [
        pocket(TOOL_D / 2.0, PocketPattern::Contour),
        pocket(STEPOVER, PocketPattern::Zigzag),
    ] {
        let op_feed = op.feed_rate().expect("the fixture states a feed");
        let session = generated(op);
        let moves = &session.result_at(0).expect("a result").toolpath().moves;
        let cuts: Vec<&Move> = moves
            .iter()
            .filter(|m| m.intent == MoveIntent::ClearingCut)
            .collect();
        assert!(!cuts.is_empty(), "non-vacuity: the op cuts");
        for m in cuts {
            assert_eq!(m.feed_cap, None);
            assert!(
                feed_of(m).is_some_and(|f| f >= op_feed - 1e-9),
                "no cut runs below the op feed: {:?}",
                feed_of(m)
            );
        }
    }
}
