use crate::state::job::{AlignmentPin, FlipAxis, StockConfig};
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, stock: &mut StockConfig, events: &mut Vec<AppEvent>) {
    ui.heading("Stock Setup");
    ui.separator();

    let mut changed = false;

    // Material picker
    ui.add_space(4.0);
    let catalog = rs_cam_core::material::Material::catalog();
    let current_label = stock.material.label();

    ui.horizontal(|ui| {
        ui.label("Material:");
        egui::ComboBox::from_id_salt("stock_material")
            .selected_text(&current_label)
            .show_ui(ui, |ui| {
                for (label, mat) in &catalog {
                    if ui
                        .selectable_label(stock.material == *mat, *label)
                        .clicked()
                    {
                        stock.material = mat.clone();
                        changed = true;
                        events.push(AppEvent::StockMaterialChanged);
                    }
                }
            });
    });

    // Show material properties (read-only)
    egui::Grid::new("material_info")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Hardness Index:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.label(
                egui::RichText::new(format!("{:.2}", stock.material.hardness_index()))
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.end_row();

            ui.label(
                egui::RichText::new("Kc:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.label(
                egui::RichText::new(format!("{:.1} N/mm\u{00B2}", stock.material.kc_n_per_mm2()))
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            ui.end_row();
        });

    ui.add_space(8.0);

    ui.label("Dimensions:");
    egui::Grid::new("stock_dims")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.x)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.y)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.z)
                        .suffix(" mm")
                        .speed(0.5)
                        .range(0.1..=10000.0),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.label("Origin:");
    egui::Grid::new("stock_origin")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_x)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_y)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.origin_z)
                        .suffix(" mm")
                        .speed(0.5),
                )
                .changed();
            ui.end_row();
        });

    ui.add_space(8.0);
    changed |= ui
        .checkbox(&mut stock.auto_from_model, "Auto from model")
        .changed();
    if stock.auto_from_model {
        ui.horizontal(|ui| {
            ui.label("Padding:");
            changed |= ui
                .add(
                    egui::DragValue::new(&mut stock.padding)
                        .suffix(" mm")
                        .speed(0.1)
                        .range(0.0..=100.0),
                )
                .changed();
        });
    }

    if changed {
        events.push(AppEvent::StockChanged);
    }

    ui.add_space(12.0);
    draw_alignment_pins(ui, stock, events);
}

/// Draw the "Alignment Pins" collapsible section in the stock panel.
fn draw_alignment_pins(ui: &mut egui::Ui, stock: &mut StockConfig, events: &mut Vec<AppEvent>) {
    let header = egui::RichText::new("Alignment Pins")
        .strong()
        .color(egui::Color32::from_rgb(180, 180, 195));

    egui::CollapsingHeader::new(header)
        .default_open(true)
        .show(ui, |ui| {
            let mut changed = false;

            // Flip axis dropdown
            let flip_label = match stock.flip_axis {
                Some(fa) => fa.label(),
                None => "None",
            };
            ui.horizontal(|ui| {
                ui.label("Flip axis:");
                egui::ComboBox::from_id_salt("flip_axis")
                    .selected_text(flip_label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(stock.flip_axis.is_none(), "None")
                            .clicked()
                        {
                            stock.flip_axis = None;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                stock.flip_axis == Some(FlipAxis::Horizontal),
                                "Horizontal",
                            )
                            .clicked()
                        {
                            stock.flip_axis = Some(FlipAxis::Horizontal);
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                stock.flip_axis == Some(FlipAxis::Vertical),
                                "Vertical",
                            )
                            .clicked()
                        {
                            stock.flip_axis = Some(FlipAxis::Vertical);
                            changed = true;
                        }
                    });
            });

            ui.add_space(4.0);

            // Shared pin diameter (physical dowels are one size)
            if !stock.alignment_pins.is_empty() {
                let mut shared_diameter = stock.alignment_pins[0].diameter;
                ui.horizontal(|ui| {
                    ui.label("Pin diameter:");
                    if ui
                        .add(
                            egui::DragValue::new(&mut shared_diameter)
                                .suffix(" mm")
                                .speed(0.1)
                                .range(1.0..=25.0),
                        )
                        .changed()
                    {
                        for pin in stock.alignment_pins.iter_mut() {
                            pin.diameter = shared_diameter;
                        }
                        changed = true;
                    }
                });
                ui.add_space(4.0);
            }

            // Pin list
            let mut remove_idx: Option<usize> = None;
            let mut mirror_idx: Option<usize> = None;

            for (i, pin) in stock.alignment_pins.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Pin {}:", i + 1));
                        ui.label("X");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut pin.x)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.x),
                            )
                            .changed();
                        ui.label("Y");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut pin.y)
                                    .suffix(" mm")
                                    .speed(0.5)
                                    .range(0.0..=stock.y),
                            )
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(48.0);
                        if stock.flip_axis.is_some() && ui.small_button("Mirror").clicked() {
                            mirror_idx = Some(i);
                        }
                        if ui
                            .small_button(
                                egui::RichText::new("Remove")
                                    .color(egui::Color32::from_rgb(200, 100, 100)),
                            )
                            .clicked()
                        {
                            remove_idx = Some(i);
                        }
                    });
                    ui.add_space(2.0);
                });
            }

            // Process deferred actions (borrow after mutable iteration is done)
            if let Some((idx, axis)) = mirror_idx.zip(stock.flip_axis) {
                let src = &stock.alignment_pins[idx];
                let mirrored = mirror_pin(src, axis, stock.x, stock.y);
                // Check if a pin already exists near the mirrored position
                let existing = stock.alignment_pins.iter_mut().enumerate().find(|(j, p)| {
                    *j != idx && (p.x - mirrored.x).abs() < 0.5 && (p.y - mirrored.y).abs() < 0.5
                });
                if let Some((_j, existing_pin)) = existing {
                    // Snap the existing pin to exact mirrored position
                    existing_pin.x = mirrored.x;
                    existing_pin.y = mirrored.y;
                } else {
                    stock.alignment_pins.push(mirrored);
                }
                changed = true;
            }

            if let Some(idx) = remove_idx {
                stock.alignment_pins.remove(idx);
                changed = true;
            }

            // Buttons row
            ui.add_space(4.0);
            if ui.small_button("+ Add Pin").clicked() {
                let default_diameter = stock
                    .alignment_pins
                    .first()
                    .map(|p| p.diameter)
                    .unwrap_or(6.0);
                stock.alignment_pins.push(AlignmentPin::new(
                    stock.x / 2.0,
                    stock.y / 2.0,
                    default_diameter,
                ));
                changed = true;
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let pin_count_id = ui.id().with("auto_place_count");
                let mut count: usize =
                    ui.data_mut(|d| *d.get_persisted_mut_or(pin_count_id, 2_usize));
                if ui
                    .add(
                        egui::DragValue::new(&mut count)
                            .prefix("Pins: ")
                            .range(2..=8),
                    )
                    .changed()
                {
                    ui.data_mut(|d| d.insert_persisted(pin_count_id, count));
                }
                if ui.small_button("Auto-place").clicked() {
                    auto_place_pins(stock, count);
                    changed = true;
                }
            });

            // Symmetry warning
            if let Some(axis) = stock.flip_axis
                && !stock.alignment_pins.is_empty()
                && !pins_are_symmetric(&stock.alignment_pins, axis, stock.x, stock.y)
            {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Pins are not symmetric about the flip axis")
                        .small()
                        .color(egui::Color32::from_rgb(220, 180, 60)),
                );
            }

            // Out-of-bounds warning
            if stock
                .alignment_pins
                .iter()
                .any(|p| p.x < 0.0 || p.x > stock.x || p.y < 0.0 || p.y > stock.y)
            {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("One or more pins are outside the stock bounds")
                        .small()
                        .color(egui::Color32::from_rgb(220, 100, 100)),
                );
            }

            if changed {
                events.push(AppEvent::StockChanged);
            }
        });
}

/// Create the mirror of a pin about the flip axis.
fn mirror_pin(pin: &AlignmentPin, axis: FlipAxis, stock_x: f64, stock_y: f64) -> AlignmentPin {
    match axis {
        // Horizontal flip: mirror about the X centerline → Y is reflected
        FlipAxis::Horizontal => AlignmentPin::new(pin.x, stock_y - pin.y, pin.diameter),
        // Vertical flip: mirror about the Y centerline → X is reflected
        FlipAxis::Vertical => AlignmentPin::new(stock_x - pin.x, pin.y, pin.diameter),
    }
}

/// Place `count` pins evenly distributed in the stock margin (padding area).
fn auto_place_pins(stock: &mut StockConfig, count: usize) {
    // Place pins in the center of the padding margin so they hit excess
    // stock, not the model. Fall back to 10mm if padding is too small.
    let margin = if stock.padding > 2.0 {
        stock.padding / 2.0
    } else {
        10.0_f64.min(stock.x / 4.0).min(stock.y / 4.0)
    };
    let diameter = stock
        .alignment_pins
        .first()
        .map(|p| p.diameter)
        .unwrap_or(6.0);

    stock.alignment_pins.clear();

    match stock.flip_axis {
        Some(FlipAxis::Horizontal) => {
            // Pins along the flip axis centerline (Y = stock.y/2),
            // evenly spaced from left margin to right margin.
            let cy = stock.y / 2.0;
            let x_start = margin;
            let x_end = stock.x - margin;
            if count == 1 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(stock.x / 2.0, cy, diameter));
            } else {
                let step = (x_end - x_start) / (count - 1) as f64;
                for i in 0..count {
                    stock.alignment_pins.push(AlignmentPin::new(
                        x_start + step * i as f64,
                        cy,
                        diameter,
                    ));
                }
            }
        }
        Some(FlipAxis::Vertical) => {
            // Pins along the flip axis centerline (X = stock.x/2),
            // evenly spaced from front margin to back margin.
            let cx = stock.x / 2.0;
            let y_start = margin;
            let y_end = stock.y - margin;
            if count == 1 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(cx, stock.y / 2.0, diameter));
            } else {
                let step = (y_end - y_start) / (count - 1) as f64;
                for i in 0..count {
                    stock.alignment_pins.push(AlignmentPin::new(
                        cx,
                        y_start + step * i as f64,
                        diameter,
                    ));
                }
            }
        }
        None => {
            // No flip axis — distribute pins around the perimeter.
            // 2 pins: diagonal corners. 3+: spread along edges.
            if count <= 2 {
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(margin, margin, diameter));
                if count == 2 {
                    stock.alignment_pins.push(AlignmentPin::new(
                        stock.x - margin,
                        stock.y - margin,
                        diameter,
                    ));
                }
            } else {
                // Place pins at evenly spaced positions around the perimeter
                let corners: &[[f64; 2]] = &[
                    [margin, margin],
                    [stock.x - margin, margin],
                    [stock.x - margin, stock.y - margin],
                    [margin, stock.y - margin],
                ];
                for i in 0..count {
                    let t = i as f64 / count as f64 * 4.0;
                    let seg = t.floor() as usize % 4;
                    let frac = t - seg as f64;
                    let [x0, y0] = corners[seg];
                    let [x1, y1] = corners[(seg + 1) % 4];
                    stock.alignment_pins.push(AlignmentPin::new(
                        x0 + (x1 - x0) * frac,
                        y0 + (y1 - y0) * frac,
                        diameter,
                    ));
                }
            }
        }
    }
}

/// Check if pins are symmetric about the flip axis (within tolerance).
fn pins_are_symmetric(pins: &[AlignmentPin], axis: FlipAxis, stock_x: f64, stock_y: f64) -> bool {
    const TOL: f64 = 0.5; // mm tolerance

    // For each pin, check that its mirror exists in the set
    for pin in pins {
        let m = mirror_pin(pin, axis, stock_x, stock_y);
        let has_mirror = pins
            .iter()
            .any(|p| (p.x - m.x).abs() < TOL && (p.y - m.y).abs() < TOL);
        if !has_mirror {
            return false;
        }
    }
    true
}
