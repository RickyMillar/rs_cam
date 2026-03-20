use super::AppEvent;
use crate::state::AppState;
use crate::state::job::ToolType;
use crate::state::selection::Selection;

pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Project");
    ui.separator();

    // Job name
    ui.label(
        egui::RichText::new(format!("Job: {}", state.job.name))
            .strong()
            .color(egui::Color32::from_rgb(200, 200, 210)),
    );

    ui.add_space(4.0);

    // Stock
    let stock_selected = state.selection == Selection::Stock;
    let stock_label = format!(
        "Stock ({:.0} x {:.0} x {:.0} mm)",
        state.job.stock.x, state.job.stock.y, state.job.stock.z
    );
    if ui
        .selectable_label(stock_selected, &stock_label)
        .clicked()
    {
        events.push(AppEvent::Select(Selection::Stock));
    }

    // Post processor
    let post_selected = state.selection == Selection::PostProcessor;
    let post_label = format!("Post Processor: {}", state.job.post.format.label());
    if ui.selectable_label(post_selected, &post_label).clicked() {
        events.push(AppEvent::Select(Selection::PostProcessor));
    }

    ui.add_space(4.0);

    // Models section
    ui.collapsing("Models", |ui| {
        if state.job.models.is_empty() {
            ui.label(
                egui::RichText::new("No models imported")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        for model in &state.job.models {
            let selected = state.selection == Selection::Model(model.id);
            let icon = match model.kind {
                crate::state::job::ModelKind::Stl => "STL",
                crate::state::job::ModelKind::Svg => "SVG",
                crate::state::job::ModelKind::Dxf => "DXF",
            };
            let label = format!("[{}] {}", icon, model.name);
            if ui.selectable_label(selected, &label).clicked() {
                events.push(AppEvent::Select(Selection::Model(model.id)));
            }
        }
    });

    // Tool library section
    ui.collapsing("Tool Library", |ui| {
        if state.job.tools.is_empty() {
            ui.label(
                egui::RichText::new("No tools defined")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        for tool in &state.job.tools {
            let selected = state.selection == Selection::Tool(tool.id);
            let label = tool.summary();
            let response = ui.selectable_label(selected, &label);
            if response.clicked() {
                events.push(AppEvent::Select(Selection::Tool(tool.id)));
            }
            response.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    events.push(AppEvent::DuplicateTool(tool.id));
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveTool(tool.id));
                    ui.close_menu();
                }
            });
        }

        ui.add_space(4.0);
        ui.menu_button("+ Add Tool", |ui| {
            for &tt in ToolType::ALL {
                if ui.button(tt.label()).clicked() {
                    events.push(AppEvent::AddTool(tt));
                    ui.close_menu();
                }
            }
        });
    });

    ui.add_space(8.0);

    // Import button
    if ui.button("+ Import STL...").clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("STL Files", &["stl", "STL"])
            .pick_file()
        {
            events.push(AppEvent::ImportStl(path));
        }
    }
}
