//! Shared per-toolpath row controls used by both the Toolpaths-workspace
//! panel and the Simulation workspace op list.
//!
//! Renders compact toggle buttons — eye (overall visibility), cut, rapid,
//! bullseye (isolate) — and, for the toolpath-queue panel, inline
//! Enable/Disable + Duplicate (SHE-006). All use tight symbolic glyphs so
//! they fit on one row next to the toolpath name.

use crate::state::toolpath::ToolpathId;
use crate::state::viewport::ViewportState;
use crate::ui::AppEvent;
use crate::ui::theme;

/// `queue_state` carries the toolpath's `enabled` flag when rendered in the
/// toolpath-queue panel (`Some` → also render the inline Enable/Disable +
/// Duplicate queue toggles, SHE-006); the Simulation op list passes `None`
/// (those queue actions don't belong in the sim view).
pub fn draw(
    ui: &mut egui::Ui,
    tp_id: ToolpathId,
    overall_visible: bool,
    queue_state: Option<bool>,
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
            "Cutting moves are hidden globally (viewport Show \u{25BE} \u{2192} Paths). \
             Enable there to use this per-toolpath toggle.",
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
            "Rapid moves are hidden globally (viewport Show \u{25BE} \u{2192} Rapids). \
             Enable there to use this per-toolpath toggle.",
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

    // SHE-006 — queue-management toggles, only in the toolpath panel. These
    // give the menu-only Enable/Disable + Duplicate a visible inline cue; the
    // passive dim-name colouring for disabled ops stays as reinforcement, and
    // the context menu remains the full superset.
    if let Some(enabled) = queue_state {
        // Enable/Disable (power glyph). Filled when enabled, dim when off.
        let power_color = if enabled {
            theme::TEXT_HEADING
        } else {
            theme::TEXT_DIM
        };
        let power_btn =
            egui::Button::new(egui::RichText::new("\u{23FB}").small().color(power_color))
                .min_size(egui::vec2(18.0, 16.0));
        if ui
            .add(power_btn)
            .on_hover_text(if enabled {
                "Disable this toolpath (excluded from generation, simulation and output)."
            } else {
                "Enable this toolpath."
            })
            .clicked()
        {
            events.push(AppEvent::ToggleToolpathEnabled(tp_id));
        }

        // Duplicate (two-page glyph).
        let dup_btn = egui::Button::new(
            egui::RichText::new("\u{2398}")
                .small()
                .color(theme::TEXT_HEADING),
        )
        .min_size(egui::vec2(18.0, 16.0));
        if ui
            .add(dup_btn)
            .on_hover_text("Duplicate this toolpath.")
            .clicked()
        {
            events.push(AppEvent::DuplicateToolpath(tp_id));
        }
    }
}
