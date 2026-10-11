//! The one door of a pin edit (P5, 2026-10-12).
//!
//! A pin edit is one of the three pin rows (`SetAlignmentPins`, and its
//! thin forms `AddAlignmentPin` and `RemoveAlignmentPin`). The row changes
//! the stock pins and stales the pin-drill operations. Two steps are not
//! a core rule, so they are here:
//!
//! - the view mirrors the [`Effects`] (the stale stamps, the simulation,
//!   the GPU upload, the edited flag);
//! - the pin-drill operation follows the pins: the door creates it, moves
//!   its holes, or removes it ([`AppController::sync_alignment_pin_drill`]).
//!
//! Every surface that edits the pins ends here: the MCP describe step
//! calls [`AppController::finish_pin_edit`], the undo arm and the two-sided
//! setup call [`AppController::apply_pin_edit`], and the stock panel
//! raises `pin_drill_sync`, which the frame loop discharges through the
//! same synchronisation. Thus a GUI edit and an MCP edit leave the same
//! toolpath list.

use rs_cam_core::session::{
    AddToolpathArgs, Command, Effects, RemoveToolpathArgs, SessionError,
    SetAlignmentPinDrillHolesArgs,
};

use crate::compute::ComputeBackend;
use crate::state::toolpath::ToolpathId;

use super::super::AppController;

/// What the pin-drill synchronisation did.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PinDrillSync {
    /// Nothing to do: no pins and no pin-drill operation, or the
    /// operation already drills the pins.
    Unchanged,
    /// The door created the pin-drill operation in this setup (the first
    /// `Top` setup at 0 deg).
    Created {
        toolpath_id: ToolpathId,
        setup_index: usize,
    },
    /// The door moved the holes of the pin-drill operation to the pins.
    Updated { toolpath_id: ToolpathId },
    /// The pins are gone, so the door removed the pin-drill operation.
    Removed { toolpath_id: ToolpathId },
    /// Pins exist and no pin-drill operation exists, and the door cannot
    /// create one. The text says why.
    Refused(String),
}

impl PinDrillSync {
    /// The reply word for this outcome.
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

/// The answer of the pin-edit door.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PinEdit {
    /// The toolpath indices that went stale, after the synchronisation,
    /// in ascending order. A removed toolpath is not in the list.
    pub stale: Vec<usize>,
    /// What the synchronisation did to the pin-drill operation.
    pub pin_drill: PinDrillSync,
}

/// The refusal when no setup is a `Top` setup at 0 deg.
///
/// The pin-drill operation drills the dowel holes from the top of the
/// stock, at the pin positions as the stock panel dimensions them. The
/// generator does not turn the pin holes with the setup. So a flipped
/// setup, or a `Top` setup turned 90, 180 or 270 deg, drills the same SET
/// of holes only when the pattern maps onto itself under that turn (C7).
/// The door does not put the operation there (lead ruling 2026-10-12).
pub(crate) const PIN_DRILL_NEEDS_TOP_SETUP: &str = "The pin-drill operation was not created: no \
     setup has face 'Top' at 0 deg. The pin holes are drilled from the top of the stock at the \
     stock-panel positions, and the holes are not turned with a turned setup. Add a Top setup \
     at 0 deg, then edit the pins again.";

/// The refusal when the project has no tool.
pub(crate) const PIN_DRILL_NEEDS_TOOL: &str = "The pin-drill operation was not created: the \
     project has no tool. Add a drill, then edit the pins again.";

impl<B: ComputeBackend> AppController<B> {
    /// Apply one pin row and run the rest of the door.
    ///
    /// # Errors
    ///
    /// The session's refusal of the row. A refused row changes nothing,
    /// so the door does not run.
    pub(crate) fn apply_pin_edit(&mut self, command: Command) -> Result<PinEdit, SessionError> {
        let effects = self.state.session.apply(command)?;
        Ok(self.finish_pin_edit(&effects))
    }

    /// Run the door after a pin row that the caller applied.
    ///
    /// The MCP dispatch applies every row through one arm, so its describe
    /// step calls this with the row's [`Effects`].
    pub(crate) fn finish_pin_edit(&mut self, effects: &Effects) -> PinEdit {
        let mut stale_ids: Vec<ToolpathId> = effects
            .stale
            .iter()
            .filter_map(|&index| self.state.session.toolpath_configs().get(index))
            .map(|tc| tc.id)
            .collect();
        crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
        if effects.simulation_cleared {
            self.invalidate_simulation();
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();

        let (pin_drill, sync_stale) = self.sync_pin_drill_inner();
        stale_ids.extend(sync_stale);

        let stale: Vec<usize> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .filter(|(_, tc)| stale_ids.contains(&tc.id))
            .map(|(index, _)| index)
            .collect();
        PinEdit { stale, pin_drill }
    }

    /// Create, move or remove the pin-drill operation so that it drills
    /// the stock pins, and tell the operator when it cannot.
    ///
    /// The GUI doors call this. The MCP door does not push the refusal as
    /// a notification: its reply carries the refusal, and the MCP toast
    /// reads the reply (one toast per request).
    pub(crate) fn sync_alignment_pin_drill(&mut self) -> PinDrillSync {
        let (outcome, _) = self.sync_pin_drill_inner();
        if let PinDrillSync::Refused(reason) = &outcome {
            self.push_notification(reason.clone(), super::super::Severity::Warning);
        }
        outcome
    }

    /// The synchronisation, and the ids of the toolpaths it made stale.
    fn sync_pin_drill_inner(&mut self) -> (PinDrillSync, Vec<ToolpathId>) {
        use crate::state::toolpath::OperationConfig;

        let pin_holes: Vec<[f64; 2]> = self
            .state
            .session
            .stock_config()
            .alignment_pins
            .iter()
            .map(|p| [p.x, p.y])
            .collect();

        // Find existing pin drill toolpath across all setups.
        let existing: Option<(usize, ToolpathId, Vec<[f64; 2]>)> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find_map(|(idx, tc)| match &tc.operation {
                OperationConfig::AlignmentPinDrill(c) => Some((idx, tc.id, c.holes.clone())),
                _ => None,
            });

        match existing {
            None if pin_holes.is_empty() => (PinDrillSync::Unchanged, Vec::new()),
            None => (self.create_pin_drill(pin_holes), Vec::new()),
            Some((idx, toolpath_id, _)) if pin_holes.is_empty() => {
                // The pins are gone: remove the pin-drill operation.
                let command = Command::RemoveToolpath(RemoveToolpathArgs { index: idx });
                match self.apply_quietly(command) {
                    Some(_) => (PinDrillSync::Removed { toolpath_id }, Vec::new()),
                    None => (PinDrillSync::Unchanged, Vec::new()),
                }
            }
            Some((_, _, holes)) if holes == pin_holes => (PinDrillSync::Unchanged, Vec::new()),
            Some((idx, toolpath_id, _)) => {
                // Pins exist and the operation exists: move its holes.
                let command = Command::SetAlignmentPinDrillHoles(SetAlignmentPinDrillHolesArgs {
                    index: idx,
                    holes: pin_holes,
                });
                let stale: Vec<ToolpathId> = self
                    .apply_quietly(command)
                    .map(|effects| {
                        effects
                            .stale
                            .iter()
                            .filter_map(|&i| self.state.session.toolpath_configs().get(i))
                            .map(|tc| tc.id)
                            .collect()
                    })
                    .unwrap_or_default();
                // The row stamps `stale_since` on every index it dropped.
                // Dropping the cached geometry is a view decision, and no
                // core answer carries it.
                if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&toolpath_id) {
                    rt.result = None;
                }
                (PinDrillSync::Updated { toolpath_id }, stale)
            }
        }
    }

    /// Create the pin-drill operation in the first `Top` setup.
    fn create_pin_drill(&mut self, holes: Vec<[f64; 2]>) -> PinDrillSync {
        use crate::state::toolpath::{AlignmentPinDrillConfig, OperationConfig};

        // C7: the first Top setup at 0 deg, not setup index 0.
        let Some(setup_idx) = self.state.session.list_setups().iter().position(|s| {
            s.face_up == rs_cam_core::compute::transform::FaceUp::Top
                && s.z_rotation == rs_cam_core::compute::transform::ZRotation::Deg0
        }) else {
            return PinDrillSync::Refused(PIN_DRILL_NEEDS_TOP_SETUP.to_owned());
        };
        let Some(tool_id) = self.state.session.tools().first().map(|t| t.id.0) else {
            return PinDrillSync::Refused(PIN_DRILL_NEEDS_TOOL.to_owned());
        };
        let model_id = self
            .state
            .session
            .models()
            .first()
            .map(|m| m.id)
            .unwrap_or(0);
        // F1 (2026-06-10): run the suggest funnel instead of raw
        // defaults — `..Default::default()` shipped the hardcoded
        // 300 mm/min feed, which on a Ø6 pin drill is exactly the
        // SolidWood rubbing floor (50 mm/min per mm Ø). Mirrors
        // the add-toolpath path in `events/toolpath.rs`. On a
        // suggest refusal, fall back to defaults rather than
        // skipping the auto-create.
        let stock_bbox = self.state.session.stock_bbox();
        let stock_padding = self.state.session.stock_config().padding;
        let stock_ctx =
            rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(stock_bbox, stock_padding);
        let model_bbox = self.state.session.model_bbox(model_id);
        let suggested = self
            .state
            .session
            .tools()
            .iter()
            .find(|t| t.id.0 == tool_id)
            .and_then(|tool| {
                rs_cam_core::feeds::suggest::suggest_params(
                    rs_cam_core::feeds::suggest::SuggestParamsInput {
                        op_type: crate::state::toolpath::OperationType::AlignmentPinDrill,
                        tool,
                        machine: self.state.session.machine(),
                        material: &self.state.session.stock_config().material,
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
        let (cfg, feeds_provenance) = match suggested {
            Some(s) => match s.operation {
                OperationConfig::AlignmentPinDrill(mut c) => {
                    c.holes = holes;
                    (c, s.provenance)
                }
                // suggest_params builds from the requested op_type,
                // so this arm is unreachable; defaults keep it total.
                _ => (
                    AlignmentPinDrillConfig {
                        holes,
                        ..Default::default()
                    },
                    rs_cam_core::feeds::FeedsProvenance::default(),
                ),
            },
            None => {
                tracing::warn!("pin-drill auto-create: suggest refused; falling back to defaults");
                (
                    AlignmentPinDrillConfig {
                        holes,
                        ..Default::default()
                    },
                    rs_cam_core::feeds::FeedsProvenance::default(),
                )
            }
        };
        let tc = rs_cam_core::session::ToolpathConfig {
            id: rs_cam_core::ToolpathId(0), // will be assigned by session
            name: "Pin Drill".to_owned(),
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
            face_selection: None,
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
            .and_then(|index| self.state.session.toolpath_configs().get(index))
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
