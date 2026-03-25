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
pub mod sim_timeline;
pub mod status_bar;
pub mod theme;
pub mod toolpath_panel;
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

    // Selection / view
    Select(crate::state::selection::Selection),
    SetViewPreset(ViewPreset),
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
    /// Re-upload simulation mesh with new viz colors.
    SimVizModeChanged,

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
