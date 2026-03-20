use super::AppEvent;
use crate::state::AppState;

pub fn draw(ctx: &egui::Context, _state: &AppState, events: &mut Vec<AppEvent>) {
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
                ui.separator();
                if ui.button("Quit").clicked() {
                    events.push(AppEvent::Quit);
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
