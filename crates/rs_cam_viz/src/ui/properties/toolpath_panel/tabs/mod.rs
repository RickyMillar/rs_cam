//! The tab bodies of the toolpath inspector, one file per tab.

// ── The tab bodies ──────────────────────────────────────────────────────
//
// UI-01: `draw_toolpath_panel` ran 1 255 lines because every tab drew
// inline. Each tab is now its own function, so the `match` above is the
// dispatch it always claimed to be. The moves are pure: no widget, no draw
// order and no egui id changed. Each function re-binds the `inputs` fields
// it needs under the names its body already used.

pub(super) mod feeds;
pub(in crate::ui::properties) mod geometry;
pub(super) mod heights;
pub(super) mod linking;
pub(super) mod passes;

/// The `selection_status` report of the op selection, when it answered.
fn op_report(
    inputs: &crate::ui::properties::ToolpathPanelInputs,
) -> Option<&rs_cam_core::session::SelectionReport> {
    match inputs.selection_report.as_ref() {
        Some(Ok(report)) => Some(report),
        Some(Err(_)) | None => None,
    }
}
