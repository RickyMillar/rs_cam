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
                if let Some((idx, tc)) = self.state.session.find_toolpath_config_by_id(tp_id.0) {
                    let _ = self.state.session.set_toolpath_enabled(idx, !tc.enabled);
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
            AppEvent::ClearIsolation => {
                self.state.viewport.isolate_toolpath = None;
                self.pending_upload = true;
            }
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
                if let Some((idx, tc)) =
                    self.state.session.find_toolpath_config_by_id(toolpath_id.0)
                {
                    // Compute new face selection by toggling the given face_id
                    let mut faces = tc.face_selection.clone().unwrap_or_default();
                    if let Some(pos) = faces.iter().position(|f| *f == face_id) {
                        faces.remove(pos);
                    } else {
                        faces.push(face_id);
                    }
                    let new_selection = if faces.is_empty() { None } else { Some(faces) };
                    let _ = self.state.session.set_face_selection(idx, new_selection);

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
                self.state.session.invalidate_machine();
                self.state.gui.mark_edited();
            }

            // --- F&S suggest modal ---
            AppEvent::OpenSuggestModal(toolpath_id) => {
                self.state.suggest_modal_for = Some(toolpath_id.0);
            }
            AppEvent::CloseSuggestModal => {
                self.state.suggest_modal_for = None;
            }
            AppEvent::ApplySuggestedFeed {
                toolpath_id,
                feed_mm_min,
                spindle_rpm,
            } => {
                self.apply_suggested_feed(toolpath_id, feed_mm_min, spindle_rpm);
            }

            // --- Optimize modal (U2) ---
            AppEvent::OpenOptimizeModal(toolpath_id) => {
                self.open_optimize_modal(toolpath_id);
            }
            AppEvent::CloseOptimizeModal => {
                self.state.optimize_modal = None;
            }
            AppEvent::ApplyOptimizeCandidate {
                toolpath_id,
                candidate_index,
            } => {
                self.apply_optimize_candidate(toolpath_id, candidate_index);
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
            | AppEvent::ToggleProjection
            | AppEvent::PreviewOrientation(_)
            | AppEvent::ResetView
            | AppEvent::SwitchWorkspace(_)
            | AppEvent::SimVizModeChanged
            | AppEvent::ShowShortcuts
            | AppEvent::SetToolLoadOverride { .. }
            | AppEvent::Quit => {}
        }
    }

    /// Apply a feed/RPM suggestion via the extended snapshot mutation.
    /// Routes through `apply_toolpath_param_snapshot` (Engineering
    /// Default 7) so the relevant `feeds_auto.*` flags are flipped to
    /// `false` transactionally with the operation write — closes the
    /// silent-LUT-overwrite bug the older `set_toolpath_param`-cascade
    /// path had.
    fn apply_suggested_feed(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        feed_mm_min: f64,
        spindle_rpm: Option<u32>,
    ) {
        let index = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id.0);
        let Some(idx) = index else {
            self.push_notification(
                format!("Apply failed: toolpath id {} not found", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };

        // Snapshot the fields we need from the current config, then
        // mutate the operation and feeds_auto for the candidate apply.
        let Some(tc) = self.state.session.get_toolpath_config(idx) else {
            self.push_notification(
                format!("Apply failed: toolpath {} disappeared", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };
        let mut new_op = tc.operation.clone();
        let dressups = tc.dressups.clone();
        let face_selection = tc.face_selection.clone();
        let mut feeds_auto = tc.feeds_auto.clone();

        // Always changing feed.
        new_op.set_feed_rate(feed_mm_min);
        feeds_auto.feed_rate = false;
        // RPM only if Some.
        if let Some(rpm) = spindle_rpm {
            new_op.set_spindle_rpm(Some(rpm));
            feeds_auto.spindle_speed = false;
        }

        if let Err(e) = self.state.session.apply_toolpath_param_snapshot(
            idx,
            new_op,
            dressups,
            face_selection,
            feeds_auto,
        ) {
            self.push_notification(
                format!("Apply failed: {e}"),
                crate::controller::Severity::Error,
            );
            return;
        }

        self.state.gui.mark_edited();
        self.state.suggest_modal_for = None;
        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id.0) {
            rt.stale_since = Some(std::time::Instant::now());
        }
        let rpm_text = spindle_rpm.map_or_else(String::new, |r| format!(" @ {r} RPM"));
        self.push_notification(
            format!(
                "Applied {feed_mm_min:.0} mm/min{rpm_text} to toolpath {}. \
                 Regenerate to apply.",
                toolpath_id.0
            ),
            crate::controller::Severity::Info,
        );
    }

    /// Open the per-toolpath Optimize modal. Runs `optimize_toolpath`
    /// synchronously and stashes the resulting outcome on
    /// `AppState::optimize_modal`. Synchronous = the GUI freezes for
    /// the duration; the worker-thread integration in U3 will move
    /// this off the main thread with a Loading state.
    fn open_optimize_modal(&mut self, toolpath_id: crate::state::toolpath::ToolpathId) {
        use rs_cam_core::tool_load::optimize::{OptimizeOutcome, optimize_toolpath};
        use rs_cam_core::tool_load::suggest::RefuseReason;
        use std::sync::atomic::AtomicBool;

        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id.0)
        else {
            self.push_notification(
                format!("Optimize failed: toolpath id {} not found", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };

        // Need a baseline trace. Without one the optimizer can't
        // score; surface a typed Skipped so the modal can render
        // a clear "run sim first" message.
        let trace_clone = self
            .state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.clone());
        let Some(trace) = trace_clone else {
            self.state.optimize_modal = Some(crate::state::OptimizeModalState {
                toolpath_id: toolpath_id.0,
                status: crate::state::OptimizeRunStatus::Ready(
                    OptimizeOutcome::Skipped {
                        reason: RefuseReason::SimulationRequired,
                    },
                ),
            });
            return;
        };

        // Synchronous run. The OS-level cancel flag is unused for U2
        // (the modal blocks until this returns); U3 wires it.
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut self.state.session, &trace, idx, &cancel);
        self.state.optimize_modal = Some(crate::state::OptimizeModalState {
            toolpath_id: toolpath_id.0,
            status: crate::state::OptimizeRunStatus::Ready(outcome),
        });
    }

    /// Apply a candidate from the cached Optimize outcome. Index 0 is
    /// the baseline (no-op); higher indexes select a non-baseline
    /// candidate. Routes through `apply_toolpath_param_snapshot` with
    /// the candidate's params + a `feeds_auto` whose flags are flipped
    /// per the candidate's `ParamDelta` (Resolution 7's mapping).
    fn apply_optimize_candidate(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        candidate_index: usize,
    ) {
        use rs_cam_core::tool_load::optimize::{
            OptimizeOutcome, feeds_auto_for_candidate,
        };

        // Lookup phase: extract everything we need from the cached
        // outcome and the toolpath config, then drop the borrow before
        // any mutation. The session call needs &mut; the optimize_modal
        // read needs &.
        let Some(modal) = self.state.optimize_modal.as_ref() else {
            self.push_notification(
                "Apply failed: Optimize modal not open".to_owned(),
                crate::controller::Severity::Error,
            );
            return;
        };
        let crate::state::OptimizeRunStatus::Ready(OptimizeOutcome::Ranked(candidates)) =
            &modal.status
        else {
            self.push_notification(
                "Apply failed: Optimize has no candidates to apply".to_owned(),
                crate::controller::Severity::Error,
            );
            return;
        };
        let Some(candidate) = candidates.get(candidate_index) else {
            self.push_notification(
                format!("Apply failed: candidate index {candidate_index} out of range"),
                crate::controller::Severity::Error,
            );
            return;
        };
        if candidate_index == 0 {
            // Index 0 is the baseline — nothing to do.
            self.state.optimize_modal = None;
            return;
        }
        let candidate_op = candidate.params.clone();
        let candidate_delta = candidate.delta.clone();

        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id.0)
        else {
            self.push_notification(
                format!("Apply failed: toolpath id {} not found", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };
        let Some(tc) = self.state.session.get_toolpath_config(idx) else {
            self.push_notification(
                format!("Apply failed: toolpath {} disappeared", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };
        let dressups = tc.dressups.clone();
        let face_selection = tc.face_selection.clone();
        let baseline_feeds_auto = tc.feeds_auto.clone();
        let feeds_auto = feeds_auto_for_candidate(&baseline_feeds_auto, &candidate_delta);

        if let Err(e) = self.state.session.apply_toolpath_param_snapshot(
            idx,
            candidate_op,
            dressups,
            face_selection,
            feeds_auto,
        ) {
            self.push_notification(
                format!("Apply failed: {e}"),
                crate::controller::Severity::Error,
            );
            return;
        }

        self.state.gui.mark_edited();
        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id.0) {
            rt.stale_since = Some(std::time::Instant::now());
        }
        self.state.optimize_modal = None;
        self.push_notification(
            format!(
                "Applied optimize candidate to toolpath {}. Regenerate to apply.",
                toolpath_id.0
            ),
            crate::controller::Severity::Info,
        );
    }
}
