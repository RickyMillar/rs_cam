//! G-ADAPTRAPIDLIFT — a 2D Adaptive rapid travels in XY only at the rapid
//! plane; it leaves cut depth straight up.
//!
//! `adaptive/path.rs` `segments_to_toolpath` emitted a `Rapid` segment as one
//! G0 from the cutter's position at cut depth straight to the next entry at
//! safe Z, so the cutter climbed on a diagonal through whatever stood beside
//! it: the pocket wall, an island, residue. It now retracts Z-only first with
//! the shared `Toolpath::final_retract`, the shape the rest of the codebase
//! emits (retract, XY rapid at the retract height, descend).
//!
//! Before, on the six-island fixture (`common::adaptive_islands`, 91bb006a):
//! 38 diagonal rapids leave cut depth, and the live rapid check of the
//! session simulation (`RapidClearanceCheck`, the stock as it stands at that
//! point of playback, with the cutter's profile) reports 34 strikes, every
//! one on a diagonal rapid (the first: from (57, -37, -3) past the pocket
//! wall toward (-41.4, -10.3, 10)).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use common::adaptive_islands::{adaptive_session, stock, toolpath_of};
use rs_cam_core::toolpath::MoveType;

/// Every XY rapid runs at the rapid plane, and the live rapid check finds
/// no strike.
#[test]
fn a_rapid_leaves_cut_depth_straight_up() {
    let session = adaptive_session(false);
    let tp = toolpath_of(&session);
    let stock = stock();
    let stock_top = stock.origin_z + stock.z;
    // The rapid plane: the retract height the path climbs to. It must
    // clear the stock top, or the check below is vacuous.
    let plane = tp
        .moves
        .iter()
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        plane > stock_top,
        "rapid plane {plane} is not above the stock top {stock_top}"
    );
    let mut xy_rapids = 0;
    let mut low = Vec::new();
    for (i, w) in tp.moves.windows(2).enumerate() {
        let (a, b) = (w[0].target, w[1].target);
        if !matches!(w[1].move_type, MoveType::Rapid) || (b.x - a.x).hypot(b.y - a.y) < 1e-9 {
            continue;
        }
        xy_rapids += 1;
        if a.z.min(b.z) < plane - 1e-6 {
            low.push((i + 1, a.z, b.z));
        }
    }
    let strikes = session
        .simulation_result()
        .expect("simulated")
        .rapid_collisions
        .len();
    eprintln!(
        "G-ADAPTRAPIDLIFT: {xy_rapids} XY rapids, {} below the plane {plane}; {strikes} live \
         rapid strikes",
        low.len()
    );
    assert!(xy_rapids >= 1, "no XY rapid: the fixture tests nothing");
    assert!(
        low.is_empty(),
        "{} rapids travel in XY below the rapid plane {plane} (move, from z, to z): {:?}",
        low.len(),
        &low[..low.len().min(5)]
    );
    assert_eq!(strikes, 0, "the live rapid check reports strikes");
}
