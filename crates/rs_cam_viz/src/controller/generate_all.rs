//! The generation plan: the state and the pure decisions every surface that
//! makes a scope current shares.
//!
//! W1 of `planning/gen_sim_rest_ux_2026-09-18/`. The round-based fixpoint is
//! gone. The ORDER now lives in core, at
//! [`rs_cam_core::session::generation_plan::plan`], because the GUI, the MCP
//! server and the CLI all need the same answer to "what makes this current".
//! What lives here is the async state machine's data: the step list the
//! driver walks, the outcome it records per step, the progress a button
//! reads, and where the finished summary is delivered.
//!
//! The driver itself is `controller::events::compute`
//! (`start_plan`, `pump_plan`, `record_step_outcome`, `finish_plan`).

use std::collections::HashSet;

use serde::Serialize;

use rs_cam_core::compute::config::AwaitingPriorStock;
use rs_cam_core::ids::SetupId;
use rs_cam_core::session::ToolpathConfig;

use crate::state::toolpath::{StockSource, ToolpathId};

/// One row of an `awaiting_prior_stock` ARRAY: the waiting operation, and
/// the block it carries.
///
/// W5 item (a). The object sites report the BLOCKER alone and serialise
/// [`AwaitingPriorStock`] directly. An array site has to name the waiting
/// operation too, so it flattens the same struct under the three identifying
/// keys. One type, so the two subjects cannot drift apart.
///
/// It lives here rather than in `mcp_bridge`, which is `#[cfg(feature =
/// "mcp")]`, because [`GenerationPlan`] carries these rows and compiles
/// without the feature. `mcp_bridge` re-exports it, so every `use
/// crate::mcp_bridge::BlockedRow` still names this one type.
///
/// Pinned by `tests/awaiting_prior_stock_has_one_shape_d4.rs`.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedRow {
    pub toolpath_id: ToolpathId,
    pub toolpath_index: usize,
    pub name: String,
    #[serde(flatten)]
    pub block: AwaitingPriorStock,
}

/// Which ops one `generate_all` covers, and which of them force a simulation.
///
/// Stays `pub`: it is the return type of `pub fn generate_all_scope`, so a
/// crate-private form raises `private_interfaces` (S29, 2026-09-16).
pub struct GenerateAllScope {
    /// Every enabled toolpath, in project order.
    pub enabled: Vec<ToolpathId>,
    /// 0-based project indices of the enabled ops whose stock comes from a
    /// simulation.
    pub rest_op_indices: Vec<usize>,
}

/// Read the plan's inputs off a project.
#[must_use]
pub fn generate_all_scope(configs: &[ToolpathConfig]) -> GenerateAllScope {
    GenerateAllScope {
        enabled: configs
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.id)
            .collect(),
        rest_op_indices: configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| tc.enabled && tc.stock_source == StockSource::FromRemainingStock)
            .map(|(idx, _)| idx)
            .collect(),
    }
}

/// The simulation cell a plan ran at, as the MCP reply states it
/// (G-RESTRES).
///
/// Every simulation of the project runs at the ONE stored value. An MCP
/// caller that passed `simulation_resolution_mm` SET that value (operator
/// ruling 2026-09-24, Q3), and `set_by_this_call` says so.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResolutionReport {
    /// The cell every simulation of the plan used, in mm.
    pub mm: f64,
    /// `"auto"` or `"fixed"`: the stored mode.
    pub mode: &'static str,
    /// This call wrote the project value.
    pub set_by_this_call: bool,
}

impl ResolutionReport {
    /// The report for the session as it stands.
    #[must_use]
    pub fn of(session: &rs_cam_core::session::ProjectSession, set_by_this_call: bool) -> Self {
        Self {
            mm: session.simulation_resolution_mm(),
            mode: match session.simulation_resolution() {
                rs_cam_core::session::SimulationResolution::Auto => "auto",
                rs_cam_core::session::SimulationResolution::Fixed(_) => "fixed",
            },
            set_by_this_call,
        }
    }
}

/// Where a finished plan reports to.
///
/// The MCP variant is gated because `mcp_bridge` is: everything else in this
/// module compiles either way, which is the point of re-homing it.
pub enum GenerateAllSink {
    /// GUI Generate All — the summary as a toast.
    Gui,
    #[cfg(feature = "mcp")]
    Mcp {
        response_tx: tokio::sync::oneshot::Sender<crate::mcp_bridge::McpResponse>,
        /// Optional channel for streaming per-step progress to the client.
        progress_tx: Option<tokio::sync::mpsc::Sender<crate::mcp_bridge::ProgressUpdate>>,
    },
}

impl GenerateAllSink {
    #[cfg(feature = "mcp")]
    #[must_use]
    pub fn progress_tx(
        &self,
    ) -> Option<&tokio::sync::mpsc::Sender<crate::mcp_bridge::ProgressUpdate>> {
        match self {
            Self::Gui => None,
            Self::Mcp { progress_tx, .. } => progress_tx.as_ref(),
        }
    }
}

/// One piece of work the driver submits, in plan order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStep {
    /// Generate one operation.
    Generate(ToolpathId),
    /// Simulate setups `0..=setup` so `upto` can read its prior stock.
    ///
    /// `setup` is the setup's STABLE id. The driver resolves it to a
    /// position and covers EVERY enabled operation in `0..=position`; `upto`
    /// is a label, and narrowing the request to it shifts the phantom scan
    /// (core's `generation_plan` module doc).
    SimulatePrefix { setup: SetupId, upto: ToolpathId },
    /// The closing full-project simulation, so the Simulation workspace
    /// lands fresh. Every plan that simulates appends it, at the stored
    /// project resolution (G-RESTRES).
    SimulateAll,
}

/// What one step did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// Not reached, or still in flight.
    Pending,
    Done,
    /// Still no stock AFTER its own prefix simulation. Terminal for this op.
    Blocked(String),
    Failed(String),
    Cancelled,
    /// Nothing to submit. Not an error.
    Skipped(String),
}

/// How the plan's own simulation step ended.
///
/// The three arms are the three the simulation drain can reach. A cancelled
/// or failed simulation stops the plan, because every later step in that
/// setup needs the snapshot that never came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanSimOutcome {
    Done,
    Cancelled,
    Failed(String),
}

/// What the step in flight is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    /// Generating this operation. The string is its name, read at read time,
    /// so a rename mid-plan reads correctly.
    Generating(ToolpathId, String),
    /// Simulating. The string is the label to draw.
    ///
    /// A [`PlanStep::SimulateAll`] reports the LAST setup's id, because that
    /// is the last setup it covers, with the label "All setups".
    Simulating(SetupId, String),
}

/// The plan's position, for the Generate All button (W3) and the MCP plan
/// beat (W5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationPlanProgress {
    /// 1-based position of the step in flight.
    pub step: usize,
    pub of: usize,
    pub activity: Activity,
    pub cancellable: bool,
}

impl GenerationPlanProgress {
    /// Is the step in flight a simulation?
    #[must_use]
    pub fn is_simulating(&self) -> bool {
        matches!(self.activity, Activity::Simulating(..))
    }
}

/// State for one in-flight plan, whichever surface started it.
pub struct GenerationPlan {
    pub steps: Vec<PlanStep>,
    pub outcomes: Vec<StepOutcome>,
    /// The step in flight. `== steps.len()` means finished.
    pub cursor: usize,
    /// A submit is out and the drain owes this plan an outcome.
    pub in_flight: bool,
    /// Re-entrancy guard for a submit that completes inside itself. Every
    /// submit-time refusal funnels through `toolpath_completion_landed`,
    /// which re-enters the driver while `pump_plan` is on the stack.
    pub(crate) pumping: bool,
    pub cancelled: bool,
    /// The cell the plan simulates at, for the reply. `None` on a GUI plan.
    pub resolution_report: Option<ResolutionReport>,
    pub loop_error: Option<String>,
    pub sink: GenerateAllSink,
    /// `GuiState::edit_counter` at `start_plan`. An operator edit moves it
    /// and cancels the plan; the plan's own completions never do.
    pub edit_counter: u64,
    /// The operation a single Generate names (R6). It regenerates even when
    /// it already holds a result; its ancestors do not.
    pub target: Option<ToolpathId>,
    pub generated: usize,
    pub failed: usize,
    /// `(toolpath id, message)` for genuine failures.
    pub errors: Vec<(usize, String)>,
    /// Ops still waiting on upstream simulated stock. Deliberately NOT
    /// counted in `failed`: "cannot yet" and "cannot ever" are different
    /// states.
    pub blocked: Vec<BlockedRow>,
    /// Simulations this plan ran on the caller's behalf.
    pub simulations: usize,
    /// Setups holding an operation that blocked or failed. A later prefix
    /// simulation of one of them cannot unlock anything: the phantom scan
    /// latches at the first enabled operation with no result, which is that
    /// one. Those steps are skipped, not run.
    pub(crate) blocked_setups: HashSet<SetupId>,
}

impl GenerationPlan {
    /// Arm a plan that has submitted nothing yet.
    #[must_use]
    pub fn new(steps: Vec<PlanStep>, sink: GenerateAllSink) -> Self {
        let outcomes = vec![StepOutcome::Pending; steps.len()];
        Self {
            steps,
            outcomes,
            cursor: 0,
            in_flight: false,
            pumping: false,
            cancelled: false,
            resolution_report: None,
            loop_error: None,
            sink,
            edit_counter: 0,
            target: None,
            generated: 0,
            failed: 0,
            errors: Vec::new(),
            blocked: Vec::new(),
            simulations: 0,
            blocked_setups: HashSet::new(),
        }
    }

    /// The operation this plan regenerates even when it is already current.
    #[must_use]
    pub fn with_target(mut self, target: ToolpathId) -> Self {
        self.target = Some(target);
        self
    }

    /// The step in flight, or `None` when the plan has finished.
    #[must_use]
    pub fn current_step(&self) -> Option<&PlanStep> {
        self.steps.get(self.cursor)
    }

    /// The operation the cursor's `Generate` step names, if it is one.
    ///
    /// This is the one rule that decides both "does this completion close a
    /// step" and "does this blocked submit belong to the plan".
    #[must_use]
    pub fn generate_target(&self) -> Option<ToolpathId> {
        match self.current_step() {
            Some(PlanStep::Generate(id)) => Some(*id),
            _ => None,
        }
    }

    /// Is the cursor on a simulation step?
    #[must_use]
    pub fn on_simulation(&self) -> bool {
        matches!(
            self.current_step(),
            Some(PlanStep::SimulatePrefix { .. } | PlanStep::SimulateAll)
        )
    }

    /// Did any step generate an operation?
    ///
    /// The closing full simulation runs only when something moved; a plan
    /// whose every operation was already current changes no stock.
    #[must_use]
    pub fn generated_anything(&self) -> bool {
        self.generated > 0
    }

    /// Freeze this run into the shape the renderers consume.
    #[must_use]
    pub fn completed_summary(&self) -> GenerateAllSummary {
        GenerateAllSummary {
            generated: self.generated,
            failed: self.failed,
            errors: self.errors.clone(),
            blocked: self.blocked.clone(),
            steps: self.steps.len(),
            simulations: self.simulations,
            resolution_report: self.resolution_report.clone(),
            loop_error: self.loop_error.clone(),
        }
    }
}

/// The outcome of one plan, ready to render.
pub struct GenerateAllSummary {
    pub generated: usize,
    pub failed: usize,
    /// `(toolpath id, message)` for genuine failures.
    pub errors: Vec<(usize, String)>,
    /// Ops still waiting on upstream simulated stock, one [`BlockedRow`]
    /// each. Separate from `errors` on purpose: an agent must be able to
    /// tell "cannot yet" from "cannot ever" without parsing prose.
    pub blocked: Vec<BlockedRow>,
    /// How many steps the plan held. A round no longer exists.
    pub steps: usize,
    /// How many simulations the plan ran on the caller's behalf.
    pub simulations: usize,
    /// The cell the plan simulated at, and whether this call set it.
    pub resolution_report: Option<ResolutionReport>,
    /// The plan itself stopped (not an individual toolpath).
    pub loop_error: Option<String>,
}

/// The one-line account both surfaces give of a finished run.
#[must_use]
pub fn generate_all_headline(summary: &GenerateAllSummary) -> String {
    let mut headline = format!("Generated {} toolpaths", summary.generated);
    if summary.failed > 0 {
        headline.push_str(&format!(", {} failed", summary.failed));
    }
    if !summary.blocked.is_empty() {
        headline.push_str(&format!(
            ", {} still waiting on upstream simulated stock",
            summary.blocked.len()
        ));
    }
    headline.push_str(&format!(
        " (in {} step{}, {} simulation{})",
        summary.steps,
        if summary.steps == 1 { "" } else { "s" },
        summary.simulations,
        if summary.simulations == 1 { "" } else { "s" },
    ));
    if let Some(err) = &summary.loop_error {
        headline.push_str(&format!(". The plan stopped early: {err}"));
    }
    headline
}
