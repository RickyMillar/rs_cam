//! A/M10 — a rapid collision must be a property of the TOOLPATH, not of the
//! grid someone happened to verify it on.
//!
//! Oracle: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M10, and the TP15 RCA (`4f590f3`).
//!
//! ## The coupling this file exists to break
//!
//! `dressup::optimize_entry_descents` lowers a safe-Z-to-cut-depth plunge by
//! rapiding down to just above the input stock's material ceiling. It reads
//! that ceiling from `TriDexelStock::max_top_z_in_disc` on the
//! **generation-time snapshot** — whose cell size is a side effect of
//! whatever resolution the last simulation ran at (`session/compute.rs`
//! resolves `gen_initial_stock` from `sim.prior_stocks`; nothing passes a
//! resolution). The collision detector then walks the rapid against a
//! **different** grid, sized by *this* simulation's `request.resolution`.
//!
//! Two grids, two independently-chosen cell sizes, and a strict `pz <
//! stock_top` predicate with no tolerance. On wanaka the same generated
//! chain measured **0 / 15 / 20** rapid collisions at 0.5 / 0.25 / 0.1 mm.
//! Nothing about the toolpath changed between those three numbers.
//!
//! ## Why padding cannot close it
//!
//! The shipped mitigation pads the descent target by `2 * cell_size`. That
//! bounds the *crest* class — a ridge top the coarse grid samples on its
//! flanks — because the error there scales with the cell. It cannot bound
//! the *sliver* class: an uncut rib narrower than one coarse cell, standing
//! at full stock height between two rough passes. Sub-cell coverage
//! (`ray_blend_above`) blends such a cell down toward the cut floor in
//! proportion to how much of it was swept, so the coarse grid reports a top
//! that is neither the floor nor the rib — while the rib itself is whatever
//! the original stock was, an unbounded distance above. `SLIVER` below is
//! that class, built to scale: no pad expressible in cells can reach it.
//!
//! ## What is pinned
//!
//! One toolpath, generated ONCE against the coarse snapshot, verified at
//! 0.5 / 0.25 / 0.1 mm. The collision count must be the same number at all
//! three. The standing rule — *never clear collisions across mismatched
//! resolutions* — is only enforceable if that number exists.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::collision::check_rapid_collisions_against_stock;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::optimize_entry_descents;
use rs_cam_core::geo::P3;
use rs_cam_core::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// The resolution ladder the TP15 RCA measured 0 / 15 / 20 on.
const LADDER: [f64; 3] = [0.5, 0.25, 0.1];

/// The resolution the toolpath is PLANNED against — the coarse end of the
/// ladder, matching `SimulationOptions::default().resolution`.
const PLANNING_CELL: f64 = 0.5;

// ── The fixture: two rough swaths with an uncut rib between them ────────
//
// Stock 0..24 x 0..24 x 0..20. Two flat-endmill swaths of radius 2.0 run
// along +Y at x = 7.85 and x = 12.15, each cutting to z = 2.0. Swath 1
// clears x ∈ [5.85, 9.85]; swath 2 clears x ∈ [10.15, 14.15]. Between them
// stands a rib 0.30 mm wide and 18 mm tall, at full stock height.
//
// 0.30 mm is under one 0.5 mm cell and over two 0.1 mm cells: the coarse
// grid can only ever see the rib as partial coverage on the cell that
// straddles it, the fine grid resolves it outright. That is the whole
// mechanism, with no mesh, no operation and no session in the way.

const STOCK_TOP_Z: f64 = 20.0;
const CUT_FLOOR_Z: f64 = 2.0;
const RIB_X: f64 = 10.0;
const RIB_Y: f64 = 12.0;
const SWATH_RADIUS: f64 = 2.0;
const SAFE_Z: f64 = 25.0;

/// Radius of the descending tool. Deliberately smaller than the rib's
/// distance to open ground: the ceiling disc must see ONLY swath floors and
/// the rib, never the untouched stock outside the swaths — otherwise the
/// disc-max reads 20.0 for the trivial reason and the fixture proves
/// nothing.
const DESCENT_TOOL_RADIUS: f64 = 1.0;

fn stock_at(cell: f64) -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 24.0, 24.0, 0.0, STOCK_TOP_Z, cell);
    let cutter = FlatEndmill::new(SWATH_RADIUS * 2.0, 30.0);
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    for centre_x in [RIB_X - 0.15 - SWATH_RADIUS, RIB_X + 0.15 + SWATH_RADIUS] {
        stock.stamp_linear_segment(
            &lut,
            SWATH_RADIUS,
            P3::new(centre_x, 2.0, CUT_FLOOR_Z),
            P3::new(centre_x, 22.0, CUT_FLOOR_Z),
            StockCutDirection::FromTop,
        );
    }
    stock
}

/// A minimal entry: traverse at safe Z, then plunge to the cut floor at the
/// rib's XY. Shape matches every generator's shared emitter — a `Rapid`
/// followed by a same-XY `EntryPlunge`, which is exactly what
/// `optimize_entry_descents` looks for.
fn entry_over_rib() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.5, 0.5, SAFE_Z), MoveIntent::Linking);
    tp.rapid_to_with_intent(P3::new(RIB_X, RIB_Y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(RIB_X, RIB_Y, CUT_FLOOR_Z),
        500.0,
        MoveIntent::EntryPlunge,
    );
    tp
}

fn collisions_at(tp: &Toolpath, cell: f64) -> usize {
    check_rapid_collisions_against_stock(tp, &stock_at(cell).z_grid).len()
}

/// The Z the optimizer chose. Selecting on "the first `Linking` rapid" would
/// pick the safe-Z traverse that precedes it — both are `Rapid` + `Linking`,
/// and only the inserted one descends.
fn inserted_descent_z(tp: &Toolpath) -> f64 {
    tp.moves
        .iter()
        .find(|m| {
            m.move_type == MoveType::Rapid
                && m.intent == MoveIntent::Linking
                && m.target.z < SAFE_Z - 1e-9
        })
        .map(|m| m.target.z)
        .expect("an inserted descent below safe Z")
}

/// The measured top of the rib's column at each resolution — the quantity
/// the two grids disagree about, printed so a future reader can see the
/// disagreement rather than infer it from a collision count.
fn rib_top_at(cell: f64) -> f64 {
    let stock = stock_at(cell);
    let g = &stock.z_grid;
    let (row, col) = g
        .world_to_cell(RIB_X, RIB_Y)
        .expect("rib is inside the grid");
    f64::from(g.top_z_at(row, col).expect("rib column holds material"))
}

#[test]
fn the_fixture_actually_hides_the_rib_from_the_coarse_grid() {
    // Guard against a fixture that proves nothing: if the coarse grid
    // already reported the rib at full height there would be no divergence
    // to fix, and a later "stable" result would be vacuous.
    let coarse = rib_top_at(PLANNING_CELL);
    let fine = rib_top_at(0.1);
    println!("rib top: {PLANNING_CELL} mm -> {coarse:.3}, 0.1 mm -> {fine:.3}");

    assert!(
        (fine - STOCK_TOP_Z).abs() < 1e-3,
        "0.1 mm grid should resolve the 0.30 mm rib at full stock height \
         ({STOCK_TOP_Z}); got {fine:.3}. The fixture's rib is mis-sized."
    );
    assert!(
        coarse < fine - 3.0,
        "the {PLANNING_CELL} mm grid must under-read the rib by more than the \
         2-cell pad can cover, or this file tests nothing; coarse {coarse:.3} \
         vs fine {fine:.3}"
    );
}

#[test]
fn descent_planned_coarse_does_not_collide_at_any_verification_resolution() {
    let mut tp = entry_over_rib();
    let planning_stock = stock_at(PLANNING_CELL);
    let splits = optimize_entry_descents(
        &mut tp,
        Some(&planning_stock),
        STOCK_TOP_Z,
        DESCENT_TOOL_RADIUS,
    );
    assert_eq!(
        splits, 1,
        "the fixture must exercise the optimizer; it inserted no descent"
    );

    let descent_z = inserted_descent_z(&tp);

    let counts: Vec<usize> = LADDER.iter().map(|&c| collisions_at(&tp, c)).collect();
    println!(
        "A/M10 ladder — planned at {PLANNING_CELL} mm, descent target z = {descent_z:.3}\n  \
         collisions {:?} at {:?} mm",
        counts, LADDER
    );

    assert!(
        counts.iter().all(|&c| c == counts[0]),
        "A/M10: rapid collisions must not depend on the verification grid. \
         Got {counts:?} at {LADDER:?} mm on ONE toolpath planned against the \
         {PLANNING_CELL} mm snapshot. The descent target sits at \
         {descent_z:.3}; the rib the coarse grid smoothed away stands at \
         {STOCK_TOP_Z}."
    );

    // Stability at a wrong-but-consistent number would satisfy the letter of
    // the gate and none of its purpose.
    assert_eq!(
        counts[0], 0,
        "A/M10: the descent must clear the rib, not merely fail to notice it \
         consistently. Descent target {descent_z:.3} vs rib top {STOCK_TOP_Z}."
    );
}

#[test]
fn a_descent_over_swept_ground_still_descends() {
    // The counterweight to the gate above: a ceiling that is conservative
    // everywhere is trivially collision-free and useless. Over the middle of
    // a fully-swept swath — no rib, no partial cells within the disc — the
    // optimizer must still lower the rapid to near the cut floor.
    let mut tp = Toolpath::new();
    let x = RIB_X - 0.15 - SWATH_RADIUS;
    tp.rapid_to_with_intent(P3::new(0.5, 0.5, SAFE_Z), MoveIntent::Linking);
    tp.rapid_to_with_intent(P3::new(x, RIB_Y, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(
        P3::new(x, RIB_Y, CUT_FLOOR_Z),
        500.0,
        MoveIntent::EntryPlunge,
    );

    let planning_stock = stock_at(PLANNING_CELL);
    let splits = optimize_entry_descents(&mut tp, Some(&planning_stock), STOCK_TOP_Z, 0.5);
    assert_eq!(splits, 1, "a descent over swept ground must still be split");

    let descent_z = inserted_descent_z(&tp);
    println!("swept-ground descent target z = {descent_z:.3} (floor {CUT_FLOOR_Z})");
    assert!(
        descent_z < CUT_FLOOR_Z + 5.0,
        "conservatism must be local to the sliver: over fully-swept ground the \
         descent should land near the cut floor ({CUT_FLOOR_Z}), got {descent_z:.3}"
    );
}
