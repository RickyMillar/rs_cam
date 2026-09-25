//! G-PECKSPLIT (2026-09-25) — the entry-descent pass never turns a peck
//! ladder's retract into a rapid into stock.
//!
//! ## The strike
//!
//! Wanaka airrun, "Back Rough" (adaptive3d, Ø6 flat, FRESH stock, `top_z`
//! pinned at the model top), move 3448: a `Rapid` from Z 11.000 to 9.033
//! inside a plunge entry, over a column the peck ladder had cut only to
//! 10.5 — 1.47 mm into material at every simulation cell tried. The
//! planner emitted the ladder correctly (feed to 10.5, retract to 11.0, feed
//! to 8.512). `dressup::optimize_entry_descents_with_provenance` then made
//! two mistakes at once:
//!
//! 1. It took the fresh-stock ceiling from `heights.top_z` (the pinned model
//!    top, 7.03) instead of the stock top (25), so its target was
//!    7.03 + `PLUNGE_CLEARANCE_MM` = 9.033.
//! 2. It matched ANY rapid followed by an `EntryPlunge` at the same XY,
//!    including the ladder's inter-peck retract, and split it.
//!
//! ## The fixture
//!
//! A dome (plate at Z 2, crown at 6.7) under 23.6 mm of fresh stock, a 6 mm
//! flat end mill, Depth/Pass 3, plunge entries, `top_z` pinned at the model
//! top — the Back Rough shape. The levels land so a peck ladder's last floor
//! is 9.1 over an entry at 8.6: before the fix the pass rapided the
//! retract (9.6) down to 6.7 + 2 = 8.7, 0.4 mm into the column, and the
//! live rapid check reported it (1 rapid collision at move 232). The model
//! top and stock top are chosen for that; nothing else is tuned.
//!
//! ## Red before, green after
//!
//! Measured 2026-09-25 by reverting the fix in the working tree:
//!
//! - both halves reverted: 5 of 5 red (the ladder retract is split to 8.7
//!   at move 232, below the 9.1 floor; the pinned rough has one move more
//!   than the Auto one; the 2D approach rapids to 12, inside the stock);
//! - predicate only reverted: `a_peck_ladder_retract_stays_a_retract` red
//!   (2 splits, not 1) — this fixture's ladder then splits at the stock
//!   top, above its start, so only the unit shape sees it;
//! - stock-top source only reverted:
//!   `the_fresh_split_target_is_the_stock_top_not_a_pinned_top` red.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_peck_ladder_is_never_split_into_a_rapid_into_stock_g_pecksplit
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    DressupEntryStyle, HeightMode, HeightReference, ReferenceOffset, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, AdaptiveConfig, ClearingStrategy,
};
use rs_cam_core::dressup::optimize_entry_descents;
use rs_cam_core::geo::P3;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, MoveType, PLUNGE_CLEARANCE_MM, Toolpath};
use rs_cam_core::trace::toolpath_spans::SpanKind;

const HALF: f64 = 20.0;
/// Fresh stock top: 16.9 mm over the model top, as Back Rough's overhead.
const STOCK_TOP_Z: f64 = 23.6;
/// The dome's crown — the model top a pinned `top_z` resolves to.
const MODEL_TOP_Z: f64 = 6.7;
const XY_EPS: f64 = 1e-6;

fn model_z(x: f64, y: f64) -> f64 {
    let r2 = x * x + y * y;
    2.0 + (MODEL_TOP_Z - 2.0) * (1.0 - r2 / 225.0).max(0.0)
}

/// The Back Rough shape; `pin_model_top` pins `top_z` at the model top,
/// otherwise `top_z` is Auto (the stock top).
fn session(pin_model_top: bool) -> ProjectSession {
    let cfg = Adaptive3dConfig {
        stepover: 2.5,
        depth_per_pass: 3.0,
        stock_to_leave_axial: 0.5,
        entry_style: Adaptive3dEntryStyle::Plunge,
        clearing_strategy: ClearingStrategy::ContourParallel,
        ..Adaptive3dConfig::default()
    };
    let mut s = common::session::single_op_session_with(
        common::session::stock_over(HALF, STOCK_TOP_Z),
        make_endmill_6mm(),
        common::session::mesh_model(common::meshes::height_field(HALF, 1.0, model_z), "dome"),
        "Back Rough",
        OperationConfig::Adaptive3d(cfg),
        |tc| {
            tc.stock_source = StockSource::Fresh;
            if pin_model_top {
                tc.heights.top_z = HeightMode::FromReference(ReferenceOffset {
                    reference: HeightReference::ModelTop,
                    offset: 0.0,
                });
            }
        },
    );
    common::session::generate(&mut s, 0);
    s
}

fn same_xy(a: P3, b: P3) -> bool {
    (a.x - b.x).abs() < XY_EPS && (a.y - b.y).abs() < XY_EPS
}

/// Indices of every inter-peck retract: a `Retract` rapid straight up from
/// an `EntryPlunge` floor at the same XY.
fn peck_retracts(tp: &Toolpath) -> Vec<usize> {
    (1..tp.moves.len())
        .filter(|&i| {
            let (prev, m) = (&tp.moves[i - 1], &tp.moves[i]);
            m.move_type == MoveType::Rapid
                && m.intent == MoveIntent::Retract
                && prev.intent == MoveIntent::EntryPlunge
                && same_xy(prev.target, m.target)
                && m.target.z > prev.target.z
        })
        .collect()
}

/// (a) The live rapid check reads no collision on the Back Rough shape, and
/// no rapid in an entry span ends below a floor that span already fed at
/// the same XY (the geometric oracle, independent of the simulator).
#[test]
fn no_rapid_in_an_entry_ends_in_stock() {
    let mut s = session(true);
    let result = s.get_result(0).expect("result");
    let tp = result.toolpath().clone();
    let spans = result.annotated().spans.clone();

    // Non-vacuity: the fixture must hold a peck ladder.
    assert!(
        !peck_retracts(&tp).is_empty(),
        "the fixture emits no peck ladder, so it tests nothing"
    );

    let mut entries = 0usize;
    for span in spans.iter().filter(|sp| sp.kind == SpanKind::Entry) {
        entries += 1;
        let mut fed: Vec<P3> = Vec::new();
        for i in span.start_move..span.end_move.min(tp.moves.len()) {
            let m = &tp.moves[i];
            if m.move_type == MoveType::Rapid {
                // The floor this entry has fed at this XY: the lowest fed
                // point there. Below it stands material the op knows of.
                let floor = fed
                    .iter()
                    .filter(|f| same_xy(**f, m.target))
                    .map(|f| f.z)
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    floor.is_infinite() || m.target.z >= floor - 1e-9,
                    "move {i}: rapid to Z {:.3} below the floor {floor:.3} this entry already fed",
                    m.target.z
                );
            } else if m.move_type.is_cutting() {
                fed.push(m.target);
            }
        }
    }
    assert!(entries > 0, "no entry spans");

    let cancel = AtomicBool::new(false);
    let sim = s
        .run_simulation(&SimulationOptions::default(), &cancel)
        .expect("simulation");
    assert_eq!(
        sim.rapid_collisions.len(),
        0,
        "rapid collisions: {:?}",
        sim.rapid_collisions
    );
}

/// (c) Every inter-peck retract is followed directly by the next peck — no
/// rapid is inserted between them.
#[test]
fn a_peck_ladder_retract_is_followed_by_its_next_peck() {
    let s = session(true);
    let tp = s.get_result(0).expect("result").toolpath().clone();
    let retracts = peck_retracts(&tp);
    assert!(!retracts.is_empty(), "no peck ladder in the fixture");
    for i in retracts {
        let next = &tp.moves[i + 1];
        assert!(
            next.intent == MoveIntent::EntryPlunge && same_xy(next.target, tp.moves[i].target),
            "move {}: the peck retract to Z {:.3} is followed by {:?} {:?} to Z {:.3}",
            i,
            tp.moves[i].target.z,
            next.move_type,
            next.intent,
            next.target.z
        );
    }
}

/// (b) The fresh-stock ceiling is the stock top, never a pinned `top_z`.
/// The adaptive3d rough already ignores a pinned top (it anchors on the
/// stock), so pinning `top_z` at the model top must not change one move.
#[test]
fn pinning_top_z_to_the_model_top_changes_nothing_on_a_fresh_rough() {
    let pinned = session(true);
    let auto = session(false);
    let a = pinned.get_result(0).expect("pinned").toolpath();
    let b = auto.get_result(0).expect("auto").toolpath();
    assert_eq!(a.moves.len(), b.moves.len(), "move counts differ");
    for (i, (ma, mb)) in a.moves.iter().zip(&b.moves).enumerate() {
        assert!(
            ma.move_type == mb.move_type
                && ma.intent == mb.intent
                && (ma.target - mb.target).norm() < 1e-9,
            "move {i}: pinned {:?} {:?} {:?} vs auto {:?} {:?} {:?}",
            ma.move_type,
            ma.intent,
            ma.target,
            mb.move_type,
            mb.intent,
            mb.target
        );
    }
    // And the old fallback's fingerprint is gone: no rapid ends at the
    // model top + clearance.
    let old_target = MODEL_TOP_Z + PLUNGE_CLEARANCE_MM;
    assert!(
        !a.moves
            .iter()
            .any(|m| m.move_type == MoveType::Rapid && (m.target.z - old_target).abs() < 1e-9),
        "a rapid still ends at model top + clearance ({old_target})"
    );
}

/// (b) on an approach the pass does split: a 2D adaptive plunges from safe
/// Z at every entry. With `top_z` pinned 10 mm under a fresh stock top, the
/// split target must be the stock top + clearance (22), never the pinned
/// top + clearance (12), which is 8 mm inside the fresh stock.
#[test]
fn the_fresh_split_target_is_the_stock_top_not_a_pinned_top() {
    const TOP: f64 = 20.0;
    const PINNED: f64 = 10.0;
    let cfg = AdaptiveConfig {
        depth: 3.0,
        depth_per_pass: 3.0,
        ..AdaptiveConfig::default()
    };
    let mut s = common::session::single_op_session_with(
        common::session::stock_over(HALF, TOP),
        make_endmill_6mm(),
        common::session::polygon_model(vec![common::session::square_polygon(HALF - 5.0)], "sq"),
        "Pocket",
        OperationConfig::Adaptive(cfg),
        |tc| {
            tc.stock_source = StockSource::Fresh;
            tc.dressups.entry_style = DressupEntryStyle::None;
            tc.heights.top_z = HeightMode::Manual(PINNED);
            tc.heights.bottom_z = HeightMode::Manual(PINNED - 3.0);
        },
    );
    common::session::generate(&mut s, 0);
    let tp = s.get_result(0).expect("result").toolpath();
    let target = TOP + PLUNGE_CLEARANCE_MM;
    let splits = tp
        .moves
        .iter()
        .filter(|m| m.move_type == MoveType::Rapid && (m.target.z - target).abs() < 1e-9)
        .count();
    for (i, m) in tp.moves.iter().enumerate() {
        assert!(
            m.move_type != MoveType::Rapid || m.target.z >= target - 1e-9,
            "move {i}: rapid to Z {:.3}, inside the fresh stock (top {TOP})",
            m.target.z
        );
    }
    assert!(
        splits > 0,
        "no approach was split to the stock top + clearance"
    );
}

/// The unit shape: one approach from safe Z, one peck ladder. With no stock
/// snapshot the ceiling is the `fresh_stock_top_z` argument.
fn approach_and_ladder() -> Toolpath {
    let mut tp = Toolpath::new();
    // Approach at (5, 5): safe Z, then a long plunge.
    tp.rapid_to_with_intent(P3::new(5.0, 5.0, 30.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(5.0, 5.0, 10.0), 500.0, MoveIntent::EntryPlunge);
    // A peck ladder at (0, 0): approach, peck to 20, retract to 20.5,
    // final peck to 16.
    tp.rapid_to_with_intent(P3::new(5.0, 5.0, 30.0), MoveIntent::Retract);
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 30.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 20.0), 500.0, MoveIntent::EntryPlunge);
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 20.5), MoveIntent::Retract);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 16.0), 500.0, MoveIntent::EntryPlunge);
    tp
}

/// (c) at the unit level: the approach is split to the ceiling + clearance;
/// the retract inside the ladder is not, though its geometry passes every
/// other gate (a ceiling of 15 puts the target 17 between 16 and 20).
#[test]
fn a_peck_ladder_retract_stays_a_retract() {
    let tool = FlatEndmill::new(6.0, 25.0);
    let mut tp = approach_and_ladder();
    let before = tp.moves.clone();
    let splits = optimize_entry_descents(&mut tp, None, 15.0, 3.0, &tool, None);
    assert_eq!(splits, 1, "exactly the approach splits: {:?}", tp.moves);
    // The approach: rapid to 15 + clearance, then the plunge.
    assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
    assert!((tp.moves[1].target.z - (15.0 + PLUNGE_CLEARANCE_MM)).abs() < 1e-9);
    // Everything after the approach is untouched.
    assert_eq!(tp.moves.len(), before.len() + 1);
    for (a, b) in tp.moves[3..].iter().zip(&before[2..]) {
        assert_eq!(a.move_type, b.move_type);
        assert_eq!(a.intent, b.intent);
        assert!((a.target - b.target).norm() < 1e-12);
    }
}
