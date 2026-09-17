// Re-export all support types from rs_cam_core::compute::config.
// These types were moved to core as part of the service layer extraction (Phase 1).
pub use rs_cam_core::compute::config::{
    ArcFitParams, AwaitingPriorStock, BoundaryConfig, BoundaryContainment, BoundarySource,
    ComputeStatus, DogboneParams, DressupConfig, DressupEntryStyle, HeightContext, HeightMode,
    HeightReference, HeightsConfig, LeadParams, LinkDressupParams, ReferenceOffset,
    ResolvedHeights, RestAnalysisConfig, StockSource, ToolpathId,
};
pub use rs_cam_core::compute::toolpath_stats::ToolpathStats;
