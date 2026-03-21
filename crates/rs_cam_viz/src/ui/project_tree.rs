use super::AppEvent;
use crate::render::toolpath_render::palette_color;
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
        .selectable_label(
            state.selection == Selection::Stock,
            format!(
                "Stock ({:.0} x {:.0} x {:.0} mm)",
                state.job.stock.x, state.job.stock.y, state.job.stock.z
            ),
        )
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

    // Machine
    if ui
        .selectable_label(
            state.selection == Selection::Machine,
            format!("Machine: {}", state.job.machine.name),
        )
        .clicked()
    {
        events.push(AppEvent::Select(Selection::Machine));
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

            // Status indicator: colored unicode circle
            let (status_icon, status_color) = match &tp.status {
                ComputeStatus::Pending => ("\u{25CB}", egui::Color32::from_rgb(120, 120, 130)), // hollow circle
                ComputeStatus::Computing => ("\u{25CF}", egui::Color32::from_rgb(200, 180, 80)), // yellow filled
                ComputeStatus::Done => ("\u{25CF}", egui::Color32::from_rgb(80, 180, 80)), // green filled
                ComputeStatus::Error(_) => ("\u{25CF}", egui::Color32::from_rgb(220, 80, 80)), // red filled
            };

            // Palette color swatch matching viewport color
            let pc = palette_color(i);
            let swatch_color = egui::Color32::from_rgb(
                (pc[0] * 255.0) as u8,
                (pc[1] * 255.0) as u8,
                (pc[2] * 255.0) as u8,
            );

            // Visibility/enabled indicators
            let dim = !tp.enabled || !tp.visible;

            let response = ui
                .horizontal(|ui| {
                    // Palette color swatch (small colored square)
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, swatch_color);

                    // Status circle
                    ui.label(
                        egui::RichText::new(status_icon)
                            .color(status_color)
                            .size(10.0),
                    );

                    // Toolpath name
                    let text_color = if dim {
                        egui::Color32::from_rgb(100, 100, 110)
                    } else if selected {
                        egui::Color32::from_rgb(220, 220, 230)
                    } else {
                        egui::Color32::from_rgb(180, 180, 190)
                    };
                    let label = format!("[{}] {}", i + 1, tp.name);
                    let resp = ui
                        .selectable_label(selected, egui::RichText::new(&label).color(text_color));
                    if resp.clicked() {
                        events.push(AppEvent::Select(Selection::Toolpath(tp.id)));
                    }
                    resp
                })
                .inner;

            response.context_menu(|ui| {
                let vis_label = if tp.visible { "Hide" } else { "Show" };
                if ui.button(vis_label).clicked() {
                    events.push(AppEvent::ToggleToolpathVisibility(tp.id));
                    ui.close_menu();
                }
                let en_label = if tp.enabled { "Disable" } else { "Enable" };
                if ui.button(en_label).clicked() {
                    events.push(AppEvent::ToggleToolpathEnabled(tp.id));
                    ui.close_menu();
                }
                if ui.button("Duplicate").clicked() {
                    events.push(AppEvent::DuplicateToolpath(tp.id));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Move Up").clicked() {
                    events.push(AppEvent::MoveToolpathUp(tp.id));
                    ui.close_menu();
                }
                if ui.button("Move Down").clicked() {
                    events.push(AppEvent::MoveToolpathDown(tp.id));
                    ui.close_menu();
                }
                ui.separator();
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
        if ui.small_button("+ STL").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("STL", &["stl", "STL"])
                .pick_file()
        {
            events.push(AppEvent::ImportStl(path));
        }
        if ui.small_button("+ SVG").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG", &["svg", "SVG"])
                .pick_file()
        {
            events.push(AppEvent::ImportSvg(path));
        }
        if ui.small_button("+ DXF").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf", "DXF"])
                .pick_file()
        {
            events.push(AppEvent::ImportDxf(path));
        }
    });
}
