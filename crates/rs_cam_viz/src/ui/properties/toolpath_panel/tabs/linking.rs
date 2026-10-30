//! The Linking tab (R2) of the toolpath inspector.

use crate::state::toolpath::{DressupConfig, UiProcessRole};
use crate::ui::properties::linking_dressup::{
    Fields, draw_dressup_params, draw_linking_advanced, draw_linking_params,
};

/// The Linking tab (R2, W3e): "Entry & exit" and "Links"
/// (`draw_linking_params`), "Path quality" (`draw_dressup_params`: arc
/// fitting and dogbone; the Dressup tab before R2), then the ONE Advanced
/// section and "Reset to recommended". Dogbone draws only where it
/// applies: not on a 3D surface op (`OperationConfig::is_3d`), unless a
/// project file turned it on there.
///
/// `placement` is THE placement of the findings of this toolpath; each row
/// reads its field state from it (`Fields`).
pub(in crate::ui::properties::toolpath_panel) fn draw_linking_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    tool: Option<&crate::state::job::ToolConfig>,
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
    placement: &crate::ui::properties::placement::Placement<'_>,
) {
    let fields = Fields::new(&placement.by_field);
    draw_linking_params(ui, entry, tool, toolpath_heights, fields);
    let dogbone_applies = !entry.operation.is_3d() || entry.dressups.dogbone.is_some();
    draw_dressup_params(ui, &mut entry.dressups, dogbone_applies, fields);
    draw_linking_advanced(ui, entry, fields);
    // "Reset to recommended" writes the dressup set of the op role; R2
    // deleted the "n/8 dressups active" count beside it.
    let role = entry.operation.op_type().spec().ui_process_role;
    ui.add_space(crate::ui::tokens::SPACE_2);
    if ui
        .add(crate::ui::components::Button::quiet("Reset to recommended"))
        .on_hover_text(format!(
            "Apply the recommended linking and dressup values for {} operations.",
            match role {
                UiProcessRole::Roughing => "roughing",
                UiProcessRole::SemiFinish => "semi-finish",
                UiProcessRole::Finish => "finishing",
            }
        ))
        .clicked()
    {
        entry.dressups = DressupConfig::for_role(role);
    }
}
