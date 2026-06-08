use super::AppEvent;
use crate::state::AppState;
use crate::state::job::SetupId;
use crate::state::selection::Selection;

pub fn draw(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let ctx = ui.ctx().clone();
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
            events.push(AppEvent::OpenExportWizard);
        }
        // Power-user escape hatch: Ctrl+Alt+E skips the wizard and jumps
        // straight to the legacy direct-export pre-flight + file dialog.
        if modifiers.ctrl && modifiers.alt && i.key_pressed(egui::Key::E) {
            events.push(AppEvent::ExportGcode);
        }
        if modifiers.ctrl && !modifiers.shift && i.key_pressed(egui::Key::O) {
            events.push(AppEvent::OpenJob);
        }
    });

    egui::Panel::top("menu_bar").show_inside(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Import STL...").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("STL Files", &["stl", "STL"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportStl(path));
                    }
                }
                if ui.button("Import SVG...").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("SVG Files", &["svg", "SVG"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportSvg(path));
                    }
                }
                if ui.button("Import DXF...").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DXF Files", &["dxf", "DXF"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportDxf(path));
                    }
                }
                if ui.button("Import STEP...").clicked() {
                    ui.close();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("STEP Files", &["step", "stp", "STEP", "STP"])
                        .pick_file()
                    {
                        events.push(AppEvent::ImportStep(path));
                    }
                }
                ui.separator();
                if ui.add(egui::Button::new("Open Job...  Ctrl+O")).clicked() {
                    ui.close();
                    events.push(AppEvent::OpenJob);
                }
                if ui
                    .add(egui::Button::new("Save Job").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    ui.close();
                    events.push(AppEvent::SaveJob);
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Export G-code...").shortcut_text("Ctrl+Shift+E"))
                    .clicked()
                {
                    ui.close();
                    events.push(AppEvent::OpenExportWizard);
                }
                ui.menu_button("Direct export (skip wizard)", |ui| {
                    if ui
                        .add(egui::Button::new("All toolpaths").shortcut_text("Ctrl+Alt+E"))
                        .clicked()
                    {
                        ui.close();
                        events.push(AppEvent::ExportGcode);
                    }
                    if state.session.list_setups().len() > 1 {
                        if ui.button("Combined (M0 pauses)").clicked() {
                            ui.close();
                            events.push(AppEvent::ExportCombinedGcode);
                        }
                        ui.separator();
                        for setup in state.session.list_setups() {
                            let label = format!("Setup: {}", setup.name);
                            if ui.button(&label).clicked() {
                                ui.close();
                                events.push(AppEvent::ExportSetupGcode(SetupId(setup.id)));
                            }
                        }
                    }
                });
                if ui.button("Export Setup Sheet...").clicked() {
                    ui.close();
                    events.push(AppEvent::ExportSetupSheet);
                }
                if ui.button("Export SVG Preview...").clicked() {
                    ui.close();
                    events.push(AppEvent::ExportSvgPreview);
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    events.push(AppEvent::Quit);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui
                    .add(egui::Button::new("Undo").shortcut_text("Ctrl+Z"))
                    .clicked()
                {
                    ui.close();
                    events.push(AppEvent::Undo);
                }
                if ui
                    .add(egui::Button::new("Redo").shortcut_text("Ctrl+Shift+Z"))
                    .clicked()
                {
                    ui.close();
                    events.push(AppEvent::Redo);
                }
                ui.separator();
                // W1.2/P6-001 — was a hardcoded add_enabled(false) advertising
                // a "Del" it never fired. Enable + wire it to the selection.
                let delete_event = match state.selection {
                    Selection::Toolpath(id) => Some(AppEvent::RemoveToolpath(id)),
                    Selection::Tool(id) => Some(AppEvent::RemoveTool(id)),
                    _ => None,
                };
                if ui
                    .add_enabled(
                        delete_event.is_some(),
                        egui::Button::new("Delete Selected").shortcut_text("Del"),
                    )
                    .clicked()
                {
                    ui.close();
                    if let Some(ev) = delete_event {
                        events.push(ev);
                    }
                }
            });

            ui.menu_button("Toolpath", |ui| {
                if ui.button("Generate All").clicked() {
                    ui.close();
                    events.push(AppEvent::GenerateAll);
                }
                let optimize_enabled = state.simulation.has_results() && !state.is_optimizing;
                if ui
                    .add_enabled(optimize_enabled, egui::Button::new("Optimize project…"))
                    .on_disabled_hover_text(
                        "Run a simulation first — the optimizer needs a baseline cut trace.",
                    )
                    .clicked()
                {
                    ui.close();
                    events.push(AppEvent::OpenOptimizeProject);
                }
            });

            ui.menu_button("Tools", |ui| {
                if ui.button("Tool Library…").clicked() {
                    ui.close();
                    events.push(AppEvent::OpenToolLibrary);
                }
            });

            ui.menu_button("Workspace", |ui| {
                if ui.button("Setup").clicked() {
                    ui.close();
                    events.push(AppEvent::SwitchWorkspace(crate::state::Workspace::Setup));
                }
                if ui.button("Toolpaths").clicked() {
                    ui.close();
                    events.push(AppEvent::SwitchWorkspace(
                        crate::state::Workspace::Toolpaths,
                    ));
                }
                if ui.button("Simulation").clicked() {
                    ui.close();
                    events.push(AppEvent::SwitchWorkspace(
                        crate::state::Workspace::Simulation,
                    ));
                }
            });

            ui.menu_button("Simulation", |ui| {
                if ui.button("Run Simulation").clicked() {
                    ui.close();
                    events.push(AppEvent::RunSimulation);
                }
                if ui.button("Reset Simulation").clicked() {
                    ui.close();
                    events.push(AppEvent::ResetSimulation);
                }
                ui.separator();
                if ui.button("Check Holder Clearance").clicked() {
                    ui.close();
                    events.push(AppEvent::RunCollisionCheck);
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Reset View").clicked() {
                    ui.close();
                    events.push(AppEvent::ResetView);
                }
                ui.separator();
                if ui.button("Top").clicked() {
                    ui.close();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Top,
                    ));
                }
                if ui.button("Front").clicked() {
                    ui.close();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Front,
                    ));
                }
                if ui.button("Right").clicked() {
                    ui.close();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Right,
                    ));
                }
                if ui.button("Isometric").clicked() {
                    ui.close();
                    events.push(AppEvent::SetViewPreset(
                        crate::render::camera::ViewPreset::Isometric,
                    ));
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts...").clicked() {
                    ui.close();
                    events.push(AppEvent::ShowShortcuts);
                }
            });
        });
    });
}
