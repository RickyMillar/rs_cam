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
        .session
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
    state.session.models().iter().find_map(|model| {
        model.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
            model
                .polygons
                .as_deref()
                .and_then(|polys| rs_cam_core::session::polygons_bbox(polys))
        })
    })
}

/// Apply the stock panel's draft through the one command door.
///
/// G-FRESHSTATE: `Command::SetStockConfig` drops every toolpath result,
/// because heights reference the stock top, an inherited boundary
/// follows the stock outline, and the material sets the feeds. The panel
/// used to write `stock_mut()` in place and ask for that invalidation
/// with an `AppEvent`; the event is gone and the command carries the
/// rule.
///
/// Two things the deleted `StockChanged` handler did are NOT core rules,
/// so they stay here:
///
/// - `auto_from_model` re-sizes the stock around the first model. The
///   handler did it before invalidating; this does it before the write,
///   so one command carries the finished record.
/// - the viewport buffers and the pin-drill operation both read what
///   moved. The panel raises both flags; the frame loop discharges them.
pub(crate) fn apply_stock_draft(state: &mut AppState, mut draft: crate::state::job::StockConfig) {
    use rs_cam_core::session::{Command, SetStockConfigArgs};
    if draft.auto_from_model
        && let Some(bbox) = first_model_bbox(state)
    {
        draft.update_from_bbox(&bbox);
    }
    // A finished edit is not the same as a CHANGED one. A `DragValue`
    // reports `lost_focus` when the operator clicks into the field and
    // out of it again, and this row drops every toolpath result — so an
    // ungated apply would drop the project on a click that typed
    // nothing. The event this replaced was gated on `changed()`.
    if draft == *state.session.stock_config() {
        return;
    }
    let command = Command::SetStockConfig(SetStockConfigArgs {
        stock: Box::new(draft),
    });
    match state.session.apply(command) {
        Ok(effects) => {
            crate::state::stale::stamp_stale(state, &effects.stale);
            state.gui.mark_edited();
            state.panel_side_effects.upload = true;
            state.panel_side_effects.pin_drill_sync = true;
            if effects.simulation_cleared {
                state.panel_side_effects.invalidate_simulation = true;
            }
        }
        Err(error) => {
            tracing::warn!("the stock edit was refused: {error}");
        }
    }
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
/// view itself, so it raises `PanelSideEffects::invalidate_simulation`
/// and the frame loop discharges it.
pub(super) fn apply_panel_command(
    state: &mut AppState,
    command: rs_cam_core::session::Command,
) -> bool {
    match state.session.apply(command) {
        Ok(effects) => {
            crate::state::stale::stamp_stale(state, &effects.stale);
            if effects.simulation_cleared {
                state.panel_side_effects.invalidate_simulation = true;
            }
            true
        }
        Err(error) => {
            tracing::warn!("the properties panel edit was refused: {error}");
            false
        }
    }
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

/// Apply the fixture panel's draft through `Command::ReplaceFixture`.
///
/// G-FRESHSTATE: the row drops every result in the setup, because a
/// fixture is a collision input of every operation in it. The panel
/// wrote the fixture in place and dropped nothing, so a clamp could move
/// under a cached holder-clearance verdict.
pub(crate) fn apply_fixture_draft(
    state: &mut AppState,
    setup_index: usize,
    fixture_id: crate::state::job::FixtureId,
    fixture: rs_cam_core::session::Fixture,
) {
    use rs_cam_core::session::{Command, ReplaceFixtureArgs};
    let command = Command::ReplaceFixture(ReplaceFixtureArgs {
        setup_index,
        fixture_id,
        fixture: Box::new(fixture),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
        state.panel_side_effects.upload = true;
    }
}

/// Apply the keep-out panel's draft through `Command::ReplaceKeepOut`.
///
/// The twin of [`apply_fixture_draft`], and it drops the same set.
pub(super) fn apply_keep_out_draft(
    state: &mut AppState,
    setup_index: usize,
    zone_id: crate::state::job::KeepOutId,
    zone: rs_cam_core::session::KeepOutZone,
) {
    use rs_cam_core::session::{Command, ReplaceKeepOutArgs};
    let command = Command::ReplaceKeepOut(ReplaceKeepOutArgs {
        setup_index,
        zone_id,
        zone: Box::new(zone),
    });
    if apply_panel_command(state, command) {
        state.gui.mark_edited();
        state.panel_side_effects.upload = true;
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
        || draft.safety_factor != stored.safety_factor
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

/// Write what a GRBL `$$` dump carries, through
/// `Command::ImportMachineSettings`.
///
/// A dump carries one field more than the kinematics block — the travel
/// rate — so the two rows carry two payloads (plan section 15 ruling 6).
/// `None` means the dump published no travel rate, and the machine keeps
/// the one it has.
pub(super) fn apply_machine_import(
    state: &mut AppState,
    kinematics: rs_cam_core::machine::kinematics::MachineKinematics,
    max_feed_mm_min: Option<f64>,
) {
    use rs_cam_core::session::{Command, ImportMachineSettingsArgs};
    let command = Command::ImportMachineSettings(ImportMachineSettingsArgs {
        kinematics: Box::new(kinematics),
        max_feed_mm_min,
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
                    new: state.gui.post.clone(),
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
                    new: state.session.machine().clone(),
                });
            state.gui.mark_edited();
        } else {
            state.history.machine_snapshot = Some(old);
        }
    }
}

/// Flush toolpath params undo snapshot if the user navigated away from a toolpath.
pub(super) fn flush_toolpath_snapshot(state: &mut AppState) {
    if let Some((tp_id, old_op, old_dressups, old_faces, old_provenance)) =
        state.history.toolpath_snapshot.take()
    {
        if !matches!(state.selection, crate::state::selection::Selection::Toolpath(id) if id == tp_id)
        {
            if let Some((_, tc)) = state.session.find_toolpath_config_by_id(tp_id) {
                state
                    .history
                    .push(crate::state::history::UndoAction::ToolpathParamChange {
                        tp_id,
                        old_op,
                        new_op: tc.operation.clone(),
                        old_dressups,
                        new_dressups: tc.dressups.clone(),
                        old_face_selection: old_faces,
                        new_face_selection: tc.face_selection.clone(),
                        old_feeds_provenance: old_provenance,
                        new_feeds_provenance: tc.feeds_provenance.clone(),
                    });
                state.gui.mark_edited();
            }
        } else {
            state.history.toolpath_snapshot =
                Some((tp_id, old_op, old_dressups, old_faces, old_provenance));
        }
    }
}
