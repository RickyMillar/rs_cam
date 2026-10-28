use super::job::{ModelId, ObstacleId, SetupId, ToolId};
use super::toolpath::ToolpathId;

/// What is currently selected in the project tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    None,
    Stock,
    PostProcessor,
    Machine,
    Model(ModelId),
    Tool(ToolId),
    Setup(SetupId),
    /// K1: an obstacle of a setup (a fixture or a keep-out zone).
    Obstacle(SetupId, ObstacleId),
    Toolpath(ToolpathId),
}
