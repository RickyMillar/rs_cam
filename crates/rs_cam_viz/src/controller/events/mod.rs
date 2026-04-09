mod compute;
mod model;
mod simulation;
mod toolpath;
mod undo;

use crate::compute::ComputeBackend;
use crate::state::selection::Selection;
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
                self.handle_remove_fixture(setup_id, fixture_id);
            }
            AppEvent::AddKeepOut(setup_id) => self.handle_add_keep_out(setup_id),
            AppEvent::RemoveKeepOut(setup_id, keep_out_id) => {
                self.handle_remove_keep_out(setup_id, keep_out_id);
            }
            AppEvent::FixtureChanged => {
                self.pending_upload = true;
                self.state.gui.mark_edited();
            }

            // --- Toolpath events ---
            AppEvent::AddToolpath(op_type) => self.handle_add_toolpath(op_type),
            AppEvent::DuplicateToolpath(tp_id) => self.handle_duplicate_toolpath(tp_id),
            AppEvent::MoveToolpathUp(tp_id) => self.handle_move_toolpath_up(tp_id),
            AppEvent::MoveToolpathDown(tp_id) => self.handle_move_toolpath_down(tp_id),
            AppEvent::ReorderToolpath(tp_id, target_idx) => {
                self.handle_reorder_toolpath(tp_id, target_idx);
            }
            AppEvent::MoveToolpathToSetup(tp_id, setup_id, idx) => {
                self.handle_move_toolpath_to_setup(tp_id, setup_id, idx);
            }
            AppEvent::ToggleToolpathEnabled(tp_id) => {
                if let Some((idx, _tc)) =
                    self.state.session.find_toolpath_config_by_id(tp_id.0)
                    && let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx)
                {
                    tc.enabled = !tc.enabled;
                }
            }
            AppEvent::RemoveToolpath(tp_id) => self.handle_remove_toolpath(tp_id),
            AppEvent::GenerateToolpath(tp_id) => self.submit_toolpath_compute(tp_id),
            AppEvent::GenerateAll => self.handle_generate_all(),
            AppEvent::ToggleToolpathVisibility(tp_id) => {
                if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                    rt.visible = !rt.visible;
                    self.pending_upload = true;
                }
            }
            AppEvent::ToggleIsolateToolpath => self.handle_toggle_isolate_toolpath(),
            AppEvent::InspectToolpathInSimulation(tp_id) => {
                self.handle_inspect_toolpath_in_simulation(tp_id);
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
                if let Some((idx, _)) =
                    self.state.session.find_toolpath_config_by_id(toolpath_id.0)
                {
                    if let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx) {
                        let faces = tc.face_selection.get_or_insert_with(Vec::new);
                        if let Some(pos) = faces.iter().position(|f| *f == face_id) {
                            faces.remove(pos);
                        } else {
                            faces.push(face_id);
                        }
                        if faces.is_empty() {
                            tc.face_selection = None;
                        }
                    }
                    // Mark stale in GUI runtime
                    if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id.0) {
                        rt.stale_since = Some(std::time::Instant::now());
                    }
                    self.state.selection = Selection::Toolpath(toolpath_id);
                    self.state.gui.mark_edited();
                    self.pending_upload = true;
                }
            }

            // --- Undo / redo ---
            AppEvent::Undo => self.undo(),
            AppEvent::Redo => self.redo(),

            // --- Stock / machine events ---
            AppEvent::StockChanged => self.handle_stock_changed(),
            AppEvent::StockMaterialChanged => {
                self.state.gui.mark_edited();
            }
            AppEvent::MachineChanged => {
                self.state.gui.mark_edited();
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
}
