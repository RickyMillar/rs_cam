//! Shared per-toolpath row controls used by both the Toolpaths-workspace
//! panel and the Simulation workspace op list.
//!
//! Renders three compact toggle buttons — eye (overall visibility), cut,
//! rapid — plus a bullseye (isolate). All use tight symbolic glyphs so they
//! fit on one row next to the toolpath name.

use crate::state::toolpath::ToolpathId;
use crate::state::viewport::ViewportState;
use crate::ui::AppEvent;
use crate::ui::theme;

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
        events.push(AppEvent::ToggleToolpathVisibility(tp_id));
    }

    // Per-toolpath cut / rapid visibility. Entries default to both-visible.
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
    if ui
        .add(cut_btn)
        .on_hover_text(if entry.show_cutting {
            "Hide green cutting/feed moves for this toolpath."
        } else {
            "Show green cutting/feed moves for this toolpath."
        })
        .clicked()
    {
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
    if ui
        .add(rapid_btn)
        .on_hover_text(if entry.show_rapids {
            "Hide orange rapid-traverse moves for this toolpath."
        } else {
            "Show orange rapid-traverse moves for this toolpath."
        })
        .clicked()
    {
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
            "Clear isolation and show all visible toolpaths."
        } else {
            "Show only this toolpath in the viewport; click again to clear isolation."
        })
        .clicked()
    {
        if is_isolated {
            events.push(AppEvent::ClearIsolation);
        } else {
            // Select this toolpath first so the handler isolates the right one.
            events.push(AppEvent::Select(
                crate::state::selection::Selection::Toolpath(tp_id),
            ));
            events.push(AppEvent::ToggleIsolateToolpath);
        }
    }
}
