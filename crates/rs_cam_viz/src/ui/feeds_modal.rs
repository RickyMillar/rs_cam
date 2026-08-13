//! Redesigned Feeds & Speeds modal.
//!
//! Replaces the old in-panel feeds card with a wide modal that
//! pairs a side-by-side current/recommended comparison with three
//! machinist-style charts:
//!
//! - **Chart A** — advance/tooth vs diameter (the "tool size" axis)
//! - **Chart B** — advance/tooth vs hardness (the "material" axis)
//! - **Chart C** — feed vs RPM at constant advance/tooth (the nomogram)
//!
//! Every quantity these charts plot is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)` — the modal is a pre-simulation surface and has
//! no measured value to show. The achieved figure lives on the properties
//! panel's operating-point card, after a sim. Checkpoint H2, 2026-08-08.
//!
//! The modal re-derives a [`FeedsPreview`] from the live session every frame.
//!
//! **Apply contract (Checkpoint I, 2026-08-12).** The modal is an
//! *explanatory* surface that happens to offer two writes; it is not a second
//! application API. Both writes — `⚡ Apply all` and the explore-chart apply —
//! go through `feeds::suggest::apply`, the same funnel the properties panel
//! uses, and both are replaced by the refusal text when
//! `validate_tool_for_operation` declines the tool × operation pairing (I-3).
//! The six per-row `Apply` buttons that used to sit in the comparison grid are
//! **deleted** (I-1): they wrote `FeedsExplain::recommended` raw, so on the
//! shipped default fixture the DOC row put 4.445 mm into an operation whose
//! clamped depth is 1.27 mm. See
//! `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md`.
//!
//! Phases (per FEEDS_AND_SPEEDS redesign plan):
//! - Phase 1: comparison card + Chart C
//! - Phase 2: Charts A & B + provenance disclosure
//! - Phase 3: drag-to-explore on Chart C
//! - Phase 4: project-rollup mode (per-toolpath table)

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points, Polygon};
use rs_cam_core::feeds::rationale::{RationaleEntry, SuggestRationale};
use rs_cam_core::feeds::suggest::FeedsPreview;
use rs_cam_core::feeds::{
    FeedsExplain, ToolGeometryHint, vendor_lut::HardnessKind, vendor_lut::MaterialFamily,
    vendor_lut::ToolFamily,
};

use super::components::compare::{self, CompareRow};
use super::components::{ProvKind, ProvenanceBadge};
use super::{AppEvent, theme};
use crate::state::AppState;
use crate::state::{FeedsModalMode, ProjectFeedsSort};

/// Top-level draw entry. Short-circuits when no modal is open.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(modal) = state.feeds_modal.as_ref() else {
        return;
    };
    let toolpath_id = modal.toolpath_id;

    let mut still_open = true;
    let title = match modal.mode {
        FeedsModalMode::Toolpath => {
            let Some(name) = toolpath_name(state, toolpath_id) else {
                // The toolpath is gone (project changed under an open
                // modal). Pre-fix this rendered a broken shell titled
                // "toolpath N" — caught by the 2026-06-11 capture sweep.
                events.push(AppEvent::CloseFeedsModal);
                return;
            };
            format!("Feeds & Speeds — {name}")
        }
        FeedsModalMode::Project => "Feeds & Speeds — All toolpaths".to_owned(),
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(960.0)
        .default_height(620.0)
        .open(&mut still_open)
        .show(ctx, |ui| {
            draw_header(ui, modal.mode, events);
            ui.add_space(2.0);
            draw_spindle_strategy_row(
                ui,
                state.session.post_config().spindle_strategy,
                state.session.machine(),
                events,
            );
            ui.separator();
            match modal.mode {
                FeedsModalMode::Toolpath => {
                    draw_toolpath_view(ui, state, toolpath_id, modal, events);
                }
                FeedsModalMode::Project => {
                    draw_project_view(ui, state, modal, events);
                }
            }
        });

    if !still_open {
        events.push(AppEvent::CloseFeedsModal);
    }
}

fn toolpath_name(state: &AppState, toolpath_id: rs_cam_core::ToolpathId) -> Option<String> {
    state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .map(|tc| tc.name.clone())
}

fn draw_header(ui: &mut egui::Ui, mode: FeedsModalMode, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        let toolpath_active = mode == FeedsModalMode::Toolpath;
        let project_active = mode == FeedsModalMode::Project;
        if ui
            .selectable_label(toolpath_active, "This toolpath")
            .on_hover_text("Per-toolpath recommendation, charts, and Apply buttons.")
            .clicked()
            && !toolpath_active
        {
            events.push(AppEvent::SetFeedsModalMode(FeedsModalMode::Toolpath));
        }
        if ui
            .selectable_label(project_active, "All toolpaths")
            .on_hover_text("Project-wide rollup with per-toolpath Δ rows.")
            .clicked()
            && !project_active
        {
            events.push(AppEvent::SetFeedsModalMode(FeedsModalMode::Project));
        }
    });
}

/// Project-level spindle policy selector. Emits
/// `AppEvent::SetSpindleStrategy` when the operator toggles. Sits at
/// the modal head so its effect on every recommendation below is
/// visible — the row even surfaces the machine ceiling so the operator
/// can sanity-check the headroom.
fn draw_spindle_strategy_row(
    ui: &mut egui::Ui,
    current: rs_cam_core::feeds::SpindleStrategy,
    machine: &rs_cam_core::machine::MachineProfile,
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::feeds::SpindleStrategy;
    let (_, max_rpm) = machine.rpm_range();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Spindle policy:")
                .small()
                .color(theme::TEXT_DIM),
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
        ui.label(
            egui::RichText::new(format!("(spindle max {:.0} RPM)", max_rpm))
                .small()
                .color(theme::TEXT_DIM),
        );
    });
}

// ────────────────────────────────────────────────────────────────────
// Toolpath view (Phase 1 + 2 + 3)
// ────────────────────────────────────────────────────────────────────

fn draw_toolpath_view(
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
        .show_inside(ui, |ui| {
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

    egui::CentralPanel::default().show_inside(ui, |ui| {
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
        ui.label(egui::RichText::new("Tool:").small().color(theme::TEXT_DIM));
        ui.label(
            egui::RichText::new(format!(
                "{:.2} mm · {} flute · {}",
                explain.tool_diameter_mm,
                explain.flute_count,
                tool_family_label(explain.query.tool_family),
            ))
            .small()
            .color(theme::TEXT_STRONG),
        );
        ui.separator();
        ui.label(
            egui::RichText::new("Material:")
                .small()
                .color(theme::TEXT_DIM),
        );
        ui.label(
            egui::RichText::new(format!(
                "{} ({})",
                material_family_label(explain.query.material_family),
                hardness_label(explain.query_hardness_kind, explain.query_hardness_value),
            ))
            .small()
            .color(theme::TEXT_STRONG),
        );
        ui.separator();
        ui.label(
            egui::RichText::new("Source:")
                .small()
                .color(theme::TEXT_DIM),
        );
        // Source signal via the one provenance vocabulary (ProvenanceBadge);
        // the extrapolation caveat stays an explicit amber note rather than
        // recolouring the source, so a vendor row reads canonically green and
        // "approx" is a separate, honest signal (P7-003 collapse).
        match &explain.matched_row {
            Some(row) => {
                ui.add(ProvenanceBadge::new(ProvKind::VendorLut).reference(&row.observation_id));
                if row.is_extrapolated {
                    ui.label(
                        egui::RichText::new(format!("approx ×{:.2}", combined_scale(row)))
                            .small()
                            .color(theme::WARNING_MILD),
                    );
                }
            }
            None => {
                ui.add(ProvenanceBadge::new(ProvKind::Formula));
            }
        }
    });
}

fn combined_scale(row: &rs_cam_core::feeds::vendor_lookup::MatchedRow) -> f64 {
    row.chipload_diameter_scale * row.chipload_hardness_scale
}

fn tool_family_label(f: ToolFamily) -> &'static str {
    match f {
        ToolFamily::FlatEnd => "Flat end",
        ToolFamily::BallNose => "Ball nose",
        ToolFamily::BullNose => "Bull nose",
        ToolFamily::ChamferVbit => "V-bit",
        ToolFamily::TaperedBallNose => "Tapered ball",
        ToolFamily::FacingBit => "Facing bit",
    }
}

fn material_family_label(m: MaterialFamily) -> &'static str {
    match m {
        MaterialFamily::Softwood => "Softwood",
        MaterialFamily::Hardwood => "Hardwood",
        MaterialFamily::PlywoodSoftwood => "Plywood (soft)",
        MaterialFamily::PlywoodHardwood => "Plywood (hard)",
        MaterialFamily::Mdf => "MDF",
        MaterialFamily::Hdf => "HDF",
        MaterialFamily::Particleboard => "Particle board",
        MaterialFamily::Acrylic => "Acrylic",
        MaterialFamily::Hdpe => "HDPE",
        MaterialFamily::Delrin => "Delrin",
        MaterialFamily::Polycarbonate => "Polycarbonate",
        MaterialFamily::Aluminum => "Aluminum",
        MaterialFamily::Fiberglass => "Fiberglass",
    }
}

fn hardness_label(kind: Option<HardnessKind>, value: Option<f64>) -> String {
    match (kind, value) {
        (Some(HardnessKind::Janka), Some(v)) => format!("Janka {v:.0}"),
        (Some(HardnessKind::ShoreD), Some(v)) => format!("Shore D {v:.0}"),
        (Some(HardnessKind::Hb), Some(v)) => format!("HB {v:.0}"),
        _ => "—".to_owned(),
    }
}

// ── Comparison card ─────────────────────────────────────────────────

/// Live current values pulled from the toolpath's `OperationConfig`.
/// All values are denormalised so the card can render without
/// dispatching on operation variant.
#[derive(Debug, Clone, Copy)]
struct CurrentValues {
    feed_rate_mm_min: f64,
    plunge_rate_mm_min: f64,
    spindle_rpm: Option<u32>,
    depth_per_pass: Option<f64>,
    stepover: Option<f64>,
    flute_count: u32,
    /// Active scallop target (mm). `Some` means stepover is derived from
    /// the cusp geometry, not a manual value. Only DropCutter exposes
    /// this as an *optional* override (see S1).
    scallop_height: Option<f64>,
    /// True when this operation supports the scallop-driven-stepover
    /// override (currently DropCutter / 3D Finish). Gates the
    /// "Scallop height" input and the derived-stepover rendering.
    supports_scallop_override: bool,
    /// Pass role of this operation — drives the finish-only chipload-min
    /// warning (S3).
    pass_role: rs_cam_core::feeds::PassRole,
}

impl CurrentValues {
    /// Compute the resulting chipload from current settings:
    /// `feed / (rpm * flutes)`. Returns 0.0 when any input is missing
    /// or zero — the UI degrades gracefully.
    fn chipload_mm(&self) -> f64 {
        let rpm = match self.spindle_rpm {
            Some(r) if r > 0 => f64::from(r),
            _ => return 0.0,
        };
        if self.flute_count == 0 {
            return 0.0;
        }
        self.feed_rate_mm_min / (rpm * f64::from(self.flute_count))
    }
}

fn read_current_values(
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
fn compute_preview(state: &AppState, toolpath_id: rs_cam_core::ToolpathId) -> Option<FeedsPreview> {
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
            .spacing([10.0, 4.0])
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
                CompareRow::new(
                    "DOC",
                    current.depth_per_pass,
                    Some(explain.recommended.axial_depth_mm),
                    " mm",
                    0.01,
                )
                .show(ui);
                woc_row(ui, current, explain);
                CompareRow::new(
                    "Commanded advance/tooth",
                    Some(current.chipload_mm()),
                    Some(explain.recommended.chip_load_mm),
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

/// Ball-tip radius (mm) used for scallop/cusp geometry, or `None` for
/// tools without a spherical tip (the scallop override has no effect on
/// those — the cusp curve is undefined).
fn ball_tip_radius(explain: &FeedsExplain) -> Option<f64> {
    match explain.tool_geometry {
        ToolGeometryHint::Ball => Some(explain.tool_diameter_mm / 2.0),
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some(tip_radius),
        _ => None,
    }
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
            ui.label(
                egui::RichText::new(format!(
                    "→ ae {:.3} mm",
                    explain.recommended.radial_width_mm
                ))
                .small()
                .color(theme::SUCCESS),
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

/// Vendor chipload band `(min, max)` for the matched row, with the same
/// `None`-max fallback the charts use. `None` when no row matched.
fn vendor_band(explain: &FeedsExplain) -> Option<(f64, f64)> {
    explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    })
}

/// For tapered-ball / V-bit tools, describe the engaged cutting diameter
/// at the operation's depth of cut: `(doc_mm, engaged_dia_mm,
/// tip_dia_mm, tip_label)`. `None` for flat / ball / bull tools, whose
/// engaged diameter equals the nominal diameter regardless of DOC (no
/// cone shoulder), so the annotation would be redundant.
fn engaged_diameter_context(
    current: &CurrentValues,
    explain: &FeedsExplain,
) -> Option<(f64, f64, f64, &'static str)> {
    let (tip_dia, kind) = match explain.tool_geometry {
        ToolGeometryHint::TaperedBall { tip_radius, .. } => (tip_radius * 2.0, "tip"),
        ToolGeometryHint::VBit { tip_diameter, .. } => (tip_diameter, "tip"),
        _ => return None,
    };
    // Use the operator's per-pass DOC when set, else the DOC the
    // recommendation itself was built on.
    let doc = current
        .depth_per_pass
        .filter(|d| *d > 0.0)
        .unwrap_or(explain.recommended.axial_depth_mm);
    if doc <= 0.0 {
        return None;
    }
    let engaged = explain.tool_geometry.engaged_diameter_at_doc(
        doc,
        explain.tool_diameter_mm,
        explain.shank_diameter_mm,
    );
    Some((doc, engaged, tip_dia, kind))
}

/// S2 — engaged-diameter-at-DOC annotation row. Renders just below the
/// context chip for tapered-ball / V-bit tools, where the published tip
/// size badly understates what's actually cutting at depth.
fn draw_engaged_diameter_row(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    let Some((doc, engaged, tip_dia, kind)) = engaged_diameter_context(current, explain) else {
        return;
    };
    // Flag when the engaged diameter has hit the shank, or differs from
    // the published tip by more than 50 % — i.e. the cone shoulder is
    // doing most of the work and the tip spec is misleading.
    let flag = engaged >= explain.shank_diameter_mm - 1e-6
        || (tip_dia > 1e-6 && (engaged - tip_dia).abs() / tip_dia > 0.5);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Engaged ⌀:")
                .small()
                .color(theme::TEXT_DIM),
        );
        let icon = if flag { "⚠ " } else { "" };
        ui.label(
            egui::RichText::new(format!(
                "{icon}{engaged:.2} mm at DOC {doc:.2} mm (vs {kind} ⌀ {tip_dia:.2} mm)"
            ))
            .small()
            .color(if flag {
                theme::WARNING_MILD
            } else {
                theme::TEXT_STRONG
            }),
        )
        .on_hover_text(
            "The cone shoulder does most of the cutting at depth. Advance/tooth \
             bounds in this modal apply to this engaged diameter, computed \
             at this DOC — not the published tool tip size.",
        );
    });
}

/// S3 — chipload-min warning. For finishing operations a chipload below
/// the vendor band minimum is the common failure mode (operator slows
/// feed for surface quality → rubbing / burning). Surface it loudly.
fn draw_chipload_min_warning(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    if current.pass_role != rs_cam_core::feeds::PassRole::Finish {
        return;
    }
    let cl = current.chipload_mm();
    if cl <= 0.0 {
        return;
    }
    let Some((lo, _hi)) = vendor_band(explain) else {
        return;
    };
    if cl >= lo {
        return;
    }
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(
            "⚠ Commanded advance/tooth below the vendor band minimum — risk of \
             rubbing or burning. Feed too slow, RPM too high, or both. Raise feed \
             or drop RPM.",
        )
        .small()
        .color(theme::WARNING_MILD),
    );
}

/// S4 — engaged-diameter chipload attestation. One italic line, for
/// tapered-ball / V-bit tools, telling the operator the chipload number
/// is computed at the engaged diameter (the correct thing) rather than
/// the tool tip — so they can trust it.
fn draw_chipload_engaged_attestation(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
) {
    let Some((doc, engaged, _tip, _kind)) = engaged_diameter_context(current, explain) else {
        return;
    };
    ui.label(
        egui::RichText::new(format!(
            "Advance/tooth computed at engaged diameter ⌀ {engaged:.2} mm (DOC \
             {doc:.2} mm), not the tool tip."
        ))
        .small()
        .italics()
        .color(theme::TEXT_DIM),
    );
}

// compare_row / format_optional / format_delta / draw_power_bar / power_color
// / draw_mrr_row were lifted to `ui::components::compare` (CL 4/4) so the
// optimizer rollup and the Feeds Details drawer share one implementation.

// ── Provenance disclosure ───────────────────────────────────────────

fn draw_provenance_disclosure(
    ui: &mut egui::Ui,
    explain: &FeedsExplain,
    show: bool,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        let arrow = if show { "▼" } else { "▶" };
        if ui
            .small_button(format!("{arrow} How is this calculated?"))
            .clicked()
        {
            events.push(AppEvent::ToggleFeedsProvenance);
        }
    });
    if !show {
        return;
    }
    egui::Frame::group(ui.style()).show(ui, |ui| match &explain.matched_row {
        Some(row) => {
            egui::Grid::new("feeds_modal_prov")
                .num_columns(2)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Row").small().color(theme::TEXT_DIM));
                    ui.label(egui::RichText::new(&row.observation_id).small());
                    ui.end_row();
                    ui.label(egui::RichText::new("Vendor").small().color(theme::TEXT_DIM));
                    ui.label(egui::RichText::new(row.source_vendor.to_string()).small());
                    ui.end_row();
                    ui.label(
                        egui::RichText::new("Calibrated for")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.2} mm tool, {} flute",
                            row.row_diameter_mm, explain.flute_count
                        ))
                        .small(),
                    );
                    ui.end_row();
                    ui.label(
                        egui::RichText::new("Scaling")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    let scaling_color = if row.is_extrapolated {
                        theme::WARNING_MILD
                    } else {
                        theme::SUCCESS
                    };
                    let scaling_label = if (row.chipload_diameter_scale - 1.0).abs() < 1e-3
                        && (row.chipload_hardness_scale - 1.0).abs() < 1e-3
                    {
                        "none (direct match)".to_owned()
                    } else {
                        format!(
                            "diameter ×{:.2} · hardness ×{:.2}{}",
                            row.chipload_diameter_scale,
                            row.chipload_hardness_scale,
                            if row.is_extrapolated {
                                " (approximate)"
                            } else {
                                ""
                            }
                        )
                    };
                    ui.label(
                        egui::RichText::new(scaling_label)
                            .small()
                            .color(scaling_color),
                    );
                    ui.end_row();
                    if let (Some(min), Some(max)) = (row.chip_load_min_mm, row.chip_load_max_mm) {
                        ui.label(
                            egui::RichText::new("Scaled band")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(
                            egui::RichText::new(format!("{min:.4}–{max:.4} mm/tooth")).small(),
                        );
                        ui.end_row();
                    }
                    if let (Some(lo), Some(hi)) = (row.rpm_min, row.rpm_max) {
                        ui.label(
                            egui::RichText::new("Vendor RPM range")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(egui::RichText::new(format!("{lo:.0}–{hi:.0}")).small());
                        ui.end_row();
                    } else if let Some(nom) = row.rpm_nominal {
                        ui.label(
                            egui::RichText::new("Vendor RPM (nominal)")
                                .small()
                                .color(theme::TEXT_DIM),
                        );
                        ui.label(egui::RichText::new(format!("{nom:.0}")).small());
                        ui.end_row();
                    }
                });
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "No vendor LUT row matched. Recommendation is from the \
                     empirical formula. Re-check against vendor data before use.",
                )
                .small()
                .color(theme::WARNING_MILD),
            );
        }
    });
}

// ── Warnings ────────────────────────────────────────────────────────

fn draw_warnings(ui: &mut egui::Ui, explain: &FeedsExplain) {
    if explain.recommended.warnings.is_empty() {
        return;
    }
    ui.label(
        egui::RichText::new("Warnings")
            .small()
            .strong()
            .color(theme::WARNING_MILD),
    );
    for w in &explain.recommended.warnings {
        let text = match w {
            rs_cam_core::feeds::FeedsWarning::FeedRateClamped { requested, actual } => {
                format!("Feed clamped: {requested:.0} → {actual:.0} mm/min (machine limit)")
            }
            rs_cam_core::feeds::FeedsWarning::PowerLimited {
                required_kw,
                available_kw,
            } => {
                format!("Power limited: {required_kw:.2} kW needed, {available_kw:.2} kW available")
            }
            rs_cam_core::feeds::FeedsWarning::DocExceedsFlute { requested, capped } => {
                format!("DOC capped: {requested:.1} → {capped:.1} mm (flute guard)")
            }
            rs_cam_core::feeds::FeedsWarning::SlottingDetected { doc_reduced_to } => {
                format!("Slotting detected: DOC reduced to {doc_reduced_to:.1} mm")
            }
            rs_cam_core::feeds::FeedsWarning::ScallopInvalid {
                target,
                max_possible,
            } => format!("Invalid scallop: {target:.3} mm (max {max_possible:.1} mm)"),
            rs_cam_core::feeds::FeedsWarning::ShankTooLarge { shank_mm, max_mm } => {
                format!("Shank {shank_mm:.1} mm exceeds max {max_mm:.1} mm")
            }
            rs_cam_core::feeds::FeedsWarning::ChiploadClampedToFloor {
                requested,
                floor,
                band_capped_from,
            } => match band_capped_from {
                None => {
                    format!(
                        "Commanded advance/tooth below rubbing floor: \
                         {requested:.3} → {floor:.3} mm/tooth"
                    )
                }
                Some(global) => format!(
                    "Commanded advance/tooth raised to vendor band ceiling: \
                     {requested:.3} → {floor:.3} mm/tooth \
                     (whole band is below the {global:.3} rubbing floor — expect burnishing)"
                ),
            },
            // Checkpoint K (a3) — RPM-anchor row: the number above it in
            // this modal is the empirical formula's, and the modal's band
            // chart has nothing to draw for it.
            // Checkpoint K (a4) — the routing refused; there is no vendor
            // row behind any number on this surface.
            rs_cam_core::feeds::FeedsWarning::NoVendorRowsForRoutedOperation {
                operation_kind,
                tool_family,
                missing_rows,
            } => format!(
                "No vendor data for {operation_kind} on a {tool_family} cutter \
                 — formula-derived, no band ({missing_rows})"
            ),
            rs_cam_core::feeds::FeedsWarning::VendorRowPublishesNoChipload {
                observation_id,
                formula_chipload_mm,
            } => format!(
                "Vendor row {observation_id} publishes RPM only — the \
                 {formula_chipload_mm:.4} mm/tooth shown is the empirical formula's, and \
                 this recommendation carries no vendor band (the post-sim gate uses a \
                 different, chipload-bearing row)"
            ),
            rs_cam_core::feeds::FeedsWarning::DrillFeedClampedToEnvelope {
                requested,
                actual,
                envelope_lo,
                envelope_hi,
            } => format!(
                "Drill feed clamped: {requested:.0} → {actual:.0} mm/min (envelope {envelope_lo:.0}–{envelope_hi:.0})"
            ),
        };
        ui.label(
            egui::RichText::new(format!("⚠ {text}"))
                .small()
                .color(theme::WARNING_MILD),
        );
    }
}

// ── Suggest rationale (v3.1) ─────────────────────────────────────────

/// Render the rationale tree the combined-Suggest orchestrator
/// produces alongside its parameter writes. Each entry corresponds to
/// one [`SuggestWarning`] emitted by `enforce_invariants`. Renders
/// nothing when the rationale is empty (Suggest made no rewrites
/// worth surfacing).
fn draw_rationale(ui: &mut egui::Ui, rationale: &SuggestRationale) {
    if rationale.is_empty() {
        return;
    }
    ui.label(
        egui::RichText::new("Why these values?")
            .small()
            .strong()
            .color(theme::TEXT_STRONG),
    );
    for entry in &rationale.entries {
        draw_rationale_entry(ui, entry);
    }
}

fn draw_rationale_entry(ui: &mut egui::Ui, entry: &RationaleEntry) {
    ui.label(
        egui::RichText::new(format!("• {}", entry.headline))
            .small()
            .color(theme::TEXT_STRONG),
    );
    if let Some(detail) = &entry.detail {
        ui.label(
            egui::RichText::new(format!("    {detail}"))
                .small()
                .color(theme::TEXT_DIM),
        );
    }
}

// ────────────────────────────────────────────────────────────────────
// Chipload-math breakdown
// ────────────────────────────────────────────────────────────────────

/// Render the full chipload → feed pipeline so the user can see *why*
/// the recommended diamond sits where it does on the nomogram. The
/// "target" chipload comes from either a vendor LUT row (midpoint) or
/// the empirical formula `K₀ × D^p × (1/H)^q`. Each derate is then
/// listed with its multiplier, and the final effective chipload
/// matches `feed / (RPM × flutes)` — the chart marker's location.
fn draw_chipload_breakdown(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let d = &explain.recommended.derates;
    let flutes = explain.flute_count.max(1) as f64;
    let effective = if explain.recommended.rpm > 0.0 {
        explain.recommended.feed_rate_mm_min / (explain.recommended.rpm * flutes)
    } else {
        0.0
    };

    egui::CollapsingHeader::new(
        egui::RichText::new("Why is the recommendation here?")
            .strong()
            .color(theme::TEXT_STRONG),
    )
    .default_open(false)
    .show(ui, |ui| {
        // ── Step 1: target chipload ──────────────────────────────────
        ui.label(
            egui::RichText::new("Target advance/tooth")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        match (&explain.recommended.chipload_source, &d.formula) {
            (rs_cam_core::feeds::ChiploadSource::VendorLut { observation_id }, _) => {
                let lut_band = match &explain.matched_row {
                    Some(row) => match (row.chip_load_min_mm, row.chip_load_max_mm) {
                        (Some(lo), Some(hi)) => {
                            format!(" (band {lo:.4}–{hi:.4})")
                        }
                        _ => String::new(),
                    },
                    None => String::new(),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "Vendor LUT midpoint: {:.4} mm/tooth{lut_band}",
                        d.target_chip_load_mm
                    ))
                    .small(),
                );
                ui.label(
                    egui::RichText::new(format!("from row: {observation_id}"))
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }
            (rs_cam_core::feeds::ChiploadSource::FormulaFallback, Some(f))
            | (rs_cam_core::feeds::ChiploadSource::EdgeRadiusFloor, Some(f)) => {
                ui.label(
                    egui::RichText::new(
                        "No vendor LUT matched — using empirical formula:",
                    )
                    .small()
                    .color(theme::WARNING_MILD),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "  fz = K₀ · D^p · (1/H)^q\n     = {:.4} · {:.2}^{:.2} · (1/{:.1})^{:.2}\n     = {:.4} mm/tooth",
                        f.k0, f.diameter_mm, f.p, f.feed_scale_factor, f.q, f.result_mm_tooth
                    ))
                    .small()
                    .monospace(),
                );
                ui.label(
                    egui::RichText::new(
                        "K₀/p/q come from MachineProfile.chip_load; D is your tool diameter; \
                         H is the material hardness index.",
                    )
                    .small()
                    .color(theme::TEXT_DIM),
                );
            }
            _ => {
                ui.label(
                    egui::RichText::new(format!(
                        "Starting advance/tooth: {:.4} mm/tooth",
                        d.target_chip_load_mm
                    ))
                    .small(),
                );
            }
        }

        ui.add_space(6.0);

        // ── Step 2: derate chain ─────────────────────────────────────
        ui.label(
            egui::RichText::new("Derates applied")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        egui::Grid::new("feeds_modal_derates")
            .num_columns(3)
            .spacing([8.0, 1.0])
            .show(ui, |ui| {
                // Header
                ui.label(
                    egui::RichText::new("factor")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new("×")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new("note")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                ui.end_row();

                derate_row(
                    ui,
                    "radial chip-thinning",
                    d.radial_chip_thinning,
                    if d.radial_chip_thinning > 1.001 {
                        "thin chip at small stepover — feed faster"
                    } else {
                        "stepover deep enough, no thinning"
                    },
                );
                if (d.axial_chip_thinning - 1.0).abs() > 1e-3 {
                    derate_row(
                        ui,
                        "axial chip-thinning",
                        d.axial_chip_thinning,
                        "ball / tapered-ball at shallow DOC",
                    );
                }
                if (d.combined_chip_thinning - d.radial_chip_thinning * d.axial_chip_thinning)
                    .abs()
                    > 1e-3
                {
                    derate_row(
                        ui,
                        "combined chip-thinning (clamped 1.0–4.0)",
                        d.combined_chip_thinning,
                        "guard so feed doesn't explode",
                    );
                }
                derate_row(
                    ui,
                    "depth-tier feed derate",
                    d.depth_tier,
                    if d.depth_tier < 0.999 {
                        "deep cut — slow feed to limit deflection"
                    } else {
                        "shallow / nominal depth"
                    },
                );
                derate_row(
                    ui,
                    "L/D overhang",
                    d.ld_overhang,
                    if d.ld_overhang < 0.999 {
                        "long tool — back off to limit deflection"
                    } else {
                        "stickout reasonable for tool diameter"
                    },
                );
                derate_row(
                    ui,
                    "workholding rigidity",
                    d.workholding,
                    match d.workholding {
                        x if x < 0.99 => "Low rigidity (tape / vacuum) — back off",
                        x if x > 1.01 => "High rigidity (vise / bolted) — push up",
                        _ => "Medium — no adjustment",
                    },
                );
                if d.power_limit < 0.999 {
                    derate_row(
                        ui,
                        "power limit",
                        d.power_limit,
                        "spindle can't deliver more power — feed reduced",
                    );
                }
                if d.feed_clamp < 0.999 {
                    derate_row(
                        ui,
                        "feed-cap clamp",
                        d.feed_clamp,
                        "hit machine.max_feed_mm_min",
                    );
                }
                derate_row(
                    ui,
                    "machine safety factor",
                    d.safety_factor,
                    "extra margin so the recommendation is comfortably safe",
                );
                // Spindle speedup is the only ≥ 1.0 "derate" in the
                // chain — it walks the constant-chipload line up the
                // speed axis when SpindleStrategy::MaxSpeed is on.
                // Hidden when at unity (the default MatchChart state)
                // to avoid clutter on every recommendation.
                if (d.spindle_speedup - 1.0).abs() > 1e-3 {
                    derate_row(
                        ui,
                        "spindle speedup",
                        d.spindle_speedup,
                        "MaxSpeed policy — RPM lifted toward spindle ceiling, feed scaled to keep the commanded advance/tooth constant",
                    );
                }
            });

        ui.add_space(6.0);

        // ── Step 3: effective chipload ───────────────────────────────
        let combined = d.combined_factor();
        let derate_pct = ((1.0 - combined) * 100.0).max(0.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Commanded advance/tooth at recommendation:")
                    .small()
                    .strong()
                    .color(theme::TEXT_STRONG),
            );
            ui.label(
                egui::RichText::new(format!("{effective:.4} mm/tooth"))
                    .small()
                    .strong()
                    .color(theme::SUCCESS),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "= target {:.4} × {:.3} (combined derate, {derate_pct:.0}% reduction)",
                d.target_chip_load_mm, combined
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.label(
            egui::RichText::new(format!(
                "= feed {:.0} mm/min ÷ ({:.0} RPM × {} flutes)",
                explain.recommended.feed_rate_mm_min, explain.recommended.rpm, explain.flute_count
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        if let Some((_, hi)) = explain.matched_row.as_ref().and_then(|r| {
            r.chip_load_min_mm.zip(r.chip_load_max_mm)
        }) && effective > 0.0
        {
            let pct_of_max = (effective / hi) * 100.0;
            ui.label(
                egui::RichText::new(format!(
                    "= {pct_of_max:.0}% of vendor band max ({hi:.4} mm/tooth)"
                ))
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    });
}

fn derate_row(ui: &mut egui::Ui, label: &str, value: f64, note: &str) {
    let color = if value < 0.999 {
        theme::WARNING_MILD
    } else if value > 1.001 {
        theme::SUCCESS
    } else {
        theme::TEXT_DIM
    };
    ui.label(egui::RichText::new(label).small());
    ui.label(
        egui::RichText::new(format!("{value:.3}"))
            .small()
            .color(color)
            .monospace(),
    );
    ui.label(egui::RichText::new(note).small().color(theme::TEXT_DIM));
    ui.end_row();
}

// ────────────────────────────────────────────────────────────────────
// Chart C — Feed vs RPM (the nomogram)
// ────────────────────────────────────────────────────────────────────

fn draw_chart_c(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Feed vs RPM (drag the red point to explore)")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    ui.add_space(2.0);
    let flutes = explain.flute_count.max(1) as f64;
    let env = &explain.machine;

    // RPM/Feed axis bounds — show a bit past the machine cap so the
    // wall is visible.
    let rpm_axis_max = env.spindle_max_rpm * 1.15;
    let feed_axis_max = env.max_feed_mm_min * 1.05;

    // Vendor band — three iso-chipload diagonals (min, mid, max).
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });
    let band_admit = band.map(|(lo, hi)| {
        // 5 % tolerance band (matches optimizer's breakage_tolerance default).
        let admit_hi = hi * 1.05;
        let admit_lo = (lo * 0.95).max(0.0);
        (admit_lo, lo, hi, admit_hi)
    });

    // Vendor RPM column.
    let vendor_rpm = explain
        .matched_row
        .as_ref()
        .map(|row| (row.rpm_min, row.rpm_max, row.rpm_nominal));

    // Compute drag-to-explore preview point. We capture the operating
    // point from `modal.explore` if set, otherwise show the current op.
    let explore = modal.explore;
    let display_point = explore.unwrap_or(crate::state::NomogramExplore {
        rpm: current.spindle_rpm.map(f64::from).unwrap_or(0.0),
        feed_mm_min: current.feed_rate_mm_min,
    });

    let plot_response = Plot::new("feeds_modal_nomogram")
        .height(280.0)
        .include_x(0.0)
        .include_y(0.0)
        .include_x(rpm_axis_max)
        .include_y(feed_axis_max)
        .x_axis_label("RPM")
        .y_axis_label("feed (mm/min)")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            // 1. Band wedge polygon — clipped at machine RPM cap and
            //    feed cap so the band visually stops at the wall.
            if let Some((lo, hi)) = band {
                let band_poly =
                    wedge_polygon(lo, hi, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                plot_ui.polygon(
                    Polygon::new("", band_poly)
                        .fill_color(egui::Color32::from_rgba_unmultiplied(80, 180, 80, 50))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(80, 180, 80, 120),
                        ))
                        .name(format!("Vendor band {lo:.4}–{hi:.4} mm/tooth")),
                );
                // Inline label inside the band at the cap intersection
                // so the user can see "VENDOR BAND" on the chart itself.
                let mid_cl = (lo + hi) * 0.5;
                let mid_pts =
                    clip_iso_line(mid_cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                if let Some(end) = mid_pts.last() {
                    plot_ui.text(
                        egui_plot::Text::new(
                            "",
                            egui_plot::PlotPoint::new(end[0] * 0.55, end[1] * 0.5),
                            egui::RichText::new(format!(
                                "VENDOR BAND\n{lo:.4}\u{2013}{hi:.4} mm advance/tooth"
                            ))
                            .small()
                            .color(egui::Color32::from_rgba_unmultiplied(80, 180, 80, 220)),
                        )
                        .anchor(egui::Align2::CENTER_CENTER),
                    );
                }
            }
            if let Some((admit_lo, lo, hi, admit_hi)) = band_admit {
                // Two thinner ribbons in the 5 % tolerance zones —
                // also clipped at the machine caps.
                let low_admit = wedge_polygon(
                    admit_lo,
                    lo,
                    env.spindle_max_rpm,
                    flutes,
                    env.max_feed_mm_min,
                );
                let high_admit = wedge_polygon(
                    hi,
                    admit_hi,
                    env.spindle_max_rpm,
                    flutes,
                    env.max_feed_mm_min,
                );
                let warn = egui::Color32::from_rgba_unmultiplied(220, 180, 60, 50);
                let warn_edge = egui::Color32::from_rgba_unmultiplied(220, 180, 60, 100);
                plot_ui.polygon(
                    Polygon::new("", low_admit)
                        .fill_color(warn)
                        .stroke(egui::Stroke::new(1.0, warn_edge))
                        .name("Band-admitted (low)"),
                );
                plot_ui.polygon(
                    Polygon::new("", high_admit)
                        .fill_color(warn)
                        .stroke(egui::Stroke::new(1.0, warn_edge))
                        .name("Band-admitted (high)"),
                );
            }

            // 2. Three iso-chipload diagonals — each one is labelled
            //    inline at its right-end so the chipload value is
            //    visible without consulting a legend.
            if let Some((lo, hi)) = band {
                let mid = (lo + hi) * 0.5;
                for (cl, color, prefix) in [
                    (lo, egui::Color32::from_rgb(180, 130, 60), "min"),
                    (mid, egui::Color32::from_rgb(120, 180, 120), "mid"),
                    (hi, egui::Color32::from_rgb(200, 90, 90), "max"),
                ] {
                    let line_pts =
                        clip_iso_line(cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                    plot_ui.line(
                        Line::new("", PlotPoints::from(line_pts.clone()))
                            .color(color)
                            .width(1.5)
                            .name(format!("chipload {prefix} {cl:.4} mm/tooth")),
                    );
                    // Drop an inline value tag at the last point of the
                    // clipped line. The line ends either at the RPM cap
                    // (sloped) or at the feed cap (horizontal); either
                    // way the last point is in-frame.
                    if let Some(end) = line_pts.last() {
                        plot_ui.text(
                            egui_plot::Text::new(
                                "",
                                egui_plot::PlotPoint::new(end[0], end[1]),
                                egui::RichText::new(format!(" {prefix} {cl:.4}"))
                                    .small()
                                    .color(color),
                            )
                            .anchor(egui::Align2::LEFT_BOTTOM),
                        );
                    }
                }
            }

            // 3. Machine envelope — shaded forbidden zones, walls, labels.
            draw_machine_envelope(plot_ui, env, rpm_axis_max, feed_axis_max);

            // 4. Vendor RPM column.
            if let Some((Some(lo), Some(hi), _)) = vendor_rpm {
                plot_ui.polygon(
                    Polygon::new(
                        "",
                        PlotPoints::from(vec![
                            [lo, 0.0],
                            [hi, 0.0],
                            [hi, feed_axis_max],
                            [lo, feed_axis_max],
                        ]),
                    )
                    .fill_color(egui::Color32::from_rgba_unmultiplied(100, 160, 200, 25))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(100, 160, 200, 80),
                    ))
                    .name("Vendor RPM range"),
                );
            }

            // 5. Current operating point.
            if let Some(rpm) = current.spindle_rpm {
                plot_ui.points(
                    Points::new("", vec![[f64::from(rpm), current.feed_rate_mm_min]])
                        .shape(MarkerShape::Circle)
                        .filled(true)
                        .radius(6.0)
                        .color(theme::ERROR)
                        .name("Current"),
                );
            }
            // Target (pre-derate) operating point. Shows where the
            // recommendation *would* sit if the safety factor and
            // overhang / workholding / power / feed-clamp derates
            // weren't applied. The arrow from target → recommended
            // makes the derate visually obvious.
            let target_chipload = explain.recommended.derates.target_chip_load_mm;
            if target_chipload > 0.0 && explain.recommended.rpm > 0.0 {
                let target_feed = target_chipload * explain.recommended.rpm * flutes;
                if (target_feed - explain.recommended.feed_rate_mm_min).abs() > 1.0 {
                    plot_ui.points(
                        Points::new("", vec![[explain.recommended.rpm, target_feed]])
                            .shape(MarkerShape::Circle)
                            .filled(false)
                            .radius(7.0)
                            .color(egui::Color32::from_rgb(100, 160, 240))
                            .name(format!("Target pre-derate ({target_chipload:.4} mm/tooth)")),
                    );
                    // Arrow from target down to derated recommendation.
                    plot_ui.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![
                                [explain.recommended.rpm, target_feed],
                                [
                                    explain.recommended.rpm,
                                    explain.recommended.feed_rate_mm_min,
                                ],
                            ]),
                        )
                        .color(egui::Color32::from_rgba_unmultiplied(100, 160, 240, 180))
                        .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                        .width(1.5)
                        .name("derate chain"),
                    );
                    // Inline label on the derate arrow.
                    let mid_feed = (target_feed + explain.recommended.feed_rate_mm_min) * 0.5;
                    let derate_pct = (1.0 - explain.recommended.derates.combined_factor()) * 100.0;
                    plot_ui.text(
                        egui_plot::Text::new(
                            "",
                            egui_plot::PlotPoint::new(explain.recommended.rpm, mid_feed),
                            egui::RichText::new(format!(" −{derate_pct:.0}% derate"))
                                .small()
                                .color(egui::Color32::from_rgba_unmultiplied(100, 160, 240, 220)),
                        )
                        .anchor(egui::Align2::LEFT_CENTER),
                    );
                }
            }

            // Recommended operating point (after derates).
            plot_ui.points(
                Points::new(
                    "",
                    vec![[
                        explain.recommended.rpm,
                        explain.recommended.feed_rate_mm_min,
                    ]],
                )
                .shape(MarkerShape::Diamond)
                .filled(true)
                .radius(6.0)
                .color(egui::Color32::from_rgb(100, 160, 240))
                .name("Recommended (after derates)"),
            );

            // 6. Drag-to-explore overlay (Phase 3).
            if explore.is_some() {
                plot_ui.points(
                    Points::new("", vec![[display_point.rpm, display_point.feed_mm_min]])
                        .shape(MarkerShape::Cross)
                        .filled(true)
                        .radius(8.0)
                        .color(egui::Color32::from_rgb(220, 200, 80))
                        .name("Explore"),
                );
                // Proposed-move line from current → explore.
                if let Some(rpm) = current.spindle_rpm {
                    plot_ui.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![
                                [f64::from(rpm), current.feed_rate_mm_min],
                                [display_point.rpm, display_point.feed_mm_min],
                            ]),
                        )
                        .color(egui::Color32::from_rgba_unmultiplied(220, 200, 80, 180))
                        .style(egui_plot::LineStyle::Dashed { length: 5.0 })
                        .width(1.2)
                        .name("proposal"),
                    );
                }
            }

            // 7. Hover readout — shows the resulting chipload, the
            //    chart bucket the hover lands in (within / admit /
            //    BURN / BREAK), and the machine-cap clearance at the
            //    pointer's RPM/feed. Same text as the verdict line so
            //    the user can learn the chart by hovering.
            if let Some(hover) = plot_ui.pointer_coordinate() {
                // Read the CLAMPED coordinates once and derive everything
                // from them, readout and cap-note alike — see
                // `hover_readout_chipload`.
                let (rpm, feed) = (hover.x.max(0.0), hover.y.max(0.0));
                let cl = hover_readout_chipload(rpm, feed, flutes);
                let (verdict, color) = chipload_verdict(cl, explain);
                let cap_note = if rpm > env.spindle_max_rpm {
                    " · past spindle cap".to_owned()
                } else if feed > env.max_feed_mm_min {
                    " · past feed cap".to_owned()
                } else if rpm < env.spindle_min_rpm {
                    " · below spindle min".to_owned()
                } else {
                    String::new()
                };
                let label = format!(
                    "{rpm:.0} RPM · {feed:.0} mm/min\n→ commanded advance/tooth \
                     {cl:.4} mm/tooth · {verdict}{cap_note}",
                );
                // Anchor the readout near the top-left of the chart so
                // it stays out of the band area.
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(rpm_axis_max * 0.02, feed_axis_max * 0.97),
                        egui::RichText::new(label)
                            .small()
                            .background_color(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140))
                            .color(color),
                    )
                    .anchor(egui::Align2::LEFT_TOP),
                );
            }
        });

    // Band legend below the chart — colour swatch + numeric range for
    // every overlay on the plot. This is the single best lever for
    // "what does the green/yellow/blue/red mean".
    draw_chart_c_legend(ui, current, explain);

    // Click inside the plot sets the explore point. Egui_plot only
    // surfaces the pointer coordinate while the response is hovered;
    // we route a single click on press → release.
    if explore.is_some()
        && plot_response.response.clicked()
        && let Some(coord) = plot_response.response.interact_pointer_pos()
    {
        // Translate screen-space click back to plot coordinates.
        let plot_coord = plot_response.transform.value_from_position(coord);
        let new_rpm = plot_coord.x.clamp(env.spindle_min_rpm, env.spindle_max_rpm);
        let new_feed = plot_coord.y.clamp(0.0, env.max_feed_mm_min);
        events.push(AppEvent::SetFeedsExplore(Some(
            crate::state::NomogramExplore {
                rpm: new_rpm,
                feed_mm_min: new_feed,
            },
        )));
    }

    // Phase 3 — Explore controls.
    draw_explore_controls(ui, current, explain, refusal, toolpath_id, modal, events);
}

/// Band legend rendered under Chart C. Each row is `[swatch] label —
/// numeric range`. Compact, fits in two columns.
fn draw_chart_c_legend(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    let env = &explain.machine;
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });
    let vendor_rpm = explain
        .matched_row
        .as_ref()
        .map(|row| (row.rpm_min, row.rpm_max, row.rpm_nominal));

    let mut entries: Vec<LegendEntry> = Vec::new();

    if let Some((lo, hi)) = band {
        let mid = (lo + hi) * 0.5;
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            egui::Color32::from_rgba_unmultiplied(80, 180, 80, 200),
            "Vendor band",
            format!("{lo:.4}–{hi:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            egui::Color32::from_rgba_unmultiplied(220, 180, 60, 180),
            "+5% tolerance",
            format!(
                "{:.4}–{lo:.4}  ·  {hi:.4}–{:.4} mm/tooth",
                lo * 0.95,
                hi * 1.05
            ),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            egui::Color32::from_rgb(180, 130, 60),
            "iso-advance min",
            format!("{lo:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            egui::Color32::from_rgb(120, 180, 120),
            "iso-advance mid",
            format!("{mid:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            egui::Color32::from_rgb(200, 90, 90),
            "iso-advance max",
            format!("{hi:.4} mm/tooth"),
        ));
    }

    if let Some((lo, hi, nominal)) = vendor_rpm {
        let value = match (lo, hi, nominal) {
            (Some(lo), Some(hi), _) => format!("{lo:.0}–{hi:.0} RPM"),
            (None, None, Some(nom)) => format!("{nom:.0} RPM (nominal)"),
            (Some(v), None, _) | (None, Some(v), _) => format!("{v:.0} RPM"),
            _ => "—".to_owned(),
        };
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            egui::Color32::from_rgba_unmultiplied(100, 160, 200, 200),
            "Vendor RPM range",
            value,
        ));
    }

    entries.push(LegendEntry::new(
        LegendSwatch::FilledSquare,
        egui::Color32::from_rgba_unmultiplied(200, 90, 90, 150),
        "Machine forbidden",
        format!(
            "> {} RPM   or   > {} mm/min",
            env.spindle_max_rpm as i64, env.max_feed_mm_min as i64
        ),
    ));
    if env.spindle_min_rpm > 0.0 {
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            egui::Color32::from_rgba_unmultiplied(150, 150, 160, 150),
            "Below spindle min",
            format!("< {} RPM", env.spindle_min_rpm as i64),
        ));
    }
    if let Some(rpm) = current.spindle_rpm {
        entries.push(LegendEntry::new(
            LegendSwatch::Circle,
            theme::ERROR,
            "● Current",
            format!("{rpm} RPM · {:.0} mm/min", current.feed_rate_mm_min),
        ));
    }
    let target_cl = explain.recommended.derates.target_chip_load_mm;
    if target_cl > 0.0 && explain.recommended.rpm > 0.0 {
        let target_feed = target_cl * explain.recommended.rpm * explain.flute_count.max(1) as f64;
        if (target_feed - explain.recommended.feed_rate_mm_min).abs() > 1.0 {
            entries.push(LegendEntry::new(
                LegendSwatch::Circle,
                egui::Color32::from_rgb(100, 160, 240),
                "○ Target (pre-derate)",
                format!("{target_cl:.4} mm/tooth · {target_feed:.0} mm/min"),
            ));
        }
    }
    entries.push(LegendEntry::new(
        LegendSwatch::Diamond,
        egui::Color32::from_rgb(100, 160, 240),
        "◆ Recommended (effective)",
        format!(
            "{:.0} RPM · {:.0} mm/min · chipload {:.4} mm/tooth",
            explain.recommended.rpm,
            explain.recommended.feed_rate_mm_min,
            if explain.recommended.rpm > 0.0 {
                explain.recommended.feed_rate_mm_min
                    / (explain.recommended.rpm * explain.flute_count.max(1) as f64)
            } else {
                0.0
            }
        ),
    ));

    ui.add_space(4.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("What the colours mean")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        ui.add_space(2.0);
        // Two-column grid: swatch+label on the left, value on the right.
        egui::Grid::new("feeds_modal_chart_c_legend")
            .num_columns(2)
            .spacing([10.0, 2.0])
            .show(ui, |ui| {
                for entry in &entries {
                    ui.horizontal(|ui| {
                        draw_legend_swatch(ui, entry.swatch, entry.color);
                        ui.label(
                            egui::RichText::new(entry.label)
                                .small()
                                .color(theme::TEXT_STRONG),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&entry.value)
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.end_row();
                }
            });
    });
}

#[derive(Debug, Clone, Copy)]
enum LegendSwatch {
    FilledSquare,
    Line,
    Circle,
    Diamond,
}

struct LegendEntry {
    swatch: LegendSwatch,
    color: egui::Color32,
    label: &'static str,
    value: String,
}

impl LegendEntry {
    fn new(swatch: LegendSwatch, color: egui::Color32, label: &'static str, value: String) -> Self {
        Self {
            swatch,
            color,
            label,
            value,
        }
    }
}

fn draw_legend_swatch(ui: &mut egui::Ui, swatch: LegendSwatch, color: egui::Color32) {
    let size = egui::vec2(14.0, 10.0);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    match swatch {
        LegendSwatch::FilledSquare => {
            painter.rect_filled(rect, 2.0, color);
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(0.5, color.linear_multiply(1.4)),
                egui::StrokeKind::Middle,
            );
        }
        LegendSwatch::Line => {
            let y = rect.center().y;
            painter.line_segment(
                [
                    egui::pos2(rect.left() + 1.0, y),
                    egui::pos2(rect.right() - 1.0, y),
                ],
                egui::Stroke::new(2.0, color),
            );
        }
        LegendSwatch::Circle => {
            painter.circle_filled(rect.center(), 4.0, color);
        }
        LegendSwatch::Diamond => {
            let c = rect.center();
            let r = 5.0;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, c.y - r),
                    egui::pos2(c.x + r, c.y),
                    egui::pos2(c.x, c.y + r),
                    egui::pos2(c.x - r, c.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
    }
}

/// Build the polygon for a chipload band wedge between `cl_lo` and
/// `cl_hi`, clipped to the chart's RPM and feed extents.
fn wedge_polygon(
    cl_lo: f64,
    cl_hi: f64,
    rpm_max: f64,
    flutes: f64,
    feed_cap: f64,
) -> PlotPoints<'static> {
    // The wedge has two sides:
    //   lower: feed = cl_lo * rpm * flutes
    //   upper: feed = cl_hi * rpm * flutes
    // We clip both at rpm_max and feed_cap.
    let lo_line = clip_iso_line(cl_lo, rpm_max, flutes, feed_cap);
    let hi_line = clip_iso_line(cl_hi, rpm_max, flutes, feed_cap);
    // Stitch into a polygon: low-line forward, high-line reversed.
    let mut pts: Vec<[f64; 2]> = lo_line;
    pts.extend(hi_line.into_iter().rev());
    PlotPoints::from(pts)
}

fn clip_iso_line(cl: f64, rpm_max: f64, flutes: f64, feed_cap: f64) -> Vec<[f64; 2]> {
    let mut out = vec![[0.0, 0.0]];
    let feed_at_rpm_max = cl * rpm_max * flutes;
    if feed_at_rpm_max <= feed_cap {
        out.push([rpm_max, feed_at_rpm_max]);
    } else {
        // Slope binds before RPM cap — find RPM where feed hits cap.
        let rpm_cross = if cl > 0.0 && flutes > 0.0 {
            feed_cap / (cl * flutes)
        } else {
            rpm_max
        };
        out.push([rpm_cross, feed_cap]);
        out.push([rpm_max, feed_cap]);
    }
    out
}

// ── Drag-to-explore controls (Phase 3) ──────────────────────────────

fn draw_explore_controls(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    let env = &explain.machine;
    let flutes = explain.flute_count.max(1) as f64;
    let active = modal.explore.is_some();
    let mut explore = modal.explore.unwrap_or(crate::state::NomogramExplore {
        rpm: current
            .spindle_rpm
            .map(f64::from)
            .unwrap_or(explain.recommended.rpm),
        feed_mm_min: current.feed_rate_mm_min.max(1.0),
    });
    let initial = explore;
    ui.horizontal(|ui| {
        if !active && ui.button("⊕ Start exploring").clicked() {
            // Activate explore at the current point.
            events.push(AppEvent::SetFeedsExplore(Some(explore)));
            return;
        }
        if active {
            ui.label(
                egui::RichText::new("Explore:")
                    .small()
                    .strong()
                    .color(theme::TEXT_STRONG),
            );
            ui.add(
                egui::Slider::new(&mut explore.rpm, env.spindle_min_rpm..=env.spindle_max_rpm)
                    .text("RPM")
                    .integer(),
            );
            ui.add(
                egui::Slider::new(&mut explore.feed_mm_min, 0.0..=env.max_feed_mm_min)
                    .text("feed mm/min")
                    .step_by(10.0),
            );
        }
    });
    if active {
        let preview_chipload = if explore.rpm > 0.0 {
            explore.feed_mm_min / (explore.rpm * flutes)
        } else {
            0.0
        };
        let (verdict_text, color) = chipload_verdict(preview_chipload, explain);
        let preview_power = preview_power_kw(explain, explore.feed_mm_min);
        let power_pct = if env.max_power_kw > 0.0 {
            (preview_power / env.max_power_kw).clamp(0.0, 2.0) * 100.0
        } else {
            0.0
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "→ commanded advance/tooth {preview_chipload:.4} mm/tooth · {verdict_text}"
                ))
                .small()
                .color(color),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "power {preview_power:.2} kW ({power_pct:.0}% of cap)"
                ))
                .small()
                .color(if power_pct > 90.0 {
                    theme::ERROR
                } else if power_pct > 70.0 {
                    theme::WARNING_MILD
                } else {
                    theme::SUCCESS
                }),
            );
        });
        ui.horizontal(|ui| {
            // I-3: the explore apply is an apply affordance like any other, so
            // a refused pairing replaces it with the refusal rather than
            // offering a write the panel would never offer. Exploring the
            // nomogram stays available — reading it is not writing.
            if let Some(err) = refusal {
                ui.label(
                    egui::RichText::new(format!("Cannot apply — {err}"))
                        .small()
                        .color(theme::ERROR),
                );
            } else if ui
                .button("✓ Apply explored values")
                .on_hover_text(
                    "Overwrite feed and RPM with the explored values, after the safety \
                     clamps (plunge is pulled down to the new feed if it would exceed it). \
                     Does not change the cut (DOC/WOC). The explored feed itself is never \
                     re-solved.",
                )
                .clicked()
            {
                events.push(AppEvent::ApplyFeedsExplore {
                    toolpath_id,
                    feed_mm_min: explore.feed_mm_min,
                    rpm: explore.rpm,
                });
                events.push(AppEvent::SetFeedsExplore(None));
            }
            if ui
                .button("⟲ Reset to current")
                .on_hover_text("Snap explored point back to the current operating values.")
                .clicked()
            {
                let snap = crate::state::NomogramExplore {
                    rpm: current
                        .spindle_rpm
                        .map(f64::from)
                        .unwrap_or(explain.recommended.rpm),
                    feed_mm_min: current.feed_rate_mm_min.max(1.0),
                };
                events.push(AppEvent::SetFeedsExplore(Some(snap)));
            }
            if ui
                .button("→ Snap to recommended")
                .on_hover_text("Snap explored point to the recommendation.")
                .clicked()
            {
                let snap = crate::state::NomogramExplore {
                    rpm: explain.recommended.rpm,
                    feed_mm_min: explain.recommended.feed_rate_mm_min,
                };
                events.push(AppEvent::SetFeedsExplore(Some(snap)));
            }
            if ui.button("✕ Close explore").clicked() {
                events.push(AppEvent::SetFeedsExplore(None));
            }
        });
        // Persist slider edits — only emit when something actually
        // changed to avoid event spam per frame.
        if (explore.rpm - initial.rpm).abs() > 0.5
            || (explore.feed_mm_min - initial.feed_mm_min).abs() > 0.5
        {
            events.push(AppEvent::SetFeedsExplore(Some(explore)));
        }
    }
}

/// Estimate power at the explored feed by scaling the recommendation's
/// power linearly with feed. Good enough as an interactive preview —
/// not the same as the calc, but matches the optimizer's first-order
/// model for adaptive ops.
fn preview_power_kw(explain: &FeedsExplain, explore_feed: f64) -> f64 {
    let rec = &explain.recommended;
    if rec.feed_rate_mm_min <= 0.0 {
        return 0.0;
    }
    (rec.power_kw * (explore_feed / rec.feed_rate_mm_min)).max(0.0)
}

/// Advance/tooth for the nomogram's hover readout — **display only**.
///
/// The bug this exists to hold (ledgered in
/// `planning/review_2026-08-04/ORCHESTRATION_LOG.md`, W10-LV item 10, as
/// a cosmetic wart): the readout printed the RPM and feed through
/// `.max(0.0)` but divided the RAW pointer coordinates, so dragging
/// below the feed axis produced a line that read
/// `18000 RPM · 0 mm/min → −0.0928 mm/tooth`. Three quantities on one
/// line, two of them clamped and one not — a negative advance per tooth
/// is not a thing, and the number did not correspond to the RPM and feed
/// printed beside it.
///
/// The fix is agreement, not a second clamp: the caller clamps once and
/// this function is handed the same values the label prints. Zero RPM
/// (or zero flutes) yields `0.0` rather than an infinity, which is what
/// the pre-existing `hover.x > 0.0` guard already did.
///
/// **Nothing downstream reads this.** It formats a tooltip; no recipe,
/// gate or export consumes it.
fn hover_readout_chipload(rpm: f64, feed_mm_min: f64, flutes: f64) -> f64 {
    let divisor = rpm * flutes;
    if divisor > 0.0 {
        (feed_mm_min / divisor).max(0.0)
    } else {
        0.0
    }
}

fn chipload_verdict(cl: f64, explain: &FeedsExplain) -> (&'static str, egui::Color32) {
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });
    let Some((lo, hi)) = band else {
        return ("no band", theme::TEXT_DIM);
    };
    if cl < lo * 0.95 {
        ("BURN risk (below band)", theme::ERROR)
    } else if cl < lo {
        ("below band (5% admit)", theme::WARNING_MILD)
    } else if cl <= hi {
        ("within band", theme::SUCCESS)
    } else if cl <= hi * 1.05 {
        ("above band (5% admit)", theme::WARNING_MILD)
    } else {
        ("BREAK risk (above band)", theme::ERROR)
    }
}

// ────────────────────────────────────────────────────────────────────
// Chart A — Advance/tooth vs Diameter
// ────────────────────────────────────────────────────────────────────

fn draw_chart_a(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    ui.label(
        egui::RichText::new("Advance/tooth vs Diameter")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    let rows = explain.rows_by_diameter();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No matching vendor rows for this material.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    // Pre-compute the raw vendor min/max curves so the legend below the
    // chart can show numeric ranges.
    let mut min_pts: Vec<[f64; 2]> = Vec::new();
    let mut max_pts: Vec<[f64; 2]> = Vec::new();
    for row in &rows {
        let d = row.row_diameter_mm;
        // Reverse the diameter scaling so the chart shows the raw
        // vendor row values, not the scaled-for-this-tool values. The
        // scaling factor is the same across all rows in the cohort
        // (same query hardness).
        let raw_min = row.chip_load_min_mm.unwrap_or(0.0) / row.chipload_diameter_scale.max(1e-6);
        let raw_max = row.chip_load_max_mm.unwrap_or(0.0) / row.chipload_diameter_scale.max(1e-6);
        if raw_min > 0.0 {
            min_pts.push([d, raw_min]);
        }
        if raw_max > 0.0 {
            max_pts.push([d, raw_max]);
        }
    }
    min_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    max_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));

    let band_color = egui::Color32::from_rgba_unmultiplied(80, 180, 80, 50);
    let min_color = egui::Color32::from_rgb(180, 130, 60);
    let max_color = egui::Color32::from_rgb(200, 90, 90);

    Plot::new("feeds_modal_chart_a")
        .height(180.0)
        .width(360.0)
        .x_axis_label("diameter (mm)")
        .y_axis_label("commanded advance/tooth (mm)")
        .show(ui, |plot_ui| {
            // Shaded band between min and max — only when both curves
            // share at least two diameters (otherwise the polygon is
            // degenerate and adds visual noise).
            if min_pts.len() >= 2 && max_pts.len() >= 2 {
                let mut band_poly: Vec<[f64; 2]> = min_pts.clone();
                band_poly.extend(max_pts.iter().rev().copied());
                plot_ui.polygon(
                    Polygon::new("", PlotPoints::from(band_poly))
                        .fill_color(band_color)
                        .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                        .name("vendor band (this material)"),
                );
            }
            plot_ui.line(
                Line::new("", PlotPoints::from(min_pts.clone()))
                    .color(min_color)
                    .width(1.5)
                    .name("vendor min"),
            );
            plot_ui.line(
                Line::new("", PlotPoints::from(max_pts.clone()))
                    .color(max_color)
                    .width(1.5)
                    .name("vendor max"),
            );
            plot_ui.points(
                Points::new("", min_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0)
                    .color(min_color),
            );
            plot_ui.points(
                Points::new("", max_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0)
                    .color(max_color),
            );
            // Inline value labels next to each calibrated diameter so
            // the user can read the band values straight from the chart.
            for p in &min_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(min_color),
                    )
                    .anchor(egui::Align2::CENTER_TOP),
                );
            }
            for p in &max_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(max_color),
                    )
                    .anchor(egui::Align2::CENTER_BOTTOM),
                );
            }

            // Your tool vertical line spanning the band height.
            let y_top = max_pts.iter().map(|p| p[1]).fold(0.0_f64, f64::max) * 1.15;
            plot_ui.line(
                Line::new(
                    "",
                    PlotPoints::from(vec![
                        [explain.tool_diameter_mm, 0.0],
                        [explain.tool_diameter_mm, y_top.max(0.05)],
                    ]),
                )
                .color(theme::TEXT_DIM)
                .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                .name(format!("your tool {:.2} mm", explain.tool_diameter_mm)),
            );
            plot_ui.text(
                egui_plot::Text::new(
                    "",
                    egui_plot::PlotPoint::new(explain.tool_diameter_mm, y_top.max(0.05)),
                    egui::RichText::new(format!(" {:.2} mm", explain.tool_diameter_mm))
                        .small()
                        .color(theme::TEXT_DIM),
                )
                .anchor(egui::Align2::LEFT_TOP),
            );

            // Current and recommended chipload at your diameter.
            plot_ui.points(
                Points::new("", vec![[explain.tool_diameter_mm, current.chipload_mm()]])
                    .shape(MarkerShape::Circle)
                    .filled(true)
                    .radius(5.0)
                    .color(theme::ERROR)
                    .name("Current"),
            );
            plot_ui.points(
                Points::new(
                    "",
                    vec![[explain.tool_diameter_mm, explain.recommended.chip_load_mm]],
                )
                .shape(MarkerShape::Diamond)
                .filled(true)
                .radius(5.0)
                .color(egui::Color32::from_rgb(100, 160, 240))
                .name("Recommended"),
            );
        });

    // Per-chart band summary.
    let cl_min_for_your_tool = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm);
    let cl_max_for_your_tool = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_max_mm);
    let band_text = match (cl_min_for_your_tool, cl_max_for_your_tool) {
        (Some(lo), Some(hi)) => format!(
            "{lo:.4}–{hi:.4} mm/tooth at your {:.2} mm tool",
            explain.tool_diameter_mm
        ),
        (None, Some(hi)) => format!("≤ {hi:.4} mm/tooth at your tool"),
        _ => "no scaled band available".to_owned(),
    };
    draw_mini_chart_legend(
        ui,
        "feeds_modal_chart_a_legend",
        &[
            MiniLegend {
                swatch: LegendSwatch::FilledSquare,
                color: band_color,
                label: "vendor band",
                value: band_text,
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: min_color,
                label: "vendor min line",
                value: format!("{} calibrated diameters", min_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: max_color,
                label: "vendor max line",
                value: format!("{} calibrated diameters", max_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Circle,
                color: theme::ERROR,
                label: "● current",
                value: format!("{:.4} mm/tooth", current.chipload_mm()),
            },
            MiniLegend {
                swatch: LegendSwatch::Diamond,
                color: egui::Color32::from_rgb(100, 160, 240),
                label: "◆ recommended",
                value: format!("{:.4} mm/tooth", explain.recommended.chip_load_mm),
            },
        ],
    );
}

// ────────────────────────────────────────────────────────────────────
// Chart B — Advance/tooth vs Hardness
// ────────────────────────────────────────────────────────────────────

/// Mini-legend row used under Charts A and B.
struct MiniLegend {
    swatch: LegendSwatch,
    color: egui::Color32,
    label: &'static str,
    value: String,
}

fn draw_mini_chart_legend(ui: &mut egui::Ui, id: &str, entries: &[MiniLegend]) {
    ui.add_space(2.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new(id)
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for entry in entries {
                    ui.horizontal(|ui| {
                        draw_legend_swatch(ui, entry.swatch, entry.color);
                        ui.label(
                            egui::RichText::new(entry.label)
                                .small()
                                .color(theme::TEXT_STRONG),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&entry.value)
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.end_row();
                }
            });
    });
}

fn draw_chart_b(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    ui.label(
        egui::RichText::new("Advance/tooth vs Hardness")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    let rows = explain.rows_by_hardness();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No matching vendor rows at this diameter.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    // Each sibling row was matched with a hardness_scale of
    // (row.hardness / query.hardness). To plot against actual row
    // hardness we recover it from `query_hardness_value` × scale.
    let q_hardness = explain.query_hardness_value.unwrap_or(0.0);

    let band_color = egui::Color32::from_rgba_unmultiplied(80, 180, 80, 50);
    let min_color = egui::Color32::from_rgb(180, 130, 60);
    let max_color = egui::Color32::from_rgb(200, 90, 90);
    let mut min_pts: Vec<[f64; 2]> = Vec::new();
    let mut max_pts: Vec<[f64; 2]> = Vec::new();
    for row in &rows {
        let row_hardness = if row.chipload_hardness_scale > 0.0 {
            q_hardness * row.chipload_hardness_scale
        } else {
            q_hardness
        };
        let raw_min = row.chip_load_min_mm.unwrap_or(0.0) / row.chipload_hardness_scale.max(1e-6);
        let raw_max = row.chip_load_max_mm.unwrap_or(0.0) / row.chipload_hardness_scale.max(1e-6);
        if raw_min > 0.0 {
            min_pts.push([row_hardness, raw_min]);
        }
        if raw_max > 0.0 {
            max_pts.push([row_hardness, raw_max]);
        }
    }
    min_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    max_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));

    Plot::new("feeds_modal_chart_b")
        .height(180.0)
        .width(360.0)
        .x_axis_label(hardness_axis_label(explain.query_hardness_kind))
        .y_axis_label("commanded advance/tooth (mm)")
        .show(ui, |plot_ui| {
            if min_pts.len() >= 2 && max_pts.len() >= 2 {
                let mut band_poly: Vec<[f64; 2]> = min_pts.clone();
                band_poly.extend(max_pts.iter().rev().copied());
                plot_ui.polygon(
                    Polygon::new("", PlotPoints::from(band_poly))
                        .fill_color(band_color)
                        .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                        .name("vendor band (this diameter)"),
                );
            }
            plot_ui.line(
                Line::new("", PlotPoints::from(min_pts.clone()))
                    .color(min_color)
                    .width(1.5)
                    .name("vendor min"),
            );
            plot_ui.line(
                Line::new("", PlotPoints::from(max_pts.clone()))
                    .color(max_color)
                    .width(1.5)
                    .name("vendor max"),
            );
            plot_ui.points(
                Points::new("", min_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0)
                    .color(min_color),
            );
            plot_ui.points(
                Points::new("", max_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0)
                    .color(max_color),
            );
            // Inline value tags.
            for p in &min_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(min_color),
                    )
                    .anchor(egui::Align2::CENTER_TOP),
                );
            }
            for p in &max_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(max_color),
                    )
                    .anchor(egui::Align2::CENTER_BOTTOM),
                );
            }

            // Your material vertical line spanning the band height.
            let y_top = max_pts.iter().map(|p| p[1]).fold(0.0_f64, f64::max) * 1.15;
            if q_hardness > 0.0 {
                plot_ui.line(
                    Line::new(
                        "",
                        PlotPoints::from(vec![[q_hardness, 0.0], [q_hardness, y_top.max(0.05)]]),
                    )
                    .color(theme::TEXT_DIM)
                    .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                    .name(format!("your material ({q_hardness:.0})")),
                );
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(q_hardness, y_top.max(0.05)),
                        egui::RichText::new(format!(" {q_hardness:.0}"))
                            .small()
                            .color(theme::TEXT_DIM),
                    )
                    .anchor(egui::Align2::LEFT_TOP),
                );
            }
            // Current and recommended.
            plot_ui.points(
                Points::new("", vec![[q_hardness, current.chipload_mm()]])
                    .shape(MarkerShape::Circle)
                    .filled(true)
                    .radius(5.0)
                    .color(theme::ERROR)
                    .name("Current"),
            );
            plot_ui.points(
                Points::new("", vec![[q_hardness, explain.recommended.chip_load_mm]])
                    .shape(MarkerShape::Diamond)
                    .filled(true)
                    .radius(5.0)
                    .color(egui::Color32::from_rgb(100, 160, 240))
                    .name("Recommended"),
            );
        });

    let cl_min = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm);
    let cl_max = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_max_mm);
    let band_text = match (cl_min, cl_max) {
        (Some(lo), Some(hi)) => {
            format!("{lo:.4}–{hi:.4} mm/tooth at your material ({q_hardness:.0})")
        }
        (None, Some(hi)) => format!("≤ {hi:.4} mm/tooth at your material"),
        _ => "no scaled band available".to_owned(),
    };
    draw_mini_chart_legend(
        ui,
        "feeds_modal_chart_b_legend",
        &[
            MiniLegend {
                swatch: LegendSwatch::FilledSquare,
                color: band_color,
                label: "vendor band",
                value: band_text,
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: min_color,
                label: "vendor min line",
                value: format!("{} materials sampled", min_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: max_color,
                label: "vendor max line",
                value: format!("{} materials sampled", max_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Circle,
                color: theme::ERROR,
                label: "● current",
                value: format!("{:.4} mm/tooth", current.chipload_mm()),
            },
            MiniLegend {
                swatch: LegendSwatch::Diamond,
                color: egui::Color32::from_rgb(100, 160, 240),
                label: "◆ recommended",
                value: format!("{:.4} mm/tooth", explain.recommended.chip_load_mm),
            },
        ],
    );
}

fn hardness_axis_label(kind: Option<HardnessKind>) -> &'static str {
    match kind {
        Some(HardnessKind::Janka) => "Janka (lbf)",
        Some(HardnessKind::ShoreD) => "Shore D",
        Some(HardnessKind::Hb) => "Brinell (HB)",
        None => "hardness",
    }
}

// ────────────────────────────────────────────────────────────────────
// Project rollup (Phase 4)
// ────────────────────────────────────────────────────────────────────

fn draw_project_view(
    ui: &mut egui::Ui,
    state: &AppState,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    let mut rows: Vec<ProjectFeedsRow> = state
        .session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .filter_map(|tc| {
            let current = read_current_values(state, tc.id)?;
            let preview = compute_preview(state, tc.id)?;
            let refusal = preview.refusal().map(ToString::to_string);
            Some(ProjectFeedsRow {
                id: tc.id,
                name: tc.name.clone(),
                current,
                explain: preview.explain().clone(),
                refusal,
            })
        })
        .collect();
    let sort = modal.project_sort;
    match sort {
        ProjectFeedsSort::Index => {}
        ProjectFeedsSort::Speedup => {
            rows.sort_by(|a, b| {
                speedup(&b.current, &b.explain).total_cmp(&speedup(&a.current, &a.explain))
            });
        }
        ProjectFeedsSort::Name => {
            rows.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No enabled toolpaths.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }

    // ── Bottleneck callout ──────────────────────────────────────────
    draw_bottleneck_callout(ui, &rows);
    ui.add_space(6.0);

    // ── Project scatter overlay ────────────────────────────────────
    ui.horizontal(|ui| {
        let mut show_scatter = modal.project_show_scatter;
        if ui
            .checkbox(&mut show_scatter, "Show feed-RPM scatter")
            .on_hover_text(
                "Overlay every toolpath's current→recommended move on a single feed-RPM chart.",
            )
            .changed()
        {
            events.push(AppEvent::SetFeedsProjectScatter(show_scatter));
        }
    });
    if modal.project_show_scatter {
        draw_project_scatter(ui, &rows);
        ui.add_space(6.0);
    }

    // ── Toolbar: sort + select-all + apply ─────────────────────────
    let selected_count = modal.project_selected.len();
    let total = rows.len();
    ui.horizontal(|ui| {
        let all_selected = selected_count == total && total > 0;
        let any_selected = selected_count > 0;
        if ui
            .checkbox(
                &mut { all_selected },
                format!("Select all ({selected_count}/{total})"),
            )
            .clicked()
        {
            events.push(AppEvent::SetFeedsProjectSelectAll(!all_selected));
        }
        ui.separator();
        let mut apply_btn = ui.add_enabled(
            any_selected,
            egui::Button::new("⚡ Apply selected — changes the cut"),
        );
        apply_btn = apply_btn.on_hover_text(
            "Apply Feeds recommendations to every checked toolpath. \
             CHANGES THE CUT: DOC and WOC move as well as the speeds. \
             Rows whose tool cannot run their operation are skipped and reported.",
        );
        if apply_btn.clicked() {
            events.push(AppEvent::ApplyFeedsProjectSelected);
        }
        if ui
            .button("⚡⚡ Apply all toolpaths — changes the cut")
            .on_hover_text(
                "Apply to every enabled toolpath regardless of selection. \
                 CHANGES THE CUT: DOC and WOC move as well as the speeds. \
                 Rows whose tool cannot run their operation are skipped and reported.",
            )
            .clicked()
        {
            events.push(AppEvent::ApplyFeedsProject);
        }
        ui.separator();
        ui.label(egui::RichText::new("Sort:").small().color(theme::TEXT_DIM));
        if ui
            .selectable_label(sort == ProjectFeedsSort::Index, "Order")
            .clicked()
        {
            events.push(AppEvent::SetFeedsProjectSort(ProjectFeedsSort::Index));
        }
        if ui
            .selectable_label(sort == ProjectFeedsSort::Speedup, "Speedup")
            .clicked()
        {
            events.push(AppEvent::SetFeedsProjectSort(ProjectFeedsSort::Speedup));
        }
        if ui
            .selectable_label(sort == ProjectFeedsSort::Name, "Name")
            .clicked()
        {
            events.push(AppEvent::SetFeedsProjectSort(ProjectFeedsSort::Name));
        }
    });

    ui.add_space(4.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("feeds_modal_project_table")
            .num_columns(9)
            .spacing([10.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for h in [
                    "", "Toolpath", "Feed", "Rec feed", "Δ", "DOC", "Rec DOC", "WOC", "Apply",
                ] {
                    ui.label(
                        egui::RichText::new(h)
                            .small()
                            .strong()
                            .color(theme::TEXT_DIM),
                    );
                }
                ui.end_row();

                for r in &rows {
                    let mut checked = modal.project_selected.contains(&r.id);
                    if ui.checkbox(&mut checked, "").changed() {
                        events.push(AppEvent::ToggleFeedsProjectRow(r.id));
                    }
                    match &r.refusal {
                        Some(why) => {
                            ui.label(
                                egui::RichText::new(format!("{} ⚠", r.name))
                                    .small()
                                    .color(theme::WARNING),
                            )
                            .on_hover_text(format!(
                                "This tool cannot run this operation, so no apply — batch or \
                                 single — will write to it.\n{why}"
                            ));
                        }
                        None => {
                            ui.label(egui::RichText::new(&r.name).small());
                        }
                    }
                    ui.label(format!("{:.0}", r.current.feed_rate_mm_min));
                    ui.label(format!("{:.0}", r.explain.recommended.feed_rate_mm_min));
                    ui.label(compare::delta_tag(
                        Some(r.current.feed_rate_mm_min),
                        Some(r.explain.recommended.feed_rate_mm_min),
                    ));
                    ui.label(compare::format_optional(r.current.depth_per_pass, "", 0.01));
                    ui.label(format!("{:.2}", r.explain.recommended.axial_depth_mm));
                    ui.label(compare::format_optional(r.current.stepover, "", 0.01));
                    match &r.refusal {
                        Some(why) => {
                            ui.label(egui::RichText::new("refused").small().color(theme::ERROR))
                                .on_hover_text(why);
                        }
                        None => {
                            if ui
                                .small_button("Apply")
                                .on_hover_text(
                                    "Apply the recommendation to this toolpath. \
                                     CHANGES THE CUT (DOC/WOC) as well as the speeds.",
                                )
                                .clicked()
                            {
                                events.push(AppEvent::ApplyFeedsAll(r.id));
                            }
                        }
                    }
                    ui.end_row();
                }
            });
    });
}

/// Find the toolpath with the largest feed-speedup potential and
/// surface it as a one-line callout. Skips toolpaths whose current
/// feed is already at or above the recommendation.
fn draw_bottleneck_callout(ui: &mut egui::Ui, rows: &[ProjectFeedsRow]) {
    let bottleneck = rows
        .iter()
        .filter_map(|r| {
            let s = speedup(&r.current, &r.explain);
            if s > 1.05 { Some((r, s)) } else { None }
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));

    egui::Frame::group(ui.style()).show(ui, |ui| match bottleneck {
        Some((row, s)) => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Biggest opportunity:")
                        .small()
                        .strong()
                        .color(theme::TEXT_STRONG),
                );
                ui.label(
                    egui::RichText::new(&row.name)
                        .small()
                        .color(theme::TEXT_STRONG),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "feed {:.0} → {:.0} mm/min ({:.2}× speedup)",
                        row.current.feed_rate_mm_min,
                        row.explain.recommended.feed_rate_mm_min,
                        s
                    ))
                    .small()
                    .color(theme::SUCCESS),
                );
            });
            // Aggregate cycle-time gain estimate — sum of inverse speedups.
            let agg = aggregate_speedup(rows);
            if agg > 1.05 {
                ui.label(
                    egui::RichText::new(format!(
                        "Estimated project speedup if all applied: {agg:.2}×"
                    ))
                    .small()
                    .color(theme::TEXT_DIM),
                );
            }
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "Every toolpath is already at or above its recommendation — no project-wide gain available.",
                )
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    });
}

/// Crude project speedup estimate: weighted by feed-rate inverse
/// (treats faster feeds as proxy for cycle-time saved). Just for
/// the callout — the real number requires a sim.
fn aggregate_speedup(rows: &[ProjectFeedsRow]) -> f64 {
    if rows.is_empty() {
        return 1.0;
    }
    let mut t_current = 0.0;
    let mut t_recommended = 0.0;
    for r in rows {
        if r.current.feed_rate_mm_min > 0.0 {
            t_current += 1.0 / r.current.feed_rate_mm_min;
        }
        if r.explain.recommended.feed_rate_mm_min > 0.0 {
            t_recommended += 1.0 / r.explain.recommended.feed_rate_mm_min;
        }
    }
    if t_recommended > 0.0 {
        t_current / t_recommended
    } else {
        1.0
    }
}

/// Scatter chart showing every toolpath on the feed-RPM plane.
/// Current = red ●, Recommended = blue ◆, line connecting the two.
fn draw_project_scatter(ui: &mut egui::Ui, rows: &[ProjectFeedsRow]) {
    if rows.is_empty() {
        return;
    }
    let max_rpm = rows
        .iter()
        .filter_map(|r| r.current.spindle_rpm.map(f64::from))
        .chain(rows.iter().map(|r| r.explain.recommended.rpm))
        .chain(rows.iter().map(|r| r.explain.machine.spindle_max_rpm))
        .fold(0.0_f64, f64::max)
        * 1.1;
    let max_feed = rows
        .iter()
        .map(|r| r.current.feed_rate_mm_min)
        .chain(rows.iter().map(|r| r.explain.recommended.feed_rate_mm_min))
        .chain(rows.iter().map(|r| r.explain.machine.max_feed_mm_min))
        .fold(0.0_f64, f64::max)
        * 1.05;

    // The machine envelope is the same for every toolpath (project-wide
    // machine profile), so we take it from the first row.
    let env_opt = rows.first().map(|r| r.explain.machine.clone());

    Plot::new("feeds_modal_project_scatter")
        .height(220.0)
        .include_x(0.0)
        .include_y(0.0)
        .include_x(max_rpm)
        .include_y(max_feed)
        .x_axis_label("RPM")
        .y_axis_label("feed (mm/min)")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            // Machine envelope — forbidden zones + walls, same grammar as Chart C.
            if let Some(env) = env_opt.as_ref() {
                draw_machine_envelope(plot_ui, env, max_rpm, max_feed);
            }
            for r in rows {
                let cur = match r.current.spindle_rpm {
                    Some(rpm) if rpm > 0 => Some([f64::from(rpm), r.current.feed_rate_mm_min]),
                    _ => None,
                };
                let rec = [
                    r.explain.recommended.rpm,
                    r.explain.recommended.feed_rate_mm_min,
                ];
                if let Some(c) = cur {
                    // Connecting arrow line.
                    plot_ui.line(
                        Line::new("", PlotPoints::from(vec![c, rec]))
                            .color(egui::Color32::from_rgba_unmultiplied(140, 140, 160, 140))
                            .width(1.0),
                    );
                    plot_ui.points(
                        Points::new("", vec![c])
                            .shape(MarkerShape::Circle)
                            .filled(true)
                            .radius(4.0)
                            .color(theme::ERROR),
                    );
                }
                plot_ui.points(
                    Points::new("", vec![rec])
                        .shape(MarkerShape::Diamond)
                        .filled(true)
                        .radius(4.5)
                        .color(egui::Color32::from_rgb(100, 160, 240)),
                );
            }
        });
}

/// Render the shared machine-envelope overlay used by Chart C and the
/// project scatter: shaded forbidden zones beyond max RPM and max feed
/// (plus the left-side dim zone below spindle min RPM when non-zero),
/// solid red borders on the cap walls, and value labels.
fn draw_machine_envelope(
    plot_ui: &mut egui_plot::PlotUi,
    env: &rs_cam_core::feeds::MachineEnvelope,
    axis_rpm_max: f64,
    axis_feed_max: f64,
) {
    let forbidden_fill = egui::Color32::from_rgba_unmultiplied(200, 90, 90, 35);
    let forbidden_edge = egui::Color32::from_rgba_unmultiplied(200, 90, 90, 200);

    if axis_rpm_max > env.spindle_max_rpm {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [env.spindle_max_rpm, 0.0],
                    [axis_rpm_max, 0.0],
                    [axis_rpm_max, axis_feed_max],
                    [env.spindle_max_rpm, axis_feed_max],
                ]),
            )
            .fill_color(forbidden_fill)
            .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
            .name(format!(
                "Past machine cap ({} RPM)",
                env.spindle_max_rpm as i64
            )),
        );
    }
    if axis_feed_max > env.max_feed_mm_min {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [0.0, env.max_feed_mm_min],
                    [axis_rpm_max, env.max_feed_mm_min],
                    [axis_rpm_max, axis_feed_max],
                    [0.0, axis_feed_max],
                ]),
            )
            .fill_color(forbidden_fill)
            .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
            .name(format!(
                "Past machine feed ({} mm/min)",
                env.max_feed_mm_min as i64
            )),
        );
    }
    if env.spindle_min_rpm > 0.0 {
        plot_ui.polygon(
            Polygon::new(
                "",
                PlotPoints::from(vec![
                    [0.0, 0.0],
                    [env.spindle_min_rpm, 0.0],
                    [env.spindle_min_rpm, axis_feed_max],
                    [0.0, axis_feed_max],
                ]),
            )
            .fill_color(egui::Color32::from_rgba_unmultiplied(150, 150, 160, 25))
            .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
            .name(format!(
                "Below spindle min ({} RPM)",
                env.spindle_min_rpm as i64
            )),
        );
        plot_ui.line(
            Line::new(
                "",
                PlotPoints::from(vec![
                    [env.spindle_min_rpm, 0.0],
                    [env.spindle_min_rpm, axis_feed_max],
                ]),
            )
            .color(egui::Color32::from_rgb(150, 150, 160))
            .width(1.5)
            .name(format!("spindle min {} RPM", env.spindle_min_rpm as i64)),
        );
    }
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![
                [env.spindle_max_rpm, 0.0],
                [env.spindle_max_rpm, axis_feed_max],
            ]),
        )
        .color(forbidden_edge)
        .width(2.0)
        .name(format!("machine max {} RPM", env.spindle_max_rpm as i64)),
    );
    plot_ui.line(
        Line::new(
            "",
            PlotPoints::from(vec![
                [0.0, env.max_feed_mm_min],
                [axis_rpm_max, env.max_feed_mm_min],
            ]),
        )
        .color(forbidden_edge)
        .width(2.0)
        .name(format!(
            "machine max feed {} mm/min",
            env.max_feed_mm_min as i64
        )),
    );
    plot_ui.text(
        egui_plot::Text::new(
            "",
            egui_plot::PlotPoint::new(env.spindle_max_rpm, axis_feed_max * 0.97),
            egui::RichText::new(format!("max {} RPM ⬢", env.spindle_max_rpm as i64))
                .small()
                .color(forbidden_edge),
        )
        .anchor(egui::Align2::RIGHT_TOP),
    );
    plot_ui.text(
        egui_plot::Text::new(
            "",
            egui_plot::PlotPoint::new(axis_rpm_max * 0.97, env.max_feed_mm_min),
            egui::RichText::new(format!("max {} mm/min ⬢", env.max_feed_mm_min as i64))
                .small()
                .color(forbidden_edge),
        )
        .anchor(egui::Align2::RIGHT_BOTTOM),
    );
}

struct ProjectFeedsRow {
    id: rs_cam_core::ToolpathId,
    name: String,
    current: CurrentValues,
    explain: FeedsExplain,
    /// `Some` when `validate_tool_for_operation` refuses this row's tool ×
    /// operation pairing. Pre-fix (A-3 §3.5) the rollup had no idea: the row
    /// rendered a recommendation and an Apply button like any other, and
    /// `⚡⚡ Apply all toolpaths` swept the refused row up **silently**. The
    /// batch handler now skips it and says so; the table marks it.
    refusal: Option<String>,
}

fn speedup(current: &CurrentValues, explain: &FeedsExplain) -> f64 {
    if current.feed_rate_mm_min <= 0.0 {
        return 1.0;
    }
    explain.recommended.feed_rate_mm_min / current.feed_rate_mm_min
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::hover_readout_chipload;

    /// The nomogram hover readout must never print a negative advance
    /// per tooth.
    ///
    /// Pre-fix reproduction, preserved as the first case: the readout
    /// divided the RAW pointer coordinates while printing the CLAMPED
    /// ones, so a drag below the feed axis at 18000 RPM on a 2-flute
    /// cutter printed `18000 RPM · 0 mm/min → −0.0928 mm/tooth`
    /// (W10-LV item 10). The literal −0.0928 is reproduced here from the
    /// feed that produces it, so the case is the observed one and not a
    /// paraphrase: −0.0928 × 18000 × 2 = −3340.8 mm/min.
    ///
    /// Display-only: this function formats a tooltip. No recipe number
    /// moves.
    #[test]
    fn hover_readout_never_prints_a_negative_chipload() {
        // The exact reported reading, through the pre-fix arithmetic.
        let raw: f64 = -3340.8 / (18000.0 * 2.0);
        assert!(
            (raw - -0.0928).abs() < 1e-4,
            "fixture must reproduce the reported −0.0928 mm/tooth, got {raw}"
        );

        // Same pointer position, through the shipped path: the caller
        // clamps the feed to 0 and hands this function the same value it
        // prints.
        assert_eq!(hover_readout_chipload(18000.0, 0.0, 2.0), 0.0);

        // And the clamp is defended at the function too, so a caller
        // that forgets cannot resurrect the wart.
        assert_eq!(hover_readout_chipload(18000.0, -3340.8, 2.0), 0.0);

        // Zero RPM / zero flutes must be 0.0, not an infinity or a NaN —
        // both reach `{:.4}` in the label.
        assert_eq!(hover_readout_chipload(0.0, 2520.0, 2.0), 0.0);
        assert_eq!(hover_readout_chipload(0.0, 0.0, 2.0), 0.0);
        assert_eq!(hover_readout_chipload(18000.0, 2520.0, 0.0), 0.0);

        // The ordinary case is untouched: 2520 / (18000 × 2) = 0.07.
        let ok = hover_readout_chipload(18000.0, 2520.0, 2.0);
        assert!((ok - 0.07).abs() < 1e-12, "got {ok}");
    }
}
