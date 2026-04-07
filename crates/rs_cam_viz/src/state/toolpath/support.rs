// Re-export all support types from rs_cam_core::compute::config.
// These types were moved to core as part of the service layer extraction (Phase 1).
pub use rs_cam_core::compute::config::{
    BoundaryContainment, ComputeStatus, DressupConfig, DressupEntryStyle, FeedsAutoMode,
    HeightContext, HeightMode, HeightReference, HeightsConfig, ReferenceOffset, ResolvedHeights,
    RetractStrategy, StockSource, ToolpathId, ToolpathStats,
};
