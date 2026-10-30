//! The Feeds tab of the toolpath inspector.

use crate::ui::AppEvent;
use crate::ui::properties::ToolpathPanelInputs;
use crate::ui::properties::feeds_speeds::{
    calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer,
};

/// The Feeds tab: how fast.
pub(in crate::ui::properties::toolpath_panel) fn draw_feeds_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    events: &mut Vec<AppEvent>,
) {
    let tool_configs = inputs.tool_configs.as_slice();
    let material = &inputs.material;
    let machine = &inputs.machine;
    let spindle_strategy = inputs.spindle_strategy;
    let project_default_rpm = inputs.project_default_rpm;
    let load_verdict = inputs.load_verdict();

    let tool_info = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| (t.diameter, t.tool_type));
    let (tool_diameter, tool_type) =
        tool_info.unwrap_or((6.0, crate::state::job::ToolType::EndMill));
    ui.horizontal(|ui| {
        if ui
            .button("Explore…")
            .on_hover_text("Open the feed-versus-RPM nomogram for this operation.")
            .clicked()
        {
            events.push(AppEvent::OpenFeedsModal(entry.id));
        }
    });
    ui.add_space(4.0);
    draw_speed_controls(ui, entry);
    ui.add_space(4.0);
    if let Some(tool_cfg) = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t)
    {
        calculate_and_apply_feeds(
            ui,
            entry,
            tool_cfg,
            material,
            machine,
            spindle_strategy,
            project_default_rpm,
            load_verdict,
            model_bbox,
            events,
        );
    }
    // Vendor cutting data viewer (always available, filtered by tool)
    draw_vendor_lut_viewer(ui, tool_type, tool_diameter);
}
