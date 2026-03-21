use super::AppEvent;
use crate::state::AppState;

pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    // Keyboard shortcuts
    let modifiers = ctx.input(|i| i.modifiers);
    ctx.input(|i| {
        if modifiers.ctrl && i.key_pressed(egui::Key::Z) {
            if modifiers.shift {
                events.push(AppEvent::Redo);
            } else {
                events.push(AppEvent::Undo);
            }
        }
        if modifiers.ctrl && i.key_pressed(egui::Key::S) {
            events.push(AppEvent::SaveJob);
        }
        if modifiers.ctrl && modifiers.shift && i.key_pressed(egui::Key::E) {
            events.push(AppEvent::ExportGcode);
        }
    });

    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Import STL...").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("STL Files", &["stl", "STL"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportStl(path));
                    }
                }
                if ui.button("Import SVG...").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("SVG Files", &["svg", "SVG"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportSvg(path));
                    }
                }
                if ui.button("Import DXF...").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DXF Files", &["dxf", "DXF"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportDxf(path));
                    }
                }
                ui.separator();
                if ui.button("Open Job...").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::OpenJob);
                }
                if ui.add(egui::Button::new("Save Job  Ctrl+S")).clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SaveJob);
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Export G-code (all)...  Ctrl+Shift+E"))
                    .clicked()
                {
                    ui.close_menu();
                    events.push(AppEvent::ExportGcode);
                }
                if state.job.setups.len() > 1 {
                    if ui.button("Export Combined (M0 pauses)...").clicked() {
                        ui.close_menu();
                        events.push(AppEvent::ExportCombinedGcode);
                    }
                    ui.separator();
                    for setup in &state.job.setups {
                        let label = format!("Export '{}'...", setup.name);
                        if ui.button(&label).clicked() {
                            ui.close_menu();
                            events.push(AppEvent::ExportSetupGcode(setup.id));
                        }
                    }
                }
                if ui.button("Export Setup Sheet...").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::ExportSetupSheet);
                }
                if ui.button("Export SVG Preview...").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::ExportSvgPreview);
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    events.push(AppEvent::Quit);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui.add(egui::Button::new("Undo  Ctrl+Z")).clicked() {
                    ui.close_menu();
                    events.push(AppEvent::Undo);
                }
                if ui.add(egui::Button::new("Redo  Ctrl+Shift+Z")).clicked() {
                    ui.close_menu();
                    events.push(AppEvent::Redo);
                }
            });

            ui.menu_button("Toolpath", |ui| {
                if ui.button("Generate All").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::GenerateAll);
                }
            });

            ui.menu_button("Workspace", |ui| {
                if ui.button("Setup").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SwitchWorkspace(crate::state::Workspace::Setup));
                }
                if ui.button("Toolpaths").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SwitchWorkspace(
                        crate::state::Workspace::Toolpaths,
                    ));
                }
                if ui.button("Simulation").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SwitchWorkspace(
                        crate::state::Workspace::Simulation,
                    ));
                }
            });

            ui.menu_button("Simulation", |ui| {
                if ui.button("Run Simulation").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::RunSimulation);
                }
                if ui.button("Reset Simulation").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::ResetSimulation);
                }
                ui.separator();
                if ui.button("Check Holder Clearance").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::RunCollisionCheck);
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Reset View").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::ResetView);
                }
                ui.separator();
                if ui.button("Top").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Top,
                    ));
                }
                if ui.button("Front").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Front,
                    ));
                }
                if ui.button("Right").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Right,
                    ));
                }
                if ui.button("Isometric").clicked() {
                    ui.close_menu();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Isometric,
                    ));
                }
            });
        });
    });
}
