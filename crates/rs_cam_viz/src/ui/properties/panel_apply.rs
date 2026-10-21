//! Panel drafts and the command door.
//!
//! An immediate-mode panel edits a CLONE of the project record and applies
//! one `Command` when the edit finishes. These are the apply and flush
//! halves of that idiom (WP6): they own no state and draw nothing.

use crate::state::AppState;

/// TOO-003 — flush the pending tool draft if the user navigated away from
/// the tool. Edits are pending-until-committed in the panel, but navigating
/// away **auto-commits** them (the spec's accepted fallback) so a stray
/// click elsewhere can't silently drop edits. If still editing the same
/// tool, the draft is left in place.
pub(super) fn flush_tool_draft(state: &mut AppState) {
    let Some((tool_id, draft)) = state.history.tool_draft.take() else {
        return;
    };
    if matches!(state.selection, crate::state::selection::Selection::Tool(id) if id == tool_id) {
        // Still editing — keep the draft.
        state.history.tool_draft = Some((tool_id, draft));
        return;
    }
    // Navigated away: commit any pending edits against the live tool.
    commit_tool_draft(state, tool_id, draft);
}

/// Commit a tool draft to the session: no-op if it matches the committed
/// tool, else push an undo step, write it, invalidate dependent toolpaths,
/// and mark the project edited.
pub(crate) fn commit_tool_draft(
    state: &mut AppState,
    tool_id: crate::state::job::ToolId,
    draft: crate::state::job::ToolConfig,
) {
    let Some(committed) = state
        .session()
        .tools()
        .iter()
        .find(|t| t.id == tool_id)
        .cloned()
    else {
        return;
    };
    if draft == committed {
        return;
    }
    // F029: a draft outside its domain is not committed and takes no undo
    // step. `Command::ReplaceTool` refuses it too; the editor names why.
    if let Err(e) = draft.validate() {
        tracing::warn!("the tool draft was refused: {e}");
        return;
    }
    state
        .history
        .push(crate::state::history::UndoAction::ToolChange {
            tool_id,
            old: committed,
            new: draft.clone(),
        });
    // WP6: one door writes the tool and drops what it machines.
    //
    // G-FRESHSTATE: `Command::ReplaceTool` drops the core results of
    // every op this tool machines, and of every planned-tier ladder that
    // names it. The panel used to write `tools_mut()` in place and call
    // `invalidate_tool` after it, which is the same pair of steps behind
    // two doors — and a caller that forgot the second left the cards
    // green while export fell back to the GUI's copy of the OLD
    // geometry.
    use rs_cam_core::session::{Command, ReplaceToolArgs};
    let command = Command::ReplaceTool(ReplaceToolArgs {
        tool_id: tool_id.0,
        config: Box::new(draft),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
    }
}

/// The bounding box of the first model that carries geometry.
///
/// WP6. `AppController::first_model_bbox` answers the same question, and
/// a draw site cannot reach the controller. The two read the same two
/// sources in the same order.
fn first_model_bbox(state: &AppState) -> Option<rs_cam_core::geo::BoundingBox3> {
    state
        .session()
        .models()
        .iter()
        .find_map(rs_cam_core::session::LoadedModel::bbox)
}

/// Apply the stock panel's draft through the one command door.
///
/// G-FRESHSTATE: `Command::SetStockConfig` drops every toolpath result,
/// because heights reference the stock top, an inherited boundary
/// follows the stock outline, and the material sets the feeds. An edit of
/// the alignment pins alone takes `Command::SetAlignmentPins` instead
/// ([`stock_edit_command`]). The panel
/// used to write `stock_mut()` in place and ask for that invalidation
/// with an `AppEvent`; the event is gone and the command carries the
/// rule.
///
/// Two things the deleted `StockChanged` handler did are NOT core rules,
/// so they stay here:
///
/// - `auto_from_model` re-sizes the stock around the first model. The
///   handler did it before invalidating; this does it before the write,
///   so one command carries the finished record. R12: the refit runs
///   ONLY when no dimension or origin row moved; a moved row clears the
///   flag instead. `stock::reconcile_auto_from_model` holds that rule
///   and mirrors the MCP `set_stock_config` door.
/// - the viewport buffers and the pin-drill operation both read what
///   moved. The panel raises both flags; the frame loop discharges them.
pub(crate) fn apply_stock_draft(state: &mut AppState, draft: crate::state::job::StockConfig) {
    let draft = super::stock::reconcile_auto_from_model(
        state.session().stock_config(),
        draft,
        first_model_bbox(state).as_ref(),
    );
    // A finished edit is not the same as a CHANGED one. A `DragValue`
    // reports `lost_focus` when the operator clicks into the field and
    // out of it again, and this row drops every toolpath result — so an
    // ungated apply would drop the project on a click that typed
    // nothing. The event this replaced was gated on `changed()`.
    // A draft that moves the pins AND the part-model choice in one frame
    // (a focused pattern field loses focus to a part-model click) takes
    // two narrow rows, not one `SetStockConfig` that drops every result.
    if let Some(first) = part_models_first(state.session().stock_config(), &draft)
        && apply_panel_command(state, first)
    {
        state.gui.mark_edited();
    }
    let Some(command) = stock_edit_command(state.session().stock_config(), draft) else {
        return;
    };
    // The door stamps the stale set, raises the simulation flag and
    // logs a refusal (F096, F056).
    if state.apply(command).is_ok() {
        state.gui.mark_edited();
        state.panel_side_effects.upload = true;
        state.panel_side_effects.pin_drill_sync = true;
    }
}

/// The one command that writes a stock edit, or `None` when `draft`
/// equals `stored`.
///
/// P4 (operator ruling 2026-10-12): an edit that moves ONLY the alignment
/// pins is `Command::SetAlignmentPins`, which stales the pin-drill
/// operations and what depends on them. P7: an edit of ONLY the part-model
/// choice is `Command::SetPinPartModels`, which stales nothing. Any other
/// stock edit is `Command::SetStockConfig`, which drops every result. The
/// stock panel and both undo arms call this function, so an undo of a pin
/// edit stales the same set as the edit.
pub(crate) fn stock_edit_command(
    stored: &crate::state::job::StockConfig,
    draft: crate::state::job::StockConfig,
) -> Option<rs_cam_core::session::Command> {
    use rs_cam_core::session::{
        Command, SetAlignmentPinsArgs, SetPinPartModelsArgs, SetStockConfigArgs,
    };
    if draft == *stored {
        return None;
    }
    let pins_only = crate::state::job::StockConfig {
        alignment_pins: stored.alignment_pins.clone(),
        ..draft.clone()
    } == *stored;
    let part_models_only = crate::state::job::StockConfig {
        pin_part_models: stored.pin_part_models.clone(),
        ..draft.clone()
    } == *stored;
    Some(if pins_only {
        Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: draft.alignment_pins,
        })
    } else if part_models_only {
        Command::SetPinPartModels(SetPinPartModelsArgs {
            models: draft.pin_part_models,
        })
    } else {
        Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(draft),
        })
    })
}

/// The `SetPinPartModels` row to apply BEFORE [`stock_edit_command`], when
/// `draft` differs from `stored` in the part-model choice AND the pins and
/// in nothing else. After it, the rest of the draft is a pin-only edit,
/// which [`stock_edit_command`] gives as `SetAlignmentPins`. `None` in
/// every other case. The stock panel and the undo arm both call it.
pub(crate) fn part_models_first(
    stored: &crate::state::job::StockConfig,
    draft: &crate::state::job::StockConfig,
) -> Option<rs_cam_core::session::Command> {
    use rs_cam_core::session::{Command, SetPinPartModelsArgs};
    let both = draft.pin_part_models != stored.pin_part_models
        && draft.alignment_pins != stored.alignment_pins;
    let only_those = crate::state::job::StockConfig {
        alignment_pins: stored.alignment_pins.clone(),
        pin_part_models: stored.pin_part_models.clone(),
        ..draft.clone()
    } == *stored;
    (both && only_those).then(|| {
        Command::SetPinPartModels(SetPinPartModelsArgs {
            models: draft.pin_part_models.clone(),
        })
    })
}

/// Run one command from a draw site and stamp what it dropped.
///
/// The shape follows `write_entry_config_to_session` (WP5): apply, stamp
/// every index `Effects::stale` names, and report a refusal to the log
/// rather than to a panel that has already drawn. It answers whether the
/// command was applied, so a caller can dirty the project once for a
/// group of them.
///
/// WP19: it mirrors the OTHER half of the answer too. A row that drops
/// the session's simulation reports `Effects::simulation_cleared`, and
/// the viewport must not go on drawing a simulation the session no
/// longer holds (WP11b, N12 item 10). A draw site cannot clear the
/// view itself. F096: `AppState::apply` raises
/// `PanelSideEffects::invalidate_simulation`, and the frame loop
/// discharges it.
pub(super) fn apply_panel_command(
    state: &mut AppState,
    command: rs_cam_core::session::Command,
) -> bool {
    // `AppState::apply` stamps, raises the flag and logs a refusal (F096).
    state.apply(command).is_ok()
}

/// Apply the setup panel's draft, one command per field group that moved.
///
/// Four groups, four rows. The name is NOT one of them: the panel pushes
/// `AppEvent::RenameSetup`, which the controller already routes through
/// the core setter.
///
/// G-FRESHSTATE: `SetSetupFace`, `SetSetupRotation` and `SetSetupModels`
/// each drop every result in the setup — the frame and the model scope
/// decide what a generation in this setup reads. `SetSetupDatum` drops
/// nothing by design: the datum reaches the export alone.
pub(super) fn apply_setup_draft(
    state: &mut AppState,
    setup_index: usize,
    stored: &rs_cam_core::session::SetupData,
    draft: &rs_cam_core::session::SetupData,
) {
    use rs_cam_core::session::{
        Command, SetSetupDatumArgs, SetSetupFaceArgs, SetSetupModelsArgs, SetSetupRotationArgs,
    };
    let mut commands: Vec<Command> = Vec::new();
    if draft.face_up != stored.face_up {
        commands.push(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index,
            face_up: draft.face_up,
        }));
    }
    if draft.z_rotation != stored.z_rotation {
        commands.push(Command::SetSetupRotation(SetSetupRotationArgs {
            setup_index,
            z_rotation: draft.z_rotation,
        }));
    }
    if draft.datum != stored.datum {
        commands.push(Command::SetSetupDatum(SetSetupDatumArgs {
            setup_index,
            datum: draft.datum.clone(),
        }));
    }
    if draft.model_ids != stored.model_ids {
        commands.push(Command::SetSetupModels(SetSetupModelsArgs {
            setup_index,
            model_ids: draft.model_ids.clone(),
        }));
    }
    let mut applied = false;
    for command in commands {
        applied |= apply_panel_command(state, command);
    }
    if applied {
        state.gui.mark_edited();
        state.panel_side_effects.upload = true;
    }
}

/// Apply the obstacle editor's draft through `Command::ReplaceObstacle`
/// (K1, K2).
///
/// `stored` is the record the setup holds. A draft equal to it applies
/// nothing. The draft keeps the STORED `enabled` flag: the list-row
/// checkbox is the one control for that field, so an open editor cannot
/// undo a row toggle.
///
/// G-FRESHSTATE: an edit that moves the obstacle drops every result in the
/// setup and its dependents, because every operation in the setup keeps
/// out of it. A rename drops nothing.
///
/// # Errors
/// The core refusal text. The draw site writes it into the editor's
/// banner (S4 refusal contract) and keeps the draft.
pub(crate) fn apply_obstacle_draft(
    state: &mut AppState,
    setup_index: usize,
    stored: &rs_cam_core::session::Obstacle,
    mut obstacle: rs_cam_core::session::Obstacle,
) -> Result<(), String> {
    use rs_cam_core::session::{Command, ReplaceObstacleArgs};
    obstacle.enabled = stored.enabled;
    if obstacle == *stored {
        return Ok(());
    }
    let Some(setup_id) = state
        .session()
        .list_setups()
        .get(setup_index)
        .map(|s| crate::state::job::SetupId(s.id))
    else {
        return Err(format!("no setup at index {setup_index}"));
    };
    let command = Command::ReplaceObstacle(ReplaceObstacleArgs {
        setup_index,
        obstacle_id: stored.id,
        obstacle: Box::new(obstacle.clone()),
    });
    match state.apply(command) {
        Ok(_) => {
            state
                .history
                .push(crate::state::history::UndoAction::ObstacleChange {
                    setup_id,
                    obstacle_id: stored.id,
                    old: Some(stored.clone()),
                    new: Some(obstacle),
                });
            state.gui.mark_edited();
            state.panel_side_effects.upload = true;
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Apply a machine head through `Command::SetMachineHead` (H2): a preset
/// or the Head section's draft.
///
/// A head equal to the stored head applies nothing. The core refuses a
/// head that breaks a rule of `MachineHead::validate`. A change of the
/// fixed parts drops every result; a shoe-only change drops the
/// simulation. The view mirrors both through the `Effects` (WP19).
///
/// # Errors
/// The core refusal text. The draw site writes it into the section's
/// banner (S4 refusal contract) and keeps the draft.
pub(crate) fn apply_head(
    state: &mut AppState,
    head: rs_cam_core::machine::head::MachineHead,
) -> Result<(), String> {
    use rs_cam_core::session::{Command, SetMachineHeadArgs};
    let stored = state.session().machine().head.clone();
    if head == stored {
        return Ok(());
    }
    let command = Command::SetMachineHead(SetMachineHeadArgs {
        head: Box::new(head.clone()),
    });
    match state.apply(command) {
        Ok(_) => {
            state
                .history
                .push(crate::state::history::UndoAction::HeadChange {
                    old: Box::new(stored),
                    new: Box::new(head),
                });
            state.gui.mark_edited();
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Whether the machine panel's own three dials moved.
///
/// `MachineProfile` carries no `PartialEq`, and only three of its fields
/// are writable here: the travel rate, the shank limit and the safety
/// factor. The kinematics block is compared separately, against its own
/// row.
///
/// The comparison exists because a finished edit is not the same as a
/// changed one: a `Slider` reports `drag_stopped` when the operator
/// clicks the handle and lets go, and `SetMachine` clears the cached
/// simulation.
pub(super) fn machine_panel_fields_moved(
    draft: &rs_cam_core::machine::MachineProfile,
    stored: &rs_cam_core::machine::MachineProfile,
) -> bool {
    draft.max_feed_mm_min != stored.max_feed_mm_min
        || draft.max_shank_mm != stored.max_shank_mm
        || draft.aggressiveness != stored.aggressiveness
}

/// Install a whole machine profile through `Command::SetMachine`.
///
/// The row clears the cached simulation: timing and feed modulation read
/// the machine, and geometry does not. A profile that arrives from the
/// library is a snapshot of a named machine; L6 deleted the reference
/// field, so there is no link left to keep or clear.
pub(super) fn apply_machine(state: &mut AppState, machine: rs_cam_core::machine::MachineProfile) {
    use rs_cam_core::session::{Command, SetMachineArgs};
    let command = Command::SetMachine(SetMachineArgs {
        machine: Box::new(machine),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
    }
}

/// Write the machine's kinematics block through
/// `Command::SetMachineKinematics`.
///
/// `None` means the grid produced no block, which happens only before
/// the operator touches a value. The row drops the library link: the
/// numbers are inline now and no library entry published them.
pub(super) fn apply_machine_kinematics(
    state: &mut AppState,
    kinematics: Option<rs_cam_core::machine::kinematics::MachineKinematics>,
) {
    let Some(kinematics) = kinematics else {
        return;
    };
    use rs_cam_core::session::{Command, SetMachineKinematicsArgs};
    let command = Command::SetMachineKinematics(SetMachineKinematicsArgs {
        kinematics: Box::new(kinematics),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
    }
}

/// Write what a GRBL `$$` dump states, through
/// `Command::ImportMachineSettings`.
///
/// B6: the row writes only the stated fields; every other machine field
/// keeps its value.
///
/// G10: a dump that names grblHAL also moves the post dialect, so the
/// view's post mirror is rebuilt from the session here.
pub(super) fn apply_machine_import(
    state: &mut AppState,
    import: rs_cam_core::machine::kinematics::GrblImport,
) {
    use rs_cam_core::session::{Command, ImportMachineSettingsArgs};
    let command = Command::ImportMachineSettings(ImportMachineSettingsArgs {
        import: Box::new(import),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
    }
}

/// Flush post undo snapshot if the user navigated away from post.
pub(super) fn flush_post_snapshot(state: &mut AppState) {
    if let Some(old) = state.history.post_snapshot.take() {
        if !matches!(
            state.selection,
            crate::state::selection::Selection::PostProcessor
        ) {
            state
                .history
                .push(crate::state::history::UndoAction::PostChange {
                    old,
                    new: state.session().post_config().clone(),
                });
            state.gui.mark_edited();
        } else {
            state.history.post_snapshot = Some(old);
        }
    }
}

/// Flush machine undo snapshot if the user navigated away from machine.
pub(super) fn flush_machine_snapshot(state: &mut AppState) {
    if let Some(old) = state.history.machine_snapshot.take() {
        if !matches!(state.selection, crate::state::selection::Selection::Machine) {
            state
                .history
                .push(crate::state::history::UndoAction::MachineChange {
                    old,
                    new: state.session().machine().clone(),
                });
            state.gui.mark_edited();
        } else {
            state.history.machine_snapshot = Some(old);
        }
    }
}

/// Flush toolpath params undo snapshot if the user navigated away from a toolpath.
pub(super) fn flush_toolpath_snapshot(state: &mut AppState) {
    if let Some((tp_id, old_op, old_dressups, old_boundary, old_provenance)) =
        state.history.toolpath_snapshot.take()
    {
        if !matches!(state.selection, crate::state::selection::Selection::Toolpath(id) if id == tp_id)
        {
            if let Some((_, tc)) = state.session().find_toolpath_config_by_id(tp_id) {
                state
                    .history
                    .push(crate::state::history::UndoAction::ToolpathParamChange {
                        tp_id,
                        old_op,
                        new_op: tc.operation.clone(),
                        old_dressups,
                        new_dressups: tc.dressups.clone(),
                        old_boundary,
                        new_boundary: tc.boundary.clone(),
                        old_feeds_provenance: old_provenance,
                        new_feeds_provenance: tc.feeds_provenance.clone(),
                    });
                state.gui.mark_edited();
            }
        } else {
            state.history.toolpath_snapshot =
                Some((tp_id, old_op, old_dressups, old_boundary, old_provenance));
        }
    }
}
