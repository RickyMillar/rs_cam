use crate::state::job::{PostConfig, PostFormat};

pub fn draw(ui: &mut egui::Ui, post: &mut PostConfig) {
    ui.heading("Post Processor");
    ui.separator();

    egui::Grid::new("post_params")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Format:");
            egui::ComboBox::from_id_salt("post_format")
                .selected_text(post.format.label())
                .show_ui(ui, |ui| {
                    for &fmt in PostFormat::ALL {
                        ui.selectable_value(&mut post.format, fmt, fmt.label());
                    }
                });
            ui.end_row();

            ui.label("Spindle Speed:");
            ui.add(
                egui::DragValue::new(&mut post.spindle_speed)
                    .suffix(" RPM")
                    .speed(100)
                    .range(0..=60000),
            );
            ui.end_row();

            ui.label("Safe Z:");
            ui.add(
                egui::DragValue::new(&mut post.safe_z)
                    .suffix(" mm")
                    .speed(0.5)
                    .range(0.0..=500.0),
            );
            ui.end_row();
        });
}
