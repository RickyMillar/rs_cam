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
    RowTier, compute_tab_badges, draw_geometry_wiring, draw_start_from, draw_toolpath_tabs,
    merge_stateful_gate_rows, render_diagnostic_row, rest_region_pathology_line,
    wrapped_small_label,
};
use super::{
    ReachPanelSummary, ToolpathPanelInputs, ToolpathPanelSnapshot, ToolpathTab,
    boundary_summary_line, operations, rest_grid_footprint_area,
};
use crate::state::toolpath::{
    BoundaryContainment, BoundarySource, DressupConfig, OperationConfig, StockSource, UiProcessRole,
};
use crate::ui::AppEvent;
use crate::ui::components::{DataRow, DataTable, OneLine, Role, UiExt as _};

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
    // P4.3: the pick mode and the `+ Rule` draft of the Selection
    // sections, threaded in as `show_reach_map` is.
    selection_ui: &mut crate::state::pick::SelectionUi,
    // F033 / UI-09: the last refused heights edit (`state.panels`), threaded
    // in as `selection_ui` is.
    heights_refusal: &mut crate::state::panels::HeightsRefusal,
    events: &mut Vec<AppEvent>,
) {
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
    let reach = &inputs.reach;
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
    let manual_generation =
        !entry.auto_regen && matches!(entry.run, crate::state::runtime::ToolpathRun::Pending);
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
                    .truncate()
                    .show_tooltip_when_elided(false),
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
        if let Some(result) = entry.run.result() {
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
        // W2.5 (D10): one word for the feed stage of the stored result,
        // the one export emits. The hover is the sentence the G-code
        // header prints.
        if let Some(stage) = inputs.feed_stage {
            let colour = if stage.is_superseded() {
                crate::ui::tokens::CAUTION
            } else {
                crate::ui::tokens::TEXT_FAINT
            };
            ui.label(egui::RichText::new(stage.badge()).small().color(colour))
                .on_hover_text(stage.statement());
        }
    });
    if !validation_errors.is_empty() {
        // L-2 (geometry selection P4.3): the Selection section owns the
        // refusal text. A validation error that IS that refusal gets one
        // line here, which says why Generate is off and names the section.
        let section_refusal = inputs
            .selection_report
            .as_ref()
            .and_then(|read| read.as_ref().ok())
            .and_then(|report| report.refusal.as_ref())
            .map(ToString::to_string);
        for err in &validation_errors {
            if section_refusal.as_deref() == Some(err.as_str()) {
                ui.add(
                    OneLine::new("The selection refuses. See Selection below.")
                        .color(crate::ui::tokens::DANGER)
                        .detail(err.clone()),
                );
                continue;
            }
            // A sentence, so it wraps explicitly rather than inheriting a
            // wrap mode from whatever layout encloses this panel
            // (G-REACHWRAP). It blocks Generate, so it is the danger colour
            // and it is the one header line that may wrap (IC1).
            wrapped_small_label(ui, err.clone(), crate::ui::tokens::DANGER);
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
    let tab_id = ui.id().with("tp_tab").with(entry.id.0);
    let mut active_tab: ToolpathTab = tab_override.unwrap_or_else(|| {
        ui.memory(|mem| mem.data.get_temp(tab_id))
            .unwrap_or(ToolpathTab::Geometry)
    });
    let tab_badges = compute_tab_badges(
        entry,
        &diagnostics,
        toolpath_heights,
        shoe_finding.is_some(),
    );
    draw_toolpath_tabs(ui, &mut active_tab, &tab_badges);
    ui.memory_mut(|mem| mem.data.insert_temp(tab_id, active_tab));
    ui.separator();

    // ── Tab content ─────────────────────────────────────────────────

    match active_tab {
        ToolpathTab::Geometry => {
            draw_geometry_tab(ui, entry, inputs, model_bbox, selection_ui, events);
        }

        ToolpathTab::FeedsSpeeds => draw_feeds_tab(ui, entry, inputs, model_bbox, events),

        // How moves connect: entry/exit, move optimization, retract
        // strategy (W3.2 — lifted out of the old Dressups tab). This arm is
        // already one call: `draw_linking_params` IS the Linking tab, and it
        // lives in `linking_dressup.rs` beside the Dressup rows it shares.
        ToolpathTab::Linking => draw_linking_params(ui, entry, tool_for_diags, toolpath_heights),

        ToolpathTab::Heights => {
            draw_heights_tab(ui, entry, inputs, heights_refusal, events);
        }

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
        // `Extend` mode ran past the panel and clipped. They now get the
        // panel's full width.
        //
        // IC1 (2026-10-13): the reading is ONE line. The grid line (the cell,
        // the floor and the bar) is its hover. When the tolerance is below
        // the grid floor, the over-statement is its own caution line, so the
        // missing guarantee is never only on a hover. Round 2: the readings
        // are rows of one `DataTable`.
        let mut table = DataTable::new("toolpath_reach_readings");
        match reach {
            ReachPanelSummary::Measured {
                unreachable_pct,
                max_gap_mm,
                grid_line,
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
                let hover = format!("{area_basis_note}\n{}", grid_line.clone());
                table = table
                    .row(
                        DataRow::value("Unreachable", format!("{unreachable_pct:.1} %"))
                            .hover(hover.clone()),
                    )
                    .row(
                        DataRow::measured("Max gap", format!("{max_gap_mm:.2}"), "mm").hover(hover),
                    );
                if *tolerance_below_floor {
                    // The shared sentence, quoted - not a fourth
                    // hand-written paraphrase of the same bias.
                    table = table.row(DataRow::note(
                        Role::Caution,
                        format!("\u{26A0} {}", over_statement_note.clone()),
                    ));
                }
            }
            ReachPanelSummary::Failed(message) => {
                table = table.row(DataRow::note(Role::Caution, format!("reach: {message}")));
            }
            ReachPanelSummary::Computing
            | ReachPanelSummary::NotMeasured
            | ReachPanelSummary::Idle => {}
        }
        table.show(ui);
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

/// The label of the 3D op face-pick button (geometry selection P4.D).
pub(super) const PICK_FACES_LABEL: &str = "Pick faces";

/// The hover of [`PICK_FACES_LABEL`].
const PICK_FACES_HOVER: &str = "Click flat faces in the viewport to bound this operation. \
    When the boundary source is not Selection, the first face sets the source to Selection \
    with that face. Undo restores the source.";

/// The "Pick faces" row of an op that holds no selection (every 3D op):
/// the button turns Pick on for the boundary selection, for ANY boundary
/// source (geometry selection P4.D, ruling L-8 revised). The first face
/// click then sets the source to `Selection` with that face
/// (`ProjectSession::face_click_edit`). An op that no boundary moves (D3)
/// draws no row.
fn draw_boundary_pick_row(
    ui: &mut egui::Ui,
    entry: &crate::state::toolpath::ToolpathEntry,
    selection_ui: &mut crate::state::pick::SelectionUi,
) {
    use rs_cam_core::session::{BOUNDARY_ITEM_USE, SelectionTarget};
    if entry.operation.selection().is_some()
        || !entry
            .operation
            .capabilities()
            .territory_xy
            .session_clip_applies()
    {
        return;
    }
    let on = selection_ui.pick_is_on(entry.id, SelectionTarget::Boundary);
    let button = if on {
        crate::ui::components::Button::primary(PICK_FACES_LABEL)
    } else {
        crate::ui::components::Button::new(PICK_FACES_LABEL)
    };
    if ui.add(button).on_hover_text(PICK_FACES_HOVER).clicked() {
        selection_ui.set_pick(if on {
            None
        } else {
            Some(crate::state::pick::PickMode::new(
                entry.id,
                SelectionTarget::Boundary,
                BOUNDARY_ITEM_USE,
            ))
        });
    }
}

/// The Geometry tab: what to cut. `properties::tests` renders it (D3).
pub(super) fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    model_bbox: Option<&rs_cam_core::geo::BoundingBox3>,
    selection_ui: &mut crate::state::pick::SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    let tools = inputs.tools.as_slice();
    let models = inputs.models.as_slice();
    let tool_configs = inputs.tool_configs.as_slice();
    let boundary_source_candidates = inputs.boundary_source_candidates.as_slice();
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

    // DC5: the geometry WIRING opens this tab — Tool, Input model, the
    // Selection section (P4.D) and the stock source. It used to sit behind a `Geometry`
    // disclosure above the tab strip, which gave one name two homes
    // at one level. One name, one place (Rule A).
    draw_geometry_wiring(ui, entry, inputs);
    // ── Selection (geometry selection P4.3; P4.D: at the top) ─────
    // P4.D: the section sits directly below the Input combo, so the model
    // and what the op takes from it read as one block.
    // The op selection of every op that holds one. The section sends
    // `EditToolpathSelection` only (L-1); it reads the stored selection
    // and does not write the draft entry.
    if let Some(selection) = entry.operation.selection() {
        let op_default = entry
            .operation
            .default_selection()
            .unwrap_or_else(rs_cam_core::selection::Selection::empty);
        super::selection_section::draw_selection_section(
            ui,
            &super::selection_section::SelectionSectionInputs {
                toolpath_id: entry.id,
                toolpath_model: rs_cam_core::ids::ModelId(entry.model_id.0),
                target: rs_cam_core::session::SelectionTarget::Op,
                selection,
                op_default: &op_default,
                report: inputs.selection_report.as_ref(),
                item_use: entry.operation.capabilities().item_use,
                models,
                sources: &inputs.rule_sources,
                note_items: &inputs.selection_note_items,
                model_kinds: &inputs.toolpath_model_kinds,
                // P6.7: the hole size rows read the tool diameter.
                tool_diameter: tool_configs
                    .iter()
                    .find(|(id, _)| *id == entry.tool_id)
                    .map(|(_, tool)| tool.diameter),
            },
            selection_ui,
            events,
        );
    }

    // P3.7 (O-2, option A): a face pick under a floor that is not "each
    // picked face" keeps its floor; one line under the Selection section
    // offers the choice. The button writes the draft floor, which goes
    // back through the config as every op row does; the section itself
    // writes `EditToolpathSelection` only (L-1).
    draw_each_face_hint(ui, entry, toolpath_heights);
    draw_boundary_pick_row(ui, entry, selection_ui);
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
        .find(|(id, _)| *id == entry.tool_id)
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
    //
    // The Adaptive3d "Optimal load" knob needs the active tool's radius to
    // map engagement to stepover.
    let tool_radius = tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map_or(0.0, |(_, t)| t.diameter / 2.0);
    // P3.7: the floor references and the face floors of an op that holds
    // a floor (the floor row and its re-expression, L-10).
    let floor_inputs = toolpath_heights
        .filter(|_| entry.operation.floor().is_some())
        .map(|answer| super::FloorInputs::new(answer, &entry.heights, op_report(inputs)));
    let mut draw_ctx = operations::registry::OpDrawCtx {
        tools,
        models,
        holes,
        pills: feeds_for_pills,
        depth_caution,
        through_cut,
        tool_radius,
        resolved_claims_reference,
        stock_source: entry.stock_source,
        toolpath_id: entry.id,
        selection_ui: &mut *selection_ui,
        floor: floor_inputs.as_ref(),
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

    // ── Machining Boundary ─────────────────────────────────────
    // W3.2: the boundary defines *what region to cut*, so it lives
    // under Geometry (was on the old Dressups tab).
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
            // UX-R03-009 (G-BOUNDARYINHERIT): the stored source is the
            // one generation uses (`session/compute.rs` clones
            // `tc.boundary` unconditionally). Name it, then show the
            // controls that drive it. The controller assigns the model
            // silhouette on add for 3D ops on a mesh; that provenance is
            // not stored, so the line never claims "(auto)".
            ui.add(OneLine::new(boundary_summary_line(
                &entry.boundary,
                false,
                models,
            )));
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
                            entry.boundary.source = BoundarySource::ModelSilhouette;
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
                            if !matches!(entry.boundary.source, BoundarySource::Selection { .. }) {
                                entry.boundary.source = BoundarySource::Selection {
                                    selection: inputs.boundary_default.clone(),
                                    holes: false,
                                };
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
                                "Boundary = rest regions computed by another \
                                 toolpath's rest analysis. Pick the source \
                                 toolpath below."
                            } else {
                                "No other toolpaths in this project yet — add \
                                 one and generate it to use as the source. Picking it here \
                                 switches its rest analysis on."
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

            // P4.3: the Selection section draws the boundary selection
            // (it replaces the "Outline model" combo). It sends edits with
            // `target: Boundary`; Reset restores `inputs.boundary_default`.
            if let BoundarySource::Selection { selection, holes } = &mut entry.boundary.source {
                super::selection_section::draw_selection_section(
                    ui,
                    &super::selection_section::SelectionSectionInputs {
                        toolpath_id: entry.id,
                        toolpath_model: rs_cam_core::ids::ModelId(entry.model_id.0),
                        target: rs_cam_core::session::SelectionTarget::Boundary,
                        selection,
                        op_default: &inputs.boundary_default,
                        report: inputs.boundary_report.as_ref(),
                        item_use: rs_cam_core::session::BOUNDARY_ITEM_USE,
                        models,
                        sources: &inputs.rule_sources,
                        note_items: &inputs.boundary_note_items,
                        model_kinds: &inputs.toolpath_model_kinds,
                        tool_diameter: None,
                    },
                    selection_ui,
                    events,
                );
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
                        .truncate()
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
                             rest regions. ANY operation produces them: picking it here \
                             switches its analysis on, and the project needs a mesh. The \
                             pencil rest-depth detector and the Unified Finish claims \
                             pipeline attach their own.",
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
                    ui.add(rest_region_pathology_line(pathology));
                }
            }

            // Containment mode
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
                        .on_hover_text(
                            "Entire tool stays inside boundary (shrinks by tool radius)",
                        );
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
            format!("Rest regions \u{2192} {}", rest_region_consumers.join(", "))
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
                // whenever it overlaps, so under "After previous ops"
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
    let spindle_strategy = inputs.spindle_strategy;
    let project_default_rpm = inputs.project_default_rpm;
    let load_verdict = inputs.load_verdict();

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

/// The text of the O-2 hint (geometry selection P3.7, design 8.1).
pub(crate) const EACH_FACE_HINT: &str =
    "Faces set the top. To cut down to each face, choose Each picked face.";

/// The label of the button of the O-2 hint.
pub(crate) const EACH_FACE_HINT_BUTTON: &str = "Each picked face";

/// The O-2 hint (option A): when an up face is picked and the floor of the
/// op is not "each picked face", one line offers that floor. The button
/// sets `Floor::EachFace { depth: 0 }`: each face is cut down to its own
/// plane. Nothing cuts differently until the operator clicks.
fn draw_each_face_hint(
    ui: &mut egui::Ui,
    entry: &mut crate::state::toolpath::ToolpathEntry,
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
) {
    use rs_cam_core::compute::floor::Floor;
    let Some(floor) = entry.operation.floor() else {
        return;
    };
    let face_picked = toolpath_heights
        .and_then(|answer| answer.floor_references(&entry.heights).face_bottom_z)
        .is_some();
    if !face_picked || matches!(floor, Floor::EachFace { .. }) {
        return;
    }
    ui.horizontal(|ui| {
        ui.add(OneLine::new(format!(
            "{} {EACH_FACE_HINT}",
            crate::ui::tokens::GLYPH_DETAIL
        )));
    });
    if ui
        .add(crate::ui::components::Button::quiet(EACH_FACE_HINT_BUTTON))
        .on_hover_text(
            "Set the floor to Each picked face, depth 0: each face is cut down to its own \
             plane, from the stock top.",
        )
        .clicked()
    {
        let _ = entry.operation.set_floor(Floor::EachFace { depth: 0.0 });
    }
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
    // IC1: the diagram summarises the five planes, so it comes first. The
    // rows below it set them.
    if let Some(answer) = answer {
        // P3.7: the floor of an `OpFloor` op. The diagram drags it in its
        // own reference (L-10); the rows show it read-only (L-11). The
        // draft floor goes back through the config, as every op row does.
        let floor_inputs = entry
            .operation
            .floor()
            .map(|_| super::FloorInputs::new(answer, &entry.heights, op_report(inputs)));
        let mut floor = entry.operation.floor();
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
        if let Some(floor) = floor
            && entry.operation.floor() != Some(floor)
        {
            let _ = entry.operation.set_floor(floor);
        }
        ui.add_space(6.0);
        draw_heights_params(
            ui,
            &mut entry.heights,
            answer,
            op_type,
            entry.operation.floor().as_ref(),
            heights_refusal,
        );
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
