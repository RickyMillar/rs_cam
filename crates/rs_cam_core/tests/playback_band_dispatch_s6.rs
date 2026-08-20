//! SIM w6 sentry: banded playback replay must be **bit-identical** to the
//! serial one, on every channel, at every thread count.
//!
//! `DELTA_sim_w4.md` §6 left the non-metric playback replay
//! (`simulate_toolpath_with_lut_cancel` → `stamping::stamp_segment_on_grid`)
//! 100 % serial. This wave bands it. The contract is stricter than the metric
//! side's was, and for a reason that has nothing to do with tidiness:
//!
//! * `compute/simulate.rs` runs this kernel against `global_stock` for **every**
//!   toolpath, and against `group_stock` for every toolpath whose metrics are
//!   off — and `group_stock` is what `prior_stocks` snapshots, which is what
//!   `StockSource::FromRemainingStock` generation reads. A single last-bit
//!   difference in this grid changes generated G-code downstream.
//! * So there is no reassociation latitude here at all. `whole_path.rs` had to
//!   argue its volume sums merge in the same order; this kernel has no
//!   accumulators, so the identity is structural — and that argument is worth
//!   exactly as much as the test below.
//!
//! Everything is compared as **bit patterns** (`to_bits`), never with `==` on
//! floats and never with a tolerance.
//!
//! Four fixtures, chosen against the risk rather than for coverage optics:
//!
//! | fixture | what it is there for |
//! |---|---|
//! | `two_and_a_half_d` | a multi-op 2.5D job: raster clear, contour, drill plunges (the degenerate branch), arcs |
//! | `three_d` | continuously varying Z — ramps and a helix, so `segment_tip_low` and the per-cell depth interpolation both move |
//! | `cascade` | three toolpaths into ONE stock, then the remaining-stock probes a `FromRemainingStock` op would sample |
//! | side-grid arm | `FromBack`, i.e. the lazily-created Y grid rather than the Z one |
//!
//! ```text
//! cargo test -p rs_cam_core --test playback_band_dispatch_s6
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel::DexelGrid;
use rs_cam_core::dexel_stock::{
    PlaybackDispatch, PlaybackDispatchStats, StockCutDirection, TriDexelStock,
};
use rs_cam_core::geo::P3;
use rs_cam_core::radial_profile::LUT_SAMPLES;
use rs_cam_core::radial_profile::RadialProfileLUT;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;

// ── Fixtures ────────────────────────────────────────────────────────────

/// A 2.5D clearing pass: overlapping raster lines at two depths, so adjacent
/// bands touch the same rows and the S2 per-cell early-out gets exercised by a
/// genuine re-pass rather than by air.
fn raster_clear() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, 12.0));
    for level in 0..3 {
        let z = 9.0 - 2.0 * level as f64;
        for i in 0..12 {
            let y = 4.0 + 2.0 * i as f64;
            let (x0, x1) = if i % 2 == 0 { (4.0, 34.0) } else { (34.0, 4.0) };
            tp.feed_to(P3::new(x0, y, z), 900.0);
            tp.feed_to(P3::new(x1, y, z), 1200.0);
        }
    }
    tp.final_retract(12.0);
    tp
}

/// A contour: arcs plus straight links, and a lead-in that deliberately leaves
/// the stock on one side — the `clamped_cell_bbox` regime, which is where the
/// wave-3 §6 defect lived and where a band clip is most likely to go wrong.
fn contour() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-6.0, 15.0, 12.0));
    tp.feed_to(P3::new(-6.0, 15.0, 7.0), 400.0);
    tp.feed_to(P3::new(10.0, 15.0, 7.0), 1000.0);
    tp.arc_cw_to(P3::new(20.0, 25.0, 7.0), 10.0, 0.0, 900.0);
    tp.feed_to(P3::new(30.0, 25.0, 7.0), 1000.0);
    tp.arc_ccw_to(P3::new(36.0, 19.0, 7.0), 0.0, -6.0, 900.0);
    // …and off the far edge, so a stamp lands entirely outside the grid.
    tp.feed_to(P3::new(36.0, 40.0, 7.0), 1000.0);
    tp.final_retract(12.0);
    tp
}

/// Plunges — the kernel's degenerate branch, which is a different code path
/// with its own bbox, its own mip query and its own `point_cell_coverage`.
fn drill_plunges() -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..24 {
        let x = 6.0 + 4.0 * (i % 8) as f64;
        let y = 6.0 + 5.0 * (i / 8) as f64;
        tp.rapid_to(P3::new(x, y, 12.0));
        tp.feed_to(P3::new(x, y, 3.0), 250.0);
        tp.feed_to(P3::new(x, y, 12.0), 800.0);
    }
    tp.final_retract(12.0);
    tp
}

/// Continuously-varying Z: a serpentine drape with a ramp on every link and a
/// helical entry. Nothing here holds a constant depth, so the per-cell
/// `sd + t_center·seg_dd` interpolation and `segment_tip_low` are both live on
/// essentially every stamp.
fn three_d_drape() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(8.0, 8.0, 12.0));
    // Helical entry.
    for step in 0..24 {
        let t = step as f64 / 24.0;
        let a = t * std::f64::consts::TAU * 2.0;
        tp.feed_to(
            P3::new(8.0 + 4.0 * a.cos(), 8.0 + 4.0 * a.sin(), 10.5 - 3.0 * t),
            400.0,
        );
    }
    for i in 0..18 {
        let y = 5.0 + 1.3 * i as f64;
        let (x0, x1) = if i % 2 == 0 { (5.0, 34.0) } else { (34.0, 5.0) };
        // Every segment is a ramp; the surface undulates along X too.
        for k in 0..8 {
            let t0 = k as f64 / 8.0;
            let t1 = (k + 1) as f64 / 8.0;
            let x = x0 + (x1 - x0) * t1;
            let z = 8.0 - 1.4 * ((x * 0.31).sin() + (y * 0.27).cos()) - 0.6 * t0;
            tp.feed_to(P3::new(x, y, z), 1100.0);
        }
    }
    tp.final_retract(12.0);
    tp
}

// ── Harness ─────────────────────────────────────────────────────────────

fn fresh_stock(cell_size: f64) -> TriDexelStock {
    TriDexelStock::from_stock(0.0, 0.0, 40.0, 30.0, 0.0, 12.0, cell_size)
}

/// Replay `paths` into ONE stock, in order — which is exactly what
/// `compute/simulate.rs` does to `global_stock` and to a metrics-off
/// `group_stock`.
fn replay(
    paths: &[Toolpath],
    cutter: &dyn MillingCutter,
    cell_size: f64,
    direction: StockCutDirection,
    dispatch: PlaybackDispatch,
) -> (TriDexelStock, PlaybackDispatchStats) {
    let mut stock = fresh_stock(cell_size);
    stock.playback_dispatch = dispatch;
    let lut = RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut last = PlaybackDispatchStats::default();
    for tp in paths {
        stock
            .simulate_toolpath_with_lut_cancel(tp, &lut, cutter.radius(), direction, &never_cancel)
            .expect("never cancelled");
        // Every replay resets the counter, so accumulate the widest reading
        // rather than keeping only the last toolpath's.
        let s = stock.last_playback_dispatch;
        last = PlaybackDispatchStats {
            batches: last.batches + s.batches,
            bands: s.bands.max(last.bands),
            max_jobs_in_a_batch: s.max_jobs_in_a_batch.max(last.max_jobs_in_a_batch),
            max_partials_in_a_batch: s.max_partials_in_a_batch.max(last.max_partials_in_a_batch),
            band_tasks_run: last.band_tasks_run + s.band_tasks_run,
        };
    }
    (stock, last)
}

/// The full grid fingerprint: **every dexel span on every ray**, plus the
/// sliver-safe channel, on whichever grid the direction actually used.
///
/// Both live grids are checked, not just the Z one — a side-cut direction
/// creates the Y or X grid lazily and it is the one the stamps landed in.
fn assert_grids_bit_identical(a: &TriDexelStock, b: &TriDexelStock, what: &str) {
    fn pair<'g>(
        what: &str,
        axis: &str,
        x: Option<&'g DexelGrid>,
        y: Option<&'g DexelGrid>,
    ) -> Option<(&'g DexelGrid, &'g DexelGrid)> {
        match (x, y) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => panic!("{what}: one run created the {axis} grid and the other did not"),
        }
    }
    let grids = [
        ("z", Some((&a.z_grid, &b.z_grid))),
        ("y", pair(what, "y", a.y_grid.as_ref(), b.y_grid.as_ref())),
        ("x", pair(what, "x", a.x_grid.as_ref(), b.x_grid.as_ref())),
    ];
    let mut total_spans = 0usize;
    for (axis, pair) in grids {
        let Some((ga, gb)) = pair else { continue };
        assert_eq!(ga.rays.len(), gb.rays.len(), "{what}/{axis}: ray count");
        for (i, (ra, rb)) in ga.rays.iter().zip(gb.rays.iter()).enumerate() {
            assert_eq!(ra.len(), rb.len(), "{what}/{axis}: cell {i} segment count");
            total_spans += ra.len();
            for (sa, sb) in ra.iter().zip(rb.iter()) {
                assert_eq!(
                    (sa.enter.to_bits(), sa.exit.to_bits()),
                    (sb.enter.to_bits(), sb.exit.to_bits()),
                    "{what}/{axis}: cell {i} segment bits"
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
                "{what}/{axis}: conservative_top bits at cell {i}"
            );
        }
    }
    assert!(
        total_spans > 0,
        "{what}: the fixture left NO dexel spans anywhere — an all-empty grid \
         compares equal to another all-empty grid for free"
    );
}

/// The downstream-geometry check, and the reason this file exists.
///
/// `StockSource::FromRemainingStock` generation does not read the raw grid; it
/// asks the stock what material is left near a point. These are the two queries
/// it uses (`max_top_z_in_disc` and its sliver-safe twin), sampled over a
/// lattice that covers the whole blank. A grid difference that somehow did not
/// reach a ray span would still have to survive this to be invisible.
fn assert_remaining_stock_probes_identical(a: &TriDexelStock, b: &TriDexelStock, what: &str) {
    let mut cut_probes = 0usize;
    for iy in 0..15 {
        for ix in 0..20 {
            let (x, y) = (2.0 * ix as f64, 2.0 * iy as f64);
            for r in [1.0_f64, 3.5] {
                let (pa, pb) = (a.max_top_z_in_disc(x, y, r), b.max_top_z_in_disc(x, y, r));
                assert_eq!(
                    pa.map(f64::to_bits),
                    pb.map(f64::to_bits),
                    "{what}: max_top_z_in_disc({x}, {y}, {r}) — this is what a \
                     FromRemainingStock op samples"
                );
                if pa.is_some_and(|z| z < 11.9) {
                    cut_probes += 1;
                }
                let (ca, cb) = (
                    a.max_conservative_top_z_in_disc(x, y, r),
                    b.max_conservative_top_z_in_disc(x, y, r),
                );
                assert_eq!(
                    ca.map(f64::to_bits),
                    cb.map(f64::to_bits),
                    "{what}: max_conservative_top_z_in_disc({x}, {y}, {r})"
                );
            }
            let (sa, sb) = (
                a.local_material_sum(x, y, 3.0),
                b.local_material_sum(x, y, 3.0),
            );
            assert_eq!(
                sa.to_bits(),
                sb.to_bits(),
                "{what}: local_material_sum({x}, {y}, 3.0)"
            );
        }
    }
    assert!(
        cut_probes > 20,
        "{what}: only {cut_probes} probes saw material below the stock top — \
         the lattice is sampling untouched blank, so it would pass over a \
         completely un-stamped grid"
    );
}

/// **Anti-vacuity, and this programme's most-repeated trap.** A dispatch that
/// resolves to one band, or to one batch, or that never enters the parallel
/// region at all, passes every bit-identity check above for free.
///
/// `band_tasks_run` is the one that cannot be faked: the other four counters are
/// recorded on the serial side and would keep counting if the `par_bands` call
/// were reduced to nothing by an empty range, a truncating `zip` or a `None`
/// slice. That one is incremented by the closure that does the stamping.
fn assert_playback_dispatch_is_not_vacuous(stats: &PlaybackDispatchStats, what: &str) {
    assert!(
        stats.bands > 1,
        "{what}: the grid resolved to {} band(s) — nothing to dispatch",
        stats.bands
    );
    assert!(
        stats.batches > 1,
        "{what}: the whole replay fitted in {} batch — the batch boundary (mip \
         refresh, reduce, tail flush) is never exercised",
        stats.batches
    );
    assert!(
        stats.max_jobs_in_a_batch > 1,
        "{what}: the largest batch held {} stamp — this is serial dispatch \
         wearing a different name",
        stats.max_jobs_in_a_batch
    );
    assert!(
        stats.max_partials_in_a_batch > stats.max_jobs_in_a_batch,
        "{what}: {} partials over {} stamps — no stamp was split across bands",
        stats.max_partials_in_a_batch,
        stats.max_jobs_in_a_batch
    );
    assert!(
        stats.band_tasks_run > stats.batches,
        "{what}: {} band task(s) over {} batches — the PARALLEL ARM did not run \
         more than one band per dispatch, so the banding is inert and sound, \
         which is the failure mode that is silent",
        stats.band_tasks_run,
        stats.batches
    );
}

/// Thread counts the sentries sweep: one, four, and everything the box has.
fn thread_counts() -> Vec<usize> {
    let max = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut counts = vec![1usize, 4];
    if max > 4 {
        counts.push(max);
    }
    counts.dedup();
    counts
}

#[allow(clippy::type_complexity)]
fn fixtures() -> Vec<(&'static str, Vec<Toolpath>, Box<dyn MillingCutter>, f64)> {
    vec![
        (
            "two_and_a_half_d",
            vec![raster_clear(), contour(), drill_plunges()],
            Box::new(FlatEndmill::new(6.0, 25.0)),
            0.25,
        ),
        (
            "three_d",
            vec![three_d_drape()],
            Box::new(BallEndmill::new(6.0, 25.0)),
            0.25,
        ),
        // The FromRemainingStock risk case: a chain of ops accumulating into one
        // stock, at a fine cell size, with a second cutter — the shape
        // `compute/simulate.rs` builds `group_stock` and `global_stock` in.
        (
            "cascade",
            vec![raster_clear(), drill_plunges(), contour(), three_d_drape()],
            Box::new(FlatEndmill::new(3.0, 20.0)),
            0.2,
        ),
    ]
}

// ── The sentries ────────────────────────────────────────────────────────

/// **The load-bearing one.** Banded and serial playback must agree bit for bit
/// — on every dexel span, on `conservative_top`, and on the remaining-stock
/// queries a `FromRemainingStock` op would sample — at 1, 4 and max threads.
#[test]
fn playback_banded_matches_serial_bit_for_bit() {
    for (name, paths, cutter, cell_size) in fixtures() {
        for threads in thread_counts() {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("thread pool builds");
            let (serial, serial_stats) = pool.install(|| {
                replay(
                    &paths,
                    cutter.as_ref(),
                    cell_size,
                    StockCutDirection::FromTop,
                    PlaybackDispatch::Serial,
                )
            });
            let (banded, banded_stats) = pool.install(|| {
                replay(
                    &paths,
                    cutter.as_ref(),
                    cell_size,
                    StockCutDirection::FromTop,
                    PlaybackDispatch::Banded,
                )
            });
            let what = format!("{name} threads={threads}");
            assert_eq!(
                serial_stats,
                PlaybackDispatchStats::default(),
                "{what}: the serial arm reported dispatch stats — it is not serial"
            );
            assert_playback_dispatch_is_not_vacuous(&banded_stats, &what);
            assert_grids_bit_identical(&serial, &banded, &what);
            assert_remaining_stock_probes_identical(&serial, &banded, &what);
        }
    }
}

/// The shipped default must be the banded path, and it must be the same answer
/// as forcing either arm explicitly.
///
/// Without this, `Auto` could quietly stop selecting banded dispatch and every
/// test above would keep passing while measuring nothing that ships.
#[test]
fn auto_selects_banded_and_agrees_with_both_explicit_arms() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let paths = vec![raster_clear(), contour(), drill_plunges()];
    let (auto, auto_stats) = replay(
        &paths,
        &cutter,
        0.25,
        StockCutDirection::FromTop,
        PlaybackDispatch::Auto,
    );
    assert_playback_dispatch_is_not_vacuous(&auto_stats, "auto");
    for (arm, mode) in [
        ("serial", PlaybackDispatch::Serial),
        ("banded", PlaybackDispatch::Banded),
    ] {
        let (other, _) = replay(&paths, &cutter, 0.25, StockCutDirection::FromTop, mode);
        assert_grids_bit_identical(&auto, &other, &format!("auto vs {arm}"));
        assert_remaining_stock_probes_identical(&auto, &other, &format!("auto vs {arm}"));
    }
}

/// The band decomposition and the batch boundaries are functions of the grid,
/// never of `current_num_threads()`. So a `global_stock` captured on a 4-core
/// box has to reproduce on a 24-core one, bit for bit — otherwise the G-code a
/// `FromRemainingStock` op generates depends on the machine that ran the sim.
#[test]
fn playback_banded_is_bit_identical_across_thread_counts() {
    for (name, paths, cutter, cell_size) in fixtures() {
        let mut reference: Option<(TriDexelStock, PlaybackDispatchStats)> = None;
        for threads in thread_counts() {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("thread pool builds");
            let run = pool.install(|| {
                replay(
                    &paths,
                    cutter.as_ref(),
                    cell_size,
                    StockCutDirection::FromTop,
                    PlaybackDispatch::Banded,
                )
            });
            let what = format!("{name} threads={threads}");
            assert_playback_dispatch_is_not_vacuous(&run.1, &what);
            match &reference {
                None => reference = Some(run),
                Some((ref_stock, ref_stats)) => {
                    assert_grids_bit_identical(ref_stock, &run.0, &what);
                    assert_remaining_stock_probes_identical(ref_stock, &run.0, &what);
                    assert_eq!(
                        ref_stats.batches, run.1.batches,
                        "{what}: the batch count moved with the thread count"
                    );
                    assert_eq!(
                        ref_stats.bands, run.1.bands,
                        "{what}: the band count moved with the thread count"
                    );
                }
            }
        }
    }
}

/// Determinism run to run at one thread count: the same banded replay twice
/// must give the same fingerprint. Distinct from the sweep above — this is the
/// one that would catch a race inside a single pool configuration, where work
/// stealing lands differently on two runs.
#[test]
fn playback_banded_is_deterministic_run_to_run() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let paths = vec![raster_clear(), drill_plunges(), contour()];
    let threads = *thread_counts().last().expect("at least one thread count");
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool builds");
    let mut reference: Option<TriDexelStock> = None;
    for run in 0..4 {
        let (stock, stats) = pool.install(|| {
            replay(
                &paths,
                &cutter,
                0.25,
                StockCutDirection::FromTop,
                PlaybackDispatch::Banded,
            )
        });
        let what = format!("run {run} at {threads} threads");
        assert_playback_dispatch_is_not_vacuous(&stats, &what);
        match &reference {
            None => reference = Some(stock),
            Some(first) => {
                assert_grids_bit_identical(first, &stock, &what);
                assert_remaining_stock_probes_identical(first, &stock, &what);
            }
        }
    }
}

/// The side grids are a different lazily-created `DexelGrid` with a different
/// axis mapping, and the band decomposition is defined over *rows* of whichever
/// grid the direction selects. Cheap to get wrong, cheap to pin.
#[test]
fn playback_banded_matches_serial_on_a_side_grid() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(6.0, 40.0, 6.0));
    for i in 0..8 {
        let z = 2.0 + 1.1 * i as f64;
        let (x0, x1) = if i % 2 == 0 { (6.0, 32.0) } else { (32.0, 6.0) };
        tp.feed_to(P3::new(x0, 22.0, z), 900.0);
        tp.feed_to(P3::new(x1, 22.0, z), 1200.0);
    }
    // A plunge along the ray axis of the Y grid, i.e. degenerate in (x, z).
    tp.feed_to(P3::new(20.0, 8.0, 6.0), 300.0);
    let paths = vec![tp];
    for threads in thread_counts() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool builds");
        let (serial, _) = pool.install(|| {
            replay(
                &paths,
                &cutter,
                0.25,
                StockCutDirection::FromBack,
                PlaybackDispatch::Serial,
            )
        });
        let (banded, stats) = pool.install(|| {
            replay(
                &paths,
                &cutter,
                0.25,
                StockCutDirection::FromBack,
                PlaybackDispatch::Banded,
            )
        });
        let what = format!("side-grid threads={threads}");
        assert_playback_dispatch_is_not_vacuous(&stats, &what);
        assert!(
            banded.y_grid.is_some(),
            "{what}: FromBack did not create the Y grid — the arm names a grid \
             it never reached"
        );
        assert_grids_bit_identical(&serial, &banded, &what);
    }
}

/// The coarsest arm must still reach multi-band AND multi-batch with margin, so
/// a later shrink of a fixture is a red test rather than a quiet loss of
/// coverage.
#[test]
fn the_playback_fixtures_span_multiple_bands_and_batches() {
    for (name, paths, cutter, cell_size) in fixtures() {
        let (_, stats) = replay(
            &paths,
            cutter.as_ref(),
            cell_size,
            StockCutDirection::FromTop,
            PlaybackDispatch::Banded,
        );
        assert_playback_dispatch_is_not_vacuous(&stats, name);
        assert!(
            stats.bands >= 4 && stats.batches >= 3,
            "{name} resolved to {} bands / {} batches; the sentries want \
             comfortable margin, not a boundary case",
            stats.bands,
            stats.batches
        );
    }
}

/// A toolpath that leaves the stock must not panic and must not diverge —
/// `DELTA_sim_w3.md` §6's shape, now through the banded path, where the band
/// clip is applied on TOP of `clamped_cell_bbox` and is a seventh place the
/// same defect could be introduced.
#[test]
fn a_banded_replay_that_leaves_the_stock_matches_serial() {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 37.0, 12.0));
    for (name, dx, dy) in [
        ("above", 0.0, 37.0),
        ("below", 0.0, -9.0),
        ("left", -12.0, 15.0),
        ("right", 52.0, 15.0),
        ("corner", -12.0, -9.0),
    ] {
        let _ = name;
        tp.feed_to(P3::new(dx, dy, 7.0), 600.0);
        tp.feed_to(P3::new(dx + 4.0, dy + 2.0, 7.0), 900.0);
        // A plunge outside the grid too — the degenerate branch's own clamp.
        tp.feed_to(P3::new(dx + 4.0, dy + 2.0, 3.0), 300.0);
    }
    // …and one line genuinely inside, so the fixture is not "nothing happened".
    tp.feed_to(P3::new(4.0, 15.0, 7.0), 600.0);
    tp.feed_to(P3::new(36.0, 15.0, 7.0), 1200.0);
    tp.final_retract(12.0);

    let paths = vec![tp];
    let (serial, _) = replay(
        &paths,
        &cutter,
        0.25,
        StockCutDirection::FromTop,
        PlaybackDispatch::Serial,
    );
    let (banded, stats) = replay(
        &paths,
        &cutter,
        0.25,
        StockCutDirection::FromTop,
        PlaybackDispatch::Banded,
    );
    assert_playback_dispatch_is_not_vacuous(&stats, "off-grid");
    assert_grids_bit_identical(&serial, &banded, "off-grid");
    assert_remaining_stock_probes_identical(&serial, &banded, "off-grid");
}
