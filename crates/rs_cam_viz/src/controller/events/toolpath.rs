use crate::compute::ComputeBackend;
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::state::toolpath::{OperationConfig, ToolpathId};
use crate::ui::AppEvent;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Toolpath helpers ─────────────────────────────────────────────────

    pub(crate) fn handle_add_toolpath(&mut self, op_type: crate::state::toolpath::OperationType) {
        let target_setup_idx = match self.state.selection {
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(tp_id).and_then(|sid| {
                self.state
                    .session
                    .list_setups()
                    .iter()
                    .position(|s| s.id == sid.0)
            }),
            Selection::Setup(setup_id) => self
                .state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id.0),
            Selection::Fixture(setup_id, _) => self
                .state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id.0),
            Selection::KeepOut(setup_id, _) => self
                .state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id.0),
            _ => None,
        }
        .or(Some(0)); // default to first setup

        let Some(tool_id) = self.state.session.tools().first().map(|t| t.id.0) else {
            tracing::warn!("Cannot add toolpath: no tools defined");
            self.push_notification(
                "Cannot add toolpath: no tools defined".into(),
                super::super::Severity::Warning,
            );
            return;
        };
        // Roadmap B.1–B.3 — pull stock-aware depth defaults so e.g.
        // a fresh DropCutter starts with `min_z = stock_bottom_z`
        // instead of the hard-coded `-50.0` that clipped any stock
        // not sitting at exactly that depth.
        let stock_bbox = self.state.session.stock_bbox();
        let stock_padding = self.state.session.stock_config().padding;
        let ctx = rs_cam_core::compute::catalog::NewDefaultCtx::from_stock_bbox(
            stock_bbox,
            stock_padding,
        );
        let operation = OperationConfig::new_default_with_ctx(op_type, &ctx);
        // Capture is_3d before `operation` moves into the toolpath
        // config below — used by the boundary auto-enable (B.7).
        let op_is_3d = operation.is_3d();
        if !operation.is_stock_based() && self.state.session.models().is_empty() {
            let msg = format!(
                "Cannot add {} toolpath: import geometry first",
                operation.label()
            );
            tracing::warn!("{msg}");
            self.push_notification(msg, super::super::Severity::Warning);
            return;
        }
        let model_id = self
            .state
            .session
            .models()
            .first()
            .map(|m| m.id)
            .unwrap_or(0);

        let tc = rs_cam_core::session::ToolpathConfig {
            id: 0, // will be assigned by session
            name: format!(
                "{} {}",
                op_type.label(),
                self.state.session.toolpath_configs().len() + 1
            ),
            enabled: true,
            operation,
            dressups: crate::state::toolpath::DressupConfig::for_op(op_type),
            heights: crate::state::toolpath::HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            // Roadmap B.7 — for 3D ops on a 3D mesh model, default the
            // machining boundary to the model silhouette so the cutter
            // doesn't sweep over the whole stock area on small parts in
            // oversized stock. Falls back to the default (disabled) for
            // 2D / stock-based ops or when no mesh model is available.
            boundary: {
                let has_mesh = self
                    .state
                    .session
                    .models()
                    .iter()
                    .any(|m| m.mesh.is_some());
                if op_is_3d && has_mesh {
                    crate::state::toolpath::BoundaryConfig {
                        enabled: true,
                        source: crate::state::toolpath::BoundarySource::ModelSilhouette,
                        ..crate::state::toolpath::BoundaryConfig::default()
                    }
                } else {
                    crate::state::toolpath::BoundaryConfig::default()
                }
            },
            boundary_inherit: true,
            stock_source: crate::state::toolpath::StockSource::Fresh,
            coolant: rs_cam_core::gcode::CoolantMode::Off,
            face_selection: None,
            feeds_auto: crate::state::toolpath::FeedsAutoMode::default(),
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
        };

        if let Some(setup_idx) = target_setup_idx
            && let Ok(tp_idx) = self.state.session.add_toolpath(setup_idx, tc)
            && let Some(tc) = self.state.session.toolpath_configs().get(tp_idx)
        {
            let tp_id = ToolpathId(tc.id);
            // Create GUI runtime entry
            self.state.gui.toolpath_rt.insert(
                tc.id,
                crate::state::runtime::ToolpathRuntime::new(tc.operation.default_auto_regen()),
            );
            self.state.selection = Selection::Toolpath(tp_id);
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_duplicate_toolpath(&mut self, tp_id: ToolpathId) {
        let setup_idx = self.setup_of_toolpath(tp_id).and_then(|sid| {
            self.state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == sid.0)
        });

        // Build a new ToolpathConfig by reading fields from the source
        let dup = self
            .state
            .session
            .find_toolpath_config_by_id(tp_id.0)
            .map(|(_, src)| {
                rs_cam_core::session::ToolpathConfig {
                    id: 0, // will be assigned by session
                    name: format!("{} (copy)", src.name),
                    enabled: src.enabled,
                    operation: src.operation.clone(),
                    dressups: src.dressups.clone(),
                    heights: src.heights.clone(),
                    tool_id: src.tool_id,
                    model_id: src.model_id,
                    pre_gcode: src.pre_gcode.clone(),
                    post_gcode: src.post_gcode.clone(),
                    boundary: src.boundary.clone(),
                    boundary_inherit: src.boundary_inherit,
                    stock_source: src.stock_source,
                    coolant: src.coolant,
                    face_selection: src.face_selection.clone(),
                    feeds_auto: src.feeds_auto.clone(),
                    debug_options: src.debug_options,
                }
            });

        if let Some(tc) = dup {
            if let Some(setup_idx) = setup_idx
                && let Ok(tp_idx) = self.state.session.add_toolpath(setup_idx, tc)
                && let Some(new_tc) = self.state.session.toolpath_configs().get(tp_idx)
            {
                let new_id = ToolpathId(new_tc.id);
                self.state.gui.toolpath_rt.insert(
                    new_tc.id,
                    crate::state::runtime::ToolpathRuntime::new(
                        new_tc.operation.default_auto_regen(),
                    ),
                );
                self.state.selection = Selection::Toolpath(new_id);
            }
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_up(&mut self, tp_id: ToolpathId) {
        // Find the setup and local position of this toolpath
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0)
            && let Some(setup) = self
                .state
                .session
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices.contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices.iter().position(|&i| i == tp_idx)
            && local_pos > 0
        {
            // SAFETY: local_pos - 1 is valid since local_pos > 0
            #[allow(clippy::indexing_slicing)]
            let swap_with = setup.toolpath_indices[local_pos - 1];
            let _ = self.state.session.reorder_toolpath(tp_idx, swap_with);
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_down(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0)
            && let Some(setup) = self
                .state
                .session
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices.contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices.iter().position(|&i| i == tp_idx)
            && local_pos + 1 < setup.toolpath_indices.len()
            && let Some(&swap_with) = setup.toolpath_indices.get(local_pos + 1)
        {
            let _ = self.state.session.reorder_toolpath(tp_idx, swap_with);
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_reorder_toolpath(&mut self, tp_id: ToolpathId, target_idx: usize) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0)
            && let Some(setup) = self
                .state
                .session
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices.contains(&tp_idx))
        {
            let clamped = target_idx.min(setup.toolpath_indices.len().saturating_sub(1));
            if let Some(&target_global_idx) = setup.toolpath_indices.get(clamped)
                && tp_idx != target_global_idx
            {
                let _ = self
                    .state
                    .session
                    .reorder_toolpath(tp_idx, target_global_idx);
                self.state.gui.mark_edited();
            }
        }
    }

    pub(crate) fn handle_move_toolpath_to_setup(
        &mut self,
        tp_id: ToolpathId,
        setup_id: crate::state::job::SetupId,
        _idx: usize,
    ) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0) {
            // Resolve the target setup's index from its ID
            let target_idx = self
                .state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id.0);
            if let Some(target_setup_idx) = target_idx {
                let _ = self
                    .state
                    .session
                    .move_toolpath_to_setup(tp_idx, target_setup_idx);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_toolpath(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0) {
            let _ = self.state.session.remove_toolpath(tp_idx);
            self.state.gui.toolpath_rt.remove(&tp_id.0);
        }
        if self.state.selection == Selection::Toolpath(tp_id) {
            self.state.selection = Selection::None;
        }
        if self.state.viewport.isolate_toolpath == Some(tp_id) {
            self.state.viewport.isolate_toolpath = None;
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_generate_all(&mut self) {
        let ids: Vec<_> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .map(|tc| ToolpathId(tc.id))
            .collect();
        for id in ids {
            self.submit_toolpath_compute(id);
        }
    }

    pub(crate) fn handle_toggle_isolate_toolpath(&mut self) {
        if let Selection::Toolpath(id) = self.state.selection {
            if self.state.viewport.isolate_toolpath == Some(id) {
                self.state.viewport.isolate_toolpath = None;
            } else {
                self.state.viewport.isolate_toolpath = Some(id);
            }
            self.pending_upload = true;
        }
    }

    pub(crate) fn handle_inspect_toolpath_in_simulation(&mut self, tp_id: ToolpathId) {
        self.events
            .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
        if let Some(boundary) = self
            .state
            .simulation
            .boundaries()
            .iter()
            .find(|boundary| boundary.id == tp_id)
        {
            self.events
                .push(AppEvent::SimJumpToMove(boundary.start_move));
        } else {
            let has_result = self
                .state
                .gui
                .toolpath_rt
                .get(&tp_id.0)
                .and_then(|rt| rt.result.as_ref())
                .is_some();
            if has_result {
                self.state.simulation.debug.pending_inspect_toolpath = Some(tp_id);
                self.events.push(AppEvent::RunSimulationWith(vec![tp_id]));
            }
        }
    }
}
