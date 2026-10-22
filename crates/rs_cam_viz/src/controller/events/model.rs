use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::session::{
    AddObstacleArgs, AddSetupArgs, AddToolArgs, Command, Obstacle, RemoveModelArgs,
    RemoveObstacleArgs, RemoveSetupArgs, RemoveToolArgs, ReplaceObstacleArgs, SetAlignmentPinsArgs,
    SetMachineArgs, SetSetupNameArgs, SetSetupPauseMessageArgs,
};

use crate::compute::ComputeBackend;
use crate::state::job::{ModelId, SetupId, ToolConfig};
use crate::state::selection::Selection;
use crate::ui::properties::pins::{KeyedPinInputs, keyed_pin_plan};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Tree / selection helpers ─────────────────────────────────────────

    pub(crate) fn handle_select(&mut self, selection: &Selection) {
        let old_setup = match &self.state.selection {
            Selection::Setup(id) => Some(*id),
            Selection::Obstacle(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(*tp_id),
            _ => None,
        };
        let new_setup = match selection {
            Selection::Setup(id) => Some(*id),
            Selection::Obstacle(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(*tp_id),
            _ => None,
        };
        // `upload_gpu_data` (app/gpu_upload.rs) keys the height-plane and
        // rest-heatmap overlays off "whichever toolpath is currently
        // `Selection::Toolpath`", rebuilt only when `pending_upload` fires —
        // not every frame. The setup-changed check above catches switching
        // setups, but switching the selected *toolpath within the same
        // setup* (e.g. selecting a pencil rest-analysis toolpath right
        // after a scallop in the same setup) left both overlays showing
        // stale data from whichever toolpath was selected the last time
        // something else happened to set `pending_upload`. Any actual
        // selection change needs the same treatment.
        if old_setup != new_setup || self.state.selection != *selection {
            self.pending_upload = true;
        }
        self.state.selection = selection.clone();
    }

    /// Find the setup that owns a given toolpath ID.
    pub(crate) fn setup_of_toolpath(
        &self,
        tp_id: crate::state::toolpath::ToolpathId,
    ) -> Option<SetupId> {
        // Find which setup contains this toolpath by checking toolpath_indices
        let (tp_index, _) = self.state.session().find_toolpath_config_by_id(tp_id)?;
        self.state
            .session()
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices().contains(&tp_index))
            .map(|s| s.id)
    }

    pub(crate) fn handle_add_tool(&mut self, tool_type: crate::state::job::ToolType) {
        let tool = ToolConfig::new_default(crate::state::job::ToolId::placeholder(), tool_type);
        let command = Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        });
        let created = self.apply_created(command);
        // The session assigned the ID — read it back.
        if let Some(tool) = created.and_then(|idx| self.state.session().tools().get(idx)) {
            self.state.selection = Selection::Tool(tool.id);
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_add_tool_from_library(&mut self, mut tool: ToolConfig) {
        // The catalog tool's id is project-irrelevant; the session
        // reassigns it on insert. Reset to a sentinel first.
        tool.id = crate::state::job::ToolId::placeholder();
        let command = Command::AddToolFromLibrary(AddToolArgs {
            tool: Box::new(tool),
        });
        let created = self.apply_created(command);
        if let Some(tool) = created.and_then(|idx| self.state.session().tools().get(idx)) {
            self.state.selection = Selection::Tool(tool.id);
        }
        self.state.gui.mark_edited();
    }

    // ── Tool Library modal ──────────────────────────────────────────────

    /// Load a fresh snapshot of every catalog into the modal state. Used
    /// both to open the modal and to refresh it after a mutation.
    fn load_tool_library_snapshot(&mut self) {
        let catalogs = rs_cam_core::io::tool_library::list_libraries()
            .into_iter()
            .filter_map(|name| {
                rs_cam_core::io::tool_library::load_library(&name)
                    .ok()
                    .map(|catalog| (name, catalog))
            })
            .collect();
        self.state.tool_library_modal = Some(crate::state::ToolLibraryModalState { catalogs });
    }

    /// Refresh the snapshot only if the modal is open (post-mutation).
    fn refresh_tool_library_snapshot(&mut self) {
        if self.state.tool_library_modal.is_some() {
            self.load_tool_library_snapshot();
        }
    }

    pub(crate) fn open_tool_library(&mut self) {
        self.state.close_modals_for_exclusivity();
        self.load_tool_library_snapshot();
    }

    /// Report a tool-library error to the user, prefixed with context.
    fn report_tool_library_error(
        &mut self,
        context: &str,
        err: &rs_cam_core::io::tool_library::ToolLibraryError,
    ) {
        tracing::error!("{context}: {err}");
        self.push_notification(format!("{context}: {err}"), super::super::Severity::Error);
    }

    pub(crate) fn delete_library_tool(&mut self, catalog: &str, index: usize) {
        if let Err(e) = rs_cam_core::io::tool_library::remove_tool_at(catalog, index) {
            self.report_tool_library_error("Delete tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn update_library_tool(&mut self, catalog: &str, index: usize, tool: ToolConfig) {
        if let Err(e) = rs_cam_core::io::tool_library::update_tool_at(catalog, index, tool) {
            self.report_tool_library_error("Update tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn move_library_tool(&mut self, from: &str, index: usize, to: &str) {
        if let Err(e) = rs_cam_core::io::tool_library::move_tool(from, index, to) {
            self.report_tool_library_error("Move tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn create_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::io::tool_library::create_library(name) {
            self.report_tool_library_error("Create catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn delete_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::io::tool_library::delete_library(name) {
            self.report_tool_library_error("Delete catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn rename_tool_catalog(&mut self, old: &str, new: &str) {
        if let Err(e) = rs_cam_core::io::tool_library::rename_library(old, new) {
            self.report_tool_library_error("Rename catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn dedupe_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::io::tool_library::dedupe_library(name) {
            self.report_tool_library_error("Dedupe catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    // ── Machine library (snapshot model) ────────────────────────────────

    /// Import a library machine as a SNAPSHOT copy into the project's
    /// inline machine (no live link), then invalidate machine-dependent
    /// state.
    pub(crate) fn import_machine_from_library(&mut self, name: &str) {
        match rs_cam_core::io::machine_library::load(name) {
            Ok(profile) => {
                // `SetMachine` invalidates exactly as `invalidate_machine`
                // does (§19 ruling 2), so the two calls become one.
                let command = Command::SetMachine(SetMachineArgs {
                    machine: Box::new(profile),
                });
                if self.apply_controller_command(command, "the machine profile") {
                    self.state.gui.mark_edited();
                    self.set_status(format!("Imported machine '{name}' (snapshot copy)"));
                }
            }
            Err(e) => self.report_machine_library_error("Import machine failed", &e),
        }
    }

    pub(crate) fn save_machine_to_library(&mut self, name: &str) {
        match rs_cam_core::io::machine_library::save(name, self.state.session().machine()) {
            Ok(path) => self.set_status(format!("Saved machine to {}", path.display())),
            Err(e) => self.report_machine_library_error("Save machine failed", &e),
        }
    }

    pub(crate) fn delete_machine_from_library(&mut self, name: &str) {
        match rs_cam_core::io::machine_library::delete(name) {
            Ok(()) => self.set_status(format!("Deleted machine '{name}' from library")),
            Err(e) => self.report_machine_library_error("Delete machine failed", &e),
        }
    }

    pub(crate) fn rename_machine_in_library(&mut self, old: &str, new: &str) {
        match rs_cam_core::io::machine_library::rename(old, new) {
            Ok(()) => self.set_status(format!("Renamed machine '{old}' → '{new}'")),
            Err(e) => self.report_machine_library_error("Rename machine failed", &e),
        }
    }

    fn report_machine_library_error(
        &mut self,
        context: &str,
        err: &rs_cam_core::io::machine_library::MachineLibraryError,
    ) {
        tracing::error!("{context}: {err}");
        self.push_notification(format!("{context}: {err}"), super::super::Severity::Error);
    }

    pub(crate) fn handle_duplicate_tool(&mut self, tool_id: crate::state::job::ToolId) {
        if let Some(src) = self
            .state
            .session()
            .tools()
            .iter()
            .find(|tool| tool.id == tool_id)
        {
            let mut duplicate = src.clone();
            duplicate.name = format!("{} (copy)", duplicate.name);
            let command = Command::AddTool(AddToolArgs {
                tool: Box::new(duplicate),
            });
            let created = self.apply_created(command);
            if let Some(tool) = created.and_then(|idx| self.state.session().tools().get(idx)) {
                self.state.selection = Selection::Tool(tool.id);
            }
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_tool(&mut self, tool_id: crate::state::job::ToolId) {
        let index = self
            .state
            .session()
            .tools()
            .iter()
            .position(|t| t.id == tool_id);
        if let Some(idx) = index {
            let command = Command::RemoveTool(RemoveToolArgs { index: idx });
            match self.apply(command) {
                Ok(_) => {
                    // WP19: the row runs `invalidate_tool`, which drops
                    // the session's simulation. `AppController::apply`
                    // clears the viewport one too (F096).
                    if self.state.selection == Selection::Tool(tool_id) {
                        self.state.selection = Selection::None;
                    }
                    self.state.gui.mark_edited();
                }
                Err(rs_cam_core::session::SessionError::ToolInUse(_)) => {
                    tracing::warn!(
                        "Cannot remove tool {:?}: still referenced by one or more toolpaths",
                        tool_id
                    );
                    self.push_notification(
                        "Cannot remove tool: still referenced by one or more toolpaths".into(),
                        super::super::Severity::Warning,
                    );
                }
                Err(e) => {
                    self.push_notification(e.to_string(), super::super::Severity::Error);
                }
            }
        }
    }

    pub(crate) fn handle_add_setup(&mut self) {
        let next_id = self.state.session().list_setups().len() + 1;
        let name = format!("Setup {next_id}");
        let command = Command::AddSetup(AddSetupArgs {
            name: Some(name),
            face_up: FaceUp::default(),
        });
        let created = self.apply_created(command);
        if let Some(setup) = created.and_then(|idx| self.state.session().list_setups().get(idx)) {
            self.state.selection = Selection::Setup(setup.id);
        }
        self.state.gui.mark_edited();
    }

    /// Open the shared removal confirmation for a setup that still exists.
    pub(crate) fn request_remove_setup(&mut self, setup_id: SetupId) {
        let setups = self.state.session().list_setups();
        if setups.len() <= 1 {
            return;
        }
        let Some(setup) = setups.iter().find(|setup| setup.id == setup_id) else {
            return;
        };
        self.state.panels.pending_setup_removal =
            Some(crate::state::panels::SetupRemovalConfirmation {
                setup_id,
                setup_name: setup.name.clone(),
            });
    }

    /// Remove a setup after cancelling any work that could otherwise adopt
    /// results for its now-deleted toolpaths.
    pub(crate) fn handle_remove_setup(&mut self, setup_id: SetupId) {
        self.state.panels.pending_setup_removal = None;
        let setups = self.state.session().list_setups();
        let Some((index, setup)) = setups
            .iter()
            .enumerate()
            .find(|(_, setup)| setup.id == setup_id)
        else {
            return;
        };
        // Refusing the final setup is not an edit, so it must not cancel an
        // unrelated generation that is already in flight.
        if setups.len() <= 1 {
            return;
        }

        let replacement = setups
            .get(if index > 0 { index - 1 } else { 1 })
            .map(|setup| setup.id);
        let removed_ids: Vec<_> = setup
            .toolpath_indices()
            .iter()
            .filter_map(|&tp| {
                self.state
                    .session()
                    .toolpath_configs()
                    .get(tp)
                    .map(|config| config.id)
            })
            .collect();

        self.cancel_plan_resolution();
        self.cancel_generation_plan();
        self.compute
            .cancel_lane(crate::compute::ComputeLane::Toolpath);

        let Some(effects) =
            self.apply_quietly(Command::RemoveSetup(RemoveSetupArgs { setup_index: index }))
        else {
            return;
        };
        // `apply_quietly` clears the view when a stored core simulation was
        // dropped. An in-flight-only run has no stored core value yet, but
        // still has to be cancelled after the epoch moves.
        if !effects.simulation_cleared {
            self.invalidate_simulation();
        }

        for id in &removed_ids {
            self.state.gui.toolpath_rt.remove(id);
            self.superseded_toolpaths.remove(id);
        }
        if self
            .state
            .gui
            .pending_toolpath_tab
            .as_ref()
            .is_some_and(|(id, _)| removed_ids.contains(id))
        {
            self.state.gui.pending_toolpath_tab = None;
        }
        if self
            .state
            .simulation
            .debug
            .pending_inspect_toolpath
            .is_some_and(|id| removed_ids.contains(&id))
        {
            self.state.simulation.debug.pending_inspect_toolpath = None;
        }

        self.state.selection = replacement.map_or(Selection::None, Selection::Setup);
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_setup_two_sided(&mut self) {
        let has_flipped = self
            .state
            .session()
            .list_setups()
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let next_id = self.state.session().list_setups().len() + 1;
            let name = format!("Setup {next_id}");
            let command = Command::AddSetup(AddSetupArgs {
                name: Some(name),
                face_up: FaceUp::Bottom,
            });
            let _ = self.apply_quietly(command);
        }

        // Key the pins to the flip the project actually programs, not to
        // an assumption: the full map, `face_up` AND `z_rotation` (P2), of
        // the first Bottom setup. The block above makes sure one exists.
        // The core placer refuses a turn it does not place for, rather
        // than guessing.
        let (flip_face, flip_turn) = self
            .state
            .session()
            .list_setups()
            .iter()
            .map(|s| (s.face_up, s.z_rotation))
            .find(|(f, _)| *f == FaceUp::Bottom)
            .unwrap_or((FaceUp::Bottom, ZRotation::Deg0));

        // The pin wizard's "Auto-place" calls the same planner
        // with the same inputs: the obstacles (the default part models,
        // the keep-outs and the fixtures) and the diameter of the tool
        // that drills the pin holes (G-PINPANEL, P6). The two doors cannot
        // disagree.
        let pin_inputs = KeyedPinInputs::from_session(self.state.session());
        let plan = keyed_pin_plan(
            self.state.session().stock_config(),
            flip_face,
            flip_turn,
            &pin_inputs,
        )
        .map_err(|e| e.to_string());

        // Existing pins stay. With no pins, the placed set goes in through
        // `SetAlignmentPins` (P4), which stales only the pin-drill
        // operations. A refusal writes nothing: a placement that hangs off
        // the blank or seats turned is worse than none, because the
        // operator finds out at the flip with the part already cut.
        let mut refusal: Option<String> = None;
        if self
            .state
            .session()
            .stock_config()
            .alignment_pins
            .is_empty()
        {
            match plan {
                Ok(pins) => {
                    let command = Command::SetAlignmentPins(SetAlignmentPinsArgs {
                        pins: pins.to_vec(),
                    });
                    // P5: the one pin-edit door. It also creates the
                    // pin-drill operation.
                    if let Err(error) = self.apply_pin_edit(command) {
                        refusal = Some(format!("Could not write the registration pins: {error}"));
                    }
                }
                Err(message) => refusal = Some(message),
            }
        }

        if let Some(message) = refusal {
            self.push_notification(message, super::super::Severity::Error);
        }

        // Whatever pins are stored now — freshly placed or pre-existing —
        // get judged against the flip they have to register.
        for warning in self
            .state
            .session()
            .stock_config()
            .validate_pins_for_flip(flip_face, flip_turn)
            .warnings()
        {
            tracing::warn!("{warning}");
            self.push_notification(warning, super::super::Severity::Warning);
        }

        self.pending_upload = true;
        self.state.gui.mark_edited();
        let _ = self.sync_alignment_pin_drill();
        self.state.selection = Selection::Stock;
    }

    /// Write the name the operator reads for a setup.
    ///
    /// The setup tree names the setup by ID, and the command row takes an
    /// index like every other setup row, so this door resolves the one to
    /// the other. The row drops no result: the name reaches the pause
    /// message and the setup sheet, never a generation input.
    pub(crate) fn handle_rename_setup(&mut self, setup_id: SetupId, name: String) {
        if let Some(setup_index) = self
            .state
            .session()
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id)
        {
            let command = Command::SetSetupName(SetSetupNameArgs { setup_index, name });
            if self.apply_controller_command(command, "the setup name") {
                self.state.gui.mark_edited();
            }
        }
    }

    /// Write the message the operator reads at a setup change.
    ///
    /// The export wizard names the setup by ID, and the command row takes
    /// an index like every other setup row, so this door resolves the
    /// one to the other. The row drops no result: the message reaches the
    /// export alone, beside the `M0` the post emits.
    pub(crate) fn set_setup_pause_message(
        &mut self,
        setup_id: rs_cam_core::ids::SetupId,
        message: Option<String>,
    ) {
        let Some(setup_index) = self
            .state
            .session()
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id)
        else {
            return;
        };
        let command = Command::SetSetupPauseMessage(SetSetupPauseMessageArgs {
            setup_index,
            message,
        });
        if self.apply_controller_command(command, "the setup pause message") {
            self.state.gui.mark_edited();
        }
    }

    /// K1: add a new box obstacle to the setup, with the default spans
    /// and clearance (`Obstacle::new_default_box`), and select it.
    ///
    /// K2: the door reads the `Effects`. Only an applied add selects the
    /// new record, opens the editor on it, pushes the undo entry and
    /// marks the project edited.
    pub(crate) fn handle_add_obstacle(&mut self, setup_id: SetupId) {
        let Some((idx, obstacle_id)) =
            self.state
                .session()
                .find_setup_by_id(setup_id)
                .map(|(idx, s)| {
                    (
                        idx,
                        rs_cam_core::compute::obstacle::next_obstacle_id(&s.obstacles),
                    )
                })
        else {
            return;
        };
        let record = Obstacle::new_default_box(obstacle_id);
        let command = Command::AddObstacle(AddObstacleArgs {
            setup_index: idx,
            obstacle: Box::new(record.clone()),
        });
        if self.apply_quietly(command).is_none() {
            return;
        }
        self.state.selection = Selection::Obstacle(setup_id, obstacle_id);
        self.state.panels.obstacle_editor = Some(crate::state::panels::ObstacleEditor {
            setup_id,
            obstacle_id,
            draft: record.clone(),
            refusal: None,
        });
        self.state
            .history
            .push(crate::state::history::UndoAction::ObstacleChange {
                setup_id,
                obstacle_id,
                old: None,
                new: Some(record),
            });
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    /// K1: remove one obstacle of the setup. K2: an applied remove pushes
    /// the undo entry and closes an editor open on the obstacle.
    pub(crate) fn handle_remove_obstacle(
        &mut self,
        setup_id: SetupId,
        obstacle_id: crate::state::job::ObstacleId,
    ) {
        let Some(stored) = self.stored_obstacle(setup_id, obstacle_id) else {
            return;
        };
        let Some((idx, _)) = self.state.session().find_setup_by_id(setup_id) else {
            return;
        };
        let command = Command::RemoveObstacle(RemoveObstacleArgs {
            setup_index: idx,
            obstacle_id,
        });
        if self.apply_quietly(command).is_none() {
            return;
        }
        self.state
            .history
            .push(crate::state::history::UndoAction::ObstacleChange {
                setup_id,
                obstacle_id,
                old: Some(stored),
                new: None,
            });
        self.forget_obstacle_view(setup_id, obstacle_id);
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    /// K2: the list-row checkbox. The door applies `ReplaceObstacle` with
    /// the STORED record and the new flag (the S4 `SetEnabled` shape), so
    /// an unapplied editor draft cannot ride along.
    pub(crate) fn handle_set_obstacle_enabled(
        &mut self,
        setup_id: SetupId,
        obstacle_id: crate::state::job::ObstacleId,
        enabled: bool,
    ) {
        let Some(stored) = self.stored_obstacle(setup_id, obstacle_id) else {
            return;
        };
        if stored.enabled == enabled {
            return;
        }
        let Some((idx, _)) = self.state.session().find_setup_by_id(setup_id) else {
            return;
        };
        let mut record = stored.clone();
        record.enabled = enabled;
        let command = Command::ReplaceObstacle(ReplaceObstacleArgs {
            setup_index: idx,
            obstacle_id,
            obstacle: Box::new(record.clone()),
        });
        if self.apply_quietly(command).is_none() {
            return;
        }
        // An open editor keeps its draft, but takes the new flag: Apply
        // writes the draft, and the draft must not undo this toggle.
        if let Some(editor) = self.state.panels.obstacle_editor.as_mut()
            && editor.setup_id == setup_id
            && editor.obstacle_id == obstacle_id
        {
            editor.draft.enabled = enabled;
        }
        self.state
            .history
            .push(crate::state::history::UndoAction::ObstacleChange {
                setup_id,
                obstacle_id,
                old: Some(stored),
                new: Some(record),
            });
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    /// K2: put the obstacle `obstacle_id` of the setup into the state
    /// `target` (an undo or a redo). `None` is "absent". The present record
    /// decides the command: remove, add or replace. Every write goes
    /// through `apply_quietly`, so the view mirrors the `Effects`.
    ///
    /// An undo of a remove adds the record again at the END of the list
    /// (core `AddObstacle` appends). The order has no effect on the keep-out.
    pub(crate) fn apply_obstacle_snapshot(
        &mut self,
        setup_id: SetupId,
        obstacle_id: crate::state::job::ObstacleId,
        target: Option<Obstacle>,
    ) {
        let Some((setup_index, _)) = self.state.session().find_setup_by_id(setup_id) else {
            return;
        };
        let present = self.stored_obstacle(setup_id, obstacle_id);
        let command = match (present, target) {
            (Some(_), None) => Command::RemoveObstacle(RemoveObstacleArgs {
                setup_index,
                obstacle_id,
            }),
            (None, Some(record)) => Command::AddObstacle(AddObstacleArgs {
                setup_index,
                obstacle: Box::new(record),
            }),
            (Some(_), Some(record)) => Command::ReplaceObstacle(ReplaceObstacleArgs {
                setup_index,
                obstacle_id,
                obstacle: Box::new(record),
            }),
            (None, None) => return,
        };
        let _ = self.apply_quietly(command);
        if self.stored_obstacle(setup_id, obstacle_id).is_none() {
            self.forget_obstacle_view(setup_id, obstacle_id);
        } else if let Some(editor) = self.state.panels.obstacle_editor.as_ref()
            && editor.setup_id == setup_id
            && editor.obstacle_id == obstacle_id
        {
            // The editor draft predates the undo; it re-seeds from the
            // restored record on the next frame.
            self.state.panels.obstacle_editor = None;
        }
        self.pending_upload = true;
    }

    /// The stored record of one obstacle, or `None`.
    fn stored_obstacle(
        &self,
        setup_id: SetupId,
        obstacle_id: crate::state::job::ObstacleId,
    ) -> Option<Obstacle> {
        self.state
            .session()
            .find_setup_by_id(setup_id)
            .and_then(|(_, setup)| setup.obstacles.iter().find(|o| o.id == obstacle_id))
            .cloned()
    }

    /// The obstacle is gone: close an editor open on it, and move a
    /// selection of it to its setup.
    pub(crate) fn forget_obstacle_view(
        &mut self,
        setup_id: SetupId,
        obstacle_id: crate::state::job::ObstacleId,
    ) {
        crate::state::panels::forget_obstacle(
            &mut self.state.panels,
            &mut self.state.selection,
            setup_id,
            obstacle_id,
        );
    }

    /// Run one stock-change action of the setup panel (S4) as one S2
    /// command: `AddStockChange`, `ReplaceStockChange`, `MoveStockChange`
    /// or `RemoveStockChange`.
    ///
    /// The door mirrors the `Effects` like every other controller door. A
    /// refusal reaches the operator twice: in the editor, when the action
    /// came from it, and as a notification.
    pub(crate) fn handle_stock_change_intent(
        &mut self,
        setup_id: SetupId,
        intent: crate::ui::properties::stock_changes::StockChangeIntent,
    ) {
        use crate::ui::properties::stock_changes::StockChangeIntent;
        use rs_cam_core::session::{
            AddStockChangeArgs, MoveStockChangeArgs, RemoveStockChangeArgs, ReplaceStockChangeArgs,
        };
        let Some((setup_index, setup)) = self.state.session().find_setup_by_id(setup_id) else {
            return;
        };
        let (command, from_editor) = match intent {
            StockChangeIntent::Save { change, is_new } => {
                let command = if is_new {
                    Command::AddStockChange(AddStockChangeArgs {
                        setup_index,
                        change,
                    })
                } else {
                    Command::ReplaceStockChange(ReplaceStockChangeArgs {
                        setup_index,
                        change_id: change.id,
                        change,
                    })
                };
                (command, true)
            }
            StockChangeIntent::SetEnabled { change_id, enabled } => {
                let Some(stored) = setup.stock_changes.iter().find(|c| c.id == change_id) else {
                    return;
                };
                let mut change = stored.clone();
                change.enabled = enabled;
                (
                    Command::ReplaceStockChange(ReplaceStockChangeArgs {
                        setup_index,
                        change_id,
                        change: Box::new(change),
                    }),
                    false,
                )
            }
            StockChangeIntent::Move {
                change_id,
                to_position,
            } => (
                Command::MoveStockChange(MoveStockChangeArgs {
                    setup_index,
                    change_id,
                    to_position,
                }),
                false,
            ),
            StockChangeIntent::Remove(change_id) => {
                if self
                    .state
                    .panels
                    .stock_change_editor
                    .as_ref()
                    .is_some_and(|e| e.setup_id == setup_id && !e.is_new && e.draft.id == change_id)
                {
                    self.state.panels.stock_change_editor = None;
                }
                (
                    Command::RemoveStockChange(RemoveStockChangeArgs {
                        setup_index,
                        change_id,
                    }),
                    false,
                )
            }
        };
        match self.apply(command) {
            Ok(_) => {
                if from_editor {
                    self.state.panels.stock_change_editor = None;
                }
                self.state.gui.mark_edited();
            }
            Err(error) => {
                let message = error.to_string();
                if from_editor
                    && let Some(editor) = self.state.panels.stock_change_editor.as_mut()
                    && editor.setup_id == setup_id
                {
                    editor.refusal = Some(message.clone());
                }
                self.push_notification(
                    format!("Could not write the stock change: {message}"),
                    super::super::Severity::Error,
                );
            }
        }
    }

    // ── Model helpers ────────────────────────────────────────────────────

    pub(crate) fn handle_remove_model(&mut self, model_id: ModelId) {
        // Check if any toolpath still references this model
        let in_use = self
            .state
            .session()
            .toolpath_configs()
            .iter()
            .any(|tc| tc.model_id == model_id);
        if in_use {
            tracing::warn!(
                "Cannot remove model {:?}: still referenced by one or more toolpaths",
                model_id
            );
            self.push_notification(
                "Cannot remove model: still referenced by one or more toolpaths".into(),
                super::super::Severity::Warning,
            );
        } else if let Some(idx) = self
            .state
            .session()
            .models()
            .iter()
            .position(|m| m.id == model_id)
        {
            let command = Command::RemoveModel(RemoveModelArgs { index: idx });
            // F056: a refused removal keeps the model, the selection and
            // the clean mark. The door logs the refusal.
            if self.apply_quietly(command).is_none() {
                return;
            }
            let clear_selection = matches!(
                self.state.selection,
                Selection::Model(mid) | Selection::Face(mid, _) | Selection::Faces(mid, _)
                    if mid == model_id
            );
            if clear_selection {
                self.state.selection = Selection::None;
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    // ── Stock / config helpers ───────────────────────────────────────────

    // WP6 deleted `handle_stock_changed` with its event. The stock panel
    // applies `Command::SetStockConfig`, which drops every toolpath
    // result (G-FRESHSTATE / R0.1 section 7 Q1, operator ruling
    // 2026-09-10) and reports the set, so the panel stamps it through
    // `state::stale::stamp_stale`. The three steps that are not a core
    // rule moved to `ui::properties::apply_stock_draft`: the
    // `auto_from_model` re-size, the GPU upload flag and the pin-drill
    // synchronisation. The pin-drill synchronisation is in `pins.rs`, the
    // one door of a pin edit (P5).
}
