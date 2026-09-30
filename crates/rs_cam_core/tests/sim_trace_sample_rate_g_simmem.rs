//! G-SIMMEM (2026-09-30) — the cut trace records at its declared sample step.
//!
//! The operator's 350 x 500 mm terrain project took the desktop down twice
//! (2.3 GB -> 19.4 GB in 30 s at 0.5 mm cells). The growth was the cut trace:
//! the metric walk pushed one 280-byte `SimulationCutSample` per STAMPING
//! subsegment, and a sloped move is stamped in `⌈|Δz| / 0.02 mm⌉` of them, so
//! a terrain finish carried 5-6 samples per declared `sample_step_mm`
//! (rivmap100 scallop: 1.66 M samples against 0.29 M by length; the 350 mm
//! repro asked for one 9.5 GB allocation). See
//! `planning/sim_memory_2026-09-30/RESULTS.md`.
//!
//! The pin: on a steep fixture, under EVERY stamp dispatch, a toolpath's
//! trace holds at most `Σ max(1, ⌈len / step⌉)` samples — the count its
//! length and the declared step ask for, independent of `|Δz|` — while the
//! quantities its consumers total are the ones the stock and the clock say:
//! the removed volume is the volume the stock lost, and the last sample's
//! clock is the path's own feed time. Red before the fix (the count was
//! `Σ max(⌈len/step⌉, ⌈|Δz|/0.02⌉)`, 25x the bound on these 45° ramps).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StampDispatch, StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::dexel::ray_material_length;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const CELL: f64 = 0.5;
/// The step `compute::simulate` derives from the cell: `max(cell, 0.25)`.
const STEP: f64 = 0.5;
const FEED: f64 = 1200.0;
const RAPID: f64 = 5000.0;
const TOP: f64 = 0.0;

/// Twelve lanes of 45° ramps down to -8 mm and back up: every cutting move is
/// Z-limited (`|Δz| / 0.02 = 400` stamps against `⌈11.3 / 0.5⌉ = 23` by
/// length).
fn steep_raster() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, TOP + 5.0));
    tp.rapid_to(P3::new(4.0, 4.0, TOP + 0.5));
    for lane in 0..12 {
        let y = 4.0 + 2.5 * lane as f64;
        if lane > 0 {
            tp.feed_to(P3::new(4.0, y, TOP + 0.5), FEED);
        }
        tp.feed_to(P3::new(12.0, y, TOP - 8.0 + 0.5), FEED);
        tp.feed_to(P3::new(20.0, y, TOP + 0.5), FEED);
        tp.feed_to(P3::new(28.0, y, TOP - 8.0 + 0.5), FEED);
        tp.feed_to(P3::new(36.0, y, TOP + 0.5), FEED);
        tp.rapid_to(P3::new(36.0, y, TOP + 5.0));
        tp.rapid_to(P3::new(4.0, y + 2.5, TOP + 5.0));
        tp.rapid_to(P3::new(4.0, y + 2.5, TOP + 0.5));
    }
    tp
}

/// `Σ max(1, ⌈len / step⌉)` over the moves, and the path's feed time.
fn length_bound_and_time(tp: &Toolpath) -> (usize, f64) {
    let mut bound = 0usize;
    let mut time_s = 0.0;
    for pair in tp.moves.windows(2) {
        let len = (pair[1].target - pair[0].target).norm();
        if len <= 1e-9 {
            continue;
        }
        bound += ((len / STEP).ceil() as usize).max(1);
        let feed = match pair[1].move_type {
            MoveType::Rapid => RAPID,
            MoveType::Linear { feed_rate } => feed_rate,
            _ => panic!("the fixture has no arcs"),
        };
        time_s += len / feed * 60.0;
    }
    (bound, time_s)
}

fn material_volume(stock: &TriDexelStock) -> f64 {
    let grid = &stock.z_grid;
    let area = grid.cell_size * grid.cell_size;
    grid.rays
        .iter()
        .map(|ray| f64::from(ray_material_length(ray)) * area)
        .sum()
}

fn walk(dispatch: StampDispatch) -> (Vec<SimulationCutSample>, f64, Toolpath) {
    let cutter = BallEndmill::new(6.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 40.0, TOP - 12.0, TOP, CELL);
    stock.stamp_dispatch = dispatch;
    let before = material_volume(&stock);
    let tp = steep_raster();
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            &tp,
            &lut,
            &cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(1),
            18_000,
            2,
            RAPID,
            STEP,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            None,
        )
        .expect("never cancelled");
    let removed_by_stock = before - material_volume(&stock);
    (samples, removed_by_stock, tp)
}

#[test]
fn a_steep_toolpath_keeps_the_sample_count_its_length_asks_for() {
    for dispatch in [
        StampDispatch::Swept,
        StampDispatch::WholeToolpath,
        StampDispatch::PerStamp,
    ] {
        let (samples, removed_by_stock, tp) = walk(dispatch);
        let (bound, feed_time_s) = length_bound_and_time(&tp);

        // The structure that grew: bounded by length and step, not by |Δz|.
        assert!(
            samples.len() <= bound,
            "{dispatch:?}: {} samples for a path whose length at a {STEP} mm step asks for \
             at most {bound} — the stamping subdivision is leaking into the trace",
            samples.len()
        );
        let bytes = samples.len() * std::mem::size_of::<SimulationCutSample>();
        assert!(
            bytes <= bound * std::mem::size_of::<SimulationCutSample>(),
            "{dispatch:?}: {bytes} trace bytes"
        );
        // Non-vacuous: the fixture really is Z-limited.
        let cutting = samples.iter().filter(|s| s.is_cutting).count();
        assert!(
            cutting > 100,
            "{dispatch:?}: only {cutting} cutting samples"
        );

        // A dense, renumbered index.
        for (i, s) in samples.iter().enumerate() {
            assert_eq!(s.sample_index, i, "{dispatch:?}");
        }

        // What the consumers total is kept: the clock and the volume.
        let clock = samples.last().unwrap().cumulative_time_s;
        let summed: f64 = samples.iter().map(|s| s.segment_time_s).sum();
        assert!(
            (clock - feed_time_s).abs() < 1e-9 * feed_time_s,
            "{dispatch:?}: clock {clock} vs path time {feed_time_s}"
        );
        assert!((summed - feed_time_s).abs() < 1e-9 * feed_time_s);
        let removed: f64 = samples.iter().map(|s| s.removed_volume_est_mm3).sum();
        assert!(removed_by_stock > 1000.0, "the fixture must cut");
        assert!(
            (removed - removed_by_stock).abs() < 1e-3 * removed_by_stock,
            "{dispatch:?}: trace says {removed} mm³ removed, the stock lost {removed_by_stock}"
        );
    }
}
