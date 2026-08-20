//! S3 sentry: row-band parallel stamping must not depend on the thread count.
//!
//! `PERF_REVIEW.md` S3 replaces the single-threaded stamp loop with a row-band
//! decomposition. Two properties have to hold, and only one of them is about
//! speed:
//!
//! 1. **Per-cell mutation order is preserved.** `dexel::ray_blend_above` with
//!    `f < 1` is not commutative, so two stamps landing on one cell in the
//!    wrong order leave a plausible-looking surface that is wrong in a way no
//!    aggregate will show. Row bands give this for free — a cell belongs to
//!    exactly one band — but "for free" is a claim, and this is where it is
//!    checked.
//! 2. **The result does not move with the machine.** The band boundaries are a
//!    function of the grid's row count, never of `current_num_threads()`, so
//!    the same input must produce the same grid and the same sample stream at
//!    1 thread and at N. If they were thread-derived, the reassociated volume
//!    sum would differ between a 4-core box and a 32-core one and every golden
//!    that pins it would be un-reproducible.
//!
//! Everything here is compared as **bit patterns** (`to_bits`), never with
//! `==` and never with a tolerance: a last-ULP divergence is precisely the
//! signature this test exists to catch, and it is how wave 1 found G3's
//! unsound bbox reject and S7's false exactness claim.
//!
//! ```text
//! cargo test -p rs_cam_core --test band_stamping_determinism_s3
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;

/// A pass that mixes everything the band split has to survive: overlapping
/// raster lines (adjacent bands touching the same rows), a ramp (a tip that
/// varies along the segment), a plunge (the degenerate branch), an arc, and a
/// re-pass over already-cut ground (the S2 early-out interacting with the
/// bands).
fn mixed_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, 12.0));
    for i in 0..8 {
        let y = 4.0 + 2.2 * i as f64;
        let (x0, x1) = if i % 2 == 0 { (4.0, 34.0) } else { (34.0, 4.0) };
        tp.feed_to(P3::new(x0, y, 8.0), 900.0);
        tp.feed_to(P3::new(x1, y, 8.0), 1200.0);
    }
    // A ramp down, then a plunge, then a re-pass at the same depth.
    tp.feed_to(P3::new(10.0, 10.0, 6.5), 600.0);
    tp.feed_to(P3::new(28.0, 14.0, 5.0), 600.0);
    tp.feed_to(P3::new(28.0, 14.0, 3.0), 200.0);
    tp.feed_to(P3::new(10.0, 14.0, 3.0), 900.0);
    tp.feed_to(P3::new(10.0, 14.0, 3.0), 900.0);
    // An arc, so the `MoveType::ArcCW` linearisation is banded too.
    tp.arc_cw_to(P3::new(20.0, 24.0, 3.0), 5.0, 5.0, 900.0);
    tp.final_retract(12.0);
    tp
}

fn simulate(
    cutter: &dyn MillingCutter,
    cell_size: f64,
) -> (TriDexelStock, Vec<SimulationCutSample>) {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, cell_size);
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let tp = mixed_pass();
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_lut_metrics_cancel(
            &tp,
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
        )
        .expect("never cancelled");
    (stock, samples)
}

fn assert_grids_bit_identical(a: &TriDexelStock, b: &TriDexelStock, what: &str) {
    let (ga, gb) = (&a.z_grid, &b.z_grid);
    assert_eq!(ga.rays.len(), gb.rays.len(), "{what}: ray count");
    for (i, (ra, rb)) in ga.rays.iter().zip(gb.rays.iter()).enumerate() {
        assert_eq!(ra.len(), rb.len(), "{what}: cell {i} segment count");
        for (sa, sb) in ra.iter().zip(rb.iter()) {
            assert_eq!(
                (sa.enter.to_bits(), sa.exit.to_bits()),
                (sb.enter.to_bits(), sb.exit.to_bits()),
                "{what}: cell {i} segment bits"
            );
        }
    }
    for (i, (ca, cb)) in ga
        .conservative_top
        .iter()
        .zip(gb.conservative_top.iter())
        .enumerate()
    {
        assert_eq!(
            ca.to_bits(),
            cb.to_bits(),
            "{what}: conservative_top bits at cell {i}"
        );
    }
}

fn assert_samples_bit_identical(a: &[SimulationCutSample], b: &[SimulationCutSample], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: sample count");
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(x.sample_index, y.sample_index, "{what}: sample {i} index");
        assert_eq!(x.move_index, y.move_index, "{what}: sample {i} move");
        assert_eq!(
            x.cut_kinematics, y.cut_kinematics,
            "{what}: sample {i} kinematics"
        );
        assert_eq!(x.is_cutting, y.is_cutting, "{what}: sample {i} is_cutting");
        for k in 0..3 {
            assert_eq!(
                x.position[k].to_bits(),
                y.position[k].to_bits(),
                "{what}: sample {i} position[{k}]"
            );
        }
        for (name, xa, ya) in [
            (
                "cumulative_time_s",
                x.cumulative_time_s,
                y.cumulative_time_s,
            ),
            ("segment_time_s", x.segment_time_s, y.segment_time_s),
            ("axial_doc_mm", x.axial_doc_mm, y.axial_doc_mm),
            (
                "axial_engagement_mm",
                x.axial_engagement_mm,
                y.axial_engagement_mm,
            ),
            (
                "plunge_descent_mm",
                x.plunge_descent_mm,
                y.plunge_descent_mm,
            ),
            (
                "removed_volume_est_mm3",
                x.removed_volume_est_mm3,
                y.removed_volume_est_mm3,
            ),
            ("mrr_mm3_s", x.mrr_mm3_s, y.mrr_mm3_s),
            (
                "radial_woc_fraction",
                x.engagement.radial_woc_fraction,
                y.engagement.radial_woc_fraction,
            ),
        ] {
            assert_eq!(
                xa.to_bits(),
                ya.to_bits(),
                "{what}: sample {i} {name} ({xa} vs {ya})"
            );
        }
        assert_eq!(
            x.arc_engagement_radians.map(f64::to_bits),
            y.arc_engagement_radians.map(f64::to_bits),
            "{what}: sample {i} arc"
        );
        assert_eq!(
            x.effective_chip_thickness_mm.map(f64::to_bits),
            y.effective_chip_thickness_mm.map(f64::to_bits),
            "{what}: sample {i} chip"
        );
    }
}

/// The load-bearing one. Pin the pool at 1, 2, 4 and 8 threads and require the
/// grid AND the sample stream to be bit-identical to the 1-thread run.
///
/// Note that this is a *stronger* statement than "the parallel path agrees
/// with the parallel path": at one thread rayon still runs the same band
/// decomposition, so what is being pinned is that nothing about the schedule
/// leaks into the answer — not the number of workers, not who steals what, not
/// the order the joins complete.
#[test]
fn band_stamping_is_bit_identical_across_thread_counts() {
    for (name, cutter) in [
        (
            "flat6",
            Box::new(FlatEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
        ("ball6", Box::new(BallEndmill::new(6.0, 25.0))),
    ] {
        for &cell_size in &[0.2_f64, 0.5] {
            let mut reference: Option<(TriDexelStock, Vec<SimulationCutSample>)> = None;
            for threads in [1usize, 2, 4, 8] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("thread pool builds");
                let run = pool.install(|| simulate(cutter.as_ref(), cell_size));
                match &reference {
                    None => reference = Some(run),
                    Some((ref_stock, ref_samples)) => {
                        let what = format!("{name} cs={cell_size} threads={threads}");
                        assert_grids_bit_identical(ref_stock, &run.0, &what);
                        assert_samples_bit_identical(ref_samples, &run.1, &what);
                    }
                }
            }
            let (_, samples) = reference.expect("at least one run");
            assert!(
                samples.iter().filter(|s| s.is_cutting).count() > 500,
                "{name} cs={cell_size}: fixture stopped cutting — a determinism \
                 test over an empty stream passes for free"
            );
        }
    }
}

/// Anti-vacuity for the parallel path itself: the fixture must be big enough
/// that at least one stamp is actually dispatched across bands, or the test
/// above is only comparing four serial runs.
///
/// Checked structurally rather than by instrumenting the kernel: the grid has
/// to span more than one band, and the cutter footprint has to span more than
/// one band too, which is the condition `stamp_wants_threads` gates on.
#[test]
fn the_fixture_actually_spans_multiple_bands() {
    let stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, 0.2);
    let rows = stock.z_grid.rows;
    assert!(rows > 8, "grid is {rows} rows — a single band");
    let footprint_rows = (2.0 * 3.0 / stock.z_grid.cell_size).ceil() as usize;
    assert!(
        footprint_rows > 8,
        "a Ø6 footprint covers {footprint_rows} rows at cs={} — inside one \
         band, so no stamp can be split",
        stock.z_grid.cell_size
    );
}
