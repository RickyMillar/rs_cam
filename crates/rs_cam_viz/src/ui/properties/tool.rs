use crate::state::job::{CutDirection, ToolConfig, ToolMaterial, ToolType};

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
            let mut flutes_f = tool.flute_count as f64;
            if ui
                .add(
                    egui::DragValue::new(&mut flutes_f)
                        .range(1.0..=8.0)
                        .speed(0.1),
                )
                .changed()
            {
                tool.flute_count = (flutes_f as u32).clamp(1, 8);
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
                    for &cd in CutDirection::ALL {
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
}

/// Draw a 2D cross-section preview of the tool profile.
fn draw_tool_preview(ui: &mut egui::Ui, tool: &ToolConfig) {
    let desired_size = egui::vec2(ui.available_width().min(240.0), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(22, 22, 28));

    let cx = rect.center().x;
    let bottom = rect.bottom() - 10.0;

    // Cast all tool params to f32 for drawing
    let r = tool.diameter as f32 / 2.0;
    let cl = tool.cutting_length as f32;
    let cr = tool.corner_radius as f32;
    let inc_angle = tool.included_angle as f32;
    let taper_ha = tool.taper_half_angle as f32;
    let shaft_d = tool.shaft_diameter as f32;

    // Scale: fit the tool diameter into ~60% of the preview width
    let scale = (desired_size.x * 0.3) / r.max(0.1);
    let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(160, 170, 190));

    match tool.tool_type {
        ToolType::EndMill => {
            let hw = r * scale;
            let h = cl.min(r * 4.0) * scale;
            let left = cx - hw;
            let right = cx + hw;
            let top = bottom - h;
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(left, bottom),
                    egui::pos2(right, bottom),
                    egui::pos2(right, top),
                    egui::pos2(left, top),
                    egui::pos2(left, bottom),
                ],
                stroke,
            ));
        }
        ToolType::BallNose => {
            let hw = r * scale;
            let ball_h = r * scale;
            let shaft_h = (cl - r).max(0.0).min(r * 3.0) * scale;
            let ball_cy = bottom - ball_h;
            let top = ball_cy - shaft_h;

            let mut pts = vec![
                egui::pos2(cx - hw, ball_cy),
                egui::pos2(cx - hw, top),
                egui::pos2(cx + hw, top),
                egui::pos2(cx + hw, ball_cy),
            ];
            for i in 0..=32 {
                let a = std::f32::consts::PI * (i as f32) / 32.0;
                pts.push(egui::pos2(cx + hw * a.cos(), ball_cy + ball_h * a.sin()));
            }
            painter.add(egui::Shape::line(pts, stroke));
        }
        ToolType::BullNose => {
            let hw = r * scale;
            let crs = cr.min(r) * scale;
            let flat_hw = hw - crs;
            let shaft_h = (cl - cr).max(0.0).min(r * 3.0) * scale;
            let arc_cy = bottom - crs;
            let top = arc_cy - shaft_h;

            let mut pts = vec![
                egui::pos2(cx - hw, arc_cy),
                egui::pos2(cx - hw, top),
                egui::pos2(cx + hw, top),
                egui::pos2(cx + hw, arc_cy),
            ];
            // Right corner arc
            for i in 0..=16 {
                let a = std::f32::consts::FRAC_PI_2 * (i as f32) / 16.0;
                pts.push(egui::pos2(
                    cx + flat_hw + crs * a.cos(),
                    arc_cy + crs * a.sin(),
                ));
            }
            // Flat bottom (already at bottom after right arc ends at (cx+flat_hw, bottom))
            pts.push(egui::pos2(cx - flat_hw, bottom));
            // Left corner arc
            for i in 0..=16 {
                let a =
                    std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * (i as f32) / 16.0;
                pts.push(egui::pos2(
                    cx - flat_hw + crs * a.cos(),
                    arc_cy + crs * a.sin(),
                ));
            }
            painter.add(egui::Shape::line(pts, stroke));
        }
        ToolType::VBit => {
            let hw = r * scale;
            let half_a = (inc_angle / 2.0).to_radians();
            let cone_h = (r / half_a.tan()) * scale;
            let shaft_h = (cl * scale - cone_h).max(0.0).min(hw * 2.0);
            let cone_top = bottom - cone_h;
            let top = cone_top - shaft_h;

            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(cx - hw, cone_top),
                    egui::pos2(cx - hw, top),
                    egui::pos2(cx + hw, top),
                    egui::pos2(cx + hw, cone_top),
                    egui::pos2(cx, bottom),
                    egui::pos2(cx - hw, cone_top),
                ],
                stroke,
            ));
        }
        ToolType::TaperedBallNose => {
            let ball_r = r * scale;
            let shaft_hw = (shaft_d / 2.0) * scale;
            let alpha = taper_ha.to_radians();
            let ball_cy = bottom - ball_r;

            let r_contact = ball_r * alpha.cos();
            let h_contact = ball_r * (1.0 - alpha.sin());
            let total_h = cl.min(r * 8.0) * scale;
            let top = ball_cy - total_h + ball_r;

            let mut pts = Vec::new();
            // Left shaft down to junction
            pts.push(egui::pos2(cx - shaft_hw, top));
            pts.push(egui::pos2(cx - r_contact, ball_cy - h_contact));
            // Ball arc
            let start_a = std::f32::consts::FRAC_PI_2 + alpha;
            let end_a = std::f32::consts::FRAC_PI_2 * 3.0 - alpha;
            for i in 0..=32 {
                let a = start_a + (end_a - start_a) * (i as f32) / 32.0;
                pts.push(egui::pos2(
                    cx - ball_r * a.cos(),
                    ball_cy + ball_r * a.sin(),
                ));
            }
            // Right junction up to shaft
            pts.push(egui::pos2(cx + shaft_hw, top));
            pts.push(egui::pos2(cx - shaft_hw, top));
            painter.add(egui::Shape::line(pts, stroke));
        }
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
