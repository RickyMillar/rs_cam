//! `draw_toolpath_panel` — the tab host itself.
//!
//! One function. It draws the shared header, the tab strip, the diagnostic
//! ribbon and whichever tab is active, and it delegates every tab body to a
//! sibling child of `ui::properties`.

use super::feeds_speeds::{calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer};
use super::linking_dressup::{draw_dressup_params, draw_linking_params, dressup_active_count};
use super::operations::{draw_height_diagram, draw_heights_params, validate_toolpath};
use super::pills::PillSuggestions;
use super::tab_badges::{
    RowTier, compute_tab_badges, draw_geometry_wiring, draw_toolpath_tabs,
    merge_stateful_gate_rows, render_diagnostic_row, rest_region_pathology_caption,
    wrapped_small_label,
};
use super::{
    ReachPanelSummary, ToolpathPanelInputs, ToolpathPanelSnapshot, ToolpathTab,
    boundary_summary_line, operations, rest_grid_footprint_area,
};
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig, HeightContext,
    OperationConfig, UiProcessRole,
};
use crate::ui::AppEvent;

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
    // P5 — `show_reach_map` is the viewport's reach-map checkbox, threaded in
    // as a `&mut bool` rather than reached through `AppState`, because this
    // panel takes no state reference; the caller copies the flag out and
    // writes it back.
    show_reach_map: &mut bool,
    events: &mut Vec<AppEvent>,
) {
    // Disjoint borrows of the snapshot's fields. The body edits the entry
    // and only reads the other three, so the four live side by side.
    let entry = &mut snapshot.entry;
    let preconditions = &snapshot.preconditions;
    let model_refs = &snapshot.model_refs;
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
    let height_ctx = inputs.height_ctx.as_ref();
    let stale_default_defects = inputs.stale_default_defects.as_slice();
    let load_verdict = inputs.load_verdict.as_ref();
    let tab_override = inputs.tab_override;
    let reach = &inputs.reach;
    let freshness = &inputs.freshness;

    // The inspector is a fixed-width side panel. Keep children — especially
    // long Feeds annotations — from enlarging its requested width.
    ui.set_max_width(ui.available_width());

    // ── Shared header (always visible above tabs) ───────────────────

    // Name — single editable home for the toolpath name; the duplicate
    // read-only heading was dropped (density pass 2026-06-11).
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut entry.name);
    });

    ui.separator();

    // Generate button + status (always visible) — promoted to the top
    // (SHE-004) so "is it generated? any warnings?" reads before the geometry
    // combos the user rarely revisits after setup.
    ui.add_space(4.0);
    let validation_errors = validate_toolpath(entry, validation);
    let can_generate = !tools.is_empty() && validation_errors.is_empty();
    // DC5 (Pattern C): the state the "requires manual generation" sentence
    // used to carry. Read before the row so the closure holds no extra
    // borrow of `entry`.
    let manual_generation = !entry.auto_regen && matches!(entry.status, ComputeStatus::Pending);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            events.push(AppEvent::GenerateToolpath(entry.id));
        }
        // F2.2 — the same one state the card chip and the workspace counts
        // read (R0.1 §4.4), in place of the raw `ComputeStatus`. The two
        // agree on six of seven; the seventh is why this changed.
        use crate::state::freshness::FreshnessState;
        match freshness {
            FreshnessState::NoResult => {
                // One word, then a hover. A printed sentence that names the
                // button next to it tells the reader nothing the button does
                // not already say.
                if manual_generation {
                    ui.label(egui::RichText::new("Manual").color(crate::ui::tokens::TEXT_MUTED))
                        .on_hover_text(
                            "This operation does not regenerate on its own. Press G, or click \
                         Generate, to compute it.",
                        );
                } else {
                    ui.label("Ready");
                }
            }
            FreshnessState::Regenerating => {
                ui.label("Computing...");
            }
            FreshnessState::Current => {
                ui.label(egui::RichText::new("Done").color(crate::ui::tokens::OK));
            }
            // NOT green, and it says both halves: the generation finished,
            // AND what it produced no longer answers the configuration on
            // this screen. "Done" alone was the whole defect — the header
            // sat directly above the fields the operator had just changed
            // and reported success at them.
            FreshnessState::EditedSince => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Done \u{00B7} edited since \u{2014} regenerate")
                            .color(crate::ui::tokens::CAUTION),
                    )
                    .wrap(),
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
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Waiting on upstream stock")
                            .color(crate::ui::tokens::CAUTION),
                    )
                    .wrap(),
                )
                .on_hover_text(&block.message);
            }
            FreshnessState::Disabled => {
                ui.label(egui::RichText::new("Disabled").color(crate::ui::tokens::TEXT_MUTED));
            }
            FreshnessState::Error(e) => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Error: {e}")).color(crate::ui::tokens::DANGER),
                    )
                    .wrap(),
                )
                .on_hover_text(e);
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
        if let Some(result) = &entry.result {
            let is_stale = matches!(freshness, FreshnessState::EditedSince);
            let moves = result.stats.move_count;
            let text = if is_stale {
                format!("old: {moves} moves")
            } else {
                format!("{moves} moves")
            };
            ui.label(
                egui::RichText::new(text)
                    .small()
                    .color(crate::ui::tokens::TEXT_FAINT),
            )
            .on_hover_text(if is_stale {
                "This count is from the previous generation. Regenerate the \
                 operation to measure the settings shown here."
            } else {
                "The move count of the current generation."
            });
        }
    });
    if !validation_errors.is_empty() {
        for err in &validation_errors {
            // A sentence, so it wraps explicitly rather than inheriting a
            // wrap mode from whatever layout encloses this panel
            // (G-REACHWRAP).
            wrapped_small_label(ui, err.clone(), crate::ui::tokens::CAUTION);
        }
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
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, tc)| tc);
    let diagnostics = operations::collect_diagnostics(
        entry,
        tool_for_diags,
        stale_default_defects,
        height_ctx,
        preconditions,
        model_refs,
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

    // ── Tab bar ─────────────────────────────────────────────────────

    ui.add_space(8.0);
    let tab_id = ui.id().with("tp_tab").with(entry.id.0);
    let mut active_tab: ToolpathTab = tab_override.unwrap_or_else(|| {
        ui.memory(|mem| mem.data.get_temp(tab_id))
            .unwrap_or(ToolpathTab::Geometry)
    });
    let tab_badges = compute_tab_badges(entry, &diagnostics, height_ctx);
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        ToolpathTab::Geometry => draw_geometry_tab(ui, entry, inputs, model_bbox),

        ToolpathTab::FeedsSpeeds => draw_feeds_tab(ui, entry, inputs, model_bbox, events),

        // How moves connect: entry/exit, move optimization, retract
        // strategy (W3.2 — lifted out of the old Dressups tab). This arm is
        // already one call: `draw_linking_params` IS the Linking tab, and it
        // lives in `linking_dressup.rs` beside the Dressup rows it shares.
        ToolpathTab::Linking => draw_linking_params(ui, entry, height_ctx),

        ToolpathTab::Heights => draw_heights_tab(ui, entry, height_ctx),

        ToolpathTab::Dressup => draw_dressup_tab(ui, entry),
    }

    // ── Panel footer ────────────────────────────────────────────────
    //
    // DC5 (Pattern A / Pattern C): everything below belongs to the panel, not
    // to a tab, so it renders BELOW the tab strip's content. The reach
    // readings and the diagnostic ribbon used to draw ABOVE the strip, which
    // put a paragraph of text between the Generate button and the tabs and
    // left the reader unable to tell what contained what.
    ui.separator();

    // Per-tool reach map (P5). Reach-capable operations only — every other
    // operation gets no checkbox at all rather than a disabled one, because
    // the question does not apply to it (`OperationType::supports_reach_map`).
    if entry.operation.op_type().supports_reach_map() {
        ui.horizontal(|ui| {
            ui.checkbox(show_reach_map, "Show reach map").on_hover_text(
                "Colour the model by what THIS toolpath's cutter can form: green where \
                     the cutter reaches the surface within the operation's tolerance, red \
                     where a gap is left. The measure is top-down, so undersides, overhangs \
                     and vertical walls are NOT MEASURED and keep the plain model colour.",
            );
            // Only the two SHORT status words share the checkbox's row.
            // A horizontal layout's wrap mode is `Extend` (egui's
            // `Ui::wrap_mode`: a layout that is neither vertical nor
            // main-wrapped falls to the `Extend` arm), so a bare `ui.label`
            // here runs past the panel and clips. A `Label::wrap()` WOULD
            // still wrap — the label's own mode overrides the Ui's
            // (`label.rs:191`, `self.wrap_mode.unwrap_or_else(|| ui.wrap_mode())`) —
            // but at `ui.available_width()`, which on this row is only what
            // the checkbox leaves: a narrow column, not the panel width.
            // These two readings are under twenty characters and carry no
            // caveat, so either fate is fine for them; the ones that do
            // carry a caveat are below, where they get the full width
            // (G-REACHWRAP).
            match reach {
                ReachPanelSummary::Computing => {
                    ui.label(
                        egui::RichText::new("reach: computing…")
                            .small()
                            .color(crate::ui::tokens::TEXT_MUTED),
                    );
                }
                // A zero percentage over an empty population is
                // indistinguishable from a clean part, so it is never shown
                // as one.
                ReachPanelSummary::NotMeasured => {
                    ui.label(
                        egui::RichText::new("reach: not measured")
                            .small()
                            .color(crate::ui::tokens::CAUTION),
                    );
                }
                ReachPanelSummary::Measured { .. }
                | ReachPanelSummary::Failed(_)
                | ReachPanelSummary::Idle => {}
            }
        });

        // G-REACHWRAP (UX-R09-001, 2026-09-10): the readings used to sit on
        // the checkbox's row as bare `ui.label`s, which under that row's
        // `Extend` mode ran past the panel and clipped — the review's
        // 1400×900 capture shows the summary cut off mid-number with
        // `grid_note` never drawn at all
        // (`results/W02/evidence/01_finish_defaults.png`). `grid_note` and
        // `over_statement_note` are the MISSING-GUARANTEE sentences — the
        // cell, the floor and the bar, and the statement that the grid
        // over-states — so a clipped one reads as an unqualified result.
        // They now get the panel's full width, one wrapped line each.
        match reach {
            ReachPanelSummary::Measured {
                unreachable_pct,
                max_gap_mm,
                grid_note,
                area_basis_note,
                over_statement_note,
                tolerance_below_floor,
            } => {
                // The base comes from `ReachMap::area_basis_note`, not
                // from a sentence written here. This line said "of
                // MEASURED area" while the panel legend said "of 3D
                // surface area, rim-eroded 3.0 mm" - two surfaces, one
                // quantity, two descriptions, and that is the drift the
                // shared notes exist to prevent.
                wrapped_small_label(
                    ui,
                    format!(
                        "unreachable {unreachable_pct:.1} % {area_basis_note} · \
                         max gap {max_gap_mm:.2} mm"
                    ),
                    crate::ui::tokens::TEXT_STRONG,
                );
                wrapped_small_label(
                    ui,
                    grid_note.clone(),
                    if *tolerance_below_floor {
                        crate::ui::tokens::CAUTION
                    } else {
                        crate::ui::tokens::TEXT_MUTED
                    },
                );
                if *tolerance_below_floor {
                    // The shared sentence, quoted - not a fourth
                    // hand-written paraphrase of the same bias.
                    wrapped_small_label(
                        ui,
                        over_statement_note.clone(),
                        crate::ui::tokens::CAUTION,
                    );
                }
            }
            ReachPanelSummary::Failed(message) => {
                wrapped_small_label(ui, format!("reach: {message}"), crate::ui::tokens::CAUTION);
            }
            ReachPanelSummary::Computing
            | ReachPanelSummary::NotMeasured
            | ReachPanelSummary::Idle => {}
        }
    }

    for d in &actionable {
        render_diagnostic_row(ui, d, RowTier::Actionable, entry, stale_default_defects);
    }
    let (merged_gate_rows, stateful_rest) = merge_stateful_gate_rows(&stateful);
    for d in &merged_gate_rows {
        render_diagnostic_row(ui, d, RowTier::Stateful, entry, stale_default_defects);
    }
    for d in &stateful_rest {
        render_diagnostic_row(ui, d, RowTier::Stateful, entry, stale_default_defects);
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
}

// ── The tab bodies ──────────────────────────────────────────────────────
//
// UI-01: `draw_toolpath_panel` ran 1 255 lines because every tab drew
// inline. Each tab is now its own function, so the `match` above is the
// dispatch it always claimed to be. The moves are pure: no widget, no draw
// order and no egui id changed. Each function re-binds the `inputs` fields
// it needs under the names its body already used.

/// The Geometry tab: what to cut.
fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
) {
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let tool_configs = inputs.tool_configs.as_slice();
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();
    let rest_region_consumers = inputs.rest_region_consumers.as_slice();
    let material = &inputs.material;
    let machine = &inputs.machine;
    let workholding = inputs.workholding;
    let spindle_strategy = inputs.spindle_strategy;
    let model_has_enriched = inputs.model_has_enriched;
    let model_is_step_missing_brep = inputs.model_is_step_missing_brep;
    let height_ctx = inputs.height_ctx.as_ref();
    let stale_default_defects = inputs.stale_default_defects.as_slice();
    let drill_layers = inputs.drill_layers.as_slice();
    let drill_targets = inputs.drill_targets.as_slice();

    ui.add_space(4.0);

    // DC5: the geometry WIRING opens this tab — Tool, Input model,
    // Faces and the stock source. It used to sit behind a `Geometry`
    // disclosure above the tab strip, which gave one name two homes
    // at one level. One name, one place (Rule A).
    draw_geometry_wiring(
        ui,
        entry,
        tools,
        models,
        model_has_enriched,
        model_is_step_missing_brep,
    );
    ui.separator();
    ui.add_space(4.0);

    // Validator-driven Fix banner (PR-2C Phase 1). One row per
    // detected stale-default rule for this TP, with a one-click
    // Fix that mutates the operation directly. Defects are
    // recomputed by the caller each frame, so the banner
    // disappears as soon as the fix takes effect.
    for defect in stale_default_defects {
        egui::Frame::group(ui.style())
            .fill(crate::ui::tokens::TINT_CAUTION)
            .stroke(egui::Stroke::new(1.0_f32, crate::ui::tokens::CAUTION))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("\u{26A0}")
                            .color(crate::ui::tokens::CAUTION)
                            .strong(),
                    );
                    ui.label(egui::RichText::new(&defect.title).strong());
                });
                ui.label(
                    egui::RichText::new(&defect.detail)
                        .small()
                        .color(crate::ui::tokens::TEXT_STRONG),
                );
                if ui
                    .small_button(format!("\u{2713} Fix (set to {:.3})", defect.new_value))
                    .on_hover_text(
                        "Apply the validator's auto-fix to this toolpath. \
                         Mark stale and regenerate to apply.",
                    )
                    .clicked()
                {
                    rs_cam_core::compute::validate::apply_stale_default_to_op(
                        &mut entry.operation,
                        &mut entry.feeds_provenance,
                        defect,
                    );
                    entry.stale_since = Some(std::time::Instant::now());
                }
            });
        ui.add_space(2.0);
    }

    // Compute + cache the LUT feeds result so the Geometry-row ⚡
    // pills (stepover / depth-per-pass) can render. Feed advice and
    // the single validated Apply all route live on the Feeds tab.
    let pill_tool_cfg = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, t)| t);
    if let Some(tool_cfg) = pill_tool_cfg {
        entry.feeds_result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            &entry.operation,
            tool_cfg,
            material,
            machine,
            workholding,
            rs_cam_core::feeds::embedded_vendor_lut(),
            spindle_strategy,
        )
        .ok();
    }
    // G-PILLCLAMP (UX-R03-014): one dry run of the apply funnel per
    // frame, so every ⚡ pill below offers and writes the value
    // the canonical Apply all route would write for its field — not
    // the raw calculator number (4.2 mm vs 1.2 mm of DOC on the demo
    // pocket).
    let pills = match (entry.feeds_result.as_ref(), pill_tool_cfg) {
        (Some(result), Some(tool_cfg)) => Some(PillSuggestions::new(
            &entry.operation,
            result,
            tool_cfg,
            machine,
            material,
            model_bbox,
        )),
        _ => None,
    };

    // Operation description from spec (consistent across all operations)
    let spec = entry.operation.op_type().spec();
    ui.label(
        egui::RichText::new(spec.description)
            .italics()
            .color(crate::ui::tokens::TEXT_MUTED),
    );
    ui.add_space(2.0);
    // PR-2D Phase 2 — pass the per-field pill suggestions to every
    // per-op draw so each numeric field can render an inline ⚡
    // Suggest pill. Built above from the cached `entry.feeds_result`
    // and the funnel dry run, so this is just a borrow.
    let feeds_for_pills = pills.as_ref();
    // A/M6: read before the mutable borrow of `entry.operation`
    // below. Both are `Copy`, so nothing is held across it.
    let resolved_claims_reference = entry.result.as_ref().and_then(|r| r.stats.claims_reference);
    // G-DEPTHSTOCK (UX-R03-007): the same rule the header ribbon
    // prints, read once here and handed to the depth field's row.
    // `Copy`, so nothing is held across the mutable borrow below.
    let depth_caution = height_ctx
        .and_then(|hctx| operations::depth_beyond_stock(&entry.operation, &entry.heights, hctx));
    let depth_caution = depth_caution.as_ref();
    // G-THROUGHCUT (UX-R03-006): Profile only. Read here, before the
    // mutable borrow, for the same reason as the caution above.
    let through_cut = match (&entry.operation, height_ctx) {
        (OperationConfig::Profile(cfg), Some(hctx)) => {
            operations::profile_through_cut(cfg, &entry.heights, hctx)
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
    //
    // `entry.operation` and `entry.stock_source` are disjoint fields, so both
    // are borrowed at once; the Pencil editor is the one that writes the
    // stock source.
    // The Adaptive3d "Optimal load" knob needs the active tool's radius to
    // map engagement to stepover.
    let tool_radius = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map_or(0.0, |(_, t)| t.diameter / 2.0);
    let mut stock_source_changed = false;
    let mut draw_ctx = operations::registry::OpDrawCtx {
        tools,
        models,
        drill_layers,
        drill_targets,
        pills: feeds_for_pills,
        depth_caution,
        through_cut,
        tool_radius,
        resolved_claims_reference,
        stock_source: &mut entry.stock_source,
        stock_source_changed: &mut stock_source_changed,
    };
    operations::registry::draw_editor_and_diagram(ui, &mut entry.operation, &mut draw_ctx);
    if stock_source_changed {
        entry.stale_since = Some(std::time::Instant::now());
    }
    // G-PILLCLAMP: a pill wrote its field this frame — stamp the
    // recommendation's provenance (the value is the funnel's, not a hand
    // edit) and remember it for the flush.
    if let Some((field, preview)) = pills.as_ref().and_then(|p| p.take_clicked()) {
        entry
            .feeds_provenance
            .set(field, preview.provenance.clone());
        entry.pill_stamped_fields.push(field);
    }

    // ── Machining Boundary ─────────────────────────────────────
    // W3.2: the boundary defines *what region to cut*, so it lives
    // under Geometry (was on the old Dressups tab).
    crate::ui::components::SectionHeader::new("Machining Boundary").show(ui);
    ui.checkbox(&mut entry.boundary.enabled, "Enable boundary")
        .on_hover_text(
            "Restrict toolpath to a boundary polygon. \
             Moves outside the boundary are converted to rapids at safe Z.",
        );
    if entry.boundary.enabled {
        // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the
        // one generation uses (`session/compute.rs` clones
        // `tc.boundary` unconditionally). Name it, then show the
        // controls that drive it. The controller assigns the model
        // silhouette on add for 3D ops on a mesh; that provenance is
        // not stored, so the line never claims "(auto)".
        ui.label(
            egui::RichText::new(boundary_summary_line(&entry.boundary, false))
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
        // Source selector
        ui.horizontal(|ui| {
            ui.label("Source:");
            egui::ComboBox::from_id_salt("boundary_source")
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
                        entry.boundary.source = BoundarySource::ModelSilhouette;
                    }
                    if ui
                        .selectable_label(
                            matches!(entry.boundary.source, BoundarySource::FaceSelection),
                            "Face Selection",
                        )
                        .on_hover_text("Boundary derived from selected STEP faces")
                        .clicked()
                    {
                        entry.boundary.source = BoundarySource::FaceSelection;
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
                            "Boundary = rest regions computed by another \
                             toolpath's rest analysis. Pick the source \
                             toolpath below."
                        } else {
                            "No other toolpaths in this project yet — add \
                             one, switch on its Rest Analysis and \
                             generate it to use as the source."
                        })
                        .clicked()
                    {
                        let default_source = boundary_source_candidates
                            .first()
                            .map(|(candidate_id, _, _, _, _)| *candidate_id)
                            .unwrap_or(entry.id);
                        entry.boundary.source = BoundarySource::DerivedRestRegions {
                            source_toolpath_id: default_source,
                        };
                    }
                });
        });

        // Rest-regions source-toolpath picker (P2.2) — only shown
        // when `Source` above is set to `DerivedRestRegions`.
        // Candidates are every other toolpath in the session;
        // ones with a cached result whose rest regions are
        // already non-empty are labelled "(regions ready)" and
        // sorted first, but a not-yet-generated toolpath is
        // still selectable — generation fails hard with a clear
        // message if the source turns out unusable.
        if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
            &mut entry.boundary.source
        {
            ui.horizontal(|ui| {
                ui.label("Rest source:");
                let current_label = boundary_source_candidates
                    .iter()
                    .find(|(candidate_id, _, _, _, _)| candidate_id == source_toolpath_id)
                    .map(|(_, name, ready, _, _)| {
                        if *ready {
                            format!("{name} (regions ready)")
                        } else {
                            name.clone()
                        }
                    })
                    .unwrap_or_else(|| "(toolpath not found)".to_owned());
                let mut sorted = boundary_source_candidates.to_vec();
                sorted.sort_by_key(|(_, _, ready, _, _)| !*ready);
                egui::ComboBox::from_id_salt("boundary_rest_source")
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for (candidate_id, name, ready, _, _) in &sorted {
                            let label = if *ready {
                                format!("{name} (regions ready)")
                            } else {
                                name.clone()
                            };
                            let selected = *source_toolpath_id == *candidate_id;
                            if ui.selectable_label(selected, label).clicked() {
                                *source_toolpath_id = *candidate_id;
                            }
                        }
                    })
                    .response
                    .on_hover_text(
                        "The toolpath whose rest analysis supplies the \
                         rest regions. ANY operation produces them when \
                         its Rest Analysis is on and the project carries \
                         a mesh; the pencil rest-depth detector and the \
                         Unified Finish claims pipeline attach their own.",
                    );
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
                .find(|(candidate_id, _, _, _, _)| candidate_id == source_toolpath_id);
            let selected_regions = selected.and_then(|(_, _, _, regions, _)| regions.as_ref());
            // `None` twice over: no candidate selected, or the
            // selected one has no rest grid. Both are "not
            // measured", and `classify_rest_regions` stays silent.
            let source_footprint_area = selected.and_then(|(_, _, _, _, area)| *area);
            if let Some(regions) = selected_regions
                && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
                    regions,
                    source_footprint_area,
                )
            {
                ui.label(
                    egui::RichText::new(rest_region_pathology_caption(pathology))
                        .small()
                        .color(crate::ui::tokens::CAUTION),
                );
            }
        }

        // Containment mode
        ui.horizontal(|ui| {
            ui.label("Containment:");
            egui::ComboBox::from_id_salt("boundary_contain")
                .selected_text(match entry.boundary.containment {
                    BoundaryContainment::Center => "Center",
                    BoundaryContainment::Inside => "Inside",
                    BoundaryContainment::Outside => "Outside",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut entry.boundary.containment,
                        BoundaryContainment::Center,
                        "Center",
                    )
                    .on_hover_text("Tool center stays inside boundary");
                    ui.selectable_value(
                        &mut entry.boundary.containment,
                        BoundaryContainment::Inside,
                        "Inside",
                    )
                    .on_hover_text("Entire tool stays inside boundary (shrinks by tool radius)");
                    ui.selectable_value(
                        &mut entry.boundary.containment,
                        BoundaryContainment::Outside,
                        "Outside",
                    )
                    .on_hover_text("Tool edge can extend outside boundary");
                });
        });

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

    // ── Rest Analysis (P2.5 → P2 pencil-panel consolidation) ────
    // Sibling of Machining Boundary: any toolpath can turn on the
    // rest-depth detector against ITS OWN tool, attaching the
    // heatmap grid + derived regions this op leaves behind — the
    // same analysis that used to be pencil-only. Now demand-driven
    // rather than a manual checkbox on every op: a downstream
    // `Rest Regions` boundary auto-enables it (see the
    // `auto_enable_rest_source` handling below this panel's draw
    // call, and `session::auto_enable_rest_analysis_for_source`
    // for the MCP-path twin), and it's
    // hidden entirely on a `rest_depth` pencil, whose own detector
    // already attaches the same artifacts (invisibly, per
    // `compute::execute::attach_generic_rest_analysis`'s precedence
    // check) — showing a second, redundant control there was the
    // third overlapping rest control this consolidation removes.
    let is_rest_depth_pencil = matches!(
        &entry.operation,
        OperationConfig::Pencil(cfg)
            if cfg.detector == rs_cam_core::finish::pencil::PencilDetector::RestDepth
    );
    ui.add_space(8.0);
    if is_rest_depth_pencil {
        ui.label(
            egui::RichText::new("Rest heatmap & regions: produced by the Rest depth detector.")
                .small()
                .color(crate::ui::tokens::TEXT_MUTED),
        );
    } else {
        crate::ui::components::SectionHeader::new("Rest Analysis").show(ui);
        if rest_region_consumers.is_empty() {
            ui.checkbox(
                &mut entry.rest_analysis.enabled,
                "Compute rest heatmap (material left after this op)",
            )
            .on_hover_text(
                "Run the rest-depth detector against this toolpath's own tool \
                 after generation, attaching a heatmap grid. Also makes this op \
                 selectable as a `Rest Regions` boundary source on other \
                 toolpaths.",
            );
        } else {
            // Demand-driven: a consumer's boundary picker (or the
            // MCP `set_boundary_config` path) already flipped this
            // on — see `auto_enable_rest_analysis_for_source`. Force
            // it here too so a stale project file (or a consumer
            // whose boundary was set before this session started)
            // still reflects reality.
            if !entry.rest_analysis.enabled {
                entry.rest_analysis.enabled = true;
                entry.stale_since = Some(std::time::Instant::now());
            }
            ui.label(format!(
                "Producing rest regions for: {}",
                rest_region_consumers.join(", ")
            ))
            .on_hover_text(
                "Enabled automatically — those toolpaths use this op's rest \
                 regions as their machining boundary.",
            );
        }
        if entry.rest_analysis.enabled {
            ui.horizontal(|ui| {
                ui.label("Reference:");
                let ref_label = entry
                    .rest_analysis
                    .reference_tool_id
                    .and_then(|rid| tools.iter().find(|(id, _, _)| *id == rid))
                    .map(|(_, name, _)| name.as_str())
                    .unwrap_or("Self / machined stock");
                egui::ComboBox::from_id_salt("rest_analysis_reference_tool")
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
                        "The reference the rest gate measures 'deeper than'. \
                         Unset = prefer the actual machined stock from a prior \
                         simulation, else a self-referenced bare-surface probe.",
                    );
            });
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
                    "A cell counts as REST material once the reference floats \
                     more than this above the true surface.",
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
                    "Extra clearance added around detected rest regions beyond \
                     this toolpath's own tool radius.",
                );
            });
        }
    }

    // Sliver-storm / giant-region caption (2026-07-06 incident):
    // classify THIS toolpath's own generated rest regions, whether
    // they came from the checkbox-driven detector above or (for a
    // rest_depth pencil) the detector it always runs. Deliberately
    // outside the `is_rest_depth_pencil` branch so both cases show
    // it.
    //
    // LH-2: the giant-region denominator is THIS result's own rest
    // grid — covered cells × cell², the same grid the regions were
    // extracted from. It used to be the model's XY bounding
    // rectangle, which overstates a non-rectangular part's footprint
    // and made the warning fire late (`MEASUREMENT_DOMAINS.md` X-4).
    // No grid ⇒ 0.0 ⇒ silence, never a guess.
    if let Some(result) = &entry.result
        && let Some(regions) = result.annotated.rest_regions.as_ref()
        && let Some(pathology) = rs_cam_core::surface::rest_field::classify_rest_regions(
            regions,
            rest_grid_footprint_area(result.annotated.rest_grid.as_deref()),
        )
    {
        ui.label(
            egui::RichText::new(rest_region_pathology_caption(pathology))
                .small()
                .color(crate::ui::tokens::CAUTION),
        );
    }
}

/// The Feeds & Speeds tab: how fast.
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
    let workholding = inputs.workholding;
    let spindle_strategy = inputs.spindle_strategy;
    let project_default_rpm = inputs.project_default_rpm;
    let load_verdict = inputs.load_verdict.as_ref();

    let tool_info = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
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
    draw_speed_controls(ui, entry, project_default_rpm);
    ui.add_space(4.0);
    if let Some(tool_cfg) = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, t)| t)
    {
        calculate_and_apply_feeds(
            ui,
            entry,
            tool_cfg,
            material,
            machine,
            workholding,
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
fn draw_heights_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    height_ctx: Option<&HeightContext>,
) {
    let fallback_ctx = HeightContext::simple(10.0, 5.0);
    let ctx = height_ctx.unwrap_or(&fallback_ctx);
    // F1.19 / G-BOTTOMPIN: the Bottom row is annotated per operation,
    // so the panel needs the operation. Read before the mutable
    // borrow of `entry.heights`; `OperationType` is `Copy`.
    // F-6 / R33: the diagram takes the same operation, so its Bottom
    // line agrees with the sentence the panel prints beside the row.
    let op_type = entry.operation.op_type();
    draw_heights_params(ui, &mut entry.heights, ctx, op_type);
    ui.add_space(6.0);
    draw_height_diagram(ui, &mut entry.heights, ctx, op_type);
}

/// The Dressup tab: edge work and path quality.
fn draw_dressup_tab(ui: &mut egui::Ui, entry: &mut crate::state::toolpath::ToolpathEntry) {
    // Active dressup summary + reset button
    let (active, total) = dressup_active_count(&entry.dressups);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{active}/{total} dressups active"))
                .small()
                .color(if active > 0 {
                    crate::ui::tokens::OK
                } else {
                    crate::ui::tokens::TEXT_MUTED
                }),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let role = entry.operation.op_type().spec().ui_process_role;
            if ui
                .small_button("Reset to recommended")
                .on_hover_text(format!(
                    "Apply recommended dressups for {} operations",
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
        });
    });

    // Edge work / path quality (W3.2 — Entry/Exit, Optimization
    // and Retract moved to the Linking tab; Machining Boundary
    // moved to Geometry).
    draw_dressup_params(ui, &mut entry.dressups);

    // Manual G-code fields (pre_gcode, post_gcode) kept in state
    // for future export wiring — UI removed until export is implemented.
}
