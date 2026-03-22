mod execute;
pub mod helpers;
mod semantic;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use rs_cam_core::adaptive::AdaptiveParams;
use rs_cam_core::adaptive3d::{Adaptive3dParams, EntryStyle3d};
use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::chamfer::{ChamferParams, chamfer_toolpath};
use rs_cam_core::collision::{
    CollisionReport, RapidCollision, ToolAssembly, check_collisions_interpolated_with_cancel,
};
use rs_cam_core::depth::{DepthDistribution, DepthStepping, depth_stepped_toolpath};
use rs_cam_core::dexel_mesh::dexel_stock_to_mesh;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::{
    EntryStyle as CoreEntryStyle, LinkMoveParams, apply_dogbones, apply_entry, apply_lead_in_out,
    apply_link_moves, apply_tabs, even_tabs,
};
use rs_cam_core::drill::{DrillCycle, DrillParams, drill_toolpath};
use rs_cam_core::dropcutter::batch_drop_cutter_with_cancel;
use rs_cam_core::feedopt::{FeedOptParams, optimize_feed_rates};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::horizontal_finish::{HorizontalFinishParams, horizontal_finish_toolpath};
use rs_cam_core::inlay::{InlayParams, inlay_toolpaths};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::pencil::PencilParams;
use rs_cam_core::pocket::{PocketParams, pocket_toolpath};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::{ProfileParams, profile_toolpath};
use rs_cam_core::project_curve::{ProjectCurveParams, project_curve_toolpath};
use rs_cam_core::radial_finish::{RadialFinishParams, radial_finish_toolpath};
use rs_cam_core::ramp_finish::{CutDirection as CoreCutDir, RampFinishParams};
use rs_cam_core::rest::{RestParams, rest_machining_toolpath};
use rs_cam_core::scallop::{ScallopDirection as CoreScalDir, ScallopParams};
use rs_cam_core::spiral_finish::{SpiralDirection as CoreSpiralDir, SpiralFinishParams};
use rs_cam_core::steep_shallow::{SteepShallowParams, steep_shallow_toolpath};
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
};
use rs_cam_core::toolpath::{MoveType, Toolpath, raster_toolpath_from_grid};
use rs_cam_core::trace::{TraceCompensation as CoreTraceComp, TraceParams, trace_toolpath};
use rs_cam_core::vcarve::{VCarveParams, vcarve_toolpath};
use rs_cam_core::waterline::{WaterlineParams, waterline_toolpath_with_cancel};
use rs_cam_core::zigzag::{ZigzagParams, zigzag_toolpath};

use super::{ComputeBackend, ComputeError, ComputeLane, ComputeMessage, LaneSnapshot, LaneState};
use crate::state::job::{ToolConfig, ToolType};
use crate::state::toolpath::*;

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
    pub boundary_enabled: bool,
    pub boundary_containment: crate::state::toolpath::BoundaryContainment,
    /// Fixture and keep-out footprints to subtract from the machining boundary.
    pub keep_out_footprints: Vec<Polygon2>,
    pub heights: crate::state::toolpath::ResolvedHeights,
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

/// A group of toolpaths from one setup, pre-transformed to the global stock frame.
pub struct SetupSimGroup {
    pub toolpaths: Vec<SetupSimToolpath>,
    /// Cut direction derived from the setup's FaceUp orientation.
    pub direction: StockCutDirection,
}

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

pub struct SimCheckpointMesh {
    pub boundary_index: usize,
    pub mesh: StockMesh,
    pub stock: TriDexelStock,
}

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
    cancel: Arc<AtomicBool>,
}

impl<Request> LaneQueue<Request> {
    fn new(lane: ComputeLane) -> Arc<Self> {
        Arc::new(Self {
            lane,
            inner: Mutex::new(LaneInner::new()),
            wake: Condvar::new(),
            cancel: Arc::new(AtomicBool::new(false)),
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
    result_rx: mpsc::Receiver<ComputeMessage>,
}

impl ThreadedComputeBackend {
    pub fn new() -> Self {
        let toolpath_lane = LaneQueue::new(ComputeLane::Toolpath);
        let analysis_lane = LaneQueue::new(ComputeLane::Analysis);
        let (result_tx, result_rx) = mpsc::channel::<ComputeMessage>();

        spawn_toolpath_lane(Arc::clone(&toolpath_lane), result_tx.clone());
        spawn_analysis_lane(Arc::clone(&analysis_lane), result_tx);

        Self {
            toolpath_lane,
            analysis_lane,
            result_rx,
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
        AnalysisRequest::Collision(_) => "Collision check".to_string(),
    }
}

fn spawn_toolpath_lane(
    lane: Arc<LaneQueue<ComputeRequest>>,
    result_tx: mpsc::Sender<ComputeMessage>,
) {
    std::thread::spawn(move || {
        loop {
            let request = {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                while inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                    inner.current_phase = None;
                    inner.started_at = None;
                    inner.active_toolpath_id = None;
                    inner = lane.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                let request = inner.queue.pop_front().expect("queue checked");
                lane.cancel.store(false, Ordering::SeqCst);
                inner.state = LaneState::Running;
                inner.current_job = Some(toolpath_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                inner.active_toolpath_id = Some(request.toolpath_id);
                request
            };

            // Wrap the compute + result-send body in catch_unwind so a panic
            // in any operation does not kill the worker thread or poison mutexes
            // permanently.  On panic we log the error, reset the lane to Idle,
            // send an error result back, and continue the loop.
            let toolpath_id = request.toolpath_id;
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let phase_tracker = ToolpathPhaseTracker::new(Arc::clone(&lane));
                let mut outcome =
                    execute::run_compute_with_phase(&request, lane.cancel.as_ref(), &phase_tracker);
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
                eprintln!("[toolpath worker] panic recovered: {msg}");

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
                        "Internal error (panic): {msg}"
                    ))),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                }));
            }
        }
    });
}

fn spawn_analysis_lane(
    lane: Arc<LaneQueue<AnalysisRequest>>,
    result_tx: mpsc::Sender<ComputeMessage>,
) {
    std::thread::spawn(move || {
        loop {
            let request = {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                while inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                    inner.current_phase = None;
                    inner.started_at = None;
                    inner.active_toolpath_id = None;
                    inner = lane.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                let request = inner.queue.pop_front().expect("queue checked");
                lane.cancel.store(false, Ordering::SeqCst);
                inner.state = LaneState::Running;
                inner.current_job = Some(analysis_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                request
            };

            // Wrap the analysis body in catch_unwind so a panic in simulation
            // or collision checking does not kill the worker thread.  On panic
            // we log the error, reset lane state to Idle, and continue.
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let result = match request {
                    AnalysisRequest::Simulation(request) => {
                        let set_phase = |phase: &str| {
                            let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                            inner.current_phase = Some(phase.to_string());
                        };
                        let result = execute::run_simulation_with_phase(
                            &request,
                            lane.cancel.as_ref(),
                            set_phase,
                        );
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
                            inner.current_phase = Some(phase.to_string());
                        };
                        let result = helpers::run_collision_check_with_phase(
                            &request,
                            lane.cancel.as_ref(),
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
                eprintln!("[analysis worker] panic recovered: {msg}");

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
    });
}

/// Extract a human-readable message from a panic payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
