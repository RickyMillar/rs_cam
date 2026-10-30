//! The Feeds tab of the toolpath inspector.

use crate::ui::AppEvent;
use crate::ui::properties::ToolpathPanelInputs;
use crate::ui::properties::feeds_speeds::{calculate_and_apply_feeds, draw_speed_controls};
use crate::ui::properties::linking_dressup::Fields;

/// The Feeds tab: how fast. The speed rows (R1: a suggested value shows
/// ⚡) and the Chipload row, the inline feed-versus-RPM chart with the
/// recommendation under it (W3i), the measured operating point, and
/// `Vendor data…`, which opens the vendor cutting data window.
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

    draw_speed_controls(
        ui,
        entry,
        tool.map(|t| t.flute_count),
        Fields::new(&placement.by_field),
    );

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
            events,
        );
        ui.add_space(crate::ui::tokens::SPACE_2);
        // The vendor cutting data of the bound tool's family, in its own
        // window (W3i). With no tool there is no family, so no button.
        if ui
            .add(crate::ui::components::Button::quiet("Vendor data\u{2026}"))
            .on_hover_text(
                "Open the vendor cutting data table for this tool's family. The rows at \
                 this tool's diameter are in green.",
            )
            .clicked()
        {
            events.push(AppEvent::OpenVendorData(entry.id));
        }
    }
}
