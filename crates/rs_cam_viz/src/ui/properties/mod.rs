pub mod stock;
pub mod tool;
pub mod post;
pub mod pocket;

use crate::state::AppState;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

pub fn draw(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    match state.selection.clone() {
        Selection::None => {
            ui.label(
                egui::RichText::new("Select an item in the project tree")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        Selection::Stock => {
            stock::draw(ui, &mut state.job.stock, events);
        }
        Selection::PostProcessor => {
            post::draw(ui, &mut state.job.post);
        }
        Selection::Model(id) => {
            if let Some(model) = state.job.models.iter().find(|m| m.id == id) {
                ui.heading(&model.name);
                ui.separator();
                ui.label(format!("Type: {:?}", model.kind));
                ui.label(format!("Path: {}", model.path.display()));
                if let Some(mesh) = &model.mesh {
                    ui.add_space(4.0);
                    ui.label(format!("Vertices: {}", mesh.vertices.len()));
                    ui.label(format!("Triangles: {}", mesh.triangles.len()));
                    let bb = &mesh.bbox;
                    ui.label(format!(
                        "Bounds: {:.1} x {:.1} x {:.1} mm",
                        bb.max.x - bb.min.x,
                        bb.max.y - bb.min.y,
                        bb.max.z - bb.min.z
                    ));
                }
            }
        }
        Selection::Tool(id) => {
            if let Some(tool) = state.job.tools.iter_mut().find(|t| t.id == id) {
                tool::draw(ui, tool);
            }
        }
        Selection::Toolpath(id) => {
            // Need split borrow: toolpath entry (mut) + job (read for tool/model lists)
            let tp_idx = state.job.toolpaths.iter().position(|tp| tp.id == id);
            if let Some(idx) = tp_idx {
                // Split the toolpaths vec to get mutable entry + immutable job context
                let (_before, rest) = state.job.toolpaths.split_at_mut(idx);
                let (entry, _after) = rest.split_first_mut().unwrap();
                // We can't borrow job.tools while job.toolpaths is borrowed mutably,
                // so we snapshot what we need
                let tools_snapshot: Vec<_> = state
                    .job
                    .tools
                    .iter()
                    .map(|t| (t.id, t.summary()))
                    .collect();
                let models_snapshot: Vec<_> = state
                    .job
                    .models
                    .iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect();

                // Inline draw since we can't easily pass split borrows to pocket::draw
                draw_toolpath_props(ui, entry, &tools_snapshot, &models_snapshot, events);
            }
        }
    }
}

fn draw_toolpath_props(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    tools: &[(crate::state::job::ToolId, String)],
    models: &[(crate::state::job::ModelId, String)],
    events: &mut Vec<AppEvent>,
) {
    use crate::state::toolpath::{ComputeStatus, OperationConfig, PocketPattern};

    ui.heading(&entry.name);
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    ui.horizontal(|ui| {
        ui.label("Tool:");
        let tool_label = tools
            .iter()
            .find(|(id, _)| *id == entry.tool_id)
            .map(|(_, s)| s.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_tool")
            .selected_text(tool_label)
            .show_ui(ui, |ui| {
                for (id, name) in tools {
                    ui.selectable_value(&mut entry.tool_id, *id, name.as_str());
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("Input:");
        let model_label = models
            .iter()
            .find(|(id, _)| *id == entry.model_id)
            .map(|(_, s)| s.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("tp_model")
            .selected_text(model_label)
            .show_ui(ui, |ui| {
                for (id, name) in models {
                    ui.selectable_value(&mut entry.model_id, *id, name.as_str());
                }
            });
    });

    ui.add_space(8.0);

    let OperationConfig::Pocket(cfg) = &mut entry.operation;
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
                ui.add(egui::DragValue::new(&mut cfg.stepover).suffix(" mm").speed(0.1).range(0.05..=50.0));
                ui.end_row();

                ui.label("Depth:");
                ui.add(egui::DragValue::new(&mut cfg.depth).suffix(" mm").speed(0.1).range(0.1..=100.0));
                ui.end_row();

                ui.label("Depth per Pass:");
                ui.add(egui::DragValue::new(&mut cfg.depth_per_pass).suffix(" mm").speed(0.1).range(0.1..=50.0));
                ui.end_row();

                ui.label("Feed Rate:");
                ui.add(egui::DragValue::new(&mut cfg.feed_rate).suffix(" mm/min").speed(10.0).range(1.0..=50000.0));
                ui.end_row();

                ui.label("Plunge Rate:");
                ui.add(egui::DragValue::new(&mut cfg.plunge_rate).suffix(" mm/min").speed(10.0).range(1.0..=10000.0));
                ui.end_row();

                ui.label("Climb Milling:");
                ui.checkbox(&mut cfg.climb, "");
                ui.end_row();

                if cfg.pattern == PocketPattern::Zigzag {
                    ui.label("Angle:");
                    ui.add(egui::DragValue::new(&mut cfg.angle).suffix(" deg").speed(1.0).range(0.0..=360.0));
                    ui.end_row();
                }
            });

    ui.add_space(12.0);

    // Generate button + status
    ui.horizontal(|ui| {
        let can_generate = !tools.is_empty();
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }

        match &entry.status {
            ComputeStatus::Pending => { ui.label("Ready"); }
            ComputeStatus::Computing(pct) => { ui.label(format!("Computing... {:.0}%", pct * 100.0)); }
            ComputeStatus::Done => {
                ui.label(egui::RichText::new("Done").color(egui::Color32::from_rgb(100, 180, 100)));
            }
            ComputeStatus::Error(e) => {
                ui.label(egui::RichText::new(format!("Error: {}", e)).color(egui::Color32::from_rgb(220, 80, 80)));
            }
        }
    });

    if let Some(result) = &entry.result {
        ui.add_space(4.0);
        ui.label(format!("Moves: {}", result.stats.move_count));
        ui.label(format!("Cutting: {:.0} mm", result.stats.cutting_distance));
        ui.label(format!("Rapid: {:.0} mm", result.stats.rapid_distance));
    }
}
