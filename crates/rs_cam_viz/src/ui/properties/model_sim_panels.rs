//! The two side panels the inspector draws for a non-toolpath selection:
//! the model's properties and the simulation's own panel.

use crate::state::AppState;
use crate::ui::AppEvent;
use crate::ui::components::UiExt as _;

pub(super) fn draw_model_properties(
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

    // G-MODELRELINK (F4.3). Two things this panel could not do before.
    //
    // It could not say WHY. `LoadedModel::load_error` holds the loader's own
    // reason and was rendered nowhere in the GUI — only MCP `inspect_model`
    // ever published it — so a model that failed to load showed three lines
    // (name, type, path) and no explanation.
    //
    // And it offered no repair. The only routes were "Reload from disk" (the
    // same path that just failed) and "Delete" (refused while any toolpath
    // references it), so a project moved between machines was stuck. "Locate
    // file…" is the missing one, and it is shown on healthy models too
    // (R0.7 §7 Q3): pointing a model at a different file is the same
    // operation whether or not the old one is still there.
    if let Some(detail) = &model.load_error {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("Not loaded: {detail}"))
                    .small()
                    .color(crate::ui::tokens::CAUTION),
            )
            .wrap(),
        );
    }
    let (filter_label, extensions): (&str, &[&str]) = match model.kind {
        Some(crate::state::job::ModelKind::Stl) => ("STL Files", &["stl", "STL"]),
        Some(crate::state::job::ModelKind::Dxf) => ("DXF Files", &["dxf", "DXF"]),
        Some(crate::state::job::ModelKind::Svg) => ("SVG Files", &["svg", "SVG"]),
        Some(crate::state::job::ModelKind::Step) => ("STEP Files", &["step", "stp", "STEP", "STP"]),
        None => ("Model Files", &["stl", "dxf", "svg", "step", "stp"]),
    };
    if ui
        .button("Locate file\u{2026}")
        .on_hover_text(
            "Point this model at a different file on disk. The model keeps its name, its declared units and its identity, so every operation built on it keeps working — and every one of them is marked for regeneration, because the geometry changed.",
        )
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter(filter_label, extensions)
            .pick_file()
    {
        events.push(AppEvent::RelinkModel(id, path));
    }

    if let Some(mesh) = &model.mesh {
        let bb = &mesh.bbox;
        let dx = bb.max.x - bb.min.x;
        let dy = bb.max.y - bb.min.y;
        let dz = bb.max.z - bb.min.z;

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Dimensions (after scaling)")
                .strong()
                .color(crate::ui::tokens::TEXT_STRONG),
        );
        ui.param_grid("mesh_dims", |ui| {
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
                    .color(crate::ui::tokens::CAUTION),
            );
        } else if min_dim > 5000.0 {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Very large! Check units").color(crate::ui::tokens::CAUTION),
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
                .color(crate::ui::tokens::CAUTION),
            );
        }

        // Units / scale selector (all formats including STEP)
        {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Units / Scale")
                    .strong()
                    .color(crate::ui::tokens::TEXT_STRONG),
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

    // Expert-grade internals — mesh counts, BREP topology, polygon listing —
    // grouped under one default-closed disclosure (density pass 2026-06-11).
    // Dimensions, warnings, and units/scale stay top-level above.
    if model.mesh.is_some() || model.polygons.is_some() {
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Details")
            .id_salt("model_details")
            .default_open(false)
            .show(ui, |ui| {
                if let Some(mesh) = &model.mesh {
                    ui.label(
                        egui::RichText::new("Mesh Info")
                            .strong()
                            .color(crate::ui::tokens::TEXT_STRONG),
                    );
                    ui.param_grid("mesh_info", |ui| {
                        ui.label("Vertices:");
                        ui.label(format!("{}", mesh.vertices.len()));
                        ui.end_row();
                        ui.label("Triangles:");
                        ui.label(format!("{}", mesh.triangles.len()));
                        ui.end_row();
                    });
                }

                // BREP face metadata (STEP only)
                if let Some(enriched) = &model.enriched_mesh {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("BREP Topology")
                            .strong()
                            .color(crate::ui::tokens::TEXT_STRONG),
                    );
                    ui.param_grid("brep_info", |ui| {
                        ui.label("Faces:");
                        ui.label(format!("{}", enriched.face_count()));
                        ui.end_row();
                        ui.label("Adjacency pairs:");
                        ui.label(format!("{}", enriched.adjacency.len()));
                        ui.end_row();

                        // Surface type histogram
                        use rs_cam_core::geometry::enriched_mesh::SurfaceType;
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
            });
    }
}

pub(super) fn draw_simulation_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Simulation");
    ui.separator();

    // Toolpath checklist
    ui.label(
        egui::RichText::new("Included Toolpaths")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
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
        let color = crate::ui::tokens::from_linear_rgb(pc);
        let is_current = current_boundary_id == Some(*id);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("\u{25CF}").color(color));
            let text = if is_current {
                egui::RichText::new(name)
                    .strong()
                    .color(egui::Color32::WHITE)
            } else {
                egui::RichText::new(name).color(crate::ui::tokens::TEXT_STRONG)
            };
            ui.label(text);
            ui.label(
                egui::RichText::new(tool_name)
                    .small()
                    .color(crate::ui::tokens::TEXT_MUTED),
            );
        });

        // Per-toolpath visibility controls: eye / cut / rapid.
        //
        // P6 (audit §2d, fix §6.7) — these two lines used to be plain "Cut" /
        // "Rapid" checkboxes writing the SAME map entry as each operation
        // row's C / R glyphs. Both writes took effect, so it was a UX defect
        // rather than a correctness one, and the two homes were not
        // equivalent: the row controls grey each button when its global flag
        // is off and the disabled hover NAMES the control blocking it, while
        // these checkboxes stayed clickable and appeared to work with
        // `show_cutting` ANDing them away. One state, one affordance — the
        // richer one, which is also the one the Inspector points at.
        let overall_visible = state.gui.toolpath_rt.get(id).is_none_or(|rt| rt.visible);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            crate::ui::toolpath_row_controls::draw(
                ui,
                *id,
                overall_visible,
                &mut state.viewport,
                events,
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
                .color(crate::ui::tokens::TEXT_STRONG),
        );
        ui.param_grid("sim_tool_pos", |ui| {
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
                .color(crate::ui::tokens::TEXT_STRONG),
        );
        ui.label(format!("{} ({})", boundary.name, boundary.tool_name));
        ui.label(format!("Move {}/{}", within, total));
    }
}
