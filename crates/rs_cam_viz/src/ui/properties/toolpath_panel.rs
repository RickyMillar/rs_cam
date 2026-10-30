//! `draw_toolpath_panel` — the tab host itself.
//!
//! One function. It draws the shared header, the tab strip, the diagnostic
//! ribbon and whichever tab is active, and it delegates every tab body to a
//! sibling child of `ui::properties`.

use super::feeds_speeds::{calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer};
use super::linking_dressup::{draw_dressup_params, draw_linking_params};
use super::operations::{draw_height_diagram, draw_heights_params, validate_toolpath};
use super::pills::PillSuggestions;
use super::tab_badges::{
    RowTier, compute_tab_badges, draw_input_row, draw_start_from, draw_tool_row,
    draw_toolpath_tabs, merge_stateful_gate_rows, render_diagnostic_row,
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
use crate::ui::components::{OneLine, PropRow, Role, UiExt as _};

/// The header line of a selection refusal (FR-2 F27, IC1): the Geometry tab
/// owns the text; the header names the tab, and the full text is the hover.
pub(crate) const REFUSED_SEE_GEOMETRY: &str = "Refused \u{00B7} see Geometry";

/// G-E (§7.7): the state word of the Generate row when the last run
/// refused. The full refusal is its hover.
pub(crate) const REFUSED_STATE: &str = "Refused";

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
    let stale_default_defects = inputs.stale_default_defects.as_slice();
    let load_verdict = inputs.load_verdict();
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
    // R2 / P1: the Tool row is the one tool control of the inspector.
    draw_tool_row(ui, entry, inputs, events);

    ui.separator();

    // Generate button + status (always visible) — promoted to the top
    // (SHE-004) so "is it generated? any warnings?" reads before the geometry
    // combos the user rarely revisits after setup.
    ui.add_space(4.0);
    let validation_errors = validate_toolpath(entry, validation);
    // F20: the first blocking finding of the stored result.
    let blocking = crate::ui::toolpath_panel::blocking_finding(
        entry.run.result().map(|result| &result.stats),
        entry.id,
    );
    let can_generate = !tools.is_empty() && validation_errors.is_empty();
    // DC5 (Pattern C): the state the "requires manual generation" sentence
    // used to carry. Read before the row so the closure holds no extra
    // borrow of `entry`.
    let manual_generation =
        !entry.auto_regen && matches!(entry.run, crate::state::runtime::ToolpathRun::Pending);
    let header = HeaderRow {
        can_generate,
        freshness,
        manual_generation,
        blocking: blocking.as_ref(),
        moves: entry.run.result().map(|result| result.stats.move_count),
        feed_stage: inputs.feed_stage,
        caps: inputs.caps,
    };
    if draw_generate_row(ui, &header) {
        events.push(AppEvent::GenerateToolpath(entry.id));
    }
    // G-E (§7.5, §7.7): the last run's refusal gets one face line under
    // the row, which truncates; its hover holds the full text. A
    // validation refusal answers the settings on screen, so when one
    // stands it takes the line and the state word keeps the last run.
    if let crate::state::freshness::FreshnessState::Error(e) = freshness
        && validation_errors.is_empty()
    {
        // G-E (3): a rest-boundary refusal that is the last run's text
        // gets its short face and its action; any other refusal shows its
        // own text, truncated.
        match inputs
            .rest_boundary_refusal
            .as_ref()
            .and_then(|refusal| rest_refusal_face(refusal).filter(|_| refusal.to_string() == *e))
        {
            Some(face) => draw_refusal_face(ui, &face, e, events),
            None => {
                ui.add(OneLine::new(e.as_str()).color(crate::ui::tokens::DANGER));
            }
        }
    }
    // G-C (§2 d): the D53 refusal, owned by the Containment row of the
    // Geometry tab (R2: the Machining Boundary section moved there).
    let cut_refusal = operations::rest_consumer_refusal(validation, entry).map(str::to_owned);
    if !validation_errors.is_empty() {
        // L-2, FR-2 F27 (IC1): the Geometry tab owns the refusal text of
        // the selection. A validation error that IS that refusal (the
        // section refusal, or the drill form of it from `hole_refusal`) gets
        // ONE short line here, with the full text on hover. Before F27 the
        // drill form did not match the section text, so the header painted
        // the full paragraph a second time and pushed the tabs down.
        let section_refusal = inputs
            .selection_report
            .as_ref()
            .and_then(|read| read.as_ref().ok())
            .and_then(|report| report.refusal.as_ref())
            .map(ToString::to_string);
        let hole_refusal = operations::hole_refusal(validation, entry.id);
        let (owned, other): (Vec<&String>, Vec<&String>) =
            validation_errors.iter().partition(|err| {
                section_refusal.as_deref() == Some(err.as_str())
                    || hole_refusal == Some(err.as_str())
            });
        // G-C: the D53 refusal is the Geometry tab's (the Containment row).
        let cut_owned = other
            .iter()
            .any(|err| cut_refusal.as_deref() == Some(err.as_str()));
        // G-E (4), §7.2 rule 5: above the tab strip at most ONE refusal
        // line. The Geometry tab's refusal names its tab; else the first
        // validator sentence is the face, truncated. The hover holds every
        // validation error. Before G-E each sentence wrapped (up to 3 rows
        // each), and the tabs moved down.
        let detail = owned
            .iter()
            .chain(other.iter())
            .map(|err| err.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let face = if !owned.is_empty() || cut_owned {
            REFUSED_SEE_GEOMETRY
        } else {
            other.first().map_or("", |err| err.as_str())
        };
        ui.add(
            OneLine::new(face)
                .color(crate::ui::tokens::DANGER)
                .detail(detail),
        );
    }

    // Contextual diagnostics (non-blocking). Native `Diagnostic`
    // rendering — splits findings into three tiers:
    //  * Actionable (state Current, severity ≥ Caution) — coloured row
    //    with evidence + confidence + optional Fix button.
    //  * Stateful (NeedsSimulation / StaleEvidence) — neutral grey
    //    "needs current simulation" / "re-run sim" rows.
    //  * Hints (state Current, severity ≤ Hint) — collapsed by default.
    //
    // Load-gate diagnostics (chipload / power / deflection / drill)
    // surface in the same ribbon now that we have `load_verdict`.
    //
    // DC5: the tiers are BUILT here, because the tab strip's badges read
    // them. The rows DRAW in the panel footer, below the tab content.
    let tool_for_diags = tool_configs
        .iter()
        .find(|(id, _)| Some(*id) == entry.tool_id)
        .map(|(_, tc)| tc);
    let diagnostics = operations::collect_diagnostics(
        entry,
        tool_for_diags,
        stale_default_defects,
        toolpath_heights,
        preconditions,
        model_refs,
        suggest_warnings,
        load_verdict,
    );
    // DC5 (Pattern C): the auto-regen workflow notice used to be synthesised
    // here as a `Category::State` diagnostic and printed as the sentence
    // "This operation requires manual generation. Press G or click Generate."
    // The sentence only named the button beside it. The state now reads as
    // one word on the Generate row, with the same fact on hover.
    let mut actionable = Vec::new();
    let mut stateful = Vec::new();
    let mut hints = Vec::new();
    for d in &diagnostics {
        use rs_cam_core::diagnostics::DiagnosticState;
        match d.state {
            DiagnosticState::Current if d.severity.is_actionable() => actionable.push(d),
            DiagnosticState::Current => {
                // Workflow / State category notices render as stateful
                // (neutral) — they don't deserve the yellow tier even
                // though they live in `Current` state.
                if matches!(d.category, rs_cam_core::diagnostics::Category::State) {
                    stateful.push(d);
                } else {
                    hints.push(d);
                }
            }
            DiagnosticState::NeedsSimulation | DiagnosticState::StaleEvidence => stateful.push(d),
            DiagnosticState::NotApplicable => {}
        }
    }
    // H2: the shoe gap finding (`project.shoe_gap_too_low`) is a project
    // verdict about this toolpath. The footer is its one owner here; the
    // Heights tab, which holds the shoe row, carries the badge.
    let shoe_finding = inputs.shoe.as_ref().and_then(|shoe| shoe.finding.as_ref());
    actionable.extend(shoe_finding);

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
    let mut tab_badges = compute_tab_badges(
        entry,
        &diagnostics,
        toolpath_heights,
        shoe_finding.is_some(),
    );
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

    // ── Panel footer ────────────────────────────────────────────────
    //
    // DC5 (Pattern A / Pattern C): everything below belongs to the panel, not
    // to a tab, so it renders BELOW the tab strip's content. The reach
    // readings and the diagnostic ribbon used to draw ABOVE the strip, which
    // put a paragraph of text between the Generate button and the tabs and
    // left the reader unable to tell what contained what.
    ui.separator();

    // G-E (2), §7.2 rule 4: the findings are a bounded list. At most
    // `FOOTER_FINDINGS_CAP` rows, then the `NoticeStack` remainder line,
    // which shows the rest. Every critical and blocking row shows whatever
    // the cap (the `NoticeStack` safety rule).
    let (merged_gate_rows, stateful_rest) = merge_stateful_gate_rows(&stateful);
    let rows: Vec<(&rs_cam_core::diagnostics::Diagnostic, RowTier)> = actionable
        .iter()
        .map(|d| (*d, RowTier::Actionable))
        .chain(merged_gate_rows.iter().map(|d| (d, RowTier::Stateful)))
        .chain(stateful_rest.iter().map(|d| (*d, RowTier::Stateful)))
        .collect();
    let expand_id = ui.id().with("tp_findings_all").with(entry.id.raw());
    let expanded = ui.data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
    let shown = bounded_findings(
        &rows.iter().map(|(d, _)| d.severity).collect::<Vec<_>>(),
        expanded,
    );
    for &index in &shown {
        if let Some((d, tier)) = rows.get(index) {
            render_diagnostic_row(ui, d, *tier, entry, stale_default_defects);
        }
    }
    if shown.len() < rows.len()
        && crate::ui::components::notice::overflow_row(ui, shown.len(), rows.len())
    {
        ui.data_mut(|d| d.insert_temp(expand_id, true));
    }

    // DC5 (Pattern C): the hint list is a COUNT at the bottom of the panel,
    // not a block of text at the top. The operator's words were "hints is a
    // lot of text and its at the top, why?". Every hint and its severity
    // order survive; the row's placement and its resting weight are what
    // changed. `id_salt` pins the collapse state, which the header text used
    // to derive and would otherwise reset on every count change.
    if !hints.is_empty() {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} hints", hints.len()))
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
        )
        .id_salt("tp_hints")
        .default_open(false)
        .show(ui, |ui| {
            for d in &hints {
                render_diagnostic_row(ui, d, RowTier::Hint, entry, stale_default_defects);
            }
        });
    }
    // Live look round 3: the caller records the tab on screen
    // (`GuiState::active_toolpath_tab`), which the height planes read.
    active_tab
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

/// What the Generate row of the header shows (FR-2 F22).
pub(super) struct HeaderRow<'a> {
    pub can_generate: bool,
    pub freshness: &'a crate::state::freshness::FreshnessState,
    pub manual_generation: bool,
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
        matches!(
            self.freshness,
            crate::state::freshness::FreshnessState::Current
        ) && self.blocking.is_some()
    }

    /// The move count text: "old: N moves" on an edited result.
    fn moves_text(&self) -> Option<String> {
        let moves = self.moves?;
        Some(
            if matches!(
                self.freshness,
                crate::state::freshness::FreshnessState::EditedSince
            ) {
                format!("old: {moves} moves")
            } else {
                format!("{moves} moves")
            },
        )
    }
}

/// The Generate row of the header (FR-2 F22). The row never grows the
/// panel. True when Generate was clicked.
///
/// - A blocked result (F20, the safety signal): "Done · BLOCKED" takes its
///   full width FIRST and never truncates. Then the feed stage and the
///   move count paint only when they fit (the move count drops first);
///   the hover of the state word keeps them.
/// - Every other state: the feed stage and the move count take their
///   width first, at the right edge; the state word truncates in the
///   rest, with its text on hover. Before F22 "Done · edited since —
///   regenerate" and "old: N moves" ran past the panel, and every row
///   below took the wider width.
pub(super) fn draw_generate_row(ui: &mut egui::Ui, row: &HeaderRow<'_>) -> bool {
    ui.horizontal(|ui| {
        let clicked = ui
            .add_enabled(row.can_generate, egui::Button::new("Generate"))
            .clicked();
        if row.is_blocked() {
            draw_header_state(
                ui,
                row.freshness,
                row.manual_generation,
                row.blocking,
                &figures_hover(row),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                draw_header_figures(ui, row, true);
            });
        } else {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                draw_header_figures(ui, row, false);
                let rest = ui.available_width().max(0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(rest, crate::ui::tokens::ROW_DENSE),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(rest);
                        draw_header_state(
                            ui,
                            row.freshness,
                            row.manual_generation,
                            row.blocking,
                            "",
                        );
                    },
                );
            });
        }
        clicked
    })
    .inner
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
    use crate::ui::tokens::GLYPH_DANGER;
    use rs_cam_core::session::RestBoundaryRefusal;
    match refusal {
        RestBoundaryRefusal::NotGenerated {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("{GLYPH_DANGER} Generate '{source_name}' first"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::NoRegions {
            source_toolpath_id,
            source_name,
        } => Some(RefusalFace {
            text: format!("{GLYPH_DANGER} '{source_name}' left no regions"),
            generate: Some((*source_toolpath_id, source_name.clone())),
        }),
        RestBoundaryRefusal::OwnRegions => Some(RefusalFace {
            text: "Refused \u{00B7} source is this op".to_owned(),
            generate: None,
        }),
        RestBoundaryRefusal::SourceMissing { .. } => None,
    }
}

/// The glyph of the action that generates a refusal's source.
pub(super) const GENERATE_SOURCE_GLYPH: &str = "\u{27F3}";

/// Draw one refusal face: the face truncates in the width the action
/// leaves; `detail` (the full refusal) is its hover.
fn draw_refusal_face(
    ui: &mut egui::Ui,
    face: &RefusalFace,
    detail: &str,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((source, name)) = &face.generate
                && ui
                    .small_button(GENERATE_SOURCE_GLYPH)
                    .on_hover_text(format!("Generate '{name}'"))
                    .clicked()
            {
                events.push(AppEvent::GenerateToolpath(*source));
            }
            let rest = ui.available_width().max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(rest, crate::ui::tokens::ROW_DENSE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_max_width(rest);
                    ui.add(
                        OneLine::new(face.text.as_str())
                            .color(crate::ui::tokens::DANGER)
                            .detail(detail),
                    );
                },
            );
        });
    });
}

/// G-E (2): the footer shows at most this many finding rows before the
/// remainder line.
pub(super) const FOOTER_FINDINGS_CAP: usize = 3;

/// G-E (2): the indices of the footer finding rows to draw, in their
/// order. All of them when `expanded`. Else the first
/// [`FOOTER_FINDINGS_CAP`], and every critical or blocking row however far
/// down it is (the `NoticeStack` safety rule: a cap never hides one).
pub(super) fn bounded_findings(
    severities: &[rs_cam_core::diagnostics::Severity],
    expanded: bool,
) -> Vec<usize> {
    use rs_cam_core::diagnostics::Severity;
    severities
        .iter()
        .enumerate()
        .filter(|(index, severity)| {
            expanded || *index < FOOTER_FINDINGS_CAP || severity.rank() >= Severity::Critical.rank()
        })
        .map(|(index, _)| index)
        .collect()
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

/// The figures of the header row, right to left: the feed stage at the
/// right edge, the move count to its left. With `fit`, a figure that does
/// not fit the width that is left is not painted (the move count drops
/// first, then the feed stage).
fn draw_header_figures(ui: &mut egui::Ui, row: &HeaderRow<'_>, fit: bool) {
    // W2.5 (D10): one word for the feed stage of the stored result,
    // the one export emits. The hover is the sentence the G-code
    // header prints. Right to left: the stage paints first.
    let mut stage_shown = true;
    if let Some(stage) = row.feed_stage {
        stage_shown = !fit || small_width(ui, stage.badge()) <= ui.available_width();
        if stage_shown {
            let colour = if stage.is_superseded() {
                crate::ui::tokens::CAUTION
            } else {
                crate::ui::tokens::TEXT_FAINT
            };
            ui.label(egui::RichText::new(stage.badge()).small().color(colour))
                .on_hover_text(stage.statement());
        }
    }
    // DC1 ruling R27 moved the card's figures off the toolpath card, so
    // this is the product's per-operation move count. It must say whose
    // generation it counts.
    //
    // F2.2 made the card prefix an edited operation's figures with
    // `old: `, for a reason that applies here unchanged: a confident
    // number outweighs a quiet state word beside it. R27 deleted the
    // surface that carried the mark, so the mark moves with the figure.
    // The word is the card's word, so the two surfaces cannot fork.
    if let Some(text) = row.moves_text() {
        if fit && (!stage_shown || small_width(ui, &text) > ui.available_width()) {
            return;
        }
        let is_stale = text.starts_with("old: ");
        ui.label(
            egui::RichText::new(text)
                .small()
                .color(crate::ui::tokens::TEXT_FAINT),
        )
        .on_hover_text({
            let count = if is_stale {
                "This count is from the previous generation. Regenerate the \
                 operation to measure the settings shown here."
            } else {
                "The move count of the current generation."
            };
            match row.caps {
                Some(caps) => format!("{count}\n{}", caps.line()),
                None => count.to_owned(),
            }
        });
    }
}

/// The state word of the header row (FR-2 F22): one line that truncates,
/// with its text on hover.
pub(super) fn draw_header_state(
    ui: &mut egui::Ui,
    freshness: &crate::state::freshness::FreshnessState,
    manual_generation: bool,
    blocking: Option<&rs_cam_core::diagnostics::Diagnostic>,
    figures: &str,
) {
    use crate::state::freshness::FreshnessState;
    // F2.2 — the same one state the card chip and the workspace counts
    // read (R0.1 §4.4), in place of the raw `ComputeStatus`. The two
    // agree on six of seven; the seventh is why this changed.
    match freshness {
        FreshnessState::NoResult => {
            // One word, then a hover. A printed sentence that names the
            // button next to it tells the reader nothing the button does
            // not already say.
            if manual_generation {
                state_word(
                    ui,
                    egui::RichText::new("Manual").color(crate::ui::tokens::TEXT_MUTED),
                )
                .on_hover_text(
                    "This operation does not regenerate on its own. Press G, or click \
                     Generate, to compute it.",
                );
            } else {
                state_word(ui, egui::RichText::new("Ready"))
                    .on_hover_text("Ready: this operation has no result yet.");
            }
        }
        FreshnessState::Regenerating => {
            state_word(ui, egui::RichText::new("Computing..."))
                .on_hover_text("Computing: the operation is in generation.");
        }
        // F20 (SAFETY visibility): a current result with a blocking
        // finding is not "Done": export refuses it. The header names
        // the state, and its hover holds the finding with its fix. The
        // footer row stays the one owner of the finding row (IC1).
        FreshnessState::Current if let Some(finding) = blocking => {
            // The safety signal never truncates (FR-2 F22): the row gives it
            // its full width first. The hover keeps the figures that the
            // row drops.
            let figures = if figures.is_empty() {
                String::new()
            } else {
                format!("\n{figures}")
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(crate::ui::toolpath_panel::BLOCKED_HEADER)
                        .color(crate::ui::tokens::DANGER),
                )
                .extend(),
            )
            .on_hover_text(format!(
                "A blocking finding stands, so this toolpath does not go to G-code. {}{figures}",
                finding.message
            ));
        }
        FreshnessState::Current => {
            state_word(ui, egui::RichText::new("Done").color(crate::ui::tokens::OK))
                .on_hover_text("Done: the result answers the settings shown here.");
        }
        // NOT green, and it says both halves: the generation finished,
        // AND what it produced no longer answers the configuration on
        // this screen. "Done" alone was the whole defect — the header
        // sat directly above the fields the operator had just changed
        // and reported success at them.
        FreshnessState::EditedSince => {
            state_word(
                ui,
                egui::RichText::new("Done \u{00B7} edited since \u{2014} regenerate")
                    .color(crate::ui::tokens::CAUTION),
            )
            .on_hover_text(
                "This operation generated successfully, then one of its inputs changed. \
                 The path in the viewport and the figures on this panel are from that \
                 earlier generation, not from the settings shown here.",
            );
        }
        FreshnessState::WaitingOnUpstream(block) => {
            // Amber, not red: this operation is fine, it is waiting its
            // turn. The hover names the operation it is waiting for.
            // G-E: one row; it truncates, it does not wrap.
            state_word(
                ui,
                egui::RichText::new("Waiting on upstream stock").color(crate::ui::tokens::CAUTION),
            )
            .on_hover_text(&block.message);
        }
        FreshnessState::Disabled => {
            state_word(
                ui,
                egui::RichText::new("Disabled").color(crate::ui::tokens::TEXT_MUTED),
            )
            .on_hover_text("Disabled: this operation does not go to G-code.");
        }
        // G-E (§7.7): the last run refused. One word in the row, the full
        // text on its hover. The text printed in full here took 3 to 15
        // rows and moved the tab strip down by up to 257 points.
        FreshnessState::Error(e) => {
            state_word(
                ui,
                egui::RichText::new(REFUSED_STATE).color(crate::ui::tokens::DANGER),
            )
            .on_hover_text(e);
        }
    }
}

/// One state word of the header row: a label that truncates in the width
/// that the figures leave (FR-2 F22). The caller adds the hover.
fn state_word(ui: &mut egui::Ui, text: egui::RichText) -> egui::Response {
    ui.add(
        egui::Label::new(text)
            .truncate()
            .show_tooltip_when_elided(false),
    )
}
