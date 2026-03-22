use rs_cam_core::enriched_mesh::FaceGroupId;

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
    /// Single BREP face selected on an enriched mesh model.
    Face(ModelId, FaceGroupId),
    /// Multiple BREP faces selected (shift+click accumulation).
    Faces(ModelId, Vec<FaceGroupId>),
}
