mod compute;
mod model;
mod planner;
pub(crate) mod simulation;
mod toolpath;
mod undo;

use rs_cam_core::session::{
    Command, Effects, ReplaceToolpathConfigArgs, RestoreToolpathSnapshotArgs,
    SetDrillSelectedHolesArgs, SetFaceSelectionArgs, SetPostConfigArgs,
    SetToolpathDebugOptionsArgs, SetToolpathEnabledArgs,
};

use crate::compute::ComputeBackend;
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::{
    DeleteLibraryToolArgs, MoveLibraryToolArgs, NoArgs, RenameMachineInLibraryArgs,
    RenameToolCatalogArgs, SetToolLoadOverrideArgs, SimJumpToMoveArgs, SimJumpToOpStartArgs,
    UiCommand, UpdateLibraryToolArgs,
};

use super::AppController;

/// The ids of every ENABLED toolpath, in project order.
///
/// Two project-scope feeds handlers need this set: the rollup's seed and its
/// `Select all`. Before DC5a the same expression was written out twice — once
/// in `open_feeds_modal` as the seed, once in the select-all arm — so the two
/// could drift. One expansion now serves both.
fn enabled_toolpath_ids(
    state: &crate::state::AppState,
) -> std::collections::BTreeSet<rs_cam_core::ToolpathId> {
    state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .map(|tc| tc.id)
        .collect()
}

impl<B: ComputeBackend> AppController<B> {
    /// Run one command from an event arm, and stamp what it dropped.
    ///
    /// `subject` names what the operator was editing, so a refusal
    /// reaches them in their own words. The method answers whether the
    /// command was applied, so a caller can dirty the project once for a
    /// group of them.
    ///
    /// The door mirrors BOTH halves of the answer (WP19, plan §28).
    /// `Effects::stale` reaches the toolpath cards.
    /// `Effects::simulation_cleared` reaches the viewport, because the
    /// session and the viewport hold ONE simulation (WP11b, N12 item
    /// 10). The operator ruling is one rule for every row: a session
    /// that drops its simulation leaves no viewport showing one.
    ///
    /// A caller that calls `invalidate_simulation` itself KEEPS that
    /// call. The flag is `before && !after` over the SESSION's
    /// simulation, so it is false when the session held none — and the
    /// view can hold playback state, collision positions and a pending
    /// upload the session never mirrored. `invalidate_simulation` is
    /// idempotent, so the two together cost one pass.
    fn apply_controller_command(&mut self, command: Command, subject: &str) -> bool {
        match self.state.session.apply(command) {
            Ok(effects) => {
                crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
                if effects.simulation_cleared {
                    self.invalidate_simulation();
                }
                true
            }
            Err(error) => {
                self.push_notification(
                    format!("Could not write {subject}: {error}"),
                    crate::controller::Severity::Error,
                );
                false
            }
        }
    }

    /// Run one command from an event arm, and stamp what it dropped.
    ///
    /// The twin of [`Self::apply_controller_command`] for a site that
    /// SWALLOWED its setter's answer before WP15a. Such a site showed
    /// the operator nothing on a refusal, so this door pushes no
    /// notification either: the routing moves the write onto
    /// `ProjectSession::apply` and leaves the messages alone.
    ///
    /// The answer carries the command's own [`Effects`], so a caller
    /// that read the setter's return — the new index in
    /// [`Effects::created`], for example — reads it here.
    ///
    /// It mirrors `Effects::simulation_cleared` too, on the rule and
    /// for the reasons [`Self::apply_controller_command`] states
    /// (WP19, plan §28). A caller that also calls
    /// `invalidate_simulation` keeps that call.
    fn apply_quietly(&mut self, command: Command) -> Option<Effects> {
        match self.state.session.apply(command) {
            Ok(effects) => {
                crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
                if effects.simulation_cleared {
                    self.invalidate_simulation();
                }
                Some(effects)
            }
            Err(_) => None,
        }
    }

    /// Run one command, and report the index its row created.
    ///
    /// The four add rows report the index core allocated in
    /// [`Effects::created`], which is what a site read from the setter's
    /// own answer before WP15a.
    ///
    /// `pub(crate)` because `controller/io.rs` is a SIBLING module, not
    /// a child of this one, and its four import doors read the same
    /// answer.
    pub(crate) fn apply_created(&mut self, command: Command) -> Option<usize> {
        self.apply_quietly(command)
            .and_then(|effects| effects.created)
    }

    /// Turn the generator debug capture on or off for every toolpath.
    ///
    /// One `SetToolpathDebugOptions` per index. The row moves no
    /// revision: a debug trace is an OUTPUT of a generation, never an
    /// input to one, so this drops no cached result.
    fn set_generator_trace_capture_all(&mut self, enabled: bool) {
        let count = self.state.session.toolpath_count();
        for index in 0..count {
            let Some(tc) = self.state.session.get_toolpath_config(index) else {
                continue;
            };
            let mut debug_options = tc.debug_options;
            if debug_options.enabled == enabled {
                continue;
            }
            debug_options.enabled = enabled;
            let command = Command::SetToolpathDebugOptions(SetToolpathDebugOptionsArgs {
                index,
                debug_options,
            });
            self.apply_controller_command(command, "the generator debug capture");
        }
    }

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
            AppEvent::RelinkModel(model_id, ref path) => {
                let path = path.clone();
                if let Err(error) = self.relink_model(model_id, &path) {
                    self.push_error(&error);
                }
            }
            AppEvent::AddTool(tool_type) => self.handle_add_tool(tool_type),
            AppEvent::AddToolFromLibrary(tool) => self.handle_add_tool_from_library(*tool),
            AppEvent::DuplicateTool(tool_id) => self.handle_duplicate_tool(tool_id),
            AppEvent::RemoveTool(tool_id) => self.handle_remove_tool(tool_id),
            AppEvent::ImportMachineFromLibrary(name) => self.import_machine_from_library(&name),

            AppEvent::AddSetup => self.handle_add_setup(),
            AppEvent::RequestRemoveSetup(setup_id) => self.request_remove_setup(setup_id),
            AppEvent::CancelRemoveSetup => self.state.panels.pending_setup_removal = None,
            AppEvent::RemoveSetup(setup_id) => self.handle_remove_setup(setup_id),
            AppEvent::SetupTwoSided => self.handle_setup_two_sided(),
            AppEvent::RenameSetup(setup_id, name) => self.handle_rename_setup(setup_id, name),
            AppEvent::AddFixture(setup_id) => self.handle_add_fixture(setup_id),
            AppEvent::RemoveFixture(setup_id, fixture_id) => {
                self.handle_remove_fixture(setup_id, fixture_id);
            }
            AppEvent::AddKeepOut(setup_id) => self.handle_add_keep_out(setup_id),
            AppEvent::RemoveKeepOut(setup_id, keep_out_id) => {
                self.handle_remove_keep_out(setup_id, keep_out_id);
            }
            // --- Toolpath events ---
            AppEvent::AddToolpath(op_type) => {
                // The handler reports the command's `Effects`. The
                // operator route needs none of it: `apply_quietly`
                // already stamped the stale set and mirrored the
                // simulation. The MCP route reads the field (WP28).
                self.handle_add_toolpath(op_type);
            }
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
                if let Some((idx, tc)) = self.state.session.find_toolpath_config_by_id(tp_id) {
                    let enabled = !tc.enabled;
                    let command = Command::SetToolpathEnabled(SetToolpathEnabledArgs {
                        index: idx,
                        enabled,
                    });
                    let _ = self.apply_quietly(command);
                    // G-FRESHSTATE: the flip changes what the job cuts and
                    // what every downstream rest op starts from, so the
                    // project is dirty. It used to record neither that nor
                    // the staleness.
                    //
                    // WP19 (plan §28): the door CLEARS the viewport's
                    // simulation here, it no longer only stales it. The
                    // core row drops the session's simulation, and
                    // `ProjectSession::start` then refuses every
                    // `FromRemainingStock` operation while a banner still
                    // shows the operator stock that is not there. One rule
                    // for every row, no per-row exception.
                    self.state.gui.mark_edited();
                }
            }
            AppEvent::RemoveToolpath(tp_id) => self.handle_remove_toolpath(tp_id),
            // R6: one Generate makes the named operation current, which
            // means running its ancestors first.
            AppEvent::GenerateToolpath(tp_id) => self.handle_generate_toolpath(tp_id),
            AppEvent::SetSimulationResolution(resolution) => {
                if let Err(error) = self.set_simulation_resolution(resolution) {
                    self.push_notification(
                        format!("Could not set the simulation resolution: {error}"),
                        super::Severity::Warning,
                    );
                }
            }
            AppEvent::GenerateAll => self.handle_generate_all(),
            AppEvent::GenerateSetupOnly(setup_id) => self.handle_generate_setup_only(setup_id),
            AppEvent::CancelGeneration => self.cancel_generation_plan(),

            // --- Simulation events ---
            AppEvent::RunSimulation => {
                let _submitted = self.run_simulation_with_all();
            }
            AppEvent::RunSimulationWith(ids) => self.run_simulation_with_ids(&ids),

            // --- Compute / check events ---
            AppEvent::RunCollisionCheck => self.request_collision_check(),

            // --- Face selection ---
            AppEvent::ToggleFaceSelection {
                toolpath_id,
                model_id: _,
                face_id,
            } => {
                if let Some((idx, tc)) = self.state.session.find_toolpath_config_by_id(toolpath_id)
                {
                    // Compute new face selection by toggling the given face_id
                    let mut faces = tc.face_selection.clone().unwrap_or_default();
                    if let Some(pos) = faces.iter().position(|f| *f == face_id) {
                        faces.remove(pos);
                    } else {
                        faces.push(face_id);
                    }
                    let new_selection = if faces.is_empty() { None } else { Some(faces) };
                    // WP15a: the row stamps every index `Effects::stale`
                    // names. The site stamped this toolpath alone, and
                    // the setter drops the downstream stock chain with
                    // it — the WP8 shape.
                    let command = Command::SetFaceSelection(SetFaceSelectionArgs {
                        index: idx,
                        face_ids: new_selection,
                    });
                    let _ = self.apply_quietly(command);
                    self.state.selection = Selection::Toolpath(toolpath_id);
                    self.state.gui.mark_edited();
                    self.pending_upload = true;
                }
            }

            // --- Drill target selection ---
            AppEvent::ToggleDrillTarget { toolpath_id, xy } => {
                use rs_cam_core::compute::catalog::OperationConfig;
                if let Some((idx, tc)) = self.state.session.find_toolpath_config_by_id(toolpath_id)
                {
                    let current = match &tc.operation {
                        OperationConfig::Drill(c) => c.selected_holes.clone(),
                        OperationConfig::AlignmentPinDrill(c) => c.selected_holes.clone(),
                        _ => None,
                    };
                    let mut holes = current.unwrap_or_default();
                    // Toggle membership with a tolerance (no float == on
                    // picks). The core constant, not a local copy: the
                    // generator resolves a pick against the model's targets
                    // at this same distance (G-DRILLPICKSTALE).
                    const EPS: f64 = rs_cam_core::compute::execute::DRILL_PICK_MATCH_EPS_MM;
                    if let Some(pos) = holes
                        .iter()
                        .position(|h| (h[0] - xy[0]).abs() < EPS && (h[1] - xy[1]).abs() < EPS)
                    {
                        holes.remove(pos);
                    } else {
                        holes.push(xy);
                    }
                    // Empty selection reverts to the legacy default (None) so a
                    // viewport deselect doesn't leave a confusing "nothing" state.
                    let new_selection = if holes.is_empty() { None } else { Some(holes) };
                    // N6 (WP8): the setter drops the downstream stock
                    // chain now, so the stamp follows `Effects::stale`
                    // rather than naming this toolpath alone.
                    let command = Command::SetDrillSelectedHoles(SetDrillSelectedHolesArgs {
                        index: idx,
                        selected_holes: new_selection,
                    });
                    let _ = self.apply_quietly(command);
                    self.state.selection = Selection::Toolpath(toolpath_id);
                    self.state.gui.mark_edited();
                    self.pending_upload = true;
                }
            }

            // --- Undo / redo ---
            AppEvent::Undo => self.undo(),
            AppEvent::Redo => self.redo(),

            // --- Optimize modal (U2) ---
            AppEvent::OpenOptimizeModal(toolpath_id) => {
                self.open_optimize_modal(toolpath_id);
            }
            AppEvent::ApplyOptimizeCandidate {
                toolpath_id,
                candidate_index,
            } => {
                self.apply_optimize_candidate(toolpath_id, candidate_index);
            }
            AppEvent::ReoptimizeWithAxisOverride {
                toolpath_id,
                axis,
                value,
            } => {
                self.reoptimize_with_axis_override(toolpath_id, axis, value);
            }

            // --- Optimize project (U3) ---
            AppEvent::OpenOptimizeProject => {
                self.open_optimize_project();
            }
            AppEvent::ApplyOptimizeProject => {
                self.apply_optimize_project();
            }
            AppEvent::PreviewMultitoolPlan => {
                self.request_multitool_preview();
            }
            AppEvent::ApplyMultitoolPlan => {
                self.apply_multitool_planner();
            }

            // --- Feeds & Speeds modal (redesigned Feeds tab) ---
            AppEvent::OpenFeedsModal(toolpath_id) => {
                self.open_feeds_modal(toolpath_id);
            }
            AppEvent::SetSpindleStrategy(strategy) => {
                // Project-level setting; updates both the session
                // post (persisted to TOML on save) and the viz post
                // mirror (drives the modal's preview values on next
                // frame). Invalidates Suggest's cached output via the
                // existing dirty-tracking hooks.
                if self.state.session.post_config().spindle_strategy != strategy {
                    // `SetPostConfig` replaces the WHOLE block, so a
                    // surface that offers one field reads the current
                    // block, writes that field and sends the result.
                    let mut post = self.state.session.post_config().clone();
                    post.spindle_strategy = strategy;
                    let command = Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(post),
                    });
                    if self.apply_controller_command(command, "the spindle strategy") {
                        // SHL-01: one door rebuilds the mirror. This arm
                        // copied `spindle_strategy` alone, and it copied
                        // it even when the command was refused.
                        self.refresh_post_mirror();
                    }
                    self.state.gui.mark_edited();
                }
            }
            AppEvent::ApplyFeedsAll(toolpath_id) => {
                self.apply_feeds_all(toolpath_id);
            }
            AppEvent::ApplyFeedsSpeeds(toolpath_id) => {
                self.apply_feeds_speeds(toolpath_id);
            }
            AppEvent::SetDropCutterScallopHeight { toolpath_id, value } => {
                self.set_drop_cutter_scallop_height(toolpath_id, value);
            }
            AppEvent::ApplyFeedsProject => {
                self.apply_feeds_project();
            }
            AppEvent::ApplyFeedsExplore {
                toolpath_id,
                feed_mm_min,
                rpm,
            } => {
                self.apply_feeds_explore(toolpath_id, feed_mm_min, rpm);
            }
            AppEvent::ApplyFeedsProjectSelected => {
                self.apply_feeds_project_selected();
            }

            AppEvent::SetGeneratorTraceCaptureAll(enabled) => {
                self.set_generator_trace_capture_all(enabled);
            }

            // --- Pass-through events handled elsewhere ---
            AppEvent::ExportCombinedGcode
            | AppEvent::ExportSetupGcode(_)
            | AppEvent::ExportGcodeConfirmed
            | AppEvent::ExportSetupSheet
            | AppEvent::ExportSvgPreview
            | AppEvent::SaveJob
            | AppEvent::OpenJob
            | AppEvent::WizardSetStep(_)
            | AppEvent::WizardSetPost(_)
            | AppEvent::WizardSetOutputLayout(_)
            | AppEvent::WizardSetFilenameTemplate(_)
            | AppEvent::WizardSetWcsOverride(_)
            | AppEvent::WizardSetUnitsOverride(_)
            | AppEvent::WizardSetSafeZOverride(_)
            | AppEvent::WizardSetDryRun(_)
            | AppEvent::WizardSetSpindleWarmup(_)
            | AppEvent::WizardSetToolChangeMode(_)
            | AppEvent::WizardSetSetupPauseMessage { .. }
            | AppEvent::WizardSetAllowValidatorErrors(_)
            | AppEvent::WizardSave => {}

            // --- View commands (WP13) ---
            //
            // Every arm here writes the VIEW and never `ProjectSession`.
            // `crates/rs_cam_viz/tests/command_surface_completeness.rs`
            // measures that, with two named exemptions.
            AppEvent::Ui(cmd) => match cmd {
                // --- Tree / selection events ---
                UiCommand::Select(ref selection) => self.handle_select(selection),
                // --- Tool Library modal ---
                UiCommand::OpenToolLibrary(NoArgs) => self.open_tool_library(),
                UiCommand::CloseToolLibrary(NoArgs) => self.state.tool_library_modal = None,
                UiCommand::DeleteLibraryTool(DeleteLibraryToolArgs { catalog, index }) => {
                    self.delete_library_tool(&catalog, index);
                }
                UiCommand::UpdateLibraryTool(UpdateLibraryToolArgs {
                    catalog,
                    index,
                    tool,
                }) => self.update_library_tool(&catalog, index, *tool),
                UiCommand::MoveLibraryTool(MoveLibraryToolArgs { from, index, to }) => {
                    self.move_library_tool(&from, index, &to);
                }
                UiCommand::CreateToolCatalog(name) => self.create_tool_catalog(&name),
                UiCommand::DeleteToolCatalog(name) => self.delete_tool_catalog(&name),
                UiCommand::RenameToolCatalog(RenameToolCatalogArgs { old, new }) => {
                    self.rename_tool_catalog(&old, &new);
                }
                UiCommand::DedupeToolCatalog(name) => self.dedupe_tool_catalog(&name),
                // --- Machine Library modal (snapshot model) ---
                UiCommand::OpenMachineLibrary(NoArgs) => {
                    self.state.close_modals_for_exclusivity();
                    self.state.machine_library_open = true;
                }
                UiCommand::CloseMachineLibrary(NoArgs) => self.state.machine_library_open = false,
                UiCommand::SaveMachineToLibrary(name) => self.save_machine_to_library(&name),
                UiCommand::DeleteMachineFromLibrary(name) => {
                    self.delete_machine_from_library(&name);
                }
                UiCommand::RenameMachineInLibrary(RenameMachineInLibraryArgs { old, new }) => {
                    self.rename_machine_in_library(&old, &new);
                }
                UiCommand::ToggleToolpathVisibility(tp_id) => {
                    if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id) {
                        rt.visible = !rt.visible;
                        self.pending_upload = true;
                    }
                }
                UiCommand::ToggleShowAllToolpaths(NoArgs) => {
                    // WP27 — write through the Overlays funnel, never the
                    // field: one write door keeps the panel, this button and
                    // MCP `set_ui_view` on one rule.
                    if let Some(row) = crate::ui::overlays::registry::row("all_toolpaths") {
                        let on = (row.get)(&self.state);
                        crate::ui::overlays::registry::set_overlay(&mut self.state, row, !on);
                        self.pending_upload = true;
                    }
                }
                UiCommand::InspectToolpathInSimulation(tp_id) => {
                    self.handle_inspect_toolpath_in_simulation(tp_id);
                }
                UiCommand::ToggleSimPlayback(NoArgs) => {
                    self.state.simulation.playback.playing =
                        !self.state.simulation.playback.playing;
                }
                UiCommand::ResetSimulation(NoArgs) => self.handle_reset_simulation(),
                UiCommand::SimJumpToMove(SimJumpToMoveArgs { move_index }) => {
                    self.handle_sim_jump_to_move(move_index);
                }
                UiCommand::SimStepForward(NoArgs) => self.handle_sim_step_forward(),
                UiCommand::SimStepBackward(NoArgs) => self.handle_sim_step_backward(),
                UiCommand::SimJumpToStart(NoArgs) => self.handle_sim_jump_to_start(),
                UiCommand::SimJumpToEnd(NoArgs) => self.handle_sim_jump_to_end(),
                UiCommand::SimJumpToOpStart(SimJumpToOpStartArgs { boundary_index }) => {
                    self.handle_sim_jump_to_op_start(boundary_index);
                }
                UiCommand::CancelCompute(NoArgs) => self.compute.cancel_all(),
                UiCommand::CancelOptimizeRun(NoArgs) => {
                    self.cancel_optimize_run();
                }
                UiCommand::CloseOptimizeModal(NoArgs) => {
                    // WP14b: the run is a `Job` on the SHARED FIFO Job
                    // lane, so a lane cancel would also kill an MCP
                    // caller's job queued behind it. Arm this submit's
                    // own flag instead, and leave the entry in the map:
                    // the job still completes, the drain discards the
                    // outcome against a closed modal, and the drain is
                    // where `optimize_run` goes back to `None`.
                    //
                    // This arm KEEPS arm-and-close. WP24's progress row
                    // cancels without closing, which is a different
                    // thing: if close stopped cancelling,
                    // `land_optimize_outcome` would discard the answer
                    // against a closed modal and the operator would pay
                    // for a search whose result nothing shows.
                    if self.state.is_optimizing() {
                        self.cancel_gui_optimize_jobs();
                    }
                    self.state.optimize_modal = None;
                }
                UiCommand::CloseOptimizeProject(NoArgs) => {
                    // The project rollup keeps the Optimize lane (§28
                    // ruling 6): it has no registry row, so it is still
                    // the only thing on that lane and a lane cancel
                    // reaches nobody else.
                    if self.state.is_optimizing() {
                        self.compute
                            .cancel_lane(crate::compute::ComputeLane::Optimize);
                    }
                    self.state.optimize_project = None;
                }
                UiCommand::ToggleOptimizeProjectRow(idx) => {
                    if let Some(view) = self.state.optimize_project.as_mut()
                        && let Some(slot) = view.row_selected.get_mut(idx)
                    {
                        *slot = !*slot;
                    }
                }
                // --- Multi-tool finishing planner (Phase U) ---
                UiCommand::OpenMultitoolPlanner(NoArgs) => {
                    self.open_multitool_planner();
                }
                UiCommand::CloseMultitoolPlanner(NoArgs) => {
                    self.close_multitool_planner();
                }
                UiCommand::CloseFeedsModal(NoArgs) => {
                    self.state.feeds_modal = None;
                }
                UiCommand::SetProjectFeedsOpen(open) => {
                    self.state.project_feeds.open = open;
                    if open {
                        // THE SEED (DC5a). Before the split this ran as a
                        // side effect of opening the per-operation feeds
                        // modal. It belongs to the rollup, so it runs at the
                        // rollup's own door. An unseeded rollup opens with
                        // every row unticked, which reads as "nothing to
                        // report" on a project-scope surface.
                        self.state.project_feeds.selected = enabled_toolpath_ids(&self.state);
                        self.state.project_feeds.show_scatter = true;
                    }
                }
                UiCommand::SetFeedsProjectSort(sort) => {
                    self.state.project_feeds.sort = sort;
                }
                UiCommand::SetFeedsExplore(explore) => {
                    if let Some(modal) = self.state.feeds_modal.as_mut() {
                        modal.explore = explore;
                    }
                }
                UiCommand::ToggleFeedsProjectRow(id) => {
                    if !self.state.project_feeds.selected.insert(id) {
                        self.state.project_feeds.selected.remove(&id);
                    }
                }
                UiCommand::SetFeedsProjectScatter(show) => {
                    self.state.project_feeds.show_scatter = show;
                }
                UiCommand::SetFeedsProjectSelectAll(select_all) => {
                    if select_all {
                        self.state.project_feeds.selected = enabled_toolpath_ids(&self.state);
                    } else {
                        self.state.project_feeds.selected.clear();
                    }
                }
                UiCommand::ExportGcode(NoArgs) => {}
                UiCommand::SetViewPreset(_) => {}
                UiCommand::ToggleProjection(NoArgs) => {}
                UiCommand::PreviewOrientation(_) => {}
                UiCommand::ResetView(NoArgs) => {}
                UiCommand::SwitchWorkspace(_) => {}
                UiCommand::ShowShortcuts(NoArgs) => {}
                UiCommand::SetToolLoadOverride(SetToolLoadOverrideArgs { .. }) => {}
                UiCommand::SetStaleExportPolicy(_) => {}
                UiCommand::OpenExportWizard(NoArgs) => {}
                UiCommand::CloseExportWizard(NoArgs) => {}
                UiCommand::Quit(NoArgs) => {}
                // Handled on the MCP request path alone: no GUI control
                // emits one, and no keyboard route reaches one.
                UiCommand::SimScrubToolpath(_)
                | UiCommand::SimJumpToToolpathStart(_)
                | UiCommand::SimJumpToToolpathEnd(_)
                | UiCommand::ScreenshotSimulation(_)
                | UiCommand::ScreenshotToolpath(_)
                | UiCommand::ScreenshotGui(_)
                | UiCommand::SetUiView(_) => {}
            },
        }
    }

    /// Open the per-toolpath Optimize modal.
    ///
    /// WP14b: this submits the `optimize_toolpath` `Job` row on the Job
    /// lane. Step (i) runs here on the frame loop and CLONES the session
    /// into the handle, so the view keeps its own and every panel stays
    /// readable. The modal opens in `Loading`; the answer lands through
    /// `ComputeMessage::Job` on a later drain.
    ///
    /// **The baseline trace comes from the SESSION now** (§28 ruling 5),
    /// not from the view's simulation slot. The two slots diverge after a
    /// mutation clears the session's, so an Optimize issued after an edit
    /// refuses at submit where it used to score against the older trace.
    /// That refusal renders as the same `Skipped { SimulationRequired }`
    /// card the operator already knows.
    fn open_optimize_modal(&mut self, toolpath_id: crate::state::toolpath::ToolpathId) {
        use rs_cam_core::tool_load::RefuseReason;
        use rs_cam_core::tool_load::optimize::{OptimizeOutcome, OptimizeProgress};

        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id)
        else {
            self.push_notification(
                format!("Optimize failed: toolpath id {} not found", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };

        // Refuse to start a second Optimize run while one is already in
        // flight (§28 ruling 8).
        //
        // WP24 deleted the full-screen placeholder, so the buttons that
        // fire this event are CLICKABLE during a run and the refusal is
        // reachable by hand. A `tracing::warn!` alone is invisible to the
        // operator, so the refusal pushes a toast that NAMES the run
        // holding the policy.
        if self.state.is_optimizing() {
            let busy = self.optimize_run_label();
            tracing::warn!("Ignored OpenOptimizeModal — {busy} is already running");
            self.push_notification(
                format!("{busy} is already running — one Optimize run at a time."),
                crate::controller::Severity::Warning,
            );
            return;
        }

        // §22 ruling 3: a FRESH flag per submit. The close arm arms it.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // WP29 — a FRESH progress sink per submit, beside the flag. The
        // handle writes it on the worker thread and the row reads it on the
        // frame loop.
        let progress = std::sync::Arc::new(OptimizeProgress::default());
        let started = self.state.session.start(
            rs_cam_core::session::Job::OptimizeToolpath(
                rs_cam_core::session::OptimizeToolpathArgs { index: idx },
            ),
            &cancel,
        );
        let mut handle = match started {
            Ok(handle) => handle,
            Err(rs_cam_core::session::SessionError::SimulationRequired(_)) => {
                // Surface the typed Skipped the modal already renders as
                // "run a simulation first", rather than a toast.
                self.state.close_modals_for_exclusivity();
                self.state.optimize_modal = Some(crate::state::OptimizeModalState {
                    toolpath_id,
                    status: crate::state::OptimizeRunStatus::Ready(OptimizeOutcome::skipped(
                        RefuseReason::SimulationRequired,
                    )),
                });
                return;
            }
            Err(error) => {
                self.push_notification(
                    format!("Optimize failed: {error}"),
                    crate::controller::Severity::Error,
                );
                return;
            }
        };

        // `submit_gui_job` stamps `AppState::optimize_run`, so the stamp
        // runs AFTER this call. `close_modals_for_exclusivity` SPARES a
        // running Optimize, and a stamp before it would make the rule
        // spare the previous settled modal instead of closing it.
        self.state.close_modals_for_exclusivity();
        self.state.optimize_modal = Some(crate::state::OptimizeModalState {
            toolpath_id,
            status: crate::state::OptimizeRunStatus::Loading,
        });
        // The progress rides the HANDLE, not a parameter on
        // `execute_optimize_toolpath`: that signature is pinned by
        // `optimize_toolpath_is_a_job_wp14b.rs`.
        if let rs_cam_core::session::JobHandle::OptimizeToolpath(optimize) = &mut handle {
            optimize.with_progress(std::sync::Arc::clone(&progress));
        }
        self.submit_gui_job(
            handle,
            cancel,
            crate::controller::GuiJobTarget::OptimizeModal { toolpath_id },
            Some(progress),
        );
    }

    /// Name the Optimize run in flight, for a refusal toast and a log line.
    ///
    /// One function serves both, so the sentence the operator reads names
    /// the same run the workspace bar's progress row is showing. The
    /// fallback answers the caller that asks with no run in flight; such a
    /// caller has already tested [`crate::state::AppState::is_optimizing`].
    pub(crate) fn optimize_run_label(&self) -> String {
        self.state.optimize_run.as_ref().map_or_else(
            || "An Optimize run".to_owned(),
            |run| run.kind.label(&self.state.session),
        )
    }

    /// Cancel the Optimize run in flight, and close no window (WP24).
    ///
    /// The workspace bar's progress row is the caller. It arms the ONE
    /// submit that is running — not every entry in `gui_jobs`, and not a
    /// whole lane. The `Job` lane is FIFO and shared with the MCP surface,
    /// so `cancel_lane(ComputeLane::Job)` would also kill an MCP caller's
    /// job queued behind this one. An exact cancel is possible because
    /// `OptimizeRun` carries the submit's id.
    ///
    /// The run STATE stays. The drain is the one clearing site, and a
    /// cancelled Optimize still returns a partial outcome.
    fn cancel_optimize_run(&mut self) {
        let Some(run) = self.state.optimize_run.as_ref() else {
            return;
        };
        let kind = run.kind;
        let job_id = run.job_id;
        match kind {
            crate::state::OptimizeRunKind::Toolpath { .. }
            | crate::state::OptimizeRunKind::MultitoolPreview => {
                if let Some(job) = job_id.and_then(|id| self.gui_jobs.get(&id)) {
                    job.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
            crate::state::OptimizeRunKind::Project => {
                // The rollup is the only arm left on the Optimize lane
                // (§28 ruling 6), so a lane cancel reaches nobody else.
                self.compute
                    .cancel_lane(crate::compute::ComputeLane::Optimize);
            }
        }
        if let Some(run) = self.state.optimize_run.as_mut() {
            run.cancel_requested = true;
        }
    }

    /// Arm the cancel flag of every `Job` this GUI started for the
    /// Optimize modal or the planner dialog.
    ///
    /// The entries STAY in the map. The lane still reports the job, and
    /// the drain is the one place a GUI-started job clears
    /// `AppState::optimize_run`. Removing the entry here would strand the
    /// run on the state with nothing left to clear it.
    ///
    /// This helper is the CLOSE arms' cancel, and it is deliberately broad.
    /// WP24's [`Self::cancel_optimize_run`] arms one entry instead, because
    /// the progress row knows which submit is running.
    fn cancel_gui_optimize_jobs(&self) {
        for job in self.gui_jobs.values() {
            job.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Apply a candidate from the cached Optimize outcome. Index 0 is
    /// the baseline (no-op); higher indexes select a non-baseline
    /// candidate. Routes through `Command::RestoreToolpathSnapshot`
    /// with the candidate's params and its provenance stamp.
    fn apply_optimize_candidate(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        candidate_index: usize,
    ) {
        use rs_cam_core::tool_load::optimize::OutcomeKind;

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
        let candidates = match &modal.status {
            crate::state::OptimizeRunStatus::Ready(outcome)
                if matches!(
                    outcome.kind,
                    OutcomeKind::Ranked | OutcomeKind::MarginalSafe
                ) =>
            {
                &outcome.candidates
            }
            _ => {
                self.push_notification(
                    "Apply failed: Optimize has no candidates to apply".to_owned(),
                    crate::controller::Severity::Error,
                );
                return;
            }
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

        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id)
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
        // W2.1: stamp Optimizer provenance on the dimensions the chosen
        // candidate actually changed, before the baseline operation is replaced.
        let baseline_op = tc.operation.clone();
        let new_provenance = tc
            .feeds_provenance
            .clone()
            .stamped_optimizer(&baseline_op, &candidate_op);

        // One command writes the params and the provenance stamp. Two
        // calls left an undo able to restore the params under the later
        // stamp (WP8, N14).
        let restored = self.state.session.apply(Command::RestoreToolpathSnapshot(
            RestoreToolpathSnapshotArgs {
                index: idx,
                operation: Box::new(candidate_op),
                dressups: Box::new(dressups),
                face_selection,
                feeds_provenance: Some(Box::new(new_provenance)),
            },
        ));
        let effects = match restored {
            Ok(effects) => effects,
            Err(e) => {
                self.push_notification(
                    format!("Apply failed: {e}"),
                    crate::controller::Severity::Error,
                );
                return;
            }
        };

        self.state.gui.mark_edited();
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        self.state.optimize_modal = None;

        // Roadmap F.2 — auto-verify after Apply. Set the pending flag,
        // submit the regen, and let the drain handler kick a full
        // project sim when this toolpath's regen lands. The user gets
        // a live verdict for the applied params without clicking
        // Regenerate → Run Simulation by hand.
        //
        // We only chain the re-sim when the project had a baseline
        // sim — otherwise there's nothing to "verify against" and a
        // forced sim against an unused project just wastes wall-clock
        // time.
        let had_baseline_sim = self.state.simulation.results.is_some();
        if had_baseline_sim {
            self.state.pending_apply_resim = Some(toolpath_id.0);
            self.push_notification(
                format!(
                    "Applied optimize candidate to toolpath {}; regenerating and verifying…",
                    toolpath_id.0
                ),
                crate::controller::Severity::Info,
            );
            self.submit_toolpath_compute(toolpath_id);
        } else {
            self.push_notification(
                format!(
                    "Applied optimize candidate to toolpath {}. Regenerate to take effect.",
                    toolpath_id.0
                ),
                crate::controller::Severity::Info,
            );
        }
    }

    /// OPT-005 — accept an operator suggestion from the Optimize modal:
    /// set the named axis to the suggested value on the toolpath, then
    /// re-open the modal so the search re-runs against the new baseline.
    /// An explicit operator click (never an auto-apply) that removes the
    /// manual re-typing the prose suggestions used to require.
    fn reoptimize_with_axis_override(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        axis: rs_cam_core::tool_load::optimize::KnobAxis,
        value: f64,
    ) {
        use rs_cam_core::tool_load::optimize::KnobAxis;

        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id)
        else {
            self.push_notification(
                format!(
                    "Re-optimize failed: toolpath id {} not found",
                    toolpath_id.0
                ),
                crate::controller::Severity::Error,
            );
            return;
        };
        let Some(tc) = self.state.session.get_toolpath_config(idx) else {
            self.push_notification(
                format!("Re-optimize failed: toolpath {} disappeared", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        };
        let baseline_op = tc.operation.clone();
        let mut new_op = baseline_op.clone();
        let dressups = tc.dressups.clone();
        let face_selection = tc.face_selection.clone();
        match axis {
            KnobAxis::Feed => new_op.set_feed_rate(value),
            KnobAxis::SpindleRpm => new_op.set_spindle_rpm(Some(value.round().max(0.0) as u32)),
            // These two setters report whether the operation carries the
            // field (N5). This path keeps its existing behaviour and
            // discards the answer.
            KnobAxis::Stepover => {
                new_op.set_stepover(value);
            }
            KnobAxis::DepthPerPass => {
                new_op.set_depth_per_pass(value);
            }
            KnobAxis::ScallopHeight => new_op.set_scallop_height(value),
        }
        // Checkpoint I-4 (2026-08-12): route the accepted axis value through
        // the apply funnel's clamp stage. This is the ONE optimizer write that
        // joins the funnel — the candidate applies above (O1/O3 in the A-3
        // census) stay out, deliberately, because their operating points were
        // scored against a simulated cut trace end to end and re-clamping them
        // against a pre-simulation estimator would replace the stronger
        // evidence with the weaker. This path is different in kind: it writes
        // a **suggested, never-simulated** single value, and before this it
        // did so with no clamp at all — hazard (c) in a second neighbourhood.
        // The accepted number is not re-solved; only the safety clamps run.
        // Q1: the bbox of the model this toolpath machines. The clamp
        // stage reads `SuggestContext::model_bbox`; this site used to
        // pass `None`. Bound before the closure below, which borrows
        // `self`. `upstream_leftover_stock_mm` stays `None`: no lookup
        // here gives it, and v1 does not read it.
        let model_bbox = self.state.session.model_bbox(tc.model_id);
        let clamp_warnings = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id == rs_cam_core::compute::ToolId(tc.tool_id))
            .cloned()
            .map(|tool| {
                let machine = self.state.session.machine().clone();
                let material = self.state.session.stock_config().material.clone();
                let pass_role = new_op.feeds_style().1;
                rs_cam_core::feeds::suggest::resolve_operation_invariants(
                    &mut new_op,
                    &tool,
                    &machine,
                    &material,
                    pass_role,
                    rs_cam_core::feeds::suggest::SuggestContext {
                        model_bbox: model_bbox.as_ref(),
                        ..rs_cam_core::feeds::suggest::SuggestContext::default()
                    },
                )
            })
            .unwrap_or_default();
        // W2.1: the override value originates from the optimizer's own
        // suggestion, so stamp Optimizer provenance on the changed dim.
        let new_provenance = tc
            .feeds_provenance
            .clone()
            .stamped_optimizer(&baseline_op, &new_op);

        let restored = self.state.session.apply(Command::RestoreToolpathSnapshot(
            RestoreToolpathSnapshotArgs {
                index: idx,
                operation: Box::new(new_op),
                dressups: Box::new(dressups),
                face_selection,
                feeds_provenance: Some(Box::new(new_provenance)),
            },
        ));
        let effects = match restored {
            Ok(effects) => effects,
            Err(e) => {
                self.push_notification(
                    format!("Re-optimize failed: {e}"),
                    crate::controller::Severity::Error,
                );
                return;
            }
        };
        if !clamp_warnings.is_empty() {
            // A clamp that fires silently is the defect, not the fix: the
            // operator accepted a number and got a different one.
            self.push_notification(
                format!(
                    "Accepted {axis:?} suggestion, then clamped it for safety: {}",
                    clamp_warnings
                        .iter()
                        .map(|w| format!("{w:?}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                crate::controller::Severity::Warning,
            );
        }
        self.state.gui.mark_edited();
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        // Re-run the search against the new baseline. open_optimize_modal
        // reuses the cached baseline trace; the modal's FreshnessGate
        // banner flags that the trace is now one edit behind.
        self.open_optimize_modal(toolpath_id);
    }

    // ── Feeds & Speeds modal ───────────────────────────────────────

    /// Open the redesigned Feeds & Speeds modal for the given
    /// toolpath. Idempotent — re-opening for the same toolpath just
    /// refocuses; opening for a different toolpath swaps the target.
    /// The modal re-derives `FeedsExplain` every frame, so no payload
    /// needs to be stashed here.
    fn open_feeds_modal(&mut self, toolpath_id: crate::state::toolpath::ToolpathId) {
        // Validate that the toolpath exists. The button that fires
        // this event should already be hidden when no toolpath is
        // selected, but a stray automation call could land here.
        if !self
            .state
            .session
            .toolpath_configs()
            .iter()
            .any(|tc| tc.id == toolpath_id)
        {
            self.push_notification(
                format!("Feeds modal: toolpath id {} not found", toolpath_id.0),
                crate::controller::Severity::Error,
            );
            return;
        }
        self.state.close_modals_for_exclusivity();
        self.state.feeds_modal = Some(crate::state::FeedsModalState {
            toolpath_id,
            explore: None,
        });
    }

    /// **The GUI's one apply path from a recommendation into an
    /// `OperationConfig`** (Checkpoint I, 2026-08-12).
    ///
    /// Every Feeds & Speeds modal write — the comparison card's `⚡ Apply
    /// all`, the project rollup's per-row Apply, `⚡ Apply selected`, `⚡⚡
    /// Apply all toolpaths`, and the nomogram's explore apply — lands here,
    /// and here calls `feeds::suggest::apply`. Two consequences are the whole
    /// point of the wave:
    ///
    /// 1. A tool × operation pairing the engine refuses **cannot be written**,
    ///    because `FeedsPreview::applicable` hands back nothing to write.
    ///    Pre-fix the modal happily applied recipes the properties panel would
    ///    not even display (A-3 §3.1).
    /// 2. The values written are the invariant-resolved ones — the plunge and
    ///    stepover clamps, the rigidity / cutting-length DOC clamps and the
    ///    deflection back-off all run. Pre-fix the per-field buttons skipped
    ///    the lot and wrote 4.445 mm of DOC where the funnel writes 1.27 mm.
    ///
    /// `Err` carries the refusal text, so batch callers can report which rows
    /// they skipped instead of sweeping them up silently (A-3 §3.5).
    fn apply_feeds_through_funnel(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        scope: rs_cam_core::feeds::suggest::ApplyScope,
        explored_speeds: Option<(f64, f64)>,
    ) -> Result<Effects, String> {
        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id)
        else {
            return Err(format!("toolpath {} not found", toolpath_id.0));
        };
        let (operation, tool_id, pass_role) = {
            let Some(tc) = self.state.session.toolpath_configs().get(idx) else {
                return Err(format!("toolpath {} disappeared", toolpath_id.0));
            };
            (
                tc.operation.clone(),
                tc.tool_id,
                tc.operation.feeds_style().1,
            )
        };
        let Some(tool) = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id == rs_cam_core::compute::ToolId(tool_id))
            .cloned()
        else {
            return Err(format!("toolpath {} has no tool", toolpath_id.0));
        };
        let machine = self.state.session.machine().clone();
        let material = self.state.session.stock_config().material.clone();
        let spindle_strategy = self.state.session.post_config().spindle_strategy;

        let preview = rs_cam_core::feeds::suggest::feeds_preview_for_operation(
            &operation,
            &tool,
            &material,
            &machine,
            rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy,
        );
        let Some(rec) = preview.applicable() else {
            return Err(preview
                .refusal()
                .map(ToString::to_string)
                .unwrap_or_else(|| "recommendation is not applicable".to_owned()));
        };
        let rec = match explored_speeds {
            Some((feed_mm_min, rpm)) => rec.with_explored_speeds(feed_mm_min, rpm),
            None => rec,
        };

        // The funnel writes a DRAFT clone and applies it through the
        // command door. `ReplaceToolpathConfig` writes the configuration
        // unconditionally and gates its drop on
        // `ToolpathConfig::generation_inputs_signature` — which IS the
        // N13 rule this arm used to run by hand, in core's own words and
        // at one site (WP5).
        let Some(mut draft) = self.state.session.toolpath_configs().get(idx).cloned() else {
            return Err(format!("toolpath {} disappeared", toolpath_id.0));
        };
        // Q1: the bbox of the model this toolpath machines. The apply
        // funnel's clamp stage reads `SuggestContext::model_bbox`; this
        // site used to pass `None`. `upstream_leftover_stock_mm` stays
        // `None`: no lookup here gives it, and v1 does not read it.
        let model_bbox = self.state.session.model_bbox(draft.model_id);
        rs_cam_core::feeds::suggest::apply(
            &rec,
            scope,
            &mut draft.operation,
            &mut draft.feeds_provenance,
            rs_cam_core::feeds::suggest::ApplyContext {
                tool: &tool,
                machine: &machine,
                material: &material,
                pass_role,
                suggest: rs_cam_core::feeds::suggest::SuggestContext {
                    model_bbox: model_bbox.as_ref(),
                    ..rs_cam_core::feeds::suggest::SuggestContext::default()
                },
            },
        );
        let command = Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: idx,
            config: Box::new(draft),
        });
        let effects = self
            .state
            .session
            .apply(command)
            .map_err(|error| format!("toolpath {}: {error}", toolpath_id.0))?;
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        // WP19 (plan §28): N13 made this funnel drop the core result, so
        // the row clears the session's simulation on the frames the
        // signature moves. The viewport holds the same one.
        if effects.simulation_cleared {
            self.invalidate_simulation();
        }
        self.state.gui.mark_edited();
        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id) {
            rt.stale_since = Some(std::time::Instant::now());
        }
        Ok(effects)
    }

    /// The agent-facing entry to the same funnel (Checkpoint I-5).
    ///
    /// Before this, the MCP surface had **no** apply tool at all: an agent's
    /// only write was `set_toolpath_param`, a raw operator write that is
    /// neither feeds-validated nor invariant-funnelled — i.e. the agent had
    /// the old modal's contract with none of the modal's preview. This gives
    /// it the panel's guarantees instead, with the scope stated explicitly
    /// rather than implied by which button was clicked.
    ///
    /// `Err` carries the refusal text verbatim, so the tool can return it to
    /// the agent instead of reporting a successful no-op.
    ///
    /// `Ok` carries the funnel's own [`Effects`]. WP28 part 2: the MCP
    /// reply reports `Effects::stale` — the set the command's setter
    /// dropped — instead of the tag-driven answer `compute_stale_set`
    /// gave, which named the edited index alone and walked no stock
    /// chain (N15). The funnel has already stamped that set on the view,
    /// so a caller reads the field and stamps nothing of its own.
    pub fn apply_feeds_recommendation(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        scope: rs_cam_core::feeds::suggest::ApplyScope,
    ) -> Result<Effects, String> {
        self.apply_feeds_through_funnel(toolpath_id, scope, None)
    }

    /// Apply every recommended Feeds value to the given toolpath in one
    /// transactional update — the modal's `⚡ Apply all`, and the project
    /// rollup's per-row Apply.
    ///
    /// This writes DOC/WOC as well as the speeds, which is why both buttons
    /// now say "changes the cut" on their face. On a refused pairing the modal
    /// draws the refusal instead of the button, so reaching this with an
    /// unrunnable pairing means the state moved under an open modal — it
    /// refuses and notifies rather than writing.
    fn apply_feeds_all(&mut self, toolpath_id: crate::state::toolpath::ToolpathId) {
        if let Err(why) = self.apply_feeds_through_funnel(
            toolpath_id,
            rs_cam_core::feeds::suggest::ApplyScope::Both,
            None,
        ) {
            self.push_notification(
                format!("Feeds not applied to toolpath {}: {why}", toolpath_id.0),
                crate::controller::Severity::Warning,
            );
        }
    }

    /// Apply only the recommended SPEEDS to the given toolpath — the
    /// comparison card's `Match vendor chipload`.
    ///
    /// Phase C of `planning/load_model_2026-09-16/SPEC.md`. The chipload
    /// verdict row can tell the operator their chipload is thin and costs
    /// them 1.6× tool wear; before this the only write on the Feeds tab was
    /// `⚡ Apply all`, so the only offered answer to "my feed is wrong" also
    /// rewrote DOC and WOC. This one holds the cut geometry byte-identical
    /// and moves feed, plunge and RPM alone.
    ///
    /// Same funnel, same refusal rule, same staleness as
    /// [`Self::apply_feeds_all`] — only the [`ApplyScope`] differs.
    ///
    /// [`ApplyScope`]: rs_cam_core::feeds::suggest::ApplyScope
    fn apply_feeds_speeds(&mut self, toolpath_id: crate::state::toolpath::ToolpathId) {
        if let Err(why) = self.apply_feeds_through_funnel(
            toolpath_id,
            rs_cam_core::feeds::suggest::ApplyScope::Speeds,
            None,
        ) {
            self.push_notification(
                format!("Feeds not applied to toolpath {}: {why}", toolpath_id.0),
                crate::controller::Severity::Warning,
            );
        }
    }

    /// S1 — set or clear the scallop-driven-stepover target on a
    /// DropCutter toolpath. `None` restores the formula-based stepover.
    /// No-op when the toolpath isn't a DropCutter.
    fn set_drop_cutter_scallop_height(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        value: Option<f64>,
    ) {
        let Some(idx) = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == toolpath_id)
        else {
            return;
        };
        // A draft plus one `ReplaceToolpathConfig`. The dial is an
        // `Option<f64>`, which `set_toolpath_param`'s JSON value cannot
        // carry cleanly, and the row's signature gate drops the chain the
        // scallop target belongs to.
        let Some(mut draft) = self.state.session.toolpath_configs().get(idx).cloned() else {
            return;
        };
        if let rs_cam_core::compute::catalog::OperationConfig::DropCutter(cfg) =
            &mut draft.operation
        {
            if cfg.scallop_height == value {
                return;
            }
            cfg.scallop_height = value;
        } else {
            return;
        }
        let command = Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: idx,
            config: Box::new(draft),
        });
        if !self.apply_controller_command(command, "the scallop target") {
            return;
        }
        self.state.gui.mark_edited();
        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id) {
            rt.stale_since = Some(std::time::Instant::now());
        }
    }

    /// Apply Feeds recommendations to every toolpath whose row is
    /// currently checked in the project rollup.
    ///
    /// DC5a repointed this at `state.project_feeds`. It used to read the
    /// selection off the per-operation feeds modal, so `Apply selected`
    /// applied to NOTHING once that modal was closed — the same defect as
    /// the sort and the scatter toggle, on the one control that writes.
    fn apply_feeds_project_selected(&mut self) {
        let ids: Vec<crate::state::toolpath::ToolpathId> =
            self.state.project_feeds.selected.iter().copied().collect();
        self.apply_feeds_batch(&ids, "selected toolpaths");
    }

    /// Apply Feeds recommendations across every enabled toolpath
    /// (Phase 4). Single transactional sweep.
    fn apply_feeds_project(&mut self) {
        let ids: Vec<crate::state::toolpath::ToolpathId> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.id)
            .collect();
        self.apply_feeds_batch(&ids, "every enabled toolpath");
    }

    /// Fan the funnel across a set of toolpaths and **report what it
    /// skipped**.
    ///
    /// A-3 §3.5 measured the pre-fix behaviour: the batch called the
    /// infallible path per id, so a refused pairing sitting anywhere in the
    /// project took the write silently, inside a sweep the user believed they
    /// understood. Now a refused row is skipped and named — the batch is
    /// allowed to be partial, but never quietly.
    fn apply_feeds_batch(&mut self, ids: &[crate::state::toolpath::ToolpathId], what: &str) {
        let mut applied = 0usize;
        let mut skipped: Vec<String> = Vec::new();
        for &id in ids {
            match self.apply_feeds_through_funnel(
                id,
                rs_cam_core::feeds::suggest::ApplyScope::Both,
                None,
            ) {
                Ok(_) => applied += 1,
                Err(why) => {
                    let name = self
                        .state
                        .session
                        .toolpath_configs()
                        .iter()
                        .find(|tc| tc.id == id)
                        .map(|tc| tc.name.clone())
                        .unwrap_or_else(|| format!("toolpath {}", id.0));
                    skipped.push(format!("{name} ({why})"));
                }
            }
        }
        if skipped.is_empty() {
            self.push_notification(
                format!("Applied Feeds recommendations to {applied} of {what}."),
                crate::controller::Severity::Info,
            );
        } else {
            self.push_notification(
                format!(
                    "Applied Feeds recommendations to {applied} of {what}; skipped {} \
                     that Suggest refused: {}",
                    skipped.len(),
                    skipped.join("; ")
                ),
                crate::controller::Severity::Warning,
            );
        }
    }

    /// Apply a custom (feed, RPM) pair from the Chart C drag-to-explore
    /// release. Other fields stay as-is.
    ///
    /// Checkpoint I-1 routed this through the funnel: it used to write the two
    /// dragged values with a bare `set_feed_rate` / `set_spindle_rpm`, so a
    /// feed dragged below the operation's plunge rate left the machine
    /// plunging faster than it cut. It now goes through
    /// `ApplyScope::Speeds`, which takes the clamps. The dragged values
    /// themselves are **not** re-solved — see
    /// `ApplicableRecommendation::with_explored_speeds` for why the chipload
    /// band is dropped on this path.
    fn apply_feeds_explore(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        feed_mm_min: f64,
        rpm: f64,
    ) {
        if let Err(why) = self.apply_feeds_through_funnel(
            toolpath_id,
            rs_cam_core::feeds::suggest::ApplyScope::Speeds,
            Some((feed_mm_min, rpm)),
        ) {
            self.push_notification(
                format!(
                    "Explored values not applied to toolpath {}: {why}",
                    toolpath_id.0
                ),
                crate::controller::Severity::Warning,
            );
        }
    }

    // NOTE (A-4, 2026-08-12): the `compute_feeds_explain` helper that used to
    // sit here is gone. It handed the *infallible* explain payload to the
    // apply handlers, which is how a refused pairing became writable in the
    // first place — the payload has no slot to say "this tool cannot run this
    // operation", so the handler had nothing to check. Apply paths now build a
    // `FeedsPreview` inside `apply_feeds_through_funnel`, which carries the
    // refusal and the numbers together.

    /// Open the project-level Optimize rollup. Submits an
    /// `OptimizeRequest::Project` to the Optimize lane over a CLONE of
    /// the session. The view opens in `Loading` immediately and the
    /// rollup populates when the worker returns.
    ///
    /// The rollup has no registry row and gets none in WP14b (§28 ruling
    /// 6). It stays on this lane; what changed is that the session is
    /// copied rather than moved, so the view keeps its own and the result
    /// carries none back.
    fn open_optimize_project(&mut self) {
        // Pre-flight: needs a baseline trace.
        let trace_clone = self
            .state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.clone());
        let Some(trace) = trace_clone else {
            self.push_notification(
                "Run a simulation first — Optimize needs a baseline trace.".to_owned(),
                crate::controller::Severity::Warning,
            );
            return;
        };

        if self.state.is_optimizing() {
            let busy = self.optimize_run_label();
            tracing::warn!("Ignored OpenOptimizeProject — {busy} is already running");
            self.push_notification(
                format!("{busy} is already running — one Optimize run at a time."),
                crate::controller::Severity::Warning,
            );
            return;
        }

        // The walk mutates what it scores, so it needs a session of its
        // own. It takes a COPY; the view keeps the original.
        let session = self.state.session.clone();
        // The stamp runs AFTER the exclusivity call, for the reason
        // `open_optimize_modal` records: the rule spares a RUNNING
        // Optimize, so a stamp before it would spare the previous settled
        // modal. The rollup stamps itself rather than going through
        // `submit_gui_job`, because it rides `ComputeLane::Optimize` and
        // carries no `Job` id.
        self.state.close_modals_for_exclusivity();
        self.state.optimize_run = Some(crate::state::OptimizeRun {
            kind: crate::state::OptimizeRunKind::Project,
            job_id: None,
            started_at: std::time::Instant::now(),
            cancel_requested: false,
            // The rollup runs `optimize_toolpath` per toolpath through its
            // own lane and attaches no progress sink, so the row shows the
            // elapsed seconds alone. The follow-on is one line —
            // `impl ProgressReporter for OptimizeProgress` — and it is
            // NOT in WP29's scope.
            progress: None,
        });
        self.state.optimize_project = Some(crate::state::OptimizeProjectState {
            status: crate::state::OptimizeProjectStatus::Loading,
            row_selected: Vec::new(),
        });
        self.compute
            .submit_optimize(crate::compute::OptimizeRequest::Project {
                session,
                baseline_trace: trace,
            });
    }

    /// Apply every selected row from the project rollup. Each row's
    /// candidate is the first-safe recommendation from that row's
    /// outcome. Routes through `Command::RestoreToolpathSnapshot` for
    /// each toolpath; closes the rollup and marks every touched
    /// toolpath — and every toolpath downstream of one — stale so
    /// auto-regen kicks in.
    fn apply_optimize_project(&mut self) {
        use rs_cam_core::tool_load::optimize::OutcomeKind;

        // Lookup phase: pull out (toolpath_id, params, delta) tuples
        // from the cached state. Drop the borrow before any mutation.
        let Some(view) = self.state.optimize_project.as_ref() else {
            return;
        };
        let crate::state::OptimizeProjectStatus::Ready(report) = &view.status else {
            return;
        };
        let mut targets: Vec<(usize, rs_cam_core::compute::catalog::OperationConfig)> = Vec::new();
        for (idx, ((toolpath_index, outcome), selected)) in report
            .per_toolpath
            .iter()
            .zip(view.row_selected.iter())
            .enumerate()
        {
            if !selected {
                continue;
            }
            if outcome.kind != OutcomeKind::Ranked {
                continue;
            }
            let Some(candidate) = outcome.first_safe() else {
                tracing::debug!("Skipping row {idx}: no first_safe candidate");
                continue;
            };
            targets.push((*toolpath_index, candidate.params.clone()));
        }

        if targets.is_empty() {
            self.push_notification(
                "No applicable rows selected.".to_owned(),
                crate::controller::Severity::Info,
            );
            return;
        }

        let mut applied: usize = 0;
        let mut failed: Vec<String> = Vec::new();
        for (toolpath_index, params) in targets {
            let Some(tc) = self.state.session.get_toolpath_config(toolpath_index) else {
                failed.push(format!("toolpath idx {toolpath_index} disappeared"));
                continue;
            };
            let dressups = tc.dressups.clone();
            let face_selection = tc.face_selection.clone();
            // W2.1: stamp Optimizer on changed dimensions before applying.
            let baseline_op = tc.operation.clone();
            let new_provenance = tc
                .feeds_provenance
                .clone()
                .stamped_optimizer(&baseline_op, &params);

            let restored = self.state.session.apply(Command::RestoreToolpathSnapshot(
                RestoreToolpathSnapshotArgs {
                    index: toolpath_index,
                    operation: Box::new(params),
                    dressups: Box::new(dressups),
                    face_selection,
                    feeds_provenance: Some(Box::new(new_provenance)),
                },
            ));
            let effects = match restored {
                Ok(effects) => effects,
                Err(e) => {
                    failed.push(format!("toolpath idx {toolpath_index}: {e}"));
                    continue;
                }
            };
            // Mark stale so auto-regen picks it up. The set covers the
            // rows downstream of this one too.
            crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
            applied += 1;
        }

        self.state.gui.mark_edited();
        // Keep the rollup open. The drain handler will transition
        // Reconciling -> Reconciled when the project sim completes.
        // The intervening regen is async via the GUI's auto-regen
        // path: each touched toolpath is marked stale; the regen
        // results land on `gui.toolpath_rt`, and we kick the sim
        // once they all complete (see `maybe_kick_reconciliation_sim`).
        let touched_ids: Vec<rs_cam_core::ToolpathId> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .map(|tc| tc.id)
            .collect();
        if let Some(view) = self.state.optimize_project.as_mut()
            && let crate::state::OptimizeProjectStatus::Ready(report) = &view.status
        {
            view.status = crate::state::OptimizeProjectStatus::Reconciling(report.clone());
        }
        self.state.pending_reconciliation_for_ids = touched_ids;
        self.push_notification(
            if failed.is_empty() {
                format!(
                    "Applied {applied} candidate(s). Regenerating + running reconciliation sim…"
                )
            } else {
                format!(
                    "Applied {applied} candidates; {failed_count} failed: {first}",
                    failed_count = failed.len(),
                    first = failed.first().cloned().unwrap_or_default()
                )
            },
            if failed.is_empty() {
                crate::controller::Severity::Info
            } else {
                crate::controller::Severity::Warning
            },
        );

        // Submit regen for every enabled toolpath (worker lane). The
        // toolpath-drain handler watches `pending_reconciliation_for_ids`
        // and kicks the sim once they're all done.
        let enabled_ids: Vec<crate::state::toolpath::ToolpathId> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.id)
            .collect();
        for id in enabled_ids {
            self.submit_toolpath_compute(id);
        }
    }
}
