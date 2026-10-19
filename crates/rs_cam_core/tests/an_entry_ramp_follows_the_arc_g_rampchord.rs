//! G-RAMPCHORD — a rest entry ramp follows an arc move along the arc, never
//! along its chord.
//!
//! # The defect
//!
//! `dressup::optimize_entry_descents*` runs after the arc fit. On a
//! rest-driven surface-riding pass (`RestEntryRamp`), it replaces a fed
//! descent with zig-zag laps along the run that follows the plunge
//! (`pencil::plan_entry_ramp`). The run was built from the move TARGETS
//! alone, so an arc move gave the run its two end points, and the laps cut
//! along the chord between them. The chord leaves the op's own cutter path:
//! on a waterline ring it cuts into the part. The W4 conformance suite
//! (check 2b) measured the rest Waterline 3.1 mm under the cap surface in
//! 493 cells, 356.6 mm³ removed against 156.5 mm³ for the Fresh op.
//!
//! # The fix under test
//!
//! `dressup::entry_descent::upcoming_run` samples an arc move along the
//! arc, within 0.01 mm of it. Every op that the descent pass ramps gets
//! the fix: the run builder is the one site.
//!
//! # The claim
//!
//! A ring of four quarter arcs, entered by a fed plunge through standing
//! material: every lap point, and every point along every lap move, lies
//! on the ring within [`TOL_MM`]. The pre-fix laps ran along the chord of
//! the first quarter arc, 1.17 mm inside a 4 mm ring at its midpoint.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::{RestEntryRamp, optimize_entry_descents};
use rs_cam_core::finish::pencil::tip_contact_radius;
use rs_cam_core::geo::P3;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

const CENTRE: (f64, f64) = (12.0, 12.0);
const RING_RADIUS_MM: f64 = 4.0;
/// The ring's cut Z. The block stands to [`STOCK_TOP_Z`] over it, so the
/// descent is deeper than one bite budget and the pass ramps it.
const RING_Z: f64 = 0.0;
const STOCK_TOP_Z: f64 = 1.4;
const SAFE_Z: f64 = 12.0;
const CUT_FEED: f64 = 782.0;
const PLUNGE_RATE: f64 = 171.0;
/// The most a lap point may stand off the ring (mm): the run's chord
/// tolerance (0.01 mm) and float slack.
const TOL_MM: f64 = 0.02;

fn tapered_ball() -> TaperedBallEndmill {
    TaperedBallEndmill::new(2.0, 7.1, 6.0, 20.0).unwrap()
}

/// A rapid over the ring start, a fed plunge to the ring, then the ring as
/// four counter-clockwise quarter arcs.
fn ring_toolpath() -> Toolpath {
    let (cx, cy) = CENTRE;
    let r = RING_RADIUS_MM;
    let at = |k: usize| {
        let a = k as f64 * std::f64::consts::FRAC_PI_2;
        P3::new(cx + r * a.cos(), cy + r * a.sin(), RING_Z)
    };
    let start = at(0);
    let mut moves = vec![
        Move {
            target: P3::new(start.x, start.y, SAFE_Z),
            move_type: MoveType::Rapid,
            intent: MoveIntent::Linking,
        },
        Move {
            target: start,
            move_type: MoveType::Linear {
                feed_rate: PLUNGE_RATE,
            },
            intent: MoveIntent::EntryPlunge,
        },
    ];
    for k in 1..=4 {
        let from = at(k - 1);
        moves.push(Move {
            target: at(k % 4),
            move_type: MoveType::ArcCCW {
                i: cx - from.x,
                j: cy - from.y,
                feed_rate: CUT_FEED,
            },
            intent: MoveIntent::FinishingCut,
        });
    }
    moves.push(Move {
        target: P3::new(start.x, start.y, SAFE_Z),
        move_type: MoveType::Rapid,
        intent: MoveIntent::Retract,
    });
    Toolpath { moves }
}

fn off_ring(p: P3) -> f64 {
    ((p.x - CENTRE.0).hypot(p.y - CENTRE.1) - RING_RADIUS_MM).abs()
}

#[test]
fn a_rest_entry_ramp_laps_on_the_arc_not_its_chord() {
    let cutter = tapered_ball();
    let stock = TriDexelStock::from_stock(0.0, 0.0, 24.0, 24.0, -5.0, STOCK_TOP_Z, 0.2);
    let cfg = RestEntryRamp {
        contact_radius_mm: tip_contact_radius(&cutter),
        feed_rate: CUT_FEED,
        plunge_rate: PLUNGE_RATE,
    };
    let mut tp = ring_toolpath();
    optimize_entry_descents(
        &mut tp,
        Some(&stock),
        STOCK_TOP_Z,
        cutter.envelope_radius_mm(),
        &cutter,
        Some(&cfg),
    );

    // Non-vacuity: the pass ramped this entry.
    let laps = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .count();
    assert!(
        laps > 0,
        "the descent was not ramped; the claim proves nothing"
    );

    let mut worst = 0.0_f64;
    for w in tp.moves.windows(2) {
        if w[1].intent != MoveIntent::EntryRamp {
            continue;
        }
        let (a, b) = (w[0].target, w[1].target);
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            let p = P3::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y), 0.0);
            worst = worst.max(off_ring(p));
        }
    }
    assert!(
        worst <= TOL_MM,
        "a lap stands {worst:.3} mm off the ring (tolerance {TOL_MM} mm): the ramp cuts \
         along an arc's chord, off the op's own cutter path"
    );
}
