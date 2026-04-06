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
pub use operations::{ToolpathValidationContext, collect_warnings, validate_toolpath};

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig, DressupEntryStyle,
    HeightContext, HeightsConfig, OperationConfig, ProfileSide, RetractStrategy, SpiralDirection,
    StockSource, ToolpathEntry, TraceCompensation,
};
use crate::ui::AppEvent;
use crate::ui::automation;

/// Global embedded vendor LUT, loaded once on first access.
static VENDOR_LUT: std::sync::LazyLock<rs_cam_core::feeds::VendorLut> =
    std::sync::LazyLock::new(rs_cam_core::feeds::VendorLut::embedded);

/// Flush tool undo snapshot if the user navigated away from a tool.
fn flush_tool_snapshot(state: &mut AppState) {
    if let Some((tool_id, old)) = state.history.tool_snapshot.take() {
        if !matches!(state.selection, crate::state::selection::Selection::Tool(id) if id == tool_id)
        {
            if let Some(current) = state.job.tools.iter().find(|t| t.id == tool_id) {
                state
                    .history
                    .push(crate::state::history::UndoAction::ToolChange {
                        tool_id,
                        old,
                        new: current.clone(),
                    });
                state.job.mark_edited();
            }
        } else {
            // Still editing — put the snapshot back.
            state.history.tool_snapshot = Some((tool_id, old));
        }
    }
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
                    new: state.job.post.clone(),
                });
            state.job.mark_edited();
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
                    new: state.job.machine.clone(),
                });
            state.job.mark_edited();
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
            if let Some(entry) = state.job.find_toolpath(tp_id) {
                state
                    .history
                    .push(crate::state::history::UndoAction::ToolpathParamChange {
                        tp_id,
                        old_op,
                        new_op: entry.operation.clone(),
                        old_dressups,
                        new_dressups: entry.dressups.clone(),
                        old_face_selection: old_faces,
                        new_face_selection: entry.face_selection.clone(),
                    });
                state.job.mark_edited();
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
    flush_tool_snapshot(state);
    flush_post_snapshot(state);
    flush_machine_snapshot(state);
    flush_toolpath_snapshot(state);

    match state.selection.clone() {
        Selection::None => {
            if state.job.models.is_empty()
                && state.job.setups.iter().all(|s| s.toolpaths.is_empty())
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
                state.history.stock_snapshot = Some(state.job.stock.clone());
            }
            let has_flipped_setup = state
                .job
                .setups
                .iter()
                .any(|s| s.face_up != crate::state::job::FaceUp::Top);
            stock::draw(ui, &mut state.job.stock, has_flipped_setup, events);
            // If an edit just finished (DragValue released), push undo
            if events
                .iter()
                .any(|e| matches!(e, AppEvent::StockChanged | AppEvent::StockMaterialChanged))
                && let Some(old) = state.history.stock_snapshot.take()
                && old != state.job.stock
            {
                state
                    .history
                    .push(crate::state::history::UndoAction::StockChange {
                        old,
                        new: state.job.stock.clone(),
                    });
            }
        }
        Selection::PostProcessor => {
            // Capture snapshot for undo before editing
            if state.history.post_snapshot.is_none() {
                state.history.post_snapshot = Some(state.job.post.clone());
            }
            post::draw(ui, &mut state.job.post);
        }
        Selection::Machine => {
            // Capture snapshot for undo before editing
            if state.history.machine_snapshot.is_none() {
                state.history.machine_snapshot = Some(state.job.machine.clone());
            }
            draw_machine_panel(ui, state, events);
        }
        Selection::Model(id) => {
            draw_model_properties(ui, id, state, events);
        }
        Selection::Tool(id) => {
            // Capture snapshot for undo before editing
            if state.history.tool_snapshot.is_none()
                && let Some(t) = state.job.tools.iter().find(|t| t.id == id)
            {
                state.history.tool_snapshot = Some((id, t.clone()));
            }
            if let Some(t) = state.job.tools.iter_mut().find(|t| t.id == id) {
                tool::draw(ui, t);
            }
        }
        Selection::Setup(setup_id) => {
            if let Some(setup_state) = state
                .job
                .setups
                .iter_mut()
                .find(|setup| setup.id == setup_id)
            {
                let pin_count = state.job.stock.alignment_pins.len();
                let has_flip_axis = state.job.stock.flip_axis.is_some();
                let all_models: Vec<_> = state
                    .job
                    .models
                    .iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect();
                setup::draw(
                    ui,
                    setup_id,
                    setup_state,
                    pin_count,
                    has_flip_axis,
                    &all_models,
                    events,
                );
            }
        }
        Selection::Fixture(setup_id, fixture_id) => {
            if let Some(setup_state) = state
                .job
                .setups
                .iter_mut()
                .find(|setup| setup.id == setup_id)
                && let Some(fixture) = setup_state
                    .fixtures
                    .iter_mut()
                    .find(|fixture| fixture.id == fixture_id)
            {
                setup::draw_fixture_properties(ui, setup_id, fixture, events);
            }
        }
        Selection::KeepOut(setup_id, keep_out_id) => {
            if let Some(setup_state) = state
                .job
                .setups
                .iter_mut()
                .find(|setup| setup.id == setup_id)
                && let Some(zone) = setup_state
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
                .job
                .models
                .iter()
                .find(|m| m.id == model_id)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("Face: {}", face_id.0));
            if let Some(model) = state.job.models.iter().find(|m| m.id == model_id)
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
                .job
                .models
                .iter()
                .find(|m| m.id == model_id)
                .map(|m| m.name.as_str())
                .unwrap_or("Unknown");
            ui.label(format!("Model: {model_name}"));
            ui.label(format!("{} faces selected", face_ids.len()));
        }
        Selection::Toolpath(id) => {
            // Capture snapshot for undo before editing
            if state.history.toolpath_snapshot.is_none()
                && let Some(entry) = state.job.find_toolpath(id)
            {
                state.history.toolpath_snapshot = Some((
                    id,
                    entry.operation.clone(),
                    entry.dressups.clone(),
                    entry.face_selection.clone(),
                ));
            }
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state
                .job
                .tools
                .iter()
                .map(|t| (t.id, t.summary(), t.diameter))
                .collect();
            // Filter models by setup's model_ids (empty = all).
            let setup_for_tp = state
                .job
                .setups
                .iter()
                .find(|s| s.toolpaths.iter().any(|t| t.id == id));
            let models: Vec<_> = if let Some(setup) = setup_for_tp {
                setup
                    .available_models(&state.job.models)
                    .into_iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect()
            } else {
                state
                    .job
                    .models
                    .iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect()
            };
            // Snapshot tool configs for feeds calculation
            let tool_configs: Vec<_> = state.job.tools.iter().map(|t| (t.id, t.clone())).collect();
            let validation = ToolpathValidationContext::from_job(&state.job);
            let material = state.job.stock.material.clone();
            let machine = state.job.machine.clone();
            let workholding = state.job.stock.workholding_rigidity;

            // Check if the toolpath's model has enriched mesh (for face selection UI)
            let model_has_enriched = state
                .job
                .find_toolpath(id)
                .and_then(|tp| state.job.models.iter().find(|m| m.id == tp.model_id))
                .map(|m| m.enriched_mesh.is_some())
                .unwrap_or(false);

            // Snapshot height context before mutable borrow (needs stock + model bbox)
            let height_ctx = state
                .job
                .find_toolpath(id)
                .map(|tp| state.job.height_context_for(tp));

            // Snapshot operation and heights for stale_since detection
            let op_before = state
                .job
                .find_toolpath(id)
                .map(|e| serde_json::to_string(&e.operation).unwrap_or_default());
            let heights_before = state
                .job
                .find_toolpath(id)
                .map(|e| format!("{:?}", e.heights));

            if let Some(entry) = state.job.find_toolpath_mut(id) {
                draw_toolpath_panel(
                    ui,
                    entry,
                    &tools,
                    &models,
                    &tool_configs,
                    &validation,
                    &material,
                    &machine,
                    workholding,
                    model_has_enriched,
                    height_ctx.as_ref(),
                    events,
                );
            }

            // B3a: set stale_since when parameters or heights change
            if let Some(entry) = state.job.find_toolpath_mut(id) {
                let op_changed = op_before.as_ref().is_some_and(|b| {
                    *b != serde_json::to_string(&entry.operation).unwrap_or_default()
                });
                let heights_changed = heights_before
                    .as_ref()
                    .is_some_and(|b| *b != format!("{:?}", entry.heights));
                if op_changed || heights_changed {
                    entry.stale_since = Some(std::time::Instant::now());
                    state.job.mark_edited();
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

    let Some(model) = state.job.models.iter().find(|m| m.id == id) else {
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

            let current_units = model.units;
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

// SAFETY: selected_idx from position() within presets; i from enumerate over presets
#[allow(clippy::indexing_slicing)]
fn draw_machine_panel(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Machine Setup");
    ui.separator();

    let presets = rs_cam_core::machine::MachineProfile::presets();
    let current_key = state.job.machine.to_key();
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
                        state.job.machine = presets[i].1.clone();
                        events.push(AppEvent::MachineChanged);
                    }
                }
            });
    });

    ui.add_space(8.0);

    // Show machine specs (read-only)
    egui::Grid::new("machine_specs")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            let (min_rpm, max_rpm) = state.job.machine.rpm_range();
            ui.label("RPM Range:");
            ui.label(format!("{:.0} - {:.0}", min_rpm, max_rpm));
            ui.end_row();

            let max_power = match state.job.machine.power {
                rs_cam_core::machine::PowerModel::VfdConstantTorque { rated_power_kw, .. } => {
                    rated_power_kw
                }
                rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => power_kw,
            };
            ui.label("Power:");
            ui.label(format!("{:.2} kW", max_power));
            ui.end_row();

            ui.label("Max Feed:");
            ui.label(format!("{:.0} mm/min", state.job.machine.max_feed_mm_min));
            ui.end_row();

            ui.label("Max Shank:");
            ui.label(format!("{:.1} mm", state.job.machine.max_shank_mm));
            ui.end_row();
        });

    ui.add_space(8.0);

    // Safety factor / aggressiveness slider
    ui.horizontal(|ui| {
        ui.label("Aggressiveness:");
        if ui
            .add(
                egui::Slider::new(&mut state.job.machine.safety_factor, 0.60..=0.95)
                    .text("")
                    .show_value(true),
            )
            .changed()
        {
            events.push(AppEvent::MachineChanged);
        }
    });
    ui.label(
        egui::RichText::new(if state.job.machine.safety_factor < 0.72 {
            "Conservative — safer for new setups"
        } else if state.job.machine.safety_factor > 0.85 {
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
        let rigidity = &mut state.job.stock.workholding_rigidity;
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
        state.job.mark_edited();
    }
    ui.label(
        egui::RichText::new(match state.job.stock.workholding_rigidity {
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
fn operation_to_feeds_family(
    op: &OperationConfig,
) -> (
    rs_cam_core::feeds::OperationFamily,
    rs_cam_core::feeds::PassRole,
) {
    op.feeds_style()
}

/// Derive ToolGeometryHint from the tool's own geometry_hint() method.
fn tool_geometry_hint(
    tool: &crate::state::job::ToolConfig,
) -> rs_cam_core::feeds::ToolGeometryHint {
    crate::compute::worker::helpers::build_cutter(tool).to_geometry_hint()
}

/// Extract operation-specific parameter hints for the feeds calculator.
/// Returns (axial_depth_hint, radial_width_hint, scallop_hint).
fn operation_feeds_hints(op: &OperationConfig) -> (Option<f64>, Option<f64>, Option<f64>) {
    match op {
        // Scallop: scallop_height drives stepover for ball tools
        OperationConfig::Scallop(cfg) => (None, None, Some(cfg.scallop_height)),
        // Waterline: z_step is the axial slice height
        OperationConfig::Waterline(cfg) => (Some(cfg.z_step), None, None),
        // SteepShallow: z_step for the steep (waterline) portion
        OperationConfig::SteepShallow(cfg) => (Some(cfg.z_step), None, None),
        // VCarve: max_depth hints the axial depth
        OperationConfig::VCarve(cfg) => (Some(cfg.max_depth), None, None),
        // RampFinish: max_stepdown is the axial depth
        OperationConfig::RampFinish(cfg) => (Some(cfg.max_stepdown), None, None),
        // All others: let the calculator use defaults
        _ => (None, None, None),
    }
}

/// Run feeds calculation, auto-write into operation, and draw the feeds card.
fn calculate_and_apply_feeds(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
) {
    let has_any_auto = entry.feeds_auto.feed_rate
        || entry.feeds_auto.plunge_rate
        || entry.feeds_auto.stepover
        || entry.feeds_auto.depth_per_pass;

    if !has_any_auto {
        // Only draw the card if we have a cached result
        if entry.feeds_result.is_some() {
            draw_feeds_card(ui, entry);
        }
        return;
    }

    let (family, role) = operation_to_feeds_family(&entry.operation);

    // Extract operation-specific hints for the calculator
    let (axial_hint, radial_hint, scallop_hint) = operation_feeds_hints(&entry.operation);

    let input = rs_cam_core::feeds::FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: tool_geometry_hint(tool),
        material,
        machine,
        operation: family,
        pass_role: role,
        axial_depth_mm: axial_hint,
        radial_width_mm: radial_hint,
        target_scallop_mm: scallop_hint,
        vendor_lut: Some(&*VENDOR_LUT),
        setup: rs_cam_core::feeds::SetupContext {
            tool_overhang_mm: Some(tool.stickout),
            workholding_rigidity: workholding,
        },
    };

    let result = rs_cam_core::feeds::calculate(&input);

    // Auto-write calculated values into the operation config
    if entry.feeds_auto.feed_rate {
        entry.operation.set_feed_rate(result.feed_rate_mm_min);
    }
    if entry.feeds_auto.plunge_rate {
        entry.operation.set_plunge_rate(result.plunge_rate_mm_min);
    }
    if entry.feeds_auto.stepover {
        entry.operation.set_stepover(result.radial_width_mm);
    }
    if entry.feeds_auto.depth_per_pass {
        entry.operation.set_depth_per_pass(result.axial_depth_mm);
    }

    entry.feeds_result = Some(result);
    draw_feeds_card(ui, entry);
}

fn draw_feeds_card(ui: &mut egui::Ui, entry: &ToolpathEntry) {
    ui.add_space(8.0);
    ui.collapsing("Feeds & Speeds", |ui| {
        if let Some(result) = &entry.feeds_result {
            egui::Grid::new("feeds_card")
                .num_columns(2)
                .spacing([8.0, 3.0])
                .show(ui, |ui| {
                    ui.label("RPM:");
                    ui.label(format!("{:.0}", result.rpm));
                    ui.end_row();
                    ui.label("Chip Load:");
                    ui.label(format!("{:.4} mm/tooth", result.chip_load_mm));
                    ui.end_row();
                    ui.label("Feed:");
                    ui.label(format!("{:.0} mm/min", result.feed_rate_mm_min));
                    ui.end_row();
                    ui.label("Plunge:");
                    ui.label(format!("{:.0} mm/min", result.plunge_rate_mm_min));
                    ui.end_row();
                    ui.label("DOC:");
                    ui.label(format!("{:.2} mm", result.axial_depth_mm));
                    ui.end_row();
                    ui.label("WOC:");
                    ui.label(format!("{:.2} mm", result.radial_width_mm));
                    ui.end_row();

                    // Power bar
                    ui.label("Power:");
                    let frac = if result.available_power_kw > 0.0 {
                        (result.power_kw / result.available_power_kw).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let color = if frac > 0.9 {
                        egui::Color32::from_rgb(220, 80, 80)
                    } else if frac > 0.7 {
                        egui::Color32::from_rgb(220, 180, 60)
                    } else {
                        egui::Color32::from_rgb(80, 180, 80)
                    };
                    ui.horizontal(|ui| {
                        let bar = egui::ProgressBar::new(frac as f32)
                            .fill(color)
                            .desired_width(100.0);
                        ui.add(bar);
                        ui.label(format!(
                            "{:.2}/{:.2}kW",
                            result.power_kw, result.available_power_kw
                        ));
                    });
                    ui.end_row();

                    ui.label("MRR:");
                    ui.label(format!("{:.0} mm\u{00B3}/min", result.mrr_mm3_min));
                    ui.end_row();
                });

            // Vendor source
            if let Some(src) = &result.vendor_source {
                ui.label(
                    egui::RichText::new(format!("Source: {src}"))
                        .small()
                        .color(egui::Color32::from_rgb(100, 160, 200)),
                );
            }

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
#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolpathTab {
    Params,
    Feeds,
    Heights,
    Mods,
}

impl ToolpathTab {
    const ALL: &[ToolpathTab] = &[
        ToolpathTab::Params,
        ToolpathTab::Feeds,
        ToolpathTab::Heights,
        ToolpathTab::Mods,
    ];

    fn label(self) -> &'static str {
        match self {
            ToolpathTab::Params => "Params",
            ToolpathTab::Feeds => "Feeds",
            ToolpathTab::Heights => "Heights",
            ToolpathTab::Mods => "Mods",
        }
    }
}

/// Per-tab badge state for the tab bar.
struct TabBadges {
    feeds_badge: Option<egui::Color32>,
    heights_badge: Option<egui::Color32>,
    mods_badge: Option<egui::Color32>,
}

impl TabBadges {
    fn for_tab(&self, tab: ToolpathTab) -> Option<egui::Color32> {
        match tab {
            ToolpathTab::Feeds => self.feeds_badge,
            ToolpathTab::Heights => self.heights_badge,
            ToolpathTab::Mods => self.mods_badge,
            ToolpathTab::Params => None,
        }
    }
}

fn compute_tab_badges(entry: &ToolpathEntry, warnings: &[operations::OperationWarning], height_ctx: Option<&HeightContext>) -> TabBadges {
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

    // Mods: badge if warnings mention tool-operation issues (from collect_warnings)
    let mods_badge = if warnings.iter().any(|w| w.severity == operations::WarningSeverity::Error) {
        Some(egui::Color32::from_rgb(220, 100, 80))
    } else if warnings.iter().any(|w| w.severity == operations::WarningSeverity::Warning) {
        Some(egui::Color32::from_rgb(220, 180, 60))
    } else {
        None
    };

    TabBadges { feeds_badge, heights_badge, mods_badge }
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
                egui::WidgetText::LayoutJob(job)
            } else {
                egui::RichText::new(tab.label()).color(text_color).strong().into()
            };
            let button = egui::Button::new(label)
                .fill(bg)
                .rounding(egui::Rounding {
                    nw: 4.0,
                    ne: 4.0,
                    sw: 0.0,
                    se: 0.0,
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
    model_has_enriched: bool,
    height_ctx: Option<&HeightContext>,
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

    // Face selection (STEP models only)
    if model_has_enriched {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Face Selection")
                .strong()
                .color(egui::Color32::from_rgb(180, 180, 195)),
        );
        let face_count = entry.face_selection.as_ref().map(|f| f.len()).unwrap_or(0);
        if face_count > 0 {
            ui.label(format!(
                "{} face{} selected",
                face_count,
                if face_count == 1 { "" } else { "s" }
            ));
            if ui.small_button("Clear Faces").clicked() {
                entry.face_selection = None;
                entry.stale_since = Some(std::time::Instant::now());
            }
        } else {
            ui.label(
                egui::RichText::new("Click faces in viewport to select")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        ui.label(
            egui::RichText::new("Tip: click faces in the 3D view while this toolpath is selected")
                .small()
                .color(egui::Color32::from_rgb(100, 100, 110)),
        );
    }

    // Stock source toggle
    ui.add_space(8.0);
    {
        let mut use_remaining = entry.stock_source == StockSource::FromRemainingStock;
        let resp = ui
            .checkbox(&mut use_remaining, "Use remaining stock")
            .on_hover_text(
                "When enabled, prior operations in this setup are simulated to determine \
                 remaining material. The toolpath will skip air cuts and adapt to the \
                 actual stock state.",
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

    // Generate button + status (always visible)
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

    // Contextual warnings (non-blocking)
    let warnings = collect_warnings(entry, validation, height_ctx);
    if !warnings.is_empty() {
        for w in &warnings {
            ui.label(egui::RichText::new(&w.message).small().color(w.severity.color()));
        }
    }

    // ── Tab bar ─────────────────────────────────────────────────────

    ui.add_space(8.0);
    let tab_id = ui.id().with("tp_tab").with(entry.id.0);
    let mut active_tab: ToolpathTab = ui
        .memory(|mem| mem.data.get_temp(tab_id))
        .unwrap_or(ToolpathTab::Params);
    let tab_badges = compute_tab_badges(entry, &warnings, height_ctx);
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        ToolpathTab::Params => {
            // Auto-feeds toggles — show which parameters are auto-calculated
            let auto = &mut entry.feeds_auto;
            let has_any_auto =
                auto.feed_rate || auto.plunge_rate || auto.stepover || auto.depth_per_pass;
            let auto_label = if has_any_auto {
                "Auto Feeds (on)"
            } else {
                "Auto Feeds (off)"
            };
            let auto_color = if has_any_auto {
                egui::Color32::from_rgb(100, 180, 100)
            } else {
                egui::Color32::from_rgb(140, 140, 155)
            };
            ui.collapsing(
                egui::RichText::new(auto_label).small().color(auto_color),
                |ui| {
                    ui.label(
                        egui::RichText::new(
                            "When on, values are computed from tool/material/machine",
                        )
                        .small()
                        .italics()
                        .color(egui::Color32::from_rgb(110, 110, 120)),
                    );
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut auto.feed_rate, "Feed");
                        ui.checkbox(&mut auto.plunge_rate, "Plunge");
                        ui.checkbox(&mut auto.stepover, "Stepover");
                        ui.checkbox(&mut auto.depth_per_pass, "DOC");
                    });
                    if has_any_auto {
                        ui.label(
                            egui::RichText::new(
                                "Auto values shown in Feeds tab; manual edits below are overridden",
                            )
                            .small()
                            .color(egui::Color32::from_rgb(220, 170, 60)),
                        );
                    }
                },
            );

            // Compact auto-feed summary (when auto-feeds are active)
            if has_any_auto
                && let Some(result) = &entry.feeds_result
            {
                ui.label(
                    egui::RichText::new(format!(
                        "Auto: {:.0} mm/min feed, {:.0} RPM",
                        result.feed_rate_mm_min, result.rpm,
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(100, 170, 140)),
                );
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Cutting Parameters")
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 195)),
            );
            match &mut entry.operation {
                OperationConfig::Face(cfg) => draw_face_params(ui, cfg),
                OperationConfig::Pocket(cfg) => draw_pocket_params(ui, cfg),
                OperationConfig::Profile(cfg) => draw_profile_params(ui, cfg),
                OperationConfig::Adaptive(cfg) => draw_adaptive_params(ui, cfg),
                OperationConfig::VCarve(cfg) => draw_vcarve_params(ui, cfg),
                OperationConfig::Rest(cfg) => draw_rest_params(ui, cfg, tools),
                OperationConfig::Inlay(cfg) => draw_inlay_params(ui, cfg),
                OperationConfig::Zigzag(cfg) => draw_zigzag_params(ui, cfg),
                OperationConfig::Trace(cfg) => draw_trace_params(ui, cfg),
                OperationConfig::Drill(cfg) => draw_drill_params(ui, cfg),
                OperationConfig::Chamfer(cfg) => draw_chamfer_params(ui, cfg),
                OperationConfig::DropCutter(cfg) => draw_dropcutter_params(ui, cfg),
                OperationConfig::Adaptive3d(cfg) => draw_adaptive3d_params(ui, cfg),
                OperationConfig::Waterline(cfg) => draw_waterline_params(ui, cfg),
                OperationConfig::Pencil(cfg) => draw_pencil_params(ui, cfg),
                OperationConfig::Scallop(cfg) => draw_scallop_params(ui, cfg),
                OperationConfig::SteepShallow(cfg) => draw_steep_shallow_params(ui, cfg),
                OperationConfig::RampFinish(cfg) => draw_ramp_finish_params(ui, cfg),
                OperationConfig::SpiralFinish(cfg) => draw_spiral_finish_params(ui, cfg),
                OperationConfig::RadialFinish(cfg) => draw_radial_finish_params(ui, cfg),
                OperationConfig::HorizontalFinish(cfg) => draw_horizontal_finish_params(ui, cfg),
                OperationConfig::ProjectCurve(cfg) => {
                    draw_project_curve_params(ui, cfg, models);
                }
                OperationConfig::AlignmentPinDrill(cfg) => {
                    draw_alignment_pin_drill_params(ui, cfg);
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
        }

        ToolpathTab::Feeds => {
            let tool_info = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| (t.diameter, t.tool_type));
            let (tool_diameter, tool_type) =
                tool_info.unwrap_or((6.0, crate::state::job::ToolType::EndMill));
            if let Some(tool_cfg) = tool_configs
                .iter()
                .find(|(id, _)| *id == entry.tool_id)
                .map(|(_, t)| t)
            {
                calculate_and_apply_feeds(ui, entry, tool_cfg, material, machine, workholding);
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

                ui.label(egui::RichText::new(format!(
                    "Power = MRR \u{00D7} Kc / 60e6 = {:.2} kW (of {:.2} kW available)",
                    result.power_kw, result.available_power_kw
                )).font(font.clone()).color(val));

                if result.power_limited {
                    ui.label(egui::RichText::new(
                        "Feed was reduced to stay within spindle power"
                    ).font(font.clone()).color(egui::Color32::from_rgb(220, 170, 60)));
                }

                ui.label(egui::RichText::new(format!(
                    "Plunge = {:.0} mm/min ({:.0}% of feed)",
                    result.plunge_rate_mm_min,
                    result.plunge_rate_mm_min / result.feed_rate_mm_min.max(1.0) * 100.0
                )).font(font).color(val));

                // Engagement diagram
                ui.add_space(6.0);
                draw_engagement_diagram(ui, result, tool_diameter, tool_type);
            }
        }

        ToolpathTab::Heights => {
            let fallback_ctx = HeightContext::simple(10.0, 5.0);
            let ctx = height_ctx.unwrap_or(&fallback_ctx);
            draw_heights_params(ui, &mut entry.heights, ctx);
            ui.add_space(6.0);
            draw_height_diagram(ui, &mut entry.heights, ctx);
        }

        ToolpathTab::Mods => {
            // --- Machining Boundary ---
            ui.separator();
            ui.label("Machining Boundary");
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

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Modifications")
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 195)),
            );
            draw_dressup_params(ui, entry, height_ctx);

            ui.add_space(8.0);
            ui.collapsing("Debugging", |ui| {
                ui.checkbox(&mut entry.debug_options.enabled, "Capture debug trace")
                    .on_hover_text(
                        "Record semantic and performance trace data for the Simulation debugger. Re-generate to apply changes.",
                    );
                if entry.debug_options.enabled {
                    ui.label(
                        egui::RichText::new("Re-generate this toolpath to refresh trace data.")
                            .small()
                            .color(egui::Color32::from_rgb(140, 170, 230)),
                    );
                }
            });

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
    let label_resp = ui.label(label);
    let resp = ui.add(
        egui::DragValue::new(val)
            .suffix(suffix)
            .speed(speed)
            .range(range),
    );
    if label.trim().trim_end_matches(':') == "Stock to Leave" {
        automation::record(ui, "properties_stock_to_leave", &resp, "Stock to Leave");
        automation::record(
            ui,
            "properties_stock_to_leave_label",
            &label_resp,
            "Stock to Leave",
        );
    }
    if let Some(tip) = tooltip_for(label) {
        resp.on_hover_text(tip);
    }
    ui.end_row();
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
        _ => return None,
    })
}

// ── Dressup configuration ────────────────────────────────────────────────

fn draw_dressup_params(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    height_ctx: Option<&HeightContext>,
) {
    let cfg = &mut entry.dressups;
    ui.horizontal(|ui| {
        ui.label("Entry Style:");
        egui::ComboBox::from_id_salt("dressup_entry")
            .selected_text(match cfg.entry_style {
                DressupEntryStyle::None => "None",
                DressupEntryStyle::Ramp => "Ramp",
                DressupEntryStyle::Helix => "Helix",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::None, "None");
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Ramp, "Ramp");
                ui.selectable_value(&mut cfg.entry_style, DressupEntryStyle::Helix, "Helix");
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
    // Entry style preview — bundled right after the settings that control it
    {
        let fallback_ctx = HeightContext::simple(10.0, 5.0);
        let ctx = height_ctx.unwrap_or(&fallback_ctx);
        ui.add_space(4.0);
        draw_entry_preview_diagram(ui, cfg, ctx, &entry.heights);
    }

    ui.add_space(4.0);
    ui.checkbox(&mut cfg.dogbone, "Dogbone overcuts");
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
    ui.checkbox(&mut cfg.lead_in_out, "Lead-in / lead-out");
    if cfg.lead_in_out {
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
    ui.checkbox(&mut cfg.link_moves, "Link moves (keep tool down)");
    if cfg.link_moves {
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
    ui.checkbox(&mut cfg.arc_fitting, "Arc fitting (G2/G3)");
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
        ui.checkbox(&mut cfg.feed_optimization, "Feed rate optimization");
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
        .on_hover_text(
            "Reorder toolpath segments to minimize rapid travel distance (TSP optimization)",
        );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Retract Strategy:");
        egui::ComboBox::from_id_salt("retract_strat")
            .selected_text(match cfg.retract_strategy {
                RetractStrategy::Full => "Full",
                RetractStrategy::Minimum => "Minimum",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut cfg.retract_strategy, RetractStrategy::Full, "Full")
                    .on_hover_text("Always retract to retract height (safest)");
                ui.selectable_value(
                    &mut cfg.retract_strategy,
                    RetractStrategy::Minimum,
                    "Minimum",
                )
                .on_hover_text("Retract just above nearby path (faster, less safe)");
            });
    });
}
