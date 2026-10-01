//! B1, B2, B3 — the memory budget core.
//!
//! Plan: `planning/memory_budget_2026-10-01/PLAN.md`, findings B1 (no
//! estimate before a job), B2 (four grid caps disagree) and B3 (a cancel
//! has no reason).
//!
//! - The estimators are the plan's R formula, with every element size from
//!   `size_of` of the real type. The expected values below are derived by
//!   hand from the same `size_of` calls, term by term.
//! - The three grid caps now come from one source. The tests compare each
//!   against the legacy literal formula, bit for bit, so current callers get
//!   the same cells.
//! - The guard records WHY a job stopped, and the first reason stays.

// SAFETY: test module; a failed unwrap or panic is a failed test.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::mem::size_of;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rs_cam_core::budget::estimate::{
    dexel_cell_bytes, mesh_cell_bytes, move_bytes, trace_sample_bytes,
};
use rs_cam_core::budget::grid::{
    GRID_CELL_CEILING, ceiling_clamp, cell_for_cap, effective_cell, effective_grid_cells,
};
use rs_cam_core::budget::{
    BudgetGuard, CellFit, GridCapRole, MemoryBudget, SimulationLoad, StopReason, UsageProbe,
    estimate_simulation_bytes, grid_cells, largest_cell_that_fits,
};
use rs_cam_core::interrupt::{CancelCheck, FlagCancel};
use rs_cam_core::stock::dexel::{DexelGrid, DexelRay};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::toolpath::Move;

// ── Estimators ─────────────────────────────────────────────────────────

#[test]
fn per_element_sizes_come_from_the_real_types() {
    assert_eq!(
        dexel_cell_bytes(),
        (size_of::<DexelRay>() + size_of::<f32>()) as u64
    );
    // Marching cubes reserves, per corner, top + bottom vertices of three
    // f32 for position and three for colour, and 12 u32 indices per cell.
    let mesh = 2 * 3 * size_of::<f32>() + 2 * 3 * size_of::<f32>() + 12 * size_of::<u32>();
    assert_eq!(mesh_cell_bytes(), mesh as u64);
    assert_eq!(move_bytes(), size_of::<Move>() as u64);
    assert_eq!(
        trace_sample_bytes(),
        size_of::<SimulationCutSample>() as u64
    );
}

#[test]
fn the_simulation_estimate_is_the_held_result_formula() {
    let d = dexel_cell_bytes();
    let m = mesh_cell_bytes();
    let (cells, e, g, moves, samples) = (1_000_u64, 7_u64, 2_u64, 10_u64, 5_u64);
    // After waves 1-2 (`budget/estimate.rs`, the module doc):
    // R = C x [E x checkpoint stock + E x prior stock + G x group tail stock
    //          + G x composite mesh + (2 x mesh + dexel) scrub]
    //     + moves + trace
    let at_rest_per_cell = e * d + e * d + g * d + g * m;
    let scrub_per_cell = 2 * m + d;
    let per_cell = at_rest_per_cell + scrub_per_cell;
    let with_trace = cells * per_cell + moves * move_bytes() + samples * trace_sample_bytes();
    let without_trace = cells * per_cell + moves * move_bytes();
    assert_eq!(
        estimate_simulation_bytes(cells, e, g, moves, samples, true),
        with_trace
    );
    assert_eq!(
        estimate_simulation_bytes(cells, e, g, moves, samples, false),
        without_trace
    );

    let load = SimulationLoad {
        simulated_toolpaths: e,
        setup_groups: g,
        moves,
        trace_samples: samples,
        metrics_on: true,
    };
    assert_eq!(load.bytes_per_cell(), per_cell);
    assert_eq!(
        load.fixed_bytes(),
        moves * move_bytes() + samples * trace_sample_bytes()
    );
    let terms = load.estimate(cells);
    assert_eq!(terms.checkpoint_stock, cells * e * d);
    assert_eq!(terms.prior_stock, cells * e * d);
    assert_eq!(terms.group_tail_stock, cells * g * d);
    assert_eq!(terms.composite_mesh, cells * g * m);
    assert_eq!(terms.scrub, cells * scrub_per_cell);
    assert_eq!(terms.total(), with_trace);
    assert_eq!(terms.at_rest(), with_trace - cells * scrub_per_cell);
}

/// The MEASURED anchors of `planning/memory_budget_2026-10-01/BASELINES.md`
/// on rivmap350 at 0.2 mm (C = 1901 x 2551 columns, E = 7, G = 2).
///
/// The model has no lower bound from an anchor: RSS also holds the
/// generation results, the allocator overhead and the terms the module doc
/// lists as not counted. The moves count of the fixture is not in the
/// baselines, so the moves term is 0 here. So each check is `<=` only.
///
/// - W1, GUI, metrics OFF: held after `generate_all` = 4.28 - 0.65 GiB (the
///   load step). No scrub had run, so the anchor reads `at_rest`. W1 was
///   before W2-E; W2-E removed only handles the view shares, so the held
///   result at rest is the same or smaller now.
/// - W2, CLI, metrics ON: peak RSS 7 286 720 kB with 5 159 038 trace
///   samples. The CLI never scrubs, so the anchor reads `at_rest`.
#[test]
fn the_held_result_estimate_sits_at_or_below_the_measured_anchors() {
    const GIB: u64 = 1 << 30;
    let cells = grid_cells(380.0, 510.0, 0.2) as u64;
    assert_eq!(cells, 1901 * 2551);
    let gui_metrics_off = SimulationLoad {
        simulated_toolpaths: 7,
        setup_groups: 2,
        moves: 0,
        trace_samples: 0,
        metrics_on: false,
    };
    let w1_held = (428 - 65) * GIB / 100;
    let gui = gui_metrics_off.estimate(cells);
    assert!(
        gui.at_rest() <= w1_held,
        "W1 GUI anchor: estimate {} B > measured {} B",
        gui.at_rest(),
        w1_held
    );

    let cli_metrics_on = SimulationLoad {
        trace_samples: 5_159_038,
        metrics_on: true,
        ..gui_metrics_off
    };
    let w2_peak = 7_286_720 * 1024;
    let cli = cli_metrics_on.estimate(cells);
    assert!(
        cli.at_rest() <= w2_peak,
        "W2 CLI anchor: estimate {} B > measured peak {} B",
        cli.at_rest(),
        w2_peak
    );
}

#[test]
fn grid_cells_uses_the_dexel_rounding() {
    // rivmap350: 380 x 510 mm at 0.2 mm is 1901 x 2551 (plan, "Model").
    assert_eq!(grid_cells(380.0, 510.0, 0.2), 1901 * 2551);
    // The count equals what `DexelGrid::from_bounds` allocates.
    let bbox = rs_cam_core::geo::BoundingBox3 {
        min: rs_cam_core::geo::P3::new(0.0, 0.0, -5.0),
        max: rs_cam_core::geo::P3::new(37.3, 12.9, 0.0),
    };
    for cell in [0.1, 0.25, 0.7, 3.0] {
        let grid = DexelGrid::z_grid_from_bounds(&bbox, cell);
        assert_eq!(
            grid_cells(37.3, 12.9, cell),
            grid.rows * grid.cols,
            "cell {cell}"
        );
        assert_eq!(
            effective_grid_cells(37.3, 12.9, cell),
            grid.rows * grid.cols
        );
    }
}

#[test]
fn the_finest_cell_that_fits_is_on_the_budget_boundary() {
    let load = SimulationLoad {
        simulated_toolpaths: 7,
        setup_groups: 2,
        moves: 800_000,
        trace_samples: 0,
        metrics_on: false,
    };
    let (w, d) = (380.0, 510.0);
    let limit = 8_u64 << 30;
    let budget = MemoryBudget::with_limit(limit);
    let fits = |cell: f64| load.estimate(grid_cells(w, d, cell) as u64).total() <= limit;
    let CellFit::Fits(cell) = largest_cell_that_fits(&budget, w, d, &load) else {
        panic!("an 8 GiB budget fits some cell");
    };
    assert!(fits(cell), "the answer fits");
    let finer = f64::from_bits(cell.to_bits() - 1);
    assert!(!fits(finer), "the next finer f64 does not fit");

    assert_eq!(
        largest_cell_that_fits(&MemoryBudget::UNLIMITED, w, d, &load),
        CellFit::Unlimited
    );
    let tiny = MemoryBudget::with_limit(1);
    assert!(matches!(
        largest_cell_that_fits(&tiny, w, d, &load),
        CellFit::Nothing { limit_bytes: 1, .. }
    ));
}

// ── Grid caps (B2) ─────────────────────────────────────────────────────

/// `DexelGrid::would_exceed_grid` before B2, with its literal 16 M.
fn legacy_would_exceed(cell: f64, u: f64, v: f64) -> Option<f64> {
    let cs = if cell < 1e-6 { 1e-6 } else { cell };
    let cols = (u / cs).ceil() as usize + 1;
    let rows = (v / cs).ceil() as usize + 1;
    // The legacy `rows * cols` overflowed on a 5 m box at the 1e-6 floor;
    // saturation only differs from it where it panicked.
    if rows.saturating_mul(cols) > 16_000_000 {
        Some((u * v / 16_000_000_f64).sqrt())
    } else {
        None
    }
}

#[test]
fn the_three_caps_keep_their_legacy_values() {
    assert_eq!(GRID_CELL_CEILING, 16_000_000);
    assert_eq!(GridCapRole::Ceiling.cell_cap(), 16_000_000);
    assert_eq!(GridCapRole::AutoResolution.cell_cap(), 8_000_000);
    assert_eq!(GridCapRole::EntryReplay.cell_cap(), 4_000_000);
}

#[test]
fn old_callers_get_the_same_cells_bit_for_bit() {
    let extents = [
        (100.0, 100.0),
        (380.0, 510.0),
        (1220.0, 2440.0),
        (5000.0, 5000.0),
    ];
    let cells = [0.0, -1.0, 1e-7, 0.02, 0.05, 0.1, 0.2, 0.5, 2.0];
    for (u, v) in extents {
        for cell in cells {
            let legacy = legacy_would_exceed(cell, u, v);
            assert_eq!(
                DexelGrid::would_exceed_grid(cell, u, v).map(f64::to_bits),
                legacy.map(f64::to_bits),
                "would_exceed {cell} {u}x{v}"
            );
            assert_eq!(
                ceiling_clamp(cell, u, v).map(f64::to_bits),
                legacy.map(f64::to_bits)
            );
            let legacy_effective = legacy.unwrap_or_else(|| cell.max(1e-6));
            assert_eq!(
                DexelGrid::effective_cell_size(cell, u, v).to_bits(),
                legacy_effective.to_bits(),
                "effective {cell} {u}x{v}"
            );
            assert_eq!(
                effective_cell(cell, u, v).to_bits(),
                legacy_effective.to_bits()
            );
        }
        // The entry replay prism: `(w * h / 4.0e6).sqrt()` before B2.
        assert_eq!(
            cell_for_cap(u, v, GridCapRole::EntryReplay.cell_cap()).to_bits(),
            (u * v / 4.0e6).sqrt().to_bits()
        );
        // The auto rule: `((sx * sy) / 8_000_000.0).sqrt()` before B2.
        assert_eq!(
            cell_for_cap(u, v, GridCapRole::AutoResolution.cell_cap()).to_bits(),
            ((u * v) / 8_000_000.0).sqrt().to_bits()
        );
    }
}

#[test]
fn a_budget_only_tightens_a_cap() {
    let per_cell = 1_000;
    let tight = MemoryBudget::with_limit(per_cell * 1_000);
    assert_eq!(GridCapRole::Ceiling.cell_cap_under(&tight, per_cell), 1_000);
    let loose = MemoryBudget::with_limit(u64::MAX);
    assert_eq!(
        GridCapRole::Ceiling.cell_cap_under(&loose, per_cell),
        GRID_CELL_CEILING
    );
    assert_eq!(
        GridCapRole::AutoResolution.cell_cap_under(&MemoryBudget::UNLIMITED, per_cell),
        GridCapRole::AutoResolution.cell_cap()
    );
}

// ── Guard (B3) ─────────────────────────────────────────────────────────

struct FakeProbe(Arc<AtomicU64>);

impl UsageProbe for FakeProbe {
    fn used_bytes(&self) -> Option<u64> {
        Some(self.0.load(Ordering::SeqCst))
    }
}

fn guard(limit: Option<u64>, used: u64, interval: Duration) -> (BudgetGuard, Arc<AtomicU64>) {
    let usage = Arc::new(AtomicU64::new(used));
    let guard = BudgetGuard::with_probe(
        Arc::new(AtomicBool::new(false)),
        MemoryBudget { limit_bytes: limit },
        Box::new(FakeProbe(Arc::clone(&usage))),
        interval,
    );
    (guard, usage)
}

#[test]
fn a_user_cancel_is_recorded_as_user() {
    let (g, _) = guard(Some(100), 0, Duration::ZERO);
    assert!(!g.cancelled());
    assert_eq!(g.stop_reason(), None);
    g.request_cancel();
    assert!(g.cancelled());
    assert_eq!(g.stop_reason(), Some(StopReason::User));
}

#[test]
fn usage_over_the_limit_stops_with_over_budget() {
    let (g, usage) = guard(Some(100), 50, Duration::ZERO);
    assert!(!g.cancelled());
    usage.store(150, Ordering::SeqCst);
    assert!(g.cancelled());
    assert!(g.flag().load(Ordering::SeqCst), "the shared flag is set");
    assert_eq!(
        g.stop_reason(),
        Some(StopReason::OverBudget {
            need_bytes: 150,
            limit_bytes: 100,
        })
    );
    // The first reason stays.
    g.request_cancel();
    assert!(matches!(
        g.stop_reason(),
        Some(StopReason::OverBudget { .. })
    ));
}

#[test]
fn a_flag_set_elsewhere_reads_as_user_and_no_limit_never_trips() {
    let (g, _) = guard(None, u64::MAX, Duration::ZERO);
    assert!(!g.cancelled(), "no limit: usage never stops the job");
    g.flag().store(true, Ordering::SeqCst);
    assert!(g.cancelled());
    assert_eq!(g.stop_reason(), Some(StopReason::User));
}

#[test]
fn the_probe_obeys_its_rate_limit() {
    let (g, usage) = guard(Some(100), 50, Duration::from_secs(3600));
    assert!(!g.poll(), "the first poll reads the probe: under the limit");
    usage.store(150, Ordering::SeqCst);
    assert!(!g.poll(), "inside the interval the probe is not read");
    assert!(g.probe_now(), "probe_now ignores the rate limit");
    assert!(g.poll());
}

#[test]
fn check_estimate_refuses_before_allocation() {
    let (g, _) = guard(Some(100), 0, Duration::ZERO);
    assert_eq!(g.check_estimate(100), Ok(()));
    let refused = StopReason::OverBudget {
        need_bytes: 101,
        limit_bytes: 100,
    };
    assert_eq!(g.check_estimate(101), Err(refused));
    assert_eq!(g.stop_reason(), Some(refused));
    assert_eq!(
        g.check_estimate(1),
        Err(refused),
        "a stopped job stays stopped"
    );
}

#[test]
fn a_flag_reader_stops_when_the_watcher_trips_the_budget() {
    let (g, usage) = guard(Some(100), 50, Duration::from_millis(1));
    let g = Arc::new(g);
    let watch = g.watch();
    // An algorithm that only reads `&AtomicBool`, through the adapter.
    let reader = FlagCancel(g.flag());
    assert!(!reader.cancelled());
    // The stop rule (`budget/guard.rs`): the guard stops a job only when the
    // process crosses the limit DURING the job. So wait until the watcher
    // has read a value under the limit, then cross it.
    let start = Instant::now();
    while !g.is_armed() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the watcher did not read the probe"
        );
        std::thread::yield_now();
    }
    usage.store(150, Ordering::SeqCst);
    let start = Instant::now();
    while !reader.cancelled() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the watcher did not trip the flag"
        );
        std::thread::yield_now();
    }
    drop(watch);
    assert!(matches!(
        g.stop_reason(),
        Some(StopReason::OverBudget { .. })
    ));
}
