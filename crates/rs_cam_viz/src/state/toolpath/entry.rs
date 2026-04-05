use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::enriched_mesh::FaceGroupId;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::toolpath::Toolpath;

use crate::state::job::{ModelId, ToolId};

use super::catalog::OperationConfig;
use super::support::{
    BoundaryContainment, ComputeStatus, DressupConfig, FeedsAutoMode, HeightsConfig, StockSource,
    ToolpathId, ToolpathStats,
};

#[derive(Debug, Clone)]
pub struct ToolpathEntryInit {
    pub id: ToolpathId,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub locked: bool,
    pub tool_id: ToolId,
    pub model_id: ModelId,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub heights: HeightsConfig,
    pub boundary_enabled: bool,
    pub boundary_containment: BoundaryContainment,
    pub coolant: CoolantMode,
    pub pre_gcode: String,
    pub post_gcode: String,
    pub stock_source: StockSource,
    pub auto_regen: Option<bool>,
    pub feeds_auto: FeedsAutoMode,
    pub face_selection: Option<Vec<FaceGroupId>>,
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
}

impl ToolpathEntryInit {
    pub fn new(
        id: ToolpathId,
        name: String,
        tool_id: ToolId,
        model_id: ModelId,
        operation: OperationConfig,
    ) -> Self {
        Self {
            id,
            name,
            enabled: true,
            visible: true,
            locked: false,
            tool_id,
            model_id,
            operation,
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            boundary_enabled: false,
            boundary_containment: BoundaryContainment::Center,
            coolant: CoolantMode::Off,
            pre_gcode: String::new(),
            post_gcode: String::new(),
            stock_source: StockSource::Fresh,
            auto_regen: None,
            feeds_auto: FeedsAutoMode::default(),
            face_selection: None,
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
        }
    }

    pub fn new_toolpath(
        id: ToolpathId,
        name: String,
        tool_id: ToolId,
        model_id: ModelId,
        op_type: super::catalog::OperationType,
    ) -> Self {
        Self::new(
            id,
            name,
            tool_id,
            model_id,
            OperationConfig::new_default(op_type),
        )
    }

    pub fn from_loaded_state(
        id: ToolpathId,
        name: String,
        tool_id: ToolId,
        model_id: ModelId,
        operation: OperationConfig,
    ) -> Self {
        Self::new(id, name, tool_id, model_id, operation)
    }

    pub fn duplicate_from(source: &ToolpathEntry, new_id: ToolpathId, new_name: String) -> Self {
        Self {
            id: new_id,
            name: new_name,
            enabled: source.enabled,
            visible: source.visible,
            locked: source.locked,
            tool_id: source.tool_id,
            model_id: source.model_id,
            operation: source.operation.clone(),
            dressups: source.dressups.clone(),
            heights: source.heights.clone(),
            boundary_enabled: source.boundary_enabled,
            boundary_containment: source.boundary_containment,
            coolant: source.coolant,
            pre_gcode: source.pre_gcode.clone(),
            post_gcode: source.post_gcode.clone(),
            stock_source: source.stock_source,
            auto_regen: Some(source.auto_regen),
            feeds_auto: source.feeds_auto.clone(),
            face_selection: source.face_selection.clone(),
            debug_options: source.debug_options,
        }
    }
}

pub struct ToolpathEntry {
    pub id: ToolpathId,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub locked: bool,
    pub tool_id: ToolId,
    pub model_id: ModelId,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub heights: HeightsConfig,
    pub boundary_enabled: bool,
    pub boundary_containment: BoundaryContainment,
    pub coolant: CoolantMode,
    pub pre_gcode: String,
    pub post_gcode: String,
    pub stock_source: StockSource,
    pub status: ComputeStatus,
    pub result: Option<ToolpathResult>,
    pub stale_since: Option<std::time::Instant>,
    pub auto_regen: bool,
    pub feeds_auto: FeedsAutoMode,
    pub face_selection: Option<Vec<FaceGroupId>>,
    pub feeds_result: Option<rs_cam_core::feeds::FeedsResult>,
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

pub struct ToolpathResult {
    pub toolpath: Arc<Toolpath>,
    pub stats: ToolpathStats,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

impl ToolpathEntry {
    pub fn from_init(init: ToolpathEntryInit) -> Self {
        let auto_regen = init
            .auto_regen
            .unwrap_or_else(|| init.operation.default_auto_regen());
        Self {
            id: init.id,
            name: init.name,
            enabled: init.enabled,
            visible: init.visible,
            locked: init.locked,
            tool_id: init.tool_id,
            model_id: init.model_id,
            operation: init.operation,
            dressups: init.dressups,
            heights: init.heights,
            boundary_enabled: init.boundary_enabled,
            boundary_containment: init.boundary_containment,
            coolant: init.coolant,
            pre_gcode: init.pre_gcode,
            post_gcode: init.post_gcode,
            stock_source: init.stock_source,
            status: ComputeStatus::Pending,
            result: None,
            stale_since: None,
            auto_regen,
            feeds_auto: init.feeds_auto,
            face_selection: init.face_selection,
            feeds_result: None,
            debug_options: init.debug_options,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        }
    }

    pub fn new(
        id: ToolpathId,
        name: String,
        tool_id: ToolId,
        model_id: ModelId,
        operation: OperationConfig,
    ) -> Self {
        Self::from_init(ToolpathEntryInit::new(
            id, name, tool_id, model_id, operation,
        ))
    }

    pub fn for_operation(
        id: ToolpathId,
        name: String,
        tool_id: ToolId,
        model_id: ModelId,
        op_type: super::catalog::OperationType,
    ) -> Self {
        Self::from_init(ToolpathEntryInit::new_toolpath(
            id, name, tool_id, model_id, op_type,
        ))
    }

    pub fn duplicate_as(&self, new_id: ToolpathId, new_name: String) -> Self {
        Self::from_init(ToolpathEntryInit::duplicate_from(self, new_id, new_name))
    }

    pub fn clear_runtime_state(&mut self) {
        self.status = ComputeStatus::Pending;
        self.result = None;
        self.stale_since = None;
        self.feeds_result = None;
        self.debug_trace = None;
        self.semantic_trace = None;
        self.debug_trace_path = None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::state::job::{ModelId, ToolId};
    use crate::state::toolpath::{
        Adaptive3dConfig, OperationConfig, OperationType, StockSource, ToolpathStats,
    };

    #[test]
    fn new_toolpath_uses_operation_defaults() {
        let entry = ToolpathEntry::for_operation(
            ToolpathId(1),
            "Pocket".to_owned(),
            ToolId(1),
            ModelId(1),
            OperationType::Pocket,
        );
        assert!(entry.auto_regen);
        assert!(matches!(entry.operation, OperationConfig::Pocket(_)));
    }

    #[test]
    fn duplicate_preserves_editable_state_but_not_runtime() {
        let mut source = ToolpathEntry::for_operation(
            ToolpathId(1),
            "Adaptive 3D".to_owned(),
            ToolId(1),
            ModelId(2),
            OperationType::Adaptive3d,
        );
        source.enabled = false;
        source.stock_source = StockSource::FromRemainingStock;
        source.status = ComputeStatus::Done;
        source.result = Some(ToolpathResult {
            toolpath: Arc::new(Toolpath::new()),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        });

        let duplicate = source.duplicate_as(ToolpathId(9), "Adaptive 3D Copy".to_owned());
        assert_eq!(duplicate.id, ToolpathId(9));
        assert!(!duplicate.enabled);
        assert_eq!(duplicate.stock_source, StockSource::FromRemainingStock);
        assert!(matches!(
            duplicate.operation,
            OperationConfig::Adaptive3d(Adaptive3dConfig { .. })
        ));
        assert!(matches!(duplicate.status, ComputeStatus::Pending));
        assert!(duplicate.result.is_none());
    }

    #[test]
    fn loaded_init_can_override_auto_regen_and_runtime_reset() {
        let mut init = ToolpathEntryInit::from_loaded_state(
            ToolpathId(4),
            "Loaded".to_owned(),
            ToolId(2),
            ModelId(3),
            OperationConfig::new_default(OperationType::DropCutter),
        );
        init.auto_regen = Some(true);
        let mut entry = ToolpathEntry::from_init(init);
        assert!(entry.auto_regen);

        entry.status = ComputeStatus::Error("x".to_owned());
        entry.result = Some(ToolpathResult {
            toolpath: Arc::new(Toolpath::new()),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        });
        let recorder = rs_cam_core::debug_trace::ToolpathDebugRecorder::new("Loaded", "DropCutter");
        let trace = Arc::new(recorder.finish());
        entry.debug_trace = Some(trace);
        entry.debug_trace_path = Some(std::env::temp_dir().join("loaded_trace.json"));
        entry.stale_since = Some(std::time::Instant::now());
        entry.clear_runtime_state();
        assert!(matches!(entry.status, ComputeStatus::Pending));
        assert!(entry.result.is_none());
        assert!(entry.feeds_result.is_none());
        assert!(entry.debug_trace.is_none());
        assert!(entry.debug_trace_path.is_none());
        assert!(entry.stale_since.is_none());
    }
}
