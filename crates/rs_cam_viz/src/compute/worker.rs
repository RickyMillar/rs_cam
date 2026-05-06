mod execute;
pub mod helpers;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests;

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::{
    LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out, apply_link_moves,
    filter_air_cuts,
};
use rs_cam_core::feedopt::{FeedOptParams, optimize_feed_rates};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::toolpath::{MoveType, Toolpath};

use super::{ComputeBackend, ComputeError, ComputeLane, ComputeMessage, LaneSnapshot, LaneState};
use crate::state::job::ToolConfig;
#[cfg(test)]
use crate::state::job::ToolType;
use crate::state::toolpath::{
    DressupConfig, DressupEntryStyle, OperationConfig, StockSource, ToolpathId, ToolpathResult,
};

pub struct ComputeRequest {
    pub toolpath_id: ToolpathId,
    pub toolpath_name: String,
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub enriched_mesh: Option<Arc<rs_cam_core::enriched_mesh::EnrichedMesh>>,
    pub face_selection: Option<Vec<rs_cam_core::enriched_mesh::FaceGroupId>>,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub stock_source: StockSource,
    pub tool: ToolConfig,
    pub safe_z: f64,
    pub prev_tool_radius: Option<f64>,
    pub stock_bbox: Option<BoundingBox3>,
    pub boundary: crate::state::toolpath::BoundaryConfig,
    /// Fixture and keep-out footprints to subtract from the machining boundary.
    pub keep_out_footprints: Vec<Polygon2>,
    pub heights: crate::state::toolpath::ResolvedHeights,
    /// Pre-computed Z levels for depth stepping (top->bottom).
    /// Empty for operations that don't use standard depth stepping (3D ops, etc.).
    pub cutting_levels: Vec<f64>,
    /// Pre-simulated remaining stock from prior toolpaths in the same setup.
    pub prior_stock: Option<TriDexelStock>,
}

pub struct ComputeResult {
    pub toolpath_id: ToolpathId,
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct SetupSimToolpath {
    pub id: ToolpathId,
    pub name: String,
    pub toolpath: Arc<Toolpath>,
    pub tool: ToolConfig,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
}

/// A group of toolpaths from one setup in setup-local coordinates.
///
/// Toolpaths are NOT transformed to global — they stay in the setup's local
/// frame.  The simulation creates a per-group stock from `local_stock_bbox`
/// and always stamps from the top (Z-axis down).  After simulation the mesh
/// is transformed to global coordinates for compositing.
pub struct SetupSimGroup {
    /// Toolpaths in setup-local frame.
    pub toolpaths: Vec<SetupSimToolpath>,
    /// Bounding box for this setup's stock in local coordinates
    /// (origin at 0,0,0; max at effective width/depth/height).
    pub local_stock_bbox: BoundingBox3,
    /// Transform info to convert local coordinates back to global stock frame.
    /// `None` when the setup is identity (FaceUp::Top, ZRotation::None).
    pub local_to_global: Option<SetupTransformInfo>,
}

// Re-export from core — the struct and all methods now live in rs_cam_core.
pub use rs_cam_core::compute::SetupTransformInfo;

pub struct SimulationRequest {
    /// Per-setup groups, processed sequentially on one stock.
    pub groups: Vec<SetupSimGroup>,
    pub stock_bbox: BoundingBox3,
    pub stock_top_z: f64,
    pub resolution: f64,
    pub metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions,
    pub spindle_rpm: u32,
    pub rapid_feed_mm_min: f64,
    /// Optional model mesh for deviation computation (sim_z vs model_z).
    pub model_mesh: Option<Arc<TriangleMesh>>,
}

pub struct SimBoundary {
    pub id: ToolpathId,
    pub name: String,
    pub tool_name: String,
    pub start_move: usize,
    pub end_move: usize,
    /// Cut direction for this toolpath's setup.
    pub direction: StockCutDirection,
}

pub use rs_cam_core::compute::simulate::SimCheckpointMesh;

pub struct SimulationResult {
    pub mesh: StockMesh,
    pub total_moves: usize,
    pub deviations: Option<Vec<f32>>,
    pub boundaries: Vec<SimBoundary>,
    pub checkpoints: Vec<SimCheckpointMesh>,
    /// Pre-transformed toolpath data for incremental playback.
    /// Each entry: (toolpath, tool_config, direction).
    pub playback_data: Vec<(Arc<Toolpath>, ToolConfig, StockCutDirection)>,
    /// Rapid-through-stock collisions detected during simulation.
    pub rapid_collisions: Vec<RapidCollision>,
    /// Move indices with rapid collisions (for timeline markers).
    pub rapid_collision_move_indices: Vec<usize>,
    pub cut_trace: Option<Arc<rs_cam_core::simulation_cut::SimulationCutTrace>>,
    pub cut_trace_path: Option<PathBuf>,
    /// True when the requested resolution was coarsened to fit within grid limits.
    pub resolution_clamped: bool,
}

pub struct CollisionRequest {
    pub toolpath: Arc<Toolpath>,
    pub tool: ToolConfig,
    pub mesh: Arc<TriangleMesh>,
}

pub struct CollisionResult {
    pub report: CollisionReport,
    pub positions: Vec<[f32; 3]>,
}

/// Request to the Optimize worker lane. Both variants own a
/// `ProjectSession` for the duration of the run — the main thread
/// `mem::replace`s its session into the request; the worker mutates
/// freely; the session comes back on [`OptimizeResult::session`].
#[allow(clippy::large_enum_variant)]
pub enum OptimizeRequest {
    /// Run `optimize_toolpath` on a single toolpath. Surfaces in the
    /// per-toolpath modal (U2's worker-thread retrofit).
    Toolpath {
        session: rs_cam_core::session::ProjectSession,
        baseline_trace: Arc<rs_cam_core::simulation_cut::SimulationCutTrace>,
        toolpath_index: usize,
        /// Stable id from the toolpath config — pass-through so the
        /// main thread can match the result to the open modal even if
        /// indices have shifted.
        toolpath_id: usize,
    },
    /// Run `optimize_project` over every enabled toolpath. Surfaces in
    /// the U3 rollup view.
    Project {
        session: rs_cam_core::session::ProjectSession,
        baseline_trace: Arc<rs_cam_core::simulation_cut::SimulationCutTrace>,
    },
}

/// Result from the Optimize worker. Always carries the session back
/// for the main thread to swap into `AppState::session`. Cancellation
/// surfaces inside `OptimizeResultKind` (the inner outcome carries a
/// "cancelled" narrative), not as an `Err`, so the session never gets
/// dropped on the floor.
pub struct OptimizeResult {
    pub session: rs_cam_core::session::ProjectSession,
    pub kind: OptimizeResultKind,
}

pub enum OptimizeResultKind {
    Toolpath {
        toolpath_id: usize,
        outcome: rs_cam_core::tool_load::optimize::OptimizeOutcome,
    },
    Project {
        report: rs_cam_core::tool_load::optimize::ProjectOptimizeReport,
    },
}

#[allow(clippy::large_enum_variant)]
enum AnalysisRequest {
    Simulation(SimulationRequest),
    Collision(CollisionRequest),
}

struct LaneInner<Request> {
    queue: VecDeque<Request>,
    state: LaneState,
    current_job: Option<String>,
    current_phase: Option<String>,
    started_at: Option<Instant>,
    active_toolpath_id: Option<ToolpathId>,
}

impl<Request> LaneInner<Request> {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            state: LaneState::Idle,
            current_job: None,
            current_phase: None,
            started_at: None,
            active_toolpath_id: None,
        }
    }
}

struct LaneQueue<Request> {
    lane: ComputeLane,
    inner: Mutex<LaneInner<Request>>,
    wake: Condvar,
    cancel: AtomicBool,
    shutdown: AtomicBool,
}

impl<Request> LaneQueue<Request> {
    fn new(lane: ComputeLane) -> Arc<Self> {
        Arc::new(Self {
            lane,
            inner: Mutex::new(LaneInner::new()),
            wake: Condvar::new(),
            cancel: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        })
    }

    fn snapshot(&self) -> LaneSnapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        LaneSnapshot {
            lane: self.lane,
            state: inner.state,
            queue_depth: inner.queue.len(),
            current_job: inner.current_job.clone(),
            current_phase: inner.current_phase.clone(),
            started_at: inner.started_at,
        }
    }
}

#[derive(Clone)]
struct ToolpathPhaseTracker {
    lane: Arc<LaneQueue<ComputeRequest>>,
}

struct ToolpathPhaseScope {
    tracker: ToolpathPhaseTracker,
    previous_phase: Option<String>,
    finished: bool,
}

impl ToolpathPhaseTracker {
    fn new(lane: Arc<LaneQueue<ComputeRequest>>) -> Self {
        Self { lane }
    }

    fn start_phase(&self, phase: impl Into<String>) -> ToolpathPhaseScope {
        let previous_phase = self.replace_phase(Some(phase.into()));
        ToolpathPhaseScope {
            tracker: self.clone(),
            previous_phase,
            finished: false,
        }
    }

    fn clear(&self) {
        self.replace_phase(None);
    }

    fn replace_phase(&self, phase: Option<String>) -> Option<String> {
        let mut inner = self.lane.inner.lock().unwrap_or_else(|e| e.into_inner());
        let previous = inner.current_phase.clone();
        inner.current_phase = phase;
        previous
    }
}

impl rs_cam_core::debug_trace::ToolpathPhaseSink for ToolpathPhaseTracker {
    fn set_phase(&self, phase: Option<String>) {
        self.replace_phase(phase);
    }
}

impl ToolpathPhaseScope {
    fn finish_inner(&mut self) {
        if self.finished {
            return;
        }
        self.tracker.replace_phase(self.previous_phase.clone());
        self.finished = true;
    }
}

impl Drop for ToolpathPhaseScope {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

pub struct ThreadedComputeBackend {
    toolpath_lane: Arc<LaneQueue<ComputeRequest>>,
    analysis_lane: Arc<LaneQueue<AnalysisRequest>>,
    optimize_lane: Arc<LaneQueue<OptimizeRequest>>,
    result_rx: mpsc::Receiver<ComputeMessage>,
    toolpath_handle: Option<std::thread::JoinHandle<()>>,
    analysis_handle: Option<std::thread::JoinHandle<()>>,
    optimize_handle: Option<std::thread::JoinHandle<()>>,
}

impl ThreadedComputeBackend {
    pub fn new() -> Self {
        let toolpath_lane = LaneQueue::new(ComputeLane::Toolpath);
        let analysis_lane = LaneQueue::new(ComputeLane::Analysis);
        let optimize_lane = LaneQueue::new(ComputeLane::Optimize);
        let (result_tx, result_rx) = mpsc::sync_channel::<ComputeMessage>(64);

        let toolpath_handle = spawn_toolpath_lane(Arc::clone(&toolpath_lane), result_tx.clone());
        let analysis_handle = spawn_analysis_lane(Arc::clone(&analysis_lane), result_tx.clone());
        let optimize_handle = spawn_optimize_lane(Arc::clone(&optimize_lane), result_tx);

        Self {
            toolpath_lane,
            analysis_lane,
            optimize_lane,
            result_rx,
            toolpath_handle: Some(toolpath_handle),
            analysis_handle: Some(analysis_handle),
            optimize_handle: Some(optimize_handle),
        }
    }
}

impl Drop for ThreadedComputeBackend {
    fn drop(&mut self) {
        self.toolpath_lane.shutdown.store(true, Ordering::SeqCst);
        self.analysis_lane.shutdown.store(true, Ordering::SeqCst);
        self.optimize_lane.shutdown.store(true, Ordering::SeqCst);
        self.toolpath_lane.wake.notify_all();
        self.analysis_lane.wake.notify_all();
        self.optimize_lane.wake.notify_all();
        if let Some(h) = self.toolpath_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.analysis_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.optimize_handle.take() {
            let _ = h.join();
        }
    }
}

impl Default for ThreadedComputeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputeBackend for ThreadedComputeBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) {
        let mut inner = self
            .toolpath_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner
            .queue
            .retain(|queued| queued.toolpath_id != request.toolpath_id);
        if inner.active_toolpath_id == Some(request.toolpath_id) {
            self.toolpath_lane.cancel.store(true, Ordering::SeqCst);
            if inner.state == LaneState::Running {
                inner.state = LaneState::Cancelling;
            }
        }
        inner.queue.push_back(request);
        if inner.started_at.is_none() {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(toolpath_job_label);
            inner.current_phase = None;
        }
        self.toolpath_lane.wake.notify_one();
    }

    fn submit_simulation(&mut self, request: SimulationRequest) {
        self.submit_analysis(AnalysisRequest::Simulation(request));
    }

    fn submit_collision(&mut self, request: CollisionRequest) {
        self.submit_analysis(AnalysisRequest::Collision(request));
    }

    fn submit_optimize(&mut self, request: OptimizeRequest) {
        let mut inner = self
            .optimize_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // The Optimize lane only ever runs one job at a time — a new
        // submit replaces any queued job (the modal closes the previous
        // run before opening a new one) and cancels any in-flight job.
        inner.queue.clear();
        inner.queue.push_back(request);
        if inner.started_at.is_some() {
            self.optimize_lane.cancel.store(true, Ordering::SeqCst);
            inner.state = LaneState::Cancelling;
        } else {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(optimize_job_label);
            inner.current_phase = None;
        }
        self.optimize_lane.wake.notify_one();
    }

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => {
                let mut inner = self
                    .toolpath_lane
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if inner.started_at.is_some() {
                    self.toolpath_lane.cancel.store(true, Ordering::SeqCst);
                    inner.state = LaneState::Cancelling;
                }
            }
            ComputeLane::Analysis => {
                let mut inner = self
                    .analysis_lane
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if inner.started_at.is_some() {
                    self.analysis_lane.cancel.store(true, Ordering::SeqCst);
                    inner.state = LaneState::Cancelling;
                }
            }
            ComputeLane::Optimize => {
                let mut inner = self
                    .optimize_lane
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if inner.started_at.is_some() {
                    self.optimize_lane.cancel.store(true, Ordering::SeqCst);
                    inner.state = LaneState::Cancelling;
                }
            }
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            results.push(result);
        }
        results
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.snapshot(),
            ComputeLane::Analysis => self.analysis_lane.snapshot(),
            ComputeLane::Optimize => self.optimize_lane.snapshot(),
        }
    }
}

impl ThreadedComputeBackend {
    fn submit_analysis(&mut self, request: AnalysisRequest) {
        let mut inner = self
            .analysis_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner.queue.clear();
        inner.queue.push_back(request);
        if inner.started_at.is_some() {
            self.analysis_lane.cancel.store(true, Ordering::SeqCst);
            inner.state = LaneState::Cancelling;
        } else {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(analysis_job_label);
            inner.current_phase = None;
        }
        self.analysis_lane.wake.notify_one();
    }
}

fn toolpath_job_label(request: &ComputeRequest) -> String {
    format!("{} ({})", request.toolpath_name, request.operation.label())
}

fn analysis_job_label(request: &AnalysisRequest) -> String {
    match request {
        AnalysisRequest::Simulation(request) => {
            let count: usize = request.groups.iter().map(|g| g.toolpaths.len()).sum();
            format!("Simulation ({count} toolpaths)")
        }
        AnalysisRequest::Collision(_) => "Collision check".to_owned(),
    }
}

fn optimize_job_label(request: &OptimizeRequest) -> String {
    match request {
        OptimizeRequest::Toolpath { toolpath_index, .. } => {
            format!("Optimize toolpath #{toolpath_index}")
        }
        OptimizeRequest::Project { .. } => "Optimize project".to_owned(),
    }
}

/// Bridge that lets the optimizer's `ProgressReporter` updates land
/// in the lane's `current_phase` field. The modal reads the phase
/// through `lane_snapshot()` once per frame.
struct LaneProgressBridge {
    lane: Arc<LaneQueue<OptimizeRequest>>,
}

impl rs_cam_core::tool_load::optimize::ProgressReporter for LaneProgressBridge {
    fn report(&self, _completed: usize, _total: usize, label: &str) {
        let mut inner = self.lane.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.current_phase = Some(label.to_owned());
    }
}

fn spawn_toolpath_lane(
    lane: Arc<LaneQueue<ComputeRequest>>,
    result_tx: mpsc::SyncSender<ComputeMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let request = {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                while inner.queue.is_empty() {
                    if lane.shutdown.load(Ordering::SeqCst) {
                        return;
                    }
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                    inner.current_phase = None;
                    inner.started_at = None;
                    inner.active_toolpath_id = None;
                    inner = lane.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                if lane.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                // SAFETY: loop condition guarantees queue is non-empty
                #[allow(clippy::expect_used)]
                let request = inner.queue.pop_front().expect("queue checked");
                lane.cancel.store(false, Ordering::SeqCst);
                inner.state = LaneState::Running;
                inner.current_job = Some(toolpath_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                inner.active_toolpath_id = Some(request.toolpath_id);
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // Wrap the compute + result-send body in catch_unwind so a panic
            // in any operation does not kill the worker thread or poison mutexes
            // permanently.  On panic we log the error, reset the lane to Idle,
            // send an error result back, and continue the loop.
            let toolpath_id = request.toolpath_id;
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let phase_tracker = ToolpathPhaseTracker::new(Arc::clone(&lane));
                let mut outcome =
                    execute::run_compute_with_phase(&request, &lane.cancel, &phase_tracker);
                if lane.cancel.load(Ordering::SeqCst) && outcome.result.is_ok() {
                    outcome.result = Err(ComputeError::Cancelled);
                }
                phase_tracker.clear();

                {
                    let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.active_toolpath_id = None;
                    inner.started_at = None;
                    inner.current_phase = None;
                    if inner.queue.is_empty() {
                        inner.state = LaneState::Idle;
                        inner.current_job = None;
                    } else {
                        inner.state = LaneState::Queued;
                        inner.current_job = inner.queue.front().map(toolpath_job_label);
                    }
                }

                let _ = result_tx.send(ComputeMessage::Toolpath(ComputeResult {
                    toolpath_id: request.toolpath_id,
                    result: outcome.result,
                    debug_trace: outcome.debug_trace,
                    semantic_trace: outcome.semantic_trace,
                    debug_trace_path: outcome.debug_trace_path,
                }));
            }));

            if let Err(panic_payload) = caught {
                let msg = panic_message(&panic_payload);
                tracing::error!("rs_cam crashed due to internal error (toolpath worker): {msg}");

                // Reset lane state so subsequent jobs can still run.
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.active_toolpath_id = None;
                inner.started_at = None;
                inner.current_phase = None;
                if inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                } else {
                    inner.state = LaneState::Queued;
                    inner.current_job = inner.queue.front().map(toolpath_job_label);
                }
                drop(inner);

                let _ = result_tx.send(ComputeMessage::Toolpath(ComputeResult {
                    toolpath_id,
                    result: Err(ComputeError::Message(format!(
                        "Crashed due to internal error: {msg}"
                    ))),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                }));
            }
        }
    })
}

fn spawn_analysis_lane(
    lane: Arc<LaneQueue<AnalysisRequest>>,
    result_tx: mpsc::SyncSender<ComputeMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let request = {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                while inner.queue.is_empty() {
                    if lane.shutdown.load(Ordering::SeqCst) {
                        return;
                    }
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                    inner.current_phase = None;
                    inner.started_at = None;
                    inner.active_toolpath_id = None;
                    inner = lane.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                if lane.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                // SAFETY: loop condition guarantees queue is non-empty
                #[allow(clippy::expect_used)]
                let request = inner.queue.pop_front().expect("queue checked");
                lane.cancel.store(false, Ordering::SeqCst);
                inner.state = LaneState::Running;
                inner.current_job = Some(analysis_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // Wrap the analysis body in catch_unwind so a panic in simulation
            // or collision checking does not kill the worker thread.  On panic
            // we log the error, reset lane state to Idle, and continue.
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let result = match request {
                    AnalysisRequest::Simulation(request) => {
                        let set_phase = |phase: &str| {
                            let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                            inner.current_phase = Some(phase.to_owned());
                        };
                        let result =
                            execute::run_simulation_with_phase(&request, &lane.cancel, set_phase);
                        let result = if lane.cancel.load(Ordering::SeqCst) && result.is_ok() {
                            Err(ComputeError::Cancelled)
                        } else {
                            result
                        };
                        ComputeMessage::Simulation(result)
                    }
                    AnalysisRequest::Collision(request) => {
                        let set_phase = |phase: &str| {
                            let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                            inner.current_phase = Some(phase.to_owned());
                        };
                        let result = helpers::run_collision_check_with_phase(
                            &request,
                            &lane.cancel,
                            set_phase,
                        );
                        let result = if lane.cancel.load(Ordering::SeqCst) && result.is_ok() {
                            Err(ComputeError::Cancelled)
                        } else {
                            result
                        };
                        ComputeMessage::Collision(result)
                    }
                };

                {
                    let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.started_at = None;
                    inner.current_phase = None;
                    if inner.queue.is_empty() {
                        inner.state = LaneState::Idle;
                        inner.current_job = None;
                    } else {
                        inner.state = LaneState::Queued;
                        inner.current_job = inner.queue.front().map(analysis_job_label);
                    }
                }

                let _ = result_tx.send(result);
            }));

            if let Err(panic_payload) = caught {
                let msg = panic_message(&panic_payload);
                tracing::error!("rs_cam crashed due to internal error (analysis worker): {msg}");

                // Reset lane state so subsequent jobs can still run.
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.started_at = None;
                inner.current_phase = None;
                if inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                } else {
                    inner.state = LaneState::Queued;
                    inner.current_job = inner.queue.front().map(analysis_job_label);
                }
            }
        }
    })
}

fn spawn_optimize_lane(
    lane: Arc<LaneQueue<OptimizeRequest>>,
    result_tx: mpsc::SyncSender<ComputeMessage>,
) -> std::thread::JoinHandle<()> {
    use rs_cam_core::tool_load::RefuseReason;
    use rs_cam_core::tool_load::optimize::{
        OptimizeOutcome, ProjectOptimizeReport, optimize_project, optimize_toolpath,
    };

    std::thread::spawn(move || {
        loop {
            let request = {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                while inner.queue.is_empty() {
                    if lane.shutdown.load(Ordering::SeqCst) {
                        return;
                    }
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                    inner.current_phase = None;
                    inner.started_at = None;
                    inner.active_toolpath_id = None;
                    inner = lane.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                if lane.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                // SAFETY: loop condition guarantees queue is non-empty
                #[allow(clippy::expect_used)]
                let request = inner.queue.pop_front().expect("queue checked");
                lane.cancel.store(false, Ordering::SeqCst);
                inner.state = LaneState::Running;
                inner.current_job = Some(optimize_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // The optimizer carries the session through the request
            // and produces an OptimizeResult that returns it. We do
            // NOT wrap this in catch_unwind: a panic mid-optimize
            // would lose the session, which is worse than killing
            // the worker thread (the user can restart). Optimizer is
            // covered by 70+ unit tests; panic risk is low.
            let progress = LaneProgressBridge {
                lane: Arc::clone(&lane),
            };
            let result = match request {
                OptimizeRequest::Toolpath {
                    mut session,
                    baseline_trace,
                    toolpath_index,
                    toolpath_id,
                } => {
                    let outcome = optimize_toolpath(
                        &mut session,
                        &baseline_trace,
                        toolpath_index,
                        &lane.cancel,
                    );
                    // If the cancel flag was set, surface the
                    // partial outcome with a "cancelled" narrative —
                    // optimize_toolpath itself produces this when it
                    // observes the cancel between candidates.
                    let outcome = if lane.cancel.load(Ordering::SeqCst) {
                        match outcome {
                            OptimizeOutcome::Ranked(_)
                            | OptimizeOutcome::NoSafeImprovement { .. } => outcome,
                            OptimizeOutcome::Skipped { .. } => OptimizeOutcome::NoSafeImprovement {
                                reason: RefuseReason::NoImprovementFound,
                                explanation: "cancelled before optimization could run".to_owned(),
                                attempted: Vec::new(),
                            },
                        }
                    } else {
                        outcome
                    };
                    OptimizeResult {
                        session,
                        kind: OptimizeResultKind::Toolpath {
                            toolpath_id,
                            outcome,
                        },
                    }
                }
                OptimizeRequest::Project {
                    mut session,
                    baseline_trace,
                } => {
                    let report: ProjectOptimizeReport =
                        optimize_project(&mut session, &baseline_trace, &progress, &lane.cancel);
                    OptimizeResult {
                        session,
                        kind: OptimizeResultKind::Project { report },
                    }
                }
            };

            // Reset lane state before sending so a follow-up submit
            // doesn't see Running.
            {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.started_at = None;
                inner.current_phase = None;
                if inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                } else {
                    inner.state = LaneState::Queued;
                    inner.current_job = inner.queue.front().map(optimize_job_label);
                }
            }

            let _ = result_tx.send(ComputeMessage::Optimize(result));
        }
    })
}

/// Extract a human-readable message from a panic payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_owned()
    }
}
