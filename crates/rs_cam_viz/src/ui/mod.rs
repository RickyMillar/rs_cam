pub mod automation;
pub mod menu_bar;
pub mod preflight;
pub mod project_tree;
pub mod properties;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod sim_timeline;
pub mod status_bar;
pub mod viewport_overlay;

use crate::render::camera::ViewPreset;
use crate::state::job::{ToolId, ToolType};
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
    ExportSetupSheet,
    ExportSvgPreview,
    SaveJob,
    OpenJob,

    // Selection / view
    Select(crate::state::selection::Selection),
    SetViewPreset(ViewPreset),
    ResetView,

    // Tools
    AddTool(ToolType),
    DuplicateTool(ToolId),
    RemoveTool(ToolId),

    // Toolpaths
    AddToolpath(OperationType),
    DuplicateToolpath(ToolpathId),
    RemoveToolpath(ToolpathId),
    MoveToolpathUp(ToolpathId),
    MoveToolpathDown(ToolpathId),
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

    // Simulation workspace
    EnterSimulation,
    ExitSimulation,
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
