//! S1 sentries: swept-volume stamping.
//!
//! `PERF_REVIEW.md` S1 replaces per-subsegment stamping with one pass per chunk
//! of consecutive subsegments. It is the campaign's first **deliberately
//! metric-changing** fix, so the net around it has to separate three claims
//! that are usually one:
//!
//! 1. **The pure-vertical arm is bit-identical.** The by_z subdivision's
//!    redundancy is loop order, not subdivision — `(su, sv)` is literally the
//!    same `f64` for every subsegment of an exactly-vertical descent, so
//!    coverage, the LUT probe and the upper-bound `sqrt` are loop invariants.
//!    Hoisting them cannot move a bit, and this is where that is checked
//!    rather than asserted.
//! 2. **The lateral arm is deterministic.** It changes what is measured; it
//!    must not change it differently on a 4-core box than on a 32-core one.
//!    Same property, same bit-level test, as the S3 sentries.
//! 3. **The dispatch is not vacuous.** A fixture that resolves to one chunk or
//!    one band passes every check above for free.
//!
//! ```text
//! cargo test -p rs_cam_core --test swept_stamping_s1
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // Three of the tests here ARE measurements — they print the slope they
    // assert on so a reader can see the shape, not just the verdict. That is
    // the point of `print_swept_metric_delta`, and `tracing` is not wired in
    // a test binary.
    clippy::print_stdout
)]

use rs_cam_core::dexel_stock::{StampDispatch, StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;

/// Raster lines, a ramp, a plunge, a re-pass and an arc — the same shape the S3
/// sentry uses, so the two nets cover the same geometry.
fn mixed_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, 12.0));
    for i in 0..8 {
        let y = 4.0 + 2.2 * i as f64;
        let (x0, x1) = if i % 2 == 0 { (4.0, 34.0) } else { (34.0, 4.0) };
        tp.feed_to(P3::new(x0, y, 8.0), 900.0);
        tp.feed_to(P3::new(x1, y, 8.0), 1200.0);
    }
    tp.feed_to(P3::new(10.0, 10.0, 6.5), 600.0);
    tp.feed_to(P3::new(28.0, 14.0, 5.0), 600.0);
    tp.feed_to(P3::new(28.0, 14.0, 3.0), 200.0);
    tp.feed_to(P3::new(10.0, 14.0, 3.0), 900.0);
    tp.feed_to(P3::new(10.0, 14.0, 3.0), 900.0);
    tp.arc_cw_to(P3::new(20.0, 24.0, 3.0), 5.0, 5.0, 900.0);
    tp.final_retract(12.0);
    tp
}

/// Exactly-vertical descents and lifts only — the `ChunkKind::PureVertical`
/// arm, and nothing else. Every `feed_to` here shares its XY with the move
/// before it, so `end.x - start.x` and `end.y - start.y` are exactly `0.0` and
/// the hoisting precondition holds.
fn plunge_pass() -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..12 {
        let x = 5.0 + 5.0 * (i % 6) as f64;
        let y = 7.0 + 12.0 * (i / 6) as f64;
        tp.rapid_to(P3::new(x, y, 12.0));
        tp.feed_to(P3::new(x, y, 9.0), 300.0);
        tp.feed_to(P3::new(x, y, 12.0), 900.0);
    }
    tp.final_retract(12.0);
    tp
}

fn simulate_with(
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    dispatch: StampDispatch,
) -> (TriDexelStock, Vec<SimulationCutSample>) {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, cell_size);
    stock.stamp_dispatch = dispatch;
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_lut_metrics_cancel(
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
        )
        .expect("never cancelled");
    (stock, samples)
}

/// Same driver, but on a stock deliberately too small to be worth batching —
/// used by `swept_applies_to_a_single_band_grid`.
fn simulate_small_stock(
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    dispatch: StampDispatch,
) -> (TriDexelStock, Vec<SimulationCutSample>) {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 7.0, 0.0, 12.0, cell_size);
    stock.stamp_dispatch = dispatch;
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_lut_metrics_cancel(
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
    }
}

/// **Claim 1.** An exactly-vertical descent stamped as a chunk must produce the
/// same bits as the same descent stamped one 0.02 mm slice at a time.
///
/// This is the load-bearing sentry for S1(b): `PERF_REVIEW.md` prescribes an
/// *analytic* replacement for the by_z subdivision, which would be an
/// approximation with an error to bound. The landed form is not an
/// approximation at all — it is the same arithmetic with the loops inverted —
/// and the only honest way to say so is a bit-level comparison of the grid AND
/// the sample stream, including `plunge_descent_mm` and the per-slice
/// `removed_volume_est_mm3` that a drill-adjacent reader would consume.
#[test]
fn pure_vertical_chunks_are_bit_identical_to_per_stamp() {
    let tp = plunge_pass();
    for (label, cutter) in [
        (
            "flat6",
            Box::new(FlatEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
        (
            "ball6",
            Box::new(BallEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
    ] {
        for cell_size in [0.2_f64, 0.5] {
            let what = format!("{label}/cs{cell_size}");
            let per_stamp = simulate_with(&tp, cutter.as_ref(), cell_size, StampDispatch::PerStamp);
            let swept = simulate_with(&tp, cutter.as_ref(), cell_size, StampDispatch::Swept);
            assert!(
                swept.1.iter().filter(|s| s.is_cutting).count() > 500,
                "{what}: fixture is vacuous — {} cutting samples",
                swept.1.iter().filter(|s| s.is_cutting).count()
            );
            assert_grids_bit_identical(&per_stamp.0, &swept.0, &format!("{what}: grid"));
            assert_samples_bit_identical(&per_stamp.1, &swept.1, &format!("{what}: samples"));
        }
    }
}

/// **Claim 1b, and the landable subset.** `SweptPlungeOnly` must be bit-identical
/// to the shipped dispatch on a fixture that mixes *everything* — rasters, a
/// ramp, a plunge, a re-pass, an arc — not just on pure descents.
///
/// The mode grows a chunk only where growing it is provably free: an
/// exactly-vertical run (loop inversion) or a single bin (`bins == 1`, where the
/// swept kernel *is* the shipped kernel). If this passes, the `by_z` half of S1
/// carries no re-baseline and no decision at all — which is the single most
/// useful thing this wave can establish.
#[test]
fn swept_plunge_only_is_bit_identical_to_the_shipped_dispatch() {
    for tp in [mixed_pass(), plunge_pass()] {
        for (label, cutter) in [
            (
                "flat6",
                Box::new(FlatEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
            ),
            (
                "ball6",
                Box::new(BallEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
            ),
        ] {
            for cell_size in [0.2_f64, 0.5] {
                let what = format!("{label}/cs{cell_size}");
                let shipped = simulate_with(
                    &tp,
                    cutter.as_ref(),
                    cell_size,
                    StampDispatch::WholeToolpath,
                );
                let hoisted = simulate_with(
                    &tp,
                    cutter.as_ref(),
                    cell_size,
                    StampDispatch::SweptPlungeOnly,
                );
                assert_grids_bit_identical(&shipped.0, &hoisted.0, &format!("{what}: grid"));
                assert_samples_bit_identical(&shipped.1, &hoisted.1, &format!("{what}: samples"));
                assert!(
                    hoisted.0.last_stamp_dispatch.batches > 0,
                    "{what}: swept-plunge dispatch never ran"
                );
            }
        }
    }
}

/// **Claim 2.** Swept dispatch must not move with the core count.
///
/// The chunk decomposition is a function of the move's subdivision, the tool
/// radius and the cell size — never of `current_num_threads()` — and the band
/// decomposition underneath it is wave 2's, unchanged. So one thread and eight
/// must agree bit-for-bit, on the metric-CHANGING arm as much as on the
/// metric-neutral ones: a golden re-baselined on this mode is worthless if the
/// mode is not reproducible.
#[test]
fn swept_dispatch_is_bit_identical_across_thread_counts() {
    let tp = mixed_pass();
    for (label, cutter) in [
        (
            "flat6",
            Box::new(FlatEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
        (
            "ball6",
            Box::new(BallEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
    ] {
        for cell_size in [0.2_f64, 0.5] {
            let mut reference: Option<(TriDexelStock, Vec<SimulationCutSample>)> = None;
            for threads in [1usize, 2, 4, 8] {
                let what = format!("{label}/cs{cell_size}/{threads}t");
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .expect("pool");
                let run = pool.install(|| {
                    simulate_with(&tp, cutter.as_ref(), cell_size, StampDispatch::Swept)
                });
                match reference.as_ref() {
                    None => reference = Some(run),
                    Some(r) => {
                        assert_grids_bit_identical(&r.0, &run.0, &format!("{what}: grid"));
                        assert_samples_bit_identical(&r.1, &run.1, &format!("{what}: samples"));
                        assert_eq!(
                            r.0.last_stamp_dispatch.batches, run.0.last_stamp_dispatch.batches,
                            "{what}: batch boundaries moved with the thread count"
                        );
                    }
                }
            }
        }
    }
}

/// The second falsifiable claim behind the re-baseline, and the one that
/// explains the largest number in the golden diff.
///
/// The shipped air-cut percentage counts a *sample* as air when its **radial
/// engagement** reads below 0.02 — not when it removed nothing. Those are
/// different questions, and the removal proxy this test was first written
/// against reads 0 % on every arm, which is how the distinction was found.
///
/// At `cell_size > sample_step` the engagement criterion is a
/// **grid-quantisation artifact**: several consecutive subsegments advance the
/// tool within one cell column, the first of them takes the fresh material,
/// and the rest find no cell with `pre_fresh > FRESH_MATERIAL_THRESHOLD_MM` at
/// all — so `perp_max` never exceeds `perp_min`, radial engagement is exactly
/// `0.0`, and the sample is booked as air. The reported air fraction therefore
/// grows with the simulation cell size on a toolpath that has not changed. The
/// Phase 0 2.5D golden runs at `cell = 1.0 mm` against `sample_step = 0.25 mm`,
/// a 4:1 ratio, which is why its Pocket arm reports 60 % air on a pass that is
/// cutting throughout.
///
/// A swept pass gives every bin the fresh cells of its own longitudinal slice,
/// so its air fraction should be far flatter in cell size. This pins the
/// comparison of *slopes*, which is what the claim actually is — not either
/// absolute value.
#[test]
fn per_stamp_air_cut_pct_grows_with_cell_size_and_swept_does_not() {
    let tp = mixed_pass();
    let cutter = FlatEndmill::new(6.0, 25.0);
    let air = |v: &[SimulationCutSample]| -> f64 {
        let cutting: f64 = v
            .iter()
            .filter(|s| s.is_cutting)
            .map(|s| s.segment_time_s)
            .sum();
        let air: f64 = v
            .iter()
            // Verbatim from `SimulationCutTrace::accumulate`
            // (`simulation_cut.rs:1195`).
            .filter(|s| s.is_cutting && s.engagement.radial_woc_fraction < 0.02)
            .map(|s| s.segment_time_s)
            .sum();
        if cutting <= 0.0 {
            0.0
        } else {
            100.0 * air / cutting
        }
    };
    let mut old_air = Vec::new();
    let mut new_air = Vec::new();
    for cell_size in [0.25_f64, 0.5, 1.0] {
        let o = air(&simulate_with(&tp, &cutter, cell_size, StampDispatch::WholeToolpath).1);
        let n = air(&simulate_with(&tp, &cutter, cell_size, StampDispatch::Swept).1);
        println!("cs{cell_size}: per-stamp air {o:.2}% | swept air {n:.2}%");
        old_air.push(o);
        new_air.push(n);
    }
    let spread = |v: &[f64]| v.iter().cloned().fold(0.0_f64, f64::max) - v[0];
    assert!(
        spread(&old_air) > 20.0,
        "per-stamp air-cut % did not grow with cell size ({old_air:?}) — the \
         quantisation explanation for the golden's air-cut movement is wrong"
    );
    assert!(
        spread(&new_air) < spread(&old_air) / 4.0,
        "swept air-cut % is as cell-size-dependent as per-stamp's \
         ({new_air:?} vs {old_air:?})"
    );

    // Above the lateral-resolution limit BOTH read the same near-total air,
    // and that is the correct behaviour, not a failure of either dispatch.
    // `PERP_COVERAGE_GATE` needs two qualifying cell centres at different
    // perpendicular offsets for a width to exist at all; a 2 mm grid under a
    // Ø6 cutter does not have them, so radial engagement is structurally
    // zero. Swept stamping does NOT rescue that case and must not be sold as
    // if it does — `sim_measurability`'s abstention is what covers it.
    let coarse_old = air(&simulate_with(&tp, &cutter, 2.0, StampDispatch::WholeToolpath).1);
    let coarse_new = air(&simulate_with(&tp, &cutter, 2.0, StampDispatch::Swept).1);
    println!("cs2 (below lateral resolution): per-stamp {coarse_old:.2}% | swept {coarse_new:.2}%");
    assert!(
        coarse_old > 90.0 && (coarse_new - coarse_old).abs() < 1.0,
        "at cs=2.0 the two dispatches should agree on near-total air \
         ({coarse_old} vs {coarse_new}) — the lateral-resolution floor is not a \
         dispatch property"
    );
}

/// The third: measured axial DOC should equal *commanded* axial DOC on a
/// flat-bottomed 2.5D pass. That is F-024's own property, and the shipped
/// kernel misses it — the Phase 0 golden records `peak_axial_doc_mm` of
/// **2.0625** and **2.6367** on Pocket and Profile passes commanded at
/// **3.0 mm**, an under-read of up to 31 %, because a partially covered cell's
/// removal is split across the subsegments that blend it. A swept pass removes
/// the cell once and reads the whole depth.
#[test]
fn swept_peak_axial_doc_reaches_the_commanded_depth_and_per_stamp_does_not() {
    // One flat pass at a commanded 3 mm DOC into fresh stock, nothing else.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 15.0, 12.0));
    tp.feed_to(P3::new(4.0, 15.0, 9.0), 300.0);
    tp.feed_to(P3::new(36.0, 15.0, 9.0), 900.0);
    tp.final_retract(12.0);
    let cutter = FlatEndmill::new(6.0, 25.0);
    let peak = |v: &[SimulationCutSample]| v.iter().map(|s| s.axial_doc_mm).fold(0.0_f64, f64::max);
    for cell_size in [0.5_f64, 1.0] {
        let o = peak(&simulate_with(&tp, &cutter, cell_size, StampDispatch::WholeToolpath).1);
        let n = peak(&simulate_with(&tp, &cutter, cell_size, StampDispatch::Swept).1);
        println!("cs{cell_size}: commanded 3.000 | per-stamp {o:.4} | swept {n:.4}");
        assert!(
            (n - 3.0).abs() < 1e-9,
            "cs{cell_size}: swept peak axial DOC {n} != commanded 3.0"
        );
        assert!(
            o < n,
            "cs{cell_size}: per-stamp peak axial DOC {o} did not under-read \
             the commanded 3.0 — the F-024 argument for S1 is wrong"
        );
    }
}

/// **Claim 3.** The fixture must actually chunk, batch and band — otherwise the
/// two tests above pass on a degenerate schedule.
#[test]
fn swept_dispatch_is_not_vacuous() {
    let tp = mixed_pass();
    let cutter = FlatEndmill::new(6.0, 25.0);
    let (stock, samples) = simulate_with(&tp, &cutter, 0.2, StampDispatch::Swept);
    let stats = stock.last_stamp_dispatch;
    assert!(stats.bands > 1, "one band: {stats:?}");
    assert!(stats.batches > 1, "one batch: {stats:?}");
    assert!(
        stats.max_jobs_in_a_batch > 1,
        "one chunk per batch: {stats:?}"
    );
    assert!(
        stats.max_partials_in_a_batch > stats.max_jobs_in_a_batch,
        "chunks never spanned more than one band: {stats:?}"
    );
    assert!(
        samples.iter().filter(|s| s.is_cutting).count() > 500,
        "fixture is vacuous"
    );
}

/// `Auto` resolves to the metric-changing shape — the w5b flip, stated as a
/// property rather than as a comment on an enum.
///
/// **Both halves matter.** "`Auto` == `Swept`" alone would pass if `resolved()`
/// were ever quietly reverted *and* the two shapes happened to agree on this
/// fixture; the second assertion is a non-vacuity guard that fails red the
/// moment `Auto` goes back to publishing the old measure. This inverts
/// `auto_never_selects_swept_dispatch`, which pinned the opposite property
/// while the landing decision was open.
#[test]
fn auto_selects_swept_dispatch() {
    let tp = mixed_pass();
    let cutter = FlatEndmill::new(6.0, 25.0);
    let auto = simulate_with(&tp, &cutter, 0.2, StampDispatch::Auto);
    let swept = simulate_with(&tp, &cutter, 0.2, StampDispatch::Swept);
    assert_grids_bit_identical(&auto.0, &swept.0, "auto == swept: grid");
    assert_samples_bit_identical(&auto.1, &swept.1, "auto == swept: samples");

    // Non-vacuity: the shape `Auto` used to resolve to must still be reachable
    // AND must still differ, or the assertion above proves nothing.
    let whole = simulate_with(&tp, &cutter, 0.2, StampDispatch::WholeToolpath);
    let air = |s: &[SimulationCutSample]| {
        let cutting = s.iter().filter(|s| s.is_cutting).count();
        let air = s
            .iter()
            .filter(|s| s.is_cutting && s.engagement.radial_woc_fraction < 0.02)
            .count();
        air as f64 / cutting.max(1) as f64
    };
    assert!(
        (air(&auto.1) - air(&whole.1)).abs() > 1e-9,
        "auto and whole_path agree on air fraction ({:.6} vs {:.6}) — either the \
         flip reverted or this fixture no longer separates the two measures",
        air(&auto.1),
        air(&whole.1)
    );
}

/// A grid too small to be worth *batching* must still get the new measure.
///
/// `MIN_BANDS_FOR_SWEPT` was 2 while swept was opt-in. At the default that
/// threshold would mean a stock of fewer than `2 · BAND_ROWS = 16` rows
/// silently publishes the pre-w5b measure while every larger stock publishes
/// the new one — a metric that changes with the stock's row count. This pins
/// the fix: on a grid of one band, `Auto` still agrees with `Swept` and still
/// differs from the per-stamp kernel.
#[test]
fn swept_applies_to_a_single_band_grid() {
    let tp = mixed_pass();
    let cutter = FlatEndmill::new(6.0, 25.0);
    // 7 mm of Y at 1.0 mm/cell is 8 rows (endpoints inclusive) — one band.
    let cell = 1.0;
    let auto = simulate_small_stock(&tp, &cutter, cell, StampDispatch::Auto);
    let swept = simulate_small_stock(&tp, &cutter, cell, StampDispatch::Swept);
    let per = simulate_small_stock(&tp, &cutter, cell, StampDispatch::PerStamp);
    assert_eq!(
        auto.0.z_grid.rows.div_ceil(8),
        1,
        "fixture is not a one-band grid (rows {}); the whole point of this test \
         is the band count",
        auto.0.z_grid.rows
    );
    assert_samples_bit_identical(&auto.1, &swept.1, "one-band auto == swept");
    assert!(
        auto.1.iter().filter(|s| s.is_cutting).count() > 10,
        "one-band fixture is vacuous"
    );
    let removed =
        |s: &[SimulationCutSample]| s.iter().map(|s| s.removed_volume_est_mm3).sum::<f64>();
    assert!(
        (removed(&auto.1) - removed(&per.1)).abs() > 1e-9,
        "one-band swept removal is bit-equal to per-stamp ({:.6} vs {:.6}) — the \
         swept driver was skipped for this grid",
        removed(&auto.1),
        removed(&per.1)
    );
}

fn simulate_step(
    tp: &Toolpath,
    cutter: &dyn MillingCutter,
    cell_size: f64,
    step: f64,
    dispatch: StampDispatch,
) -> Vec<SimulationCutSample> {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, cell_size);
    stock.stamp_dispatch = dispatch;
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_lut_metrics_cancel(
            tp,
            &lut,
            cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            step,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("never cancelled")
}

/// The measurement that decides which estimator is the more honest one.
///
/// Removed volume is a property of the *stock and the cutter*, not of how
/// finely the driver chose to sample the move. The shipped per-subsegment
/// kernel does not have that property at partially-covered cells: `f`-blending
/// the same boundary cell `N` times leaves `(1 − f)^N` of its above-surface
/// slice, so halving `sample_step` removes strictly more material. A swept pass
/// blends each cell **once**, at the coverage of the whole chunk's stadium, and
/// is invariant.
///
/// This test does not assert which one is "right" — it pins the *invariance*,
/// which is falsifiable and is the thing the decision rests on. If a future
/// change makes swept density-dependent, this fails.
#[test]
fn swept_removal_is_sample_density_independent_and_per_stamp_is_not() {
    let tp = mixed_pass();
    let cutter = FlatEndmill::new(6.0, 25.0);
    let total =
        |v: &[SimulationCutSample]| -> f64 { v.iter().map(|s| s.removed_volume_est_mm3).sum() };
    for cell_size in [0.2_f64, 0.5] {
        let old_fine = total(&simulate_step(
            &tp,
            &cutter,
            cell_size,
            0.1,
            StampDispatch::WholeToolpath,
        ));
        let old_coarse = total(&simulate_step(
            &tp,
            &cutter,
            cell_size,
            0.5,
            StampDispatch::WholeToolpath,
        ));
        let new_fine = total(&simulate_step(
            &tp,
            &cutter,
            cell_size,
            0.1,
            StampDispatch::Swept,
        ));
        let new_coarse = total(&simulate_step(
            &tp,
            &cutter,
            cell_size,
            0.5,
            StampDispatch::Swept,
        ));
        let old_spread = (old_fine - old_coarse).abs() / old_coarse.max(1e-9);
        let new_spread = (new_fine - new_coarse).abs() / new_coarse.max(1e-9);
        println!(
            "cs{cell_size}: per-stamp {old_coarse:.3} -> {old_fine:.3} ({:.2}%) | \
             swept {new_coarse:.3} -> {new_fine:.3} ({:.2}%)",
            100.0 * old_spread,
            100.0 * new_spread
        );
        assert!(
            new_spread < old_spread,
            "cs{cell_size}: swept ({:.3}%) is not less density-dependent than \
             per-stamp ({:.3}%) — the central claim of S1's re-baseline is false",
            100.0 * new_spread,
            100.0 * old_spread
        );
        assert!(
            new_spread < 0.02,
            "cs{cell_size}: swept removal moved {:.3}% across a 5x change in \
             sample_step; it is documented as invariant",
            100.0 * new_spread
        );
    }
}

/// Characterisation, not a gate: print the aggregate movement between the
/// shipped dispatch and the swept one on the mixed fixture, so the decision
/// package quotes measured numbers rather than predicted ones.
///
/// ```text
/// cargo test -p rs_cam_core --test swept_stamping_s1 -- --ignored --nocapture
/// ```
#[test]
#[ignore = "measurement, not a gate — prints the S1 metric delta"]
fn print_swept_metric_delta() {
    let tp = mixed_pass();
    for (label, cutter) in [
        (
            "flat6",
            Box::new(FlatEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
        (
            "ball6",
            Box::new(BallEndmill::new(6.0, 25.0)) as Box<dyn MillingCutter>,
        ),
    ] {
        for cell_size in [0.2_f64, 0.5] {
            let (_, old) = simulate_with(
                &tp,
                cutter.as_ref(),
                cell_size,
                StampDispatch::WholeToolpath,
            );
            let (_, new) = simulate_with(&tp, cutter.as_ref(), cell_size, StampDispatch::Swept);
            let sum = |v: &[SimulationCutSample], f: fn(&SimulationCutSample) -> f64| -> f64 {
                v.iter().map(f).sum()
            };
            let peak = |v: &[SimulationCutSample], f: fn(&SimulationCutSample) -> f64| -> f64 {
                v.iter().map(f).fold(0.0_f64, f64::max)
            };
            let avg_eng = |v: &[SimulationCutSample]| -> f64 {
                let cut: Vec<_> = v.iter().filter(|s| s.is_cutting).collect();
                if cut.is_empty() {
                    return 0.0;
                }
                cut.iter()
                    .map(|s| s.engagement.radial_woc_fraction)
                    .sum::<f64>()
                    / cut.len() as f64
            };
            let air = |v: &[SimulationCutSample]| -> f64 {
                let cutting: f64 = v
                    .iter()
                    .filter(|s| s.is_cutting)
                    .map(|s| s.segment_time_s)
                    .sum();
                let air: f64 = v
                    .iter()
                    .filter(|s| s.is_cutting && s.removed_volume_est_mm3 <= 1e-9)
                    .map(|s| s.segment_time_s)
                    .sum();
                if cutting <= 0.0 {
                    0.0
                } else {
                    100.0 * air / cutting
                }
            };
            let rows: [(&str, f64, f64); 6] = [
                (
                    "total_removed_mm3",
                    sum(&old, |s| s.removed_volume_est_mm3),
                    sum(&new, |s| s.removed_volume_est_mm3),
                ),
                (
                    "peak_axial_doc_mm",
                    peak(&old, |s| s.axial_doc_mm),
                    peak(&new, |s| s.axial_doc_mm),
                ),
                (
                    "peak_plunge_descent_mm",
                    peak(&old, |s| s.plunge_descent_mm),
                    peak(&new, |s| s.plunge_descent_mm),
                ),
                ("avg_radial_woc", avg_eng(&old), avg_eng(&new)),
                ("air_pct_of_cutting", air(&old), air(&new)),
                (
                    "peak_radial_woc",
                    peak(&old, |s| s.engagement.radial_woc_fraction),
                    peak(&new, |s| s.engagement.radial_woc_fraction),
                ),
            ];
            println!(
                "== {label}/cs{cell_size} (samples {} vs {}) ==",
                old.len(),
                new.len()
            );
            for (name, o, n) in rows {
                let pct = if o.abs() > 1e-12 {
                    100.0 * (n - o) / o
                } else {
                    f64::NAN
                };
                println!("  {name:<24} {o:>14.6} -> {n:>14.6}  ({pct:+.2}%)");
            }
        }
    }
}
