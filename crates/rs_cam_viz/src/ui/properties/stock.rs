use crate::state::job::StockConfig;
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
}
