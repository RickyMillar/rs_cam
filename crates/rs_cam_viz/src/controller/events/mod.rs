mod compute;
mod model;
mod model_faces;
pub(crate) mod pins;
mod planner;
mod preferences;
pub(crate) mod simulation;
mod toolpath;
mod undo;

use rs_cam_core::session::{
    Command, EditSelectionArgs, Effects, ReplaceToolpathConfigArgs, SetPostConfigArgs,
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

    /// Apply one edit to the selection `target` of `toolpath_id` (geometry
    /// selection P2.6, P4.1): the one door of the viewport pick and the
    /// panel edits. A refusal (an op that holds no selection, or a boundary
    /// whose source is not a selection) is named in a notification. N6
    /// (WP8): the stamp follows `Effects::stale`, which holds the
    /// downstream stock chain.
    ///
    /// The edit lands in `operation` or in `boundary`, and the inspector
    /// undo snapshot of the toolpath (`toolpath_snapshot`) covers both
    /// (D11).
    fn apply_selection_edit(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        target: rs_cam_core::session::SelectionTarget,
        edit: rs_cam_core::selection::SelectionEdit,
    ) {
        let Some((index, _)) = self.state.session.find_toolpath_config_by_id(toolpath_id) else {
            return;
        };
        let command = Command::EditSelection(EditSelectionArgs {
            index,
            target,
            edit,
        });
        if self.apply_controller_command(command, "the selection") {
            self.state.selection = Selection::Toolpath(toolpath_id);
            self.state.gui.mark_edited();
            self.pending_upload = true;
        }
    }

    /// One act of the viewport tab editor (geometry selection P4.6): read
    /// the tab layout (core, the tabs generation cuts), build the new
    /// `tab_points` (`apply_tab_edit`: the L-5 conversion, the O-4 refusal)
    /// and write them with ONE `SetToolpathParam`. A refusal is a notice
    /// and writes nothing.
    ///
    /// One act is one undo step. The inspector undo snapshot of the
    /// toolpath (`toolpath_snapshot`) flushes as one step when the operator
    /// leaves the toolpath, so this door first pushes the edits the
    /// snapshot holds as their own step, then the act, and then moves the
    /// snapshot to the state after the act. The flush then holds only the
    /// later panel edits.
    fn apply_tab_edit(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        edit: &rs_cam_core::session::TabEdit,
    ) {
        use crate::state::history::UndoAction;
        use rs_cam_core::compute::catalog::OperationConfig;
        let Some((index, tc)) = self.state.session.find_toolpath_config_by_id(toolpath_id) else {
            return;
        };
        let OperationConfig::Profile(cfg) = &tc.operation else {
            return;
        };
        let points =
            crate::state::tab_edit::read_layout(&self.state.session, index).and_then(|layout| {
                rs_cam_core::session::apply_tab_edit(&layout, &cfg.tab_points, edit)
            });
        let points = match points {
            Ok(points) if points == cfg.tab_points => return,
            Ok(points) => points,
            Err(refusal) => {
                self.push_notification(
                    format!("Could not edit the tabs: {refusal}"),
                    crate::controller::Severity::Warning,
                );
                return;
            }
        };
        let before = (
            tc.operation.clone(),
            tc.dressups.clone(),
            tc.boundary.clone(),
            tc.feeds_provenance.clone(),
        );
        let command = Command::SetToolpathParam(rs_cam_core::session::SetToolpathParamArgs {
            index,
            param: "tab_points".to_owned(),
            value: serde_json::json!(points),
        });
        if !self.apply_controller_command(command, "the tabs") {
            return;
        }
        let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(toolpath_id) else {
            return;
        };
        let after = (
            tc.operation.clone(),
            tc.dressups.clone(),
            tc.boundary.clone(),
            tc.feeds_provenance.clone(),
        );
        let same = |a: &crate::state::toolpath::OperationConfig,
                    b: &crate::state::toolpath::OperationConfig,
                    c: &crate::state::toolpath::DressupConfig,
                    d: &crate::state::toolpath::DressupConfig| {
            serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
                && serde_json::to_value(c).ok() == serde_json::to_value(d).ok()
        };
        // The panel edits since the snapshot: their own step first.
        if let Some((snap_id, old_op, old_dressups, old_boundary, old_provenance)) =
            self.state.history.toolpath_snapshot.take()
        {
            if snap_id == toolpath_id {
                let moved = !same(&old_op, &before.0, &old_dressups, &before.1)
                    || old_boundary != before.2
                    || old_provenance != before.3;
                if moved {
                    self.state.history.push(UndoAction::ToolpathParamChange {
                        tp_id: toolpath_id,
                        old_op,
                        new_op: before.0.clone(),
                        old_dressups,
                        new_dressups: before.1.clone(),
                        old_boundary,
                        new_boundary: before.2.clone(),
                        old_feeds_provenance: old_provenance,
                        new_feeds_provenance: before.3.clone(),
                    });
                }
                self.state.history.toolpath_snapshot = Some((
                    toolpath_id,
                    after.0.clone(),
                    after.1.clone(),
                    after.2.clone(),
                    after.3.clone(),
                ));
            } else {
                self.state.history.toolpath_snapshot =
                    Some((snap_id, old_op, old_dressups, old_boundary, old_provenance));
            }
        }
        self.state.history.push(UndoAction::ToolpathParamChange {
            tp_id: toolpath_id,
            old_op: before.0,
            new_op: after.0,
            old_dressups: before.1,
            new_dressups: after.1,
            old_boundary: before.2,
            new_boundary: after.2,
            old_feeds_provenance: before.3,
            new_feeds_provenance: after.3,
        });
        self.state.selection = Selection::Toolpath(toolpath_id);
        self.state.gui.mark_edited();
        self.pending_upload = true;
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
            // P4.8: a drawing opens the import dialog; Import adds it.
            AppEvent::ImportSvg(path) => {
                self.open_import_dialog(&path, crate::state::job::ModelKind::Svg);
            }
            AppEvent::ImportDxf(path) => {
                self.open_import_dialog(&path, crate::state::job::ModelKind::Dxf);
            }
            AppEvent::ImportDialogEdit(units, options) => {
                self.edit_import_dialog(units, options);
            }
            AppEvent::ImportDialogConfirm => {
                if let Err(error) = self.confirm_import_dialog() {
                    self.push_error(&error);
                }
            }
            AppEvent::ImportDialogCancel => self.cancel_import_dialog(),
            AppEvent::SetModelImportOptions(model_id, options) => {
                if let Err(error) = self.set_model_import_options(model_id, options) {
                    self.push_error(&error);
                }
            }
            AppEvent::EditModelDrawing { model_id, edit } => {
                if let Err(error) = self.edit_drawing_ledger(model_id, edit) {
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
            AppEvent::PinAddSetup(face_up) => self.handle_pin_add_setup(face_up),
            AppEvent::PinPartModels(models) => self.set_pin_part_models(models),
            AppEvent::PinWizardApply {
                pins,
                add_drills,
                use_tools,
            } => {
                let _ = self.apply_pin_wizard(pins, &add_drills, &use_tools);
            }
            AppEvent::RenameSetup(setup_id, name) => self.handle_rename_setup(setup_id, name),
            AppEvent::AddObstacle(setup_id) => self.handle_add_obstacle(setup_id),
            AppEvent::RemoveObstacle(setup_id, obstacle_id) => {
                self.handle_remove_obstacle(setup_id, obstacle_id);
            }
            AppEvent::SetObstacleEnabled(setup_id, obstacle_id, enabled) => {
                self.handle_set_obstacle_enabled(setup_id, obstacle_id, enabled);
            }
            AppEvent::SetToolpathShoe { tp_id, shoe, merge } => {
                self.handle_set_toolpath_shoe(tp_id, shoe, merge);
            }
            AppEvent::EditStockChanges(setup_id, intent) => {
                self.handle_stock_change_intent(setup_id, intent);
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
            AppEvent::CompareRestWithFresh(tp_id) => {
                if let Err(error) = self.compare_rest_with_fresh(tp_id) {
                    self.push_notification(
                        format!("Could not compare with fresh stock: {error}"),
                        super::Severity::Warning,
                    );
                }
            }

            // --- Geometry selection (P2.6) ---
            AppEvent::TogglePick {
                toolpath_id,
                target,
                key,
            } => {
                // Lead ruling P2.2: no toggle edit. Resolve first through
                // THE resolution (the `selection_status` memo), then send
                // `AddPick` or `RemovePick`. A selected item that no pick
                // names is a rule match; `RemovePick` excludes it.
                let edit = self
                    .state
                    .session
                    .find_toolpath_config_by_id(toolpath_id)
                    .map(|(_, tc)| self.state.session.pick_edit(tc, target, key));
                match edit {
                    Some(Ok(edit)) => self.apply_selection_edit(toolpath_id, target, edit),
                    Some(Err(error)) => self.push_notification(
                        format!("Could not read the selection: {error}"),
                        super::Severity::Warning,
                    ),
                    None => {}
                }
            }
            AppEvent::PickLayer {
                toolpath_id,
                target,
                key,
                kinds,
                circle,
            } => {
                // Geometry selection P4.5: core builds the layer rule edit
                // (the replace-under-default rule lives there, beside
                // `pick_edit`), so MCP and the GUI agree. An item with no
                // layer gives a notice and no edit.
                let edit = self
                    .state
                    .session
                    .find_toolpath_config_by_id(toolpath_id)
                    .map(|(_, tc)| {
                        self.state
                            .session
                            .layer_rule_edit(tc, target, &key, &kinds, circle)
                    });
                match edit {
                    Some(Ok(edit)) => self.apply_selection_edit(toolpath_id, target, edit),
                    Some(Err(error)) => self.push_notification(
                        format!("Could not add the layer: {error}"),
                        super::Severity::Warning,
                    ),
                    None => {}
                }
            }
            AppEvent::EditToolpathSelection {
                toolpath_id,
                target,
                edit,
            } => {
                self.apply_selection_edit(toolpath_id, target, edit);
            }
            AppEvent::EditTabs { toolpath_id, edit } => {
                self.apply_tab_edit(toolpath_id, &edit);
            }

            // --- Undo / redo ---
            AppEvent::Undo => self.undo(),
            AppEvent::Redo => self.redo(),

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

            // --- File ▸ Preferences ---
            AppEvent::OpenPreferences => self.open_preferences(),
            AppEvent::ApplyPreferences => self.apply_preferences(),

            // --- Pass-through events handled elsewhere ---
            AppEvent::ExportCombinedGcode
            | AppEvent::ExportSetupGcode(_)
            | AppEvent::ExportGcodeConfirmed
            | AppEvent::ExportSetupSheet
            | AppEvent::ExportSvgPreview
            | AppEvent::FrameSelectionItems { .. }
            | AppEvent::FrameModelItems { .. }
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
            | AppEvent::WizardSetControllerOption(..)
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
                UiCommand::CancelJobRun(NoArgs) => {
                    self.cancel_job_run();
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

    /// Name the GUI job in flight, for a refusal toast and a log line.
    ///
    /// One function serves both, so the sentence the operator reads names
    /// the same run the workspace bar's progress row is showing. The
    /// fallback answers the caller that asks with no run in flight; such a
    /// caller has already tested [`crate::state::AppState::is_job_running`].
    pub(crate) fn job_run_label(&self) -> String {
        self.state
            .job_run
            .as_ref()
            .map_or_else(|| "A job".to_owned(), |run| run.kind.label().to_owned())
    }

    /// Cancel the GUI job in flight, and close no window (WP24).
    ///
    /// The workspace bar's progress row is the caller. It arms the ONE
    /// submit that is running — not every entry in `gui_jobs`, and not a
    /// whole lane. The `Job` lane is FIFO and shared with the MCP surface,
    /// so `cancel_lane(ComputeLane::Job)` would also kill an MCP caller's
    /// job queued behind this one. An exact cancel is possible because
    /// `JobRun` carries the submit's id.
    ///
    /// The run STATE stays. The drain is the one clearing site.
    fn cancel_job_run(&mut self) {
        let Some(run) = self.state.job_run.as_mut() else {
            return;
        };
        if let Some(job) = self.gui_jobs.get(&run.job_id) {
            job.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        run.cancel_requested = true;
    }

    /// Arm the cancel flag of every `Job` this GUI started for the planner
    /// dialog.
    ///
    /// The entries STAY in the map. The lane still reports the job, and
    /// the drain is the one place a GUI-started job clears
    /// `AppState::job_run`. Removing the entry here would strand the run
    /// on the state with nothing left to clear it.
    ///
    /// This helper is the CLOSE arm's cancel, and it is deliberately broad.
    /// WP24's [`Self::cancel_job_run`] arms one entry instead, because the
    /// progress row knows which submit is running.
    fn cancel_gui_jobs(&self) {
        for job in self.gui_jobs.values() {
            job.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
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
                    // G6 ramp: the entry θ of this toolpath, as the card
                    // passes it, so the two show one number.
                    dressups: Some(&draft.dressups),
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
        // B19: `stamp_stale` above is the only stamp. An Apply that changes
        // no generation input stales nothing, so auto-regen submits nothing.
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
        // B19: `apply_controller_command` stamps the stale set core returns.
        self.state.gui.mark_edited();
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
}
