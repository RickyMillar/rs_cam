//! One freshness state per toolpath, derived — never stored.
//!
//! Implements `planning/ui_fix_2026-09-09/research/R0.1.md` §4 option A.
//! Before this module three stores each held a piece of the answer and
//! disagreed: the core result cache (`ProjectSession::results`), a GUI
//! timestamp (`ToolpathRuntime::stale_since`) and a project-wide edit
//! counter. The core cache is now the truth about result validity; the
//! GUI adds only what the core cannot know — that the operator is looking
//! at drawable geometry from a previous generation.
//!
//! `stale_since` keeps its remaining job, the 500 ms auto-regeneration
//! debounce clock, and stops being a claim about correctness.

use crate::state::toolpath::ToolpathId;
use rs_cam_core::compute::config::{AwaitingPriorStock, ComputeStatus};
use rs_cam_core::session::{ProjectSession, ToolpathConfig};

use super::runtime::{GuiState, ToolpathRuntime};

/// Where a toolpath stands, as every surface should read it.
///
/// Derived on every read from the core result cache plus the compute
/// lane's own status. Never stored and never persisted, so it cannot
/// drift from the thing it describes.
#[derive(Debug, Clone, PartialEq)]
pub enum FreshnessState {
    /// The core holds a result generated from the current inputs.
    Current,
    /// An input changed after the last generation. The GUI's own
    /// `ToolpathRuntime::result` still holds the old geometry so the
    /// viewport can draw it; the core holds nothing.
    EditedSince,
    /// The compute lane owns it right now.
    Regenerating,
    /// Blocked on upstream simulated stock (A/M11) — sequencing, not
    /// failure.
    WaitingOnUpstream(AwaitingPriorStock),
    /// Generation failed.
    Error(String),
    /// Never generated, or cancelled / forgotten.
    NoResult,
    /// `enabled == false`. Wins over everything else.
    Disabled,
}

impl FreshnessState {
    /// Stable machine-readable label, for the MCP wire and for tests.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::EditedSince => "edited_since",
            Self::Regenerating => "regenerating",
            Self::WaitingOnUpstream(_) => "awaiting_prior_stock",
            Self::Error(_) => "error",
            Self::NoResult => "no_result",
            Self::Disabled => "disabled",
        }
    }

    /// Whether a consumer may treat this toolpath's geometry as the
    /// answer for the configuration on screen. Only [`Self::Current`] may.
    ///
    /// `EditedSince` is deliberately NOT current: the geometry the GUI
    /// still draws was generated from inputs the project no longer has.
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current)
    }
}

/// Derive the state for one toolpath.
///
/// `core_has_result` is `session.get_result(index).is_some()` — the core
/// cache is the truth about result validity, and every mutating path in
/// `ProjectSession` drops the entry through `drop_result`.
///
/// The core alone cannot tell "never generated" from "had a result, then
/// edited": both are an absent entry. The GUI supplies that distinction
/// from its retained `ToolpathRuntime::result`, which is a display matter
/// and stays a display matter.
pub fn freshness(
    tc: &ToolpathConfig,
    rt: Option<&ToolpathRuntime>,
    core_has_result: bool,
) -> FreshnessState {
    if !tc.enabled {
        return FreshnessState::Disabled;
    }
    match rt.map(|r| &r.status) {
        Some(ComputeStatus::Computing) => FreshnessState::Regenerating,
        Some(ComputeStatus::AwaitingPriorStock(blocked)) => {
            FreshnessState::WaitingOnUpstream(blocked.clone())
        }
        Some(ComputeStatus::Error(message)) => FreshnessState::Error(message.clone()),
        _ if core_has_result => FreshnessState::Current,
        _ if rt.is_some_and(|r| r.result.is_some()) => FreshnessState::EditedSince,
        _ => FreshnessState::NoResult,
    }
}

/// Derive the state for the toolpath at `index`, or `None` when no
/// toolpath has that index.
pub fn freshness_at(
    session: &ProjectSession,
    gui: &GuiState,
    index: usize,
) -> Option<FreshnessState> {
    let tc = session.get_toolpath_config(index)?;
    let rt = gui.toolpath_rt.get(&tc.id);
    Some(freshness(tc, rt, session.get_result(index).is_some()))
}

/// Where the SIMULATION stands, as every surface should read it.
///
/// The toolpath sibling of [`FreshnessState`], and derived the same way:
/// computed on every read, never stored, so it cannot drift from the thing
/// it describes. The core answers for every project input, because
/// `ProjectSession::drop_simulation` is the one site that clears the field.
/// The GUI answers only for its own view: a release, a run in flight, and
/// the result it draws.
///
/// Before this enum the answer lived in `SimulationState::is_stale`, a
/// comparison of the project-wide edit counter against a stamp. That
/// counter moves for edits the core keeps a simulation through (an export
/// wizard field, a machine saved to the library) and stays still for edits
/// the core clears it on, so the two stores disagreed (G-FRESHNESSDISAGREE).
///
/// [`Self::EditedSince`] is RARE on a GUI path. Three controller doors
/// mirror `Effects::simulation_cleared` into
/// `AppController::invalidate_simulation`, which wipes `last_run` as well
/// as the results, so a mirrored GUI edit reads [`Self::NoRun`]. The
/// `EditedSince` arm is reached where no door mirrors: the MCP surface,
/// Optimize Apply, a panel edit the frame loop has not discharged yet, and
/// a late run the core refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimFreshness {
    /// No run has landed, or an edit cleared both the core and the view.
    NoRun,
    /// A run is in flight, and the view released no previous run for it.
    /// The submit door stamped `SimulationState::submitted_simulation_epoch`
    /// and no result has consumed it yet.
    Running,
    /// The view released the previous run for a new run (M1, memory
    /// programme 2026-10-01; `SimulationState::release_for_new_run`).
    ///
    /// The view holds no result, so the viewport shows no simulated stock.
    /// The core still holds the previous run: rest generation reads its
    /// prior stocks, and export reads its trace. `last_run` still names
    /// that run. `in_flight` is `true` while the new run works, and `false`
    /// after the new run was cancelled or failed.
    Released {
        /// `true` while the run that caused the release is in flight.
        in_flight: bool,
    },
    /// The core holds the simulation these inputs produce.
    Current,
    /// The core dropped the simulation; the view still draws the trace.
    EditedSince,
    /// An enabled operation holds no core result (G-STALECARDS). An edit
    /// dropped it and nothing regenerated it, so no simulation can carve it:
    /// a re-run does not help, a regenerate of this operation does. The id
    /// is the first such operation in plan order.
    Ungenerated(ToolpathId),
}

impl SimFreshness {
    /// Stable machine-readable label, for the MCP wire and for tests.
    pub fn label(self) -> &'static str {
        match self {
            Self::NoRun => "no_run",
            Self::Running => "running",
            Self::Released { in_flight: true } => "released_running",
            Self::Released { in_flight: false } => "released",
            Self::Current => "current",
            Self::EditedSince => "edited_since",
            Self::Ungenerated(_) => "operation_not_generated",
        }
    }

    /// Every arm a reader must not present as current evidence.
    ///
    /// [`Self::NoRun`] and [`Self::Running`] are false here: neither one
    /// holds a result to mislabel. A reader that wants "there is nothing
    /// to show" asks for the arm, not for this predicate.
    ///
    /// [`Self::Released`] is true. The view shows nothing, but the core
    /// still holds the previous run and other surfaces read it. That run
    /// is not the answer the operator asked for when the new run started.
    pub fn is_stale(self) -> bool {
        matches!(
            self,
            Self::Released { .. } | Self::EditedSince | Self::Ungenerated(_)
        )
    }

    /// True while a simulation run is in flight: [`Self::Running`], and
    /// [`Self::Released`] with `in_flight`. A "computing" indicator reads
    /// this, not `== Running`, because a re-run reads `Released`.
    pub fn is_in_flight(self) -> bool {
        matches!(self, Self::Running | Self::Released { in_flight: true })
    }
}

/// Derive the simulation's state from the core and the view together.
///
/// The order of the arms is the order of the questions.
/// 1. A release. The submit door releases the view of the previous run
///    before the new run starts, and keeps `last_run` and the core
///    simulation. So "the view holds no result, `last_run` names a run, and
///    the core holds a simulation" is a release, with the run in flight or
///    not. `invalidate_simulation` (an edit, or Reset) clears `last_run`,
///    so it never reads as a release.
/// 2. A run in flight with no release: the first run of the project, or a
///    run after Reset.
/// 3. The view: with nothing to draw there is no claim to qualify.
/// 4. The core, which owns every project input. The GUI holds no capture
///    option of its own: every run captures the cut trace (operator ruling
///    2026-10-02, "always capture").
pub fn simulation_freshness(
    session: &ProjectSession,
    sim: &crate::state::simulation::SimulationState,
) -> SimFreshness {
    let in_flight = sim.submitted_simulation_epoch.is_some();
    if !sim.has_results() && sim.last_run.is_some() && session.simulation_result().is_some() {
        return SimFreshness::Released { in_flight };
    }
    if in_flight {
        return SimFreshness::Running;
    }
    if !sim.has_results() {
        return SimFreshness::NoRun;
    }
    if session.simulation_result().is_none() {
        return SimFreshness::EditedSince;
    }
    // G-STALECARDS: the builder carves core results only, so an enabled
    // operation without one is missing from the run. The core's per-row
    // evidence calls that row stale, and so does this.
    if let Some(ungenerated) = first_ungenerated(session) {
        return SimFreshness::Ungenerated(ungenerated);
    }
    SimFreshness::Current
}

/// The first enabled operation, in plan order, with no core result.
fn first_ungenerated(session: &ProjectSession) -> Option<ToolpathId> {
    session.list_setups().iter().find_map(|setup| {
        setup.toolpath_indices.iter().find_map(|&index| {
            let tc = session.get_toolpath_config(index)?;
            (tc.enabled && session.get_result(index).is_none()).then_some(tc.id)
        })
    })
}
