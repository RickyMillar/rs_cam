//! Compute configuration, execution helpers, simulation, and collision checking.
//!
//! This module contains the shared config types (Phase 1), execution helpers
//! (Phase 2), and simulation/collision logic (Phase 3) extracted from
//! `rs_cam_viz` so they can be used by both the GUI and CLI.

pub mod catalog;
pub mod collision_check;
pub mod config;
pub mod cutter;
pub mod operation_configs;
pub mod semantic_helpers;
pub mod simulate;
pub mod stats;
pub mod stock_config;
pub mod tool_config;
pub mod transform;

// ── Phase 1: Config type re-exports ──

pub use catalog::{
    DepthSemantics, GeometryRequirement, OperationConfig, OperationFamily, OperationParams,
    OperationSpec, OperationType, UiOperationFamily, UiProcessRole,
    feed_optimization_unavailable_reason,
};

pub use config::{
    BoundaryContainment, ComputeStatus, DressupConfig, DressupEntryStyle, FeedsAutoMode,
    HeightContext, HeightMode, HeightReference, HeightsConfig, ReferenceOffset, ResolvedHeights,
    RetractStrategy, StockSource, ToolpathId, ToolpathStats,
};

pub use operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig,
    ClearingStrategy, CompensationType, CutDirection, DrillConfig, DrillCycleType,
    DropCutterConfig, FaceConfig, FaceDirection, HorizontalFinishConfig, InlayConfig, PencilConfig,
    PocketConfig, PocketPattern, ProfileConfig, ProfileSide, ProjectCurveConfig,
    RadialFinishConfig, RampFinishConfig, RegionOrdering, RestConfig, ScallopConfig,
    ScallopDirection, SpiralDirection, SpiralFinishConfig, SteepShallowConfig, TraceCompensation,
    TraceConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};

pub use tool_config::{BitCutDirection, ToolConfig, ToolId, ToolMaterial, ToolType};

pub use stock_config::{
    AlignmentPin, FixtureId, FlipAxis, KeepOutId, ModelId, ModelKind, ModelUnits, PostConfig,
    PostFormat, SetupId, StockConfig,
};

pub use transform::{FaceUp, ZRotation};

// ── Phase 2: Execution helper re-exports ──

pub use cutter::build_cutter;
pub use semantic_helpers::{
    CutRun, append_toolpath, bind_scope_to_full_toolpath, bind_scope_to_run, contour_toolpath,
    cutting_runs, line_toolpath,
};
pub use stats::compute_stats;
