use super::AppEvent;
use crate::state::AppState;
use crate::state::job::ToolType;
use crate::state::selection::Selection;
use crate::state::toolpath::{ComputeStatus, OperationType};

pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Project");
    ui.separator();

    ui.label(
        egui::RichText::new(format!("Job: {}", state.job.name))
            .strong()
            .color(egui::Color32::from_rgb(200, 200, 210)),
    );

    ui.add_space(4.0);

    // Stock
    if ui
        .selectable_label(state.selection == Selection::Stock, format!(
            "Stock ({:.0} x {:.0} x {:.0} mm)",
            state.job.stock.x, state.job.stock.y, state.job.stock.z
        ))
        .clicked()
    {
        events.push(AppEvent::Select(Selection::Stock));
    }

    // Post processor
    if ui
        .selectable_label(
            state.selection == Selection::PostProcessor,
            format!("Post Processor: {}", state.job.post.format.label()),
        )
        .clicked()
    {
        events.push(AppEvent::Select(Selection::PostProcessor));
    }

    ui.add_space(4.0);

    // Models
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
            if ui
                .selectable_label(selected, format!("[{}] {}", icon, model.name))
                .clicked()
            {
                events.push(AppEvent::Select(Selection::Model(model.id)));
            }
        }
    });

    // Tool library
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
            let response = ui.selectable_label(selected, tool.summary());
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

    // Toolpaths
    ui.collapsing("Toolpaths", |ui| {
        if state.job.toolpaths.is_empty() {
            ui.label(
                egui::RichText::new("No toolpaths")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }
        for (i, tp) in state.job.toolpaths.iter().enumerate() {
            let selected = state.selection == Selection::Toolpath(tp.id);
            let status_icon = match &tp.status {
                ComputeStatus::Pending => "  ",
                ComputeStatus::Computing(_) => "~ ",
                ComputeStatus::Done => "* ",
                ComputeStatus::Error(_) => "! ",
            };
            let label = format!("[{}] {}{}", i + 1, status_icon, tp.name);
            let response = ui.selectable_label(selected, &label);
            if response.clicked() {
                events.push(AppEvent::Select(Selection::Toolpath(tp.id)));
            }
            response.context_menu(|ui| {
                let vis_label = if tp.visible { "Hide" } else { "Show" };
                if ui.button(vis_label).clicked() {
                    events.push(AppEvent::ToggleToolpathVisibility(tp.id));
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    events.push(AppEvent::RemoveToolpath(tp.id));
                    ui.close_menu();
                }
            });
        }

        ui.add_space(4.0);
        ui.menu_button("+ Add Toolpath", |ui| {
            ui.label(egui::RichText::new("2.5D (from SVG)").strong());
            for &op in OperationType::ALL_2D {
                if ui.button(op.label()).clicked() {
                    events.push(AppEvent::AddToolpath(op));
                    ui.close_menu();
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("3D (from STL)").strong());
            for &op in OperationType::ALL_3D {
                if ui.button(op.label()).clicked() {
                    events.push(AppEvent::AddToolpath(op));
                    ui.close_menu();
                }
            }
        });
    });

    ui.add_space(8.0);

    // Import buttons
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("+ STL").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("STL", &["stl", "STL"])
                .pick_file()
            {
                events.push(AppEvent::ImportStl(path));
            }
        }
        if ui.small_button("+ SVG").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG", &["svg", "SVG"])
                .pick_file()
            {
                events.push(AppEvent::ImportSvg(path));
            }
        }
        if ui.small_button("+ DXF").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf", "DXF"])
                .pick_file()
            {
                events.push(AppEvent::ImportDxf(path));
            }
        }
    });
}
