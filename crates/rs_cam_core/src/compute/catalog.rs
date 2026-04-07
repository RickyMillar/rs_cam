use serde::{Deserialize, Serialize};

use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole};

use super::config::StockSource;
use super::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig, DrillConfig,
    DropCutterConfig, FaceConfig, HorizontalFinishConfig, InlayConfig, PencilConfig, PocketConfig,
    ProfileConfig, ProjectCurveConfig, RadialFinishConfig, RampFinishConfig, RestConfig,
    ScallopConfig, SpiralFinishConfig, SteepShallowConfig, TraceConfig, VCarveConfig,
    WaterlineConfig, ZigzagConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    TwoPointFiveD,
    ThreeD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryRequirement {
    Stock,
    Polygons,
    Mesh,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiOperationFamily {
    Pocket,
    Contour,
    Trace,
    Parallel,
    Scallop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiProcessRole {
    Roughing,
    SemiFinish,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepthSemantics {
    Explicit(f64),
    DerivedStockTop(f64),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSpec {
    pub label: &'static str,
    pub family: OperationFamily,
    pub geometry: GeometryRequirement,
    pub default_auto_regen: bool,
    pub ui_family: UiOperationFamily,
    pub ui_process_role: UiProcessRole,
    pub feeds_family: FeedsOperationFamily,
    pub feeds_pass_role: PassRole,
}

/// Operation type for creating new toolpaths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Face,
    Pocket,
    Profile,
    Adaptive,
    VCarve,
    Rest,
    Inlay,
    Zigzag,
    Trace,
    Drill,
    Chamfer,
    DropCutter,
    Adaptive3d,
    Waterline,
    Pencil,
    Scallop,
    SteepShallow,
    RampFinish,
    SpiralFinish,
    RadialFinish,
    HorizontalFinish,
    ProjectCurve,
    /// Auto-generated drilling operation for stock alignment pin holes.
    AlignmentPinDrill,
}

impl OperationType {
    pub const ALL: &[OperationType] = &[
        OperationType::Face,
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Rest,
        OperationType::Inlay,
        OperationType::Zigzag,
        OperationType::Trace,
        OperationType::Drill,
        OperationType::Chamfer,
        OperationType::DropCutter,
        OperationType::Adaptive3d,
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::Scallop,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
        OperationType::ProjectCurve,
        OperationType::AlignmentPinDrill,
    ];

    pub const ALL_2D: &[OperationType] = &[
        OperationType::Face,
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Rest,
        OperationType::Inlay,
        OperationType::Zigzag,
        OperationType::Trace,
        OperationType::Drill,
        OperationType::Chamfer,
    ];

    pub const ALL_3D: &[OperationType] = &[
        OperationType::DropCutter,
        OperationType::Adaptive3d,
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::Scallop,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
        OperationType::ProjectCurve,
    ];

    pub fn spec(self) -> OperationSpec {
        match self {
            OperationType::Face => OperationSpec {
                label: "Face",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Stock,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Pocket => OperationSpec {
                label: "Pocket",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Profile => OperationSpec {
                label: "Profile",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Contour,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Contour,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Adaptive => OperationSpec {
                label: "Adaptive",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Adaptive,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::VCarve => OperationSpec {
                label: "VCarve",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::Rest => OperationSpec {
                label: "Rest Machining",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Inlay => OperationSpec {
                label: "Inlay",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::Zigzag => OperationSpec {
                label: "Zigzag",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Trace => OperationSpec {
                label: "Trace",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::Drill => OperationSpec {
                label: "Drill",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Chamfer => OperationSpec {
                label: "Chamfer",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Polygons,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::DropCutter => OperationSpec {
                label: "3D Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Parallel,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Parallel,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::Adaptive3d => OperationSpec {
                label: "3D Rough",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Adaptive,
                feeds_pass_role: PassRole::Roughing,
            },
            OperationType::Waterline => OperationSpec {
                label: "Waterline",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Contour,
                ui_process_role: UiProcessRole::SemiFinish,
                feeds_family: FeedsOperationFamily::Contour,
                feeds_pass_role: PassRole::SemiFinish,
            },
            OperationType::Pencil => OperationSpec {
                label: "Pencil Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::Scallop => OperationSpec {
                label: "Scallop Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Scallop,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Scallop,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::SteepShallow => OperationSpec {
                label: "Steep/Shallow",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Contour,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Contour,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::RampFinish => OperationSpec {
                label: "Ramp Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Parallel,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Parallel,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::SpiralFinish => OperationSpec {
                label: "Spiral Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Scallop,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Scallop,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::RadialFinish => OperationSpec {
                label: "Radial Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Parallel,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Parallel,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::HorizontalFinish => OperationSpec {
                label: "Horizontal Finish",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Mesh,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Parallel,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Parallel,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::ProjectCurve => OperationSpec {
                label: "Project Curve",
                family: OperationFamily::ThreeD,
                geometry: GeometryRequirement::Both,
                default_auto_regen: false,
                ui_family: UiOperationFamily::Trace,
                ui_process_role: UiProcessRole::Finish,
                feeds_family: FeedsOperationFamily::Trace,
                feeds_pass_role: PassRole::Finish,
            },
            OperationType::AlignmentPinDrill => OperationSpec {
                label: "Pin Drill",
                family: OperationFamily::TwoPointFiveD,
                geometry: GeometryRequirement::Stock,
                default_auto_regen: true,
                ui_family: UiOperationFamily::Pocket,
                ui_process_role: UiProcessRole::Roughing,
                feeds_family: FeedsOperationFamily::Pocket,
                feeds_pass_role: PassRole::Roughing,
            },
        }
    }

    pub fn label(self) -> &'static str {
        self.spec().label
    }
}

/// Common parameter accessors for all operation configs.
///
/// Implemented by each config struct to eliminate per-variant match arms.
/// Optional fields (stepover, depth_per_pass) return None by default.
pub trait OperationParams {
    fn feed_rate(&self) -> f64;
    fn set_feed_rate(&mut self, value: f64);

    /// Returns the plunge rate. Drill operations return feed_rate since they're purely vertical.
    fn plunge_rate(&self) -> f64;
    fn set_plunge_rate(&mut self, value: f64);

    fn stepover(&self) -> Option<f64> {
        None
    }
    fn set_stepover(&mut self, _value: f64) {}

    fn depth_per_pass(&self) -> Option<f64> {
        None
    }
    fn set_depth_per_pass(&mut self, _value: f64) {}

    fn depth_semantics(&self) -> DepthSemantics;
}

/// Operation-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum OperationConfig {
    Face(FaceConfig),
    Pocket(PocketConfig),
    Profile(ProfileConfig),
    Adaptive(AdaptiveConfig),
    VCarve(VCarveConfig),
    Rest(RestConfig),
    Inlay(InlayConfig),
    Zigzag(ZigzagConfig),
    Trace(TraceConfig),
    Drill(DrillConfig),
    Chamfer(ChamferConfig),
    DropCutter(DropCutterConfig),
    Adaptive3d(Adaptive3dConfig),
    Waterline(WaterlineConfig),
    Pencil(PencilConfig),
    Scallop(ScallopConfig),
    SteepShallow(SteepShallowConfig),
    RampFinish(RampFinishConfig),
    SpiralFinish(SpiralFinishConfig),
    RadialFinish(RadialFinishConfig),
    HorizontalFinish(HorizontalFinishConfig),
    ProjectCurve(ProjectCurveConfig),
    AlignmentPinDrill(AlignmentPinDrillConfig),
}

impl OperationConfig {
    pub fn op_type(&self) -> OperationType {
        match self {
            OperationConfig::Face(_) => OperationType::Face,
            OperationConfig::Pocket(_) => OperationType::Pocket,
            OperationConfig::Profile(_) => OperationType::Profile,
            OperationConfig::Adaptive(_) => OperationType::Adaptive,
            OperationConfig::VCarve(_) => OperationType::VCarve,
            OperationConfig::Rest(_) => OperationType::Rest,
            OperationConfig::Inlay(_) => OperationType::Inlay,
            OperationConfig::Zigzag(_) => OperationType::Zigzag,
            OperationConfig::Trace(_) => OperationType::Trace,
            OperationConfig::Drill(_) => OperationType::Drill,
            OperationConfig::Chamfer(_) => OperationType::Chamfer,
            OperationConfig::DropCutter(_) => OperationType::DropCutter,
            OperationConfig::Adaptive3d(_) => OperationType::Adaptive3d,
            OperationConfig::Waterline(_) => OperationType::Waterline,
            OperationConfig::Pencil(_) => OperationType::Pencil,
            OperationConfig::Scallop(_) => OperationType::Scallop,
            OperationConfig::SteepShallow(_) => OperationType::SteepShallow,
            OperationConfig::RampFinish(_) => OperationType::RampFinish,
            OperationConfig::SpiralFinish(_) => OperationType::SpiralFinish,
            OperationConfig::RadialFinish(_) => OperationType::RadialFinish,
            OperationConfig::HorizontalFinish(_) => OperationType::HorizontalFinish,
            OperationConfig::ProjectCurve(_) => OperationType::ProjectCurve,
            OperationConfig::AlignmentPinDrill(_) => OperationType::AlignmentPinDrill,
        }
    }

    pub fn spec(&self) -> OperationSpec {
        self.op_type().spec()
    }

    pub fn label(&self) -> &'static str {
        self.spec().label
    }

    pub fn family(&self) -> OperationFamily {
        self.spec().family
    }

    pub fn geometry_requirement(&self) -> GeometryRequirement {
        self.spec().geometry
    }

    pub fn default_auto_regen(&self) -> bool {
        self.spec().default_auto_regen
    }

    pub fn ui_style(&self) -> (UiOperationFamily, UiProcessRole) {
        let spec = self.spec();
        (spec.ui_family, spec.ui_process_role)
    }

    pub fn feeds_style(&self) -> (FeedsOperationFamily, PassRole) {
        let spec = self.spec();
        (spec.feeds_family, spec.feeds_pass_role)
    }

    pub fn is_3d(&self) -> bool {
        self.family() == OperationFamily::ThreeD
    }

    pub fn is_stock_based(&self) -> bool {
        self.geometry_requirement() == GeometryRequirement::Stock
    }

    pub fn needs_both(&self) -> bool {
        self.geometry_requirement() == GeometryRequirement::Both
    }

    pub fn as_params(&self) -> &dyn OperationParams {
        match self {
            OperationConfig::Face(c) => c,
            OperationConfig::Pocket(c) => c,
            OperationConfig::Profile(c) => c,
            OperationConfig::Adaptive(c) => c,
            OperationConfig::VCarve(c) => c,
            OperationConfig::Rest(c) => c,
            OperationConfig::Inlay(c) => c,
            OperationConfig::Zigzag(c) => c,
            OperationConfig::Trace(c) => c,
            OperationConfig::Drill(c) => c,
            OperationConfig::Chamfer(c) => c,
            OperationConfig::DropCutter(c) => c,
            OperationConfig::Adaptive3d(c) => c,
            OperationConfig::Waterline(c) => c,
            OperationConfig::Pencil(c) => c,
            OperationConfig::Scallop(c) => c,
            OperationConfig::SteepShallow(c) => c,
            OperationConfig::RampFinish(c) => c,
            OperationConfig::SpiralFinish(c) => c,
            OperationConfig::RadialFinish(c) => c,
            OperationConfig::HorizontalFinish(c) => c,
            OperationConfig::ProjectCurve(c) => c,
            OperationConfig::AlignmentPinDrill(c) => c,
        }
    }

    pub fn as_params_mut(&mut self) -> &mut dyn OperationParams {
        match self {
            OperationConfig::Face(c) => c,
            OperationConfig::Pocket(c) => c,
            OperationConfig::Profile(c) => c,
            OperationConfig::Adaptive(c) => c,
            OperationConfig::VCarve(c) => c,
            OperationConfig::Rest(c) => c,
            OperationConfig::Inlay(c) => c,
            OperationConfig::Zigzag(c) => c,
            OperationConfig::Trace(c) => c,
            OperationConfig::Drill(c) => c,
            OperationConfig::Chamfer(c) => c,
            OperationConfig::DropCutter(c) => c,
            OperationConfig::Adaptive3d(c) => c,
            OperationConfig::Waterline(c) => c,
            OperationConfig::Pencil(c) => c,
            OperationConfig::Scallop(c) => c,
            OperationConfig::SteepShallow(c) => c,
            OperationConfig::RampFinish(c) => c,
            OperationConfig::SpiralFinish(c) => c,
            OperationConfig::RadialFinish(c) => c,
            OperationConfig::HorizontalFinish(c) => c,
            OperationConfig::ProjectCurve(c) => c,
            OperationConfig::AlignmentPinDrill(c) => c,
        }
    }

    pub fn feed_rate(&self) -> f64 {
        self.as_params().feed_rate()
    }

    pub fn set_feed_rate(&mut self, value: f64) {
        self.as_params_mut().set_feed_rate(value);
    }

    /// Returns the plunge rate.
    /// Drill ops are purely vertical -- feed_rate IS the plunge rate.
    pub fn plunge_rate(&self) -> f64 {
        self.as_params().plunge_rate()
    }

    pub fn set_plunge_rate(&mut self, value: f64) {
        self.as_params_mut().set_plunge_rate(value);
    }

    pub fn stepover(&self) -> Option<f64> {
        self.as_params().stepover()
    }

    pub fn set_stepover(&mut self, value: f64) {
        self.as_params_mut().set_stepover(value);
    }

    pub fn depth_per_pass(&self) -> Option<f64> {
        self.as_params().depth_per_pass()
    }

    pub fn set_depth_per_pass(&mut self, value: f64) {
        self.as_params_mut().set_depth_per_pass(value);
    }

    pub fn depth_semantics(&self) -> DepthSemantics {
        self.as_params().depth_semantics()
    }

    pub fn default_depth_for_heights(&self) -> f64 {
        match self.depth_semantics() {
            DepthSemantics::Explicit(value) | DepthSemantics::DerivedStockTop(value) => value.abs(),
            DepthSemantics::None => 0.0,
        }
    }

    pub fn new_default(op_type: OperationType) -> Self {
        match op_type {
            OperationType::Face => OperationConfig::Face(FaceConfig::default()),
            OperationType::Pocket => OperationConfig::Pocket(PocketConfig::default()),
            OperationType::Profile => OperationConfig::Profile(ProfileConfig::default()),
            OperationType::Adaptive => OperationConfig::Adaptive(AdaptiveConfig::default()),
            OperationType::VCarve => OperationConfig::VCarve(VCarveConfig::default()),
            OperationType::Rest => OperationConfig::Rest(RestConfig::default()),
            OperationType::Inlay => OperationConfig::Inlay(InlayConfig::default()),
            OperationType::Zigzag => OperationConfig::Zigzag(ZigzagConfig::default()),
            OperationType::Trace => OperationConfig::Trace(TraceConfig::default()),
            OperationType::Drill => OperationConfig::Drill(DrillConfig::default()),
            OperationType::Chamfer => OperationConfig::Chamfer(ChamferConfig::default()),
            OperationType::DropCutter => OperationConfig::DropCutter(DropCutterConfig::default()),
            OperationType::Adaptive3d => OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            OperationType::Waterline => OperationConfig::Waterline(WaterlineConfig::default()),
            OperationType::Pencil => OperationConfig::Pencil(PencilConfig::default()),
            OperationType::Scallop => OperationConfig::Scallop(ScallopConfig::default()),
            OperationType::SteepShallow => {
                OperationConfig::SteepShallow(SteepShallowConfig::default())
            }
            OperationType::RampFinish => OperationConfig::RampFinish(RampFinishConfig::default()),
            OperationType::SpiralFinish => {
                OperationConfig::SpiralFinish(SpiralFinishConfig::default())
            }
            OperationType::RadialFinish => {
                OperationConfig::RadialFinish(RadialFinishConfig::default())
            }
            OperationType::HorizontalFinish => {
                OperationConfig::HorizontalFinish(HorizontalFinishConfig::default())
            }
            OperationType::ProjectCurve => {
                OperationConfig::ProjectCurve(ProjectCurveConfig::default())
            }
            OperationType::AlignmentPinDrill => {
                OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default())
            }
        }
    }
}

pub fn feed_optimization_unavailable_reason(
    operation: &OperationConfig,
    stock_source: StockSource,
) -> Option<&'static str> {
    if stock_source == StockSource::FromRemainingStock {
        return Some(
            "Phase 1 feed optimization only supports fresh stock, not remaining-stock workflows.",
        );
    }
    if matches!(operation, OperationConfig::Rest(_)) {
        return Some(
            "Rest machining depends on prior tool removal, so feed optimization is disabled for now.",
        );
    }
    if operation.is_3d() {
        return Some(
            "Phase 1 feed optimization only supports operations that start from flat stock, not mesh-derived surfaces.",
        );
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn operation_catalog_is_exhaustive_and_consistent() {
        assert_eq!(OperationType::ALL.len(), 23);
        for &op_type in OperationType::ALL {
            let config = OperationConfig::new_default(op_type);
            assert_eq!(config.op_type(), op_type);
            assert_eq!(config.label(), op_type.label());
        }
    }

    #[test]
    fn operation_partitions_cover_all_variants_once() {
        use std::collections::HashSet;
        let all: HashSet<_> = OperationType::ALL.iter().collect();
        let twod: HashSet<_> = OperationType::ALL_2D.iter().collect();
        let threed: HashSet<_> = OperationType::ALL_3D.iter().collect();
        // 2D and 3D must not overlap
        assert!(twod.is_disjoint(&threed), "ALL_2D and ALL_3D overlap");
        // Both must be subsets of ALL
        assert!(twod.is_subset(&all), "ALL_2D contains items not in ALL");
        assert!(threed.is_subset(&all), "ALL_3D contains items not in ALL");
        // System-only ops (not in user menus) are the difference
        let menu_ops: HashSet<_> = twod.union(&threed).collect();
        let system_only = all.len() - menu_ops.len();
        assert!(
            system_only > 0,
            "Expected at least AlignmentPinDrill as system-only"
        );
    }

    #[test]
    fn depthless_finishing_ops_resolve_to_none() {
        for op in [
            OperationConfig::Pencil(PencilConfig::default()),
            OperationConfig::Scallop(ScallopConfig::default()),
            OperationConfig::SteepShallow(SteepShallowConfig::default()),
            OperationConfig::RampFinish(RampFinishConfig::default()),
            OperationConfig::SpiralFinish(SpiralFinishConfig::default()),
            OperationConfig::RadialFinish(RadialFinishConfig::default()),
            OperationConfig::HorizontalFinish(HorizontalFinishConfig::default()),
        ] {
            assert!(matches!(op.depth_semantics(), DepthSemantics::None));
            assert_eq!(op.default_depth_for_heights(), 0.0);
        }
    }
}
