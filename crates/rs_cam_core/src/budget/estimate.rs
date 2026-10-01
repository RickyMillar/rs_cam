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
use super::grid::{effective_cell, effective_grid_cells, floor_cell, grid_cells};
use crate::compute::simulate::SimulationRequest;
use crate::geo::BoundingBox3;
use crate::stock::dexel::DexelRay;
use crate::stock::simulation_cut::SimulationCutSample;
use crate::toolpath::{Move, Toolpath};

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

// ── Wave 3: the request load, the trace count and the preflight ─────────

/// The smallest trace sample step, in mm.
///
/// COPY of the literal in `compute/simulate.rs` (`run_simulation_memoized`:
/// `let sample_step_mm = request.resolution.max(0.25);`). The simulator does
/// not call [`trace_sample_step_mm`] yet; a change there must change this
/// value too.
pub const TRACE_SAMPLE_STEP_FLOOR_MM: f64 = 0.25;

/// The trace sample step the simulator uses at `resolution_mm`.
#[must_use]
pub fn trace_sample_step_mm(resolution_mm: f64) -> f64 {
    resolution_mm.max(TRACE_SAMPLE_STEP_FLOOR_MM)
}

/// The cut-trace samples one toolpath keeps, about one per sample step of
/// path length.
///
/// G-SIMMEM (`75aa2869`): "the cut trace keeps one sample per sample step".
/// Each move with a length counts `max(1, ceil(length / step))`; a move with
/// no length counts nothing. The walk's own reservation
/// (`dexel_stock/simulation.rs`, `estimate_sample_count`) is private and
/// coalesces Z-subdivided moves differently, so this count is an estimate
/// for the budget, not the walk's exact count.
#[must_use]
pub fn estimate_trace_samples(toolpath: &Toolpath, sample_step_mm: f64) -> u64 {
    let step = floor_cell(sample_step_mm);
    let mut total = 0u64;
    for pair in toolpath.moves.windows(2) {
        let [from, to] = pair else { continue };
        let length = (to.target - from.target).norm();
        if length.is_nan() || length <= 0.0 {
            continue;
        }
        let by_length = (length / step).ceil();
        let count = if by_length.is_finite() && by_length >= 1.0 {
            by_length as u64
        } else {
            1
        };
        total = total.saturating_add(count);
    }
    total
}

/// What one simulation request will hold: its grid footprint, its cell and
/// its load. The preflight and the worker ledger read the same value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationNeed {
    /// The X extent of the largest grid of the run, in mm.
    pub footprint_w: f64,
    /// The Y extent of the largest grid of the run, in mm.
    pub footprint_d: f64,
    /// The requested cell, in mm, before the ceiling clamp.
    pub cell_mm: f64,
    /// Everything that does not depend on the cell.
    pub load: SimulationLoad,
}

impl SimulationNeed {
    /// The need of `request`.
    ///
    /// - E: the toolpath entries of every group.
    /// - G: the groups (a group with no entry still builds its stock).
    /// - moves: every entry's moves.
    /// - trace: [`estimate_trace_samples`] at [`trace_sample_step_mm`],
    ///   counted only when `metric_options.enabled`.
    /// - footprint: the grid with the most columns among the request's
    ///   `stock_bbox` and each group's `local_stock_bbox`. The model R uses
    ///   ONE cell count for every grid; this takes the largest.
    #[must_use]
    pub fn of_request(request: &SimulationRequest) -> Self {
        let step = trace_sample_step_mm(request.resolution);
        let metrics_on = request.metric_options.enabled;
        let mut load = SimulationLoad {
            setup_groups: bytes(request.groups.len()),
            metrics_on,
            ..SimulationLoad::default()
        };
        for entry in request
            .groups
            .iter()
            .flat_map(|group| group.toolpaths.iter())
        {
            let toolpath = &entry.annotated.toolpath;
            load.simulated_toolpaths = load.simulated_toolpaths.saturating_add(1);
            load.moves = load.moves.saturating_add(bytes(toolpath.moves.len()));
            if metrics_on {
                load.trace_samples = load
                    .trace_samples
                    .saturating_add(estimate_trace_samples(toolpath, step));
            }
        }
        let boxes = std::iter::once(&request.stock_bbox).chain(
            request
                .groups
                .iter()
                .filter_map(|group| group.local_stock_bbox.as_ref()),
        );
        Self::over_largest(boxes, request.resolution, load)
    }

    /// The need of `load` over the largest of `boxes` at `cell_mm`.
    #[must_use]
    pub fn over_largest<'a>(
        boxes: impl IntoIterator<Item = &'a BoundingBox3>,
        cell_mm: f64,
        load: SimulationLoad,
    ) -> Self {
        let mut best = (0.0, 0.0, 0usize);
        for bbox in boxes {
            let w = (bbox.max.x - bbox.min.x).max(0.0);
            let d = (bbox.max.y - bbox.min.y).max(0.0);
            let cells = effective_grid_cells(w, d, cell_mm);
            if cells > best.2 {
                best = (w, d, cells);
            }
        }
        Self {
            footprint_w: best.0,
            footprint_d: best.1,
            cell_mm,
            load,
        }
    }

    /// The columns of the largest grid, after the floor and the ceiling
    /// clamp.
    #[must_use]
    pub fn cells(&self) -> u64 {
        bytes(effective_grid_cells(
            self.footprint_w,
            self.footprint_d,
            self.cell_mm,
        ))
    }

    /// The cell the grid really uses, in mm, after the clamp.
    #[must_use]
    pub fn effective_cell_mm(&self) -> f64 {
        effective_cell(self.cell_mm, self.footprint_w, self.footprint_d)
    }

    /// The estimate in bytes (the plan's R for this run).
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.load.estimate(self.cells()).total()
    }
}

/// Why the preflight refused a simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreflightRefusal {
    /// The estimate of the run, in bytes.
    pub need_bytes: u64,
    /// The limit, in bytes.
    pub limit_bytes: u64,
    /// The resident size of an otherwise idle process, in bytes.
    pub baseline_bytes: u64,
    /// The finest cell whose estimate fits beside the baseline.
    pub fit: CellFit,
    /// The cell the refused run asked for, after the ceiling clamp, in mm.
    pub requested_cell_mm: f64,
}

/// The preflight: `Ok` when `baseline_bytes + need` fits the budget, or
/// when the budget has no limit.
///
/// `baseline_bytes` is the resident size of an otherwise IDLE process, not
/// the live reading: a live reading also counts the result this run
/// replaces and the jobs the ledger is about to finish. So a refusal means
/// that even an idle process cannot hold the run. A run that passes can
/// still meet the guard during the run.
///
/// The refusal carries the finest cell that fits the room beside the
/// baseline ([`largest_cell_that_fits`]). The grid is never coarsened
/// here; the caller names the cell, and the operator decides.
///
/// # Errors
/// [`PreflightRefusal`] when the run does not fit.
pub fn preflight_simulation(
    budget: &MemoryBudget,
    baseline_bytes: u64,
    need: &SimulationNeed,
) -> Result<(), PreflightRefusal> {
    let Some(limit_bytes) = budget.limit_bytes else {
        return Ok(());
    };
    let need_bytes = need.bytes();
    if baseline_bytes.saturating_add(need_bytes) <= limit_bytes {
        return Ok(());
    }
    let room = MemoryBudget::with_limit(limit_bytes.saturating_sub(baseline_bytes));
    let fit = largest_cell_that_fits(&room, need.footprint_w, need.footprint_d, &need.load);
    Err(PreflightRefusal {
        need_bytes,
        limit_bytes,
        baseline_bytes,
        fit,
        requested_cell_mm: need.effective_cell_mm(),
    })
}

// SAFETY: test module; a failed `let ... else` is a failed test.
#[allow(clippy::panic)]
#[cfg(test)]
mod wave3_tests {
    //! The preflight arithmetic. The limits are arbitrary values on the two
    //! sides of an estimate that the real types give; none is a memory
    //! claim.

    use super::{
        CellFit, MemoryBudget, SimulationLoad, SimulationNeed, estimate_trace_samples,
        preflight_simulation,
    };
    use crate::geo::{BoundingBox3, P3};
    use crate::toolpath::Toolpath;

    fn need() -> SimulationNeed {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(100.0, 50.0, 10.0),
        };
        let load = SimulationLoad {
            simulated_toolpaths: 3,
            setup_groups: 1,
            moves: 10,
            trace_samples: 0,
            metrics_on: false,
        };
        SimulationNeed::over_largest([&bbox], 0.5, load)
    }

    #[test]
    fn the_trace_count_is_one_sample_per_step_of_length() {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(0.0, 0.0, 5.0), 100.0);
        toolpath.feed_to(P3::new(10.0, 0.0, 5.0), 100.0);
        // 0 for the zero-length move, ceil(10 / 0.3) = 34 for the cut.
        assert_eq!(estimate_trace_samples(&toolpath, 0.3), 34);
    }

    #[test]
    fn the_preflight_refuses_only_past_the_room_beside_the_baseline() {
        let need = need();
        let bytes = need.bytes();
        let baseline = bytes / 4;
        let fits = MemoryBudget::with_limit(baseline + bytes);
        assert_eq!(preflight_simulation(&fits, baseline, &need), Ok(()));
        assert_eq!(
            preflight_simulation(&MemoryBudget::UNLIMITED, u64::MAX, &need),
            Ok(())
        );

        let tight = MemoryBudget::with_limit(baseline + bytes - 1);
        let Err(refusal) = preflight_simulation(&tight, baseline, &need) else {
            panic!("one byte short must refuse");
        };
        assert_eq!(refusal.need_bytes, bytes);
        assert_eq!(refusal.limit_bytes, baseline + bytes - 1);
        assert_eq!(refusal.baseline_bytes, baseline);
        let CellFit::Fits(cell) = refusal.fit else {
            panic!("a coarser cell fits: {:?}", refusal.fit);
        };
        assert!(cell > refusal.requested_cell_mm, "the fit is coarser");
    }
}
