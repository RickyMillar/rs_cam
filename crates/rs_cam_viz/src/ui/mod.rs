#![deny(clippy::indexing_slicing)]

pub mod automation;
pub mod components;
pub mod export_wizard;
pub mod feeds_modal;
pub mod menu_bar;
pub mod optimize_modal;
pub mod optimize_project;
pub mod preflight;
pub mod properties;
pub mod readiness;
pub mod readiness_panel;
pub mod setup_panel;
pub mod shortcuts_window;
pub mod sim_debug;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod sim_timeline;
pub mod status_bar;
pub mod theme;
pub mod tool_library_modal;
pub mod toolpath_panel;
pub mod toolpath_row_controls;
pub mod viewport_overlay;
pub mod workspace_bar;

use crate::render::camera::ViewPreset;
use crate::state::Workspace;
use crate::state::job::{
    FaceUp, FixtureId, KeepOutId, ModelId, SetupId, ToolConfig, ToolId, ToolType,
};
use crate::state::toolpath::{OperationType, ToolpathId};
use rs_cam_core::enriched_mesh::FaceGroupId;
use std::path::PathBuf;

/// Recommended-value field that the Feeds modal's per-row Apply
/// buttons target. Routed via [`AppEvent::ApplyFeedsField`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsField {
    Rpm,
    Feed,
    Plunge,
    Doc,
    Woc,
}

/// Events emitted by UI components, processed after the UI pass.
#[derive(Debug)]
pub enum AppEvent {
    // File
    ImportStl(PathBuf),
    ImportSvg(PathBuf),
    ImportDxf(PathBuf),
    ImportStep(PathBuf),
    RescaleModel(ModelId, crate::state::job::ModelUnits),
    RemoveModel(ModelId),
    ReloadModel(ModelId),
    ExportGcode,
    ExportCombinedGcode,
    ExportSetupGcode(SetupId),
    ExportSetupSheet,
    ExportSvgPreview,
    SaveJob,
    OpenJob,
    /// Toggle generator-trace capture on every toolpath (sets
    /// `debug_options.enabled` across the whole project).
    SetGeneratorTraceCaptureAll(bool),

    // Selection / view
    Select(crate::state::selection::Selection),
    SetViewPreset(ViewPreset),
    ToggleProjection,
    ClearIsolation,
    PreviewOrientation(FaceUp),
    ResetView,

    // Tools
    AddTool(ToolType),
    /// Add a fully-specified tool copied from a library catalog. The
    /// session reassigns the id on insert.
    AddToolFromLibrary(Box<ToolConfig>),
    DuplicateTool(ToolId),
    RemoveTool(ToolId),

    // Tool Library modal
    /// Open the Tool Library management modal. Loads a snapshot of every
    /// catalog in the per-user library dir into `tool_library_modal`.
    OpenToolLibrary,
    /// Close the Tool Library modal.
    CloseToolLibrary,
    /// Delete the tool at `index` in catalog `catalog`, then refresh the
    /// modal snapshot.
    DeleteLibraryTool {
        catalog: String,
        index: usize,
    },
    /// Replace the tool at `index` in catalog `catalog` with an edited
    /// copy, then refresh the snapshot.
    UpdateLibraryTool {
        catalog: String,
        index: usize,
        tool: Box<ToolConfig>,
    },
    /// Move the tool at `index` from catalog `from` to catalog `to`.
    MoveLibraryTool {
        from: String,
        index: usize,
        to: String,
    },
    /// Create a new empty catalog.
    CreateToolCatalog(String),
    /// Delete a whole catalog file.
    DeleteToolCatalog(String),
    /// Rename a catalog file.
    RenameToolCatalog {
        old: String,
        new: String,
    },
    /// De-duplicate the tools in a catalog (keep first of each geometry).
    DedupeToolCatalog(String),

    // Setups
    AddSetup,
    RemoveSetup(SetupId),
    RenameSetup(SetupId, String),
    /// One-click two-sided setup: create flipped Setup 2, set flip axis, auto-place pins.
    SetupTwoSided,

    // Fixtures and keep-out zones
    AddFixture(SetupId),
    RemoveFixture(SetupId, FixtureId),
    AddKeepOut(SetupId),
    RemoveKeepOut(SetupId, KeepOutId),
    FixtureChanged,

    // Toolpaths
    AddToolpath(OperationType),
    DuplicateToolpath(ToolpathId),
    RemoveToolpath(ToolpathId),
    MoveToolpathUp(ToolpathId),
    MoveToolpathDown(ToolpathId),
    /// Reorder a toolpath within its current setup to a target index.
    ReorderToolpath(ToolpathId, usize),
    /// Move a toolpath from its current setup to a different setup at a target index.
    MoveToolpathToSetup(ToolpathId, SetupId, usize),
    ToggleToolpathEnabled(ToolpathId),
    GenerateToolpath(ToolpathId),
    GenerateAll,
    ToggleToolpathVisibility(ToolpathId),
    ToggleIsolateToolpath,
    InspectToolpathInSimulation(ToolpathId),

    // Simulation
    RunSimulation,
    RunSimulationWith(Vec<ToolpathId>),
    ResetSimulation,
    ToggleSimPlayback,

    // Workspace navigation
    SwitchWorkspace(Workspace),
    SimStepForward,
    SimStepBackward,
    SimJumpToStart,
    SimJumpToEnd,
    SimJumpToMove(usize),
    SimJumpToOpStart(usize),
    SimJumpToOpEnd(usize),

    // Pre-flight / Export
    ExportGcodeConfirmed,
    /// Open the multi-step Export Wizard at the user's last-visited step.
    OpenExportWizard,
    /// Close the wizard. Persistent settings on `session.wizard()` are
    /// preserved; only the transient modal state goes away.
    CloseExportWizard,
    /// Switch the visible wizard step. Bounds-checked in the handler.
    WizardSetStep(u8),
    /// Update the post-processor format from the wizard's Step 1 dropdown.
    WizardSetPost(crate::state::job::PostFormat),
    /// Step 2: pick how the emitted g-code is split across files.
    WizardSetOutputLayout(rs_cam_core::session::OutputLayout),
    /// Step 2: update the filename-template field. Substitutions like
    /// `{job}` / `{setup}` / `{toolpath}` are applied at save time
    /// based on the active layout.
    WizardSetFilenameTemplate(String),
    /// Step 3: WCS override. `None` = use the post's default.
    WizardSetWcsOverride(Option<rs_cam_core::gcode::WcsCode>),
    /// Step 3: units override. `None` = inherit from the post.
    WizardSetUnitsOverride(Option<rs_cam_core::gcode::Units>),
    /// Step 3: safe-Z override (mm). `None` = use the project default
    /// (`gui.post.safe_z`).
    WizardSetSafeZOverride(Option<f64>),
    /// Step 3: dry-run mode. When true, the emit pass clamps every
    /// cutting move's Z to the effective safe-Z so the spindle stays
    /// in air for the whole program.
    WizardSetDryRun(bool),
    /// Step 4: spindle warmup dwell in seconds. Zero disables.
    WizardSetSpindleWarmup(u32),
    /// Step 4: tool-change handling override. `None` = use the post's
    /// `tool_change` template; `Pause`/`M6` swap the template;
    /// `Suppress` strips tool-change blocks (keeping per-tool RPM).
    WizardSetToolChangeMode(Option<rs_cam_core::gcode::ToolChangeMode>),
    /// Step 4.5: per-setup pause-message override. `None` falls back to
    /// the default `Setup change: <name>` text emitted before the
    /// inter-setup `M0`. `Some("...")` replaces it verbatim — used to
    /// instruct the operator to run a sender macro (Z probe, corner
    /// probe, home) before pressing Resume.
    WizardSetSetupPauseMessage {
        setup_id: usize,
        message: Option<String>,
    },
    /// Step 5: toggle the "I understand the risks, allow save with
    /// validator errors" override.
    WizardSetAllowValidatorErrors(bool),
    /// Step 6: commit the export to disk using the wizard's settings.
    /// Pops a file/directory picker, writes the file(s), pushes a
    /// notification, and closes the wizard on success.
    WizardSave,
    /// Set the tool-load export-gate override flags. The two flags are
    /// independent — `accept_unmodeled` only bypasses `Unmodeled` verdicts,
    /// `accept_exceeded` only bypasses `Exceeds` verdicts.
    SetToolLoadOverride {
        accept_unmodeled: bool,
        accept_exceeded: bool,
    },
    /// Re-upload simulation mesh with new viz colors.
    SimVizModeChanged,

    // Optimize (U2 of OPTIMIZER_UX_PLAN.md)
    /// Open the Optimize modal for a specific toolpath. Triggers
    /// optimize_toolpath synchronously and stashes the outcome on
    /// `AppState::optimize_modal`. Long-running — the GUI freezes
    /// until U3's worker-thread integration lands.
    OpenOptimizeModal(ToolpathId),
    /// Close the Optimize modal.
    CloseOptimizeModal,
    /// Apply a candidate from the Optimize modal. Carries the candidate
    /// index into `OptimizeOutcome::Ranked(..)` so the controller can
    /// look up the params + delta from the cached outcome rather than
    /// shipping the full `OperationConfig` through an event.
    ApplyOptimizeCandidate {
        toolpath_id: ToolpathId,
        /// Index into the cached `Ranked` candidates list. Index 0 is
        /// the baseline (apply does nothing); index ≥ 1 selects a
        /// non-baseline candidate.
        candidate_index: usize,
    },
    /// OPT-005 — accept an operator suggestion from the Optimize modal:
    /// set the named axis to `value` on the toolpath, then re-run the
    /// search against the new baseline (re-opens the modal). An explicit
    /// operator click — never an auto-apply — that removes the manual
    /// re-typing step the prose suggestions used to require.
    ReoptimizeWithAxisOverride {
        toolpath_id: ToolpathId,
        axis: rs_cam_core::tool_load::optimize::KnobAxis,
        value: f64,
    },

    // Feeds & Speeds modal (Feeds-tab redesign)
    /// Open the redesigned Feeds & Speeds modal focused on a specific
    /// toolpath. The modal renders the current/recommended comparison
    /// card plus three machinist charts; it re-derives the underlying
    /// `FeedsExplain` from session state every frame.
    OpenFeedsModal(ToolpathId),
    /// Close the Feeds & Speeds modal.
    CloseFeedsModal,
    /// Toggle between single-toolpath and project-rollup mode within
    /// the Feeds & Speeds modal.
    SetFeedsModalMode(crate::state::FeedsModalMode),
    /// Set the project-level spindle policy (chart-fidelity vs
    /// max-speed). Persists into `ProjectPostConfig.spindle_strategy`.
    /// All Suggest calls and the Feeds modal use the new policy on the
    /// next frame. Re-emit per change since the strategy alters every
    /// toolpath's recommendation simultaneously.
    SetSpindleStrategy(rs_cam_core::feeds::SpindleStrategy),
    /// Apply a single recommended value to a toolpath. Field-scoped so
    /// per-row Apply buttons (RPM, feed, plunge, DOC, WOC) route here.
    ApplyFeedsField {
        toolpath_id: ToolpathId,
        field: FeedsField,
    },
    /// Apply every recommended value (RPM, feed, plunge, DOC, WOC) to a
    /// toolpath in one transactional update. The Apply-all button on
    /// the comparison card routes here.
    ApplyFeedsAll(ToolpathId),
    /// S1 — set (or clear) the scallop-driven-stepover target on a
    /// DropCutter toolpath. `Some(h)` switches WOC to be derived from
    /// the cusp height + tool tip radius; `None` restores the formula
    /// stepover. No-op for operations that don't carry the field.
    SetDropCutterScallopHeight {
        toolpath_id: ToolpathId,
        value: Option<f64>,
    },
    /// Apply the Feeds recommendation across every selected toolpath
    /// in project-rollup mode.
    ApplyFeedsProject,
    /// Toggle the "How is this calculated?" provenance disclosure.
    ToggleFeedsProvenance,
    /// Apply a custom (feed, rpm) pair from the Chart C drag-to-explore
    /// interaction. Routed only when the user releases the drag inside
    /// the chart bounds and confirms.
    ApplyFeedsExplore {
        toolpath_id: ToolpathId,
        feed_mm_min: f64,
        rpm: f64,
    },
    /// Change the sort order in the Feeds modal's project-rollup table.
    SetFeedsProjectSort(crate::state::ProjectFeedsSort),
    /// Set the drag-to-explore overlay point on the feed-RPM nomogram.
    /// `None` clears the overlay (reverts the chart marker back to the
    /// current/recommended pair).
    SetFeedsExplore(Option<crate::state::NomogramExplore>),
    /// Toggle a row in the Feeds project-view selection.
    ToggleFeedsProjectRow(ToolpathId),
    /// Apply Feeds recommendations to every selected (checked) toolpath.
    ApplyFeedsProjectSelected,
    /// Set the project-view scatter overlay visibility.
    SetFeedsProjectScatter(bool),
    /// Select / deselect every project-view row in one click.
    SetFeedsProjectSelectAll(bool),

    // Optimize project (U3 of OPTIMIZER_UX_PLAN.md)
    /// Open the project-level Optimize rollup. Submits an
    /// `OptimizeRequest::Project` to the worker lane, which walks
    /// every enabled toolpath. The view opens in `Loading` state
    /// immediately; the rollup populates when the worker returns.
    OpenOptimizeProject,
    /// Close the rollup view. Cancels the worker lane if it's still
    /// running and discards any in-flight result.
    CloseOptimizeProject,
    /// Toggle the row checkbox for batch Apply. The controller flips
    /// the bool at the given index in `optimize_project.row_selected`.
    ToggleOptimizeProjectRow(usize),
    /// Apply every row whose checkbox is currently true. Each
    /// applied candidate is the first-safe recommendation from that
    /// row's outcome. Routes through `apply_toolpath_param_snapshot`.
    ApplyOptimizeProject,

    // Collision
    RunCollisionCheck,

    // Compute
    CancelCompute,

    // Face selection
    ToggleFaceSelection {
        toolpath_id: ToolpathId,
        model_id: ModelId,
        face_id: FaceGroupId,
    },

    // Edit
    StockChanged,
    StockMaterialChanged,
    MachineChanged,
    Undo,
    Redo,

    // Help
    ShowShortcuts,

    Quit,
}
