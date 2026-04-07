// Re-export all operation config types from rs_cam_core::compute::operation_configs.
// These types were moved to core as part of the service layer extraction (Phase 1).
pub use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig,
    ClearingStrategy, CompensationType, CutDirection, DrillConfig, DrillCycleType,
    DropCutterConfig, FaceConfig, FaceDirection, HorizontalFinishConfig, InlayConfig, PencilConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, ProjectCurveConfig,
    RadialFinishConfig, RampFinishConfig, RegionOrdering, RestConfig, ScallopConfig,
    ScallopDirection, SpiralDirection, SpiralFinishConfig, SteepShallowConfig, TraceCompensation,
    TraceConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};
