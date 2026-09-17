//! S3 sentries — the air-cut filter classifies a sample for the whole CUTTER.
//!
//! Phase S1 measured 982 rapid descents into standing material on a shipped
//! job (`planning/rapid_safety_2026-08-28/S1_RESULTS.md`); S2 gave the
//! detector the profile-aware query that can see them. This file pins the
//! EMITTER fix.
//!
//! The emitter is `dressup::filter_air_cuts`. The generator emits a safe fed
//! `EntryPlunge` at each raster link; the filter then decided whether that
//! plunge was "all air" with a **zero-radius centerline probe** — the
//! parameter in the cutter's place was a `tool_radius: f64` documented as
//! "reserved for future per-cell radius checks" and never read. On wanaka the
//! plunge's own column read clear by +0.1 mm while the taper's flank stood
//! −1.3 mm inside an inter-pass crest 0.5–2.9 mm off-axis, so the fed plunge
//! was reclassified as air, dropped, and replaced by a rapid descending to
//! the resume Z with zero clearance. The emitter's blindness and the
//! detector's were the same blindness, which is why they masked each other.
//!
//! ```text
//! cargo test -p rs_cam_core --test air_filter_tool_aware_s3
//! ```
//!
//! The four tests are the four things the fix has to get right at once: see
//! the measured class (1), still convert genuinely-clear air so the fix costs
//! no travel (2), consult the tool's PROFILE and not merely its envelope (3),
//! and close the loop against S2's live detector on the emitted motion (4).
//!
//! Every margin below is at least 4× the filter's 0.1 mm tolerance, and the
//! fixtures ride conical flanks — the kerf-rim √-over-read documented in
//! `tests/rapid_live_check_crest_s2.rs`'s header applies to a tool whose
//! flank turns vertical at its envelope rim and is deliberately not exercised
//! here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::without_provenance;
use rs_cam_core::dressup::{AirBridgePolicy, filter_air_cuts};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// 0.2 mm cells: fine enough that the ridge edges land on cell centres, and
/// the same resolution the S2 sentries use.
const CELL: f64 = 0.2;
/// Fresh stock top, and the height of a full-height ridge.
const STOCK_TOP_Z: f64 = 2.0;
/// What the previous operation left everywhere except the ridge.
const FLOOR_Z: f64 = 0.0;
/// Where the finish pass cuts — 0.5 mm above the cleared floor, so the
/// plunge's own column reads AIR to a centerline probe. That is what makes
/// these fixtures the measured class rather than an ordinary plunge.
const CUT_Z: f64 = 0.5;
/// The plane the links retract to.
const SAFE_Z: f64 = 12.0;
/// The production tolerance `compute::execute` passes at step 7.
const TOLERANCE_MM: f64 = 0.1;

/// The wanaka finish tool's shape class: Ø3 ball tip on a 10° taper to a Ø6
/// shaft. Envelope radius 3 mm; `height_at_radius(0.859) = 0.270`,
/// `height_at_radius(1.859) = 3.405`.
fn finish_taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(3.0, 10.0, 6.0, 25.0)
}

/// Same envelope radius, no flank: `height_at_radius(r) = 0` for every r
/// inside 3 mm. The control tool for test 3.
fn flat_same_envelope() -> FlatEndmill {
    FlatEndmill::new(6.0, 25.0)
}

/// A 20 × 20 board machined down to `FLOOR_Z` everywhere except a ridge
/// spanning `min_x ..= max_x`, left standing at `ridge_top_z`.
///
/// Built with `clear_above_at`, which lowers the sliver-safe
/// `conservative_top` bound along with the ray — the state every production
/// stamping route leaves, and the one the clearance query reads.
fn ridge_stock(min_x: f64, max_x: f64, ridge_top_z: f64) -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, STOCK_TOP_Z, CELL);
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let origin_u = stock.z_grid.origin_u;
    let cs = stock.z_grid.cell_size;
    for row in 0..rows {
        for col in 0..cols {
            let x = origin_u + col as f64 * cs;
            let top = if (min_x..=max_x).contains(&x) {
                ridge_top_z
            } else {
                FLOOR_Z
            };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// The raster link shape at one plunge: rapid to the plunge XY at `SAFE_Z`,
/// fed `EntryPlunge` down to `CUT_Z`, one cutting pass parallel to the ridge,
/// retract. Move 1 is the plunge under test.
fn link_and_pass(x: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(x, 6.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(x, 6.0, CUT_Z), 300.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(x, 14.0, CUT_Z), 900.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(x, 14.0, SAFE_Z), MoveIntent::Retract);
    tp
}

fn filtered(tp: &Toolpath, stock: &TriDexelStock, cutter: &dyn MillingCutter) -> Toolpath {
    without_provenance(filter_air_cuts(
        AnnotatedToolpath::new(tp.clone()),
        stock,
        cutter,
        SAFE_Z,
        TOLERANCE_MM,
        AirBridgePolicy::Always,
    ))
    .toolpath
}

/// What a zero-radius probe sees at `(x, y)`: the top of that one column.
/// Used to prove a fixture really is the blind class before asserting the fix.
fn centerline_top_z(stock: &TriDexelStock, x: f64, y: f64) -> f64 {
    let (row, col) = stock
        .z_grid
        .world_to_cell(x, y)
        .expect("sample is on the grid");
    stock
        .z_grid
        .top_z_at(row, col)
        .map_or(f64::NEG_INFINITY, f64::from)
}

fn is_fed(tp: &Toolpath, idx: usize) -> bool {
    matches!(tp.moves[idx].move_type, MoveType::Linear { .. })
}

fn all_rapid(tp: &Toolpath) -> bool {
    tp.moves.iter().all(|m| m.move_type == MoveType::Rapid)
}

/// THE MEASURED CLASS — red against the pre-S3 point probe.
///
/// The plunge column is machined to `FLOOR_Z`, so a centerline probe reads
/// 0.5 mm of air under the tip and the pre-fix filter converted the whole fed
/// plunge to a rapid. A full-height ridge stands 1.0 mm off-axis: the taper's
/// profile there is only 0.270 mm above its tip, so the tip must stand at
/// 1.730 to clear it and the commanded 0.5 drives the flank 1.230 mm into
/// hardwood — 12× the filter's tolerance.
#[test]
fn a_plunge_whose_flank_strikes_an_off_axis_ridge_stays_fed() {
    let cutter = finish_taper();
    let stock = ridge_stock(11.0, 12.2, STOCK_TOP_Z);
    let tp = link_and_pass(10.0);

    assert!(
        centerline_top_z(&stock, 10.0, 6.0) + TOLERANCE_MM < CUT_Z,
        "fixture check: the plunge's own column must read AIR to a \
         zero-radius probe, or this is not the class S1 measured"
    );

    let result = filtered(&tp, &stock, &cutter);

    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "nothing may be dropped: every move of this link contacts the ridge \
         somewhere under the envelope"
    );
    assert!(
        is_fed(&result, 1),
        "the EntryPlunge must still be a FED move — converting it is the S1 \
         defect: the replacement rapid descends to the resume Z with zero \
         clearance while the ridge stands 1.230 mm in the tool's way"
    );
    assert_eq!(
        result.moves[1].intent,
        MoveIntent::EntryPlunge,
        "a surviving move keeps its intent"
    );
    assert!(
        is_fed(&result, 2),
        "the cutting pass runs parallel to the ridge at the same offset, so \
         it is material for its whole length too"
    );
}

/// EFFICIENCY GUARD — the fix must not cost travel on genuinely clear air.
///
/// The same link over the same floor, with the ridge moved to 2.7 mm
/// off-axis. There the taper stands 7.38 mm above its tip, so a 2 mm ridge
/// passes underneath with room to spare and the binding constraint is the
/// machined floor at 0.0 — 0.4 mm below the tip, 4× the tolerance. The link
/// must still collapse to rapid travel.
#[test]
fn a_link_the_whole_tool_clears_still_becomes_rapid_travel() {
    let cutter = finish_taper();
    let stock = ridge_stock(12.7, 13.9, STOCK_TOP_Z);
    let tp = link_and_pass(10.0);

    let result = filtered(&tp, &stock, &cutter);

    assert!(
        result.moves.len() < tp.moves.len(),
        "the fed moves are in air for the whole tool and must be dropped: {} \
         moves out of {}",
        result.moves.len(),
        tp.moves.len()
    );
    assert!(
        all_rapid(&result),
        "no fed move may survive an all-air link — trading the false negative \
         for lost travel is not a fix"
    );
}

/// PROFILE PRECISION — the query must read the tool's SHAPE, not its bounding
/// cylinder.
///
/// One geometry, two tools of the same 3 mm envelope radius: a 1.0 mm ridge
/// 2.0 mm off-axis. The taper's flank stands 3.405 mm above its tip there, so
/// the ridge passes under it and the link is genuinely air. A flat endmill of
/// the same envelope has no flank at all — `height_at_radius` is 0 across the
/// disc — so the same ridge stands 0.5 mm in ITS way and the same link must
/// stay fed.
///
/// An envelope-only ("highest material anywhere under the tool") query would
/// answer "material" for both and quietly cost the taper its link; a
/// centerline query answers "air" for both and puts the flat endmill through
/// the ridge. Only the profile separates them.
#[test]
fn the_taper_clears_a_low_ridge_the_flat_endmill_of_the_same_envelope_does_not() {
    let stock = ridge_stock(11.9, 12.5, 1.0);
    let tp = link_and_pass(10.0);

    let taper_result = filtered(&tp, &stock, &finish_taper());
    assert!(
        all_rapid(&taper_result),
        "the taper's flank stands 3.405 mm above its tip 1.859 mm out; a \
         1.0 mm ridge cannot reach it"
    );

    let flat_result = filtered(&tp, &stock, &flat_same_envelope());
    assert_eq!(
        flat_result.moves.len(),
        tp.moves.len(),
        "the flat endmill's bottom is flat across the whole envelope, so the \
         ridge is 0.5 mm in its way and nothing may be dropped"
    );
    assert!(
        is_fed(&flat_result, 1),
        "same geometry, same envelope radius, opposite verdict — that \
         difference IS the profile query"
    );
}

/// END TO END — the emitted motion, judged by S2's detector.
///
/// Two raster runs either side of a standing ridge, linked by the shape S1
/// read out of the shipped G-code: retract, hop, descend, resume. Filtered
/// with the S3 classifier, both plunges stay fed and the replay reports zero
/// rapid-through-stock collisions. The pre-S3 emission is rebuilt by hand
/// (the two plunges as rapids, which is exactly what the filter used to leave
/// behind) and the same detector flags it — so the sentry is not vacuous, and
/// emitter and detector now agree on one fixture instead of masking each
/// other.
#[test]
fn the_filtered_link_replays_with_no_rapid_collisions() {
    let cutter = finish_taper();
    let stock = ridge_stock(11.0, 12.2, STOCK_TOP_Z);

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(10.0, 6.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(10.0, 6.0, CUT_Z), 300.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(10.0, 14.0, CUT_Z), 900.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(10.0, 14.0, SAFE_Z), MoveIntent::Retract);
    tp.rapid_to_with_intent(P3::new(10.6, 14.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(10.6, 14.0, CUT_Z), 300.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(10.6, 6.0, CUT_Z), 900.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(10.6, 6.0, SAFE_Z), MoveIntent::Retract);

    let result = filtered(&tp, &stock, &cutter);
    assert_eq!(
        result.moves.len(),
        tp.moves.len(),
        "both plunges sit within the ridge's reach and must survive"
    );
    assert!(
        is_fed(&result, 1) && is_fed(&result, 5),
        "both link descents must be fed moves after the filter"
    );

    let mut replay = ridge_stock(11.0, 12.2, STOCK_TOP_Z);
    assert_eq!(
        live_rapid_hits(&mut replay, &result, &cutter),
        Vec::<usize>::new(),
        "with both plunges fed, no rapid of this link enters material"
    );

    // The pre-S3 emission: the filter dropped each fed plunge and left a rapid
    // descending to the resume Z in its place.
    let mut pre_fix = Toolpath::new();
    for (i, m) in tp.moves.iter().enumerate() {
        if i == 1 || i == 5 {
            pre_fix.rapid_to_with_intent(m.target, MoveIntent::Linking);
        } else {
            pre_fix.moves.push(m.clone());
        }
    }
    let mut pre_fix_stock = ridge_stock(11.0, 12.2, STOCK_TOP_Z);
    let hits = live_rapid_hits(&mut pre_fix_stock, &pre_fix, &cutter);
    assert!(
        hits.contains(&1),
        "vacuity guard: the emission this fix replaces must be caught by S2's \
         detector — the first descent meets an untouched ridge 1.230 mm \
         inside the tool. hits: {hits:?}"
    );
}

/// S2's live, profile-aware rapid check over the metric replay walk — the
/// production seam — returning the move indices it flagged.
fn live_rapid_hits(
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
