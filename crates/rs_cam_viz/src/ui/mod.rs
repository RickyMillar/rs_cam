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
    ImportStl(PathBuf),
    ImportSvg(PathBuf),
    Select(crate::state::selection::Selection),
    SetViewPreset(ViewPreset),
    ResetView,
    AddTool(ToolType),
    DuplicateTool(ToolId),
    RemoveTool(ToolId),
    AddToolpath(OperationType),
    RemoveToolpath(ToolpathId),
    GenerateToolpath(ToolpathId),
    ToggleToolpathVisibility(ToolpathId),
    StockChanged,
    Quit,
}
