use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct UiAutomationSnapshot {
    pub widgets: BTreeMap<String, UiWidgetState>,
}

#[derive(Clone)]
pub struct UiWidgetState {
    pub label: String,
    pub rect: egui::Rect,
    pub enabled: bool,
}

impl Default for UiWidgetState {
    fn default() -> Self {
        Self {
            label: String::new(),
            rect: egui::Rect::NOTHING,
            enabled: false,
        }
    }
}

fn snapshot_id() -> egui::Id {
    egui::Id::new("rs_cam_ui_automation_snapshot")
}

pub fn begin_frame(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(snapshot_id(), UiAutomationSnapshot::default()));
}

pub fn snapshot(ctx: &egui::Context) -> UiAutomationSnapshot {
    ctx.data(|data| data.get_temp(snapshot_id()).unwrap_or_default())
}

pub fn record(ui: &egui::Ui, automation_id: &'static str, response: &egui::Response, label: &str) {
    ui.ctx().data_mut(|data| {
        let mut snapshot: UiAutomationSnapshot = data.get_temp(snapshot_id()).unwrap_or_default();
        snapshot.widgets.insert(
            automation_id.to_owned(),
            UiWidgetState {
                label: label.to_owned(),
                rect: response.rect,
                enabled: response.enabled(),
            },
        );
        data.insert_temp(snapshot_id(), snapshot);
    });
}
