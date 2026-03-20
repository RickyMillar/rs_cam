pub mod menu_bar;
pub mod project_tree;
pub mod properties;
pub mod status_bar;
pub mod viewport_overlay;

use crate::render::camera::ViewPreset;
use crate::state::job::{ToolId, ToolType};
use std::path::PathBuf;

/// Events emitted by UI components, processed after the UI pass.
#[derive(Debug)]
pub enum AppEvent {
    ImportStl(PathBuf),
    Select(crate::state::selection::Selection),
    SetViewPreset(ViewPreset),
    ResetView,
    AddTool(ToolType),
    DuplicateTool(ToolId),
    RemoveTool(ToolId),
    /// Stock or tool params were edited inline; re-upload GPU data.
    StockChanged,
    Quit,
}
