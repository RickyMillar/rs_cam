//! S2 sentries — the live-stock, profile-aware rapid clearance check.
//!
//! Phase S1 measured 982 descending rapids entering standing material on a
//! shipped job while `rapid_collision_count` read **zero**
//! (`planning/rapid_safety_2026-08-28/S1_RESULTS.md`). §3 of that document
//! resolved the mechanism: the descents clear at their exact XY, and the
//! material stands **0.5–2.9 mm off-axis** — inter-pass crests at rough swath
//! edges, struck by the taper's flank. A zero-radius point probe of a frozen
//! pre-op snapshot (`check_rapid_collisions_against_stock`) cannot see that
//! class, and a naive disc upgrade of that same frozen snapshot would flag the
//! op's own already-cut rows instead.
//!
//! The fix rides the simulator's own walk, so each rapid is judged against the
//! stock AS IT EXISTS at that point of playback, with
//! `max_clearance_tip_z_for_profile`. These four tests pin the four things that
//! has to get right at once: it must SEE the measured class, must not invent a
//! false positive where the tool's own profile clears the obstacle, must not
//! flag a re-entry into material the same toolpath removed (with no F3-style
//! heuristic in the live path), and must still agree with the old point probe
//! on flat-tool-over-flat-stock geometry, where the two questions coincide.
//!
//! ```text
//! cargo test -p rs_cam_core --test rapid_live_check_crest_s2
//! ```
//!
//! **Known conservatism, deliberately not exercised here.** The clearance
//! primitive evaluates the profile at `r − half_diagonal`, so for a cutter
//! whose flank turns vertical at its envelope radius (a full ball nose; a flat
//! endmill's kerf wall) the over-read near the rim is `h(R) − h(R − hd)`,
//! ≈ `sqrt(2·R·hd)` for a ball — which grows as the SQUARE ROOT of the cell
//! size against a tolerance linear in it. Such a tool descending into a kerf of
//! exactly its own width can therefore read as a strike. Conical flanks (the
//! tapered tools of the measured class) have a linear over-read and are
//! unaffected. Test 3 below uses a pocket two passes wide for that reason, and
//! says so.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::collision::{RapidClearanceCheck, check_rapid_collisions_against_stock};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::Toolpath;

/// 0.2 mm cells → clearance tolerance ≈ 0.441 mm (2.207 × cell). Every margin
/// quoted below is against that figure.
const CELL: f64 = 0.2;
/// Stock top, and therefore the top of the standing crest.
const CREST_TOP_Z: f64 = 2.0;
/// What the crest stands on: the surrounding material, cut away.
const FLOOR_Z: f64 = 0.0;

/// The wanaka finish tool's shape class: Ø3 ball tip on a 10° taper to a Ø6
/// shaft, so the envelope radius is 3 mm and the flank rises steeply.
fn finish_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(3.0, 10.0, 6.0, 25.0)
}

/// A 20 × 20 board cut down to `FLOOR_Z` everywhere except a wall of full-height
/// material spanning `ridge_min_x ..= ridge_max_x` — the inter-pass crest of the
/// measured class, built by direct cell edits so the geometry is exact.
fn crest_stock(ridge_min_x: f64, ridge_max_x: f64) -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, CREST_TOP_Z, CELL);
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let origin_u = stock.z_grid.origin_u;
    let cs = stock.z_grid.cell_size;
    for row in 0..rows {
        for col in 0..cols {
            let x = origin_u + col as f64 * cs;
            if x < ridge_min_x || x > ridge_max_x {
                stock.clear_above_at(row, col, FLOOR_Z as f32);
            }
        }
    }
    stock
}

/// Anchor high above the descent XY, then rapid straight down to `target_z`.
/// Move index 1 is the descent.
fn descent_toolpath(x: f64, y: f64, target_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(x, y, 12.0));
    tp.rapid_to(P3::new(x, y, target_z));
    tp
}

/// Run the metric walk — the production seam — with the live check attached,
/// and return the move indices it flagged.
fn live_hits_metric(
    stock: &mut TriDexelStock,
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
) -> Vec<usize> {
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut check = RapidClearanceCheck::new(cutter);
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            &lut,
            cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.25,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    check
        .into_hits()
        .into_iter()
        .map(|c| c.move_index)
        .collect()
}

/// The same, through the non-metric playback walk (`replay_moves`), which the
/// simulator uses when metrics are switched off. Both walks must answer alike.
fn live_hits_playback(
    stock: &mut TriDexelStock,
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
) -> Vec<usize> {
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut check = RapidClearanceCheck::new(cutter);
    stock
        .simulate_toolpath_with_lut_cancel_rapid_checked(
            tp,
            &lut,
            cutter.radius(),
            StockCutDirection::FromTop,
            &never_cancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    check
        .into_hits()
        .into_iter()
        .map(|c| c.move_index)
        .collect()
}

fn point_probe_hits(stock: &TriDexelStock, tp: &Toolpath) -> Vec<usize> {
    check_rapid_collisions_against_stock(tp, &stock.z_grid)
        .into_iter()
        .map(|c| c.move_index)
        .collect()
}

/// THE CREST CASE — the class S1 measured, reproduced in miniature.
///
/// A 2 mm crest stands 1.0 mm off the descent's axis. The taper needs only
/// `height_at_radius(0.86) = 0.27 mm` of clearance there, so a descent to the
/// surrounding floor drives the flank 1.73 mm into the crest — four times the
/// 0.441 mm tolerance. The descent's OWN column is cut to the floor, which is
/// exactly why the point probe is silent: S-b, in one fixture.
#[test]
fn an_off_axis_crest_inside_the_envelope_is_flagged() {
    let cutter = finish_taper();
    let tp = descent_toolpath(10.0, 10.0, FLOOR_Z);
    let mut stock = crest_stock(11.0, 12.2);

    assert_eq!(
        point_probe_hits(&stock, &tp),
        Vec::<usize>::new(),
        "the zero-radius point probe must stay silent here — if it flags, the \
         fixture is not the S-b class"
    );

    assert_eq!(
        live_hits_metric(&mut stock, &tp, &cutter),
        vec![1],
        "the descent drives the taper's flank 1.73 mm into a crest 1.0 mm \
         off-axis; the live profile-aware check must see it"
    );

    let mut playback_stock = crest_stock(11.0, 12.2);
    assert_eq!(
        live_hits_playback(&mut playback_stock, &tp, &cutter),
        vec![1],
        "the non-metric playback walk must answer the same"
    );
}

/// FALSE-POSITIVE GUARD — the same crest, moved out to where the tool's own
/// shape clears it.
///
/// At 2.8 mm off-axis the taper stands 7.9 mm above its tip, so a 2 mm crest
/// passes underneath. The test is not vacuous: a radius-aware but
/// profile-BLIND disc query (the "highest material anywhere under the tool"
/// reading) would demand 2.0 mm of lift here and flag, so only consulting the
/// profile passes it.
#[test]
fn a_crest_the_profile_clears_is_not_flagged() {
    let cutter = finish_taper();
    let tp = descent_toolpath(10.0, 10.0, FLOOR_Z);
    let mut stock = crest_stock(12.7, 13.9);

    assert_eq!(
        live_hits_metric(&mut stock, &tp, &cutter),
        Vec::<usize>::new(),
        "a 2 mm crest 2.8 mm off-axis sits under a flank standing 7.9 mm above \
         the tip — flagging it would trade the false negative for a false \
         positive"
    );
}

/// OWN-KERF RE-ENTRY — the artefact class a disc query on the FROZEN snapshot
/// would produce, and the reason the check rides the live walk.
///
/// Two overlapping passes cut a pocket 10 mm wide at Z −2; the tool then
/// retracts, hops to the pocket centre and descends to −1.9. The live stock
/// knows that column — and the whole probe disc around it — is air. No
/// F3-style same-XY walk-back is involved: the descent XY is not the XY of any
/// preceding feed, so that heuristic could not fire even if it had been ported.
///
/// The pocket is deliberately two passes wide. A kerf of exactly the tool's own
/// width puts uncut wall inside the query's half-cell dilation ring, which is
/// the separate conservatism recorded in this file's header.
#[test]
fn a_re_entry_into_material_this_toolpath_removed_is_not_flagged() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 8.0, 5.0));
    tp.feed_to(P3::new(5.0, 8.0, -2.0), 300.0);
    tp.feed_to(P3::new(25.0, 8.0, -2.0), 900.0);
    tp.feed_to(P3::new(25.0, 12.0, -2.0), 900.0);
    tp.feed_to(P3::new(5.0, 12.0, -2.0), 900.0);
    tp.rapid_to(P3::new(5.0, 12.0, 5.0));
    tp.rapid_to(P3::new(15.0, 10.0, 5.0));
    tp.rapid_to(P3::new(15.0, 10.0, -1.9));

    let fresh = TriDexelStock::from_stock(0.0, 0.0, 30.0, 20.0, -5.0, 2.0, CELL);
    assert_eq!(
        point_probe_hits(&fresh, &tp),
        vec![7],
        "against the frozen pre-op snapshot the descent looks like a plunge \
         through 3.9 mm of stock — that is what the live walk has to see past"
    );

    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 20.0, -5.0, 2.0, CELL);
    assert_eq!(
        live_hits_metric(&mut stock, &tp, &cutter),
        Vec::<usize>::new(),
        "the pocket floor stands at −2.0 across the whole probe disc; a descent \
         to −1.9 is 0.1 mm of air"
    );
}

/// FLAT-ENDMILL DISCIPLINE — on the geometry where the two queries ask the same
/// question, they must give the same verdicts.
///
/// A flat endmill's `height_at_radius` is 0 everywhere inside its envelope, so
/// over a flat-topped stock the profile query reduces to "is the tip below the
/// stock top". This is an ANCHOR, not a byte-identity claim: the live check
/// reads `conservative_top` over a disc on live stock and subtracts a
/// tolerance, the pre-pass reads `ray_top` at a point on a snapshot with none.
/// Both must flag the descent into material and neither the traverse above it.
#[test]
fn flat_endmill_over_flat_stock_matches_the_point_probe_verdicts() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(2.0, 10.0, 5.0));
    // Move 1: clear traverse 3 mm above the stock top.
    tp.rapid_to(P3::new(18.0, 10.0, 5.0));
    // Move 2: descent to Z 1.0 — 1.0 mm under the 2.0 stock top.
    tp.rapid_to(P3::new(18.0, 10.0, 1.0));

    let fresh = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, 2.0, CELL);
    assert_eq!(point_probe_hits(&fresh, &tp), vec![2]);

    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, 2.0, CELL);
    assert_eq!(
        live_hits_metric(&mut stock, &tp, &cutter),
        vec![2],
        "descent into material flags, clear traverse does not — the two queries \
         must not disagree where they coincide"
    );
}
