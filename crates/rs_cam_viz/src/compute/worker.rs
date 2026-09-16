pub(crate) mod execute;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod gen_parity_p0_tests;
pub mod helpers;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod test_fixture;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

use super::{
    CancelOutcome, ComputeBackend, ComputeError, ComputeLane, ComputeMessage, GenerationControl,
    JobRequestId, LaneControl, LaneSnapshot, LaneState, ToolpathSubmitOutcome,
};
use crate::state::job::ToolConfig;
use crate::state::toolpath::{ToolpathId, ToolpathResult};

/// One generation job, on its way to the toolpath lane.
///
/// WP11b: the request is the core `Job` handle plus the few facts that are
/// the GUI's own. Before it, this struct carried 27 public fields and the
/// controller filled every one of them — a second assembly of the inputs
/// `ProjectSession::resolve_generation_inputs` already produces (tracker row
/// N12). The controller now calls `ProjectSession::start` on the frame loop
/// and ships what that hands back.
///
/// The handle owns everything it needs, so it crosses the thread boundary
/// with no session borrow.
pub struct ComputeRequest {
    /// What `ProjectSession::start` captured. The worker runs it through
    /// `rs_cam_core::session::execute_job`.
    pub handle: rs_cam_core::session::GenerateToolpathHandle,
    /// The facts the lane and the drain need that the handle does not carry.
    pub viz: VizExtras,
}

/// The GUI's own bookkeeping for one submit.
///
/// Everything here is a viz concern: the lane keys its queue and its
/// snapshot by the toolpath id, and the cancel flag belongs to THIS submit.
pub struct VizExtras {
    /// The toolpath this job generates. The lane dedupes its queue on it,
    /// reports it on [`LaneSnapshot`], and the drain routes the reply by it.
    ///
    /// The handle carries the toolpath's INDEX, which is a different
    /// identity: an index moves when a toolpath is reordered or removed.
    pub toolpath_id: ToolpathId,
    /// The cancel flag of this submit, per §22 ruling 3.
    ///
    /// NOT the lane's flag. `submit_toolpath` sets the lane flag when a
    /// resubmit supersedes the active job, and only the worker thread
    /// clears it, so a flag borrowed from the lane can already read `true`
    /// on the frame loop — `ProjectSession::start` polls its flag and would
    /// refuse the new job for the old job's cancel. The lane maps "cancel
    /// this job" onto the flag of the job it is running.
    pub cancel: Arc<AtomicBool>,
}

pub struct ComputeResult {
    pub toolpath_id: ToolpathId,
    /// The generation-input revision `ProjectSession::start` read, carried
    /// back so the adopt names it.
    ///
    /// `None` means NOT STAMPED — a hand-built reply in a test. The drain
    /// then adopts at the toolpath's CURRENT revision, which is what it did
    /// for an unstamped completion before WP11b. Every real submit carries
    /// `Some`.
    pub revision: Option<u64>,
    pub result: Result<ToolpathResult, ComputeError>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct SetupSimToolpath {
    pub id: ToolpathId,
    pub name: String,
    pub annotated: Arc<AnnotatedToolpath>,
    pub tool: ToolConfig,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    /// Per-toolpath spindle RPM override. `None` means use the simulation
    /// request's project/post default RPM.
    pub spindle_rpm: Option<u32>,
    /// P4: true for drill / pin-drill kinds — see `SimToolpathEntry`.
    #[allow(dead_code)] // populated by caller; consumed by execute pipeline
    pub metrics_not_applicable: bool,
    /// §6.E first-class drill-op view (dual-representation invariant).
    /// When `Some`, the worker forwards this onto `SimToolpathEntry.drill_op`
    /// so the simulator uses analytical removal instead of per-segment
    /// stamping. Populated by [`crate::controller::events::simulation`]
    /// from the session's cached `ToolpathComputeResult`.
    pub drill_op: Option<Arc<rs_cam_core::drill_op::DrillOp>>,
    /// Hash of the toolpath's `OperationConfig` at sim-build time.
    /// Forwarded to `SimToolpathEntry.operation_config_hash` so the
    /// provenance builder can stamp it without holding the config.
    /// Lets [`rs_cam_core::gcode::sim_trace_is_fresh`] invalidate
    /// cached load verdicts on config-only edits like `feed_rate`.
    pub operation_config_hash: u64,
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
    /// F.4 — phantom `prior_stocks` snapshot for a not-yet-generated
    /// toolpath. Forwarded verbatim onto the core
    /// `rs_cam_core::compute::simulate::SimGroupEntry::phantom_prior_stock`
    /// of the same name — see that field's doc comment for the full
    /// validity rule and `PhantomPriorStockScan` for how the controller
    /// computes it.
    pub phantom_prior_stock: Option<(usize, ToolpathId)>,
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
    /// F-035 plumbing — the active `MachineProfile.kinematics` (or
    /// `None` if the profile carries no kinematics block). The viz
    /// controller copies this from
    /// `self.state.session.machine().kinematics` at submit time so
    /// the worker can forward it into the core
    /// `SimulationRequest::kinematics`. When `None`, the core path
    /// keeps `kinematics: None` and runtime + gates stay byte-
    /// identical to pre-F-034 / pre-F-035.
    pub kinematics: Option<rs_cam_core::machine_kinematics::MachineKinematics>,
    /// F-035 — when `true` AND `kinematics` is `Some`, the simulator
    /// stamps a per-move predicted-feed map on the trace and the
    /// chipload + power gates consume it. Default `false` keeps the
    /// GUI's gate verdicts byte-identical to pre-F-035.
    pub use_predicted_feed_in_gates: bool,
    /// F-034/F-035 — the machine's `max_feed_mm_min` cap, forwarded
    /// alongside `kinematics` so the integrator can clamp commanded
    /// feeds inside the same envelope the controller would. Defaults
    /// to the same value as `rapid_feed_mm_min` when not specified.
    pub max_feed_mm_min: f64,
    /// S5 — leave a prefix snapshot behind for the next simulation to resume
    /// from (`rs_cam_core::compute::sim_prefix`).
    ///
    /// Set only by the `generate_all` fixpoint ladder, which is the one caller
    /// that runs several simulations over a growing project in quick
    /// succession. Every other simulation still *consumes* a held snapshot if
    /// it matches — that is free — but leaves none, so the memo's memory is
    /// released at the next run rather than held for the life of the session.
    pub memoize_prefix: bool,
}

pub use rs_cam_core::compute::simulate::{SimBoundary, SimCheckpointMesh};

/// One entry in the live-sim playback stream: a pre-transformed toolpath, the
/// tool config, the cut direction to stamp it with, and (for drill operations)
/// the analytical `DrillOp` in the same frame.
///
/// `drill_op = Some(...)` instructs `update_live_sim` to use
/// `TriDexelStock::apply_drill_op` rather than `simulate_toolpath_range` —
/// matching what the compute path applies to each checkpoint stock.
///
/// **Frames.** Read [`Self::frame`] before reading anything else: it says
/// which frame `toolpath`, `drill_op` and `stock_bbox` are in, and therefore
/// which stock this entry may be stamped into. Entries of the two kinds must
/// never be stamped into the same stock.
pub struct PlaybackToolpath {
    /// Moves, expressed in [`Self::frame`].
    pub toolpath: Arc<Toolpath>,
    pub tool: ToolConfig,
    /// Direction to stamp with, in [`Self::frame`]. Always `FromTop` for a
    /// setup-local entry, because setup-local Z is always the tool axis.
    pub direction: StockCutDirection,
    /// Analytic drill removal, expressed in [`Self::frame`].
    pub drill_op: Option<Arc<rs_cam_core::drill_op::DrillOp>>,
    /// Ordinal of the setup group this entry belongs to. Playback resets
    /// whenever the playhead crosses into a different group, because the two
    /// groups need not share a frame.
    pub group: usize,
    /// The frame `toolpath` / `drill_op` / `stock_bbox` are in.
    ///
    /// * `None` — the ZERO-ROOTED stock-relative **global** playback frame,
    ///   shared by every group whose tool axis is global Z. This is what has
    ///   always shipped, and it is what keeps a two-sided project's earlier
    ///   cuts on screen while a later setup replays.
    /// * `Some(info)` — the **setup-local** frame, for a lateral setup, whose
    ///   cut the global stock cannot represent at all (G-LATERALSCRUB; see
    ///   `rs_cam_core::compute::simulate::SimCheckpointMesh::stock_local_to_global`).
    ///   The extracted mesh is mapped back with `info.local_to_global`.
    pub frame: Option<rs_cam_core::compute::SetupTransformInfo>,
    /// Bounding box of the stock this entry stamps into, in [`Self::frame`].
    pub stock_bbox: rs_cam_core::geo::BoundingBox3,
}

pub struct SimulationResult {
    pub mesh: StockMesh,
    pub total_moves: usize,
    pub deviations: Option<Vec<f32>>,
    /// Pointwise per-dexel-column deviations, forwarded verbatim from
    /// `rs_cam_core::compute::simulate::SimulationResult::column_deviations`.
    /// The honest instrument for quality metrics — the per-vertex
    /// `deviations` above are corner-averaged mesh samples suitable for
    /// display, not histograms (P2.g Task 1).
    pub column_deviations: Option<Vec<rs_cam_core::compute::simulate::ColumnDeviation>>,
    pub boundaries: Vec<SimBoundary>,
    /// `Arc`-shared with the core result — see
    /// `rs_cam_core::compute::simulate::SimulationResult::checkpoints` (S5).
    pub checkpoints: Vec<Arc<SimCheckpointMesh>>,
    /// Pre-transformed toolpath data for incremental playback.
    /// Each entry: (toolpath, tool_config, direction).
    pub playback_data: Vec<PlaybackToolpath>,
    /// Rapid-through-stock collisions detected during simulation.
    pub rapid_collisions: Vec<RapidCollision>,
    /// Move indices with rapid collisions (for timeline markers).
    pub rapid_collision_move_indices: Vec<usize>,
    pub cut_trace: Option<Arc<rs_cam_core::simulation_cut::SimulationCutTrace>>,
    pub cut_trace_path: Option<PathBuf>,
    /// True when the requested resolution was coarsened to fit within grid limits.
    pub resolution_clamped: bool,
    /// The dexel COLUMN grid cell this simulation actually used (mm),
    /// forwarded verbatim from
    /// `rs_cam_core::compute::simulate::SimulationResult::column_grid_cell_mm`.
    ///
    /// B7 divergence 2 (2026-08-06): this is a property of the TRACE, and it
    /// is not the same quantity as `SimulationState::resolution`, which is
    /// the dial the NEXT simulation will use. The measurability floors are
    /// cell-size dependent, so a reader that consults the dial can return a
    /// different `NotMeasurable` verdict from core's on identical evidence.
    /// Carried so the GUI's narration can ask the same question core's does.
    pub column_grid_cell_mm: f64,
    /// Per-toolpath snapshots of the material stock *before* that toolpath
    /// carves — forwarded verbatim from the core
    /// `rs_cam_core::compute::simulate::SimulationResult::prior_stocks`.
    /// Retained on `SimulationResults` (F.4) so the submit-time
    /// `FromRemainingStock` gate can look a toolpath's snapshot up by id
    /// directly, instead of re-deriving it from `boundaries()` /
    /// `checkpoints()` position arithmetic.
    pub prior_stocks: HashMap<ToolpathId, Arc<TriDexelStock>>,
}

pub struct CollisionRequest {
    pub annotated: Arc<AnnotatedToolpath>,
    pub tool: ToolConfig,
    pub mesh: Arc<TriangleMesh>,
    /// Workholding fixtures (clearance-expanded boxes) the assembly must
    /// clear, built from the toolpath's setup via
    /// `ProjectSession::collision_obstacles_for_toolpath`. W0.1 / P6-003.
    pub obstacles: Vec<rs_cam_core::collision::CollisionObstacle>,
}

pub struct CollisionResult {
    pub report: CollisionReport,
    pub positions: Vec<[f32; 3]>,
}

/// Request to the Optimize worker lane. Its one variant owns a
/// `ProjectSession` for the duration of the run — the main thread CLONES
/// its session into the request and keeps the original.
///
/// WP14b emptied the other two arms. `Toolpath` became the
/// `optimize_toolpath` `Job` row and `MultitoolPreview` became the
/// existing `preview_tier_map` row, so both now ride
/// [`ComputeLane::Job`](super::ComputeLane::Job) over a handle. The
/// project rollup has no registry row and gets none now (§28 ruling 6),
/// so it stays here — over a clone, not over a `mem::replace`.
pub enum OptimizeRequest {
    /// Run `optimize_project` over every enabled toolpath. Surfaces in
    /// the U3 rollup view.
    Project {
        /// The main thread's own session, CLONED. The worker mutates it
        /// freely and it is dropped with the request; nothing comes
        /// back but the report.
        session: rs_cam_core::session::ProjectSession,
        baseline_trace: Arc<rs_cam_core::simulation_cut::SimulationCutTrace>,
    },
}

/// Result from the Optimize worker.
///
/// It carries NO session. Before WP14b the main thread held
/// `ProjectSession::new_empty()` while the lane ran, so the result had to
/// carry the real one back and a panic on this lane lost the project.
/// The request now owns a clone, so there is nothing to give back.
pub struct OptimizeResult {
    pub kind: OptimizeResultKind,
}

pub enum OptimizeResultKind {
    Project {
        report: rs_cam_core::tool_load::optimize::ProjectOptimizeReport,
    },
}

#[allow(clippy::large_enum_variant)]
enum AnalysisRequest {
    Simulation(SimulationRequest),
    Collision(CollisionRequest),
}

/// One reach-map walk for the viewport overlay (P5).
///
/// Carries no session. `rs_cam_core::reach_map::ReachMapRequest` owns its
/// mesh, spatial index and cutter, so the UI thread resolves one through
/// `ProjectSession::reach_map_spec` — which does no drop-cutter work — and
/// hands it over without lending the session out.
pub struct ReachRequest {
    pub toolpath_id: ToolpathId,
    pub spec: rs_cam_core::reach_map::ReachMapRequest,
}

/// A finished reach-map walk.
///
/// The colours ride back with the map. `ReachMap::vertex_gaps` runs one
/// spatial-index query per mesh vertex, so computing them here rather than in
/// the GPU upload pass keeps a board-sized terrain's per-vertex walk off the
/// frame loop.
pub struct ReachResult {
    /// The toolpath the walk was resolved for. The controller drops a result
    /// whose id no longer matches the selection rather than reporting it.
    pub toolpath_id: ToolpathId,
    pub result: Result<Arc<rs_cam_core::reach_map::ReachMap>, ComputeError>,
    /// One colour per vertex of the mesh the map was measured over, in that
    /// mesh's vertex order. Empty when the walk failed.
    ///
    /// The mesh itself does NOT ride back. The upload pass draws the model in
    /// the DISPLAY frame, which it derives from the session, and checks this
    /// vector's length against that mesh — carrying a second copy of the mesh
    /// here would only invite the two to be confused.
    pub colors: Arc<Vec<[f32; 3]>>,
}

/// One core `Job` row's work step, on its way to the [`ComputeLane::Job`]
/// lane.
///
/// Carries no session. `ProjectSession::start` captured everything the
/// work reads into the handle on the frame loop, so this crosses the
/// thread boundary with no session borrow — the same property
/// [`ReachRequest`] has, and the reason the two rows that ride this lane
/// leave the GUI usable while they run.
pub struct JobRequest {
    /// Identifies this submit. The drain routes the answer by it.
    pub id: JobRequestId,
    /// What `ProjectSession::start` captured.
    pub handle: rs_cam_core::session::JobHandle,
    /// The cancel flag of THIS submit, per §22 ruling 3. NOT the lane's
    /// flag: a flag borrowed from the lane can already read `true` on the
    /// frame loop, and the work polls its own.
    pub cancel: Arc<AtomicBool>,
}

/// A finished `Job` work step.
pub struct JobResult {
    /// The submit this answers.
    pub id: JobRequestId,
    /// The answer, or why there is none.
    pub answer: Result<rs_cam_core::session::JobAnswer, ComputeError>,
}

struct LaneInner<Request> {
    queue: VecDeque<Request>,
    state: LaneState,
    current_job: Option<String>,
    current_phase: Option<String>,
    started_at: Option<Instant>,
    active_toolpath_id: Option<ToolpathId>,
    /// A/M12 — the 0-based position of the in-flight toolpath, so
    /// `generation_status` can name the op by the same index every other MCP
    /// call uses. Only the toolpath lane ever sets it.
    active_toolpath_index: Option<usize>,
    /// The cancel flag of the job the lane is running (§22 ruling 3).
    ///
    /// "Cancel this lane" and "a resubmit supersedes the active job" both
    /// mean the SAME job, so both set this as well as the lane flag. `None`
    /// means the lane runs nothing.
    active_cancel: Option<Arc<AtomicBool>>,
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
            active_toolpath_index: None,
            active_cancel: None,
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
            active_toolpath_id: inner.active_toolpath_id.map(|id| id.0),
            active_toolpath_index: inner.active_toolpath_index,
        }
    }
}

/// A/M12: the toolpath lane answers cancel + status requests on the *caller's*
/// thread. Both operations take only `inner`, whose critical sections are a
/// handful of instructions (queue push/pop, phase-string swap) held by the
/// worker thread — never for the duration of a generation.
impl LaneControl for LaneQueue<ComputeRequest> {
    fn snapshot(&self) -> LaneSnapshot {
        LaneQueue::snapshot(self)
    }

    fn request_cancel(&self) -> CancelOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let was_busy = inner.started_at.is_some();
        if was_busy {
            self.cancel.store(true, Ordering::SeqCst);
            // §22 ruling 3: the running job polls its own flag.
            if let Some(active) = inner.active_cancel.as_ref() {
                active.store(true, Ordering::SeqCst);
            }
            inner.state = LaneState::Cancelling;
        }
        let snapshot = LaneSnapshot {
            lane: self.lane,
            state: inner.state,
            queue_depth: inner.queue.len(),
            current_job: inner.current_job.clone(),
            current_phase: inner.current_phase.clone(),
            started_at: inner.started_at,
            active_toolpath_id: inner.active_toolpath_id.map(|id| id.0),
            active_toolpath_index: inner.active_toolpath_index,
        };
        CancelOutcome { was_busy, snapshot }
    }
}

#[derive(Clone)]
struct ToolpathPhaseTracker {
    lane: Arc<LaneQueue<ComputeRequest>>,
}

impl ToolpathPhaseTracker {
    fn new(lane: Arc<LaneQueue<ComputeRequest>>) -> Self {
        Self { lane }
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

pub struct ThreadedComputeBackend {
    toolpath_lane: Arc<LaneQueue<ComputeRequest>>,
    analysis_lane: Arc<LaneQueue<AnalysisRequest>>,
    optimize_lane: Arc<LaneQueue<OptimizeRequest>>,
    /// P5 — the reach-map overlay's own lane. See [`ComputeLane::Reach`] for
    /// why it is not a third `AnalysisRequest` variant.
    reach_lane: Arc<LaneQueue<ReachRequest>>,
    /// WP14a — the `Job` lane. See [`ComputeLane::Job`].
    job_lane: Arc<LaneQueue<JobRequest>>,
    result_rx: mpsc::Receiver<ComputeMessage>,
    toolpath_handle: Option<std::thread::JoinHandle<()>>,
    analysis_handle: Option<std::thread::JoinHandle<()>>,
    optimize_handle: Option<std::thread::JoinHandle<()>>,
    reach_handle: Option<std::thread::JoinHandle<()>>,
    job_handle: Option<std::thread::JoinHandle<()>>,
    /// S5 — the analysis lane's simulation prefix memo. Shared so the GUI
    /// thread can drop the snapshot at a known point (`clear_sim_prefix_cache`)
    /// without waiting behind whatever is queued on the lane. The lane
    /// serialises simulations, so the mutex is only ever contended by that
    /// explicit clear.
    sim_prefix_cache: Arc<Mutex<rs_cam_core::compute::sim_prefix::SimPrefixCache>>,
}

impl ThreadedComputeBackend {
    pub fn new() -> Self {
        let toolpath_lane = LaneQueue::new(ComputeLane::Toolpath);
        let analysis_lane = LaneQueue::new(ComputeLane::Analysis);
        let optimize_lane = LaneQueue::new(ComputeLane::Optimize);
        let reach_lane = LaneQueue::new(ComputeLane::Reach);
        let job_lane = LaneQueue::new(ComputeLane::Job);
        let (result_tx, result_rx) = mpsc::sync_channel::<ComputeMessage>(64);
        let sim_prefix_cache = Arc::new(Mutex::new(
            rs_cam_core::compute::sim_prefix::SimPrefixCache::new(),
        ));

        let toolpath_handle = spawn_toolpath_lane(Arc::clone(&toolpath_lane), result_tx.clone());
        let analysis_handle = spawn_analysis_lane(
            Arc::clone(&analysis_lane),
            result_tx.clone(),
            Arc::clone(&sim_prefix_cache),
        );
        let optimize_handle = spawn_optimize_lane(Arc::clone(&optimize_lane), result_tx.clone());
        let reach_handle = spawn_reach_lane(Arc::clone(&reach_lane), result_tx.clone());
        let job_handle = spawn_job_lane(Arc::clone(&job_lane), result_tx);

        Self {
            toolpath_lane,
            analysis_lane,
            optimize_lane,
            reach_lane,
            job_lane,
            result_rx,
            toolpath_handle: Some(toolpath_handle),
            analysis_handle: Some(analysis_handle),
            optimize_handle: Some(optimize_handle),
            reach_handle: Some(reach_handle),
            job_handle: Some(job_handle),
            sim_prefix_cache,
        }
    }
}

impl Drop for ThreadedComputeBackend {
    fn drop(&mut self) {
        self.toolpath_lane.shutdown.store(true, Ordering::SeqCst);
        self.analysis_lane.shutdown.store(true, Ordering::SeqCst);
        self.optimize_lane.shutdown.store(true, Ordering::SeqCst);
        self.reach_lane.shutdown.store(true, Ordering::SeqCst);
        self.job_lane.shutdown.store(true, Ordering::SeqCst);
        self.toolpath_lane.wake.notify_all();
        self.analysis_lane.wake.notify_all();
        self.optimize_lane.wake.notify_all();
        self.reach_lane.wake.notify_all();
        self.job_lane.wake.notify_all();
        if let Some(h) = self.toolpath_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.analysis_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.optimize_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.reach_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.job_handle.take() {
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
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let mut inner = self
            .toolpath_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner
            .queue
            .retain(|queued| queued.viz.toolpath_id != request.viz.toolpath_id);
        // G-REGEN-RACE: decided here, under the lane lock, and reported to
        // the caller. The push below is unconditional, so `SupersededActive`
        // is a promise that a replacement request exists — which is what
        // licenses the drain to treat the resulting `Cancelled` as
        // bookkeeping rather than as the toolpath's outcome.
        let mut outcome = ToolpathSubmitOutcome::Queued;
        if inner.active_toolpath_id == Some(request.viz.toolpath_id) {
            self.toolpath_lane.cancel.store(true, Ordering::SeqCst);
            // §22 ruling 3: the running job polls ITS OWN flag, so the
            // supersede must set that one too. The lane flag stays because
            // the worker thread reads it after the generation returns.
            if let Some(active) = inner.active_cancel.as_ref() {
                active.store(true, Ordering::SeqCst);
            }
            if inner.state == LaneState::Running {
                inner.state = LaneState::Cancelling;
            }
            outcome = ToolpathSubmitOutcome::SupersededActive;
        }
        inner.queue.push_back(request);
        if inner.started_at.is_none() {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(toolpath_job_label);
            inner.current_phase = None;
        }
        self.toolpath_lane.wake.notify_one();
        outcome
    }

    fn submit_simulation(&mut self, request: SimulationRequest) {
        self.submit_analysis(AnalysisRequest::Simulation(request));
    }

    fn clear_sim_prefix_cache(&mut self) {
        self.sim_prefix_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
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

    /// Latest selection wins — the same rule the Optimize lane keeps, and for
    /// the same reason: only one answer is ever on screen, so an older walk
    /// has nothing left to produce.
    fn submit_job(&mut self, request: JobRequest) {
        let mut inner = self
            .job_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // FIFO, and a submit supersedes nothing. Two previews at different
        // dial sets are two different questions, and each has a caller
        // waiting on its own answer.
        inner.queue.push_back(request);
        if inner.started_at.is_none() {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(job_label);
            inner.current_phase = None;
        }
        self.job_lane.wake.notify_one();
    }

    fn submit_reach_map(&mut self, request: ReachRequest) {
        let mut inner = self
            .reach_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner.queue.clear();
        inner.queue.push_back(request);
        if inner.started_at.is_some() {
            self.reach_lane.cancel.store(true, Ordering::SeqCst);
            inner.state = LaneState::Cancelling;
        } else {
            inner.state = LaneState::Queued;
            inner.current_job = inner.queue.front().map(reach_job_label);
            inner.current_phase = None;
        }
        self.reach_lane.wake.notify_one();
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
                    // §22 ruling 3: the running job polls its own flag.
                    if let Some(active) = inner.active_cancel.as_ref() {
                        active.store(true, Ordering::SeqCst);
                    }
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
            ComputeLane::Reach => {
                let mut inner = self
                    .reach_lane
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if inner.started_at.is_some() {
                    self.reach_lane.cancel.store(true, Ordering::SeqCst);
                    inner.state = LaneState::Cancelling;
                }
            }
            ComputeLane::Job => {
                let mut inner = self
                    .job_lane
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if inner.started_at.is_some() {
                    self.job_lane.cancel.store(true, Ordering::SeqCst);
                    // §22 ruling 3: the running job polls its own flag.
                    if let Some(active) = inner.active_cancel.as_ref() {
                        active.store(true, Ordering::SeqCst);
                    }
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
            ComputeLane::Reach => self.reach_lane.snapshot(),
            ComputeLane::Job => self.job_lane.snapshot(),
        }
    }

    fn generation_control(&self) -> GenerationControl {
        GenerationControl::new(Arc::clone(&self.toolpath_lane) as Arc<dyn LaneControl>)
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
    format!(
        "{} ({})",
        request.handle.toolpath_name(),
        request.handle.op_label()
    )
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
        OptimizeRequest::Project { .. } => "Optimize project".to_owned(),
    }
}

fn reach_job_label(request: &ReachRequest) -> String {
    format!("Reach map (toolpath #{})", request.toolpath_id.0)
}

/// The status-bar label of one `Job` submit.
fn job_label(request: &JobRequest) -> String {
    use rs_cam_core::session::JobHandle;
    match &request.handle {
        JobHandle::GenerateToolpath(handle) => {
            format!("Generate toolpath #{}", handle.index)
        }
        JobHandle::RecommendClearingStrategy(handle) => {
            format!("Strategy advisor ({})", handle.toolpath_name())
        }
        JobHandle::PreviewTierMap(handle) => {
            format!("Tier map preview ({} tools)", handle.tool_count())
        }
        JobHandle::OptimizeToolpath(handle) => {
            format!("Optimize toolpath #{}", handle.index)
        }
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
                    inner.active_toolpath_index = None;
                    inner.active_cancel = None;
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
                inner.active_toolpath_id = Some(request.viz.toolpath_id);
                inner.active_toolpath_index = Some(request.handle.index);
                // The flag a cancel of THIS job must set.
                inner.active_cancel = Some(Arc::clone(&request.viz.cancel));
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // Wrap the compute + result-send body in catch_unwind so a panic
            // in any operation does not kill the worker thread or poison mutexes
            // permanently.  On panic we log the error, reset the lane to Idle,
            // send an error result back, and continue the loop.
            let toolpath_id = request.viz.toolpath_id;
            let revision = Some(request.handle.revision);
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let phase_tracker = ToolpathPhaseTracker::new(Arc::clone(&lane));
                let mut outcome = execute::run_compute_with_phase(&request, &phase_tracker);
                // A cancelled generation reports `Cancelled` whatever the
                // generator said. `execute_job` returns a plain
                // `OperationFailed` when the flag stops it — core carries no
                // cancel variant — so the flag is the evidence, not the
                // message. Widened from the pre-WP11b `&& is_ok()`: the two
                // agree, because the old path already mapped
                // `OperationError::Cancelled` to `ComputeError::Cancelled`.
                if lane.cancel.load(Ordering::SeqCst) {
                    outcome.result = Err(ComputeError::Cancelled);
                }
                phase_tracker.clear();

                {
                    let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.active_toolpath_id = None;
                    inner.active_cancel = None;
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

                let _ = result_tx.send(ComputeMessage::Toolpath(Box::new(ComputeResult {
                    toolpath_id: request.viz.toolpath_id,
                    revision,
                    result: outcome.result,
                    debug_trace: outcome.debug_trace,
                    semantic_trace: outcome.semantic_trace,
                    debug_trace_path: outcome.debug_trace_path,
                })));
            }));

            if let Err(panic_payload) = caught {
                let msg = panic_message(&panic_payload);
                tracing::error!("rs_cam crashed due to internal error (toolpath worker): {msg}");

                // Reset lane state so subsequent jobs can still run.
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.active_toolpath_id = None;
                inner.active_cancel = None;
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

                let _ = result_tx.send(ComputeMessage::Toolpath(Box::new(ComputeResult {
                    toolpath_id,
                    revision,
                    result: Err(ComputeError::Message(format!(
                        "Crashed due to internal error: {msg}"
                    ))),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                })));
            }
        }
    })
}

fn spawn_analysis_lane(
    lane: Arc<LaneQueue<AnalysisRequest>>,
    result_tx: mpsc::SyncSender<ComputeMessage>,
    sim_prefix_cache: Arc<Mutex<rs_cam_core::compute::sim_prefix::SimPrefixCache>>,
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
                    inner.active_toolpath_index = None;
                    inner.active_cancel = None;
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
                        // S5: the memo is held for the whole simulation, which
                        // is why it lives on the lane rather than in a global.
                        let mut cache = sim_prefix_cache.lock().unwrap_or_else(|e| e.into_inner());
                        let result = execute::run_simulation_with_phase(
                            &request,
                            &lane.cancel,
                            set_phase,
                            Some(rs_cam_core::compute::sim_prefix::SimMemo {
                                store: request.memoize_prefix,
                                cache: &mut cache,
                            }),
                        );
                        drop(cache);
                        let result = if lane.cancel.load(Ordering::SeqCst) && result.is_ok() {
                            Err(ComputeError::Cancelled)
                        } else {
                            result
                        };
                        ComputeMessage::Simulation(result.map(Box::new))
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
    use rs_cam_core::tool_load::optimize::{ProjectOptimizeReport, optimize_project};

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
                    inner.active_toolpath_index = None;
                    inner.active_cancel = None;
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

            // This lane carries NO `catch_unwind`, and WP14b changed why.
            // Before it, a panic here lost the main thread's only session,
            // which was worse than killing the worker. The request now
            // owns a clone, so a panic loses the rollup and the clone and
            // nothing else — the trade-off is the worker thread, which
            // does not come back until the app restarts. The `Job` lane
            // catches instead, because a job has a caller waiting on a
            // oneshot. Adding one here is a separate decision; do not
            // make it silently.
            let progress = LaneProgressBridge {
                lane: Arc::clone(&lane),
            };
            let result = match request {
                OptimizeRequest::Project {
                    mut session,
                    baseline_trace,
                } => {
                    let report: ProjectOptimizeReport =
                        optimize_project(&mut session, &baseline_trace, &progress, &lane.cancel);
                    OptimizeResult {
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

            let _ = result_tx.send(ComputeMessage::Optimize(Box::new(result)));
        }
    })
}

/// The `Job` lane's worker thread (WP14a).
///
/// It pops one [`JobRequest`], runs its work step and sends the answer
/// back. Every step is a free function over a handle, so this thread holds
/// no session and the GUI stays usable while the job runs.
///
/// **A panic reports an error rather than nothing.** The reach lane can
/// drop a panicked walk silently, because an overlay that never arrives
/// leaves the plain model on screen. A job has a caller waiting on a
/// oneshot, and a dropped answer would hang it.
fn spawn_job_lane(
    lane: Arc<LaneQueue<JobRequest>>,
    result_tx: mpsc::SyncSender<ComputeMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let mut request = {
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
                    inner.active_toolpath_index = None;
                    inner.active_cancel = None;
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
                inner.current_job = Some(job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                // `active_toolpath_id` is deliberately NOT set, for the
                // reason `ComputeLane::Reach` states: MCP `generation_status`
                // reads it as the op being GENERATED.
                inner.active_cancel = Some(Arc::clone(&request.cancel));
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            let id = request.id;
            let answer = match std::panic::catch_unwind(AssertUnwindSafe(|| run_job(&mut request)))
            {
                Ok(answer) => answer,
                Err(panic_payload) => {
                    let msg = panic_message(&panic_payload);
                    tracing::error!("rs_cam crashed due to internal error (job worker): {msg}");
                    Err(ComputeError::Message(format!("the job panicked: {msg}")))
                }
            };
            // A cancelled job reports `Cancelled` whatever the step said:
            // core carries no cancel variant, so the flag is the evidence.
            let answer =
                if request.cancel.load(Ordering::SeqCst) || lane.cancel.load(Ordering::SeqCst) {
                    Err(ComputeError::Cancelled)
                } else {
                    answer
                };

            {
                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.started_at = None;
                inner.current_phase = None;
                inner.active_cancel = None;
                if inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                } else {
                    inner.state = LaneState::Queued;
                    inner.current_job = inner.queue.front().map(job_label);
                }
            }

            let _ = result_tx.send(ComputeMessage::Job(Box::new(JobResult { id, answer })));
        }
    })
}

/// Run one `Job` row's work step.
///
/// Every arm is a free function over the row's own handle, so this holds
/// no session of the main thread's. `generate_toolpath` is the exception
/// and it is not an exception to that rule: it runs on the toolpath lane,
/// which reports its own result type, so this lane refuses it rather than
/// producing a second answer for it.
///
/// The request is taken by `&mut` for the `optimize_toolpath` arm alone
/// (§28 ruling 4): that handle owns a cloned session and the optimizer
/// takes it mutably, so a shared handle would cost a second clone.
fn run_job(request: &mut JobRequest) -> Result<rs_cam_core::session::JobAnswer, ComputeError> {
    use rs_cam_core::session::{JobAnswer, JobHandle};
    let cancel = Arc::clone(&request.cancel);
    match &mut request.handle {
        JobHandle::GenerateToolpath(_) => Err(ComputeError::Message(
            "generate_toolpath runs on the toolpath lane, which reports its own result".to_owned(),
        )),
        JobHandle::RecommendClearingStrategy(handle) => {
            rs_cam_core::session::execute_recommend_clearing_strategy(handle, &cancel)
                .map(|answer| JobAnswer::RecommendClearingStrategy(Box::new(answer)))
                .map_err(|error| ComputeError::Message(error.to_string()))
        }
        JobHandle::PreviewTierMap(handle) => {
            rs_cam_core::session::execute_preview_tier_map(handle, &cancel)
                .map(|answer| JobAnswer::PreviewTierMap(Box::new(answer)))
                .map_err(|error| ComputeError::Message(error.to_string()))
        }
        JobHandle::OptimizeToolpath(handle) => {
            let outcome = rs_cam_core::session::execute_optimize_toolpath(handle, &cancel);
            Ok(JobAnswer::OptimizeToolpath(Box::new(outcome)))
        }
    }
}

/// The reach-map overlay's worker (P5).
///
/// Mirrors [`spawn_analysis_lane`] in shape: pop under the lane lock, run
/// outside it, reset the lane, send. Two things it does differently, and both
/// are deliberate:
///
/// * It computes the per-vertex colours here, beside the map.
///   [`rs_cam_core::reach_map::ReachMap::vertex_gaps`] runs one
///   spatial-index query per mesh vertex — hundreds of thousands on a
///   board-sized terrain — so building them in the GPU upload pass would put
///   that walk on the frame loop.
/// * A cancelled walk reports [`ComputeError::Cancelled`] rather than a
///   message. The controller must tell a supersede from a failure: a
///   supersede leaves the overlay idle for the sweep to retry, a failure is
///   shown to the operator.
fn spawn_reach_lane(
    lane: Arc<LaneQueue<ReachRequest>>,
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
                    inner.active_toolpath_index = None;
                    inner.active_cancel = None;
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
                inner.current_job = Some(reach_job_label(&request));
                inner.current_phase = None;
                inner.started_at = Some(Instant::now());
                // `active_toolpath_id` is deliberately NOT set. Its doc on
                // `LaneSnapshot` says it is `None` on every lane but the
                // toolpath one, and MCP `generation_status` reads it as the
                // op being GENERATED. The reach job's own label carries the
                // id for anyone reading the status bar.
                request
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // Same guard the toolpath and analysis lanes carry: a panic in
            // the walk must not kill the worker or poison the lane mutex.
            let toolpath_id = request.toolpath_id;
            let mesh = Arc::clone(&request.spec.mesh);
            let index = Arc::clone(&request.spec.index);
            // `AtomicBool` does not implement `CancelCheck`; the trait has a
            // blanket impl for `Fn() -> bool`, so the lane's flag is read
            // through a closure — the same adapter `session::multitool`
            // builds at its own `cached_tier_map` call.
            let cancel_fn = || lane.cancel.load(Ordering::SeqCst);
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let built =
                    match rs_cam_core::reach_map_cache::cached_reach_map(&request.spec, &cancel_fn)
                    {
                        Ok(map) => Ok(map),
                        // `Cancelled` is a unit struct and carries nothing to
                        // preserve; the lane's own error says the same thing.
                        Err(rs_cam_core::interrupt::Cancelled) => Err(ComputeError::Cancelled),
                    };
                let built = if lane.cancel.load(Ordering::SeqCst) {
                    Err(ComputeError::Cancelled)
                } else {
                    built
                };
                let colors = built.as_ref().map_or_else(
                    |_| Vec::new(),
                    |map| {
                        crate::state::runtime::reach_overlay_colors(
                            map.as_ref(),
                            mesh.as_ref(),
                            index.as_ref(),
                        )
                    },
                );

                {
                    let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.started_at = None;
                    inner.current_phase = None;
                    if inner.queue.is_empty() {
                        inner.state = LaneState::Idle;
                        inner.current_job = None;
                    } else {
                        inner.state = LaneState::Queued;
                        inner.current_job = inner.queue.front().map(reach_job_label);
                    }
                }

                let _ = result_tx.send(ComputeMessage::Reach(Box::new(ReachResult {
                    toolpath_id,
                    result: built,
                    colors: Arc::new(colors),
                })));
            }));

            if let Err(panic_payload) = caught {
                let msg = panic_message(&panic_payload);
                tracing::error!("rs_cam crashed due to internal error (reach worker): {msg}");

                let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.started_at = None;
                inner.current_phase = None;
                if inner.queue.is_empty() {
                    inner.state = LaneState::Idle;
                    inner.current_job = None;
                } else {
                    inner.state = LaneState::Queued;
                    inner.current_job = inner.queue.front().map(reach_job_label);
                }
            }
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
