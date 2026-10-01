#![deny(clippy::indexing_slicing)]

pub mod worker;

use std::sync::Arc;
use std::time::{Duration, Instant};

pub use worker::{
    CollisionRequest, CollisionResult, ComputeRequest, ComputeResult, JobRequest, JobResult,
    OptimizeRequest, OptimizeResult, OptimizeResultKind, ReachRequest, ReachResult,
    ArtifactPolicy, SetupTransformInfo, SimulationRequest, SimulationResult,
    ThreadedComputeBackend, VizExtras,
};

/// Identifies one [`JobRequest`] submit, so the drain routes its answer
/// back to the caller that asked.
///
/// A counter, not the row id: two `preview_tier_map` calls can be in
/// flight at once, and a key that named the ROW would let the second
/// answer be delivered to the first caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobRequestId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeLane {
    Toolpath,
    Analysis,
    /// Worker for the PROJECT Optimize rollup.
    ///
    /// The request owns a CLONE of the `ProjectSession` for the run and
    /// drops it; the result carries the report alone. The per-toolpath
    /// search left this lane in WP14b and rides [`Self::Job`] now.
    Optimize,
    /// Worker for a core `Job` row's work step (WP14a).
    ///
    /// **Its own lane, and it carries no session of the main thread's.**
    /// Two of the three rows that ride it — `recommend_clearing_strategy`
    /// and `preview_tier_map` — are READS whose step (ii) is a free
    /// function over a handle. The third, `optimize_toolpath` (WP14b),
    /// mutates, but it mutates the CLONE its own handle owns. Either way
    /// the session stays on the main thread and stays usable while the
    /// job runs.
    ///
    /// The queue is FIFO and a submit supersedes nothing. Two previews at
    /// different dial sets are two different questions, and dropping the
    /// first would answer neither.
    Job,
    /// Worker for the per-tool reach-map overlay (P5).
    ///
    /// **Its own lane, not a fourth `AnalysisRequest` variant.** The Analysis
    /// lane's submit rule is "latest job wins": it clears the queue and
    /// cancels whatever is running, through one shared cancel flag. That is
    /// right for a simulation superseding a simulation, and wrong in both
    /// directions here — selecting a toolpath would kill a running
    /// simulation, and starting a simulation would drop a queued reach map
    /// and leave the overlay reading `computing…` for ever. A reach walk is
    /// also seconds, so queueing it behind a minutes-long simulation would
    /// make an overlay wait on a verification run it has nothing to do with.
    Reach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneState {
    Idle,
    Queued,
    Running,
    Cancelling,
}

#[derive(Debug, Clone)]
pub struct LaneSnapshot {
    pub lane: ComputeLane,
    pub state: LaneState,
    pub queue_depth: usize,
    pub current_job: Option<String>,
    pub current_phase: Option<String>,
    pub started_at: Option<Instant>,
    /// A/M12: stable id of the toolpath the lane is chewing on right now.
    /// `None` on the analysis / optimize lanes and whenever the lane is idle.
    pub active_toolpath_id: Option<usize>,
    /// A/M12: 0-based position of that toolpath in
    /// `ProjectSession::toolpath_configs()` at submit time. Stamped onto the
    /// [`worker::ComputeRequest`] by the controller because the lane has no
    /// session access of its own — an MCP `generation_status` served off the
    /// GUI thread has no other way to say *which* op is in flight.
    pub active_toolpath_index: Option<usize>,
}

impl LaneSnapshot {
    pub fn idle(lane: ComputeLane) -> Self {
        Self {
            lane,
            state: LaneState::Idle,
            queue_depth: 0,
            current_job: None,
            current_phase: None,
            started_at: None,
            active_toolpath_id: None,
            active_toolpath_index: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, LaneState::Running | LaneState::Cancelling)
    }

    pub fn elapsed(&self) -> Option<Duration> {
        self.started_at.map(|started_at| started_at.elapsed())
    }
}

/// What a [`GenerationControl::request_cancel`] actually did.
#[derive(Debug, Clone)]
pub struct CancelOutcome {
    /// `true` when the toolpath lane was Running/Cancelling **at the moment
    /// the flag was set** — not at the moment the caller asked. Before A/M12
    /// this call queued behind the GUI frame loop, so a `was_busy: false`
    /// could be reported by a cancel that was serviced minutes after it was
    /// issued (measured live 2026-07-30). It is now serviced synchronously on
    /// the caller's own thread, so the two moments coincide.
    pub was_busy: bool,
    /// Lane state as observed while setting the flag.
    pub snapshot: LaneSnapshot,
}

/// Observe and abort the toolpath compute lane **from any thread**, taking no
/// lock that a running generation can hold for longer than a few
/// instructions.
///
/// A/M12: every MCP call used to reach the engine through exactly one door —
/// `RsCamApp::drain_mcp_requests`, which runs on the egui main thread once per
/// repaint and handles requests strictly sequentially. Anything that stalls
/// that thread therefore stalls `cancel_generation` and every status read too,
/// which is precisely when they are needed. This handle is the second door:
/// it wraps the lane's own `AtomicBool` + its short-critical-section
/// `Mutex<LaneInner>` (held only for the microseconds it takes to push/pop a
/// queue entry or stamp a phase string), so the escape hatches never queue
/// behind a generation.
#[derive(Clone)]
pub struct GenerationControl(Arc<dyn LaneControl>);

/// The lane-side half of [`GenerationControl`]. Implemented by the real
/// `LaneQueue<ComputeRequest>`; test backends use
/// [`GenerationControl::detached`].
pub trait LaneControl: Send + Sync {
    fn snapshot(&self) -> LaneSnapshot;
    /// Set the lane's cancel flag if it is busy. Must not block on anything a
    /// running generation holds.
    fn request_cancel(&self) -> CancelOutcome;
}

struct DetachedLane;

impl LaneControl for DetachedLane {
    fn snapshot(&self) -> LaneSnapshot {
        LaneSnapshot::idle(ComputeLane::Toolpath)
    }

    fn request_cancel(&self) -> CancelOutcome {
        CancelOutcome {
            was_busy: false,
            snapshot: LaneSnapshot::idle(ComputeLane::Toolpath),
        }
    }
}

impl GenerationControl {
    pub fn new(inner: Arc<dyn LaneControl>) -> Self {
        Self(inner)
    }

    /// A control wired to nothing: always idle, cancels nothing. For backends
    /// that run no lane (scripted test doubles).
    pub fn detached() -> Self {
        Self(Arc::new(DetachedLane))
    }

    pub fn snapshot(&self) -> LaneSnapshot {
        self.0.snapshot()
    }

    pub fn request_cancel(&self) -> CancelOutcome {
        self.0.request_cancel()
    }
}

impl std::fmt::Debug for GenerationControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenerationControl")
            .field("snapshot", &self.0.snapshot())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeError {
    /// The operator cancelled the job, or a resubmit superseded it.
    Cancelled,
    /// The memory budget stopped the job (plan B3,
    /// `planning/memory_budget_2026-10-01/PLAN.md`).
    ///
    /// A lane gives this variant when its job's
    /// `rs_cam_core::budget::BudgetGuard` records
    /// `StopReason::OverBudget`. It is never shown as a plain "Cancelled":
    /// the operator must see why the job stopped and what to change.
    OverBudget {
        /// The bytes the job needs: an estimate before the job, or the
        /// resident size that the probe read during it.
        need_bytes: u64,
        /// The limit, in bytes.
        limit_bytes: u64,
    },
    Message(String),
}

impl ComputeError {
    /// The sentence a surface shows for an over-budget stop of `job`, for
    /// example `"Simulation"`.
    ///
    /// It names the two values and the two remedies: a larger budget in the
    /// settings file, or a coarser simulation cell.
    #[must_use]
    pub fn over_budget_message(job: &str, need_bytes: u64, limit_bytes: u64) -> String {
        format!(
            "{job} stopped: the job needs about {}, but the memory budget is {}. \
             Set a larger [memory] limit in {} or use a coarser simulation cell.",
            rs_cam_core::budget::format_bytes(need_bytes),
            rs_cam_core::budget::format_bytes(limit_bytes),
            settings_file_text(),
        )
    }

    /// The sentence a surface shows when the preflight refuses `job` before
    /// it allocates (memory programme wave 3, "Architecture" 4e).
    ///
    /// It names the need, the idle baseline, the limit, and the finest
    /// simulation cell that fits. The grid is NEVER coarsened in silence:
    /// the sentence names the cell, and the operator chooses.
    #[must_use]
    pub fn preflight_refusal_message(
        job: &str,
        refusal: &rs_cam_core::budget::estimate::PreflightRefusal,
    ) -> String {
        use rs_cam_core::budget::{CellFit, format_bytes};
        let fit = match refusal.fit {
            CellFit::Fits(cell_mm) => format!(
                "The finest simulation cell that fits is {} mm; this run asked for {} mm.",
                cell_text_up(cell_mm),
                cell_text_up(refusal.requested_cell_mm),
            ),
            CellFit::Nothing { need_bytes, .. } => format!(
                "No simulation cell fits: the coarsest grid still needs about {}.",
                format_bytes(need_bytes)
            ),
            // A refusal has a limit, so this arm is not reached.
            CellFit::Unlimited => String::new(),
        };
        format!(
            "{job} refused before it started: it needs about {}, the idle app holds about {}, \
             and the memory budget is {}. {fit} Set a larger [memory] limit in {} or use a \
             coarser simulation cell. The cell was not changed.",
            format_bytes(refusal.need_bytes),
            format_bytes(refusal.baseline_bytes),
            format_bytes(refusal.limit_bytes),
            settings_file_text(),
        )
    }
}

/// The settings file a budget message names.
fn settings_file_text() -> String {
    crate::io::app_settings::settings_path().map_or_else(
        || "~/.config/rs_cam/settings.toml".to_owned(),
        |path| path.display().to_string(),
    )
}

/// A cell size in mm with three decimals, rounded UP, so that the cell the
/// text names is never finer than the cell that fits.
fn cell_text_up(cell_mm: f64) -> String {
    /// Thousandths of a millimetre: the three decimals of the text.
    const PER_MM: f64 = 1000.0;
    format!("{:.3}", (cell_mm * PER_MM).ceil() / PER_MM)
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Cancelled"),
            Self::OverBudget {
                need_bytes,
                limit_bytes,
            } => f.write_str(&Self::over_budget_message("Job", *need_bytes, *limit_bytes)),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl From<rs_cam_core::budget::StopReason> for ComputeError {
    fn from(reason: rs_cam_core::budget::StopReason) -> Self {
        match reason {
            rs_cam_core::budget::StopReason::User => Self::Cancelled,
            rs_cam_core::budget::StopReason::OverBudget {
                need_bytes,
                limit_bytes,
            } => Self::OverBudget {
                need_bytes,
                limit_bytes,
            },
        }
    }
}

impl From<rs_cam_core::compute::simulate::SimulationError> for ComputeError {
    fn from(error: rs_cam_core::compute::simulate::SimulationError) -> Self {
        use rs_cam_core::compute::simulate::SimulationError;
        match error {
            SimulationError::Cancelled => Self::Cancelled,
            SimulationError::OverBudget {
                need_bytes,
                limit_bytes,
            } => Self::OverBudget {
                need_bytes,
                limit_bytes,
            },
        }
    }
}

impl std::error::Error for ComputeError {}

/// What a toolpath submit did to the toolpath lane.
///
/// **G-REGEN-RACE.** The toolpath lane's submit rule is
/// resubmit-cancels-and-requeues: a request for a toolpath that is already
/// the lane's *active* job cancels that job and queues the new request in
/// the same lock. The `ComputeError::Cancelled` that comes back from the
/// abandoned job is therefore lane bookkeeping, **not an outcome for the
/// toolpath** — a replacement is already queued and will produce the real
/// result. Nothing downstream could tell the two apart, so
/// `drain_compute_results` reported the supersede as a terminal failure
/// ("`<name>`: generation cancelled") to whichever MCP request happened to
/// be waiting. This is the fact that makes them distinguishable, and it is
/// decided **inside the lane lock** rather than inferred from a snapshot,
/// because inferring it is the same race one level up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolpathSubmitOutcome {
    /// The request was queued; no in-flight job was touched.
    Queued,
    /// The request replaced the lane's active job for the same toolpath.
    /// Exactly one `ComputeError::Cancelled` for that toolpath is expected
    /// to drain as a consequence, and it must not be read as a result.
    SupersededActive,
}

impl From<String> for ComputeError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

pub use rs_cam_core::compute::OperationError;

impl From<OperationError> for ComputeError {
    fn from(e: OperationError) -> Self {
        match e {
            OperationError::Cancelled => ComputeError::Cancelled,
            OperationError::OverBudget {
                need_bytes,
                limit_bytes,
            } => ComputeError::OverBudget {
                need_bytes,
                limit_bytes,
            },
            other => ComputeError::Message(other.to_string()),
        }
    }
}

pub enum ComputeMessage {
    /// Toolpath lane completion.
    ///
    /// **Boxed on purpose (C5).** `ComputeResult` carries a
    /// `ToolpathResult` (and through it a whole `ToolpathStats`), which is
    /// far larger than any other variant's payload. Three separate waves
    /// each paid the `clippy::large_enum_variant` tax by boxing one more
    /// rarely-`Some` finding inside `ToolpathStats` just to keep this enum
    /// balanced. Boxing the variant itself ends that tax permanently: the
    /// payload's size no longer constrains what may be added to a
    /// generation finding.
    Toolpath(Box<ComputeResult>),
    Simulation(Result<Box<SimulationResult>, ComputeError>),
    Collision(Result<CollisionResult, ComputeError>),
    /// Optimize lane completion — the project rollup's report. It
    /// carries NO session since WP14b: the request owned a clone, so
    /// there is nothing to swap back. Cancellation still produces a
    /// `Cancelled` outcome inside the report rather than an `Err`.
    Optimize(Box<OptimizeResult>),
    /// Reach lane completion (P5). Boxed for the same reason the toolpath
    /// variant is: the payload carries a whole per-vertex colour vector.
    Reach(Box<ReachResult>),
    /// `Job` lane completion (WP14a). Boxed: a `JobAnswer` carries a whole
    /// tier map on one of its arms.
    Job(Box<JobResult>),
}

pub trait ComputeBackend: Send {
    /// Submit a toolpath generation. The return value says whether this
    /// submit **superseded** the lane's active job for the same toolpath —
    /// see [`ToolpathSubmitOutcome`]. A backend with no real lane returns
    /// [`ToolpathSubmitOutcome::Queued`].
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome;
    fn submit_simulation(&mut self, request: SimulationRequest);
    fn submit_collision(&mut self, request: CollisionRequest);
    /// Submit an Optimize request. The request owns a CLONE of the
    /// `ProjectSession` and the worker drops it with the request; the
    /// [`ComputeMessage::Optimize`] reply carries the report alone.
    /// Before WP14b the main thread MOVED its session in and the reply
    /// carried it back.
    fn submit_optimize(&mut self, request: OptimizeRequest);

    /// Submit one core `Job` row's work step to the [`ComputeLane::Job`]
    /// lane.
    ///
    /// Defaulted to a no-op for the same reason [`Self::submit_reach_map`]
    /// is: a scripted test backend runs no lane, and an answer that never
    /// arrives leaves the caller's oneshot dropped rather than wrong.
    fn submit_job(&mut self, _request: JobRequest) {}

    /// Submit a reach-map walk for the viewport overlay (P5).
    ///
    /// Defaulted to a no-op for the same reason `clear_sim_prefix_cache` is:
    /// a scripted test backend runs no lane, and an overlay that never
    /// arrives leaves the plain model on screen. A backend that grows a real
    /// Reach lane overrides it.
    fn submit_reach_map(&mut self, _request: ReachRequest) {}

    fn cancel_lane(&mut self, lane: ComputeLane);
    fn drain_results(&mut self) -> Vec<ComputeMessage>;
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot;
    /// A/M12: a thread-safe handle onto the **toolpath** lane that can be
    /// observed and cancelled without going through the GUI frame loop.
    /// Deliberately not a defaulted trait method — a backend that grows a
    /// real lane must decide explicitly whether the escape hatches reach it.
    fn generation_control(&self) -> GenerationControl;

    /// S5 — drop the analysis lane's simulation prefix snapshot.
    ///
    /// Defaulted to a no-op: a backend with no real analysis lane has nothing
    /// to drop, and a memo that is never populated is never stale. The GUI
    /// calls this when the `generate_all` fixpoint ladder settles, so the
    /// snapshot's memory is released at a known point instead of waiting for
    /// the next simulation to consume it.
    fn clear_sim_prefix_cache(&mut self) {}

    /// The memory budget of the heavy lanes (plan B1).
    ///
    /// Defaulted to no limit: a scripted test backend runs no lane and has
    /// nothing to guard.
    fn memory_budget(&self) -> rs_cam_core::budget::MemoryBudget {
        rs_cam_core::budget::MemoryBudget::UNLIMITED
    }

    /// Change the memory budget of the heavy lanes at run time (File ▸
    /// Preferences ▸ Apply). A job that runs keeps its old guard; the next
    /// job uses `budget`.
    ///
    /// Defaulted to a no-op: a scripted test backend runs no lane and has
    /// no budget to change.
    fn set_memory_budget(&mut self, _budget: rs_cam_core::budget::MemoryBudget) {}

    /// The bytes that the running heavy jobs reserved in the ledger (plan
    /// B4). Defaulted to zero for a backend with no ledger.
    fn memory_reserved_bytes(&self) -> u64 {
        0
    }

    /// The simulation preflight (memory programme wave 3): `Err` when even
    /// an otherwise idle process cannot hold `need` under the budget.
    ///
    /// The controller calls it BEFORE it releases the view of the previous
    /// run and before it submits, so a refusal allocates nothing and
    /// changes nothing. Defaulted to `Ok`: a backend with no budget refuses
    /// nothing.
    ///
    /// # Errors
    /// The refusal, with the need, the limit, the baseline and the finest
    /// cell that fits.
    fn preflight_simulation(
        &self,
        _need: &rs_cam_core::budget::estimate::SimulationNeed,
    ) -> Result<(), rs_cam_core::budget::estimate::PreflightRefusal> {
        Ok(())
    }

    fn cancel_all(&mut self) {
        self.cancel_lane(ComputeLane::Toolpath);
        self.cancel_lane(ComputeLane::Analysis);
        self.cancel_lane(ComputeLane::Optimize);
        self.cancel_lane(ComputeLane::Reach);
        self.cancel_lane(ComputeLane::Job);
    }

    /// Every lane, in declaration order.
    ///
    /// The Reach lane is in here, not left out as a private overlay detail:
    /// `RsCamApp::needs_pump_tick` and the repaint arm both fold over this
    /// array, so a lane missing from it can run to completion while the
    /// event loop is parked and its result waits for an unrelated wake-up.
    fn lane_snapshots(&self) -> [LaneSnapshot; 5] {
        [
            self.lane_snapshot(ComputeLane::Toolpath),
            self.lane_snapshot(ComputeLane::Analysis),
            self.lane_snapshot(ComputeLane::Optimize),
            self.lane_snapshot(ComputeLane::Reach),
            self.lane_snapshot(ComputeLane::Job),
        ]
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    /// C5 sentry. Every variant of the compute channel enum must stay
    /// pointer-sized-ish. Before C5, `Toolpath` carried a whole
    /// `ComputeResult` inline, so `clippy::large_enum_variant` (denied
    /// workspace-wide) fired every time a new generation finding was added
    /// to `ToolpathStats` — three waves each "fixed" that by boxing one more
    /// inner `Option`. Boxing the variant ends the tax; this test is what
    /// stops it coming back.
    #[test]
    fn compute_message_stays_small() {
        let size = std::mem::size_of::<ComputeMessage>();
        assert!(
            size <= 128,
            "ComputeMessage grew to {size} bytes — a variant is carrying a \
             large payload inline again; box it rather than shrinking the \
             payload (C5)"
        );
    }
}
