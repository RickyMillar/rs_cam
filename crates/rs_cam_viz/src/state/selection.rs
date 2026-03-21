use super::job::{FixtureId, KeepOutId, ModelId, SetupId, ToolId};
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
    Fixture(SetupId, FixtureId),
    KeepOut(SetupId, KeepOutId),
    Toolpath(ToolpathId),
}
