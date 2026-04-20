use crate::state::job::{PostConfig, PostFormat};
use crate::ui::theme;
use rs_cam_core::compute::config::{SAFE_Z_CLEARANCE_MM, effective_safe_z};

pub fn draw(ui: &mut egui::Ui, post: &mut PostConfig, stock_top_z: f64) {
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

    // Surface the compute-time clamp: `effective_safe_z` floors the user's
    // raw safe_z at `stock_top + SAFE_Z_CLEARANCE_MM`. If that's higher than
    // what the user entered, the UI value is misleading — show the actual
    // effective value so the user knows what compute is using.
    let effective = effective_safe_z(post.safe_z, stock_top_z);
    if effective > post.safe_z + 1e-6 {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "\u{26A0} Safe Z raised to {:.2} mm to clear stock (top {:.2} + {:.0} mm)",
                effective, stock_top_z, SAFE_Z_CLEARANCE_MM
            ))
            .small()
            .color(theme::WARNING),
        )
        .on_hover_text(
            "Compute clamps Safe Z so rapids clear the uncut stock. Increase the value above the stock top to silence this warning.",
        );
    } else {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("Effective: {:.2} mm", effective))
                .small()
                .color(theme::TEXT_DIM),
        );
    }

    ui.add_space(8.0);
    ui.checkbox(
        &mut post.high_feedrate_mode,
        "Safe Rapids (G0 → G1 at feed speed)",
    )
    .on_hover_text(
        "Replace rapids (G0) with G1 at high feedrate for machines with unpredictable rapid motion",
    );
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
