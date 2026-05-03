pub mod history;
pub mod job;
pub mod runtime;
pub mod selection;
pub mod simulation;
pub mod toolpath;
pub mod viewport;

use history::UndoHistory;
use runtime::GuiState;
use selection::Selection;
use simulation::SimulationState;
use viewport::ViewportState;

/// Which top-level workspace the user is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    /// Setup definition: stock orientation, datum, workholding, fixtures.
    Setup,
    /// Toolpath authoring — parameters, feeds, strategies.
    Toolpaths,
    /// Verification — material removal, collisions, cycle time, safety.
    Simulation,
}

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub workspace: Workspace,
    /// Unified project session — single source of truth for CAM data.
    pub session: rs_cam_core::session::ProjectSession,
    /// GUI-only runtime overlay (dirty flag, per-toolpath display state, datum config).
    pub gui: GuiState,
    pub selection: Selection,
    pub viewport: ViewportState,
    pub simulation: SimulationState,
    pub history: UndoHistory,
    /// Show pre-flight checklist modal before export.
    pub show_preflight: bool,
    /// Show keyboard shortcuts reference window.
    pub show_shortcuts: bool,
    /// Cached state of the per-toolpath Optimize modal. `None` when
    /// closed. The optimizer is expensive (~1-2 min per toolpath at
    /// the Stage 0/1/2 settings), so unlike the suggest modal we
    /// cannot recompute every frame — the outcome is captured here on
    /// open and rendered every frame from this cache.
    pub optimize_modal: Option<OptimizeModalState>,
    /// Cached project-level Optimize rollup (U3). `None` until the
    /// user clicks the toolbar Optimize-project button. While the
    /// worker is running, status is `Loading`; once the result lands,
    /// the rollup view renders the report. Mirrors the per-toolpath
    /// modal's lifecycle so the UI shapes stay consistent.
    pub optimize_project: Option<OptimizeProjectState>,
    /// `true` while the Optimize worker thread holds the session.
    /// During this window the main thread renders an empty placeholder
    /// session — every panel that reads `state.session` should check
    /// this flag and short-circuit to a "Optimize running…" view, with
    /// the modal as the only interactive surface.
    pub is_optimizing: bool,
}

/// Persistent state for the per-toolpath Optimize modal. Carries the
/// toolpath being optimized plus the cached outcome (or its Loading /
/// Failed states for the worker-thread integration that lands in U3).
#[derive(Debug, Clone)]
pub struct OptimizeModalState {
    pub toolpath_id: usize,
    pub status: OptimizeRunStatus,
}

/// Lifecycle of one Optimize run as the modal sees it. The lane
/// driver moves `Loading -> Ready` (or `Failed`) as the worker
/// completes; the modal renders the right view per status.
#[derive(Debug, Clone)]
pub enum OptimizeRunStatus {
    /// Optimizer is running on a worker thread. The modal shows a
    /// progress strip and a Cancel button.
    Loading,
    /// Optimizer finished. The outcome is the source of truth for
    /// every row in the modal's candidate table.
    Ready(rs_cam_core::tool_load::optimize::OptimizeOutcome),
    /// Optimizer errored out. String is the diagnostic for the user.
    Failed(String),
}

/// Persistent state for the project-level Optimize rollup (U3).
/// Mirrors `OptimizeModalState`'s lifecycle but without a single
/// toolpath_id — the rollup spans every enabled toolpath.
#[derive(Debug, Clone)]
pub struct OptimizeProjectState {
    pub status: OptimizeProjectStatus,
    /// Per-row checkbox state for batch Apply. Index aligned with
    /// `ProjectOptimizeReport::per_toolpath`. Defaults to true on the
    /// rows whose outcome has a recommended candidate; the user can
    /// flip individual rows before clicking Apply selected.
    pub row_selected: Vec<bool>,
}

#[derive(Debug, Clone)]
pub enum OptimizeProjectStatus {
    /// Worker is running. Rollup view shows progress + cancel.
    Loading,
    /// Worker finished. Render the rollup with bottleneck callout
    /// and the per-toolpath rows.
    Ready(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
    /// Worker failed. String is the diagnostic for the user.
    Failed(String),
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: Workspace::Toolpaths,
            session: rs_cam_core::session::ProjectSession::new_empty(),
            gui: GuiState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
            simulation: SimulationState::new(),
            history: UndoHistory::new(),
            show_preflight: false,
            show_shortcuts: false,
            optimize_modal: None,
            optimize_project: None,
            is_optimizing: false,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
