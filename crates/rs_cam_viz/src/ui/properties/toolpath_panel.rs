//! `draw_toolpath_panel` — the tab host itself.
//!
//! `draw_toolpath_panel` draws the shared header (the name, the Tool row,
//! the status line and the issue stack), the tab strip and whichever tab is
//! active. W2b: the panel has no footer; `placement::place` gives each
//! finding its one place.
//!
//! W7: the status line (`header.rs`), the issue stack (`issues.rs`) and each
//! tab body (`tabs/`) are children of this module in `toolpath_panel/`.
//! This file keeps the tab host and `strategy_suggestion`.

mod header;
pub(super) mod issues;
pub(super) mod tabs;

pub(super) use header::{HeaderRow, draw_generate_row};
pub(super) use issues::panel_diagnostics;
use issues::{draw_issue_stack, gate_refusals};
use tabs::feeds::draw_feeds_tab;
pub(super) use tabs::geometry::{ReachView, draw_geometry_tab};
use tabs::heights::draw_heights_tab;
use tabs::linking::draw_linking_tab;
pub(super) use tabs::passes::draw_passes_tab;

use super::operations::validate_toolpath;
use super::tab_badges::{compute_tab_badges, draw_tool_row, draw_toolpath_tabs};
use super::{ToolpathPanelInputs, ToolpathPanelSnapshot, ToolpathTab, operations};
use crate::ui::AppEvent;
use crate::ui::components::{Notice, PropRow};

#[expect(
    clippy::too_many_arguments,
    reason = "the snapshot, the inputs and the GUI state slots the panel edits in place (`state.panels`, `state.gui`), each threaded in alone"
)]
pub(super) fn draw_toolpath_panel(
    ui: &mut egui::Ui,
    // The panel's whole read side: the owned entry, both static
    // diagnostic contexts and the model bbox, built once by
    // `toolpath_panel_snapshot`. They travel together so a caller cannot
    // render the entry while substituting a default for one of the rest.
    snapshot: &mut ToolpathPanelSnapshot,
    // UI-01: every read-only value the panel draws from, built once by
    // `toolpath_panel_inputs`. These were 21 separate parameters.
    inputs: &ToolpathPanelInputs,
    // R3: the reach overlay as the Reach row reads it, and the row's
    // Show/Hide request. The caller switches the overlay through the
    // overlay registry (`set_overlay`); this panel takes no state
    // reference.
    reach_view: &mut ReachView,
    // P4.3: the pick mode and the `+ Rule` draft of the Selection
    // sections, threaded in as `reach_view` is.
    selection_ui: &mut crate::state::pick::SelectionUi,
    // F033 / UI-09: the last refused heights edit (`state.panels`), threaded
    // in as `selection_ui` is.
    heights_refusal: &mut crate::state::panels::HeightsRefusal,
    // G-C (D-2): the `⟲` badge of an auto-set containment (`state.panels`),
    // threaded in as `heights_refusal` is.
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
    events: &mut Vec<AppEvent>,
) -> ToolpathTab {
    // Disjoint borrows of the snapshot's fields. The body edits the entry
    // and only reads the other three, so the four live side by side.
    let entry = &mut snapshot.entry;
    let preconditions = &snapshot.preconditions;
    let model_refs = &snapshot.model_refs;
    let suggest_warnings = snapshot.suggest_warnings.as_deref();
    // Q1: what both Suggest sites below put in
    // `SuggestContext::model_bbox`. The pill funnel and the Feeds card
    // passed `SuggestContext::default()` before, so the runtime-sanity
    // stepover back-off could not fire in the GUI.
    let model_bbox = snapshot.model_bbox.as_ref();

    // UI-01: what the HOST itself draws from. Every other field of
    // `inputs` belongs to one tab and is re-bound in that tab's function.
    let tools = inputs.tools.as_slice();
    let tool_configs = inputs.tool_configs.as_slice();
    let validation = &inputs.validation;
    // P3.1: THE heights of the session; `None` when they did not resolve.
    let toolpath_heights = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let tab_override = inputs.tab_override;
    let freshness = &inputs.freshness;

    // The inspector is a fixed-width side panel. Keep children — especially
    // long Feeds annotations — from enlarging its requested width.
    ui.set_max_width(ui.available_width());

    // ── Shared header (always visible above tabs) ───────────────────

    // Name — single editable home for the toolpath name; the duplicate
    // read-only heading was dropped (density pass 2026-06-11).
    // IC1: the operation's description is the hover of the name row. It used
    // to be an italic line at the top of the Geometry tab.
    // W3g: a `PropRow` like the Tool row under it, so the header reads Name,
    // Tool and the status line on one label column.
    let spec = entry.operation.op_type().spec();
    let name_hover = format!("{}: {}", spec.label, spec.description);
    let _ = PropRow::new("Name")
        .hover(Some(&name_hover))
        .show_widget(ui, egui::TextEdit::singleline(&mut entry.name));
    // W2b: THE finding list of this toolpath and THE placement of each
    // finding. `placement::place` is the only reader of the list here.
    let tool_for_diags = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc);
    // F216, W3i: fill the feeds memo before the placement and before any
    // tab draws, so the feed findings are placed on the first frame of a
    // selection whatever tab is active.
    super::feeds_speeds::refresh_feeds_memo(entry, inputs, model_bbox);
    let diagnostics = panel_diagnostics(entry, preconditions, model_refs, suggest_warnings, inputs);
    let placement = super::placement::place(&diagnostics, &entry.operation);

    // R2 / P1: the Tool row is the one tool control of the inspector. W2b:
    // a tool finding is its state and its hover.
    draw_tool_row(ui, entry, inputs, &placement.tool, events);

    // W2b: the status line. One word and one button from core
    // `toolpath_run_state` (the rule MCP `list_toolpaths` reads), the move
    // count as faint meta, the notes counter.
    ui.add_space(4.0);
    let validation_errors = validate_toolpath(entry, validation);
    // F20: the first blocking finding of the stored result.
    let blocking = crate::ui::toolpath_panel::blocking_finding(
        entry.run.result().map(|result| &result.stats),
        entry.id,
    );
    let notes: Vec<Notice> = placement.note_notices();
    let blocking_hover = blocking.as_ref().map(super::placement::blocking_hover);
    let header = HeaderRow {
        run_state: inputs.run_state.as_ref(),
        can_run: !tools.is_empty() && validation_errors.is_empty(),
        manual_generation: !entry.auto_regen,
        waiting_detail: match freshness {
            crate::state::freshness::FreshnessState::WaitingOnUpstream(block) => {
                Some(block.message.as_str())
            }
            _ => None,
        },
        blocking: blocking_hover.as_deref(),
        moves: entry.run.result().map(|result| result.stats.move_count),
        feed_stage: inputs.feed_stage,
        caps: inputs.caps,
    };
    if draw_generate_row(ui, &header, &notes) {
        events.push(AppEvent::GenerateToolpath(entry.id));
    }

    // G-C (§2 d): the D53 refusal, owned by the Containment row of the
    // Geometry tab (R2: the Machining Boundary section moved there); the
    // Geometry tab shows a dot for it.
    let cut_refusal = operations::rest_consumer_refusal(validation, entry).map(str::to_owned);

    // W2b: the issue stack under the header. The last run's refusal (ONE
    // row), the findings the placement gives it, and the validator
    // refusals that no core finding states.
    draw_issue_stack(
        ui,
        entry,
        inputs,
        &placement,
        cut_refusal.as_deref(),
        &gate_refusals(
            &validation_errors,
            entry,
            inputs,
            tool_for_diags,
            cut_refusal.as_deref(),
        ),
        events,
    );

    // ── Tab bar ─────────────────────────────────────────────────────

    ui.add_space(8.0);
    let tab_id = ui.id().with("tp_tab").with(entry.id.raw());
    let mut active_tab: ToolpathTab = tab_override.unwrap_or_else(|| {
        ui.memory(|mem| mem.data.get_temp(tab_id))
            .unwrap_or(ToolpathTab::Geometry)
    });
    // R2: a tab that the op does not draw falls back to Geometry.
    if !active_tab.applies(&entry.operation) {
        active_tab = ToolpathTab::Geometry;
    }
    let mut tab_badges = compute_tab_badges(entry, &placement, toolpath_heights);
    // G-C (review §2 d): the D53 refusal puts a dot on the Geometry tab,
    // which holds its fix (the Containment row).
    if cut_refusal.is_some() {
        tab_badges.raise(ToolpathTab::Geometry, crate::ui::tokens::DANGER);
    }
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges, &entry.operation);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        // GT-1: what the op cuts. R2: the Input row, the selection, the
        // floor, the machining boundary and the Reach row.
        ToolpathTab::Geometry => {
            draw_geometry_tab(
                ui,
                entry,
                inputs,
                selection_ui,
                containment_autocorrect,
                reach_view,
                placement
                    .by_field
                    .get(rs_cam_core::diagnostics::presentation::SELECTION_KEY)
                    .map_or(&[][..], Vec::as_slice),
                events,
            );
        }
        ToolpathTab::Passes => {
            draw_passes_tab(
                ui,
                entry,
                inputs,
                strategy_suggestion(suggest_warnings),
                selection_ui,
                super::linking_dressup::Fields::new(&placement.by_field),
                events,
            );
        }

        ToolpathTab::FeedsSpeeds => {
            draw_feeds_tab(ui, entry, inputs, model_bbox, &placement, events);
        }

        // How moves connect: entry/exit, move optimization, retract
        // strategy, then arc fitting and dogbone (R2: the Dressup tab is
        // deleted; its rows are here).
        ToolpathTab::Linking => {
            draw_linking_tab(ui, entry, tool_for_diags, toolpath_heights, &placement);
        }

        ToolpathTab::Heights => {
            draw_heights_tab(ui, entry, inputs, heights_refusal, &placement, events);
        }
    }

    // Live look round 3: the caller records the tab on screen
    // (`GuiState::active_toolpath_tab`), which the height planes read.
    active_tab
}

/// G-D (§3): the clearing strategy that the Suggest run recommends and did
/// not apply, from its records. The token is the serde spelling of
/// `ClearingStrategy`, so serde reads it; there is no second token table.
pub(super) fn strategy_suggestion(
    warnings: Option<&[rs_cam_core::feeds::suggest::SuggestWarning]>,
) -> Option<crate::state::toolpath::ClearingStrategy> {
    warnings?.iter().find_map(|warning| match warning {
        rs_cam_core::feeds::suggest::SuggestWarning::StrategyRecommendedNotApplied {
            param: "clearing_strategy",
            recommended,
            ..
        } => serde_json::from_value(serde_json::Value::String(recommended.clone())).ok(),
        _ => None,
    })
}
