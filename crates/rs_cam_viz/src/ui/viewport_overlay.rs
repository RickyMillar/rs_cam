use super::AppEvent;
use crate::render::camera::ViewPreset;

pub fn draw(ui: &mut egui::Ui, events: &mut Vec<AppEvent>) {
    // View preset buttons in the top-left of the viewport
    ui.horizontal(|ui| {
        let btn = |ui: &mut egui::Ui, label: &str, preset: ViewPreset, events: &mut Vec<AppEvent>| {
            if ui
                .small_button(label)
                .on_hover_text(format!("{:?} view", preset))
                .clicked()
            {
                events.push(AppEvent::SetViewPreset(preset));
            }
        };
        btn(ui, "Top", ViewPreset::Top, events);
        btn(ui, "Front", ViewPreset::Front, events);
        btn(ui, "Right", ViewPreset::Right, events);
        btn(ui, "Iso", ViewPreset::Isometric, events);
    });
}
