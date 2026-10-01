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

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use rs_cam_core::budget::{
    BudgetGuard, MemoryBudget, MemoryLimit, ProcessRss, StopReason, UsageProbe,
};
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::stock::collision::CollisionReport;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

use super::{
    CancelOutcome, ComputeBackend, ComputeError, ComputeLane, ComputeMessage, GenerationControl,
    JobRequestId, LaneControl, LaneSnapshot, LaneState, MemoryControl, MemoryStatus,
    MemoryStatusSource, ToolpathSubmitOutcome,
};
use crate::state::job::ToolConfig;
use crate::state::toolpath::{ToolpathId, ToolpathResult};

/// The artifact folder for the settings value `file`: the production
/// resolver (`rs_cam_core::settings::paths::artifact_dir`).
#[cfg(not(test))]
fn default_artifact_dir(file: Option<&std::path::Path>) -> Option<PathBuf> {
    rs_cam_core::settings::paths::artifact_dir(file)
}

/// The artifact folder for the settings value `file` in a unit-test build:
/// the file value, else [`test_artifact_dir`]. Never the user cache.
#[cfg(test)]
fn default_artifact_dir(file: Option<&std::path::Path>) -> Option<PathBuf> {
    Some(file.map_or_else(test_artifact_dir, std::path::Path::to_path_buf))
}

/// The per-process scratch artifact folder of a unit-test build, under
/// `std::env::temp_dir()`.
#[cfg(test)]
pub(crate) fn test_artifact_dir() -> PathBuf {
    std::env::temp_dir().join(format!("rs_cam_viz_test_artifacts_{}", std::process::id()))
}

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
    /// Where a debug trace file goes when the operator asked for a trace.
    pub artifacts: ArtifactPolicy,
}

/// Whether, and where, a lane writes its diagnostic files: the simulation
/// cut-trace file (`simulation_metrics/`) and the toolpath debug trace
/// (`toolpath_debug/`).
///
/// The controller builds it from `[diagnostics]` of the settings file
/// (File ▸ Preferences ▸ Diagnostics) for each submit, so a change applies
/// to the next job. A test gives a scratch folder, so no test writes the
/// operator's cache folder.
///
/// It decides FILES only. The in-memory cut trace is always captured
/// (`state/CLAUDE.md`: every run captures the cut trace), and nothing in the
/// product reads the file back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPolicy {
    /// Write the simulation cut-trace file. Default off: operator ruling
    /// 2026-10-02.
    pub save_cut_trace: bool,
    /// How many cut-trace files stay in `simulation_metrics/` (G-SIMDUMP:
    /// each file can be several GB).
    pub cut_trace_retain: usize,
    /// The artifact folder. `None` when no folder resolves: then no file
    /// is written.
    pub dir: Option<PathBuf>,
}

impl ArtifactPolicy {
    /// The policy of `[diagnostics]`. The folder is the file's value, else
    /// the per-user cache folder (`rs_cam_core::settings::paths`).
    ///
    /// In a unit-test build the default folder is [`test_artifact_dir`],
    /// not the user cache. Every controller job reads its folder here, so
    /// no test with default settings can write into the operator's cache
    /// folder. A file value still wins.
    #[must_use]
    pub fn from_settings(diagnostics: &rs_cam_core::settings::DiagnosticsSettings) -> Self {
        Self {
            save_cut_trace: diagnostics.save_cut_trace,
            cut_trace_retain: diagnostics.cut_trace_retain,
            dir: default_artifact_dir(diagnostics.artifact_dir.as_deref()),
        }
    }

    /// No file of any kind.
    #[must_use]
    pub fn none() -> Self {
        Self {
            save_cut_trace: false,
            cut_trace_retain: rs_cam_core::settings::DEFAULT_CUT_TRACE_RETAIN,
            dir: None,
        }
    }

    /// The cut-trace folder, when the switch is on and a folder resolves.
    #[must_use]
    pub fn cut_trace_dir(&self) -> Option<PathBuf> {
        if self.save_cut_trace {
            self.dir.as_ref().map(|dir| dir.join("simulation_metrics"))
        } else {
            None
        }
    }

    /// The debug-trace folder, when a folder resolves. The operator's
    /// per-toolpath trace switch decides whether the file is written.
    #[must_use]
    pub fn toolpath_debug_dir(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join("toolpath_debug"))
    }
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
    pub debug_trace: Option<Arc<rs_cam_core::trace::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

// Re-export from core — the struct and all methods now live in rs_cam_core.
pub use rs_cam_core::compute::SetupTransformInfo;

/// The simulation lane's request: one core simulation request, plus the one
/// viz-only dial core carries no slot for.
///
/// **One request type, not a mirror.** Every field of the simulation itself
/// lives on [`rs_cam_core::compute::simulate::SimulationRequest`]. The viz
/// held `SetupSimGroup`, `SetupSimToolpath` and a field-identical twin of
/// core's request until 2026-09-18, and one translation site copied them
/// across field by field, so a new core field dropped out at every hop
/// (CMP-19). [`SimulationResult`] below carries the same shape for the same
/// reason.
pub struct SimulationRequest {
    /// The request, in core's own type. Read its field docs there.
    pub core: rs_cam_core::compute::simulate::SimulationRequest,
    /// S5 — leave a prefix snapshot behind for the next simulation to resume
    /// from (`rs_cam_core::compute::sim_prefix`).
    ///
    /// Set only by the `generate_all` fixpoint ladder, which is the one caller
    /// that runs several simulations over a growing project in quick
    /// succession. Every other simulation still *consumes* a held snapshot if
    /// it matches — that is free — but leaves none, so the memo's memory is
    /// released at the next run rather than held for the life of the session.
    pub memoize_prefix: bool,
    /// Whether, and where, the run writes its cut-trace file.
    pub artifacts: ArtifactPolicy,
}

pub use rs_cam_core::compute::simulate::{SimBoundary, SimCheckpointMesh};

/// One entry in the live-sim playback stream: a pre-transformed toolpath, the
/// tool, the cut direction to stamp it with, and (for drill operations)
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
    /// The same cutter the compute pass stamped the checkpoint stocks with,
    /// shared from `SimToolpathEntry::tool`. The playback loop used to hold
    /// the `ToolConfig` and rebuild the cutter once per frame.
    pub tool: Arc<rs_cam_core::tool::ToolDefinition>,
    /// Direction to stamp with, in [`Self::frame`]. Always `FromTop` for a
    /// setup-local entry, because setup-local Z is always the tool axis.
    pub direction: StockCutDirection,
    /// Analytic drill removal, expressed in [`Self::frame`].
    pub drill_op: Option<Arc<rs_cam_core::ops::drill_op::DrillOp>>,
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

/// The simulation lane's answer: one core simulation, plus the two
/// viewport-only artifacts core carries no slot for.
///
/// **One simulation type, not a mirror.** Every field of the simulation
/// itself lives on [`rs_cam_core::compute::simulate::SimulationResult`].
/// The viz held a hand-written twin of that field list until 2026-09-17,
/// and one map site copied twelve fields into core's type by hand, so a
/// new core field dropped out of the session's copy in silence. That is
/// the class of defect C04 found after a CLI mirror had dropped eleven
/// fields.
pub struct SimulationResult {
    /// The simulation, in core's own type. Read its field docs there.
    pub core: rs_cam_core::compute::simulate::SimulationResult,
    /// Pre-transformed toolpath data for incremental playback.
    /// Each entry: (toolpath, tool_config, direction).
    pub playback_data: Vec<PlaybackToolpath>,
    /// Where the run wrote its cut-trace artifact — a viz-only filesystem
    /// concern, so core carries no slot for it.
    pub cut_trace_path: Option<PathBuf>,
}

pub struct CollisionRequest {
    pub annotated: Arc<AnnotatedToolpath>,
    pub tool: ToolConfig,
    pub mesh: Arc<TriangleMesh>,
    /// Workholding fixtures (clearance-expanded boxes) the assembly must
    /// clear, built from the toolpath's setup via
    /// `ProjectSession::collision_obstacles_for_toolpath`. W0.1 / P6-003.
    pub obstacles: Vec<rs_cam_core::stock::collision::CollisionObstacle>,
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
        baseline_trace: Arc<rs_cam_core::stock::simulation_cut::SimulationCutTrace>,
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
    /// `Some` when the memory budget stopped the run (plan B3). The report
    /// then holds what the run found before the stop.
    ///
    /// Only [`StopReason::OverBudget`] is stored here. An operator cancel
    /// stays inside the report as a `Cancelled` outcome, as before.
    pub budget_stop: Option<StopReason>,
}

pub enum OptimizeResultKind {
    Project {
        report: rs_cam_core::tool_load::optimize::ProjectOptimizeReport,
    },
}

#[allow(clippy::large_enum_variant)]
enum AnalysisRequest {
    /// A simulation and its memory estimate for the ledger, in bytes.
    /// `None` means NOT ESTIMATED: the budget has no limit, so the submit
    /// does not spend the time to count.
    Simulation(SimulationRequest, Option<u64>),
    Collision(CollisionRequest),
}

/// One reach-map walk for the viewport overlay (P5).
///
/// Carries no session. `rs_cam_core::maps::reach_map::ReachMapRequest` owns its
/// mesh, spatial index and cutter, so the UI thread resolves one through
/// `ProjectSession::reach_map_spec` — which does no drop-cutter work — and
/// hands it over without lending the session out.
pub struct ReachRequest {
    pub toolpath_id: ToolpathId,
    pub spec: rs_cam_core::maps::reach_map::ReachMapRequest,
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
    /// The memo key of the request the walk answered. The controller drops a
    /// result whose key is not the overlay's current key: a walk sent before
    /// a tool or parameter edit must not show the previous tool's map.
    pub key: rs_cam_core::maps::reach_map_cache::ReachRequestKey,
    pub result: Result<Arc<rs_cam_core::maps::reach_map::ReachMap>, ComputeError>,
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

/// The memory budget of the heavy lanes, and the probe a job reads its
/// usage with (plan B1 and B3, `planning/memory_budget_2026-10-01/PLAN.md`).
///
/// Each heavy job (toolpath, simulation and collision, project optimize)
/// builds ONE [`BudgetGuard`] from this over the flag the job already polls,
/// and runs [`BudgetGuard::watch`] while the job runs. The watch thread
/// sets that flag when the process goes over the limit, so every existing
/// `&AtomicBool` check also stops on the budget, with no change to an
/// algorithm signature. The lane then reads `stop_reason()` and reports
/// [`ComputeError::OverBudget`], not [`ComputeError::Cancelled`].
///
/// A budget with no limit starts no thread and records no reason.
///
/// **The budget can change at run time** (File ▸ Preferences, operator
/// ruling 2026-10-02). Every clone of a `JobBudget` and the
/// [`BudgetLedger`] share ONE cell, so [`Self::set`] reaches every lane at
/// once. A job copies the budget into its guard when it starts
/// ([`Self::guard`]), so a running job keeps its old limit, and the next
/// job reads the new one.
#[derive(Clone)]
pub struct JobBudget {
    budget: Arc<Mutex<BudgetCell>>,
    probe: Arc<dyn UsageProbe>,
    interval: Duration,
}

/// The one shared budget cell: the budget that the lanes enforce, and the
/// setting that gave it. Only `generation_status` reads the setting; it
/// names the source of the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BudgetCell {
    budget: MemoryBudget,
    setting: MemoryLimit,
}

/// The setting that gives exactly `budget`: no limit is `Unlimited`, a
/// limit is `Bytes`. A caller that knows the real setting gives it instead.
fn setting_of(budget: MemoryBudget) -> MemoryLimit {
    budget
        .limit_bytes
        .map_or(MemoryLimit::Unlimited, MemoryLimit::Bytes)
}

impl JobBudget {
    /// The budget with the process RSS probe and the core rate limit.
    pub fn new(budget: MemoryBudget) -> Self {
        Self::with_probe(
            budget,
            Arc::new(ProcessRss),
            rs_cam_core::budget::guard::PROBE_INTERVAL,
        )
    }

    /// The budget with a given probe and rate limit. Tests give a fake
    /// probe and a zero interval.
    pub fn with_probe(
        budget: MemoryBudget,
        probe: Arc<dyn UsageProbe>,
        interval: Duration,
    ) -> Self {
        Self {
            budget: Arc::new(Mutex::new(BudgetCell {
                budget,
                setting: setting_of(budget),
            })),
            probe,
            interval,
        }
    }

    /// The budget this value enforces now.
    pub fn budget(&self) -> MemoryBudget {
        self.cell().budget
    }

    fn cell(&self) -> BudgetCell {
        *self.budget.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Change the budget for every job that starts after this call. A job
    /// that runs keeps the guard it started with. The setting becomes the
    /// one that gives exactly `budget` ([`Self::set_configured`] names the
    /// real one).
    pub fn set(&self, budget: MemoryBudget) {
        self.set_configured(budget, setting_of(budget));
    }

    /// Change the budget, and record the `setting` that gave it.
    pub fn set_configured(&self, budget: MemoryBudget, setting: MemoryLimit) {
        *self.budget.lock().unwrap_or_else(|e| e.into_inner()) = BudgetCell { budget, setting };
    }

    /// One guard for one job, over the flag that job polls. The guard holds
    /// a COPY of the budget now, so a later [`Self::set`] does not change
    /// it.
    fn guard(&self, flag: Arc<AtomicBool>) -> Arc<BudgetGuard> {
        Arc::new(BudgetGuard::with_probe(
            flag,
            self.budget(),
            Box::new(SharedProbe(Arc::clone(&self.probe))),
            self.interval,
        ))
    }
}

/// The [`MemoryStatusSource`] of the threaded backend: the shared budget
/// cell, the ledger and the probe. Every read takes one short lock at a
/// time, so the MCP thread never waits for a job.
struct LedgerMemoryStatus {
    budget: JobBudget,
    ledger: Arc<BudgetLedger>,
}

impl MemoryStatusSource for LedgerMemoryStatus {
    fn read(&self) -> MemoryStatus {
        // The budget lock and the ledger lock are taken one after the
        // other, never nested (the order rule of `BudgetLedger::admit`).
        let cell = self.budget.cell();
        let reserved_bytes = self.ledger.reserved_bytes();
        MemoryStatus {
            limit_bytes: cell.budget.limit_bytes,
            setting: cell.setting,
            reserved_bytes,
            process_rss_bytes: self.budget.probe.used_bytes(),
        }
    }
}

/// A [`UsageProbe`] that the guards of many jobs share.
struct SharedProbe(Arc<dyn UsageProbe>);

impl UsageProbe for SharedProbe {
    fn used_bytes(&self) -> Option<u64> {
        self.0.used_bytes()
    }
}

/// The lane error of a job that the memory budget stopped, or `None`.
///
/// Read it AFTER the job returns. A job that the budget stopped returns
/// `Cancelled`, a plain failure, or even `Ok` when the stop came after its
/// last flag read. In each case the lane reports the budget stop, so the
/// operator sees why and never a plain "Cancelled". This is the same rule
/// the lanes keep for an operator cancel: the flag is the evidence, not
/// the job's own answer.
fn over_budget_stop(guard: &BudgetGuard) -> Option<ComputeError> {
    match guard.stop_reason() {
        Some(reason @ StopReason::OverBudget { .. }) => Some(reason.into()),
        Some(StopReason::User) | None => None,
    }
}

/// The memory ledger of the heavy lanes (plan B4, "Architecture" 3,
/// `planning/memory_budget_2026-10-01/PLAN.md`).
///
/// The toolpath, analysis and optimize lanes share ONE ledger. Each of their
/// jobs takes a [`LedgerTicket`] in [`LaneQueue::dequeue_reserved`] before
/// it runs, and the ticket gives the reservation back when it drops. The
/// lane loop holds the ticket in a local, so it drops at the end of the job
/// on every path: a result, a cancel, an error and a caught panic.
///
/// **The wait rule.** A job WAITS in its queue (state `Queued`, visible to
/// the status bar and to MCP `generation_status`) when ALL of these are
/// true:
///
/// 1. the budget has a limit;
/// 2. the job has an estimate (`RunningJob::estimate_bytes` is `Some`);
/// 3. another heavy job runs now;
/// 4. the probe reading + the reserved bytes + the estimate is over the
///    limit.
///
/// Rule 3 makes the ledger an ORDER, never a refusal: when nothing else
/// runs, nothing can give memory back, so the job runs and the preflight and
/// the guard decide. Rule 2 means an unknown estimate never blocks. The
/// reading counts what the running jobs hold so far, and the reserved bytes
/// count their full estimates again, so the sum is on the safe side: two
/// jobs that would fit can run one after the other.
///
/// A budget with no limit never waits, so the lanes keep today's
/// concurrency.
///
/// The ledger reads the budget from the cell of the backend's
/// [`JobBudget`], so a budget change applies to the next admission. A job
/// that waits checks again at once (`ThreadedComputeBackend::set_memory_budget`
/// wakes it).
pub(crate) struct BudgetLedger {
    budget: Arc<Mutex<BudgetCell>>,
    probe: Arc<dyn UsageProbe>,
    state: Mutex<LedgerState>,
    /// Signalled when a ticket drops, and at shutdown.
    released: Condvar,
}

#[derive(Debug, Default, Clone, Copy)]
struct LedgerState {
    /// The sum of the estimates of the running heavy jobs, in bytes.
    reserved_bytes: u64,
    /// The heavy jobs that run now, with an estimate or without.
    running: usize,
}

/// Why a job waits for the ledger. The lane shows it as the job's phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LedgerWait {
    estimate_bytes: u64,
    used_bytes: u64,
    reserved_bytes: u64,
    limit_bytes: u64,
}

impl LedgerWait {
    /// The phase text of a waiting job.
    fn phase(&self) -> String {
        use rs_cam_core::budget::format_bytes;
        format!(
            "Waiting for memory: needs about {}; {} in use and {} reserved by running jobs; \
             the limit is {}",
            format_bytes(self.estimate_bytes),
            format_bytes(self.used_bytes),
            format_bytes(self.reserved_bytes),
            format_bytes(self.limit_bytes),
        )
    }
}

/// The ledger's answer to one job.
enum Admission {
    Run(LedgerTicket),
    Wait(LedgerWait),
}

/// One running heavy job's reservation. Dropping it gives the bytes back
/// and wakes every waiting lane.
pub(crate) struct LedgerTicket {
    ledger: Arc<BudgetLedger>,
    bytes: u64,
}

impl Drop for LedgerTicket {
    fn drop(&mut self) {
        {
            let mut state = self.ledger.state.lock().unwrap_or_else(|e| e.into_inner());
            state.reserved_bytes = state.reserved_bytes.saturating_sub(self.bytes);
            state.running = state.running.saturating_sub(1);
        }
        self.ledger.released.notify_all();
    }
}

impl BudgetLedger {
    /// A ledger with its own budget cell and probe. Only the tests build
    /// one this way; the worker shares the cell of its `JobBudget`.
    #[cfg(test)]
    pub(crate) fn new(budget: MemoryBudget, probe: Arc<dyn UsageProbe>) -> Arc<Self> {
        Self::sharing(&JobBudget::with_probe(budget, probe, Duration::ZERO))
    }

    /// A ledger that reads the budget cell and the probe of `budget`.
    fn sharing(budget: &JobBudget) -> Arc<Self> {
        Arc::new(Self {
            budget: Arc::clone(&budget.budget),
            probe: Arc::clone(&budget.probe),
            state: Mutex::new(LedgerState::default()),
            released: Condvar::new(),
        })
    }

    /// The bytes the running heavy jobs reserved.
    pub(crate) fn reserved_bytes(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reserved_bytes
    }

    /// Admit a job with `estimate_bytes`, or say why it must wait. See the
    /// wait rule on the type.
    fn admit(self: &Arc<Self>, estimate_bytes: Option<u64>) -> Admission {
        // Read the budget cell BEFORE the state lock: the two locks never
        // nest.
        let limit = self
            .budget
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .budget
            .limit_bytes;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let (Some(limit_bytes), Some(estimate)) = (limit, estimate_bytes)
            && state.running > 0
            && let Some(used) = self.probe.used_bytes()
            && used
                .saturating_add(state.reserved_bytes)
                .saturating_add(estimate)
                > limit_bytes
        {
            return Admission::Wait(LedgerWait {
                estimate_bytes: estimate,
                used_bytes: used,
                reserved_bytes: state.reserved_bytes,
                limit_bytes,
            });
        }
        let bytes = estimate_bytes.unwrap_or(0);
        state.reserved_bytes = state.reserved_bytes.saturating_add(bytes);
        state.running = state.running.saturating_add(1);
        Admission::Run(LedgerTicket {
            ledger: Arc::clone(self),
            bytes,
        })
    }

    /// Wait until a ticket drops, or `timeout` passes. The timeout is the
    /// backstop for a release between the admission and this wait, and for
    /// a reading that falls without a release.
    fn wait_for_release(&self, timeout: Duration) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (_state, _timed_out) = self
            .released
            .wait_timeout(state, timeout)
            .unwrap_or_else(|e| e.into_inner());
    }

    /// Wake every waiting lane, for a shutdown.
    fn wake_all(&self) {
        self.released.notify_all();
    }
}

/// How often a waiting job checks the ledger again when no ticket drops.
/// The core probe rate limit; not a memory number.
const LEDGER_RECHECK: Duration = rs_cam_core::budget::guard::PROBE_INTERVAL;

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

    /// Forget the job that just finished. The lane runs nothing.
    fn go_idle(&mut self) {
        self.state = LaneState::Idle;
        self.current_job = None;
        self.current_phase = None;
        self.started_at = None;
        self.active_toolpath_id = None;
        self.active_toolpath_index = None;
        self.active_cancel = None;
    }
}

/// What a lane records about the request it has just taken off its queue.
///
/// **SHL-06.** The five `spawn_*_lane` loops each wrote these fields
/// inline, a ~20-line block repeated verbatim, and the two lanes that
/// leave `active_toolpath_id` unset explained the omission in prose. It is
/// a field now, so every lane states its answer.
struct RunningJob {
    /// The label the status bar and MCP `generation_status` show.
    label: String,
    /// The toolpath being GENERATED.
    ///
    /// Only the toolpath lane answers `Some`. The job and reach lanes run
    /// work FOR a toolpath without generating it, and MCP
    /// `generation_status` reads this field as the op being generated, so
    /// an id here would report a generation that is not running. Those
    /// lanes carry the id in their label instead.
    active_toolpath_id: Option<ToolpathId>,
    /// The 0-based plan position of that toolpath, so `generation_status`
    /// names the op by the index every other MCP call uses.
    active_toolpath_index: Option<usize>,
    /// The cancel flag of THIS job (§22 ruling 3).
    ///
    /// `None` on a lane whose requests carry no per-job flag; a cancel
    /// there sets the lane flag alone.
    active_cancel: Option<Arc<AtomicBool>>,
    /// The memory the job is estimated to hold, in bytes, for the
    /// [`BudgetLedger`]. `None` means NOT ESTIMATED, and such a job never
    /// waits.
    ///
    /// The analysis lane fills it for a simulation
    /// (`rs_cam_core::budget::estimate::SimulationNeed`). A generation, a
    /// collision check and a project optimize have no sourced estimate yet,
    /// so they stay `None`; they still count as running heavy jobs.
    estimate_bytes: Option<u64>,
}

impl RunningJob {
    /// A job the status surfaces know by its label alone.
    fn labelled(label: String) -> Self {
        Self {
            label,
            active_toolpath_id: None,
            active_toolpath_index: None,
            active_cancel: None,
            estimate_bytes: None,
        }
    }

    /// Name the toolpath this job GENERATES. The toolpath lane alone.
    fn generating(mut self, id: ToolpathId, index: usize) -> Self {
        self.active_toolpath_id = Some(id);
        self.active_toolpath_index = Some(index);
        self
    }

    /// Name the flag a cancel of this job must set.
    fn cancelled_by(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.active_cancel = Some(cancel);
        self
    }

    /// State the memory the job is estimated to hold.
    fn reserving(mut self, estimate_bytes: Option<u64>) -> Self {
        self.estimate_bytes = estimate_bytes;
        self
    }
}

struct LaneQueue<Request> {
    lane: ComputeLane,
    inner: Mutex<LaneInner<Request>>,
    wake: Condvar,
    /// The lane flag. An `Arc` so that a job's [`BudgetGuard`] can own it:
    /// on the analysis and optimize lanes the guard sets THIS flag when the
    /// budget stops the job.
    cancel: Arc<AtomicBool>,
    shutdown: AtomicBool,
    /// The shared ledger of the heavy lanes. `None` on the reach and job
    /// lanes, which run no heavy job.
    ledger: Option<Arc<BudgetLedger>>,
}

impl<Request> LaneQueue<Request> {
    /// Wait for a request, take it, and mark the lane running.
    ///
    /// `None` means the lane is shutting down and the thread must return.
    ///
    /// **SHL-06.** This is the one dequeue prologue. Every `spawn_*_lane`
    /// held its own copy, so a fix or a new `LaneInner` field had to be
    /// applied five times by hand — `active_toolpath_id`, `active_cancel`
    /// and `current_phase` each arrived that way.
    ///
    /// For a lane with a ledger this DROPS the ledger ticket at once. A heavy
    /// lane calls [`Self::dequeue_reserved`] and keeps the ticket.
    fn dequeue_running(&self, describe: impl Fn(&Request) -> RunningJob) -> Option<Request> {
        self.dequeue_reserved(describe)
            .map(|(request, _ticket)| request)
    }

    /// [`Self::dequeue_running`], plus the ledger ticket of the job.
    ///
    /// The job stays at the FRONT of the queue while the ledger tells it to
    /// wait (wait rule: [`BudgetLedger`]). The lane then reads `Queued`, its
    /// current job is the waiting job, and its phase says what the job waits
    /// for. The lane lock is NOT held during the wait, so a submit or a
    /// cancel on the frame loop never blocks on it. After each wake the
    /// prologue reads the queue again, so a submit that replaced the waiting
    /// job is honoured.
    ///
    /// The caller keeps the ticket until the job has finished. Dropping it
    /// gives the reservation back.
    fn dequeue_reserved(
        &self,
        describe: impl Fn(&Request) -> RunningJob,
    ) -> Option<(Request, Option<LedgerTicket>)> {
        loop {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            while inner.queue.is_empty() {
                if self.shutdown.load(Ordering::SeqCst) {
                    return None;
                }
                inner.go_idle();
                inner = self.wake.wait(inner).unwrap_or_else(|e| e.into_inner());
            }
            if self.shutdown.load(Ordering::SeqCst) {
                return None;
            }
            let Some(front) = inner.queue.front() else {
                continue;
            };
            let running = describe(front);
            let ticket = match self.ledger.as_ref() {
                None => None,
                Some(ledger) => match ledger.admit(running.estimate_bytes) {
                    Admission::Run(ticket) => Some(ticket),
                    Admission::Wait(wait) => {
                        tracing::debug!(lane = ?self.lane, ?wait, "memory ledger: the job waits");
                        inner.state = LaneState::Queued;
                        inner.current_job = Some(running.label);
                        inner.current_phase = Some(wait.phase());
                        drop(inner);
                        ledger.wait_for_release(LEDGER_RECHECK);
                        continue;
                    }
                },
            };
            // `front` above proved the queue is not empty, and the lock has
            // been held since.
            // SAFETY: the queue is not empty here.
            #[allow(clippy::expect_used)]
            let request = inner.queue.pop_front().expect("queue checked");
            self.cancel.store(false, Ordering::SeqCst);
            inner.state = LaneState::Running;
            inner.current_job = Some(running.label);
            inner.current_phase = None;
            inner.started_at = Some(Instant::now());
            inner.active_toolpath_id = running.active_toolpath_id;
            inner.active_toolpath_index = running.active_toolpath_index;
            inner.active_cancel = running.active_cancel;
            return Some((request, ticket));
        }
    }

    fn new(lane: ComputeLane) -> Arc<Self> {
        Self::build(lane, None)
    }

    /// A heavy lane: its jobs take tickets from `ledger`.
    fn with_ledger(lane: ComputeLane, ledger: Arc<BudgetLedger>) -> Arc<Self> {
        Self::build(lane, Some(ledger))
    }

    fn build(lane: ComputeLane, ledger: Option<Arc<BudgetLedger>>) -> Arc<Self> {
        Arc::new(Self {
            lane,
            inner: Mutex::new(LaneInner::new()),
            wake: Condvar::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            shutdown: AtomicBool::new(false),
            ledger,
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

impl rs_cam_core::trace::debug_trace::ToolpathPhaseSink for ToolpathPhaseTracker {
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
    /// The memory budget of the heavy lanes (plan B1, B3).
    budget: JobBudget,
    /// The shared ledger of the heavy lanes (plan B4).
    ledger: Arc<BudgetLedger>,
    /// The resident size of the process when the backend started, in
    /// bytes: the "otherwise idle" baseline of the simulation preflight.
    /// `None` when the budget has no limit or the platform gives no
    /// reading. A change from no limit to a limit reads it then
    /// (`set_memory_budget`).
    ///
    /// It is a LOWER bound of what an idle session holds: it does not count
    /// a project that was loaded later. A live reading would also count the
    /// previous simulation, which the new run replaces, and a running
    /// generation, which the ledger waits for; on rivmap350 that double
    /// count alone would refuse a run that fits (BASELINES.md, W1). The
    /// guard stays the net for what the baseline does not count.
    baseline_bytes: Option<u64>,
}

impl ThreadedComputeBackend {
    /// A backend with NO memory limit. Its lanes behave as they did before
    /// the memory budget existed.
    pub fn new() -> Self {
        Self::with_budget(MemoryBudget::UNLIMITED)
    }

    /// A backend whose heavy lanes stop a job over `budget`.
    ///
    /// The app reads the budget from `crate::io::app_settings` at start. A
    /// budget with no limit starts no probe thread, so it is the same as
    /// [`Self::new`].
    pub fn with_budget(budget: MemoryBudget) -> Self {
        Self::with_job_budget(JobBudget::new(budget))
    }

    /// A backend whose heavy lanes stop a job over `budget`, which the
    /// settings value `setting` gave. The app starts with this, so the MCP
    /// `generation_status` names the true source of the limit.
    pub fn with_configured_budget(budget: MemoryBudget, setting: MemoryLimit) -> Self {
        let job_budget = JobBudget::new(budget);
        job_budget.set_configured(budget, setting);
        Self::with_job_budget(job_budget)
    }

    /// The memory budget the heavy lanes enforce.
    pub fn memory_budget(&self) -> MemoryBudget {
        self.budget.budget()
    }

    /// A backend with a given budget AND usage probe. Tests give a fake
    /// probe here.
    pub(crate) fn with_job_budget(budget: JobBudget) -> Self {
        let ledger = BudgetLedger::sharing(&budget);
        let baseline_bytes = if budget.budget().is_limited() {
            budget.probe.used_bytes()
        } else {
            None
        };
        let toolpath_lane = LaneQueue::with_ledger(ComputeLane::Toolpath, Arc::clone(&ledger));
        let analysis_lane = LaneQueue::with_ledger(ComputeLane::Analysis, Arc::clone(&ledger));
        let optimize_lane = LaneQueue::with_ledger(ComputeLane::Optimize, Arc::clone(&ledger));
        let reach_lane = LaneQueue::new(ComputeLane::Reach);
        let job_lane = LaneQueue::new(ComputeLane::Job);
        let (result_tx, result_rx) = mpsc::sync_channel::<ComputeMessage>(64);
        let sim_prefix_cache = Arc::new(Mutex::new(
            rs_cam_core::compute::sim_prefix::SimPrefixCache::new(),
        ));

        // The Reach and Job lanes get no budget guard (memory programme
        // wave 2, group G): a reach walk is seconds of work, and the Job
        // lane's answer type carries no stop reason yet.
        let toolpath_handle = spawn_toolpath_lane(
            Arc::clone(&toolpath_lane),
            result_tx.clone(),
            budget.clone(),
        );
        let analysis_handle = spawn_analysis_lane(
            Arc::clone(&analysis_lane),
            result_tx.clone(),
            Arc::clone(&sim_prefix_cache),
            budget.clone(),
        );
        let optimize_handle = spawn_optimize_lane(
            Arc::clone(&optimize_lane),
            result_tx.clone(),
            budget.clone(),
        );
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
            budget,
            ledger,
            baseline_bytes,
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
        // A heavy lane that waits for the ledger waits on the ledger.
        self.ledger.wake_all();
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
        // The ledger reads the estimate. With no limit the ledger never
        // waits, so the submit does not count.
        let estimate = self.budget.budget().is_limited().then(|| {
            rs_cam_core::budget::estimate::SimulationNeed::of_request(&request.core).bytes()
        });
        self.submit_analysis(AnalysisRequest::Simulation(request, estimate));
    }

    fn memory_budget(&self) -> MemoryBudget {
        self.budget.budget()
    }

    /// File ▸ Preferences ▸ Apply. The ONE cell changes, so:
    ///
    /// - a job that runs keeps the guard it started with;
    /// - a queued job, and every job after it, meets the new limit in the
    ///   ledger and in its new guard;
    /// - a job that waits for the ledger checks again at once.
    ///
    /// The backend reads the idle baseline at start only when the budget
    /// has a limit. A change from no limit to a limit reads it here, else
    /// the simulation preflight could never refuse. That reading is late,
    /// so it can count a loaded project; the guard stays the net.
    fn set_memory_budget(&mut self, budget: MemoryBudget, setting: MemoryLimit) {
        self.budget.set_configured(budget, setting);
        if self.baseline_bytes.is_none() && budget.is_limited() {
            self.baseline_bytes = self.budget.probe.used_bytes();
        }
        self.ledger.wake_all();
    }

    fn memory_reserved_bytes(&self) -> u64 {
        self.ledger.reserved_bytes()
    }

    fn memory_control(&self) -> MemoryControl {
        MemoryControl::new(Arc::new(LedgerMemoryStatus {
            budget: self.budget.clone(),
            ledger: Arc::clone(&self.ledger),
        }))
    }

    fn preflight_simulation(
        &self,
        need: &rs_cam_core::budget::estimate::SimulationNeed,
    ) -> Result<(), rs_cam_core::budget::estimate::PreflightRefusal> {
        // No baseline: no limit, or no reading on this platform. Then there
        // is nothing to judge, and the run goes ahead as it does today.
        let Some(baseline) = self.baseline_bytes else {
            return Ok(());
        };
        rs_cam_core::budget::estimate::preflight_simulation(&self.budget.budget(), baseline, need)
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
        AnalysisRequest::Simulation(request, _) => {
            let count: usize = request.core.groups.iter().map(|g| g.toolpaths.len()).sum();
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
    budget: JobBudget,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // The ticket lives to the end of this iteration: the ledger
            // reservation ends with the job, on every path.
            let Some((request, _ticket)) = lane.dequeue_reserved(|request| {
                RunningJob::labelled(toolpath_job_label(request))
                    .generating(request.viz.toolpath_id, request.handle.index)
                    .cancelled_by(Arc::clone(&request.viz.cancel))
            }) else {
                return;
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
            // The guard owns the flag that `execute_job` polls, which is the
            // job's own flag (§22 ruling 3), not the lane flag.
            let guard = budget.guard(Arc::clone(&request.viz.cancel));
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let phase_tracker = ToolpathPhaseTracker::new(Arc::clone(&lane));
                let watch = guard.watch();
                let mut outcome = execute::run_compute_with_phase(&request, &phase_tracker);
                drop(watch);
                // A cancelled generation reports `Cancelled` whatever the
                // generator said. `execute_job` returns a plain
                // `OperationFailed` when the flag stops it — core carries no
                // cancel variant — so the flag is the evidence, not the
                // message. Widened from the pre-WP11b `&& is_ok()`: the two
                // agree, because the old path already mapped
                // `OperationError::Cancelled` to `ComputeError::Cancelled`.
                //
                // The LANE flag comes first. Only an operator cancel or a
                // resubmit sets it (the guard sets the job flag), and
                // G-REGEN-RACE needs the `Cancelled` of a superseded job.
                // A budget stop with no lane cancel reports `OverBudget`.
                if lane.cancel.load(Ordering::SeqCst) {
                    outcome.result = Err(ComputeError::Cancelled);
                } else if let Some(stop) = over_budget_stop(&guard) {
                    outcome.result = Err(stop);
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
                let msg = rs_cam_core::util::panic_message::panic_payload_message(&*panic_payload);
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
    budget: JobBudget,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // The ticket lives to the end of this iteration: the ledger
            // reservation ends with the job, on every path.
            let Some((request, _ticket)) = lane.dequeue_reserved(|request| {
                let estimate = match request {
                    AnalysisRequest::Simulation(_, estimate) => *estimate,
                    AnalysisRequest::Collision(_) => None,
                };
                RunningJob::labelled(analysis_job_label(request)).reserving(estimate)
            }) else {
                return;
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // The guard owns the lane flag: the simulation and the collision
            // check poll that flag, and a budget stop sets it.
            let guard = budget.guard(Arc::clone(&lane.cancel));

            // Wrap the analysis body in catch_unwind so a panic in simulation
            // or collision checking does not kill the worker thread.  On panic
            // we log the error, reset lane state to Idle, and continue.
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let result = match request {
                    AnalysisRequest::Simulation(request, _) => {
                        let set_phase = |phase: &str| {
                            let mut inner = lane.inner.lock().unwrap_or_else(|e| e.into_inner());
                            inner.current_phase = Some(phase.to_owned());
                        };
                        // S5: the memo is held for the whole simulation, which
                        // is why it lives on the lane rather than in a global.
                        let mut cache = sim_prefix_cache.lock().unwrap_or_else(|e| e.into_inner());
                        let watch = guard.watch();
                        let result = execute::run_simulation_with_phase(
                            &request,
                            guard.flag(),
                            set_phase,
                            Some(rs_cam_core::compute::sim_prefix::SimMemo {
                                store: request.memoize_prefix,
                                cache: &mut cache,
                            }),
                        );
                        drop(watch);
                        drop(cache);
                        // B3: a budget stop wins over every other answer, so
                        // the lane never overwrites it with `Cancelled`.
                        let result = if let Some(stop) = over_budget_stop(&guard) {
                            Err(stop)
                        } else if lane.cancel.load(Ordering::SeqCst) && result.is_ok() {
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
                        let watch = guard.watch();
                        let result = helpers::run_collision_check_with_phase(
                            &request,
                            guard.flag(),
                            set_phase,
                        );
                        drop(watch);
                        let result = if let Some(stop) = over_budget_stop(&guard) {
                            Err(stop)
                        } else if lane.cancel.load(Ordering::SeqCst) && result.is_ok() {
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
                let msg = rs_cam_core::util::panic_message::panic_payload_message(&*panic_payload);
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
    budget: JobBudget,
) -> std::thread::JoinHandle<()> {
    use rs_cam_core::tool_load::optimize::{ProjectOptimizeReport, optimize_project};

    std::thread::spawn(move || {
        loop {
            // The ticket lives to the end of this iteration. This lane has
            // no `catch_unwind`; a panic unwinds the thread and drops the
            // ticket with it, so the ledger does not keep a dead job.
            let Some((request, _ticket)) =
                lane.dequeue_reserved(|request| RunningJob::labelled(optimize_job_label(request)))
            else {
                return;
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
            // The guard owns the lane flag, which the optimizer polls.
            let guard = budget.guard(Arc::clone(&lane.cancel));
            let watch = guard.watch();
            let kind = match request {
                OptimizeRequest::Project {
                    mut session,
                    baseline_trace,
                } => {
                    let report: ProjectOptimizeReport =
                        optimize_project(&mut session, &baseline_trace, &progress, guard.flag());
                    OptimizeResultKind::Project { report }
                }
            };
            drop(watch);
            let budget_stop = match guard.stop_reason() {
                Some(reason @ StopReason::OverBudget { .. }) => Some(reason),
                Some(StopReason::User) | None => None,
            };
            let result = OptimizeResult { kind, budget_stop };

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
            // `generating` is NOT called: a job runs work FOR a toolpath
            // without generating it. See `RunningJob::active_toolpath_id`.
            let Some(mut request) = lane.dequeue_running(|request| {
                RunningJob::labelled(job_label(request)).cancelled_by(Arc::clone(&request.cancel))
            }) else {
                return;
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            let id = request.id;
            let answer = match std::panic::catch_unwind(AssertUnwindSafe(|| run_job(&mut request)))
            {
                Ok(answer) => answer,
                Err(panic_payload) => {
                    let msg =
                        rs_cam_core::util::panic_message::panic_payload_message(&*panic_payload);
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
///   [`rs_cam_core::maps::reach_map::ReachMap::vertex_gaps`] runs one
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
            let Some(request) =
                lane.dequeue_running(|request| RunningJob::labelled(reach_job_label(request)))
            else {
                return;
            };

            if lane.shutdown.load(Ordering::SeqCst) {
                return;
            }

            // Same guard the toolpath and analysis lanes carry: a panic in
            // the walk must not kill the worker or poison the lane mutex.
            let toolpath_id = request.toolpath_id;
            let key = rs_cam_core::maps::reach_map_cache::ReachRequestKey::of(&request.spec);
            let mesh = Arc::clone(&request.spec.mesh);
            let index = Arc::clone(&request.spec.index);
            // `AtomicBool` does not implement `CancelCheck`; the trait has a
            // blanket impl for `Fn() -> bool`, so the lane's flag is read
            // through a closure — the same adapter `session::multitool`
            // builds at its own `cached_tier_map` call.
            let cancel_fn = || lane.cancel.load(Ordering::SeqCst);
            let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let built = match rs_cam_core::maps::reach_map_cache::cached_reach_map(
                    &request.spec,
                    &cancel_fn,
                ) {
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
                    key,
                    result: built,
                    colors: Arc::new(colors),
                })));
            }));

            if let Err(panic_payload) = caught {
                let msg = rs_cam_core::util::panic_message::panic_payload_message(&*panic_payload);
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
