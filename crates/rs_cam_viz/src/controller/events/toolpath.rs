use rs_cam_core::session::{
    AddBoreFromDrillArgs, AddToolpathArgs, Command, Effects, MoveToolpathToSetupArgs,
    RemoveToolpathArgs, ReorderToolpathArgs,
};

use crate::compute::ComputeBackend;
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui_command::{SimJumpToMoveArgs, UiCommand};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Toolpath helpers ─────────────────────────────────────────────────

    /// Add one toolpath the way the operator's Add menu does, and report
    /// the command's own [`Effects`].
    ///
    /// The op is drafted by core `draft_toolpath` (S11 parity):
    ///
    /// - R2: the first tool in tool-list order that the op accepts. With
    ///   none, the op is made with NO tool; a toast names the state ("No
    ///   tool fits: needs ...") and the op refuses generation until a tool
    ///   is set.
    /// - Q7: the model by the core rule (`model_for_new_op`), with the
    ///   model selected in the Setup rail or the viewport.
    /// - Q7 (O-PICK): BREP faces picked in the viewport become the op
    ///   selection.
    ///
    /// `None` means nothing was created: no geometry for an op that needs
    /// a model, or a command refusal. The refusal reaches the operator as a
    /// toast (G-GUIADD).
    ///
    /// WP28 part 2: the answer carries `Effects` because the MCP
    /// `add_toolpath_via_gui` reply reports `Effects::stale` — the set
    /// this command's setter dropped. An append moves one revision, so
    /// the honest answer is the created index. The reply used to report
    /// EVERY index, because `compute_stale_set(MutationKind::AllToolpaths)`
    /// answered from the tag alone. The GUI route already stamped the
    /// narrow set, so the two routes now agree.
    pub(crate) fn handle_add_toolpath(
        &mut self,
        op_type: crate::state::toolpath::OperationType,
    ) -> Option<Effects> {
        self.add_toolpath_with_model(op_type, None)
    }

    /// [`Self::handle_add_toolpath`] with a stated selected model, which
    /// replaces the model of the GUI selection (MCP
    /// `add_toolpath_via_gui` `model_id`). `None` reads the GUI selection
    /// only.
    pub(crate) fn add_toolpath_with_model(
        &mut self,
        op_type: crate::state::toolpath::OperationType,
        model_override: Option<rs_cam_core::ids::ModelId>,
    ) -> Option<Effects> {
        let (target_setup_idx, selected_model) = self.add_target(model_override);
        let name = format!(
            "{} {}",
            op_type.label(),
            self.state.session().toolpath_configs().len() + 1
        );
        // S11 parity: THE draft door of core (`ProjectSession::draft_toolpath`),
        // which MCP and the CLI job share.
        let request = rs_cam_core::session::DraftRequest::new(op_type, target_setup_idx, name)
            .selected_model(selected_model);
        let draft = match self.state.session().draft_toolpath(request) {
            Ok(draft) => draft,
            Err(error) => {
                let msg = format!("Cannot add toolpath: {error}");
                tracing::warn!("{msg}");
                self.push_notification(msg, super::super::Severity::Warning);
                return None;
            }
        };
        // W2b: no toast for a draft with no tool or no feeds. Each is a
        // STATE, and the inspector shows it: the red Tool row, the empty
        // feed fields and the "Not ready" status word. A toast is for an
        // event only.
        let rs_cam_core::session::ToolpathDraft {
            config: tc,
            feeds_refusal: _,
            no_tool: _,
            // The GUI add passes no picks, so no pick drops.
            dropped_picks: _,
        } = draft;
        if !tc.operation.is_stock_based() && self.state.session().models().is_empty() {
            let msg = format!(
                "Cannot add {} toolpath: import geometry first",
                tc.operation.label()
            );
            tracing::warn!("{msg}");
            self.push_notification(msg, super::super::Severity::Warning);
            return None;
        }
        let target_setup_idx = Some(target_setup_idx);

        let effects = target_setup_idx.and_then(|setup_idx| {
            let command = Command::AddToolpath(AddToolpathArgs {
                setup_index: setup_idx,
                config: Box::new(tc),
            });
            self.apply_add_or_toast(command)
        });
        let created = effects.as_ref().and_then(|effects| effects.created);
        // UX-7b, R-B, R-A: the add set a floor (THE each-face, each-hole or
        // part outline rule of core); the notice is the one core note.
        if let Some(note) = effects.as_ref().and_then(|e| e.floor_set_note()) {
            self.push_notification(note.to_owned(), super::super::Severity::Info);
        }
        if let Some(tp_idx) = created
            && let Some(tc) = self.state.session().toolpath_configs().get(tp_idx)
        {
            let tp_id = tc.id;
            // Create GUI runtime entry
            self.state.gui.toolpath_rt.insert(
                tc.id,
                crate::state::runtime::ToolpathRuntime::new(tc.operation.default_auto_regen()),
            );
            self.set_focus(Selection::Toolpath(tp_id));
        }
        self.state.gui.mark_edited();
        effects
    }

    /// Step 7.8: the add menu's "Geodesic body + edges". The pair goes in
    /// the setup of the GUI selection, with the selected model (Q7), the
    /// first fitting tool and the edge op (D-T2: its enable box skips it).
    pub(crate) fn handle_add_geodesic_pair(&mut self) {
        let (setup_index, selected_model) = self.add_target(None);
        let request = rs_cam_core::session::GeodesicPairRequest::new(setup_index, selected_model);
        if let Err(msg) = self.add_geodesic_pair(request) {
            tracing::warn!("{msg}");
            self.push_notification(msg, super::super::Severity::Warning);
        }
    }

    /// Step 7.8: draft the pair through core (`draft_geodesic_pair`, THE
    /// draft door) and add it with ONE `AddGeodesicBodyAndEdges` command.
    /// The GUI part is the op add's: a runtime entry per op and the
    /// selection on op 1 (W2b: no state toast). The GUI and MCP both call it.
    ///
    /// # Errors
    ///
    /// The text of a draft or command refusal ("Cannot add toolpath: …").
    pub(crate) fn add_geodesic_pair(
        &mut self,
        request: rs_cam_core::session::GeodesicPairRequest,
    ) -> Result<GeodesicPairAdded, String> {
        let setup_index = request.setup_index;
        let draft = self
            .state
            .session()
            .draft_geodesic_pair(request)
            .map_err(|error| format!("Cannot add toolpath: {error}"))?;
        // W2b: no toast for a draft with no tool or no feeds (a STATE the
        // inspector shows), as the op add. The MCP reply keeps the notes.
        let notes: Vec<(Option<String>, Option<String>)> = std::iter::once(&draft.body)
            .chain(draft.edges.as_ref())
            .map(|d| (d.no_tool.clone(), d.feeds_refusal.clone()))
            .collect();
        let effects = self
            .apply(Command::AddGeodesicBodyAndEdges(
                rs_cam_core::session::AddGeodesicBodyAndEdgesArgs {
                    setup_index,
                    body: Box::new(draft.body.config),
                    edges: draft.edges.map(|edge| Box::new(edge.config)),
                },
            ))
            .map_err(|error| format!("Cannot add toolpath: {error}"))?;
        let body_index = effects
            .created
            .ok_or_else(|| "Cannot add toolpath: the command created nothing".to_owned())?;
        let edges_index = (notes.len() > 1).then_some(body_index + 1);
        let mut body_id = None;
        for index in std::iter::once(body_index).chain(edges_index) {
            if let Some(tc) = self.state.session().toolpath_configs().get(index) {
                let (id, auto_regen) = (tc.id, tc.operation.default_auto_regen());
                body_id = body_id.or(Some(id));
                self.state
                    .gui
                    .toolpath_rt
                    .insert(id, crate::state::runtime::ToolpathRuntime::new(auto_regen));
            }
        }
        if let Some(id) = body_id {
            self.set_focus(Selection::Toolpath(id));
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(GeodesicPairAdded {
            body_index,
            edges_index,
            no_tool: notes.iter().find_map(|(no_tool, _)| no_tool.clone()),
            feeds_refusal: notes.iter().find_map(|(_, feeds)| feeds.clone()),
        })
    }

    /// Where an add goes, from the GUI selection (Q7): the setup index and
    /// the selected model (`model_override` wins). Shared by the op add and
    /// the template (step 7.8). The tree focus holds no faces. An op gets
    /// its faces from its own selection (`Command::EditSelection`).
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "projection: only the named variant(s) carry this (named: Selection::Toolpath, Selection::Setup, Selection::Obstacle, Selection::Model)"
    )]
    fn add_target(
        &self,
        model_override: Option<rs_cam_core::ids::ModelId>,
    ) -> (usize, Option<rs_cam_core::ids::ModelId>) {
        // Q7: the model selected in the Setup rail or the viewport.
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "projection: only the named variant(s) carry this (named: Selection::Model)"
        )]
        let selected_model = match *self.state.focus() {
            Selection::Model(model_id) => Some(model_id),
            _ => None,
        };
        let selected_model = model_override.or(selected_model);
        let setups = self.state.session().list_setups();
        let target_setup_idx = match *self.state.focus() {
            Selection::Toolpath(tp_id) => self
                .setup_of_toolpath(tp_id)
                .and_then(|sid| setups.iter().position(|s| s.id == sid)),
            Selection::Setup(setup_id) | Selection::Obstacle(setup_id, _) => {
                setups.iter().position(|s| s.id == setup_id)
            }
            // Q7: a selected model adds into the first setup whose
            // "Models" list holds it (empty = every model).
            Selection::Model(_) => selected_model.and_then(|model_id| {
                setups
                    .iter()
                    .position(|s| s.model_ids.is_empty() || s.model_ids.contains(&model_id))
            }),
            _ => None,
        }
        .unwrap_or(0); // default to first setup
        (target_setup_idx, selected_model)
    }

    /// Apply one add command. A refusal shows as a Warning toast, so a
    /// refused add is never silent: the toast is the only record that
    /// the operator sees.
    fn apply_add_or_toast(&mut self, command: Command) -> Option<Effects> {
        match self.apply(command) {
            Ok(effects) => Some(effects),
            Err(error) => {
                let msg = format!("Cannot add toolpath: {error}");
                tracing::warn!("{msg}");
                self.push_notification(msg, super::super::Severity::Warning);
                None
            }
        }
    }

    pub(crate) fn handle_duplicate_toolpath(&mut self, tp_id: ToolpathId) {
        let setup_idx = self.setup_of_toolpath(tp_id).and_then(|sid| {
            self.state
                .session()
                .list_setups()
                .iter()
                .position(|s| s.id == sid)
        });

        // Build a new ToolpathConfig by reading fields from the source
        let dup = self
            .state
            .session()
            .find_toolpath_config_by_id(tp_id)
            .map(|(_, src)| {
                rs_cam_core::session::ToolpathConfig {
                    shoe: src.shoe,
                    feeds_lock: src.feeds_lock,
                    id: rs_cam_core::ToolpathId::placeholder(), // will be assigned by session
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
                    rest_analysis: src.rest_analysis.clone(),
                    stock_source: src.stock_source,
                    coolant: src.coolant,
                    debug_options: src.debug_options,
                    feeds_provenance: src.feeds_provenance.clone(),
                    planner_origin: None,
                }
            });

        if let Some(tc) = dup {
            let created = setup_idx.and_then(|setup_idx| {
                let command = Command::AddToolpath(AddToolpathArgs {
                    setup_index: setup_idx,
                    config: Box::new(tc),
                });
                self.apply_add_or_toast(command)
                    .and_then(|effects| effects.created)
            });
            if let Some(tp_idx) = created
                && let Some(new_tc) = self.state.session().toolpath_configs().get(tp_idx)
            {
                let new_id = new_tc.id;
                self.state.gui.toolpath_rt.insert(
                    new_tc.id,
                    crate::state::runtime::ToolpathRuntime::new(
                        new_tc.operation.default_auto_regen(),
                    ),
                );
                self.set_focus(Selection::Toolpath(new_id));
            }
            self.state.gui.mark_edited();
        }
    }

    /// Geometry selection P6.7 (L-17): "Add a Bore for these". One
    /// `AddBoreFromDrill` command adds the Bore after the Drill, and one
    /// undo step (`UndoAction::BoreAdded`) removes it. The selection stays
    /// on the Drill: the operator reads its hole rows, and a move of the
    /// selection would flush a parameter snapshot of the Drill into the
    /// history before this step.
    pub(crate) fn handle_add_bore_from_drill(&mut self, drill_id: ToolpathId) {
        if let Some(bore_id) = self.add_bore_from_drill(drill_id) {
            self.state
                .history
                .push(crate::state::history::UndoAction::BoreAdded { drill_id, bore_id });
            self.state.gui.mark_edited();
        }
    }

    /// Apply `AddBoreFromDrill` to the Drill `drill_id` and make the GUI
    /// runtime entry of the Bore. Returns the id of the Bore, or `None`
    /// when the Drill is gone or the command refused. A refusal shows as
    /// a toast.
    pub(super) fn add_bore_from_drill(&mut self, drill_id: ToolpathId) -> Option<ToolpathId> {
        let (index, _) = self.state.session().find_toolpath_config_by_id(drill_id)?;
        let created = self
            .apply_add_or_toast(Command::AddBoreFromDrill(AddBoreFromDrillArgs {
                index,
                drill_id,
            }))?
            .created?;
        let tc = self.state.session().toolpath_configs().get(created)?;
        let (bore_id, auto_regen) = (tc.id, tc.operation.default_auto_regen());
        // Q9 + R2: when no project tool fits the holes the Bore has no
        // tool. W2b: no toast; the red Tool row of the inspector shows it.
        self.state.gui.toolpath_rt.insert(
            bore_id,
            crate::state::runtime::ToolpathRuntime::new(auto_regen),
        );
        self.pending_upload = true;
        Some(bore_id)
    }

    pub(crate) fn handle_move_toolpath_up(&mut self, tp_id: ToolpathId) {
        // Find the setup and local position of this toolpath
        if let Some((tp_idx, _)) = self.state.session().find_toolpath_config_by_id(tp_id)
            && let Some(setup) = self
                .state
                .session()
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices().contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices().iter().position(|&i| i == tp_idx)
            && local_pos > 0
        {
            #[expect(
                clippy::indexing_slicing,
                reason = "local_pos - 1 is valid since local_pos > 0"
            )]
            let swap_with = setup.toolpath_indices()[local_pos - 1];
            let command = Command::ReorderToolpath(ReorderToolpathArgs {
                from_index: tp_idx,
                to_index: swap_with,
            });
            let _ = self.apply_quietly(command);
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_down(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session().find_toolpath_config_by_id(tp_id)
            && let Some(setup) = self
                .state
                .session()
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices().contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices().iter().position(|&i| i == tp_idx)
            && local_pos + 1 < setup.toolpath_indices().len()
            && let Some(&swap_with) = setup.toolpath_indices().get(local_pos + 1)
        {
            let command = Command::ReorderToolpath(ReorderToolpathArgs {
                from_index: tp_idx,
                to_index: swap_with,
            });
            let _ = self.apply_quietly(command);
            self.state.gui.mark_edited();
        }
    }

    /// Drop a dragged card into a gap in its own setup's plan order.
    ///
    /// `target_idx` is an insertion GAP (`0..=len`), not a card position, so
    /// it has to be turned into the card the moved op should land against
    /// before core's insert can use it (G-DROPINDEX):
    ///
    /// - a gap ABOVE the dragged card (`gap <= local_pos`) means "before the
    ///   card currently at `gap`", so the target card is `gap` itself;
    /// - a gap BELOW it (`gap > local_pos`) means "after the card currently
    ///   at `gap - 1`", so the target card is `gap - 1`.
    ///
    /// Both land the op exactly in the gap, because `reorder_toolpath`
    /// inserts before an upward target and after a downward one.
    pub(crate) fn handle_reorder_toolpath(&mut self, tp_id: ToolpathId, target_idx: usize) {
        if let Some((tp_idx, _)) = self.state.session().find_toolpath_config_by_id(tp_id)
            && let Some(setup) = self
                .state
                .session()
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices().contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices().iter().position(|&i| i == tp_idx)
        {
            let gap = target_idx.min(setup.toolpath_indices().len());
            let clamped = if gap > local_pos { gap - 1 } else { gap };
            if let Some(&target_global_idx) = setup.toolpath_indices().get(clamped)
                && tp_idx != target_global_idx
            {
                let command = Command::ReorderToolpath(ReorderToolpathArgs {
                    from_index: tp_idx,
                    to_index: target_global_idx,
                });
                // F056: a refused reorder changed nothing, so it does not
                // dirty the project. The door logs the refusal.
                if self.apply_quietly(command).is_some() {
                    self.state.gui.mark_edited();
                }
            }
        }
    }

    /// Drop a dragged card into another setup.
    ///
    /// `position` is the insertion gap in the TARGET setup's plan order, or
    /// `None` to append. Core clamps it, so a gap read off a shorter list
    /// than the target's is a landing at the end, never a refusal.
    pub(crate) fn handle_move_toolpath_to_setup(
        &mut self,
        tp_id: ToolpathId,
        setup_id: crate::state::job::SetupId,
        position: Option<usize>,
    ) {
        if let Some((tp_idx, _)) = self.state.session().find_toolpath_config_by_id(tp_id) {
            // Resolve the target setup's index from its ID
            let target_idx = self
                .state
                .session()
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id);
            if let Some(target_setup_idx) = target_idx {
                let command = Command::MoveToolpathToSetup(MoveToolpathToSetupArgs {
                    toolpath_index: tp_idx,
                    target_setup_index: target_setup_idx,
                    target_position: position,
                });
                // F056: only an applied move dirties the project. The
                // door logs a refusal.
                if self.apply_quietly(command).is_some() {
                    self.pending_upload = true;
                    self.state.gui.mark_edited();
                }
            }
        }
    }

    /// H2: the toolpath panel's dust shoe row. One `SetToolpathShoe`; an
    /// applied edit pushes the undo step and marks the project edited. The
    /// row moves no toolpath; the core drops the simulation, and
    /// `apply_quietly` mirrors that. `merge` joins a later frame of one gap
    /// drag to the newest undo step.
    pub(crate) fn handle_set_toolpath_shoe(
        &mut self,
        tp_id: ToolpathId,
        shoe: rs_cam_core::machine::head::ShoeSetting,
        merge: bool,
    ) {
        let Some((index, old)) = self
            .state
            .session()
            .find_toolpath_config_by_id(tp_id)
            .map(|(index, tc)| (index, tc.shoe))
        else {
            return;
        };
        if old == shoe {
            return;
        }
        let command =
            Command::SetToolpathShoe(rs_cam_core::session::SetToolpathShoeArgs { index, shoe });
        if self.apply_quietly(command).is_none() {
            self.push_notification(
                "The dust shoe setting was refused: a gap must be 0 mm or more.".to_owned(),
                super::super::Severity::Warning,
            );
            return;
        }
        if !(merge && self.state.history.merge_shoe_edit(tp_id, shoe)) {
            self.state
                .history
                .push(crate::state::history::UndoAction::ToolpathShoeChange {
                    tp_id,
                    old,
                    new: shoe,
                });
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_remove_toolpath(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session().find_toolpath_config_by_id(tp_id) {
            let command = Command::RemoveToolpath(RemoveToolpathArgs { index: tp_idx });
            let _ = self.apply_quietly(command);
            self.state.gui.toolpath_rt.remove(&tp_id);
            // W1: the viz twin of the Regions rule is NOT called here
            // (F040 deleted it). `remove_toolpath` walks the dependents
            // BEFORE it takes the config out
            // (`session/mutation/toolpath.rs:88-91`), so the Regions edge
            // still resolves, the walker names every consumer, and
            // `apply_quietly` stamps them from `Effects::stale`.
        }
        if *self.state.focus() == Selection::Toolpath(tp_id) {
            self.set_focus(Selection::None);
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }

    /// Generate every enabled operation, simulating in between wherever a
    /// rest operation needs its stock (W1, `PLAN.md` §4.2).
    ///
    /// G-GENALLDISABLED (2026-09-10): the walk filters on `tc.enabled`, so a
    /// disabled operation is never generated. Disabling an op promises
    /// exclusion "from generation, simulation and output" (the row-control
    /// hover), core's own `generate_all` skips disabled ops, and export
    /// filters them out.
    pub(crate) fn handle_generate_all(&mut self) {
        use crate::controller::generate_all::generate_all_scope;
        use rs_cam_core::session::generation_plan::Scope;

        let scope = generate_all_scope(self.state.session().toolpath_configs());
        if scope.enabled.is_empty() {
            self.push_notification(
                "No enabled toolpaths to generate".into(),
                super::super::Severity::Warning,
            );
            return;
        }
        let _ = self.start_gui_plan(Scope::Project, None);
    }

    /// Make one operation current, and everything it depends on first (R6).
    ///
    /// A single Generate used to submit the named operation alone, so a rest
    /// operation whose predecessor was stale blocked, and a Regions consumer
    /// clipped against regions its source no longer held. The ancestor plan
    /// generates the source first, in plan order, and simulates where a Stock
    /// edge needs a snapshot.
    pub(crate) fn handle_generate_toolpath(&mut self, tp_id: ToolpathId) {
        use rs_cam_core::session::generation_plan::Scope;

        let _ = self.start_gui_plan(Scope::Ancestors(tp_id), Some(tp_id));
    }

    /// The MCP `generate_toolpath` door: the same ancestor plan, and NO
    /// confirm modal (G-MCPMODAL, ruling Q5 of 2026-09-24).
    ///
    /// An MCP call cannot answer a GUI pop-up and must not wait on a click.
    /// When the stored cell is too coarse for the rest, the call refuses
    /// with the sentence the CLI prints, and the waiter resolves at once.
    #[cfg_attr(
        not(feature = "mcp"),
        expect(
            dead_code,
            reason = "only the MCP server reads it, and the `mcp` feature is off"
        )
    )]
    pub(crate) fn handle_generate_toolpath_mcp(&mut self, tp_id: ToolpathId) {
        use rs_cam_core::session::generation_plan::{Scope, Step, plan};

        let needs_simulation = plan(self.state.session(), Scope::Ancestors(tp_id))
            .iter()
            .any(|step| matches!(step, Step::Simulate { .. }));
        if needs_simulation && let Some(refusal) = self.state.session().rest_resolution_refusal() {
            self.fail_toolpath_submit(tp_id, format!("generate_toolpath refused: {refusal}"));
            return;
        }
        let _ = self.start_gui_plan(Scope::Ancestors(tp_id), Some(tp_id));
    }

    /// Make one setup current, and nothing else.
    ///
    /// `Scope::Setup` is NARROW: only this setup's operations get a Generate
    /// step, while its Simulate steps still cover every setup before it as
    /// those setups stand, so an earlier setup holding an ungenerated enabled
    /// operation stalls this setup's rest.
    ///
    /// It takes the same door as Generate All, so the R1 resolution question
    /// and the busy refusal are the same ones.
    pub(crate) fn handle_generate_setup_only(&mut self, setup_id: rs_cam_core::ids::SetupId) {
        use rs_cam_core::session::generation_plan::Scope;

        let _ = self.start_gui_plan(Scope::Setup(setup_id), None);
    }

    /// Arm a GUI plan over `scope`, asking about the cell size first when the
    /// stored project value is too coarse for the rest (R1).
    ///
    /// `target` is the operation the operator named. It regenerates even when
    /// it already holds a result; its ancestors do not.
    ///
    /// Answers `false` when nothing was armed, either because a plan already
    /// runs or because one is waiting on the resolution question. A caller
    /// holding a waiter must check [`Self::plan_is_busy`] BEFORE it stores
    /// one, because a plan that never starts resolves nothing.
    pub(crate) fn start_gui_plan(
        &mut self,
        scope: rs_cam_core::session::generation_plan::Scope,
        target: Option<ToolpathId>,
    ) -> bool {
        use crate::controller::generate_all::{GenerateAllSink, GenerationPlan, PlanStep};
        use rs_cam_core::session::SimulationResolution;
        use rs_cam_core::session::generation_plan::{Step, plan};

        // A second plan would race the first one's cursor, and a plan armed
        // behind an unanswered question would submit at a cell size the
        // operator has not agreed to.
        if self.plan_is_busy() {
            return false;
        }

        // R1, on the ONE stored value (G-RESTRES). The plan asks only when
        // it simulates and the stored `Fixed` cell is coarser than the rest
        // the project machines needs: at that cell the snapshot cannot see
        // what the coarse tool left. `Auto` already takes the finer of the
        // two, so it never asks.
        let simulates = plan(self.state.session(), scope)
            .iter()
            .any(|step| matches!(step, Step::Simulate { .. }));
        if simulates
            && let SimulationResolution::Fixed(stored_mm) =
                self.state.session().simulation_resolution()
            && let Some(required) = self.state.session().rest_resolution_required_mm()
            && stored_mm > required
        {
            self.pending_plan_confirm =
                Some(self.build_resolution_confirm(scope, required, stored_mm, target));
            return false;
        }

        let mut steps = self.plan_steps(scope, true);
        if !steps.is_empty() {
            // The GUI always closes with a full simulation, so the Simulation
            // workspace lands fresh. The step records `Skipped` when nothing
            // generated, because then the stock did not move.
            steps.push(PlanStep::SimulateAll);
        }
        let mut plan = GenerationPlan::new(steps, GenerateAllSink::Gui);
        if let Some(target) = target {
            plan = plan.with_target(target);
        }
        self.start_plan(plan);
        true
    }

    /// The question the operator answers when the stored cell is too coarse.
    fn build_resolution_confirm(
        &self,
        scope: rs_cam_core::session::generation_plan::Scope,
        required_mm: f64,
        panel_mm: f64,
        target: Option<ToolpathId>,
    ) -> crate::controller::PlanResolutionConfirm {
        use rs_cam_core::session::generation_plan::{Step, plan};

        let operations: Vec<String> = plan(self.state.session(), scope)
            .into_iter()
            .filter_map(|step| match step {
                Step::Simulate { upto, .. } => self
                    .state
                    .session()
                    .find_toolpath_config_by_id(upto)
                    .map(|(_, tc)| tc.name.clone()),
                Step::Generate { .. } => None,
            })
            .collect();
        let names = if operations.is_empty() {
            "the rest operations".to_owned()
        } else {
            operations.join(", ")
        };
        crate::controller::PlanResolutionConfirm {
            scope,
            target,
            required_mm,
            panel_mm,
            operations,
            message: format!(
                "To generate rest for {names}, the simulation needs cells of \
                 {required_mm:.3} mm. The project resolution is {panel_mm:.3} mm. A finer \
                 simulation is slower."
            ),
            accept_label: format!("Use {required_mm:.3} mm"),
        }
    }

    /// Take the finer cell size and start the plan.
    ///
    /// The answer SETS the stored project value (G-RESTRES): the plan, the
    /// next Run Simulation, MCP and the CLI all read it. It stays `Fixed`:
    /// the operator pinned a value, and the answer is a different pinned
    /// value, not a return to auto.
    pub fn accept_plan_resolution(&mut self) {
        let Some(confirm) = self.pending_plan_confirm.take() else {
            return;
        };
        if let Err(error) = self.set_simulation_resolution(
            rs_cam_core::session::SimulationResolution::Fixed(confirm.required_mm),
        ) {
            self.push_notification(
                format!("Could not set the simulation resolution: {error}"),
                super::super::Severity::Warning,
            );
            return;
        }
        let _ = self.start_gui_plan(confirm.scope, confirm.target);
    }

    /// Answer "no". Nothing is submitted and the panel is untouched.
    pub fn cancel_plan_resolution(&mut self) {
        self.pending_plan_confirm = None;
    }

    pub(crate) fn handle_inspect_toolpath_in_simulation(&mut self, tp_id: ToolpathId) {
        self.events.push(AppEvent::Ui(UiCommand::SwitchWorkspace(
            Workspace::Simulation,
        )));
        if let Some(boundary) = self
            .state
            .simulation
            .boundaries()
            .iter()
            .find(|boundary| boundary.id == tp_id)
        {
            self.events
                .push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: boundary.start_move,
                })));
        } else {
            let has_result = self
                .state
                .gui
                .toolpath_rt
                .get(&tp_id)
                .and_then(|rt| rt.run.result())
                .is_some();
            if has_result {
                self.state.simulation.debug.pending_inspect_toolpath = Some(tp_id);
                self.events.push(AppEvent::RunSimulationWith(vec![tp_id]));
            }
        }
    }
}

/// Step 7.8: what [`AppController::add_geodesic_pair`] added: the indices
/// of op 1 and the edge op (`None` without it), and the R2 and S11 notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeodesicPairAdded {
    pub body_index: usize,
    pub edges_index: Option<usize>,
    pub no_tool: Option<String>,
    pub feeds_refusal: Option<String>,
}
