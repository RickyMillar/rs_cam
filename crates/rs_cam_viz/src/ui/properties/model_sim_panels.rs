//! The two side panels the inspector draws for a non-toolpath selection:
//! the model's properties and the simulation's own panel.

use crate::state::AppState;
use crate::ui::AppEvent;

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

    // IC1: the kind and the path are one row. The path is often longer than
    // the panel, so it truncates and its hover gives it in full.
    let kind = match model.kind {
        Some(crate::state::job::ModelKind::Stl) => "STL",
        Some(crate::state::job::ModelKind::Dxf) => "DXF",
        Some(crate::state::job::ModelKind::Svg) => "SVG",
        Some(crate::state::job::ModelKind::Step) => "STEP",
        None => "Unknown type",
    };
    ui.add(
        crate::ui::components::OneLine::new(format!("{kind} \u{00B7} {}", model.path.display()))
            .color(crate::ui::tokens::TEXT_BODY),
    );

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
    {
        // F218: the dialog runs off the frame; the pick sends
        // `AppEvent::RelinkModel(id, path)`.
        events.push(
            crate::app::file_dialog::FileDialogRequest::pick_file()
                .filter(filter_label, extensions)
                .event(crate::app::file_dialog::AfterPick::RelinkModel(id)),
        );
    }

    if let Some(mesh) = &model.mesh {
        let bb = &mesh.bbox;
        let dx = bb.max.x - bb.min.x;
        let dy = bb.max.y - bb.min.y;
        let dz = bb.max.z - bb.min.z;

        ui.add_space(8.0);
        crate::ui::components::SectionHeader::new("Dimensions (after scaling)").show(ui);
        // Round 2: the extents are rows of one table; the range of each axis
        // is its hover. A size or normal caution is one status row.
        use crate::ui::components::{DataRow, DataTable, Role};
        let axis = |name: &str, size: f64, lo: f64, hi: f64| {
            DataRow::measured(name, format!("{size:.2}"), "mm")
                .hover(format!("From {lo:.3} mm to {hi:.3} mm."))
        };
        let mut table = DataTable::new("mesh_dims")
            .row(axis("X", dx, bb.min.x, bb.max.x))
            .row(axis("Y", dy, bb.min.y, bb.max.y))
            .row(axis("Z", dz, bb.min.z, bb.max.z));

        // Size hint
        let max_dim = dx.max(dy).max(dz);
        let min_dim = dx.min(dy).min(dz);
        if max_dim < 1.0 {
            table = table.row(
                DataRow::note(Role::Caution, "\u{26A0} Very small: metres?")
                    .hover("The model is under 1 mm. It is probably in metres; scale it x1000."),
            );
        } else if min_dim > 5000.0 {
            table = table.row(DataRow::note(
                Role::Caution,
                "\u{26A0} Very large: check the units",
            ));
        }

        // Normal flip warning (D1): check winding consistency
        if let Some(report) = &model.winding_report
            && *report > 1.0
        {
            table = table.row(
                DataRow::note(
                    Role::Caution,
                    format!("\u{26A0} {report:.1}% inconsistent normals"),
                )
                .hover("The loader fixed the normals when it read the model."),
            );
        }
        table.show(ui);

        // Units / scale selector (all formats including STEP)
        {
            ui.add_space(8.0);
            crate::ui::components::SectionHeader::new("Units / Scale").show(ui);

            let current_units = model.units.unwrap_or(ModelUnits::Millimeters);
            let current_label = current_units.label();

            ui.horizontal(|ui| {
                ui.label("Import as:");
                egui::ComboBox::from_id_salt("model_units")
                    .truncate()
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

    // P4.8: a drawing (SVG, DXF) shows its scale, its join tolerance and
    // the join report of its import: the rows of the import dialog. A
    // drawing has no mesh, so the Units / Scale section above does not
    // draw for it.
    if let Some(report) = &model.import_report {
        use crate::ui::components::DataTable;
        ui.add_space(8.0);
        crate::ui::components::SectionHeader::new("Import").show(ui);
        let current = model.units.unwrap_or(ModelUnits::Millimeters);
        if let Some(units) = crate::ui::import_modal::units_row(
            ui,
            &mut state.panels.number_drafts,
            egui::Id::new(("model_units_row", id)),
            current,
        ) {
            events.push(AppEvent::RescaleModel(id, units));
        }
        if let Some(options) = crate::ui::import_modal::join_row(
            ui,
            &mut state.panels.number_drafts,
            egui::Id::new(("model_join_tolerance", id)),
            model.import_options,
        ) {
            events.push(AppEvent::SetModelImportOptions(id, options));
        }
        DataTable::new(("model_import_report", id))
            .row(crate::ui::import_modal::report_row(report))
            .show(ui);
    }

    // P4.9: the open shapes of a drawing and the edit ledger.
    if model.items.has_drawing() || !model.drawing.edits().is_empty() {
        draw_open_shapes(ui, id, model, &mut state.panels, events);
    }

    // Expert-grade internals — mesh counts, BREP topology, polygon listing —
    // grouped under one default-closed disclosure (density pass 2026-06-11).
    // Dimensions, warnings, and units/scale stay top-level above.
    if model.mesh.is_some() || model.items.has_drawing() {
        use crate::ui::components::{DataRow, DataTable, Details};
        ui.add_space(8.0);
        // Round 2: the one `Details` disclosure, and one table of counts.
        let mut table = DataTable::new("model_details_table");
        if let Some(mesh) = &model.mesh {
            table = table
                .row(DataRow::value("Vertices", mesh.vertices.len().to_string()))
                .row(DataRow::value(
                    "Triangles",
                    mesh.triangles.len().to_string(),
                ));
        }
        // BREP face metadata (STEP only)
        if let Some(enriched) = &model.enriched_mesh {
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
            table = table
                .row(DataRow::value(
                    "BREP faces",
                    enriched.face_count().to_string(),
                ))
                .row(DataRow::value(
                    "Adjacency pairs",
                    enriched.adjacency.len().to_string(),
                ))
                .row(DataRow::value("Surface types", parts.join(", ")));
        }
        // Geometry selection P2.7: the items by kind and by layer
        // (`ItemSet::counts`), the counts MCP `inspect_model` reports under
        // `items`.
        if !model.items.is_empty() {
            let counts = model.items.counts();
            table = table.row(DataRow::value(
                "Items",
                rs_cam_core::selection::ItemCounts::line(&counts.by_kind),
            ));
            for (layer, by_kind) in &counts.by_layer {
                table = table.row(DataRow::value(
                    format!("  layer {layer}"),
                    rs_cam_core::selection::ItemCounts::line(by_kind),
                ));
            }
        }
        // The drawn rings of the model, nested (`ItemSet::drawing_curves`),
        // the same list MCP `inspect_model` reports under `polygons`.
        let curves = model
            .items
            .has_drawing()
            .then(|| model.items.drawing_curves());
        if let Some(polys) = &curves {
            table = table.row(DataRow::value("Polygons", polys.len().to_string()));
            for (i, p) in polys.iter().enumerate().take(5) {
                table = table.row(DataRow::value(
                    format!("  #{}", i + 1),
                    format!("{} pts, {} holes", p.exterior.len(), p.holes.len()),
                ));
            }
            if polys.len() > 5 {
                table = table.row(DataRow::value(
                    "  more",
                    format!("{} polygons", polys.len() - 5),
                ));
            }
        }
        let count = table.len();
        Details::new("model_details").count(count).show(ui, |ui| {
            table.show(ui);
        });
    }
}

/// The most open paths the "Open shapes" section lists one by one. A
/// drawing of text strokes can have hundreds; "Join all within" acts on
/// every one of them.
const OPEN_SHAPE_ROWS: usize = 12;

/// The "Open shapes" section of a drawing (geometry selection P4.9): each
/// open path with "Show me" and its two close acts, "Join all within", and
/// the ledger in one closed `Details` with a Remove per edit. Every act is
/// one `EditModelDrawing` event. The rows read core's `open_shapes`, the
/// list MCP `inspect_model` gives.
fn draw_open_shapes(
    ui: &mut egui::Ui,
    id: crate::state::job::ModelId,
    model: &rs_cam_core::session::LoadedModel,
    panels: &mut crate::state::panels::PanelDrafts,
    events: &mut Vec<AppEvent>,
) {
    use crate::ui::components::{Button, Details};
    use crate::ui::tokens;
    use rs_cam_core::selection::DrawingEdit;
    use rs_cam_core::session::LedgerEdit;

    let open = rs_cam_core::selection::open_shapes(&model.items);
    // The ledger stores file units; the panel shows model mm.
    let edits = model.drawing.edits_in_model_mm(model.drawing_scale());
    if open.is_empty() && edits.is_empty() {
        return;
    }
    let send = |events: &mut Vec<AppEvent>, edit: LedgerEdit| {
        events.push(AppEvent::EditModelDrawing { model_id: id, edit });
    };
    ui.add_space(8.0);
    crate::ui::components::SectionHeader::new("Open shapes")
        .trailing(format!("{} open", open.len()))
        .hover(
            "The open paths of the drawing. An import never closes a gap by itself. \
             Close a path or join paths here; the project keeps each act and does it \
             again after a reload.",
        )
        .show(ui);
    for shape in open.iter().take(OPEN_SHAPE_ROWS) {
        let layer = shape
            .layer
            .as_deref()
            .map_or_else(|| "Path".to_owned(), |l| format!("Path on layer {l}"));
        let face = format!(
            "{layer}, {:.2} mm, gap {:.2} mm",
            shape.length_mm, shape.gap_mm
        );
        let hover = format!(
            "An open path that starts at ({:.3}, {:.3}). The gap is the distance from its end \
             to its start.",
            shape.key.anchor[0], shape.key.anchor[1]
        );
        if super::selection_section::line_with_actions(
            ui,
            "",
            &face,
            tokens::CAUTION,
            &hover,
            &[(
                super::selection_section::SHOW_ME,
                "Frame this path in the viewport.",
            )],
        ) == Some(0)
        {
            events.push(AppEvent::FrameModelItems {
                model_id: id,
                keys: vec![shape.key.clone()],
            });
        }
        ui.horizontal(|ui| {
            if ui
                .add(
                    Button::quiet(format!("Close with line ({:.2} mm)", shape.gap_mm))
                        .enabled(shape.can_close_with_line),
                )
                .on_hover_text("Add a straight line from the end of the path to its start.")
                .clicked()
            {
                send(
                    events,
                    LedgerEdit::Add(DrawingEdit::CloseWithLine {
                        path: shape.key.clone(),
                    }),
                );
            }
            if ui
                .add(Button::quiet("Move ends together").enabled(shape.can_move_ends_together))
                .on_hover_text(
                    "Move the end point of the path onto its start point. No line is added.",
                )
                .clicked()
            {
                send(
                    events,
                    LedgerEdit::Add(DrawingEdit::MoveEndsTogether {
                        path: shape.key.clone(),
                    }),
                );
            }
        });
    }
    if open.len() > OPEN_SHAPE_ROWS {
        ui.add(
            crate::ui::components::OneLine::new(format!(
                "{} more open paths",
                open.len() - OPEN_SHAPE_ROWS
            ))
            .color(tokens::TEXT_BODY),
        );
    }
    if !open.is_empty() {
        let committed = panels
            .join_all_mm
            .unwrap_or(crate::state::panels::DEFAULT_JOIN_ALL_MM);
        if let Some(value) = crate::ui::import_modal::committed_number(
            ui,
            &mut panels.number_drafts,
            egui::Id::new(("model_join_all", id)),
            crate::ui::import_modal::NumberField {
                label: "Join tolerance",
                suffix: " mm",
                speed: 0.01,
                range: 0.0001..=100.0,
                hover: "Open path ends closer than this join, on one layer. A path whose \
                        ends meet closes. The value is in mm of the model.",
            },
            committed,
        ) {
            panels.join_all_mm = Some(value);
        }
        let tolerance_mm = panels
            .join_all_mm
            .unwrap_or(crate::state::panels::DEFAULT_JOIN_ALL_MM);
        if ui
            .add(Button::quiet(format!(
                "Join all within {tolerance_mm:.2} mm"
            )))
            .on_hover_text(
                "Join the open paths of each layer whose ends meet within the tolerance.",
            )
            .clicked()
        {
            send(
                events,
                LedgerEdit::Add(DrawingEdit::JoinWithin {
                    layer: None,
                    tolerance_mm,
                }),
            );
        }
    }
    if edits.is_empty() {
        return;
    }
    // A lost edit shows as a lost pick does: one DANGER line with Remove,
    // outside the closed details (it is never skipped in silence).
    let outcomes = model.drawing.outcomes();
    let lost_of = |index: usize| {
        outcomes
            .iter()
            .find(|o| o.edit == index)
            .and_then(|o| o.result.as_ref().err())
    };
    for (index, edit) in edits.iter().enumerate() {
        let Some(why) = lost_of(index) else {
            continue;
        };
        if super::selection_section::line_with_actions(
            ui,
            "",
            &format!("Edit {} does not apply: {}", index + 1, why.label()),
            tokens::DANGER,
            &format!(
                "{}. The edit is kept; after a re-import its path is gone or changed. \
                 Remove it, or import the drawing again.",
                edit.label()
            ),
            &[("Remove", "Remove this edit from the drawing.")],
        ) == Some(0)
        {
            send(events, LedgerEdit::Remove(index));
        }
    }
    let lost = model.drawing.lost_count();
    let summary = if lost > 0 {
        format!("{} edits stored ({lost} lost)", edits.len())
    } else {
        format!("{} edits stored", edits.len())
    };
    Details::new(("model_drawing_ledger", id))
        .count(edits.len())
        .show(ui, |ui| {
            ui.add(crate::ui::components::OneLine::new(summary).color(tokens::TEXT_BODY));
            for (index, edit) in edits.iter().enumerate() {
                let color = if lost_of(index).is_some() {
                    tokens::DANGER
                } else {
                    tokens::TEXT_BODY
                };
                if super::selection_section::line_with_actions(
                    ui,
                    "",
                    &format!("{}. {}", index + 1, edit.label()),
                    color,
                    "One stored act. Remove it to undo its effect; the edits after it run again.",
                    &[("Remove", "Remove this edit from the drawing.")],
                ) == Some(0)
                {
                    send(events, LedgerEdit::Remove(index));
                }
            }
        });
}

pub(super) fn draw_simulation_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Simulation");
    ui.separator();

    // Toolpath checklist
    crate::ui::components::SectionHeader::new("Included Toolpaths").show(ui);

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
        crate::ui::components::SectionHeader::new("Tool Position").show(ui);
        use crate::ui::components::{DataRow, DataTable};
        DataTable::new("sim_tool_pos")
            .row(DataRow::measured("X", format!("{:.3}", pos[0]), "mm"))
            .row(DataRow::measured("Y", format!("{:.3}", pos[1]), "mm"))
            .row(DataRow::measured("Z", format!("{:.3}", pos[2]), "mm"))
            .show(ui);
    }

    ui.add_space(8.0);

    // Current operation info
    if let Some(boundary) = state.simulation.current_boundary() {
        let (within, total) = state.simulation.current_toolpath_progress();
        crate::ui::components::SectionHeader::new("Current Operation").show(ui);
        use crate::ui::components::{DataRow, DataTable};
        DataTable::new("sim_current_operation")
            .row(DataRow::value("Operation", boundary.name.clone()))
            .row(DataRow::value("Tool", boundary.tool_name.clone()))
            .row(DataRow::value("Move", format!("{within} / {total}")))
            .show(ui);
    }
}
