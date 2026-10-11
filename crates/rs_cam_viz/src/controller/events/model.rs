use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::session::{
    AddFixtureArgs, AddKeepOutArgs, AddSetupArgs, AddToolArgs, Command, Fixture, KeepOutZone,
    RemoveFixtureArgs, RemoveKeepOutArgs, RemoveModelArgs, RemoveSetupArgs, RemoveToolArgs,
    SetAlignmentPinsArgs, SetMachineArgs, SetSetupNameArgs, SetSetupPauseMessageArgs,
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
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(*tp_id),
            _ => None,
        };
        let new_setup = match selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
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
        let (tp_index, _) = self.state.session.find_toolpath_config_by_id(tp_id)?;
        self.state
            .session
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_index))
            .map(|s| SetupId(s.id))
    }

    pub(crate) fn handle_add_tool(&mut self, tool_type: crate::state::job::ToolType) {
        let tool = ToolConfig::new_default(crate::state::job::ToolId(0), tool_type);
        let command = Command::AddTool(AddToolArgs {
            tool: Box::new(tool),
        });
        let created = self.apply_created(command);
        // The session assigned the ID — read it back.
        if let Some(tool) = created.and_then(|idx| self.state.session.tools().get(idx)) {
            self.state.selection = Selection::Tool(tool.id);
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_add_tool_from_library(&mut self, mut tool: ToolConfig) {
        // The catalog tool's id is project-irrelevant; the session
        // reassigns it on insert. Reset to a sentinel first.
        tool.id = crate::state::job::ToolId(0);
        let command = Command::AddToolFromLibrary(AddToolArgs {
            tool: Box::new(tool),
        });
        let created = self.apply_created(command);
        if let Some(tool) = created.and_then(|idx| self.state.session.tools().get(idx)) {
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
        match rs_cam_core::io::machine_library::save(name, self.state.session.machine()) {
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
            .session
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
            if let Some(tool) = created.and_then(|idx| self.state.session.tools().get(idx)) {
                self.state.selection = Selection::Tool(tool.id);
            }
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_tool(&mut self, tool_id: crate::state::job::ToolId) {
        let index = self
            .state
            .session
            .tools()
            .iter()
            .position(|t| t.id == tool_id);
        if let Some(idx) = index {
            let command = Command::RemoveTool(RemoveToolArgs { index: idx });
            match self.state.session.apply(command) {
                Ok(effects) => {
                    crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
                    // WP19: the row runs `invalidate_tool`, which drops
                    // the session's simulation. The viewport holds the
                    // same one (WP11b, N12 item 10).
                    if effects.simulation_cleared {
                        self.invalidate_simulation();
                    }
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
        let next_id = self.state.session.list_setups().len() + 1;
        let name = format!("Setup {next_id}");
        let command = Command::AddSetup(AddSetupArgs {
            name: Some(name),
            face_up: FaceUp::default(),
        });
        let created = self.apply_created(command);
        if let Some(setup) = created.and_then(|idx| self.state.session.list_setups().get(idx)) {
            self.state.selection = Selection::Setup(SetupId(setup.id));
        }
        self.state.gui.mark_edited();
    }

    /// Open the shared removal confirmation for a setup that still exists.
    pub(crate) fn request_remove_setup(&mut self, setup_id: SetupId) {
        let setups = self.state.session.list_setups();
        if setups.len() <= 1 {
            return;
        }
        let Some(setup) = setups.iter().find(|setup| setup.id == setup_id.0) else {
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
        let setups = self.state.session.list_setups();
        let Some((index, setup)) = setups
            .iter()
            .enumerate()
            .find(|(_, setup)| setup.id == setup_id.0)
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
            .map(|setup| SetupId(setup.id));
        let removed_ids: Vec<_> = setup
            .toolpath_indices
            .iter()
            .filter_map(|&tp| {
                self.state
                    .session
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
        self.state
            .pending_reconciliation_for_ids
            .retain(|id| !removed_ids.contains(id));
        if self
            .state
            .pending_apply_resim
            .is_some_and(|id| removed_ids.iter().any(|removed| removed.0 == id))
        {
            self.state.pending_apply_resim = None;
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
            .session
            .list_setups()
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let next_id = self.state.session.list_setups().len() + 1;
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
            .session
            .list_setups()
            .iter()
            .map(|s| (s.face_up, s.z_rotation))
            .find(|(f, _)| *f == FaceUp::Bottom)
            .unwrap_or((FaceUp::Bottom, ZRotation::Deg0));

        // The stock panel's "Auto-place keyed pins" calls the same planner
        // with the same inputs: the obstacles (the default part models,
        // the keep-outs and the fixtures) and the diameter of the tool
        // that drills the pin holes (G-PINPANEL, P6). The two doors cannot
        // disagree.
        let pin_inputs = KeyedPinInputs::from_session(&self.state.session);
        let plan = keyed_pin_plan(
            self.state.session.stock_config(),
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
        if self.state.session.stock_config().alignment_pins.is_empty() {
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
            .session
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
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
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
    pub(crate) fn set_setup_pause_message(&mut self, setup_id: usize, message: Option<String>) {
        let Some(setup_index) = self
            .state
            .session
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

    pub(crate) fn handle_add_fixture(&mut self, setup_id: SetupId) {
        // Generate a fixture ID based on existing max
        let max_id = self
            .state
            .session
            .list_setups()
            .iter()
            .flat_map(|s| s.fixtures.iter())
            .map(|f| f.id.0)
            .max()
            .map_or(0, |id| id + 1);
        let fixture_id = crate::state::job::FixtureId(max_id);

        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            // C02: the six numbers this used to spell out are core's
            // serde defaults. A file that omitted `size_y` restored a
            // fixture the GUI would never have drawn.
            let fixture = Fixture::new_default(fixture_id);
            let command = Command::AddFixture(AddFixtureArgs {
                setup_index: idx,
                fixture: Box::new(fixture),
            });
            let _ = self.apply_quietly(command);
            self.state.selection = Selection::Fixture(setup_id, fixture_id);
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_fixture(
        &mut self,
        setup_id: SetupId,
        fixture_id: crate::state::job::FixtureId,
    ) {
        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let command = Command::RemoveFixture(RemoveFixtureArgs {
                setup_index: idx,
                fixture_id,
            });
            let _ = self.apply_quietly(command);
            if self.state.selection == Selection::Fixture(setup_id, fixture_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_add_keep_out(&mut self, setup_id: SetupId) {
        let max_id = self
            .state
            .session
            .list_setups()
            .iter()
            .flat_map(|s| s.keep_out_zones.iter())
            .map(|k| k.id.0)
            .max()
            .map_or(0, |id| id + 1);
        let keep_out_id = crate::state::job::KeepOutId(max_id);

        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let zone = KeepOutZone {
                id: keep_out_id,
                name: format!("Keep-Out {}", keep_out_id.0 + 1),
                enabled: true,
                origin_x: 0.0,
                origin_y: 0.0,
                size_x: 20.0,
                size_y: 20.0,
            };
            let command = Command::AddKeepOut(AddKeepOutArgs {
                setup_index: idx,
                zone: Box::new(zone),
            });
            let _ = self.apply_quietly(command);
            self.state.selection = Selection::KeepOut(setup_id, keep_out_id);
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_keep_out(
        &mut self,
        setup_id: SetupId,
        keep_out_id: crate::state::job::KeepOutId,
    ) {
        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let command = Command::RemoveKeepOut(RemoveKeepOutArgs {
                setup_index: idx,
                zone_id: keep_out_id,
            });
            let _ = self.apply_quietly(command);
            if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
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
        let Some((setup_index, setup)) = self.state.session.find_setup_by_id(setup_id.0) else {
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
        match self.state.session.apply(command) {
            Ok(effects) => {
                crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
                if effects.simulation_cleared {
                    self.invalidate_simulation();
                }
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
            .session
            .toolpath_configs()
            .iter()
            .any(|tc| tc.model_id == model_id.0);
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
            .session
            .models()
            .iter()
            .position(|m| m.id == model_id.0)
        {
            let command = Command::RemoveModel(RemoveModelArgs { index: idx });
            let _ = self.apply_quietly(command);
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
