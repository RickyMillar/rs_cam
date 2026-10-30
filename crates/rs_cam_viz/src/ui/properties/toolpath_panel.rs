//! `draw_toolpath_panel` — the tab host itself.
//!
//! One function. It draws the shared header (the name, the Tool row, the
//! status line and the issue stack), the tab strip and whichever tab is
//! active, and it delegates every tab body to a sibling child of
//! `ui::properties`. W2b: the panel has no footer; `placement::place` gives
//! each finding its one place.

use super::feeds_speeds::{calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer};
use super::linking_dressup::{draw_dressup_params, draw_linking_params};
use super::operations::{draw_height_diagram, draw_heights_params, validate_toolpath};
use super::pills::PillSuggestions;
use super::tab_badges::{
    compute_tab_badges, draw_input_row, draw_start_from, draw_tool_row, draw_toolpath_tabs,
    rest_region_pathology_line,
};
use super::{
    ReachPanelSummary, ToolpathPanelInputs, ToolpathPanelSnapshot, ToolpathTab,
    boundary_summary_line, operations, rest_grid_footprint_area,
};
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, DressupConfig, OperationConfig, StockSource, UiProcessRole,
};
use crate::ui::AppEvent;
use crate::ui::components::{
    NotesCounter, Notice, NoticeStack, OneLine, PropRow, Role, UiExt as _,
};

/// W2b: the state words of the status line, one per core `RunState`.
pub(crate) const WORD_DISABLED: &str = "Disabled";
pub(crate) const WORD_NOT_READY: &str = "Not ready";
pub(crate) const WORD_READY: &str = "Ready";
pub(crate) const WORD_GENERATING: &str = "Generating";
pub(crate) const WORD_DONE: &str = "Done";
pub(crate) const WORD_OUT_OF_DATE: &str = "Out of date";
pub(crate) const WORD_FAILED: &str = "Failed";
pub(crate) const WORD_WAITING: &str = "Waiting";

/// G-C (§2 d): the act on the Containment row that writes `Inside`.
pub(crate) const USE_INSIDE: &str = "Use Inside";

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
    let spec = entry.operation.op_type().spec();
    ui.horizontal(|ui| {
        ui.label("Name:")
            .on_hover_text(format!("{}: {}", spec.label, spec.description));
        ui.text_edit_singleline(&mut entry.name);
    });
    // W2b: THE finding list of this toolpath and THE placement of each
    // finding. `placement::place` is the only reader of the list here.
    let tool_for_diags = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc);
    let diagnostics = panel_diagnostics(entry, preconditions, model_refs, suggest_warnings, inputs);
    let placement = super::placement::place(&diagnostics);

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
    let notes: Vec<Notice> = placement
        .notes
        .iter()
        .map(|d| Notice::new(Role::Info, d.face.as_str()).detail(super::placement::detail(d)))
        .collect();
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
        blocking: blocking.as_ref(),
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
                events,
            );
        }
        ToolpathTab::Passes => {
            draw_passes_tab(
                ui,
                entry,
                inputs,
                model_bbox,
                strategy_suggestion(suggest_warnings),
                selection_ui,
                events,
            );
        }

        ToolpathTab::FeedsSpeeds => draw_feeds_tab(ui, entry, inputs, model_bbox, events),

        // How moves connect: entry/exit, move optimization, retract
        // strategy, then arc fitting and dogbone (R2: the Dressup tab is
        // deleted; its rows are here).
        ToolpathTab::Linking => draw_linking_tab(ui, entry, tool_for_diags, toolpath_heights),

        ToolpathTab::Heights => {
            draw_heights_tab(ui, entry, inputs, heights_refusal, events);
        }
    }

    // Live look round 3: the caller records the tab on screen
    // (`GuiState::active_toolpath_tab`), which the height planes read.
    active_tab
}

/// W2b: THE finding list the toolpath inspector places: the core list of
/// the op (`collect_diagnostics`; with no tool, core's no-tool list) and
/// the shoe gap finding (H2: a project verdict about this toolpath). The
/// panel and the `properties::placed_findings` sentry door both call it.
pub(super) fn panel_diagnostics(
    entry: &crate::state::toolpath::ToolpathEntry,
    preconditions: &rs_cam_core::diagnostics::diagnose::PreconditionContext,
    model_refs: &rs_cam_core::diagnostics::diagnose::ModelRefContext,
    suggest_warnings: Option<&[rs_cam_core::feeds::suggest::SuggestWarning]>,
    inputs: &ToolpathPanelInputs,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    let tool = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc);
    let toolpath_heights = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let mut diagnostics = operations::collect_diagnostics(
        entry,
        tool,
        &inputs.stale_default_defects,
        toolpath_heights,
        preconditions,
        model_refs,
        suggest_warnings,
        inputs.load_verdict(),
    );
    diagnostics.extend(inputs.shoe.as_ref().and_then(|shoe| shoe.finding.clone()));
    diagnostics
}

// ── The tab bodies ──────────────────────────────────────────────────────
//
// UI-01: `draw_toolpath_panel` ran 1 255 lines because every tab drew
// inline. Each tab is now its own function, so the `match` above is the
// dispatch it always claimed to be. The moves are pure: no widget, no draw
// order and no egui id changed. Each function re-binds the `inputs` fields
// it needs under the names its body already used.

/// The Passes tab (R2; the Cut tab before it): the start stock, the op
/// parameters and the rest analysis. The tool is the header Tool row; the
/// input model and the machining boundary are on the Geometry tab.
/// `properties::tests` renders it (D3).
pub(super) fn draw_passes_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    strategy_suggestion: Option<crate::state::toolpath::ClearingStrategy>,
    selection_ui: &mut crate::state::pick::SelectionUi,
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
            super::pills::pill_previews,
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
                ui.horizontal(|ui| {
                    ui.label("Reference:");
                    let ref_label = entry
                        .rest_analysis
                        .reference_tool_id
                        .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
                        .map(|(_, name, _)| name.as_str())
                        .unwrap_or("Self / machined stock");
                    egui::ComboBox::from_id_salt("rest_analysis_reference_tool")
                        .truncate()
                        .selected_text(ref_label)
                        .show_ui(ui, |ui| {
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
                        })
                        .response
                        .on_hover_text(
                            "The reference the gate measures 'deeper than'. Unset = prefer \
                             the machined stock from a prior simulation, else a \
                             self-referenced bare-surface probe.",
                        );
                });
            }
            ui.horizontal(|ui| {
                ui.label("Cell Size:");
                ui.add(
                    egui::DragValue::new(&mut entry.rest_analysis.cell_mm)
                        .speed(0.05)
                        .range(0.05..=10.0)
                        .suffix(" mm"),
                )
                .on_hover_text("XY grid cell size for the rest field. Smaller = finer regions.");
            });
            ui.horizontal(|ui| {
                ui.label("Min Valley Depth:");
                ui.add(
                    egui::DragValue::new(&mut entry.rest_analysis.min_valley_depth)
                        .speed(0.01)
                        .range(0.0..=5.0)
                        .suffix(" mm"),
                )
                .on_hover_text(
                    "A cell counts as REST material once the reference floats more than \
                     this above the true surface.",
                );
            });
            ui.horizontal(|ui| {
                ui.label("Region Margin:");
                ui.add(
                    egui::DragValue::new(&mut entry.rest_analysis.region_margin_mm)
                        .speed(0.05)
                        .range(0.0..=10.0)
                        .suffix(" mm"),
                )
                .on_hover_text(
                    "Extra clearance added around detected regions beyond this \
                     toolpath's own tool radius.",
                );
            });
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

/// The Geometry tab (R2): the Input row, the op selection and the floor
/// (`geometry_tab.rs`, events only, L-1), the Machining Boundary section
/// and its selection list, then the Reach row of a reach-capable op.
/// `properties::tests` renders it (D3, G-C, G-D, R7a).
pub(super) fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    selection_ui: &mut crate::state::pick::SelectionUi,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
    reach_view: &mut ReachView,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    draw_input_row(ui, entry, inputs);
    super::geometry_tab::draw_geometry_tab(ui, entry, inputs, selection_ui, events);
    ui.add_space(6.0);
    draw_machining_boundary(ui, entry, inputs, containment_autocorrect);
    super::geometry_tab::draw_boundary_selection(ui, entry, inputs, selection_ui, events);
    if entry.operation.op_type().supports_reach_map() {
        ui.add_space(6.0);
        if draw_reach_row(ui, &inputs.reach, reach_view) {
            reach_view.toggle = true;
        }
    }
}

/// The Machining Boundary section of the Geometry tab (R2: it was at the
/// foot of the Cut tab). It writes the draft entry: the enable box, the
/// source, the holes option, the rest source, the containment and the
/// offset. The rows keep their pre-R2 form; W3a moves them onto `PropRow`.
fn draw_machining_boundary(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    containment_autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) {
    containment_autocorrect.scope(entry.id);
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();
    // W3.2: the boundary defines *what region to cut*, so it lives
    // under Geometry (R2: it was at the foot of the Cut tab).
    // D3: an op that no boundary moves (`TerritoryXy::SetupMap`, Pin
    // Drill: its holes are absolute setup-map positions) draws no
    // boundary controls. The panel reads the capability, not the op type.
    if entry
        .operation
        .capabilities()
        .territory_xy
        .session_clip_applies()
    {
        crate::ui::components::SectionHeader::new("Machining Boundary").show(ui);
        ui.checkbox(&mut entry.boundary.enabled, "Enable boundary")
            .on_hover_text(
                "Restrict toolpath to a boundary polygon. \
                 Moves outside the boundary are converted to rapids at safe Z.",
            );
        if entry.boundary.enabled {
            // R7a: the source buttons write the offset of THE source-change
            // rule (`BoundaryConfig::with_source`), which reads the tool
            // diameter of the op.
            let tool_diameter = entry
                .tool_id
                .and_then(|tool_id| tools.iter().find(|(id, _, _)| *id == tool_id))
                .map(|(_, _, diameter)| *diameter);
            // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the
            // one generation uses (`session/compute.rs` clones
            // `tc.boundary` unconditionally). G-D (ruling D-1): its name is
            // the hover of the Source combo, not a row above it; the row
            // repeated the combo. The controller assigns the model
            // silhouette on add for 3D ops on a mesh; that provenance is
            // not stored, so the hover never claims "(auto)".
            let source_summary = boundary_summary_line(&entry.boundary, false, models);
            // Source selector
            ui.horizontal(|ui| {
                ui.label("Source:");
                egui::ComboBox::from_id_salt("boundary_source")
                    .truncate()
                    .selected_text(entry.boundary.source.label())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(entry.boundary.source, BoundarySource::Stock),
                                "Stock",
                            )
                            .on_hover_text("Use the stock bounding rectangle")
                            .clicked()
                        {
                            entry.boundary.source = BoundarySource::Stock;
                        }
                        if ui
                            .selectable_label(
                                matches!(entry.boundary.source, BoundarySource::ModelSilhouette),
                                "Model Silhouette",
                            )
                            .on_hover_text(
                                "XY projection of the 3D model. Only machines \
                                 where the model actually is.",
                            )
                            .clicked()
                        {
                            entry.boundary = entry
                                .boundary
                                .with_source(BoundarySource::ModelSilhouette, tool_diameter);
                        }
                        // P4.3: the one door to a `Selection` boundary. It
                        // is enabled when a model holds a drawing item or a
                        // face item, so a STEP-only project reaches it.
                        let selection_enabled = inputs.boundary_selection_enabled;
                        if ui
                            .add_enabled(
                                selection_enabled,
                                egui::Button::selectable(
                                    matches!(
                                        entry.boundary.source,
                                        BoundarySource::Selection { .. }
                                    ),
                                    "Selection",
                                ),
                            )
                            .on_hover_text(if selection_enabled {
                                "Use a selection of items as the boundary: the closed \
                                 shapes of a drawing, or faces picked in the viewport. \
                                 The items can come from another model. Holes in the \
                                 shapes stay uncut. Edit the selection below."
                            } else {
                                "No model in this project has a drawing item or a face. \
                                 Import a DXF, an SVG or a STEP model to use this source."
                            })
                            .clicked()
                        {
                            // Keep the current selection when the source is
                            // already a selection; else write the value that a
                            // boundary Reset restores: the closed shapes of the
                            // first model with a closed shape, else no item.
                            // The source change writes offset 0 (R7a).
                            if !matches!(entry.boundary.source, BoundarySource::Selection { .. }) {
                                entry.boundary = entry.boundary.with_source(
                                    BoundarySource::Selection {
                                        selection: inputs.boundary_default.clone(),
                                        holes: false,
                                    },
                                    tool_diameter,
                                );
                            }
                        }
                        let has_rest_candidates = !boundary_source_candidates.is_empty();
                        if ui
                            .add_enabled(
                                has_rest_candidates,
                                egui::Button::selectable(
                                    matches!(
                                        entry.boundary.source,
                                        BoundarySource::DerivedRestRegions { .. }
                                    ),
                                    "Rest Regions",
                                ),
                            )
                            .on_hover_text(if has_rest_candidates {
                                "The regions another toolpath leaves: the pieces a \
                                 Geodesic Adaptive rough did not cut, or another op's \
                                 rest analysis. Pick the source below."
                            } else {
                                "No other toolpaths in this project yet — add \
                                 one and generate it to use as the source. Picking it here \
                                 switches its rest analysis on."
                            })
                            .clicked()
                        {
                            let default_source = boundary_source_candidates
                                .first()
                                .map(|(candidate_id, _, _, _, _, _)| *candidate_id)
                                .unwrap_or(entry.id);
                            entry.boundary.source = BoundarySource::DerivedRestRegions {
                                source_toolpath_id: default_source,
                            };
                            // G-C (D-2): the pick may set `Inside`.
                            pick_sets_inside(
                                &mut entry.boundary.containment,
                                inputs.rest_pick.as_ref(),
                                default_source,
                                containment_autocorrect,
                            );
                        }
                    })
                    .response
                    .on_hover_text(source_summary);
            });

            // GT-Q7: the boundary selection list follows this section on
            // the same tab. This section keeps the source and the holes
            // option.
            if let BoundarySource::Selection { holes, .. } = &mut entry.boundary.source {
                // The holes option inverts the regions: the toolpath cuts
                // inside the holes of the shapes and stays off the shapes.
                ui.checkbox(holes, "Machine the holes").on_hover_text(
                    "The toolpath cuts inside the holes of the shapes, for example \
                     the area inside an edge band, and stays off the shapes.",
                );
            }

            // Rest-regions source-toolpath picker (P2.2) — only shown
            // when `Source` above is set to `DerivedRestRegions`.
            // Candidates are every other toolpath in the session;
            // ones with a cached result whose rest regions are
            // already non-empty are labelled "· N regions" (G-C) and
            // sorted first, but a not-yet-generated toolpath is
            // still selectable — generation fails hard with a clear
            // message if the source turns out unusable.
            if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                &mut entry.boundary.source
            {
                ui.horizontal(|ui| {
                    ui.label("Rest source:");
                    let current = boundary_source_candidates
                        .iter()
                        .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
                    let current_label = current.map_or_else(
                        || "(toolpath not found)".to_owned(),
                        |(_, name, _, regions, _, _)| rest_source_label(name, regions.as_deref()),
                    );
                    // G-C (§6 b): the top of the regions first, then the
                    // combo's own hover.
                    let hover = match current.and_then(|(_, _, _, _, _, top_z)| *top_z) {
                        Some(z) => format!(
                            "Top of the regions: Z {z:.2}.\nThe toolpath whose regions bound \
                             this one. Picking it switches its rest analysis on."
                        ),
                        None => "The toolpath whose regions bound this one. Picking it \
                                 switches its rest analysis on."
                            .to_owned(),
                    };
                    let mut sorted = boundary_source_candidates.to_vec();
                    sorted.sort_by_key(|(_, _, ready, _, _, _)| !*ready);
                    egui::ComboBox::from_id_salt("boundary_rest_source")
                        .truncate()
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (candidate_id, name, _, regions, _, _) in &sorted {
                                let label = rest_source_label(name, regions.as_deref());
                                let selected = *source_toolpath_id == *candidate_id;
                                if ui.selectable_label(selected, label).clicked() {
                                    *source_toolpath_id = *candidate_id;
                                }
                            }
                        })
                        .response
                        .on_hover_text(hover);
                });

                // Sliver-storm / giant-region caption (2026-07-06
                // incident): the SOURCE toolpath is who suffers the
                // per-island generation explosion or the "barely
                // restricts anything" giant-region case, so classify
                // ITS cached regions (captured in
                // `boundary_source_candidates` alongside `ready`),
                // not this consumer's own (this toolpath has none —
                // it's the one consuming the boundary).
                //
                // LH-2: the denominator is the SOURCE toolpath's own
                // rest-grid footprint (captured alongside its
                // regions), not this model's bounding rectangle.
                let selected = boundary_source_candidates
                    .iter()
                    .find(|(candidate_id, _, _, _, _, _)| candidate_id == source_toolpath_id);
                let selected_regions =
                    selected.and_then(|(_, _, _, regions, _, _)| regions.as_ref());
                // `None` twice over: no candidate selected, or the
                // selected one has no rest grid. Both are "not
                // measured", and `classify_rest_regions` stays silent.
                let source_footprint_area = selected.and_then(|(_, _, _, _, area, _)| *area);
                if let Some(regions) = selected_regions
                    && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
                        regions,
                        source_footprint_area,
                    )
                {
                    ui.add(rest_region_pathology_line(pathology));
                }
            }

            // Containment mode. G-C (§2 c-e): for a rest-regions source
            // whose tool is smaller than this one, `Center` and `Outside`
            // are grey (they refuse, D53) with the sizes on hover; while the
            // draft holds one of them, `[Use Inside]` sits at the right of
            // the row, unless `Inside` would collapse the boundary. The `⟲`
            // badge marks a containment the `Rest Regions` pick set (D-2).
            let draft_source = match entry.boundary.source {
                BoundarySource::DerivedRestRegions { source_toolpath_id } => {
                    Some(source_toolpath_id)
                }
                _ => None,
            };
            let refused = inputs
                .rest_containment
                .as_ref()
                .filter(|check| Some(check.larger.source_toolpath_id) == draft_source);
            let before = entry.boundary.containment;
            ui.horizontal(|ui| {
                ui.label("Containment:");
                egui::ComboBox::from_id_salt("boundary_contain")
                    .truncate()
                    .selected_text(match entry.boundary.containment {
                        BoundaryContainment::Center => "Center",
                        BoundaryContainment::Inside => "Inside",
                        BoundaryContainment::Outside => "Outside",
                    })
                    .show_ui(ui, |ui| {
                        let mut option = |ui: &mut egui::Ui,
                                          value: BoundaryContainment,
                                          label: &str,
                                          hover: &str| {
                            let enabled = refused.is_none() || value == BoundaryContainment::Inside;
                            let response = ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::selectable(
                                        entry.boundary.containment == value,
                                        label,
                                    ),
                                )
                                .on_hover_text(hover);
                            let response = match refused {
                                Some(check) => response.on_disabled_hover_text(check.hover()),
                                None => response,
                            };
                            if response.clicked() {
                                entry.boundary.containment = value;
                            }
                        };
                        option(
                            ui,
                            BoundaryContainment::Center,
                            "Center",
                            "Tool center stays inside boundary",
                        );
                        option(
                            ui,
                            BoundaryContainment::Inside,
                            "Inside",
                            "Entire tool stays inside boundary (shrinks by tool radius)",
                        );
                        option(
                            ui,
                            BoundaryContainment::Outside,
                            "Outside",
                            "Tool edge can extend outside boundary",
                        );
                    });
                if let Some(hover) = containment_autocorrect.hover() {
                    ui.add(
                        crate::ui::components::ProvenanceBadge::new(
                            crate::ui::components::ProvKind::AutoCorrect,
                        )
                        .compact(),
                    )
                    .on_hover_text(hover);
                }
                if let Some(check) = refused
                    && entry.boundary.containment != BoundaryContainment::Inside
                {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let keeps = check.inside == crate::state::runtime::InsideAnswer::Keeps;
                        let response = ui
                            .add_enabled(keeps, egui::Button::new(USE_INSIDE).small())
                            .on_hover_text(check.hover())
                            .on_disabled_hover_text(check.hover());
                        if response.clicked() {
                            entry.boundary.containment = BoundaryContainment::Inside;
                        }
                    });
                }
            });
            // The badge stands until the operator edits the containment.
            if entry.boundary.containment != before {
                containment_autocorrect.clear();
            }

            // Offset
            ui.horizontal(|ui| {
                ui.label("Offset:");
                ui.add(
                    egui::DragValue::new(&mut entry.boundary.offset)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .on_hover_text(
                    "Expand (positive) or shrink (negative) the boundary. \
                     Applied before tool-radius containment.",
                );
            });
        }
    }
}

/// The Feeds tab: how fast.
fn draw_feeds_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    events: &mut Vec<AppEvent>,
) {
    let tool_configs = inputs.tool_configs.as_slice();
    let material = &inputs.material;
    let machine = &inputs.machine;
    let spindle_strategy = inputs.spindle_strategy;
    let project_default_rpm = inputs.project_default_rpm;
    let load_verdict = inputs.load_verdict();

    let tool_info = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| (t.diameter, t.tool_type));
    let (tool_diameter, tool_type) =
        tool_info.unwrap_or((6.0, crate::state::job::ToolType::EndMill));
    ui.horizontal(|ui| {
        if ui
            .button("Explore…")
            .on_hover_text("Open the feed-versus-RPM nomogram for this operation.")
            .clicked()
        {
            events.push(AppEvent::OpenFeedsModal(entry.id));
        }
    });
    ui.add_space(4.0);
    draw_speed_controls(ui, entry);
    ui.add_space(4.0);
    if let Some(tool_cfg) = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, t)| t)
    {
        calculate_and_apply_feeds(
            ui,
            entry,
            tool_cfg,
            material,
            machine,
            spindle_strategy,
            project_default_rpm,
            load_verdict,
            model_bbox,
            events,
        );
    }
    // Vendor cutting data viewer (always available, filtered by tool)
    draw_vendor_lut_viewer(ui, tool_type, tool_diameter);
}

/// The Heights tab: the Z planes.
///
/// P3.1: the diagram and the rows read THE heights of the session
/// (`ProjectSession::toolpath_heights`). When they do not resolve, the tab
/// names the error and draws no row: a row needs the stock and the model Z.
fn draw_heights_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &super::ToolpathPanelInputs,
    heights_refusal: &mut crate::state::panels::HeightsRefusal,
    events: &mut Vec<AppEvent>,
) {
    heights_refusal.scope(entry.id);
    let answer = match inputs.toolpath_heights.as_ref() {
        Some(Ok(answer)) => Some(answer),
        Some(Err(error)) => {
            ui.add(crate::ui::components::OneLine::new(format!(
                "Heights: {error}"
            )));
            None
        }
        None => None,
    };
    // F1.19 / G-BOTTOMPIN: the Bottom row is annotated per operation,
    // so the panel needs the operation. Read before the mutable
    // borrow of `entry.heights`; `OperationType` is `Copy`.
    // F-6 / R33: the diagram takes the same operation, so its Bottom
    // line agrees with the sentence the panel prints beside the row.
    let op_type = entry.operation.op_type();
    // P3.7, D6: the floor of an `OpFloor` op. The diagram drags it in its
    // own reference (L-10), and the Bottom height row under the Top height
    // row edits it. Both write ONE draft floor; it goes back through the
    // config after both, as every op row does.
    let floor_inputs = answer.and_then(|answer| {
        entry
            .operation
            .floor()
            .map(|_| super::FloorInputs::new(answer, &entry.heights, op_report(inputs)))
    });
    // G-DEPTHSTOCK: the caution of a floor below the stock bottom goes
    // with the Bottom height row. Read before the mutable borrows.
    let depth_caution = answer.and_then(|answer| {
        operations::depth_beyond_stock(&entry.operation, &entry.heights, answer)
    });
    let mut floor = entry.operation.floor();
    // IC1: the diagram summarises the five planes, so it comes first. The
    // rows below it set them.
    if let Some(answer) = answer {
        let diagram_floor = floor
            .as_mut()
            .zip(floor_inputs.as_ref())
            .map(|(floor, inputs)| super::DiagramFloor { floor, inputs });
        draw_height_diagram(
            ui,
            &mut entry.heights,
            answer,
            op_type,
            diagram_floor,
            heights_refusal,
        );
        ui.add_space(6.0);
    }
    draw_heights_params(
        ui,
        &mut entry.heights,
        answer,
        op_type,
        floor.as_mut().map(|floor| operations::BottomHeight {
            floor,
            inputs: floor_inputs.as_ref(),
            depth_caution: depth_caution.as_ref(),
        }),
        heights_refusal,
    );
    if let Some(floor) = floor
        && entry.operation.floor().as_ref() != Some(&floor)
    {
        let _ = entry.operation.set_floor(floor);
    }
    // H2: the dust shoe sits with the safety heights. It moves no
    // toolpath; it decides what the checks read above the tool.
    if let Some(shoe) = inputs.shoe.as_ref() {
        ui.add_space(6.0);
        super::shoe_row::draw_shoe_row(ui, entry.id, shoe, events);
    }
}

/// The `selection_status` report of the op selection, when it answered.
fn op_report(
    inputs: &super::ToolpathPanelInputs,
) -> Option<&rs_cam_core::session::SelectionReport> {
    match inputs.selection_report.as_ref() {
        Some(Ok(report)) => Some(report),
        Some(Err(_)) | None => None,
    }
}

/// The Linking tab (R2): entry and exit, links and retracts
/// (`draw_linking_params`), then arc fitting and dogbone
/// (`draw_dressup_params`; the Dressup tab before R2). Dogbone draws only
/// where it applies: not on a 3D surface op (`OperationConfig::is_3d`),
/// unless a project file turned it on there.
fn draw_linking_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    tool: Option<&crate::state::job::ToolConfig>,
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
) {
    draw_linking_params(ui, entry, tool, toolpath_heights);
    ui.add_space(6.0);
    let dogbone_applies = !entry.operation.is_3d() || entry.dressups.dogbone.is_some();
    draw_dressup_params(ui, &mut entry.dressups, dogbone_applies);
    // "Reset to recommended" writes the dressup set of the op role; R2
    // deleted the "n/8 dressups active" count beside it.
    let role = entry.operation.op_type().spec().ui_process_role;
    ui.add_space(4.0);
    if ui
        .small_button("Reset to recommended")
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

/// The Reach row of the Geometry tab (R3): one read-only `PropRow`. The
/// value is the unreachable share of the reach overlay (CAUTION above
/// zero), or "computing…". The quiet action Show/Hide asks the caller to
/// switch the overlay; the dock and the legend own the rest. True when the
/// action was clicked.
fn draw_reach_row(ui: &mut egui::Ui, reach: &ReachPanelSummary, view: &ReachView) -> bool {
    let (value, state, detail): (String, Option<Role>, Option<String>) = match reach {
        ReachPanelSummary::Measured {
            unreachable_pct,
            max_gap_mm,
            grid_line,
            area_basis_note,
            over_statement_note,
            tolerance_below_floor,
        } => {
            // The base comes from `ReachMap::area_basis_note`; the bias
            // sentence is `ReachMap::over_statement_note`, quoted on the
            // hover only when the bar is under the grid floor.
            let mut detail =
                format!("Worst gap {max_gap_mm:.2} mm. {area_basis_note}\n{grid_line}");
            if *tolerance_below_floor {
                detail.push('\n');
                detail.push_str(over_statement_note);
            }
            (
                format!("Unreachable {unreachable_pct:.0} %"),
                (*unreachable_pct > 0.0).then_some(Role::Caution),
                Some(detail),
            )
        }
        ReachPanelSummary::Computing => ("computing\u{2026}".to_owned(), None, None),
        ReachPanelSummary::NotMeasured => (
            "not measured".to_owned(),
            Some(Role::Caution),
            Some("The reach walk found no upward surface to judge.".to_owned()),
        ),
        ReachPanelSummary::Failed(message) => (
            "failed".to_owned(),
            Some(Role::Caution),
            Some(message.clone()),
        ),
        ReachPanelSummary::Idle => {
            let value = if view.on { "waiting" } else { "off" };
            (value.to_owned(), None, None)
        }
    };
    let action = if view.on { "Hide" } else { "Show" };
    let row = PropRow::new("Reach")
        .state(state)
        .hover(Some(REACH_ROW_HOVER))
        .detail(detail.as_deref());
    // A refused switch has no action; the reason is the hover.
    let row = match (&view.refusal, view.on) {
        (Some(_), false) => row,
        _ => row.action(action),
    };
    let out = row.show_read_only(ui, value);
    if let Some(reason) = view.refusal.as_ref().filter(|_| !view.on) {
        let _ = out
            .response
            .on_hover_text(format!("Show is not available: {reason}."));
    }
    out.action_clicked
}

/// The hover of the Reach row.
const REACH_ROW_HOVER: &str = "The share of the surface that the cutter of this toolpath \
    cannot form within its tolerance. Show colours the model; the legend explains the colours.";

/// R3: the reach overlay as the Reach row reads it, and the Show/Hide
/// request of the row. The caller fills `on` and `refusal` from the overlay
/// registry row `reach_map` and, when `toggle` is set after the draw,
/// switches the overlay through `registry::set_overlay`.
#[derive(Default)]
pub(crate) struct ReachView {
    /// The reach overlay is on.
    pub on: bool,
    /// Why the overlay cannot go on now (the registry precondition).
    pub refusal: Option<String>,
    /// The row asked to switch the overlay this frame.
    pub toggle: bool,
}

/// What the status line of the header shows (FR-2 F22, W2b).
pub(super) struct HeaderRow<'a> {
    /// THE run state of the toolpath (core `toolpath_run_state`).
    pub run_state: Option<&'a rs_cam_core::session::RunState>,
    /// The GUI can submit a run: the project has a tool and no validator
    /// refusal stands.
    pub can_run: bool,
    /// The op does not regenerate on its own (the hover of "Ready").
    pub manual_generation: bool,
    /// The sentence of a blocked submit of a waiting rest op, when the GUI
    /// holds one (the hover of "Waiting").
    pub waiting_detail: Option<&'a str>,
    /// F20: the first blocking finding of the stored result.
    pub blocking: Option<&'a rs_cam_core::diagnostics::Diagnostic>,
    /// The move count of the stored result.
    pub moves: Option<usize>,
    pub feed_stage: Option<rs_cam_core::session::FeedStage>,
    /// G-B: the moves of the stored result that carry a feed cap.
    pub caps: Option<crate::state::runtime::CapFigures>,
}

impl HeaderRow<'_> {
    /// True when the state word is the F20 safety signal "Done · BLOCKED".
    fn is_blocked(&self) -> bool {
        matches!(self.run_state, Some(rs_cam_core::session::RunState::Done))
            && self.blocking.is_some()
    }

    /// The move count text: "old: N moves" on a result that is out of date.
    fn moves_text(&self) -> Option<String> {
        let moves = self.moves?;
        Some(
            if matches!(
                self.run_state,
                Some(rs_cam_core::session::RunState::OutOfDate)
            ) {
                format!("old: {moves} moves")
            } else {
                format!("{moves} moves")
            },
        )
    }

    /// The word, its colour and its hover.
    fn word(&self) -> (&'static str, egui::Color32, String) {
        use crate::ui::tokens;
        use rs_cam_core::session::RunState;
        if self.is_blocked()
            && let Some(finding) = self.blocking
        {
            return (
                crate::ui::toolpath_panel::BLOCKED_HEADER,
                tokens::DANGER,
                format!(
                    "A blocking finding stands, so this toolpath does not go to G-code. {}",
                    finding.message
                ),
            );
        }
        match self.run_state {
            Some(RunState::Disabled) => (
                WORD_DISABLED,
                tokens::TEXT_MUTED,
                "Disabled: this operation does not go to G-code.".to_owned(),
            ),
            Some(RunState::NotReady { .. }) => (
                WORD_NOT_READY,
                tokens::TEXT_MUTED,
                "Not ready: generation refuses until the marked inputs are set.".to_owned(),
            ),
            Some(RunState::Generating) => (
                WORD_GENERATING,
                tokens::TEXT_BODY,
                "Generating: the operation is in generation.".to_owned(),
            ),
            Some(RunState::Done) => (
                WORD_DONE,
                tokens::OK,
                "Done: the result answers the settings shown here.".to_owned(),
            ),
            Some(RunState::OutOfDate) => (
                WORD_OUT_OF_DATE,
                tokens::CAUTION,
                "Out of date: this operation generated, then one of its inputs changed. \
                 The path in the viewport and the figures on this panel are from that \
                 earlier generation."
                    .to_owned(),
            ),
            Some(RunState::Failed { .. }) => (
                WORD_FAILED,
                tokens::DANGER,
                "Failed: the last run refused. The issue row under this line gives the \
                 reason. An edit of the inputs clears it."
                    .to_owned(),
            ),
            Some(RunState::Waiting { .. }) => (
                WORD_WAITING,
                tokens::CAUTION,
                self.waiting_detail.map_or_else(
                    || {
                        "Waiting: this rest operation needs the simulated stock of the \
                         operations before it. Generate runs them and the simulation first."
                            .to_owned()
                    },
                    str::to_owned,
                ),
            ),
            Some(RunState::Ready) | None => (
                WORD_READY,
                tokens::TEXT_BODY,
                if self.manual_generation {
                    "Ready: this operation has no result yet, and it does not regenerate \
                     on its own. Press G, or click Generate."
                        .to_owned()
                } else {
                    "Ready: this operation has no result yet.".to_owned()
                },
            ),
        }
    }

    /// The one button of the state, or `None` for no button. W2b: a state
    /// that asks for a tool or for a fix of the inputs shows a disabled
    /// Generate; the red Tool row and the issue rows are the call to action.
    fn button(&self) -> Option<crate::ui::components::Button> {
        use crate::ui::components::Button;
        use rs_cam_core::session::{NextAction, RunState, next_action};
        let state = self.run_state.cloned().unwrap_or(RunState::Ready);
        let button = match next_action(&state) {
            NextAction::ChooseTool | NextAction::FixInputs => {
                return Some(Button::primary("Generate").enabled(false));
            }
            NextAction::Generate => Button::primary("Generate"),
            NextAction::Regenerate => Button::quiet("Regenerate"),
            NextAction::Retry => Button::quiet("Retry"),
            NextAction::None => match state {
                // A current result can run again on request.
                RunState::Done => Button::quiet("Regenerate"),
                _ => return None,
            },
        };
        Some(button.enabled(self.can_run))
    }
}

/// The status line of the header (FR-2 F22, W2b). The row never grows the
/// panel. True when its button was clicked.
///
/// From the right: the one button, the notes counter, then the move count
/// as faint meta (its hover holds the feed stage and the caps). The state
/// word takes the rest and truncates, except the F20 safety signal
/// "Done · BLOCKED": it takes its full width first and the move count drops.
pub(super) fn draw_generate_row(ui: &mut egui::Ui, row: &HeaderRow<'_>, notes: &[Notice]) -> bool {
    let (word, colour, hover) = row.word();
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clicked = row.button().is_some_and(|button| ui.add(button).clicked());
            let _ = NotesCounter::new(notes).show(ui);
            if let Some(text) = row.moves_text() {
                let word_room = if row.is_blocked() {
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    ui.painter()
                        .layout_no_wrap(word.to_owned(), font, colour)
                        .size()
                        .x
                } else {
                    0.0
                };
                if small_width(ui, &text) + word_room <= ui.available_width() {
                    ui.label(
                        egui::RichText::new(text)
                            .small()
                            .color(crate::ui::tokens::TEXT_FAINT),
                    )
                    .on_hover_text(figures_hover(row));
                }
            }
            let rest = ui.available_width().max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(rest, crate::ui::tokens::ROW_DENSE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_max_width(rest);
                    let text = egui::RichText::new(word).color(colour);
                    let label = if row.is_blocked() {
                        egui::Label::new(text).extend()
                    } else {
                        egui::Label::new(text)
                            .truncate()
                            .show_tooltip_when_elided(false)
                    };
                    let figures = figures_hover(row);
                    let hover = if figures.is_empty() {
                        hover
                    } else {
                        format!("{hover}\n{figures}")
                    };
                    ui.add(label).on_hover_text(hover);
                },
            );
            clicked
        })
        .inner
    })
    .inner
}

/// The validator text of a missing model; core states it as
/// `ref.model_missing` (an issue row of its own).
const SELECTED_MODEL_MISSING: &str = "Selected model is missing";

/// What an issue row of the stack does when its Fix button is clicked.
enum IssueAct {
    None,
    /// Apply the stale-default fix of `stale_default_defects[index]`.
    StaleDefault(usize),
    /// Generate the source toolpath of a rest-boundary refusal.
    Generate(crate::state::toolpath::ToolpathId),
}

/// The validator refusals that no core finding states (W2b). Each other
/// refusal has ONE owner elsewhere: the no-tool text and the tool-kind
/// refusal are the Tool row, the stepover and the drill holes are core
/// findings the placement gives a place, the selection refusal is
/// `precondition.selection_refused`, a missing model is `ref.model_missing`,
/// and the D53 refusal is the
/// Containment row on the Geometry tab. What is left (the model geometry
/// checks of `validate_toolpath`) is a core gap; these rows hold it until
/// core states it.
fn gate_refusals(
    validation_errors: &[String],
    entry: &crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
    cut_refusal: Option<&str>,
) -> Vec<String> {
    // With no tool the validator gives only the no-tool text.
    let Some(tool) = tool else {
        return Vec::new();
    };
    let tool_kind = entry
        .operation
        .op_type()
        .tool_refusal(tool.tool_type.cutter_kind());
    let section_refusal = inputs
        .selection_report
        .as_ref()
        .and_then(|read| read.as_ref().ok())
        .and_then(|report| report.refusal.as_ref())
        .map(ToString::to_string);
    let hole = operations::hole_refusal(&inputs.validation, entry.id);
    validation_errors
        .iter()
        .filter(|err| {
            let err = err.as_str();
            tool_kind.as_deref() != Some(err)
                && err != operations::registry::STEPOVER_EXCEEDS_DIAMETER_REFUSAL
                && hole != Some(err)
                && section_refusal.as_deref() != Some(err)
                && cut_refusal != Some(err)
                && err != SELECTED_MODEL_MISSING
        })
        .cloned()
        .collect()
}

/// The issue stack under the header (W2b): the NoticeStack issue form, a
/// cap of 3, every blocker shown. The rows, in order:
///
/// 1. The last run's refusal (`RunState::Failed`), ONE row. A rest-boundary
///    refusal takes its short face and the `⟳` action (G-E (3)). A D53
///    refusal takes none: the Containment row owns it.
/// 2. The findings the placement gives the stack, face on the row, detail
///    on hover, one Fix button for a stale-default fix.
/// 3. The validator refusals no core finding states ([`gate_refusals`]).
fn draw_issue_stack(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    placement: &super::placement::Placement<'_>,
    cut_refusal: Option<&str>,
    gates: &[String],
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::diagnostics::DiagnosticFix;
    let mut notices = Vec::new();
    let mut acts = Vec::new();
    // A D53 refusal of the last run is the Containment row's (the Geometry
    // tab and its dot own it), so it takes no issue row.
    if let Some(rs_cam_core::session::RunState::Failed { reason }) = inputs
        .run_state
        .as_ref()
        .filter(|state| !matches!(state, rs_cam_core::session::RunState::Failed { reason } if cut_refusal == Some(reason.as_str())))
    {
        let face = inputs
            .rest_boundary_refusal
            .as_ref()
            .filter(|refusal| refusal.to_string() == *reason)
            .and_then(rest_refusal_face);
        match face {
            Some(face) => {
                let mut notice = Notice::new(Role::Danger, face.text).detail(reason.as_str());
                let act = match face.generate {
                    Some((source, _)) => {
                        notice = notice.fix(GENERATE_SOURCE_GLYPH);
                        IssueAct::Generate(source)
                    }
                    None => IssueAct::None,
                };
                notices.push(notice);
                acts.push(act);
            }
            None => {
                notices.push(Notice::new(Role::Danger, reason.as_str()));
                acts.push(IssueAct::None);
            }
        }
    }
    for d in &placement.issues {
        let mut notice = Notice::new(super::placement::role_of(d), d.face.as_str())
            .detail(super::placement::detail(d));
        let defect = match &d.fix {
            Some(DiagnosticFix::ApplyStaleDefault { rule_id, .. }) => inputs
                .stale_default_defects
                .iter()
                .position(|defect| defect.rule_id.id() == rule_id.as_str()),
            _ => None,
        };
        let act = match defect {
            Some(index) => {
                notice = notice.fix("Fix");
                IssueAct::StaleDefault(index)
            }
            None => IssueAct::None,
        };
        notices.push(notice);
        acts.push(act);
    }
    for gate in gates {
        notices.push(Notice::new(Role::Danger, gate.as_str()));
        acts.push(IssueAct::None);
    }
    if notices.is_empty() {
        return;
    }
    let expand_id = ui.id().with("tp_issues_all").with(entry.id.raw());
    let expanded = ui.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
    ui.add_space(4.0);
    let response = NoticeStack::issues(notices)
        .expanded(expanded)
        .show_issues(ui);
    if response.expand {
        ui.data_mut(|d| d.insert_temp(expand_id, true));
    }
    match response.fix.and_then(|index| acts.get(index)) {
        Some(IssueAct::StaleDefault(index)) => {
            if let Some(defect) = inputs.stale_default_defects.get(*index) {
                rs_cam_core::compute::validate::apply_stale_default_to_op(
                    &mut entry.operation,
                    &mut entry.feeds_provenance,
                    defect,
                );
                entry.stale_since = Some(std::time::Instant::now());
            }
        }
        Some(IssueAct::Generate(source)) => events.push(AppEvent::GenerateToolpath(*source)),
        Some(IssueAct::None) | None => {}
    }
}

/// G-C (D-2, ruled yes): the `Rest Regions` pick of `source` writes
/// `Inside` when this op's tool is larger than the source's (`pick`), the
/// containment is not `Inside` yet, and `Inside` keeps area (the collapse
/// check, G-BOUNDCOLLAPSE). The Containment combo then shows the `⟲` badge
/// with the sizes on its hover. True when it wrote `Inside`.
pub(super) fn pick_sets_inside(
    containment: &mut BoundaryContainment,
    pick: Option<&super::RestContainment>,
    source: crate::state::toolpath::ToolpathId,
    autocorrect: &mut crate::state::panels::ContainmentAutoCorrect,
) -> bool {
    let Some(pick) = pick.filter(|pick| {
        pick.larger.source_toolpath_id == source
            && pick.inside == crate::state::runtime::InsideAnswer::Keeps
    }) else {
        return false;
    };
    if *containment == BoundaryContainment::Inside {
        return false;
    }
    *containment = BoundaryContainment::Inside;
    autocorrect.set(format!(
        "Set to Inside: this tool (\u{00D8}{:.2}) is larger than the tool of '{}' \
         (\u{00D8}{:.2}).",
        2.0 * pick.larger.tool_radius_mm,
        pick.larger.source_name,
        2.0 * pick.larger.source_tool_radius_mm,
    ));
    true
}

/// G-C (§6, review): the item text of a rest source: `{name} · {N} regions`
/// when its result has regions, else the plain name. One phrase for the
/// count, as the source's own disclosure title.
pub(super) fn rest_source_label(
    name: &str,
    regions: Option<&Vec<rs_cam_core::polygon::Polygon2>>,
) -> String {
    match regions.map(Vec::len) {
        Some(n) if n > 0 => format!("{name} \u{00B7} {}", regions_phrase(n)),
        _ => name.to_owned(),
    }
}

/// `N regions` (`1 region`).
pub(super) fn regions_phrase(n: usize) -> String {
    if n == 1 {
        "1 region".to_owned()
    } else {
        format!("{n} regions")
    }
}

/// G-E (3): the one-line face of a rest-boundary refusal, and the
/// toolpath its action generates.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RefusalFace {
    pub text: String,
    /// The source toolpath that the `⟳` action generates, and its name.
    pub generate: Option<(crate::state::toolpath::ToolpathId, String)>,
}

/// G-E (3), §7.7: the short face of a rest-boundary refusal. `None` for a
/// refusal with no short face (a lost source id), which keeps its text.
pub(super) fn rest_refusal_face(
    refusal: &rs_cam_core::session::RestBoundaryRefusal,
) -> Option<RefusalFace> {
    use rs_cam_core::session::RestBoundaryRefusal;
    match refusal {
        RestBoundaryRefusal::NotGenerated {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("Generate '{source_name}' first"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::NoRegions {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("'{source_name}' left no regions"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::OwnRegions => Some(RefusalFace {
            text: "The source is this op".to_owned(),
            generate: None,
        }),
        RestBoundaryRefusal::SourceMissing { .. } => None,
    }
}

/// The glyph of the action that generates a refusal's source.
pub(super) const GENERATE_SOURCE_GLYPH: &str = "\u{27F3}";

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

/// The figures as one hover line, for a state word that may push them
/// out of the row.
fn figures_hover(row: &HeaderRow<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(text) = row.moves_text() {
        parts.push(text);
    }
    if let Some(caps) = row.caps {
        parts.push(caps.line());
    }
    if let Some(stage) = row.feed_stage {
        parts.push(stage.statement());
    }
    parts.join("\n")
}

/// The width of one small label of `text`.
fn small_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Small.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, crate::ui::tokens::TEXT_FAINT)
        .size()
        .x
        + ui.spacing().item_spacing.x
}
