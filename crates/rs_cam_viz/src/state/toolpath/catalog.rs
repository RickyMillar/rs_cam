// Re-export all catalog types from rs_cam_core::compute::catalog.
// These types were moved to core as part of the service layer extraction (Phase 1).
pub use rs_cam_core::compute::catalog::{
    DepthSemantics, GeometryRequirement, OperationConfig, OperationFamily, OperationParams,
    OperationSpec, OperationType, UiOperationFamily, UiProcessRole,
    feed_optimization_unavailable_reason,
};
