//! M8 (memory budget 2026-10-01): the metric walk with no sample record
//! carves the same stock as the walk that records every sample.
//!
//! Plan: `planning/memory_budget_2026-10-01/PLAN.md`, finding M8.
//!
//! # The defect
//!
//! With cutting metrics off, `compute/simulate.rs::carve_entry` still let
//! the metric walk build every `SimulationCutSample` (280 bytes, plus one
//! heap copy of its `span_path`) and reserve the estimated count, and then
//! dropped them. `TriDexelStock::simulate_toolpath_with_lut_metric_walk`
//! now takes `record_samples`; with `false` it builds and reserves nothing.
//!
//! # What this file proves
//!
//! 1. The unrecorded walk returns zero samples. The recorded walk returns
//!    more than zero, so the comparison is not vacuous.
//! 2. The two walks leave the same stock, bit for bit: the snapshot hash,
//!    every Z ray bound and every `conservative_top` value.
//! 3. The two walks use the same stamp schedule (`last_stamp_dispatch`) and
//!    report the same rapid strikes.
//!
//! Each claim holds under every `StampDispatch` shape. The swept shapes
//! read the slot numbers to grow a chunk, so they are the ones that fail
//! if the unrecorded walk hands out different slots.
//!
//! # The fixture
//!
//! A 40 x 30 x 12 mm stock at a 0.5 mm cell: raster lines, a ramp, a slope,
//! twelve 9 mm plunges, a CW and a CCW arc, a `Retract`-tagged feed and a
//! rapid that strikes uncut stock (the S2 rapid check flushes the queues
//! there). A 9 mm plunge at a 0.25 mm sample step is 450 stamps by the
//! 0.02 mm Z cap and 36 by length, so the coalescer drops 450 - 35 = 415
//! samples per plunge. Twelve plunges drop about 5000 samples. The
//! coalescer flushes when the drop reaches one grid of bytes in samples:
//! 80 x 60 = 4800 columns of at least 28 bytes over 280 bytes is about 480.
//! The mid-walk coalescer flush therefore runs several times in this walk.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test unrecorded_metric_walk_carves_the_same_m8
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::toolpath_stats::StockSnapshotStamp;
use rs_cam_core::dexel_stock::{StampDispatch, StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const SAMPLE_STEP_MM: f64 = 0.25;
const CELL_MM: f64 = 0.5;

fn fixture() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, 14.0));
    for i in 0..6 {
        let y = 4.0 + 2.2 * f64::from(i);
        let (x0, x1) = if i % 2 == 0 { (4.0, 34.0) } else { (34.0, 4.0) };
        tp.feed_to(P3::new(x0, y, 10.0), 900.0);
        tp.feed_to(P3::new(x1, y, 10.0), 1200.0);
    }
    // A ramp and a slope: Z-subdivided lateral moves.
    tp.feed_to(P3::new(10.0, 10.0, 8.5), 600.0);
    tp.feed_to(P3::new(28.0, 14.0, 6.0), 600.0);
    tp.arc_cw_to(P3::new(20.0, 22.0, 6.0), -4.0, 4.0, 900.0);
    tp.arc_ccw_to(P3::new(12.0, 14.0, 6.0), -4.0, -4.0, 900.0);
    // A lift tagged `Retract`: no stamp, and no sample in either walk's
    // carve.
    tp.feed_to_with_intent(P3::new(12.0, 14.0, 14.0), 900.0, MoveIntent::Retract);
    // A rapid down into uncut stock: the S2 rapid check strikes it.
    tp.rapid_to(P3::new(36.0, 26.0, 11.0));
    tp.rapid_to(P3::new(36.0, 26.0, 14.0));
    // Twelve 9 mm plunges: the coalescer's mid-walk flush.
    for i in 0..12 {
        let x = 5.0 + 5.0 * f64::from(i % 6);
        let y = 7.0 + 12.0 * f64::from(i / 6);
        tp.rapid_to(P3::new(x, y, 14.0));
        tp.feed_to(P3::new(x, y, 3.0), 300.0);
        tp.feed_to(P3::new(x, y, 14.0), 900.0);
    }
    tp.final_retract(14.0);
    tp
}

struct Walk {
    stock: TriDexelStock,
    samples: Vec<SimulationCutSample>,
    rapid_strikes: Vec<usize>,
}

fn walk(
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    dispatch: StampDispatch,
    record_samples: bool,
) -> Walk {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, CELL_MM);
    stock.stamp_dispatch = dispatch;
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut rapid_check = RapidClearanceCheck::new(cutter);
    let samples = stock
        .simulate_toolpath_with_lut_metric_walk(
            tp,
            &lut,
            cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            SAMPLE_STEP_MM,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            Some(&mut rapid_check),
            record_samples,
        )
        .expect("never cancelled");
    let rapid_strikes = rapid_check.hits().iter().map(|h| h.move_index).collect();
    Walk {
        stock,
        samples,
        rapid_strikes,
    }
}

/// The first Z ray whose bounds differ by any bit, or `None`.
fn first_ray_difference(a: &TriDexelStock, b: &TriDexelStock) -> Option<usize> {
    if a.z_grid.rays.len() != b.z_grid.rays.len() {
        return Some(0);
    }
    a.z_grid
        .rays
        .iter()
        .zip(&b.z_grid.rays)
        .position(|(ra, rb)| {
            ra.len() != rb.len()
                || ra.iter().zip(rb.iter()).any(|(sa, sb)| {
                    sa.enter.to_bits() != sb.enter.to_bits()
                        || sa.exit.to_bits() != sb.exit.to_bits()
                })
        })
}

fn top_bits(stock: &TriDexelStock) -> Vec<u32> {
    stock
        .z_grid
        .conservative_top
        .iter()
        .map(|t| t.to_bits())
        .collect()
}

fn assert_same_carve(cutter: &dyn MillingCutter, label: &str) {
    let tp = fixture();
    for dispatch in [
        StampDispatch::Auto,
        StampDispatch::PerStamp,
        StampDispatch::WholeToolpath,
        StampDispatch::Swept,
        StampDispatch::SweptPlungeOnly,
    ] {
        let recorded = walk(&tp, cutter, dispatch, true);
        let unrecorded = walk(&tp, cutter, dispatch, false);

        assert!(
            !recorded.samples.is_empty(),
            "{label} {dispatch:?}: the recording walk returned no samples, \
             so the comparison is vacuous"
        );
        assert!(
            unrecorded.samples.is_empty(),
            "{label} {dispatch:?}: the unrecorded walk returned {} samples",
            unrecorded.samples.len()
        );
        assert_eq!(
            first_ray_difference(&recorded.stock, &unrecorded.stock),
            None,
            "{label} {dispatch:?}: a Z ray differs between the two walks"
        );
        assert!(
            top_bits(&recorded.stock) == top_bits(&unrecorded.stock),
            "{label} {dispatch:?}: conservative_top differs between the two walks"
        );
        assert_eq!(
            StockSnapshotStamp::of(&recorded.stock),
            StockSnapshotStamp::of(&unrecorded.stock),
            "{label} {dispatch:?}: the snapshot hash differs"
        );
        assert_eq!(
            recorded.stock.last_stamp_dispatch, unrecorded.stock.last_stamp_dispatch,
            "{label} {dispatch:?}: the stamp schedule differs"
        );
        assert!(
            !recorded.rapid_strikes.is_empty(),
            "{label} {dispatch:?}: the fixture rapid did not strike, so the \
             rapid-check flush is not exercised"
        );
        assert_eq!(
            recorded.rapid_strikes, unrecorded.rapid_strikes,
            "{label} {dispatch:?}: the rapid strikes differ"
        );
    }
}

#[test]
fn the_unrecorded_walk_carves_the_same_stock_with_a_flat_end_mill() {
    let cutter = FlatEndmill::new(6.0, 20.0);
    assert_same_carve(&cutter, "flat 6 mm");
}

#[test]
fn the_unrecorded_walk_carves_the_same_stock_with_a_ball_end_mill() {
    let cutter = BallEndmill::new(6.0, 20.0);
    assert_same_carve(&cutter, "ball 6 mm");
}
