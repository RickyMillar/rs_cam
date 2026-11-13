//! The Passes tab (R2) of the toolpath inspector. P5b (operator ruling
//! 2026-11-03): the Feeds tab merged into it, so each adjustment shows
//! beside its cut. Top to bottom (a diagram comes before its rows):
//!
//! 1. the safe envelope plot or bar (`SAFE CUT`, the ⚡ chip);
//! 2. the feed x rpm nomogram, which follows the candidate ring;
//! 3. the start stock and the op editor (the depth and stepover rows);
//! 4. the feed, rpm and plunge rows, then the Suggest card, then one row with
//!    the closed "Limits" drawer and `Vendor data…` (P29-10 10f);
//! 5. the rest analysis disclosure, when a demand names it.

use super::geometry::regions_phrase;
use super::op_report;
use crate::state::toolpath::{OperationConfig, StockSource};
use crate::ui::AppEvent;
use crate::ui::components::{OneLine, UiExt as _};
use crate::ui::properties::feeds_speeds::{
    FeedsBlock, draw_feeds_chart, draw_feeds_recommendation, draw_speed_controls,
};
use crate::ui::properties::linking_dressup::{Fields, combo_in_row, num_in_row, plain_row};
use crate::ui::properties::pills::PillSuggestions;
use crate::ui::properties::tab_badges::{draw_start_from, rest_region_pathology_line};
use crate::ui::properties::{ToolpathPanelInputs, operations, rest_grid_footprint_area};

/// The Passes tab (R2; the Cut tab before it; P5b: with the feeds): the
/// envelope plot, the nomogram, the start stock, the op parameters, the
/// speed rows, the Suggest card and the rest analysis. The tool is the
/// header Tool row; the input model and the machining boundary are on the
/// Geometry tab. `properties::tests` renders it (D3).
#[expect(
    clippy::too_many_arguments,
    reason = "the inputs and the GUI state slots the tab edits in place (`state.panels`), each threaded in alone, as the panel host threads them"
)]
pub(in crate::ui::properties) fn draw_passes_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    strategy_suggestion: Option<crate::state::toolpath::ClearingStrategy>,
    selection_ui: &mut crate::state::pick::SelectionUi,
    fields: Fields<'_>,
    suggest_slots: &rs_cam_core::feeds::suggest::SuggestSlots,
    candidates: &mut crate::state::panels::EnvelopeCandidates,
    // P29-10 10f: the open state of the card's "Limits" drawer
    // (`state.panels.card_limits_open`), threaded in as `candidates` is.
    limits_open: &mut bool,
    events: &mut Vec<AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let tool_configs = inputs.tool_configs.as_slice();
    let rest_region_consumers = inputs.rest_region_consumers.as_slice();
    // W2: the rest-heatmap overlay is on FOR THIS TOOLPATH. It is the
    // second demand for the rest dials, beside a regions consumer.
    let rest_heatmap_on = inputs.rest_heatmap_on;
    let toolpath_heights = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let holes = inputs.holes.as_ref();

    ui.add_space(4.0);

    // P5b: the feeds inputs of the bound tool. With no tool there is no
    // envelope, no chart and no card.
    let tool = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t);
    let block = tool.map(|tool| FeedsBlock {
        tool,
        material: &inputs.material,
        machine: &inputs.machine,
        spindle_strategy: inputs.spindle_strategy,
        spindle_rpm_ceiling: inputs.spindle_rpm_ceiling,
        load_verdict: inputs.load_verdict(),
        suggest_slots,
    });

    if let Some(block) = &block {
        // 1. K2b: the safe envelope (the memo built it, F216).
        let envelope = entry.feeds_memo.envelope_shared();
        let refusal = entry.feeds_memo.refusal().map(ToString::to_string);
        let _ = crate::ui::feeds::envelope::draw_envelope(
            ui,
            entry.id,
            &entry.operation,
            envelope.as_deref(),
            refusal.as_deref(),
            candidates,
            events,
        );
        // 2. The nomogram: it draws the speeds of the candidate while one
        //    exists (live link), a lookup in the envelope's speeds grid.
        let candidate = envelope.as_deref().and_then(|env| {
            crate::ui::feeds::envelope::candidate_speeds(env, candidates.get(entry.id))
        });
        draw_feeds_chart(ui, entry, block, candidate);
        ui.add_space(4.0);
    }

    // The stock source opens the op rows. R2: the Tool row is in the
    // header and the Input row is on the Geometry tab.
    draw_start_from(ui, entry, inputs, events);
    ui.separator();
    ui.add_space(4.0);

    // IC1: the validator's stale-default finding has ONE owner, the
    // footer diagnostic row with its Fix button. A banner here printed the
    // same finding a second time, with a three-row paragraph.

    // The LUT feeds result the ⚡ pills (stepover / depth-per-pass) read.
    // F216, W3i: the panel host fills the memo once per frame before any
    // tab draws (`feeds_speeds::refresh_feeds_memo`); this tab reads it.
    let pill_tool_cfg = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t);
    // G-PILLCLAMP (UX-R03-014): the dry run of the apply funnel, so
    // every ⚡ pill below offers and writes the value the canonical
    // Apply all route would write for its field — not the raw
    // calculator number (4.2 mm vs 1.2 mm of DOC on the demo pocket).
    let pills = match (
        entry.feeds_result.as_ref(),
        entry.feeds_memo.previews(),
        pill_tool_cfg,
    ) {
        (Some(result), Some(previews), Some(_)) => {
            Some(PillSuggestions::from_previews(result, previews))
        }
        _ => None,
    };

    // PR-2D Phase 2 — pass the per-field pill suggestions to every
    // per-op draw so each numeric field can render an inline ⚡
    // Suggest pill. Built above from the memo of `entry.feeds_result`
    // and the funnel dry run, so this is just a borrow.
    let feeds_for_pills = pills.as_ref();
    // A/M6: read before the mutable borrow of `entry.operation`
    // below. Both are `Copy`, so nothing is held across it.
    let resolved_claims_reference = entry.run.result().and_then(|r| r.stats.claims_reference);
    // G-DEPTHSTOCK (UX-R03-007): the same rule the header ribbon
    // prints, read once here and handed to the depth field's row.
    // `Copy`, so nothing is held across the mutable borrow below.
    let depth_caution = toolpath_heights.and_then(|answer| {
        operations::depth_beyond_stock(&entry.operation, &entry.heights, answer)
    });
    let depth_caution = depth_caution.as_ref();
    // G-THROUGHCUT (UX-R03-006): Profile only. Read here, before the
    // mutable borrow, for the same reason as the caution above.
    let through_cut = match (&entry.operation, toolpath_heights) {
        (OperationConfig::Profile(cfg), Some(answer)) => {
            operations::profile_through_cut(cfg, &entry.heights, answer)
        }
        _ => None,
    };
    let through_cut = through_cut.as_ref();
    // UI-05: one registry row per operation carries the editor, the diagram
    // and the validation arm. The editor match and the diagram match used to
    // sit here, and the diagram one ended `_ => {}` — a new operation drew
    // nothing and nothing said so. `operations::registry::OP_UI_ROWS` is the
    // table, and `operations_registry` holds it against
    // `OperationType::ALL`.
    let mut draw_ctx = operations::registry::OpDrawCtx {
        tools,
        models,
        holes,
        pin_holes: &inputs.pin_holes,
        pills: feeds_for_pills,
        depth_caution,
        through_cut,
        resolved_claims_reference,
        stock_source: entry.stock_source,
        toolpath_id: entry.id,
        selection_ui: &mut *selection_ui,
        model_areas: op_report(inputs)
            .is_some_and(rs_cam_core::session::SelectionReport::holds_a_model_area),
        strategy_suggestion,
        boundary_enabled: entry.boundary.enabled,
        material: &inputs.material,
        fields,
    };
    operations::registry::draw_editor_and_diagram(ui, &mut entry.operation, &mut draw_ctx);
    // G-PILLCLAMP: a pill wrote its field this frame — stamp the
    // recommendation's provenance (the value is the funnel's, not a hand
    // edit) and remember it for the flush.
    if let Some((field, preview)) = pills.as_ref().and_then(|p| p.take_clicked()) {
        entry
            .feeds_provenance
            .set(field, preview.provenance.clone());
        entry.pill_stamped_fields.push(field);
        // P5b (K2b, Checkpoint I-1): a geometry pill is no per-field apply.
        // It applies the envelope proposal through the one funnel.
        events.push(AppEvent::ApplyFeedsPick {
            toolpath_id: entry.id,
            pick: rs_cam_core::feeds::envelope::PickRequest::Proposal,
        });
    }

    // 4. The speed rows (R1: a suggested value shows ⚡) and the Chipload
    //    row, then the recommendation card and the measured operating
    //    point, then `Vendor data…` (the Feeds tab before P5b).
    draw_speed_controls(
        ui,
        entry,
        tool.map(|t| t.flute_count_planning()),
        &inputs.machine,
        fields,
    );
    if let Some(block) = &block {
        // The card, then ONE row with the closed "Limits" drawer and
        // `Vendor data…` (P29-10 10f), then the measured operating point.
        draw_feeds_recommendation(ui, entry, block, limits_open, events);
    }

    // ── Rest analysis: a demand-titled disclosure ───────────────
    // W2 (G-STARTFROM). There is no "Compute rest heatmap" checkbox and
    // no draw-time flip (D5). The analysis costs a rest-field solve at
    // generation, so it runs only on DEMAND, and the demand names itself
    // in the title:
    //   - another operation takes this one's rest regions as its
    //     boundary (`rest_region_consumers`), or
    //   - the operator switched the rest-heatmap overlay on for this
    //     operation.
    // Both doors WRITE `rest_analysis.enabled` through
    // `Command::AutoEnableRestAnalysis`: the boundary picker and the
    // overlay action in `ui::properties::apply_auto_enable`, the MCP
    // `set_boundary_config` setter, and the loader's post-pass. A draw
    // reads the flag; it never sets it.
    let is_rest_depth_pencil = matches!(
        &entry.operation,
        OperationConfig::Pencil(cfg)
            if cfg.detector == rs_cam_core::finish::pencil::PencilDetector::RestDepth
    );
    if !rest_region_consumers.is_empty() || rest_heatmap_on {
        let title = if rest_region_consumers.is_empty() {
            "Rest heatmap overlay".to_owned()
        } else {
            // G-C (§6 c): the count of this op's own regions, in the
            // phrase of the consumer's source combo.
            let count = entry
                .run
                .result()
                .and_then(|result| result.annotated.produced.rest_regions.as_ref())
                .map_or(0, |regions| regions.len());
            let consumers = rest_region_consumers.join(", ");
            if count > 0 {
                format!(
                    "Rest regions \u{2192} {consumers} \u{00B7} {}",
                    regions_phrase(count)
                )
            } else {
                format!("Rest regions \u{2192} {consumers}")
            }
        };
        ui.add_space(8.0);
        ui.disclosure("rest_regions", &title, false, |ui| {
            if is_rest_depth_pencil {
                // The rest-depth detector attaches the same grid and the
                // same regions from the pencil dials one grid above, so
                // this operation has no second set to show.
                ui.add(
                    OneLine::new("Grid and regions: from the Rest depth detector").detail(
                        "The pencil's Rest depth detector produces the grid and the regions.",
                    ),
                );
                return;
            }
            if entry.stock_source == StockSource::Fresh {
                // `RestReferenceChoice::resolve` prefers the machined stock
                // whenever it overlaps, so under "Rest machining: from previous ops"
                // this combo is inert and naming it would be a lie.
                let ref_label = entry
                    .rest_analysis
                    .reference_tool_id
                    .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
                    .map(|(_, name, _)| name.as_str())
                    .unwrap_or("Self / machined stock");
                combo_in_row(
                    ui,
                    plain_row(
                        "Reference",
                        Some(
                            "The reference the gate measures 'deeper than'. Unset = prefer \
                             the machined stock from a prior simulation, else a \
                             self-referenced bare-surface probe.",
                        ),
                    ),
                    "rest_analysis_reference_tool",
                    ref_label,
                    |ui| {
                        if ui
                            .selectable_label(
                                entry.rest_analysis.reference_tool_id.is_none(),
                                "Self / machined stock",
                            )
                            .clicked()
                        {
                            entry.rest_analysis.reference_tool_id = None;
                        }
                        for (id, name, _) in tools {
                            let selected = entry.rest_analysis.reference_tool_id == Some(*id);
                            if ui.selectable_label(selected, name.as_str()).clicked() {
                                entry.rest_analysis.reference_tool_id = Some(*id);
                            }
                        }
                    },
                );
            }
            num_in_row(
                ui,
                plain_row(
                    "Cell size",
                    Some("XY grid cell size for the rest field. Smaller = finer regions."),
                ),
                &mut entry.rest_analysis.cell_mm,
                " mm",
                0.05,
                0.05..=10.0,
            );
            num_in_row(
                ui,
                plain_row(
                    "Min valley depth",
                    Some(
                        "A cell counts as REST material once the reference floats more than \
                         this above the true surface.",
                    ),
                ),
                &mut entry.rest_analysis.min_valley_depth,
                " mm",
                0.01,
                0.0..=5.0,
            );
            num_in_row(
                ui,
                plain_row(
                    "Region margin",
                    Some(
                        "Extra clearance added around detected regions beyond this \
                         toolpath's own tool radius.",
                    ),
                ),
                &mut entry.rest_analysis.region_margin_mm,
                " mm",
                0.05,
                0.0..=10.0,
            );
        })
        .header_response
        .on_hover_text(
            "The rest field this operation leaves, and the regions the operations \
             above take as their boundary.",
        );
    }

    // Sliver-storm / giant-region caption (2026-07-06 incident):
    // classify THIS toolpath's own generated rest regions, whether the
    // generic rest analysis produced them or (for a rest_depth pencil)
    // the detector it always runs. Deliberately outside the disclosure
    // above, so both cases show it and a closed disclosure cannot hide
    // a caution.
    //
    // LH-2: the giant-region denominator is THIS result's own rest
    // grid — covered cells × cell², the same grid the regions were
    // extracted from. It used to be the model's XY bounding
    // rectangle, which overstates a non-rectangular part's footprint
    // and made the warning fire late (`MEASUREMENT_DOMAINS.md` X-4).
    // No grid ⇒ 0.0 ⇒ silence, never a guess.
    if let Some(result) = entry.run.result()
        && let Some(regions) = result.annotated.produced.rest_regions.as_ref()
        && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
            regions,
            rest_grid_footprint_area(result.annotated.produced.rest_grid.as_deref()),
        )
    {
        ui.add(rest_region_pathology_line(pathology));
    }
}
