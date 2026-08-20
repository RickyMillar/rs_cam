#![deny(clippy::indexing_slicing)]

pub mod worker;

use std::sync::Arc;
use std::time::{Duration, Instant};

pub use worker::{
    CollisionRequest, CollisionResult, ComputeRequest, ComputeResult, OptimizeRequest,
    OptimizeResult, OptimizeResultKind, SetupSimGroup, SetupSimToolpath, SetupTransformInfo,
    SimulationRequest, SimulationResult, ThreadedComputeBackend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeLane {
    Toolpath,
    Analysis,
    /// Worker for the per-toolpath / project Optimize search. Each
    /// request takes ownership of the `ProjectSession` for the run; the
    /// session is returned attached to the result so the main thread
    /// can swap it back.
    Optimize,
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
    Cancelled,
    Message(String),
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Message(message) => f.write_str(message),
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
    /// Optimize lane completion. Always carries a session for the main
    /// thread to swap back; cancellation produces a `Cancelled` outcome
    /// inside `OptimizeResultKind` rather than an `Err`, so we never
    /// drop the session on the floor.
    Optimize(Box<OptimizeResult>),
}

pub trait ComputeBackend: Send {
    /// Submit a toolpath generation. The return value says whether this
    /// submit **superseded** the lane's active job for the same toolpath —
    /// see [`ToolpathSubmitOutcome`]. A backend with no real lane returns
    /// [`ToolpathSubmitOutcome::Queued`].
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome;
    fn submit_simulation(&mut self, request: SimulationRequest);
    fn submit_collision(&mut self, request: CollisionRequest);
    /// Submit an Optimize request. The request takes ownership of the
    /// `ProjectSession`; the worker returns it on the [`ComputeMessage::Optimize`]
    /// reply so the main thread can put it back on `AppState::session`.
    fn submit_optimize(&mut self, request: OptimizeRequest);
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

    fn cancel_all(&mut self) {
        self.cancel_lane(ComputeLane::Toolpath);
        self.cancel_lane(ComputeLane::Analysis);
        self.cancel_lane(ComputeLane::Optimize);
    }

    fn lane_snapshots(&self) -> [LaneSnapshot; 3] {
        [
            self.lane_snapshot(ComputeLane::Toolpath),
            self.lane_snapshot(ComputeLane::Analysis),
            self.lane_snapshot(ComputeLane::Optimize),
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
