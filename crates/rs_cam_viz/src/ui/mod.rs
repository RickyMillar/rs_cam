#![deny(clippy::indexing_slicing)]

pub mod automation;
pub mod menu_bar;
pub mod preflight;
pub mod project_tree;
pub mod properties;
pub mod setup_panel;
pub mod shortcuts_window;
pub mod sim_debug;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod optimize_modal;
pub mod optimize_project;
pub mod sim_timeline;
pub mod status_bar;
pub mod theme;
pub mod toolpath_panel;
pub mod toolpath_row_controls;
pub mod viewport_overlay;
pub mod workspace_bar;

use crate::render::camera::ViewPreset;
use crate::state::Workspace;
use crate::state::job::{FaceUp, FixtureId, KeepOutId, ModelId, SetupId, ToolId, ToolType};
use crate::state::toolpath::{OperationType, ToolpathId};
use rs_cam_core::enriched_mesh::FaceGroupId;
use std::path::PathBuf;

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
    DuplicateTool(ToolId),
    RemoveTool(ToolId),

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
