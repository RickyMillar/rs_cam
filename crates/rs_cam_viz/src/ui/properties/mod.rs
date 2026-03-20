pub mod stock;
pub mod tool;
pub mod post;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::*;
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    // When simulation is active, show simulation panel instead of normal properties
    if state.simulation.active {
        draw_simulation_panel(ui, state, events);
        return;
    }

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
            stock::draw(ui, &mut state.job.stock, events);
            // If an edit just finished (DragValue released), push undo
            if events.iter().any(|e| matches!(e, AppEvent::StockChanged)) {
                if let Some(old) = state.history.stock_snapshot.take() {
                    if old.x != state.job.stock.x || old.y != state.job.stock.y || old.z != state.job.stock.z
                        || old.origin_x != state.job.stock.origin_x || old.origin_y != state.job.stock.origin_y
                        || old.origin_z != state.job.stock.origin_z || old.padding != state.job.stock.padding
                    {
                        state.history.push(crate::state::history::UndoAction::StockChange {
                            old,
                            new: state.job.stock.clone(),
                        });
                    }
                }
            }
        }
        Selection::PostProcessor => {
            post::draw(ui, &mut state.job.post);
        }
        Selection::Machine => {
            draw_machine_panel(ui, state, events);
        }
        Selection::Model(id) => {
            draw_model_properties(ui, id, state, events);
        }
        Selection::Tool(id) => {
            if let Some(t) = state.job.tools.iter_mut().find(|t| t.id == id) {
                tool::draw(ui, t);
            }
        }
        Selection::Toolpath(id) => {
            // Snapshot tool/model lists to avoid borrow conflict with toolpaths
            let tools: Vec<_> = state.job.tools.iter().map(|t| (t.id, t.summary(), t.diameter)).collect();
            let models: Vec<_> = state.job.models.iter().map(|m| (m.id, m.name.clone())).collect();
            // Snapshot tool configs for feeds calculation
            let tool_configs: Vec<_> = state.job.tools.iter().map(|t| (t.id, t.clone())).collect();
            let material = state.job.stock.material.clone();
            let machine = state.job.machine.clone();

            if let Some(entry) = state.job.toolpaths.iter_mut().find(|t| t.id == id) {
                draw_toolpath_panel(ui, entry, &tools, &models, &tool_configs, &material, &machine, events);
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
                ui.label(format!("{:.3} mm  ({:.3} to {:.3})", dx, bb.min.x, bb.max.x));
                ui.end_row();
                ui.label("Y:");
                ui.label(format!("{:.3} mm  ({:.3} to {:.3})", dy, bb.min.y, bb.max.y));
                ui.end_row();
                ui.label("Z:");
                ui.label(format!("{:.3} mm  ({:.3} to {:.3})", dz, bb.min.z, bb.max.z));
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
        if let Some(report) = &model.winding_report {
            if *report > 1.0 {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "\u{26A0} {:.1}% inconsistent normals detected (auto-fixed on load)",
                        report
                    ))
                    .color(egui::Color32::from_rgb(220, 190, 60)),
                );
            }
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

    for (i, boundary) in state.simulation.boundaries.iter().enumerate() {
        let pc = crate::render::toolpath_render::palette_color(i);
        let color = egui::Color32::from_rgb(
            (pc[0] * 255.0) as u8,
            (pc[1] * 255.0) as u8,
            (pc[2] * 255.0) as u8,
        );
        let is_current = state.simulation.current_boundary()
            .map(|b| b.id == boundary.id)
            .unwrap_or(false);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("\u{25CF}").color(color));
            let text = if is_current {
                egui::RichText::new(&boundary.name).strong().color(egui::Color32::WHITE)
            } else {
                egui::RichText::new(&boundary.name).color(egui::Color32::from_rgb(180, 180, 190))
            };
            ui.label(text);
            ui.label(
                egui::RichText::new(&boundary.tool_name)
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 140)),
            );
        });

        // Progress bar for this toolpath
        let progress = if state.simulation.current_move >= boundary.end_move {
            1.0
        } else if state.simulation.current_move <= boundary.start_move {
            0.0
        } else {
            (state.simulation.current_move - boundary.start_move) as f32
                / (boundary.end_move - boundary.start_move).max(1) as f32
        };
        let bar = egui::ProgressBar::new(progress)
            .fill(color)
            .desired_width(ui.available_width() - 16.0);
        ui.add(bar);

        // Jump-to-boundary button
        let boundary_start = boundary.start_move;
        ui.horizontal(|ui| {
            if ui.small_button("Jump to start").clicked() {
                state.simulation.current_move = boundary_start;
                state.simulation.playing = false;
            }
        });

        ui.add_space(2.0);
    }

    ui.add_space(8.0);

    // Tool position readout
    if let Some(pos) = state.simulation.tool_position {
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

fn draw_machine_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _events: &mut Vec<AppEvent>,
) {
    ui.heading("Machine Setup");
    ui.separator();

    let presets = rs_cam_core::machine::MachineProfile::presets();
    let current_key = state.job.machine.to_key();
    let mut selected_idx = presets.iter().position(|(_, p)| p.to_key() == current_key).unwrap_or(0);

    ui.horizontal(|ui| {
        ui.label("Preset:");
        egui::ComboBox::from_id_salt("machine_preset")
            .selected_text(presets[selected_idx].0)
            .show_ui(ui, |ui| {
                for (i, (label, _)) in presets.iter().enumerate() {
                    if ui.selectable_value(&mut selected_idx, i, *label).changed() {
                        state.job.machine = presets[i].1.clone();
                        state.job.dirty = true;
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
                rs_cam_core::machine::PowerModel::VfdConstantTorque { rated_power_kw, .. } => rated_power_kw,
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
        if ui.add(
            egui::Slider::new(&mut state.job.machine.safety_factor, 0.60..=0.95)
                .text("")
                .show_value(true),
        ).changed() {
            state.job.dirty = true;
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
}

/// Map OperationConfig variant to (OperationFamily, PassRole) for the feeds calculator.
fn operation_to_feeds_family(op: &OperationConfig) -> (rs_cam_core::feeds::OperationFamily, rs_cam_core::feeds::PassRole) {
    use rs_cam_core::feeds::{OperationFamily as OF, PassRole as PR};
    match op {
        OperationConfig::Pocket(_) => (OF::Pocket, PR::Roughing),
        OperationConfig::Profile(_) => (OF::Contour, PR::Roughing),
        OperationConfig::Adaptive(_) | OperationConfig::Adaptive3d(_) => (OF::Adaptive, PR::Roughing),
        OperationConfig::VCarve(_) | OperationConfig::Inlay(_) => (OF::Trace, PR::Finish),
        OperationConfig::DropCutter(_) => (OF::Parallel, PR::Finish),
        OperationConfig::Scallop(_) => (OF::Scallop, PR::Finish),
        OperationConfig::Pencil(_) => (OF::Trace, PR::Finish),
        OperationConfig::Waterline(_) => (OF::Contour, PR::SemiFinish),
        OperationConfig::SteepShallow(_) => (OF::Contour, PR::Finish),
        OperationConfig::RampFinish(_) => (OF::Parallel, PR::Finish),
        OperationConfig::Zigzag(_) | OperationConfig::Rest(_) => (OF::Pocket, PR::Roughing),
    }
}

/// Map ToolType + ToolConfig to ToolGeometryHint for the feeds calculator.
fn tool_geometry_hint(tool: &crate::state::job::ToolConfig) -> rs_cam_core::feeds::ToolGeometryHint {
    use crate::state::job::ToolType;
    use rs_cam_core::feeds::ToolGeometryHint;
    match tool.tool_type {
        ToolType::EndMill => ToolGeometryHint::Flat,
        ToolType::BallNose => ToolGeometryHint::Ball,
        ToolType::BullNose => ToolGeometryHint::Bull { corner_radius: tool.corner_radius },
        ToolType::VBit => ToolGeometryHint::VBit { included_angle: tool.included_angle, tip_diameter: 0.2 },
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
            egui::Grid::new("feeds_card").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
                ui.label("RPM:"); ui.label(format!("{:.0}", result.rpm)); ui.end_row();
                ui.label("Chip Load:"); ui.label(format!("{:.4} mm/tooth", result.chip_load_mm)); ui.end_row();
                ui.label("Feed:"); ui.label(format!("{:.0} mm/min", result.feed_rate_mm_min)); ui.end_row();
                ui.label("Plunge:"); ui.label(format!("{:.0} mm/min", result.plunge_rate_mm_min)); ui.end_row();
                ui.label("DOC:"); ui.label(format!("{:.2} mm", result.axial_depth_mm)); ui.end_row();
                ui.label("WOC:"); ui.label(format!("{:.2} mm", result.radial_width_mm)); ui.end_row();

                // Power bar
                ui.label("Power:");
                let frac = if result.available_power_kw > 0.0 {
                    (result.power_kw / result.available_power_kw).clamp(0.0, 1.0)
                } else { 0.0 };
                let color = if frac > 0.9 { egui::Color32::from_rgb(220, 80, 80) }
                           else if frac > 0.7 { egui::Color32::from_rgb(220, 180, 60) }
                           else { egui::Color32::from_rgb(80, 180, 80) };
                ui.horizontal(|ui| {
                    let bar = egui::ProgressBar::new(frac as f32).fill(color).desired_width(100.0);
                    ui.add(bar);
                    ui.label(format!("{:.2}/{:.2}kW", result.power_kw, result.available_power_kw));
                });
                ui.end_row();

                ui.label("MRR:"); ui.label(format!("{:.0} mm\u{00B3}/min", result.mrr_mm3_min)); ui.end_row();
            });

            // Warnings
            for w in &result.warnings {
                let text = match w {
                    rs_cam_core::feeds::FeedsWarning::FeedRateClamped { requested, actual } =>
                        format!("Feed clamped: {requested:.0} -> {actual:.0} mm/min (machine limit)"),
                    rs_cam_core::feeds::FeedsWarning::PowerLimited { required_kw, available_kw } =>
                        format!("Power limited: {required_kw:.2}kW needed, {available_kw:.2}kW available"),
                    rs_cam_core::feeds::FeedsWarning::DocExceedsFlute { requested, capped } =>
                        format!("DOC capped: {requested:.1} -> {capped:.1}mm (flute guard)"),
                    rs_cam_core::feeds::FeedsWarning::SlottingDetected { doc_reduced_to } =>
                        format!("Slotting detected: DOC reduced to {doc_reduced_to:.1}mm"),
                    rs_cam_core::feeds::FeedsWarning::ScallopInvalid { target, max_possible } =>
                        format!("Invalid scallop: {target:.3}mm (max {max_possible:.1}mm)"),
                    rs_cam_core::feeds::FeedsWarning::ShankTooLarge { shank_mm, max_mm } =>
                        format!("Shank {shank_mm:.1}mm exceeds max {max_mm:.1}mm"),
                };
                ui.label(egui::RichText::new(format!("! {text}"))
                    .small().color(egui::Color32::from_rgb(220, 170, 60)));
            }
        }
    });
}

fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
    models: &[(crate::state::job::ModelId, String)],
    tool_configs: &[(crate::state::job::ToolId, crate::state::job::ToolConfig)],
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
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
        let tool_label = tools.iter().find(|(id, _, _)| *id == entry.tool_id)
            .map(|(_, s, _)| s.as_str()).unwrap_or("(none)");
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
        let model_label = models.iter().find(|(id, _)| *id == entry.model_id)
            .map(|(_, s)| s.as_str()).unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_model")
            .selected_text(model_label)
            .show_ui(ui, |ui| {
                for (id, name) in models {
                    ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                }
            });
    });

    // Stock source selector (Phase C)
    ui.horizontal(|ui| {
        ui.label("Stock Source:");
        egui::ComboBox::from_id_salt("tp_stock_src")
            .selected_text(match entry.stock_source {
                StockSource::Fresh => "Fresh Stock",
                StockSource::FromRemainingStock => "Remaining Stock",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut entry.stock_source, StockSource::Fresh, "Fresh Stock");
                ui.selectable_value(&mut entry.stock_source, StockSource::FromRemainingStock, "Remaining Stock (after prior operations)");
            });
    });
    if entry.stock_source == StockSource::FromRemainingStock {
        ui.label(
            egui::RichText::new("Will simulate prior toolpaths to determine starting material.")
                .small()
                .italics()
                .color(egui::Color32::from_rgb(150, 150, 130)),
        );
    }

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Cutting Parameters")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    // Operation-specific parameters
    match &mut entry.operation {
        OperationConfig::Pocket(cfg) => draw_pocket_params(ui, cfg),
        OperationConfig::Profile(cfg) => draw_profile_params(ui, cfg),
        OperationConfig::Adaptive(cfg) => draw_adaptive_params(ui, cfg),
        OperationConfig::VCarve(cfg) => draw_vcarve_params(ui, cfg),
        OperationConfig::Rest(cfg) => draw_rest_params(ui, cfg, tools),
        OperationConfig::Inlay(cfg) => draw_inlay_params(ui, cfg),
        OperationConfig::Zigzag(cfg) => draw_zigzag_params(ui, cfg),
        OperationConfig::DropCutter(cfg) => draw_dropcutter_params(ui, cfg),
        OperationConfig::Adaptive3d(cfg) => draw_adaptive3d_params(ui, cfg),
        OperationConfig::Waterline(cfg) => draw_waterline_params(ui, cfg),
        OperationConfig::Pencil(cfg) => draw_pencil_params(ui, cfg),
        OperationConfig::Scallop(cfg) => draw_scallop_params(ui, cfg),
        OperationConfig::SteepShallow(cfg) => draw_steep_shallow_params(ui, cfg),
        OperationConfig::RampFinish(cfg) => draw_ramp_finish_params(ui, cfg),
    }

    // --- Feeds & Speeds calculation ---
    if let Some(tool_cfg) = tool_configs.iter().find(|(id, _)| *id == entry.tool_id).map(|(_, t)| t) {
        calculate_and_apply_feeds(ui, entry, tool_cfg, material, machine);
    }

    // Dressup modifications
    ui.add_space(8.0);
    ui.collapsing("Modifications", |ui| {
        draw_dressup_params(ui, &mut entry.dressups);
    });

    ui.add_space(12.0);

    // Validation
    let validation_errors = validate_toolpath(entry, tools);
    if !validation_errors.is_empty() {
        ui.add_space(4.0);
        for err in &validation_errors {
            ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(220, 150, 60)).small());
        }
    }

    // Generate button + status
    let can_generate = !tools.is_empty() && validation_errors.is_empty();
    ui.horizontal(|ui| {
        if ui.add_enabled(can_generate, egui::Button::new("Generate")).clicked() {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }
        match &entry.status {
            ComputeStatus::Pending => { ui.label("Ready"); }
            ComputeStatus::Computing(p) => { ui.label(format!("Computing... {:.0}%", p * 100.0)); }
            ComputeStatus::Done => {
                ui.label(egui::RichText::new("Done").color(egui::Color32::from_rgb(100, 180, 100)));
            }
            ComputeStatus::Error(e) => {
                ui.label(egui::RichText::new(format!("Error: {e}")).color(egui::Color32::from_rgb(220, 80, 80)));
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

fn dv(ui: &mut egui::Ui, label: &str, val: &mut f64, suffix: &str, speed: f64, range: std::ops::RangeInclusive<f64>) {
    ui.label(label);
    let resp = ui.add(egui::DragValue::new(val).suffix(suffix).speed(speed).range(range));
    if let Some(tip) = tooltip_for(label) {
        resp.on_hover_text(tip);
    }
    ui.end_row();
}

fn tooltip_for(label: &str) -> Option<&'static str> {
    Some(match label.trim().trim_end_matches(':') {
        "Stepover" => "Distance between passes. 40-60% of diameter for roughing, 10-20% for finishing.",
        "Depth" => "Total cut depth from stock surface.",
        "Depth/Pass" | "Depth per Pass" => "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter for large.",
        "Feed Rate" => "Cutting speed (mm/min). Wood: 500-2000 for small tools, 1500-4000 for large.",
        "Plunge Rate" => "Vertical feed speed (mm/min). Typically 30-50% of feed rate.",
        "Tolerance" => "Geometric tolerance for path approximation. Smaller = more accurate, slower.",
        "Min Cut Radius" | "Min Cutting Radius" => "Blend sharp corners with arcs of at least this radius.",
        "Stock to Leave" => "Material left on surface for finish pass. 0.2-0.5mm typical.",
        "Stock Top Z" => "Z height of the stock material top surface.",
        "Scallop Height" => "Target cusp height between passes. 0.05-0.2mm for finishing.",
        "Threshold Angle" => "Angle dividing steep (waterline) from shallow (raster) regions.",
        "Max Stepdown" => "Maximum Z step between ramp passes.",
        "Z Step" => "Vertical distance between waterline Z levels.",
        "Sampling" => "XY grid resolution for push-cutter sampling.",
        "Bitangency Angle" => "Minimum dihedral angle to detect concave edges. 140-170 deg typical.",
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
        _ => return None,
    })
}

fn draw_pocket_params(ui: &mut egui::Ui, cfg: &mut PocketConfig) {
    egui::Grid::new("pocket_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Pattern:");
        egui::ComboBox::from_id_salt("pocket_pat").selected_text(match cfg.pattern {
            PocketPattern::Contour => "Contour", PocketPattern::Zigzag => "Zigzag",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
            ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
        });
        ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        ui.label("Climb:"); ui.checkbox(&mut cfg.climb, ""); ui.end_row();
        if cfg.pattern == PocketPattern::Zigzag {
            dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
        }
    });
}

fn draw_profile_params(ui: &mut egui::Ui, cfg: &mut ProfileConfig) {
    egui::Grid::new("profile_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Side:");
        egui::ComboBox::from_id_salt("prof_side").selected_text(match cfg.side {
            ProfileSide::Outside => "Outside", ProfileSide::Inside => "Inside",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.side, ProfileSide::Outside, "Outside");
            ui.selectable_value(&mut cfg.side, ProfileSide::Inside, "Inside");
        });
        ui.end_row();
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        ui.label("Climb:"); ui.checkbox(&mut cfg.climb, ""); ui.end_row();
    });
    ui.add_space(8.0);
    ui.collapsing("Tabs", |ui| {
        egui::Grid::new("tab_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Count:");
            let mut count = cfg.tab_count as i32;
            if ui.add(egui::DragValue::new(&mut count).range(0..=20)).changed() {
                cfg.tab_count = count.max(0) as usize;
            }
            ui.end_row();
            if cfg.tab_count > 0 {
                dv(ui, "Width:", &mut cfg.tab_width, " mm", 0.5, 1.0..=50.0);
                dv(ui, "Height:", &mut cfg.tab_height, " mm", 0.5, 0.5..=20.0);
            }
        });
    });
}

fn draw_adaptive_params(ui: &mut egui::Ui, cfg: &mut AdaptiveConfig) {
    egui::Grid::new("adapt_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        ui.label("Slot Clearing:"); ui.checkbox(&mut cfg.slot_clearing, ""); ui.end_row();
        dv(ui, "Min Cut Radius:", &mut cfg.min_cutting_radius, " mm", 0.1, 0.0..=50.0);
    });
}

fn draw_vcarve_params(ui: &mut egui::Ui, cfg: &mut VCarveConfig) {
    ui.label(egui::RichText::new("Requires V-Bit tool").italics().color(egui::Color32::from_rgb(150, 140, 110)));
    egui::Grid::new("vcarve_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Max Depth:", &mut cfg.max_depth, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.05, 0.01..=10.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_rest_params(
    ui: &mut egui::Ui,
    cfg: &mut RestConfig,
    tools: &[(crate::state::job::ToolId, String, f64)],
) {
    egui::Grid::new("rest_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Previous Tool:");
        let prev_label = cfg.prev_tool_id
            .and_then(|pid| tools.iter().find(|(id, _, _)| *id == pid))
            .map(|(_, s, _)| s.as_str())
            .unwrap_or("(select)");
        egui::ComboBox::from_id_salt("rest_prev_tool").selected_text(prev_label).show_ui(ui, |ui| {
            for (id, name, _) in tools {
                let selected = cfg.prev_tool_id == Some(*id);
                if ui.selectable_label(selected, name.as_str()).clicked() {
                    cfg.prev_tool_id = Some(*id);
                }
            }
        });
        ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}

fn draw_inlay_params(ui: &mut egui::Ui, cfg: &mut InlayConfig) {
    ui.label(egui::RichText::new("Requires V-Bit tool").italics().color(egui::Color32::from_rgb(150, 140, 110)));
    egui::Grid::new("inlay_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Pocket Depth:", &mut cfg.pocket_depth, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Glue Gap:", &mut cfg.glue_gap, " mm", 0.01, 0.0..=2.0);
        dv(ui, "Flat Depth:", &mut cfg.flat_depth, " mm", 0.1, 0.0..=20.0);
        dv(ui, "Boundary Offset:", &mut cfg.boundary_offset, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Flat Tool Radius:", &mut cfg.flat_tool_radius, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_zigzag_params(ui: &mut egui::Ui, cfg: &mut ZigzagConfig) {
    egui::Grid::new("zigzag_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth:", &mut cfg.depth, " mm", 0.1, 0.1..=100.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Angle:", &mut cfg.angle, " deg", 1.0, 0.0..=360.0);
    });
}

// ── 3D operation parameters ──────────────────────────────────────────────

fn draw_dropcutter_params(ui: &mut egui::Ui, cfg: &mut DropCutterConfig) {
    egui::Grid::new("dc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Min Z:", &mut cfg.min_z, " mm", 0.5, -500.0..=0.0);
    });
}

fn draw_adaptive3d_params(ui: &mut egui::Ui, cfg: &mut Adaptive3dConfig) {
    egui::Grid::new("a3d_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Depth/Pass:", &mut cfg.depth_per_pass, " mm", 0.1, 0.1..=50.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Stock Top Z:", &mut cfg.stock_top_z, " mm", 0.5, -100.0..=200.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        dv(ui, "Min Cut Radius:", &mut cfg.min_cutting_radius, " mm", 0.1, 0.0..=50.0);
        ui.label("Entry Style:");
        egui::ComboBox::from_id_salt("a3d_entry").selected_text(match cfg.entry_style {
            EntryStyle::Plunge => "Plunge", EntryStyle::Helix => "Helix", EntryStyle::Ramp => "Ramp",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Plunge, "Plunge");
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Helix, "Helix");
            ui.selectable_value(&mut cfg.entry_style, EntryStyle::Ramp, "Ramp");
        });
        ui.end_row();
        dv(ui, "Fine Stepdown:", &mut cfg.fine_stepdown, " mm", 0.1, 0.0..=10.0);
        ui.label("Detect Flat:"); ui.checkbox(&mut cfg.detect_flat_areas, ""); ui.end_row();
        ui.label("Ordering:");
        egui::ComboBox::from_id_salt("a3d_ord").selected_text(match cfg.region_ordering {
            RegionOrdering::Global => "Global", RegionOrdering::ByArea => "By Area",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::Global, "Global");
            ui.selectable_value(&mut cfg.region_ordering, RegionOrdering::ByArea, "By Area");
        });
        ui.end_row();
    });
}

fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
    egui::Grid::new("wl_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Start Z:", &mut cfg.start_z, " mm", 0.5, -200.0..=200.0);
        dv(ui, "Final Z:", &mut cfg.final_z, " mm", 0.5, -200.0..=200.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
    });
}

fn draw_pencil_params(ui: &mut egui::Ui, cfg: &mut PencilConfig) {
    egui::Grid::new("pen_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Bitangency Angle:", &mut cfg.bitangency_angle, " deg", 1.0, 90.0..=180.0);
        dv(ui, "Min Cut Length:", &mut cfg.min_cut_length, " mm", 0.5, 0.5..=50.0);
        dv(ui, "Hookup Distance:", &mut cfg.hookup_distance, " mm", 0.5, 0.5..=50.0);
        ui.label("Offset Passes:");
        let mut n = cfg.num_offset_passes as i32;
        if ui.add(egui::DragValue::new(&mut n).range(0..=10)).changed() { cfg.num_offset_passes = n.max(0) as usize; }
        ui.end_row();
        dv(ui, "Offset Stepover:", &mut cfg.offset_stepover, " mm", 0.1, 0.05..=10.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
    });
}

fn draw_scallop_params(ui: &mut egui::Ui, cfg: &mut ScallopConfig) {
    egui::Grid::new("sc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Scallop Height:", &mut cfg.scallop_height, " mm", 0.01, 0.01..=2.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("sc_dir").selected_text(match cfg.direction {
            ScallopDirection::OutsideIn => "Outside In", ScallopDirection::InsideOut => "Inside Out",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.direction, ScallopDirection::OutsideIn, "Outside In");
            ui.selectable_value(&mut cfg.direction, ScallopDirection::InsideOut, "Inside Out");
        });
        ui.end_row();
        ui.label("Continuous:"); ui.checkbox(&mut cfg.continuous, ""); ui.end_row();
        dv(ui, "Slope From:", &mut cfg.slope_from, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
    });
}

fn draw_steep_shallow_params(ui: &mut egui::Ui, cfg: &mut SteepShallowConfig) {
    egui::Grid::new("ss_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Threshold Angle:", &mut cfg.threshold_angle, " deg", 1.0, 10.0..=80.0);
        dv(ui, "Overlap:", &mut cfg.overlap_distance, " mm", 0.1, 0.0..=10.0);
        dv(ui, "Wall Clearance:", &mut cfg.wall_clearance, " mm", 0.1, 0.0..=10.0);
        ui.label("Steep First:"); ui.checkbox(&mut cfg.steep_first, ""); ui.end_row();
        dv(ui, "Stepover:", &mut cfg.stepover, " mm", 0.1, 0.05..=50.0);
        dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

fn draw_ramp_finish_params(ui: &mut egui::Ui, cfg: &mut RampFinishConfig) {
    egui::Grid::new("rf_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dv(ui, "Max Stepdown:", &mut cfg.max_stepdown, " mm", 0.1, 0.05..=10.0);
        dv(ui, "Slope From:", &mut cfg.slope_from, " deg", 1.0, 0.0..=90.0);
        dv(ui, "Slope To:", &mut cfg.slope_to, " deg", 1.0, 0.0..=90.0);
        ui.label("Direction:");
        egui::ComboBox::from_id_salt("rf_dir").selected_text(match cfg.direction {
            CutDirection::Climb => "Climb", CutDirection::Conventional => "Conventional",
            CutDirection::BothWays => "Both Ways",
        }).show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.direction, CutDirection::Climb, "Climb");
            ui.selectable_value(&mut cfg.direction, CutDirection::Conventional, "Conventional");
            ui.selectable_value(&mut cfg.direction, CutDirection::BothWays, "Both Ways");
        });
        ui.end_row();
        ui.label("Bottom Up:"); ui.checkbox(&mut cfg.order_bottom_up, ""); ui.end_row();
        dv(ui, "Feed Rate:", &mut cfg.feed_rate, " mm/min", 10.0, 1.0..=50000.0);
        dv(ui, "Plunge Rate:", &mut cfg.plunge_rate, " mm/min", 10.0, 1.0..=10000.0);
        dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
        dv(ui, "Stock to Leave:", &mut cfg.stock_to_leave, " mm", 0.05, 0.0..=10.0);
        dv(ui, "Tolerance:", &mut cfg.tolerance, " mm", 0.01, 0.01..=1.0);
    });
}

// ── Validation ───────────────────────────────────────────────────────────

fn validate_toolpath(
    entry: &ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String, f64)],
) -> Vec<String> {
    let mut errs = Vec::new();

    let tool = tools.iter().find(|(id, _, _)| *id == entry.tool_id);
    if tool.is_none() {
        errs.push("No tool selected".into());
        return errs;
    }
    let (_, _, tool_diameter) = tool.unwrap();

    match &entry.operation {
        OperationConfig::Pocket(c) => {
            if c.stepover >= *tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::Adaptive(c) => {
            if c.stepover >= *tool_diameter {
                errs.push("Stepover must be less than tool diameter".into());
            }
        }
        OperationConfig::VCarve(_) => {
            let is_vbit = tools.iter().find(|(id, _, _)| *id == entry.tool_id)
                .map(|(_, name, _)| name.contains("V-Bit")).unwrap_or(false);
            if !is_vbit {
                errs.push("VCarve requires a V-Bit tool".into());
            }
        }
        OperationConfig::Inlay(_) => {
            let is_vbit = tools.iter().find(|(id, _, _)| *id == entry.tool_id)
                .map(|(_, name, _)| name.contains("V-Bit")).unwrap_or(false);
            if !is_vbit {
                errs.push("Inlay requires a V-Bit tool".into());
            }
        }
        OperationConfig::Rest(c) => {
            if c.prev_tool_id.is_none() {
                errs.push("Previous tool not selected".into());
            } else if let Some(prev) = c.prev_tool_id {
                let prev_d = tools.iter().find(|(id, _, _)| *id == prev).map(|(_, _, d)| *d);
                if let Some(pd) = prev_d {
                    if pd <= *tool_diameter {
                        errs.push("Previous tool must be larger than current tool".into());
                    }
                }
            }
        }
        _ => {}
    }

    errs
}

// ── Dressup configuration ────────────────────────────────────────────────

fn draw_dressup_params(ui: &mut egui::Ui, cfg: &mut DressupConfig) {
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
            egui::Grid::new("ramp_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                dv(ui, "  Max Angle:", &mut cfg.ramp_angle, " deg", 0.5, 0.5..=15.0);
            });
        }
        DressupEntryStyle::Helix => {
            egui::Grid::new("helix_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                dv(ui, "  Radius:", &mut cfg.helix_radius, " mm", 0.1, 0.5..=20.0);
                dv(ui, "  Pitch:", &mut cfg.helix_pitch, " mm", 0.1, 0.2..=10.0);
            });
        }
        DressupEntryStyle::None => {}
    }
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.dogbone, "Dogbone overcuts");
    if cfg.dogbone {
        egui::Grid::new("dog_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Angle:", &mut cfg.dogbone_angle, " deg", 1.0, 45.0..=135.0);
        });
    }
    ui.checkbox(&mut cfg.lead_in_out, "Lead-in / lead-out");
    if cfg.lead_in_out {
        egui::Grid::new("lead_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Radius:", &mut cfg.lead_radius, " mm", 0.1, 0.5..=20.0);
        });
    }
    ui.checkbox(&mut cfg.link_moves, "Link moves (keep tool down)");
    if cfg.link_moves {
        egui::Grid::new("link_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Distance:", &mut cfg.link_max_distance, " mm", 0.5, 1.0..=50.0);
            dv(ui, "  Feed Rate:", &mut cfg.link_feed_rate, " mm/min", 10.0, 50.0..=5000.0);
        });
    }
    ui.checkbox(&mut cfg.arc_fitting, "Arc fitting (G2/G3)");
    if cfg.arc_fitting {
        egui::Grid::new("arc_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Tolerance:", &mut cfg.arc_tolerance, " mm", 0.01, 0.01..=0.5);
        });
    }
    ui.checkbox(&mut cfg.feed_optimization, "Feed rate optimization");
    if cfg.feed_optimization {
        egui::Grid::new("fopt_p").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            dv(ui, "  Max Rate:", &mut cfg.feed_max_rate, " mm/min", 50.0, 500.0..=20000.0);
            dv(ui, "  Ramp Rate:", &mut cfg.feed_ramp_rate, " mm/min/mm", 10.0, 10.0..=2000.0);
        });
    }
}
