//! The one door of a pin edit (P5, 2026-10-12; one drill per pin size
//! since R1).
//!
//! A pin edit is one of the three pin rows (`SetAlignmentPins`, and its
//! thin forms `AddAlignmentPin` and `RemoveAlignmentPin`). The row changes
//! the stock pins and stales the pin-drill operations. These steps are not
//! a core rule, so they are here:
//!
//! - the view mirrors the [`Effects`] (the stale stamps, the simulation,
//!   the GPU upload, the edited flag);
//! - the pin-drill operations follow the pins: ONE operation for each
//!   distinct pin diameter. The door creates a missing one and removes an
//!   operation whose diameter no pin has
//!   ([`AppController::sync_alignment_pin_drill`]). An operation reads the
//!   live pins at generation (W6.5), and the pin row stales it (P4);
//! - a load-time pin warning that the live pins no longer give leaves the
//!   load-warning list (R5a).
//!
//! Every surface that edits the pins ends here: the MCP describe step
//! calls [`AppController::finish_pin_edit`], the undo arm, the two-sided
//! setup and the pin wizard call [`AppController::apply_pin_edit`], and a
//! stock draft with a pin change raises `pin_drill_sync`, which the frame
//! loop discharges through the same synchronisation. Thus a GUI edit and
//! an MCP edit leave the same toolpath list.
//!
//! The drill of one size is chosen here too
//! ([`AppController::bind_pin_drill_tool`]): the pin wizard's Apply adds the
//! drills it queued and binds the tools it chose (R2, R4). The wizard's
//! part-model choice applies here too
//! ([`AppController::set_pin_part_models`]).

use rs_cam_core::compute::alignment_pins::pin_diameters;
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, Command, Effects, RemoveToolpathArgs, SessionError,
    SetToolpathToolArgs,
};

use crate::compute::ComputeBackend;
use crate::state::toolpath::ToolpathId;

use super::super::AppController;

/// What the pin-drill synchronisation did for one pin size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PinDrillSync {
    /// Nothing to do: the operation already drills the pins of the size.
    Unchanged,
    /// The door created the pin-drill operation of the size in this setup
    /// (the first `Top` setup at 0 deg).
    Created {
        toolpath_id: ToolpathId,
        setup_index: usize,
    },
    /// The door bound the operation to another tool of the size.
    Updated { toolpath_id: ToolpathId },
    /// No pin has the size of the operation any more, so the door removed
    /// it. A second operation of one size is removed too.
    Removed { toolpath_id: ToolpathId },
    /// Pins of the size exist, no operation drills them, and the door
    /// cannot create one. The text says why.
    Refused(String),
}

impl PinDrillSync {
    /// The reply word for this outcome.
    #[cfg_attr(
        not(feature = "mcp"),
        expect(
            dead_code,
            reason = "only the MCP server reads it, and the `mcp` feature is off"
        )
    )]
    pub(crate) fn action(&self) -> &'static str {
        match self {
            PinDrillSync::Unchanged => "unchanged",
            PinDrillSync::Created { .. } => "created",
            PinDrillSync::Updated { .. } => "updated",
            PinDrillSync::Removed { .. } => "removed",
            PinDrillSync::Refused(_) => "refused",
        }
    }
}

/// The outcome of the synchronisation for one pin size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PinDrillSizeSync {
    /// The pin diameter of the size, in mm.
    pub diameter: f64,
    pub outcome: PinDrillSync,
}

/// The answer of the pin-edit door.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PinEdit {
    /// The toolpath indices that went stale, after the synchronisation,
    /// in ascending order. A removed toolpath is not in the list.
    pub stale: Vec<usize>,
    /// What the synchronisation did, one row for each pin size and each
    /// removed operation, in that order.
    pub pin_drill: Vec<PinDrillSizeSync>,
}

impl PinEdit {
    /// The outcome of the size `diameter`, or `None`.
    #[cfg(test)]
    pub(crate) fn outcome_of(&self, diameter: f64) -> Option<&PinDrillSync> {
        self.pin_drill
            .iter()
            .find(|s| rs_cam_core::compute::alignment_pins::same_pin_diameter(s.diameter, diameter))
            .map(|s| &s.outcome)
    }
}

/// The refusal when no setup is a `Top` setup at 0 deg.
///
/// The pin-drill operation drills the dowel holes from the top of the
/// stock, at the pin positions as the pin wizard places them. The
/// generator does not turn the pin holes with the setup. So a flipped
/// setup, or a `Top` setup turned 90, 180 or 270 deg, drills the same SET
/// of holes only when the pattern maps onto itself under that turn (C7).
/// The door does not put the operation there (lead ruling 2026-10-12).
pub(crate) const PIN_DRILL_NEEDS_TOP_SETUP: &str = "The pin-drill operation was not created: no \
     setup has face 'Top' at 0 deg. The pin holes are drilled from the top of the stock at the \
     stock-panel positions, and the holes are not turned with a turned setup. Add a Top setup \
     at 0 deg; the pin wizard's 'Add a Top setup' does this.";

/// The refusal when no tool drills the holes of the pin size `diameter`
/// (`ProjectSession::pin_drill_tool_for`). The door does not take
/// `tools[0]`: on the mini test piece that is a 3 mm end mill, and 3 mm
/// holes do not take 6 mm dowels (P6).
///
/// The text names the fix in the pin wizard (R2).
/// The MCP route adds the tool with `add_tool` and edits the pins again.
#[must_use]
pub(crate) fn pin_drill_needs_tool(diameter: f64) -> String {
    format!(
        "The pin-drill operation of the {diameter:.2} mm pins was not created: no tool of \
         {diameter:.2} mm is in the project. Press 'Add a {diameter:.2} mm drill' in the pin \
         wizard, or add a {diameter:.2} mm tool (MCP add_tool) and edit the pins again."
    )
}

/// Why the door cannot create a pin-drill operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PinDrillBlocker {
    /// No setup is a `Top` setup at 0 deg ([`PIN_DRILL_NEEDS_TOP_SETUP`]).
    NeedsTopSetup,
    /// No tool drills the holes of the pin size
    /// ([`pin_drill_needs_tool`]).
    NeedsTool { diameter: f64 },
}

impl PinDrillBlocker {
    /// The refusal text.
    #[must_use]
    #[cfg_attr(
        not(feature = "mcp"),
        expect(
            dead_code,
            reason = "only the MCP server reads it, and the `mcp` feature is off"
        )
    )]
    pub(crate) fn text(&self) -> String {
        match self {
            PinDrillBlocker::NeedsTopSetup => PIN_DRILL_NEEDS_TOP_SETUP.to_owned(),
            PinDrillBlocker::NeedsTool { diameter } => pin_drill_needs_tool(*diameter),
        }
    }
}

/// Why the door cannot create the pin-drill operation of the pin size
/// `diameter` now, or `None` when it can: [`PinDrillBlocker::NeedsTopSetup`]
/// with no `Top` setup at 0 deg, else [`PinDrillBlocker::NeedsTool`] with
/// no tool that drills the holes of the size.
///
/// The door, the stock panel's pin block and the wizard's per-size rows
/// read this one function, so a row states the refusal the door gives.
pub(crate) fn pin_drill_blocker(
    session: &rs_cam_core::session::ProjectSession,
    diameter: f64,
) -> Option<PinDrillBlocker> {
    if pin_drill_setup(session).is_none() {
        return Some(PinDrillBlocker::NeedsTopSetup);
    }
    if session.pin_drill_tool_for(diameter).is_none() {
        return Some(PinDrillBlocker::NeedsTool { diameter });
    }
    None
}

/// The setup that takes a new pin-drill operation: the first `Top` setup
/// at 0 deg (C7), not setup index 0.
fn pin_drill_setup(session: &rs_cam_core::session::ProjectSession) -> Option<usize> {
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};
    session
        .list_setups()
        .iter()
        .position(|s| s.face_up == FaceUp::Top && s.z_rotation == ZRotation::Deg0)
}

impl<B: ComputeBackend> AppController<B> {
    /// Apply one pin row and run the rest of the door.
    ///
    /// # Errors
    ///
    /// The session's refusal of the row. A refused row changes nothing,
    /// so the door does not run.
    pub(crate) fn apply_pin_edit(&mut self, command: Command) -> Result<PinEdit, SessionError> {
        let effects = self.apply(command)?;
        Ok(self.finish_pin_edit(&effects))
    }

    /// Run the door after a pin row that the caller applied.
    ///
    /// The MCP dispatch applies every row through one arm, so its describe
    /// step calls this with the row's [`Effects`].
    pub(crate) fn finish_pin_edit(&mut self, effects: &Effects) -> PinEdit {
        let stale_ids: Vec<ToolpathId> = effects
            .stale
            .iter()
            .filter_map(|&index| self.state.session().toolpath_configs().get(index))
            .map(|tc| tc.id)
            .collect();
        // `AppController::apply` stamped `effects.stale` and cleared the
        // viewport simulation (F096).
        self.pending_upload = true;
        self.state.gui.mark_edited();

        let pin_drill = self.sync_pin_drill_inner();
        self.refresh_pin_load_warnings();

        let stale: Vec<usize> = self
            .state
            .session()
            .toolpath_configs()
            .iter()
            .enumerate()
            .filter(|(_, tc)| stale_ids.contains(&tc.id))
            .map(|(index, _)| index)
            .collect();
        PinEdit { stale, pin_drill }
    }

    /// Create, move or remove the pin-drill operations so that they drill
    /// the stock pins, one operation for each pin size, and tell the
    /// operator when one cannot be created.
    ///
    /// The GUI doors call this. The MCP door does not push the refusal as
    /// a notification: its reply carries the refusal, and the MCP toast
    /// reads the reply (one toast per request).
    pub(crate) fn sync_alignment_pin_drill(&mut self) -> Vec<PinDrillSizeSync> {
        let outcome = self.sync_pin_drill_inner();
        self.refresh_pin_load_warnings();
        // A refusal toasts once. A nudge of a set with a size that has no
        // drill refuses on every edit, and the pin block and the wizard
        // already say so in place; a toast on each edit is noise.
        let refusals: Vec<String> = outcome
            .iter()
            .filter_map(|size| match &size.outcome {
                PinDrillSync::Refused(reason) => Some(reason.clone()),
                _ => None,
            })
            .collect();
        for reason in &refusals {
            if !self.pin_drill_refusals_toasted.contains(reason) {
                self.push_notification(reason.clone(), super::super::Severity::Warning);
            }
        }
        self.pin_drill_refusals_toasted = refusals;
        outcome
    }

    /// Bind the pin-drill operation of the pin size `diameter` to the
    /// tool `tool_id` (R2: the per-size tool chooser of the pin wizard).
    ///
    /// The operation exists: a `SetToolpathTool` row rebinds it. It does
    /// not exist: the door creates it with that tool.
    pub(crate) fn bind_pin_drill_tool(
        &mut self,
        diameter: f64,
        tool_id: rs_cam_core::ids::ToolId,
    ) -> PinDrillSync {
        let outcome = match self.state.session().pin_drill_op_for(diameter) {
            Some(index) => {
                let toolpath_id = self
                    .state
                    .session()
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| tc.id);
                let command = Command::SetToolpathTool(SetToolpathToolArgs { index, tool_id });
                match (self.apply_quietly(command), toolpath_id) {
                    (Some(_), Some(toolpath_id)) => {
                        // F031: the rebind dropped the result, so the run
                        // record reads Pending. It used to clear the
                        // result and keep `Done`.
                        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id) {
                            rt.run = crate::state::runtime::ToolpathRun::Pending;
                        }
                        PinDrillSync::Updated { toolpath_id }
                    }
                    _ => PinDrillSync::Refused(
                        "The session refused the tool of the pin-drill operation.".to_owned(),
                    ),
                }
            }
            None => self.create_pin_drill(diameter, Some(tool_id)),
        };
        self.pending_upload = true;
        self.state.gui.mark_edited();
        if let PinDrillSync::Refused(reason) = &outcome {
            self.push_notification(reason.clone(), super::super::Severity::Warning);
        }
        outcome
    }

    /// Apply the pin wizard (R4): the minimum rows, in this order.
    ///
    /// 1. One `AddTool` row for each size of `add_drills`
    ///    (`ProjectSession::pin_drill_tool_template`).
    /// 2. One `SetAlignmentPins` row with `pins`, through
    ///    [`Self::apply_pin_edit`]: the door synchronises the pin-drill
    ///    operations, one for each pin size.
    /// 3. A size whose drill the wizard added gets that drill, and a size
    ///    in `use_tools` gets the tool the operator chose
    ///    ([`Self::bind_pin_drill_tool`]), when the door chose another tool.
    ///
    /// The pin edit is ONE undo step (`UndoAction::StockChange`). A tool
    /// add and the door's toolpath rows are outside the undo history, as
    /// for every pin edit; `UndoAction` has no tool or toolpath add.
    ///
    /// # Errors
    ///
    /// The session's refusal of the pin row.
    pub(crate) fn apply_pin_wizard(
        &mut self,
        pins: Vec<crate::state::job::AlignmentPin>,
        add_drills: &[f64],
        use_tools: &[(f64, rs_cam_core::ids::ToolId)],
    ) -> Result<PinEdit, SessionError> {
        let old = self.state.session().stock_config().clone();
        let mut added: Vec<(f64, rs_cam_core::ids::ToolId)> = Vec::new();
        for &d in add_drills {
            let tool = self.state.session().pin_drill_tool_template(d);
            if let Some(index) = self.apply_created(Command::AddTool(AddToolArgs {
                tool: Box::new(tool),
            })) && let Some(t) = self.state.session().tools().get(index)
            {
                added.push((d, t.id));
            }
        }
        let edit = self.apply_pin_edit(Command::SetAlignmentPins(
            rs_cam_core::session::SetAlignmentPinsArgs { pins },
        ))?;
        for (d, tool_id) in added.into_iter().chain(use_tools.iter().copied()) {
            let bound = self
                .state
                .session()
                .pin_drill_op_for(d)
                .and_then(|i| self.state.session().toolpath_configs().get(i))
                .map(|tc| tc.tool_id);
            // No operation: the synchronisation above refused the size and
            // said why; a bind would refuse and toast a second time.
            if bound.is_some_and(|b| b != tool_id) {
                let _ = self.bind_pin_drill_tool(d, tool_id);
            }
        }
        let new = self.state.session().stock_config().clone();
        if old != new {
            self.state
                .history
                .push(crate::state::history::UndoAction::StockChange { old, new });
        }
        for size in &edit.pin_drill {
            if let PinDrillSync::Refused(reason) = &size.outcome {
                self.push_notification(reason.clone(), super::super::Severity::Warning);
            }
        }
        Ok(edit)
    }

    /// Add a setup with the face `face_up` at 0 deg for the pins, then run
    /// the pin-drill synchronisation. The selection stays, so the operator
    /// stays in the pin wizard (R4).
    ///
    /// A `Top` setup gives the pin-drill operation a home; a `Bottom`
    /// setup gives the pins a flip to key.
    pub(crate) fn handle_pin_add_setup(&mut self, face_up: crate::state::job::FaceUp) {
        let next_id = self.state.session().list_setups().len() + 1;
        let command = Command::AddSetup(rs_cam_core::session::AddSetupArgs {
            name: Some(format!("Setup {next_id}")),
            face_up,
        });
        if self.apply_created(command).is_some() {
            self.state.gui.mark_edited();
            self.pending_upload = true;
            let _ = self.sync_alignment_pin_drill();
        }
    }

    /// Apply the pin wizard's part-model choice (`None`: the default rule)
    /// as one `SetPinPartModels` row, with one undo step. The row stales
    /// nothing.
    pub(crate) fn set_pin_part_models(&mut self, models: Option<Vec<rs_cam_core::ids::ModelId>>) {
        let old = self.state.session().stock_config().clone();
        let command =
            Command::SetPinPartModels(rs_cam_core::session::SetPinPartModelsArgs { models });
        if self.apply_quietly(command).is_none() {
            return;
        }
        self.state.gui.mark_edited();
        self.pending_upload = true;
        let new = self.state.session().stock_config().clone();
        if old != new {
            self.state
                .history
                .push(crate::state::history::UndoAction::StockChange { old, new });
        }
    }

    /// Drop a load-time pin warning that the live pins no longer give
    /// (R5a). The load-warning modal reads the list, so a pin edit that
    /// keys the set clears its warning at once. With no warning left, the
    /// modal closes.
    fn refresh_pin_load_warnings(&mut self) {
        if self.load_pin_warnings.is_empty() {
            return;
        }
        let session = self.state.session();
        let live: Vec<String> = session
            .stock_config()
            .pin_findings_for_setups(
                session
                    .list_setups()
                    .iter()
                    .map(|s| (s.face_up, s.z_rotation)),
            )
            .into_iter()
            .map(|f| f.text)
            .collect();
        let gone: Vec<String> = self
            .load_pin_warnings
            .iter()
            .filter(|w| !live.contains(w))
            .cloned()
            .collect();
        if gone.is_empty() {
            return;
        }
        self.load_warnings.retain(|w| !gone.contains(w));
        self.load_pin_warnings.retain(|w| !gone.contains(w));
        if self.load_warnings.is_empty() {
            self.show_load_warnings = false;
        }
    }

    /// The synchronisation.
    ///
    /// 1. Remove each pin-drill operation whose diameter no pin has, and
    ///    each second operation of one size.
    /// 2. For each pin size: create the missing operation.
    fn sync_pin_drill_inner(&mut self) -> Vec<PinDrillSizeSync> {
        use rs_cam_core::compute::alignment_pins::same_pin_diameter;

        let pins = self.state.session().stock_config().alignment_pins.clone();
        let sizes = pin_diameters(&pins);
        let mut out = Vec::new();

        // 1. Remove. The indices shift after each removal, so scan again.
        loop {
            let ops = self.state.session().pin_drill_ops();
            let extra = ops.iter().enumerate().find_map(|(k, &(index, d))| {
                let unused = !sizes.iter().any(|s| same_pin_diameter(*s, d));
                let second = ops
                    .get(..k)
                    .is_some_and(|before| before.iter().any(|&(_, e)| same_pin_diameter(e, d)));
                (unused || second).then_some((index, d))
            });
            let Some((index, diameter)) = extra else {
                break;
            };
            let Some(toolpath_id) = self
                .state
                .session()
                .toolpath_configs()
                .get(index)
                .map(|tc| tc.id)
            else {
                break;
            };
            let command = Command::RemoveToolpath(RemoveToolpathArgs { index });
            if self.apply_quietly(command).is_none() {
                break;
            }
            out.push(PinDrillSizeSync {
                diameter,
                outcome: PinDrillSync::Removed { toolpath_id },
            });
        }

        // 2. One operation for each size. An existing operation reads the
        // live pins at generation (W6.5), so a pin move changes nothing
        // here: the `SetAlignmentPins` row already staled it (P4).
        for diameter in sizes {
            let outcome = match self.state.session().pin_drill_op_for(diameter) {
                None => self.create_pin_drill(diameter, None),
                Some(_) => PinDrillSync::Unchanged,
            };
            out.push(PinDrillSizeSync { diameter, outcome });
        }
        out
    }

    /// Create the pin-drill operation of the pin size `diameter` in the
    /// first `Top` setup at 0 deg, with `tool` or, for `None`, the tool
    /// `ProjectSession::pin_drill_tool_for` names.
    ///
    /// A second size copies the drilling settings of the first operation
    /// (the spoilboard penetration, the cycle, the peck and the retract),
    /// so every dowel hole goes to the same depth. The feeds come from the
    /// suggest funnel of the new tool.
    fn create_pin_drill(
        &mut self,
        diameter: f64,
        tool: Option<rs_cam_core::ids::ToolId>,
    ) -> PinDrillSync {
        use crate::state::toolpath::{AlignmentPinDrillConfig, OperationConfig};

        let Some(setup_idx) = pin_drill_setup(self.state.session()) else {
            return PinDrillSync::Refused(PIN_DRILL_NEEDS_TOP_SETUP.to_owned());
        };
        let Some(tool_id) = tool.or_else(|| self.state.session().pin_drill_tool_for(diameter))
        else {
            return PinDrillSync::Refused(pin_drill_needs_tool(diameter));
        };
        let sibling: Option<AlignmentPinDrillConfig> = self
            .state
            .session()
            .toolpath_configs()
            .iter()
            .find_map(|tc| match &tc.operation {
                OperationConfig::AlignmentPinDrill(c) => Some(c.clone()),
                _ => None,
            });
        let model_id = self
            .state
            .session()
            .models()
            .first()
            .map_or(rs_cam_core::ids::ModelId::placeholder(), |m| m.id);
        // F1 (2026-06-10): run the suggest funnel instead of raw
        // defaults — `..Default::default()` shipped the hardcoded
        // 300 mm/min feed, which on a Ø6 pin drill is exactly the
        // SolidWood rubbing floor (50 mm/min per mm Ø). Mirrors
        // the add-toolpath path in `events/toolpath.rs`. On a
        // suggest refusal, fall back to defaults rather than
        // skipping the auto-create.
        let stock_bbox = self.state.session().stock_bbox();
        let stock_padding = self.state.session().stock_config().padding;
        let stock_ctx =
            rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(stock_bbox, stock_padding);
        let model_bbox = self.state.session().model_bbox_by_id(model_id);
        let suggested = self
            .state
            .session()
            .tools()
            .iter()
            .find(|t| t.id == tool_id)
            .and_then(|tool| {
                rs_cam_core::feeds::suggest::suggest_params(
                    rs_cam_core::feeds::suggest::SuggestParamsInput {
                        op_type: crate::state::toolpath::OperationType::AlignmentPinDrill,
                        tool,
                        machine: self.state.session().machine(),
                        material: &self.state.session().stock_config().material,
                        lut: rs_cam_core::feeds::embedded_vendor_lut(),
                        stock_ctx: &stock_ctx,
                        spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
                        // Q1: the bbox the runtime-sanity
                        // back-off reads. `model_id` is already
                        // resolved above. The stock reaches
                        // Suggest through `stock_ctx`, so
                        // `SuggestContext::stock` stays empty.
                        // `upstream_leftover_stock_mm` stays
                        // `None`: no lookup here gives it.
                        context: rs_cam_core::feeds::suggest::SuggestContext {
                            model_bbox: model_bbox.as_ref(),
                            ..rs_cam_core::feeds::suggest::SuggestContext::default()
                        },
                    },
                )
                .ok()
            });
        let (mut cfg, feeds_provenance) = match suggested {
            Some(s) => match s.operation {
                OperationConfig::AlignmentPinDrill(c) => (c, s.provenance),
                // suggest_params builds from the requested op_type,
                // so this arm is unreachable; defaults keep it total.
                _ => (
                    AlignmentPinDrillConfig::default(),
                    rs_cam_core::feeds::FeedsProvenance::default(),
                ),
            },
            None => {
                tracing::warn!("pin-drill auto-create: suggest refused; falling back to defaults");
                (
                    AlignmentPinDrillConfig::default(),
                    rs_cam_core::feeds::FeedsProvenance::default(),
                )
            }
        };
        cfg.diameter = diameter;
        if let Some(sibling) = &sibling {
            cfg.spoilboard_penetration = sibling.spoilboard_penetration;
            cfg.cycle = sibling.cycle;
            cfg.peck_depth = sibling.peck_depth;
            cfg.retract_z = sibling.retract_z;
        }
        // One size keeps the plain name; a second size names its diameter.
        let name = if sibling.is_some() {
            format!("Pin Drill {diameter:.2} mm")
        } else {
            "Pin Drill".to_owned()
        };
        let tc = rs_cam_core::session::ToolpathConfig {
            shoe: Default::default(),
            id: rs_cam_core::ToolpathId::placeholder(), // will be assigned by session
            name,
            enabled: true,
            operation: OperationConfig::AlignmentPinDrill(cfg),
            dressups: crate::state::toolpath::DressupConfig::default(),
            heights: crate::state::toolpath::HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::state::toolpath::BoundaryConfig::default(),
            boundary_inherit: true,
            rest_analysis: crate::state::toolpath::RestAnalysisConfig::default(),
            stock_source: crate::state::toolpath::StockSource::Fresh,
            coolant: rs_cam_core::gcode::CoolantMode::Off,
            debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance,
            planner_origin: None,
        };
        let command = Command::AddToolpath(AddToolpathArgs {
            setup_index: setup_idx,
            config: Box::new(tc),
        });
        let created = self
            .apply_quietly(command)
            .and_then(|effects| effects.created)
            .and_then(|index| self.state.session().toolpath_configs().get(index))
            .map(|tc| tc.id);
        match created {
            Some(toolpath_id) => PinDrillSync::Created {
                toolpath_id,
                setup_index: setup_idx,
            },
            None => PinDrillSync::Refused(
                "The pin-drill operation was not created: the session refused the new \
                 toolpath."
                    .to_owned(),
            ),
        }
    }
}
