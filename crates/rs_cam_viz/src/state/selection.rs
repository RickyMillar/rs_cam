use super::job::{ModelId, ToolId};

/// What is currently selected in the project tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    None,
    Stock,
    PostProcessor,
    Model(ModelId),
    Tool(ToolId),
}
