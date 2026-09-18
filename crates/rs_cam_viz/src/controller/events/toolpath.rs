use rs_cam_core::session::{
    AddToolpathArgs, Command, Effects, MoveToolpathToSetupArgs, RemoveToolpathArgs,
    ReorderToolpathArgs,
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
    /// `None` means nothing was created: no tool, no geometry, or the
    /// add-time Suggest door refused the operation and tool pairing. The
    /// refusal reaches the operator as a toast and nothing else carries
    /// it (G-GUIADD).
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
            return None;
        };
        // Roadmap B.1–B.3 — pull stock-aware depth defaults so e.g.
        // a fresh DropCutter starts with `min_z = stock_bottom_z`
        // instead of the hard-coded `-50.0` that clipped any stock
        // not sitting at exactly that depth.
        let stock_bbox = self.state.session.stock_bbox();
        let stock_padding = self.state.session.stock_config().padding;
        let stock_ctx =
            rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(stock_bbox, stock_padding);
        // Q1: the model this toolpath will use is the one the config
        // below writes. Reading it BEFORE the Suggest call is the whole
        // fix: `SuggestContext::model_bbox` gates the runtime-sanity
        // stepover back-off, and every surface used to pass `None`.
        let model_id = self
            .state
            .session
            .models()
            .first()
            .map(|m| m.id)
            .unwrap_or(0);
        let model_bbox = self.state.session.model_bbox(model_id);
        let Some(tool) = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id.0 == tool_id)
        else {
            tracing::warn!("Cannot add toolpath: selected tool not found");
            self.push_notification(
                "Cannot add toolpath: selected tool not found".into(),
                super::super::Severity::Warning,
            );
            return None;
        };
        let (operation, feeds_provenance) = match rs_cam_core::feeds::suggest::suggest_params(
            rs_cam_core::feeds::suggest::SuggestParamsInput {
                op_type,
                tool,
                machine: self.state.session.machine(),
                material: &self.state.session.stock_config().material,
                workholding: self.state.session.stock_config().workholding_rigidity,
                lut: rs_cam_core::feeds::embedded_vendor_lut(),
                stock_ctx: &stock_ctx,
                spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
                // Q1: the bbox the runtime-sanity back-off reads. The
                // stock reaches Suggest through `stock_ctx` above, so
                // `SuggestContext::stock` stays empty rather than
                // carrying the same value twice.
                // `upstream_leftover_stock_mm` stays `None`: no lookup
                // here gives it, and v1 does not read it.
                context: rs_cam_core::feeds::suggest::SuggestContext {
                    model_bbox: model_bbox.as_ref(),
                    ..rs_cam_core::feeds::suggest::SuggestContext::default()
                },
            },
        ) {
            Ok(s) => (s.operation, s.provenance),
            Err(e) => {
                // Engine refused the tool × operation combination
                // (e.g. flat endmill on a Scallop op — no tip radius
                // means the scallop-stepover formula is undefined).
                // Surface the refusal to the user and bail; the
                // toolpath is not added.
                let msg = format!("Cannot add toolpath: {e}");
                tracing::warn!("{msg}");
                self.push_notification(msg, super::super::Severity::Warning);
                return None;
            }
        };
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
            return None;
        }
        let tc = rs_cam_core::session::ToolpathConfig {
            id: rs_cam_core::ToolpathId(0), // will be assigned by session
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
                let has_mesh = self.state.session.models().iter().any(|m| m.mesh.is_some());
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
            rest_analysis: crate::state::toolpath::RestAnalysisConfig::default(),
            stock_source: crate::state::toolpath::StockSource::Fresh,
            coolant: rs_cam_core::gcode::CoolantMode::Off,
            face_selection: None,
            debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance,
            planner_origin: None,
        };

        let effects = target_setup_idx.and_then(|setup_idx| {
            let command = Command::AddToolpath(AddToolpathArgs {
                setup_index: setup_idx,
                config: Box::new(tc),
            });
            self.apply_quietly(command)
        });
        let created = effects.as_ref().and_then(|effects| effects.created);
        if let Some(tp_idx) = created
            && let Some(tc) = self.state.session.toolpath_configs().get(tp_idx)
        {
            let tp_id = tc.id;
            // Create GUI runtime entry
            self.state.gui.toolpath_rt.insert(
                tc.id,
                crate::state::runtime::ToolpathRuntime::new(tc.operation.default_auto_regen()),
            );
            self.state.selection = Selection::Toolpath(tp_id);
        }
        self.state.gui.mark_edited();
        effects
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
            .find_toolpath_config_by_id(tp_id)
            .map(|(_, src)| {
                rs_cam_core::session::ToolpathConfig {
                    id: rs_cam_core::ToolpathId(0), // will be assigned by session
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
                    face_selection: src.face_selection.clone(),
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
                self.apply_created(command)
            });
            if let Some(tp_idx) = created
                && let Some(new_tc) = self.state.session.toolpath_configs().get(tp_idx)
            {
                let new_id = new_tc.id;
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
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id)
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
            let command = Command::ReorderToolpath(ReorderToolpathArgs {
                from_index: tp_idx,
                to_index: swap_with,
            });
            let _ = self.apply_quietly(command);
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_down(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id)
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
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id)
            && let Some(setup) = self
                .state
                .session
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices.contains(&tp_idx))
            && let Some(local_pos) = setup.toolpath_indices.iter().position(|&i| i == tp_idx)
        {
            let gap = target_idx.min(setup.toolpath_indices.len());
            let clamped = if gap > local_pos { gap - 1 } else { gap };
            if let Some(&target_global_idx) = setup.toolpath_indices.get(clamped)
                && tp_idx != target_global_idx
            {
                let command = Command::ReorderToolpath(ReorderToolpathArgs {
                    from_index: tp_idx,
                    to_index: target_global_idx,
                });
                let _ = self.apply_quietly(command);
                self.state.gui.mark_edited();
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
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id) {
            // Resolve the target setup's index from its ID
            let target_idx = self
                .state
                .session
                .list_setups()
                .iter()
                .position(|s| s.id == setup_id.0);
            if let Some(target_setup_idx) = target_idx {
                let command = Command::MoveToolpathToSetup(MoveToolpathToSetupArgs {
                    toolpath_index: tp_idx,
                    target_setup_index: target_setup_idx,
                    target_position: position,
                });
                let _ = self.apply_quietly(command);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_toolpath(&mut self, tp_id: ToolpathId) {
        if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id) {
            let command = Command::RemoveToolpath(RemoveToolpathArgs { index: tp_idx });
            let _ = self.apply_quietly(command);
            self.state.gui.toolpath_rt.remove(&tp_id);
            // W1: the viz twin `mark_derived_rest_dependents_stale` is NOT
            // called here any more. `remove_toolpath` walks the dependents
            // BEFORE it takes the config out
            // (`session/mutation/toolpath.rs:88-91`), so the Regions edge
            // still resolves, the walker names every consumer, and
            // `apply_quietly` stamps them from `Effects::stale`.
        }
        if self.state.selection == Selection::Toolpath(tp_id) {
            self.state.selection = Selection::None;
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

        let scope = generate_all_scope(self.state.session.toolpath_configs());
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

    /// Arm a GUI plan over `scope`, asking about the cell size first when the
    /// panel holds one that is too coarse for the rest (R1).
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
        use rs_cam_core::session::generation_plan::required_resolution_mm;

        // A second plan would race the first one's cursor, and a plan armed
        // behind an unanswered question would submit at a cell size the
        // operator has not agreed to.
        if self.plan_is_busy() {
            return false;
        }

        let required = required_resolution_mm(&self.state.session, scope);

        // R1. The Simulation panel's setting IS the operator's standing
        // choice: Run Simulation uses it as it stands, so reading it is not a
        // silent default. The plan only asks when the panel's PINNED value is
        // coarser than the rest it machines needs, because at that cell size
        // the snapshot cannot see what the coarse tool left.
        if let Some(required) = required
            && !self.state.simulation.auto_resolution
            && self.state.simulation.resolution.is_finite()
            && self.state.simulation.resolution > required
        {
            self.pending_plan_confirm =
                Some(self.build_resolution_confirm(scope, required, target));
            return false;
        }

        let mut steps = self.plan_steps(scope, true);
        if !steps.is_empty() {
            // The GUI always closes with a full simulation, so the Simulation
            // workspace lands fresh. The step records `Skipped` when nothing
            // generated, because then the stock did not move.
            steps.push(PlanStep::SimulateAll);
        }
        let mut plan = GenerationPlan::new(
            steps,
            required.map(crate::controller::generate_all::PlanResolution::AtMost),
            GenerateAllSink::Gui,
        );
        if let Some(target) = target {
            plan = plan.with_target(target);
        }
        self.start_plan(plan);
        true
    }

    /// The question the operator answers when the panel is too coarse.
    fn build_resolution_confirm(
        &self,
        scope: rs_cam_core::session::generation_plan::Scope,
        required_mm: f64,
        target: Option<ToolpathId>,
    ) -> crate::controller::PlanResolutionConfirm {
        use rs_cam_core::session::generation_plan::{Step, plan};

        let operations: Vec<String> = plan(&self.state.session, scope)
            .into_iter()
            .filter_map(|step| match step {
                Step::Simulate { upto, .. } => self
                    .state
                    .session
                    .find_toolpath_config_by_id(upto)
                    .map(|(_, tc)| tc.name.clone()),
                Step::Generate { .. } => None,
            })
            .collect();
        let panel_mm = self.state.simulation.resolution;
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
                 {required_mm:.3} mm. The panel is set to {panel_mm:.3} mm. A finer \
                 simulation is slower."
            ),
            accept_label: format!("Use {required_mm:.3} mm"),
        }
    }

    /// Take the finer cell size and start the plan.
    ///
    /// This is the ONE place a plan writes the Simulation panel. "Auto from
    /// tool size" stays off: the operator pinned a value, and the answer here
    /// is a different pinned value, not a return to auto.
    pub fn accept_plan_resolution(&mut self) {
        let Some(confirm) = self.pending_plan_confirm.take() else {
            return;
        };
        self.state.simulation.resolution = confirm.required_mm;
        self.state.simulation.auto_resolution = false;
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
                .and_then(|rt| rt.result.as_ref())
                .is_some();
            if has_result {
                self.state.simulation.debug.pending_inspect_toolpath = Some(tp_id);
                self.events.push(AppEvent::RunSimulationWith(vec![tp_id]));
            }
        }
    }
}
