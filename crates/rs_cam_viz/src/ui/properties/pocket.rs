use crate::state::job::JobState;
use crate::state::toolpath::{ComputeStatus, PocketConfig, PocketPattern, ToolpathEntry};
use crate::ui::AppEvent;

pub fn draw(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    job: &JobState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading(&entry.name);
    ui.separator();

    // Editable name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    // Tool selector
    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tool_label = job
            .tools
            .iter()
            .find(|t| t.id == entry.tool_id)
            .map(|t| t.summary())
            .unwrap_or_else(|| "(none)".to_string());
        egui::ComboBox::from_id_salt("tp_tool")
            .selected_text(&tool_label)
            .show_ui(ui, |ui| {
                for tool in &job.tools {
                    ui.selectable_value(&mut entry.tool_id, tool.id, tool.summary());
                }
            });
    });

    // Input model
    ui.horizontal(|ui| {
        ui.label("Input:");
        let model_label = job
            .models
            .iter()
            .find(|m| m.id == entry.model_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "(none)".to_string());
        egui::ComboBox::from_id_salt("tp_model")
            .selected_text(&model_label)
            .show_ui(ui, |ui| {
                for model in &job.models {
                    ui.selectable_value(&mut entry.model_id, model.id, &model.name);
                }
            });
    });

    ui.add_space(8.0);

    // Pocket-specific parameters
    let crate::state::toolpath::OperationConfig::Pocket(cfg) = &mut entry.operation;
    draw_pocket_params(ui, cfg);

    ui.add_space(12.0);

    // Generate button
    ui.horizontal(|ui| {
        let can_generate = job.tools.iter().any(|t| t.id == entry.tool_id);
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }

        match &entry.status {
            ComputeStatus::Pending => {
                ui.label("Ready");
            }
            ComputeStatus::Computing => {
                ui.label("Computing...");
            }
            ComputeStatus::Done => {
                ui.label(egui::RichText::new("Done").color(egui::Color32::from_rgb(100, 180, 100)));
            }
            ComputeStatus::Error(e) => {
                ui.label(
                    egui::RichText::new(format!("Error: {}", e))
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                );
            }
        }
    });

    // Stats
    if let Some(result) = &entry.result {
        ui.add_space(4.0);
        ui.label(format!("Moves: {}", result.stats.move_count));
        ui.label(format!("Cutting: {:.0} mm", result.stats.cutting_distance));
        ui.label(format!("Rapid: {:.0} mm", result.stats.rapid_distance));
    }
}

fn draw_pocket_params(ui: &mut egui::Ui, cfg: &mut PocketConfig) {
    ui.label(
        egui::RichText::new("Cutting Parameters")
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 195)),
    );

    egui::Grid::new("pocket_params")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Pattern:");
            egui::ComboBox::from_id_salt("pocket_pattern")
                .selected_text(match cfg.pattern {
                    PocketPattern::Contour => "Contour",
                    PocketPattern::Zigzag => "Zigzag",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.pattern, PocketPattern::Contour, "Contour");
                    ui.selectable_value(&mut cfg.pattern, PocketPattern::Zigzag, "Zigzag");
                });
            ui.end_row();

            ui.label("Stepover:");
            ui.add(
                egui::DragValue::new(&mut cfg.stepover)
                    .suffix(" mm")
                    .speed(0.1)
                    .range(0.05..=50.0),
            );
            ui.end_row();

            ui.label("Depth:");
            ui.add(
                egui::DragValue::new(&mut cfg.depth)
                    .suffix(" mm")
                    .speed(0.1)
                    .range(0.1..=100.0),
            );
            ui.end_row();

            ui.label("Depth per Pass:");
            ui.add(
                egui::DragValue::new(&mut cfg.depth_per_pass)
                    .suffix(" mm")
                    .speed(0.1)
                    .range(0.1..=50.0),
            );
            ui.end_row();

            ui.label("Feed Rate:");
            ui.add(
                egui::DragValue::new(&mut cfg.feed_rate)
                    .suffix(" mm/min")
                    .speed(10.0)
                    .range(1.0..=50000.0),
            );
            ui.end_row();

            ui.label("Plunge Rate:");
            ui.add(
                egui::DragValue::new(&mut cfg.plunge_rate)
                    .suffix(" mm/min")
                    .speed(10.0)
                    .range(1.0..=10000.0),
            );
            ui.end_row();

            ui.label("Climb Milling:");
            ui.checkbox(&mut cfg.climb, "");
            ui.end_row();

            if cfg.pattern == PocketPattern::Zigzag {
                ui.label("Angle:");
                ui.add(
                    egui::DragValue::new(&mut cfg.angle)
                        .suffix(" deg")
                        .speed(1.0)
                        .range(0.0..=360.0),
                );
                ui.end_row();
            }
        });
}
