//! Compare and apply — the per-operation feeds job.
//!
//! This file answers "what is this operation set to, what does the
//! recommendation say, and do I take it?". It is the job DC5a's plan puts on
//! the Feeds tab of the inspector, because per-operation editing is what the
//! inspector is for. That move is a LATER package; this file is the split
//! only, and the code inside it is unchanged.
//!
//! **Apply contract (Checkpoint I, 2026-08-12).** Both writes — `⚡ Apply
//! all` and the explore-chart apply — go through `feeds::suggest::apply`,
//! the same funnel the properties panel uses, and both are replaced by the
//! refusal text when `validate_tool_for_operation` declines the tool ×
//! operation pairing (I-3).

use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::rationale::SuggestRationale;
use rs_cam_core::feeds::suggest::FeedsPreview;

use super::explore::{draw_chart_a, draw_chart_b, draw_chart_c};
use super::shared::{
    CurrentValues, ball_tip_radius, combined_scale, hardness_label, material_family_label,
    tool_family_label,
};
use super::why::{
    draw_chipload_breakdown, draw_chipload_engaged_attestation, draw_chipload_min_warning,
    draw_engaged_diameter_row, draw_provenance_disclosure, draw_rationale, draw_warnings,
};
use crate::state::AppState;
use crate::ui::components::compare::{self, CompareRow};
use crate::ui::components::{ProvKind, ProvenanceBadge};
use crate::ui::properties::feeds_rows;
use crate::ui::{AppEvent, theme};

/// Project-level spindle policy selector. Emits
/// `AppEvent::SetSpindleStrategy` when the operator toggles. Sits at
/// the modal head so its effect on every recommendation below is
/// visible — the row even surfaces the machine ceiling so the operator
/// can sanity-check the headroom.
pub(crate) fn draw_spindle_strategy_row(
    ui: &mut egui::Ui,
    current: rs_cam_core::feeds::SpindleStrategy,
    machine: &rs_cam_core::machine::MachineProfile,
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::feeds::SpindleStrategy;
    let (_, max_rpm) = machine.rpm_range();
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new("Spindle policy:")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        let mut next = current;
        // MatchChart radio
        if ui
            .radio(current == SpindleStrategy::MatchChart, "Match chart")
            .on_hover_text(
                "Use the LUT row's chart-published RPM verbatim. \
                 Tightest match to the vendor band's own test conditions.",
            )
            .clicked()
        {
            next = SpindleStrategy::MatchChart;
        }
        if ui
            .radio(
                current == SpindleStrategy::MaxSpeed,
                "Max speed (constant advance/tooth)",
            )
            .on_hover_text(format!(
                "Push RPM up to the spindle ceiling ({:.0} RPM × \
                 {:.0}% headroom), scaling feed proportionally to \
                 keep the commanded advance/tooth constant. Capped by vendor rpm_max \
                 when published, and by a {:.1}× hard limit over the \
                 chart RPM. Power-limit derate still applies on top \
                 — if the higher operating point exceeds spindle \
                 power, feed comes back down.",
                max_rpm,
                rs_cam_core::feeds::SPINDLE_CEILING_HEADROOM * 100.0,
                rs_cam_core::feeds::MAX_SPINDLE_SPEEDUP,
            ))
            .clicked()
        {
            next = SpindleStrategy::MaxSpeed;
        }
        if next != current {
            events.push(AppEvent::SetSpindleStrategy(next));
        }
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("(spindle max {:.0} RPM)", max_rpm))
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
    });
}

// ────────────────────────────────────────────────────────────────────
// Toolpath view (Phase 1 + 2 + 3)
// ────────────────────────────────────────────────────────────────────

pub(crate) fn draw_toolpath_view(
    ui: &mut egui::Ui,
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    let Some(preview) = compute_preview(state, toolpath_id) else {
        ui.label(
            egui::RichText::new(
                "Could not build explanation — missing tool, material, or machine.",
            )
            .small()
            .color(theme::WARNING),
        );
        return;
    };
    let explain = preview.explain();
    let refusal = preview.refusal();
    let Some(current) = read_current_values(state, toolpath_id) else {
        ui.label("Toolpath disappeared.");
        return;
    };

    draw_context_chip(ui, explain);
    // S2 — engaged-diameter-at-DOC annotation for tapered/V tools.
    draw_engaged_diameter_row(ui, &current, explain);
    ui.add_space(8.0);

    // Two-column body: left = comparison card + provenance; right = charts.
    egui::Panel::left("feeds_modal_left")
        .resizable(true)
        .default_size(380.0)
        .min_size(320.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_comparison_card(ui, &current, explain, refusal, toolpath_id, events);
                ui.add_space(8.0);
                draw_chipload_breakdown(ui, explain);
                ui.add_space(8.0);
                draw_provenance_disclosure(ui, explain, modal.show_provenance, events);
                ui.add_space(8.0);
                draw_warnings(ui, explain);
                ui.add_space(8.0);
                let rationale = compute_suggest_rationale(state, toolpath_id);
                draw_rationale(ui, &rationale);
            });
        });

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            draw_chart_c(ui, &current, explain, refusal, toolpath_id, modal, events);
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    draw_chart_a(ui, &current, explain);
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    draw_chart_b(ui, &current, explain);
                });
            });
        });
    });
}

// ── Context chip ────────────────────────────────────────────────────

fn draw_context_chip(ui: &mut egui::Ui, explain: &FeedsExplain) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new("Tool:").small().color(theme::TEXT_DIM)).wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{:.2} mm · {} flute · {}",
                    explain.tool_diameter_mm,
                    explain.flute_count,
                    tool_family_label(explain.query.tool_family),
                ))
                .small()
                .color(theme::TEXT_STRONG),
            )
            .wrap(),
        );
        ui.separator();
        ui.add(
            egui::Label::new(
                egui::RichText::new("Material:")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{} ({})",
                    material_family_label(explain.query.material_family),
                    hardness_label(explain.query_hardness_kind, explain.query_hardness_value),
                ))
                .small()
                .color(theme::TEXT_STRONG),
            )
            .wrap(),
        );
        ui.separator();
        ui.add(
            egui::Label::new(
                egui::RichText::new("Source:")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        // Source signal via the one provenance vocabulary (ProvenanceBadge);
        // the extrapolation caveat stays an explicit amber note rather than
        // recolouring the source, so a vendor row reads canonically green and
        // "approx" is a separate, honest signal (P7-003 collapse).
        match &explain.matched_row {
            Some(row) => {
                ui.add(ProvenanceBadge::new(ProvKind::VendorLut).reference(&row.observation_id));
                if row.is_extrapolated {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("approx ×{:.2}", combined_scale(row)))
                                .small()
                                .color(theme::WARNING_MILD),
                        )
                        .wrap(),
                    );
                }
            }
            None => {
                ui.add(ProvenanceBadge::new(ProvKind::Formula));
            }
        }
    });
}

pub(crate) fn read_current_values(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Option<CurrentValues> {
    let tc = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)?;
    let tool = state
        .session
        .tools()
        .iter()
        .find(|t| t.id == rs_cam_core::compute::ToolId(tc.tool_id))?;
    Some(CurrentValues {
        feed_rate_mm_min: tc.operation.feed_rate(),
        plunge_rate_mm_min: tc.operation.plunge_rate(),
        spindle_rpm: tc.operation.spindle_rpm(),
        depth_per_pass: tc.operation.depth_per_pass(),
        stepover: tc.operation.stepover(),
        flute_count: tool.flute_count,
        scallop_height: tc.operation.scallop_height(),
        supports_scallop_override: matches!(
            tc.operation.op_type(),
            rs_cam_core::compute::catalog::OperationType::DropCutter
        ),
        pass_role: tc.operation.feeds_style().1,
    })
}

/// Build the modal's payload.
///
/// Returns a [`FeedsPreview`], not a bare [`FeedsExplain`]: the explain payload
/// is infallible by design (you want the nomogram even for a pairing you would
/// decline to run) and therefore cannot tell this surface that the pairing was
/// refused — which is exactly how the modal came to offer eleven writes on
/// operations the engine had already declared unrunnable (A-3 census §3.1).
/// The preview carries the refusal alongside the numbers, and hands out
/// something writable only when there isn't one.
pub(crate) fn compute_preview(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Option<FeedsPreview> {
    let tc = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)?;
    let tool = state
        .session
        .tools()
        .iter()
        .find(|t| t.id == rs_cam_core::compute::ToolId(tc.tool_id))?;
    let stock = state.session.stock_config();
    Some(rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &tc.operation,
        tool,
        &stock.material,
        state.session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    ))
}

/// v3.1: derive a [`SuggestRationale`] tree from the live operation by
/// running the same Suggest invocation `--apply-suggest` would use, then
/// converting the warnings into rationale entries. Returns an empty
/// rationale (rendered as nothing) when the Suggest call refuses
/// (unmatched tool × op, etc.) — the user has no information they need
/// to act on in that case.
///
/// T10 (Phase 4): the context assembly + Suggest invocation live in
/// [`rs_cam_core::session::ProjectSession::cutter_op_profile`], shared
/// with the MCP `get_suggest_rationale` surface.
fn compute_suggest_rationale(
    state: &AppState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> SuggestRationale {
    let Some(tc) = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
    else {
        return SuggestRationale::default();
    };
    let Some(profile) = state.session.cutter_op_profile(tc) else {
        return SuggestRationale::default();
    };
    match profile.feasibility {
        Ok(()) => SuggestRationale::from_warnings(&profile.warnings),
        Err(_) => SuggestRationale::default(),
    }
}

fn draw_comparison_card(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Recommendation")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    ui.add_space(4.0);

    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new("feeds_modal_compare")
            .num_columns(5)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                // Header row
                ui.label(egui::RichText::new("").small());
                ui.label(
                    egui::RichText::new("Current")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new("Recommended")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(egui::RichText::new("Δ").small().color(theme::TEXT_DIM));
                ui.label(egui::RichText::new("").small());
                ui.end_row();

                CompareRow::new(
                    "RPM",
                    current.spindle_rpm.map(f64::from),
                    Some(explain.recommended.rpm),
                    "",
                    1.0,
                )
                .show(ui);
                CompareRow::new(
                    "Feed",
                    Some(current.feed_rate_mm_min),
                    Some(explain.recommended.feed_rate_mm_min),
                    " mm/min",
                    1.0,
                )
                .show(ui);
                CompareRow::new(
                    "Plunge",
                    Some(current.plunge_rate_mm_min),
                    Some(explain.recommended.plunge_rate_mm_min),
                    " mm/min",
                    1.0,
                )
                .show(ui);
                // G-FEEDSLABEL (UX-R03-005): the recommended DOC / WOC are
                // the RAW calculator values — `⚡ Apply all` passes them
                // through `enforce_invariants`, which can lower them (the
                // rigidity cap put 1.2 where this cell reads 4.2 on the R03
                // pocket) — so the cell says so. The advance row prints
                // feed ÷ (RPM × flutes) in BOTH columns; it used to print
                // the pre-derate target chipload beside the current
                // advance under a "Commanded" label.
                CompareRow::new(
                    "DOC",
                    current.depth_per_pass,
                    Some(explain.recommended.axial_depth_mm),
                    " mm",
                    0.01,
                )
                .recommended_note(feeds_rows::CALCULATOR_NOTE)
                .show(ui);
                woc_row(ui, current, explain);
                CompareRow::new(
                    feeds_rows::MODAL_ADVANCE_ROW_LABEL,
                    Some(current.chipload_mm()),
                    feeds_rows::advance_per_tooth_mm(
                        explain.recommended.feed_rate_mm_min,
                        explain.recommended.rpm,
                        current.flute_count,
                    ),
                    " mm/tooth",
                    0.0001,
                )
                .show(ui);
            });

        // S3 — chipload-min (rubbing/burning) warning, finish ops only.
        draw_chipload_min_warning(ui, current, explain);
        // S4 — engaged-diameter chipload attestation (tapered/V tools).
        draw_chipload_engaged_attestation(ui, current, explain);

        // S1 — scallop-driven stepover control (DropCutter only).
        if current.supports_scallop_override {
            draw_scallop_control(ui, current, explain, toolpath_id, events);
        }

        ui.add_space(4.0);
        compare::power_bar(
            ui,
            explain.recommended.power_kw,
            explain.recommended.available_power_kw,
        );
        ui.add_space(2.0);
        compare::mrr_row(ui, explain.recommended.mrr_mm3_min);

        ui.add_space(6.0);
        draw_apply_column(ui, refusal, toolpath_id, events);
    });
}

/// The modal's Apply column.
///
/// Checkpoint I-3 (2026-08-12): on a tool × operation pairing
/// `validate_tool_for_operation` refuses, the modal still opens and still
/// draws every chart — the explanatory job is the modal's real job — but the
/// **whole Apply column is replaced by the refusal**. The write becomes
/// impossible; the explanation survives. Before this, the modal previewed and
/// applied recipes on pairings the properties panel declined to show at all.
fn draw_apply_column(
    ui: &mut egui::Ui,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    if let Some(err) = refusal {
        ui.label(
            egui::RichText::new("Cannot apply — this tool cannot run this operation")
                .small()
                .strong()
                .color(theme::ERROR),
        );
        ui.label(
            egui::RichText::new(err.to_string())
                .small()
                .color(theme::WARNING),
        );
        ui.label(
            egui::RichText::new(
                "The numbers above are shown so you can see what the calculator would \
                 suggest and why the pairing is refused. Change the tool (or the \
                 operation) to enable Apply.",
            )
            .small()
            .color(theme::TEXT_DIM),
        );
        return;
    }
    ui.horizontal(|ui| {
        if ui
            .button("⚡ Apply all — changes the cut")
            .on_hover_text(
                "Overwrite RPM, feed, and plunge (how fast) AND DOC/WOC (the cut) with \
                 the recommended values, after the safety clamps. \
                 CHANGES THE CUT: the applied DOC/WOC are the invariant-resolved values, \
                 identical to the properties panel's \"Apply recommended speeds\" plus \
                 \"Apply cut geometry\". To move only the speeds, use the panel.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsAll(toolpath_id));
        }
    });
}

/// WOC (stepover) comparison row. When the operation is in
/// scallop-driven-stepover mode (S1) the row is rendered as *derived*:
/// the label gains an "(auto from scallop)" marker and a tooltip
/// spelling out the chord-height math, so it's visually distinct from a
/// manually-entered stepover.
fn woc_row(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    let scallop_active = current.supports_scallop_override && current.scallop_height.is_some();
    if !scallop_active {
        CompareRow::new(
            "WOC",
            current.stepover,
            Some(explain.recommended.radial_width_mm),
            " mm",
            0.01,
        )
        .recommended_note(feeds_rows::CALCULATOR_NOTE)
        .show(ui);
        return;
    }

    let h = current.scallop_height.unwrap_or(0.0);
    let derived = explain.recommended.radial_width_mm;
    let tip_r = ball_tip_radius(explain);
    let math = match tip_r {
        Some(r) => format!(
            "Auto-derived from scallop height.\n\
             h = {:.0} μm, tip r = {:.2} mm\n\
             ae = 2·√(2·r·h − h²) = {derived:.3} mm",
            h * 1000.0,
            r,
        ),
        None => "Scallop-driven stepover needs a ball or tapered-ball tool; \
                 the formula default is used instead."
            .to_owned(),
    };

    ui.label(egui::RichText::new("WOC ⓢ").small().color(theme::TEXT_DIM))
        .on_hover_text(&math);
    ui.label(compare::format_optional(current.stepover, " mm", 0.01));
    ui.label(
        egui::RichText::new(format!("{derived:.2} mm (auto)"))
            .small()
            .color(theme::SUCCESS),
    )
    .on_hover_text(&math);
    ui.label(compare::delta_tag(current.stepover, Some(derived)));
    // The scallop-derived variant of the per-field WOC apply (census row M6)
    // was deleted with the other five at Checkpoint I-1: it wrote
    // `explain.recommended.radial_width_mm` raw, so it skipped
    // `clamp_stepover_to_diameter` and the runtime back-off along with
    // everything else. The derived value is still *shown* — reading it is the
    // row's job — and `⚡ Apply all` writes the clamped form of it.
    ui.label("");
    ui.end_row();
}

/// S1 — scallop-driven-stepover control for DropCutter. A checkbox
/// toggles the override on/off; when on, a micron DragValue dials the
/// target cusp height and a live readout shows the derived stepover.
fn draw_scallop_control(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    let tip_r = ball_tip_radius(explain);
    ui.horizontal(|ui| {
        let mut enabled = current.scallop_height.is_some();
        if ui
            .checkbox(&mut enabled, "Scallop-driven stepover")
            .on_hover_text(
                "Derive WOC from a target cusp (scallop) height and the tool's \
                 ball-tip radius instead of the formula default. Turn off to \
                 restore the formula-based stepover.",
            )
            .changed()
        {
            let value = enabled.then(|| current.scallop_height.unwrap_or(0.010));
            events.push(AppEvent::SetDropCutterScallopHeight { toolpath_id, value });
        }
        if let Some(h) = current.scallop_height {
            let mut microns = h * 1000.0;
            let resp = ui.add(
                egui::DragValue::new(&mut microns)
                    .speed(0.5)
                    .range(1.0..=500.0)
                    .suffix(" μm"),
            );
            if resp.changed() {
                let value = Some((microns / 1000.0).max(0.0001));
                events.push(AppEvent::SetDropCutterScallopHeight { toolpath_id, value });
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "→ ae {:.3} mm",
                        explain.recommended.radial_width_mm
                    ))
                    .small()
                    .color(theme::SUCCESS),
                )
                .wrap(),
            );
        }
    });
    if current.scallop_height.is_some() && tip_r.is_none() {
        ui.label(
            egui::RichText::new(
                "⚠ This tool has no spherical tip — scallop height is ignored; \
                 the formula stepover is used.",
            )
            .small()
            .color(theme::WARNING_MILD),
        );
    }
}
