pub(crate) mod execute;
pub mod helpers;
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
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

use super::{
    CancelOutcome, ComputeBackend, ComputeError, ComputeLane, ComputeMessage, GenerationControl,
    LaneControl, LaneSnapshot, LaneState, ToolpathSubmitOutcome,
};
use crate::state::job::ToolConfig;
#[cfg(test)]
use crate::state::job::ToolType;
use crate::state::toolpath::{
    DressupConfig, OperationConfig, StockSource, ToolpathId, ToolpathResult,
};

pub struct ComputeRequest {
    pub toolpath_id: ToolpathId,
    /// A/M12: 0-based position in `ProjectSession::toolpath_configs()` at
    /// submit time. The lane republishes it on [`LaneSnapshot`] so
    /// `generation_status` can name the in-flight op by the same index every
    /// other MCP call uses. The lane itself has no session access.
    pub toolpath_index: usize,
    pub toolpath_name: String,
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
    /// G-DRILLPICK-FRAME: the setup's local<->global transform, `None` for
    /// identity setups. The controller transforms mesh and polygons into the
    /// emission frame before submitting, but `selected_holes` ride along on
    /// the operation config as raw model coordinates, so the generator needs
    /// the matrix itself to put drill picks in the same frame as everything
    /// else. Mirrors `ResolvedGenInputs::setup_transform` on the core path.
    pub setup_transform: Option<rs_cam_core::compute::transform::SetupTransformInfo>,
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
    /// R1 (pencil): resolved real reference tool config when the Pencil op names
    /// one via `reference_tool_id`. Resolved in the controller (which has the
    /// tool list), threaded to `execute_operation_annotated`. `None` = nominal.
    pub reference_tool_cfg: Option<ToolConfig>,
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
    /// Workpiece material (F-016). Forwarded into the drill-op view so
    /// `chip_welding` / `peck_adequacy` / `plunge_feed` gates evaluate
    /// against the actual stock, not `Material::default()`.
    pub material: rs_cam_core::material::Material,
    /// P2.2/P2.3 (rest-region boundary): the full, un-unioned set of rest
    /// region polygons for `BoundarySource::DerivedRestRegions`, resolved
    /// from the source toolpath's cached result. Resolved by the
    /// controller (which has cross-toolpath access via `gui.toolpath_rt`)
    /// — this worker's `ComputeRequest` is scoped to a single toolpath and
    /// has no way to look up another toolpath's result itself.
    ///
    /// The controller fails the toolpath hard (see
    /// `submit_toolpath_compute`) rather than submitting a request when
    /// the source is missing, self-referential, ungenerated, or has no
    /// rest regions — so a worker that receives `Some` here can assume
    /// the `Vec` is non-empty. `None` only when the boundary source isn't
    /// `DerivedRestRegions`.
    ///
    /// The real enforcement clip (further down in `run_compute_with_phase_tracker`)
    /// uses every region in this set via `ProjectSession::apply_boundary_clip_multi`
    /// — the adaptive3d pre-clip optimization in `generate_via_core` unions
    /// them down to one polygon only when that union collapses cleanly.
    pub derived_rest_regions: Option<Vec<Polygon2>>,
    /// Op-agnostic rest analysis (P2.5). Mirrors `boundary` — resolved by
    /// the controller from the toolpath's `ToolpathEntry::rest_analysis`
    /// and threaded to `execute_operation_annotated_with_regions`.
    pub rest_analysis: crate::state::toolpath::RestAnalysisConfig,
    /// P1 quantitative linker — the machine envelope generators use to
    /// cost link candidates (pencil hookup). Snapshot taken when the
    /// request is built; `None` disables cost-based link decisions
    /// (legacy always-link behavior).
    pub link_kinematics: Option<rs_cam_core::machine_kinematics::LinkKinematics>,
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
        toolpath_id: rs_cam_core::ToolpathId,
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
        toolpath_id: rs_cam_core::ToolpathId,
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
    /// A/M12 — see [`ComputeRequest::toolpath_index`]. Only the toolpath lane
    /// ever sets it.
    active_toolpath_index: Option<usize>,
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
        let optimize_handle = spawn_optimize_lane(Arc::clone(&optimize_lane), result_tx);

        Self {
            toolpath_lane,
            analysis_lane,
            optimize_lane,
            result_rx,
            toolpath_handle: Some(toolpath_handle),
            analysis_handle: Some(analysis_handle),
            optimize_handle: Some(optimize_handle),
            sim_prefix_cache,
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
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let mut inner = self
            .toolpath_lane
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        inner
            .queue
            .retain(|queued| queued.toolpath_id != request.toolpath_id);
        // G-REGEN-RACE: decided here, under the lane lock, and reported to
        // the caller. The push below is unconditional, so `SupersededActive`
        // is a promise that a replacement request exists — which is what
        // licenses the drain to treat the resulting `Cancelled` as
        // bookkeeping rather than as the toolpath's outcome.
        let mut outcome = ToolpathSubmitOutcome::Queued;
        if inner.active_toolpath_id == Some(request.toolpath_id) {
            self.toolpath_lane.cancel.store(true, Ordering::SeqCst);
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
                    inner.active_toolpath_index = None;
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
                inner.active_toolpath_index = Some(request.toolpath_index);
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

                let _ = result_tx.send(ComputeMessage::Toolpath(Box::new(ComputeResult {
                    toolpath_id: request.toolpath_id,
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
    use rs_cam_core::tool_load::RefuseReason;
    use rs_cam_core::tool_load::optimize::{
        OptimizeOutcome, OutcomeKind, OutcomeNarrative, ProjectOptimizeReport, optimize_project,
        optimize_toolpath,
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
                    inner.active_toolpath_index = None;
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
                        match outcome.kind {
                            OutcomeKind::Ranked
                            | OutcomeKind::MarginalSafe
                            | OutcomeKind::TradeOff
                            | OutcomeKind::NoSafeImprovement => outcome,
                            OutcomeKind::Skipped => OptimizeOutcome::no_safe_improvement(
                                Vec::new(),
                                RefuseReason::NoImprovementFound,
                                OutcomeNarrative {
                                    explanation: "cancelled before optimization could run"
                                        .to_owned(),
                                    ..OutcomeNarrative::default()
                                },
                            ),
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

            let _ = result_tx.send(ComputeMessage::Optimize(Box::new(result)));
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
