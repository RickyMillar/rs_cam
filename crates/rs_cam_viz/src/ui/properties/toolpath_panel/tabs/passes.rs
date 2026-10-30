//! The Passes tab (R2) of the toolpath inspector.

use super::geometry::regions_phrase;
use super::op_report;
use crate::state::toolpath::{OperationConfig, StockSource};
use crate::ui::AppEvent;
use crate::ui::components::{OneLine, UiExt as _};
use crate::ui::properties::linking_dressup::{Fields, combo_in_row, num_in_row, plain_row};
use crate::ui::properties::pills::PillSuggestions;
use crate::ui::properties::tab_badges::{draw_start_from, rest_region_pathology_line};
use crate::ui::properties::{ToolpathPanelInputs, operations, rest_grid_footprint_area};

/// The Passes tab (R2; the Cut tab before it): the start stock, the op
/// parameters and the rest analysis. The tool is the header Tool row; the
/// input model and the machining boundary are on the Geometry tab.
/// `properties::tests` renders it (D3).
#[expect(
    clippy::too_many_arguments,
    reason = "the tab reads the entry, the inputs and the field states of the placement"
)]
pub(in crate::ui::properties) fn draw_passes_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    strategy_suggestion: Option<crate::state::toolpath::ClearingStrategy>,
    selection_ui: &mut crate::state::pick::SelectionUi,
    fields: Fields<'_>,
    events: &mut Vec<AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let tool_configs = inputs.tool_configs.as_slice();
    let rest_region_consumers = inputs.rest_region_consumers.as_slice();
    // W2: the rest-heatmap overlay is on FOR THIS TOOLPATH. It is the
    // second demand for the rest dials, beside a regions consumer.
    let rest_heatmap_on = inputs.rest_heatmap_on;
    let material = &inputs.material;
    let machine = &inputs.machine;
    let spindle_strategy = inputs.spindle_strategy;
    let toolpath_heights = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let holes = inputs.holes.as_ref();

    ui.add_space(4.0);

    // The stock source opens this tab. R2: the Tool row is in the header
    // and the Input row is on the Geometry tab.
    draw_start_from(ui, entry, inputs, events);
    ui.separator();
    ui.add_space(4.0);

    // IC1: the validator's stale-default finding has ONE owner, the
    // footer diagnostic row with its Fix button. A banner here printed the
    // same finding a second time, with a three-row paragraph.

    // The LUT feeds result the Geometry-row ⚡ pills (stepover /
    // depth-per-pass) read. Feed advice and the single validated Apply
    // all route live on the Feeds tab. F216: the memo runs the
    // calculator and the funnel dry run only when an input moved, not
    // on every frame.
    let pill_tool_cfg = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t);
    if let Some(tool_cfg) = pill_tool_cfg {
        entry.feeds_memo.refresh(
            &mut entry.feeds_result,
            crate::state::runtime::FeedsInputs {
                operation: &entry.operation,
                tool: tool_cfg,
                material,
                machine,
                spindle_strategy,
                model_bbox,
            },
            crate::ui::properties::pills::pill_previews,
        );
    }
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
