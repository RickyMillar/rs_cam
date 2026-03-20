use super::job::{ModelId, ToolId};
use super::toolpath::ToolpathId;

/// What is currently selected in the project tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    None,
    Stock,
    PostProcessor,
    Model(ModelId),
    Tool(ToolId),
    Toolpath(ToolpathId),
}
