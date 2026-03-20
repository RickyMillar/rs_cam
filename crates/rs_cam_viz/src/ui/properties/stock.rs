use crate::state::job::StockConfig;
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, stock: &mut StockConfig, events: &mut Vec<AppEvent>) {
    ui.heading("Stock Setup");
    ui.separator();

    let mut changed = false;

    ui.label("Dimensions:");
    egui::Grid::new("stock_dims")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("X:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.x).suffix(" mm").speed(0.5).range(0.1..=10000.0))
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.y).suffix(" mm").speed(0.5).range(0.1..=10000.0))
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.z).suffix(" mm").speed(0.5).range(0.1..=10000.0))
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
                .add(egui::DragValue::new(&mut stock.origin_x).suffix(" mm").speed(0.5))
                .changed();
            ui.end_row();

            ui.label("Y:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.origin_y).suffix(" mm").speed(0.5))
                .changed();
            ui.end_row();

            ui.label("Z:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.origin_z).suffix(" mm").speed(0.5))
                .changed();
            ui.end_row();
        });

    ui.add_space(8.0);
    changed |= ui.checkbox(&mut stock.auto_from_model, "Auto from model").changed();
    if stock.auto_from_model {
        ui.horizontal(|ui| {
            ui.label("Padding:");
            changed |= ui
                .add(egui::DragValue::new(&mut stock.padding).suffix(" mm").speed(0.1).range(0.0..=100.0))
                .changed();
        });
    }

    if changed {
        events.push(AppEvent::StockChanged);
    }
}
