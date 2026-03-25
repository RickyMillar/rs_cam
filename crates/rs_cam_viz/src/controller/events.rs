use std::sync::Arc;

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeError, ComputeLane, ComputeMessage, ComputeRequest,
    SetupSimGroup, SetupSimToolpath, SimulationRequest,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::BoundingBox3;

use crate::state::Workspace;
use crate::state::history::UndoAction;
use crate::state::job::{AlignmentPin, FaceUp, Fixture, FlipAxis, KeepOutZone, Setup, ToolConfig};
use crate::state::selection::Selection;
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
use crate::state::toolpath::{
    AlignmentPinDrillConfig, ComputeStatus, OperationConfig, StockSource, ToolpathEntry,
    ToolpathEntryInit, ToolpathId,
};
use crate::ui::AppEvent;

use super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub fn handle_internal_event(&mut self, event: AppEvent) {
        match event {
            // --- Import / model events ---
            AppEvent::ImportStl(path) => {
                if let Err(error) = self.import_stl_path(&path) {
                    self.push_error(&error);
                }
            }
            AppEvent::ImportSvg(path) => {
                if let Err(error) = self.import_svg_path(&path) {
                    self.push_error(&error);
                }
            }
            AppEvent::ImportDxf(path) => {
                if let Err(error) = self.import_dxf_path(&path) {
                    self.push_error(&error);
                }
            }
            // ImportStep is handled at the app level (camera fitting + status)
            AppEvent::ImportStep(_) => {}
            AppEvent::RescaleModel(model_id, units) => {
                if let Err(error) = self.rescale_model(model_id, units) {
                    self.push_error(&error);
                }
            }
            AppEvent::RemoveModel(model_id) => self.handle_remove_model(model_id),
            AppEvent::ReloadModel(model_id) => {
                if let Err(error) = self.reload_model(model_id) {
                    self.push_error(&error);
                }
            }

            // --- Tree / selection events ---
            AppEvent::Select(ref selection) => self.handle_select(selection),
            AppEvent::AddTool(tool_type) => self.handle_add_tool(tool_type),
            AppEvent::DuplicateTool(tool_id) => self.handle_duplicate_tool(tool_id),
            AppEvent::RemoveTool(tool_id) => self.handle_remove_tool(tool_id),
            AppEvent::AddSetup => self.handle_add_setup(),
            AppEvent::SetupTwoSided => self.handle_setup_two_sided(),
            AppEvent::RemoveSetup(setup_id) => self.handle_remove_setup(setup_id),
            AppEvent::RenameSetup(setup_id, name) => self.handle_rename_setup(setup_id, name),
            AppEvent::AddFixture(setup_id) => self.handle_add_fixture(setup_id),
            AppEvent::RemoveFixture(setup_id, fixture_id) => {
                self.handle_remove_fixture(setup_id, fixture_id)
            }
            AppEvent::AddKeepOut(setup_id) => self.handle_add_keep_out(setup_id),
            AppEvent::RemoveKeepOut(setup_id, keep_out_id) => {
                self.handle_remove_keep_out(setup_id, keep_out_id)
            }
            AppEvent::FixtureChanged => {
                self.pending_upload = true;
                self.state.job.mark_edited();
            }

            // --- Toolpath events ---
            AppEvent::AddToolpath(op_type) => self.handle_add_toolpath(op_type),
            AppEvent::DuplicateToolpath(tp_id) => self.handle_duplicate_toolpath(tp_id),
            AppEvent::MoveToolpathUp(tp_id) => self.handle_move_toolpath_up(tp_id),
            AppEvent::MoveToolpathDown(tp_id) => self.handle_move_toolpath_down(tp_id),
            AppEvent::ReorderToolpath(tp_id, target_idx) => {
                self.handle_reorder_toolpath(tp_id, target_idx)
            }
            AppEvent::MoveToolpathToSetup(tp_id, setup_id, idx) => {
                self.handle_move_toolpath_to_setup(tp_id, setup_id, idx)
            }
            AppEvent::ToggleToolpathEnabled(tp_id) => {
                if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                    toolpath.enabled = !toolpath.enabled;
                }
            }
            AppEvent::RemoveToolpath(tp_id) => self.handle_remove_toolpath(tp_id),
            AppEvent::GenerateToolpath(tp_id) => self.submit_toolpath_compute(tp_id),
            AppEvent::GenerateAll => self.handle_generate_all(),
            AppEvent::ToggleToolpathVisibility(tp_id) => {
                if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                    toolpath.visible = !toolpath.visible;
                    self.pending_upload = true;
                }
            }
            AppEvent::ToggleIsolateToolpath => self.handle_toggle_isolate_toolpath(),
            AppEvent::InspectToolpathInSimulation(tp_id) => {
                self.handle_inspect_toolpath_in_simulation(tp_id)
            }

            // --- Simulation events ---
            AppEvent::RunSimulation => self.run_simulation_with_all(),
            AppEvent::RunSimulationWith(ids) => self.run_simulation_with_ids(&ids),
            AppEvent::ToggleSimPlayback => {
                self.state.simulation.playback.playing = !self.state.simulation.playback.playing;
            }
            AppEvent::ResetSimulation => self.handle_reset_simulation(),
            AppEvent::SimJumpToMove(move_idx) => self.handle_sim_jump_to_move(move_idx),
            AppEvent::SimStepForward => self.handle_sim_step_forward(),
            AppEvent::SimStepBackward => self.handle_sim_step_backward(),
            AppEvent::SimJumpToStart => self.handle_sim_jump_to_start(),
            AppEvent::SimJumpToEnd => self.handle_sim_jump_to_end(),
            AppEvent::SimJumpToOpStart(idx) => self.handle_sim_jump_to_op_start(idx),
            AppEvent::SimJumpToOpEnd(idx) => self.handle_sim_jump_to_op_end(idx),

            // --- Compute / check events ---
            AppEvent::RunCollisionCheck => self.request_collision_check(),
            AppEvent::CancelCompute => self.compute.cancel_all(),

            // --- Face selection ---
            AppEvent::ToggleFaceSelection {
                toolpath_id,
                model_id: _,
                face_id,
            } => {
                if let Some(entry) = self.state.job.find_toolpath_mut(toolpath_id) {
                    let faces = entry.face_selection.get_or_insert_with(Vec::new);
                    if let Some(pos) = faces.iter().position(|f| *f == face_id) {
                        faces.remove(pos);
                    } else {
                        faces.push(face_id);
                    }
                    if faces.is_empty() {
                        entry.face_selection = None;
                    }
                    // Keep toolpath selected so properties pane stays visible.
                    // Face highlighting is driven by the toolpath's face_selection,
                    // not the visual Selection enum.
                    self.state.selection = Selection::Toolpath(toolpath_id);
                    entry.stale_since = Some(std::time::Instant::now());
                    self.state.job.dirty = true;
                    self.pending_upload = true;
                }
            }

            // --- Undo / redo ---
            AppEvent::Undo => self.undo(),
            AppEvent::Redo => self.redo(),

            // --- Stock / machine events ---
            AppEvent::StockChanged => self.handle_stock_changed(),
            AppEvent::StockMaterialChanged => {
                self.state.job.mark_edited();
            }
            AppEvent::MachineChanged => {
                self.state.job.mark_edited();
            }

            // --- Pass-through events handled elsewhere ---
            AppEvent::ExportGcode
            | AppEvent::ExportCombinedGcode
            | AppEvent::ExportSetupGcode(_)
            | AppEvent::ExportGcodeConfirmed
            | AppEvent::ExportSetupSheet
            | AppEvent::ExportSvgPreview
            | AppEvent::SaveJob
            | AppEvent::OpenJob
            | AppEvent::SetViewPreset(_)
            | AppEvent::PreviewOrientation(_)
            | AppEvent::ResetView
            | AppEvent::SwitchWorkspace(_)
            | AppEvent::SimVizModeChanged
            | AppEvent::ShowShortcuts
            | AppEvent::Quit => {}
        }
    }

    // ── Tree / selection helpers ─────────────────────────────────────────

    fn handle_select(&mut self, selection: &Selection) {
        let old_setup = match &self.state.selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(*tp_id),
            _ => None,
        };
        let new_setup = match selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(*tp_id),
            _ => None,
        };
        if old_setup != new_setup {
            self.pending_upload = true;
        }
        self.state.selection = selection.clone();
    }

    fn handle_add_tool(&mut self, tool_type: crate::state::job::ToolType) {
        let id = self.state.job.next_tool_id();
        let tool = ToolConfig::new_default(id, tool_type);
        self.state.selection = Selection::Tool(id);
        self.state.job.tools.push(tool);
        self.state.job.mark_edited();
    }

    fn handle_duplicate_tool(&mut self, tool_id: crate::state::job::ToolId) {
        if let Some(src) = self.state.job.tools.iter().find(|tool| tool.id == tool_id) {
            let mut duplicate = src.clone();
            let new_id = self.state.job.next_tool_id();
            duplicate.id = new_id;
            duplicate.name = format!("{} (copy)", duplicate.name);
            self.state.selection = Selection::Tool(new_id);
            self.state.job.tools.push(duplicate);
            self.state.job.mark_edited();
        }
    }

    fn handle_remove_tool(&mut self, tool_id: crate::state::job::ToolId) {
        let in_use = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .any(|entry| entry.tool_id == tool_id);
        if in_use {
            tracing::warn!(
                "Cannot remove tool {:?}: still referenced by one or more toolpaths",
                tool_id
            );
            self.push_notification(
                "Cannot remove tool: still referenced by one or more toolpaths".into(),
                super::Severity::Warning,
            );
        } else {
            self.state.job.tools.retain(|tool| tool.id != tool_id);
            if self.state.selection == Selection::Tool(tool_id) {
                self.state.selection = Selection::None;
            }
            self.state.job.mark_edited();
        }
    }

    fn handle_add_setup(&mut self) {
        let id = self.state.job.next_setup_id();
        let name = format!("Setup {}", id.0 + 1);
        self.state.job.setups.push(Setup::new(id, name));
        self.state.selection = Selection::Setup(id);
        self.state.job.mark_edited();
    }

    fn handle_setup_two_sided(&mut self) {
        let has_flipped = self
            .state
            .job
            .setups
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let id = self.state.job.next_setup_id();
            let mut setup = Setup::new(id, format!("Setup {}", id.0 + 1));
            setup.face_up = FaceUp::Bottom;
            self.state.job.setups.push(setup);
        }
        if self.state.job.stock.flip_axis.is_none() {
            self.state.job.stock.flip_axis = Some(FlipAxis::Horizontal);
        }
        if self.state.job.stock.alignment_pins.is_empty() {
            let margin = if self.state.job.stock.padding > 2.0 {
                self.state.job.stock.padding / 2.0
            } else {
                10.0_f64
                    .min(self.state.job.stock.x / 4.0)
                    .min(self.state.job.stock.y / 4.0)
            };
            let cy = self.state.job.stock.y / 2.0;
            self.state
                .job
                .stock
                .alignment_pins
                .push(AlignmentPin::new(margin, cy, 6.0));
            self.state.job.stock.alignment_pins.push(AlignmentPin::new(
                self.state.job.stock.x - margin,
                cy,
                6.0,
            ));
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
        self.sync_alignment_pin_drill();
        self.state.selection = Selection::Stock;
    }

    fn handle_remove_setup(&mut self, setup_id: crate::state::job::SetupId) {
        if self.state.job.setups.len() > 1 {
            self.state.job.setups.retain(|setup| setup.id != setup_id);
            match self.state.selection {
                Selection::Setup(id) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                Selection::Fixture(id, _) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                Selection::KeepOut(id, _) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                _ => {}
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    fn handle_rename_setup(&mut self, setup_id: crate::state::job::SetupId, name: String) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.name = name;
            self.state.job.mark_edited();
        }
    }

    fn handle_add_fixture(&mut self, setup_id: crate::state::job::SetupId) {
        let fixture_id = self.state.job.next_fixture_id();
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.fixtures.push(Fixture::new_default(fixture_id));
            self.state.selection = Selection::Fixture(setup_id, fixture_id);
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    fn handle_remove_fixture(
        &mut self,
        setup_id: crate::state::job::SetupId,
        fixture_id: crate::state::job::FixtureId,
    ) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.fixtures.retain(|fixture| fixture.id != fixture_id);
            if self.state.selection == Selection::Fixture(setup_id, fixture_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    fn handle_add_keep_out(&mut self, setup_id: crate::state::job::SetupId) {
        let keep_out_id = self.state.job.next_keep_out_id();
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup
                .keep_out_zones
                .push(KeepOutZone::new_default(keep_out_id));
            self.state.selection = Selection::KeepOut(setup_id, keep_out_id);
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    fn handle_remove_keep_out(
        &mut self,
        setup_id: crate::state::job::SetupId,
        keep_out_id: crate::state::job::KeepOutId,
    ) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup
                .keep_out_zones
                .retain(|keep_out| keep_out.id != keep_out_id);
            if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    // ── Model helpers ────────────────────────────────────────────────────

    fn handle_remove_model(&mut self, model_id: crate::state::job::ModelId) {
        // Check if any toolpath references this model
        let in_use = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .any(|entry| entry.model_id == model_id);
        if in_use {
            tracing::warn!(
                "Cannot remove model {:?}: still referenced by one or more toolpaths",
                model_id
            );
            self.push_notification(
                "Cannot remove model: still referenced by one or more toolpaths".into(),
                super::Severity::Warning,
            );
        } else {
            self.state.job.models.retain(|model| model.id != model_id);
            let clear_selection = matches!(
                self.state.selection,
                Selection::Model(mid) | Selection::Face(mid, _) | Selection::Faces(mid, _)
                    if mid == model_id
            );
            if clear_selection {
                self.state.selection = Selection::None;
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    // ── Toolpath helpers ─────────────────────────────────────────────────

    fn handle_add_toolpath(&mut self, op_type: crate::state::toolpath::OperationType) {
        let target_setup_id = match self.state.selection {
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(tp_id),
            Selection::Setup(setup_id) => Some(setup_id),
            Selection::Fixture(setup_id, _) => Some(setup_id),
            Selection::KeepOut(setup_id, _) => Some(setup_id),
            _ => None,
        }
        .or_else(|| self.state.job.setups.first().map(|setup| setup.id));

        let Some(tool_id) = self.state.job.tools.first().map(|tool| tool.id) else {
            tracing::warn!("Cannot add toolpath: no tools defined");
            self.push_notification(
                "Cannot add toolpath: no tools defined".into(),
                super::Severity::Warning,
            );
            return;
        };
        let operation = OperationConfig::new_default(op_type);
        if !operation.is_stock_based() && self.state.job.models.is_empty() {
            let msg = format!(
                "Cannot add {} toolpath: import geometry first",
                operation.label()
            );
            tracing::warn!("{msg}");
            self.push_notification(msg, super::Severity::Warning);
            return;
        }
        let id = self.state.job.next_toolpath_id();
        let model_id = self
            .state
            .job
            .models
            .first()
            .map(|model| model.id)
            .unwrap_or(crate::state::job::ModelId(0));
        let entry = ToolpathEntry::new(
            id,
            format!("{} {}", op_type.label(), id.0 + 1),
            tool_id,
            model_id,
            operation,
        );
        self.state.selection = Selection::Toolpath(id);
        if let Some(setup_id) = target_setup_id {
            self.state.job.push_toolpath_to_setup(setup_id, entry);
        } else {
            self.state.job.push_toolpath(entry);
        }
        self.state.job.mark_edited();
    }

    fn handle_duplicate_toolpath(&mut self, tp_id: ToolpathId) {
        let new_id = self.state.job.next_toolpath_id();
        let target_setup_id = self.state.job.setup_of_toolpath(tp_id);
        if let Some(src) = self.state.job.find_toolpath(tp_id) {
            self.state.selection = Selection::Toolpath(new_id);
            let entry = src.duplicate_as(new_id, format!("{} (copy)", src.name));
            if let Some(setup_id) = target_setup_id {
                self.state.job.push_toolpath_to_setup(setup_id, entry);
            } else {
                self.state.job.push_toolpath(entry);
            }
            self.state.job.mark_edited();
        }
    }

    fn handle_move_toolpath_up(&mut self, tp_id: ToolpathId) {
        if self.state.job.move_toolpath_up(tp_id) {
            self.state.job.mark_edited();
        }
    }

    fn handle_move_toolpath_down(&mut self, tp_id: ToolpathId) {
        if self.state.job.move_toolpath_down(tp_id) {
            self.state.job.mark_edited();
        }
    }

    fn handle_reorder_toolpath(&mut self, tp_id: ToolpathId, target_idx: usize) {
        if self.state.job.reorder_toolpath(tp_id, target_idx) {
            self.state.job.mark_edited();
        }
    }

    fn handle_move_toolpath_to_setup(
        &mut self,
        tp_id: ToolpathId,
        setup_id: crate::state::job::SetupId,
        idx: usize,
    ) {
        if self.state.job.move_toolpath_to_setup(tp_id, setup_id, idx) {
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    fn handle_remove_toolpath(&mut self, tp_id: ToolpathId) {
        self.state.job.remove_toolpath(tp_id);
        if self.state.selection == Selection::Toolpath(tp_id) {
            self.state.selection = Selection::None;
        }
        if self.state.viewport.isolate_toolpath == Some(tp_id) {
            self.state.viewport.isolate_toolpath = None;
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
    }

    fn handle_generate_all(&mut self) {
        let ids: Vec<_> = self
            .state
            .job
            .all_toolpaths()
            .map(|toolpath| toolpath.id)
            .collect();
        for id in ids {
            self.submit_toolpath_compute(id);
        }
    }

    fn handle_toggle_isolate_toolpath(&mut self) {
        if let Selection::Toolpath(id) = self.state.selection {
            if self.state.viewport.isolate_toolpath == Some(id) {
                self.state.viewport.isolate_toolpath = None;
            } else {
                self.state.viewport.isolate_toolpath = Some(id);
            }
            self.pending_upload = true;
        }
    }

    fn handle_inspect_toolpath_in_simulation(&mut self, tp_id: ToolpathId) {
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
        } else if self
            .state
            .job
            .find_toolpath(tp_id)
            .and_then(|toolpath| toolpath.result.as_ref())
            .is_some()
        {
            self.state.simulation.debug.pending_inspect_toolpath = Some(tp_id);
            self.events.push(AppEvent::RunSimulationWith(vec![tp_id]));
        }
    }

    // ── Simulation helpers ───────────────────────────────────────────────

    fn handle_reset_simulation(&mut self) {
        self.compute.cancel_lane(ComputeLane::Analysis);
        let sim = &mut self.state.simulation;
        sim.results = None;
        sim.playback = Default::default();
        sim.checks = Default::default();
        sim.last_run = None;
        self.collision_positions.clear();
        self.pending_upload = true;
    }

    fn handle_sim_jump_to_move(&mut self, move_idx: usize) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = move_idx.min(total);
        }
    }

    fn handle_sim_step_forward(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = (pb.current_move + 1).min(total);
        }
    }

    fn handle_sim_step_backward(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = pb.current_move.saturating_sub(1);
        }
    }

    fn handle_sim_jump_to_start(&mut self) {
        if self.state.simulation.has_results() {
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = 0;
        }
    }

    fn handle_sim_jump_to_end(&mut self) {
        if self.state.simulation.has_results() {
            let total = self.state.simulation.total_moves();
            let pb = &mut self.state.simulation.playback;
            pb.playing = false;
            pb.current_move = total;
        }
    }

    fn handle_sim_jump_to_op_start(&mut self, boundary_idx: usize) {
        if let Some(start) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.start_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = start;
        }
    }

    fn handle_sim_jump_to_op_end(&mut self, boundary_idx: usize) {
        if let Some(end) = self
            .state
            .simulation
            .boundaries()
            .get(boundary_idx)
            .map(|b| b.end_move)
        {
            self.state.simulation.playback.playing = false;
            self.state.simulation.playback.current_move = end;
        }
    }

    // ── Stock / config helpers ───────────────────────────────────────────

    fn handle_stock_changed(&mut self) {
        if self.state.job.stock.auto_from_model
            && let Some(bbox) = self
                .state
                .job
                .models
                .iter()
                .find_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox))
        {
            self.state.job.stock.update_from_bbox(&bbox);
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
        self.sync_alignment_pin_drill();
    }

    /// Build per-setup simulation groups by applying a per-setup toolpath filter.
    /// Returns `(groups, all_toolpaths_flat, stock_bbox)` or `None` if no
    /// toolpaths matched.
    fn build_simulation_groups(
        &self,
        mut include_toolpath: impl FnMut(usize, &ToolpathEntry) -> bool,
        mut stop_after_setup: impl FnMut(usize) -> bool,
    ) -> Option<(Vec<SetupSimGroup>, Vec<SetupSimToolpath>, BoundingBox3)> {
        let stock = &self.state.job.stock;
        let stock_bbox = BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
        };

        let mut groups: Vec<SetupSimGroup> = Vec::new();
        let mut all_toolpaths_flat = Vec::new();

        for (i, setup) in self.state.job.setups.iter().enumerate() {
            let direction = face_up_to_direction(setup.face_up);
            let toolpaths: Vec<_> = setup
                .toolpaths
                .iter()
                .filter(|tp| include_toolpath(i, tp))
                .filter_map(|tp| {
                    let result = tp.result.as_ref()?;
                    let tool = self
                        .state
                        .job
                        .tools
                        .iter()
                        .find(|t| t.id == tp.tool_id)?
                        .clone();
                    let transformed = if setup.needs_transform() {
                        Arc::new(transform_toolpath_to_stock_frame(
                            &result.toolpath,
                            setup,
                            stock,
                        ))
                    } else {
                        Arc::clone(&result.toolpath)
                    };
                    Some(SetupSimToolpath {
                        id: tp.id,
                        name: tp.name.clone(),
                        toolpath: transformed,
                        tool,
                        semantic_trace: tp.semantic_trace.clone(),
                    })
                })
                .collect();

            if !toolpaths.is_empty() {
                all_toolpaths_flat.extend(toolpaths.clone());
                groups.push(SetupSimGroup {
                    toolpaths,
                    direction,
                });
            }

            if stop_after_setup(i) {
                break;
            }
        }

        if groups.is_empty() {
            return None;
        }
        Some((groups, all_toolpaths_flat, stock_bbox))
    }

    /// Submit a simulation request, handling auto-resolution and model mesh.
    fn submit_simulation_for_groups(
        &mut self,
        groups: Vec<SetupSimGroup>,
        all_toolpaths_flat: &[SetupSimToolpath],
        stock_bbox: BoundingBox3,
        _model_setup_idx: Option<usize>,
    ) {
        if self.state.simulation.auto_resolution {
            self.state.simulation.resolution =
                auto_resolution_for_tools(all_toolpaths_flat, &stock_bbox);
        }

        // Pass model mesh in world coordinates — same frame as the dexel stock mesh.
        let model_mesh = self.state.job.models.iter().find_map(|m| m.mesh.clone());

        self.compute.submit_simulation(SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution: self.state.simulation.resolution,
            metric_options: self.state.simulation.metric_options,
            spindle_rpm: self.state.job.post.spindle_speed,
            rapid_feed_mm_min: if self.state.job.post.high_feedrate_mode {
                self.state.job.post.high_feedrate.max(1.0)
            } else {
                self.state.job.machine.max_feed_mm_min.max(1.0)
            },
            model_mesh,
        });
    }

    pub fn run_simulation_with_all(&mut self) {
        let Some((groups, all_toolpaths_flat, stock_bbox)) = self.build_simulation_groups(
            |_setup_idx, tp| tp.enabled,
            |_setup_idx| false, // never stop early
        ) else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::Severity::Warning,
            );
            return;
        };
        self.submit_simulation_for_groups(groups, &all_toolpaths_flat, stock_bbox, Some(0));
    }

    pub fn run_simulation_with_ids(&mut self, ids: &[ToolpathId]) {
        let target_setup_idx = self
            .state
            .job
            .setups
            .iter()
            .position(|s| s.toolpaths.iter().any(|tp| ids.contains(&tp.id)));
        let Some(target_setup_idx) = target_setup_idx else {
            tracing::warn!("No computed toolpaths to simulate");
            self.push_notification(
                "No computed toolpaths to simulate".into(),
                super::Severity::Warning,
            );
            return;
        };

        let Some((groups, all_toolpaths_flat, stock_bbox)) = self.build_simulation_groups(
            |setup_idx, tp| {
                if setup_idx == target_setup_idx {
                    ids.contains(&tp.id)
                } else if setup_idx < target_setup_idx {
                    tp.enabled
                } else {
                    false
                }
            },
            |setup_idx| setup_idx == target_setup_idx,
        ) else {
            return;
        };
        self.submit_simulation_for_groups(
            groups,
            &all_toolpaths_flat,
            stock_bbox,
            Some(target_setup_idx),
        );
    }

    pub fn request_collision_check(&mut self) {
        let toolpath_data = self.state.job.all_toolpaths().find_map(|toolpath| {
            let result = toolpath.result.as_ref()?;
            let tool = self
                .state
                .job
                .tools
                .iter()
                .find(|tool| tool.id == toolpath.tool_id)?
                .clone();
            let mesh = self
                .state
                .job
                .models
                .iter()
                .find(|model| model.id == toolpath.model_id)
                .and_then(|model| model.mesh.clone())?;
            Some((Arc::clone(&result.toolpath), tool, mesh))
        });

        if let Some((toolpath, tool, mesh)) = toolpath_data {
            self.compute.submit_collision(CollisionRequest {
                toolpath,
                tool,
                mesh,
            });
        } else {
            tracing::warn!("No toolpath with STL mesh available for collision check");
            self.push_notification(
                "No toolpath with STL mesh available for collision check".into(),
                super::Severity::Warning,
            );
        }
    }

    /// Create, update, or remove the auto-generated alignment pin drill toolpath.
    fn sync_alignment_pin_drill(&mut self) {
        let has_pins = !self.state.job.stock.alignment_pins.is_empty();

        // Find existing pin drill toolpath across all setups.
        let existing = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|s| s.toolpaths.iter().map(move |tp| (s.id, tp)))
            .find(|(_, tp)| matches!(tp.operation, OperationConfig::AlignmentPinDrill(_)))
            .map(|(sid, tp)| (sid, tp.id));

        if has_pins && existing.is_none() {
            // Auto-create in Setup 1 at index 0, but only if a tool exists.
            let first_tool_id = self.state.job.tools.first().map(|t| t.id);
            if let (Some(setup), Some(tool_id)) = (self.state.job.setups.first(), first_tool_id) {
                let setup_id = setup.id;
                let id = self.state.job.next_toolpath_id();
                let model_id = self
                    .state
                    .job
                    .models
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(crate::state::job::ModelId(0));
                let holes: Vec<[f64; 2]> = self
                    .state
                    .job
                    .stock
                    .alignment_pins
                    .iter()
                    .map(|p| [p.x, p.y])
                    .collect();
                let cfg = AlignmentPinDrillConfig {
                    holes,
                    ..Default::default()
                };
                let entry = ToolpathEntry::from_init(ToolpathEntryInit::new(
                    id,
                    "Pin Drill".to_string(),
                    tool_id,
                    model_id,
                    OperationConfig::AlignmentPinDrill(cfg),
                ));
                // Insert at index 0 (first operation in setup).
                if let Some(setup) = self.state.job.setups.iter_mut().find(|s| s.id == setup_id) {
                    setup.toolpaths.insert(0, entry);
                }
            }
        } else if !has_pins {
            // Remove pin drill toolpath if pins were all deleted.
            if let Some((_, tp_id)) = existing {
                self.state.job.remove_toolpath(tp_id);
            }
        } else if let Some((_, tp_id)) = existing {
            // Pins exist and toolpath exists — update hole positions and mark stale.
            let new_holes: Vec<[f64; 2]> = self
                .state
                .job
                .stock
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
            if let Some(tp) = self.state.job.find_toolpath_mut(tp_id) {
                if let OperationConfig::AlignmentPinDrill(ref mut cfg) = tp.operation {
                    cfg.holes = new_holes;
                }
                tp.result = None;
                tp.stale_since = Some(std::time::Instant::now());
            }
        }
    }

    pub fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((
            tool_id,
            model_id,
            mut operation,
            dressups,
            heights_config,
            stock_source,
            toolpath_name,
            boundary_enabled,
            boundary_containment,
            debug_options,
            face_selection_for_toolpath,
        )) = self.state.job.find_toolpath(tp_id).map(|toolpath| {
            (
                toolpath.tool_id,
                toolpath.model_id,
                toolpath.operation.clone(),
                toolpath.dressups.clone(),
                toolpath.heights.clone(),
                toolpath.stock_source,
                toolpath.name.clone(),
                toolpath.boundary_enabled,
                toolpath.boundary_containment,
                toolpath.debug_options,
                toolpath.face_selection.clone(),
            )
        })
        else {
            return;
        };

        let Some(tool) = self
            .state
            .job
            .tools
            .iter()
            .find(|tool| tool.id == tool_id)
            .cloned()
        else {
            return;
        };

        // Run the same validation the UI uses so both paths are consistent.
        {
            let validation =
                crate::ui::properties::ToolpathValidationContext::from_job(&self.state.job);
            if let Some(entry) = self.state.job.find_toolpath(tp_id) {
                let errs = crate::ui::properties::validate_toolpath(entry, &validation);
                if !errs.is_empty() {
                    if let Some(tp) = self.state.job.find_toolpath_mut(tp_id) {
                        tp.status = ComputeStatus::Error(errs.join("; "));
                    }
                    return;
                }
            }
        }

        let setup_ref = self
            .state
            .job
            .setups
            .iter()
            .find(|setup| setup.toolpaths.iter().any(|toolpath| toolpath.id == tp_id));
        let mut keep_out_footprints = setup_ref
            .map(|setup| {
                let mut footprints = Vec::new();
                for fixture in &setup.fixtures {
                    if fixture.enabled {
                        footprints.push(fixture.footprint());
                    }
                }
                for keep_out in &setup.keep_out_zones {
                    if keep_out.enabled {
                        footprints.push(keep_out.footprint());
                    }
                }
                footprints
            })
            .unwrap_or_default();
        let transform_setup = setup_ref.map(|setup| {
            let mut transform_setup = Setup::new(setup.id, setup.name.clone());
            transform_setup.face_up = setup.face_up;
            transform_setup.z_rotation = setup.z_rotation;
            transform_setup
        });
        let stock_snapshot = self.state.job.stock.clone();

        let model = self
            .state
            .job
            .models
            .iter()
            .find(|model| model.id == model_id);
        let mut polygons = model.and_then(|model| model.polygons.clone());
        let mut mesh = model.and_then(|model| model.mesh.clone());
        let enriched_mesh = model.and_then(|model| model.enriched_mesh.clone());
        let face_selection = face_selection_for_toolpath;

        // Derive polygons from selected BREP faces when no explicit polygons exist.
        // This enables all 2.5D operations (pocket, profile, adaptive, trace, etc.)
        // to work with STEP models by extracting face boundary loops as Polygon2.
        // Also extract the face Z height to set the toolpath top_z correctly.
        let mut face_top_z: Option<f64> = None;
        if polygons.is_none()
            && let (Some(face_ids), Some(enriched)) = (&face_selection, &enriched_mesh)
            && !face_ids.is_empty()
        {
            if let Some(poly) = enriched.faces_boundary_as_polygon(face_ids) {
                polygons = Some(Arc::new(vec![poly]));
                // Extract the Z height from the selected faces' bounding boxes.
                // For horizontal planar faces, bbox.min.z ≈ bbox.max.z ≈ face Z.
                let z = face_ids
                    .iter()
                    .filter_map(|fid| enriched.face_group(*fid))
                    .map(|fg| fg.bbox.max.z)
                    .fold(f64::NEG_INFINITY, f64::max);
                if z.is_finite() {
                    face_top_z = Some(z);
                }
            } else {
                tracing::warn!(
                    "Selected faces did not produce a boundary polygon (non-horizontal or non-planar)"
                );
                self.status_message = Some((
                    "Face selection ignored: selected faces are not horizontal planes".to_string(),
                    std::time::Instant::now(),
                ));
            }
        }

        if let Some(transform_setup) = transform_setup.as_ref() {
            if let Some(raw_mesh) = mesh.as_ref() {
                mesh = Some(Arc::new(crate::state::job::transform_mesh(
                    raw_mesh,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            if let Some(raw_polygons) = polygons.as_ref() {
                polygons = Some(Arc::new(crate::state::job::transform_polygons(
                    raw_polygons,
                    transform_setup,
                    &stock_snapshot,
                )));
            }
            keep_out_footprints = crate::state::job::transform_polygons(
                &keep_out_footprints,
                transform_setup,
                &stock_snapshot,
            );
        }

        let is_3d = operation.is_3d();
        if is_3d && mesh.is_none() {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status =
                    ComputeStatus::Error("No 3D mesh (import STL or STEP)".to_string());
            }
            return;
        }
        if !is_3d && !operation.is_stock_based() && polygons.is_none() {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                toolpath.status = ComputeStatus::Error(
                    "No 2D geometry (import SVG/DXF or select STEP faces)".to_string(),
                );
            }
            return;
        }

        let prev_tool_radius = if let OperationConfig::Rest(config) = &operation {
            config.prev_tool_id.and_then(|prev_tool_id| {
                self.state
                    .job
                    .tools
                    .iter()
                    .find(|tool| tool.id == prev_tool_id)
                    .map(|tool| tool.diameter / 2.0)
            })
        } else {
            None
        };

        // Refresh pin drill holes from current stock state before submitting.
        if let OperationConfig::AlignmentPinDrill(ref mut cfg) = operation {
            cfg.holes = self
                .state
                .job
                .stock
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
        }

        if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
            toolpath.status = ComputeStatus::Computing;
            toolpath.result = None;
            toolpath.debug_trace = None;
            toolpath.semantic_trace = None;
            toolpath.debug_trace_path = None;
        }

        let safe_z = self.state.job.post.safe_z;
        let stock_bb = self.state.job.stock.bbox();
        let model_bb = self
            .state
            .job
            .models
            .iter()
            .find(|m| m.id == model_id)
            .and_then(|m| m.bbox());
        let height_ctx = crate::state::toolpath::HeightContext {
            safe_z,
            op_depth: operation.default_depth_for_heights(),
            stock_top_z: stock_bb.max.z,
            stock_bottom_z: stock_bb.min.z,
            model_top_z: model_bb.map(|b| b.max.z),
            model_bottom_z: model_bb.map(|b| b.min.z),
        };
        let mut heights = heights_config.resolve(&height_ctx);
        // When face selection provides a Z height, use it as the top_z
        // so the toolpath cuts at the face level, not at Z=0.
        if let Some(fz) = face_top_z
            && heights_config.top_z.is_auto()
        {
            heights.top_z = fz;
            // Shift bottom_z relative to the face top
            if heights_config.bottom_z.is_auto() {
                heights.bottom_z = fz - operation.default_depth_for_heights().abs();
            }
        }
        let stock_bbox = if let Some(transform_setup) = transform_setup.as_ref() {
            let (width, depth, height) = transform_setup.effective_stock(&stock_snapshot);
            Some(BoundingBox3 {
                min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                max: rs_cam_core::geo::P3::new(width, depth, height),
            })
        } else {
            Some(self.state.job.stock.bbox())
        };

        let prior_stock = if stock_source == StockSource::FromRemainingStock {
            stock_bbox
                .as_ref()
                .and_then(|bbox| self.build_prior_stock(tp_id, bbox, tool.diameter))
        } else {
            None
        };

        self.compute.submit_toolpath(ComputeRequest {
            toolpath_id: tp_id,
            toolpath_name,
            debug_options,
            polygons,
            mesh,
            enriched_mesh,
            face_selection,
            operation,
            dressups,
            stock_source,
            tool,
            safe_z,
            prev_tool_radius,
            stock_bbox,
            boundary_enabled,
            boundary_containment,
            keep_out_footprints,
            heights,
            prior_stock,
        });
    }

    /// Build a TriDexelStock representing the remaining material after simulating
    /// all prior enabled toolpaths (those that appear before `tp_id`) in the same
    /// setup.  Returns `None` when there are no prior results to simulate.
    // SAFETY: tp_index from position() within setup.toolpaths, slice always in bounds
    #[allow(clippy::indexing_slicing)]
    fn build_prior_stock(
        &self,
        tp_id: ToolpathId,
        stock_bbox: &BoundingBox3,
        tool_diameter: f64,
    ) -> Option<TriDexelStock> {
        // Find the setup that contains this toolpath and the position within it.
        let (setup, tp_index) = self.state.job.setups.iter().find_map(|setup| {
            let pos = setup.toolpaths.iter().position(|tp| tp.id == tp_id)?;
            Some((setup, pos))
        })?;

        // Collect prior toolpaths: those before tp_index that are enabled and
        // have a computed result.
        let prior: Vec<_> = setup.toolpaths[..tp_index]
            .iter()
            .filter(|tp| tp.enabled && tp.result.is_some())
            .collect();

        if prior.is_empty() {
            return None;
        }

        // Resolution: tool_diameter / 4, clamped to [0.25, 2.0].
        let resolution = (tool_diameter / 4.0).clamp(0.25, 2.0);
        let mut stock = TriDexelStock::from_bounds(stock_bbox, resolution);

        // In the setup-local frame the cutting direction is always FromTop
        // because the mesh and toolpaths have already been rotated so that the
        // face-up direction aligns with +Z.
        let direction = rs_cam_core::dexel_stock::StockCutDirection::FromTop;

        for tp in &prior {
            // SAFETY: prior list was filtered to only include entries with Some result
            #[allow(clippy::expect_used)]
            let result = tp.result.as_ref().expect("filtered for Some above");
            let tool = self.state.job.tools.iter().find(|t| t.id == tp.tool_id);
            if let Some(tool) = tool {
                let cutter = crate::compute::worker::helpers::build_cutter(tool);
                stock.simulate_toolpath(&result.toolpath, cutter.as_ref(), direction);
            }
        }

        Some(stock)
    }

    pub fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(result.toolpath_id) {
                        toolpath.debug_trace = result.debug_trace.clone();
                        toolpath.semantic_trace = result.semantic_trace.clone();
                        toolpath.debug_trace_path = result.debug_trace_path.clone();
                        match result.result {
                            Ok(computed) => {
                                toolpath.status = ComputeStatus::Done;
                                toolpath.result = Some(computed);
                            }
                            Err(ComputeError::Cancelled) => {
                                toolpath.status = ComputeStatus::Pending;
                                toolpath.result = None;
                            }
                            Err(ComputeError::Message(error)) => {
                                toolpath.status = ComputeStatus::Error(error);
                                toolpath.result = None;
                            }
                        }
                    }
                    self.pending_upload = true;
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        // Build boundaries
                        let boundaries: Vec<_> = simulation
                            .boundaries
                            .iter()
                            .map(|boundary| crate::state::simulation::ToolpathBoundary {
                                id: boundary.id,
                                name: boundary.name.clone(),
                                tool_name: boundary.tool_name.clone(),
                                start_move: boundary.start_move,
                                end_move: boundary.end_move,
                                direction: boundary.direction,
                            })
                            .collect();

                        // Build setup boundaries
                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.state.job.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .job
                                            .setups
                                            .iter()
                                            .find(|setup| setup.id == setup_id)
                                            .map(|setup| setup.name.clone())
                                            .unwrap_or_default();
                                        sbs.push(crate::state::simulation::SetupBoundary {
                                            setup_id,
                                            setup_name,
                                            start_move: boundary.start_move,
                                        });
                                    }
                                    last_setup_id = setup_id;
                                }
                            }
                            sbs
                        };

                        // Build checkpoints
                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                mesh: checkpoint.mesh,
                                stock: Some(checkpoint.stock),
                            })
                            .collect();

                        // Store rapid collision data in checks
                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        // Cache deviations for viz mode re-coloring.
                        // display_mesh starts as None — the first playback frame
                        // will fill it in from the live stock, showing progressive
                        // cutting from the uncut block.
                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = None;

                        // Global stock bbox (stock-relative, origin at 0,0,0)
                        let stock_bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                            max: rs_cam_core::geo::P3::new(
                                self.state.job.stock.x,
                                self.state.job.stock.y,
                                self.state.job.stock.z,
                            ),
                        };

                        // Store results as cached artifact
                        self.state.simulation.results = Some(SimulationResults {
                            mesh: simulation.mesh,
                            total_moves: simulation.total_moves,
                            boundaries,
                            setup_boundaries,
                            checkpoints,
                            selected_toolpaths: None,
                            playback_data: simulation.playback_data,
                            stock_bbox,
                            cut_trace: simulation.cut_trace,
                            cut_trace_path: simulation.cut_trace_path,
                        });

                        let inspect_target =
                            self.state.simulation.debug.pending_inspect_toolpath.take();
                        if let Some(move_index) = inspect_target.and_then(|toolpath_id| {
                            self.state
                                .simulation
                                .boundaries()
                                .iter()
                                .find(|boundary| boundary.id == toolpath_id)
                                .map(|boundary| boundary.start_move)
                        }) {
                            self.state.simulation.playback.current_move = move_index;
                            self.state.simulation.playback.playing = false;
                        } else {
                            // Start playback from the beginning so the user sees
                            // the tool progressively cutting the uncut block.
                            self.state.simulation.playback.current_move = 0;
                            self.state.simulation.playback.playing = true;
                        }

                        // Store fresh tri-dexel stock for playback (global frame)
                        let initial_stock = TriDexelStock::from_bounds(
                            &stock_bbox,
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_stock = Some(initial_stock);
                        self.state.simulation.playback.live_sim_move = 0;

                        // Update staleness metadata
                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: self.state.job.edit_counter,
                        });

                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Simulation failed: {error}");
                        self.push_notification(
                            format!("Simulation failed: {error}"),
                            super::Severity::Error,
                        );
                    }
                },
                ComputeMessage::Collision(result) => match result {
                    Ok(collision) => {
                        let count = collision.report.collisions.len();
                        if count == 0 {
                            tracing::info!("No holder clearance issues detected");
                            self.push_notification(
                                "No holder clearance issues detected".into(),
                                super::Severity::Info,
                            );
                        } else {
                            let msg = format!(
                                "{} holder clearance issues, min safe stickout: {:.1} mm",
                                count, collision.report.min_safe_stickout
                            );
                            tracing::warn!("{msg}");
                            self.push_notification(msg, super::Severity::Warning);
                        }
                        // Wire results into simulation checks state
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        self.state.simulation.checks.collision_report = Some(collision.report);
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;
                    }
                    Err(ComputeError::Cancelled) => {}
                    Err(ComputeError::Message(error)) => {
                        tracing::error!("Collision check failed: {error}");
                        self.push_notification(
                            format!("Collision check failed: {error}"),
                            super::Severity::Error,
                        );
                    }
                },
            }
        }
    }

    fn undo(&mut self) {
        if let Some(action) = self.state.history.undo() {
            match action {
                UndoAction::StockChange { old, .. } => {
                    self.state.job.stock = old;
                    self.pending_upload = true;
                }
                UndoAction::PostChange { old, .. } => {
                    self.state.job.post = old;
                }
                UndoAction::ToolChange { tool_id, old, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = old;
                    }
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    old_op,
                    old_dressups,
                    old_face_selection,
                    ..
                } => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                        toolpath.operation = old_op;
                        toolpath.dressups = old_dressups;
                        toolpath.face_selection = old_face_selection;
                    }
                }
                UndoAction::MachineChange { old, .. } => {
                    self.state.job.machine = old;
                }
            }
        }
    }

    fn redo(&mut self) {
        if let Some(action) = self.state.history.redo() {
            match action {
                UndoAction::StockChange { new, .. } => {
                    self.state.job.stock = new;
                    self.pending_upload = true;
                }
                UndoAction::PostChange { new, .. } => {
                    self.state.job.post = new;
                }
                UndoAction::ToolChange { tool_id, new, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = new;
                    }
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    new_op,
                    new_dressups,
                    new_face_selection,
                    ..
                } => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                        toolpath.operation = new_op;
                        toolpath.dressups = new_dressups;
                        toolpath.face_selection = new_face_selection;
                    }
                }
                UndoAction::MachineChange { new, .. } => {
                    self.state.job.machine = new;
                }
            }
        }
    }
}

/// Targets ~5 cells across the smallest tool radius so curved profiles
/// (especially ball nose) are visually resolved.  Clamped to [0.02, 0.5] mm
/// and further limited so the grid stays under ~8 M cells.
/// Derive the stock cut direction from a setup's face-up orientation.
fn face_up_to_direction(face_up: FaceUp) -> StockCutDirection {
    match face_up {
        FaceUp::Top => StockCutDirection::FromTop,
        FaceUp::Bottom => StockCutDirection::FromBottom,
        FaceUp::Front => StockCutDirection::FromFront,
        FaceUp::Back => StockCutDirection::FromBack,
        FaceUp::Left => StockCutDirection::FromLeft,
        FaceUp::Right => StockCutDirection::FromRight,
    }
}

/// Transform a toolpath from a setup's local coordinate frame to the global
/// stock-relative frame (origin at 0,0,0, axes aligned with physical stock).
///
/// For arc moves (CW/CCW), the offset vector (i,j) is transformed by the
/// linear part of the affine transform, and arc direction is flipped when the
/// XY component of the transform is a reflection.
fn transform_toolpath_to_stock_frame(
    toolpath: &rs_cam_core::toolpath::Toolpath,
    setup: &Setup,
    stock: &crate::state::job::StockConfig,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::{Move, MoveType, Toolpath};

    let (eff_w, eff_d, _) = setup.face_up.effective_stock(stock.x, stock.y, stock.z);

    // Point transform: undo ZRotation, then undo FaceUp (local → global stock-relative)
    let xform = |p: P3| -> P3 {
        let unrotated = setup.z_rotation.inverse_transform_point(p, eff_w, eff_d);
        setup
            .face_up
            .inverse_transform_point(unrotated, stock.x, stock.y, stock.z)
    };

    // Direction transform for arc offsets (i,j,0): linear part only (no translation).
    let o_g = xform(P3::new(0.0, 0.0, 0.0));
    let dir_xform = |di: f64, dj: f64| -> (f64, f64) {
        let p_g = xform(P3::new(di, dj, 0.0));
        (p_g.x - o_g.x, p_g.y - o_g.y)
    };

    // Determine if XY transform is a reflection (negative determinant → flip arc direction).
    let ex_g = xform(P3::new(1.0, 0.0, 0.0));
    let ey_g = xform(P3::new(0.0, 1.0, 0.0));
    let det = (ex_g.x - o_g.x) * (ey_g.y - o_g.y) - (ex_g.y - o_g.y) * (ey_g.x - o_g.x);
    let flip_arcs = det < 0.0;

    let new_moves: Vec<Move> = toolpath
        .moves
        .iter()
        .map(|m| {
            let target = xform(m.target);
            let move_type = match m.move_type {
                MoveType::Rapid => MoveType::Rapid,
                MoveType::Linear { feed_rate } => MoveType::Linear { feed_rate },
                MoveType::ArcCW { i, j, feed_rate } => {
                    let (ni, nj) = dir_xform(i, j);
                    if flip_arcs {
                        MoveType::ArcCCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    } else {
                        MoveType::ArcCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    }
                }
                MoveType::ArcCCW { i, j, feed_rate } => {
                    let (ni, nj) = dir_xform(i, j);
                    if flip_arcs {
                        MoveType::ArcCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    } else {
                        MoveType::ArcCCW {
                            i: ni,
                            j: nj,
                            feed_rate,
                        }
                    }
                }
            };
            Move { target, move_type }
        })
        .collect();

    Toolpath { moves: new_moves }
}

fn auto_resolution_for_tools(toolpaths: &[SetupSimToolpath], stock_bbox: &BoundingBox3) -> f64 {
    let min_radius = toolpaths
        .iter()
        .map(|toolpath| toolpath.tool.diameter / 2.0)
        .fold(f64::INFINITY, f64::min);

    // 5 cells across the radius gives decent curve resolution
    let from_tool = (min_radius / 5.0).clamp(0.02, 0.5);

    // Cap so grid stays under ~8M cells (reasonable memory / mesh size)
    let max_cells: f64 = 8_000_000.0;
    let sx = stock_bbox.max.x - stock_bbox.min.x;
    let sy = stock_bbox.max.y - stock_bbox.min.y;
    let from_grid = ((sx * sy) / max_cells).sqrt().max(0.02);

    let resolution = from_tool.max(from_grid);

    tracing::info!(
        "Auto sim resolution: {:.3} mm (smallest tool \u{00D8}{:.2} mm, grid ~{}x{})",
        resolution,
        min_radius * 2.0,
        (sx / resolution).ceil() as usize,
        (sy / resolution).ceil() as usize,
    );

    resolution
}
