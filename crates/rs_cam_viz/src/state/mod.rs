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
    /// Job-readiness dashboard — "is this safe to cut?" in one place (W3.8).
    Readiness,
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
    /// Show the multi-step Export Wizard. Persistent settings live on
    /// `session.wizard()`; this flag and `wizard_active_step` are
    /// transient UI state.
    pub show_export_wizard: bool,
    /// 0-indexed currently visible wizard step. Initialised from
    /// `session.wizard().last_step_visited` when the wizard opens so
    /// the user resumes where they left off.
    pub wizard_active_step: u8,
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
    /// Set after the user clicks Apply selected on the Optimize-project
    /// rollup. Holds the toolpath ids that need to finish regenerating
    /// before we kick the reconciliation sim. Empty otherwise.
    pub pending_reconciliation_for_ids: Vec<rs_cam_core::ToolpathId>,
    /// Toolpath id of a per-TP Optimize candidate just applied via
    /// `apply_optimize_candidate`. When the corresponding regen lands,
    /// the drain handler auto-triggers a full project sim so the
    /// freshly-applied params are verified against live simulation
    /// without the user having to click Run Simulation manually
    /// (Roadmap F.2). `None` when no per-TP Apply is in flight.
    pub pending_apply_resim: Option<usize>,
    /// Cached state of the redesigned Feeds & Speeds modal. `None`
    /// when closed. Built on open by `explain_for_operation`; the
    /// modal re-derives `FeedsExplain` every frame from session state
    /// so live param edits are reflected without an extra refresh
    /// path.
    pub feeds_modal: Option<FeedsModalState>,
    /// Cached state of the Tool Library management modal. `None` when
    /// closed. Holds a snapshot of every catalog loaded on open; the
    /// controller refreshes the snapshot after any mutation so the
    /// modal can render without per-frame disk I/O.
    pub tool_library_modal: Option<ToolLibraryModalState>,
}

/// Persistent state for the Tool Library modal. The `catalogs` snapshot
/// is loaded by the controller when the modal opens and re-loaded after
/// every mutating action; the modal renders entirely from it. Ephemeral
/// view state (selection, filter, edit drafts) lives in egui temp memory.
#[derive(Debug, Clone)]
pub struct ToolLibraryModalState {
    pub catalogs: Vec<(String, rs_cam_core::tool_library::ToolCatalog)>,
}

/// Persistent state for the Feeds & Speeds modal. Carries the focused
/// toolpath plus per-modal UI state (active mode, hover state, "what
/// if" drag, etc.). The underlying recommendation + chart data is
/// derived fresh from the session each frame.
#[derive(Debug, Clone)]
pub struct FeedsModalState {
    pub toolpath_id: rs_cam_core::ToolpathId,
    pub mode: FeedsModalMode,
    /// Phase 3 — drag-to-explore on the feed-RPM nomogram. `Some` while
    /// the user is dragging the operating point.
    pub explore: Option<NomogramExplore>,
    /// Phase 2 — "How is this calculated?" disclosure expanded.
    pub show_provenance: bool,
    /// Phase 4 — sortable column for the All-toolpaths table.
    pub project_sort: ProjectFeedsSort,
    /// Phase 4 — set of toolpath IDs whose row checkbox is currently
    /// ticked. Empty == nothing selected (Apply selected disabled).
    /// Defaults to every enabled toolpath when project view is opened.
    pub project_selected: std::collections::BTreeSet<rs_cam_core::ToolpathId>,
    /// Phase 4 — toggle for the project-view scatter overlay.
    pub project_show_scatter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsModalMode {
    /// Single-toolpath view: comparison card + three charts.
    Toolpath,
    /// Project rollup: one row per toolpath, sortable.
    Project,
}

/// Local UI state for the drag-to-explore interaction on Chart C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NomogramExplore {
    /// Currently dragged (rpm, feed) operating point.
    pub rpm: f64,
    pub feed_mm_min: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFeedsSort {
    Index,
    Speedup,
    Name,
}

/// Persistent state for the per-toolpath Optimize modal. Carries the
/// toolpath being optimized plus the cached outcome (or its Loading /
/// Failed states for the worker-thread integration that lands in U3).
#[derive(Debug, Clone)]
pub struct OptimizeModalState {
    pub toolpath_id: rs_cam_core::ToolpathId,
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
    /// User clicked Apply selected; the project sim is now running
    /// end-to-end with the applied params. Rollup stays visible with
    /// the candidate-isolated values dimmed; reconciled column lights
    /// up when the sim completes.
    Reconciling(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
    /// Project sim completed; per-row `reconciled_cycle_time_s` /
    /// `reconciled_verdict` are populated. Rows whose reconciled
    /// values disagree with candidate-isolated values are flagged.
    Reconciled(rs_cam_core::tool_load::optimize::ProjectOptimizeReport),
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
            show_export_wizard: false,
            wizard_active_step: 0,
            optimize_modal: None,
            optimize_project: None,
            is_optimizing: false,
            pending_reconciliation_for_ids: Vec::new(),
            pending_apply_resim: None,
            feeds_modal: None,
            tool_library_modal: None,
        }
    }

    /// Modal exclusivity (density pass Batch 2): every modal-open path
    /// calls this first, so opening one modal closes the others — the
    /// 2026-06-11 capture sweep produced a 3-deep stack (Optimize
    /// spinner over Tool Library over a feeds modal). A *running*
    /// Optimize modal survives: it is a progress surface for an
    /// expensive in-flight search, and closing it here would discard
    /// the user's run; settled Optimize outcomes close like the rest.
    pub fn close_modals_for_exclusivity(&mut self) {
        self.feeds_modal = None;
        self.tool_library_modal = None;
        self.show_export_wizard = false;
        self.show_preflight = false;
        self.show_shortcuts = false;
        if !self.is_optimizing {
            self.optimize_modal = None;
            self.optimize_project = None;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn feeds_modal_fixture() -> FeedsModalState {
        FeedsModalState {
            toolpath_id: rs_cam_core::ToolpathId(0),
            mode: FeedsModalMode::Toolpath,
            explore: None,
            show_provenance: false,
            project_sort: ProjectFeedsSort::Index,
            project_selected: std::collections::BTreeSet::new(),
            project_show_scatter: false,
        }
    }

    /// Density pass Batch 2 — opening any modal closes the others. The
    /// 2026-06-11 capture sweep stacked Optimize over Tool Library over
    /// a feeds modal; exclusivity makes that state unrepresentable.
    #[test]
    fn close_modals_for_exclusivity_closes_settled_modals() {
        let mut state = AppState::new();
        state.feeds_modal = Some(feeds_modal_fixture());
        state.tool_library_modal = Some(ToolLibraryModalState { catalogs: vec![] });
        state.show_export_wizard = true;
        state.show_preflight = true;
        state.optimize_project = Some(OptimizeProjectState {
            status: OptimizeProjectStatus::Loading,
            row_selected: Vec::new(),
        });

        state.close_modals_for_exclusivity();

        assert!(state.feeds_modal.is_none());
        assert!(state.tool_library_modal.is_none());
        assert!(!state.show_export_wizard);
        assert!(!state.show_preflight);
        assert!(state.optimize_project.is_none(), "settled optimize closes");
    }

    /// A *running* Optimize modal must survive exclusivity — closing it
    /// would silently discard an expensive in-flight search.
    #[test]
    fn close_modals_for_exclusivity_spares_running_optimize() {
        let mut state = AppState::new();
        state.is_optimizing = true;
        state.optimize_modal = Some(OptimizeModalState {
            toolpath_id: rs_cam_core::ToolpathId(0),
            status: OptimizeRunStatus::Loading,
        });
        state.feeds_modal = Some(feeds_modal_fixture());

        state.close_modals_for_exclusivity();

        assert!(state.optimize_modal.is_some(), "running optimize survives");
        assert!(state.feeds_modal.is_none(), "others still close");
    }
}
