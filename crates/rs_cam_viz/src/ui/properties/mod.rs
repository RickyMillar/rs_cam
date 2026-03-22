mod operations;
pub mod post;
pub mod setup;
pub mod stock;
pub mod tool;

pub use operations::validate_toolpath;
use operations::*;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::*;
use crate::ui::AppEvent;
use crate::ui::automation;

/// Global embedded vendor LUT, loaded once on first access.
static VENDOR_LUT: std::sync::LazyLock<rs_cam_core::feeds::vendor_lut::VendorLut> =
    std::sync::LazyLock::new(rs_cam_core::feeds::vendor_lut::VendorLut::embedded);

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
    if let Some((tp_id, old_op, old_dressups)) = state.history.toolpath_snapshot.take() {
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
                    });
                state.job.mark_edited();
            }
        } else {
            state.history.toolpath_snapshot = Some((tp_id, old_op, old_dressups));
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
            ui.label(
                egui::RichText::new("Select an item in the project tree")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
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
            if events.iter().any(|e| matches!(e, AppEvent::StockChanged))
                && let Some(old) = state.history.stock_snapshot.take()
                && (old.x != state.job.stock.x
                    || old.y != state.job.stock.y
                    || old.z != state.job.stock.z
                    || old.origin_x != state.job.stock.origin_x
                    || old.origin_y != state.job.stock.origin_y
                    || old.origin_z != state.job.stock.origin_z
                    || old.padding != state.job.stock.padding)
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
                setup::draw(ui, setup_id, setup_state, pin_count, has_flip_axis, events);
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
            ui.label(format!("Model: {:?}", model_id));
            ui.label(format!("Face: {}", face_id.0));
            if let Some(model) = state.job.models.iter().find(|m| m.id == model_id) {
                if let Some(enriched) = &model.enriched_mesh {
                    if let Some(group) = enriched.face_group(face_id) {
                        ui.label(format!("Surface type: {:?}", group.surface_type));
                        ui.label(format!("Triangles: {}", group.triangle_range.len()));
                    }
                }
            }
        }
        Selection::Faces(model_id, ref face_ids) => {
            ui.heading("Faces Selected");
            ui.separator();
            ui.label(format!("Model: {:?}", model_id));
            ui.label(format!("{} faces selected", face_ids.len()));
        }
        Selection::Toolpath(id) => {
            // Capture snapshot for undo before editing
            if state.history.toolpath_snapshot.is_none()
                && let Some(entry) = state.job.find_toolpath(id)
            {
                state.history.toolpath_snapshot =
                    Some((id, entry.operation.clone(), entry.dressups.clone()));
            }
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state
                .job
                .tools
                .iter()
                .map(|t| (t.id, t.summary(), t.diameter))
                .collect();
            let models: Vec<_> = state
                .job
                .models
                .iter()
                .map(|m| (m.id, m.name.clone()))
                .collect();
            // Snapshot tool configs for feeds calculation
            let tool_configs: Vec<_> = state.job.tools.iter().map(|t| (t.id, t.clone())).collect();
            let material = state.job.stock.material.clone();
            let machine = state.job.machine.clone();
            let workholding = state.job.stock.workholding_rigidity;

            // Snapshot operation for stale_since detection
            let op_before = state
                .job
                .find_toolpath(id)
                .map(|e| serde_json::to_string(&e.operation).unwrap_or_default());

            if let Some(entry) = state.job.find_toolpath_mut(id) {
                draw_toolpath_panel(
                    ui,
                    entry,
                    &tools,
                    &models,
                    &tool_configs,
                    &material,
                    &machine,
                    workholding,
                    events,
                );
            }

            // B3a: set stale_since when parameters change
            if let Some(before) = op_before
                && let Some(entry) = state.job.find_toolpath_mut(id)
            {
                let after = serde_json::to_string(&entry.operation).unwrap_or_default();
                if before != after {
                    entry.stale_since = Some(std::time::Instant::now());
                    state.job.mark_edited();
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
    use crate::state::job::{ModelKind, ModelUnits};

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
                    if planes > 0 { parts.push(format!("{planes} plane")); }
                    if cylinders > 0 { parts.push(format!("{cylinders} cyl")); }
                    if other > 0 { parts.push(format!("{other} other")); }
                    ui.label(parts.join(", "));
                    ui.end_row();
                });
        }

        // Units / scale selector (STL only)
        if model.kind == ModelKind::Stl {
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

/// Map ToolType + ToolConfig to ToolGeometryHint for the feeds calculator.
fn tool_geometry_hint(
    tool: &crate::state::job::ToolConfig,
) -> rs_cam_core::feeds::ToolGeometryHint {
    use crate::state::job::ToolType;
    use rs_cam_core::feeds::ToolGeometryHint;
    match tool.tool_type {
        ToolType::EndMill => ToolGeometryHint::Flat,
        ToolType::BallNose => ToolGeometryHint::Ball,
        ToolType::BullNose => ToolGeometryHint::Bull {
            corner_radius: tool.corner_radius,
        },
        ToolType::VBit => ToolGeometryHint::VBit {
            included_angle: tool.included_angle,
            tip_diameter: 0.2,
        },
        ToolType::TaperedBallNose => ToolGeometryHint::TaperedBall {
            tip_radius: tool.diameter / 2.0,
            taper_angle_deg: tool.taper_half_angle,
        },
    }
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

#[allow(clippy::too_many_arguments)]
fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    tool_configs: &[(crate::state::job::ToolId, crate::state::job::ToolConfig)],
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    workholding: rs_cam_core::feeds::WorkholdingRigidity,
    events: &mut Vec<AppEvent>,
) {
    ui.heading(&entry.name);
    ui.separator();

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

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Cutting Parameters")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    // Operation-specific parameters
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
        OperationConfig::ProjectCurve(cfg) => draw_project_curve_params(ui, cfg),
        OperationConfig::AlignmentPinDrill(cfg) => draw_alignment_pin_drill_params(ui, cfg),
    }

    // --- Feeds & Speeds calculation ---
    if let Some(tool_cfg) = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, t)| t)
    {
        calculate_and_apply_feeds(ui, entry, tool_cfg, material, machine, workholding);
    }

    // Machining boundary
    ui.add_space(8.0);
    ui.collapsing("Machining Boundary", |ui| {
        ui.checkbox(&mut entry.boundary_enabled, "Clip to stock boundary")
            .on_hover_text("Restrict toolpath to within the stock material bounds");
        if entry.boundary_enabled {
            ui.horizontal(|ui| {
                ui.label("Containment:");
                egui::ComboBox::from_id_salt("boundary_contain")
                    .selected_text(match entry.boundary_containment {
                        BoundaryContainment::Center => "Center",
                        BoundaryContainment::Inside => "Inside",
                        BoundaryContainment::Outside => "Outside",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut entry.boundary_containment,
                            BoundaryContainment::Center,
                            "Center",
                        )
                        .on_hover_text("Tool center stays inside boundary");
                        ui.selectable_value(
                            &mut entry.boundary_containment,
                            BoundaryContainment::Inside,
                            "Inside",
                        )
                        .on_hover_text(
                            "Entire tool stays inside boundary (shrinks by tool radius)",
                        );
                        ui.selectable_value(
                            &mut entry.boundary_containment,
                            BoundaryContainment::Outside,
                            "Outside",
                        )
                        .on_hover_text("Tool edge can extend outside boundary");
                    });
            });
        }
    });

    ui.add_space(4.0);
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

    // Manual G-code
    ui.add_space(4.0);
    ui.collapsing("Manual G-code", |ui| {
        ui.label(
            egui::RichText::new("Raw G-code inserted before/after this operation in export")
                .small()
                .italics()
                .color(egui::Color32::from_rgb(130, 130, 140)),
        );
        ui.label("Before:");
        ui.text_edit_multiline(&mut entry.pre_gcode);
        ui.label("After:");
        ui.text_edit_multiline(&mut entry.post_gcode);
    });

    // Heights
    ui.add_space(4.0);
    ui.collapsing("Heights", |ui| {
        draw_heights_params(ui, &mut entry.heights);
    });

    // Dressup modifications
    ui.add_space(4.0);
    ui.collapsing("Modifications", |ui| {
        draw_dressup_params(ui, entry);
    });

    ui.add_space(12.0);

    // Validation
    let validation_errors = validate_toolpath(entry, tools);
    if !validation_errors.is_empty() {
        ui.add_space(4.0);
        for err in &validation_errors {
            ui.label(
                egui::RichText::new(err)
                    .color(egui::Color32::from_rgb(220, 150, 60))
                    .small(),
            );
        }
    }

    // Generate button + status
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
                ui.label(
                    egui::RichText::new(format!("Error: {e}"))
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                );
            }
        }
    });

    if let Some(result) = &entry.result {
        ui.add_space(4.0);
        ui.label(format!("Moves: {}", result.stats.move_count));
        ui.label(format!("Cutting: {:.0} mm", result.stats.cutting_distance));
        ui.label(format!("Rapid: {:.0} mm", result.stats.rapid_distance));
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
        "Retract Z" => "R-plane height: rapid down to here, then feed into material.",
        "Angular Step" => "Degrees between radial spokes. Smaller = more passes, finer finish.",
        "Point Spacing" => "Distance between sample points along curves. Smaller = smoother.",
        "Angle Threshold" => "Max slope angle (degrees) to consider a surface flat/horizontal.",
        "Stock to Leave" => "Finishing allowance kept on the surface for a later pass.",
        _ => return None,
    })
}

// ── Dressup configuration ────────────────────────────────────────────────

fn draw_dressup_params(ui: &mut egui::Ui, entry: &mut ToolpathEntry) {
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
