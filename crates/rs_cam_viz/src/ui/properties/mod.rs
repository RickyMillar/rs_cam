mod operations;
pub mod post;
pub mod setup;
pub mod stock;
pub mod tool;

use operations::{
    StepoverPattern, draw_adaptive_params, draw_adaptive3d_params, draw_alignment_pin_drill_params,
    draw_chamfer_params, draw_dogbone_diagram, draw_drill_params, draw_dropcutter_params,
    draw_face_params, draw_height_diagram, draw_heights_params, draw_horizontal_finish_params,
    draw_inlay_diagram, draw_inlay_params, draw_lead_in_out_diagram, draw_outline_diagram,
    draw_pencil_diagram, draw_pencil_params, draw_pocket_params, draw_point_set_diagram,
    draw_profile_params, draw_project_curve_params, draw_radial_diagram, draw_radial_finish_params,
    draw_ramp_finish_diagram, draw_ramp_finish_params, draw_rest_params, draw_scallop_params,
    draw_spiral_diagram, draw_spiral_finish_params, draw_steep_shallow_diagram,
    draw_steep_shallow_params, draw_stepover_diagram, draw_trace_params, draw_vcarve_params,
    draw_waterline_params, draw_zigzag_params,
};
pub use operations::{ToolpathValidationContext, validate_toolpath, validate_toolpath_config};

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig, DressupEntryStyle,
    HeightContext, HeightsConfig, OperationConfig, ProfileSide, RetractStrategy, SpiralDirection,
    StockSource, ToolpathEntry, TraceCompensation, UiProcessRole,
};
use crate::ui::AppEvent;
use crate::ui::automation;
use crate::ui::components::{
    PrecedenceField, ProvKind, ProvenanceBadge, Suggestion, UiExt, ValueRow, mrr_row, power_bar,
};

/// Paint a brief blue glow behind a UI region when an MCP parameter was recently changed.
/// Call this right after allocating the widget/row so the highlight paints behind it.
/// The glow fades out over 2 seconds.
#[cfg(feature = "mcp")]
pub fn mcp_highlight_effect(ui: &mut egui::Ui, gui: &crate::state::runtime::GuiState, key: &str) {
    if let Some(when) = gui.mcp_highlights.get(key) {
        let elapsed = when.elapsed().as_secs_f32();
        let duration = 2.0; // 2-second fade
        if elapsed < duration {
            // SAFETY: arithmetic is bounded: (1.0 - 0..1) * 80.0 = 0..80, fits u8.
            #[allow(clippy::indexing_slicing)]
            let alpha = ((1.0 - elapsed / duration) * 80.0) as u8;
            let rect = ui.max_rect();
            ui.painter().rect_filled(
                rect,
                4.0,
                egui::Color32::from_rgba_unmultiplied(100, 180, 255, alpha),
            );
        }
    }
}

/// TOO-003 — flush the pending tool draft if the user navigated away from
/// the tool. Edits are pending-until-committed in the panel, but navigating
/// away **auto-commits** them (the spec's accepted fallback) so a stray
/// click elsewhere can't silently drop edits. If still editing the same
/// tool, the draft is left in place.
fn flush_tool_draft(state: &mut AppState) {
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
fn commit_tool_draft(
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
    if let Some(t) = state
        .session
        .tools_mut()
        .iter_mut()
        .find(|t| t.id == tool_id)
    {
        *t = draft;
    }
    state.session.invalidate_tool(tool_id.0);
    state.gui.mark_edited();
}

/// Flush post undo snapshot if the user navigated away from post.
fn flush_post_snapshot(state: &mut AppState) {
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
fn flush_machine_snapshot(state: &mut AppState) {
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
fn flush_toolpath_snapshot(state: &mut AppState) {
    if let Some((tp_id, old_op, old_dressups, old_faces)) = state.history.toolpath_snapshot.take() {
        if !matches!(state.selection, crate::state::selection::Selection::Toolpath(id) if id == tp_id)
        {
            if let Some((_, tc)) = state.session.find_toolpath_config_by_id(tp_id.0) {
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
                    });
                state.gui.mark_edited();
            }
        } else {
            state.history.toolpath_snapshot = Some((tp_id, old_op, old_dressups, old_faces));
        }
    }
}

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    // Show simulation panel only when in the Simulation workspace
    if state.workspace == crate::state::Workspace::Simulation && state.simulation.has_results() {
        draw_simulation_panel(ui, state, events);
        return;
    }

    // Flush pending undo snapshots when selection changes away from a tracked panel.
    flush_tool_draft(state);
    flush_post_snapshot(state);
    flush_machine_snapshot(state);
    flush_toolpath_snapshot(state);

    match state.selection.clone() {
        Selection::None => {
            if state.session.models().is_empty()
                && state
                    .session
                    .list_setups()
                    .iter()
                    .all(|s| s.toolpath_indices.is_empty())
            {
                ui.label(
                    egui::RichText::new("Getting started:")
                        .strong()
                        .color(egui::Color32::from_rgb(180, 180, 195)),
                );
                ui.add_space(4.0);
                ui.label("1. Import a model (File > Import)");
                ui.label("2. Configure stock dimensions");
                ui.label("3. Add a cutting tool");
                ui.label("4. Create a toolpath");
                ui.label("5. Generate and export G-code");
            } else {
                ui.label(
                    egui::RichText::new("Select an item in the project tree")
                        .italics()
                        .color(egui::Color32::from_rgb(120, 120, 130)),
                );
            }
        }
        Selection::Stock => {
            // Capture snapshot for undo before editing
            if state.history.stock_snapshot.is_none() {
                state.history.stock_snapshot = Some(state.session.stock_config().clone());
            }
            let has_flipped_setup = state
                .session
                .list_setups()
                .iter()
                .any(|s| s.face_up != crate::state::job::FaceUp::Top);
            stock::draw(ui, state.session.stock_mut(), has_flipped_setup, events);
            // If an edit just finished (DragValue released), push undo
            if events
                .iter()
                .any(|e| matches!(e, AppEvent::StockChanged | AppEvent::StockMaterialChanged))
                && let Some(old) = state.history.stock_snapshot.take()
                && old != *state.session.stock_config()
            {
                state
                    .history
                    .push(crate::state::history::UndoAction::StockChange {
                        old,
                        new: state.session.stock_config().clone(),
                    });
            }
        }
        Selection::PostProcessor => {
            // Capture snapshot for undo before editing
            if state.history.post_snapshot.is_none() {
                state.history.post_snapshot = Some(state.gui.post.clone());
            }
            let stock_top = state.session.stock_config().origin_z + state.session.stock_config().z;
            post::draw(ui, &mut state.gui.post, stock_top);
            // W2.2 [P1-004]: the session is canonical for post config. Push the
            // edited gui.post straight through so `session.post_config()` can't
            // lag behind the panel (the stale window the GUI-vs-MCP race read).
            // Guarded on a real change because set_post_config invalidates the
            // simulation cache — we must not wipe it every idle frame.
            let session_post = crate::state::runtime::GuiState::post_to_session(&state.gui.post);
            if *state.session.post_config() != session_post {
                state.session.set_post_config(session_post);
            }
        }
        Selection::Machine => {
            // Capture snapshot for undo before editing
            if state.history.machine_snapshot.is_none() {
                state.history.machine_snapshot = Some(state.session.machine().clone());
            }
            draw_machine_panel(ui, state, events);
        }
        Selection::Model(id) => {
            draw_model_properties(ui, id, state, events);
        }
        Selection::Tool(id) => {
            // TOO-003 — draft-commit: edit a clone, commit on Apply (or on
            // navigate-away via flush_tool_draft), discard on Revert.
            if let Some(committed) = state.session.tools().iter().find(|t| t.id == id).cloned() {
                // (Re)initialise the draft when entering a different tool.
                if state.history.tool_draft.as_ref().map(|(d, _)| *d) != Some(id) {
                    state.history.tool_draft = Some((id, committed.clone()));
                }
                let modified = state
                    .history
                    .tool_draft
                    .as_ref()
                    .is_some_and(|(_, draft)| *draft != committed);
                let mut action = tool::ToolEditAction::None;
                if let Some((_, draft)) = state.history.tool_draft.as_mut() {
                    action = tool::draw(ui, draft, modified);
                }
                match action {
                    tool::ToolEditAction::Apply => {
                        if let Some((_, draft)) = state.history.tool_draft.clone() {
                            commit_tool_draft(state, id, draft);
                        }
                    }
                    tool::ToolEditAction::Revert => {
                        state.history.tool_draft = Some((id, committed));
                    }
                    tool::ToolEditAction::None => {}
                }
            } else {
                // Tool vanished (e.g. deleted while selected).
                state.history.tool_draft = None;
            }
        }
        Selection::Setup(setup_id) => {
            let pin_count = state.session.stock_config().alignment_pins.len();
            let has_flip_axis = state.session.stock_config().flip_axis.is_some();
            let all_models: Vec<_> = state
                .session
                .models()
                .iter()
                .map(|m| (crate::state::job::ModelId(m.id), m.name.clone()))
                .collect();
            if let Some((_, setup_data)) = state.session.find_setup_by_id_mut(setup_id.0) {
                let setup_rt = state.gui.setup_rt_or_default(setup_id.0);
                setup::draw(
                    ui,
                    setup_id,
                    setup_data,
                    setup_rt,
                    pin_count,
                    has_flip_axis,
                    &all_models,
                    events,
                );
            }
        }
        Selection::Fixture(setup_id, fixture_id) => {
            if let Some((_, setup_data)) = state.session.find_setup_by_id_mut(setup_id.0)
                && let Some(fixture) = setup_data
                    .fixtures
                    .iter_mut()
                    .find(|fixture| fixture.id == fixture_id)
            {
                setup::draw_fixture_properties(ui, setup_id, fixture, events);
            }
        }
        Selection::KeepOut(setup_id, keep_out_id) => {
            if let Some((_, setup_data)) = state.session.find_setup_by_id_mut(setup_id.0)
                && let Some(zone) = setup_data
                    .keep_out_zones
                    .iter_mut()
                    .find(|zone| zone.id == keep_out_id)
            {
                setup::draw_keep_out_properties(ui, setup_id, zone, events);
            }
        }
        Selection::Face(model_id, face_id) => {
            ui.heading("Face Selected");
            ui.separator();
            let model_name = state
                .session
                .models()
                .iter()
                .find(|m| m.id == model_id.0)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("Face: {}", face_id.0));
            if let Some(model) = state.session.models().iter().find(|m| m.id == model_id.0)
                && let Some(enriched) = &model.enriched_mesh
                && let Some(group) = enriched.face_group(face_id)
            {
                ui.label(format!("Surface type: {:?}", group.surface_type));
                ui.label(format!("Triangles: {}", group.triangle_range.len()));
            }
        }
        Selection::Faces(model_id, ref face_ids) => {
            ui.heading("Faces Selected");
            ui.separator();
            let model_name = state
                .session
                .models()
                .iter()
                .find(|m| m.id == model_id.0)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("{} faces selected", face_ids.len()));
        }
        Selection::Toolpath(id) => {
            // Capture snapshot for undo before editing
            if state.history.toolpath_snapshot.is_none()
                && let Some((_, tc)) = state.session.find_toolpath_config_by_id(id.0)
            {
                state.history.toolpath_snapshot = Some((
                    id,
                    tc.operation.clone(),
                    tc.dressups.clone(),
                    tc.face_selection.clone(),
                ));
            }
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state
                .session
                .tools()
                .iter()
                .map(|t| (t.id, t.summary(), t.diameter))
                .collect();
            // Filter models by setup's model_ids (empty = all).
            // For now use all models — setup model scoping will be
            // wired via SetupRuntime in a later pass.
            let models: Vec<_> = state
                .session
                .models()
                .iter()
                .map(|m| (crate::state::job::ModelId(m.id), m.name.clone()))
                .collect();
            // Snapshot tool configs for feeds calculation
            let tool_configs: Vec<_> = state
                .session
                .tools()
                .iter()
                .map(|t| (t.id, t.clone()))
                .collect();
            let validation = ToolpathValidationContext::from_session(&state.session);
            let material = state.session.stock_config().material.clone();
            let machine = state.session.machine().clone();
            let workholding = state.session.stock_config().workholding_rigidity;

            // Check if the toolpath's model has enriched mesh (for face selection UI)
            let model_for_panel = state
                .session
                .find_toolpath_config_by_id(id.0)
                .and_then(|(_, tc)| state.session.models().iter().find(|m| m.id == tc.model_id));
            let model_has_enriched = model_for_panel
                .map(|m| m.enriched_mesh.is_some())
                .unwrap_or(false);
            // Defensive: if a STEP model loaded without BREP (e.g. older
            // project file or future loader regression), surface a warning
            // so the face picker isn't silently absent.
            let model_is_step_missing_brep = model_for_panel
                .map(|m| {
                    m.kind == Some(rs_cam_core::compute::stock_config::ModelKind::Step)
                        && m.enriched_mesh.is_none()
                })
                .unwrap_or(false);

            // Snapshot height context before mutable borrow. Use the shared
            // helper so model_top/bottom_z are in the setup-local frame.
            let height_ctx = state
                .session
                .find_toolpath_config_by_id(id.0)
                .map(|(_, tc)| crate::state::job::height_context_from_session(&state.session, tc));

            // Snapshot operation and heights for stale_since detection
            let op_before = state
                .session
                .find_toolpath_config_by_id(id.0)
                .map(|(_, tc)| serde_json::to_string(&tc.operation).unwrap_or_default());
            let heights_before = state
                .session
                .find_toolpath_config_by_id(id.0)
                .map(|(_, tc)| format!("{:?}", tc.heights));

            // Pre-compute stale-default defects for this TP so the panel
            // can render the validator banner without needing a session
            // reference. Defects are recomputed each frame, so a Fix
            // click takes effect immediately on the next render.
            let stale_default_defects = state
                .session
                .find_toolpath_config_by_id(id.0)
                .map(|(_, tc)| {
                    let tool =
                        state.session.tools().iter().find(|t| {
                            t.id == rs_cam_core::compute::tool_config::ToolId(tc.tool_id)
                        });
                    let stock_bottom_z = state.session.stock_config().origin_z;
                    rs_cam_core::compute::validate::validate_one_toolpath(
                        tc,
                        tool,
                        &state.session.stock_config().material,
                        stock_bottom_z,
                    )
                })
                .unwrap_or_default();

            // Compute the load verdict for this TP so the params panel can
            // surface chipload / power / deflection / drill-gate
            // diagnostics in the unified ribbon rather than only in the
            // separate tool-load surface.
            let load_report = {
                let sim_trace = state
                    .simulation
                    .results
                    .as_ref()
                    .and_then(|r| r.cut_trace.as_deref());
                rs_cam_core::gcode::project_load_report(&state.session, sim_trace)
            };
            let load_verdict_for_tp = load_report
                .per_toolpath
                .iter()
                .find(|v| v.toolpath_id == id.0)
                .cloned();

            // Build a temporary ToolpathEntry from session config + gui runtime
            // so the existing draw_toolpath_panel can work unchanged.
            if let Some(mut entry) =
                build_entry_from_session_and_gui(id, &state.session, &state.gui)
            {
                draw_toolpath_panel(
                    ui,
                    &mut entry,
                    &tools,
                    &models,
                    &tool_configs,
                    &validation,
                    &material,
                    &machine,
                    workholding,
                    state.session.post_config().spindle_strategy,
                    state.session.post_config().spindle_speed,
                    model_has_enriched,
                    model_is_step_missing_brep,
                    height_ctx.as_ref(),
                    &stale_default_defects,
                    load_verdict_for_tp.as_ref(),
                    events,
                );

                // Write config changes back to session
                write_entry_config_to_session(&entry, &mut state.session);
                // Write runtime changes back to gui
                write_entry_runtime_to_gui(&entry, &mut state.gui);
            }

            // B3a: set stale_since when parameters or heights change
            if let Some((_, tc)) = state.session.find_toolpath_config_by_id(id.0) {
                let op_changed = op_before.as_ref().is_some_and(|b| {
                    *b != serde_json::to_string(&tc.operation).unwrap_or_default()
                });
                let heights_changed = heights_before
                    .as_ref()
                    .is_some_and(|b| *b != format!("{:?}", tc.heights));
                if op_changed || heights_changed {
                    if let Some(rt) = state.gui.toolpath_rt.get_mut(&id.0) {
                        rt.stale_since = Some(std::time::Instant::now());
                    }
                    state.gui.mark_edited();
                }
                if heights_changed {
                    // Trigger GPU re-upload so height plane positions update
                    events.push(AppEvent::StockChanged);
                }
            }
        }
    }
}

fn draw_model_properties(
    ui: &mut egui::Ui,
    id: crate::state::job::ModelId,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
) {
    use crate::state::job::ModelUnits;

    let Some(model) = state.session.models().iter().find(|m| m.id == id.0) else {
        return;
    };

    ui.heading(&model.name);
    ui.separator();

    ui.label(format!("Type: {:?}", model.kind));
    ui.label(format!("Path: {}", model.path.display()));

    if let Some(mesh) = &model.mesh {
        let bb = &mesh.bbox;
        let dx = bb.max.x - bb.min.x;
        let dy = bb.max.y - bb.min.y;
        let dz = bb.max.z - bb.min.z;

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Mesh Info")
                .strong()
                .color(egui::Color32::from_rgb(180, 180, 195)),
        );
        egui::Grid::new("mesh_info")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("Vertices:");
                ui.label(format!("{}", mesh.vertices.len()));
                ui.end_row();
                ui.label("Triangles:");
                ui.label(format!("{}", mesh.triangles.len()));
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Dimensions (after scaling)")
                .strong()
                .color(egui::Color32::from_rgb(180, 180, 195)),
        );
        egui::Grid::new("mesh_dims")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("X:");
                ui.label(format!(
                    "{:.3} mm  ({:.3} to {:.3})",
                    dx, bb.min.x, bb.max.x
                ));
                ui.end_row();
                ui.label("Y:");
                ui.label(format!(
                    "{:.3} mm  ({:.3} to {:.3})",
                    dy, bb.min.y, bb.max.y
                ));
                ui.end_row();
                ui.label("Z:");
                ui.label(format!(
                    "{:.3} mm  ({:.3} to {:.3})",
                    dz, bb.min.z, bb.max.z
                ));
                ui.end_row();
            });

        // Size hint
        let max_dim = dx.max(dy).max(dz);
        let min_dim = dx.min(dy).min(dz);
        if max_dim < 1.0 {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Very small! Probably in meters - try scaling x1000")
                    .color(egui::Color32::from_rgb(220, 170, 60)),
            );
        } else if min_dim > 5000.0 {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Very large! Check units")
                    .color(egui::Color32::from_rgb(220, 170, 60)),
            );
        }

        // Normal flip warning (D1): check winding consistency
        if let Some(report) = &model.winding_report
            && *report > 1.0
        {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "\u{26A0} {:.1}% inconsistent normals detected (auto-fixed on load)",
                    report
                ))
                .color(egui::Color32::from_rgb(220, 190, 60)),
            );
        }

        // BREP face metadata (STEP only)
        if let Some(enriched) = &model.enriched_mesh {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("BREP Topology")
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 195)),
            );
            egui::Grid::new("brep_info")
                .num_columns(2)
                .spacing([8.0, 3.0])
                .show(ui, |ui| {
                    ui.label("Faces:");
                    ui.label(format!("{}", enriched.face_count()));
                    ui.end_row();
                    ui.label("Adjacency pairs:");
                    ui.label(format!("{}", enriched.adjacency.len()));
                    ui.end_row();

                    // Surface type histogram
                    use rs_cam_core::enriched_mesh::SurfaceType;
                    let mut planes = 0;
                    let mut cylinders = 0;
                    let mut other = 0;
                    for group in &enriched.face_groups {
                        match group.surface_type {
                            SurfaceType::Plane => planes += 1,
                            SurfaceType::Cylinder => cylinders += 1,
                            _ => other += 1,
                        }
                    }
                    ui.label("Surface types:");
                    let mut parts = Vec::new();
                    if planes > 0 {
                        parts.push(format!("{planes} plane"));
                    }
                    if cylinders > 0 {
                        parts.push(format!("{cylinders} cyl"));
                    }
                    if other > 0 {
                        parts.push(format!("{other} other"));
                    }
                    ui.label(parts.join(", "));
                    ui.end_row();
                });
        }

        // Units / scale selector (all formats including STEP)
        {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Units / Scale")
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 195)),
            );

            let current_units = model.units.unwrap_or(ModelUnits::Millimeters);
            let current_label = current_units.label();

            ui.horizontal(|ui| {
                ui.label("Import as:");
                egui::ComboBox::from_id_salt("model_units")
                    .selected_text(&current_label)
                    .show_ui(ui, |ui| {
                        for &(units, label) in ModelUnits::PRESETS {
                            if ui
                                .selectable_label(
                                    std::mem::discriminant(&units)
                                        == std::mem::discriminant(&current_units)
                                        && units.scale_factor() == current_units.scale_factor(),
                                    label,
                                )
                                .clicked()
                            {
                                events.push(AppEvent::RescaleModel(id, units));
                            }
                        }
                    });
            });

            // Custom scale
            let mut custom_scale = current_units.scale_factor();
            ui.horizontal(|ui| {
                ui.label("Custom scale:");
                ui.push_id(("custom_scale", id), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut custom_scale)
                                .speed(0.1)
                                .range(0.001..=100000.0),
                        )
                        .changed()
                    {
                        events.push(AppEvent::RescaleModel(id, ModelUnits::Custom(custom_scale)));
                    }
                });
            });
        }
    }

    if let Some(polys) = &model.polygons {
        ui.add_space(8.0);
        ui.label(format!("Polygons: {}", polys.len()));
        for (i, p) in polys.iter().enumerate().take(5) {
            ui.label(format!(
                "  #{}: {} pts, {} holes",
                i + 1,
                p.exterior.len(),
                p.holes.len()
            ));
        }
        if polys.len() > 5 {
            ui.label(format!("  ... and {} more", polys.len() - 5));
        }
    }
}

fn draw_simulation_panel(ui: &mut egui::Ui, state: &mut AppState, _events: &mut Vec<AppEvent>) {
    ui.heading("Simulation");
    ui.separator();

    // Toolpath checklist
    ui.label(
        egui::RichText::new("Included Toolpaths")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    // Snapshot boundary data to avoid borrow conflicts with playback mutation below.
    let boundary_snapshots: Vec<_> = state
        .simulation
        .boundaries()
        .iter()
        .map(|b| {
            (
                b.id,
                b.name.clone(),
                b.tool_name.clone(),
                b.start_move,
                b.end_move,
            )
        })
        .collect();
    let current_boundary_id = state.simulation.current_boundary().map(|b| b.id);

    for (i, (id, name, tool_name, start_move, end_move)) in boundary_snapshots.iter().enumerate() {
        let pc = crate::render::toolpath_render::palette_color(i);
        let color = egui::Color32::from_rgb(
            (pc[0] * 255.0) as u8,
            (pc[1] * 255.0) as u8,
            (pc[2] * 255.0) as u8,
        );
        let is_current = current_boundary_id == Some(*id);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("\u{25CF}").color(color));
            let text = if is_current {
                egui::RichText::new(name)
                    .strong()
                    .color(egui::Color32::WHITE)
            } else {
                egui::RichText::new(name).color(egui::Color32::from_rgb(180, 180, 190))
            };
            ui.label(text);
            ui.label(
                egui::RichText::new(tool_name)
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 140)),
            );
        });

        // Per-move-type visibility checkboxes for this toolpath.
        let entry = state
            .viewport
            .toolpath_move_visibility
            .entry(*id)
            .or_default();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.checkbox(&mut entry.show_cutting, "Cut")
                .on_hover_text("Show cutting/feed moves for this toolpath");
            ui.checkbox(&mut entry.show_rapids, "Rapid")
                .on_hover_text("Show rapid moves for this toolpath");
        });

        // Progress bar for this toolpath
        let current = state.simulation.playback.current_move;
        let progress = if current >= *end_move {
            1.0
        } else if current <= *start_move {
            0.0
        } else {
            (current - start_move) as f32 / (end_move - start_move).max(1) as f32
        };
        let bar = egui::ProgressBar::new(progress)
            .fill(color)
            .desired_width(ui.available_width() - 16.0);
        ui.add(bar);

        // Jump-to-boundary button
        let boundary_start = *start_move;
        ui.horizontal(|ui| {
            if ui.small_button("Jump to start").clicked() {
                state.simulation.playback.current_move = boundary_start;
                state.simulation.playback.playing = false;
            }
        });

        ui.add_space(2.0);
    }

    ui.add_space(8.0);

    // Tool position readout
    if let Some(pos) = state.simulation.playback.tool_position {
        ui.label(
            egui::RichText::new("Tool Position")
                .strong()
                .color(egui::Color32::from_rgb(180, 180, 195)),
        );
        egui::Grid::new("sim_tool_pos")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("X:");
                ui.label(format!("{:.3} mm", pos[0]));
                ui.end_row();
                ui.label("Y:");
                ui.label(format!("{:.3} mm", pos[1]));
                ui.end_row();
                ui.label("Z:");
                ui.label(format!("{:.3} mm", pos[2]));
                ui.end_row();
            });
    }

    ui.add_space(8.0);

    // Current operation info
    if let Some(boundary) = state.simulation.current_boundary() {
        let (within, total) = state.simulation.current_toolpath_progress();
        ui.label(
            egui::RichText::new("Current Operation")
                .strong()
                .color(egui::Color32::from_rgb(180, 180, 195)),
        );
        ui.label(format!("{} ({})", boundary.name, boundary.tool_name));
        ui.label(format!("Move {}/{}", within, total));
    }
}

/// Machine-library UX: reference a reusable machine file (single source
/// of truth) or save the current machine into the library. Selecting a
/// library machine loads its values and links the project to it
/// (`machine_ref`); the link is persisted on save and the library file
/// overrides the inline copy on reload.
fn draw_machine_library_row(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    let status_id = egui::Id::new("machine_lib_status");
    let name_id = egui::Id::new("machine_lib_save_name");

    let machines = rs_cam_core::machine_library::list();
    let current_ref = state.session.machine_ref().map(str::to_owned);

    ui.horizontal(|ui| {
        ui.label("Library:");
        let selected_text = current_ref
            .clone()
            .unwrap_or_else(|| "— none (inline copy) —".to_owned());
        egui::ComboBox::from_id_salt("machine_library_ref")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current_ref.is_none(), "— none (inline copy) —")
                    .clicked()
                {
                    state.session.set_machine_ref(None);
                    state.gui.mark_edited();
                }
                for name in &machines {
                    let is_sel = current_ref.as_deref() == Some(name.as_str());
                    if ui.selectable_label(is_sel, name).clicked() {
                        match rs_cam_core::machine_library::load(name) {
                            Ok(profile) => {
                                *state.session.machine_mut() = profile;
                                state.session.set_machine_ref(Some(name.clone()));
                                events.push(AppEvent::MachineChanged);
                                ui.data_mut(|d| {
                                    d.insert_temp(
                                        status_id,
                                        format!("Loaded '{name}' from library"),
                                    );
                                });
                            }
                            Err(e) => {
                                tracing::error!("machine library load failed: {e}");
                                ui.data_mut(|d| {
                                    d.insert_temp(status_id, format!("Load failed: {e}"));
                                });
                            }
                        }
                    }
                }
            });
        if machines.is_empty() {
            ui.label(egui::RichText::new("(library empty)").small().weak());
        }
    });

    ui.horizontal(|ui| {
        ui.label("Save as:");
        let mut name: String = ui.data(|d| d.get_temp::<String>(name_id).unwrap_or_default());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut name)
                .desired_width(140.0)
                .hint_text("machine name"),
        );
        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(name_id, name.clone()));
        }
        let trimmed = name.trim().to_owned();
        if ui
            .add_enabled(!trimmed.is_empty(), egui::Button::new("Save to library"))
            .clicked()
        {
            match rs_cam_core::machine_library::save(&trimmed, state.session.machine()) {
                Ok(path) => {
                    state.session.set_machine_ref(Some(trimmed.clone()));
                    state.gui.mark_edited();
                    ui.data_mut(|d| {
                        d.insert_temp(status_id, format!("Saved to {}", path.display()));
                    });
                }
                Err(e) => {
                    tracing::error!("machine library save failed: {e}");
                    ui.data_mut(|d| d.insert_temp(status_id, format!("Save failed: {e}")));
                }
            }
        }
    });

    if let Some(name) = state.session.machine_ref() {
        ui.label(
            egui::RichText::new(format!(
                "Linked to library '{name}' — edits to the file apply on reload"
            ))
            .small()
            .color(egui::Color32::from_rgb(120, 160, 120)),
        );
    }
    if let Some(msg) = ui.data(|d| d.get_temp::<String>(status_id)) {
        ui.label(egui::RichText::new(msg).small().weak());
    }
}

// SAFETY: selected_idx from position() within presets; i from enumerate over presets
#[allow(clippy::indexing_slicing)]
fn draw_machine_panel(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Machine Setup");
    ui.separator();

    let presets = rs_cam_core::machine::MachineProfile::presets();
    let current_key = state.session.machine().to_key();
    let mut selected_idx = presets
        .iter()
        .position(|(_, p)| p.to_key() == current_key)
        .unwrap_or(0);

    ui.horizontal(|ui| {
        ui.label("Preset:");
        egui::ComboBox::from_id_salt("machine_preset")
            .selected_text(presets[selected_idx].0)
            .show_ui(ui, |ui| {
                for (i, (label, _)) in presets.iter().enumerate() {
                    if ui.selectable_value(&mut selected_idx, i, *label).changed() {
                        *state.session.machine_mut() = presets[i].1.clone();
                        // Loading a built-in preset breaks any library
                        // link — the values no longer come from the file.
                        state.session.set_machine_ref(None);
                        events.push(AppEvent::MachineChanged);
                    }
                }
            });
    });

    draw_machine_library_row(ui, state, events);

    ui.add_space(8.0);

    // Show machine specs (read-only)
    egui::Grid::new("machine_specs")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            let (min_rpm, max_rpm) = state.session.machine().rpm_range();
            ui.label("RPM Range:");
            ui.label(format!("{:.0} - {:.0}", min_rpm, max_rpm));
            ui.end_row();

            let max_power = match state.session.machine().power {
                rs_cam_core::machine::PowerModel::VfdConstantTorque { rated_power_kw, .. } => {
                    rated_power_kw
                }
                rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => power_kw,
            };
            ui.label("Power:");
            ui.label(format!("{:.2} kW", max_power));
            ui.end_row();

            ui.label("Max Feed:");
            ui.label(format!(
                "{:.0} mm/min",
                state.session.machine().max_feed_mm_min
            ));
            ui.end_row();

            ui.label("Max Shank:");
            ui.label(format!("{:.1} mm", state.session.machine().max_shank_mm));
            ui.end_row();
        });

    ui.add_space(8.0);

    // Safety factor / aggressiveness slider
    ui.horizontal(|ui| {
        ui.label("Aggressiveness:");
        if ui
            .add(
                egui::Slider::new(&mut state.session.machine_mut().safety_factor, 0.60..=0.95)
                    .text("")
                    .show_value(true),
            )
            .changed()
        {
            events.push(AppEvent::MachineChanged);
        }
    });
    ui.label(
        egui::RichText::new(if state.session.machine().safety_factor < 0.72 {
            "Conservative — safer for new setups"
        } else if state.session.machine().safety_factor > 0.85 {
            "Aggressive — experienced operators only"
        } else {
            "Balanced — good for most work"
        })
        .small()
        .color(egui::Color32::from_rgb(140, 140, 150)),
    );

    ui.add_space(8.0);

    // Workholding rigidity selector
    let mut rigidity_changed = false;
    ui.horizontal(|ui| {
        ui.label("Workholding:");
        use rs_cam_core::feeds::WorkholdingRigidity;
        let rigidity = &mut state.session.stock_mut().workholding_rigidity;
        let label = match rigidity {
            WorkholdingRigidity::Low => "Low",
            WorkholdingRigidity::Medium => "Medium",
            WorkholdingRigidity::High => "High",
        };
        egui::ComboBox::from_id_salt("workholding_rigidity")
            .selected_text(label)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(rigidity, WorkholdingRigidity::Low, "Low")
                    .changed()
                    || ui
                        .selectable_value(rigidity, WorkholdingRigidity::Medium, "Medium")
                        .changed()
                    || ui
                        .selectable_value(rigidity, WorkholdingRigidity::High, "High")
                        .changed()
                {
                    rigidity_changed = true;
                }
            });
    });
    if rigidity_changed {
        state.gui.mark_edited();
    }
    ui.label(
        egui::RichText::new(match state.session.stock_config().workholding_rigidity {
            rs_cam_core::feeds::WorkholdingRigidity::Low => {
                "Low — tape/CA glue, vacuum table, thin stock"
            }
            rs_cam_core::feeds::WorkholdingRigidity::Medium => {
                "Medium — clamps, toggle clamps, most setups"
            }
            rs_cam_core::feeds::WorkholdingRigidity::High => {
                "High — heavy vise, bolted fixture, thick stock"
            }
        })
        .small()
        .color(egui::Color32::from_rgb(140, 140, 150)),
    );
}

/// Map OperationConfig variant to (OperationFamily, PassRole) for the feeds calculator.
/// Run the LUT calculator (read-only), cache the result on the entry,
/// and draw the feeds card. The calculator never writes to the operation
/// here (Roadmap F.5) — the SPEED / CUT recipe buttons inside
/// `draw_feeds_card` are the only path that pushes calculated values into
/// the op (W3.1: speeds and cut geometry are applied separately).
#[allow(clippy::too_many_arguments)]
fn calculate_and_apply_feeds(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    project_default_rpm: u32,
) {
    match rs_cam_core::feeds::suggest::feeds_result_for_operation(
        &entry.operation,
        tool,
        material,
        machine,
        workholding,
        rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy,
    ) {
        Ok(result) => {
            entry.feeds_result = Some(result);
            draw_feeds_card(ui, entry, tool, machine, material, project_default_rpm);
        }
        Err(e) => {
            // Engine refused — the tool × operation combination is
            // physically unrunnable. Clear any stale cache and show
            // the refusal in place of the feeds card so the user sees
            // why no recipe is offered.
            entry.feeds_result = None;
            ui.add_space(8.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 80, 80),
                format!("Feeds unavailable: {e}"),
            );
            // No LUT recipe, but feed/plunge moved off the Geometry tab in
            // W3.2, so this is the only place to set them by hand. (Drill
            // keeps its feed on the Geometry tab next to the drill cycle.)
            if !matches!(
                entry.operation,
                OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_)
            ) {
                ui.add_space(4.0);
                ui.named_section("SPEED \u{2014} how fast (manual)", |ui| {
                    egui::Grid::new("feeds_manual_speed")
                        .num_columns(2)
                        .spacing([8.0, 3.0])
                        .show(ui, |ui| {
                            let mut feed = entry.operation.feed_rate();
                            if ValueRow::new("Feed:", &mut feed, " mm/min", 50.0, 1.0..=50000.0)
                                .show(ui)
                                .edited
                            {
                                entry.operation.set_feed_rate(feed);
                                entry.stale_since = Some(std::time::Instant::now());
                            }
                            let mut plunge = entry.operation.plunge_rate();
                            if ValueRow::new("Plunge:", &mut plunge, " mm/min", 10.0, 1.0..=10000.0)
                                .show(ui)
                                .edited
                            {
                                entry.operation.set_plunge_rate(plunge);
                                entry.stale_since = Some(std::time::Instant::now());
                            }
                        });
                });
            }
        }
    }
}

fn draw_feeds_card(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: &crate::state::job::ToolConfig,
    machine: &rs_cam_core::machine::MachineProfile,
    material: &rs_cam_core::material::Material,
    project_default_rpm: u32,
) {
    ui.add_space(8.0);
    ui.collapsing("Feeds & Speeds", |ui| {
        // Read-only snapshot of the cached LUT result so we can borrow
        // `entry.operation` mutably from the recipe buttons below.
        let Some(result) = entry.feeds_result.clone() else {
            return;
        };
        // Capture op capabilities as plain bools so no borrow of
        // `entry.operation` is held across the mutating recipe closures.
        let pass_role = entry.operation.feeds_style().1;
        let has_stepover = entry.operation.as_params().stepover().is_some();
        let has_dpp = entry.operation.as_params().depth_per_pass().is_some();
        // Drill ops are Z-only (plunge_rate IS feed_rate, no WOC/DOC); their
        // single feed is edited on the Geometry tab next to the drill cycle,
        // so the SPEED section shows it read-only to avoid a duplicate /
        // no-op "Plunge" field (W3.2).
        let is_drill = matches!(
            entry.operation,
            OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_)
        );

        // ── SPEED — how fast (feed / plunge / RPM) ──
        ui.named_section("SPEED \u{2014} how fast", |ui| {
            // The live LUT recommendation drives each field's ⚡ suggest.
            let (speed_kind, speed_ref) = prov_from_chipload(&result.chipload_source);
            egui::Grid::new("feeds_card_speed")
                .num_columns(2)
                .spacing([8.0, 3.0])
                .show(ui, |ui| {
                    // Feed / Plunge — editable here (W3.2 relocated them off
                    // the Geometry tab), each with a per-field ⚡ that applies
                    // only its own recommendation.
                    if is_drill {
                        ui.label("Feed:");
                        ui.label(format!("{:.0} mm/min", result.feed_rate_mm_min));
                        ui.end_row();
                    } else {
                        let mut feed = entry.operation.feed_rate();
                        if ValueRow::new("Feed:", &mut feed, " mm/min", 50.0, 1.0..=50000.0)
                            .suggest(Suggestion {
                                recommended: result.feed_rate_mm_min,
                                source: speed_kind,
                                reference: speed_ref,
                            })
                            .show(ui)
                            .edited
                        {
                            entry.operation.set_feed_rate(feed);
                            entry.stale_since = Some(std::time::Instant::now());
                        }
                        let mut plunge = entry.operation.plunge_rate();
                        if ValueRow::new("Plunge:", &mut plunge, " mm/min", 10.0, 1.0..=10000.0)
                            .suggest(Suggestion {
                                recommended: result.plunge_rate_mm_min,
                                source: speed_kind,
                                reference: speed_ref,
                            })
                            .show(ui)
                            .edited
                        {
                            entry.operation.set_plunge_rate(plunge);
                            entry.stale_since = Some(std::time::Instant::now());
                        }
                    }
                    ui.label("Chip Load:");
                    ui.label(format!("{:.4} mm/tooth", result.chip_load_mm));
                    ui.end_row();
                    // Spindle override vs project default. W3.1 relocated this
                    // from the per-op Params tab so the precedence renders
                    // honestly.
                    let mut spindle = entry.operation.spindle_rpm();
                    if PrecedenceField::new("Spindle:", &mut spindle, project_default_rpm)
                        .suffix(" RPM")
                        .speed(100.0)
                        .range(1_000..=60_000)
                        .tooltip(
                            "Override the project default spindle speed for this operation. \
                             Leave unchecked to follow the post-config spindle speed.",
                        )
                        .show(ui)
                    {
                        entry.operation.set_spindle_rpm(spindle);
                        entry.stale_since = Some(std::time::Instant::now());
                    }
                });
            // W3.1: SPEED-only apply — never rewrites the cut geometry.
            if ui
                .button("\u{26A1}\u{26A1} Apply recommended speeds")
                .on_hover_text(
                    "Overwrite feed, plunge, and RPM with the recommended values. \
                     Does not change the cut (DOC/WOC).",
                )
                .clicked()
            {
                rs_cam_core::feeds::suggest::apply_speeds_to_op(
                    &mut entry.operation,
                    &mut entry.feeds_provenance,
                    &result,
                    tool,
                    machine,
                    material,
                    pass_role,
                    rs_cam_core::feeds::suggest::SuggestContext::default(),
                );
                entry.stale_since = Some(std::time::Instant::now());
            }
        });

        // ── CUT — how deep/wide (changes the cut) ──
        if has_stepover || has_dpp {
            ui.named_section("CUT \u{2014} changes the cut", |ui| {
                egui::Grid::new("feeds_card_cut")
                    .num_columns(2)
                    .spacing([8.0, 3.0])
                    .show(ui, |ui| {
                        if has_dpp {
                            ui.label("DOC:");
                            ui.label(format!("{:.2} mm", result.axial_depth_mm));
                            ui.end_row();
                        }
                        if has_stepover {
                            ui.label("WOC:");
                            ui.label(format!("{:.2} mm", result.radial_width_mm));
                            ui.end_row();
                        }
                    });
                // W3.1: cut-geometry apply is separate + attributed — it
                // changes the cut, so it is never folded into "Apply speeds".
                if ui
                    .button("\u{26A1} Apply cut geometry")
                    .on_hover_text(
                        "Overwrite DOC/WOC with the recommended values. \
                         Changes the cut.",
                    )
                    .clicked()
                {
                    rs_cam_core::feeds::suggest::apply_cut_geometry_to_op(
                        &mut entry.operation,
                        &mut entry.feeds_provenance,
                        &result,
                        tool,
                        machine,
                        material,
                        pass_role,
                        rs_cam_core::feeds::suggest::SuggestContext::default(),
                    );
                    entry.stale_since = Some(std::time::Instant::now());
                }
            });
        }

        // ── Derived (read-only) ──
        ui.named_section("Derived", |ui| {
            power_bar(ui, result.power_kw, result.available_power_kw);
            mrr_row(ui, result.mrr_mm3_min);
        });

        {
            // Vendor source — the one provenance vocabulary (was a third,
            // cyan, source-colour treatment; collapsed onto ProvenanceBadge to
            // kill the P7-003 three-colours-for-one-signal divergence).
            let (kind, reference) = prov_from_chipload(&result.chipload_source);
            ui.add(ProvenanceBadge::new(kind).reference_opt(reference));

            // Warnings
            for w in &result.warnings {
                let text = match w {
                    rs_cam_core::feeds::FeedsWarning::FeedRateClamped { requested, actual } => {
                        format!(
                            "Feed clamped: {requested:.0} -> {actual:.0} mm/min (machine limit)"
                        )
                    }
                    rs_cam_core::feeds::FeedsWarning::PowerLimited {
                        required_kw,
                        available_kw,
                    } => format!(
                        "Power limited: {required_kw:.2}kW needed, {available_kw:.2}kW available"
                    ),
                    rs_cam_core::feeds::FeedsWarning::DocExceedsFlute { requested, capped } => {
                        format!("DOC capped: {requested:.1} -> {capped:.1}mm (flute guard)")
                    }
                    rs_cam_core::feeds::FeedsWarning::SlottingDetected { doc_reduced_to } => {
                        format!("Slotting detected: DOC reduced to {doc_reduced_to:.1}mm")
                    }
                    rs_cam_core::feeds::FeedsWarning::ScallopInvalid {
                        target,
                        max_possible,
                    } => format!("Invalid scallop: {target:.3}mm (max {max_possible:.1}mm)"),
                    rs_cam_core::feeds::FeedsWarning::ShankTooLarge { shank_mm, max_mm } => {
                        format!("Shank {shank_mm:.1}mm exceeds max {max_mm:.1}mm")
                    }
                    rs_cam_core::feeds::FeedsWarning::ChiploadClampedToFloor {
                        requested,
                        floor,
                    } => format!(
                        "Chipload below rubbing floor: {requested:.3} -> {floor:.3}mm/tooth"
                    ),
                };
                ui.label(
                    egui::RichText::new(format!("! {text}"))
                        .small()
                        .color(egui::Color32::from_rgb(220, 170, 60)),
                );
            }
        }
    });
}

// ── Vendor LUT viewer ──────────────────────────────────────────────────

/// Map `ToolType` to the vendor LUT `ToolFamily` for filtering.
fn tool_type_to_lut_family(
    tt: crate::state::job::ToolType,
) -> rs_cam_core::feeds::vendor_lut::ToolFamily {
    use rs_cam_core::feeds::vendor_lut::ToolFamily;
    match tt {
        crate::state::job::ToolType::EndMill => ToolFamily::FlatEnd,
        crate::state::job::ToolType::BallNose => ToolFamily::BallNose,
        crate::state::job::ToolType::BullNose => ToolFamily::BullNose,
        crate::state::job::ToolType::VBit => ToolFamily::ChamferVbit,
        crate::state::job::ToolType::TaperedBallNose => ToolFamily::TaperedBallNose,
    }
}

/// Human-readable label for a `MaterialFamily`.
fn material_family_label(m: rs_cam_core::feeds::vendor_lut::MaterialFamily) -> &'static str {
    use rs_cam_core::feeds::vendor_lut::MaterialFamily;
    match m {
        MaterialFamily::Softwood => "Softwood",
        MaterialFamily::Hardwood => "Hardwood",
        MaterialFamily::PlywoodSoftwood => "Plywood (Soft)",
        MaterialFamily::PlywoodHardwood => "Plywood (Hard)",
        MaterialFamily::Mdf => "MDF",
        MaterialFamily::Hdf => "HDF",
        MaterialFamily::Particleboard => "Particleboard",
        MaterialFamily::Acrylic => "Acrylic",
        MaterialFamily::Hdpe => "HDPE",
        MaterialFamily::Polycarbonate => "Polycarbonate",
        MaterialFamily::Delrin => "Delrin",
        MaterialFamily::Aluminum => "Aluminum",
        MaterialFamily::Fiberglass => "Fiberglass",
    }
}

/// Human-readable label for a `ToolFamily`.
fn tool_family_label(f: rs_cam_core::feeds::vendor_lut::ToolFamily) -> &'static str {
    use rs_cam_core::feeds::vendor_lut::ToolFamily;
    match f {
        ToolFamily::FlatEnd => "Flat End",
        ToolFamily::BallNose => "Ball Nose",
        ToolFamily::TaperedBallNose => "Tapered Ball",
        ToolFamily::BullNose => "Bull Nose",
        ToolFamily::ChamferVbit => "V-Bit",
        ToolFamily::FacingBit => "Facing",
    }
}

/// Human-readable label for an `EvidenceGrade`.
fn evidence_grade_label(g: rs_cam_core::feeds::vendor_lut::EvidenceGrade) -> &'static str {
    use rs_cam_core::feeds::vendor_lut::EvidenceGrade;
    match g {
        EvidenceGrade::A => "A (vendor)",
        EvidenceGrade::B => "B (derived)",
        EvidenceGrade::C => "C (community)",
    }
}

/// Draw a collapsible vendor cutting data viewer, filtered by current tool.
fn draw_vendor_lut_viewer(
    ui: &mut egui::Ui,
    tool_type: crate::state::job::ToolType,
    tool_diameter: f64,
) {
    ui.add_space(8.0);

    let header = egui::RichText::new("Vendor Cutting Data")
        .strong()
        .color(egui::Color32::from_rgb(180, 180, 195));

    egui::CollapsingHeader::new(header)
        .default_open(false)
        .show(ui, |ui| {
            let lut = rs_cam_core::feeds::embedded_vendor_lut();
            let target_family = tool_type_to_lut_family(tool_type);

            // Filter observations: match tool family, and prefer matching diameter
            // (show all diameters for this family so the user can see the full picture).
            let matching: Vec<&rs_cam_core::feeds::vendor_lut::VendorObservation> = lut
                .observations
                .iter()
                .filter(|obs| obs.tool_family == target_family)
                .collect();

            if matching.is_empty() {
                ui.label(
                    egui::RichText::new("No matching vendor data for this tool type")
                        .small()
                        .color(egui::Color32::from_rgb(160, 140, 100)),
                );
                return;
            }

            ui.label(
                egui::RichText::new(format!(
                    "{} observations for {} tools (current: {:.1} mm)",
                    matching.len(),
                    tool_family_label(target_family),
                    tool_diameter,
                ))
                .small()
                .color(egui::Color32::from_rgb(140, 140, 155)),
            );
            ui.add_space(4.0);

            // Table header
            let dim = egui::Color32::from_rgb(120, 125, 140);
            let val = egui::Color32::from_rgb(170, 170, 185);
            let highlight = egui::Color32::from_rgb(100, 180, 140);
            let header_font = egui::FontId::proportional(9.0);
            let body_font = egui::FontId::proportional(9.0);

            egui::Grid::new("vendor_lut_table")
                .num_columns(7)
                .spacing([6.0, 2.0])
                .striped(true)
                .show(ui, |ui| {
                    // Column headers
                    for label in [
                        "Material", "Dia (mm)", "Flutes", "RPM", "Chipload", "DOC (mm)", "Grade",
                    ] {
                        ui.label(
                            egui::RichText::new(label)
                                .font(header_font.clone())
                                .strong()
                                .color(dim),
                        );
                    }
                    ui.end_row();

                    for obs in &matching {
                        // Highlight rows matching the current tool diameter (within 0.1mm).
                        // Rows with `diameter_mm = None` (v-bit charts, diameter-window
                        // articles) are never highlighted as exact-diameter matches —
                        // their match criterion is angle / material, not diameter.
                        let is_diameter_match = match obs.diameter_mm {
                            Some(d) => (d - tool_diameter).abs() < 0.1,
                            None => false,
                        };
                        let row_color = if is_diameter_match { highlight } else { val };

                        // Material
                        ui.label(
                            egui::RichText::new(material_family_label(obs.material_family))
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // Diameter (— if the row has no diameter anchor)
                        let diameter_text = match obs.diameter_mm {
                            Some(d) => format!("{:.1}", d),
                            None => "—".to_owned(),
                        };
                        ui.label(
                            egui::RichText::new(diameter_text)
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // Flutes
                        ui.label(
                            egui::RichText::new(format!("{}", obs.flute_count))
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // RPM range
                        let rpm_text = match (obs.rpm_min, obs.rpm_max) {
                            (Some(lo), Some(hi)) => format!("{lo:.0}-{hi:.0}"),
                            (Some(lo), None) => format!("{lo:.0}"),
                            (None, Some(hi)) => format!("{hi:.0}"),
                            (None, None) => {
                                if let Some(nom) = obs.rpm_nominal {
                                    format!("{nom:.0}")
                                } else {
                                    "-".to_owned()
                                }
                            }
                        };
                        ui.label(
                            egui::RichText::new(rpm_text)
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // Chipload range (mm/tooth)
                        let chip_text = match (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth)
                        {
                            (Some(lo), Some(hi)) => format!("{lo:.3}-{hi:.3}"),
                            (Some(lo), None) => format!("{lo:.3}"),
                            (None, Some(hi)) => format!("{hi:.3}"),
                            (None, None) => "-".to_owned(),
                        };
                        ui.label(
                            egui::RichText::new(chip_text)
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // DOC range (ap)
                        let doc_text = match (obs.ap_min_mm, obs.ap_max_mm) {
                            (Some(lo), Some(hi)) => format!("{lo:.1}-{hi:.1}"),
                            (Some(v), None) | (None, Some(v)) => format!("{v:.1}"),
                            (None, None) => "-".to_owned(),
                        };
                        ui.label(
                            egui::RichText::new(doc_text)
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        // Evidence grade
                        ui.label(
                            egui::RichText::new(evidence_grade_label(obs.evidence_grade))
                                .font(body_font.clone())
                                .color(row_color),
                        );

                        ui.end_row();
                    }
                });
        });
}

// ── Engagement diagram ──────────────────────────────────────────────────

/// Draw a split-view engagement diagram: top-down WOC (left) + side DOC (right).
fn draw_engagement_diagram(
    ui: &mut egui::Ui,
    result: &rs_cam_core::feeds::FeedsResult,
    tool_diameter: f64,
    tool_type: crate::state::job::ToolType,
) {
    let desired_size = egui::vec2(ui.available_width().min(260.0), 150.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    let tool_r = tool_diameter / 2.0;
    let woc = result.radial_width_mm;
    let doc = result.axial_depth_mm;
    let tool_color = egui::Color32::from_rgb(160, 170, 190);
    let mat_color = egui::Color32::from_rgb(50, 50, 65);
    let dim_color = egui::Color32::from_rgb(100, 160, 220);
    let info_color = egui::Color32::from_rgb(140, 140, 155);

    // Divider: split canvas at ~55%
    let mid_x = rect.left() + rect.width() * 0.52;
    painter.line_segment(
        [
            egui::pos2(mid_x, rect.top() + 4.0),
            egui::pos2(mid_x, rect.bottom() - 4.0),
        ],
        egui::Stroke::new(0.5, egui::Color32::from_rgb(40, 40, 50)),
    );

    // ── LEFT: Top-down WOC view ────────────────────────────────────
    let left_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 4.0, rect.top() + 16.0),
        egui::pos2(mid_x - 4.0, rect.bottom() - 16.0),
    );
    let scale_woc = (left_rect.width() * 0.35) / tool_r.max(0.01) as f32;
    let cx = left_rect.center().x;
    let cy = left_rect.center().y;
    let tr = tool_r as f32 * scale_woc;

    // Material block
    let mat_left = cx + tr - (woc as f32 * scale_woc);
    let mat_right = left_rect.right();
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(mat_left, cy - tr - 6.0),
            egui::pos2(mat_right, cy + tr + 6.0),
        ),
        0.0,
        mat_color,
    );

    // WOC crescent
    if woc > 0.0 && woc <= tool_diameter {
        let engage_frac = (woc / tool_diameter).clamp(0.0, 1.0);
        let half_angle = (engage_frac * std::f32::consts::PI as f64).min(std::f64::consts::PI);
        let mut pts = Vec::with_capacity(34);
        let steps = 32;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let a = -half_angle + 2.0 * half_angle * t;
            let px = cx + (tool_r * a.cos()) as f32 * scale_woc;
            let py = cy + (tool_r * a.sin()) as f32 * scale_woc;
            if px >= mat_left {
                pts.push(egui::pos2(px, py));
            }
        }
        if pts.len() >= 2 {
            let first_y = pts.first().map(|p| p.y).unwrap_or(cy);
            let last_y = pts.last().map(|p| p.y).unwrap_or(cy);
            pts.push(egui::pos2(mat_left, last_y));
            pts.push(egui::pos2(mat_left, first_y));
            painter.add(egui::Shape::convex_polygon(
                pts,
                egui::Color32::from_rgba_premultiplied(60, 140, 200, 50),
                egui::Stroke::NONE,
            ));
        }
    }

    painter.circle_stroke(egui::pos2(cx, cy), tr, egui::Stroke::new(1.5, tool_color));
    painter.circle_filled(egui::pos2(cx, cy), 1.5, tool_color);

    // WOC label
    painter.text(
        egui::pos2(cx, cy + tr + 10.0),
        egui::Align2::CENTER_TOP,
        format!("WOC {woc:.2}"),
        egui::FontId::proportional(8.0),
        dim_color,
    );

    // "Top" label
    painter.text(
        egui::pos2(left_rect.center().x, rect.top() + 3.0),
        egui::Align2::CENTER_TOP,
        "Top",
        egui::FontId::proportional(8.0),
        info_color,
    );

    // ── RIGHT: Side DOC view ───────────────────────────────────────
    let right_rect = egui::Rect::from_min_max(
        egui::pos2(mid_x + 4.0, rect.top() + 16.0),
        egui::pos2(rect.right() - 4.0, rect.bottom() - 16.0),
    );

    // Scale: fit max(doc, tool_diameter) into the right panel height
    let max_z_extent = doc.max(tool_diameter).max(1.0);
    let scale_doc = (right_rect.height() * 0.7) / max_z_extent as f32;
    let scx = right_rect.center().x;

    // Material surface at top of side view
    let surface_y = right_rect.top() + right_rect.height() * 0.15;
    let tool_hw = tool_r as f32 * scale_doc;
    let doc_px = doc as f32 * scale_doc;

    // Material block (below surface)
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(right_rect.left(), surface_y),
            egui::pos2(right_rect.right(), right_rect.bottom()),
        ),
        0.0,
        mat_color,
    );

    // Material surface line
    painter.line_segment(
        [
            egui::pos2(right_rect.left(), surface_y),
            egui::pos2(right_rect.right(), surface_y),
        ],
        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 95)),
    );

    // DOC shaded region (where tool cuts)
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(scx - tool_hw, surface_y),
            egui::pos2(scx + tool_hw, surface_y + doc_px),
        ),
        0.0,
        egui::Color32::from_rgba_premultiplied(60, 140, 200, 40),
    );

    // Tool profile (simplified side view)
    let tool_top = surface_y - tool_hw * 0.4; // shaft extends above surface
    let tool_bottom = surface_y + doc_px;
    use crate::state::job::ToolType;
    match tool_type {
        ToolType::BallNose => {
            // Shaft rectangle above, semicircle at bottom
            let ball_cy = tool_bottom - tool_hw;
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(scx - tool_hw, ball_cy),
                    egui::pos2(scx - tool_hw, tool_top),
                    egui::pos2(scx + tool_hw, tool_top),
                    egui::pos2(scx + tool_hw, ball_cy),
                ],
                egui::Stroke::new(1.5, tool_color),
            ));
            let mut arc_pts = vec![egui::pos2(scx + tool_hw, ball_cy)];
            for i in 0..=16 {
                let a = std::f32::consts::PI * (i as f32) / 16.0;
                arc_pts.push(egui::pos2(
                    scx + tool_hw * a.cos(),
                    ball_cy + tool_hw * a.sin(),
                ));
            }
            painter.add(egui::Shape::line(
                arc_pts,
                egui::Stroke::new(1.5, tool_color),
            ));
        }
        _ => {
            // EndMill / BullNose / VBit — simple rectangle
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(scx - tool_hw, tool_bottom),
                    egui::pos2(scx - tool_hw, tool_top),
                    egui::pos2(scx + tool_hw, tool_top),
                    egui::pos2(scx + tool_hw, tool_bottom),
                    egui::pos2(scx - tool_hw, tool_bottom),
                ],
                egui::Stroke::new(1.5, tool_color),
            ));
        }
    }

    // DOC dimension line (right side)
    let dim_x = scx + tool_hw + 8.0;
    painter.line_segment(
        [
            egui::pos2(dim_x, surface_y),
            egui::pos2(dim_x, surface_y + doc_px),
        ],
        egui::Stroke::new(1.0, dim_color),
    );
    // Ticks
    painter.line_segment(
        [
            egui::pos2(dim_x - 3.0, surface_y),
            egui::pos2(dim_x + 3.0, surface_y),
        ],
        egui::Stroke::new(1.0, dim_color),
    );
    painter.line_segment(
        [
            egui::pos2(dim_x - 3.0, surface_y + doc_px),
            egui::pos2(dim_x + 3.0, surface_y + doc_px),
        ],
        egui::Stroke::new(1.0, dim_color),
    );
    painter.text(
        egui::pos2(dim_x + 2.0, surface_y + doc_px / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{doc:.2}"),
        egui::FontId::proportional(8.0),
        dim_color,
    );

    // "Side" label
    painter.text(
        egui::pos2(right_rect.center().x, rect.top() + 3.0),
        egui::Align2::CENTER_TOP,
        "Side",
        egui::FontId::proportional(8.0),
        info_color,
    );

    // Stats at bottom
    let stats_y = rect.bottom() - 4.0;
    painter.text(
        egui::pos2(rect.left() + 4.0, stats_y),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "Chip {:.4}  MRR {:.0} mm\u{00B3}/min",
            result.chip_load_mm, result.mrr_mm3_min
        ),
        egui::FontId::proportional(8.0),
        info_color,
    );
}

// ── Entry style preview diagram ─────────────────────────────────────────

/// Draw a 2D side-view of the entry style geometry (ramp or helix).
fn draw_entry_preview_diagram(
    ui: &mut egui::Ui,
    dressups: &DressupConfig,
    height_ctx: &HeightContext,
    heights: &HeightsConfig,
) {
    let resolved = heights.resolve(height_ctx);
    let feed_z = resolved.feed_z;
    let top_z = resolved.top_z;
    let z_drop = feed_z - top_z;
    if z_drop <= 0.0 {
        return;
    }

    let desired_size = egui::vec2(ui.available_width().min(260.0), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 26));

    // Z range with margin
    let z_min = top_z - z_drop * 0.15;
    let z_max = feed_z + z_drop * 0.25;

    let z_to_y = |z: f64| -> f32 {
        let frac = (z - z_min) / (z_max - z_min);
        rect.bottom() - (frac as f32) * rect.height()
    };

    let feed_y = z_to_y(feed_z);
    let top_y = z_to_y(top_z);
    let dim_color = egui::Color32::from_rgb(100, 100, 115);
    let scale_color = egui::Color32::from_rgb(70, 70, 85);

    // Z-axis scale bar on the left edge
    let scale_x = rect.left() + 3.0;
    painter.line_segment(
        [egui::pos2(scale_x, feed_y), egui::pos2(scale_x, top_y)],
        egui::Stroke::new(1.0, scale_color),
    );
    // Ticks + values at feed_z and top_z
    painter.line_segment(
        [
            egui::pos2(scale_x, feed_y),
            egui::pos2(scale_x + 4.0, feed_y),
        ],
        egui::Stroke::new(1.0, scale_color),
    );
    painter.line_segment(
        [egui::pos2(scale_x, top_y), egui::pos2(scale_x + 4.0, top_y)],
        egui::Stroke::new(1.0, scale_color),
    );
    // Z drop distance label
    painter.text(
        egui::pos2(scale_x + 2.0, (feed_y + top_y) / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{z_drop:.1}"),
        egui::FontId::proportional(8.0),
        scale_color,
    );

    // Dashed horizontal reference lines
    for &(z, label) in &[(feed_z, "Feed Z"), (top_z, "Top Z")] {
        let y = z_to_y(z);
        // Draw dashed line
        let dash_len = 6.0;
        let gap_len = 4.0;
        let mut x = rect.left() + 12.0;
        while x < rect.right() - 50.0 {
            let end_x = (x + dash_len).min(rect.right() - 50.0);
            painter.line_segment(
                [egui::pos2(x, y), egui::pos2(end_x, y)],
                egui::Stroke::new(0.5, dim_color),
            );
            x += dash_len + gap_len;
        }
        painter.text(
            egui::pos2(rect.right() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{label} {z:.1}"),
            egui::FontId::proportional(8.0),
            dim_color,
        );
    }

    let entry_color = egui::Color32::from_rgb(50, 230, 230);
    let stroke = egui::Stroke::new(2.0, entry_color);
    let cx = rect.center().x;

    match dressups.entry_style {
        DressupEntryStyle::Ramp => {
            let angle_rad = (dressups.ramp_angle as f32).to_radians();
            let ramp_horiz = z_drop as f32 / angle_rad.tan().max(0.01);

            // Scale horizontal distance to fit canvas
            let available_w = rect.width() * 0.6;
            let h_scale = available_w / ramp_horiz.max(1.0);
            let v_height = (top_y - feed_y).abs();
            let h_pixels = ramp_horiz * h_scale.min(1.0);

            let start_x = cx - h_pixels / 2.0;
            let end_x = cx + h_pixels / 2.0;

            // Ramp line
            painter.add(egui::Shape::line(
                vec![egui::pos2(start_x, feed_y), egui::pos2(end_x, top_y)],
                stroke,
            ));

            // Angle arc annotation
            let arc_r = 20.0_f32;
            let mut arc_pts = Vec::with_capacity(12);
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let a = -angle_rad * t;
                arc_pts.push(egui::pos2(
                    start_x + arc_r * a.cos(),
                    feed_y - arc_r * a.sin(),
                ));
            }
            painter.add(egui::Shape::line(
                arc_pts,
                egui::Stroke::new(1.0, entry_color),
            ));
            painter.text(
                egui::pos2(start_x + arc_r + 4.0, feed_y - 8.0),
                egui::Align2::LEFT_CENTER,
                format!("{:.1}\u{00B0}", dressups.ramp_angle),
                egui::FontId::proportional(9.0),
                entry_color,
            );

            // Entry point marker
            painter.circle_filled(egui::pos2(end_x, top_y), 3.0, entry_color);

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Ramp Entry",
                egui::FontId::proportional(10.0),
                entry_color,
            );

            let _ = v_height;
        }
        DressupEntryStyle::Helix => {
            let radius = dressups.helix_radius;
            let pitch = dressups.helix_pitch;
            let turns = z_drop / pitch.max(0.01);

            // Side view of helix: sinusoidal wave descending
            let total_angle = turns * std::f64::consts::TAU;
            let steps = (turns * 32.0).clamp(32.0, 200.0) as usize;

            // Scale radius to fit canvas
            let available_w = rect.width() * 0.5;
            let r_pixels = (radius as f32 * available_w / (radius as f32 * 2.0).max(1.0))
                .min(available_w / 2.0);

            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let angle = total_angle * t;
                let z = feed_z - z_drop * t;
                let x_off = (radius * angle.cos()) as f32 * (r_pixels / radius.max(0.01) as f32);
                pts.push(egui::pos2(cx + x_off, z_to_y(z)));
            }
            painter.add(egui::Shape::line(pts, stroke));

            // Entry point marker
            painter.circle_filled(egui::pos2(cx, top_y), 3.0, entry_color);

            // Radius annotation
            painter.line_segment(
                [
                    egui::pos2(cx, z_to_y(feed_z)),
                    egui::pos2(cx + r_pixels, z_to_y(feed_z)),
                ],
                egui::Stroke::new(1.0, dim_color),
            );
            painter.text(
                egui::pos2(cx + r_pixels / 2.0, z_to_y(feed_z) - 8.0),
                egui::Align2::CENTER_BOTTOM,
                format!("r={radius:.1}"),
                egui::FontId::proportional(8.0),
                dim_color,
            );

            // Label
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                format!("Helix Entry ({turns:.1} turns)"),
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
        DressupEntryStyle::None => {
            // Vertical plunge arrow
            painter.line_segment([egui::pos2(cx, feed_y), egui::pos2(cx, top_y)], stroke);
            // Arrowhead
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(cx - 4.0, top_y - 8.0),
                    egui::pos2(cx, top_y),
                    egui::pos2(cx + 4.0, top_y - 8.0),
                ],
                stroke,
            ));
            painter.text(
                egui::pos2(rect.left() + 6.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                "Direct Plunge",
                egui::FontId::proportional(10.0),
                entry_color,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
// ── Toolpath property tab system ─────────────────────────────────────────
// The per-toolpath properties panel is chartered into five concern tabs
// (IA cleanup W3.2, FINAL_DESIGN §4): Geometry (what to cut), Feeds & Speeds
// (how fast — the SPEED/CUT split from W3.1), Linking (how moves connect:
// entry/exit, move optimization, retract), Heights (Z planes), and Dressup
// (edge work / path quality). Replaces the old [Params][Feeds][Heights][Dressups].
#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolpathTab {
    Geometry,
    FeedsSpeeds,
    Linking,
    Heights,
    Dressup,
}

impl ToolpathTab {
    const ALL: &[ToolpathTab] = &[
        ToolpathTab::Geometry,
        ToolpathTab::FeedsSpeeds,
        ToolpathTab::Linking,
        ToolpathTab::Heights,
        ToolpathTab::Dressup,
    ];

    fn label(self) -> &'static str {
        match self {
            ToolpathTab::Geometry => "Geometry",
            ToolpathTab::FeedsSpeeds => "Feeds & Speeds",
            ToolpathTab::Linking => "Linking",
            ToolpathTab::Heights => "Heights",
            ToolpathTab::Dressup => "Dressup",
        }
    }
}

/// Per-tab badge state for the tab bar.
struct TabBadges {
    feeds_badge: Option<egui::Color32>,
    heights_badge: Option<egui::Color32>,
    dressup_badge: Option<egui::Color32>,
}

impl TabBadges {
    fn for_tab(&self, tab: ToolpathTab) -> Option<egui::Color32> {
        match tab {
            ToolpathTab::FeedsSpeeds => self.feeds_badge,
            ToolpathTab::Heights => self.heights_badge,
            ToolpathTab::Dressup => self.dressup_badge,
            ToolpathTab::Geometry | ToolpathTab::Linking => None,
        }
    }
}

fn compute_tab_badges(
    entry: &ToolpathEntry,
    diagnostics: &[rs_cam_core::diagnostics::Diagnostic],
    height_ctx: Option<&HeightContext>,
) -> TabBadges {
    use rs_cam_core::diagnostics::{Category, DiagnosticState, Severity};

    // Heights: badge if any height warning exists
    let heights_badge = if let Some(hctx) = height_ctx {
        let h = entry.heights.resolve(hctx);
        if h.bottom_z > h.top_z || h.clearance_z < h.retract_z {
            Some(egui::Color32::from_rgb(220, 100, 80)) // red
        } else if h.feed_z < h.top_z || h.retract_z < h.feed_z {
            Some(egui::Color32::from_rgb(220, 180, 60)) // yellow
        } else {
            None
        }
    } else {
        None
    };

    // Feeds: badge if any feed warning from core
    let feeds_badge = entry.feeds_result.as_ref().and_then(|r| {
        if r.warnings.is_empty() {
            None
        } else if r.power_limited {
            Some(egui::Color32::from_rgb(220, 180, 60))
        } else {
            Some(egui::Color32::from_rgb(100, 180, 220))
        }
    });

    // Mods: badge derived from actionable Current diagnostics in
    // Safety / Geometry / ToolLoad / Quality categories. Pre-sim hints
    // and State (workflow) notices stay quiet so a healthy op doesn't
    // flash a yellow tab.
    let mut has_critical = false;
    let mut has_caution = false;
    for d in diagnostics {
        if d.state != DiagnosticState::Current {
            continue;
        }
        if matches!(d.category, Category::State | Category::Efficiency) {
            continue;
        }
        match d.severity {
            Severity::Blocking | Severity::Critical => has_critical = true,
            Severity::Caution => has_caution = true,
            _ => {}
        }
    }
    let dressup_badge = if has_critical {
        Some(egui::Color32::from_rgb(220, 100, 80))
    } else if has_caution {
        Some(egui::Color32::from_rgb(220, 180, 60))
    } else {
        None
    };

    TabBadges {
        feeds_badge,
        heights_badge,
        dressup_badge,
    }
}

/// Tier of a diagnostic row in the params panel. Drives the colour
/// scheme + collapse behaviour without polluting the core schema.
#[derive(Debug, Clone, Copy)]
enum RowTier {
    Actionable,
    Stateful,
    Hint,
}

/// Render one diagnostic row in the params panel ribbon. Surfaces
/// evidence + confidence inline. When the diagnostic carries an
/// `ApplyStaleDefault` fix, the fix button is rendered alongside
/// (uses the same direct-apply path the validator banner uses).
/// `SetToolpathParam` fixes are advisory for now — the schema
/// supports them but no adapter emits them yet.
fn render_diagnostic_row(
    ui: &mut egui::Ui,
    d: &rs_cam_core::diagnostics::Diagnostic,
    tier: RowTier,
    entry: &mut ToolpathEntry,
    stale_default_defects: &[rs_cam_core::compute::validate::StaleDefault],
) {
    use rs_cam_core::diagnostics::{Category, DiagnosticFix, Severity};

    let category_label = match d.category {
        Category::Safety => "Safety",
        Category::Geometry => "Geometry",
        Category::ToolLoad => "Tool load",
        Category::Quality => "Quality",
        Category::Efficiency => "Efficiency",
        Category::State => "State",
    };
    let color = match tier {
        RowTier::Actionable => match d.severity {
            Severity::Blocking | Severity::Critical => egui::Color32::from_rgb(220, 100, 80),
            Severity::Caution => egui::Color32::from_rgb(220, 180, 60),
            _ => egui::Color32::from_rgb(100, 180, 220),
        },
        RowTier::Stateful => egui::Color32::from_rgb(140, 145, 150),
        RowTier::Hint => egui::Color32::from_rgb(130, 140, 150),
    };

    ui.horizontal_wrapped(|ui| {
        let prefix = if matches!(d.category, Category::State) && matches!(tier, RowTier::Stateful) {
            String::new()
        } else {
            format!("{category_label}: ")
        };
        ui.label(
            egui::RichText::new(format!("{prefix}{}", d.message))
                .small()
                .color(color),
        );
        if matches!(tier, RowTier::Actionable) {
            let chip_text = confidence_chip_label(d.confidence);
            if !chip_text.is_empty() {
                ui.label(
                    egui::RichText::new(chip_text)
                        .small()
                        .italics()
                        .color(egui::Color32::from_rgb(110, 115, 125)),
                );
            }
        }

        // Stale-default fix button — looks up the matching defect by
        // rule_id (the adapter only carries the id; the canonical
        // payload lives in `stale_default_defects`). Mirrors the
        // existing validator-banner Fix button so behaviour is
        // identical to clicking that.
        if let Some(DiagnosticFix::ApplyStaleDefault {
            rule_id, new_value, ..
        }) = &d.fix
            && let Some(defect) = stale_default_defects
                .iter()
                .find(|defect| defect.rule_id.id() == rule_id.as_str())
            && ui
                .small_button(format!("\u{2713} Fix ({new_value:.3})"))
                .on_hover_text(format!(
                    "Apply validator rule `{rule_id}` to this toolpath."
                ))
                .clicked()
        {
            rs_cam_core::compute::validate::apply_stale_default_to_op(
                &mut entry.operation,
                &mut entry.feeds_provenance,
                defect,
            );
            entry.stale_since = Some(std::time::Instant::now());
        }
    });

    if !matches!(tier, RowTier::Hint)
        && let Some(ev) = &d.evidence
        && let Some(line) = evidence_line(ev)
    {
        ui.label(
            egui::RichText::new(line)
                .small()
                .color(egui::Color32::from_rgb(120, 125, 135)),
        );
    }
}

/// One-line evidence summary for the panel. Returns `None` when the
/// evidence variant has nothing meaningful to render in the ribbon.
fn evidence_line(ev: &rs_cam_core::diagnostics::DiagnosticEvidence) -> Option<String> {
    use rs_cam_core::diagnostics::DiagnosticEvidence as E;
    Some(match ev {
        E::SampleRange {
            sample_start,
            sample_end,
            observed,
            threshold,
            unit,
            locality,
            ..
        } => {
            let thr = threshold
                .map(|t| format!(", threshold {t:.3}"))
                .unwrap_or_default();
            let loc = if locality.is_empty() {
                String::new()
            } else {
                format!(" [{}]", locality.as_str())
            };
            format!(
                "at samples {sample_start}-{sample_end}: observed {observed:.3} {unit}{thr}{loc}"
            )
        }
        E::LutCitation {
            row_id,
            min,
            max,
            observed,
            unit,
            extrapolated,
        } => {
            let min_s = min
                .map(|v| format!("min {v:.3}"))
                .unwrap_or_else(|| "min —".to_owned());
            let max_s = max.map(|m| format!(", max {m:.3}")).unwrap_or_default();
            let ext = if *extrapolated { " (extrapolated)" } else { "" };
            format!("LUT `{row_id}`: observed {observed:.3} {unit} ({min_s}{max_s}){ext}")
        }
        E::GeometryCompare {
            lhs_label,
            lhs_value,
            rhs_label,
            rhs_value,
            unit,
        } => format!("{lhs_label} ({lhs_value:.3} {unit}) vs {rhs_label} ({rhs_value:.3} {unit})"),
        E::Move {
            move_index,
            position,
            ..
        } => {
            if let Some([x, y, z]) = position {
                format!("at move {move_index} ({x:.2}, {y:.2}, {z:.2})")
            } else {
                format!("at move {move_index}")
            }
        }
        E::Counts {
            count,
            offender_toolpath_ids,
        } if *count > 0 => {
            if offender_toolpath_ids.is_empty() {
                format!("{count} occurrences")
            } else {
                let ids: Vec<String> = offender_toolpath_ids
                    .iter()
                    .map(|i| i.to_string())
                    .collect();
                format!("{count} occurrences across toolpaths {}", ids.join(", "))
            }
        }
        E::Counts { .. } => return None,
    })
}

fn confidence_chip_label(c: rs_cam_core::diagnostics::Confidence) -> &'static str {
    use rs_cam_core::diagnostics::Confidence as C;
    match c {
        C::Verified => "(verified)",
        C::Approximate => "(approximate)",
        C::Static => "",
        C::Heuristic => "(heuristic)",
    }
}

fn draw_toolpath_tabs(ui: &mut egui::Ui, active: &mut ToolpathTab, badges: &TabBadges) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for &tab in ToolpathTab::ALL {
            let is_active = *active == tab;
            let (bg, text_color) = if is_active {
                (
                    egui::Color32::from_rgb(55, 60, 80),
                    egui::Color32::from_rgb(220, 225, 240),
                )
            } else {
                (
                    egui::Color32::TRANSPARENT,
                    egui::Color32::from_rgb(140, 140, 155),
                )
            };
            let label = if let Some(badge_color) = badges.for_tab(tab) {
                // Prepend a colored dot
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    "\u{25CF} ",
                    0.0,
                    egui::TextFormat {
                        color: badge_color,
                        font_id: egui::FontId::proportional(8.0),
                        ..Default::default()
                    },
                );
                job.append(
                    tab.label(),
                    0.0,
                    egui::TextFormat {
                        color: text_color,
                        font_id: egui::FontId::proportional(13.0),
                        ..Default::default()
                    },
                );
                egui::WidgetText::LayoutJob(std::sync::Arc::new(job))
            } else {
                egui::RichText::new(tab.label())
                    .color(text_color)
                    .strong()
                    .into()
            };
            let button = egui::Button::new(label)
                .fill(bg)
                .corner_radius(egui::CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                })
                .min_size(egui::vec2(55.0, 24.0));
            let response = ui.add(button);
            if response.clicked() && !is_active {
                *active = tab;
            }
            if is_active {
                let rect = response.rect;
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x + 2.0, rect.max.y),
                        egui::pos2(rect.max.x - 2.0, rect.max.y),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 160, 220)),
                );
            }
            ui.add_space(2.0);
        }
    });
}

/// Build a temporary `ToolpathEntry` from session `ToolpathConfig` + GUI `ToolpathRuntime`.
///
/// This allows the existing `draw_toolpath_panel` to work unchanged while the
/// underlying data migrates from `JobState` to `ProjectSession`.
fn build_entry_from_session_and_gui(
    id: crate::state::toolpath::ToolpathId,
    session: &rs_cam_core::session::ProjectSession,
    gui: &crate::state::runtime::GuiState,
) -> Option<ToolpathEntry> {
    use crate::state::toolpath::ToolpathId;

    let (_, tc) = session.find_toolpath_config_by_id(id.0)?;
    let rt = gui.toolpath_rt.get(&id.0);
    let default_rt = crate::state::runtime::ToolpathRuntime::new(true);
    let rt = rt.unwrap_or(&default_rt);
    Some(ToolpathEntry {
        id: ToolpathId(tc.id),
        name: tc.name.clone(),
        enabled: tc.enabled,
        visible: rt.visible,
        locked: rt.locked,
        tool_id: crate::state::job::ToolId(tc.tool_id),
        model_id: crate::state::job::ModelId(tc.model_id),
        operation: tc.operation.clone(),
        dressups: tc.dressups.clone(),
        heights: tc.heights.clone(),
        boundary: tc.boundary.clone(),
        boundary_inherit: tc.boundary_inherit,
        coolant: tc.coolant,
        pre_gcode: tc.pre_gcode.clone().unwrap_or_default(),
        post_gcode: tc.post_gcode.clone().unwrap_or_default(),
        stock_source: tc.stock_source,
        status: rt.status.clone(),
        result: rt.result.clone(),
        stale_since: rt.stale_since,
        auto_regen: rt.auto_regen,
        face_selection: tc.face_selection.clone(),
        feeds_result: rt.feeds_result.clone(),
        feeds_provenance: tc.feeds_provenance.clone(),
        debug_options: tc.debug_options,
        debug_trace: rt.debug_trace.clone(),
        semantic_trace: rt.semantic_trace.clone(),
        debug_trace_path: rt.debug_trace_path.clone(),
    })
}

/// Write config changes from a `ToolpathEntry` back to the session's `ToolpathConfig`.
fn write_entry_config_to_session(
    entry: &ToolpathEntry,
    session: &mut rs_cam_core::session::ProjectSession,
) {
    if let Some((_, tc)) = session.find_toolpath_config_by_id_mut(entry.id.0) {
        // W2.1: stamp Manual on any feeds dimension the user hand-edited in the
        // param widgets this frame (value moved but provenance didn't). Must run
        // before `tc.operation` / `tc.feeds_provenance` are overwritten below.
        let mut new_provenance = entry.feeds_provenance.clone();
        new_provenance.detect_manual_edits(&tc.operation, &entry.operation, &tc.feeds_provenance);
        tc.name = entry.name.clone();
        tc.enabled = entry.enabled;
        tc.tool_id = entry.tool_id.0;
        tc.model_id = entry.model_id.0;
        tc.operation = entry.operation.clone();
        tc.dressups = entry.dressups.clone();
        tc.heights = entry.heights.clone();
        tc.boundary = entry.boundary.clone();
        tc.boundary_inherit = entry.boundary_inherit;
        tc.coolant = entry.coolant;
        tc.pre_gcode = if entry.pre_gcode.is_empty() {
            None
        } else {
            Some(entry.pre_gcode.clone())
        };
        tc.post_gcode = if entry.post_gcode.is_empty() {
            None
        } else {
            Some(entry.post_gcode.clone())
        };
        tc.stock_source = entry.stock_source;
        tc.face_selection = entry.face_selection.clone();
        tc.debug_options = entry.debug_options;
        tc.feeds_provenance = new_provenance;
    }
}

/// Write runtime changes from a `ToolpathEntry` back to the GUI's `ToolpathRuntime`.
fn write_entry_runtime_to_gui(entry: &ToolpathEntry, gui: &mut crate::state::runtime::GuiState) {
    if let Some(rt) = gui.toolpath_rt.get_mut(&entry.id.0) {
        rt.visible = entry.visible;
        rt.locked = entry.locked;
        rt.auto_regen = entry.auto_regen;
        rt.status = entry.status.clone();
        rt.stale_since = entry.stale_since;
        rt.feeds_result = entry.feeds_result.clone();
        // result, debug_trace, semantic_trace, debug_trace_path are not
        // mutated by the panel — skip to avoid unnecessary Arc clones.
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    tool_configs: &[(crate::state::job::ToolId, crate::state::job::ToolConfig)],
    validation: &ToolpathValidationContext,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    project_default_rpm: u32,
    model_has_enriched: bool,
    model_is_step_missing_brep: bool,
    height_ctx: Option<&HeightContext>,
    stale_default_defects: &[rs_cam_core::compute::validate::StaleDefault],
    load_verdict: Option<&rs_cam_core::tool_load::ToolpathLoadVerdict>,
    events: &mut Vec<AppEvent>,
) {
    ui.heading(&entry.name);
    ui.separator();

    // ── Shared header (always visible above tabs) ───────────────────

    // Name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    ui.separator();

    // Generate button + status (always visible) — promoted to the top
    // (SHE-004) so "is it generated? any warnings?" reads before the geometry
    // combos the user rarely revisits after setup.
    ui.add_space(4.0);
    let validation_errors = validate_toolpath(entry, validation);
    let can_generate = !tools.is_empty() && validation_errors.is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }
        match &entry.status {
            ComputeStatus::Pending => {
                ui.label("Ready");
            }
            ComputeStatus::Computing => {
                ui.label("Computing...");
            }
            ComputeStatus::Done => {
                ui.label(egui::RichText::new("Done").color(egui::Color32::from_rgb(100, 180, 100)));
            }
            ComputeStatus::Error(e) => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Error: {e}"))
                            .color(egui::Color32::from_rgb(220, 80, 80)),
                    )
                    .wrap(),
                )
                .on_hover_text(e);
            }
        }
        if let Some(result) = &entry.result {
            ui.label(
                egui::RichText::new(format!("{} moves", result.stats.move_count))
                    .small()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
    });
    if !validation_errors.is_empty() {
        for err in &validation_errors {
            ui.label(
                egui::RichText::new(err)
                    .color(egui::Color32::from_rgb(220, 150, 60))
                    .small(),
            );
        }
    }

    // Contextual diagnostics (non-blocking). Native `Diagnostic`
    // rendering — splits findings into three tiers:
    //  * Actionable (state Current, severity ≥ Caution) — coloured row
    //    with evidence + confidence + optional Fix button.
    //  * Stateful (NeedsSimulation / StaleEvidence) — neutral grey
    //    "needs current simulation" / "re-run sim" rows.
    //  * Hints (state Current, severity ≤ Hint) — collapsed by default.
    //
    // Load-gate diagnostics (chipload / power / deflection / drill)
    // surface in the same ribbon now that we have `load_verdict`.
    let tool_for_diags = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, tc)| tc);
    let mut diagnostics = operations::collect_diagnostics(
        entry,
        tool_for_diags,
        stale_default_defects,
        height_ctx,
        load_verdict,
    );
    // Auto-regen workflow notice: still a GUI-state-only finding
    // (depends on `entry.auto_regen` + compute state). Synthesise it
    // here so the panel renderer doesn't have to special-case it.
    if !entry.auto_regen && matches!(entry.status, ComputeStatus::Pending) {
        diagnostics.push(rs_cam_core::diagnostics::Diagnostic {
            id: rs_cam_core::diagnostics::DiagnosticId::new("workflow.needs_generation"),
            scope: rs_cam_core::diagnostics::Scope::Toolpath { id: entry.id.0 },
            category: rs_cam_core::diagnostics::Category::State,
            severity: rs_cam_core::diagnostics::Severity::Info,
            confidence: rs_cam_core::diagnostics::Confidence::Static,
            state: rs_cam_core::diagnostics::DiagnosticState::Current,
            source: rs_cam_core::diagnostics::Source::StaticValidation,
            message: "This operation requires manual generation. Press G or click Generate."
                .to_owned(),
            evidence: None,
            fix: None,
            supersedes: Vec::new(),
            suppressed_diagnostics: Vec::new(),
        });
    }

    let mut actionable = Vec::new();
    let mut stateful = Vec::new();
    let mut hints = Vec::new();
    for d in &diagnostics {
        use rs_cam_core::diagnostics::DiagnosticState;
        match d.state {
            DiagnosticState::Current if d.severity.is_actionable() => actionable.push(d),
            DiagnosticState::Current => {
                // Workflow / State category notices render as stateful
                // (neutral) — they don't deserve the yellow tier even
                // though they live in `Current` state.
                if matches!(d.category, rs_cam_core::diagnostics::Category::State) {
                    stateful.push(d);
                } else {
                    hints.push(d);
                }
            }
            DiagnosticState::NeedsSimulation | DiagnosticState::StaleEvidence => stateful.push(d),
            DiagnosticState::NotApplicable => {}
        }
    }

    for d in &actionable {
        render_diagnostic_row(ui, d, RowTier::Actionable, entry, stale_default_defects);
    }
    for d in &stateful {
        render_diagnostic_row(ui, d, RowTier::Stateful, entry, stale_default_defects);
    }
    if !hints.is_empty() {
        egui::CollapsingHeader::new(format!("Hints ({})", hints.len()))
            .default_open(false)
            .show(ui, |ui| {
                for d in &hints {
                    render_diagnostic_row(ui, d, RowTier::Hint, entry, stale_default_defects);
                }
            });
    }

    ui.separator();

    // Geometry wiring (SHE-004): Tool · Input model · Faces · stock source,
    // grouped behind one disclosure. Default-open on a fresh op (no result
    // yet); collapsed once it has a result so settled wiring gets out of the way.
    egui::CollapsingHeader::new("Geometry")
        .default_open(entry.result.is_none())
        .show(ui, |ui| {
            // Tool selector
            ui.horizontal(|ui| {
                ui.label("Tool:");
                let tool_label = tools
                    .iter()
                    .find(|(id, _, _)| *id == entry.tool_id)
                    .map(|(_, s, _)| s.as_str())
                    .unwrap_or("(none)");
                egui::ComboBox::from_id_salt("tp_tool")
                    .selected_text(tool_label)
                    .show_ui(ui, |ui| {
                        for (id, name, _) in tools {
                            ui.selectable_value(&mut entry.tool_id, *id, name.as_str());
                        }
                    });
            });

            // Model selector
            ui.horizontal(|ui| {
                ui.label("Input:");
                let model_label = models
                    .iter()
                    .find(|(id, _)| *id == entry.model_id)
                    .map(|(_, s)| s.as_str())
                    .unwrap_or("(none)");
                egui::ComboBox::from_id_salt("tp_model")
                    .selected_text(model_label)
                    .show_ui(ui, |ui| {
                        for (id, name) in models {
                            ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                        }
                    });
            });

            // BREP-not-loaded warning: surfaces when a STEP model loaded
            // without its enriched mesh (older project files written before
            // the BREP round-trip fix, or an unexpected loader regression).
            // It stays adjacent to the Input model combo it concerns.
            if model_is_step_missing_brep {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "⚠ BREP topology not loaded — face picker unavailable. Reload model.",
                    )
                    .color(egui::Color32::from_rgb(220, 160, 60))
                    .strong(),
                );
            }

            // Face selection (STEP models only)
            if model_has_enriched {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Face Selection")
                        .strong()
                        .color(egui::Color32::from_rgb(180, 180, 195)),
                );
                // SHE-005 — one affordance, not two stacked sentences. Zero
                // selected: a single muted placeholder. ≥1 selected: count +
                // Clear, no tip line. The unconditional "Tip:" sentence is
                // removed — the placeholder + the live count updating as the
                // user clicks convey the action.
                let face_count = entry.face_selection.as_ref().map(|f| f.len()).unwrap_or(0);
                if face_count > 0 {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} face{} selected",
                            face_count,
                            if face_count == 1 { "" } else { "s" }
                        ));
                        if ui.small_button("Clear").clicked() {
                            entry.face_selection = None;
                            entry.stale_since = Some(std::time::Instant::now());
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new("Pick faces in viewport \u{2197}")
                            .color(egui::Color32::from_rgb(120, 120, 130)),
                    );
                }
            }

            // Stock source toggle
            ui.add_space(8.0);
            {
                let mut use_remaining = entry.stock_source == StockSource::FromRemainingStock;
                let resp = ui
                    .checkbox(&mut use_remaining, "Use remaining stock")
                    .on_hover_text(
                        "When enabled, prior operations in this setup are simulated to \
                         determine remaining material. The toolpath will skip air cuts and \
                         adapt to the actual stock state.",
                    );
                if resp.changed() {
                    entry.stock_source = if use_remaining {
                        StockSource::FromRemainingStock
                    } else {
                        StockSource::Fresh
                    };
                    entry.stale_since = Some(std::time::Instant::now());
                }
            }
        });

    // ── Tab bar ─────────────────────────────────────────────────────

    ui.add_space(8.0);
    let tab_id = ui.id().with("tp_tab").with(entry.id.0);
    let mut active_tab: ToolpathTab = ui
        .memory(|mem| mem.data.get_temp(tab_id))
        .unwrap_or(ToolpathTab::Geometry);
    let tab_badges = compute_tab_badges(entry, &diagnostics, height_ctx);
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        ToolpathTab::Geometry => {
            ui.add_space(4.0);

            // Validator-driven Fix banner (PR-2C Phase 1). One row per
            // detected stale-default rule for this TP, with a one-click
            // Fix that mutates the operation directly. Defects are
            // recomputed by the caller each frame, so the banner
            // disappears as soon as the fix takes effect.
            for defect in stale_default_defects {
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(50, 38, 22))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(200, 150, 60),
                    ))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("\u{26A0}")
                                    .color(egui::Color32::from_rgb(220, 180, 60))
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(&defect.title).strong());
                        });
                        ui.label(
                            egui::RichText::new(&defect.detail)
                                .small()
                                .color(egui::Color32::from_rgb(180, 180, 180)),
                        );
                        if ui
                            .small_button(format!("\u{2713} Fix (set to {:.3})", defect.new_value))
                            .on_hover_text(
                                "Apply the validator's auto-fix to this toolpath. \
                                 Mark stale and regenerate to apply.",
                            )
                            .clicked()
                        {
                            rs_cam_core::compute::validate::apply_stale_default_to_op(
                                &mut entry.operation,
                                &mut entry.feeds_provenance,
                                defect,
                            );
                            entry.stale_since = Some(std::time::Instant::now());
                        }
                    });
                ui.add_space(2.0);
            }

            // Compute + cache the LUT feeds result so the per-field ⚡ pills
            // on the Geometry rows (stepover / depth-per-pass) can render.
            // The bulk "Suggest all" button was retired in W3.2 — feed /
            // plunge / RPM and DOC / WOC now apply from the SPEED and CUT
            // sections of the Feeds & Speeds tab (the split-aware applies),
            // and the engine-refusal message surfaces there too.
            if let Some(tool_cfg) = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| t)
            {
                entry.feeds_result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
                    &entry.operation,
                    tool_cfg,
                    material,
                    machine,
                    workholding,
                    rs_cam_core::feeds::embedded_vendor_lut(),
                    spindle_strategy,
                )
                .ok();
            }

            // Operation description from spec (consistent across all operations)
            let spec = entry.operation.op_type().spec();
            ui.label(
                egui::RichText::new(spec.description)
                    .italics()
                    .color(egui::Color32::from_rgb(150, 150, 130)),
            );
            ui.add_space(2.0);
            // PR-2D Phase 2 — pass the cached FeedsResult to every per-op
            // draw so each numeric field can render an inline ⚡ Suggest
            // pill. The Phase 1 block above already computed and cached
            // the result on entry.feeds_result, so this is just a borrow.
            let feeds_for_pills = entry.feeds_result.as_ref();
            match &mut entry.operation {
                OperationConfig::Face(cfg) => draw_face_params(ui, cfg, feeds_for_pills),
                OperationConfig::Pocket(cfg) => draw_pocket_params(ui, cfg, feeds_for_pills),
                OperationConfig::Profile(cfg) => draw_profile_params(ui, cfg, feeds_for_pills),
                OperationConfig::Adaptive(cfg) => draw_adaptive_params(ui, cfg, feeds_for_pills),
                OperationConfig::VCarve(cfg) => draw_vcarve_params(ui, cfg, feeds_for_pills),
                OperationConfig::Rest(cfg) => draw_rest_params(ui, cfg, tools, feeds_for_pills),
                OperationConfig::Inlay(cfg) => draw_inlay_params(ui, cfg, feeds_for_pills),
                OperationConfig::Zigzag(cfg) => draw_zigzag_params(ui, cfg, feeds_for_pills),
                OperationConfig::Trace(cfg) => draw_trace_params(ui, cfg, feeds_for_pills),
                OperationConfig::Drill(cfg) => draw_drill_params(ui, cfg, feeds_for_pills),
                OperationConfig::Chamfer(cfg) => draw_chamfer_params(ui, cfg, feeds_for_pills),
                OperationConfig::DropCutter(cfg) => {
                    draw_dropcutter_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::Adaptive3d(cfg) => {
                    draw_adaptive3d_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::Waterline(cfg) => {
                    draw_waterline_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::Pencil(cfg) => draw_pencil_params(ui, cfg, feeds_for_pills),
                OperationConfig::Scallop(cfg) => draw_scallop_params(ui, cfg, feeds_for_pills),
                OperationConfig::SteepShallow(cfg) => {
                    draw_steep_shallow_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::RampFinish(cfg) => {
                    draw_ramp_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::SpiralFinish(cfg) => {
                    draw_spiral_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::RadialFinish(cfg) => {
                    draw_radial_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::HorizontalFinish(cfg) => {
                    draw_horizontal_finish_params(ui, cfg, feeds_for_pills);
                }
                OperationConfig::ProjectCurve(cfg) => {
                    draw_project_curve_params(ui, cfg, models, feeds_for_pills);
                }
                OperationConfig::AlignmentPinDrill(cfg) => {
                    draw_alignment_pin_drill_params(ui, cfg, feeds_for_pills);
                }
            }

            // Pattern diagrams for all operation types
            ui.add_space(6.0);
            if let Some(pattern) = StepoverPattern::from_operation(&entry.operation) {
                draw_stepover_diagram(ui, &pattern);
            } else {
                match &entry.operation {
                    OperationConfig::Profile(cfg) => {
                        let side = match cfg.side {
                            ProfileSide::Outside => "Outside",
                            ProfileSide::Inside => "Inside",
                        };
                        draw_outline_diagram(ui, &format!("Profile ({side})"), Some(side));
                    }
                    OperationConfig::Chamfer(_) => {
                        draw_outline_diagram(ui, "Chamfer (edge contour)", None);
                    }
                    OperationConfig::Trace(cfg) => {
                        let comp = match cfg.compensation {
                            TraceCompensation::None => None,
                            TraceCompensation::Left => Some("Inside"),
                            TraceCompensation::Right => Some("Outside"),
                        };
                        draw_outline_diagram(ui, "Trace", comp);
                    }
                    OperationConfig::ProjectCurve(_) => {
                        draw_outline_diagram(ui, "Project Curve", None);
                    }
                    OperationConfig::Adaptive(cfg) => {
                        draw_spiral_diagram(ui, cfg.stepover, true);
                    }
                    OperationConfig::Adaptive3d(cfg) => {
                        draw_spiral_diagram(ui, cfg.stepover, true);
                    }
                    OperationConfig::SpiralFinish(cfg) => {
                        let outward = cfg.direction == SpiralDirection::InsideOut;
                        draw_spiral_diagram(ui, cfg.stepover, outward);
                    }
                    OperationConfig::RadialFinish(cfg) => {
                        draw_radial_diagram(ui, cfg.angular_step);
                    }
                    OperationConfig::Drill(_) => {
                        draw_point_set_diagram(ui, "Drill Points");
                    }
                    OperationConfig::AlignmentPinDrill(_) => {
                        draw_point_set_diagram(ui, "Pin Drill Holes");
                    }
                    OperationConfig::Pencil(cfg) => {
                        draw_pencil_diagram(ui, cfg.num_offset_passes, cfg.offset_stepover);
                    }
                    OperationConfig::SteepShallow(cfg) => {
                        draw_steep_shallow_diagram(ui, cfg.threshold_angle);
                    }
                    OperationConfig::RampFinish(cfg) => {
                        draw_ramp_finish_diagram(ui, cfg.max_stepdown);
                    }
                    OperationConfig::Inlay(cfg) => {
                        draw_inlay_diagram(ui, cfg.pocket_depth, cfg.glue_gap, cfg.flat_depth);
                    }
                    _ => {}
                }
            }

            // ── Machining Boundary ─────────────────────────────────────
            // W3.2: the boundary defines *what region to cut*, so it lives
            // under Geometry (was on the old Dressups tab).
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Machining Boundary")
                    .small()
                    .strong()
                    .color(egui::Color32::from_rgb(150, 155, 170)),
            );
            ui.checkbox(&mut entry.boundary.enabled, "Enable boundary")
                .on_hover_text(
                    "Restrict toolpath to a boundary polygon. \
                     Moves outside the boundary are converted to rapids at safe Z.",
                );
            if entry.boundary.enabled {
                ui.checkbox(&mut entry.boundary_inherit, "Inherit from stock")
                    .on_hover_text(
                        "Use the stock-level default boundary. Uncheck to \
                         configure a custom boundary for this toolpath.",
                    );
                if !entry.boundary_inherit {
                    // Source selector
                    ui.horizontal(|ui| {
                        ui.label("Source:");
                        egui::ComboBox::from_id_salt("boundary_source")
                            .selected_text(entry.boundary.source.label())
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        matches!(entry.boundary.source, BoundarySource::Stock),
                                        "Stock",
                                    )
                                    .on_hover_text("Use the stock bounding rectangle")
                                    .clicked()
                                {
                                    entry.boundary.source = BoundarySource::Stock;
                                }
                                if ui
                                    .selectable_label(
                                        matches!(
                                            entry.boundary.source,
                                            BoundarySource::ModelSilhouette
                                        ),
                                        "Model Silhouette",
                                    )
                                    .on_hover_text(
                                        "XY projection of the 3D model. Only machines \
                                         where the model actually is.",
                                    )
                                    .clicked()
                                {
                                    entry.boundary.source = BoundarySource::ModelSilhouette;
                                }
                                if ui
                                    .selectable_label(
                                        matches!(
                                            entry.boundary.source,
                                            BoundarySource::FaceSelection
                                        ),
                                        "Face Selection",
                                    )
                                    .on_hover_text("Boundary derived from selected STEP faces")
                                    .clicked()
                                {
                                    entry.boundary.source = BoundarySource::FaceSelection;
                                }
                            });
                    });

                    // Containment mode
                    ui.horizontal(|ui| {
                        ui.label("Containment:");
                        egui::ComboBox::from_id_salt("boundary_contain")
                            .selected_text(match entry.boundary.containment {
                                BoundaryContainment::Center => "Center",
                                BoundaryContainment::Inside => "Inside",
                                BoundaryContainment::Outside => "Outside",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut entry.boundary.containment,
                                    BoundaryContainment::Center,
                                    "Center",
                                )
                                .on_hover_text("Tool center stays inside boundary");
                                ui.selectable_value(
                                    &mut entry.boundary.containment,
                                    BoundaryContainment::Inside,
                                    "Inside",
                                )
                                .on_hover_text(
                                    "Entire tool stays inside boundary (shrinks by tool radius)",
                                );
                                ui.selectable_value(
                                    &mut entry.boundary.containment,
                                    BoundaryContainment::Outside,
                                    "Outside",
                                )
                                .on_hover_text("Tool edge can extend outside boundary");
                            });
                    });

                    // Offset
                    ui.horizontal(|ui| {
                        ui.label("Offset:");
                        ui.add(
                            egui::DragValue::new(&mut entry.boundary.offset)
                                .speed(0.1)
                                .suffix(" mm"),
                        )
                        .on_hover_text(
                            "Expand (positive) or shrink (negative) the boundary. \
                             Applied before tool-radius containment.",
                        );
                    });
                }
            }
        }

        ToolpathTab::FeedsSpeeds => {
            let tool_info = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| (t.diameter, t.tool_type));
            let (tool_diameter, tool_type) =
                tool_info.unwrap_or((6.0, crate::state::job::ToolType::EndMill));
            // Redesigned modal entry — pulls open the full chart-driven
            // view. Stays alongside the legacy feeds card so existing
            // muscle memory keeps working until the modal is fully
            // promoted (Phase 4).
            ui.horizontal(|ui| {
                if ui
                    .button("\u{1F4CA} Open Feeds & Speeds modal")
                    .on_hover_text(
                        "Open the redesigned Feeds & Speeds view: \
                         current-vs-recommended comparison, three machinist \
                         charts, and Apply buttons.",
                    )
                    .clicked()
                {
                    events.push(AppEvent::OpenFeedsModal(entry.id));
                }
            });
            ui.add_space(4.0);
            if let Some(tool_cfg) = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| t)
            {
                calculate_and_apply_feeds(
                    ui,
                    entry,
                    tool_cfg,
                    material,
                    machine,
                    workholding,
                    spindle_strategy,
                    project_default_rpm,
                );
            }
            if let Some(result) = &entry.feeds_result {
                // Formula breakdown — always visible, the key teaching tool
                ui.add_space(4.0);
                let flute_count = tool_configs
                    .iter()
                    .find(|(id, _)| *id == entry.tool_id)
                    .map(|(_, t)| t.flute_count)
                    .unwrap_or(2);
                let val = egui::Color32::from_rgb(170, 170, 185);
                let font = egui::FontId::proportional(9.5);

                ui.label(egui::RichText::new(format!(
                    "Feed = RPM \u{00D7} flutes \u{00D7} chipload = {:.0} \u{00D7} {} \u{00D7} {:.4} = {:.0} mm/min",
                    result.rpm, flute_count, result.chip_load_mm, result.feed_rate_mm_min
                )).font(font.clone()).color(val));

                ui.label(egui::RichText::new(format!(
                    "MRR = DOC \u{00D7} WOC \u{00D7} Feed = {:.2} \u{00D7} {:.2} \u{00D7} {:.0} = {:.0} mm\u{00B3}/min",
                    result.axial_depth_mm, result.radial_width_mm, result.feed_rate_mm_min, result.mrr_mm3_min
                )).font(font.clone()).color(val));

                ui.label(
                    egui::RichText::new(format!(
                        "Power = MRR \u{00D7} Kc / 60e6 = {:.2} kW (of {:.2} kW available)",
                        result.power_kw, result.available_power_kw
                    ))
                    .font(font.clone())
                    .color(val),
                );

                if result.power_limited {
                    ui.label(
                        egui::RichText::new("Feed was reduced to stay within spindle power")
                            .font(font.clone())
                            .color(egui::Color32::from_rgb(220, 170, 60)),
                    );
                }

                ui.label(
                    egui::RichText::new(format!(
                        "Plunge = {:.0} mm/min ({:.0}% of feed)",
                        result.plunge_rate_mm_min,
                        result.plunge_rate_mm_min / result.feed_rate_mm_min.max(1.0) * 100.0
                    ))
                    .font(font)
                    .color(val),
                );

                // Engagement diagram
                ui.add_space(6.0);
                draw_engagement_diagram(ui, result, tool_diameter, tool_type);
            }

            // Vendor cutting data viewer (always available, filtered by tool)
            draw_vendor_lut_viewer(ui, tool_type, tool_diameter);
        }

        ToolpathTab::Linking => {
            // How moves connect: entry/exit, move optimization, retract
            // strategy (W3.2 — lifted out of the old Dressups tab).
            draw_linking_params(ui, entry, height_ctx);
        }

        ToolpathTab::Heights => {
            let fallback_ctx = HeightContext::simple(10.0, 5.0);
            let ctx = height_ctx.unwrap_or(&fallback_ctx);
            draw_heights_params(ui, &mut entry.heights, ctx);
            ui.add_space(6.0);
            draw_height_diagram(ui, &mut entry.heights, ctx);
        }

        ToolpathTab::Dressup => {
            // Active dressup summary + reset button
            let (active, total) = dressup_active_count(&entry.dressups);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{active}/{total} dressups active"))
                        .small()
                        .color(if active > 0 {
                            egui::Color32::from_rgb(100, 170, 140)
                        } else {
                            egui::Color32::from_rgb(140, 140, 155)
                        }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let role = entry.operation.op_type().spec().ui_process_role;
                    if ui
                        .small_button("Reset to recommended")
                        .on_hover_text(format!(
                            "Apply recommended dressups for {} operations",
                            match role {
                                UiProcessRole::Roughing => "roughing",
                                UiProcessRole::SemiFinish => "semi-finish",
                                UiProcessRole::Finish => "finishing",
                            }
                        ))
                        .clicked()
                    {
                        entry.dressups = DressupConfig::for_role(role);
                    }
                });
            });

            // Edge work / path quality (W3.2 — Entry/Exit, Optimization
            // and Retract moved to the Linking tab; Machining Boundary
            // moved to Geometry).
            draw_dressup_params(ui, &mut entry.dressups);

            // Manual G-code fields (pre_gcode, post_gcode) kept in state
            // for future export wiring — UI removed until export is implemented.
        }
    }
}

// --- Parameter grid helpers ---

fn dv(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    // Delegates to the shared `ValueRow` component (the one labelled-input row).
    let out = ValueRow::new(label, val, suffix, speed, range)
        .tooltip(tooltip_for(label))
        .show(ui);
    record_stock_to_leave(ui, label, &out);
}

/// The "Stock to Leave" UI-automation hook, shared by `dv`/`dv_pill`.
fn record_stock_to_leave(
    ui: &mut egui::Ui,
    label: &str,
    out: &crate::ui::components::ValueRowOutcome,
) {
    if label.trim().trim_end_matches(':') == "Stock to Leave" {
        automation::record(
            ui,
            "properties_stock_to_leave",
            &out.value_response,
            "Stock to Leave",
        );
        automation::record(
            ui,
            "properties_stock_to_leave_label",
            &out.label_response,
            "Stock to Leave",
        );
    }
}

// PR-2D Phase 2 — per-field LUT Suggest pills.
//
// `dv_pill` is `dv` with an optional ⚡ Suggest pill rendered inline to the
// right of the DragValue. Both now delegate to the shared `ValueRow` /
// `SuggestButton` components; pill colour + source wording come from the one
// `ProvKind` vocabulary (green for a vendor LUT row, amber for the formula
// fallback or edge-radius floor) rather than the old local
// `pill_color_for_source` / `source_short_label` helpers.

/// Map the chipload's [`ChiploadSource`](rs_cam_core::feeds::ChiploadSource)
/// into the shared provenance vocabulary (kind + optional observation id).
fn prov_from_chipload(source: &rs_cam_core::feeds::ChiploadSource) -> (ProvKind, Option<&str>) {
    use rs_cam_core::feeds::ChiploadSource;
    match source {
        ChiploadSource::VendorLut { observation_id } => {
            (ProvKind::VendorLut, Some(observation_id.as_str()))
        }
        ChiploadSource::FormulaFallback => (ProvKind::Formula, None),
        ChiploadSource::EdgeRadiusFloor => (ProvKind::EdgeRadiusFloor, None),
    }
}

/// Same as [`dv`] but with an optional inline ⚡ Suggest pill that pushes
/// the LUT-recommended value into the field on click. Delegates to
/// [`ValueRow`] with a [`Suggestion`]; behaviour (near-match greying,
/// suggestion rounding) is identical to the pre-component path.
fn dv_pill(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suggestion: Option<(f64, &rs_cam_core::feeds::ChiploadSource)>,
) -> bool {
    let mut row = ValueRow::new(label, val, suffix, speed, range).tooltip(tooltip_for(label));
    if let Some((rec, source)) = suggestion {
        let (kind, reference) = prov_from_chipload(source);
        row = row.suggest(Suggestion {
            recommended: rec,
            source: kind,
            reference,
        });
    }
    let out = row.show(ui);
    record_stock_to_leave(ui, label, &out);
    out.suggested
}

fn tooltip_for(label: &str) -> Option<&'static str> {
    Some(match label.trim().trim_end_matches(':') {
        "Stepover" => {
            "Distance between passes. 40-60% of diameter for roughing, 10-20% for finishing."
        }
        "Depth" => "Total cut depth from stock surface.",
        "Depth/Pass" | "Depth per Pass" => {
            "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter for large."
        }
        "Feed Rate" => {
            "Cutting speed (mm/min). Wood: 500-2000 for small tools, 1500-4000 for large."
        }
        "Plunge Rate" => "Vertical feed speed (mm/min). Typically 30-50% of feed rate.",
        "Tolerance" => {
            "Geometric tolerance for path approximation. Smaller = more accurate, slower."
        }
        "Min Cut Radius" | "Min Cutting Radius" => {
            "Blend sharp corners with arcs of at least this radius."
        }
        "Wall Stock" => "Material left on walls (radial) for finish pass. 0.2-0.5mm typical.",
        "Floor Stock" => "Material left on floors (axial) for finish pass. 0.2-0.5mm typical.",
        "Stock Top Z" => "Z height of the stock material top surface.",
        "Scallop Height" => "Target cusp height between passes. 0.05-0.2mm for finishing.",
        "Threshold Angle" => "Angle dividing steep (waterline) from shallow (raster) regions.",
        "Max Stepdown" => "Maximum Z step between ramp passes.",
        "Z Step" => "Vertical distance between waterline Z levels.",
        "Sampling" => "XY grid resolution for push-cutter sampling.",
        "Bitangency Angle" => {
            "Minimum dihedral angle to detect concave edges. 140-170 deg typical."
        }
        "Min Cut Length" => "Minimum polyline length to include as a pencil pass.",
        "Hookup Distance" => "Max gap between pencil segments to connect into one pass.",
        "Max Depth" => "Maximum V-carve plunge depth. Limits how deep the V-bit goes.",
        "Glue Gap" => "Gap between male/female inlay pieces for glue. 0.05-0.15mm.",
        "Overlap" | "Overlap Distance" => "Overlap between steep and shallow regions.",
        "Wall Clearance" => "Extra clearance from vertical walls.",
        "Max Distance" => "Max XY distance to keep tool down instead of retracting.",
        "Max Angle" => "Maximum ramp angle from horizontal for entry moves.",
        "Min Z" => "Lowest Z the tool will descend to during drop-cutter.",
        "Angle" => "Zigzag/raster angle in degrees. 0 = along X axis.",
        "Fine Stepdown" => "Optional finer Z step for final passes. 0 = disabled.",
        "Stock Offset" => "Extra distance beyond stock boundary to ensure full coverage.",
        "Chamfer Width" => "Width of the chamfer on the face (mm). Depth computed from tool angle.",
        "Tip Offset" => "Distance from V-bit tip to prevent wear. Increases cut depth slightly.",
        "Peck Depth" => "Incremental depth per peck for chip evacuation.",
        "Dwell Time" => "Pause at bottom of drill hole (seconds).",
        "Retract Amt" => "Small retract distance for chip breaking between pecks.",
        "Retract Z" => {
            "R-plane height for this drill cycle: rapid down to here, then feed into material. Different from global Safe Z."
        }
        "Angular Step" => "Degrees between radial spokes. Smaller = more passes, finer finish.",
        "Point Spacing" => "Distance between sample points along curves. Smaller = smoother.",
        "Angle Threshold" => "Max slope angle (degrees) to consider a surface flat/horizontal.",
        "Stock to Leave" => "Finishing allowance kept on the surface for a later pass.",
        "Slope From" => {
            "Minimum surface slope (degrees) to machine. Faces shallower than this are skipped."
        }
        "Pocket Depth" => "Depth of the inlay pocket measured from stock surface.",
        "Flat Depth" => "Depth for flat-bottom clearing in the inlay pocket. 0 = V-only.",
        "Boundary Offset" => "Offset from the design boundary for the inlay cut. Adjusts fit.",
        "Flat Tool Radius" => "Radius of the flat endmill used to clear the pocket floor.",
        "Spoilboard" => "How far the drill penetrates into the spoilboard below the stock.",
        "Width" => "Width of holding tabs that keep the part attached to stock.",
        "Height" => "Height of holding tabs from the floor of the cut.",
        "Offset Stepover" => "Lateral step between offset cleanup passes around pencil traces.",
        "Pitch" => "Vertical drop per revolution of the helical entry move.",
        "Radius" => "Radius of the helical or arc entry/exit move.",
        "Max Rate" => "Maximum allowable feed rate during optimized sections.",
        "Ramp Rate" => "How quickly feed rate ramps up toward max (mm/min per mm of engagement).",
        "Slope To" => "Maximum surface slope (degrees) to machine. Steeper faces are skipped.",
        "Finishing Passes" => "Spring passes at final depth for dimensional accuracy.",
        "Offset Passes" => "Number of parallel offset passes around pencil traces.",
        "Count" => "Number of holding tabs placed around the profile perimeter.",
        "Continuous" => "Connect passes into a single continuous toolpath without retract.",
        "Direction" => "Cutting direction for this operation.",
        _ => return None,
    })
}

// ── Dressup configuration ────────────────────────────────────────────────

/// Count how many dressup features are currently active.
fn dressup_active_count(cfg: &DressupConfig) -> (usize, usize) {
    let total = 8;
    let mut active = 0;
    if !matches!(cfg.entry_style, DressupEntryStyle::None) {
        active += 1;
    }
    if cfg.lead_in_out {
        active += 1;
    }
    if cfg.dogbone {
        active += 1;
    }
    if cfg.arc_fitting {
        active += 1;
    }
    if cfg.link_moves {
        active += 1;
    }
    if cfg.feed_optimization {
        active += 1;
    }
    if cfg.optimize_rapid_order {
        active += 1;
    }
    if matches!(cfg.retract_strategy, RetractStrategy::Minimum) {
        active += 1;
    }
    (active, total)
}

/// Linking tab (W3.2): how moves connect — Entry & Exit, Move Optimization,
/// and Retract strategy. Split out of the old monolithic dressup panel; the
/// remaining edge-work (arc fitting / dogbone) stays in [`draw_dressup_params`].
fn draw_linking_params(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    height_ctx: Option<&HeightContext>,
) {
    // Some dressups are geometrically incompatible with specific operations
    // (project_curve traces many small rings; drop_cutter 3D finish emits
    // hundreds of raster segments — a ramp entry / lead-in / link-move at
    // each one produces diagonal trenches carving through the stock). Grey
    // those controls out so the UI reflects what compute actually does.
    // Read from the SAME registry policy `DressupConfig::normalize_for_op`
    // applies (Phase 1 T5) — no hand-synced duplicate table.
    let dressup_policy = entry.operation.op_type().registry_entry().dressup_policy;
    let op_incompatible_msg: Option<&str> = dressup_policy.strip_all_reason;
    // W1.2/P2-003 — adaptive3d (and other planner-emitted / single-pass
    // ops) coerce the dressup entry style to None via
    // EntryStylePolicy::ForceNone, independently of strip_all_reason. The
    // entry combo was gated only on the latter, so it stayed editable while
    // compute silently ignored it. Gate the entry combo on the entry policy
    // too (lead-in/out and link moves are unaffected by ForceNone, so they
    // keep using op_incompatible_msg).
    let entry_disabled_msg: Option<&str> = op_incompatible_msg.or_else(|| {
        matches!(
            dressup_policy.entry,
            rs_cam_core::compute::catalog::EntryStylePolicy::ForceNone
        )
        .then_some("This operation sets its entry move directly — the dressup entry style isn't used here.")
    });
    let cfg = &mut entry.dressups;
    let section_color = egui::Color32::from_rgb(150, 155, 170);

    // ── Entry & Exit ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Entry & Exit")
            .small()
            .strong()
            .color(section_color),
    );

    ui.horizontal(|ui| {
        ui.label("Entry Style:");
        ui.add_enabled_ui(entry_disabled_msg.is_none(), |ui| {
            let combo = egui::ComboBox::from_id_salt("dressup_entry")
                .selected_text(match cfg.entry_style {
                    DressupEntryStyle::None => "None",
                    DressupEntryStyle::Ramp => "Ramp",
                    DressupEntryStyle::Helix => "Helix",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::None, "None")
                        .on_hover_text("Plunge straight down. Fast but can burn in hard materials.");
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Ramp, "Ramp")
                        .on_hover_text("Angled descent into material. Prevents plunge burns. Recommended for most operations.");
                    ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Helix, "Helix")
                        .on_hover_text("Spiral descent. Best for deep pockets and hard materials. Spreads heat and load evenly.");
                });
            if let Some(msg) = entry_disabled_msg {
                combo.response.on_hover_text(msg);
            }
        });
    });
    match cfg.entry_style {
        DressupEntryStyle::Ramp => {
            egui::Grid::new("ramp_p")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    dv(
                        ui,
                        "  Max Angle:",
                        &mut cfg.ramp_angle,
                        " deg",
                        0.5,
                        0.5..=15.0,
                    );
                });
        }
        DressupEntryStyle::Helix => {
            egui::Grid::new("helix_p")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    dv(
                        ui,
                        "  Radius:",
                        &mut cfg.helix_radius,
                        " mm",
                        0.1,
                        0.5..=20.0,
                    );
                    dv(ui, "  Pitch:", &mut cfg.helix_pitch, " mm", 0.1, 0.2..=10.0);
                });
        }
        DressupEntryStyle::None => {}
    }
    {
        let fallback_ctx = HeightContext::simple(10.0, 5.0);
        let ctx = height_ctx.unwrap_or(&fallback_ctx);
        ui.add_space(4.0);
        draw_entry_preview_diagram(ui, cfg, ctx, &entry.heights);
    }

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        let resp = ui
            .checkbox(&mut cfg.lead_in_out, "Lead-in / lead-out")
            .on_hover_text("Add smooth arc transitions at cut start and end. Prevents tool marks at entry/exit points. Best for finishing and profile cuts.");
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if cfg.lead_in_out && op_incompatible_msg.is_none() {
        egui::Grid::new("lead_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Radius:",
                    &mut cfg.lead_radius,
                    " mm",
                    0.1,
                    0.5..=20.0,
                );
            });
        draw_lead_in_out_diagram(ui, cfg.lead_radius);
    }

    ui.add_space(6.0);

    // ── Optimization ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Optimization")
            .small()
            .strong()
            .color(section_color),
    );

    ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
        let resp = ui
            .checkbox(&mut cfg.link_moves, "Link moves (keep tool down)")
            .on_hover_text("Replace short retract-rapid-plunge sequences with slow linear feeds. Major time saver for operations with many small regions. Keeps the tool in the material instead of retracting.");
        if let Some(msg) = op_incompatible_msg {
            resp.on_hover_text(msg);
        }
    });
    if cfg.link_moves && op_incompatible_msg.is_none() {
        egui::Grid::new("link_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Distance:",
                    &mut cfg.link_max_distance,
                    " mm",
                    0.5,
                    1.0..=50.0,
                );
                dv(
                    ui,
                    "  Feed Rate:",
                    &mut cfg.link_feed_rate,
                    " mm/min",
                    10.0,
                    50.0..=5000.0,
                );
            });
    }

    let feed_opt_reason = crate::state::toolpath::feed_optimization_unavailable_reason(
        &entry.operation,
        entry.stock_source,
    );
    if let Some(reason) = feed_opt_reason {
        cfg.feed_optimization = false;
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut cfg.feed_optimization, "Feed rate optimization"),
        )
        .on_hover_text(reason);
        ui.label(
            egui::RichText::new(reason)
                .small()
                .italics()
                .color(egui::Color32::from_rgb(150, 150, 130)),
        );
    } else {
        ui.checkbox(&mut cfg.feed_optimization, "Feed rate optimization")
            .on_hover_text("Dynamically adjust feed rate based on stock engagement. Higher feed in light cuts, lower in heavy cuts. Only available for fresh-stock 2D operations.");
    }
    if cfg.feed_optimization && feed_opt_reason.is_none() {
        egui::Grid::new("fopt_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Rate:",
                    &mut cfg.feed_max_rate,
                    " mm/min",
                    50.0,
                    500.0..=20000.0,
                );
                dv(
                    ui,
                    "  Ramp Rate:",
                    &mut cfg.feed_ramp_rate,
                    " mm/min/mm",
                    10.0,
                    10.0..=2000.0,
                );
            });
    }

    ui.checkbox(&mut cfg.optimize_rapid_order, "Optimize rapid travel order")
        .on_hover_text("Reorder disconnected toolpath segments to minimize total rapid travel distance (TSP heuristic). Pure optimization with no machining risk.");

    ui.add_space(6.0);

    // ── Safety ────────────────────────────────────────────────
    ui.label(
        egui::RichText::new("Safety")
            .small()
            .strong()
            .color(section_color),
    );

    ui.horizontal(|ui| {
        ui.label("Retract Strategy:");
        egui::ComboBox::from_id_salt("retract_strat")
            .selected_text(match cfg.retract_strategy {
                RetractStrategy::Full => "Full",
                RetractStrategy::Minimum => "Minimum",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.retract_strategy, RetractStrategy::Full, "Full")
                    .on_hover_text("Always retract to retract height between cuts. Safest option \u{2014} use when unsure.");
                ui.selectable_value(&mut cfg.retract_strategy, RetractStrategy::Minimum, "Minimum")
                    .on_hover_text("Retract just above nearby path. Faster cycle time but risk of collision if geometry is complex.");
            });
    });
}

/// Dressup tab (W3.2): edge work / path quality — arc fitting and dogbone
/// overcuts. Entry/exit + optimization + retract live on the Linking tab
/// ([`draw_linking_params`]); the machining boundary lives on Geometry.
fn draw_dressup_params(ui: &mut egui::Ui, cfg: &mut DressupConfig) {
    let section_color = egui::Color32::from_rgb(150, 155, 170);

    // ── Path Quality ──────────────────────────────────────────
    ui.label(
        egui::RichText::new("Path Quality")
            .small()
            .strong()
            .color(section_color),
    );

    ui.checkbox(&mut cfg.arc_fitting, "Arc fitting (G2/G3)")
        .on_hover_text("Convert sequences of linear segments into smooth G2/G3 arcs. Reduces file size, improves surface finish, and produces smoother machine motion. Safe for all operations.");
    if cfg.arc_fitting {
        egui::Grid::new("arc_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Tolerance:",
                    &mut cfg.arc_tolerance,
                    " mm",
                    0.01,
                    0.01..=0.5,
                );
            });
    }

    ui.checkbox(&mut cfg.dogbone, "Dogbone overcuts")
        .on_hover_text("Add circular overcuts at inside corners so parts fit together. Essential for joints, inlays, and press-fit assemblies. Not needed for open pockets or 3D surfaces.");
    if cfg.dogbone {
        egui::Grid::new("dog_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                dv(
                    ui,
                    "  Max Angle:",
                    &mut cfg.dogbone_angle,
                    " deg",
                    1.0,
                    45.0..=135.0,
                );
            });
        draw_dogbone_diagram(ui, cfg.dogbone_angle);
    }
}
