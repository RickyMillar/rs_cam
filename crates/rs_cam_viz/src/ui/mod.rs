pub mod menu_bar;
pub mod project_tree;
pub mod properties;
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
    ExportGcode,
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

    // Simulation
    RunSimulation,
    ResetSimulation,
    ToggleSimPlayback,

    // Collision
    RunCollisionCheck,

    // Edit
    StockChanged,
    Undo,
    Redo,

    Quit,
}
