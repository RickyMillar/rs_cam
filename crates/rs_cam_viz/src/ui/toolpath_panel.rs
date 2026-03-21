use super::AppEvent;
use crate::render::toolpath_render::palette_color;
use crate::state::AppState;
use crate::state::selection::Selection;
use crate::state::toolpath::{ComputeStatus, OperationType};

/// Left panel for the Toolpath workspace: operation queue with status chips.
pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    ui.heading("Operations");
    ui.separator();

    // Action bar: generate all + add toolpath
    ui.horizontal(|ui| {
        if ui.button("Generate All").clicked() {
            events.push(AppEvent::GenerateAll);
        }
        ui.menu_button("+ Add", |ui| {
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

    ui.add_space(6.0);

    let multi_setup = state.job.setups.len() > 1;
    let mut global_idx = 0usize;

    for setup in &state.job.setups {
        // Setup header (only if multi-setup)
        if multi_setup {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&setup.name)
                        .strong()
                        .color(egui::Color32::from_rgb(160, 170, 200)),
                );
                // Count ready/total
                let ready = setup
                    .toolpaths
                    .iter()
                    .filter(|tp| matches!(tp.status, ComputeStatus::Done))
                    .count();
                let total = setup.toolpaths.len();
                ui.label(
                    egui::RichText::new(format!("{ready}/{total}"))
                        .small()
                        .color(egui::Color32::from_rgb(120, 120, 135)),
                );
            });
            ui.separator();
        }

        if setup.toolpaths.is_empty() {
            ui.label(
                egui::RichText::new("No toolpaths")
                    .italics()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        }

        for tp in &setup.toolpaths {
            let i = global_idx;
            global_idx += 1;
            let selected = state.selection == Selection::Toolpath(tp.id);
            let dim = !tp.enabled || !tp.visible;

            let pc = palette_color(i);
            let swatch_color = egui::Color32::from_rgb(
                (pc[0] * 255.0) as u8,
                (pc[1] * 255.0) as u8,
                (pc[2] * 255.0) as u8,
            );

            let border_color = if selected {
                swatch_color
            } else {
                egui::Color32::from_rgb(48, 48, 58)
            };

            egui::Frame::default()
                .fill(if selected {
                    egui::Color32::from_rgb(38, 42, 55)
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::new(1.0, border_color))
                .inner_margin(4.0)
                .rounding(3.0)
                .show(ui, |ui| {
                    // Row 1: swatch + status + name
                    let response = ui
                        .horizontal(|ui| {
                            // Color swatch
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(6.0, 14.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, swatch_color);

                            // Status chip
                            let (status_text, status_color) = match &tp.status {
                                ComputeStatus::Pending => {
                                    ("PEND", egui::Color32::from_rgb(120, 120, 130))
                                }
                                ComputeStatus::Computing => {
                                    ("GEN", egui::Color32::from_rgb(200, 180, 80))
                                }
                                ComputeStatus::Done => ("OK", egui::Color32::from_rgb(80, 180, 80)),
                                ComputeStatus::Error(_) => {
                                    ("ERR", egui::Color32::from_rgb(220, 80, 80))
                                }
                            };
                            ui.label(
                                egui::RichText::new(status_text)
                                    .small()
                                    .strong()
                                    .color(status_color),
                            );

                            // Name
                            let text_color = if dim {
                                egui::Color32::from_rgb(100, 100, 110)
                            } else {
                                egui::Color32::from_rgb(190, 190, 200)
                            };
                            let resp = ui.selectable_label(
                                selected,
                                egui::RichText::new(&tp.name).color(text_color),
                            );
                            if resp.clicked() {
                                events.push(AppEvent::Select(Selection::Toolpath(tp.id)));
                            }
                            resp
                        })
                        .inner;

                    // Row 2: tool info + quick actions
                    ui.horizontal(|ui| {
                        // Tool name
                        if let Some(tool) = state.job.tools.iter().find(|t| t.id == tp.tool_id) {
                            ui.label(
                                egui::RichText::new(tool.summary())
                                    .small()
                                    .color(egui::Color32::from_rgb(130, 130, 145)),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Quick generate button
                            if matches!(tp.status, ComputeStatus::Pending)
                                && ui
                                    .small_button("\u{25B6}")
                                    .on_hover_text("Generate")
                                    .clicked()
                            {
                                events.push(AppEvent::GenerateToolpath(tp.id));
                            }
                        });
                    });

                    // Context menu
                    response.context_menu(|ui| {
                        if ui.button("Generate").clicked() {
                            events.push(AppEvent::GenerateToolpath(tp.id));
                            ui.close_menu();
                        }
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
                });
        }
    }

    // Tool library (compact, collapsed by default)
    ui.add_space(12.0);
    egui::CollapsingHeader::new("Tool Library")
        .default_open(false)
        .show(ui, |ui| {
            for tool in &state.job.tools {
                let selected = state.selection == Selection::Tool(tool.id);
                if ui.selectable_label(selected, tool.summary()).clicked() {
                    events.push(AppEvent::Select(Selection::Tool(tool.id)));
                }
            }
            ui.add_space(4.0);
            ui.menu_button("+ Add Tool", |ui| {
                for &tt in crate::state::job::ToolType::ALL {
                    if ui.button(tt.label()).clicked() {
                        events.push(AppEvent::AddTool(tt));
                        ui.close_menu();
                    }
                }
            });
        });
}
