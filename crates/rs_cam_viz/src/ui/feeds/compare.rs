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

use rs_cam_core::feeds::efficiency::{ChipVerdict, CutEfficiency, cut_efficiency};
use rs_cam_core::feeds::suggest::FeedsPreview;
use rs_cam_core::feeds::{FeedsExplain, FeedsField, PowerFigure, PowerUnmodeled};
use rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM;
use rs_cam_core::tool_load::verdict::BoundSource;

use rs_cam_core::feeds::rationale::SuggestRationale;

use super::shared::{
    AppliedRecipe, CurrentValues, ball_tip_radius, combined_scale, hardness_label,
    material_family_label, tool_family_label,
};
use super::why;
use crate::state::AppState;
use crate::ui::components::compare;
use crate::ui::components::{ProvKind, ProvenanceBadge};
use crate::ui::{AppEvent, theme};

const ADVANCE_ROW_LABEL: &str = "Advance/tooth";

fn advance_per_tooth_mm(feed_mm_min: f64, rpm: f64, flutes: u32) -> Option<f64> {
    (rpm > 0.0 && flutes > 0).then_some(feed_mm_min / (rpm * f64::from(flutes)))
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
pub(crate) fn current_values_for_operation(
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
    project_default_rpm: u32,
) -> CurrentValues {
    CurrentValues {
        feed_rate_mm_min: operation.feed_rate(),
        plunge_rate_mm_min: operation.plunge_rate(),
        spindle_rpm: operation.spindle_rpm().or(Some(project_default_rpm)),
        depth_per_pass: operation.depth_per_pass(),
        stepover: operation.stepover(),
        flute_count: tool.flute_count,
        scallop_height: operation.scallop_height(),
        supports_scallop_override: matches!(
            operation.op_type(),
            rs_cam_core::compute::catalog::OperationType::DropCutter
        ),
        pass_role: operation.feeds_style().1,
    }
}

pub(crate) fn compute_preview_for_operation(
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
) -> FeedsPreview {
    rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        operation,
        tool,
        material,
        machine,
        rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy,
    )
}

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
        rs_cam_core::feeds::embedded_vendor_lut(),
        state.session.post_config().spindle_strategy,
    ))
}

// The card reads the operation, the tool, the material and the machine on
// top of the recommendation, because the chipload verdict is a statement
// about the cut, not about the recipe. `draw_feeds_card` carries the same
// allow for the same reason: bundling what the session already holds into
// a struct would build a second data model of it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_inspector_comparison(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    preview: &FeedsPreview,
    applied: &AppliedRecipe,
    rationale: Option<&SuggestRationale>,
    operation: &crate::state::toolpath::OperationConfig,
    tool: &crate::state::job::ToolConfig,
    material: &rs_cam_core::material::Material,
    machine: &rs_cam_core::machine::MachineProfile,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    let explain = preview.explain();
    // Core computes the verdict; this file renders it. `rs_cam_viz`'s rule:
    // do not recompute a narrower answer in the UI.
    //
    // G-RECOMAPPLIED: the verdict and the power row describe the cut that
    // `⚡ Apply all` writes, the same cut the rail rows print. Until
    // 2026-09-24 the verdict read the calculator's feed, RPM, depth and width
    // (and the operation's current values for the force headroom), and the
    // power row read the operation's current values. The calculator's cut is
    // evaluated too, so each hover can quote it.
    let (applied_op, applied_point) = applied.applied_cut(operation, &explain.recommended);
    let calculator_op = applied.calculator_cut(operation, &explain.recommended);
    let efficiency = cut_efficiency(&applied_op, tool, material, machine, &applied_point);
    let calculator_efficiency = cut_efficiency(
        &calculator_op,
        tool,
        material,
        machine,
        &explain.recommended,
    );
    let efficiency_note = headroom_note(
        efficiency.as_ref(),
        calculator_efficiency.as_ref(),
        &applied.cut_cause(),
    );
    let mut power =
        PowerReading::at_shipped_point(&applied_op, tool, material, machine, &applied_point);
    let calculator_power = PowerReading::at_shipped_point(
        &calculator_op,
        tool,
        material,
        machine,
        &explain.recommended,
    );
    power.calculator_note = power_note(&power, &calculator_power, &applied.cut_cause());
    draw_context_chip(ui, explain, tool);
    ui.add_space(crate::ui::tokens::SPACE_2);
    draw_comparison_card(
        ui,
        current,
        explain,
        applied,
        preview.refusal(),
        rationale,
        efficiency.as_ref(),
        efficiency_note.as_deref(),
        &power,
        toolpath_id,
        events,
    );
}

/// Tool, material and recommendation-source context belongs with the
/// canonical comparison, not in the Explore window. `horizontal_wrapped`
/// keeps this compact line within the narrow inspector (UR1/UR4).
///
/// The size reads `ToolConfig::size_label`, so a tapered ball says
/// "tip Ø2.00 mm (R1.00)" and never a bare "2.00 mm" beside an "R1.0"
/// vendor name.
///
/// Below the chip, the row's basis: the G1 size claim and the A4 material
/// label, as visible lines (extrapolation P1 step 4).
fn draw_context_chip(
    ui: &mut egui::Ui,
    explain: &FeedsExplain,
    tool: &crate::state::job::ToolConfig,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new("Tool:").small().color(theme::TEXT_DIM)).wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{} · {} flute · {}",
                    tool.size_label(),
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
        match &explain.matched_row {
            Some(row) => {
                // Compact in the chip: the observation id is one unbreakable
                // token wider than the rail; the hover keeps the full
                // reference. UR1's rule: nothing widens the inspector.
                ui.add(
                    ProvenanceBadge::new(ProvKind::VendorLut)
                        .reference(&row.observation_id)
                        .compact(),
                );
                // A G1 size claim states its own scale on the line below
                // (`why::draw_row_basis_lines`). A row that is far from the
                // query but has no claim (a V-bit, or a hardness-only
                // transfer) keeps the combined scale here.
                if row.is_extrapolated && row.size_basis.claim().is_none() {
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
    why::draw_row_basis_lines(ui, explain);
}

// SAFETY: the card carries one more borrowed payload than the argument
// threshold allows, for the reason `draw_inspector_comparison` carries its
// own allow: the two verdict rows are statements about the CUT, and bundling
// the session's own values into a struct to shorten this list would build a
// second data model of them.
#[allow(clippy::too_many_arguments)]
fn draw_comparison_card(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    applied: &AppliedRecipe,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    rationale: Option<&SuggestRationale>,
    efficiency: Option<&CutEfficiency>,
    efficiency_note: Option<&str>,
    power: &PowerReading,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    // W2: every row explains ITS OWN number. The `Why is the recommendation
    // here?` disclosure that used to sit under this card explained the recipe
    // as a whole, which is not the question an operator asks while reading a
    // row that tripled.
    let why = |row: why::RecipeRow| why::row_explanation(row, current, explain, applied, rationale);
    // G-RECOMAPPLIED: the recommended column is what `⚡ Apply all` writes,
    // read off the funnel's dry run. The raw calculator value is on each
    // row's hover.
    let r = &explain.recommended;
    let rpm = applied.value(FeedsField::SpindleRpm, r.rpm);
    let feed = applied.value(FeedsField::FeedRate, r.feed_rate_mm_min);
    let plunge = applied.value(FeedsField::PlungeRate, r.plunge_rate_mm_min);
    let doc = applied.value(FeedsField::DepthPerPass, r.axial_depth_mm);
    let woc = applied.value(FeedsField::Stepover, r.radial_width_mm);
    let advance = advance_per_tooth_mm(feed, rpm, current.flute_count);
    ui.label(
        egui::RichText::new("Recommendation")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    ui.add_space(4.0);

    egui::Frame::group(ui.style()).show(ui, |ui| {
        // UR4 moved this card from a 380-point modal column into the
        // 240-point inspector rail. The old five-column Grid answered with
        // its natural width and painted past the panel's left edge — the
        // exact defect class UR1 banned. Every row is now a wrapped
        // `current → recommended` line the rail can always hold.
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("current → recommended · Δ vs current")
                        .small()
                        .color(theme::TEXT_DIM),
                )
                .wrap(),
            );
        });

        rail_row(
            ui,
            "RPM",
            current.spindle_rpm.map(f64::from),
            Some(rpm),
            "",
            1.0,
            &why(why::RecipeRow::Rpm),
        );
        rail_row(
            ui,
            "Feed",
            Some(current.feed_rate_mm_min),
            Some(feed),
            " mm/min",
            1.0,
            &why(why::RecipeRow::Feed),
        );
        rail_row(
            ui,
            "Plunge",
            Some(current.plunge_rate_mm_min),
            Some(plunge),
            " mm/min",
            1.0,
            &why(why::RecipeRow::Plunge),
        );
        // G-FEEDSLABEL (UX-R03-005), G-RECOMAPPLIED (2026-09-24): the
        // recommended DOC / WOC are the values `⚡ Apply all` writes, after
        // `enforce_invariants` and the aggressiveness dial. Until 2026-09-24
        // this column printed the raw calculator values. On a Ø6 hardwood
        // pocket it recommended 4.20 mm of DOC where the apply wrote 0.81 mm.
        // Each row's hover keeps the calculator value. The advance row prints
        // feed ÷ (RPM × flutes) in both columns.
        rail_row(
            ui,
            "DOC",
            current.depth_per_pass,
            Some(doc),
            " mm",
            0.01,
            &why(why::RecipeRow::Doc),
        );
        woc_row(ui, current, explain, woc, &why(why::RecipeRow::Woc));
        rail_row(
            ui,
            ADVANCE_ROW_LABEL,
            Some(current.chipload_mm()),
            advance,
            " mm/tooth",
            0.0001,
            &why(why::RecipeRow::Advance),
        );

        // S1 — scallop-driven stepover control (DropCutter only).
        if current.supports_scallop_override {
            draw_scallop_control(ui, current, explain, toolpath_id, events);
        }

        ui.add_space(4.0);
        rail_efficiency_row(ui, efficiency, efficiency_note, advance);
        ui.add_space(2.0);
        rail_power_row(ui, power);
        ui.add_space(2.0);
        // MRR = DOC × WOC × feed, the calculator's own product (Step 5),
        // on the values Apply writes.
        rail_mrr_row(ui, doc * woc * feed);

        ui.add_space(6.0);
        draw_apply_column(ui, refusal, toolpath_id, events);
    });
}

/// One `label current → recommended Δ` line, wrapped so the inspector rail
/// can never be widened (or overflowed) by its content.
///
/// `explanation` is the row's own answer to "why is this number what it is",
/// and it is not optional. W2 deleted the `Why is the recommendation here?`
/// disclosure precisely because a whole-recipe explanation cannot answer a
/// per-row question; a row with nothing to say would put that hole back.
fn rail_row(
    ui: &mut egui::Ui,
    label: &str,
    current: Option<f64>,
    recommended: Option<f64>,
    unit: &str,
    precision: f64,
    explanation: &str,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{label} {}", crate::ui::tokens::GLYPH_DETAIL))
                    .small()
                    .strong()
                    .color(theme::TEXT_HEADING),
            )
            .wrap(),
        )
        .on_hover_text(explanation.to_owned());
        ui.add(
            egui::Label::new(
                egui::RichText::new(compare::format_optional(current, unit, precision))
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new("\u{2192}")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(compare::format_optional(recommended, unit, precision))
                    .small()
                    .color(theme::TEXT_STRONG),
            )
            .wrap(),
        );
        ui.add(egui::Label::new(compare::delta_tag(current, recommended)).wrap());
    });
    ui.add_space(2.0);
}

/// The label of a verdict row: the row's name, its detail mark, and its
/// workings behind the mark.
///
/// One renderer for one element (`ui/components/CLAUDE.md`). The chipload
/// verdict and the power reading are the same element — a named verdict whose
/// workings live on a hover — so they paint through one function rather than
/// two copies of one `RichText` chain. `component_contracts_up2` counts those
/// chains per file for exactly this reason: a second copy of an element is
/// how a surface stops reading the kit.
///
/// The chain stays hand-rolled rather than calling `components::text`,
/// because the rail's rung is `small` (11 points) and the kit's nearest rung,
/// `text::body_strong`, is 13. A rail row at 13 points does not fit the
/// 240-point column. Say if the kit should gain a dense rung.
fn verdict_row_label(ui: &mut egui::Ui, label: &str, hover: &str) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(format!("{label} {}", crate::ui::tokens::GLYPH_DETAIL))
                .small()
                .strong()
                .color(theme::TEXT_HEADING),
        )
        .wrap(),
    )
    .on_hover_text(hover.to_owned());
}

/// The rail's chipload verdict — where this cut sits in the corridor, and
/// what sitting there costs.
///
/// # This replaces a gauge that could not fail (Phase A)
///
/// The line here used to read `Power 0.01 of 0.60 kW (1 %)`. `feeds/mod.rs`
/// already carried the measurement that condemned it: across all three
/// shipped machine presets × ten species × Ø3/Ø6/Ø12 slots, the power branch
/// never fires at all, and **peak** utilisation is 23.6 %. Typical is 1 %: a
/// 6 mm cutter in softwood needs about seven watts. A figure whose maximum
/// observed value across the entire shipped matrix sits in its left quarter
/// cannot distinguish a safe cut from a safer one.
///
/// The chipload can. Below the vendor band there is no trade-off, only loss:
/// at the fixture's 0.038 mm/tooth the cut spends 1.6× the energy per mm³
/// AND 1.8× the time of the band midpoint. That is a state an operator can
/// act on, and this row changes it on every job that matters.
///
/// # Every number here can be absent, and absence is said out loud
///
/// `cut_efficiency` refuses as a whole when the material has no
/// primary-source `Kc`, and each ratio refuses on its own when its input is
/// missing. A `None` renders as a stated abstention — never as a zero, a
/// blank, a bare dash or a 100 %. The ratios go on the face because "1.6×
/// tool wear" is actionable; `u` in J/mm³ and the ploughing share go on the
/// hover, where the person who wants the workings will look.
fn rail_efficiency_row(
    ui: &mut egui::Ui,
    efficiency: Option<&CutEfficiency>,
    calculator_note: Option<&str>,
    fallback_advance_mm: Option<f64>,
) {
    // The advance per tooth is feed ÷ (RPM × flutes) — a commanded value,
    // not a force-model output — so it survives the model's refusal and is
    // still printed beside the abstention.
    let advance_mm = efficiency
        .map(|e| e.advance_per_tooth_mm)
        .or(fallback_advance_mm);
    let (verdict, color) = match efficiency {
        Some(e) => verdict_face(e),
        None => (
            format!(
                "{} efficiency not modelled for this material",
                crate::ui::tokens::GLYPH_UNKNOWN
            ),
            theme::TEXT_DIM,
        ),
    };
    let mut hover = match efficiency {
        Some(e) => efficiency_hover(e),
        None => unmodelled_hover(advance_mm),
    };
    if let Some(note) = calculator_note {
        hover.push_str("\n\n");
        hover.push_str(note);
    }
    // `horizontal_wrapped` so the verdict phrase wraps to its own line inside
    // the Simulation workspace's 240-point rail rather than widening it.
    ui.horizontal_wrapped(|ui| {
        verdict_row_label(ui, "Chip", &hover);
        ui.add(
            egui::Label::new(
                egui::RichText::new(compare::format_optional(advance_mm, " mm/tooth", 0.0001))
                    .small()
                    .color(theme::TEXT_STRONG),
            )
            .wrap(),
        )
        .on_hover_text(hover.clone());
        ui.add(egui::Label::new(egui::RichText::new(verdict).small().color(color)).wrap())
            .on_hover_text(hover);
    });
}

/// The verdict phrase and its colour.
///
/// Each arm prints only the ratios that exist. A missing ratio becomes a
/// short abstention in the phrase, never a silent omission and never a zero.
fn verdict_face(efficiency: &CutEfficiency) -> (String, egui::Color32) {
    match efficiency.verdict {
        ChipVerdict::Thin => {
            let mut costs = Vec::new();
            if let Some(wear) = efficiency.wear_ratio_vs_band_mid {
                costs.push(format!("{wear:.1}\u{00D7} tool wear"));
            }
            if let Some(time) = efficiency.time_ratio_vs_band_mid {
                costs.push(format!("{time:.1}\u{00D7} the time"));
            }
            let tail = if costs.is_empty() {
                "below the vendor range; the cost of that is not modelled".to_owned()
            } else {
                costs.join(", ")
            };
            (format!("\u{26A0} thin \u{2014} {tail}"), theme::WARNING)
        }
        ChipVerdict::InBand => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => headroom_phrase(headroom),
                None => "force headroom not modelled".to_owned(),
            };
            (
                format!(
                    "{} in vendor range \u{00B7} {tail}",
                    crate::ui::tokens::GLYPH_OK
                ),
                theme::SUCCESS,
            )
        }
        ChipVerdict::Heavy => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => headroom_phrase(headroom),
                None => "above the vendor range; force headroom not modelled".to_owned(),
            };
            (format!("\u{26A0} heavy \u{2014} {tail}"), theme::WARNING)
        }
        ChipVerdict::NoBand => {
            let tail = match efficiency.force_headroom {
                Some(headroom) => format!(" \u{00B7} {}", headroom_phrase(headroom)),
                None => String::new(),
            };
            (
                format!(
                    "{} no vendor chipload range matched{tail}",
                    crate::ui::tokens::GLYPH_UNKNOWN
                ),
                theme::TEXT_DIM,
            )
        }
    }
}

/// The verdict hover's sentence on the calculator's cut (G-RECOMAPPLIED).
///
/// `None` when the two cuts give the same phrase, or when either headroom is
/// not modelled: the hover already states that refusal.
fn headroom_note(
    applied: Option<&CutEfficiency>,
    calculator: Option<&CutEfficiency>,
    cause: &str,
) -> Option<String> {
    let applied = headroom_phrase(applied?.force_headroom?);
    let calculator = headroom_phrase(calculator?.force_headroom?);
    (applied != calculator).then(|| {
        format!(
            "Force headroom: calculator {calculator}; {cause}, so the cut Apply \
             writes has {applied}."
        )
    })
}

/// The power hover's sentence on the calculator's cut (G-RECOMAPPLIED).
///
/// `None` when the two cuts print the same percent, or when either figure is
/// refused: the hover already states that refusal.
fn power_note(applied: &PowerReading, calculator: &PowerReading, cause: &str) -> Option<String> {
    let applied = power_face(applied.figure.as_ref().ok()?);
    let calculator = power_face(calculator.figure.as_ref().ok()?);
    (applied != calculator).then(|| {
        format!(
            "Power: calculator {calculator} of the limit; {cause}, so the cut \
             Apply writes draws {applied}."
        )
    })
}

/// Force headroom as a phrase.
///
/// Two roundings are deliberate. A **negative** headroom is a real finding —
/// the predicted deflection is already past the bound — so it is said, not
/// clamped to zero. And a headroom above 99.5 % prints as `>99 %` rather
/// than rounding up: wood routing leaves the deflection bound almost
/// untouched on most cuts, and a flat `100 %` on the face is the shape this
/// repository has drawn four times — an absent constraint painted as an
/// all-clear. `>99 %` cannot be mistaken for one.
fn headroom_phrase(headroom: f64) -> String {
    if headroom < 0.0 {
        format!("{:.0} % past the deflection bound", -headroom * 100.0)
    } else if headroom > 0.995 {
        ">99 % force headroom".to_owned()
    } else {
        format!("{:.0} % force headroom", headroom * 100.0)
    }
}

/// The row's workings: the unit figure, the ploughing share in plain words,
/// the three bounds, and where each came from.
///
/// Every line is present in every state. A bound the model declines says it
/// declined; it does not drop out of the list, because a reader cannot tell a
/// missing line from a bound that does not bite.
fn efficiency_hover(efficiency: &CutEfficiency) -> String {
    let mut out = format!(
        "Chipload {:.4} mm/tooth — the recommendation's commanded advance per \
         tooth, feed \u{00F7} (RPM \u{00D7} flutes).\n\n\
         Energy per mm\u{00B3}: u = {:.0} J/mm\u{00B3}. {:.0} % of the cutting \
         power is rubbing rather than shearing wood. That share is the burn \
         and wear risk, and it falls as the chip gets thicker.\n\n",
        efficiency.advance_per_tooth_mm,
        efficiency.specific_energy_j_per_mm3,
        efficiency.ploughing_share * 100.0,
    );
    // A range only when the row publishes two different limits
    // (G-CHARTLINES). A one-point row prints its one value.
    match efficiency.band {
        Some(band) if band.min_mm_per_tooth < band.max_mm_per_tooth => out.push_str(&format!(
            "Vendor range: {:.3}–{:.3} mm/tooth, from the matched vendor row.\n",
            band.min_mm_per_tooth, band.max_mm_per_tooth,
        )),
        Some(band) => out.push_str(&format!(
            "Vendor value: {:.3} mm/tooth. The matched row publishes one value, \
             not a range.\n",
            band.max_mm_per_tooth,
        )),
        None => out.push_str(
            "Vendor range: none published \u{2014} no row matched this tool \
             \u{00D7} material, or the row publishes one limit only, so there \
             is no wear or time ratio to quote.\n",
        ),
    }
    out.push_str(&format!(
        "Rubbing floor: {:.3} mm/tooth — below it the edge burnishes instead \
         of cutting.\n",
        efficiency.rubbing_floor_mm,
    ));
    match efficiency.deflection_ceiling_mm {
        Some(ceiling) => out.push_str(&format!(
            "Deflection ceiling: {ceiling:.3} mm/tooth — the heaviest chip \
             that keeps predicted tip deflection inside {EXCEEDS_BOUND_MM:.3} mm.\n",
        )),
        None => out.push_str(
            "Deflection ceiling: not modelled — this tool, depth or engagement \
             leaves the deflection model nothing to solve.\n",
        ),
    }
    match efficiency.force_headroom {
        Some(headroom) if headroom >= 0.0 => out.push_str(&format!(
            "Force headroom: {:.1} % of the {EXCEEDS_BOUND_MM:.3} mm deflection \
             bound is unused.\n",
            headroom * 100.0,
        )),
        Some(headroom) => out.push_str(&format!(
            "Force headroom: none — the predicted deflection is {:.1} % past \
             the {EXCEEDS_BOUND_MM:.3} mm bound.\n",
            -headroom * 100.0,
        )),
        None => out.push_str(
            "Force headroom: not modelled — the pre-simulation deflection \
             predictor declined this pairing.\n",
        ),
    }
    match (
        efficiency.wear_ratio_vs_band_mid,
        efficiency.time_ratio_vs_band_mid,
    ) {
        (Some(wear), Some(time)) => out.push_str(&format!(
            "\nAgainst the middle of the vendor range: {wear:.2}\u{00D7} the \
             energy per mm\u{00B3}, {time:.2}\u{00D7} the cutting time.\n",
        )),
        _ if efficiency.band.is_some() => out.push_str(
            "\nWear and time ratios: not modelled — the band midpoint gives \
             the closed form no usable value.\n",
        ),
        _ => {}
    }
    out.push_str(
        "\nu = Ks + (F_edge \u{00B7} D \u{00B7} \u{03C8}) / (2 \u{00B7} ae \
         \u{00B7} fz), the affine wood-force fit in feeds::force scaled to \
         this material's Kc. Neither RPM nor depth of cut moves it; only the \
         chipload and the radial engagement do. Approximate — verify on a \
         test cut.",
    );
    out
}

/// The hover for the refusal state.
///
/// The material carries no primary-source `Kc`, so there is no force fit to
/// read and no efficiency number to publish. This says which input is
/// missing and names the refusal the engine already makes, so the empty row
/// cannot be read as a clean bill of health.
fn unmodelled_hover(advance_mm: Option<f64>) -> String {
    let mut out = String::from(
        "Efficiency is not modelled for this material.\n\n\
         The energy-per-mm\u{00B3} model needs a primary-source Kc, and this \
         material has none, so there is no cutting-force fit to read. Nothing \
         is shown rather than a fabricated number — the same refusal the load \
         gate makes through MaterialUnvalidated. No wear ratio, no time ratio \
         and no force headroom exist for this cut.\n\n",
    );
    match advance_mm {
        Some(fz) => out.push_str(&format!(
            "The {fz:.4} mm/tooth beside this label is the recommendation's \
             commanded advance per tooth, feed \u{00F7} (RPM \u{00D7} flutes). \
             It does not depend on the force model.",
        )),
        None => out.push_str(
            "The advance per tooth is not shown either: the recommendation has \
             no positive feed, RPM and flute count to divide.",
        ),
    }
    out
}

/// The width of the rail's power bar, in points.
///
/// The Simulation workspace's rail is 240 points wide and this row also
/// carries a label and the setting its bound traces back to, so the bar takes
/// the middle. `horizontal_wrapped` moves it to a line of its own when the
/// rail is narrower still.
const POWER_BAR_WIDTH: f32 = 84.0;

/// The power row's payload: the figure at the point this operation ships, or
/// the typed refusal, plus the provenance of the bound.
///
/// The provenance is built from the figure's own RPM — the number the
/// post-simulation power gate stores on its own verdict — so the predicted row
/// and the measured row name the same source. The limit is the rated power
/// curve with no fraction (ruling R4 Q2, 2026-09-24). The GUI owns no limit: the ceiling is
/// [`PowerFigure::available_kw`], and nothing here computes one.
struct PowerReading {
    figure: Result<PowerFigure, PowerUnmodeled>,
    source: Option<BoundSource>,
    /// The sentence on the calculator's cut, when it draws a different
    /// percent (G-RECOMAPPLIED).
    calculator_note: Option<String>,
}

impl PowerReading {
    /// Evaluate the power this operation will draw at the point it ships.
    ///
    /// **Not `FeedsResult::power_kw`.** That figure sits at the CALCULATOR's
    /// geometry, and `enforce_invariants` lowers the depth per pass
    /// afterwards; on a shipped Ø12 roughing preset the published figure is
    /// 11.96× the power at the depth that cuts. The recommendation supplies
    /// only the FALLBACK point here, for an operation that exposes no depth,
    /// no stepover or no speed of its own — the surface-following finishes
    /// command no axial step, so the calculator's value is the correct
    /// substitute for them.
    fn at_shipped_point(
        operation: &crate::state::toolpath::OperationConfig,
        tool: &crate::state::job::ToolConfig,
        material: &rs_cam_core::material::Material,
        machine: &rs_cam_core::machine::MachineProfile,
        recommended: &rs_cam_core::feeds::FeedsResult,
    ) -> Self {
        let figure = rs_cam_core::feeds::power_at_operating_point(
            operation,
            tool,
            material,
            machine,
            Some(rs_cam_core::feeds::suggest::CalculatorOperatingPoint {
                radial_width_mm: recommended.radial_width_mm,
                axial_depth_mm: recommended.axial_depth_mm,
                feed_rate_mm_min: recommended.feed_rate_mm_min,
                rpm: recommended.rpm,
            }),
        );
        let source = figure
            .as_ref()
            .ok()
            .map(|figure| BoundSource::MachinePowerCurve { rpm: figure.rpm });
        Self {
            figure,
            source,
            calculator_note: None,
        }
    }
}

/// The rail's power row — 0 to the machine's own limit at this spindle speed.
///
/// # Why the bar is back
///
/// It was deleted on a measurement: peak utilisation 23.6 % across the whole
/// shipped matrix, so the readout could not separate a good cut from a bad
/// one. R1 then rebuilt power on the affine force model, whose feed-free edge
/// term carries most of the load at wood chiploads, and the same sweep reads
/// median 17.8 %, p90 89.4 % and peak 100 %. The operator reinstated the bar
/// on 2026-09-18. `the_power_bar_is_informative_g_chipverdict` replaced the
/// ban with a test of its reason, and holds the new measurement.
///
/// # What the face carries, and what it does not
///
/// The face is the percent of the limit and nothing else: every limit on this
/// product reads 0 to limit, and "limit calculated from X" belongs in the
/// hover. The kW pair, the provenance clause and the four figures of the
/// operating point are on the hover. The caption is the SETTING that moves
/// the bound, so a reader can trace the limit back to it.
///
/// A 100 % reading here is a real reading, not an absent constraint painted
/// as an all-clear: the Step 6 power ladder clamps a power-limited recipe
/// onto the ceiling rather than letting it through, so the top of the scale
/// is where those recipes land.
///
/// # A refusal is stated, never drawn
///
/// `power_at_operating_point` refuses, typed, on seven missing inputs. Each
/// paints its own clause in the dim abstention style the chipload row uses.
/// An empty bar would read as a cut with power to spare, which is the
/// opposite of what an absent model says.
fn rail_power_row(ui: &mut egui::Ui, power: &PowerReading) {
    let hover = match (&power.figure, &power.source) {
        (Ok(figure), Some(source)) => {
            let mut hover = power_hover(figure, source);
            if let Some(note) = &power.calculator_note {
                hover.push_str("\n\n");
                hover.push_str(note);
            }
            hover
        }
        (Err(reason), _) => power_unmodelled_hover(*reason),
        // `PowerReading` builds the source from the figure, so an `Ok` with
        // no source cannot occur. It is worded as an abstention rather than
        // a panic because a GUI row is not the place to assert an invariant.
        (Ok(_), None) => power_unmodelled_hover(PowerUnmodeled::NoAvailablePower),
    };
    ui.horizontal_wrapped(|ui| {
        verdict_row_label(ui, "Power", &hover);
        match (&power.figure, &power.source) {
            (Ok(figure), Some(source)) => {
                // The door promises a finite, positive `available_kw`, so the
                // ratio is always defined. The FILL is clamped because a bar
                // cannot draw past its end; the TEXT is not, because a cut
                // over its ceiling must read over its ceiling.
                let utilisation = figure.required_kw / figure.available_kw;
                ui.add(
                    egui::ProgressBar::new(utilisation.clamp(0.0, 1.0) as f32)
                        .fill(compare::power_color(utilisation))
                        .desired_width(POWER_BAR_WIDTH)
                        .text(egui::RichText::new(power_face(figure)).small()),
                )
                .on_hover_text(hover.clone());
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(source.setting())
                            .small()
                            .color(theme::TEXT_DIM),
                    )
                    .wrap(),
                )
                .on_hover_text(hover.clone());
            }
            _ => {
                let clause = match &power.figure {
                    Err(reason) => reason.clause(),
                    Ok(_) => PowerUnmodeled::NoAvailablePower.clause(),
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "{} {clause}",
                            crate::ui::tokens::GLYPH_UNKNOWN
                        ))
                        .small()
                        .color(theme::TEXT_DIM),
                    )
                    .wrap(),
                )
                .on_hover_text(hover.clone());
            }
        }
    });
}

/// The face: the percent of the limit, formatted from the two figures it
/// describes. No number on this face is typed; both come off the figure.
fn power_face(figure: &PowerFigure) -> String {
    format!("{:.1} %", figure.required_kw / figure.available_kw * 100.0)
}

/// The row's workings: the kW pair the face does not carry, where the limit
/// came from, and the operating point the figure was evaluated at — so the
/// reader can see that the bar describes the depth that will be cut.
fn power_hover(figure: &PowerFigure, source: &BoundSource) -> String {
    format!(
        "Power {:.3} kW of {:.3} kW available \u{2014} {} of the limit.\n\n\
         Limit from {}. Setting: {}.\n\n\
         Evaluated at the cut `\u{26A1} Apply all` writes: depth of cut \
         {:.3} mm, width of cut {:.3} mm, {:.0} rpm, feed {:.0} mm/min. The \
         bar describes the depth that WILL be cut, not the depth the \
         calculator sized its feed at \u{2014} the rigidity clamp and the \
         aggressiveness dial can lower one without moving the other.",
        figure.required_kw,
        figure.available_kw,
        power_face(figure),
        source.clause(),
        source.setting(),
        figure.ap_mm,
        figure.ae_mm,
        figure.rpm,
        figure.feed_mm_min,
    )
}

/// The hover for a refused power figure.
///
/// It names the missing input and the engine's own typed refusal, so the
/// empty row cannot be read as a cut with power to spare.
fn power_unmodelled_hover(reason: PowerUnmodeled) -> String {
    format!(
        "Power is not modelled at this operating point.\n\n\
         {} \u{2014} so feeds::power_at_operating_point refuses with \
         {reason:?}, and there is no figure and no limit to draw. Nothing is \
         drawn rather than a zero or an empty bar: an empty bar reads as a \
         cut with power to spare, which is the opposite of what an absent \
         model says.\n\n\
         The machine's power curve is unaffected. What is missing is the \
         input named above.",
        reason.clause(),
    )
}

/// The rail's MRR readout, wrapped like its neighbours.
fn rail_mrr_row(ui: &mut egui::Ui, mrr_mm3_min: f64) {
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(egui::RichText::new("MRR:").small().color(theme::TEXT_DIM)).wrap());
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{mrr_mm3_min:.0} mm\u{00B3}/min")).small(),
            )
            .wrap(),
        );
    });
}

/// The inspector comparison's Apply column.
///
/// Checkpoint I-3 (2026-08-12): on a tool × operation pairing
/// `validate_tool_for_operation` refuses, the explanation remains visible but
/// the **whole Apply column is replaced by the refusal**. The write becomes
/// impossible; the explanation survives.
fn draw_apply_column(
    ui: &mut egui::Ui,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    events: &mut Vec<AppEvent>,
) {
    if let Some(err) = refusal {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "Cannot apply — Suggest refuses this recipe {}",
                    crate::ui::tokens::GLYPH_DETAIL
                ))
                .small()
                .strong()
                .color(theme::ERROR),
            )
            .wrap(),
        )
        .on_hover_text(
            "The numbers above show what the calculator would suggest. The \
             line below gives the reason for the refusal. Change the tool, \
             the operation or the material to enable Apply.",
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(err.to_string())
                    .small()
                    .color(theme::WARNING),
            )
            .wrap(),
        );
        return;
    }
    ui.horizontal_wrapped(|ui| {
        // A-3's contract: the 'changes the cut' attribution sits on the
        // button's FACE, not its hover — the operator must see it before
        // clicking. In the 240-point rail that means the button wraps:
        // `ui.button` would paint the full phrase past the panel edge.
        let apply =
            egui::Button::new("⚡ Apply all — changes the cut").wrap_mode(egui::TextWrapMode::Wrap);
        if ui
            .add(apply)
            .on_hover_text(
                "Overwrite RPM, feed, and plunge (how fast) AND DOC/WOC (the cut) with \
                 the recommended values, after the safety clamps. \
                 CHANGES THE CUT: the applied DOC/WOC are the invariant-resolved values. \
                 This is the one canonical route for applying the recommendation.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsAll(toolpath_id));
        }
    });
    // Phase C (SPEC.md, 2026-09-16). The verdict row above can say the
    // chipload is thin and costs 1.6× tool wear. Before this button the only
    // write on the tab was `⚡ Apply all`, so the only offered answer to a
    // wrong feed also rewrote the operator's cut geometry.
    //
    // A-3's attribution rule applies here too, in the mirror image: the face
    // says what this write changes AND what it holds, so the two buttons read
    // as one pair. This is not a per-field apply — it is the same funnel with
    // `ApplyScope::Speeds`, which the funnel was built to serve.
    ui.horizontal_wrapped(|ui| {
        let speeds =
            egui::Button::new("⚡ Match vendor chipload — changes the speeds, not the cut")
                .wrap_mode(egui::TextWrapMode::Wrap);
        if ui
            .add(speeds)
            .on_hover_text(
                "Overwrite RPM, feed, and plunge (how fast) with the recommended values, \
                 after the safety clamps, so the chipload lands in the vendor band. \
                 HOLDS THE CUT: DOC and WOC keep the values you authored. \
                 Use this when the chipload verdict reads thin or heavy and the cut \
                 geometry is already the one you want.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsSpeeds(toolpath_id));
        }
    });
}

/// WOC (stepover) comparison row. When the operation is in
/// scallop-driven-stepover mode (S1) the row is rendered as *derived*:
/// the label gains an "(auto from scallop)" marker and a tooltip
/// spelling out the chord-height math, so it's visually distinct from a
/// manually-entered stepover.
///
/// `woc` is the stepover `⚡ Apply all` writes (G-RECOMAPPLIED). The derived
/// value of the scallop formula stays on the hover.
fn woc_row(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    woc: f64,
    explanation: &str,
) {
    let scallop_active = current.supports_scallop_override && current.scallop_height.is_some();
    if !scallop_active {
        rail_row(
            ui,
            "WOC",
            current.stepover,
            Some(woc),
            " mm",
            0.01,
            explanation,
        );
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
    // G-RECOMAPPLIED: the face prints what `⚡ Apply all` writes. When the
    // funnel moves the derived value, the hover says so.
    let math = if format!("{woc:.2}") == format!("{derived:.2}") {
        math
    } else {
        format!("{math}\nThe invariant funnel changes it, so Apply writes {woc:.2} mm.")
    };

    // The scallop-derived variant of the per-field WOC apply (census row M6)
    // was deleted with the other five at Checkpoint I-1: it wrote
    // `explain.recommended.radial_width_mm` raw, so it skipped
    // `clamp_stepover_to_diameter` and the runtime back-off along with
    // everything else. The derived value is on the hover; the face shows the
    // clamped form of it that `⚡ Apply all` writes (G-RECOMAPPLIED).
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new("WOC ⓢ").small().color(theme::TEXT_DIM)).wrap(),
        )
        .on_hover_text(&math);
        ui.add(
            egui::Label::new(
                egui::RichText::new(compare::format_optional(current.stepover, " mm", 0.01))
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new("\u{2192}")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{woc:.2} mm (auto)"))
                    .small()
                    .color(theme::SUCCESS),
            )
            .wrap(),
        )
        .on_hover_text(&math);
        ui.add(egui::Label::new(compare::delta_tag(current.stepover, Some(woc))).wrap());
    });
    ui.add_space(2.0);
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
    // `horizontal_wrapped` so the DragValue and the derived readout wrap to
    // their own line inside the 240-point rail instead of overflowing it.
    ui.horizontal_wrapped(|ui| {
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
