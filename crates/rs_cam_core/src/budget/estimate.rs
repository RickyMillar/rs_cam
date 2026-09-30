//! Memory estimators: what a job will hold, before it runs (B1).
//!
//! The formula is the model R of `planning/memory_budget_2026-10-01/PLAN.md`
//! ("Model"). Every per-element size is `size_of` of the real type, never a
//! typed number, so a change to a type moves the estimate with it. W0
//! measures R on the benchmark fixtures; a term that W0 finds wrong changes
//! here and nowhere else.
//!
//! The estimate counts the inline size of each element. It does not count
//! heap memory that an element owns: a dexel ray with more than one segment
//! (`SmallVec` spill), and the `span_path` vector of a cut sample. So it is a
//! lower bound on a part with many multi-segment rays.

use std::mem::size_of;

use super::MemoryBudget;
use super::grid::{floor_cell, grid_cells};
use crate::stock::dexel::DexelRay;
use crate::stock::simulation_cut::SimulationCutSample;
use crate::toolpath::Move;

/// Bytes as `u64`. `usize` is never wider than 64 bits on a supported
/// target; a wider value saturates.
fn bytes(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Bytes of one dexel column: one [`DexelRay`] in `DexelGrid::rays` plus one
/// `f32` in `DexelGrid::conservative_top` (`stock/dexel.rs`, the
/// `DexelGrid` fields).
#[must_use]
pub fn dexel_cell_bytes() -> u64 {
    bytes(size_of::<DexelRay>() + size_of::<f32>())
}

/// Bytes of the display mesh per grid column.
///
/// `stock/dexel_mesh_mc.rs` (`z_grid_marching_cubes`, the capacity block
/// after step 3) reserves, per grid corner, a top and a bottom vertex of
/// three `f32` in `vertices` and the same again in `colors`, and per cell
/// twelve `u32` in `indices`. A grid has `(rows + 1) x (cols + 1)` corners;
/// the estimate counts one corner per cell, which is exact in the limit of a
/// large grid. The mesh keeps its reserved capacity.
#[must_use]
pub fn mesh_cell_bytes() -> u64 {
    /// Vertices per corner: the top and the bottom surface.
    const VERTICES_PER_CORNER: usize = 2;
    /// Components per vertex position and per vertex colour.
    const COMPONENTS: usize = 3;
    /// Indices per cell, from `Vec::with_capacity(cells * 12)`.
    const INDICES_PER_CELL: usize = 12;
    let positions = VERTICES_PER_CORNER * COMPONENTS * size_of::<f32>();
    let colours = VERTICES_PER_CORNER * COMPONENTS * size_of::<f32>();
    let indices = INDICES_PER_CELL * size_of::<u32>();
    bytes(positions + colours + indices)
}

/// Bytes of one toolpath move ([`Move`], `toolpath.rs`).
#[must_use]
pub fn move_bytes() -> u64 {
    bytes(size_of::<Move>())
}

/// Bytes of one cut-trace sample ([`SimulationCutSample`],
/// `stock/simulation_cut.rs`), without its `span_path` heap.
#[must_use]
pub fn trace_sample_bytes() -> u64 {
    bytes(size_of::<SimulationCutSample>())
}

/// Everything in a simulation that does not depend on the cell size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimulationLoad {
    /// E: toolpaths that the simulation carves.
    pub simulated_toolpaths: u64,
    /// G: setup groups.
    pub setup_groups: u64,
    /// Toolpath moves that the result keeps.
    pub moves: u64,
    /// Cut-trace samples, counted only when `metrics_on`.
    pub trace_samples: u64,
    /// Metric capture is on, so the result keeps the trace
    /// (`compute/simulate.rs`, the `append_samples` call under
    /// `metric_options.enabled`).
    pub metrics_on: bool,
}

/// The estimate of one kept simulation result, term by term, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimulationEstimate {
    /// E checkpoint stocks: `push_checkpoint` keeps `checkpoint()` of a
    /// stock per toolpath (`compute/simulate.rs`, `push_checkpoint`).
    pub checkpoint_stock: u64,
    /// E prior stocks: `Arc::new(group_stock.clone())` before each toolpath
    /// carves (`compute/simulate.rs`, `prior_stocks.insert`).
    pub prior_stock: u64,
    /// E checkpoint meshes: `dexel_stock_to_mesh` per toolpath
    /// (`compute/simulate.rs`, `push_checkpoint`).
    pub checkpoint_mesh: u64,
    /// G composite meshes, twice: the result's `composite_mesh`
    /// (`compute/simulate.rs`, `run.composite_mesh.append`) and the copy the
    /// session keeps (`rs_cam_viz/src/controller/events/compute.rs`, M4).
    pub composite_mesh: u64,
    /// One live playback stock (`rs_cam_viz/src/controller/events/compute.rs`,
    /// `playback.live_stock`, M5).
    pub live_stock: u64,
    /// The moves the result keeps.
    pub moves: u64,
    /// The cut trace; zero when metric capture is off.
    pub trace: u64,
}

impl SimulationEstimate {
    /// The sum of every term, saturating.
    #[must_use]
    pub fn total(&self) -> u64 {
        [
            self.checkpoint_stock,
            self.prior_stock,
            self.checkpoint_mesh,
            self.composite_mesh,
            self.live_stock,
            self.moves,
            self.trace,
        ]
        .iter()
        .fold(0u64, |sum, term| sum.saturating_add(*term))
    }
}

impl SimulationLoad {
    /// The cost of one grid column over the whole result:
    /// `E x (2 x dexel + mesh) + G x 2 x mesh + dexel`.
    #[must_use]
    pub fn bytes_per_cell(&self) -> u64 {
        self.estimate(1).total().saturating_sub(self.fixed_bytes())
    }

    /// The part of the estimate that the cell size does not change: the
    /// moves and the trace.
    #[must_use]
    pub fn fixed_bytes(&self) -> u64 {
        let trace = if self.metrics_on {
            self.trace_samples.saturating_mul(trace_sample_bytes())
        } else {
            0
        };
        self.moves
            .saturating_mul(move_bytes())
            .saturating_add(trace)
    }

    /// The estimate at `cells` grid columns, term by term.
    #[must_use]
    pub fn estimate(&self, cells: u64) -> SimulationEstimate {
        let dexel = cells.saturating_mul(dexel_cell_bytes());
        let mesh = cells.saturating_mul(mesh_cell_bytes());
        let e = self.simulated_toolpaths;
        let g = self.setup_groups;
        SimulationEstimate {
            checkpoint_stock: e.saturating_mul(dexel),
            prior_stock: e.saturating_mul(dexel),
            checkpoint_mesh: e.saturating_mul(mesh),
            composite_mesh: g.saturating_mul(2).saturating_mul(mesh),
            live_stock: dexel,
            moves: self.moves.saturating_mul(move_bytes()),
            trace: if self.metrics_on {
                self.trace_samples.saturating_mul(trace_sample_bytes())
            } else {
                0
            },
        }
    }
}

/// The bytes one kept simulation result holds (the plan's R), total.
///
/// `cells` is the columns of ONE grid; use
/// [`super::grid::effective_grid_cells`] for the cell the grid really uses.
#[must_use]
pub fn estimate_simulation_bytes(
    cells: u64,
    simulated_toolpaths: u64,
    setup_groups: u64,
    moves: u64,
    trace_samples: u64,
    metrics_on: bool,
) -> u64 {
    SimulationLoad {
        simulated_toolpaths,
        setup_groups,
        moves,
        trace_samples,
        metrics_on,
    }
    .estimate(cells)
    .total()
}

/// The answer of [`largest_cell_that_fits`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellFit {
    /// The budget has no limit, so every cell fits.
    Unlimited,
    /// The finest cell, in mm, whose simulation fits the budget.
    Fits(f64),
    /// No cell fits: the moves and the trace alone, or the smallest
    /// possible grid, already need more than the limit.
    Nothing {
        /// The estimate at the coarsest possible grid, in bytes.
        need_bytes: u64,
        /// The limit, in bytes.
        limit_bytes: u64,
    },
}

/// The finest cell size (so the LARGEST grid) whose simulation estimate fits
/// the budget, for a refusal message ("the cell that fits is X mm").
///
/// The grid column count falls as the cell grows, so a bisection between a
/// cell that does not fit and one that does finds the boundary. The loop
/// stops when the midpoint is no longer strictly between the two ends, that
/// is at `f64` precision. The answer ignores the dexel ceiling clamp; the
/// caller compares it with [`super::grid::effective_cell`].
#[must_use]
pub fn largest_cell_that_fits(
    budget: &MemoryBudget,
    footprint_w: f64,
    footprint_d: f64,
    load: &SimulationLoad,
) -> CellFit {
    let Some(limit) = budget.limit_bytes else {
        return CellFit::Unlimited;
    };
    let w = footprint_w.max(0.0);
    let d = footprint_d.max(0.0);
    let fits = |cell: f64| -> bool {
        let cells = bytes(grid_cells(w, d, cell));
        load.estimate(cells).total() <= limit
    };
    // The coarsest grid: one cell spans the whole footprint.
    let coarsest = floor_cell(w.max(d));
    if !fits(coarsest) {
        let cells = bytes(grid_cells(w, d, coarsest));
        return CellFit::Nothing {
            need_bytes: load.estimate(cells).total(),
            limit_bytes: limit,
        };
    }
    let per_cell = load.bytes_per_cell().max(1);
    let max_cells = limit.saturating_sub(load.fixed_bytes()) / per_cell;
    // At `sqrt(w d / N)` the grid holds more than `(w / c)(d / c) = N`
    // columns, so this start does not fit, unless the floor already fits.
    let start = floor_cell(((w * d) / max_cells.max(1) as f64).sqrt());
    if fits(start) {
        return CellFit::Fits(start);
    }
    let (mut lo, mut hi) = (start, coarsest);
    loop {
        let mid = lo + (hi - lo) / 2.0;
        if mid <= lo || mid >= hi {
            break;
        }
        if fits(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    CellFit::Fits(hi)
}
