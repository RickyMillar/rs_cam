mod catalog;
mod configs;
mod entry;
mod support;

pub use catalog::{
    DepthSemantics, GeometryRequirement, OperationConfig, OperationFamily, OperationParams,
    OperationSpec, OperationType, UiOperationFamily, UiProcessRole,
    feed_optimization_unavailable_reason,
};
pub use configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig,
    ClearingStrategy, CompensationType, CutDirection, DrillConfig, DrillCycleType,
    DropCutterConfig, FaceConfig, FaceDirection, HorizontalFinishConfig, InlayConfig, PencilConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, ProjectCurveConfig,
    ProjectCurveDirection, RadialFinishConfig, RampFinishConfig, RegionOrdering, RestConfig,
    ScallopConfig, ScallopDirection, SpiralDirection, SpiralFinishConfig, SteepShallowConfig,
    TraceCompensation, TraceConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};
pub use entry::{ToolpathEntry, ToolpathEntryInit, ToolpathResult};
pub use support::{
    BoundaryConfig, BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig,
    DressupEntryStyle, FeedsAutoMode, HeightContext, HeightMode, HeightReference, HeightsConfig,
    ReferenceOffset, ResolvedHeights, RetractStrategy, StockSource, ToolpathId, ToolpathStats,
};
