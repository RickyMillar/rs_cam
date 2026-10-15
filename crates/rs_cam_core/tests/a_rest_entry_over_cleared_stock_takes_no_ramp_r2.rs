//! Operator ruling 2026-10-15, item 2 — a rest entry over cleared stock
//! takes no ramp.
//!
//! # The rule
//!
//! On a rest-driven pass the stock-aware entry stage plans a bite-budgeted
//! lap ladder (`finish::pencil::plan_entry_ramp`) where material stands
//! over the entry. The ladder reads the stock ceiling over the whole lap
//! window. A run whose START point has no material above its entry Z (in
//! the reach of the tip: the W6.1 `standing_near` band, `R + cell`, at
//! `z + FRESH_MATERIAL_THRESHOLD_MM`) needs no ladder: the tool goes
//! straight down through air, as the fresh op does. A run that starts in
//! material keeps its ladder.
//!
//! # The fixture
//!
//! A board cleared to `FLOOR_Z` for `x < WALL_X` and standing at
//! `STOCK_TOP_Z` from `WALL_X` on. Each case is the session's post-clip
//! shape: a rapid to `SAFE_Z`, an `EntryPlunge` to the floor, and a pass
//! toward the wall. The entry stage is `dressup::optimize_entry_descents`
//! with a `RestEntryRamp`, the door every rest surface pass takes.
//!
//! - Over cleared stock, the start point lies `GAP_MM` from the wall: out
//!   of the `R + cell` band, but the lap window reaches material, so the
//!   ladder planner without the rule plans laps. Red without the rule.
//! - In material, the start point lies past the wall: the ladder stays.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::{RestEntryRamp, optimize_entry_descents};
use rs_cam_core::geo::P3;
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// Grid cell (mm).
const CELL: f64 = 0.25;
/// The uncut stock top (mm).
const STOCK_TOP_Z: f64 = 5.0;
/// The floor an upstream op left for `x < WALL_X` (mm).
const FLOOR_Z: f64 = 0.0;
/// The X of the wall of standing material (mm).
const WALL_X: f64 = 20.0;
/// The retract plane (mm).
const SAFE_Z: f64 = 15.0;
/// The ball diameter (mm): tip radius 1.5.
const TOOL_D: f64 = 3.0;

fn stock() -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 20.0, -5.0, STOCK_TOP_Z, CELL);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (origin_u, cs) = (stock.z_grid.origin_u, stock.z_grid.cell_size);
    for row in 0..rows {
        for col in 0..cols {
            let x = origin_u + col as f64 * cs;
            if x < WALL_X {
                stock.clear_above_at(row, col, FLOOR_Z as f32);
            }
        }
    }
    stock
}

/// The session's post-clip shape at `x`: rapid to `SAFE_Z`, plunge to the
/// floor, a pass of 10 mm toward +X at the floor in 0.25 mm points.
fn entry_at(x: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(x, 10.0, SAFE_Z), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(x, 10.0, FLOOR_Z), 300.0, MoveIntent::EntryPlunge);
    for k in 1..=40 {
        tp.feed_to_with_intent(
            P3::new(x + 0.25 * k as f64, 10.0, FLOOR_Z),
            1000.0,
            MoveIntent::FinishingCut,
        );
    }
    tp
}

/// Run the rest entry stage on `entry_at(x)`; return the count of
/// `EntryRamp` moves it emits.
fn ramp_moves_at(x: f64) -> usize {
    let stock = stock();
    let cutter = BallEndmill::new(TOOL_D, 25.0);
    let ramp = RestEntryRamp {
        contact_radius_mm: rs_cam_core::finish::pencil::tip_contact_radius(&cutter),
        feed_rate: 1000.0,
        plunge_rate: 300.0,
    };
    let mut tp = entry_at(x);
    let _ = optimize_entry_descents(
        &mut tp,
        Some(&stock),
        STOCK_TOP_Z,
        cutter.radius(),
        &cutter,
        Some(&ramp),
    );
    tp.moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryRamp)
        .count()
}

/// The start point lies this far from the wall (mm): beyond the tip's
/// `R + cell` band (1.75 mm), and inside the lap window plus the tip disc,
/// so the window's ceiling reads the wall top.
const GAP_MM: f64 = 2.25;

#[test]
fn a_rest_run_that_starts_over_cleared_stock_descends_straight() {
    let ramps = ramp_moves_at(WALL_X - GAP_MM);
    assert_eq!(
        ramps, 0,
        "the start point stands over cleared stock, so the entry is a straight \
         descent, not {ramps} ramp moves"
    );
}

#[test]
fn a_rest_run_that_starts_in_material_keeps_its_ramp() {
    let ramps = ramp_moves_at(WALL_X + 3.0);
    assert!(
        ramps > 0,
        "the start point stands under 5 mm of material, so the ladder stays"
    );
}
