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

            ui.label("Safe Z:").on_hover_text(
                "Global clearance plane for rapid moves between operations. Different from per-operation Retract Z."
            );
            ui.add(
                egui::DragValue::new(&mut post.safe_z)
                    .suffix(" mm")
                    .speed(0.5)
                    .range(0.0..=500.0),
            );
            ui.end_row();
        });

    ui.add_space(8.0);
    ui.checkbox(&mut post.high_feedrate_mode, "High Feedrate Mode (G0→G1)")
        .on_hover_text("Replace rapids (G0) with G1 at high feedrate for machines with unpredictable rapid motion");
    if post.high_feedrate_mode {
        egui::Grid::new("high_feed_p")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("  High Feed:");
                ui.add(
                    egui::DragValue::new(&mut post.high_feedrate)
                        .suffix(" mm/min")
                        .speed(50.0)
                        .range(500.0..=20000.0),
                );
                ui.end_row();
            });
    }
}
