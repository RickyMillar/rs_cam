//! Shared per-toolpath row controls, used by the Simulation workspace op
//! list and by the Simulation section of the inspector.
//!
//! Renders compact toggle buttons — eye (overall visibility), cut, rapid,
//! bullseye (isolate). All use tight symbolic glyphs so they fit on one row
//! next to the toolpath name.
//!
//! DC1 removed the third caller and the parameter that served it. The
//! operations panel no longer draws a glyph row: its card is one row with
//! one always-visible eye, and every other action sits in the card's `…`
//! menu. The `queue_state: Option<bool>` argument that added an inline
//! Enable/Disable and a Duplicate (SHE-006) went with it, because no caller
//! passed `Some` any more.

use crate::state::toolpath::ToolpathId;
use crate::state::viewport::ViewportState;
use crate::ui::AppEvent;
use crate::ui::theme;
use crate::ui_command::{NoArgs, UiCommand};

/// Draw the four per-toolpath viewport toggles for one row.
pub fn draw(
    ui: &mut egui::Ui,
    tp_id: ToolpathId,
    overall_visible: bool,
    viewport: &mut ViewportState,
    events: &mut Vec<AppEvent>,
) {
    // Overall visibility (eye). Off if the runtime's `visible` flag is off.
    let eye = if overall_visible {
        "\u{1F441}"
    } else {
        "\u{2298}"
    };
    if ui
        .small_button(eye)
        .on_hover_text(if overall_visible {
            "Hide this entire toolpath in the 3D viewport. Simulation still includes it."
        } else {
            "Show this toolpath again in the 3D viewport."
        })
        .clicked()
    {
        events.push(AppEvent::Ui(UiCommand::ToggleToolpathVisibility(tp_id)));
    }

    // Per-toolpath cut / rapid visibility. The global viewport toggles gate
    // these, so a per-row C / R does nothing while its global toggle is off —
    // grey it out so the layering explains itself (P4-004). Capture the global
    // flags before the mutable entry borrow.
    let global_cutting = viewport.show_cutting;
    let global_rapids = viewport.show_rapids;
    let entry = viewport.toolpath_move_visibility.entry(tp_id).or_default();

    let cut_text = "C";
    let cut_btn = egui::Button::new(egui::RichText::new(cut_text).small().color(
        if entry.show_cutting {
            theme::TEXT_HEADING
        } else {
            theme::TEXT_DIM
        },
    ))
    .min_size(egui::vec2(18.0, 16.0));
    let cut_resp = ui.add_enabled(global_cutting, cut_btn);
    let cut_resp = if global_cutting {
        cut_resp.on_hover_text(if entry.show_cutting {
            "Hide green cutting/feed moves for this toolpath."
        } else {
            "Show green cutting/feed moves for this toolpath."
        })
    } else {
        cut_resp.on_disabled_hover_text(
            "Cutting moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} \
             Cutting moves). Enable there to use this per-toolpath toggle.",
        )
    };
    if cut_resp.clicked() {
        entry.show_cutting = !entry.show_cutting;
    }

    let rapid_text = "R";
    let rapid_btn = egui::Button::new(egui::RichText::new(rapid_text).small().color(
        if entry.show_rapids {
            theme::TEXT_HEADING
        } else {
            theme::TEXT_DIM
        },
    ))
    .min_size(egui::vec2(18.0, 16.0));
    let rapid_resp = ui.add_enabled(global_rapids, rapid_btn);
    let rapid_resp = if global_rapids {
        rapid_resp.on_hover_text(if entry.show_rapids {
            "Hide orange rapid-traverse moves for this toolpath."
        } else {
            "Show orange rapid-traverse moves for this toolpath."
        })
    } else {
        rapid_resp.on_disabled_hover_text(
            "Rapid moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} \
             Rapids). Enable there to use this per-toolpath toggle.",
        )
    };
    if rapid_resp.clicked() {
        entry.show_rapids = !entry.show_rapids;
    }

    // Isolate (only-show-this) toggle. Target \u{25CE} = bullseye.
    let is_isolated = viewport.isolate_toolpath == Some(tp_id);
    let iso_btn = egui::Button::new(egui::RichText::new("\u{25CE}").small().color(
        if is_isolated {
            theme::WARNING
        } else {
            theme::TEXT_HEADING
        },
    ))
    .min_size(egui::vec2(18.0, 16.0));
    if ui
        .add(iso_btn)
        .on_hover_text(if is_isolated {
            "Unpin this toolpath and go back to the viewport's own rule."
        } else {
            "Pin this toolpath so it stays drawn as the selection moves. Click again to unpin."
        })
        .clicked()
    {
        if is_isolated {
            events.push(AppEvent::Ui(UiCommand::ClearIsolation(NoArgs)));
        } else {
            // Select this toolpath first so the handler isolates the right one.
            events.push(AppEvent::Ui(UiCommand::Select(
                crate::state::selection::Selection::Toolpath(tp_id),
            )));
            events.push(AppEvent::Ui(UiCommand::ToggleIsolateToolpath(NoArgs)));
        }
    }
}
