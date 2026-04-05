use crate::state::job::{BitCutDirection, ToolConfig, ToolMaterial, ToolType};

pub fn draw(ui: &mut egui::Ui, tool: &mut ToolConfig) {
    ui.heading(&tool.name);
    ui.separator();

    // Editable name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut tool.name);
    });

    ui.add_space(4.0);

    // Tool type selector
    ui.horizontal(|ui| {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("tool_type")
            .selected_text(tool.tool_type.label())
            .show_ui(ui, |ui| {
                for &tt in ToolType::ALL {
                    ui.selectable_value(&mut tool.tool_type, tt, tt.label());
                }
            });
    });

    ui.add_space(8.0);

    // Parameters grid
    egui::Grid::new("tool_params")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Diameter:");
            ui.add(
                egui::DragValue::new(&mut tool.diameter)
                    .suffix(" mm")
                    .speed(0.1)
                    .range(0.1..=100.0),
            );
            ui.end_row();

            ui.label("Cutting Length:");
            ui.add(
                egui::DragValue::new(&mut tool.cutting_length)
                    .suffix(" mm")
                    .speed(0.5)
                    .range(0.1..=200.0),
            );
            ui.end_row();

            // Flute count (critical for feeds calculation)
            ui.label("Flutes:");
            let mut flutes_i = tool.flute_count as i32;
            if ui
                .add(egui::DragValue::new(&mut flutes_i).range(1..=8))
                .changed()
            {
                tool.flute_count = flutes_i.max(1) as u32;
            }
            ui.end_row();

            // Tool material
            ui.label("Material:");
            egui::ComboBox::from_id_salt("tool_material")
                .selected_text(tool.tool_material.label())
                .show_ui(ui, |ui| {
                    for &tm in ToolMaterial::ALL {
                        ui.selectable_value(&mut tool.tool_material, tm, tm.label());
                    }
                });
            ui.end_row();

            // Cut direction
            ui.label("Cut Dir:");
            egui::ComboBox::from_id_salt("cut_direction")
                .selected_text(tool.cut_direction.label())
                .show_ui(ui, |ui| {
                    for &cd in BitCutDirection::ALL {
                        ui.selectable_value(&mut tool.cut_direction, cd, cd.label());
                    }
                });
            ui.end_row();

            // Type-specific parameters
            match tool.tool_type {
                ToolType::BullNose => {
                    ui.label("Corner Radius:");
                    ui.add(
                        egui::DragValue::new(&mut tool.corner_radius)
                            .suffix(" mm")
                            .speed(0.05)
                            .range(0.01..=tool.diameter / 2.0),
                    );
                    ui.end_row();
                }
                ToolType::VBit => {
                    ui.label("Included Angle:");
                    ui.add(
                        egui::DragValue::new(&mut tool.included_angle)
                            .suffix(" deg")
                            .speed(1.0)
                            .range(1.0..=179.0),
                    );
                    ui.end_row();
                }
                ToolType::TaperedBallNose => {
                    ui.label("Taper Half-Angle:");
                    ui.add(
                        egui::DragValue::new(&mut tool.taper_half_angle)
                            .suffix(" deg")
                            .speed(0.5)
                            .range(0.5..=89.0),
                    );
                    ui.end_row();

                    ui.label("Shaft Diameter:");
                    ui.add(
                        egui::DragValue::new(&mut tool.shaft_diameter)
                            .suffix(" mm")
                            .speed(0.1)
                            .range(tool.diameter..=100.0),
                    );
                    ui.end_row();
                }
                _ => {}
            }
        });

    // Cross-section preview
    ui.add_space(12.0);
    ui.label("Cross-Section Preview:");
    draw_tool_preview(ui, tool);

    // Holder section (collapsible)
    ui.add_space(12.0);
    ui.collapsing("Holder / Shank", |ui| {
        egui::Grid::new("holder_params")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Holder Diameter:");
                ui.add(
                    egui::DragValue::new(&mut tool.holder_diameter)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.0..=200.0),
                );
                ui.end_row();

                ui.label("Shank Diameter:");
                ui.add(
                    egui::DragValue::new(&mut tool.shank_diameter)
                        .suffix(" mm")
                        .speed(0.1)
                        .range(0.0..=100.0),
                );
                ui.end_row();

                ui.label("Shank Length:");
                ui.add(
                    egui::DragValue::new(&mut tool.shank_length)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.0..=200.0),
                );
                ui.end_row();

                ui.label("Stickout:");
                ui.add(
                    egui::DragValue::new(&mut tool.stickout)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.0..=300.0),
                );
                ui.end_row();
            });
    });
    if tool.holder_diameter < 0.01 {
        ui.label(
            egui::RichText::new("Holder not configured — collision check will be skipped")
                .small()
                .italics()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    }
}

/// Draw a 2D cross-section preview of the full tool assembly.
///
/// Uses `profile_points()` from the `MillingCutter` trait so the preview
/// automatically matches the actual cutting geometry for any tool type.
fn draw_tool_preview(ui: &mut egui::Ui, tool: &ToolConfig) {
    use rs_cam_core::tool::MillingCutter;

    let desired_size = egui::vec2(ui.available_width().min(240.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(22, 22, 28));

    let cx = rect.center().x;
    let bottom = rect.bottom() - 10.0;
    let draw_h = desired_size.y - 20.0; // vertical pixels available

    let tool_def = crate::compute::worker::helpers::build_cutter(tool);
    let cutter_r = tool_def.radius() as f32;
    let cutting_len = tool_def.length() as f32;
    let shank_r = (tool_def.shank_diameter / 2.0) as f32;
    let shank_len = tool_def.shank_length as f32;
    let holder_r = (tool_def.holder_diameter / 2.0) as f32;
    let holder_len = tool_def.holder_length() as f32;

    // Total height to display; clamp so preview isn't squished
    let total_h = (cutting_len + shank_len + holder_len).max(cutting_len * 1.2);
    let max_r = cutter_r.max(shank_r).max(holder_r).max(0.1);

    // Scale: fit both width and height with padding
    let scale_x = (desired_size.x * 0.4) / max_r;
    let scale_y = draw_h / total_h.max(0.1);
    let scale = scale_x.min(scale_y);

    let cutter_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(160, 170, 190));
    let shank_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 125, 135));
    let holder_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 92, 100));

    // --- Cutter cross-section from profile_points ---
    let profile = tool_def.profile_points(32);
    let n = profile.len();

    // Right side: trace profile from tip (bottom) upward, then vertical to cutting_length
    let mut right_pts: Vec<egui::Pos2> = Vec::with_capacity(n + 2);
    for &(r, h) in &profile {
        right_pts.push(egui::pos2(
            cx + (r as f32) * scale,
            bottom - (h as f32) * scale,
        ));
    }
    // Extend vertically from the last profile point to cutting_length
    if let Some(&last) = right_pts.last() {
        let top_y = bottom - cutting_len * scale;
        if last.y > top_y + 0.5 {
            right_pts.push(egui::pos2(cx + cutter_r * scale, top_y));
        }
    }

    // Build closed outline: right side down (reversed) → left side up
    let mut outline: Vec<egui::Pos2> = Vec::with_capacity(n * 2 + 4);
    // Right side (top to tip)
    for pt in right_pts.iter().rev() {
        outline.push(*pt);
    }
    // Left side (tip to top) — mirror X
    for pt in &right_pts {
        outline.push(egui::pos2(2.0 * cx - pt.x, pt.y));
    }
    // Close
    if let Some(&first) = outline.first() {
        outline.push(first);
    }
    painter.add(egui::Shape::line(outline, cutter_stroke));

    // --- Shank rectangle ---
    let cutter_top_y = bottom - cutting_len * scale;
    if shank_len > 0.01 && shank_r > 0.01 {
        let shank_top_y = cutter_top_y - shank_len * scale;
        let sr = shank_r * scale;
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(cx - sr, cutter_top_y),
                egui::pos2(cx + sr, cutter_top_y),
                egui::pos2(cx + sr, shank_top_y),
                egui::pos2(cx - sr, shank_top_y),
                egui::pos2(cx - sr, cutter_top_y),
            ],
            shank_stroke,
        ));
    }

    // --- Holder rectangle ---
    let shank_top_y = cutter_top_y - shank_len * scale;
    if holder_len > 0.01 && holder_r > 0.01 {
        let holder_top_y = shank_top_y - holder_len * scale;
        let hr = holder_r * scale;
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(cx - hr, shank_top_y),
                egui::pos2(cx + hr, shank_top_y),
                egui::pos2(cx + hr, holder_top_y),
                egui::pos2(cx - hr, holder_top_y),
                egui::pos2(cx - hr, shank_top_y),
            ],
            holder_stroke,
        ));
    }

    // Center line
    painter.line_segment(
        [egui::pos2(cx, rect.top() + 5.0), egui::pos2(cx, bottom)],
        egui::Stroke::new(
            0.5,
            egui::Color32::from_rgba_premultiplied(80, 80, 100, 100),
        ),
    );
}
