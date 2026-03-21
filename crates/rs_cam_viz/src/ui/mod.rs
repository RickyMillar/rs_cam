pub mod automation;
pub mod menu_bar;
pub mod preflight;
pub mod project_tree;
pub mod properties;
pub mod setup_panel;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod sim_timeline;
pub mod status_bar;
pub mod toolpath_panel;
pub mod viewport_overlay;
pub mod workspace_bar;

use crate::render::camera::ViewPreset;
use crate::state::Workspace;
use crate::state::job::{FaceUp, FixtureId, KeepOutId, SetupId, ToolId, ToolType};
use crate::state::toolpath::{OperationType, ToolpathId};
use std::path::PathBuf;

/// Events emitted by UI components, processed after the UI pass.
#[derive(Debug)]
pub enum AppEvent {
    // File
    ImportStl(PathBuf),
    ImportSvg(PathBuf),
    ImportDxf(PathBuf),
    RescaleModel(crate::state::job::ModelId, crate::state::job::ModelUnits),
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

    // Simulation
    RunSimulation,
    RunSimulationWith(Vec<ToolpathId>),
    ResetSimulation,
    ToggleSimPlayback,
    ToggleSimToolpath(ToolpathId),

    // Workspace navigation
    SwitchWorkspace(Workspace),
    SimStepForward,
    SimStepBackward,
    SimJumpToStart,
    SimJumpToEnd,
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

    // Edit
    StockChanged,
    StockMaterialChanged,
    MachineChanged,
    RecalculateFeeds(ToolpathId),
    Undo,
    Redo,

    Quit,
}
