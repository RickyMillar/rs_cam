//! The Feeds tab of the toolpath inspector.

use crate::ui::AppEvent;
use crate::ui::properties::ToolpathPanelInputs;
use crate::ui::properties::feeds_speeds::{
    calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer,
};
use crate::ui::properties::linking_dressup::Fields;

/// The Feeds tab: how fast. The speed rows (R1: a suggested value shows
/// ⚡), the measured operating point, `Explore…` (the
/// nomogram and the full comparison card) and the vendor data of the tool.
pub(in crate::ui::properties::toolpath_panel) fn draw_feeds_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    placement: &crate::ui::properties::placement::Placement<'_>,
    events: &mut Vec<AppEvent>,
) {
    let tool = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t);

    draw_speed_controls(ui, entry, Fields::new(&placement.by_field));

    if let Some(tool_cfg) = tool {
        calculate_and_apply_feeds(
            ui,
            entry,
            tool_cfg,
            &inputs.material,
            &inputs.machine,
            inputs.spindle_strategy,
            inputs.load_verdict(),
            model_bbox,
        );
        ui.add_space(4.0);
        if ui
            .add(crate::ui::components::Button::quiet("Explore…"))
            .on_hover_text(
                "Open the feed-versus-RPM nomogram and the full comparison of the \
                 recommendation for this operation.",
            )
            .clicked()
        {
            events.push(AppEvent::OpenFeedsModal(entry.id));
        }
        // The vendor cutting data of the bound tool's family. With no tool
        // there is no family and no size, so the viewer draws nothing.
        draw_vendor_lut_viewer(ui, tool_cfg.tool_type, tool_cfg.diameter);
    }
}
