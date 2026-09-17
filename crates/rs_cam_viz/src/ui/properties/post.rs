use crate::state::job::{PostConfig, PostFormat};
use crate::ui::components::UiExt as _;
use crate::ui::components::ValueRow;
use crate::ui::theme;
use rs_cam_core::compute::config::{SAFE_Z_CLEARANCE_MM, effective_safe_z};

pub fn draw(ui: &mut egui::Ui, post: &mut PostConfig, stock_top_z: f64) {
    ui.heading("Post Processor");
    ui.separator();

    ui.param_grid("post_params", |ui| {
        ui.label("Format:");
        egui::ComboBox::from_id_salt("post_format")
            .selected_text(post.format.label())
            .show_ui(ui, |ui| {
                for &fmt in PostFormat::ALL {
                    ui.selectable_value(&mut post.format, fmt, fmt.label());
                }
            });
        ui.end_row();

        // UI-10: `spindle_speed` is a `u32`. `ValueRow` edits an `f64`, and a
        // temporary f64 would change what a partial edit commits, so the row
        // keeps its own DragValue.
        ui.label("Spindle Speed:");
        ui.add(
            egui::DragValue::new(&mut post.spindle_speed)
                .suffix(" RPM")
                .speed(100)
                .range(0..=60000),
        );
        ui.end_row();

        ValueRow::new("Safe Z:", &mut post.safe_z, " mm", 0.5, 0.0..=500.0)
            .tooltip(Some(
                "Global clearance plane for rapid moves between operations.",
            ))
            .show(ui);
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
        ui.param_grid("high_feed_p", |ui| {
            ValueRow::new(
                "  High Feed:",
                &mut post.high_feedrate,
                " mm/min",
                50.0,
                500.0..=20000.0,
            )
            .show(ui);
        });
    }
}
