//! S2 probe — does `check_rapid_collisions_against_stock` flag the wanaka
//! finish-link shape?
//!
//! S1 (`planning/rapid_safety_2026-08-28/S1_RESULTS.md`) measured 601/601
//! link descents in op 7 entering standing rest material, yet the pipeline's
//! `rapid_collision_count` reads 0 — including on a fresh CLI run at HEAD
//! whose own emitted G-code contains the identical zero-clearance descent
//! (`G0 X168.200 Y220.000 Z12.000` → `G0 … Z2.211` while post-rough stock
//! stands ≥ 2.7 there). The exporter maps `G0` strictly from
//! `MoveType::Rapid` (`gcode/program_builder.rs:74-83`), so the stored
//! motion provably holds these rapids.
//!
//! This test feeds the minimal stored-move shape of one such link —
//! feed → vertical retract → traverse hop → vertical descent to the resume
//! Z — over a stock whose top stands 0.5 mm above the descent target,
//! directly into the checker.
//!
//! OUTCOME (2026-08-28): both tests PASS — the checker, its F3 walk-back,
//! timing and frame are healthy on exactly this shape. The pipeline's
//! silence was resolved the same day (S1_RESULTS.md §3): the real strikes
//! are 0.5–2.9 mm OFF-AXIS (inter-pass crests at rough swath edges), which
//! the zero-radius point probe cannot see (S-b proper). These tests stay as
//! sentries pinning the healthy behaviours the S2 fix must preserve: a
//! descent below the column top flags, a vertical retract never does.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P3;
use rs_cam_core::stock::collision::check_rapid_collisions_against_stock;
use rs_cam_core::toolpath::Toolpath;

/// Stock top at Z = 2.7 (post-rough: design 2.2 + 0.5 leave), descent to
/// Z = 2.211 — the S1 op-7 worst-strike geometry, scaled to a 20 mm board.
const STOCK_TOP_Z: f64 = 2.7;
const RESUME_Z: f64 = 2.211;
const SAFE_Z: f64 = 12.0;

fn wanaka_link_toolpath() -> Toolpath {
    let mut tp = Toolpath::new();
    // Establish position on the surface being cut (the raster run).
    tp.feed_to(P3::new(4.0, 5.0, 2.212), 778.0);
    tp.feed_to(P3::new(5.0, 5.0, 2.212), 778.0);
    // The link: vertical retract, 1.2 mm hop, vertical descent to resume Z.
    tp.rapid_to(P3::new(5.0, 5.0, SAFE_Z));
    tp.rapid_to(P3::new(6.2, 5.0, SAFE_Z));
    tp.rapid_to(P3::new(6.2, 5.0, RESUME_Z));
    // Resume cutting.
    tp.feed_to(P3::new(7.0, 5.0, 2.210), 778.0);
    tp
}

#[test]
fn the_wanaka_link_descent_is_flagged_against_standing_stock() {
    // Full box of material topping out 0.489 mm ABOVE the descent target —
    // the checker's grid is a pre-carve snapshot, exactly as in simulate.rs.
    let stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, STOCK_TOP_Z, 0.3);
    let tp = wanaka_link_toolpath();

    let hits = check_rapid_collisions_against_stock(&tp, &stock.z_grid);

    // Move indices: 0,1 feeds; 2 retract; 3 hop; 4 descent; 5 feed.
    assert!(
        !hits.iter().any(|c| c.move_index == 2),
        "vertical retract must not be flagged (S1 v2 monotone-profile proof)"
    );
    assert!(
        hits.iter().any(|c| c.move_index == 4),
        "descent to Z{RESUME_Z} under a Z{STOCK_TOP_Z} stock top must be \
         flagged; checker returned {hits:?}"
    );
}

#[test]
fn the_hop_traverse_below_stock_top_is_flagged_too() {
    // Same shape but the whole link runs below the stock top (a traverse
    // through material) — the unambiguous case; if THIS fails the checker
    // is not consulting the grid at all.
    let stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, -5.0, STOCK_TOP_Z, 0.3);
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(5.0, 5.0, 1.0), 778.0);
    tp.rapid_to(P3::new(12.0, 5.0, 1.0));

    let hits = check_rapid_collisions_against_stock(&tp, &stock.z_grid);
    assert!(
        hits.iter().any(|c| c.move_index == 1),
        "a lateral rapid 1.7 mm under the stock top must be flagged; \
         checker returned {hits:?}"
    );
}
